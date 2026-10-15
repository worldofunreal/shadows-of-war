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
    let (socket, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .unwrap();
    let (mut write, mut read) = socket.split();
    let mut config = sow_core::game_config::GameConfig::default();
    config.map_name = "world".into();
    config.bot_count = 0;
    config.nation_count = 2;
    let command = ClientMessage::JoinWithAuth {
        join: Box::new(sow_core::protocol::JoinPayload {
            name: account["display_name"].as_str().unwrap().into(),
            target_lobby_id: target,
            host_private: target.is_none(),
            build_version: sow_core::protocol::gameplay_build_identity(version),
            clan_tag: String::new(),
            civilization: sow_core::player::Civilization::Rome,
            leader: sow_core::player::Leader::Caesar,
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

async fn exit(http: &reqwest::Client, base: &Url, account: &Value, match_id: u64) -> Value {
    http.post(format!("{}/profile/match-exit", base.as_str().trim_end_matches('/')))
        .json(&json!({"account_id":account["account_id"],"auth_secret":account["auth_secret"],"match_id":match_id.to_string()}))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap()
}

pub(super) async fn verify(url: &Url, base: &Url, accounts_file: Option<&std::path::Path>) {
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
        saved.as_ref().map(|accounts| &accounts[0]),
        "Exit Audit One",
    )
    .await;
    let mut second = profile(
        &http,
        base,
        saved.as_ref().map(|accounts| &accounts[1]),
        "Exit Audit Two",
    )
    .await;
    if let Some(saved) = &saved {
        first["auth_secret"] = saved[0]["auth_secret"].clone();
        second["auth_secret"] = saved[1]["auth_secret"].clone();
    } else if let Some(path) = accounts_file {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(
            serde_json::to_string(&[&first, &second])
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
    }
    let version = std::fs::read_to_string(".version").unwrap();
    let (mut first_write, mut first_read, first_join) =
        join(url, &first, None, version.trim()).await;
    let match_id = first_join.lobby_id;
    let (mut second_write, mut second_read, second_join) =
        join(url, &second, Some(match_id), version.trim()).await;
    for (write, id) in [
        (&mut first_write, first_join.player_id),
        (&mut second_write, second_join.player_id),
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
    other_write.close().await.unwrap();
    drop(other_write);
    drop(other_read);
    let (other_write, mut other_read, _) = ready_relay(
        &relay_url,
        ClientMessage::ReconnectWithTicket {
            lobby_id: match_id,
            player_id: second_start.my_player_id.unwrap(),
            ticket: reconnect,
        },
    )
    .await;
    let still_playing = profile(&http, base, Some(&second), "Exit Audit Two").await;
    assert!(
        still_playing["profile"]["reward_receipts"]
            .get(match_id.to_string())
            .is_none(),
        "temporary disconnect must not settle an exit"
    );
    let forbidden = http.post(format!("{}/profile/match-exit", base.as_str().trim_end_matches('/')))
        .json(&json!({"account_id":second["account_id"],"auth_secret":first["auth_secret"],"match_id":match_id.to_string()}))
        .send().await.unwrap();
    assert_eq!(forbidden.status(), reqwest::StatusCode::UNAUTHORIZED);

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
        ])
        .env("SOW_LIVE_MATCH_EXIT_FILE", &fixture_path)
        .env("WASM_BINDGEN_TEST_TIMEOUT", "120")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(180);
    let status = loop {
        if let Some(status) = client.try_wait().unwrap() {
            break status;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "real WASM return exceeded its deadline"
        );
        assert!(
            matches!(
                recv(&mut other_read, 5).await,
                ServerMessage::Turn(_)
                    | ServerMessage::RelayReconnectTicket { .. }
                    | ServerMessage::Pong { .. }
            ),
            "the second player must continue receiving turns during the first exit"
        );
    };
    assert!(status.success(), "actual WASM client exit failed");
    assert!(matches!(
        recv(&mut other_read, 5).await,
        ServerMessage::Turn(_) | ServerMessage::Pong { .. }
    ));
    let settled_first = profile(&http, base, Some(&first), "Exit Audit One").await;
    for _ in 0..3 {
        let repeated = exit(&http, base, &first, match_id).await;
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
            "sow-web/shell/main_menu.interaction.test.js",
        ])
        .env("SOW_LIVE_RETURN_STATE_FILE", &state_path)
        .env("SOW_LIVE_MATCH_EXIT_FILE", &fixture_path)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "real client state did not finish reward presentation"
    );
    let mut result = exit(&http, base, &second, match_id).await;
    drop(other_write);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    while result["state"] != "confirmed" {
        assert!(
            tokio::time::Instant::now() < deadline,
            "second participant reward never finalized"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        result = exit(&http, base, &second, match_id).await;
    }
    // Personal confirmation can precede the whole replay. Wait for the
    // existing public record to prove that its verification also finished.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let detail: Value = http.get(format!("{}/matches/{match_id}", base.as_str().trim_end_matches('/')))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        assert_eq!(detail["match_id"], match_id.to_string());
        if detail["verified"] == true && detail["provisional"] == false { break; }
        assert!(tokio::time::Instant::now() < deadline, "whole replay verification never finished");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let after_closure = profile(&http, base, Some(&first), "Exit Audit One").await;
    for field in ["xp", "crowns", "laurels"] {
        assert_eq!(after_closure["profile"][field], settled_first["profile"][field],
            "whole-match closure must preserve the first participant's {field}");
    }
    let receipt = match_id.to_string();
    assert_eq!(after_closure["profile"]["reward_receipts"][&receipt]["personal_closure_turn"],
        settled_first["profile"]["reward_receipts"][&receipt]["personal_closure_turn"]);
    std::fs::remove_file(fixture_path).unwrap();
    std::fs::remove_file(state_path).unwrap();
    pass(&format!(
        "real WASM exit, permanent server confirmation, presentation and two-player closure verified for match {match_id}"
    ));
}
