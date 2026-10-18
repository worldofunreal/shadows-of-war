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
//!     POST /internal/drain            pause or resume lobby registration
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

mod progress;

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
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::io::BufReader;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::task::{Context, Poll};
use std::time::{Instant, SystemTime};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
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
static RELAY_DRAINING: AtomicBool = AtomicBool::new(false);
static LOBBY_REGISTRATION_GATE: OnceLock<RwLock<()>> = OnceLock::new();

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
    static CLIENT: OnceLock<std::sync::Mutex<Option<(String, reqwest::Client)>>> = OnceLock::new();
    let mut pool = CLIENT
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .map_err(|e| e.to_string())?;
    if let Some((url, client)) = &*pool {
        if url == db_url {
            return Ok(client.clone());
        }
    }
    let client = build_db_client(db_url)?;
    *pool = Some((db_url.to_string(), client.clone()));
    Ok(client)
}

fn build_db_client(db_url: &str) -> Result<reqwest::Client, String> {
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

/// Bounded in-memory queue between the game tick and the server forwarder.
const PROGRESS_QUEUE_CAP: usize = 256;
const LIVE_HISTORY_CAP: usize = 512;

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
    Disconnected {
        player_id: u16,
        generation: u64,
        peer: SocketAddr,
    },
    EndMatch,
    PrepareResult {
        player_id: u16,
        generation: u64,
    },
    ClosuresRecorded {
        closures: Vec<(String, u64)>,
        result: Result<(), String>,
    },
    VerifiedResult {
        command: sow_data::profile::MatchResultControl,
        reply: oneshot::Sender<()>,
    },
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

    fn can_reconnect(&self, player_id: u16) -> bool {
        matches!(
            self.participants.get(&player_id),
            Some(
                Participation::Active
                    | Participation::Disconnected { .. }
                    | Participation::Closed {
                        reason: ExitReason::Defeated,
                        ..
                    }
            )
        )
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
            Participation::Active
            | Participation::Closed {
                reason: ExitReason::Defeated,
                ..
            } => true,
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

    fn participant_closures(&self) -> Vec<sow_data::profile::ProgressClosure> {
        let mut closures = self
            .player_accounts
            .iter()
            .filter_map(|(id, account)| match self.participants.get(id) {
                Some(
                    Participation::Closing { turn, reason }
                    | Participation::Closed { turn, reason },
                ) => Some(sow_data::profile::ProgressClosure {
                    account_id: account.clone(),
                    turn_number: *turn,
                    reason: match reason {
                        ExitReason::Voluntary => "voluntary",
                        ExitReason::Timeout => "timeout",
                        ExitReason::MatchEnded => "match_ended",
                        ExitReason::Defeated => "defeated",
                    }
                    .to_string(),
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
    closing: AtomicBool,
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
    progress: Arc<progress::Forwarder>,
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
    // Identity line: the pipeline stamps git/f-stack/knob into the unit drop-in.
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
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
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
        ("POST", "/internal/match/result") => {
            let command =
                match serde_json::from_slice::<sow_data::profile::MatchResultControl>(body) {
                    Ok(command) if command.match_id.parse::<u64>().is_ok() => command,
                    _ => {
                        return (
                            "400 Bad Request",
                            serde_json::json!({"error":"invalid result identity"}),
                        );
                    }
                };
            let id = command.match_id.parse::<u64>().unwrap();
            let lobby = registry.read().await.get(&id).cloned();
            if let Some(lobby) = lobby {
                let (reply, applied) = oneshot::channel();
                if lobby
                    .ev_tx
                    .send(RelayEvent::VerifiedResult { command, reply })
                    .await
                    .is_err()
                {
                    return (
                        "503 Service Unavailable",
                        serde_json::json!({"error":"match control unavailable"}),
                    );
                }
                if applied.await.is_err() {
                    return (
                        "503 Service Unavailable",
                        serde_json::json!({"error":"match result was not applied"}),
                    );
                }
            }
            ("200 OK", serde_json::json!({"match_id":id.to_string()}))
        }
        ("GET", "/healthz") => ("200 OK", serde_json::json!({ "ok": true })),
        ("POST", "/internal/lobby/register") => {
            let _registration = LOBBY_REGISTRATION_GATE
                .get_or_init(|| RwLock::new(()))
                .read()
                .await;
            if RELAY_DRAINING.load(Ordering::Acquire) {
                return (
                    "503 Service Unavailable",
                    serde_json::json!({"error":"relay is paused for deployment"}),
                );
            }
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
        ("POST", "/internal/drain") => {
            let draining = match serde_json::from_slice::<serde_json::Value>(body)
                .ok()
                .and_then(|value| value.get("draining").and_then(serde_json::Value::as_bool))
            {
                Some(draining) => draining,
                None => {
                    return (
                        "400 Bad Request",
                        serde_json::json!({"error":"draining must be boolean"}),
                    );
                }
            };
            let gate = LOBBY_REGISTRATION_GATE.get_or_init(|| RwLock::new(()));
            if draining {
                RELAY_DRAINING.store(true, Ordering::Release);
                drop(gate.write().await);
            } else {
                let _guard = gate.write().await;
                RELAY_DRAINING.store(false, Ordering::Release);
            }
            ("200 OK", serde_json::json!({"draining":draining}))
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
                "lobby_drain_supported": true,
                "lobby_registration_paused": RELAY_DRAINING.load(Ordering::Acquire),
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
    let progress = progress::Forwarder::new(body.lobby_id);
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
    }));

    let state = Arc::new(LobbyState {
        closing: AtomicBool::new(false),
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
        progress,
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

async fn finalize_lobby(
    state: &LobbyState,
    registry: &Registry,
    closures: Vec<sow_data::profile::ProgressClosure>,
    closing: ServerMessage,
) {
    {
        let clients = state.clients.lock().await;
        state.closing.store(true, Ordering::Release);
        if let Ok(bytes) = bincode::serialize(&closing) {
            let frame = Arc::new(bytes);
            for client in clients.values() {
                let _ = client.sender.try_send(frame.clone());
            }
        }
    }
    // The registry owns the closing match until the durable
    // recorder acknowledges its final block or rejection.
    // Paused admission can then prove replacement is safe.
    let mut attempt = 0u64;
    loop {
        match state.progress.finish(closures.clone()).await {
            Ok(()) => break,
            Err(error) => {
                attempt += 1;
                if attempt == 1 || attempt % 10 == 0 {
                    error!(
                        "[rewards] final turn delivery pending match={} attempt={attempt}: {error}",
                        state.id
                    );
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    registry.write().await.remove(&state.id);
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
    let mut exit_replies = ExitReplies::new();
    let mut end_requested = false;
    let mut natural_end: Option<u64> = None;
    let mut closure_flushes = 0usize;
    let mut spawn_rate_limits = HashMap::<u16, SpawnIntentRateLimit>::new();
    let mut total_ticks: u64 = 0;
    let mut total_intents: u64 = 0;
    // Lifecycle: a lobby is removed from the registry and its live turn history
    // dropped when the match ends (GameOver / all clients leave) OR when it
    // outlives the maximum match duration. Backfill bots play indefinitely, so
    // without this ceiling a filled lobby keeps pushing to live history forever
    // and a relay worker OOMs (5.5GB in ~4min under 50k bots).
    let match_started = std::time::Instant::now();
    let max_match_secs: u64 = std::env::var("SOW_RELAY_MATCH_MAX_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(sow_data::MAX_MATCH_SECONDS)
        .clamp(1, sow_data::MAX_MATCH_SECONDS);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let mut clients = state.clients.lock().await;
                let active_relay_connections = clients.len();
                let already_finalized = state.tracker.lock().unwrap().finalized();

                let now = Instant::now();
                let ending = end_requested || match_started.elapsed().as_secs() >= max_match_secs;
                let expired = if natural_end.is_some() { Vec::new() } else if ending {
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
                            pending_closures.push((account_id, tick_number));
                        }
                    }
                }

                // Lifecycle: relay owns live connections; the server owns the
                // durable turn stream and computes the final result.
                let is_finalized = already_finalized && clients.is_empty();
                let is_timed_out = ending || natural_end.is_some();
                if (is_finalized || is_timed_out)
                    && pending_intents.is_empty()
                    && pending_closures.is_empty()
                    && closure_flushes == 0
                {
                    let reason = if natural_end.is_some() {
                        "Match finished".to_string()
                    } else if end_requested {
                        "Match ended".to_string()
                    } else if is_finalized {
                        "All participants exited".to_string()
                    } else {
                        format!("Match duration limit reached ({}s)", max_match_secs)
                    };
                    info!("[relay] lobby {} ending: {}; awaiting durable final delivery", state.id, reason);
                    let closing = natural_end.map_or_else(|| ServerMessage::LobbyClosed(ServerLobbyClosedMessage {
                        lobby_id:state.id, reason }), |terminal_turn| ServerMessage::MatchFinished { match_id:state.id, terminal_turn });
                    drop(clients);
                    let closures = state.tracker.lock().unwrap().participant_closures();
                    finalize_lobby(&state, &registry, closures, closing).await;
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

                if let Err(error) = state.progress.append(turn.clone()) {
                    error!("[rewards] turn forwarding queue failed match={}: {error}", state.id);
                }
                if !pending_closures.is_empty() {
                    closure_flushes += 1;
                    let closures = std::mem::take(&mut pending_closures);
                    let recorded = state.tracker.lock().unwrap().participant_closures();
                    let progress = state.progress.clone();
                    let events = state.ev_tx.clone();
                    tokio::spawn(async move {
                        let result = progress.flush(recorded).await;
                        let _ = events.send(RelayEvent::ClosuresRecorded { closures, result }).await;
                    });
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
            Some(event) = ev_rx.recv() => {
                match event {
                    RelayEvent::EndMatch => { end_requested = true; }
                    RelayEvent::PrepareResult { player_id, generation } => {
                        let clients = state.clients.lock().await;
                        if clients.get(&player_id).is_some_and(|c| c.generation == generation) {
                            let closures = state.tracker.lock().unwrap().participant_closures();
                            let progress = state.progress.clone();
                            tokio::spawn(async move {
                                if let Err(error) = progress.flush(closures).await {
                                    warn!("[rewards] result preparation delivery pending: {error}");
                                }
                            });
                        }
                    }
                    RelayEvent::ClosuresRecorded { closures, result } => {
                        closure_flushes = closure_flushes.saturating_sub(1);
                        match result {
                            Ok(()) => for (account, turn) in closures {
                                state.tracker.lock().unwrap().record_committed_closure(account.clone(),turn);
                                acknowledge_exit(&mut exit_replies,&account,turn);
                                info!("[exit-timing] server accepted exit match={} account={} turn={}",state.id,account,turn);
                            },
                            Err(error) => { error!("[rewards] closure delivery pending match={}: {error}",state.id); pending_closures.extend(closures); }
                        }
                    }
                    RelayEvent::VerifiedResult { command, reply } => {
                        // The durable sender retries if the HTTP response is lost; replaying
                        // this exact closure and terminal turn leaves the same relay state.
                        let clients = state.clients.lock().await;
                        let mut tracker = state.tracker.lock().unwrap();
                        for closure in command.participant_closures {
                            if let Some(id) = tracker.player_accounts.iter().find(|(_,account)| **account == closure.account_id).map(|(id,_)|*id) {
                                let reason: ExitReason = match serde_json::from_value(serde_json::Value::String(closure.reason)) { Ok(r)=>r,Err(_)=>continue };
                                if tracker.closure(&closure.account_id).is_none_or(|(turn,_)| closure.turn_number <= turn) {
                                    tracker.participants.insert(id,Participation::Closed { turn:closure.turn_number,reason });
                                }
                                let (closure_turn,reason) = tracker.closure(&closure.account_id).unwrap_or((closure.turn_number,reason));
                                if let Some(client) = clients.get(&id) {
                                    if let Ok(bytes) = bincode::serialize(&ServerMessage::ExitRecorded { account_id:closure.account_id.clone(),match_id:state.id,closure_turn,reason }) {
                                        let _ = client.sender.try_send(Arc::new(bytes));
                                    }
                                }
                                acknowledge_exit(&mut exit_replies,&closure.account_id,closure_turn);
                            }
                        }
                        if let Some(turn) = command.terminal_turn {
                            natural_end = Some(turn);
                            pending_intents.retain(|intent| matches!(intent.intent,
                                GameplayIntent::Resign | GameplayIntent::MarkDisconnected { .. }));
                            info!("[exit-timing] natural finish confirmed match={} turn={turn}",state.id);
                        }
                        drop(tracker);
                        drop(clients);
                        let _ = reply.send(());
                    }
                    RelayEvent::Gameplay { player_id, intent, generation } => {
                        let clients = state.clients.lock().await;
                        if !clients.get(&player_id).is_some_and(|client| client.generation == generation)
                            || state.tracker.lock().unwrap().is_sealed(player_id) { continue; }
                        if natural_end.is_some() && !matches!(intent,GameplayIntent::Resign | GameplayIntent::MarkDisconnected { .. }) { continue; }
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
                        let saved = {
                            let tracker = state.tracker.lock().unwrap();
                            tracker.player_accounts.get(&player_id).and_then(|account| tracker.closure(account))
                        };
                        if let Some((turn,_)) = saved {
                            clients.remove(&player_id);
                            let _ = reply.send(Ok(turn));
                            continue;
                        }
                        let (new_exit, account_id) = state.tracker.lock().unwrap().record_exit(player_id, tick_number, ExitReason::Voluntary);
                        clients.remove(&player_id);
                        if let Some(account_id) = account_id {
                            exit_replies.entry(account_id.clone()).or_default().push(reply);
                            if new_exit {
                                pending_closures.push((account_id, tick_number));
                            }
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
        warn!(
            "[relay] admission history write failed fd={} err={}",
            fd, error
        );
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
    let mut result_hint_sent = false;

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
                                    ClientMessage::PrepareResult { .. } => {
                                        if !result_hint_sent {
                                            if let (Some(lobby),Some(player_id)) = (&my_lobby,my_player_id) {
                                                let _ = lobby.ev_tx.send(RelayEvent::PrepareResult { player_id,generation }).await;
                                                result_hint_sent = true;
                                            }
                                        }
                                    }
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
    if lobby.closing.load(Ordering::Acquire) {
        warn!(
            "[relay] rejected admission to closing match lobby={} player={}",
            lobby_id, player_id
        );
        return None;
    }
    if !lobby.tracker.lock().unwrap().can_reconnect(player_id) {
        warn!(
            "[relay] rejected reconnect after participation closed lobby={} player={}",
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
    let history = match lobby.progress.snapshot() {
        Ok(history) => history,
        Err(error) => {
            warn!("[relay] live turn history unavailable lobby={lobby_id}: {error}");
            return None;
        }
    };
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
            leader: Some(Leader::of("cleopatra")),
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
    use super::{
        AdmissionState, IpAdmissionState, MatchTracker, Participation, RECONNECT_GRACE,
        RelayAdmissionPolicy, Turn, admit_gameplay_intent, deployment_end_tick, ticket_matches,
    };
    use sha2::{Digest, Sha256};
    use sow_core::game_config::GameConfig;
    use sow_core::protocol::{ExitReason, GameplayIntent};
    use std::collections::{HashMap, HashSet};
    use std::net::SocketAddr;
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
    async fn closing_match_remains_registered_until_durable_final_ack() {
        let (progress, mut responses) = super::progress::Forwarder::with_manual_final_ack();
        let (ev_tx, _ev_rx) = tokio::sync::mpsc::channel(8);
        let state = Arc::new(super::LobbyState {
            closing: std::sync::atomic::AtomicBool::new(false),
            id: 99,
            relay_port: 25_593,
            ticket_expires_at: u64::MAX,
            roster_total: 0,
            internal_bot_count: 0,
            expected_external_connections: 0,
            valid_players: HashMap::from([(1, "human".into())]),
            session_ids: HashMap::from([(1, 1)]),
            ticket_digests: HashMap::from([(1, Sha256::digest(b"closing-ticket").into())]),
            auth: Arc::new(tokio::sync::Mutex::new(super::RelayAuthState::default())),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            progress,
            tracker: Arc::new(StdMutex::new(MatchTracker {
                player_accounts: HashMap::new(),
                human_accounts: HashSet::new(),
                participants: HashMap::from([(1, Participation::Active)]),
                tracked: true,
            })),
            ev_tx,
        });
        let registry: super::Registry = Arc::new(tokio::sync::RwLock::new(HashMap::from([(
            99,
            state.clone(),
        )])));
        let closing_registry = registry.clone();
        let closing_state = state.clone();
        let task = tokio::spawn(async move {
            super::finalize_lobby(
                &closing_state,
                &closing_registry,
                Vec::new(),
                super::ServerMessage::LobbyClosed(super::ServerLobbyClosedMessage {
                    lobby_id: 99,
                    reason: "ended".into(),
                }),
            )
            .await;
        });
        let first = tokio::time::timeout(Duration::from_secs(3), responses.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            registry.read().await.contains_key(&99),
            "pending delivery must block replacement"
        );
        let (direct, _frames) = tokio::sync::mpsc::channel(8);
        assert!(
            super::try_register_with_admission(
                &registry,
                99,
                1,
                super::RelayAdmission::Initial("closing-ticket"),
                1,
                &direct
            )
            .await
            .is_none(),
            "a closing match remains visible to deployment but rejects new game admission"
        );
        first.send(Err("acknowledgment lost".into())).unwrap();
        let retry = tokio::time::timeout(Duration::from_secs(3), responses.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            registry.read().await.contains_key(&99),
            "failed acknowledgment must preserve the closing match"
        );
        retry.send(Ok(())).unwrap();
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
        assert!(!registry.read().await.contains_key(&99));
    }

    #[tokio::test]
    async fn admission_streams_history_before_live_turns_beyond_queue_capacity() {
        use futures_util::{SinkExt, StreamExt};
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
        let progress = super::progress::Forwarder::with_history(history.clone());
        let (ev_tx, _ev_rx) = tokio::sync::mpsc::channel(8);
        let lobby = Arc::new(super::LobbyState {
            closing: std::sync::atomic::AtomicBool::new(false),
            id: 42,
            relay_port: 25_592,
            ticket_expires_at: u64::MAX,
            roster_total: 1,
            internal_bot_count: 0,
            expected_external_connections: 1,
            valid_players: HashMap::from([(1, "player".to_string())]),
            session_ids: HashMap::from([(1, 7)]),
            ticket_digests: HashMap::from([(1, ticket_digest)]),
            auth: Arc::new(tokio::sync::Mutex::new(super::RelayAuthState::default())),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            progress,
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
        let live_frame =
            bincode::serialize(&super::ServerMessage::Turn(super::ServerTurnMessage {
                turn: live_turn,
            }))
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
        let server = WebSocketStream::from_raw_socket(server_io, WebSocketRole::Server, None).await;
        let client = WebSocketStream::from_raw_socket(client_io, WebSocketRole::Client, None).await;
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
        assert_eq!(tracker.participant_closures()[0].reason, "voluntary");

        tracker.record_committed_closure(human.clone(), 41);
        assert!(tracker.is_sealed(1));
        assert!(!tracker.is_sealed(2));
        assert_eq!(
            tracker.participant_closures(),
            vec![sow_data::profile::ProgressClosure {
                account_id: human.clone(),
                turn_number: 41,
                reason: "voluntary".to_string(),
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
        tracker.participants.insert(
            3,
            Participation::Closed {
                turn: 90,
                reason: ExitReason::Defeated,
            },
        );
        assert!(tracker.can_reconnect(3));
        assert!(
            tracker.reconnect(3, now + Duration::from_secs(1)),
            "a defeated teammate can keep observing"
        );
        assert!(
            tracker.is_sealed(3),
            "a defeated teammate may observe but cannot act"
        );
        assert!(tracker.record_exit(2, 90, ExitReason::Timeout).0);
        assert!(!tracker.record_exit(2, 91, ExitReason::Voluntary).0);
    }
}
