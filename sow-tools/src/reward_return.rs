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
            ranked_1v1: false,
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

async fn relay_control(host: &str, request: Value) -> Value {
    let host = host.to_string();
    let request = request.to_string().replace('\'', "'\\''");
    let command = format!(r#"sudo -u sowrelay python3 -c 'import socket,sys; s=socket.socket(socket.AF_UNIX); s.settimeout(120); s.connect("/run/sow-relay/upgrade.sock"); s.sendall(sys.argv[1].encode()); s.shutdown(socket.SHUT_WR); print(s.makefile("rb").read(65537).decode())' '{request}'"#);
    let output = tokio::task::spawn_blocking(move || Command::new("ssh")
        .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
            "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3", &host, &command])
        .output().unwrap()).await.unwrap();
    assert!(output.status.success(), "relay handover control failed: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("relay control must return JSON")
}

async fn port_owner(host: &str, port: u16) -> Value {
    // Read the coordinator's existing ownership table, not a second registry.
    relay_control(host, json!({"port_owner":port})).await
}

async fn verify_handover(url: &Url, base: &Url, http: &reqwest::Client, version: &str,
    host: &str, binary: &str, accounts: &[Value], old_port: u16,
    first_write: &mut Writer, first_read: &mut Reader, second_write: &mut Writer, second_read: &mut Reader)
{
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let probe = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
    let probes = [probe, probe + 0.001];
    let continuity = async {
        let mut stopped = stopped;
        let mut last_turn: [Option<u64>; 2] = [None, None];
        let mut counts = [0u32; 2];
        let mut pongs = [false; 2];
        let mut last_message = [tokio::time::Instant::now(); 2];
        let mut max_gap = std::time::Duration::ZERO;
        loop {
            let deadline = *last_message.iter().min().unwrap() + std::time::Duration::from_secs(2);
            let (player, message) = tokio::select! {
                _ = &mut stopped => break,
                _ = tokio::time::sleep_until(deadline) => fail("an existing player stopped receiving traffic during handover"),
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
                ServerMessage::Pong { client_time } => {
                    if client_time == probes[player] { pongs[player] = true; }
                }
                ServerMessage::RelayReconnectTicket { .. } => {}
                message => fail(&format!("existing match was interrupted during handover: {message:?}")),
            }
        }
        for received_at in last_message {
            let gap = received_at.elapsed();
            max_gap = max_gap.max(gap);
            assert!(gap < std::time::Duration::from_secs(2),
                "an existing player stopped receiving traffic before handover completed");
        }
        assert!(counts.iter().all(|count| *count >= 5), "both existing players must remain active through handover");
        assert!(pongs.iter().all(|received| *received),
            "both original connections must answer a Ping sent after the replacement starts");
        (counts, max_gap)
    };
    let exercise = async {
        let changed = relay_control(host,
            json!({"binary":binary})).await;
        assert_eq!(changed["ok"], true);
        let new_generation = changed["generation"].as_i64().unwrap();
        let old_owner = port_owner(host, old_port).await;
        assert_eq!(old_owner["owner"].as_i64(), Some(1 - new_generation));
        ws_send(first_write, &ClientMessage::Ping { client_time:probes[0] }).await;
        ws_send(second_write, &ClientMessage::Ping { client_time:probes[1] }).await;
        let (mut host_write, mut host_read, host_join) =
            join_with_match_setup(url, &accounts[0], None, version, 0, true).await;
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
        let _ = host_relay.close().await;
        natural_finish(&mut peer_game, id).await;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        for account in accounts {
            loop {
                let receipt = tokio::time::timeout_at(deadline, result(http, base, account, id)).await
                    .expect("new-generation rewards did not finish");
                if receipt["state"] == "confirmed" { break; }
            }
        }
        let _ = peer_relay.close().await;
        stop.send(()).unwrap();
    };
    let ((counts, max_gap), ()) = tokio::join!(continuity, exercise);
    pass(&format!("Live handover preserved old streams (turns={counts:?}, max_gap_ms={}); replacement served and settled a new match", max_gap.as_millis()));
}

pub(super) async fn verify(url: &Url, base: &Url, accounts_file: Option<&std::path::Path>, handover_host: Option<&str>, handover_binary: Option<&str>) {
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
        verify_handover(url, base, &http, version.trim(), host,
            handover_binary.unwrap_or("/usr/local/libexec/sow-relay/sow-relay"), &handover_accounts,
            first_start.relay_port.unwrap(), &mut other_write, &mut other_read,
            &mut survivor_write, &mut survivor_read).await;
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
    let status = Command::new("bun")
        .args([
            "test",
            "--test-name-pattern=live multiplayer return uses the confirmed client state",
            "sow-web/shell/menu_flow.test.ts",
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

struct LiveRankedSession {
    _queue_write: Writer,
    _queue_read: Reader,
    relay_write: Writer,
    relay_read: Reader,
    start: sow_core::protocol::ServerStartMessage,
}

async fn queue_ranked(url: &Url, account: &Value, version: &str) -> (Writer, Reader, tokio::time::Instant) {
    let (socket, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .unwrap_or_else(|error| fail(&format!("ranked queue connection failed: {error}")));
    let (mut write, read) = socket.split();
    let started_at = tokio::time::Instant::now();
    ws_send(
        &mut write,
        &ClientMessage::JoinWithAuth {
            join: Box::new(sow_core::protocol::JoinPayload {
                name: account["display_name"].as_str().unwrap().into(),
                target_lobby_id: None,
                host_private: false,
                build_version: sow_core::protocol::gameplay_build_identity(version),
                clan_tag: String::new(),
                leader: sow_core::player::Leader::of("caesar"),
                random_leader: true,
                host_config: None,
                password: None,
                ranked_1v1: true,
            }),
            auth: AuthProof {
                provider: "anonymous".into(),
                account_id: Some(account["account_id"].as_str().unwrap().into()),
                token: account["auth_secret"].as_str().unwrap().into(),
            },
        },
    )
    .await;
    (write, read, started_at)
}

async fn ranked_ack(read: &mut Reader) -> (sow_core::protocol::ServerJoinAckMessage, u32) {
    let mut last_elapsed = 0;
    loop {
        match recv(read, 20).await {
            ServerMessage::JoinAck(ack) => return (ack, last_elapsed),
            ServerMessage::RankedSearching { elapsed_secs } => last_elapsed = elapsed_secs,
            ServerMessage::JoinFailed(failure) => {
                fail(&format!("ranked queue rejected test account: {}", failure.reason))
            }
            ServerMessage::LobbyClosed(closed) => {
                fail(&format!("ranked queue closed before match: {}", closed.reason))
            }
            _ => {}
        }
    }
}

async fn cancel_ranked_attempt(write: &mut Writer) {
    let message = bincode::serialize(&ClientMessage::CancelRankedSearch).unwrap();
    let _ = write.send(Message::Binary(message)).await;
}

async fn start_ranked_session(
    url: &Url,
    mut queue_write: Writer,
    mut queue_read: Reader,
    ack: &sow_core::protocol::ServerJoinAckMessage,
) -> LiveRankedSession {
    ws_send(
        &mut queue_write,
        &ClientMessage::MapDownloadProgress {
            lobby_id: ack.lobby_id,
            player_id: ack.player_id,
            progress: 100,
        },
    )
    .await;
    ws_send(
        &mut queue_write,
        &ClientMessage::LobbyReady {
            lobby_id: ack.lobby_id,
            player_id: ack.player_id,
        },
    )
    .await;
    let (start, ticket) = started(&mut queue_read).await;
    assert!(start.config.ranked_1v1, "server did not start a ranked match");
    assert_eq!(start.config.nation_count, 0, "ranked match enabled nations");
    assert_eq!(start.config.max_players, 2, "ranked match is not 1v1");
    assert_eq!(start.players.len(), 2, "ranked match must have two competitors");
    let map = start.config.map_name.to_ascii_lowercase();
    assert!(
        ["oceania", "southeastasia", "northamerica", "world"].contains(&map.as_str()),
        "ranked match used a map outside its private pool: {map}"
    );
    let direct = direct_relay_url(
        url,
        start.relay_host.as_deref().unwrap(),
        start.relay_port.unwrap(),
    )
    .unwrap();
    let (relay_write, relay_read, _) = ready_relay(
        &direct,
        ClientMessage::ReadyWithTicket {
            lobby_id: ack.lobby_id,
            player_id: ack.player_id,
            ticket,
        },
    )
    .await;
    LiveRankedSession {
        _queue_write: queue_write,
        _queue_read: queue_read,
        relay_write,
        relay_read,
        start,
    }
}

async fn public_match_detail(http: &reqwest::Client, base: &Url, match_id: u64) -> Value {
    http.get(format!(
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
    .unwrap()
}

fn curl_config_value(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}

async fn current_ranked_mmr(account: &Value) -> f64 {
    let secret = std::env::var("SOW_DB_SECRET").unwrap_or_else(|_| {
        fail("SOW_DB_SECRET is required to read private matchmaking ratings for ranked verification")
    });
    let host = std::env::var("SOW_CONTROL_HOST").unwrap_or_else(|_| "ionos".into());
    let account_id = account["account_id"].as_str().unwrap();
    let body = serde_json::to_string(&json!({"account_id": account_id})).unwrap();
    let config = format!(
        "header = {}\nheader = \"Content-Type: application/json\"\ndata = {}\n",
        curl_config_value(&format!("Authorization: Bearer {secret}")),
        curl_config_value(&body),
    );
    let output = tokio::task::spawn_blocking(move || {
        let mut child = Command::new("ssh")
            .args([
                "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
                "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3",
                &host,
                "curl --silent --show-error --fail --connect-timeout 10 --max-time 15 --config - http://127.0.0.1:25585/internal/ranked-rating",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(config.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    }).await.unwrap();
    assert!(output.status.success(), "private ranked MMR query failed over SSH: {}", String::from_utf8_lossy(&output.stderr));
    let rating: Value = serde_json::from_slice(&output.stdout).unwrap();
    rating["mu"].as_f64().unwrap_or_else(|| fail("private ranked MMR was missing from the internal rating response"))
}

fn assert_ranked_record(detail: &Value, match_id: u64, expect_bot: bool) {
    assert_eq!(detail["match_id"], match_id.to_string());
    assert_eq!(detail["queue"], "Ranked1v1");
    assert_eq!(detail["mode"], "FFA");
    assert_eq!(detail["verified"], true);
    assert_eq!(detail["rating_eligible"], true);
    let participants = detail["participants"].as_array().unwrap();
    assert_eq!(participants.len(), 2);
    assert_eq!(participants.iter().filter(|p| p["is_bot"] == true).count(), usize::from(expect_bot));
    assert!(participants.iter().all(|participant| {
        let Some(breakdown) = participant["ranked_points_breakdown"].as_object() else { return false; };
        let total: Option<i64> = ["base", "opponent", "league", "streak", "protection", "limit"]
            .iter().map(|key| breakdown[*key].as_i64()).sum();
        participant["rating_before"].is_number()
            && participant["rating_after"].is_number()
            && participant["rating_delta"].is_number()
            && total == participant["rating_delta"].as_i64()
            && participant["verified"] == true
    }), "both ranked ratings must be saved with the verified match");
    if expect_bot {
        assert!((60..=180).contains(&detail["ranked_bot_iq"].as_u64().unwrap_or_default()));
    }
}

async fn require_ranked_receipt(http: &reqwest::Client, base: &Url, account: &Value, match_id: u64) -> Value {
    let receipt = result(http, base, account, match_id).await;
    assert_eq!(receipt["state"], "confirmed", "ranked result was not confirmed");
    let ranked = receipt["ranked_result"].as_object().expect("ranked presentation was not saved");
    assert!(ranked["score_before"].is_number());
    assert!(ranked["score_after"].is_number());
    assert!(ranked["games_played"].as_u64().unwrap_or_default() > 0);
    let breakdown = ranked["points_breakdown"].as_object().expect("ranked point breakdown was not saved");
    let total: i64 = ["base", "opponent", "league", "streak", "protection", "limit"]
        .iter().map(|key| breakdown[*key].as_i64().expect("ranked point breakdown component was not an integer"))
        .sum();
    assert_eq!(total, ranked["rating_delta"].as_i64().unwrap(), "ranked point breakdown must equal the saved change");
    receipt
}

pub(super) async fn verify_ranked(url: &Url, base: &Url, accounts_file: &std::path::Path) {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .unwrap();
    let accounts: Vec<Value> = serde_json::from_slice(&std::fs::read(accounts_file).unwrap()).unwrap();
    assert!(accounts.len() >= 5, "ranked acceptance requires the five persistent audit accounts");
    let audit_accounts = &accounts[..5];
    let version = std::fs::read_to_string(".version").unwrap();

    // Audit identities accumulate MMR across runs; pick the closest live pair.
    let (rating0, rating1, rating2, rating3, rating4) = tokio::join!(
        current_ranked_mmr(&audit_accounts[0]),
        current_ranked_mmr(&audit_accounts[1]),
        current_ranked_mmr(&audit_accounts[2]),
        current_ranked_mmr(&audit_accounts[3]),
        current_ranked_mmr(&audit_accounts[4]),
    );
    let ratings = [rating0, rating1, rating2, rating3, rating4];
    let (first_index, second_index) = (0..audit_accounts.len()).flat_map(|first| {
        (first + 1..audit_accounts.len()).map(move |second| (first, second))
    }).min_by(|(a, b), (c, d)| {
        (ratings[*a] - ratings[*b]).abs().total_cmp(&(ratings[*c] - ratings[*d]).abs())
    }).unwrap();
    let first = &audit_accounts[first_index];
    let second = &audit_accounts[second_index];
    assert!((ratings[first_index] - ratings[second_index]).abs() <= 300.0,
        "audit accounts have no human pair within the ranked search window");
    let first_name = first["display_name"].as_str().unwrap();
    let second_name = second["display_name"].as_str().unwrap();
    let (first_write, mut first_read, _) = queue_ranked(url, first, version.trim()).await;
    let (second_write, mut second_read, _) = queue_ranked(url, second, version.trim()).await;
    let ((first_ack, _), (second_ack, _)) = tokio::join!(
        ranked_ack(&mut first_read),
        ranked_ack(&mut second_read),
    );
    if first_ack.lobby_id != second_ack.lobby_id
        || first_ack.lobby_info.kind != sow_core::protocol::LobbyKind::Ranked1v1
        || second_ack.lobby_info.kind != sow_core::protocol::LobbyKind::Ranked1v1
        || !first_ack.is_private
        || !second_ack.is_private
        || !first_ack.lobby_info.players.iter().any(|player| player.name == first_name)
        || !first_ack.lobby_info.players.iter().any(|player| player.name == second_name)
    {
        let mut first_write = first_write;
        let mut second_write = second_write;
        cancel_ranked_attempt(&mut first_write).await;
        cancel_ranked_attempt(&mut second_write).await;
        fail("ranked acceptance did not pair only its two audit accounts; searches were cancelled");
    }
    let match_id = first_ack.lobby_id;
    let (mut first_session, mut second_session) = tokio::join!(
        start_ranked_session(url, first_write, first_read, &first_ack),
        start_ranked_session(url, second_write, second_read, &second_ack),
    );
    assert_eq!(first_session.start.players.iter().filter(|p| !p.is_ai_controlled).count(), 2);
    ws_send(
        &mut first_session.relay_write,
        &ClientMessage::Gameplay {
            intent: sow_core::protocol::GameplayIntent::Resign,
        },
    )
    .await;
    voluntary_exit_recorded(
        &mut first_session.relay_read,
        first["account_id"].as_str().unwrap(),
        match_id,
    )
    .await;
    let _ = natural_finish(&mut second_session.relay_read, match_id).await;
    let (first_receipt, second_receipt) = tokio::join!(
        require_ranked_receipt(&http, base, first, match_id),
        require_ranked_receipt(&http, base, second, match_id),
    );
    let human_record = public_match_detail(&http, base, match_id).await;
    assert_ranked_record(&human_record, match_id, false);
    assert_eq!(human_record["winner_account_id"], second["account_id"]);
    let human_results = human_record["participants"].as_array().unwrap();
    let first_result = human_results.iter().find(|p| p["account_id"] == first["account_id"]).unwrap();
    let second_result = human_results.iter().find(|p| p["account_id"] == second["account_id"]).unwrap();
    assert_eq!(first_result["won"], false);
    assert_eq!(second_result["won"], true);
    assert!(first_receipt["ranked_result"]["rating_delta"].as_i64().unwrap() <= 0);
    assert!(second_receipt["ranked_result"]["rating_delta"].as_i64().unwrap() >= 0);
    let repeated = require_ranked_receipt(&http, base, first, match_id).await;
    assert_eq!(repeated["ranked_result"], first_receipt["ranked_result"]);
    let _ = first_session.relay_write.close().await;
    let _ = second_session.relay_write.close().await;
    pass(&format!("live ranked human-human match {match_id} was verified and both rating changes were saved once"));
    drop(first_session);
    drop(second_session);

    let (bot_rating0, bot_rating1, bot_rating2, bot_rating3, bot_rating4) = tokio::join!(
        current_ranked_mmr(&audit_accounts[0]),
        current_ranked_mmr(&audit_accounts[1]),
        current_ranked_mmr(&audit_accounts[2]),
        current_ranked_mmr(&audit_accounts[3]),
        current_ranked_mmr(&audit_accounts[4]),
    );
    let bot_ratings = [bot_rating0, bot_rating1, bot_rating2, bot_rating3, bot_rating4];
    let bot_index = bot_ratings.iter().enumerate().min_by(|(a, _), (b, _)| bot_ratings[*a].total_cmp(&bot_ratings[*b]))
        .map(|(index, _)| index).unwrap();
    let bot_account = &audit_accounts[bot_index];
    let bot_rating = bot_ratings[bot_index];
    let (bot_queue_write, mut bot_queue_read, queued_at) = queue_ranked(url, bot_account, version.trim()).await;
    let (bot_ack, reported_wait) = ranked_ack(&mut bot_queue_read).await;
    let bot_candidate_count = bot_ack.lobby_info.players.iter()
        .filter(|player| player.download_progress == 100)
        .count();
    if bot_candidate_count != 1 {
        let mut bot_queue_write = bot_queue_write;
        cancel_ranked_attempt(&mut bot_queue_write).await;
        fail("ranked bot acceptance was paired with another person; search cancelled before map loading");
    }
    assert!(queued_at.elapsed() > std::time::Duration::from_secs(3));
    assert!(reported_wait >= 3, "server did not report waiting through the three-second bot threshold");
    assert_eq!(bot_ack.lobby_info.kind, sow_core::protocol::LobbyKind::Ranked1v1);
    let bot_match_id = bot_ack.lobby_id;
    let mut bot_session = start_ranked_session(url, bot_queue_write, bot_queue_read, &bot_ack).await;
    assert_eq!(bot_session.start.players.iter().filter(|p| !p.is_ai_controlled).count(), 1);
    assert_eq!(bot_session.start.players.iter().filter(|p| p.is_ai_controlled).count(), 1);
    let expected_bot_iq = (60..=180).step_by(20).min_by_key(|iq| {
        ((1000.0 + f64::from(*iq - 60) * 20.0) - bot_rating).abs() as u32
    }).unwrap_or(60);
    assert_eq!(bot_session.start.config.ranked_bot_iq, Some(expected_bot_iq),
        "bot IQ did not match the audit player's current MMR");
    ws_send(
        &mut bot_session.relay_write,
        &ClientMessage::Gameplay {
            intent: sow_core::protocol::GameplayIntent::Resign,
        },
    )
    .await;
    voluntary_exit_recorded(
        &mut bot_session.relay_read,
        bot_account["account_id"].as_str().unwrap(),
        bot_match_id,
    )
    .await;
    let bot_receipt = require_ranked_receipt(&http, base, bot_account, bot_match_id).await;
    let bot_record = public_match_detail(&http, base, bot_match_id).await;
    assert_ranked_record(&bot_record, bot_match_id, true);
    let participants = bot_record["participants"].as_array().unwrap();
    let bot_result = participants.iter().find(|participant| participant["is_bot"] == true).unwrap();
    assert_eq!(bot_record["winner_account_id"], bot_result["account_id"]);
    let human_result = participants.iter().find(|participant| participant["account_id"] == bot_account["account_id"]).unwrap();
    assert_eq!(human_result["won"], false);
    assert!(bot_receipt["ranked_result"]["rating_delta"].as_i64().unwrap() <= 0);
    let bot_id = bot_result["account_id"].as_str().unwrap();
    let bot_profile: Value = http
        .get(format!("{}/profiles/{bot_id}", base.as_str().trim_end_matches('/')))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bot_profile["is_bot"], true, "ranked bot profile was not public");
    assert_eq!(
        bot_profile["ranked_bot_iq"],
        bot_record["ranked_bot_iq"],
        "ranked bot profile did not preserve its configured IQ"
    );
    assert!(bot_profile["matches_played"].as_u64().unwrap_or_default() > 0);
    assert!(bot_profile["wins"].as_u64().unwrap_or_default() > 0);
    assert!(bot_profile["ratings"].as_array().unwrap().iter().any(|rating| {
        rating["queue"] == "Ranked1v1" && rating["games_played"].as_u64().unwrap_or_default() > 0
    }));
    assert_eq!(
        bot_profile["recent_matches"][0]["match_id"],
        bot_match_id.to_string(),
        "ranked bot profile does not show its latest duel"
    );
    let bot_history: Value = http
        .get(format!(
            "{}/profiles/{bot_id}/matches?queue=Ranked1v1&limit=1",
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
    assert_eq!(bot_history["items"][0]["match_id"], bot_match_id.to_string());
    let _ = bot_session.relay_write.close().await;
    pass(&format!("live ranked human-bot match {bot_match_id} waited over three seconds and saved both ratings, bot IQ, profile statistics, and history"));
    drop(bot_session);
}
