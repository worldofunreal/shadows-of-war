//! test-relay: End-to-end integration test for the orchestrator → relay handoff.
//!
//! Simulates a real player:
//!   1. Connect to orchestrator WS
//!   2. Send authenticated JoinWithAuth, receive JoinAck
//!   3. Send MapDownloadProgress(100) + LobbyReady
//!   4. Wait for ServerMessage::Start with relay_port + relay_host
//!   5. Connect to the relay directly (wss://relay_host:relay_port/ws/)
//!   6. Send ReadyWithTicket to relay
//!   7. Receive at least one Turn
//!   8. Send Leave, disconnect
//!
//! Usage: cargo run --bin test-relay -- --url wss://shadowsofwar.io/ws/

use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use reqwest::Url;
use sow_core::protocol::{AuthProof, ClientMessage, ServerMessage};
use tokio_tungstenite::tungstenite::protocol::Message;

fn parse_websocket_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid WebSocket URL: {error}"))?;

    if !matches!(url.scheme(), "ws" | "wss") {
        return Err(format!(
            "unsupported URL scheme {:?}; expected ws:// or wss://",
            url.scheme()
        ));
    }
    let authority = value
        .split_once("://")
        .map(|(_, remainder)| remainder.split(['/', '?', '#']).next().unwrap_or_default())
        .unwrap_or_default();
    if authority.is_empty() {
        return Err("WebSocket URL must include an authority".to_string());
    }
    if url.host_str().is_none() {
        return Err("WebSocket URL must include a host".to_string());
    }
    if url.fragment().is_some() {
        return Err("WebSocket URL must not include a fragment".to_string());
    }

    Ok(url)
}

/// Direct relay URL from a `Start{relay_host, relay_port}` handoff.
/// Keeps the orchestrator URL's scheme; production always supplies both fields.
fn direct_relay_url(orchestrator_url: &Url, relay_host: &str, relay_port: u16) -> Option<Url> {
    let mut url = orchestrator_url.clone();
    if url.set_host(Some(relay_host)).is_err() {
        return None;
    }
    let _ = url.set_port(Some(relay_port));
    url.set_path("/ws/");
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

fn step(n: u8, label: &str) {
    eprintln!("\n\x1b[1;36m[STEP {n}]\x1b[0m {label}");
}

fn pass(msg: &str) {
    eprintln!("  \x1b[1;32m✅ {msg}\x1b[0m");
}

fn fail(msg: &str) -> ! {
    eprintln!("  \x1b[1;31m❌ {msg}\x1b[0m");
    std::process::exit(1);
}

async fn ws_send(
    ws: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    msg: &ClientMessage,
) {
    let bytes = bincode::serialize(msg).unwrap();
    ws.send(Message::Binary(bytes)).await.unwrap();
}

async fn recv(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    timeout_secs: u64,
) -> ServerMessage {
    let deadline = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), async {
        loop {
            match read.next().await {
                Some(Ok(Message::Binary(data))) => {
                    if let Ok(msg) = bincode::deserialize::<ServerMessage>(&data) {
                        return msg;
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => fail(&format!("WS read error: {e}")),
                None => fail("WS stream ended unexpectedly"),
            }
        }
    });
    match deadline.await {
        Ok(msg) => msg,
        Err(_) => fail(&format!(
            "Timed out after {timeout_secs}s waiting for server message"
        )),
    }
}

#[derive(Parser, Debug)]
#[command(
    about = "End-to-end integration test for orchestrator → relay handoff",
    long_about = "Simulates a player joining via the orchestrator WebSocket, waiting for Start, \
        connecting to the relay, receiving a Turn, then leaving.\n\n\
        Example:\n  cargo run --bin test-relay -- --url wss://shadowsofwar.io/ws/"
)]
struct Args {
    /// Orchestrator WebSocket URL
    #[arg(
        long,
        default_value = "wss://shadowsofwar.io/ws/",
        value_parser = parse_websocket_url
    )]
    url: Url,

    /// Authentication provider used by the join proof
    #[arg(long)]
    auth_provider: String,

    /// Account ID for providers that use an explicit account identity
    #[arg(long)]
    auth_account_id: Option<String>,

    /// Provider token or anonymous account secret
    #[arg(long)]
    auth_token: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let url = args.url;
    let auth = AuthProof {
        provider: args.auth_provider,
        account_id: args.auth_account_id,
        token: args.auth_token,
    };

    let version = std::fs::read_to_string(".version")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();

    eprintln!("\x1b[1;33m═══ SOW Relay Integration Test ═══\x1b[0m");
    eprintln!("  Orchestrator: {url}");
    eprintln!("  Build version: {version}");

    // ── Step 1: Connect to orchestrator ─────────────────────────────────────
    step(1, "Connecting to orchestrator...");
    let (ws, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .unwrap_or_else(|e| fail(&format!("Connect failed: {e}")));
    let (mut write, mut read) = ws.split();
    pass("Connected to orchestrator");

    // ── Step 2: Join ────────────────────────────────────────────────────────
    step(2, "Sending authenticated JoinWithAuth...");
    let join = ClientMessage::JoinWithAuth {
        join: Box::new(sow_core::protocol::JoinPayload {
            name: "TestBot".to_string(),
            target_lobby_id: None,
            host_private: false,
            build_version: version,
            clan_tag: "".to_string(),
            civilization: sow_core::player::Civilization::Rome,
            leader: sow_core::player::Leader::Caesar,
            host_config: None,
            password: None,
        }),
        auth,
    };
    ws_send(&mut write, &join).await;

    let lobby_id: u64;
    let player_id: u16;

    // Drain messages until we get JoinAck
    loop {
        let msg = recv(&mut read, 10).await;
        match msg {
            ServerMessage::JoinAck(ack) => {
                lobby_id = ack.lobby_id;
                player_id = ack.player_id;
                pass(&format!(
                    "JoinAck: lobby={lobby_id}, player={player_id}, map={}",
                    ack.map_name
                ));
                break;
            }
            ServerMessage::JoinFailed(f) => fail(&format!("JoinFailed: {}", f.reason)),
            ServerMessage::LobbiesBroadcast(_) => continue, // ignore broadcasts
            other => {
                eprintln!("  (ignoring {:?})", std::mem::discriminant(&other));
                continue;
            }
        }
    }

    // ── Step 3: Send LobbyReady (skip map download) ─────────────────────────
    step(
        3,
        "Sending MapDownloadProgress(100) + LobbyReady to orchestrator...",
    );
    ws_send(
        &mut write,
        &ClientMessage::MapDownloadProgress {
            lobby_id,
            player_id,
            progress: 100,
        },
    )
    .await;
    ws_send(
        &mut write,
        &ClientMessage::LobbyReady {
            lobby_id,
            player_id,
        },
    )
    .await;
    pass("Sent LobbyReady to orchestrator");

    // ── Step 4: Wait for Start ──────────────────────────────────────────────
    step(
        4,
        "Waiting for ServerMessage::Start (up to 30s for countdown)...",
    );
    let relay_port: u16;
    let relay_host: Option<String>;
    let mut relay_ticket: Option<String> = None;
    let start_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);

    loop {
        let remaining = start_deadline.duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            fail("Timed out waiting for Start message");
        }

        let msg = tokio::time::timeout(remaining, async {
            loop {
                match read.next().await {
                    Some(Ok(Message::Binary(data))) => {
                        if let Ok(m) = bincode::deserialize::<ServerMessage>(&data) {
                            return m;
                        }
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => fail(&format!("WS error: {e}")),
                    None => fail("WS closed before Start"),
                }
            }
        })
        .await;

        match msg {
            Ok(ServerMessage::RelayTicket {
                lobby_id: ticket_lobby,
                player_id: ticket_player,
                ticket,
            }) if ticket_lobby == lobby_id && ticket_player == player_id => {
                relay_ticket = Some(ticket);
                continue;
            }
            Ok(ServerMessage::Start(start)) => {
                relay_port = start.relay_port.unwrap_or(0);
                relay_host = start.relay_host.clone();
                pass(&format!(
                    "Start received! relay_host={:?}, relay_port={}, my_id={:?}, players={}, seed={}",
                    relay_host,
                    relay_port,
                    start.my_player_id,
                    start.players.len(),
                    start.seed
                ));
                if relay_port == 0 {
                    fail("Start message has no relay_port!");
                }
                break;
            }
            Ok(ServerMessage::SyncState(s)) => {
                eprintln!(
                    "  ⏳ SyncState: time_remaining={:.1}s, is_starting={}",
                    s.time_remaining, s.is_starting
                );
                continue;
            }
            Ok(ServerMessage::LobbiesBroadcast(_)) => continue,
            Ok(other) => {
                eprintln!("  (ignoring {:?})", std::mem::discriminant(&other));
                continue;
            }
            Err(_) => fail("Timed out waiting for Start"),
        }
    }

    // Drop orchestrator connection
    drop(write);
    drop(read);
    pass("Dropped orchestrator connection");

    // ── Step 5: Connect to relay ────────────────────────────────────────────
    step(5, &format!("Connecting to relay on port {relay_port}..."));

    let host = relay_host
        .as_deref()
        .unwrap_or_else(|| fail("Start message has no relay_host!"));
    let relay_url = direct_relay_url(&url, host, relay_port)
        .unwrap_or_else(|| fail(&format!("Invalid relay_host in Start: {host}")));
    eprintln!("  Relay URL: {relay_url}");

    // Retry connection for up to 5 seconds (relay may still be booting)
    let mut relay_ws = None;
    for attempt in 1..=10 {
        match tokio_tungstenite::connect_async(relay_url.as_str()).await {
            Ok((ws, _)) => {
                relay_ws = Some(ws);
                pass(&format!("Connected to relay (attempt {attempt})"));
                break;
            }
            Err(e) => {
                eprintln!("  ⏳ Attempt {attempt}/10 failed: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
    let relay_ws = relay_ws.unwrap_or_else(|| fail("Could not connect to relay after 10 attempts"));
    let (mut r_write, mut r_read) = relay_ws.split();

    // ── Step 6: Send ReadyWithTicket to relay ───────────────────────────────
    step(6, "Sending ReadyWithTicket to relay...");
    let ready = ClientMessage::ReadyWithTicket {
        lobby_id,
        player_id,
        ticket: relay_ticket
            .unwrap_or_else(|| fail("No relay ticket was received for this player")),
    };
    let bytes = bincode::serialize(&ready).unwrap();
    r_write.send(Message::Binary(bytes)).await.unwrap();
    pass("Sent ReadyWithTicket to relay");

    // ── Step 7: Receive turns ───────────────────────────────────────────────
    step(7, "Waiting for Turn messages from relay (5s window)...");
    let mut turn_count = 0u64;
    let turn_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        let remaining = turn_deadline.duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, r_read.next()).await {
            Ok(Some(Ok(Message::Binary(data)))) => {
                if let Ok(ServerMessage::Turn(t)) = bincode::deserialize::<ServerMessage>(&data) {
                    turn_count += 1;
                    if turn_count <= 3 || turn_count.is_multiple_of(20) {
                        eprintln!(
                            "  📦 Turn #{} (intents: {})",
                            t.turn.turn_number,
                            t.turn.intents.len()
                        );
                    }
                }
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => {
                eprintln!("  ⚠️  Read error: {e}");
                break;
            }
            Ok(None) => {
                eprintln!("  ⚠️  Relay stream ended");
                break;
            }
            Err(_) => break, // timeout, that's fine
        }
    }

    if turn_count > 0 {
        pass(&format!("Received {turn_count} turns from relay"));
    } else {
        fail("Received ZERO turns from relay — relay is not sending data!");
    }

    // ── Step 8: Leave ───────────────────────────────────────────────────────
    step(8, "Sending Leave to relay...");
    let leave = bincode::serialize(&ClientMessage::Leave {}).unwrap();
    let _ = r_write.send(Message::Binary(leave)).await;
    pass("Sent Leave");

    // Done
    eprintln!("\n\x1b[1;32m═══ ALL STEPS PASSED ═══\x1b[0m");
    eprintln!("  Orchestrator handoff → Relay connection → Turn receive → Clean exit");
    eprintln!("  {turn_count} turns received in 5 seconds\n");
}

#[cfg(test)]
mod tests {
    use super::{Args, direct_relay_url, parse_websocket_url};
    use clap::Parser;

    #[test]
    fn rejects_non_websocket_and_hostless_urls() {
        assert!(parse_websocket_url("https://example.test/ws/").is_err());
        assert!(parse_websocket_url("ws:///ws/").is_err());
        assert!(parse_websocket_url("not a url").is_err());
    }

    #[test]
    fn direct_relay_url_uses_start_host_and_port() {
        let orchestrator = parse_websocket_url("wss://shadowsofwar.io/ws/").unwrap();

        assert_eq!(
            direct_relay_url(&orchestrator, "relay.shadowsofwar.io", 25_592)
                .unwrap()
                .as_str(),
            "wss://relay.shadowsofwar.io:25592/ws/"
        );
    }

    #[test]
    fn direct_relay_url_rejects_invalid_host() {
        let orchestrator = parse_websocket_url("wss://shadowsofwar.io/ws/").unwrap();

        assert!(direct_relay_url(&orchestrator, "not a host!", 25_592).is_none());
    }

    #[test]
    fn auth_arguments_are_required() {
        assert!(Args::try_parse_from(["test-relay"]).is_err());

        let args = Args::try_parse_from([
            "test-relay",
            "--auth-provider",
            "anonymous",
            "--auth-token",
            "secret",
        ])
        .unwrap();
        assert_eq!(args.auth_provider, "anonymous");
        assert_eq!(args.auth_token, "secret");
    }
}
