mod bot_fill;
mod identity;
mod lobby;
mod map_catalog;
mod map_playlist;
mod matchmaking_population;
mod replay;
mod verification;

use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use lobby::{
    JoinPlayerOpts, ServerLobby, build_lobby_broadcast, force_start, is_host_teardown, join_player,
    kick_player, leave_player, lobby_to_info, master_tick, notify_lobby_closed, set_player_team,
    sync_host_lobby_to_members,
};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use sow_core::game_config::GameConfig;
use sow_core::protocol::{
    PlayerInfo, ServerJoinAckMessage, ServerJoinFailedMessage, ServerLobbiesBroadcastMessage,
};
use std::collections::HashSet;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Semaphore, broadcast, mpsc};
use tokio_tungstenite::tungstenite::protocol::Message;

const DEFAULT_RELAY_WORKER_COUNT: usize = 4;
const RELAY_PORT_MIN: u16 = 25592;
const RELAY_PORT_MAX: u16 = 26500;
const RELAY_TICKET_TTL_SECS: u64 = 900;
const RELAY_HANDOFF_SEND_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MAX_CONNECTIONS: usize = 32_768;
const HANDSHAKE_TIMEOUT_SECS: u64 = 10;

const MAX_REPLAY_INTENTS: usize = sow_data::MAX_MATCH_ARCHIVE_BYTES / 8;
const MAX_REPLAY_INTENTS_PER_TURN: usize = 4_096;
type HmacSha256 = Hmac<Sha256>;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static REJECTED_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

struct ForwardedPeerCallback {
    ip_cell: Arc<std::sync::Mutex<String>>,
}

impl tokio_tungstenite::tungstenite::handshake::server::Callback for ForwardedPeerCallback {
    fn on_request(
        self,
        req: &tokio_tungstenite::tungstenite::handshake::server::Request,
        response: tokio_tungstenite::tungstenite::handshake::server::Response,
    ) -> Result<
        tokio_tungstenite::tungstenite::handshake::server::Response,
        tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
    > {
        if let Some(real_ip) = req.headers().get("X-Real-IP")
            && let Ok(ip) = real_ip.to_str()
            && let Ok(mut guard) = self.ip_cell.lock()
        {
            *guard = ip.to_string();
        } else if let Some(forwarded) = req.headers().get("X-Forwarded-For")
            && let Ok(ip) = forwarded.to_str()
            && let Some(first_ip) = ip.split(',').next()
            && let Ok(mut guard) = self.ip_cell.lock()
        {
            *guard = first_ip.trim().to_string();
        }
        Ok(response)
    }
}

fn next_session_id() -> u64 {
    NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed)
}

fn max_server_connections() -> Result<usize, String> {
    let value = std::env::var("SOW_SERVER_MAX_CONNECTIONS")
        .unwrap_or_else(|_| DEFAULT_MAX_CONNECTIONS.to_string());
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "SOW_SERVER_MAX_CONNECTIONS must be a positive integer".to_string())?;
    if parsed == 0 {
        return Err("SOW_SERVER_MAX_CONNECTIONS must be a positive integer".to_string());
    }
    Ok(parsed)
}

#[derive(Clone, Debug)]
struct RelayWorker {
    id: usize,
    host: String,
    mgmt_url: String,
}

fn relay_worker_count() -> usize {
    std::env::var("SOW_RELAY_WORKER_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| (1..=64).contains(count))
        .unwrap_or(DEFAULT_RELAY_WORKER_COUNT)
}

fn relay_mgmt_scheme() -> String {
    match std::env::var("SOW_RELAY_MGMT_SCHEME")
        .unwrap_or_else(|_| "https".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "http" => {
            log::error!("SOW_RELAY_MGMT_SCHEME=http is forbidden; using https");
            "https".to_string()
        }
        "https" => "https".to_string(),
        other => {
            log::warn!("Unsupported SOW_RELAY_MGMT_SCHEME={other}; using https");
            "https".to_string()
        }
    }
}

fn new_relay_ticket() -> (String, String) {
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let ticket = hex::encode(raw);
    let digest = hex::encode(Sha256::digest(ticket.as_bytes()));
    (ticket, digest)
}

fn relay_mgmt_headers(
    secret: &str,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(String, String, String), String> {
    if secret.trim().is_empty() {
        return Err("SOW_RELAY_CONTROL_SECRET missing".to_string());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock before UNIX epoch: {e}"))?
        .as_secs()
        .to_string();
    let mut nonce_bytes = [0u8; 16];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| "invalid relay control secret".to_string())?;
    mac.update(method.as_bytes());
    mac.update(b"\n");
    mac.update(path.as_bytes());
    mac.update(b"\n");
    mac.update(timestamp.as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    Ok((timestamp, nonce, hex::encode(mac.finalize().into_bytes())))
}

/// Keep the TLS hostname from the management URL for certificate validation,
/// while allowing the production host to pin DNS resolution to the relay's
/// management NIC. The game hostname and management route intentionally use
/// different network interfaces on the two-NIC relay VM.
fn relay_mgmt_client(mgmt_url: &str) -> Result<reqwest::Client, String> {
    static CLIENTS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, reqwest::Client>>,
    > = std::sync::OnceLock::new();
    let mut clients = CLIENTS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "relay management client registry lock failed".to_string())?;
    if let Some(client) = clients.get(mgmt_url) {
        return Ok(client.clone());
    }
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none());
    let parsed = reqwest::Url::parse(mgmt_url)
        .map_err(|e| format!("invalid relay management URL {mgmt_url}: {e}"))?;
    if parsed.scheme() != "https" {
        return Err(format!("relay management URL must use https: {mgmt_url}"));
    }
    if let Ok(raw_ip) = std::env::var("SOW_RELAY_MGMT_RESOLVE_IP") {
        let ip = raw_ip
            .parse::<IpAddr>()
            .map_err(|e| format!("invalid SOW_RELAY_MGMT_RESOLVE_IP={raw_ip}: {e}"))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| format!("relay management URL has no host: {mgmt_url}"))?;
        if host.parse::<IpAddr>().is_err() {
            let port = parsed.port().unwrap_or(443);
            builder = builder.resolve(host, SocketAddr::new(ip, port));
        }
    }
    let client = builder
        .build()
        .map_err(|e| format!("build relay management client: {e}"))?;
    clients.insert(mgmt_url.to_string(), client.clone());
    Ok(client)
}

fn validate_runtime_security() -> Result<(), String> {
    let scheme = std::env::var("SOW_RELAY_MGMT_SCHEME")
        .unwrap_or_else(|_| "https".to_string())
        .to_ascii_lowercase();
    if scheme != "https" {
        return Err("SOW_RELAY_MGMT_SCHEME must be https".to_string());
    }
    if let Ok(url) = std::env::var("SOW_RELAY_MGMT_URL") {
        let parsed = reqwest::Url::parse(&url)
            .map_err(|e| format!("invalid SOW_RELAY_MGMT_URL={url}: {e}"))?;
        if parsed.scheme() != "https" {
            return Err("SOW_RELAY_MGMT_URL must use https".to_string());
        }
    }
    Ok(())
}

/// Parse one worker as `game_host:mgmt_host:mgmt_port`.
///
/// `SOW_RELAY_WORKERS` is a comma-separated catalog in this format. Hosts are
/// intentionally plain DNS/IP tokens here; IPv6 literals should be supplied
/// through a front door/DNS name until the client URL contract is upgraded.
fn parse_relay_worker(spec: &str, id: usize) -> Option<RelayWorker> {
    let fields: Vec<_> = spec.trim().split(':').collect();
    let (host, mgmt_host, mgmt_port) = match fields.as_slice() {
        [host, mgmt_host, mgmt_port] => (*host, *mgmt_host, *mgmt_port),
        _ => return None,
    };
    let mgmt_port = mgmt_port.parse::<u16>().ok()?;
    if host.is_empty()
        || host.contains('/')
        || host.contains('[')
        || host.contains(']')
        || mgmt_host.is_empty()
        || mgmt_host.contains('/')
        || mgmt_host.contains('[')
        || mgmt_host.contains(']')
    {
        return None;
    }
    let scheme = relay_mgmt_scheme();
    Some(RelayWorker {
        id,
        host: host.to_string(),
        mgmt_url: format!("{scheme}://{mgmt_host}:{mgmt_port}"),
    })
}

fn relay_workers() -> Result<Vec<RelayWorker>, String> {
    let specs = std::env::var("SOW_RELAY_WORKERS")
        .map_err(|_| "SOW_RELAY_WORKERS must configure every relay worker".to_string())?;
    let workers: Vec<_> = specs
        .split(',')
        .enumerate()
        .map(|(id, spec)| {
            parse_relay_worker(spec, id).ok_or_else(|| {
                format!("SOW_RELAY_WORKERS contains an invalid worker entry at index {id}")
            })
        })
        .collect::<Result<_, _>>()?;
    if workers.is_empty() {
        return Err("SOW_RELAY_WORKERS must contain at least one worker".to_string());
    }
    Ok(workers)
}

struct RelayPortAllocator {
    next: u32,
    used: HashSet<u16>,
}

impl RelayPortAllocator {
    fn new() -> Self {
        Self {
            next: RELAY_PORT_MIN as u32,
            used: HashSet::new(),
        }
    }

    fn allocate(&mut self) -> Option<u16> {
        let capacity = (RELAY_PORT_MAX as u32 - RELAY_PORT_MIN as u32) + 1;
        for _ in 0..capacity {
            if self.next > RELAY_PORT_MAX as u32 {
                self.next = RELAY_PORT_MIN as u32;
            }
            let candidate = self.next as u16;
            self.next += 1;
            if self.used.insert(candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn release(&mut self, port: u16) {
        self.used.remove(&port);
    }
}

/// Register a lobby with its assigned relay worker over mgmt HTTP. The worker
/// confirms with 200 OK before the server broadcasts Start to the clients.
/// Retries with exponential backoff (same resilience as the old spawn path).
async fn register_relay(rc: &RelayCandidate, worker: &RelayWorker) -> Result<(), String> {
    let url = format!(
        "{}/internal/lobby/register",
        worker.mgmt_url.trim_end_matches('/')
    );

    let relay_config = serde_json::json!({
        "lobby_id": rc.lobby_id,
        "relay_port": rc.relay_port,
        "ticket_expires_at": rc.ticket_expires_at,
        "tick_number": 0,
        "players": rc.relay_players_json,
        "tick_rate_ms": rc.tick_rate_ms,
        "config": rc.config,
        "kind": rc.kind,
    });

    let secret = std::env::var("SOW_RELAY_CONTROL_SECRET")
        .map_err(|_| "SOW_RELAY_CONTROL_SECRET missing while registering relay".to_string())?;
    let path = "/internal/lobby/register";
    let body = serde_json::to_vec(&relay_config)
        .map_err(|e| format!("serialize relay registration: {e}"))?;
    let client = relay_mgmt_client(&url)?;
    for attempt in 1..=5 {
        let (timestamp, nonce, signature) = relay_mgmt_headers(&secret, "POST", path, &body)?;
        match client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-SOW-Timestamp", timestamp)
            .header("X-SOW-Nonce", nonce)
            .header("X-SOW-Signature", signature)
            .body(body.clone())
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                log::info!(
                    "[RELAY] Lobby {} accepted by relay ({} OK)",
                    rc.lobby_id,
                    url
                );
                return Ok(());
            }
            Ok(res) => {
                log::warn!(
                    "[RELAY] register lobby {}: relay returned HTTP {} (attempt {}/5)",
                    rc.lobby_id,
                    res.status(),
                    attempt
                );
            }
            Err(e) => {
                log::warn!(
                    "[RELAY] register lobby {}: relay unreachable: {e} (attempt {}/5)",
                    rc.lobby_id,
                    attempt
                );
            }
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }
    Err(format!(
        "relay registration failed for lobby {}",
        rc.lobby_id
    ))
}

/// Deliver the capability before Start, preserving wire order and refusing to
/// silently continue when the client's bounded handoff queue is unavailable.
async fn send_relay_handoff_frame(
    tx: &mpsc::Sender<Vec<u8>>,
    frame: Vec<u8>,
    lobby_id: u64,
    player_id: u16,
    label: &str,
) -> Result<(), String> {
    tokio::time::timeout(
        Duration::from_secs(RELAY_HANDOFF_SEND_TIMEOUT_SECS),
        tx.send(frame),
    )
    .await
    .map_err(|_| format!("timed out sending {label} for lobby {lobby_id} player {player_id}"))?
    .map_err(|_| {
        format!("client channel closed before {label} for lobby {lobby_id} player {player_id}")
    })
}

async fn verify_identity(
    identity: &identity::IdentityState,
    auth: &sow_core::protocol::AuthProof,
    requested_leader: sow_core::player::Leader,
    random_leader: bool,
) -> Result<identity::VerifiedIdentity, String> {
    identity
        .verify_auth_proof(auth, requested_leader, random_leader)
        .await
}

async fn register_match_start(rc: &RelayCandidate) -> Result<(), String> {
    if rc.player_ids.is_empty() {
        return Ok(());
    }
    let db_base_url =
        std::env::var("SOW_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let secret_token = std::env::var("SOW_DB_SECRET")
        .map_err(|_| "SOW_DB_SECRET missing while registering match".to_string())?;
    let url = format!("{}/match/start", db_base_url.trim_end_matches('/'));
    let trusted_players = rc
        .relay_players_json
        .iter()
        .map(|player| {
            serde_json::json!({
                "player_id": player.get("player_id"),
                "database_account_id": player.get("database_account_id"),
                "leader": player.get("leader"),
                "team": player.get("team"),
                "is_internal": player.get("is_internal"),
            })
        })
        .collect::<Vec<_>>();
    let map_sha256 = verification::register_map(&rc.config).await?;
    let payload = serde_json::json!({
        "match_id": rc.lobby_id.to_string(),
        "player_ids": rc.player_ids,
        "metadata": {
            "replay_format": sow_core::protocol::GAMEPLAY_FORMAT_VERSION,
            "simulation_rules_id": sow_core::SIMULATION_RULES_ID,
            "map_sha256": map_sha256,
            "relay_worker_index": rc.worker_index,
            "relay_port": rc.relay_port,
            "seed": rc.seed,
            "config": rc.config,
            "kind": rc.kind,
            "players": rc.start_players,
            "relay_players": trusted_players,
            "started_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        },
    });
    let client = reqwest::Client::new();

    for attempt in 1..=5 {
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {secret_token}"))
            .json(&payload)
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                log::info!("[DB] Match {} registered before relay handoff", rc.lobby_id);
                return Ok(());
            }
            Ok(res) => {
                log::warn!(
                    "[DB] match {} registration returned HTTP {} (attempt {attempt}/5)",
                    rc.lobby_id,
                    res.status()
                );
            }
            Err(e) => {
                log::warn!(
                    "[DB] match {} registration failed: {e} (attempt {attempt}/5)",
                    rc.lobby_id
                );
            }
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }
    Err(format!(
        "database match registration failed for lobby {}",
        rc.lobby_id
    ))
}

#[derive(serde::Deserialize)]
struct BotPoolSeedResponse {
    account_ids: Vec<String>,
}

/// Resolve (or lazily create) the persistent bot-account pool at boot.
///
/// Builds a deterministic spec of `SOW_BOT_POOL_SIZE` (default 10000)
/// identities where `external_id = "bot_{:05}"` and `display_name` cycles
/// through `bot_fill::names::BOT_NAMES`. Calls
/// `POST {SOW_DB_URL}/internal/bot-pool/seed` with those external_ids — the
/// db get-or-creates each one and returns the stable account_ids in order.
/// The resulting `(account_id, display_name)` pairs are installed as the
/// process-wide `BotPool`.
///
/// MENTAL MODEL (organic): the pool is identities only — it never dictates
/// timing. Entry is a chaotic drip in `bot_fill` (see its module header).
///
/// A lobby server without real Ghost identities is not healthy. Fail startup
/// instead of installing an empty pool and silently serving human-only lobbies.
async fn init_bot_pool() -> Result<(), String> {
    let pool_size: usize = env::var("SOW_BOT_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);

    if pool_size == 0 {
        return Err("SOW_BOT_POOL_SIZE must be greater than zero".to_string());
    }

    let name_pool = bot_fill::names::BOT_NAMES.as_slice();
    if name_pool.is_empty() {
        return Err("names::BOT_NAMES is empty — cannot seed pool".to_string());
    }

    let external_ids: Vec<String> = (0..pool_size).map(|i| format!("bot_{:05}", i)).collect();
    let display_names: Vec<String> = (0..pool_size)
        .map(|i| name_pool[i % name_pool.len()].to_string())
        .collect();

    let db_base_url =
        env::var("SOW_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let secret = env::var("SOW_DB_SECRET")
        .map_err(|_| "SOW_DB_SECRET missing — cannot seed pool".to_string())?;
    let url = format!(
        "{}/internal/bot-pool/seed",
        db_base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({ "external_ids": external_ids });

    let client = reqwest::Client::new();
    for attempt in 1..=5 {
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {secret}"))
            .json(&body)
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => match res.json::<BotPoolSeedResponse>().await {
                Ok(resp) if resp.account_ids.len() == pool_size => {
                    let entries: Vec<bot_fill::BotPoolEntry> = resp
                        .account_ids
                        .into_iter()
                        .zip(display_names.iter().cloned())
                        .map(|(account_id, display_name)| bot_fill::BotPoolEntry {
                            account_id,
                            display_name,
                        })
                        .collect();
                    log::info!("[BOT_POOL] resolved {} identities", entries.len());
                    bot_fill::BotPool::new(entries).install();
                    return Ok(());
                }
                Ok(resp) => log::warn!(
                    "[BOT_POOL] seed returned {} ids, expected {}",
                    resp.account_ids.len(),
                    pool_size
                ),
                Err(e) => log::warn!("[BOT_POOL] seed response parse error: {e}"),
            },
            Ok(res) => log::warn!(
                "[BOT_POOL] seed HTTP {} (attempt {attempt}/5)",
                res.status()
            ),
            Err(e) => log::warn!("[BOT_POOL] seed unreachable: {e} (attempt {attempt}/5)"),
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }

    Err(format!("seed failed after 5 attempts: {url}"))
}

/// Create the persistent, ranked-only IQ roster once per account identity.
async fn init_ranked_bot_pool() -> Result<(), String> {
    let db_base_url = env::var("SOW_DB_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let secret = env::var("SOW_DB_SECRET")
        .map_err(|_| "SOW_DB_SECRET missing — cannot seed ranked bots".to_string())?;
    let url = format!("{}/internal/ranked-bot", db_base_url.trim_end_matches('/'));
    let leaders: Vec<_> = (0..sow_core::player::Leader::count())
        .filter_map(sow_core::player::Leader::nth)
        .collect();
    if leaders.is_empty() {
        return Err("ranked bot leader catalog is empty".into());
    }
    let bots: Vec<_> = (60..=180).step_by(20).flat_map(|iq| {
        let leaders = &leaders;
        (0..8).map(move |index| {
            let leader = leaders[(iq as usize / 20 + index) % leaders.len()];
            serde_json::json!({
                "external_id": format!("ranked-iq-{iq}-{index:02}"),
                "display_name": format!("Ranked {iq}-{:02}", index + 1),
                "leader": leader.name(),
                "iq": iq,
            })
        })
    }).collect();
    let body = serde_json::json!({ "action": "seed", "bots": bots });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("ranked bot seed client: {error}"))?;
    for attempt in 1..=5 {
        match client.post(&url)
            .header("Authorization", format!("Bearer {secret}"))
            .json(&body).send().await
        {
            Ok(response) if response.status().is_success() => {
                let body: serde_json::Value = response.json().await
                    .map_err(|error| format!("ranked bot seed response: {error}"))?;
                let count = body.get("accounts").and_then(serde_json::Value::as_array).map_or(0, Vec::len);
                if count == 56 {
                    log::info!("[RANKED_BOTS] initialized {count} persistent ranked accounts");
                    return Ok(());
                }
                return Err(format!("ranked bot seed returned {count} accounts, expected 56"));
            }
            Ok(response) => log::warn!("[RANKED_BOTS] seed HTTP {} (attempt {attempt}/5)", response.status()),
            Err(error) => log::warn!("[RANKED_BOTS] seed unreachable: {error} (attempt {attempt}/5)"),
        }
        if attempt < 5 { tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await; }
    }
    Err(format!("ranked bot seed failed after 5 attempts: {url}"))
}

#[derive(Clone)]
struct RankedQueuePlayer {
    account_id: String,
    rating: f64,
    name: String,
    clan_tag: String,
    leader: sow_core::player::Leader,
    skin_style: u8,
    client_tx: mpsc::Sender<Vec<u8>>,
    ip: String,
    session_id: u64,
    search_started_at: tokio::time::Instant,
}

struct RankedQueueEntry {
    player: RankedQueuePlayer,
    last_reported_secs: u32,
    last_bot_attempt: Option<tokio::time::Instant>,
}

enum RankedQueueMessage {
    Search(RankedQueuePlayer),
    Cancel { account_id: String, session_id: u64 },
    ReleaseBot { account_id: String, match_id: u64 },
}

async fn ranked_db_post(
    client: &reqwest::Client,
    db_url: &str,
    secret: &str,
    path: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    client.post(format!("{}/{}", db_url.trim_end_matches('/'), path))
        .bearer_auth(secret)
        .json(&body)
        .send().await
        .map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| error.to_string())?
        .json().await.map_err(|error| error.to_string())
}

fn send_ranked_failure(tx: &mpsc::Sender<Vec<u8>>, reason: &str) {
    let message = sow_core::protocol::ServerMessage::JoinFailed(ServerJoinFailedMessage {
        reason: reason.to_string(),
    });
    if let Ok(bytes) = bincode::serialize(&message) {
        let _ = tx.try_send(bytes);
    }
}

fn consume_ranked_queue_message(pending: &mut Vec<RankedQueueEntry>, message: RankedQueueMessage) {
    match message {
        RankedQueueMessage::Search(mut player) => {
            if let Some(entry) = pending.iter_mut().find(|entry| entry.player.account_id == player.account_id) {
                player.search_started_at = entry.player.search_started_at;
                entry.player = player;
            } else {
                pending.push(RankedQueueEntry {
                    player,
                    last_reported_secs: u32::MAX,
                    last_bot_attempt: None,
                });
            }
        }
        RankedQueueMessage::Cancel { account_id, session_id } => {
            pending.retain(|entry| !(entry.player.account_id == account_id && entry.player.session_id == session_id));
        }
        RankedQueueMessage::ReleaseBot { .. } => {}
    }
}

fn ranked_requeue_players(lobby: &ServerLobby, leaving_player_id: u16) -> Vec<RankedQueuePlayer> {
    lobby.players.iter()
        .filter(|member| member.player_id != leaving_player_id && !member.is_internal_bot)
        .filter_map(|member| Some(RankedQueuePlayer {
            account_id: member.database_account_id.clone()?,
            rating: member.ranked_rating.filter(|rating| rating.is_finite())?,
            name: member.name.clone(),
            clan_tag: member.clan_tag.clone(),
            leader: member.leader,
            skin_style: member.skin_style,
            client_tx: member.tx.clone(),
            ip: member.ip.clone(),
            session_id: member.session_id?,
            search_started_at: member.ranked_search_started_at?,
        }))
        .filter(|player| !player.client_tx.is_closed())
        .collect()
}

fn send_ranked_lobby_closed(tx: &mpsc::Sender<Vec<u8>>, lobby_id: u64, reason: &str) {
    let closed = sow_core::protocol::ServerMessage::LobbyClosed(
        sow_core::protocol::ServerLobbyClosedMessage {
            lobby_id,
            reason: reason.to_string(),
        },
    );
    if let Ok(bytes) = bincode::serialize(&closed)
        && tx.try_send(bytes).is_err()
    {
        log::warn!("[RANKED] could not deliver {reason} for match {lobby_id}");
    }
}

fn notify_ranked_requeue(player: &RankedQueuePlayer, lobby_id: u64) {
    send_ranked_lobby_closed(&player.client_tx, lobby_id, "RANKED_REQUEUE");
}

async fn cancel_ranked_search(
    games: &Mutex<Vec<ServerLobby>>,
    pending: &mut Vec<RankedQueueEntry>,
    account_id: &str,
    session_id: u64,
    db_client: &reqwest::Client,
    db_url: &str,
    db_secret: &str,
) {
    pending.retain(|entry| !(entry.player.account_id == account_id && entry.player.session_id == session_id));
    let (lobby_id, cancelled_tx, requeue, bot_ids) = {
        let mut games = games.lock().await;
        let Some(index) = games.iter().position(|lobby| {
            lobby.kind == sow_core::protocol::LobbyKind::Ranked1v1
                && lobby.players.iter().any(|member| {
                    member.database_account_id.as_deref() == Some(account_id)
                        && member.session_id == Some(session_id)
                })
        }) else {
            return;
        };
        let lobby = &games[index];
        let leaving_player = lobby.players.iter()
            .find(|member| member.database_account_id.as_deref() == Some(account_id)
                && member.session_id == Some(session_id));
        let leaving_player_id = leaving_player
            .map(|member| member.player_id)
            .expect("ranked lobby match was found above");
        let cancelled_tx = leaving_player.map(|member| member.tx.clone())
            .expect("ranked lobby match was found above");
        let requeue = ranked_requeue_players(lobby, leaving_player_id);
        for player in &requeue {
            notify_ranked_requeue(player, lobby.id);
        }
        let bot_ids = lobby.players.iter()
            .filter(|member| member.is_internal_bot)
            .filter_map(|member| member.database_account_id.clone())
            .collect::<Vec<_>>();
        let lobby_id = lobby.id;
        games.remove(index);
        (lobby_id, cancelled_tx, requeue, bot_ids)
    };
    send_ranked_lobby_closed(&cancelled_tx, lobby_id, "RANKED_CANCELLED");
    for bot_id in bot_ids {
        release_ranked_bot(db_client, db_url, db_secret, &bot_id, lobby_id).await;
    }
    for player in requeue {
        consume_ranked_queue_message(pending, RankedQueueMessage::Search(player));
    }
}

async fn accept_ranked_queue_message(
    games: &Mutex<Vec<ServerLobby>>,
    pending: &mut Vec<RankedQueueEntry>,
    message: RankedQueueMessage,
    db_client: &reqwest::Client,
    db_url: &str,
    db_secret: &str,
) {
    match &message {
        RankedQueueMessage::ReleaseBot { account_id, match_id } => {
            release_ranked_bot(db_client, db_url, db_secret, account_id, *match_id).await;
            return;
        }
        RankedQueueMessage::Cancel { account_id, session_id } => {
            cancel_ranked_search(games, pending, account_id, *session_id, db_client, db_url, db_secret).await;
            return;
        }
        RankedQueueMessage::Search(_) => {}
    }
    let RankedQueueMessage::Search(player) = message else { unreachable!() };
    let ack = {
        let mut games = games.lock().await;
        games.iter_mut().find(|game| game.kind == sow_core::protocol::LobbyKind::Ranked1v1
            && game.players.iter().any(|member| member.database_account_id.as_deref() == Some(player.account_id.as_str())))
            .and_then(|game| {
                if let Some(member) = game.players.iter_mut().find(|member| member.database_account_id.as_deref() == Some(player.account_id.as_str())) {
                    if member.is_internal_bot { return None; }
                    member.tx = player.client_tx.clone();
                    member.session_id = Some(player.session_id);
                    Some(ServerJoinAckMessage {
                        lobby_id: game.id,
                        player_id: member.player_id,
                        map_name: game.config.map_name.clone(),
                        is_private: true,
                        lobby_info: lobby_to_info(game),
                    })
                } else { None }
            })
    };
    if let Some(ack) = ack {
        if let Ok(bytes) = bincode::serialize(&sow_core::protocol::ServerMessage::JoinAck(ack)) {
            let _ = player.client_tx.try_send(bytes);
        }
    } else {
        consume_ranked_queue_message(pending, RankedQueueMessage::Search(player));
    }
}

fn ranked_bot_fields(account: &serde_json::Value) -> Result<(String, String, sow_core::player::Leader, u32), String> {
    let account_id = account.get("account_id").and_then(serde_json::Value::as_str)
        .ok_or("ranked bot account id missing")?.to_string();
    let name = account.get("display_name").and_then(serde_json::Value::as_str)
        .ok_or("ranked bot display name missing")?.to_string();
    let bot = account.get("profile").and_then(|profile| profile.get("ranked_bot"))
        .ok_or("ranked bot profile missing")?;
    let leader_name = bot.get("leader").and_then(serde_json::Value::as_str)
        .ok_or("ranked bot leader missing")?;
    let iq = bot.get("iq").and_then(serde_json::Value::as_u64)
        .and_then(|iq| u32::try_from(iq).ok()).ok_or("ranked bot IQ missing")?;
    let leader = sow_data::commerce::leader_from_id(leader_name)
        .ok_or("ranked bot leader is not in the catalog")?;
    Ok((account_id, name, leader, iq))
}

async fn allocate_match_id(next_id: &Mutex<u64>) -> Result<u64, String> {
    let mut next = next_id.lock().await;
    let id = *next;
    *next = next.checked_add(1).ok_or("match id space exhausted")?;
    Ok(id)
}

async fn ranked_candidates(
    client: &reqwest::Client,
    db_url: &str,
    secret: &str,
) -> Result<Vec<(serde_json::Value, f64)>, String> {
    let payload = ranked_db_post(client, db_url, secret, "internal/ranked-bots/available", serde_json::json!({})).await?;
    let candidates = payload.get("candidates").and_then(serde_json::Value::as_array)
        .ok_or("ranked bot candidates missing")?;
    candidates.iter().map(|candidate| {
        let account = candidate.get("account").cloned().ok_or("ranked bot account missing")?;
        let rating = candidate.get("rating").and_then(|rating| rating.get("mu"))
            .and_then(serde_json::Value::as_f64).ok_or("ranked bot rating missing")?;
        if !rating.is_finite() { return Err("ranked bot rating is invalid".into()); }
        Ok((account, rating))
    }).collect()
}

async fn fetch_ranked_rating(
    client: &reqwest::Client,
    db_url: &str,
    secret: &str,
    account_id: &str,
) -> Result<f64, String> {
    let payload = ranked_db_post(client, db_url, secret, "internal/ranked-rating", serde_json::json!({
        "account_id": account_id
    })).await?;
    let rating = payload.get("mu").and_then(serde_json::Value::as_f64)
        .ok_or("ranked rating value missing")?;
    if rating.is_finite() { Ok(rating) } else { Err("ranked rating value is invalid".into()) }
}

async fn reserve_ranked_bot(
    client: &reqwest::Client,
    db_url: &str,
    secret: &str,
    account_id: &str,
    match_id: u64,
) -> Result<Option<serde_json::Value>, String> {
    let payload = ranked_db_post(client, db_url, secret, "internal/ranked-bot", serde_json::json!({
        "action": "reserve", "account_id": account_id, "match_id": match_id.to_string()
    })).await?;
    Ok(payload.get("accounts").and_then(serde_json::Value::as_array).and_then(|items| items.first()).cloned())
}

async fn release_ranked_bot(client: &reqwest::Client, db_url: &str, secret: &str, account_id: &str, match_id: u64) {
    if let Err(error) = ranked_db_post(client, db_url, secret, "internal/ranked-bot", serde_json::json!({
        "action": "release", "account_id": account_id, "match_id": match_id.to_string()
    })).await {
        log::error!("[RANKED_BOTS] failed to release reservation for match {match_id}: {error}");
    }
}

async fn recover_ranked_bot_reservations(
    client: &reqwest::Client,
    db_url: &str,
    db_secret: &str,
    workers: &[RelayWorker],
) -> Result<usize, String> {
    let relay_secret = std::env::var("SOW_RELAY_CONTROL_SECRET")
        .map_err(|_| "SOW_RELAY_CONTROL_SECRET missing".to_string())?;
    let mut live_match_ids = std::collections::BTreeSet::new();
    for worker in workers {
        let path = "/internal/lobbies";
        let url = format!("{}{}", worker.mgmt_url.trim_end_matches('/'), path);
        let (timestamp, nonce, signature) = relay_mgmt_headers(&relay_secret, "GET", path, &[])?;
        let response = relay_mgmt_client(&url)?
            .get(&url)
            .header("X-SOW-Timestamp", timestamp)
            .header("X-SOW-Nonce", nonce)
            .header("X-SOW-Signature", signature)
            .send().await
            .map_err(|error| format!("relay worker {} roster request: {error}", worker.id))?
            .error_for_status()
            .map_err(|error| format!("relay worker {} roster response: {error}", worker.id))?
            .json::<serde_json::Value>().await
            .map_err(|error| format!("relay worker {} roster payload: {error}", worker.id))?;
        let lobbies = response.get("lobbies").and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("relay worker {} roster is missing lobbies", worker.id))?;
        for lobby in lobbies {
            let id = lobby.get("lobby_id").and_then(serde_json::Value::as_u64)
                .ok_or_else(|| format!("relay worker {} returned an invalid lobby id", worker.id))?;
            live_match_ids.insert(id.to_string());
        }
    }
    let result = ranked_db_post(client, db_url, db_secret, "internal/ranked-bot", serde_json::json!({
        "action": "recover",
        "live_match_ids": live_match_ids.into_iter().collect::<Vec<_>>(),
    })).await?;
    result.get("released").and_then(serde_json::Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or("ranked bot recovery response is missing released count".into())
}

async fn retry_ranked_bot_reservation_recovery(
    client: reqwest::Client,
    db_url: String,
    db_secret: String,
    workers: Vec<RelayWorker>,
) {
    loop {
        match recover_ranked_bot_reservations(&client, &db_url, &db_secret, &workers).await {
            Ok(released) if released > 0 => log::info!("[RANKED_BOTS] recovered {released} stale reservations"),
            Ok(_) => {}
            Err(error) => log::error!("[RANKED_BOTS] reservation recovery failed: {error}"),
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

fn take_ranked_pair(pending: &mut Vec<RankedQueueEntry>, now: tokio::time::Instant) -> Option<[RankedQueuePlayer; 2]> {
    let searches = pending.iter().map(|entry| lobby::RankedSearch {
        account_id: entry.player.account_id.clone(),
        rating: entry.player.rating,
        entered_at: entry.player.search_started_at.into_std(),
    }).collect::<Vec<_>>();
    let (first, second) = lobby::ranked_human_pair(&searches, now.into_std())?;
    let high = first.max(second);
    let low = first.min(second);
    let second_player = pending.remove(high).player;
    let first_player = pending.remove(low).player;
    Some([first_player, second_player])
}

async fn start_ranked_lobby(
    games: &Mutex<Vec<ServerLobby>>,
    id: u64,
    humans: Vec<RankedQueuePlayer>,
    bot: Option<serde_json::Value>,
    reserved_bot: Option<String>,
    db_url: &str,
    db_secret: &str,
    db_client: &reqwest::Client,
) {
    let failure_txs = humans.iter().map(|human| human.client_tx.clone()).collect::<Vec<_>>();
    let mut participants = Vec::with_capacity(2);
    for human in humans {
        participants.push(lobby::RankedLobbyPlayer {
            name: human.name,
            clan_tag: human.clan_tag,
            leader: human.leader,
            skin_style: human.skin_style,
            account_id: human.account_id,
            tx: human.client_tx,
            ip: human.ip,
            session_id: Some(human.session_id),
            is_bot: false,
            bot_iq: None,
            ranked_rating: Some(human.rating),
            ranked_search_started_at: Some(human.search_started_at),
        });
    }
    if let Some(bot) = bot {
        match ranked_bot_fields(&bot) {
            Ok((account_id, name, leader, iq)) => {
                let (tx, _rx) = mpsc::channel(1);
                participants.push(lobby::RankedLobbyPlayer {
                    name,
                    clan_tag: String::new(),
                    leader,
                    skin_style: 0,
                    account_id,
                    tx,
                    ip: "127.0.0.1".into(),
                    session_id: None,
                    is_bot: true,
                    bot_iq: Some(iq),
                    ranked_rating: None,
                    ranked_search_started_at: None,
                });
            }
            Err(error) => {
                log::error!("[RANKED_BOTS] invalid bot selected for match {id}: {error}");
                if let Some(account_id) = reserved_bot { release_ranked_bot(db_client, db_url, db_secret, &account_id, id).await; }
                return;
            }
        }
    }
    let participants: [lobby::RankedLobbyPlayer; 2] = match participants.try_into() {
        Ok(participants) => participants,
        Err(_) => {
            log::error!("[RANKED] match {id} did not have exactly two participants");
            if let Some(account_id) = reserved_bot { release_ranked_bot(db_client, db_url, db_secret, &account_id, id).await; }
            return;
        }
    };
    let acks = {
        let mut games = games.lock().await;
        match lobby::spawn_ranked_lobby(&mut games, id, participants) {
            Ok(lobby_id) => {
                let Some(game) = games.iter().find(|game| game.id == lobby_id) else { return; };
                let lobby_info = lobby_to_info(game);
                game.players.iter().filter(|player| !player.is_internal_bot).map(|player| {
                    let ack = ServerJoinAckMessage {
                        lobby_id,
                        player_id: player.player_id,
                        map_name: game.config.map_name.clone(),
                        is_private: true,
                        lobby_info: lobby_info.clone(),
                    };
                    (player.tx.clone(), ack)
                }).collect::<Vec<_>>()
            }
            Err(error) => {
                log::error!("[RANKED] match {id} could not be created: {error}");
                Vec::new()
            }
        }
    };
    if acks.is_empty() {
        for tx in failure_txs { send_ranked_failure(&tx, "RANKED_MATCH_PREPARATION_FAILED"); }
        if let Some(account_id) = reserved_bot { release_ranked_bot(db_client, db_url, db_secret, &account_id, id).await; }
        return;
    }
    for (tx, ack) in acks {
        if let Ok(bytes) = bincode::serialize(&sow_core::protocol::ServerMessage::JoinAck(ack)) {
            let _ = tx.try_send(bytes);
        }
    }
}

async fn run_ranked_queue(
    mut rx: mpsc::Receiver<RankedQueueMessage>,
    games: Arc<Mutex<Vec<ServerLobby>>>,
    next_id: Arc<Mutex<u64>>,
    db_url: String,
    db_secret: String,
) {
    let db_client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()
        .expect("ranked matchmaking HTTP client");
    let mut pending = Vec::<RankedQueueEntry>::new();
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            message = rx.recv() => match message {
                Some(message) => accept_ranked_queue_message(&games, &mut pending, message, &db_client, &db_url, &db_secret).await,
                None => return,
            },
            _ = interval.tick() => {
                while let Ok(message) = rx.try_recv() { accept_ranked_queue_message(&games, &mut pending, message, &db_client, &db_url, &db_secret).await; }
                let now = tokio::time::Instant::now();
                for entry in &mut pending {
                    let elapsed = now.saturating_duration_since(entry.player.search_started_at).as_secs().min(u64::from(u32::MAX)) as u32;
                    if elapsed != entry.last_reported_secs {
                        entry.last_reported_secs = elapsed;
                        let update = sow_core::protocol::ServerMessage::RankedSearching { elapsed_secs: elapsed };
                        if let Ok(bytes) = bincode::serialize(&update) { let _ = entry.player.client_tx.try_send(bytes); }
                    }
                }
                if let Some(pair) = take_ranked_pair(&mut pending, now) {
                    let Ok(id) = allocate_match_id(&next_id).await else {
                        for player in pair { send_ranked_failure(&player.client_tx, "RANKED_QUEUE_UNAVAILABLE"); }
                        continue;
                    };
                    start_ranked_lobby(&games, id, pair.into(), None, None, &db_url, &db_secret, &db_client).await;
                    continue;
                }
                let Some(expired_index) = pending.iter().enumerate()
                    .filter(|(_, entry)| now.saturating_duration_since(entry.player.search_started_at) > Duration::from_secs(3)
                        && !entry.player.client_tx.is_closed()
                        && entry.last_bot_attempt.is_none_or(|attempt| now.saturating_duration_since(attempt) >= Duration::from_secs(5)))
                    .min_by_key(|(_, entry)| entry.player.search_started_at).map(|(index, _)| index) else { continue; };
                pending[expired_index].last_bot_attempt = Some(now);
                let account_id = pending[expired_index].player.account_id.clone();
                let mut candidates = match ranked_candidates(&db_client, &db_url, &db_secret).await {
                    Ok(candidates) => candidates,
                    Err(error) => {
                        log::error!("[RANKED_BOTS] candidate lookup failed: {error}");
                        continue;
                    }
                };
                while let Ok(message) = rx.try_recv() { accept_ranked_queue_message(&games, &mut pending, message, &db_client, &db_url, &db_secret).await; }
                if let Some(pair) = take_ranked_pair(&mut pending, tokio::time::Instant::now()) {
                    let Ok(id) = allocate_match_id(&next_id).await else { continue; };
                    start_ranked_lobby(&games, id, pair.into(), None, None, &db_url, &db_secret, &db_client).await;
                    continue;
                }
                let Some(index) = pending.iter().position(|entry| entry.player.account_id == account_id) else { continue; };
                let Some(player) = pending.get(index).map(|entry| entry.player.clone()) else { continue; };
                let id = match allocate_match_id(&next_id).await {
                    Ok(id) => id,
                    Err(error) => { log::error!("[RANKED] {error}"); continue; }
                };
                let mut reserved: Option<(String, serde_json::Value)> = None;
                candidates.sort_by(|a, b| (a.1 - player.rating).abs().total_cmp(&(b.1 - player.rating).abs()));
                for (candidate, _) in &candidates {
                    let Ok((bot_id, _, _, _)) = ranked_bot_fields(candidate) else { continue; };
                    match reserve_ranked_bot(&db_client, &db_url, &db_secret, &bot_id, id).await {
                        Ok(Some(account)) => { reserved = Some((bot_id, account)); break; }
                        Ok(None) => continue,
                        Err(error) => { log::error!("[RANKED_BOTS] reserve failed: {error}"); break; }
                    }
                }
                if reserved.is_none() {
                    let iq = (60..=180).step_by(20).min_by_key(|iq| {
                        ((1000.0 + f64::from(*iq - 60) * 20.0) - player.rating).abs() as u32
                    }).unwrap_or(60);
                    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
                    let leader = sow_core::player::Leader::default().name();
                    let created = ranked_db_post(&db_client, &db_url, &db_secret, "internal/ranked-bot", serde_json::json!({
                        "action": "create", "external_id": format!("dynamic-{}-{stamp}", player.account_id),
                        "display_name": format!("Ranked {iq}"), "leader": leader, "iq": iq,
                        "actor": "matchmaking"
                    })).await;
                    if let Ok(payload) = created {
                        if let Some(account) = payload.get("accounts").and_then(serde_json::Value::as_array).and_then(|items| items.first()).cloned()
                            && let Ok((bot_id, _, _, _)) = ranked_bot_fields(&account)
                        {
                            if let Ok(Some(account)) = reserve_ranked_bot(&db_client, &db_url, &db_secret, &bot_id, id).await {
                                reserved = Some((bot_id, account));
                            }
                        }
                    }
                }
            let Some((bot_id, account)) = reserved else {
                    log::error!("[RANKED_BOTS] no bot could be reserved for player {}", player.account_id);
                    continue;
                };
                while let Ok(message) = rx.try_recv() { accept_ranked_queue_message(&games, &mut pending, message, &db_client, &db_url, &db_secret).await; }
                if let Some(pair) = take_ranked_pair(&mut pending, tokio::time::Instant::now()) {
                    release_ranked_bot(&db_client, &db_url, &db_secret, &bot_id, id).await;
                    start_ranked_lobby(&games, id, pair.into(), None, None, &db_url, &db_secret, &db_client).await;
                    continue;
                }
                let Some(index) = pending.iter().position(|entry| entry.player.account_id == account_id) else {
                    release_ranked_bot(&db_client, &db_url, &db_secret, &bot_id, id).await;
                    continue;
                };
                let player = pending.remove(index).player;
                if player.client_tx.is_closed() {
                    release_ranked_bot(&db_client, &db_url, &db_secret, &bot_id, id).await;
                    continue;
                }
                start_ranked_lobby(&games, id, vec![player], Some(account), Some(bot_id), &db_url, &db_secret, &db_client).await;
            }
        }
    }
}

// =============================================================================
// RELAY INTEGRATION — worker-per-queue DPDK relay (no per-lobby processes)
// =============================================================================
//
// Each worker process owns dynamic lobby ports selected by `port % 4` and one
// kernel management port. The server registers the lobby and ONLY broadcasts
// Start{relay_port, relay_host} after the owning worker answers 200 OK.
//
// Dynamic ports are bound by the long-lived worker process through the bridge
// command ring, so no relay subprocess is created for an individual match.

/// All server events, comming from client
enum ServerEvent {
    Join {
        client_tx: mpsc::Sender<Vec<u8>>,
        name: String,
        clan_tag: String,
        leader: sow_core::player::Leader,
        random_leader: bool,
        skin_style: u8,
        target_lobby_id: Option<u64>,
        host_private: bool,
        build_version: String,
        database_account_id: Option<String>,
        host_config: Option<Box<sow_core::game_config::GameConfig>>,
        password: Option<String>,
        ip: String,
        session_id: u64,
    },

    Leave {
        lobby_id: u64,
        player_id: u16,
        session_id: u64,
    },
    MapDownloadProgress {
        lobby_id: u64,
        player_id: u16,
        progress: u8,
    },
    LobbyReady {
        lobby_id: u64,
        player_id: u16,
    },
    ForceStart {
        lobby_id: u64,
        player_id: u16,
    },
    Kick {
        lobby_id: u64,
        requester_id: u16,
        target_id: u16,
        ban: bool,
    },
    SetTeam {
        lobby_id: u64,
        requester_id: u16,
        target_id: u16,
    },
}

/// Data collected inside the lock for a lobby that needs dynamic relay binding.
/// All blocking I/O (Redis, disk, process) happens *outside* the games lock in a dedicated worker task.
struct RelayCandidate {
    lobby_id: u64,
    relay_port: u16,
    ticket_expires_at: u64,
    worker_index: usize,
    tick_rate_ms: f32,
    config: GameConfig,
    kind: String,
    seed: u64,
    start_players: Vec<PlayerInfo>,
    relay_players_json: Vec<serde_json::Value>,
    player_tickets: std::collections::HashMap<u16, String>,
    player_ids: Vec<String>,
    players_tx: Vec<(u16, mpsc::Sender<Vec<u8>>)>,
}

// Content-derived build identity (see build.rs). Proves which sources the
// running jail executes; printed once at startup as [SERVER-BOOT].
include!(concat!(env!("OUT_DIR"), "/build_epoch.rs"));

#[tokio::main]
async fn main() {
    env_logger::init();
    log::info!("[SERVER-BOOT] build_epoch={BUILD_EPOCH:#018x}");

    if std::env::var("SOW_DB_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        log::error!("SOW_DB_SECRET must be set; refusing insecure default");
        return;
    }
    if let Err(e) = validate_runtime_security() {
        log::error!("{e}");
        return;
    }

    let db_url =
        std::env::var("SOW_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let db_secret = std::env::var("SOW_DB_SECRET").expect("SOW_DB_SECRET was validated above");
    let identity_state = identity::IdentityState::from_env(db_url.clone(), db_secret.clone());

    let redis_url =
        std::env::var("SOW_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    let redis_client = redis::Client::open(redis_url).expect("Failed to connect to Redis");
    // Establish one connection at boot as a hard dependency check.
    let redis_connection = redis_client
        .get_connection()
        .expect("Failed to get Redis connection");
    drop(redis_connection);
    log::info!("Redis connected");

    // Resolve (or lazily create) the persistent bot-account pool before any
    // lobby can be promoted. Idempotent — the seed endpoint get-or-creates
    // each account, so a restart reuses the same identities and their
    // accumulated stats.
    if let Err(error) = init_bot_pool().await {
        panic!("[BOT_POOL] startup failed: {error}");
    }
    tokio::spawn(async {
        if let Err(error) = init_ranked_bot_pool().await {
            log::error!("[RANKED_BOTS] roster seed unavailable; ranked will create bots on demand: {error}");
        }
    });

    let mut games: Vec<ServerLobby> = Vec::new();
    // Reward receipts and guide source IDs survive restarts; the match sequence must too.
    let mut next_lobby_id = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("match ID client")
        .get(format!(
            "{}/internal/match-id-seed",
            db_url.trim_end_matches('/')
        ))
        .bearer_auth(&db_secret)
        .send()
        .await
        .expect("match ID recovery request")
        .error_for_status()
        .expect("match ID recovery response")
        .json::<serde_json::Value>()
        .await
        .expect("match ID recovery payload")
        .get("next_match_id")
        .and_then(serde_json::Value::as_u64)
        .filter(|id| *id > 0)
        .expect("valid durable match ID seed");
    log::info!("[matches] recovered next match ID {next_lobby_id}");

    let maps_root = map_catalog::maps_root();
    map_catalog::init(&maps_root);

    let mut retired_matchmaking_ids = HashSet::new();
    master_tick(&mut games, &mut next_lobby_id, &mut retired_matchmaking_ids);

    let games_state = Arc::new(Mutex::new(games));
    let next_id_state = Arc::new(Mutex::new(next_lobby_id));
    let retired_matchmaking_ids_state = Arc::new(Mutex::new(retired_matchmaking_ids));

    let (global_tx, _rx) = broadcast::channel::<Vec<u8>>(100);
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(100000);
    let (ranked_queue_tx, ranked_queue_rx) = mpsc::channel::<RankedQueueMessage>(10000);
    let (relay_tx, mut relay_rx) = mpsc::unbounded_channel::<RelayCandidate>();
    let relay_workers = match relay_workers() {
        Ok(workers) => workers,
        Err(error) => {
            log::error!("Relay worker catalog is invalid: {error}");
            return;
        }
    };
    let relay_worker_count = relay_worker_count();
    if relay_workers.len() != relay_worker_count {
        log::error!(
            "Dynamic relay routing requires exactly {} workers; configured {}",
            relay_worker_count,
            relay_workers.len()
        );
        return;
    }
    let recovery_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .expect("ranked reservation recovery client");
    tokio::spawn(retry_ranked_bot_reservation_recovery(
        recovery_client,
        db_url.clone(),
        db_secret.clone(),
        relay_workers.clone(),
    ));
    tokio::spawn(run_ranked_queue(
        ranked_queue_rx,
        Arc::clone(&games_state),
        Arc::clone(&next_id_state),
        db_url.clone(),
        db_secret.clone(),
    ));
    let relay_ports = Arc::new(Mutex::new(RelayPortAllocator::new()));
    log::info!(
        "Relay worker catalog: {} worker(s) {:?}",
        relay_worker_count,
        relay_workers
    );

    let games_clone = Arc::clone(&games_state);
    let next_id_clone = Arc::clone(&next_id_state);
    let retired_matchmaking_ids_for_tick = Arc::clone(&retired_matchmaking_ids_state);
    let retired_matchmaking_ids_for_joins = Arc::clone(&retired_matchmaking_ids_state);
    let global_tx_clone = global_tx.clone();
    let relay_tx_clone = relay_tx.clone();
    let ranked_queue_tx_for_lobby = ranked_queue_tx.clone();

    // ── DEDICATED ASYNC RELAY WORKER TASK ──
    // A worker owns many lobbies. The destination port is the ownership key;
    // the client receives it only after the owning worker confirms that it
    // successfully bound the port.
    let relay_workers_for_task = relay_workers.clone();
    let relay_ports_for_task = relay_ports.clone();
    let relay_ports_for_tick = relay_ports.clone();
    tokio::spawn(async move {
        while let Some(rc) = relay_rx.recv().await {
            let workers = relay_workers_for_task.clone();
            let relay_ports = relay_ports_for_task.clone();
            tokio::spawn(async move {
                if let Err(e) = register_match_start(&rc).await {
                    log::error!(
                        "[DB] lobby {} will not start without match registration: {}",
                        rc.lobby_id,
                        e
                    );
                    relay_ports.lock().await.release(rc.relay_port);
                    let closed_msg = sow_core::protocol::ServerLobbyClosedMessage {
                        lobby_id: rc.lobby_id,
                        reason: format!("DB_REGISTRATION_FAILED: {e}"),
                    };
                    if let Ok(closed_json) = bincode::serialize(
                        &sow_core::protocol::ServerMessage::LobbyClosed(closed_msg),
                    ) {
                        for (_player_id, tx) in &rc.players_tx {
                            let _ = tx.try_send(closed_json.clone());
                        }
                    }
                    return;
                }

                let Some(worker) = workers.get(rc.worker_index) else {
                    log::error!(
                        "[RELAY] worker index {} missing for lobby {} port {}",
                        rc.worker_index,
                        rc.lobby_id,
                        rc.relay_port
                    );
                    relay_ports.lock().await.release(rc.relay_port);
                    return;
                };

                match register_relay(&rc, worker).await {
                    Ok(()) => {
                        log::info!(
                            "[RELAY] Lobby {} registered with worker {} on dynamic port {}",
                            rc.lobby_id,
                            worker.id,
                            rc.relay_port
                        );

                        // Broadcast Start message to each player with their specific my_player_id.
                        for (player_id, tx) in &rc.players_tx {
                            let start_msg = sow_core::protocol::ServerStartMessage {
                                config: rc.config.clone(),
                                my_player_id: Some(*player_id),
                                lobby_id: Some(rc.lobby_id),
                                seed: rc.seed,
                                players: rc.start_players.clone(),
                                missed_turns: vec![],
                                relay_port: Some(rc.relay_port),
                                relay_host: Some(worker.host.clone()),
                            };
                            let handoff_result = async {
                                let ticket = rc.player_tickets.get(player_id).ok_or_else(|| {
                                    format!(
                                        "missing relay ticket for network player {} in lobby {}",
                                        player_id, rc.lobby_id
                                    )
                                })?;
                                let ticket_json = bincode::serialize(
                                    &sow_core::protocol::ServerMessage::RelayTicket {
                                        lobby_id: rc.lobby_id,
                                        player_id: *player_id,
                                        ticket: ticket.clone(),
                                    },
                                )
                                .map_err(|e| format!("serialize relay ticket: {e}"))?;
                                send_relay_handoff_frame(
                                    tx,
                                    ticket_json,
                                    rc.lobby_id,
                                    *player_id,
                                    "RelayTicket",
                                )
                                .await?;

                                let start_json = bincode::serialize(
                                    &sow_core::protocol::ServerMessage::Start(Box::new(start_msg)),
                                )
                                .map_err(|e| format!("serialize Start: {e}"))?;
                                send_relay_handoff_frame(
                                    tx,
                                    start_json,
                                    rc.lobby_id,
                                    *player_id,
                                    "Start",
                                )
                                .await
                            }
                            .await;
                            if let Err(e) = handoff_result {
                                log::error!(
                                    "[RELAY] lobby {} player {} handoff failed; Start not sent: {}",
                                    rc.lobby_id,
                                    player_id,
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[RELAY] lobby {} port {} registration failed: {}",
                            rc.lobby_id,
                            rc.relay_port,
                            e
                        );
                        relay_ports.lock().await.release(rc.relay_port);

                        let closed_msg = sow_core::protocol::ServerLobbyClosedMessage {
                            lobby_id: rc.lobby_id,
                            reason: format!("RELAY_REGISTRATION_FAILED: {}", e),
                        };
                        if let Ok(closed_json) = bincode::serialize(
                            &sow_core::protocol::ServerMessage::LobbyClosed(closed_msg),
                        ) {
                            for (_player_id, tx) in &rc.players_tx {
                                let _ = tx.try_send(closed_json.clone());
                            }
                        }
                    }
                }
            });
        }
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let mut tick_count: u64 = 0;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick_start = tokio::time::Instant::now();
                    tick_count += 1;

                    // ── PHASE 1: In-memory work under the lock (microseconds) ──
                    let (ready_candidates, broadcast_json) = {
                        let mut games = games_clone.lock().await;
                        let mut nid = next_id_clone.lock().await;
                        let mut retired_matchmaking_ids =
                            retired_matchmaking_ids_for_tick.lock().await;
                        master_tick(&mut games, &mut nid, &mut retired_matchmaking_ids);

                        // Extract lobbies that completed Loading and are ready for relay
                        let mut ready_candidates = Vec::new();
                        let mut i = 0;
                        while i < games.len() {
                            if games[i].phase == lobby::LobbyPhase::ReadyForRelay {
                                let Some(relay_port) = relay_ports_for_tick.lock().await.allocate() else {
                                    log::error!("dynamic relay port pool exhausted; lobby {} remains pending", games[i].id);
                                    i += 1;
                                    continue;
                                };
                                let mut lobby = games.remove(i);
                                let mut relay_players_json = Vec::new();
                                let mut player_tickets = std::collections::HashMap::new();
                                let mut start_players = Vec::new();
                                let mut player_ids = Vec::new();
                                let mut players_tx = Vec::new();
                                for p in &lobby.players {
                                    let origin = if p.is_internal_bot {
                                        "internal_bot"
                                    } else {
                                        "external_network"
                                    };
                                    log::info!(
                                        "[MATCH_AUDIT_PLAYER] lobby={} player={} origin={} ip={} session_id={:?} relay_ticket_expected={}",
                                        lobby.id,
                                        p.player_id,
                                        origin,
                                        p.ip,
                                        p.session_id,
                                        !p.is_internal_bot
                                    );
                                    // Every network participant must be present in Start so each
                                    // lockstep client can register the same player ids. Internal
                                    // backfillers go as Human + is_ai_controlled — the core
                                    // auto-plays them on every client (zero sockets).
                                    start_players.push(PlayerInfo {
                                        id: p.player_id,
                                        name: p.name.clone(),
                                        player_type: sow_core::player::PlayerType::Human,
                                        color: p.leader.filler_rgb(),
                                        team: p.team,
                                        spawn_x: 0,
                                        spawn_y: 0,
                                        leader: p.leader,
                                        skin_style: p.skin_style,
                                        is_ai_controlled: p.is_internal_bot,
                                    });
                                    let relay_ticket_digest = if p.is_internal_bot {
                                        None
                                    } else {
                                        let (ticket, digest) = new_relay_ticket();
                                        player_tickets.insert(p.player_id, ticket);
                                        Some(digest)
                                    };
                                    relay_players_json.push(serde_json::json!({
                                        "player_id": p.player_id,
                                        "name": p.name,
                                        "database_account_id": p.database_account_id,
                                        "leader": p.leader,
                                        "team": p.team,
                                        "is_internal": p.is_internal_bot,
                                        "session_id": p.session_id,
                                        "relay_ticket_digest": relay_ticket_digest,
                                    }));
                                    if let Some(acc_id) = &p.database_account_id {
                                        player_ids.push(acc_id.clone());
                                    }
                                    // Internal bots have no socket — exclude from Start delivery.
                                    if !p.is_internal_bot {
                                        players_tx.push((p.player_id, p.tx.clone()));
                                    }
                                }
                                let roster_total = lobby.players.len();
                                let internal_bot_count = lobby
                                    .players
                                    .iter()
                                    .filter(|p| p.is_internal_bot)
                                    .count();
                                let validated_external_count = roster_total - internal_bot_count;
                                log::info!(
                                    "[MATCH_AUDIT] lobby={} roster_total={} internal_bot_count={} validated_external_count={} relay_ticket_count={}",
                                    lobby.id,
                                    roster_total,
                                    internal_bot_count,
                                    validated_external_count,
                                    player_tickets.len()
                                );
                                // Resolve the final HvN opponent after ghosts and humans
                                // are known. Every client receives this same config.
                                let human_side_players = roster_total as u32;
                                if crate::matchmaking_population::finalize_for_start(
                                    &mut lobby.config,
                                    human_side_players,
                                ) {
                                    log::info!(
                                        "[HVN] Lobby {} autobalanced: human_side_players={} vs nations={}",
                                        lobby.id, human_side_players, human_side_players
                                    );
                                }
                                ready_candidates.push(RelayCandidate {
                                    lobby_id: lobby.id,
                                    relay_port,
                                    ticket_expires_at: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs()
                                        .saturating_add(RELAY_TICKET_TTL_SECS),
                                    worker_index: relay_port as usize % relay_worker_count,
                                    tick_rate_ms: lobby.config.tick_rate_ms,
                                    config: lobby.config.clone(),
                                    kind: format!("{:?}", lobby.kind),
                                    seed: lobby.seed,
                                    start_players,
                                    relay_players_json,
                                    player_tickets,
                                    player_ids,
                                    players_tx,
                                });
                            } else {
                                i += 1;
                            }
                        }

                        // Build broadcast data (serialization is in-memory)
                        let broadcast_json = if tick_count.is_multiple_of(10) {
                            let lobbies_info = build_lobby_broadcast(&games);
                            let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                            match bincode::serialize(&sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg)) {
                                Ok(json) => Some(json),
                                Err(e) => { log::error!("[BROADCAST] Failed to serialize LobbiesBroadcast: {}", e); None }
                            }
                        } else {
                            None
                        };

                        (ready_candidates, broadcast_json)
                    }; // ── LOCK RELEASED ──

                    // Enqueue relay candidates to background worker task (zero I/O in tick)
                    for rc in ready_candidates {
                        let _ = relay_tx_clone.send(rc);
                    }

                    // ── PHASE 3: Broadcast + perf (no lock needed) ──
                    // Note: Initial LobbiesBroadcast is sent on connection & HTTP endpoint.
                    // Global tick broadcast is disabled at high scale to preserve WS throughput.
                    if let Some(json) = broadcast_json {
                        let _ = global_tx_clone.send(json);
                    }

                    // Precise latency performance metric logger
                    let elapsed = tick_start.elapsed().as_millis();
                    if elapsed > 10 {
                        log::warn!("[PERF] Event loop lag detected! Master tick execution took {}ms", elapsed);
                    }
                }
                Some(event) = event_rx.recv() => {
                    let mut games = games_clone.lock().await;
                    let mut nid = next_id_clone.lock().await;
                    match event {
                        ServerEvent::Join { client_tx, name, clan_tag, leader, random_leader, skin_style, target_lobby_id, host_private, build_version, database_account_id, host_config, password, ip, session_id } => {
                            log::info!("Player {} (clan: {}, ip: {}, session_id: {}) joining with version: {}", name, clan_tag, ip, session_id, build_version);
                            let target_is_retired_matchmaking = if let Some(id) = target_lobby_id {
                                retired_matchmaking_ids_for_joins.lock().await.contains(&id)
                            } else {
                                false
                            };
                            match join_player(&mut games, &mut nid, JoinPlayerOpts {
                                name,
                                clan_tag,
                                leader,
                                random_leader,
                                skin_style,
                                client_tx: client_tx.clone(),
                                target_lobby_id,
                                target_is_retired_matchmaking,
                                host_private,
                                database_account_id,
                                host_config,
                                password,
                                ip,
                                session_id: Some(session_id),
                                }) {
                                Ok((lobby_id, player_id, map_name, is_private)) => {
                                    if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                        let lobby_info = lobby_to_info(lobby);
                                        let ack = ServerJoinAckMessage {
                                            lobby_id,
                                            player_id,
                                            map_name,
                                            is_private,
                                            lobby_info,
                                        };
                                        match bincode::serialize(&sow_core::protocol::ServerMessage::JoinAck(ack)) {
                                            Ok(json) => { let _ = client_tx.try_send(json); }
                                            Err(e) => { log::error!("[JOIN] Failed to serialize JoinAck for player {} in lobby {}: {}", player_id, lobby_id, e); }
                                        }
                                        sync_host_lobby_to_members(lobby);
                                    } else {
                                        log::error!("[JOIN] Lobby {} disappeared before JoinAck", lobby_id);
                                    }
                                }
                                Err(reason) => {
                                    log::warn!("[JOIN] Join rejected: {}", reason);
                                    let fail = ServerJoinFailedMessage { reason };
                                    match bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)) {
                                        Ok(json) => { let _ = client_tx.try_send(json); }
                                        Err(e) => { log::error!("[JOIN] Failed to serialize JoinFailed: {}", e); }
                                    }
                                }
                            }
                        }

                        ServerEvent::Leave { lobby_id, player_id, session_id } => {
                            let ranked_member = games.iter().find(|lobby| lobby.id == lobby_id
                                && lobby.kind == sow_core::protocol::LobbyKind::Ranked1v1)
                                .and_then(|lobby| lobby.players.iter().find(|member| {
                                    member.player_id == player_id && member.session_id == Some(session_id)
                                }))
                                .and_then(|member| member.database_account_id.clone());
                            if let Some(account_id) = ranked_member {
                                if ranked_queue_tx_for_lobby.try_send(RankedQueueMessage::Cancel { account_id, session_id }).is_err() {
                                    log::error!("[RANKED] cancel/requeue request could not be queued for match {lobby_id}");
                                }
                            } else if games.iter().any(|lobby| lobby.id == lobby_id
                                && lobby.kind == sow_core::protocol::LobbyKind::Ranked1v1)
                            {
                                log::info!("[RANKED] ignored stale leave for match {lobby_id} session {session_id}");
                            } else if is_host_teardown(&games, lobby_id, player_id) {
                                if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                    notify_lobby_closed(lobby, "HOST_LEFT");
                                }
                                games.retain(|g| g.id != lobby_id);
                                log::info!("[LOBBY] Host {} left Custom lobby {} — lobby dropped, members returned to menu", player_id, lobby_id);
                            } else {
                                leave_player(&mut games, lobby_id, player_id);
                                if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                    sync_host_lobby_to_members(lobby);
                                }
                            }
                        }
                        ServerEvent::Kick { lobby_id, requester_id, target_id, ban } => {
                            kick_player(&mut games, lobby_id, requester_id, target_id, ban);
                        }
                        ServerEvent::SetTeam { lobby_id, requester_id, target_id } => {
                            set_player_team(&mut games, lobby_id, requester_id, target_id);
                        }
                        ServerEvent::MapDownloadProgress { lobby_id, player_id, progress } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                if let Some(p) = lobby.players.iter_mut().find(|p| p.player_id == player_id) {
                                    p.download_progress = progress;
                                } else {
                                    log::warn!("[PROGRESS] Player {} not found in lobby {} for download progress update", player_id, lobby_id);
                                }
                                sync_host_lobby_to_members(lobby);
                            } else {
                                log::warn!("[PROGRESS] Lobby {} not found for player {} progress update", lobby_id, player_id);
                            }
                        }
                        ServerEvent::LobbyReady { lobby_id, player_id } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                if lobby.players.iter().any(|p| p.player_id == player_id) {
                                    lobby.ready_players.insert(player_id);
                                    sync_host_lobby_to_members(lobby);
                                } else {
                                    log::warn!(
                                        "[READY] Player {} not found in lobby {}",
                                        player_id,
                                        lobby_id
                                    );
                                }
                            } else {
                                log::warn!(
                                    "[READY] Lobby {} not found for player {}",
                                    lobby_id,
                                    player_id
                                );
                            }
                        }
                        ServerEvent::ForceStart { lobby_id, player_id } => {
                            log::info!("[EVENT] ForceStart received lobby={} player={}", lobby_id, player_id);
                            force_start(&mut games, lobby_id, player_id);
                        }
                    }
                }
            }
        }
    });

    let addr = std::env::var("SOW_WS_LISTEN").unwrap_or_else(|_| "0.0.0.0:25565".to_string());

    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    let max_connections = match max_server_connections() {
        Ok(value) => value,
        Err(e) => {
            log::error!("{e}");
            return;
        }
    };
    let connection_slots = Arc::new(Semaphore::new(max_connections));
    log::info!(
        "SOW-SERVER listening on ws://{} max_connections={} handshake_timeout={}s",
        addr,
        max_connections,
        HANDSHAKE_TIMEOUT_SECS
    );

    // HTTP Static File Server for maps and Admin Dashboard
    let games_for_axum = Arc::clone(&games_state);
    let redis_client_for_axum = redis_client.clone();
    let identity_state_http = identity_state.clone();
    let db_url_http = db_url.clone();
    let db_secret_http = db_secret.clone();
    let replay_http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120))
        .build()
        .expect("failed to create replay verifier HTTP client");
    tokio::spawn(async move {
        let root = maps_root.clone();
        let state = AppState {
            games: games_for_axum,
            redis_client: redis_client_for_axum,
            identity: identity_state_http,
            maps_root: root.clone(),
            db_url: db_url_http,
            db_secret: db_secret_http,
            replay_verification: Arc::new(Semaphore::new(
                std::thread::available_parallelism()
                    .map_or(1, |n| n.get().saturating_sub(1).max(1)),
            )),
            verified_prefix: Arc::new(verification::Sessions::new()),
            replay_http,
        };
        let catalog_route = axum::Router::new()
            .route(
                "/internal/match/result",
                axum::routing::post(verification::handle_result)
                    .layer(axum::extract::DefaultBodyLimit::max(64 * 1024)),
            )
            .route(
                "/internal/match/progress",
                axum::routing::post(verification::handle_progress).layer(
                    axum::extract::DefaultBodyLimit::max(sow_data::MAX_MATCH_CHUNK_BYTES * 5),
                ),
            )
            .route(
                "/maps/catalog.bin",
                axum::routing::get(|| async {
                    axum::response::Response::builder()
                        .header("Content-Type", "application/octet-stream")
                        .header("Cache-Control", "public, max-age=60")
                        .body(axum::body::Body::from(
                            map_catalog::catalog_bytes().to_vec(),
                        ))
                        .unwrap()
                }),
            )
            .route(
                "/maps/catalog.json",
                axum::routing::get(catalog_json_handler),
            )
            .route("/lobbies.json", axum::routing::get(lobbies_json_handler))
            .route(
                "/auth/playgames/exchange",
                axum::routing::post(identity::handle_playgames_exchange),
            )
            .route(
                "/auth/playgames/consume",
                axum::routing::post(identity::handle_playgames_consume),
            )
            .route(
                "/auth/playgames/poll",
                axum::routing::get(identity::handle_playgames_poll),
            )
            .route("/profile", axum::routing::get(identity::handle_profile))
            .route("/admin/api/status", axum::routing::get(admin_status))
            .route("/admin/api/analytics", axum::routing::get(admin_analytics))
            .route("/admin/analytics", axum::routing::get(admin_analytics_page))
            .route("/admin/analytics.js", axum::routing::get(admin_analytics_script))
            .with_state(state);

        let app = catalog_route
            .nest_service(
                "/maps",
                tower_http::services::ServeDir::new(root).precompressed_br(),
            )
            .layer(tower_http::cors::CorsLayer::permissive());
        let http_addr =
            std::env::var("SOW_MAPS_HTTP_LISTEN").unwrap_or_else(|_| "127.0.0.1:25566".to_string());
        log::info!(
            "SOW-SERVER HTTP serving maps and admin on http://{}",
            http_addr
        );
        let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    while let Ok((stream, addr)) = listener.accept().await {
        let permit = match connection_slots.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                let rejected = REJECTED_CONNECTIONS.fetch_add(1, Ordering::Relaxed) + 1;
                if rejected.is_power_of_two() {
                    log::warn!(
                        "Connection cap rejected orchestrator peer={} count={rejected}",
                        addr
                    );
                }
                drop(stream);
                continue;
            }
        };
        let mut global_rx = global_tx.subscribe();
        let ev_tx = event_tx.clone();
        let games_state_conn = Arc::clone(&games_state);
        let identity_state_conn = identity_state.clone();
        let ranked_queue_tx_conn = ranked_queue_tx.clone();
        let db_url_conn = db_url.clone();
        let db_secret_conn = db_secret.clone();
        tokio::spawn(async move {
            let _connection_permit = permit;
            let session_id = next_session_id();
            let ip_cell = Arc::new(std::sync::Mutex::new(addr.ip().to_string()));
            let ip_cell_clone = Arc::clone(&ip_cell);
            let ws_stream = match tokio::time::timeout(
                Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
                tokio_tungstenite::accept_hdr_async(
                    stream,
                    ForwardedPeerCallback {
                        ip_cell: ip_cell_clone,
                    },
                ),
            )
            .await
            {
                Ok(Ok(ws)) => ws,
                Ok(Err(e)) => {
                    log::error!("Handshake failed: {}", e);
                    return;
                }
                Err(_) => {
                    log::warn!("Handshake timeout peer={} session_id={}", addr, session_id);
                    return;
                }
            };
            let ip_str = if let Ok(guard) = ip_cell.lock() {
                guard.clone()
            } else {
                addr.ip().to_string()
            };
            let (mut write, mut read) = ws_stream.split();
            log::info!(
                "Client connected from IP: {} session_id={}",
                ip_str,
                session_id
            );

            let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(4096);

            // Send immediate single-cast LobbiesBroadcast snapshot on connection so the home menu loads instantly
            {
                let games_guard = games_state_conn.lock().await;
                let lobbies_info = build_lobby_broadcast(&games_guard);
                let broadcast_msg = ServerLobbiesBroadcastMessage {
                    lobbies: lobbies_info,
                };
                if let Ok(json) = bincode::serialize(
                    &sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg),
                ) {
                    let _ = direct_tx.try_send(json);
                }
            }

            let mut my_lobby_id: Option<u64> = None;
            let mut my_player_id: Option<u16> = None;
            let mut ranked_account_id: Option<String> = None;

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(msg)) => {
                                if msg.is_binary() {
                                    let data = msg.into_data();

                                    if let Ok(msg) = bincode::deserialize::<sow_core::protocol::ClientMessage>(&data) {
                                        match msg {
                                            sow_core::protocol::ClientMessage::JoinWithAuth { join, auth } => {
                                                let payload = *join;
                                                let server_version = std::env::var("SOW_BUILD_VERSION")
                                                    .unwrap_or_else(|_| std::fs::read_to_string(".version").unwrap_or_default().trim().to_string());

                                                let server_version = sow_core::protocol::gameplay_build_identity(&server_version);
                                                if payload.build_version != server_version {
                                                    log::warn!("Client version mismatch: expected {}, got {}", server_version, payload.build_version);
                                                    let fail = sow_core::protocol::ServerJoinFailedMessage { reason: "VERSION_MISMATCH".to_string() };
                                                    let json = bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)).unwrap();
                                                    let _ = direct_tx.try_send(json);
                                                    continue;
                                                }

                                                let (database_account_id, leader, skin_style) = match verify_identity(&identity_state_conn, &auth, payload.leader, payload.random_leader).await {
                                                    Ok(identity) => {
                                                        log::info!(
                                                            "[AUTH] join verified provider={} account={}",
                                                            auth.provider,
                                                            identity.account_id
                                                        );
                                                        (Some(identity.account_id), identity.leader, identity.skin_style)
                                                    }
                                                    Err(e) => {
                                                        log::warn!(
                                                            "[AUTH] join verification failed provider={}: {}",
                                                            auth.provider,
                                                            e
                                                        );
                                                        let fail = sow_core::protocol::ServerJoinFailedMessage {
                                                            reason: "IDENTITY_OR_LEADER_UNAVAILABLE".to_string(),
                                                        };
                                                        if let Ok(json) = bincode::serialize(
                                                            &sow_core::protocol::ServerMessage::JoinFailed(fail),
                                                        ) {
                                                            let _ = direct_tx.try_send(json);
                                                        }
                                                        continue;
                                                    }
                                                };

                                                if payload.ranked_1v1 {
                                                    if payload.target_lobby_id.is_some() || payload.host_private || payload.host_config.is_some() {
                                                        send_ranked_failure(&direct_tx, "INVALID_RANKED_REQUEST");
                                                        continue;
                                                    }
                                                    let Some(account_id) = database_account_id else {
                                                        send_ranked_failure(&direct_tx, "RANKED_ACCOUNT_REQUIRED");
                                                        continue;
                                                    };
                                                    let db_client = reqwest::Client::new();
                                                    let rating = match fetch_ranked_rating(&db_client, &db_url_conn, &db_secret_conn, &account_id).await {
                                                        Ok(rating) => rating,
                                                        Err(error) => {
                                                            log::error!("[RANKED] rating lookup failed for {account_id}: {error}");
                                                            send_ranked_failure(&direct_tx, "RANKED_QUEUE_UNAVAILABLE");
                                                            continue;
                                                        }
                                                    };
                                                    ranked_account_id = Some(account_id.clone());
                                                    let search_started_at = tokio::time::Instant::now();
                                                    let player = RankedQueuePlayer {
                                                        account_id,
                                                        rating,
                                                        name: payload.name,
                                                        clan_tag: payload.clan_tag,
                                                        leader,
                                                        skin_style,
                                                        client_tx: direct_tx.clone(),
                                                        ip: ip_str.clone(),
                                                        session_id,
                                                        search_started_at,
                                                    };
                                                    if ranked_queue_tx_conn.send(RankedQueueMessage::Search(player)).await.is_err() {
                                                        send_ranked_failure(&direct_tx, "RANKED_QUEUE_UNAVAILABLE");
                                                    }
                                                    continue;
                                                }

                                                let _ = ev_tx.send(ServerEvent::Join {
                                                    name: payload.name,
                                                    clan_tag: payload.clan_tag,
                                                    leader,
                                                    random_leader: payload.random_leader,
                                                    skin_style,
                                                    client_tx: direct_tx.clone(),
                                                    target_lobby_id: payload.target_lobby_id,
                                                    host_private: payload.host_private,
                                                    build_version: payload.build_version,
                                                    database_account_id,
                                                    host_config: payload.host_config,
                                                    password: payload.password,
                                                    ip: ip_str.clone(),
                                                    session_id,
                                                }).await;
                                            }
                                            sow_core::protocol::ClientMessage::Gameplay { .. } => {
                                                // Orchestrator ignores gameplay intents
                                            }
                                            sow_core::protocol::ClientMessage::Leave {} => {
                                                if let Some(account_id) = ranked_account_id.take() {
                                                    let _ = ranked_queue_tx_conn.send(RankedQueueMessage::Cancel { account_id, session_id }).await;
                                                }
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    let _ = ev_tx.send(ServerEvent::Leave {
                                                        lobby_id: l_id,
                                                        player_id: p_id,
                                                        session_id,
                                                    }).await;
                                                }
                                                my_lobby_id = None;
                                                my_player_id = None;
                                            }
                                            sow_core::protocol::ClientMessage::CancelRankedSearch => {
                                                if let Some(account_id) = ranked_account_id.take() {
                                                    let _ = ranked_queue_tx_conn.send(RankedQueueMessage::Cancel { account_id, session_id }).await;
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::ReadyWithTicket { .. } => {
                                                // Relay tickets are valid only on the direct relay
                                                // connection, never on the orchestrator socket.
                                            }
                                            sow_core::protocol::ClientMessage::ReconnectWithTicket { .. } => {
                                                // Reconnect capabilities are valid only on the direct
                                                // relay connection, never on the orchestrator socket.
                                            }
                                            sow_core::protocol::ClientMessage::MapDownloadProgress { lobby_id, player_id, progress } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id)
                                                    && lobby_id == l_id
                                                    && player_id == p_id
                                                {
                                                    let _ = ev_tx.send(ServerEvent::MapDownloadProgress {
                                                        lobby_id: l_id,
                                                        player_id: p_id,
                                                        progress,
                                                    }).await;
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::LobbyReady { lobby_id, player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id)
                                                    && lobby_id == l_id
                                                    && player_id == p_id
                                                {
                                                    let _ = ev_tx.send(ServerEvent::LobbyReady {
                                                        lobby_id: l_id,
                                                        player_id: p_id,
                                                    }).await;
                                                }
                                            }

                                            sow_core::protocol::ClientMessage::ForceStart { lobby_id, player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id)
                                                    && lobby_id == l_id
                                                    && player_id == p_id
                                                {
                                                    let _ = ev_tx.send(ServerEvent::ForceStart {
                                                        lobby_id: l_id,
                                                        player_id: p_id,
                                                    }).await;
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Kick { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id)
                                                    && lobby_id == l_id
                                                {
                                                    let _ = ev_tx.send(ServerEvent::Kick {
                                                        lobby_id: l_id,
                                                        requester_id: p_id,
                                                        target_id: target_player_id,
                                                        ban: false,
                                                    }).await;
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Ban { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id)
                                                    && lobby_id == l_id
                                                {
                                                    let _ = ev_tx.send(ServerEvent::Kick {
                                                        lobby_id: l_id,
                                                        requester_id: p_id,
                                                        target_id: target_player_id,
                                                        ban: true,
                                                    }).await;
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::SetPlayerTeam { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id)
                                                    && lobby_id == l_id
                                                {
                                                    let _ = ev_tx.send(ServerEvent::SetTeam {
                                                        lobby_id: l_id,
                                                        requester_id: p_id,
                                                        target_id: target_player_id,
                                                    }).await;
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Ping { client_time } => {
                                                let pong = sow_core::protocol::ServerMessage::Pong { client_time };
                                                let json = bincode::serialize(&pong).unwrap();
                                                let _ = direct_tx.try_send(json);
                                            }
                                            sow_core::protocol::ClientMessage::PrepareResult { .. } => {},
                                            sow_core::protocol::ClientMessage::ReservedClientMessage => {
                                                log::warn!("[SERVER] Client used a reserved protocol message slot; closing connection");
                                                break;
                                            }
                                        }
                                        continue;
                                    }

                                    log::warn!("[SERVER] Unrecognized message");
                                }
                            }
                            _ => break,
                        }
                    }
                    Ok(broadcast_data) = global_rx.recv() => {
                        // Drop lobbies broadcast packets if the client is already in a lobby/match
                        if my_lobby_id.is_none()
                            && write.send(Message::Binary(broadcast_data)).await.is_err()
                        {
                            break;
                        }
                    }
                    Some(direct_data) = direct_rx.recv() => {
                        if let Ok(server_msg) = bincode::deserialize::<sow_core::protocol::ServerMessage>(&direct_data) {
                            match server_msg {
                                sow_core::protocol::ServerMessage::JoinAck(ack) => {
                                    my_lobby_id = Some(ack.lobby_id);
                                    my_player_id = Some(ack.player_id);
                                }
                                sow_core::protocol::ServerMessage::Start(start) => {
                                    if let Some(pid) = start.my_player_id {
                                        my_player_id = Some(pid);
                                    }
                                }
                                sow_core::protocol::ServerMessage::LobbyClosed(closed) => {
                                    if my_lobby_id == Some(closed.lobby_id) {
                                        my_lobby_id = None;
                                        my_player_id = None;
                                    }
                                }
                                _ => {}
                            }
                        }

                        if write.send(Message::Binary(direct_data)).await.is_err() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {
                        break;
                    }
                }
            }

            if let Some(account_id) = ranked_account_id.take() {
                let _ = ranked_queue_tx_conn.send(RankedQueueMessage::Cancel { account_id, session_id }).await;
            }
            if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                let _ = ev_tx
                    .send(ServerEvent::Leave {
                        lobby_id: l_id,
                        player_id: p_id,
                        session_id,
                    })
                    .await;
            }
        });
    }
}

#[derive(Clone)]
struct AppState {
    games: Arc<Mutex<Vec<lobby::ServerLobby>>>,
    redis_client: redis::Client,
    identity: identity::IdentityState,
    maps_root: PathBuf,
    db_url: String,
    db_secret: String,
    replay_verification: Arc<Semaphore>,
    verified_prefix: Arc<verification::Sessions>,
    replay_http: reqwest::Client,
}

#[derive(serde::Deserialize)]
struct ReplayStartPlayer {
    player_id: u16,
    #[serde(default)]
    database_account_id: Option<String>,
    #[serde(default)]
    leader: sow_core::player::Leader,
    #[serde(default)]
    team: Option<sow_core::protocol::Team>,
    #[serde(default)]
    is_internal: bool,
}

#[derive(Debug)]
enum ReplayVerificationError {
    Invalid(String),
    HistoryRequired {
        expected_sequence: u64,
        verified_turns: u64,
    },
    Unavailable(String),
    /// The match map is not on disk. This is retryable: a later release may
    /// ship the map (campaign maps once missed a release this way). It logs
    /// at warn, never error, so a stuck match cannot fill the error log.
    MissingMap(String),
}

impl std::fmt::Display for ReplayVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) | Self::Unavailable(reason) | Self::MissingMap(reason) => {
                f.write_str(reason)
            }
            Self::HistoryRequired {
                expected_sequence,
                verified_turns,
            } => write!(
                f,
                "verified history required: sequence={expected_sequence} turns={verified_turns}"
            ),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ProgressCursor {
    sequence: u64,
    sha256: String,
    next_turn: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ActiveProgress {
    sequence: u64,
    first_turn: u64,
    end_turn: u64,
    previous_sha256: String,
    sha256: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct VerifiedPrefix {
    identity: [u8; 32],
    turns: usize,
    engine: sow_core::engine::SowEngine,
    defeats: std::collections::HashMap<u16, [u32; 3]>,
    total_intents: usize,
    #[serde(default)]
    outcomes: std::collections::BTreeMap<String, sow_data::profile::VerifiedParticipantClosure>,
    terminal_turn: Option<u64>,
    progress_cursor: Option<ProgressCursor>,
    active_progress: Option<ActiveProgress>,
    last_progress_reply: Option<sow_data::profile::VerifiedProgress>,
}

fn participant_closure_reason(
    resigned: bool,
    dead: bool,
    team_still_playing: bool,
    game_over: bool,
) -> Option<&'static str> {
    if resigned {
        Some("voluntary")
    } else if dead && (!team_still_playing || game_over) {
        Some("defeated")
    } else {
        None
    }
}

#[cfg(test)]
thread_local! {
    static VERIFIED_TURN_EXECUTIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn advance_replay(
    match_id: &str,
    turns: &[sow_core::protocol::Turn],
    start: &sow_data::profile::MatchStartRecord,
    maps_root: &Path,
    progress: &mut Option<VerifiedPrefix>,
    budget: Option<std::time::Duration>,
) -> Result<(sow_data::profile::VerifiedMatchResult, bool), ReplayVerificationError> {
    let metadata = &start.metadata;
    if metadata
        .get("simulation_rules_id")
        .and_then(serde_json::Value::as_str)
        != Some(sow_core::SIMULATION_RULES_ID)
    {
        return Err(ReplayVerificationError::Unavailable(
            "match requires its originally registered simulation worker".into(),
        ));
    }
    let seed = metadata
        .get("seed")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| ReplayVerificationError::Unavailable("trusted seed missing".to_string()))?;
    let config: GameConfig =
        serde_json::from_value(metadata.get("config").cloned().ok_or_else(|| {
            ReplayVerificationError::Unavailable("trusted config missing".to_string())
        })?)
        .map_err(|error| {
            ReplayVerificationError::Unavailable(format!("trusted config invalid: {error}"))
        })?;
    if config.map_width == 0
        || config.map_height == 0
        || config.map_width > sow_core::maps::MAX_MAP_AXIS
        || config.map_height > sow_core::maps::MAX_MAP_AXIS
        || u64::from(config.map_width) * u64::from(config.map_height)
            > u64::from(sow_core::maps::MAX_MAP_PIXELS)
        || !config.tick_rate_ms.is_finite()
        || config.tick_rate_ms <= 0.0
    {
        return Err(ReplayVerificationError::Unavailable(
            "trusted game settings are invalid".to_string(),
        ));
    }
    let players: Vec<PlayerInfo> =
        serde_json::from_value(metadata.get("players").cloned().ok_or_else(|| {
            ReplayVerificationError::Unavailable("trusted players missing".to_string())
        })?)
        .map_err(|error| {
            ReplayVerificationError::Unavailable(format!("trusted players invalid: {error}"))
        })?;
    let relay_players: Vec<ReplayStartPlayer> =
        serde_json::from_value(metadata.get("relay_players").cloned().ok_or_else(|| {
            ReplayVerificationError::Unavailable("trusted roster missing".to_string())
        })?)
        .map_err(|error| {
            ReplayVerificationError::Unavailable(format!("trusted roster invalid: {error}"))
        })?;
    if relay_players.is_empty()
        || relay_players.len() > sow_core::protocol::MAX_MATCH_PARTICIPANTS
        || players.is_empty()
        || players.len() > sow_core::protocol::MAX_MATCH_PARTICIPANTS
    {
        return Err(ReplayVerificationError::Unavailable(
            "trusted roster is empty".to_string(),
        ));
    }
    let registered_accounts = relay_players
        .iter()
        .filter_map(|player| player.database_account_id.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_accounts = start
        .player_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if registered_accounts != expected_accounts {
        return Err(ReplayVerificationError::Unavailable(
            "trusted account roster mismatch".to_string(),
        ));
    }
    let mut player_ids = std::collections::BTreeSet::new();
    let mut external_player_ids = std::collections::BTreeSet::new();
    for player in &relay_players {
        if !player_ids.insert(player.player_id) {
            return Err(ReplayVerificationError::Unavailable(
                "duplicate trusted player id".to_string(),
            ));
        }
        if !player.is_internal {
            external_player_ids.insert(player.player_id);
        }
    }
    let engine_player_ids = players
        .iter()
        .map(|player| player.id)
        .collect::<std::collections::BTreeSet<_>>();
    if engine_player_ids.len() != players.len() || engine_player_ids != player_ids {
        return Err(ReplayVerificationError::Unavailable(
            "trusted engine roster mismatch".to_string(),
        ));
    }
    let started = std::time::Instant::now();
    if turns.len() as u64 > sow_data::max_match_turns(config.tick_rate_ms)
        || turns
            .last()
            .is_some_and(|turn| turn.turn_number >= sow_data::max_match_turns(config.tick_rate_ms))
    {
        return Err(ReplayVerificationError::Invalid(
            "match duration exceeds registered clock limit".into(),
        ));
    }
    let trusted_identity = serde_json::to_vec(&(
        match_id,
        &start.player_ids,
        &start.metadata,
        sow_core::SIMULATION_RULES_ID,
    ))
    .map_err(|e| ReplayVerificationError::Unavailable(e.to_string()))?;
    let identity: [u8; 32] = Sha256::digest(trusted_identity).into();
    let first_turn = turns
        .first()
        .map_or(progress.as_ref().map_or(0, |p| p.turns as u64), |t| {
            t.turn_number
        }) as usize;
    if turns
        .iter()
        .enumerate()
        .any(|(offset, turn)| turn.turn_number != (first_turn + offset) as u64)
    {
        return Err(ReplayVerificationError::Invalid(
            "replay turn sequence is broken".into(),
        ));
    }
    if progress.as_ref().is_some_and(|p| p.identity != identity) {
        return Err(ReplayVerificationError::Unavailable(
            "registered simulation identity changed".into(),
        ));
    }
    let reused_turns = progress.as_ref().map_or(0, |p| p.turns);
    if reused_turns < first_turn {
        return Err(ReplayVerificationError::Unavailable(
            "verified history required".into(),
        ));
    }
    let mut submitted_intents = progress.as_ref().map_or(0, |p| p.total_intents);
    for turn in turns
        .iter()
        .filter(|turn| turn.turn_number as usize >= reused_turns)
    {
        if turn.intents.len() > MAX_REPLAY_INTENTS_PER_TURN {
            return Err(ReplayVerificationError::Invalid(
                "too many intents in one turn".into(),
            ));
        }
        submitted_intents = submitted_intents.saturating_add(turn.intents.len());
        if submitted_intents > MAX_REPLAY_INTENTS {
            return Err(ReplayVerificationError::Invalid(
                "replay has too many intents".into(),
            ));
        }
        if turn
            .intents
            .iter()
            .any(|intent| !external_player_ids.contains(&intent.player_id))
        {
            return Err(ReplayVerificationError::Invalid(
                "replay contains an unregistered sender".into(),
            ));
        }
    }
    let mut prior = progress.take();
    if prior.is_none() {
        let map_bytes = verification::match_map_bytes(&config, start, maps_root)?;
        let map_file = sow_core::maps::load_map_from_payload(&map_bytes).map_err(|error| {
            ReplayVerificationError::MissingMap(format!("map parse failed: {error}"))
        })?;
        if map_file.width != config.map_width || map_file.height != config.map_height {
            return Err(ReplayVerificationError::MissingMap(
                "map dimensions do not match match config".to_string(),
            ));
        }

        prior = Some(VerifiedPrefix {
            identity,
            turns: 0,
            engine: sow_core::engine::initialize_match_engine(
                config.clone(),
                seed,
                &map_bytes,
                players,
                map_file.spawns.clone(),
                map_file.geo_bounds.clone(),
                map_file.rosters.clone(),
                map_file.num_land_tiles,
            ),
            defeats: std::collections::HashMap::new(),
            total_intents: 0,
            outcomes: std::collections::BTreeMap::new(),
            terminal_turn: None,
            progress_cursor: None,
            active_progress: None,
            last_progress_reply: None,
        });
    }
    let mut saved = prior.unwrap();
    let reused_turns = saved.turns;
    let engine = &mut saved.engine;
    let defeats = &mut saved.defeats;
    let mut total_intents = saved.total_intents;
    for turn in turns
        .iter()
        .filter(|turn| turn.turn_number as usize >= reused_turns)
    {
        let index = saved.turns;
        if index > reused_turns && budget.is_some_and(|limit| started.elapsed() >= limit) {
            break;
        }
        if turn.turn_number != index as u64 {
            return Err(ReplayVerificationError::Invalid(
                "replay turn sequence is broken".to_string(),
            ));
        }
        total_intents = total_intents.saturating_add(turn.intents.len());
        let game_over_before_turn = engine.state.phase == sow_core::game::GamePhase::GameOver;
        if engine.state.phase == sow_core::game::GamePhase::GameOver {
            saved.turns = index + 1;
            continue;
        }
        #[cfg(test)]
        VERIFIED_TURN_EXECUTIONS.with(|count| count.set(count.get() + 1));
        replay::apply_turn(engine, turn);
        for event in &engine.state.events {
            let sow_core::game::GameEvent::PlayerEliminated {
                player_id,
                conqueror_id,
                ..
            } = event
            else {
                continue;
            };
            if !external_player_ids.contains(conqueror_id) {
                continue;
            }
            if let Some(victim) = engine
                .state
                .players
                .iter()
                .find(|player| player.id == *player_id)
            {
                let counts = defeats.entry(*conqueror_id).or_default();
                match victim.player_type {
                    sow_core::player::PlayerType::Human => counts[0] = counts[0].saturating_add(1),
                    sow_core::player::PlayerType::Nation => counts[1] = counts[1].saturating_add(1),
                    sow_core::player::PlayerType::Bot => counts[2] = counts[2].saturating_add(1),
                }
            }
        }
        saved.turns = index + 1;
        let game_over = engine.state.phase == sow_core::game::GamePhase::GameOver;
        if game_over && saved.terminal_turn.is_none() {
            saved.terminal_turn = Some(turn.turn_number);
        }
        for human in relay_players.iter().filter(|p| !p.is_internal) {
            let Some(account) = &human.database_account_id else {
                continue;
            };
            if saved.outcomes.contains_key(account) {
                continue;
            }
            let resigned = turn.intents.iter().any(|i| {
                i.player_id == human.player_id
                    && matches!(i.intent, sow_core::protocol::GameplayIntent::Resign)
            });
            let dead = engine
                .state
                .players
                .iter()
                .any(|p| p.id == human.player_id && !p.alive && p.has_spawned);
            // An eliminated team member can still win with living teammates.
            let team_still_playing = human.team.is_some_and(|team| {
                engine
                    .state
                    .players
                    .iter()
                    .any(|p| p.team == Some(team) && p.alive)
            });
            if let Some(reason) =
                participant_closure_reason(resigned, dead, team_still_playing, game_over)
            {
                let result = replay_result(
                    match_id,
                    engine,
                    defeats,
                    &relay_players,
                    &config,
                    Some((turn.turn_number, human.player_id)),
                    game_over_before_turn || (game_over && !resigned),
                )?;
                saved.outcomes.insert(
                    account.clone(),
                    sow_data::profile::VerifiedParticipantClosure {
                        account_id: account.clone(),
                        reason: reason.into(),
                        result,
                    },
                );
            }
        }
    }

    let complete = turns
        .last()
        .is_none_or(|turn| saved.turns > turn.turn_number as usize);
    let result = replay_result(
        match_id,
        engine,
        defeats,
        &relay_players,
        &config,
        None,
        true,
    )?;
    saved.total_intents = total_intents;
    log::info!(
        "[exit-timing] verified match={match_id} turns={} reused_turns={reused_turns} calculate_ms={}",
        saved.turns,
        started.elapsed().as_millis()
    );
    *progress = Some(saved);
    Ok((result, complete))
}

fn replay_result(
    match_id: &str,
    engine: &sow_core::engine::SowEngine,
    defeats: &std::collections::HashMap<u16, [u32; 3]>,
    relay_players: &[ReplayStartPlayer],
    config: &GameConfig,
    closure: Option<(u64, u16)>,
    allow_winner: bool,
) -> Result<sow_data::profile::VerifiedMatchResult, ReplayVerificationError> {
    let (winner_player_id, winning_team) = verified_winner_identity(
        engine.state.winner,
        engine.state.winning_team,
        relay_players,
        config.ranked_1v1,
        allow_winner,
    );
    let mut participants = Vec::with_capacity(if closure.is_some() {
        1
    } else {
        relay_players.len()
    });
    for relay_player in relay_players {
        if closure.is_some_and(|(_, player_id)| player_id != relay_player.player_id) {
            continue;
        }
        let Some(account_id) = relay_player.database_account_id.clone() else {
            continue;
        };
        let player = engine
            .state
            .players
            .iter()
            .find(|player| player.id == relay_player.player_id)
            .ok_or_else(|| {
                ReplayVerificationError::Unavailable(
                    "verified player missing from engine".to_string(),
                )
            })?;
        let counts = defeats
            .get(&relay_player.player_id)
            .copied()
            .unwrap_or_default();
        let won = match winning_team.as_deref() {
            Some(team) => relay_player
                .team
                .is_some_and(|value| format!("{value:?}") == team),
            None => winner_player_id == Some(relay_player.player_id),
        };
        participants.push(sow_data::profile::VerifiedMatchParticipant {
            player_id: relay_player.player_id,
            account_id,
            leader: Some(relay_player.leader.name().to_string()),
            team: relay_player.team.map(|team| format!("{team:?}")),
            won,
            kills: player.kills,
            deaths: player.deaths,
            assists: player.assists,
            players_defeated: counts[0],
            empires_defeated: counts[1],
            tribes_defeated: counts[2],
        });
    }
    participants.sort_by_key(|participant| participant.player_id);
    let duration_seconds = ((engine.state.tick as f64 * f64::from(config.tick_rate_ms)) / 1000.0)
        .round()
        .clamp(0.0, f64::from(u32::MAX)) as u32;
    Ok(sow_data::profile::VerifiedMatchResult {
        match_id: match_id.to_string(),
        duration_seconds,
        winner_player_id,
        winning_team,
        personal_closure_turn: closure.map(|(turn, _)| turn),
        participants,
    })
}

fn verified_winner_identity(
    winner: Option<u16>,
    winning_team: Option<sow_core::protocol::Team>,
    relay_players: &[ReplayStartPlayer],
    ranked_1v1: bool,
    allow_winner: bool,
) -> (Option<u16>, Option<String>) {
    if !allow_winner {
        return (None, None);
    }
    if let Some(team) = winning_team {
        let representative = relay_players
            .iter()
            .find(|player| {
                !player.is_internal
                    && player.database_account_id.is_some()
                    && player.team == Some(team)
            })
            .map(|player| player.player_id);
        return representative.map_or((None, None), |id| (Some(id), Some(format!("{team:?}"))));
    }
    let winner = winner.filter(|winner| {
        relay_players.iter().any(|player| {
            player.player_id == *winner
                && player.database_account_id.is_some()
                && (!player.is_internal || ranked_1v1)
        })
    });
    (winner, None)
}

async fn admin_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    // The status payload discloses player IPs and account ids — it is
    // localhost tooling only and requires the deployment bearer secret.
    let secret = std::env::var("SOW_DB_SECRET").unwrap_or_default();
    if !admin_request_authorized(&headers, &secret) {
        log::warn!("[ADMIN] unauthorized /admin/api/status request");
        return axum::response::IntoResponse::into_response((
            axum::http::StatusCode::UNAUTHORIZED,
            axum::response::Json(serde_json::json!({"error": "Unauthorized"})),
        ));
    }

    let games = state.games.lock().await;
    let mut lobbies = Vec::new();
    for lobby in &*games {
        let mut players = Vec::new();
        for p in &lobby.players {
            players.push(serde_json::json!({
                "player_id": p.player_id,
                "name": p.name,
                "clan_tag": p.clan_tag,
                "civilization": p.leader.civilization(),
                "leader": format!("{:?}", p.leader),
                "download_progress": p.download_progress,
                "ip": p.ip,
                "database_account_id": p.database_account_id,
            }));
        }
        lobbies.push(serde_json::json!({
            "id": lobby.id,
            "kind": format!("{:?}", lobby.kind),
            "is_private": lobby.is_private,
            "phase": format!("{:?}", lobby.phase),
            "countdown_secs": lobby.countdown_secs,
            "relay_port": lobby.relay_port,
            "players": players,
            "map_name": lobby.config.map_name,
            "game_mode": lobby.game_mode,
        }));
    }
    drop(games);

    // Query valkey INFO
    let valkey_info = tokio::task::spawn_blocking({
        let client = state.redis_client.clone();
        move || -> serde_json::Value {
            let mut conn = match client.get_connection() {
                Ok(c) => c,
                Err(_) => return serde_json::json!({"error": "cannot connect"}),
            };
            match redis::cmd("INFO").query::<String>(&mut conn) {
                Ok(info) => {
                    let mut result = serde_json::Map::new();
                    for line in info.lines() {
                        if line.contains(':') && !line.starts_with('#') {
                            let parts: Vec<&str> = line.splitn(2, ':').collect();
                            let key = parts[0].trim();
                            let val = parts[1].trim();
                            if [
                                "used_memory_human",
                                "used_memory_peak_human",
                                "connected_clients",
                                "blocked_clients",
                                "keyspace_hits",
                                "keyspace_misses",
                                "uptime_in_seconds",
                                "instantaneous_ops_per_sec",
                                "instantaneous_input_kbps",
                                "instantaneous_output_kbps",
                                "total_connections_received",
                                "total_commands_processed",
                                "expired_keys",
                                "evicted_keys",
                            ]
                            .contains(&key)
                            {
                                result.insert(
                                    key.to_string(),
                                    serde_json::Value::String(val.to_string()),
                                );
                            }
                        }
                    }
                    serde_json::Value::Object(result)
                }
                Err(_) => serde_json::json!({"error": "info failed"}),
            }
        }
    })
    .await
    .unwrap_or(serde_json::json!({"error": "task failed"}));

    // Query sow-database stats
    let db_base =
        std::env::var("SOW_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let db_stats_url = format!("{}/internal/stats", db_base.trim_end_matches('/'));
    let db_stats = match reqwest::Client::new()
        .get(&db_stats_url)
        .header("Authorization", format!("Bearer {secret}"))
        .send()
        .await
    {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .unwrap_or(serde_json::json!({"error": "db unreachable"})),
        Err(_) => serde_json::json!({"error": "db unreachable"}),
    };

    axum::response::IntoResponse::into_response(axum::response::Json(serde_json::json!({
        "lobbies": lobbies,
        "valkey": valkey_info,
        "database": db_stats,
    })))
}

fn admin_request_authorized(headers: &axum::http::HeaderMap, secret: &str) -> bool {
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    !secret.is_empty() && presented == Some(secret)
}

async fn admin_analytics_page() -> axum::response::Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(axum::http::header::CACHE_CONTROL, "no-store")
        .body(axum::body::Body::from(include_str!(
            "../assets/admin_analytics.html"
        )))
        .expect("valid analytics dashboard response")
}

async fn admin_analytics_script() -> axum::response::Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )
        .header(axum::http::header::CACHE_CONTROL, "no-store")
        .body(axum::body::Body::from(include_str!(
            "../assets/admin_analytics.js"
        )))
        .expect("valid analytics dashboard script response")
}

async fn admin_analytics(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
    if !admin_request_authorized(&headers, &state.db_secret) {
        return axum::response::IntoResponse::into_response((
            axum::http::StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "error": "Unauthorized" })),
        ));
    }
    let days = query
        .get("days")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(30)
        .clamp(1, 90);
    let url = format!(
        "{}/internal/analytics?days={days}",
        state.db_url.trim_end_matches('/')
    );
    let response = match state
        .replay_http
        .get(url)
        .bearer_auth(&state.db_secret)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            log::warn!(
                "[ADMIN] analytics source returned HTTP {}",
                response.status()
            );
            return axum::response::IntoResponse::into_response((
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(serde_json::json!({ "error": "Analytics unavailable" })),
            ));
        }
        Err(error) => {
            log::error!("[ADMIN] analytics source request failed: {error}");
            return axum::response::IntoResponse::into_response((
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(serde_json::json!({ "error": "Analytics unavailable" })),
            ));
        }
    };
    match response.json::<serde_json::Value>().await {
        Ok(summary) => axum::response::IntoResponse::into_response(axum::Json(summary)),
        Err(error) => {
            log::error!("[ADMIN] analytics response was invalid JSON: {error}");
            axum::response::IntoResponse::into_response((
                axum::http::StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({ "error": "Analytics unavailable" })),
            ))
        }
    }
}

async fn lobbies_json_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    let games = state.games.lock().await;
    let lobbies_info = lobby::build_lobby_broadcast(&games);
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store")
        .body(axum::body::Body::from(
            serde_json::to_string(&lobbies_info).unwrap(),
        ))
        .unwrap()
}

async fn catalog_json_handler() -> impl axum::response::IntoResponse {
    let catalog = map_catalog::catalog_json();
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .header("Cache-Control", "public, max-age=60")
        .body(axum::body::Body::from(
            serde_json::to_string(&catalog).unwrap(),
        ))
        .unwrap()
}

#[cfg(test)]
mod relay_worker_tests {
    use super::{
        DEFAULT_RELAY_WORKER_COUNT, RELAY_PORT_MAX, RELAY_PORT_MIN, RelayPortAllocator,
        RankedQueueEntry, RankedQueueMessage, RankedQueuePlayer, ReplayStartPlayer,
        cancel_ranked_search, consume_ranked_queue_message, parse_relay_worker,
        send_relay_handoff_frame, verified_winner_identity,
    };
    use crate::lobby::{LobbyPhase, PlayerConnection, ServerLobby};
    use std::collections::HashSet;
    use std::time::Duration;

    fn ranked_player(
        account_id: &str,
        session_id: u64,
        search_started_at: tokio::time::Instant,
        client_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> RankedQueuePlayer {
        RankedQueuePlayer {
            account_id: account_id.into(),
            rating: 1000.0,
            name: account_id.into(),
            clan_tag: String::new(),
            leader: sow_core::player::Leader::default(),
            skin_style: 0,
            client_tx,
            ip: "127.0.0.1".into(),
            session_id,
            search_started_at,
        }
    }

    fn ranked_member(
        player_id: u16,
        account_id: &str,
        session_id: u64,
        search_started_at: tokio::time::Instant,
        tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> PlayerConnection {
        PlayerConnection {
            name: account_id.into(),
            clan_tag: String::new(),
            player_id,
            tx,
            download_progress: 100,
            leader: sow_core::player::Leader::default(),
            random_leader: false,
            skin_style: 0,
            database_account_id: Some(account_id.into()),
            team: None,
            ip: "127.0.0.1".into(),
            session_id: Some(session_id),
            is_internal_bot: false,
            ranked_rating: Some(1000.0),
            ranked_search_started_at: Some(search_started_at),
        }
    }

    fn ranked_lobby(
        id: u64,
        players: Vec<PlayerConnection>,
    ) -> ServerLobby {
        ServerLobby {
            id,
            kind: sow_core::protocol::LobbyKind::Ranked1v1,
            is_private: true,
            phase: LobbyPhase::Loading,
            countdown_secs: 0.0,
            players,
            ready_players: HashSet::new(),
            seed: 0,
            config: sow_core::game_config::GameConfig {
                ranked_1v1: true,
                ..Default::default()
            },
            game_mode: "FFA".into(),
            relay_port: None,
            host_player_id: None,
            password: None,
            host_name: String::new(),
            banned: HashSet::new(),
            bot_fill_target: None,
            pending_bots: Vec::new(),
        }
    }

    #[test]
    fn duplicate_ranked_search_reconnects_without_resetting_queue_time() {
        let started = tokio::time::Instant::now() - Duration::from_secs(4);
        let (first_tx, _first_rx) = tokio::sync::mpsc::channel(4);
        let (reconnected_tx, _reconnected_rx) = tokio::sync::mpsc::channel(4);
        let mut pending = Vec::<RankedQueueEntry>::new();
        consume_ranked_queue_message(
            &mut pending,
            RankedQueueMessage::Search(ranked_player("account-a", 11, started, first_tx)),
        );
        consume_ranked_queue_message(
            &mut pending,
            RankedQueueMessage::Search(ranked_player(
                "account-a",
                12,
                tokio::time::Instant::now(),
                reconnected_tx,
            )),
        );
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].player.session_id, 12);
        assert_eq!(pending[0].player.search_started_at, started);
    }

    #[tokio::test]
    async fn cancel_during_preparation_requeues_only_the_opponent_with_original_wait() {
        let started = tokio::time::Instant::now() - Duration::from_secs(7);
        let (leaving_tx, mut leaving_rx) = tokio::sync::mpsc::channel(4);
        let (opponent_tx, mut opponent_rx) = tokio::sync::mpsc::channel(4);
        let lobby = ranked_lobby(
            73,
            vec![
                ranked_member(1, "leaving", 101, started, leaving_tx),
                ranked_member(2, "opponent", 202, started, opponent_tx),
            ],
        );
        let games = std::sync::Arc::new(tokio::sync::Mutex::new(vec![lobby]));
        let mut pending = Vec::new();
        let db_client = reqwest::Client::new();
        cancel_ranked_search(
            &games,
            &mut pending,
            "leaving",
            101,
            &db_client,
            "http://127.0.0.1:1",
            "unused",
        )
        .await;

        assert!(games.lock().await.is_empty());
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].player.account_id, "opponent");
        assert_eq!(pending[0].player.session_id, 202);
        assert_eq!(pending[0].player.search_started_at, started);
        let cancelled = bincode::deserialize::<sow_core::protocol::ServerMessage>(
            &leaving_rx.recv().await.unwrap(),
        )
        .unwrap();
        assert!(matches!(cancelled, sow_core::protocol::ServerMessage::LobbyClosed(message) if message.reason == "RANKED_CANCELLED"));
        let requeued = bincode::deserialize::<sow_core::protocol::ServerMessage>(
            &opponent_rx.recv().await.unwrap(),
        )
        .unwrap();
        assert!(matches!(requeued, sow_core::protocol::ServerMessage::LobbyClosed(message) if message.reason == "RANKED_REQUEUE"));

        cancel_ranked_search(
            &games,
            &mut pending,
            "leaving",
            101,
            &db_client,
            "http://127.0.0.1:1",
            "unused",
        )
        .await;
        assert_eq!(pending.len(), 1);
    }

    fn registered_human() -> ReplayStartPlayer {
        ReplayStartPlayer {
            player_id: 7,
            database_account_id: Some("0123456789abcdef0123456789abcdef".to_string()),
            leader: sow_core::player::Leader::default(),
            team: None,
            is_internal: false,
        }
    }

    #[test]
    fn an_ai_victory_settles_humans_as_defeated_without_an_unregistered_winner() {
        let human = registered_human();
        let ai = ReplayStartPlayer {
            player_id: 99,
            database_account_id: None,
            leader: sow_core::player::Leader::default(),
            team: None,
            is_internal: true,
        };
        assert_eq!(
            verified_winner_identity(Some(ai.player_id), None, &[human, ai], false, true),
            (None, None),
        );
    }

    #[test]
    fn a_registered_ranked_bot_victory_is_not_recorded_as_a_draw() {
        let human = registered_human();
        let bot = ReplayStartPlayer {
            player_id: 99,
            database_account_id: Some("abcdef0123456789abcdef0123456789".to_string()),
            leader: sow_core::player::Leader::default(),
            team: None,
            is_internal: true,
        };
        assert_eq!(
            verified_winner_identity(Some(bot.player_id), None, &[human, bot], true, true),
            (Some(99), None),
        );
    }

    #[test]
    fn an_ai_team_victory_uses_a_registered_teammate_for_the_verified_result() {
        let mut red_human = registered_human();
        red_human.team = Some(sow_core::protocol::Team::Red);
        let mut blue_human = ReplayStartPlayer {
            player_id: 8,
            ..registered_human()
        };
        blue_human.team = Some(sow_core::protocol::Team::Blue);
        let ai = ReplayStartPlayer {
            player_id: 99,
            database_account_id: None,
            leader: sow_core::player::Leader::default(),
            team: Some(sow_core::protocol::Team::Red),
            is_internal: true,
        };
        assert_eq!(
            verified_winner_identity(
                Some(ai.player_id),
                Some(sow_core::protocol::Team::Red),
                &[red_human, blue_human, ai],
                false,
                true
            ),
            (Some(7), Some("Red".into())),
        );
    }

    #[test]
    fn parses_game_and_management_hosts() {
        for (input, id, port) in [
            ("relay-a.example:mgmt.example:8080", 0, 8080),
            ("relay-a.example:mgmt.example:8083", 2, 8083),
        ] {
            let worker = parse_relay_worker(input, id).expect("valid worker");
            assert_eq!(worker.id, id);
            assert_eq!(worker.host, "relay-a.example");
            assert_eq!(worker.mgmt_url, format!("https://mgmt.example:{port}"));
        }
    }

    #[test]
    fn allocates_ports_in_range_and_by_worker_modulo() {
        let mut allocator = RelayPortAllocator::new();
        let first = allocator.allocate().expect("first dynamic port");
        let second = allocator.allocate().expect("second dynamic port");
        assert!((RELAY_PORT_MIN..=RELAY_PORT_MAX).contains(&first));
        assert!((RELAY_PORT_MIN..=RELAY_PORT_MAX).contains(&second));
        assert_eq!(first % DEFAULT_RELAY_WORKER_COUNT as u16, 0);
        assert_eq!(second % DEFAULT_RELAY_WORKER_COUNT as u16, 1);
        allocator.release(first);
        assert!(!allocator.used.contains(&first));
    }

    #[test]
    fn rejects_malformed_worker_entries() {
        assert!(parse_relay_worker("relay-a.example:8080", 0).is_none());
        assert!(parse_relay_worker("relay-a.example:mgmt.example:not-a-port", 0).is_none());
        assert!(parse_relay_worker("http://relay-a.example:mgmt.example:8080", 0).is_none());
        assert!(parse_relay_worker("[::1]:mgmt.example:8080", 0).is_none());
    }

    #[tokio::test]
    async fn handoff_frames_are_delivered_in_order() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        send_relay_handoff_frame(&tx, b"ticket".to_vec(), 7, 2, "RelayTicket")
            .await
            .expect("ticket delivery");
        let ticket = rx.recv().await.expect("ticket frame");
        assert_eq!(ticket, b"ticket");

        send_relay_handoff_frame(&tx, b"start".to_vec(), 7, 2, "Start")
            .await
            .expect("start delivery");
        let start = rx.recv().await.expect("start frame");
        assert_eq!(start, b"start");
    }
}
