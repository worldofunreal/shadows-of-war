//! Production acceptance using the real Rust/WASM exit and shared presenter.
use super::*;
use serde_json::{Value, json};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::process::{Command, Stdio};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type Writer = futures_util::stream::SplitSink<Socket, Message>;
type Reader = futures_util::stream::SplitStream<Socket>;

async fn profile(http: &reqwest::Client, base: &Url, account: Option<&Value>, name: &str) -> Value {
    http.post(format!(
        "{}/profile/anonymous",
        base.as_str().trim_end_matches('/')
    ))
    .json(&json!({"account_id":account.map(|a| &a["account_id"]),
            "auth_secret":account.map(|a| &a["auth_secret"]),"display_name":name}))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap()
}

async fn join(
    url: &Url,
    account: &Value,
    target: Option<u64>,
    version: &str,
) -> (Writer, Reader, sow_core::protocol::ServerJoinAckMessage) {
    join_with_match_setup(url, account, target, version, 2, false).await
}

async fn join_with_match_setup(
    url: &Url,
    account: &Value,
    target: Option<u64>,
    version: &str,
    nation_count: u32,
    random_spawn: bool,
) -> (Writer, Reader, sow_core::protocol::ServerJoinAckMessage) {
    let (socket, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .unwrap();
    let (mut write, mut read) = socket.split();
    let mut config = sow_core::game_config::GameConfig::default();
    config.map_name = "world".into();
    config.bot_count = 0;
    config.nation_count = nation_count;
    config.random_spawn = random_spawn;
    let command = ClientMessage::JoinWithAuth {
        join: Box::new(sow_core::protocol::JoinPayload {
            name: account["display_name"].as_str().unwrap().into(),
            target_lobby_id: target,
            host_private: target.is_none(),
            build_version: sow_core::protocol::gameplay_build_identity(version),
            clan_tag: String::new(),
            leader: sow_core::player::Leader::of("caesar"),
            random_leader: true,
            host_config: target.is_none().then(|| Box::new(config)),
            password: None,
        }),
        auth: AuthProof {
            provider: "anonymous".into(),
            account_id: Some(account["account_id"].as_str().unwrap().into()),
            token: account["auth_secret"]
                .as_str()
                .expect("audit account must own its secret")
                .into(),
        },
    };
    ws_send(&mut write, &command).await;
    loop {
        match recv(&mut read, 20).await {
            ServerMessage::JoinAck(ack) => return (write, read, ack),
            ServerMessage::JoinFailed(failure) => {
                fail(&format!("reward audit join rejected: {}", failure.reason))
            }
            _ => {}
        }
    }
}

async fn started(read: &mut Reader) -> (sow_core::protocol::ServerStartMessage, String) {
    let mut ticket = None;
    loop {
        match recv(read, 30).await {
            ServerMessage::RelayTicket { ticket: value, .. } => ticket = Some(value),
            ServerMessage::Start(start) => {
                return (*start, ticket.expect("Start must follow its relay ticket"));
            }
            ServerMessage::LobbyClosed(closed) => fail(&format!(
                "audit lobby closed before start: {}",
                closed.reason
            )),
            _ => {}
        }
    }
}

async fn natural_finish(
    read: &mut Reader,
    match_id: u64,
) -> (u64, tokio::time::Instant, tokio::time::Instant) {
    tokio::time::timeout(std::time::Duration::from_secs(90), async {
        let mut recent_turns = std::collections::VecDeque::with_capacity(256);
        loop {
            match recv(read, 90).await {
                ServerMessage::MatchFinished {
                    match_id: finished,
                    terminal_turn,
                } if finished == match_id => {
                    let terminal_turn_at = recent_turns
                        .iter()
                        .find(|(turn, _)| *turn == terminal_turn)
                        .map(|(_, received_at)| *received_at)
                        .unwrap_or_else(|| {
                            fail(&format!(
                                "terminal turn {terminal_turn} was not observed before MatchFinished"
                            ))
                        });
                    return (terminal_turn, terminal_turn_at, tokio::time::Instant::now());
                }
                ServerMessage::Turn(turn) => {
                    recent_turns.push_back((turn.turn.turn_number, tokio::time::Instant::now()));
                    if recent_turns.len() > 256 {
                        recent_turns.pop_front();
                    }
                }
                ServerMessage::Pong { .. } => {}
                message => fail(&format!(
                    "match ended unexpectedly while waiting for automatic settlement: {message:?}"
                )),
            }
        }
    })
    .await
    .unwrap_or_else(|_| fail("natural finish was not delivered within 90 seconds"))
}

async fn voluntary_exit_recorded(read: &mut Reader, account_id: &str, match_id: u64) {
    loop {
        match recv(read, 20).await {
            ServerMessage::ExitRecorded {
                account_id: recorded_account,
                match_id: recorded_match,
                reason,
                ..
            } if recorded_account == account_id && recorded_match == match_id => {
                assert_eq!(reason, sow_core::protocol::ExitReason::Voluntary);
                return;
            }
            ServerMessage::Turn(_) | ServerMessage::Pong { .. } => {}
            message => fail(&format!(
                "unexpected response while waiting for exit receipt: {message:?}"
            )),
        }
    }
}

async fn ready_relay(url: &Url, message: ClientMessage) -> (Writer, Reader, String) {
    let (socket, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .unwrap();
    let (mut write, mut read) = socket.split();
    ws_send(&mut write, &message).await;
    let mut reconnect = None;
    loop {
        match recv(&mut read, 10).await {
            ServerMessage::RelayReconnectTicket { ticket, .. } => reconnect = Some(ticket),
            ServerMessage::Turn(_) => {
                return (
                    write,
                    read,
                    reconnect.expect("relay must supply a reconnect capability"),
                );
            }
            _ => {}
        }
    }
}

async fn result(http: &reqwest::Client, base: &Url, account: &Value, match_id: u64) -> Value {
    http.post(format!(
        "{}/profile/match-result",
        base.as_str().trim_end_matches('/')
    ))
    .timeout(std::time::Duration::from_secs(25))
    .json(
        &json!({"account_id":account["account_id"],"auth_secret":account["auth_secret"],
            "match_id":match_id.to_string(),"closure_known":true}),
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap()
}

async fn relay_control(host: &str, command: String) -> Value {
    let host = host.to_string();
    let output = tokio::task::spawn_blocking(move || Command::new("ssh")
        .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", &host, &command])
        .output().unwrap()).await.unwrap();
    assert!(output.status.success(), "relay handover control failed: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("relay control must return JSON")
}

async fn port_owner(host: &str, port: u16) -> Value {
    // Read the coordinator's existing ownership table, not a second registry.
    let command = format!(r#"sudo -u sowrelay python3 -c 'import socket,json; s=socket.socket(socket.AF_UNIX); s.settimeout(5); s.connect("/run/sow-relay/upgrade.sock"); s.sendall(json.dumps({{"port_owner":{port}}}).encode()); s.shutdown(socket.SHUT_WR); print(s.makefile("rb").read(65537).decode())'"#);
    relay_control(host, command).await
}

async fn verify_handover(url: &Url, base: &Url, http: &reqwest::Client, version: &str,
    host: &str, accounts: &[Value], old_port: u16, first_read: &mut Reader, second_read: &mut Reader)
{
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let continuity = async {
        let mut stopped = stopped;
        let mut last_turn: [Option<u64>; 2] = [None, None];
        let mut counts = [0u32; 2];
        let mut last_message = [tokio::time::Instant::now(); 2];
        let mut max_gap = std::time::Duration::ZERO;
        loop {
            let (player, message) = tokio::select! {
                _ = &mut stopped => break,
                message = recv(first_read, 5) => (0, message),
                message = recv(second_read, 5) => (1, message),
            };
            let gap = last_message[player].elapsed();
            max_gap = max_gap.max(gap);
            assert!(gap < std::time::Duration::from_secs(2),
                "existing player stopped receiving traffic during handover");
            last_message[player] = tokio::time::Instant::now();
            match message {
                ServerMessage::Turn(turn) => {
                    let number = turn.turn.turn_number;
                    if let Some(previous) = last_turn[player] {
                        assert_eq!(number, previous + 1, "handover lost or duplicated a turn");
                    }
                    last_turn[player] = Some(number);
                    counts[player] += 1;
                }
                ServerMessage::Pong { .. } | ServerMessage::RelayReconnectTicket { .. } => {}
                message => fail(&format!("existing match was interrupted during handover: {message:?}")),
            }
        }
        assert!(counts.iter().all(|count| *count >= 5), "both existing players must remain active through handover");
        (counts, max_gap)
    };
    let exercise = async {
        let changed = relay_control(host,
            "sudo -u sowrelay /usr/local/libexec/sow-relay/sow-relay --fstack-upgrade /usr/local/libexec/sow-relay/sow-relay".into()).await;
        assert_eq!(changed["ok"], true);
        let new_generation = changed["generation"].as_i64().unwrap();
        let old_owner = port_owner(host, old_port).await;
        assert_eq!(old_owner["owner"].as_i64(), Some(1 - new_generation));
        let (mut host_write, mut host_read, host_join) =
            join_with_match_setup(url, &accounts[0], None, version, 2, true).await;
        let (mut peer_write, mut peer_read, peer_join) =
            join(url, &accounts[1], Some(host_join.lobby_id), version).await;
        let id = host_join.lobby_id;
        for (write, player_id) in [(&mut host_write, host_join.player_id), (&mut peer_write, peer_join.player_id)] {
            ws_send(write, &ClientMessage::MapDownloadProgress { lobby_id:id, player_id, progress:100 }).await;
            ws_send(write, &ClientMessage::LobbyReady { lobby_id:id, player_id }).await;
        }
        ws_send(&mut host_write, &ClientMessage::ForceStart { lobby_id:id, player_id:host_join.player_id }).await;
        let (start, host_ticket) = started(&mut host_read).await;
        let (_, peer_ticket) = started(&mut peer_read).await;
        let port = start.relay_port.unwrap();
        assert_eq!(port_owner(host, port).await["owner"].as_i64(), Some(new_generation),
            "new match must be owned by the replacement generation");
        let direct = direct_relay_url(url, start.relay_host.as_deref().unwrap(), port).unwrap();
        let (mut host_relay, mut host_game, _) = ready_relay(&direct, ClientMessage::ReadyWithTicket {
            lobby_id:id, player_id:host_join.player_id, ticket:host_ticket,
        }).await;
        let (mut peer_relay, mut peer_game, _) = ready_relay(&direct, ClientMessage::ReadyWithTicket {
            lobby_id:id, player_id:peer_join.player_id, ticket:peer_ticket,
        }).await;
        let mut turns = 0;
        while turns < 5 {
            if matches!(recv(&mut host_game, 5).await, ServerMessage::Turn(_)) { turns += 1; }
        }
        ws_send(&mut host_relay, &ClientMessage::Leave {}).await;
        voluntary_exit_recorded(&mut host_game, accounts[0]["account_id"].as_str().unwrap(), id).await;
        natural_finish(&mut peer_game, id).await;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        for account in accounts {
            loop {
                let receipt = tokio::time::timeout_at(deadline, result(http, base, account, id)).await
                    .expect("new-generation rewards did not finish");
                if receipt["state"] == "confirmed" { break; }
            }
        }
        let _ = host_relay.close().await;
        let _ = peer_relay.close().await;
        stop.send(()).unwrap();
    };
    let ((counts, max_gap), ()) = tokio::join!(continuity, exercise);
    pass(&format!("Live handover preserved old streams (turns={counts:?}, max_gap_ms={}); replacement served and settled a new match", max_gap.as_millis()));
}

pub(super) async fn verify(url: &Url, base: &Url, accounts_file: Option<&std::path::Path>, handover_host: Option<&str>) {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap();
    let saved: Option<Vec<Value>> = accounts_file
        .filter(|path| path.exists())
        .map(|path| serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap());
    let mut first = profile(
        &http,
        base,
        saved.as_ref().and_then(|accounts| accounts.get(0)),
        "Exit Audit One",
    )
    .await;
    let mut second = profile(
        &http,
        base,
        saved.as_ref().and_then(|accounts| accounts.get(1)),
        "Exit Audit Two",
    )
    .await;
    let mut third = profile(
        &http,
        base,
        saved.as_ref().and_then(|accounts| accounts.get(2)),
        "Exit Audit Three",
    )
    .await;
    for (index, account) in [&mut first, &mut second, &mut third]
        .into_iter()
        .enumerate()
    {
        if let Some(secret) = saved
            .as_ref()
            .and_then(|accounts| accounts.get(index))
            .and_then(|account| account.get("auth_secret"))
        {
            account["auth_secret"] = secret.clone();
        }
    }
    let mut handover_accounts = Vec::new();
    if handover_host.is_some() {
        for (index, name) in [(3, "Relay Upgrade One"), (4, "Relay Upgrade Two")] {
            let previous = saved.as_ref().and_then(|accounts| accounts.get(index));
            let mut account = profile(&http, base, previous, name).await;
            if let Some(secret) = previous.and_then(|account| account.get("auth_secret")) {
                account["auth_secret"] = secret.clone();
            }
            handover_accounts.push(account);
        }
    } else if let Some(saved) = &saved {
        handover_accounts.extend(saved.iter().skip(3).cloned());
    }
    if let Some(path) = accounts_file {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let temporary = path.with_extension("json.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .unwrap();
        file.write_all(
            serde_json::to_string(&[vec![&first, &second, &third], handover_accounts.iter().collect()].concat())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        file.sync_all().unwrap();
        drop(file);
        std::fs::rename(temporary, path).unwrap();
        std::fs::File::open(path.parent().unwrap())
            .unwrap()
            .sync_all()
            .unwrap();
    }
    let version = std::fs::read_to_string(".version").unwrap();
    let (mut first_write, mut first_read, first_join) =
        join(url, &first, None, version.trim()).await;
    let match_id = first_join.lobby_id;
    let (mut second_write, mut second_read, second_join) =
        join(url, &second, Some(match_id), version.trim()).await;
    let (mut third_write, mut third_read, third_join) =
        join(url, &third, Some(match_id), version.trim()).await;
    for (write, id) in [
        (&mut first_write, first_join.player_id),
        (&mut second_write, second_join.player_id),
        (&mut third_write, third_join.player_id),
    ] {
        ws_send(
            write,
            &ClientMessage::MapDownloadProgress {
                lobby_id: match_id,
                player_id: id,
                progress: 100,
            },
        )
        .await;
        ws_send(
            write,
            &ClientMessage::LobbyReady {
                lobby_id: match_id,
                player_id: id,
            },
        )
        .await;
    }
    ws_send(
        &mut first_write,
        &ClientMessage::ForceStart {
            lobby_id: match_id,
            player_id: first_join.player_id,
        },
    )
    .await;
    let (first_start, first_ticket) = started(&mut first_read).await;
    let (second_start, second_ticket) = started(&mut second_read).await;
    let (third_start, third_ticket) = started(&mut third_read).await;
    let relay_url = direct_relay_url(
        url,
        first_start.relay_host.as_deref().unwrap(),
        first_start.relay_port.unwrap(),
    )
    .unwrap();
    let (mut other_write, other_read, reconnect) = ready_relay(
        &relay_url,
        ClientMessage::ReadyWithTicket {
            lobby_id: match_id,
            player_id: second_start.my_player_id.unwrap(),
            ticket: second_ticket,
        },
    )
    .await;
    let (mut active_write, mut active_read, next_ticket) = ready_relay(
        &relay_url,
        ClientMessage::ReconnectWithTicket {
            lobby_id: match_id,
            player_id: second_start.my_player_id.unwrap(),
            ticket: reconnect,
        },
    )
    .await;
    // This still-authenticated old socket no longer owns the participation.
    ws_send(&mut other_write, &ClientMessage::Leave {}).await;
    drop(other_write);
    drop(other_read);
    assert!(
        matches!(
            recv(&mut active_read, 5).await,
            ServerMessage::Turn(_) | ServerMessage::Pong { .. }
        ),
        "a superseded socket cannot exit the reconnected player"
    );
    active_write.close().await.unwrap();
    drop(active_write);
    drop(active_read);
    let (mut other_write, mut other_read, handover_reconnect) = ready_relay(
        &relay_url,
        ClientMessage::ReconnectWithTicket {
            lobby_id: match_id,
            player_id: second_start.my_player_id.unwrap(),
            ticket: next_ticket,
        },
    )
    .await;
    let (mut survivor_write, mut survivor_read, _) = ready_relay(
        &relay_url,
        ClientMessage::ReadyWithTicket {
            lobby_id: match_id,
            player_id: third_start.my_player_id.unwrap(),
            ticket: third_ticket,
        },
    )
    .await;
    assert!(
        matches!(
            recv(&mut survivor_read, 5).await,
            ServerMessage::Turn(_) | ServerMessage::Pong { .. }
        ),
        "an active player continues receiving match traffic"
    );
    let still_playing = profile(&http, base, Some(&second), "Exit Audit Two").await;
    assert!(
        still_playing["profile"]["reward_receipts"]
            .get(match_id.to_string())
            .is_none(),
        "temporary disconnect must not settle an exit"
    );
    let removed_exit_command = http
        .post(format!("{}/profile/match-exit", base.as_str().trim_end_matches('/')))
        .json(&json!({"account_id":second["account_id"],"auth_secret":first["auth_secret"],"match_id":match_id.to_string()}))
        .send().await.unwrap();
    assert_eq!(
        removed_exit_command.status(),
        reqwest::StatusCode::NOT_FOUND
    );
    let wrong_account_observation = http
        .post(format!("{}/profile/match-result", base.as_str().trim_end_matches('/')))
        .json(&json!({"account_id":second["account_id"],"auth_secret":first["auth_secret"],"match_id":match_id.to_string(),"closure_known":true}))
        .send().await.unwrap();
    assert_eq!(
        wrong_account_observation.status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let fixture_path = std::env::temp_dir().join(format!(
        "sow-match-exit-{:016x}.json",
        rand::random::<u64>()
    ));
    let state_path = fixture_path.with_extension("state.json");
    let origin = base.origin().ascii_serialization();
    let fixture = json!({"account":first,"match_id":match_id,"player_id":first_join.player_id,
        "relay_url":relay_url.as_str(),"relay_ticket":first_ticket,"orchestrator_url":url.as_str(),
        "database_base":base.as_str().trim_end_matches('/'),"maps_base":format!("{origin}/maps"),
        "assets_base":format!("{origin}/assets"),"public_version":version.trim(),"state_path":state_path});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&fixture_path)
        .unwrap();
    file.write_all(serde_json::to_string(&fixture).unwrap().as_bytes())
        .unwrap();
    drop(file);
    let mut client = Command::new("cargo")
        .args([
            "test",
            "--locked",
            "-p",
            "sow-client",
            "--target",
            "wasm32-unknown-unknown",
            "--config",
            "target.wasm32-unknown-unknown.runner=\"wasm-bindgen-test-runner\"",
            "--lib",
            "reward_exit_tests::multiplayer_exit_survives_closed_port_delay_and_reload",
            "--",
            "--nocapture",
        ])
        .env("SOW_LIVE_MATCH_EXIT_FILE", &fixture_path)
        .env("WASM_BINDGEN_TEST_TIMEOUT", "120")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(180);
    let mut child_poll = tokio::time::interval(std::time::Duration::from_millis(20));
    child_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_relay_message = tokio::time::Instant::now();
    let status = loop {
        if let Some(status) = client.try_wait().unwrap() {
            break status;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "real WASM return exceeded its deadline"
        );
        let message = tokio::select! {
            message = recv(&mut other_read, 5) => Some(message),
            message = recv(&mut survivor_read, 5) => Some(message),
            _ = child_poll.tick() => None,
        };
        if let Some(message) = message {
            last_relay_message = tokio::time::Instant::now();
            assert!(
                matches!(
                    message,
                    ServerMessage::Turn(_)
                        | ServerMessage::RelayReconnectTicket { .. }
                        | ServerMessage::Pong { .. }
                ),
                "other players must keep receiving turns during the first exit"
            );
        } else {
            assert!(
                tokio::time::Instant::now().duration_since(last_relay_message)
                    < std::time::Duration::from_secs(5),
                "other players stopped receiving match traffic during the first exit"
            );
        }
    };
    assert!(status.success(), "actual WASM client exit failed");
    let pong_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut other_ping = tokio::time::interval_at(
        tokio::time::Instant::now() + std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(1),
    );
    other_ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut sent_pings = Vec::new();
    let mut received_turn = false;
    let mut received_pong = false;
    while !(received_turn && received_pong) {
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(pong_deadline) => panic!(
                "B did not receive a turn and matching Pong within 5 seconds (turn={received_turn}, pong={received_pong})"
            ),
            _ = other_ping.tick(), if !received_pong => {
                let client_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64();
                ws_send(&mut other_write, &ClientMessage::Ping { client_time }).await;
                sent_pings.push(client_time);
            }
            message = recv(&mut other_read, 5) => {
                match message {
                    ServerMessage::Turn(_) => received_turn = true,
                    ServerMessage::Pong { client_time } => {
                        assert!(
                            sent_pings.contains(&client_time),
                            "B received a Pong that did not match a Ping sent by B"
                        );
                        received_pong = true;
                    }
                    message => panic!(
                        "unexpected relay message while waiting for B's turn and Pong: {message:?}"
                    ),
                }
            }
        }
    }
    if let Some(host) = handover_host {
        verify_handover(url, base, &http, version.trim(), host, &handover_accounts,
            first_start.relay_port.unwrap(), &mut other_read, &mut survivor_read).await;
        other_write.close().await.unwrap();
        let (write, read, _) = ready_relay(&relay_url, ClientMessage::ReconnectWithTicket {
            lobby_id: match_id, player_id: second_start.my_player_id.unwrap(), ticket: handover_reconnect,
        }).await;
        other_write = write;
        other_read = read;
        pass("Existing match reconnected through its original credentials after live handover");
    }
    let settled_first = profile(&http, base, Some(&first), "Exit Audit One").await;
    for _ in 0..3 {
        let repeated = result(&http, base, &first, match_id).await;
        assert_eq!(repeated["state"], "confirmed");
        assert_eq!(repeated["profile"]["xp"], settled_first["profile"]["xp"]);
        assert_eq!(
            repeated["profile"]["crowns"],
            settled_first["profile"]["crowns"]
        );
    }
    let status = Command::new("node")
        .args([
            "--test",
            "--test-name-pattern=live multiplayer return uses the confirmed client state",
            "sow-web/shell/menu_flow.test.js",
        ])
        .env("SOW_LIVE_RETURN_STATE_FILE", &state_path)
        .env("SOW_LIVE_MATCH_EXIT_FILE", &fixture_path)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "real client state did not finish reward presentation"
    );
    // No exit command for B: its temporary absence must expire in the relay
    // while C continues receiving turns. Timing uses wall time, not tick count.
    let disconnected_at = tokio::time::Instant::now();
    other_write.close().await.unwrap();
    drop(other_write);
    drop(other_read);
    let expiry = disconnected_at + std::time::Duration::from_secs(60);
    let receive_until = expiry + std::time::Duration::from_secs(1);
    let pong_timeout = std::time::Duration::from_secs(5);
    let mut survivor_ping = tokio::time::interval_at(
        tokio::time::Instant::now() + std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(1),
    );
    survivor_ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut pending_pings: Vec<(f64, tokio::time::Instant)> = Vec::new();
    let mut sent_pings = 0;
    let mut turn_after_timeout = false;
    while tokio::time::Instant::now() < receive_until {
        let pong_deadline = pending_pings
            .first()
            .map(|(_, sent_at)| *sent_at + pong_timeout);
        let can_send_ping = tokio::time::Instant::now() + pong_timeout <= receive_until;
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(receive_until) => break,
            _ = async { tokio::time::sleep_until(pong_deadline.unwrap()).await }, if pong_deadline.is_some() => {
                fail("survivor C did not receive a matching relay Pong within 5 seconds");
            }
            _ = survivor_ping.tick(), if can_send_ping => {
                let client_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64();
                ws_send(&mut survivor_write, &ClientMessage::Ping { client_time }).await;
                pending_pings.push((client_time, tokio::time::Instant::now()));
                sent_pings += 1;
            }
            message = recv(&mut survivor_read, 5) => {
                match message {
                    ServerMessage::Turn(turn) => {
                        if tokio::time::Instant::now() >= expiry {
                            turn_after_timeout = true;
                        }
                        assert!(
                            !turn
                                .turn
                                .intents
                                .iter()
                                .any(|intent| intent.player_id == third_join.player_id
                                    && matches!(intent.intent, sow_core::protocol::GameplayIntent::Resign)),
                            "the survivor cannot be closed with B"
                        );
                    }
                    ServerMessage::Pong { client_time } => {
                        let position = pending_pings
                            .iter()
                            .position(|(sent_time, _)| *sent_time == client_time)
                            .expect("survivor C received a Pong not matching a Ping sent by C");
                        let (_, sent_at) = pending_pings.remove(position);
                        assert!(
                            tokio::time::Instant::now().duration_since(sent_at) <= pong_timeout,
                            "survivor C's relay Pong exceeded the 5 second limit"
                        );
                    }
                    message => fail(&format!(
                        "survivor lost its game during another participant's timeout: {message:?}"
                    )),
                }
            }
        }
    }
    assert!(turn_after_timeout);
    assert!(sent_pings > 0, "survivor C must send relay heartbeats");
    assert!(
        pending_pings.is_empty(),
        "survivor C has an unanswered relay Ping"
    );
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let observed = result(&http, base, &second, match_id).await;
        if observed["state"] == "confirmed" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "relay timeout never settled without an exit command"
        );
    }
    assert!(matches!(
        recv(&mut survivor_read, 5).await,
        ServerMessage::Turn(_) | ServerMessage::Pong { .. }
    ));
    let closed_survivor = survivor_write
        .send(Message::Binary(
            bincode::serialize(&ClientMessage::Leave {}).unwrap(),
        ))
        .await;
    assert!(closed_survivor.is_ok());
    let mut got_ack = false;
    while !got_ack {
        match recv(&mut survivor_read, 5).await {
            ServerMessage::ExitRecorded {
                account_id,
                match_id: ack_match,
                ..
            } => {
                assert_eq!(account_id, third["account_id"].as_str().unwrap());
                assert_eq!(ack_match, match_id);
                got_ack = true;
            }
            ServerMessage::Turn(_) | ServerMessage::Pong { .. } => {}
            message => fail(&format!("unexpected exit acknowledgment: {message:?}")),
        }
    }
    drop(survivor_write);
    drop(survivor_read);
    loop {
        let observed = result(&http, base, &third, match_id).await;
        if observed["state"] == "confirmed" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "third participant never received its reward"
        );
    }
    // Personal confirmation can precede the whole replay. Wait for the
    // existing public record to prove that its verification also finished.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let detail: Value = http
            .get(format!(
                "{}/matches/{match_id}",
                base.as_str().trim_end_matches('/')
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(detail["match_id"], match_id.to_string());
        if detail["verified"] == true && detail["provisional"] == false {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "whole replay verification never finished"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let after_closure = result(&http, base, &first, match_id).await;
    assert_eq!(after_closure["state"], "confirmed");
    for field in ["xp", "crowns", "laurels"] {
        assert_eq!(
            after_closure["profile"][field], settled_first["profile"][field],
            "whole-match closure must preserve the first participant's {field}"
        );
    }
    let receipt = match_id.to_string();
    assert_eq!(
        after_closure["profile"]["reward_receipts"][&receipt]["personal_closure_turn"],
        settled_first["profile"]["reward_receipts"][&receipt]["personal_closure_turn"]
    );
    drop(first_write);
    drop(first_read);
    drop(second_write);
    drop(second_read);
    drop(third_write);
    drop(third_read);

    // The server owns win rules. Start a normal three-player match, let two
    // players leave through the relay, and verify the remaining player gets
    // the server-detected GameOver without requesting an exit.
    let (mut finish_host_write, mut finish_host_read, finish_host_join) =
        join_with_match_setup(url, &first, None, version.trim(), 0, true).await;
    let finish_match_id = finish_host_join.lobby_id;
    let (mut finish_second_write, mut finish_second_read, finish_second_join) =
        join(url, &second, Some(finish_match_id), version.trim()).await;
    let (mut finish_third_write, mut finish_third_read, finish_third_join) =
        join(url, &third, Some(finish_match_id), version.trim()).await;
    for (write, id) in [
        (&mut finish_host_write, finish_host_join.player_id),
        (&mut finish_second_write, finish_second_join.player_id),
        (&mut finish_third_write, finish_third_join.player_id),
    ] {
        ws_send(
            write,
            &ClientMessage::MapDownloadProgress {
                lobby_id: finish_match_id,
                player_id: id,
                progress: 100,
            },
        )
        .await;
        ws_send(
            write,
            &ClientMessage::LobbyReady {
                lobby_id: finish_match_id,
                player_id: id,
            },
        )
        .await;
    }
    ws_send(
        &mut finish_host_write,
        &ClientMessage::ForceStart {
            lobby_id: finish_match_id,
            player_id: finish_host_join.player_id,
        },
    )
    .await;
    let (finish_start, finish_host_ticket) = started(&mut finish_host_read).await;
    let (_, finish_second_ticket) = started(&mut finish_second_read).await;
    let (_, finish_third_ticket) = started(&mut finish_third_read).await;
    let finish_relay_url = direct_relay_url(
        url,
        finish_start.relay_host.as_deref().unwrap(),
        finish_start.relay_port.unwrap(),
    )
    .unwrap();
    let (mut finish_host_relay, mut finish_host_game, _) = ready_relay(
        &finish_relay_url,
        ClientMessage::ReadyWithTicket {
            lobby_id: finish_match_id,
            player_id: finish_start.my_player_id.unwrap(),
            ticket: finish_host_ticket,
        },
    )
    .await;
    let (mut finish_second_relay, mut finish_second_game, _) = ready_relay(
        &finish_relay_url,
        ClientMessage::ReadyWithTicket {
            lobby_id: finish_match_id,
            player_id: finish_second_join.player_id,
            ticket: finish_second_ticket,
        },
    )
    .await;
    let (mut finish_third_relay, mut finish_third_game, _) = ready_relay(
        &finish_relay_url,
        ClientMessage::ReadyWithTicket {
            lobby_id: finish_match_id,
            player_id: finish_third_join.player_id,
            ticket: finish_third_ticket,
        },
    )
    .await;
    ws_send(&mut finish_second_relay, &ClientMessage::Leave {}).await;
    ws_send(&mut finish_third_relay, &ClientMessage::Leave {}).await;
    tokio::join!(
        voluntary_exit_recorded(
            &mut finish_second_game,
            second["account_id"].as_str().unwrap(),
            finish_match_id,
        ),
        voluntary_exit_recorded(
            &mut finish_third_game,
            third["account_id"].as_str().unwrap(),
            finish_match_id,
        ),
    );
    let _ = finish_second_relay.close().await;
    let _ = finish_third_relay.close().await;

    let host_finish = natural_finish(&mut finish_host_game, finish_match_id).await;
    let automatic_result_started = host_finish.1;
    let automatic_result_deadline = automatic_result_started + std::time::Duration::from_secs(5);
    let natural_finish_ms = host_finish
        .2
        .duration_since(automatic_result_started)
        .as_millis();
    assert!(
        natural_finish_ms <= 5_000,
        "natural GameOver confirmation exceeded 5 seconds after the terminal turn"
    );
    for account in [&first, &second, &third] {
        loop {
            let observed = tokio::time::timeout_at(
                automatic_result_deadline,
                result(&http, base, account, finish_match_id),
            )
            .await
            .unwrap_or_else(|_| fail("natural GameOver reward was not confirmed within 5 seconds"));
            if observed["state"] == "confirmed" {
                break;
            }
            assert!(
                tokio::time::Instant::now() < automatic_result_deadline,
                "natural GameOver did not settle without a client exit within 5 seconds"
            );
        }
    }
    let natural_rewards_ms = automatic_result_started.elapsed().as_millis();
    assert!(
        natural_rewards_ms <= 5_000,
        "natural GameOver rewards were not confirmed within 5 seconds of the terminal turn"
    );
    let _ = finish_host_relay.close().await;
    drop(finish_host_write);
    drop(finish_host_read);
    drop(finish_second_write);
    drop(finish_second_read);
    drop(finish_third_write);
    drop(finish_third_read);
    std::fs::remove_file(fixture_path).unwrap();
    std::fs::remove_file(state_path).unwrap();
    pass(&format!(
        "real WASM voluntary exit and three-player GameOver with automatic survivor reward verified for matches {match_id} and {finish_match_id}; terminal_turn_to_match_finished_ms={natural_finish_ms} terminal_turn_to_all_rewards_ms={natural_rewards_ms}"
    ));
}
