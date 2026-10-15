//! Crash the actual database process after durable intake, then recover its
//! pending result and profile. Only the expensive verifier is delayed here;
//! the production gate also exercises that verifier with a real game.
// The Linux preflight owns this isolated Valkey fixture. The FreeBSD builder
// compiles the shared owners; the live gate exercises the deployed service.
#![cfg(all(feature = "server", target_os = "linux"))]

use axum::{Json, Router, http::StatusCode, routing::post};
use serde_json::{Value, json};
use sow_data::profile::{VerifiedMatchParticipant, VerifiedMatchResult};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::time::Duration;

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn ready(http: &reqwest::Client, base: &str, process: &mut Process) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        assert!(process.0.try_wait().unwrap().is_none(), "database stopped before becoming ready");
        if http.get(base).send().await.is_ok() { return; }
        assert!(tokio::time::Instant::now() < deadline, "database startup timed out");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn accepted_result_survives_process_restart_and_pays_once() {
    let directory = tempfile::tempdir().unwrap();
    let valkey_port = free_port();
    let mut valkey = Process(Command::new("valkey-server")
        .args(["--bind", "127.0.0.1", "--port", &valkey_port.to_string(), "--save", "", "--appendonly", "no"])
        .current_dir(directory.path()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::net::TcpStream::connect(("127.0.0.1", valkey_port)).await.is_err() {
        assert!(valkey.0.try_wait().unwrap().is_none());
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let redis_url = format!("redis://127.0.0.1:{valkey_port}/");
    let http = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(2)).build().unwrap();
    let port = free_port();
    let base = format!("http://127.0.0.1:{port}");
    let available = Arc::new(AtomicBool::new(false));
    let requests = Arc::new(AtomicUsize::new(0));
    let result = Arc::new(std::sync::Mutex::new(VerifiedMatchResult::default()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let verifier_url = format!("http://{}", listener.local_addr().unwrap());
    let route = {
        let (available, requests, result) = (available.clone(), requests.clone(), result.clone());
        post(move || {
            let (available, requests, result) = (available.clone(), requests.clone(), result.clone());
            async move {
                requests.fetch_add(1, Ordering::SeqCst);
                if available.load(Ordering::SeqCst) {
                    (StatusCode::OK, Json(serde_json::to_value(&*result.lock().unwrap()).unwrap()))
                } else { (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"verification deliberately delayed"}))) }
            }
        })
    };
    let verifier = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/internal/replay/verify/{match_id}/participant/{account_id}/{turn}", route)).await.unwrap();
    });
    let start = || Process(Command::new(env!("CARGO_BIN_EXE_sow-database"))
        .env_clear().env("SOW_DB_PORT", port.to_string()).env("SOW_VALKEY_URL", &redis_url)
        .env("SOW_DB_SECRET", "restart-test-database").env("SOW_RELAY_SETTLEMENT_SECRET", "restart-test-relay")
        .env("SOW_REDB_PATH", directory.path().join("metadata.redb"))
        .env("SOW_REPLAY_DIR", directory.path().join("replays"))
        .env("SOW_SERVER_HTTP_URL", &verifier_url)
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let mut database = start();
    ready(&http, &base, &mut database).await;
    let account: Value = http.post(format!("{base}/profile/anonymous")).json(&json!({"display_name":"Restart Audit"}))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let account_id = account["account_id"].as_str().unwrap();
    let initial_xp = account["profile"]["xp"].as_u64().unwrap();
    *result.lock().unwrap() = VerifiedMatchResult { match_id:"42".into(), personal_closure_turn:Some(7),
        participants:vec![VerifiedMatchParticipant { player_id:1, account_id:account_id.into(), leader:Some("Caesar".into()),
            players_defeated:1, ..Default::default() }], ..Default::default() };
    http.post(format!("{base}/match/start")).bearer_auth("restart-test-database")
        .json(&json!({"match_id":"42","player_ids":[account_id],"metadata":{"relay_players":[
            {"player_id":1,"database_account_id":account_id,"leader":"Caesar","is_internal":false}]}}))
        .send().await.unwrap().error_for_status().unwrap();
    let result_url = format!("{base}/internal/match-participant-settle/42/{account_id}/7");
    let accepted = http.post(&result_url).bearer_auth("restart-test-relay").body("durable test prefix")
        .send().await.unwrap();
    assert_eq!(accepted.status(), reqwest::StatusCode::ACCEPTED);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while requests.load(Ordering::SeqCst) == 0 {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // The acknowledged result is still unverified when the actual process dies.
    drop(database);
    available.store(true, Ordering::SeqCst);
    let mut database = start();
    ready(&http, &base, &mut database).await;
    let command = json!({"account_id":account_id,"auth_secret":account["auth_secret"],"match_id":"42"});
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let settled: Value = loop {
        let response: Value = http.post(format!("{base}/profile/match-exit")).json(&command)
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        if response["state"] == "confirmed" { break response; }
        assert!(tokio::time::Instant::now() < deadline, "restarted process did not finish the accepted result");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let expected = sow_data::rewards::calculate(sow_data::rewards::RewardInput { players_defeated:1, ..Default::default() });
    assert_eq!(settled["profile"]["xp"].as_u64().unwrap(), initial_xp + u64::from(expected.xp));
    let checks = requests.load(Ordering::SeqCst);
    for _ in 0..4 {
        let ack = http.post(&result_url).bearer_auth("restart-test-relay").body("durable test prefix").send().await.unwrap();
        assert!(ack.status().is_success());
    }
    assert_eq!(requests.load(Ordering::SeqCst), checks, "accepted duplicates must not trigger another verification");
    drop(database);
    // Remove only this test's volatile cache: the third process must rebuild
    // the same confirmed profile from durable data, without replaying a reward.
    let mut cache = redis::Client::open(redis_url.as_str()).unwrap().get_multiplexed_async_connection().await.unwrap();
    redis::cmd("FLUSHDB").query_async::<()>(&mut cache).await.unwrap();
    let mut database = start();
    ready(&http, &base, &mut database).await;
    let recovered: Value = http.post(format!("{base}/profile/match-exit")).json(&command)
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    assert_eq!(recovered["state"], "confirmed");
    assert_eq!(recovered["profile"]["xp"], settled["profile"]["xp"]);
    assert_eq!(requests.load(Ordering::SeqCst), checks);
    drop(database);
    verifier.abort();
}
