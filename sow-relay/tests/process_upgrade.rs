use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sow_core::protocol::{ClientMessage, ServerMessage};
use sow_data::profile::RelayMatchProgress;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::net::Ipv4Addr;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

const CONTROL: &str = "test-control";
const SETTLEMENT: &str = "test-settlement";
const TICKET: &str = "valid-match-ticket";
type Game = WebSocketStream<TcpStream>;

struct Fixture {
    root: tempfile::TempDir,
    manager: Option<Child>,
    port: u16,
    http: reqwest::Client,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new().prefix("relay-").tempdir().unwrap();
        let path = root.path();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        let key = path.join("key.pem");
        let cert = path.join("cert.pem");
        assert!(
            StdCommand::new("openssl")
                .args([
                    "req",
                    "-x509",
                    "-newkey",
                    "rsa:2048",
                    "-nodes",
                    "-days",
                    "1",
                    "-subj",
                    "/CN=localhost",
                    "-addext",
                    "subjectAltName=DNS:localhost",
                    "-addext",
                    "basicConstraints=critical,CA:FALSE",
                    "-keyout"
                ])
                .arg(&key)
                .arg("-out")
                .arg(&cert)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        let pem = fs::read(&cert).unwrap();
        let socket = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        let http = reqwest::Client::builder()
            .no_proxy()
            .resolve("localhost", (Ipv4Addr::LOCALHOST, port).into())
            .timeout(Duration::from_secs(15))
            .add_root_certificate(reqwest::Certificate::from_pem(&pem).unwrap())
            .build()
            .unwrap();
        Self {
            root,
            manager: None,
            port,
            http,
        }
    }

    fn stage(&self, release: &str) -> PathBuf {
        let path = self.root.path().join(release);
        fs::create_dir(&path).unwrap();
        let binary = path.join("sow-relay");
        fs::copy(env!("CARGO_BIN_EXE_sow-relay"), &binary).unwrap();
        // The ELF image is identical, with a distinct artifact digest per release.
        OpenOptions::new()
            .append(true)
            .open(&binary)
            .unwrap()
            .write_all(release.as_bytes())
            .unwrap();
        let next = self.root.path().join("next");
        symlink(&binary, &next).unwrap();
        fs::rename(next, self.root.path().join("current")).unwrap();
        binary
    }

    async fn start(&mut self, binary: &Path) {
        if let Some(mut old) = self.manager.take() {
            old.kill().await.unwrap();
            old.wait().await.unwrap();
        }
        let root = self.root.path();
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("manager.log"))
            .unwrap();
        self.manager = Some(
            Command::new(binary)
                .kill_on_drop(true)
                .env("SOW_RELAY_CONTROL_SECRET", CONTROL)
                .env("SOW_RELAY_SETTLEMENT_SECRET", SETTLEMENT)
                .env("SOW_RELAY_BIN", root.join("current"))
                .env("SOW_RELAY_TLS_CERT", root.join("cert.pem"))
                .env("SOW_RELAY_TLS_KEY", root.join("key.pem"))
                .env("SOW_RELAY_BASE_MGMT_PORT", self.port.to_string())
                .env("SOW_DB_URL", "http://127.0.0.1:25585")
                .env("RUNTIME_DIRECTORY", root)
                .env("RUST_LOG", "info")
                .stdout(log.try_clone().unwrap())
                .stderr(log)
                .spawn()
                .unwrap(),
        );
        timeout(Duration::from_secs(10), async {
            let url = format!("https://localhost:{}/healthz", self.port);
            loop {
                if self
                    .http
                    .get(&url)
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
                {
                    break;
                }
                assert!(
                    self.manager.as_mut().unwrap().try_wait().unwrap().is_none(),
                    "manager exited: {}",
                    fs::read_to_string(root.join("manager.log")).unwrap()
                );
                sleep(Duration::from_millis(30)).await;
            }
        })
        .await
        .unwrap();
    }

    async fn request(&self, method: &str, path: &str, body: &str) -> (u16, Value) {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let mut bytes = [0; 16];
        OsRng.fill_bytes(&mut bytes);
        let nonce = hex::encode(bytes);
        let mut mac = Hmac::<Sha256>::new_from_slice(CONTROL.as_bytes()).unwrap();
        mac.update(format!("{method}\n{path}\n{time}\n{nonce}\n{body}").as_bytes());
        let response = self
            .http
            .request(
                method.parse().unwrap(),
                format!("https://localhost:{}{path}", self.port),
            )
            .header("X-SOW-Timestamp", time)
            .header("X-SOW-Nonce", nonce)
            .header("X-SOW-Signature", hex::encode(mac.finalize().into_bytes()))
            .body(body.to_owned())
            .send()
            .await
            .unwrap();
        (response.status().as_u16(), response.json().await.unwrap())
    }

    async fn register(&self, id: u64, port: u16) -> (u16, Value) {
        self.request(
            "POST",
            "/internal/lobby/register",
            &Self::registration(id, port).to_string(),
        )
        .await
    }

    fn registration(id: u64, port: u16) -> Value {
        json!({
            "lobby_id":id,"relay_port":port,"ticket_expires_at":4_102_444_800_u64,"tick_number":0,"tick_rate_ms":50,
            "players":[{"player_id":1,"name":"Relay test","database_account_id":"fixture",
                "relay_ticket_digest":hex::encode(Sha256::digest(TICKET.as_bytes()))}],
        })
    }

    async fn lobbies(&self) -> Vec<Value> {
        let (status, body) = self.request("GET", "/internal/lobbies", "").await;
        assert_eq!(status, 200, "{body}");
        body["lobbies"].as_array().unwrap().clone()
    }

    async fn connect(&self, port: u16, msg: ClientMessage) -> (Game, String) {
        let tcp = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        let (mut game, _) =
            tokio_tungstenite::client_async(format!("ws://localhost:{port}/ws/"), tcp)
                .await
                .unwrap();
        send(&mut game, msg).await;
        loop {
            let message = next(&mut game).await;
            if let ServerMessage::RelayReconnectTicket { ticket, .. } = message {
                return (game, ticket);
            }
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(manager) = self.manager.as_mut() {
            let _ = manager.start_kill();
        }
        // Clean up only processes holding claims in this private test directory.
        for entry in fs::read_dir(self.root.path()).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "lock") {
                continue;
            }
            if let Ok(file) = File::open(path) {
                let mut lock: libc::flock = unsafe { std::mem::zeroed() };
                lock.l_type = libc::F_WRLCK as _;
                lock.l_whence = libc::SEEK_SET as _;
                unsafe {
                    if libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) == 0
                        && lock.l_type != libc::F_UNLCK as libc::c_short
                        && lock.l_pid > 1
                    {
                        libc::kill(lock.l_pid, libc::SIGTERM);
                    }
                }
            }
        }
    }
}

async fn send(game: &mut Game, msg: ClientMessage) {
    game.send(Message::Binary(bincode::serialize(&msg).unwrap()))
        .await
        .unwrap();
}

async fn next(game: &mut Game) -> ServerMessage {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Message::Binary(bytes) = game.next().await.unwrap().unwrap() {
                return bincode::deserialize(&bytes).unwrap();
            }
        }
    })
    .await
    .unwrap()
}

async fn turn(game: &mut Game) -> u64 {
    loop {
        if let ServerMessage::Turn(message) = next(game).await {
            return message.turn.turn_number;
        }
    }
}

fn reserve_port() -> (std::net::TcpListener, u16) {
    (25592..=26500)
        .find_map(|port| {
            std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
                .ok()
                .map(|socket| (socket, port))
        })
        .expect("game port range exhausted")
}

async fn record(socket: TcpStream, finished: Arc<Mutex<Vec<String>>>) {
    let mut socket = tokio::io::BufReader::new(socket);
    let mut size = 0;
    let mut authorized = false;
    loop {
        let mut line = String::new();
        if socket.read_line(&mut line).await.unwrap() == 0 {
            return;
        }
        if line == "\r\n" {
            break;
        }
        let line = line.trim_end();
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                size = value.trim().parse().unwrap();
            }
            if name.eq_ignore_ascii_case("authorization") {
                authorized = value.trim() == "Bearer test-settlement";
            }
        }
    }
    assert!(
        authorized,
        "match did not use the dedicated settlement secret"
    );
    let mut bytes = vec![0; size];
    socket.read_exact(&mut bytes).await.unwrap();
    let batch: RelayMatchProgress = bincode::deserialize(&bytes).unwrap();
    assert!(!batch.recording_failed);
    if batch.finished {
        finished.lock().unwrap().push(batch.match_id.clone());
    }
    let body =
        json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":"durable-test-ack"})
            .to_string();
    socket.write_all(format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()
    ).as_bytes()).await.unwrap();
}

#[tokio::test]
async fn matches_keep_their_process_and_socket_through_a_control_upgrade() {
    let database = TcpListener::bind((Ipv4Addr::LOCALHOST, 25585))
        .await
        .expect("fixture needs loopback recorder port");
    let finished = Arc::new(Mutex::new(Vec::new()));
    let receipts = finished.clone();
    let recorder = tokio::spawn(async move {
        loop {
            let (socket, _) = database.accept().await.unwrap();
            tokio::spawn(record(socket, receipts.clone()));
        }
    });

    let mut f = Fixture::new();
    let original = f.stage("release-a");
    f.start(&original).await;
    let unauthorized = f
        .http
        .get(format!("https://localhost:{}/internal/lobbies", f.port))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status().as_u16(), 401);
    let (occupied, port_a) = reserve_port();
    assert_ne!(f.register(1, port_a).await.0, 200);
    assert!(f.lobbies().await.is_empty());
    drop(occupied);

    assert_eq!(f.register(1, port_a).await.0, 200);
    let old = f.lobbies().await.remove(0);
    #[cfg(target_os = "freebsd")]
    {
        let pid = old["proc_id"].as_u64().unwrap().to_string();
        let threads = StdCommand::new("procstat")
            .args(["-t", &pid])
            .output()
            .unwrap();
        assert!(threads.status.success());
        assert_eq!(String::from_utf8_lossy(&threads.stdout).lines().count(), 2);
    }
    assert_eq!(f.register(1, port_a).await.1["existing"], true);
    let mut conflict = Fixture::registration(1, port_a);
    conflict["tick_rate_ms"] = json!(60);
    assert_eq!(
        f.request("POST", "/internal/lobby/register", &conflict.to_string())
            .await
            .0,
        409
    );
    assert_eq!(f.lobbies().await[0]["proc_id"], old["proc_id"]);
    let (mut a, ticket) = f
        .connect(
            port_a,
            ClientMessage::ReadyWithTicket {
                lobby_id: 1,
                player_id: 1,
                ticket: TICKET.into(),
            },
        )
        .await;
    let before = turn(&mut a).await;
    assert_eq!(
        f.request("POST", "/internal/drain", r#"{"draining":true}"#)
            .await
            .0,
        200
    );
    let candidate = f.stage("release-b");
    let ((), ()) = tokio::join!(f.start(&candidate), async {
        let mut last = before;
        for _ in 0..12 {
            let observed = turn(&mut a).await;
            assert_eq!(observed, last + 1);
            last = observed;
        }
    });
    let owner = f.lobbies().await.remove(0);
    assert_eq!(owner["proc_id"], old["proc_id"]);
    assert_eq!(owner["executable"], original.to_str().unwrap());
    let (available, port_b) = reserve_port();
    drop(available);
    assert_eq!(
        f.register(2, port_b).await.0,
        503,
        "pause must survive service restart"
    );
    assert_eq!(
        f.request("POST", "/internal/drain", r#"{"draining":false}"#)
            .await
            .0,
        200
    );
    assert_eq!(f.register(2, port_b).await.0, 200);
    let new = f
        .lobbies()
        .await
        .into_iter()
        .find(|entry| entry["lobby_id"] == 2)
        .unwrap();
    assert_eq!(new["executable"], candidate.to_str().unwrap());
    assert_ne!(new["proc_id"], old["proc_id"]);

    a.close(None).await.unwrap();
    let (mut a, rotated) = f
        .connect(
            port_a,
            ClientMessage::ReconnectWithTicket {
                lobby_id: 1,
                player_id: 1,
                ticket: ticket.clone(),
            },
        )
        .await;
    assert_ne!(ticket, rotated);
    let (mut b, _) = f
        .connect(
            port_b,
            ClientMessage::ReadyWithTicket {
                lobby_id: 2,
                player_id: 1,
                ticket: TICKET.into(),
            },
        )
        .await;
    for game in [&mut a, &mut b] {
        send(game, ClientMessage::Leave {}).await;
        loop {
            if matches!(next(game).await, ServerMessage::ExitRecorded { .. }) {
                break;
            }
        }
        let _ = game.close(None).await;
    }
    timeout(Duration::from_secs(35), async {
        while fs::read_dir(f.root.path())
            .unwrap()
            .flatten()
            .any(|file| file.path().extension().is_some_and(|ext| ext == "lock"))
        {
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("match processes did not exit after their durable final acknowledgment");
    assert!(f.lobbies().await.is_empty());
    let mut receipts = finished.lock().unwrap().clone();
    receipts.sort();
    assert_eq!(receipts, ["1", "2"]);
    recorder.abort();
}
