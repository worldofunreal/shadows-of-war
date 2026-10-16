//! sow-relay — one F-Stack worker process over the DPDK bridge.
//!
//! Each lobby owns one dynamic TCP port in 1024..=65535. The destination port
//! modulo the number of F-Stack queues selects the owning worker. Lobbies are
//! registered by sow-server via that worker's kernel management port (HTTPS in
//! production):
//!
//!     POST /internal/lobby/register   {lobby_id, tick_number, tick_rate_ms,
//!                                      players, config?}
//!     GET  /internal/lobbies          active lobby roster (ops/validation)
//!
//! `SOW_RELAY_BASE_MGMT_PORT` selects the first management port; worker IDs
//! increment it for subsequent workers.
//! selects the management port; game listeners are created dynamically when
//! the owning worker accepts a lobby registration.
//!
//! Connections arrive via RSS on the game port. The first frame decides the
//! role: a ticketed Ready frame → relay player routed to its registered lobby;
//! silence for ORCHESTRATOR_GRACE_SECS → the optional orchestration/operations
//! WS (LobbiesBroadcast feed for bot/backfill compatibility and tooling).
//!
//! The per-lobby game loop (ticks, intents, missed-tick drop, MatchTracker,
//! finalize upload) is the sow-relay logic, now keyed by registry lookup.

use fstack_bridge::bridge::{self, Ev};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use libc::{c_int, c_void};
use log::{error, info, warn};
use rand::{RngCore, rngs::OsRng};
use redis::Commands;
use rustls::{Certificate, PrivateKey, ServerConfig};
use sha2::{Digest, Sha256};
use sow_core::game_config::{BotDifficulty, GameConfig};
use sow_core::player::Leader;
use sow_core::protocol::{
    ClientMessage, ExitReason, GameplayIntent, LobbyInfo, LobbyKind, LobbyPlayerSyncState,
    ServerLobbiesBroadcastMessage, ServerLobbyClosedMessage, ServerMessage, ServerTurnMessage,
    StampedIntent, Team, Turn,
};
use sow_core::spawn_rate_limit::SpawnIntentRateLimit;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CString;
use std::io::BufReader;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::task::{Context, Poll};
use std::time::{Instant, SystemTime};
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Mutex, RwLock, Semaphore, mpsc, oneshot};
use tokio::time::{Duration, interval};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{CloseFrame, frame::coding::CloseCode},
};

const DEFAULT_MGMT_PORT: u16 = 8080;
const DYNAMIC_PORT_MIN: u16 = 1024;
const DEFAULT_EXPECTED_WORKER_COUNT: u16 = 4;
const MAX_MGMT_BODY_BYTES: usize = 2 * 1024 * 1024;
const MGMT_CLOCK_SKEW_SECS: u64 = 30;
const MGMT_NONCE_TTL_SECS: u64 = 120;
const DEFAULT_MAX_CONNECTIONS: usize = 32_768;
const DEFAULT_MAX_CONNECTIONS_PER_IP: usize = 4_096;
const DEFAULT_HANDSHAKES_PER_IP: u32 = 512;
const MAX_ADMISSION_IPS: usize = 65_536;
const HANDSHAKE_TIMEOUT_SECS: u64 = 10;
const REPLAY_SPOOL_RETRY_SECS: u64 = 30;
const PARTICIPANT_CHECKPOINT_RETRY_SECS: u64 = 30;
type HmacSha256 = Hmac<Sha256>;

struct IpAdmissionState {
    active: usize,
    window_started: Instant,
    accepted_in_window: u32,
}

struct AdmissionState {
    active: usize,
    by_ip: HashMap<IpAddr, IpAdmissionState>,
}

struct RelayAdmissionPolicy {
    max_connections: usize,
    max_connections_per_ip: usize,
    handshakes_per_ip: u32,
    state: StdMutex<AdmissionState>,
    accepted: AtomicU64,
    rejected_global: AtomicU64,
    rejected_per_ip: AtomicU64,
    rejected_rate: AtomicU64,
    active_peak: AtomicU64,
}

static RELAY_ADMISSION: OnceLock<RelayAdmissionPolicy> = OnceLock::new();

impl RelayAdmissionPolicy {
    fn from_env() -> Result<Self, String> {
        let parse = |name: &str, default: &str| -> Result<usize, String> {
            std::env::var(name)
                .unwrap_or_else(|_| default.to_string())
                .parse::<usize>()
                .map_err(|_| format!("{name} must be a positive integer"))
                .and_then(|value| {
                    if value == 0 {
                        Err(format!("{name} must be a positive integer"))
                    } else {
                        Ok(value)
                    }
                })
        };
        let max_connections = parse(
            "SOW_RELAY_MAX_CONNECTIONS",
            &DEFAULT_MAX_CONNECTIONS.to_string(),
        )?;
        let max_connections_per_ip = parse(
            "SOW_RELAY_MAX_CONNECTIONS_PER_IP",
            &DEFAULT_MAX_CONNECTIONS_PER_IP.to_string(),
        )?;
        let handshakes_per_ip = std::env::var("SOW_RELAY_HANDSHAKES_PER_IP")
            .unwrap_or_else(|_| DEFAULT_HANDSHAKES_PER_IP.to_string())
            .parse::<u32>()
            .map_err(|_| "SOW_RELAY_HANDSHAKES_PER_IP must be a positive integer".to_string())?;
        if max_connections_per_ip > max_connections {
            return Err(
                "SOW_RELAY_MAX_CONNECTIONS_PER_IP must not exceed SOW_RELAY_MAX_CONNECTIONS"
                    .to_string(),
            );
        }
        Ok(Self {
            max_connections,
            max_connections_per_ip,
            handshakes_per_ip,
            state: StdMutex::new(AdmissionState {
                active: 0,
                by_ip: HashMap::new(),
            }),
            accepted: AtomicU64::new(0),
            rejected_global: AtomicU64::new(0),
            rejected_per_ip: AtomicU64::new(0),
            rejected_rate: AtomicU64::new(0),
            active_peak: AtomicU64::new(0),
        })
    }

    fn try_accept(&self, peer: SocketAddr) -> bool {
        let ip = peer.ip();
        let now = Instant::now();
        let mut state = self.state.lock().expect("relay admission mutex poisoned");
        if state.active >= self.max_connections {
            let count = self.rejected_global.fetch_add(1, Ordering::Relaxed) + 1;
            if count.is_power_of_two() {
                warn!(
                    "[admission] global connection cap rejected peer={} count={count}",
                    peer
                );
            }
            return false;
        }
        if !state.by_ip.contains_key(&ip) && state.by_ip.len() >= MAX_ADMISSION_IPS {
            let count = self.rejected_global.fetch_add(1, Ordering::Relaxed) + 1;
            if count.is_power_of_two() {
                warn!(
                    "[admission] IP table cap rejected peer={} count={count}",
                    peer
                );
            }
            return false;
        }
        {
            let entry = state.by_ip.entry(ip).or_insert_with(|| IpAdmissionState {
                active: 0,
                window_started: now,
                accepted_in_window: 0,
            });
            if now.duration_since(entry.window_started) >= Duration::from_secs(1) {
                entry.window_started = now;
                entry.accepted_in_window = 0;
            }
            if entry.active >= self.max_connections_per_ip {
                let count = self.rejected_per_ip.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_power_of_two() {
                    warn!(
                        "[admission] per-IP connection cap rejected peer={} count={count}",
                        peer
                    );
                }
                return false;
            }
            if entry.accepted_in_window >= self.handshakes_per_ip {
                let count = self.rejected_rate.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_power_of_two() {
                    warn!(
                        "[admission] per-IP handshake rate rejected peer={} count={count}",
                        peer
                    );
                }
                return false;
            }
        }
        state.active += 1;
        let entry = state.by_ip.get_mut(&ip).expect("admission entry inserted");
        entry.active += 1;
        entry.accepted_in_window += 1;
        let active = state.active as u64;
        self.accepted.fetch_add(1, Ordering::Relaxed);
        self.active_peak.fetch_max(active, Ordering::Relaxed);
        true
    }

    fn on_close(&self, peer: SocketAddr) {
        let ip = peer.ip();
        let now = Instant::now();
        let mut state = self.state.lock().expect("relay admission mutex poisoned");
        state.active = state.active.saturating_sub(1);
        if let Some(entry) = state.by_ip.get_mut(&ip) {
            entry.active = entry.active.saturating_sub(1);
            if entry.active == 0
                && now.duration_since(entry.window_started) >= Duration::from_secs(1)
            {
                state.by_ip.remove(&ip);
            }
        }
    }

    fn metrics(&self) -> serde_json::Value {
        let state = self.state.lock().expect("relay admission mutex poisoned");
        serde_json::json!({
            "active": state.active,
            "tracked_ips": state.by_ip.len(),
            "max_connections": self.max_connections,
            "max_connections_per_ip": self.max_connections_per_ip,
            "handshakes_per_ip": self.handshakes_per_ip,
            "accepted": self.accepted.load(Ordering::Relaxed),
            "rejected_global": self.rejected_global.load(Ordering::Relaxed),
            "rejected_per_ip": self.rejected_per_ip.load(Ordering::Relaxed),
            "rejected_rate": self.rejected_rate.load(Ordering::Relaxed),
            "active_peak": self.active_peak.load(Ordering::Relaxed),
        })
    }
}

fn relay_accept_filter(peer: SocketAddr) -> bool {
    RELAY_ADMISSION
        .get()
        .is_none_or(|policy| policy.try_accept(peer))
}

fn relay_close_hook(peer: SocketAddr) {
    if let Some(policy) = RELAY_ADMISSION.get() {
        policy.on_close(peer);
    }
}

fn strict_runtime_security() -> bool {
    std::env::var("SOW_MGMT_TLS_REQUIRED").ok().as_deref() == Some("1")
}

fn configured_db_url() -> Result<String, String> {
    let db_url =
        std::env::var("SOW_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let parsed =
        reqwest::Url::parse(&db_url).map_err(|e| format!("invalid SOW_DB_URL={db_url}: {e}"))?;
    if strict_runtime_security() && parsed.scheme() != "https" {
        return Err("SOW_DB_URL must use https when SOW_MGMT_TLS_REQUIRED=1".to_string());
    }
    if let Ok(raw_ip) = std::env::var("SOW_DB_RESOLVE_IP") {
        raw_ip
            .parse::<IpAddr>()
            .map_err(|e| format!("invalid SOW_DB_RESOLVE_IP={raw_ip}: {e}"))?;
    } else if strict_runtime_security() {
        return Err("SOW_DB_RESOLVE_IP must be set when SOW_MGMT_TLS_REQUIRED=1".to_string());
    }
    Ok(db_url)
}

fn db_client(db_url: &str) -> Result<reqwest::Client, String> {
    let parsed =
        reqwest::Url::parse(db_url).map_err(|e| format!("invalid database URL {db_url}: {e}"))?;
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none());
    if let Ok(raw_ip) = std::env::var("SOW_DB_RESOLVE_IP") {
        let ip = raw_ip
            .parse::<IpAddr>()
            .map_err(|e| format!("invalid SOW_DB_RESOLVE_IP={raw_ip}: {e}"))?;
        if let Some(host) = parsed.host_str() {
            if host.parse::<IpAddr>().is_err() {
                let port = parsed
                    .port()
                    .unwrap_or_else(|| if parsed.scheme() == "http" { 80 } else { 443 });
                builder = builder.resolve(host, SocketAddr::new(ip, port));
            }
        }
    }
    builder
        .build()
        .map_err(|e| format!("build database client: {e}"))
}

fn validate_runtime_security() -> Result<(), String> {
    if !strict_runtime_security() {
        return Err("SOW_MGMT_TLS_REQUIRED must be 1; refusing insecure relay startup".to_string());
    }
    configured_db_url().map(|_| ())
}

fn decode_ticket_digest(value: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(value).ok()?;
    bytes.try_into().ok()
}

fn ticket_matches(ticket: &str, expected: &[u8; 32]) -> bool {
    let actual = Sha256::digest(ticket.as_bytes());
    let mut diff = 0u8;
    for (left, right) in actual.iter().zip(expected.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn ticket_is_current(expires_at: u64) -> bool {
    expires_at == 0
        || std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|now| now.as_secs() <= expires_at)
            .unwrap_or(false)
}

fn expected_worker_count() -> u16 {
    std::env::var("SOW_RELAY_WORKER_COUNT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|count| (1..=64).contains(count))
        .unwrap_or(DEFAULT_EXPECTED_WORKER_COUNT)
}

static WORKER_QUEUE_ID: AtomicU16 = AtomicU16::new(0);
static WORKER_QUEUE_COUNT: AtomicU16 = AtomicU16::new(1);
static LISTENER_COUNT: AtomicU64 = AtomicU64::new(0);
static DISPATCH_LOCAL: AtomicU64 = AtomicU64::new(0);
static DISPATCH_REDIRECTED: AtomicU64 = AtomicU64::new(0);
static DISPATCH_UNMATCHED: AtomicU64 = AtomicU64::new(0);

/// TLS acceptor for direct browser connections. Reload it when Certbot swaps
/// either file so new connections use the renewed certificate without
/// restarting workers or interrupting existing games.
#[derive(PartialEq, Eq)]
struct TlsFileStamp {
    cert_len: u64,
    cert_modified: Option<SystemTime>,
    key_len: u64,
    key_modified: Option<SystemTime>,
}

#[derive(Default)]
struct TlsAcceptorState {
    files: Option<TlsFileStamp>,
    acceptor: Option<TlsAcceptor>,
}

static TLS_ACCEPTOR: OnceLock<StdMutex<TlsAcceptorState>> = OnceLock::new();

fn tls_file_stamp() -> Option<TlsFileStamp> {
    let cert_path = std::env::var("SOW_RELAY_TLS_CERT").ok()?;
    let key_path = std::env::var("SOW_RELAY_TLS_KEY").ok()?;
    let cert = std::fs::metadata(cert_path).ok()?;
    let key = std::fs::metadata(key_path).ok()?;
    Some(TlsFileStamp {
        cert_len: cert.len(),
        cert_modified: cert.modified().ok(),
        key_len: key.len(),
        key_modified: key.modified().ok(),
    })
}

fn load_tls_acceptor() -> Option<TlsAcceptor> {
    let cert_path = std::env::var("SOW_RELAY_TLS_CERT").ok()?;
    let key_path = std::env::var("SOW_RELAY_TLS_KEY").ok()?;

    let cert_bytes = std::fs::read(&cert_path).ok()?;
    let key_bytes = std::fs::read(&key_path).ok()?;

    // Parse certificates
    let mut cert_reader = BufReader::new(cert_bytes.as_slice());
    let raw_certs = match rustls_pemfile::certs(&mut cert_reader) {
        Ok(v) => v,
        Err(e) => {
            error!("[tls] cert parse error: {e}");
            return None;
        }
    };
    let certs: Vec<Certificate> = raw_certs.into_iter().map(Certificate).collect();
    if certs.is_empty() {
        error!("[tls] no certificates found in {}", cert_path);
        return None;
    }

    // Parse private key (try PKCS8, then RSA)
    let mut key_reader = BufReader::new(key_bytes.as_slice());
    let mut key_data: Option<Vec<u8>> = None;
    if let Ok(keys) = rustls_pemfile::pkcs8_private_keys(&mut key_reader) {
        if let Some(d) = keys.into_iter().next() {
            key_data = Some(d);
        }
    }
    if key_data.is_none() {
        let mut r2 = BufReader::new(key_bytes.as_slice());
        if let Ok(keys) = rustls_pemfile::rsa_private_keys(&mut r2) {
            if let Some(d) = keys.into_iter().next() {
                key_data = Some(d);
            }
        }
    }
    let key = match key_data {
        Some(k) => PrivateKey(k),
        None => {
            error!("[tls] no private key found in {}", key_path);
            return None;
        }
    };

    match ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
    {
        Ok(config) => {
            info!(
                "[tls] cert loaded from {}, key from {}",
                cert_path, key_path
            );
            Some(TlsAcceptor::from(Arc::new(config)))
        }
        Err(e) => {
            error!("[tls] config error: {e}");
            None
        }
    }
}

fn current_tls_acceptor() -> Option<TlsAcceptor> {
    let files = tls_file_stamp();
    let state = TLS_ACCEPTOR.get_or_init(|| StdMutex::new(TlsAcceptorState::default()));
    let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
    if state.files != files {
        state.files = files;
        if let Some(acceptor) = load_tls_acceptor() {
            state.acceptor = Some(acceptor);
        } else if state.acceptor.is_some() {
            warn!("[tls] certificate reload failed; retaining the current certificate");
        }
    }
    state.acceptor.clone()
}

fn base_mgmt_port() -> u16 {
    std::env::var("SOW_RELAY_BASE_MGMT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_MGMT_PORT)
}

fn worker_port(base: u16, queue_id: u16) -> Result<u16, String> {
    base.checked_add(queue_id)
        .ok_or_else(|| format!("worker port overflow: base={base} queue={queue_id}"))
}

/// Advertised relay address (data PIP) written to `sow:relay:{lobby_id}`.
fn relay_host() -> String {
    if let Ok(host) = std::env::var("SOW_RELAY_HOST") {
        if !host.trim().is_empty() {
            return host.trim().to_string();
        }
    }
    "127.0.0.1".to_string()
}

fn relay_addr(port: u16) -> String {
    format!("{}:{port}", relay_host())
}

unsafe extern "C" fn relay_packet_dispatcher(
    data: *mut c_void,
    len: *mut u16,
    queue_id: u16,
    nb_queues: u16,
) -> c_int {
    if data.is_null() || len.is_null() {
        return queue_id as c_int;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, *len as usize) };
    match fstack_bridge::tcp_destination_queue(bytes, nb_queues, DYNAMIC_PORT_MIN) {
        Some(target) if target == queue_id => {
            DISPATCH_LOCAL.fetch_add(1, Ordering::Relaxed);
            target as c_int
        }
        Some(target) => {
            DISPATCH_REDIRECTED.fetch_add(1, Ordering::Relaxed);
            target as c_int
        }
        None => {
            DISPATCH_UNMATCHED.fetch_add(1, Ordering::Relaxed);
            queue_id as c_int
        }
    }
}

/// Seconds without any frame before a connection is classified as the
/// orchestration/operations WS (the optional bot/backfill client sends nothing,
/// while players send their admission frame within ~2s).
const ORCHESTRATOR_GRACE_SECS: u64 = 3;

/// Max consecutive missed ticks before dropping a slow client.
/// At 10 ticks/s, 100 = 10 seconds of grace before dropping a slow client.
const MAX_MISSED_TICKS: u32 = 100;

/// Per-connection outbound channel capacity. Expanded to 128 to buffer transient network jitter.
const PER_CLIENT_CHANNEL: usize = 128;

/// Outbound WebSocket frame write timeout in milliseconds.
/// Set to 15000ms (15.0s) to tolerate transient TCP windowing and geographic/proxy latency.
/// Overridable via SOW_WS_WRITE_TIMEOUT_MS so the deployed value is registered
/// in the unit drop-in and visible in the [BOOT] line; the pipeline records it
/// in the relay manifest for every release.
fn ws_write_timeout_ms() -> u64 {
    static TIMEOUT: OnceLock<u64> = OnceLock::new();
    *TIMEOUT.get_or_init(|| {
        std::env::var("SOW_WS_WRITE_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value >= 100)
            .unwrap_or(15000)
    })
}

/// Per-connection bridge RX capacity (inbound DPDK mbuf guards).
const BRIDGE_RX_CAP: usize = 256;

/// Event channel capacity.
const EVENT_CHANNEL: usize = 1024;

/// Liveness: relay-initiated WS ping period. Browsers auto-Pong at the WS
/// layer; backfill/native clients Pong explicitly. A dead peer never does.
const WS_PING_SECS: u64 = 15;

/// Liveness: reap a player connection after this many seconds without a
/// single received frame (data, Ping, or Pong). A SIGKILLed peer whose FIN
/// was lost under DPDK load sends nothing, so it is reaped here instead of
/// lingering in the lobby forever.
const RX_DEADLINE_SECS: u64 = 45;

/// Number of append commands allowed to wait behind the replay writer. The
/// tick loop awaits when this queue is full, preserving every turn without
/// allowing replay backlog to grow with the match duration.
const REPLAY_QUEUE_CAP: usize = 256;
const LIVE_HISTORY_CAP: usize = 512;
/// Hard ceiling for one replay artifact. The current production replays are
/// below 1 MiB; this keeps a malformed or runaway match from becoming a RAM
/// or disk amplification vector.
const MAX_REPLAY_BYTES: u64 = 16 * 1024 * 1024;

// ---- relay event plumbing ---------------------------------------------------

enum RelayEvent {
    Gameplay {
        player_id: u16,
        intent: GameplayIntent,
        generation: u64,
    },
    VoluntaryLeave {
        player_id: u16,
        generation: u64,
        peer: SocketAddr,
        reply: oneshot::Sender<Result<u64, String>>,
    },
    ExitRequested {
        account_id: String,
        reply: oneshot::Sender<Result<u64, String>>,
    },
    Disconnected {
        player_id: u16,
        generation: u64,
        peer: SocketAddr,
    },
    EndMatch,
}

struct ClientChannel {
    sender: mpsc::Sender<Arc<Vec<u8>>>,
    missed_ticks: u32,
    generation: u64,
}

// ---- redis helpers ----------------------------------------------------------

fn redis_connect() -> Option<redis::Connection> {
    let url = std::env::var("SOW_VALKEY_URL")
        .or_else(|_| std::env::var("SOW_REDIS_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    redis::Client::open(url)
        .ok()
        .and_then(|c| c.get_connection().ok())
}

static REDIS_CON: OnceLock<Arc<std::sync::Mutex<Option<redis::Connection>>>> = OnceLock::new();

fn redis_shared() -> Arc<std::sync::Mutex<Option<redis::Connection>>> {
    REDIS_CON
        .get_or_init(|| Arc::new(std::sync::Mutex::new(redis_connect())))
        .clone()
}

enum ReplayCommand {
    Append(Turn),
    Checkpoint {
        through_turn: u64,
        participant_closures: Vec<ParticipantClosure>,
        reply: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    Finalize {
        metadata: ReplaySpoolMetadata,
        reply: oneshot::Sender<Result<PathBuf, String>>,
    },
}

#[derive(serde::Serialize)]
struct ReplayFinalizePayload<'a> {
    match_id: &'a str,
    lobby_json: &'a str,
    replay_data: Option<&'a [u8]>,
    participant_closures: &'a [ParticipantClosure],
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ParticipantSettlementPayload {
    match_id: String,
    account_id: String,
    closure_turn: u64,
    replay_sha256: String,
    #[serde(default)]
    rejected: bool,
}

struct ParticipantSettlementJob {
    payload: ParticipantSettlementPayload,
    replay_bytes: Vec<u8>,
    spool_path: PathBuf,
}

struct PendingParticipantSettlement {
    account_id: String,
    closure_turn: u64,
    replay_bytes: Vec<u8>,
    rejected: bool,
    retry_count: u8,
    direct_in_flight: bool,
    direct_retry_after: Option<Instant>,
    queued_at: Instant,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
struct ParticipantClosure {
    account_id: String,
    turn_number: u64,
    #[serde(default)]
    reason: ExitReason,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ReplaySpoolMetadata {
    lobby_json: String,
    #[serde(default)]
    phase: ReplayPhase,
    #[serde(default)]
    participant_closures: Vec<ParticipantClosure>,
    #[serde(default)]
    rejected: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReplayPhase {
    Recording,
    // Older metadata was written only at finalization.
    #[default]
    Finalized,
}

async fn persist_replay_metadata(
    path: &Path,
    metadata: &ReplaySpoolMetadata,
) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(metadata).map_err(|error| error.to_string())?;
    let mut file = tokio::fs::File::create(&temporary)
        .await
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .await
        .map_err(|error| error.to_string())?;
    file.sync_all().await.map_err(|error| error.to_string())?;
    drop(file);
    tokio::fs::rename(temporary, path)
        .await
        .map_err(|error| error.to_string())?;
    std::fs::File::open(path.parent().ok_or("metadata directory missing")?)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ReplaySpoolMetadataCompat {
    Legacy(String),
    Current(ReplaySpoolMetadata),
}

impl ReplaySpoolMetadataCompat {
    fn into_current(self) -> ReplaySpoolMetadata {
        match self {
            Self::Legacy(lobby_json) => ReplaySpoolMetadata {
                lobby_json,
                phase: ReplayPhase::Finalized,
                participant_closures: Vec::new(),
                rejected: false,
            },
            Self::Current(metadata) => metadata,
        }
    }
}

struct ReplayJournal {
    live: Arc<Mutex<VecDeque<Turn>>>,
    tx: mpsc::Sender<ReplayCommand>,
    finalized: AtomicBool,
    failed: Arc<AtomicBool>,
}

fn replay_spool_dir() -> PathBuf {
    std::env::var("SOW_REPLAY_SPOOL_DIR")
        .or_else(|_| std::env::var("SOW_REPLAY_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("replays"))
}

impl ReplayJournal {
    async fn new(match_id: u64, lobby_json: &str) -> Result<Arc<Self>, String> {
        let journal_path = replay_spool_dir().join(format!("{match_id}.journal"));
        let claim = claim_result(journal_path.clone()).await?;
        tokio::fs::create_dir_all(replay_spool_dir())
            .await
            .map_err(|error| error.to_string())?;
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&journal_path)
            .await
            .map_err(|error| error.to_string())?;
        file.sync_all().await.map_err(|error| error.to_string())?;
        let metadata = ReplaySpoolMetadata {
            lobby_json: lobby_json.to_string(),
            phase: ReplayPhase::Recording,
            participant_closures: Vec::new(),
            rejected: false,
        };
        persist_replay_metadata(&journal_path.with_extension("json"), &metadata).await?;
        let (tx, rx) = mpsc::channel(REPLAY_QUEUE_CAP);
        let failed = Arc::new(AtomicBool::new(false));
        let journal = Arc::new(Self {
            live: Arc::new(Mutex::new(VecDeque::with_capacity(LIVE_HISTORY_CAP))),
            tx,
            finalized: AtomicBool::new(false),
            failed: failed.clone(),
        });
        tokio::spawn(replay_writer(
            journal_path,
            rx,
            failed,
            file,
            claim,
            metadata,
        ));
        Ok(journal)
    }

    async fn append(&self, turn: Turn) -> Result<(), String> {
        if self.finalized.load(Ordering::Acquire) {
            return Err("replay journal already finalized".to_string());
        }
        if self.failed.load(Ordering::Acquire) {
            return Err("replay writer failed".to_string());
        }
        // Backpressure is intentional: dropping an append would silently
        // corrupt the replay. The bounded channel prevents an unbounded RAM
        // queue while the writer catches up.
        self.tx
            .send(ReplayCommand::Append(turn.clone()))
            .await
            .map_err(|_| "replay writer stopped".to_string())?;
        let mut live = self.live.lock().await;
        live.push_back(turn);
        while live.len() > LIVE_HISTORY_CAP {
            live.pop_front();
        }
        Ok(())
    }

    async fn snapshot(&self) -> Vec<Turn> {
        self.live.lock().await.iter().cloned().collect()
    }

    async fn checkpoint(
        &self,
        through_turn: u64,
        participant_closures: Vec<ParticipantClosure>,
    ) -> Result<Vec<u8>, String> {
        if self.finalized.load(Ordering::Acquire) {
            return Err("replay journal already finalized".to_string());
        }
        if self.failed.load(Ordering::Acquire) {
            return Err("replay writer failed".to_string());
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(ReplayCommand::Checkpoint {
                through_turn,
                participant_closures,
                reply,
            })
            .await
            .map_err(|_| "replay writer stopped".to_string())?;
        result
            .await
            .map_err(|_| "replay writer dropped checkpoint response".to_string())?
    }

    async fn finalize(
        &self,
        lobby_json: &str,
        closures: &[ParticipantClosure],
    ) -> Result<PathBuf, String> {
        if self.finalized.swap(true, Ordering::AcqRel) {
            return Err("replay journal already finalized".to_string());
        }
        let (reply, result) = oneshot::channel();
        if self
            .tx
            .send(ReplayCommand::Finalize {
                reply,
                metadata: ReplaySpoolMetadata {
                    lobby_json: lobby_json.to_string(),
                    phase: ReplayPhase::Finalized,
                    participant_closures: closures.to_vec(),
                    rejected: false,
                },
            })
            .await
            .is_err()
        {
            return Err("replay writer stopped".to_string());
        }
        let replay = result
            .await
            .map_err(|_| "replay writer dropped finalize response".to_string())??;
        self.live.lock().await.clear();
        Ok(replay)
    }
}

async fn replay_writer(
    journal_path: PathBuf,
    mut rx: mpsc::Receiver<ReplayCommand>,
    failed: Arc<AtomicBool>,
    mut file: tokio::fs::File,
    _journal_claim: std::fs::File,
    mut metadata: ReplaySpoolMetadata,
) {
    let mut journal_bytes = 0u64;
    while let Some(command) = rx.recv().await {
        match command {
            ReplayCommand::Append(turn) => {
                let encoded = match bincode::serialize(&turn) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        failed.store(true, Ordering::Release);
                        error!("[replay] serialize turn failed: {}", e);
                        break;
                    }
                };
                let len = encoded.len() as u32;
                let record_bytes = 4u64.saturating_add(encoded.len() as u64);
                if journal_bytes.saturating_add(record_bytes) > MAX_REPLAY_BYTES {
                    failed.store(true, Ordering::Release);
                    error!(
                        "[replay] journal {:?} exceeded {} byte limit",
                        journal_path, MAX_REPLAY_BYTES
                    );
                    break;
                }
                if file.write_all(&len.to_le_bytes()).await.is_err()
                    || file.write_all(&encoded).await.is_err()
                {
                    failed.store(true, Ordering::Release);
                    error!("[replay] append journal {:?} failed", journal_path);
                    break;
                }
                journal_bytes = journal_bytes.saturating_add(record_bytes);
            }
            ReplayCommand::Checkpoint {
                through_turn,
                participant_closures,
                reply,
            } => {
                let result = async {
                    file.flush().await.map_err(|e| e.to_string())?;
                    file.sync_all().await.map_err(|e| e.to_string())?;
                    let bytes = tokio::fs::read(&journal_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let history = decode_replay_journal(&bytes, false)?;
                    let prefix = replay_prefix_through(&history, through_turn)?;
                    metadata.participant_closures = participant_closures;
                    persist_replay_metadata(&journal_path.with_extension("json"), &metadata)
                        .await?;
                    Ok(prefix)
                }
                .await;
                let _ = reply.send(result);
            }
            ReplayCommand::Finalize { reply, metadata } => {
                let result = async {
                    file.flush().await.map_err(|e| e.to_string())?;
                    file.sync_all().await.map_err(|e| e.to_string())?;
                    let size = tokio::fs::metadata(&journal_path)
                        .await
                        .map_err(|e| e.to_string())?
                        .len();
                    if size > MAX_REPLAY_BYTES {
                        return Err(format!(
                            "replay journal exceeds {} byte limit",
                            MAX_REPLAY_BYTES
                        ));
                    }
                    let bytes = tokio::fs::read(&journal_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let history = decode_replay_journal(&bytes, false)?;
                    let canonical = bincode::serialize(&history).map_err(|e| e.to_string())?;
                    let replay_path = journal_path.with_extension("replay");
                    let temp_path = journal_path.with_extension("replay.tmp");
                    let mut replay_file = tokio::fs::File::create(&temp_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    replay_file
                        .write_all(&canonical)
                        .await
                        .map_err(|e| e.to_string())?;
                    replay_file.sync_all().await.map_err(|e| e.to_string())?;
                    drop(replay_file);
                    tokio::fs::rename(&temp_path, &replay_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    std::fs::File::open(
                        replay_path.parent().ok_or("replay directory unavailable")?,
                    )
                    .and_then(|directory| directory.sync_all())
                    .map_err(|e| e.to_string())?;
                    persist_replay_metadata(&journal_path.with_extension("json"), &metadata)
                        .await?;
                    let _ = tokio::fs::remove_file(&journal_path).await;
                    Ok(replay_path)
                }
                .await;
                if result.is_err() {
                    failed.store(true, Ordering::Release);
                }
                let _ = reply.send(result);
                break;
            }
        }
    }
}

fn decode_replay_journal(
    bytes: &[u8],
    recover_complete_records: bool,
) -> Result<Vec<Turn>, String> {
    let mut offset = 0usize;
    let mut history = Vec::new();
    while offset < bytes.len() {
        if bytes.len().saturating_sub(offset) < 4 {
            if recover_complete_records {
                break;
            }
            return Err("truncated replay record length".to_string());
        }
        let len = u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| "invalid replay record length".to_string())?,
        ) as usize;
        offset += 4;
        if len > bytes.len().saturating_sub(offset) {
            if recover_complete_records {
                break;
            }
            return Err("truncated replay record".to_string());
        }
        let turn = bincode::deserialize(&bytes[offset..offset + len])
            .map_err(|e| format!("decode replay record: {e}"))?;
        history.push(turn);
        offset += len;
    }
    Ok(history)
}

fn replay_prefix_through(history: &[Turn], through_turn: u64) -> Result<Vec<u8>, String> {
    let end = history
        .iter()
        .rposition(|turn| turn.turn_number == through_turn)
        .ok_or_else(|| format!("replay closure turn {through_turn} is missing"))?;
    bincode::serialize(&history[..=end]).map_err(|error| error.to_string())
}

fn finalize_gate() -> Arc<Semaphore> {
    static GATE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    GATE.get_or_init(|| Arc::new(Semaphore::new(1))).clone()
}

/// Keep the lock inode: unlinking it could let different processes own two
/// locks for the same result. File locks are released automatically on exit.
async fn claim_result(path: PathBuf) -> Result<std::fs::File, String> {
    tokio::task::spawn_blocking(move || {
        lock_result(&path, libc::LOCK_EX).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn lock_result(path: &Path, flags: c_int) -> std::io::Result<std::fs::File> {
    use std::os::fd::AsRawFd;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("result lock directory unavailable"))?;
    std::fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("result lock name unavailable"))?
        .to_string_lossy();
    // Recovery scans only pending artifacts, never a growing list of old locks.
    let directory = parent.join(".locks");
    std::fs::create_dir_all(&directory)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(name.as_ref()))?;
    if unsafe { libc::flock(file.as_raw_fd(), flags) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(file)
}

async fn post_replay_finalize(replay_path: &Path) -> Result<(), String> {
    let _claim = claim_result(replay_path.to_path_buf()).await?;
    if !tokio::fs::try_exists(replay_path)
        .await
        .map_err(|error| error.to_string())?
    {
        cleanup_result_spool(replay_path, &replay_path.with_extension("json")).await?;
        return Ok(()); // Another worker already delivered and removed this result.
    }
    let metadata_path = replay_path.with_extension("json");
    let metadata = tokio::fs::read(&metadata_path)
        .await
        .map_err(|error| error.to_string())?;
    let metadata = serde_json::from_slice::<ReplaySpoolMetadataCompat>(&metadata)
        .map_err(|error| error.to_string())?
        .into_current();
    if metadata.phase != ReplayPhase::Finalized {
        return Err("replay is still recording".into());
    }
    let match_id = replay_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or("invalid replay filename")?;
    if tokio::fs::metadata(replay_path)
        .await
        .map_err(|error| error.to_string())?
        .len()
        > MAX_REPLAY_BYTES
    {
        return Err("replay spool exceeds size limit".into());
    }
    let replay_data = tokio::fs::read(replay_path)
        .await
        .map_err(|error| error.to_string())?;
    let db_url = configured_db_url()?;
    let secret = std::env::var("SOW_RELAY_SETTLEMENT_SECRET")
        .map_err(|_| "SOW_RELAY_SETTLEMENT_SECRET is not available".to_string())?;
    let url = format!("{}/internal/match-finalize", db_url.trim_end_matches('/'));
    let payload = ReplayFinalizePayload {
        match_id,
        lobby_json: &metadata.lobby_json,
        replay_data: if metadata.rejected {
            None
        } else {
            Some(&replay_data)
        },
        participant_closures: &metadata.participant_closures,
    };
    let client = db_client(&db_url)?;

    for attempt in 1..=5 {
        info!(
            "Attempting raw upload/finalize to database for match {match_id} (Attempt {attempt}/5)..."
        );
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {secret}"))
            .json(&payload)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let ack = response
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|error| error.to_string())?;
                if ack.get("match_id").and_then(serde_json::Value::as_str) == Some(match_id)
                    && matches!(
                        ack.get("status").and_then(serde_json::Value::as_str),
                        Some("queued" | "finalized")
                    )
                {
                    info!("Match {match_id} durably accepted by database");
                    cleanup_result_spool(replay_path, &metadata_path).await?;
                    return Ok(());
                }
                warn!("[replay] database did not acknowledge match {match_id}; retaining spool");
            }
            Ok(response) => {
                let status = response.status();
                warn!(
                    "Attempt {attempt}/5: database returned HTTP status {} for match {match_id}",
                    status
                );
                let reason = response
                    .text()
                    .await
                    .unwrap_or_else(|error| format!("response unavailable: {error}"));
                warn!("[replay] match {match_id} remains unacknowledged: {reason}");
                if matches!(status.as_u16(), 401 | 403) {
                    return Err(format!(
                        "match {match_id} access rejected HTTP {status}: {reason}"
                    ));
                }
            }
            Err(error) => {
                warn!("Attempt {attempt}/5: network error uploading match {match_id}: {error}");
            }
        }

        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }

    Err(format!(
        "database did not accept match {match_id} after 5 attempts"
    ))
}

fn participant_spool_path(match_id: &str, account_id: &str) -> PathBuf {
    participant_spool_path_at(&replay_spool_dir(), match_id, account_id)
}

fn participant_spool_path_at(directory: &Path, match_id: &str, account_id: &str) -> PathBuf {
    let safe_account = account_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_hexdigit() || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    directory.join(format!("{match_id}-{safe_account}.participant"))
}

fn recovered_exit_prefix(mut turns: Vec<Turn>, player_id: u16) -> Result<(u64, Vec<u8>), String> {
    if !turns
        .iter()
        .enumerate()
        .all(|(index, turn)| turn.turn_number == index as u64)
    {
        return Err("orphaned journal turn sequence is invalid".into());
    }
    let closure = turns.iter().position(|turn| {
        turn.intents.iter().any(|intent| {
            intent.player_id == player_id && matches!(intent.intent, GameplayIntent::Resign)
        })
    });
    let turn = if let Some(index) = closure {
        turns.truncate(index + 1);
        index as u64
    } else {
        let turn = turns.len() as u64;
        turns.push(Turn {
            turn_number: turn,
            intents: vec![StampedIntent {
                player_id,
                intent: GameplayIntent::Resign,
            }],
        });
        turn
    };
    Ok((
        turn,
        bincode::serialize(&turns).map_err(|error| error.to_string())?,
    ))
}

/// The server proves the owning worker and registered player. The native
/// journal lock proves its writer has stopped; a closed game port alone does
/// not authorize recovery or rejection.
async fn recover_detached_exit(
    directory: &Path,
    command: &sow_data::profile::MatchExitCommand,
) -> Result<ParticipantSettlementJob, String> {
    if !command.owner_verified {
        return Err("journal owner has not been verified".into());
    }
    let player_id = command.player_id.ok_or("registered player is missing")?;
    let journal = directory.join(format!("{}.journal", command.match_id));
    let claim_path = journal.clone();
    let _claim = tokio::task::spawn_blocking(move || {
        lock_result(&claim_path, libc::LOCK_EX | libc::LOCK_NB)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("journal still owned or unavailable: {error}"))?;
    let history = match tokio::fs::read(&journal).await {
        Ok(bytes) => decode_replay_journal(&bytes, true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match tokio::fs::read(journal.with_extension("replay")).await {
                Ok(bytes) => {
                    bincode::deserialize::<Vec<Turn>>(&bytes).map_err(|error| error.to_string())
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Err("stopped relay has no recoverable turn record".into())
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        Err(error) => return Err(error.to_string()),
    };
    let (turn, replay_bytes, rejected) = match history
        .and_then(|turns| recovered_exit_prefix(turns, player_id))
    {
        Ok((turn, bytes)) => (turn, bytes, false),
        Err(reason) => {
            warn!(
                "[rewards] detached exit match={} has no valid replay prefix; recording rejection: {reason}",
                command.match_id
            );
            (0, Vec::new(), true)
        }
    };
    let payload = ParticipantSettlementPayload {
        match_id: command.match_id.clone(),
        account_id: command.account_id.clone(),
        closure_turn: turn,
        replay_sha256: hex::encode(Sha256::digest(&replay_bytes)),
        rejected,
    };
    let path = participant_spool_path_at(directory, &command.match_id, &command.account_id);
    persist_participant_spool(&path, &payload, &replay_bytes)?;
    Ok(ParticipantSettlementJob {
        payload,
        replay_bytes,
        spool_path: path,
    })
}

fn participant_replay_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("participant");
    let name = name.strip_suffix(".tmp").unwrap_or(name);
    path.with_file_name(format!("{name}.replay"))
}

fn persist_participant_spool(
    path: &Path,
    payload: &ParticipantSettlementPayload,
    replay_bytes: &[u8],
) -> Result<(), String> {
    // Publication and delivery share the same native lock.
    let _claim =
        lock_result(path, libc::LOCK_EX | libc::LOCK_NB).map_err(|error| error.to_string())?;
    let Some(parent) = path.parent() else {
        return Err("participant spool has no parent directory".to_string());
    };
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    if hex::encode(Sha256::digest(replay_bytes)) != payload.replay_sha256 {
        return Err("participant replay spool checksum differs".to_string());
    }
    let replay_path = participant_replay_path(path);
    let replay_name = replay_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("participant.replay");
    let replay_temp = replay_path.with_file_name(format!("{replay_name}.tmp"));
    let mut replay_file = std::fs::File::create(&replay_temp).map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut replay_file, replay_bytes).map_err(|error| error.to_string())?;
    replay_file.sync_all().map_err(|error| error.to_string())?;
    drop(replay_file);
    std::fs::rename(&replay_temp, &replay_path).map_err(|error| error.to_string())?;

    let temp_path = path.with_extension("participant.tmp");
    let bytes = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
    let mut file = std::fs::File::create(&temp_path).map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut file, &bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    std::fs::rename(&temp_path, path).map_err(|error| error.to_string())?;
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn post_participant_settle(
    payload: &ParticipantSettlementPayload,
    mut replay_bytes: Vec<u8>,
    spool_path: Option<&Path>,
) -> Result<(), String> {
    let delivery_started = Instant::now();
    let canonical_path = participant_spool_path(&payload.match_id, &payload.account_id);
    let _claim = claim_result(canonical_path.clone()).await?;
    // A legacy .tmp reader may have waited while the writer published its final name.
    let spool_path = if spool_path.is_some()
        && tokio::fs::try_exists(&canonical_path)
            .await
            .map_err(|error| error.to_string())?
    {
        Some(canonical_path.as_path())
    } else {
        spool_path
    };
    let mut payload = payload.clone();
    if let Some(path) = spool_path {
        if !tokio::fs::try_exists(path)
            .await
            .map_err(|error| error.to_string())?
        {
            return Ok(()); // The previous owner already received the durable ACK.
        }
        let saved = tokio::fs::read(path)
            .await
            .map_err(|error| error.to_string())?;
        let saved = serde_json::from_slice::<ParticipantSettlementPayload>(&saved)
            .map_err(|error| error.to_string())?;
        if saved.match_id != payload.match_id
            || saved.account_id != payload.account_id
            || saved.closure_turn != payload.closure_turn
        {
            return Err("participant spool identity changed before delivery".into());
        }
        replay_bytes = tokio::fs::read(participant_replay_path(path))
            .await
            .map_err(|error| error.to_string())?;
        if saved.replay_sha256 != hex::encode(Sha256::digest(&replay_bytes))
            || saved.rejected != replay_bytes.is_empty()
        {
            return Err("participant spool differs from its durable replay".into());
        }
        payload = saved;
    }
    let db_url = configured_db_url()?;
    let secret = std::env::var("SOW_RELAY_SETTLEMENT_SECRET")
        .map_err(|_| "SOW_RELAY_SETTLEMENT_SECRET is not available".to_string())?;
    let url = format!(
        "{}/internal/match-participant-settle/{}/{}/{}",
        db_url.trim_end_matches('/'),
        payload.match_id,
        payload.account_id,
        payload.closure_turn
    );
    let client = db_client(&db_url)?;
    for attempt in 1..=5 {
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {secret}"))
            .header("Content-Type", "application/octet-stream")
            .body(replay_bytes.clone())
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let ack = response
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|error| error.to_string())?;
                if ack.get("match_id").and_then(serde_json::Value::as_str)
                    == Some(payload.match_id.as_str())
                    && ack.get("account_id").and_then(serde_json::Value::as_str)
                        == Some(payload.account_id.as_str())
                    && ack.get("closure_turn").and_then(serde_json::Value::as_u64)
                        == Some(payload.closure_turn)
                    && matches!(
                        ack.get("status").and_then(serde_json::Value::as_str),
                        Some("queued" | "settled" | "rejected")
                    )
                {
                    info!(
                        "[exit-timing] delivered match={} account={} turn={} delivery_ms={}",
                        payload.match_id,
                        payload.account_id,
                        payload.closure_turn,
                        delivery_started.elapsed().as_millis()
                    );
                    if let Some(path) = spool_path {
                        cleanup_result_spool(path, &participant_replay_path(path)).await?;
                    }
                    return Ok(());
                }
                warn!(
                    "[rewards] database did not acknowledge participant match={}; retaining spool",
                    payload.match_id
                );
            }
            Ok(response) => {
                let status = response.status();
                let reason = response
                    .text()
                    .await
                    .unwrap_or_else(|error| format!("response unavailable: {error}"));
                warn!(
                    "[rewards] participant settle attempt {attempt}/5 returned HTTP {status} match={} account={}: {reason}",
                    payload.match_id, payload.account_id
                );
                if matches!(status.as_u16(), 401 | 403) {
                    return Err(format!(
                        "participant result access rejected HTTP {status}: {reason}"
                    ));
                }
            }
            Err(error) => warn!(
                "[rewards] participant settle attempt {attempt}/5 failed match={} account={}: {error}",
                payload.match_id, payload.account_id
            ),
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }
    Err(format!(
        "database did not accept participant settlement match={} account={}",
        payload.match_id, payload.account_id
    ))
}

/// Called before releasing the native delivery lock. Missing records mean
/// another worker finished; failures remain retryable against the same receipt.
async fn cleanup_result_spool(path: &Path, sidecar: &Path) -> Result<(), String> {
    for target in [path, sidecar] {
        if let Err(error) = tokio::fs::remove_file(target).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!("result cleanup {target:?} failed: {error}"));
        }
    }
    std::fs::File::open(path.parent().ok_or("result directory missing")?)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

async fn recover_participant_spool() {
    let spool_dir = replay_spool_dir();
    let mut entries = match tokio::fs::read_dir(&spool_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            warn!("[rewards] scan {:?} failed: {error}", spool_dir);
            return;
        }
    };
    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(error) => {
                warn!("[rewards] scan {:?} failed: {error}", spool_dir);
                break;
            }
        };
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if path.extension().and_then(|value| value.to_str()) != Some("participant")
            && !file_name.ends_with(".participant.tmp")
        {
            continue;
        }
        let payload_bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!(
                    "[rewards] read participant spool {:?} failed: {error}",
                    path
                );
                continue;
            }
        };
        let payload_value: serde_json::Value = match serde_json::from_slice(&payload_bytes) {
            Ok(payload) => payload,
            Err(error) => {
                warn!("[rewards] invalid participant spool {:?}: {error}", path);
                continue;
            }
        };
        if payload_value.get("closure_turn").is_none() {
            warn!(
                "[rewards] retaining legacy participant spool without a replay prefix; whole replay is required {:?}",
                path
            );
            continue;
        }
        let payload = match serde_json::from_value::<ParticipantSettlementPayload>(payload_value) {
            Ok(payload) => payload,
            Err(error) => {
                warn!("[rewards] invalid participant spool {:?}: {error}", path);
                continue;
            }
        };
        let replay_bytes = match tokio::fs::read(participant_replay_path(&path)).await {
            Ok(bytes) if hex::encode(Sha256::digest(&bytes)) == payload.replay_sha256 => bytes,
            Ok(_) => {
                warn!("[rewards] participant replay checksum mismatch {:?}", path);
                continue;
            }
            Err(error) => {
                warn!(
                    "[rewards] participant replay unavailable {:?}: {error}",
                    path
                );
                continue;
            }
        };
        if payload.rejected != replay_bytes.is_empty() {
            warn!(
                "[rewards] participant spool outcome does not match its replay prefix {:?}",
                path
            );
            continue;
        }
        let _permit = match finalize_gate().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        if let Err(error) = post_participant_settle(&payload, replay_bytes, Some(&path)).await {
            warn!("[rewards] keeping participant spool {:?}: {error}", path);
        }
    }
}

fn prepare_participant_settlement(
    match_id: u64,
    account_id: String,
    closure_turn: u64,
    replay_bytes: Vec<u8>,
    rejected: bool,
) -> Result<ParticipantSettlementJob, String> {
    let payload = ParticipantSettlementPayload {
        match_id: match_id.to_string(),
        account_id,
        closure_turn,
        replay_sha256: hex::encode(Sha256::digest(&replay_bytes)),
        rejected,
    };
    let spool_path = participant_spool_path(&payload.match_id, &payload.account_id);
    persist_participant_spool(&spool_path, &payload, &replay_bytes).map_err(|error| {
        format!(
            "participant spool write failed match={} account={}: {error}",
            payload.match_id, payload.account_id
        )
    })?;
    Ok(ParticipantSettlementJob {
        payload,
        replay_bytes,
        spool_path,
    })
}

fn retry_pending_participant_settlements(
    match_id: u64,
    pending: &mut Vec<PendingParticipantSettlement>,
    result_tx: &mpsc::UnboundedSender<(String, u64, bool)>,
) -> Vec<(String, u64)> {
    let mut retry = Vec::new();
    let mut recorded = Vec::new();
    for mut settlement in pending.drain(..) {
        let replay_bytes = if settlement.rejected {
            Vec::new()
        } else {
            settlement.replay_bytes.clone()
        };
        match prepare_participant_settlement(
            match_id,
            settlement.account_id.clone(),
            settlement.closure_turn,
            replay_bytes,
            settlement.rejected,
        ) {
            Ok(job) => {
                recorded.push((settlement.account_id.clone(), settlement.closure_turn));
                dispatch_participant_settlements(vec![job]);
            }
            Err(error) => {
                settlement.retry_count = settlement.retry_count.saturating_add(1);
                if !settlement.rejected
                    && settlement.queued_at.elapsed().as_secs() >= PARTICIPANT_CHECKPOINT_RETRY_SECS
                {
                    warn!(
                        "[rewards] lobby {match_id} cannot persist participant prefix; recording explicit rejection for account={}",
                        settlement.account_id
                    );
                    settlement.rejected = true;
                    settlement.replay_bytes.clear();
                    settlement.retry_count = 0;
                } else if settlement.retry_count == 1 || settlement.retry_count % 10 == 0 {
                    warn!("[rewards] lobby {match_id} participant spool retry failed: {error}");
                }
                if settlement.rejected
                    && !settlement.direct_in_flight
                    && settlement
                        .direct_retry_after
                        .is_none_or(|retry_after| retry_after <= Instant::now())
                {
                    let payload = ParticipantSettlementPayload {
                        match_id: match_id.to_string(),
                        account_id: settlement.account_id.clone(),
                        closure_turn: settlement.closure_turn,
                        replay_sha256: hex::encode(Sha256::digest(Vec::<u8>::new())),
                        rejected: true,
                    };
                    let replay_bytes = Vec::new();
                    let result_tx = result_tx.clone();
                    let account_id = settlement.account_id.clone();
                    let closure_turn = settlement.closure_turn;
                    settlement.direct_in_flight = true;
                    settlement.direct_retry_after =
                        Some(Instant::now() + Duration::from_secs(REPLAY_SPOOL_RETRY_SECS));
                    tokio::spawn(async move {
                        let accepted = post_participant_settle(&payload, replay_bytes, None)
                            .await
                            .is_ok();
                        let _ = result_tx.send((account_id, closure_turn, accepted));
                    });
                }
                retry.push(settlement);
            }
        }
    }
    *pending = retry;
    recorded
}

fn dispatch_participant_settlements(jobs: Vec<ParticipantSettlementJob>) {
    for ParticipantSettlementJob {
        payload,
        replay_bytes,
        spool_path,
    } in jobs
    {
        tokio::spawn(async move {
            let _permit = match finalize_gate().acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => return,
            };
            if let Err(error) =
                post_participant_settle(&payload, replay_bytes, Some(&spool_path)).await
            {
                warn!(
                    "[rewards] keeping participant spool match={} account={}: {error}",
                    payload.match_id, payload.account_id
                );
            }
        });
    }
}

async fn persist_rejected_replay_spool(
    match_id: u64,
    lobby_json: &str,
    participant_closures: &[ParticipantClosure],
) -> Result<PathBuf, String> {
    let replay_path = replay_spool_dir().join(format!("{match_id}.replay"));
    let metadata_path = replay_path.with_extension("json");
    let journal_path = replay_path.with_extension("journal");
    let _claim = claim_result(journal_path.clone()).await?;
    match tokio::fs::read(&metadata_path).await {
        Ok(bytes) => {
            let metadata = serde_json::from_slice::<ReplaySpoolMetadataCompat>(&bytes)
                .map_err(|error| error.to_string())?
                .into_current();
            if metadata.phase == ReplayPhase::Finalized {
                return Ok(replay_path);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !tokio::fs::try_exists(&journal_path)
                .await
                .map_err(|error| error.to_string())?
            {
                return Ok(replay_path); // Recovery already delivered this registered recording.
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    let replay_tmp = replay_path.with_extension("replay.tmp");
    let metadata_tmp = replay_path.with_extension("json.tmp");
    let metadata = ReplaySpoolMetadata {
        lobby_json: lobby_json.to_string(),
        phase: ReplayPhase::Finalized,
        participant_closures: participant_closures.to_vec(),
        rejected: true,
    };
    let metadata_bytes = serde_json::to_vec(&metadata).map_err(|error| error.to_string())?;
    if let Some(parent) = replay_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
    }
    let replay_file = tokio::fs::File::create(&replay_tmp)
        .await
        .map_err(|error| error.to_string())?;
    replay_file
        .sync_all()
        .await
        .map_err(|error| error.to_string())?;
    drop(replay_file);
    tokio::fs::rename(&replay_tmp, &replay_path)
        .await
        .map_err(|error| error.to_string())?;
    let metadata_file = tokio::fs::File::create(&metadata_tmp)
        .await
        .map_err(|error| error.to_string())?;
    let mut metadata_file = metadata_file;
    metadata_file
        .write_all(&metadata_bytes)
        .await
        .map_err(|error| error.to_string())?;
    metadata_file
        .sync_all()
        .await
        .map_err(|error| error.to_string())?;
    drop(metadata_file);
    tokio::fs::rename(&metadata_tmp, &metadata_path)
        .await
        .map_err(|error| error.to_string())?;
    std::fs::File::open(replay_path.parent().ok_or("replay directory missing")?)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(replay_path)
}

/// A process restart recovers the registered roster without a player request.
/// The writer's lifetime lock, shared by all workers, is the authority here.
async fn recover_recording(
    path: &Path,
    metadata: &mut ReplaySpoolMetadata,
) -> Result<bool, String> {
    let journal = path.with_extension("journal");
    let lock_path = journal.clone();
    let claim =
        tokio::task::spawn_blocking(move || lock_result(&lock_path, libc::LOCK_EX | libc::LOCK_NB))
            .await
            .map_err(|error| error.to_string())?;
    let _claim = match claim {
        Ok(claim) => claim,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    // Another recovery may have finished before this worker acquired the lock.
    let current = tokio::fs::read(path)
        .await
        .map_err(|error| error.to_string())?;
    let current = serde_json::from_slice::<ReplaySpoolMetadataCompat>(&current)
        .map_err(|error| error.to_string())?
        .into_current();
    if current.phase == ReplayPhase::Finalized {
        *metadata = current;
        return Ok(true);
    }
    let history = match tokio::fs::read(&journal).await {
        Ok(bytes) => decode_replay_journal(&bytes, true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::read(path.with_extension("replay"))
                .await
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    bincode::deserialize::<Vec<Turn>>(&bytes).map_err(|error| error.to_string())
                })
        }
        Err(error) => return Err(error.to_string()),
    };
    let roster: RegisterBody =
        serde_json::from_str(&metadata.lobby_json).map_err(|error| error.to_string())?;
    let mut turns = history.unwrap_or_else(|error| {
        warn!(
            "[replay] interrupted match={} has no valid record: {error}",
            roster.lobby_id
        );
        metadata.rejected = true;
        Vec::new()
    });
    if !turns
        .iter()
        .enumerate()
        .all(|(index, turn)| turn.turn_number == index as u64)
    {
        return Err("interrupted replay turn sequence is invalid".into());
    }
    let next_turn = turns.len() as u64;
    let mut resigns = Vec::new();
    for player in roster.players.iter().filter(|player| !player.is_internal) {
        let turn_number = turns
            .iter()
            .find(|turn| {
                turn.intents.iter().any(|intent| {
                    intent.player_id == player.player_id
                        && matches!(intent.intent, GameplayIntent::Resign)
                })
            })
            .map(|turn| turn.turn_number)
            .unwrap_or_else(|| {
                resigns.push(StampedIntent {
                    player_id: player.player_id,
                    intent: GameplayIntent::Resign,
                });
                next_turn
            });
        if let Some(account_id) = &player.database_account_id {
            if let Some(saved) = metadata
                .participant_closures
                .iter()
                .find(|closure| closure.account_id == *account_id)
            {
                if !metadata.rejected && saved.turn_number != turn_number {
                    return Err("saved personal closure differs from its journal".into());
                }
            } else {
                metadata.participant_closures.push(ParticipantClosure {
                    account_id: account_id.clone(),
                    turn_number,
                    reason: ExitReason::MatchEnded,
                });
            }
        }
    }
    if !resigns.is_empty() {
        turns.push(Turn {
            turn_number: next_turn,
            intents: resigns,
        });
    }
    let replay = if metadata.rejected {
        Vec::new()
    } else {
        bincode::serialize(&turns).map_err(|error| error.to_string())?
    };
    let replay_path = path.with_extension("replay");
    let temporary = path.with_extension("replay.tmp");
    let mut file = tokio::fs::File::create(&temporary)
        .await
        .map_err(|error| error.to_string())?;
    file.write_all(&replay)
        .await
        .map_err(|error| error.to_string())?;
    file.sync_all().await.map_err(|error| error.to_string())?;
    drop(file);
    tokio::fs::rename(temporary, replay_path)
        .await
        .map_err(|error| error.to_string())?;
    metadata.phase = ReplayPhase::Finalized;
    persist_replay_metadata(path, metadata).await?;
    if let Err(error) = tokio::fs::remove_file(journal).await {
        if error.kind() != std::io::ErrorKind::NotFound {
            warn!("[replay] recovered journal cleanup: {error}");
        }
    }
    Ok(true)
}

async fn recover_replay_spool() {
    let spool_dir = replay_spool_dir();
    let mut entries = match tokio::fs::read_dir(&spool_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            warn!("[replay] scan {:?} failed: {}", spool_dir, error);
            return;
        }
    };

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(error) => {
                warn!("[replay] scan {:?} failed: {}", spool_dir, error);
                break;
            }
        };
        let metadata_path = entry.path();
        if metadata_path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Some(match_id) = metadata_path
            .file_stem()
            .and_then(|value| value.to_str())
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        let bytes = match tokio::fs::read(&metadata_path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!("[replay] metadata unavailable match={match_id}: {error}");
                continue;
            }
        };
        let mut metadata = match serde_json::from_slice::<ReplaySpoolMetadataCompat>(&bytes) {
            Ok(metadata) => metadata.into_current(),
            Err(error) => {
                warn!("[replay] invalid metadata match={match_id}: {error}");
                continue;
            }
        };
        if metadata.phase == ReplayPhase::Recording {
            match recover_recording(&metadata_path, &mut metadata).await {
                Ok(true) => {}
                Ok(false) => continue, // Native lock proves that its writer is still alive.
                Err(error) => {
                    warn!("[replay] interrupted match={match_id} recovery deferred: {error}");
                    continue;
                }
            }
        }
        let replay_path = metadata_path.with_extension("replay");
        let _permit = match finalize_gate().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        if let Err(error) = post_replay_finalize(&replay_path).await {
            warn!("[replay] keeping pending match {match_id} for a later retry: {error}");
        }
    }
}

async fn replay_spool_worker() {
    loop {
        recover_replay_spool().await;
        recover_participant_spool().await;
        tokio::time::sleep(Duration::from_secs(REPLAY_SPOOL_RETRY_SECS)).await;
    }
}

fn trigger_match_finalize(
    match_id: u64,
    lobby_json: String,
    participant_closures: Vec<ParticipantClosure>,
    journal: Arc<ReplayJournal>,
) {
    tokio::spawn(async move {
        let _permit = match finalize_gate().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        let replay_path = match journal.finalize(&lobby_json, &participant_closures).await {
            Ok(path) => path,
            Err(e) => {
                error!("Failed to finalize replay journal for {match_id}: {e}");
                match persist_rejected_replay_spool(match_id, &lobby_json, &participant_closures)
                    .await
                {
                    Ok(replay_path) => {
                        if let Err(error) = post_replay_finalize(&replay_path).await {
                            error!(
                                "[replay] keeping explicit rejected result for match {match_id}: {error}"
                            );
                        }
                    }
                    Err(error) => {
                        error!(
                            "[replay] could not persist explicit rejected result for match {match_id}: {error}"
                        );
                    }
                }
                return;
            }
        };
        if let Err(error) = post_replay_finalize(&replay_path).await {
            error!("[CRITICAL] Keeping pending match {match_id} in local replay spool: {error}");
        }
    });
}

// One participation owner; transport channels never decide final exit.
#[derive(Clone, Debug)]
enum Participation {
    Active,
    Disconnected { since: Instant },
    Closing { turn: u64, reason: ExitReason },
    Closed { turn: u64, reason: ExitReason },
}

const RECONNECT_GRACE: Duration = Duration::from_secs(60);

struct MatchTracker {
    player_accounts: HashMap<u16, String>,
    human_accounts: HashSet<String>,
    participants: HashMap<u16, Participation>,
    tracked: bool,
    lobby_json: String,
}

impl MatchTracker {
    fn finalized(&self) -> bool {
        self.tracked
            && self.participants.values().all(|state| {
                matches!(
                    state,
                    Participation::Closing { .. } | Participation::Closed { .. }
                )
            })
    }

    fn is_sealed(&self, player_id: u16) -> bool {
        self.participants.get(&player_id).is_none_or(|state| {
            matches!(
                state,
                Participation::Closing { .. } | Participation::Closed { .. }
            )
        })
    }

    fn disconnect(&mut self, player_id: u16, now: Instant) {
        if let Some(state @ Participation::Active) = self.participants.get_mut(&player_id) {
            *state = Participation::Disconnected { since: now };
        }
    }

    fn reconnect(&mut self, player_id: u16, now: Instant) -> bool {
        let Some(state) = self.participants.get_mut(&player_id) else {
            return false;
        };
        match state {
            Participation::Active => true,
            Participation::Disconnected { since }
                if now.duration_since(*since) < RECONNECT_GRACE =>
            {
                *state = Participation::Active;
                true
            }
            _ => false,
        }
    }

    fn expired(&self, now: Instant) -> Vec<u16> {
        self.participants
            .iter()
            .filter_map(|(id, state)| match state {
                Participation::Disconnected { since }
                    if now.duration_since(*since) >= RECONNECT_GRACE =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .collect()
    }

    fn record_exit(
        &mut self,
        player_id: u16,
        turn: u64,
        reason: ExitReason,
    ) -> (bool, Option<String>) {
        if self.is_sealed(player_id) {
            return (false, None);
        }
        self.participants
            .insert(player_id, Participation::Closing { turn, reason });
        let account = self
            .player_accounts
            .get(&player_id)
            .filter(|account| self.human_accounts.contains(*account))
            .cloned();
        (true, account)
    }

    fn record_committed_closure(&mut self, account_id: String, turn_number: u64) {
        let Some(id) = self
            .player_accounts
            .iter()
            .find(|(_, account)| **account == account_id)
            .map(|(id, _)| *id)
        else {
            return;
        };
        if let Some(state @ Participation::Closing { .. }) = self.participants.get_mut(&id) {
            let Participation::Closing { turn, reason } = *state else {
                unreachable!()
            };
            debug_assert_eq!(turn, turn_number);
            *state = Participation::Closed { turn, reason };
        }
    }

    fn closure(&self, account_id: &str) -> Option<(u64, ExitReason)> {
        let id = self
            .player_accounts
            .iter()
            .find(|(_, account)| account.as_str() == account_id)?
            .0;
        match self.participants.get(id)? {
            Participation::Closed { turn, reason } => Some((*turn, *reason)),
            _ => None,
        }
    }

    fn participant_closures(&self) -> Vec<ParticipantClosure> {
        let mut closures = self
            .player_accounts
            .iter()
            .filter_map(|(id, account)| match self.participants.get(id) {
                Some(
                    Participation::Closing { turn, reason }
                    | Participation::Closed { turn, reason },
                ) => Some(ParticipantClosure {
                    account_id: account.clone(),
                    turn_number: *turn,
                    reason: *reason,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        closures.sort_by(|a, b| a.account_id.cmp(&b.account_id));
        closures
    }
}

// ---- registry / lobby state -------------------------------------------------

/// Register body — the orchestrator's lobby shape plus the dynamic game port
/// that this worker must bind before Start is broadcast.
#[derive(serde::Deserialize, serde::Serialize)]
struct RegisterBody {
    lobby_id: u64,
    relay_port: u16,
    #[serde(default)]
    ticket_expires_at: u64,
    tick_number: u64,
    tick_rate_ms: f32,
    players: Vec<PlayerEntry>,
    #[serde(default)]
    config: Option<GameConfig>,
    #[serde(default)]
    kind: String,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct PlayerEntry {
    player_id: u16,
    name: String,
    database_account_id: Option<String>,
    #[serde(default)]
    leader: Leader,
    #[serde(default)]
    team: Option<Team>,
    #[serde(default)]
    session_id: Option<u64>,
    #[serde(default)]
    relay_ticket_digest: Option<String>,
    #[serde(default)]
    is_internal: bool,
}

struct LobbyState {
    id: u64,
    relay_port: u16,
    ticket_expires_at: u64,
    roster_total: usize,
    internal_bot_count: usize,
    expected_external_connections: usize,
    valid_players: HashMap<u16, String>,
    session_ids: HashMap<u16, u64>,
    ticket_digests: HashMap<u16, [u8; 32]>,
    /// Authentication state is separate from the player roster: the initial
    /// relay ticket is one-shot, while the reconnect capability is rotated
    /// after every accepted reconnect. Only SHA-256 digests are retained.
    auth: Arc<Mutex<RelayAuthState>>,
    clients: Arc<Mutex<HashMap<u16, ClientChannel>>>,
    journal: Arc<ReplayJournal>,
    tracker: Arc<std::sync::Mutex<MatchTracker>>,
    ev_tx: mpsc::Sender<RelayEvent>,
}

#[derive(Default)]
struct RelayAuthState {
    initial_used: HashSet<u16>,
    reconnect_digests: HashMap<u16, [u8; 32]>,
}

type Registry = Arc<RwLock<HashMap<u64, Arc<LobbyState>>>>;

// ---- main (bridge boot — same shape as the fstack-bridge examples) ----------

fn main() {
    env_logger::init();
    if std::env::var("SOW_RELAY_SETTLEMENT_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        eprintln!("SOW_RELAY_SETTLEMENT_SECRET must be set; refusing shared settlement auth");
        std::process::exit(78);
    }
    if std::env::var("SOW_RELAY_CONTROL_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        eprintln!("SOW_RELAY_CONTROL_SECRET must be set; refusing unauthenticated management");
        std::process::exit(78);
    }
    if let Err(e) = validate_runtime_security() {
        eprintln!("{e}");
        std::process::exit(78);
    }

    let admission = match RelayAdmissionPolicy::from_env() {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("[sow-relay] invalid admission-control configuration: {e}");
            std::process::exit(78);
        }
    };
    if RELAY_ADMISSION.set(admission).is_err() {
        eprintln!("[sow-relay] admission policy initialized twice");
        std::process::exit(78);
    }
    if let Err(e) = bridge::set_accept_filter(relay_accept_filter) {
        eprintln!("[sow-relay] failed to install accept filter: {e}");
        std::process::exit(78);
    }
    if let Err(e) = bridge::set_close_hook(relay_close_hook) {
        eprintln!("[sow-relay] failed to install close hook: {e}");
        std::process::exit(78);
    }
    let policy = RELAY_ADMISSION.get().expect("admission policy installed");
    info!(
        "[BOOT] admission max_connections={} max_connections_per_ip={} handshakes_per_ip={}",
        policy.max_connections, policy.max_connections_per_ip, policy.handshakes_per_ip
    );
    // Identity line: the pipeline stamps git/f-stack/knob into the unit drop-in
    // so every deployed build is attributable from the journal alone.
    info!(
        "[BOOT] git={} fstack={} ws_write_timeout_ms={}",
        std::env::var("SOW_RELAY_GIT").unwrap_or_else(|_| "unknown".to_string()),
        std::env::var("SOW_FSTACK_VERSION").unwrap_or_else(|_| "unknown".to_string()),
        ws_write_timeout_ms()
    );

    let prog_args: Vec<CString> = std::env::args()
        .filter(|a| !a.starts_with("--fstack-"))
        .map(|a| CString::new(a).unwrap())
        .collect();
    let base_mgmt = base_mgmt_port();

    // TLS: load cert if configured, otherwise plain ws://.
    let tls_enabled = current_tls_acceptor().is_some();
    if tls_enabled {
        info!("[BOOT] TLS enabled — browser connects via wss:// directly");
    } else {
        info!("[BOOT] TLS disabled — plain ws:// (dev mode)");
    }
    if std::env::var("SOW_MGMT_TLS_REQUIRED").ok().as_deref() == Some("1") && !tls_enabled {
        error!("[sow-relay] management TLS is required but the relay certificate is unavailable");
        std::process::exit(78);
    }

    // TAP dev loop: inject --no-pci + net_tap0 vdev (FSTACK_TAP=1). Empty for the physical VF.
    let mut extra_eal: Vec<&str> = Vec::new();
    if std::env::var("FSTACK_TAP").ok().as_deref() == Some("1") {
        extra_eal.push("--no-pci");
        extra_eal.push("--iova-mode=va"); // TAP vdev needs VA IOVA (no physical NIC for PA)
        extra_eal.push("--vdev=net_tap0,iface=tap0");
        info!("[sow-relay] TAP mode (--no-pci + --iova-mode=va + net_tap0)");
    } else {
        info!("[sow-relay] physical VF mode (config.ini [dpdk] allow=)");
    }

    unsafe {
        if let Err(code) = fstack_bridge::init(&prog_args, &extra_eal) {
            error!("[sow-relay] init failed (code={})", code);
            std::process::exit(1);
        }

        let (mut pid, mut qid, mut nbq, mut reta) = (0u16, 0u16, 0u16, 0u16);
        if fstack_bridge::ff_rss_self_queue_info(&mut pid, &mut qid, &mut nbq, &mut reta) != 0 {
            error!("[sow-relay] unable to inspect F-Stack worker queue");
            std::process::exit(1);
        }
        if qid >= nbq || nbq == 0 {
            error!(
                "[sow-relay] invalid F-Stack queue assignment: proc_id={pid} queue_id={qid} nb_queues={nbq}"
            );
            std::process::exit(1);
        }
        let expected_workers = expected_worker_count();
        if nbq != expected_workers {
            error!(
                "[sow-relay] dynamic routing requires {} F-Stack queues, got {}",
                expected_workers, nbq
            );
            std::process::exit(1);
        }
        if pid != qid {
            error!(
                "[sow-relay] F-Stack proc_id must equal queue_id for worker ownership: proc_id={pid} queue_id={qid}"
            );
            std::process::exit(1);
        }
        let mport = match worker_port(base_mgmt, qid) {
            Ok(port) => port,
            Err(e) => {
                error!("[sow-relay] {e}");
                std::process::exit(1);
            }
        };
        WORKER_QUEUE_ID.store(qid, Ordering::Relaxed);
        WORKER_QUEUE_COUNT.store(nbq, Ordering::Relaxed);
        info!(
            "[BOOT] proc_id={} queue_id={} nb_queues={} reta_size={} dynamic_ports={}..65535 mgmt_port={}",
            pid, qid, nbq, reta, DYNAMIC_PORT_MIN, mport
        );
        fstack_bridge::ff_regist_packet_dispatcher(relay_packet_dispatcher);

        bridge::setup();
        bridge::KQ = fstack_bridge::ff_kqueue();
        if bridge::KQ < 0 {
            error!("[sow-relay] ff_kqueue failed");
            std::process::exit(1);
        }

        let registry: Registry = Arc::new(RwLock::new(HashMap::new()));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.spawn(replay_spool_worker());
        rt.spawn(mgmt_http(registry.clone(), mport));
        rt.spawn(bridge_worker(registry.clone()));

        info!(
            "[BOOT] dynamic game listeners, mgmt :{}, entering ff_run",
            mport
        );
        fstack_bridge::ff_run(bridge::driver_cb, ptr::null_mut());
    }
}

// ---- mgmt HTTP (kernel — never touches the bridge) ----------------
//
// Bind address is SOW_MGMT_LISTEN (default 127.0.0.1). Production sets it to
// 0.0.0.0 so the orchestrator (other host) can register lobbies; access is
// then locked down by the cloud NSG to the orchestrator's IP only.

fn mgmt_listen() -> String {
    std::env::var("SOW_MGMT_LISTEN").unwrap_or_else(|_| "127.0.0.1".to_string())
}

type MgmtNonceCache = Arc<Mutex<HashMap<String, u64>>>;

fn mgmt_header<'a>(headers: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    headers.get(&name.to_ascii_lowercase()).map(String::as_str)
}

async fn verify_mgmt_auth(
    method: &str,
    path: &str,
    body: &[u8],
    headers: &HashMap<String, String>,
    nonce_cache: &MgmtNonceCache,
) -> bool {
    let Some(secret) = std::env::var("SOW_RELAY_CONTROL_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        error!("[mgmt] SOW_RELAY_CONTROL_SECRET is missing");
        return false;
    };
    let Some(timestamp) = mgmt_header(headers, "x-sow-timestamp") else {
        return false;
    };
    let Some(nonce) = mgmt_header(headers, "x-sow-nonce") else {
        return false;
    };
    let Some(signature) = mgmt_header(headers, "x-sow-signature") else {
        return false;
    };
    if nonce.is_empty() || nonce.len() > 128 {
        return false;
    }
    let Ok(timestamp) = timestamp.parse::<u64>() else {
        return false;
    };
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return false;
    };
    if now.as_secs().abs_diff(timestamp) > MGMT_CLOCK_SKEW_SECS {
        return false;
    }
    let Ok(provided) = hex::decode(signature) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(method.as_bytes());
    mac.update(b"\n");
    mac.update(path.as_bytes());
    mac.update(b"\n");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    if mac.verify_slice(&provided).is_err() {
        return false;
    }

    let mut cache = nonce_cache.lock().await;
    let cutoff = now.as_secs().saturating_sub(MGMT_NONCE_TTL_SECS);
    cache.retain(|_, seen| *seen >= cutoff);
    if cache.contains_key(nonce) {
        return false;
    }
    if cache.len() >= 4096 {
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, seen)| **seen)
            .map(|(nonce, _)| nonce.clone())
        {
            cache.remove(&oldest);
        }
    }
    cache.insert(nonce.to_string(), now.as_secs());
    true
}

async fn bind_game_port(port: u16) -> Result<(), String> {
    if !(DYNAMIC_PORT_MIN..=u16::MAX).contains(&port) {
        return Err(format!("relay port {port} outside dynamic range"));
    }
    let newly_bound = bridge::listen_port(port).await?;
    if !newly_bound {
        return Ok(());
    }
    LISTENER_COUNT.fetch_add(1, Ordering::Relaxed);

    let redis_con = redis_shared();
    tokio::task::spawn_blocking(move || {
        let mut guard = redis_con.lock().unwrap();
        if let Some(ref mut con) = *guard {
            if let Err(e) = con.sadd::<_, _, ()>("sow:ports", port) {
                error!("[REDIS] SADD sow:ports {port} FAILED: {e}");
            }
        }
    });
    Ok(())
}

async fn release_game_port(port: u16) {
    let removed = match bridge::unlisten_port(port).await {
        Ok(removed) => removed,
        Err(e) => {
            error!("[relay] failed to close dynamic listener {port}: {e}");
            return;
        }
    };
    if !removed {
        return;
    }
    LISTENER_COUNT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            Some(n.saturating_sub(1))
        })
        .ok();

    let redis_con = redis_shared();
    tokio::task::spawn_blocking(move || {
        let mut guard = redis_con.lock().unwrap();
        if let Some(ref mut con) = *guard {
            if let Err(e) = con.srem::<_, _, ()>("sow:ports", port) {
                error!("[REDIS] SREM sow:ports {port} FAILED: {e}");
            }
        }
    });
}

async fn mgmt_http(registry: Registry, mport: u16) {
    let listen = mgmt_listen();
    let listener = match TcpListener::bind((listen.as_str(), mport)).await {
        Ok(l) => l,
        Err(e) => {
            error!("[mgmt] bind {}:{} failed: {}", listen, mport, e);
            return;
        }
    };
    info!("[mgmt] http listening on {}:{}", listen, mport);
    let nonce_cache: MgmtNonceCache = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (sock, _) = match listener.accept().await {
            Ok(x) => x,
            Err(_) => continue,
        };
        let reg = registry.clone();
        let nonce_cache = nonce_cache.clone();
        let tls_acceptor = current_tls_acceptor();
        tokio::spawn(async move {
            if let Some(acceptor) = tls_acceptor {
                match acceptor.accept(sock).await {
                    Ok(tls) => mgmt_connection(tls, reg, nonce_cache).await,
                    Err(e) => warn!("[mgmt] TLS handshake failed: {e}"),
                }
            } else {
                mgmt_connection(sock, reg, nonce_cache).await;
            }
        });
    }
}

async fn mgmt_connection<S>(mut sock: S, registry: Registry, nonce_cache: MgmtNonceCache)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end;
    loop {
        match sock.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = i + 4;
            break;
        }
        if buf.len() > 65536 {
            return;
        }
    }

    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let mut parts = lines.next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    let clen: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    if clen > MAX_MGMT_BODY_BYTES {
        let _ = sock
            .write_all(
                b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await;
        return;
    }

    while buf.len() < header_end + clen && clen > 0 {
        match sock.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    }
    if buf.len() < header_end + clen {
        return;
    }

    let body = &buf[header_end..header_end + clen];
    let authorized =
        path == "/healthz" || verify_mgmt_auth(&method, &path, body, &headers, &nonce_cache).await;
    let (status, resp) = if authorized {
        handle_http(&registry, &method, &path, body).await
    } else {
        (
            "401 Unauthorized",
            serde_json::json!({ "error": "unauthorized" }),
        )
    };
    let resp_body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
    let out = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        resp_body.len(),
        resp_body
    );
    let _ = sock.write_all(out.as_bytes()).await;
}

async fn handle_http(
    registry: &Registry,
    method: &str,
    path: &str,
    body: &[u8],
) -> (&'static str, serde_json::Value) {
    match (method, path) {
        ("POST", "/internal/match/exit") => {
            let command = match serde_json::from_slice::<sow_data::profile::MatchExitCommand>(body)
            {
                Ok(command)
                    if command.match_id.parse::<u64>().is_ok()
                        && sow_data::profile::is_valid_account_id(&command.account_id) =>
                {
                    command
                }
                _ => {
                    return (
                        "400 Bad Request",
                        serde_json::json!({"error":"invalid match participant"}),
                    );
                }
            };
            let id = command.match_id.parse::<u64>().unwrap();
            let lobby = registry.read().await.get(&id).cloned();
            let ack = |status: &str, turn: Option<u64>| {
                serde_json::json!({
                    "status":status,"match_id":command.match_id,"account_id":command.account_id,"closure_turn":turn,
                })
            };
            let path = participant_spool_path(&command.match_id, &command.account_id);
            if let Ok(bytes) = tokio::fs::read(&path).await {
                if let Ok(payload) = serde_json::from_slice::<ParticipantSettlementPayload>(&bytes)
                {
                    if payload.match_id == command.match_id
                        && payload.account_id == command.account_id
                    {
                        match tokio::fs::read(participant_replay_path(&path)).await {
                            Ok(replay)
                                if hex::encode(Sha256::digest(&replay))
                                    == payload.replay_sha256 =>
                            {
                                if let Err(error) = std::fs::File::open(path.parent().unwrap())
                                    .and_then(|directory| directory.sync_all())
                                {
                                    return (
                                        "503 Service Unavailable",
                                        serde_json::json!({"error":format!("exit not durable: {error}")}),
                                    );
                                }
                                return ("200 OK", ack("recorded", Some(payload.closure_turn)));
                            }
                            _ => {} // A concurrently accepted upload may have removed its copy.
                        }
                    }
                }
            }
            let Some(lobby) = lobby else {
                match tokio::fs::try_exists(replay_spool_dir().join(format!("{id}.json"))).await {
                    Ok(true) => {
                        let metadata_path = replay_spool_dir().join(format!("{id}.json"));
                        let recovery = async {
                            let bytes = tokio::fs::read(&metadata_path)
                                .await
                                .map_err(|error| error.to_string())?;
                            let mut metadata =
                                serde_json::from_slice::<ReplaySpoolMetadataCompat>(&bytes)
                                    .map_err(|error| error.to_string())?
                                    .into_current();
                            if metadata.phase == ReplayPhase::Recording {
                                recover_recording(&metadata_path, &mut metadata).await?;
                            }
                            Ok::<_, String>(())
                        }
                        .await;
                        if let Err(error) = recovery {
                            return (
                                "503 Service Unavailable",
                                serde_json::json!({"error":error}),
                            );
                        }
                        return ("200 OK", ack("awaiting_result", None));
                    }
                    Ok(false) => {}
                    Err(error) => {
                        return (
                            "503 Service Unavailable",
                            serde_json::json!({"error":format!("pending result unavailable: {error}")}),
                        );
                    }
                }
                if command.owner_verified {
                    match recover_detached_exit(&replay_spool_dir(), &command).await {
                        Ok(job) => {
                            let turn = job.payload.closure_turn;
                            dispatch_participant_settlements(vec![job]);
                            return ("200 OK", ack("recorded", Some(turn)));
                        }
                        Err(error) => {
                            return (
                                "503 Service Unavailable",
                                serde_json::json!({"error":error}),
                            );
                        }
                    }
                }
                return (
                    "404 Not Found",
                    serde_json::json!({"error":"no active match or pending result on this relay"}),
                );
            };
            let (reply, response) = oneshot::channel();
            if !matches!(
                tokio::time::timeout(
                    Duration::from_secs(3),
                    lobby.ev_tx.send(RelayEvent::ExitRequested {
                        account_id: command.account_id.clone(),
                        reply
                    })
                )
                .await,
                Ok(Ok(()))
            ) {
                return (
                    "503 Service Unavailable",
                    serde_json::json!({"error":"exit recorder unavailable"}),
                );
            }
            match tokio::time::timeout(Duration::from_secs(5), response).await {
                Ok(Ok(Ok(turn))) => ("200 OK", ack("recorded", Some(turn))),
                Ok(Ok(Err(error))) => ("400 Bad Request", serde_json::json!({"error":error})),
                _ => (
                    "503 Service Unavailable",
                    serde_json::json!({"error":"exit not yet durably recorded"}),
                ),
            }
        }
        ("GET", "/healthz") => ("200 OK", serde_json::json!({ "ok": true })),
        ("POST", "/internal/lobby/register") => {
            let rb: RegisterBody = match serde_json::from_slice(body) {
                Ok(b) => b,
                Err(e) => {
                    return (
                        "400 Bad Request",
                        serde_json::json!({ "error": format!("bad body: {e}") }),
                    );
                }
            };
            if rb.lobby_id == 0 {
                return (
                    "400 Bad Request",
                    serde_json::json!({ "error": "lobby_id required" }),
                );
            }
            if rb.ticket_expires_at == 0 {
                return (
                    "400 Bad Request",
                    serde_json::json!({ "error": "ticket_expires_at required" }),
                );
            }
            if rb.players.iter().any(|player| {
                !player.is_internal
                    && player
                        .relay_ticket_digest
                        .as_deref()
                        .and_then(decode_ticket_digest)
                        .is_none()
            }) {
                return (
                    "400 Bad Request",
                    serde_json::json!({ "error": "relay ticket digest required for every network player" }),
                );
            }
            if !(DYNAMIC_PORT_MIN..=u16::MAX).contains(&rb.relay_port) {
                return (
                    "400 Bad Request",
                    serde_json::json!({ "error": "relay_port must be in 1024..=65535" }),
                );
            }
            let queue_id = WORKER_QUEUE_ID.load(Ordering::Relaxed);
            let queue_count = WORKER_QUEUE_COUNT.load(Ordering::Relaxed);
            if queue_count == 0 || rb.relay_port % queue_count != queue_id {
                return (
                    "409 Conflict",
                    serde_json::json!({
                        "error": "relay_port belongs to another worker",
                        "relay_port": rb.relay_port,
                        "worker": rb.relay_port % queue_count.max(1),
                        "this_worker": queue_id,
                    }),
                );
            }
            if let Err(e) = bind_game_port(rb.relay_port).await {
                return ("409 Conflict", serde_json::json!({ "error": e }));
            }
            let existing = registry.read().await.get(&rb.lobby_id).cloned();
            if let Some(existing) = existing {
                if existing.relay_port != rb.relay_port {
                    return (
                        "409 Conflict",
                        serde_json::json!({ "error": "lobby already owns another relay port" }),
                    );
                }
                return (
                    "200 OK",
                    serde_json::json!({ "ok": true, "existing": true }),
                );
            }
            let port = rb.relay_port;
            if let Err(error) = spawn_lobby(registry, rb).await {
                release_game_port(port).await;
                return (
                    "503 Service Unavailable",
                    serde_json::json!({"error":format!("cannot persist match registration: {error}")}),
                );
            }
            (
                "200 OK",
                serde_json::json!({ "ok": true, "existing": false, "relay_port": port }),
            )
        }
        ("GET", "/internal/lobbies") => {
            let reg = registry.read().await;
            let lobbies: Vec<serde_json::Value> = reg
                .iter()
                .map(|(id, st)| {
                    serde_json::json!({
                        "lobby_id": id,
                        "relay_port": st.relay_port,
                        "roster_total": st.roster_total,
                        "internal_bot_count": st.internal_bot_count,
                        "expected_external_connections": st.expected_external_connections,
                        "active_relay_connections": st.clients.try_lock().map(|c| c.len()).unwrap_or(0),
                    })
                })
                .collect();
            ("200 OK", serde_json::json!({ "lobbies": lobbies }))
        }
        ("GET", "/internal/metrics") => (
            "200 OK",
            serde_json::json!({
                "proc_id": std::process::id(),
                "listener_count": LISTENER_COUNT.load(Ordering::Relaxed),
                "queue_id": WORKER_QUEUE_ID.load(Ordering::Relaxed),
                "queue_count": WORKER_QUEUE_COUNT.load(Ordering::Relaxed),
                "dispatch_local": DISPATCH_LOCAL.load(Ordering::Relaxed),
                "dispatch_redirected": DISPATCH_REDIRECTED.load(Ordering::Relaxed),
                "dispatch_unmatched": DISPATCH_UNMATCHED.load(Ordering::Relaxed),
                "admission": RELAY_ADMISSION
                    .get()
                    .map(RelayAdmissionPolicy::metrics)
                    .unwrap_or_else(|| serde_json::json!({ "initialized": false })),
            }),
        ),
        ("POST", "/internal/lobby/close") => {
            let lobby_id: u64 = serde_json::from_slice::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v.get("lobby_id").and_then(|x| x.as_u64()))
                .unwrap_or(0);
            if lobby_id == 0 {
                return (
                    "400 Bad Request",
                    serde_json::json!({ "error": "lobby_id required" }),
                );
            }
            let state = registry.read().await.get(&lobby_id).cloned();
            let removed = state.is_some();
            if let Some(state) = state {
                if !matches!(
                    tokio::time::timeout(
                        Duration::from_secs(3),
                        state.ev_tx.send(RelayEvent::EndMatch)
                    )
                    .await,
                    Ok(Ok(()))
                ) {
                    return (
                        "503 Service Unavailable",
                        serde_json::json!({"error":"match close recorder unavailable"}),
                    );
                }
            }
            info!(
                "[relay] lobby {} closed via mgmt API (removed={})",
                lobby_id, removed
            );
            (
                "200 OK",
                serde_json::json!({ "ok": true, "removed": removed }),
            )
        }
        _ => ("404 Not Found", serde_json::json!({ "error": "not found" })),
    }
}

async fn spawn_lobby(registry: &Registry, body: RegisterBody) -> Result<Arc<LobbyState>, String> {
    let spawn_phase_end_tick = deployment_end_tick(body.config.as_ref(), body.tick_rate_ms);
    let roster_total = body.players.len();
    let internal_bot_count = body.players.iter().filter(|p| p.is_internal).count();
    let expected_external_connections = roster_total.saturating_sub(internal_bot_count);
    let mut lobby_value = serde_json::to_value(&body).unwrap_or_default();
    if let Some(players) = lobby_value
        .get_mut("players")
        .and_then(|value| value.as_array_mut())
    {
        for player in players {
            if let Some(object) = player.as_object_mut() {
                object.remove("relay_ticket_digest");
            }
        }
    }
    let lobby_json = serde_json::to_string(&lobby_value).unwrap_or_default();
    let mut valid_players: HashMap<u16, String> = HashMap::new();
    let mut session_ids: HashMap<u16, u64> = HashMap::new();
    let mut ticket_digests: HashMap<u16, [u8; 32]> = HashMap::new();
    let mut player_accounts: HashMap<u16, String> = HashMap::new();
    let mut human_accounts = HashSet::new();
    let mut external_player_ids = HashSet::new();
    for p in &body.players {
        valid_players.insert(p.player_id, p.name.clone());
        if !p.is_internal {
            external_player_ids.insert(p.player_id);
        }
        if let Some(session_id) = p.session_id {
            session_ids.insert(p.player_id, session_id);
        }
        if let Some(digest) = p
            .relay_ticket_digest
            .as_deref()
            .and_then(decode_ticket_digest)
        {
            ticket_digests.insert(p.player_id, digest);
        }
        if let Some(acc) = &p.database_account_id {
            player_accounts.insert(p.player_id, acc.clone());
            if !p.is_internal {
                human_accounts.insert(acc.clone());
            }
        }
    }

    let clients = Arc::new(Mutex::new(HashMap::new()));
    let journal = ReplayJournal::new(body.lobby_id, &lobby_json).await?;
    let (ev_tx, ev_rx) = mpsc::channel::<RelayEvent>(EVENT_CHANNEL);
    let tracked = !human_accounts.is_empty();
    let tracker = Arc::new(std::sync::Mutex::new(MatchTracker {
        player_accounts,
        human_accounts,
        participants: external_player_ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    Participation::Disconnected {
                        since: Instant::now(),
                    },
                )
            })
            .collect(),
        tracked,
        lobby_json: lobby_json.clone(),
    }));

    let state = Arc::new(LobbyState {
        id: body.lobby_id,
        relay_port: body.relay_port,
        ticket_expires_at: body.ticket_expires_at,
        roster_total,
        internal_bot_count,
        expected_external_connections,
        valid_players,
        session_ids,
        ticket_digests,
        auth: Arc::new(Mutex::new(RelayAuthState::default())),
        clients,
        journal,
        tracker,
        ev_tx,
    });

    info!(
        "[registry] lobby={} roster_total={} internal_bot_count={} expected_external_connections={} active_relay_connections=0 tick_rate_ms={} tracked={}",
        state.id,
        state.roster_total,
        state.internal_bot_count,
        state.expected_external_connections,
        body.tick_rate_ms,
        tracked
    );

    registry.write().await.insert(state.id, state.clone());

    // Per-lobby Redis registration: sow:relay:{lobby_id} -> relay addr (TTL 60,
    // refreshed by the tick loop heartbeat every 10s).
    {
        let redis_con = redis_shared();
        let key = format!("sow:relay:{}", state.id);
        let val = relay_addr(body.relay_port);
        tokio::task::spawn_blocking(move || {
            let mut guard = redis_con.lock().unwrap();
            if let Some(ref mut con) = *guard {
                if let Err(e) = con.set_ex::<_, _, ()>(&key, val, 60) {
                    error!("[REDIS] SETEX {key} FAILED: {e}");
                }
            }
        });
    }

    tokio::spawn(tick_task(
        state.clone(),
        registry.clone(),
        ev_rx,
        body.tick_rate_ms as u64,
        body.tick_number,
        spawn_phase_end_tick,
    ));
    Ok(state)
}

// ---- per-lobby tick loop -----------------------------------------------------

fn deployment_end_tick(config: Option<&GameConfig>, fallback_tick_rate_ms: f32) -> Option<u64> {
    if config.is_some_and(|config| config.random_spawn) {
        return None;
    }
    let tick_rate_ms = config.map_or(fallback_tick_rate_ms, |config| config.tick_rate_ms);
    Some(sow_core::game::deployment_phase_end_tick(tick_rate_ms))
}

fn admit_gameplay_intent(
    player_id: u16,
    intent: &GameplayIntent,
    tick_number: u64,
    tick_rate_ms: f32,
    spawn_phase_end_tick: Option<u64>,
    spawn_rate_limits: &mut HashMap<u16, SpawnIntentRateLimit>,
) -> bool {
    if !matches!(intent, GameplayIntent::Spawn { .. })
        || !spawn_phase_end_tick.is_some_and(|end_tick| tick_number < end_tick)
    {
        return true;
    }
    spawn_rate_limits
        .entry(player_id)
        .or_default()
        .admit_at_tick(tick_number, tick_rate_ms)
}

type ExitReplies = HashMap<String, Vec<oneshot::Sender<Result<u64, String>>>>;

fn acknowledge_exit(replies: &mut ExitReplies, account_id: &str, turn: u64) {
    if let Some(waiters) = replies.remove(account_id) {
        for reply in waiters {
            let _ = reply.send(Ok(turn));
        }
    }
}

async fn tick_task(
    state: Arc<LobbyState>,
    registry: Registry,
    mut ev_rx: mpsc::Receiver<RelayEvent>,
    tick_rate_ms: u64,
    mut tick_number: u64,
    spawn_phase_end_tick: Option<u64>,
) {
    let mut ticker = interval(Duration::from_millis(tick_rate_ms));
    let mut last_status = std::time::Instant::now();
    let mut pending_intents = Vec::new();
    let mut pending_closures = Vec::<(String, u64)>::new();
    let mut pending_closure_checkpoint_since = HashMap::<u64, Instant>::new();
    let mut pending_participant_settlements = Vec::<PendingParticipantSettlement>::new();
    let mut exit_replies = ExitReplies::new();
    let mut replay_recording_failed = false;
    let mut end_requested = false;
    let (settlement_result_tx, mut settlement_result_rx) = mpsc::unbounded_channel();
    let mut spawn_rate_limits = HashMap::<u16, SpawnIntentRateLimit>::new();
    let mut total_ticks: u64 = 0;
    let mut total_intents: u64 = 0;
    // Lifecycle: a lobby is removed from the registry and its live replay
    // dropped when the match ends (GameOver / all clients leave) OR when it
    // outlives the maximum match duration. Backfill bots play indefinitely, so
    // without this ceiling a filled lobby keeps pushing to live history forever
    // and a relay worker OOMs (5.5GB in ~4min under 50k bots).
    let match_started = std::time::Instant::now();
    let max_match_secs: u64 = std::env::var("SOW_RELAY_MATCH_MAX_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1800);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let mut clients = state.clients.lock().await;
                let recorded = retry_pending_participant_settlements(
                    state.id,
                    &mut pending_participant_settlements,
                    &settlement_result_tx,
                );
                for (account_id, turn) in recorded {
                    state.tracker.lock().unwrap().record_committed_closure(account_id.clone(), turn);
                    acknowledge_exit(&mut exit_replies, &account_id, turn);
                }
                let active_relay_connections = clients.len();
                let already_finalized = state.tracker.lock().unwrap().finalized();

                let now = Instant::now();
                let ending = end_requested || match_started.elapsed().as_secs() >= max_match_secs;
                let expired = if ending {
                    state.tracker.lock().unwrap().participants.keys().copied().collect()
                } else { state.tracker.lock().unwrap().expired(now) };
                for player_id in expired {
                    let reason = if ending { ExitReason::MatchEnded } else { ExitReason::Timeout };
                    let (new_exit, account_id) = state.tracker.lock().unwrap().record_exit(player_id, tick_number, reason);
                    if new_exit {
                        if !ending { clients.remove(&player_id); }
                        pending_intents.push(StampedIntent { player_id, intent: GameplayIntent::Resign });
                        if let Some(account_id) = account_id {
                            info!("[exit-timing] timeout match={} player={} turn={} grace_ms={}", state.id, player_id, tick_number, RECONNECT_GRACE.as_millis());
                            pending_closure_checkpoint_since.entry(tick_number).or_insert_with(Instant::now);
                            pending_closures.push((account_id, tick_number));
                        }
                    }
                }

                // Lifecycle: enforce the maximum match duration. Broadcast
                // LobbyClosed so clients exit cleanly, finalize the replay to
                // the database, then drop the lobby from the registry. The
                // complete replay is already on disk; only live history is RAM.
                let is_finalized = already_finalized;
                let is_timed_out = ending;
                if (is_finalized || is_timed_out)
                    && pending_intents.is_empty()
                    && pending_closures.is_empty()
                    && pending_participant_settlements.is_empty()
                {
                    let reason = if is_finalized {
                        "Match finalized".to_string()
                    } else {
                        format!("Match duration limit reached ({}s)", max_match_secs)
                    };
                    info!("[relay] lobby {} ending: {}, finalizing & cleaning up", state.id, reason);
                    if let Ok(json) = bincode::serialize(
                        &ServerMessage::LobbyClosed(ServerLobbyClosedMessage {
                            lobby_id: state.id,
                            reason,
                        }),
                    ) {
                        let frame = Arc::new(json);
                        for client in clients.values() {
                            let _ = client.sender.try_send(frame.clone());
                        }
                    }
                    drop(clients);
                    let (lobby_json, closures) = {
                        let tracker = state.tracker.lock().unwrap();
                        (tracker.lobby_json.clone(), tracker.participant_closures())
                    };
                    trigger_match_finalize(
                        state.id,
                        lobby_json,
                        closures,
                        state.journal.clone(),
                    );
                    registry.write().await.remove(&state.id);
                    release_game_port(state.relay_port).await;
                    break;
                }

                let intents = std::mem::take(&mut pending_intents);
                let turn = Turn {
                    turn_number: tick_number,
                    intents,
                };
                tick_number += 1;
                total_ticks += 1;
                total_intents += turn.intents.len() as u64;

                if let Err(error) = state.journal.append(turn.clone()).await {
                    error!("[replay] lobby {} append failed; keeping match active: {error}", state.id);
                    replay_recording_failed = true;
                }

                if replay_recording_failed {
                    for (account_id, closure_turn) in std::mem::take(&mut pending_closures) {
                        pending_participant_settlements.push(PendingParticipantSettlement {
                            account_id,
                            closure_turn,
                            replay_bytes: Vec::new(),
                            rejected: true,
                            retry_count: 0,
                            direct_in_flight: false,
                            direct_retry_after: None,
                            queued_at: Instant::now(),
                        });
                    }
                    pending_closure_checkpoint_since.clear();
                } else {
                    let mut closure_groups = std::collections::BTreeMap::<u64, Vec<String>>::new();
                    for (account_id, closure_turn) in &pending_closures {
                        closure_groups
                            .entry(*closure_turn)
                            .or_default()
                            .push(account_id.clone());
                    }
                    for (closure_turn, accounts) in closure_groups {
                        let closures = state.tracker.lock().unwrap().participant_closures().into_iter()
                            .filter(|closure| closure.turn_number <= closure_turn).collect();
                        match state.journal.checkpoint(closure_turn, closures).await {
                            Ok(replay_bytes) => {
                                pending_closures
                                    .retain(|(_, turn_number)| *turn_number != closure_turn);
                                let record_started = pending_closure_checkpoint_since.remove(&closure_turn);
                                for account_id in accounts {
                                    match prepare_participant_settlement(
                                        state.id,
                                        account_id.clone(),
                                        closure_turn,
                                        replay_bytes.clone(),
                                        false,
                                    ) {
                                        Ok(job) => {
                                            state.tracker.lock().unwrap().record_committed_closure(account_id.clone(), closure_turn);
                                            info!("[exit-timing] recorded match={} account={} turn={} record_ms={}", state.id, account_id, closure_turn,
                                                record_started.map_or(0, |since| since.elapsed().as_millis()));
                                            acknowledge_exit(&mut exit_replies, &account_id, closure_turn);
                                            dispatch_participant_settlements(vec![job]);
                                        }
                                        Err(error) => {
                                            error!("[rewards] lobby {} participant spool failed; retrying while match stays active: {error}", state.id);
                                            pending_participant_settlements.push(
                                                PendingParticipantSettlement {
                                                    account_id,
                                                    closure_turn,
                                                    replay_bytes: replay_bytes.clone(),
                                                    rejected: false,
                                                    retry_count: 1,
                                                    direct_in_flight: false,
                                                    direct_retry_after: None,
                                                    queued_at: Instant::now(),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                warn!("[replay] lobby {} exit checkpoint failed; retrying: {error}", state.id);
                                let exhausted = pending_closure_checkpoint_since
                                    .get(&closure_turn)
                                    .is_some_and(|since| {
                                        since.elapsed().as_secs()
                                            >= PARTICIPANT_CHECKPOINT_RETRY_SECS
                                    });
                                if exhausted {
                                    warn!("[rewards] lobby {} personal prefix could not be recorded; recording explicit participant rejection", state.id);
                                    pending_closures
                                        .retain(|(_, turn_number)| *turn_number != closure_turn);
                                    pending_closure_checkpoint_since.remove(&closure_turn);
                                    for account_id in accounts {
                                        pending_participant_settlements.push(
                                            PendingParticipantSettlement {
                                                account_id,
                                                closure_turn,
                                                replay_bytes: Vec::new(),
                                                rejected: true,
                                                retry_count: 0,
                                                direct_in_flight: false,
                                                direct_retry_after: None,
                                                queued_at: Instant::now(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                let msg = ServerTurnMessage { turn };
                let json = Arc::new(
                    bincode::serialize(&ServerMessage::Turn(msg))
                        .expect("serialize ServerTurnMessage"),
                );

                let mut dropped_players = Vec::new();
                clients.retain(|player_id, client| {
                    match client.sender.try_send(json.clone()) {
                        Ok(()) => {
                            client.missed_ticks = 0;
                            true
                        }
                        Err(TrySendError::Full(_)) => {
                            client.missed_ticks += 1;
                            if client.missed_ticks >= MAX_MISSED_TICKS {
                                warn!("Player {player_id} dropped: {}/{} consecutive missed ticks",
                                    client.missed_ticks, MAX_MISSED_TICKS);
                                dropped_players.push((*player_id, client.generation));
                                false
                            } else {
                                if client.missed_ticks == 1 || client.missed_ticks % 10 == 0 {
                                    warn!("Player {player_id} slow: {} missed ticks",
                                        client.missed_ticks);
                                }
                                true
                            }
                        }
                        Err(TrySendError::Closed(_)) => {
                            info!("Player {player_id} channel closed, removing");
                            dropped_players.push((*player_id, client.generation));
                            false
                        }
                    }
                });

                if !dropped_players.is_empty() {
                    for (pid, _) in dropped_players {
                        state.tracker.lock().unwrap().disconnect(pid, Instant::now());
                        pending_intents.push(StampedIntent {
                            player_id: pid,
                            intent: GameplayIntent::MarkDisconnected {
                                is_disconnected: true,
                            },
                        });
                    }
                }

                if last_status.elapsed().as_secs() >= 10 {
                    info!(
                        "STATUS|lobby={}|pid={}|relay_port={}|roster_total={}|internal_bot_count={}|expected_external_connections={}|active_relay_connections={}|ticks={}|intents={}",
                        state.id,
                        std::process::id(),
                        state.relay_port,
                        state.roster_total,
                        state.internal_bot_count,
                        state.expected_external_connections,
                        active_relay_connections,
                        total_ticks,
                        total_intents
                    );
                    last_status = std::time::Instant::now();
                    // Heartbeat: refresh per-lobby Redis TTL.
                    let redis_con = redis_shared();
                    let key = format!("sow:relay:{}", state.id);
                    let val = relay_addr(state.relay_port);
                    tokio::task::spawn_blocking(move || {
                        let mut guard = redis_con.lock().unwrap();
                        if let Some(ref mut con) = *guard {
                            if let Err(e) = con.set_ex::<_, _, ()>(&key, val, 60) {
                                error!("[REDIS] SETEX {key} FAILED: {e}");
                            }
                        }
                    });
                }
            }
            Some((account_id, closure_turn, accepted)) = settlement_result_rx.recv() => {
                if accepted {
                    state.tracker.lock().unwrap().record_committed_closure(account_id.clone(), closure_turn);
                    acknowledge_exit(&mut exit_replies, &account_id, closure_turn);
                    pending_participant_settlements.retain(|settlement| {
                        settlement.account_id != account_id
                            || settlement.closure_turn != closure_turn
                    });
                    info!("[rewards] participant rejection recorded match={} account={}", state.id, account_id);
                } else if let Some(settlement) = pending_participant_settlements
                    .iter_mut()
                    .find(|settlement| {
                        settlement.account_id == account_id
                            && settlement.closure_turn == closure_turn
                    })
                {
                    settlement.direct_in_flight = false;
                }
            }
            Some(event) = ev_rx.recv() => {
                match event {
                    RelayEvent::EndMatch => { end_requested = true; }
                    RelayEvent::ExitRequested { account_id, reply } => {
                        let mut clients = state.clients.lock().await;
                        let mut tracker = state.tracker.lock().unwrap();
                        let player_id = tracker.player_accounts.iter().find(|(_, account)| **account == account_id
                            && tracker.human_accounts.contains(*account)).map(|(player, _)| *player);
                        let Some(player_id) = player_id else {
                            let _ = reply.send(Err("account is not a registered human participant".into()));
                            continue;
                        };
                        if let Some((turn, _)) = tracker.closure(&account_id) {
                            let still_recording = pending_closures.iter().any(|(account, _)| account == &account_id)
                                || pending_participant_settlements.iter().any(|entry| entry.account_id == account_id);
                            if !still_recording { let _ = reply.send(Ok(turn)); continue; }
                        }
                        let waiters = exit_replies.entry(account_id.clone()).or_default();
                        waiters.retain(|waiter| !waiter.is_closed());
                        waiters.push(reply);
                        clients.remove(&player_id);
                        let (new_exit, _) = tracker.record_exit(player_id, tick_number, ExitReason::Voluntary);
                        if new_exit {
                            pending_intents.push(StampedIntent { player_id, intent: GameplayIntent::Resign });
                            pending_closure_checkpoint_since.entry(tick_number).or_insert_with(Instant::now);
                            pending_closures.push((account_id, tick_number));
                        }
                    }
                    RelayEvent::Gameplay { player_id, intent, generation } => {
                        let clients = state.clients.lock().await;
                        if !clients.get(&player_id).is_some_and(|client| client.generation == generation)
                            || state.tracker.lock().unwrap().is_sealed(player_id) { continue; }
                        if !admit_gameplay_intent(player_id, &intent, tick_number, tick_rate_ms as f32,
                            spawn_phase_end_tick, &mut spawn_rate_limits) { continue; }
                        pending_intents.push(StampedIntent { player_id, intent });
                    }
                    RelayEvent::VoluntaryLeave { player_id, generation, peer, reply } => {
                        let mut clients = state.clients.lock().await;
                        if !clients.get(&player_id).is_some_and(|client| client.generation == generation) {
                            warn!("[relay] stale exit ignored lobby={} player={} peer={} generation={}", state.id, player_id, peer, generation);
                            let _ = reply.send(Err("stale game connection".into()));
                            continue;
                        }
                        let (new_exit, account_id) = state.tracker.lock().unwrap().record_exit(player_id, tick_number, ExitReason::Voluntary);
                        clients.remove(&player_id);
                        if let Some(account_id) = account_id {
                            exit_replies.entry(account_id.clone()).or_default().push(reply);
                            pending_closure_checkpoint_since.entry(tick_number).or_insert_with(Instant::now);
                            pending_closures.push((account_id, tick_number));
                        } else {
                            let _ = reply.send(Err("no open human participation".into()));
                        }
                        if new_exit {
                            info!("[exit-timing] requested match={} player={} turn={}", state.id, player_id, tick_number);
                            pending_intents.push(StampedIntent { player_id, intent: GameplayIntent::Resign });
                        }
                    }
                    RelayEvent::Disconnected { player_id, generation, peer } => {
                        let mut clients = state.clients.lock().await;
                        if clients.get(&player_id).is_some_and(|client| client.generation == generation) {
                            clients.remove(&player_id);
                            state.tracker.lock().unwrap().disconnect(player_id, Instant::now());
                            info!("[relay] temporary disconnect lobby={} player={} peer={} generation={}", state.id, player_id, peer, generation);
                            pending_intents.push(StampedIntent { player_id, intent: GameplayIntent::MarkDisconnected { is_disconnected: true } });
                        }
                    }
                }
            }
        }
    }
    info!("[relay] lobby {} tick task ended", state.id);
}

// ---- bridge worker (dispatch — same shape as the fstack-bridge examples) -----

async fn bridge_worker(registry: Registry) {
    let rx = bridge::rx_ring();
    let notify = bridge::notify();
    let mut conns: HashMap<c_int, mpsc::Sender<bridge::ZcRxGuard>> = HashMap::new();

    loop {
        while let Some(ev) = rx.pop() {
            match ev {
                Ev::Accept {
                    fd,
                    generation,
                    peer,
                    listener_port,
                    accepted_at,
                } => {
                    let (tx, rx_conn) = mpsc::channel(BRIDGE_RX_CAP);
                    conns.insert(fd, tx);
                    tokio::spawn(ws_task(
                        fd,
                        generation,
                        peer,
                        listener_port,
                        accepted_at,
                        rx_conn,
                        registry.clone(),
                    ));
                }
                Ev::Data { fd, guard } => match conns.get(&fd) {
                    Some(tx) => match tx.try_send(guard) {
                        Ok(()) => {}
                        Err(TrySendError::Full(guard)) => {
                            error!(
                                "[bridge] per-connection RX budget exhausted fd={fd}; closing connection to preserve TLS stream integrity"
                            );
                            drop(guard);
                            conns.remove(&fd);
                        }
                        Err(TrySendError::Closed(guard)) => drop(guard),
                    },
                    None => drop(guard),
                },
                Ev::Closed { fd } => {
                    conns.remove(&fd);
                }
            }
        }
        notify.notified().await;
    }
}

// ---- per-connection protocol task -------------------------------------------

/// Unified stream: either plain Conn or TLS-wrapped Conn.
/// tokio-tungstenite needs AsyncRead + AsyncWrite; we delegate both.
enum MaybeTlsConn {
    Plain(bridge::Conn),
    Tls(Box<tokio_rustls::server::TlsStream<bridge::Conn>>),
}

impl AsyncRead for MaybeTlsConn {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_read(cx, buf),
            MaybeTlsConn::Tls(c) => Pin::new(c.as_mut()).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for MaybeTlsConn {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_write(cx, buf),
            MaybeTlsConn::Tls(c) => Pin::new(c.as_mut()).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_flush(cx),
            MaybeTlsConn::Tls(c) => Pin::new(c.as_mut()).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_shutdown(cx),
            MaybeTlsConn::Tls(c) => Pin::new(c.as_mut()).poll_shutdown(cx),
        }
    }
}

#[derive(Clone)]
enum Role {
    RelayPlayer {
        lobby: Arc<LobbyState>,
        player_id: u16,
    },
}

struct RelayRegistration {
    role: Role,
    initial_frames: Vec<Vec<u8>>,
}

fn role_session_id(role: &Option<Role>) -> Option<u64> {
    match role {
        Some(Role::RelayPlayer { lobby, player_id }) => lobby.session_ids.get(player_id).copied(),
        None => None,
    }
}

async fn ws_task(
    fd: c_int,
    generation: u64,
    peer: SocketAddr,
    listener_port: u16,
    accepted_at: Instant,
    rx: mpsc::Receiver<bridge::ZcRxGuard>,
    registry: Registry,
) {
    info!(
        "[relay] connect origin=external_network peer={} fd={} generation={}",
        peer, fd, generation
    );
    let conn = bridge::Conn::new(fd, generation, rx);
    let accept_to_task_us = accepted_at.elapsed().as_micros();
    let tls_started = Instant::now();
    let mut tls_us = 0u128;

    // These timings are intentionally emitted once per connection, without
    // payloads or credentials. They distinguish bridge scheduling, TLS, WS,
    // and first-frame delays while keeping the hot packet path unchanged.
    let log_handshake_timing = |result: &str, tls_us: u128, ws_us: u128, first_frame_us: u128| {
        info!(
            "[relay] handshake_timing worker={} port={} peer={} fd={} generation={} accept_to_task_us={} tls_us={} ws_us={} first_frame_us={} result={}",
            WORKER_QUEUE_ID.load(Ordering::Relaxed),
            listener_port,
            peer,
            fd,
            generation,
            accept_to_task_us,
            tls_us,
            ws_us,
            first_frame_us,
            result
        );
    };

    // TLS handshake (if cert configured), then WebSocket upgrade.
    let stream = if let Some(acceptor) = current_tls_acceptor() {
        match tokio::time::timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
            acceptor.accept(conn),
        )
        .await
        {
            Ok(Ok(tls)) => {
                tls_us = tls_started.elapsed().as_micros();
                MaybeTlsConn::Tls(Box::new(tls))
            }
            Ok(Err(e)) => {
                tls_us = tls_started.elapsed().as_micros();
                warn!("[tls] handshake fail fd={} err={}", fd, e);
                log_handshake_timing("tls_error", tls_us, 0, 0);
                return;
            }
            Err(_) => {
                tls_us = tls_started.elapsed().as_micros();
                warn!("[tls] handshake timeout fd={} peer={}", fd, peer);
                log_handshake_timing("tls_timeout", tls_us, 0, 0);
                return;
            }
        }
    } else {
        MaybeTlsConn::Plain(conn)
    };

    let ws_started = Instant::now();
    let ws = match tokio::time::timeout(
        Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
        tokio_tungstenite::accept_async(stream),
    )
    .await
    {
        Ok(Ok(ws)) => ws,
        Ok(Err(e)) => {
            let ws_us = ws_started.elapsed().as_micros();
            warn!("[ws] handshake fail fd={} err={}", fd, e);
            log_handshake_timing("ws_error", tls_us, ws_us, 0);
            return;
        }
        Err(_) => {
            let ws_us = ws_started.elapsed().as_micros();
            warn!("[ws] handshake timeout fd={} peer={}", fd, peer);
            log_handshake_timing("ws_timeout", tls_us, ws_us, 0);
            return;
        }
    };
    let ws_us = ws_started.elapsed().as_micros();

    let (mut write, mut read) = ws.split();
    let (direct_tx, mut direct_rx) = mpsc::channel::<Arc<Vec<u8>>>(PER_CLIENT_CHANNEL);

    // First frame decides the role. The optional bot/backfill orchestrator (or
    // an operations client) sends nothing: classify it after a short grace
    // period.
    let first_frame_started = Instant::now();
    let first_frame_us: u128;
    let registration: Option<RelayRegistration>;
    match tokio::time::timeout(Duration::from_secs(ORCHESTRATOR_GRACE_SECS), read.next()).await {
        Ok(Some(Ok(Message::Binary(b)))) => {
            first_frame_us = first_frame_started.elapsed().as_micros();
            match bincode::deserialize::<ClientMessage>(&b) {
                Ok(ClientMessage::ReadyWithTicket {
                    lobby_id,
                    player_id,
                    ticket,
                }) => {
                    registration = try_register_with_admission(
                        &registry,
                        lobby_id,
                        player_id,
                        RelayAdmission::Initial(&ticket),
                        generation,
                        &direct_tx,
                    )
                    .await;
                    let role = registration.as_ref().map(|result| result.role.clone());
                    let audit_session_id = role_session_id(&role);
                    info!(
                        "[relay] first-frame ticket Ready origin=external_network lobby={} player={} registered={} peer={} session_id={:?} fd={} generation={}",
                        lobby_id,
                        player_id,
                        registration.is_some(),
                        peer,
                        audit_session_id,
                        fd,
                        generation
                    );
                }
                Ok(ClientMessage::ReconnectWithTicket {
                    lobby_id,
                    player_id,
                    ticket,
                }) => {
                    registration = try_register_with_admission(
                        &registry,
                        lobby_id,
                        player_id,
                        RelayAdmission::Reconnect(&ticket),
                        generation,
                        &direct_tx,
                    )
                    .await;
                    let role = registration.as_ref().map(|result| result.role.clone());
                    let audit_session_id = role_session_id(&role);
                    info!(
                        "[relay] first-frame reconnect origin=external_network lobby={} player={} registered={} peer={} session_id={:?} fd={} generation={}",
                        lobby_id,
                        player_id,
                        registration.is_some(),
                        peer,
                        audit_session_id,
                        fd,
                        generation
                    );
                }
                Ok(_) => {
                    warn!("[relay] ignored first frame fd={}", fd);
                    registration = None;
                }
                Err(e) => {
                    warn!("[relay] deserialize err {} fd={}", e, fd);
                    registration = None;
                }
            }
        }
        Ok(Some(Ok(_))) => {
            first_frame_us = first_frame_started.elapsed().as_micros();
            registration = None;
        }
        Ok(Some(Err(e))) => {
            first_frame_us = first_frame_started.elapsed().as_micros();
            warn!("[ws] recv err fd={} err={}", fd, e);
            log_handshake_timing("first_frame_error", tls_us, ws_us, first_frame_us);
            return;
        }
        Ok(None) => {
            first_frame_us = first_frame_started.elapsed().as_micros();
            log_handshake_timing("closed_before_first_frame", tls_us, ws_us, first_frame_us);
            return;
        }
        Err(_) => {
            first_frame_us = first_frame_started.elapsed().as_micros();
            // Nothing in the grace window → optional orchestration/operations
            // WS (the bot/backfill compatibility client uses this feed).
            log_handshake_timing("orchestrator_grace_timeout", tls_us, ws_us, first_frame_us);
            orchestrator_task(&mut write, &mut read, &registry).await;
            return;
        }
    }

    let role = registration.as_ref().map(|result| result.role.clone());
    let initial_frames = registration
        .map(|result| result.initial_frames)
        .unwrap_or_default();

    // A connection that sent a player frame but did not resolve to a lobby
    // owned by this worker is never a valid game session. Close it explicitly
    // instead of leaving an unowned socket in the worker's event loop. This is
    // the cross-worker ownership guard: the client must reconnect using the
    // relay endpoint carried by ServerStartMessage.
    if role.is_none() {
        warn!("[relay] rejecting unowned/invalid player connection fd={fd}");
        let _ = write.send(Message::Close(None)).await;
        log_handshake_timing("invalid_first_frame", tls_us, ws_us, first_frame_us);
        return;
    }

    // The replay prefix is sent directly before this connection begins reading
    // its bounded live-turn queue. Admission captured the prefix before adding
    // the connection to that queue, so history and live turns have one clean
    // boundary and remain ordered.
    if let Err(error) = send_admission_frames(&mut write, initial_frames).await {
        warn!("[relay] admission history write failed fd={} err={}", fd, error);
        if let Some(Role::RelayPlayer { lobby, player_id }) = &role {
            let _ = lobby
                .ev_tx
                .send(RelayEvent::Disconnected {
                    player_id: *player_id,
                    generation,
                    peer,
                })
                .await;
        }
        log_handshake_timing("admission_write_error", tls_us, ws_us, first_frame_us);
        return;
    }
    log_handshake_timing("success", tls_us, ws_us, first_frame_us);

    // Relay player: the per-connection loop, routed to its lobby.
    let my_lobby: Option<Arc<LobbyState>> = match &role {
        Some(Role::RelayPlayer { lobby, .. }) => Some(lobby.clone()),
        _ => None,
    };
    let my_player_id: Option<u16> = match &role {
        Some(Role::RelayPlayer { player_id, .. }) => Some(*player_id),
        _ => None,
    };

    if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
        info!(
            "[relay] ready origin=external_network lobby={} player={} peer={} session_id={:?} fd={} generation={}",
            lobby.id,
            pid,
            peer,
            lobby.session_ids.get(pid).copied(),
            fd,
            generation
        );
    }

    let mut last_rx = std::time::Instant::now();
    let mut last_ping = std::time::Instant::now();
    let mut ping_interval = interval(Duration::from_secs(WS_PING_SECS));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut frames_out: u64 = 0;
    let mut voluntary_leave_sent = false;

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        last_rx = std::time::Instant::now();
                        if let Message::Ping(payload) = &msg {
                            let _ = tokio::time::timeout(
                                Duration::from_millis(ws_write_timeout_ms()),
                                write.send(Message::Pong(payload.clone())),
                            )
                            .await;
                            continue;
                        }
                        if msg.is_binary() {
                            if let Ok(cmsg) = bincode::deserialize::<ClientMessage>(&msg.into_data()) {
                                match cmsg {
                                    ClientMessage::ReadyWithTicket { lobby_id: l_id, player_id, ticket } => {
                                        warn!("[relay] rejected mid-session initial ticket replay lobby={} player={} fd={} ticket_len={}", l_id, player_id, fd, ticket.len());
                                    }
                                    ClientMessage::ReconnectWithTicket { lobby_id: l_id, player_id, .. } => {
                                        warn!("[relay] rejected mid-session reconnect ticket lobby={} player={} fd={}", l_id, player_id, fd);
                                    }
                                    ClientMessage::Gameplay { intent: GameplayIntent::MarkDisconnected { .. } } => {
                                        warn!("[relay] rejected client-owned disconnect marker fd={fd}");
                                    }
                                    ClientMessage::Gameplay { intent: GameplayIntent::Resign }
                                        | ClientMessage::Leave {} => {
                                        if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                                            let (reply, response) = oneshot::channel();
                                            let requested = Instant::now();
                                            match tokio::time::timeout(
                                                Duration::from_secs(3),
                                                lobby.ev_tx.send(RelayEvent::VoluntaryLeave {
                                                    player_id: *pid,
                                                    generation,
                                                    peer,
                                                    reply,
                                                }),
                                            )
                                            .await
                                            {
                                                Ok(Ok(())) => {
                                                    voluntary_leave_sent = true;
                                                    match tokio::time::timeout(Duration::from_secs(10), response).await {
                                                        Ok(Ok(Ok(closure_turn))) => {
                                                            let account_id = lobby.tracker.lock().unwrap().player_accounts.get(pid).cloned();
                                                            if let Some(account_id) = account_id {
                                                                let reason = lobby.tracker.lock().unwrap().closure(&account_id).map_or(ExitReason::Voluntary, |(_, reason)| reason);
                                                                let ack = ServerMessage::ExitRecorded { account_id, match_id:lobby.id, closure_turn, reason };
                                                                if let Ok(bytes) = bincode::serialize(&ack) {
                                                                    match tokio::time::timeout(Duration::from_millis(ws_write_timeout_ms()), write.send(Message::Binary(bytes))).await {
                                                                        Ok(Ok(())) => info!("[exit-timing] acknowledged match={} player={} turn={} ack_ms={}", lobby.id, pid, closure_turn, requested.elapsed().as_millis()),
                                                                        result => warn!("[relay] exit acknowledgment lost match={} player={} result={result:?}", lobby.id, pid),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        result => warn!("[relay] exit acknowledgment unavailable match={} player={} result={result:?}", lobby.id, pid),
                                                    }
                                                    // Keep the transport alive until the peer receives the
                                                    // close frame; dropping it after ExitRecorded is an EOF,
                                                    // which browsers report as a connection error.
                                                    let close = async {
                                                        write.send(Message::Close(Some(CloseFrame {
                                                            code: CloseCode::Normal,
                                                            reason: "".into(),
                                                        }))).await?;
                                                        while let Some(message) = read.next().await {
                                                            if matches!(message?, Message::Close(_)) {
                                                                break;
                                                            }
                                                        }
                                                        Ok::<_, tokio_tungstenite::tungstenite::Error>(())
                                                    };
                                                    match tokio::time::timeout(Duration::from_millis(ws_write_timeout_ms()), close).await {
                                                        Ok(Ok(())) => {}
                                                        result => warn!("[relay] exit close handshake failed lobby={} player={} result={result:?}", lobby.id, pid),
                                                    }
                                                    break;
                                                }
                                                Ok(Err(error)) => {
                                                    warn!("[relay] leave event queue closed lobby={} player={} error={error}", lobby.id, pid);
                                                }
                                                Err(_) => {
                                                    warn!("[relay] leave event queue timed out lobby={} player={}", lobby.id, pid);
                                                }
                                            }
                                        }
                                    }
                                    ClientMessage::Gameplay { intent } => {
                                        if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                                            if let Err(error) = lobby.ev_tx.try_send(RelayEvent::Gameplay { player_id:*pid, intent, generation }) {
                                                warn!("[relay] gameplay queue unavailable lobby={} player={} error={error}", lobby.id, pid);
                                            }
                                        }
                                    }
                                    ClientMessage::Ping { client_time } => {
                                        let pong = ServerMessage::Pong { client_time };
                                        if let Ok(json) = bincode::serialize(&pong) {
                                            let _ = direct_tx.try_send(Arc::new(json));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => break,
                }
            }
            Some(direct_data) = direct_rx.recv() => {
                // Hot path (ticks flowing). select! may starve the interval
                // branch here, so ping + reap checks live on this branch too.
                if last_ping.elapsed() >= Duration::from_secs(WS_PING_SECS) {
                    last_ping = std::time::Instant::now();
                    if tokio::time::timeout(
                        Duration::from_millis(ws_write_timeout_ms()),
                        write.send(Message::Ping(Vec::new())),
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
                if last_rx.elapsed() > Duration::from_secs(RX_DEADLINE_SECS) {
                    warn!(
                        "[ws] fd={} rx silence {}s (ticks flowing) — reaping stale connection",
                        fd,
                        last_rx.elapsed().as_secs()
                    );
                    break;
                }
                frames_out += 1;
                if frames_out <= 5 || frames_out % 50 == 0 {
                    info!("[DIAG RELAY TX] fd={} sent frame #{} len={}", fd, frames_out, direct_data.len());
                }
                match tokio::time::timeout(
                    Duration::from_millis(ws_write_timeout_ms()),
                    write.send(Message::Binary((*direct_data).clone())),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        warn!("[DIAG RELAY TX] write.send failed fd={} err={}", fd, e);
                        break;
                    }
                    Err(_) => {
                        warn!("[DIAG RELAY TX] write.send timed out (>{}ms) fd={}", ws_write_timeout_ms(), fd);
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                // Idle path (no ticks): keepalive ping + reap check.
                // Lifecycle: if the lobby was finalized/GC'd (removed from the
                // registry) or this client was dropped by the tick task, end
                // this connection so the Arc<LobbyState> (and its live replay
                // window) is released instead of being pinned alive forever.
                if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                    let still_registered = registry.read().await.contains_key(&lobby.id);
                    if !still_registered {
                        info!("[ws] fd={} lobby {} no longer registered — closing connection", fd, lobby.id);
                        break;
                    }
                    if !lobby.clients.lock().await.contains_key(pid) {
                        info!("[ws] fd={} player {} dropped from lobby {} — closing connection", fd, pid, lobby.id);
                        break;
                    }
                }
                last_ping = std::time::Instant::now();
                if last_rx.elapsed() > Duration::from_secs(RX_DEADLINE_SECS) {
                    warn!(
                        "[ws] fd={} rx silence {}s — reaping stale connection",
                        fd,
                        last_rx.elapsed().as_secs()
                    );
                    break;
                }
                if tokio::time::timeout(
                    Duration::from_millis(ws_write_timeout_ms()),
                    write.send(Message::Ping(Vec::new())),
                )
                .await
                .is_err()
                {
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                if last_rx.elapsed() > Duration::from_secs(RX_DEADLINE_SECS) {
                    break;
                }
            }
        }
    }

    if !voluntary_leave_sent && let (Some(lobby), Some(pid)) = (my_lobby, my_player_id) {
        let _ = lobby
            .ev_tx
            .send(RelayEvent::Disconnected {
                player_id: pid,
                generation,
                peer,
            })
            .await;
    }
}

enum RelayAdmission<'a> {
    Initial(&'a str),
    Reconnect(&'a str),
}

fn new_reconnect_token() -> (String, [u8; 32]) {
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let token = hex::encode(raw);
    let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    (token, digest)
}

async fn try_register_with_admission(
    registry: &Registry,
    lobby_id: u64,
    player_id: u16,
    admission: RelayAdmission<'_>,
    generation: u64,
    direct_tx: &mpsc::Sender<Arc<Vec<u8>>>,
) -> Option<RelayRegistration> {
    let lobby = {
        let reg = registry.read().await;
        reg.get(&lobby_id).cloned()
    };
    let lobby = match lobby {
        Some(l) => l,
        None => {
            warn!(
                "[relay] invalid Ready lobby={} player={} (no such lobby)",
                lobby_id, player_id
            );
            return None;
        }
    };
    if !lobby.valid_players.contains_key(&player_id) {
        warn!(
            "[relay] invalid Ready lobby={} player={} (not a valid player)",
            lobby_id, player_id
        );
        return None;
    }
    // Same lock order as the tick owner: reconnect and sealing cannot race.
    let mut clients = lobby.clients.lock().await;
    if lobby.tracker.lock().unwrap().is_sealed(player_id) {
        warn!(
            "[relay] rejected reconnect after participation sealed lobby={} player={}",
            lobby_id, player_id
        );
        return None;
    }
    let reconnect_token = {
        let mut auth = lobby.auth.lock().await;
        match admission {
            RelayAdmission::Initial(ticket) => {
                if !ticket_is_current(lobby.ticket_expires_at) {
                    warn!(
                        "[relay] expired relay ticket lobby={} player={}",
                        lobby_id, player_id
                    );
                    return None;
                }
                let valid = lobby
                    .ticket_digests
                    .get(&player_id)
                    .is_some_and(|expected| ticket_matches(ticket, expected));
                if !valid {
                    warn!(
                        "[relay] invalid relay ticket lobby={} player={}",
                        lobby_id, player_id
                    );
                    return None;
                }
                if !auth.initial_used.insert(player_id) {
                    warn!(
                        "[relay] replayed initial relay ticket lobby={} player={}",
                        lobby_id, player_id
                    );
                    return None;
                }
                let (token, digest) = new_reconnect_token();
                auth.reconnect_digests.insert(player_id, digest);
                Some(token)
            }
            RelayAdmission::Reconnect(ticket) => {
                let valid = auth
                    .reconnect_digests
                    .get(&player_id)
                    .is_some_and(|expected| ticket_matches(ticket, expected));
                if !valid {
                    warn!(
                        "[relay] invalid or replayed reconnect ticket lobby={} player={}",
                        lobby_id, player_id
                    );
                    return None;
                }
                let (token, digest) = new_reconnect_token();
                auth.reconnect_digests.insert(player_id, digest);
                Some(token)
            }
        }
    };

    if !lobby
        .tracker
        .lock()
        .unwrap()
        .reconnect(player_id, Instant::now())
    {
        warn!(
            "[relay] reconnect grace expired lobby={} player={}",
            lobby_id, player_id
        );
        return None;
    }
    // The tick owner uses this same clients lock while appending and
    // broadcasting each turn. Snapshot before subscribing so every turn is
    // either in this prefix or in the live queue, never both or neither.
    let history = lobby.journal.snapshot().await;
    let mut initial_frames = Vec::with_capacity(history.len() + 1);
    if let Some(token) = reconnect_token {
        let message = ServerMessage::RelayReconnectTicket {
            lobby_id,
            player_id,
            ticket: token,
        };
        match bincode::serialize(&message) {
            Ok(bytes) => initial_frames.push(bytes),
            Err(e) => warn!(
                "[relay] reconnect ticket serialization failed lobby={} player={} err={}",
                lobby_id, player_id, e
            ),
        }
    }
    for past_turn in history {
        let msg = ServerTurnMessage { turn: past_turn };
        if let Ok(json) = bincode::serialize(&ServerMessage::Turn(msg)) {
            initial_frames.push(json);
        }
    }

    clients.insert(
        player_id,
        ClientChannel {
            sender: direct_tx.clone(),
            missed_ticks: 0,
            generation,
        },
    );
    drop(clients);
    let _ = lobby.ev_tx.try_send(RelayEvent::Gameplay {
        player_id,
        generation,
        intent: GameplayIntent::MarkDisconnected {
            is_disconnected: false,
        },
    });

    Some(RelayRegistration {
        role: Role::RelayPlayer { lobby, player_id },
        initial_frames,
    })
}

async fn send_admission_frames<S>(write: &mut S, frames: Vec<Vec<u8>>) -> Result<(), String>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let timeout = Duration::from_millis(ws_write_timeout_ms());
    for frame in frames {
        match tokio::time::timeout(timeout, write.feed(Message::Binary(frame))).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error.to_string()),
            Err(_) => return Err("timed out writing admission history frame".to_string()),
        }
    }
    match tokio::time::timeout(timeout, write.flush()).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("timed out flushing admission history".to_string()),
    }
}

/// Optional orchestration connection: periodic `LobbiesBroadcast` of registered
/// lobbies for bot/backfill compatibility and operations tooling.
async fn orchestrator_task(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<MaybeTlsConn>,
        Message,
    >,
    read: &mut futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<MaybeTlsConn>>,
    registry: &Registry,
) {
    let mut ticker = interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let lobbies: Vec<LobbyInfo> = {
                    let reg = registry.read().await;
                    reg.values().map(lobby_info).collect()
                };
                let msg = ServerMessage::LobbiesBroadcast(ServerLobbiesBroadcastMessage {
                    lobbies,
                });
                if let Ok(json) = bincode::serialize(&msg) {
                    match tokio::time::timeout(Duration::from_millis(ws_write_timeout_ms()), write.send(Message::Binary(json))).await {
                        Ok(Ok(())) => {}
                        _ => break,
                    }
                }
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(_)) => {}
                    _ => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(15)) => { break; }
        }
    }
    info!("[ws] orchestrator done");
}

/// Registry lobby as the broadcast LobbyInfo (roster = valid players).
fn lobby_info(state: &Arc<LobbyState>) -> LobbyInfo {
    let mut players: Vec<LobbyPlayerSyncState> = state
        .valid_players
        .iter()
        .map(|(pid, name)| LobbyPlayerSyncState {
            name: name.clone(),
            is_ready: false,
            download_progress: 100,
            leader: Some(Leader::Cleopatra),
            player_id: *pid,
            team: None,
        })
        .collect();
    players.sort_by_key(|p| p.player_id);
    LobbyInfo {
        id: state.id,
        num_players: players.len() as u32,
        max_players: players.len() as u32,
        is_counting_down: false,
        timer_secs: 0.0,
        map_name: "world".to_string(),
        game_mode: "FFA".to_string(),
        players,
        has_password: false,
        host_name: String::new(),
        bot_count: 0,
        nation_count: 0,
        bot_difficulty: BotDifficulty::Vanilla,
        kind: LobbyKind::Matchmaking,
    }
}

#[cfg(test)]
mod dispatcher_tests {
    #[test]
    fn participant_delivery_child() {
        if std::env::var_os("SOW_PARTICIPANT_DELIVERY_TEST").is_none() {
            return;
        }
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                super::recover_replay_spool().await;
                super::recover_participant_spool().await;
            });
    }

    #[test]
    fn denied_delivery_retains_result_until_a_matching_acknowledgment() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let directory = tempfile::tempdir().unwrap();
        let bytes = b"durable participant prefix";
        let payload = ParticipantSettlementPayload {
            match_id: "42".into(),
            account_id: "0123456789abcdef0123456789abcdef".into(),
            closure_turn: 7,
            replay_sha256: hex::encode(Sha256::digest(bytes)),
            rejected: false,
        };
        let path = super::participant_spool_path_at(
            directory.path(),
            &payload.match_id,
            &payload.account_id,
        );
        persist_participant_spool(&path, &payload, bytes).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let ack = serde_json::json!({"status":"queued","match_id":payload.match_id,
            "account_id":payload.account_id,"closure_turn":payload.closure_turn})
        .to_string();
        let done = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let (server_done, server_requests) = (done.clone(), requests.clone());
        listener.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            while !server_done.load(Ordering::SeqCst) {
                let (mut socket, _) = match listener.accept() {
                    Ok(socket) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("audit HTTP listener failed: {error}"),
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(end) = request.windows(4).position(|chunk| chunk == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                        let size = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse::<usize>()
                            .unwrap();
                        if request.len() >= end + 4 + size {
                            break;
                        }
                    }
                }
                let request = server_requests.fetch_add(1, Ordering::SeqCst);
                let (status, body) = if request == 0 {
                    ("401 Unauthorized", "{}")
                } else {
                    ("200 OK", ack.as_str())
                };
                // Keep all four processes waiting for the same durable result.
                std::thread::sleep(Duration::from_millis(50));
                write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let run = || {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "dispatcher_tests::participant_delivery_child"])
                .env_clear()
                .env("SOW_PARTICIPANT_DELIVERY_TEST", "1")
                .env("SOW_REPLAY_SPOOL_DIR", directory.path())
                .env("SOW_DB_URL", &url)
                .env("SOW_RELAY_SETTLEMENT_SECRET", "isolated-test-secret")
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        };
        assert!(run().wait().unwrap().success());
        assert!(
            path.exists(),
            "an access failure must retain the pending result"
        );
        assert_eq!(
            std::fs::read(participant_replay_path(&path)).unwrap(),
            bytes
        );
        let children = (0..4).map(|_| run()).collect::<Vec<_>>();
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
        let replay_path = directory.path().join("42.replay");
        std::fs::write(&replay_path, bytes).unwrap();
        let metadata = super::ReplaySpoolMetadata {
            lobby_json: "{}".into(),
            phase: super::ReplayPhase::Finalized,
            rejected: false,
            participant_closures: Vec::new(),
        };
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::persist_replay_metadata(
                &replay_path.with_extension("json"),
                &metadata,
            ))
            .unwrap();
        let children = (0..4).map(|_| run()).collect::<Vec<_>>();
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
        done.store(true, Ordering::SeqCst);
        server.join().unwrap();
        assert_eq!(
            requests.load(Ordering::SeqCst),
            3,
            "one rejected attempt, one personal delivery and one full delivery across four processes"
        );
        assert!(!replay_path.exists());
        assert!(!replay_path.with_extension("json").exists());
        assert!(!path.exists(), "the matching durable ACK allows cleanup");
        assert!(!participant_replay_path(&path).exists());
    }

    #[tokio::test]
    async fn detached_exit_recovers_durable_turns_only_after_the_writer_stops() {
        let temp = tempfile::tempdir().unwrap();
        let command = sow_data::profile::MatchExitCommand {
            match_id: "42".into(),
            account_id: "0123456789abcdef0123456789abcdef".into(),
            player_id: Some(1),
            owner_verified: true,
        };
        let turns = vec![
            Turn {
                turn_number: 0,
                intents: vec![],
            },
            Turn {
                turn_number: 1,
                intents: vec![sow_core::protocol::StampedIntent {
                    player_id: 1,
                    intent: GameplayIntent::Resign,
                }],
            },
            Turn {
                turn_number: 2,
                intents: vec![],
            },
        ];
        let journal = temp.path().join("42.journal");
        let mut bytes = Vec::new();
        for turn in &turns {
            let record = bincode::serialize(turn).unwrap();
            bytes.extend_from_slice(&(record.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&record);
        }
        // A crash may leave an incomplete final append. Only complete records
        // belong to the recovered prefix; the normal decoder remains strict.
        bytes.extend_from_slice(&10u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2]);
        assert!(super::decode_replay_journal(&bytes, false).is_err());
        assert_eq!(super::decode_replay_journal(&bytes, true).unwrap(), turns);
        std::fs::write(&journal, bytes).unwrap();
        let owner = super::lock_result(&journal, libc::LOCK_EX).unwrap();
        assert!(
            super::recover_detached_exit(temp.path(), &command)
                .await
                .is_err(),
            "an active writer must never be treated as lost"
        );
        drop(owner);
        let job = super::recover_detached_exit(temp.path(), &command)
            .await
            .unwrap();
        assert!(!job.payload.rejected);
        assert_eq!(job.payload.closure_turn, 1);
        assert_eq!(
            bincode::deserialize::<Vec<Turn>>(&job.replay_bytes).unwrap(),
            turns[..2]
        );
        assert_eq!(
            std::fs::read(super::participant_replay_path(&job.spool_path)).unwrap(),
            job.replay_bytes
        );
        let new_exit = super::recovered_exit_prefix(vec![turns[0].clone()], 1).unwrap();
        assert_eq!(new_exit.0, 1);
        assert!(matches!(
            bincode::deserialize::<Vec<Turn>>(&new_exit.1).unwrap()[1].intents[0].intent,
            GameplayIntent::Resign
        ));
        let unknown = sow_data::profile::MatchExitCommand {
            match_id: "43".into(),
            owner_verified: false,
            ..command
        };
        assert!(
            super::recover_detached_exit(temp.path(), &unknown)
                .await
                .is_err()
        );
        let lost = sow_data::profile::MatchExitCommand {
            owner_verified: true,
            ..unknown
        };
        let job = super::recover_detached_exit(temp.path(), &lost)
            .await
            .unwrap();
        assert!(job.payload.rejected);
        assert!(job.replay_bytes.is_empty());
    }
    #[test]
    fn result_claim_child() {
        let Ok(directory) = std::env::var("SOW_RESULT_CLAIM_TEST_DIR") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _claim = runtime
            .block_on(super::claim_result(directory.join("42.participant")))
            .unwrap();
        let marker = directory.join("sending");
        assert!(
            !marker.exists(),
            "two relay processes are sending the same result"
        );
        std::fs::write(&marker, b"sending").unwrap();
        let counter = directory.join("deliveries");
        let previous = std::fs::read_to_string(&counter)
            .unwrap_or_else(|_| "0".into())
            .parse::<u32>()
            .unwrap();
        // Deliberate overlap window exercises the native inter-process lock.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(counter, (previous + 1).to_string()).unwrap();
        std::fs::remove_file(marker).unwrap();
    }

    #[test]
    fn four_processes_share_one_result_claim() {
        let temp = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe().unwrap();
        let mut children = (0..4)
            .map(|_| {
                std::process::Command::new(&executable)
                    .args(["--exact", "dispatcher_tests::result_claim_child"])
                    .env("SOW_RESULT_CLAIM_TEST_DIR", temp.path())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for child in &mut children {
            assert!(child.wait().unwrap().success());
        }
        assert_eq!(
            std::fs::read_to_string(temp.path().join("deliveries")).unwrap(),
            "4"
        );
    }
    use super::{
        AdmissionState, IpAdmissionState, MatchTracker, ParticipantClosure,
        ParticipantSettlementPayload, Participation, RECONNECT_GRACE, RelayAdmissionPolicy, Turn,
        admit_gameplay_intent, deployment_end_tick, participant_replay_path,
        persist_participant_spool, replay_prefix_through, ticket_matches,
    };
    use sha2::{Digest, Sha256};
    use sow_core::game_config::GameConfig;
    use sow_core::protocol::{ExitReason, GameplayIntent};
    use std::collections::{HashMap, HashSet};
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use std::time::{Duration, Instant};

    fn policy(
        max_connections: usize,
        max_connections_per_ip: usize,
        handshakes_per_ip: u32,
    ) -> RelayAdmissionPolicy {
        RelayAdmissionPolicy {
            max_connections,
            max_connections_per_ip,
            handshakes_per_ip,
            state: StdMutex::new(AdmissionState {
                active: 0,
                by_ip: HashMap::<std::net::IpAddr, IpAdmissionState>::new(),
            }),
            accepted: std::sync::atomic::AtomicU64::new(0),
            rejected_global: std::sync::atomic::AtomicU64::new(0),
            rejected_per_ip: std::sync::atomic::AtomicU64::new(0),
            rejected_rate: std::sync::atomic::AtomicU64::new(0),
            active_peak: std::sync::atomic::AtomicU64::new(0),
        }
    }

    #[tokio::test]
    async fn admission_streams_history_before_live_turns_beyond_queue_capacity() {
        use futures_util::{SinkExt, StreamExt};
        use std::collections::VecDeque;
        use tokio_tungstenite::{
            WebSocketStream,
            tungstenite::{Message, protocol::Role as WebSocketRole},
        };

        let history_len = super::PER_CLIENT_CHANNEL + 32;
        let history = (0..history_len)
            .map(|turn_number| Turn {
                turn_number: turn_number as u64,
                intents: Vec::new(),
            })
            .collect::<Vec<_>>();
        let ticket = "initial-admission-ticket";
        let ticket_digest: [u8; 32] = Sha256::digest(ticket.as_bytes()).into();
        let account_id = "0123456789abcdef0123456789abcdef".to_string();
        let journal = Arc::new(super::ReplayJournal {
            live: Arc::new(tokio::sync::Mutex::new(VecDeque::from(history.clone()))),
            tx: tokio::sync::mpsc::channel(1).0,
            finalized: std::sync::atomic::AtomicBool::new(false),
            failed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        let (ev_tx, _ev_rx) = tokio::sync::mpsc::channel(8);
        let lobby = Arc::new(super::LobbyState {
            id: 42,
            relay_port: 25_592,
            ticket_expires_at: u64::MAX,
            roster_total: 1,
            internal_bot_count: 0,
            expected_external_connections: 1,
            valid_players: HashMap::from([(1, "player".to_string())]),
            session_ids: HashMap::from([(1, 7)]),
            ticket_digests: HashMap::from([(1, ticket_digest)]),
            auth: Arc::new(tokio::sync::Mutex::new(
                super::RelayAuthState::default(),
            )),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            journal,
            tracker: Arc::new(StdMutex::new(super::MatchTracker {
                player_accounts: HashMap::from([(1, account_id.clone())]),
                human_accounts: HashSet::from([account_id]),
                participants: HashMap::from([(
                    1,
                    Participation::Disconnected {
                        since: Instant::now(),
                    },
                )]),
                tracked: true,
                lobby_json: "{}".to_string(),
            })),
            ev_tx,
        });
        let registry: super::Registry = Arc::new(tokio::sync::RwLock::new(HashMap::from([(
            42,
            lobby.clone(),
        )])));
        let (direct_tx, mut direct_rx) = tokio::sync::mpsc::channel(super::PER_CLIENT_CHANNEL);
        let registration = super::try_register_with_admission(
            &registry,
            42,
            1,
            super::RelayAdmission::Initial(ticket),
            7,
            &direct_tx,
        )
        .await
        .expect("valid initial ticket registers the player");

        assert_eq!(registration.initial_frames.len(), history_len + 1);
        assert!(matches!(
            direct_rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        let live_turn = Turn {
            turn_number: history_len as u64,
            intents: Vec::new(),
        };
        let live_frame = bincode::serialize(&super::ServerMessage::Turn(
            super::ServerTurnMessage { turn: live_turn },
        ))
        .unwrap();
        {
            let clients = lobby.clients.lock().await;
            clients
                .get(&1)
                .expect("admitted player has a live channel")
                .sender
                .try_send(Arc::new(live_frame))
                .unwrap();
        }

        let (server_io, client_io) = tokio::io::duplex(64 * 1024);
        let server = WebSocketStream::from_raw_socket(
            server_io,
            WebSocketRole::Server,
            None,
        )
        .await;
        let client = WebSocketStream::from_raw_socket(
            client_io,
            WebSocketRole::Client,
            None,
        )
        .await;
        let (mut write, _server_read) = server.split();
        let (_client_write, mut client_read) = client.split();
        super::send_admission_frames(&mut write, registration.initial_frames)
            .await
            .unwrap();
        let live_frame = direct_rx.recv().await.unwrap();
        write
            .send(Message::Binary((*live_frame).clone()))
            .await
            .unwrap();

        for expected_index in 0..=history_len + 1 {
            let message = tokio::time::timeout(Duration::from_secs(2), client_read.next())
                .await
                .expect("all admission frames arrive promptly")
                .expect("WebSocket remains open")
                .expect("valid server WebSocket frame");
            let Message::Binary(bytes) = message else {
                panic!("expected a binary server message, got {message:?}");
            };
            let message: super::ServerMessage = bincode::deserialize(&bytes).unwrap();
            if expected_index == 0 {
                assert!(matches!(
                    message,
                    super::ServerMessage::RelayReconnectTicket {
                        lobby_id: 42,
                        player_id: 1,
                        ..
                    }
                ));
            } else {
                let expected_turn = if expected_index <= history_len {
                    (expected_index - 1) as u64
                } else {
                    history_len as u64
                };
                let super::ServerMessage::Turn(turn) = message else {
                    panic!("expected turn {expected_turn}, got {message:?}");
                };
                assert_eq!(turn.turn.turn_number, expected_turn);
            }
        }
    }

    #[test]
    fn ticket_digest_rejects_tampering() {
        let ticket = "0123456789abcdef0123456789abcdef";
        let digest = Sha256::digest(ticket.as_bytes());
        let expected: [u8; 32] = digest.into();
        assert!(ticket_matches(ticket, &expected));
        assert!(!ticket_matches(
            "0123456789abcdef0123456789abcde0",
            &expected
        ));
    }

    #[test]
    fn spawn_relocations_are_limited_per_player_only_during_deployment() {
        let mut config = GameConfig::default();
        let end_tick = deployment_end_tick(Some(&config), 100.0);
        assert_eq!(end_tick, Some(150));

        let spawn = GameplayIntent::Spawn { x: 1, y: 1 };
        let mut limits = HashMap::new();
        for _ in 0..10 {
            assert!(admit_gameplay_intent(
                1,
                &spawn,
                0,
                100.0,
                end_tick,
                &mut limits
            ));
        }
        assert!(!admit_gameplay_intent(
            1,
            &spawn,
            1,
            100.0,
            end_tick,
            &mut limits
        ));
        assert!(admit_gameplay_intent(
            2,
            &spawn,
            1,
            100.0,
            end_tick,
            &mut limits
        ));
        assert!(admit_gameplay_intent(
            1,
            &spawn,
            30,
            100.0,
            end_tick,
            &mut limits
        ));
        assert!(admit_gameplay_intent(
            1,
            &spawn,
            end_tick.unwrap(),
            100.0,
            end_tick,
            &mut limits
        ));
        assert!(admit_gameplay_intent(
            1,
            &GameplayIntent::Resign,
            1,
            100.0,
            end_tick,
            &mut limits
        ));

        config.random_spawn = true;
        assert_eq!(deployment_end_tick(Some(&config), 100.0), None);
    }

    #[test]
    fn admission_enforces_per_ip_and_releases_on_close() {
        let policy = policy(4, 1, 10);
        let first: SocketAddr = "198.51.100.1:1000".parse().unwrap();
        assert!(policy.try_accept(first));
        assert!(!policy.try_accept("198.51.100.1:1001".parse().unwrap()));
        policy.on_close(first);
        assert!(policy.try_accept("198.51.100.1:1002".parse().unwrap()));
        assert_eq!(policy.metrics()["rejected_per_ip"], 1);
    }

    #[test]
    fn admission_enforces_global_cap() {
        let policy = policy(2, 2, 10);
        assert!(policy.try_accept("198.51.100.1:1000".parse().unwrap()));
        assert!(policy.try_accept("198.51.100.2:1000".parse().unwrap()));
        assert!(!policy.try_accept("198.51.100.3:1000".parse().unwrap()));
        assert_eq!(policy.metrics()["rejected_global"], 1);
    }

    #[test]
    fn participant_exit_is_reported_once_for_humans() {
        let human = "0123456789abcdef0123456789abcdef".to_string();
        let other = "fedcba9876543210fedcba9876543210".to_string();
        let third = "00112233445566778899aabbccddeeff".to_string();
        let mut tracker = MatchTracker {
            player_accounts: HashMap::from([
                (1, human.clone()),
                (2, other.clone()),
                (3, third.clone()),
            ]),
            human_accounts: HashSet::from([human.clone(), other, third]),
            participants: [1, 2, 3]
                .into_iter()
                .map(|id| (id, Participation::Active))
                .collect(),
            tracked: true,
            lobby_json: "{}".to_string(),
        };
        assert_eq!(
            tracker.record_exit(1, 41, ExitReason::Voluntary),
            (true, Some(human.clone()))
        );
        assert!(tracker.is_sealed(1));
        assert!(!tracker.reconnect(1, Instant::now()));
        assert!(!tracker.finalized());
        assert!(
            tracker.closure(&human).is_none(),
            "sealing alone is not durable acknowledgment"
        );
        assert_eq!(
            tracker.participant_closures()[0].reason,
            ExitReason::Voluntary
        );

        tracker.record_committed_closure(human.clone(), 41);
        assert!(tracker.is_sealed(1));
        assert!(!tracker.is_sealed(2));
        assert_eq!(
            tracker.participant_closures(),
            vec![ParticipantClosure {
                account_id: human.clone(),
                turn_number: 41,
                reason: ExitReason::Voluntary,
            }]
        );

        assert_eq!(
            tracker.record_exit(1, 41, ExitReason::Voluntary),
            (false, None)
        );
        tracker.record_committed_closure(human.clone(), 42);
        assert_eq!(tracker.participant_closures()[0].turn_number, 41);
        let now = Instant::now();
        tracker.disconnect(2, now);
        assert!(tracker.reconnect(2, now + Duration::from_secs(59)));
        tracker.disconnect(2, now);
        assert_eq!(tracker.expired(now + RECONNECT_GRACE), vec![2]);
        assert!(!tracker.reconnect(2, now + RECONNECT_GRACE));
        assert!(!tracker.is_sealed(3), "other players remain active");
        assert!(tracker.record_exit(2, 90, ExitReason::Timeout).0);
        assert!(!tracker.record_exit(2, 91, ExitReason::Voluntary).0);
    }

    #[tokio::test]
    async fn registered_recording_recovers_once_only_after_writer_stops() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("42.json");
        let journal = path.with_extension("journal");
        let account_a = "0123456789abcdef0123456789abcdef";
        let account_b = "abcdef0123456789abcdef0123456789";
        let mut metadata: super::ReplaySpoolMetadata = serde_json::from_value(serde_json::json!({
            "lobby_json":serde_json::json!({"lobby_id":42,"relay_port":25594,"ticket_expires_at":9999999999u64,
                "tick_number":0,"tick_rate_ms":100.0,"players":[
                    {"player_id":1,"name":"A","database_account_id":account_a},
                    {"player_id":2,"name":"B","database_account_id":account_b}
                ]}).to_string(),
            "phase":"recording", "participant_closures":[{"account_id":account_a,"turn_number":1,"reason":"timeout"}]
        })).unwrap();
        super::persist_replay_metadata(&path, &metadata)
            .await
            .unwrap();
        let turns = [
            Turn {
                turn_number: 0,
                intents: vec![],
            },
            Turn {
                turn_number: 1,
                intents: vec![sow_core::protocol::StampedIntent {
                    player_id: 1,
                    intent: GameplayIntent::Resign,
                }],
            },
        ];
        let mut bytes = Vec::new();
        for turn in turns {
            let encoded = bincode::serialize(&turn).unwrap();
            bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            bytes.extend(encoded);
        }
        bytes.extend_from_slice(&[9, 0]); // The incomplete last record was never acknowledged.
        std::fs::write(&journal, bytes).unwrap();
        let claim = super::lock_result(&journal, libc::LOCK_EX).unwrap();
        assert!(
            !super::recover_recording(&path, &mut metadata)
                .await
                .unwrap()
        );
        drop(claim);
        assert!(
            super::recover_recording(&path, &mut metadata)
                .await
                .unwrap()
        );
        assert_eq!(metadata.participant_closures.len(), 2);
        assert_eq!(
            metadata
                .participant_closures
                .iter()
                .find(|entry| entry.account_id == account_a)
                .unwrap()
                .reason,
            ExitReason::Timeout
        );
        assert_eq!(
            metadata
                .participant_closures
                .iter()
                .find(|entry| entry.account_id == account_a)
                .unwrap()
                .turn_number,
            1
        );
        let recovered = std::fs::read(path.with_extension("replay")).unwrap();
        let decoded: Vec<Turn> = bincode::deserialize(&recovered).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[2].intents[0].player_id, 2);
        assert!(
            super::recover_recording(&path, &mut metadata)
                .await
                .unwrap()
        );
        assert_eq!(
            std::fs::read(path.with_extension("replay")).unwrap(),
            recovered
        );
        assert_eq!(metadata.participant_closures.len(), 2);
    }

    #[test]
    fn participant_spool_temp_record_uses_the_durable_replay_sidecar() {
        let temp_record = Path::new("/spool/match-account.participant.tmp");
        assert_eq!(
            participant_replay_path(temp_record),
            PathBuf::from("/spool/match-account.participant.replay")
        );
    }

    #[test]
    fn personal_checkpoint_contains_only_the_committed_prefix() {
        let history = vec![
            Turn {
                turn_number: 10,
                intents: Vec::new(),
            },
            Turn {
                turn_number: 11,
                intents: Vec::new(),
            },
            Turn {
                turn_number: 12,
                intents: Vec::new(),
            },
        ];
        let prefix = replay_prefix_through(&history, 11).unwrap();
        let decoded: Vec<Turn> = bincode::deserialize(&prefix).unwrap();
        assert_eq!(
            decoded
                .iter()
                .map(|turn| turn.turn_number)
                .collect::<Vec<_>>(),
            vec![10, 11]
        );
        assert!(replay_prefix_through(&history, 99).is_err());
    }

    #[test]
    fn rejected_participant_outcome_can_be_spooled_without_a_replay() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("500-player.participant");
        let replay_bytes = Vec::new();
        let payload = ParticipantSettlementPayload {
            match_id: "500".to_string(),
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            closure_turn: 3,
            replay_sha256: hex::encode(Sha256::digest(&replay_bytes)),
            rejected: true,
        };
        persist_participant_spool(&path, &payload, &replay_bytes).unwrap();
        let saved: ParticipantSettlementPayload =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(saved.rejected);
        assert!(
            std::fs::read(participant_replay_path(&path))
                .unwrap()
                .is_empty()
        );
    }
}
