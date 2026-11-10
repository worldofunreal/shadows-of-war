//! Own matchmaking admission. A match owns its game socket and lives independently of this service.

use super::{MAX_MGMT_BODY_BYTES, Registry, handle_http_local, register_body};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep, timeout};

type Response = (&'static str, Value);
const IPC_LIMIT: usize = MAX_MGMT_BODY_BYTES * 4 + 4096;

pub struct Supervisor {
    root: PathBuf,
    registration: Mutex<()>,
}

pub struct MatchClaim {
    _file: File,
    lock: PathBuf,
    socket: PathBuf,
}

impl MatchClaim {
    pub fn listen(&self) -> io::Result<UnixListener> {
        remove_if_exists(&self.socket)?;
        UnixListener::bind(&self.socket)
    }
}

impl Drop for MatchClaim {
    fn drop(&mut self) {
        let _ = remove_if_exists(&self.socket);
        let _ = remove_if_exists(&self.lock);
    }
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn match_lock(file: &File, command: libc::c_int) -> io::Result<libc::c_short> {
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    if unsafe { libc::fcntl(file.as_raw_fd(), command, &mut lock) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(lock.l_type)
}

#[derive(Serialize, Deserialize)]
struct ManagementRequest<'a> {
    method: &'a str,
    path: &'a str,
    body: Vec<u8>,
}

fn error(status: &'static str, message: impl ToString) -> Response {
    (status, json!({"error": message.to_string()}))
}

impl Supervisor {
    pub fn open() -> io::Result<Self> {
        let root = std::env::var_os("RUNTIME_DIRECTORY")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/run/sow-relay"));
        DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&root)?;
        let metadata = root.symlink_metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(io::Error::other(
                "relay runtime directory must be private and owned by the relay user",
            ));
        }
        Ok(Self {
            root,
            registration: Mutex::new(()),
        })
    }

    fn lock_path(&self, id: u64) -> PathBuf {
        self.root.join(format!("match-{id}.lock"))
    }

    fn socket_path(&self, id: u64) -> PathBuf {
        self.root.join(format!("match-{id}.sock"))
    }

    pub fn claim(&self, id: u64) -> io::Result<MatchClaim> {
        let lock = self.lock_path(id);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&lock)?;
        match_lock(&file, libc::F_SETLK)?;
        // A previous owner may have unlinked the lock between open and fcntl.
        if file.metadata()?.ino() != fs::metadata(&lock)?.ino() {
            return Err(io::Error::other("match lock was replaced during startup"));
        }
        Ok(MatchClaim {
            _file: file,
            lock,
            socket: self.socket_path(id),
        })
    }

    fn owns_match(&self, id: u64) -> io::Result<bool> {
        match File::open(self.lock_path(id)) {
            Ok(file) => Ok(match_lock(&file, libc::F_GETLK)? != libc::F_UNLCK as libc::c_short),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn paused(&self) -> io::Result<bool> {
        self.root.join("paused").try_exists()
    }

    async fn forward(
        &self,
        id: u64,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> io::Result<Option<(String, Value)>> {
        if !self.owns_match(id)? {
            return Ok(None);
        }
        let request = serde_json::to_vec(&ManagementRequest {
            method,
            path,
            body: body.to_vec(),
        })?;
        let result = timeout(Duration::from_secs(3), async {
            let mut stream = UnixStream::connect(self.socket_path(id)).await?;
            stream.write_all(&request).await?;
            stream.shutdown().await?;
            let mut response = Vec::new();
            stream
                .take((IPC_LIMIT + 1) as u64)
                .read_to_end(&mut response)
                .await?;
            if response.len() > IPC_LIMIT {
                return Err(io::Error::other("match response is too large"));
            }
            serde_json::from_slice(&response).map_err(io::Error::from)
        })
        .await
        .map_err(io::Error::other)
        .and_then(|result| result);
        match result {
            Ok(response) => Ok(Some(response)),
            Err(_) if !self.owns_match(id)? => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn lobby(&self, id: u64) -> io::Result<Option<Value>> {
        let Some((status, response)) = self.forward(id, "GET", "/internal/lobbies", &[]).await?
        else {
            return Ok(None);
        };
        let lobbies = response["lobbies"]
            .as_array()
            .filter(|_| status == "200 OK")
            .ok_or_else(|| io::Error::other("match did not report its roster"))?;
        if lobbies.is_empty() {
            return Ok(None);
        }
        if lobbies.len() != 1 || lobbies[0]["lobby_id"].as_u64() != Some(id) {
            return Err(io::Error::other("match reported the wrong owner"));
        }
        Ok(Some(lobbies[0].clone()))
    }

    async fn lobbies(&self) -> io::Result<Vec<Value>> {
        let _registration = self.registration.lock().await;
        let mut lobbies = Vec::new();
        for file in fs::read_dir(&self.root)? {
            let file = match file {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let name = file.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_prefix("match-"))
                .and_then(|name| name.strip_suffix(".lock"))
                .and_then(|id| id.parse().ok())
            else {
                continue;
            };
            if let Some(lobby) = self.lobby(id).await? {
                lobbies.push(lobby);
            }
        }
        Ok(lobbies)
    }

    async fn launch(&self, id: u64, port: u16, body: &[u8]) -> io::Result<()> {
        let binary = std::env::var_os("SOW_RELAY_BIN")
            .map(PathBuf::from)
            .map_or_else(std::env::current_exe, Ok)?
            .canonicalize()?;
        let log_dir = std::env::var_os("SOW_RELAY_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.root.clone());
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(log_dir.join("relay-matches.log"))?;
        let mut command = Command::new(binary);
        command
            .arg("--match")
            .stdin(Stdio::piped())
            .stdout(log.try_clone()?)
            .stderr(log)
            .env_remove("SOW_DB_SECRET")
            .env_remove("SOW_RELAY_CONTROL_SECRET");
        command.as_std_mut().process_group(0);
        let mut child = command.spawn()?;
        let ready = timeout(Duration::from_secs(10), async {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| io::Error::other("missing match stdin"))?;
            stdin.write_all(body).await?;
            drop(stdin);
            loop {
                if let Some(status) = child.try_wait()? {
                    return Err(io::Error::other(format!(
                        "match {id} exited during startup ({status})"
                    )));
                }
                if let Ok(Some(lobby)) = self.lobby(id).await {
                    if lobby["relay_port"].as_u64() == Some(port.into())
                        && lobby["proc_id"].as_u64() == child.id().map(u64::from)
                    {
                        return Ok(());
                    }
                    return Err(io::Error::other("another process owns the match"));
                }
                sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .map_err(|_| io::Error::other(format!("match {id} did not become ready")))
        .and_then(|result| result);
        if ready.is_err() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return ready;
        }
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) if status.success() => {}
                result => log::warn!("match {id} exited: {result:?}"),
            }
        });
        Ok(())
    }

    pub async fn request(&self, method: &str, path: &str, body: &[u8]) -> Response {
        self.route(method, path, body)
            .await
            .unwrap_or_else(|err| error("503 Service Unavailable", err))
    }

    async fn route(&self, method: &str, path: &str, body: &[u8]) -> io::Result<Response> {
        let response = match (method, path) {
            ("GET", "/healthz") => {
                self.lobbies().await?;
                ("200 OK", json!({"ok":true}))
            }
            ("GET", "/internal/lobbies") => ("200 OK", json!({"lobbies":self.lobbies().await?})),
            ("GET", "/internal/metrics") => (
                "200 OK",
                json!({
                    "proc_id":std::process::id(),
                    "executable":std::env::current_exe()?,
                    "listener_count":self.lobbies().await?.len(),
                    "lobby_drain_supported":true,
                    "lobby_registration_paused":self.paused()?,
                    "ws_write_timeout_ms":super::ws_write_timeout_ms(),
                }),
            ),
            ("POST", "/internal/drain") => {
                let Some(draining) = serde_json::from_slice::<Value>(body)
                    .ok()
                    .and_then(|value| value["draining"].as_bool())
                else {
                    return Ok(error("400 Bad Request", "draining must be boolean"));
                };
                let _registration = self.registration.lock().await;
                let marker = self.root.join("paused");
                if draining {
                    File::create(marker)?;
                } else {
                    remove_if_exists(&marker)?;
                }
                ("200 OK", json!({"draining":draining}))
            }
            ("POST", "/internal/lobby/register") => {
                let request = match register_body(body) {
                    Ok(request) => request,
                    Err(err) => return Ok(error("400 Bad Request", err)),
                };
                let _registration = self.registration.lock().await;
                if self.paused()? {
                    return Ok(error(
                        "503 Service Unavailable",
                        "relay is paused for deployment",
                    ));
                }
                let id = request.lobby_id;
                let port = request.relay_port;
                let existing = self.lobby(id).await?;
                if let Some(lobby) = &existing {
                    if lobby["closing"] == true
                        || lobby["registration_sha"] != hex::encode(Sha256::digest(body))
                    {
                        return Ok(error(
                            "409 Conflict",
                            "lobby already owns another registration or is closing",
                        ));
                    }
                } else {
                    self.launch(id, port, body).await?;
                }
                (
                    "200 OK",
                    json!({"ok":true,"existing":existing.is_some(),"relay_port":port}),
                )
            }
            ("POST", "/internal/match/result" | "/internal/lobby/close") => {
                let request = serde_json::from_slice::<Value>(body).unwrap_or_default();
                let id = request["match_id"]
                    .as_str()
                    .and_then(|id| id.parse().ok())
                    .or_else(|| request["lobby_id"].as_u64())
                    .filter(|id| *id != 0);
                let Some(id) = id else {
                    return Ok(error("400 Bad Request", "match id required"));
                };
                match self.forward(id, method, path, body).await? {
                    Some((status, value)) => {
                        let status = match status.as_str() {
                            "200 OK" => "200 OK",
                            "400 Bad Request" => "400 Bad Request",
                            "404 Not Found" => "404 Not Found",
                            _ => "503 Service Unavailable",
                        };
                        (status, value)
                    }
                    None if path == "/internal/lobby/close" => {
                        ("200 OK", json!({"ok":true,"removed":false}))
                    }
                    None => ("200 OK", json!({"match_id":id.to_string()})),
                }
            }
            _ => error("404 Not Found", "not found"),
        };
        Ok(response)
    }
}

pub async fn serve_match(listener: UnixListener, registry: Registry, registration_sha: String) {
    let registration_sha: Arc<str> = registration_sha.into();
    while let Ok((mut stream, _)) = listener.accept().await {
        if !stream
            .peer_cred()
            .is_ok_and(|cred| cred.uid() == unsafe { libc::geteuid() })
        {
            continue;
        }
        let registry = registry.clone();
        let registration_sha = registration_sha.clone();
        tokio::spawn(async move {
            let _ = timeout(Duration::from_secs(3), async {
                let mut bytes = Vec::new();
                (&mut stream)
                    .take((IPC_LIMIT + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await?;
                if bytes.len() > IPC_LIMIT {
                    return Err(io::Error::other("match request is too large"));
                }
                let request: ManagementRequest<'_> = serde_json::from_slice(&bytes)?;
                let mut response =
                    handle_http_local(&registry, request.method, request.path, &request.body).await;
                if let Some(lobby) = response
                    .1
                    .get_mut("lobbies")
                    .and_then(|lobbies| lobbies.get_mut(0))
                {
                    lobby["registration_sha"] = json!(&*registration_sha);
                }
                stream.write_all(&serde_json::to_vec(&response)?).await
            })
            .await;
        });
    }
}
