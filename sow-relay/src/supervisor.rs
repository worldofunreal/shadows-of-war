//! Coordinate F-Stack generations; match state stays in its owning worker.
use std::ffi::{CString, OsString};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub const FOUNDATION: &str = match option_env!("SOW_RELAY_FOUNDATION_ID") {
    Some(value) => value,
    None => "0000000000000000000000000000000000000000000000000000000000000000",
};
pub const BUILD_GIT: &str = match option_env!("SOW_RELAY_BUILD_GIT") {
    Some(value) => value,
    None => "unknown",
};
pub const DPDK: &str = match option_env!("SOW_DPDK_VERSION") {
    Some(value) => value,
    None => "unavailable",
};

pub mod native {
    unsafe extern "C" {
        pub fn sow_reload_open(
            fd: i32,
            address: usize,
            workers: u32,
            foundation: *const libc::c_char,
            generation: i32,
            queue: i32,
        ) -> i32;
        pub fn sow_reload_address() -> usize;
        pub fn sow_reload_managed() -> i32;
        pub fn sow_reload_generation() -> i32;
        pub fn sow_reload_active() -> i32;
        pub fn sow_reload_paused() -> i32;
        pub fn sow_reload_pause(paused: i32);
        pub fn sow_reload_primary_ready() -> i32;
        pub fn sow_reload_primary_tick();
        pub fn sow_reload_pid(generation: i32, queue: u32) -> u32;
        pub fn sow_reload_ready(generation: i32, queue: u32) -> i32;
        pub fn sow_reload_worker_ready();
        pub fn sow_reload_lobbies(count: u32);
        pub fn sow_reload_tick();
        pub fn sow_reload_empty(generation: i32) -> i32;
        pub fn sow_reload_owner(port: u16) -> i32;
        pub fn sow_reload_claim(port: u16) -> i32;
        pub fn sow_reload_release(port: u16);
        pub fn sow_reload_route(port: u16, target: u16) -> i32;
        pub fn sow_reload_begin() -> i32;
        pub fn sow_reload_park() -> u32;
        pub fn sow_reload_parked(generation: i32, epoch: u32) -> i32;
        pub fn sow_reload_unpark();
        pub fn sow_reload_abort();
        pub fn sow_reload_commit();
        pub fn sow_reload_handover(target: i32) -> i32;
        pub fn sow_reload_clear(generation: i32);
        pub fn sow_reload_reaped(generation: i32, queue: u32, pid: u32);
        pub fn sow_reload_nonce(digest: *const u8, now: u64, ttl: u64) -> i32;
    }
}

pub fn directory() -> PathBuf {
    std::env::var_os("RUNTIME_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/sow-relay"))
}

pub fn worker_socket(pid: u32) -> PathBuf {
    directory().join(format!("worker-{pid}.sock"))
}

pub fn managed() -> bool {
    unsafe { native::sow_reload_managed() != 0 }
}
pub fn generation() -> i32 {
    unsafe { native::sow_reload_generation() }
}
pub fn active() -> i32 {
    unsafe { native::sow_reload_active() }
}
pub fn paused() -> bool {
    unsafe { native::sow_reload_paused() != 0 }
}

fn attach(fd: i32, address: usize, worker_generation: i32, queue: i32) -> io::Result<i32> {
    let foundation = CString::new(FOUNDATION).unwrap();
    let result = unsafe {
        native::sow_reload_open(
            fd,
            address,
            super::expected_worker_count().into(),
            foundation.as_ptr(),
            worker_generation,
            queue,
        )
    };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

pub fn attach_child() -> io::Result<bool> {
    let Ok(role) = std::env::var("SOW_RELAY_ROLE") else {
        return Ok(false);
    };
    if role != "primary" && role != "worker" {
        return Err(io::Error::other("invalid managed relay role"));
    }
    let parse = |key: &str| -> io::Result<usize> {
        std::env::var(key)
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| io::Error::other(format!("missing {key}")))
    };
    let address = parse("SOW_RELAY_SHARED_ADDRESS")?;
    if address == 0 {
        return Err(io::Error::other("missing shared address"));
    }
    let worker_generation = parse("SOW_RELAY_GENERATION")? as i32;
    let queue = if role == "primary" {
        -1
    } else {
        parse("SOW_RELAY_QUEUE")? as i32
    };
    attach(200, address, worker_generation, queue)?;
    Ok(role == "primary")
}

pub unsafe extern "C" fn primary_tick(_: *mut libc::c_void) -> i32 {
    unsafe { native::sow_reload_primary_tick() };
    0
}

pub unsafe extern "C" fn worker_tick(arg: *mut libc::c_void) -> i32 {
    unsafe {
        native::sow_reload_tick();
        fstack_bridge::bridge::driver_cb(arg)
    }
}

pub fn identity() -> serde_json::Value {
    serde_json::json!({"foundation":FOUNDATION,"git":BUILD_GIT,"dpdk":DPDK})
}

/// Only these internal entry points are handled here; ./sow's interface stays
/// unchanged. The upgrade client talks to the owning service over its socket.
pub fn command_line() -> Option<io::Result<()>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--fstack-identity") => {
            println!("{}", identity());
            Some(Ok(()))
        }
        Some("--fstack-upgrade") => Some((|| {
            let binary = args
                .get(2)
                .ok_or_else(|| io::Error::other("upgrade binary required"))?;
            let mut socket = UnixStream::connect(directory().join("upgrade.sock"))?;
            socket.set_read_timeout(Some(Duration::from_secs(120)))?;
            socket.set_write_timeout(Some(Duration::from_secs(5)))?;
            let request = serde_json::to_vec(&serde_json::json!({"binary":binary}))?;
            socket.write_all(&request)?;
            socket.shutdown(std::net::Shutdown::Write)?;
            let mut response = Vec::new();
            socket.take(65537).read_to_end(&mut response)?;
            let value: serde_json::Value = serde_json::from_slice(&response)?;
            if value["ok"] != true {
                return Err(io::Error::other(value.to_string()));
            }
            println!("{value}");
            Ok(())
        })()),
        _ => None,
    }
}

fn same_user(socket: &UnixStream) -> bool {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of_val(&credentials) as libc::socklen_t;
    unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut len,
        ) == 0
            && credentials.uid == libc::geteuid()
    }
}

struct Group {
    generation: i32,
    workers: Vec<Child>,
    failed: bool,
}

impl Group {
    fn healthy(&mut self) -> io::Result<bool> {
        let mut healthy = true;
        for (queue, child) in self.workers.iter_mut().enumerate() {
            if let Some(status) = child.try_wait()? {
                if !self.failed {
                    log::error!("[reload] worker {} exited: {status}", child.id());
                }
                unsafe {
                    native::sow_reload_reaped(self.generation, queue as u32, child.id());
                }
                healthy = false;
            }
        }
        self.failed |= !healthy;
        Ok(healthy)
    }
    fn stop(&mut self) {
        for child in &mut self.workers {
            let _ = child.kill();
        }
        for child in &mut self.workers {
            let _ = child.wait();
            let _ = std::fs::remove_file(worker_socket(child.id()));
        }
    }
}

struct Supervisor {
    fd: i32,
    address: usize,
    config: PathBuf,
    executable: PathBuf,
    primary: Child,
    current: Group,
    draining: Option<Group>,
    degraded: bool,
}

fn spawn(
    binary: &Path,
    config: &Path,
    fd: i32,
    address: usize,
    worker_generation: i32,
    queue: Option<u16>,
) -> io::Result<Child> {
    let mut command = Command::new(binary);
    set_release_library_path(&mut command, binary);
    command
        .arg("--conf")
        .arg(config)
        .arg(format!(
            "--proc-type={}",
            if queue.is_some() {
                "secondary"
            } else {
                "primary"
            }
        ))
        .arg(format!("--proc-id={}", queue.map(|q| q + 1).unwrap_or(0)))
        .env(
            "SOW_RELAY_ROLE",
            if queue.is_some() { "worker" } else { "primary" },
        )
        .env("SOW_RELAY_SHARED_ADDRESS", address.to_string())
        .env("SOW_RELAY_GENERATION", worker_generation.to_string());
    if let Some(q) = queue {
        command.env("SOW_RELAY_QUEUE", q.to_string());
    }
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, 200) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn()
}

fn set_release_library_path(command: &mut Command, binary: &Path) {
    let mut path = binary
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("lib")
        .into_os_string();
    if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
        path.push(OsString::from(":"));
        path.push(existing);
    }
    command.env("LD_LIBRARY_PATH", path);
}

impl Supervisor {
    fn group(&self, binary: &Path, worker_generation: i32) -> io::Result<Group> {
        let mut group = Group {
            generation: worker_generation,
            workers: Vec::new(),
            failed: false,
        };
        for queue in 0..super::expected_worker_count() {
            match spawn(
                binary,
                &self.config,
                self.fd,
                self.address,
                worker_generation,
                Some(queue),
            ) {
                Ok(child) => group.workers.push(child),
                Err(error) => {
                    group.stop();
                    return Err(error);
                }
            }
        }
        Ok(group)
    }

    fn wait_ready(&mut self, group: &mut Group) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if self.primary.try_wait()?.is_some() || !group.healthy()? {
                return Err(io::Error::other("replacement failed before handover"));
            }
            if (0..super::expected_worker_count())
                .all(|q| unsafe { native::sow_reload_ready(group.generation, q.into()) != 0 })
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other("replacement readiness timed out"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn upgrade(&mut self, binary: PathBuf) -> io::Result<serde_json::Value> {
        if self.degraded || !self.current.healthy()? {
            return Err(io::Error::other(
                "serving relay is degraded; preserving its surviving matches",
            ));
        }
        if self.draining.is_some() {
            return Err(io::Error::other("previous matches are still draining"));
        }
        let binary = binary.canonicalize()?;
        let release_root = self
            .executable
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| io::Error::other("missing release directory"))?;
        if binary.parent().and_then(Path::parent) != Some(release_root) {
            return Err(io::Error::other(
                "replacement must be in the relay release directory",
            ));
        }
        let mut probe = Command::new(&binary);
        set_release_library_path(&mut probe, &binary);
        let probe = probe.arg("--fstack-identity").output()?;
        if !probe.status.success() {
            return Err(io::Error::other("replacement identity failed"));
        }
        let value: serde_json::Value = serde_json::from_slice(&probe.stdout)?;
        if value["foundation"] != FOUNDATION || value["dpdk"] != DPDK {
            return Err(io::Error::other(
                "network library or supervisor changed; controlled restart required",
            ));
        }
        let was_paused = paused();
        unsafe {
            native::sow_reload_pause(1);
        }
        let target = unsafe { native::sow_reload_begin() };
        if target < 0 {
            unsafe {
                native::sow_reload_pause(i32::from(was_paused));
            }
            return Err(io::Error::other("reload window already open"));
        }
        let replacement = (|| {
            let mut next = self.group(&binary, target)?;
            if let Err(error) = self.wait_ready(&mut next) {
                next.stop();
                return Err(error);
            }
            let epoch = unsafe { native::sow_reload_park() };
            let deadline = Instant::now() + Duration::from_millis(100);
            while unsafe { native::sow_reload_parked(self.current.generation, epoch) } == 0 {
                if Instant::now() >= deadline {
                    next.stop();
                    return Err(io::Error::other(
                        "old workers did not confirm parking; keeping them",
                    ));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            if unsafe { native::sow_reload_handover(target) } != 0 {
                next.stop();
                return Err(io::Error::other("packet handover refused"));
            }
            unsafe {
                native::sow_reload_unpark();
            }
            Ok(next)
        })();
        match replacement {
            Ok(next) => {
                self.draining = Some(std::mem::replace(&mut self.current, next));
                unsafe {
                    native::sow_reload_pause(i32::from(was_paused));
                }
                log::info!(
                    "[reload] generation {target} serving; previous matches remain with their workers"
                );
                Ok(
                    serde_json::json!({"ok":true,"generation":target,"git":value["git"],"foundation":FOUNDATION}),
                )
            }
            Err(error) => {
                unsafe {
                    native::sow_reload_clear(target);
                    native::sow_reload_abort();
                    native::sow_reload_pause(i32::from(was_paused));
                }
                Err(error)
            }
        }
    }

    fn revert(&mut self) -> io::Result<()> {
        let Some(old) = &mut self.draining else {
            return Err(io::Error::other("no previous generation available"));
        };
        if !old.healthy()? {
            return Err(io::Error::other("previous generation is also degraded"));
        }
        let epoch = unsafe { native::sow_reload_park() };
        let deadline = Instant::now() + Duration::from_millis(100);
        while unsafe { native::sow_reload_parked(self.current.generation, epoch) } == 0 {
            if Instant::now() >= deadline {
                unsafe {
                    native::sow_reload_unpark();
                }
                return Err(io::Error::other("replacement did not confirm parking"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let result = unsafe { native::sow_reload_handover(old.generation) };
        unsafe {
            native::sow_reload_unpark();
        }
        if result != 0 {
            return Err(io::Error::other("reclaiming packet ownership was refused"));
        }
        std::mem::swap(&mut self.current, old);
        log::error!(
            "[reload] replacement failed; previous generation serves again, retaining both generations' unfinished matches"
        );
        Ok(())
    }
}

pub fn run(config: PathBuf) -> io::Result<()> {
    let executable = std::env::current_exe()?.canonicalize()?;
    let fd = attach(-1, 0, 0, -1)?;
    let address = unsafe { native::sow_reload_address() };
    let mut primary = spawn(&executable, &config, fd, address, 0, None)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    while unsafe { native::sow_reload_primary_ready() } == 0 {
        if primary.try_wait()?.is_some() || Instant::now() >= deadline {
            let _ = primary.kill();
            let _ = primary.wait();
            return Err(io::Error::other("resident primary failed readiness"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut supervisor = Supervisor {
        fd,
        address,
        config,
        executable: executable.clone(),
        primary,
        current: Group {
            generation: 0,
            workers: Vec::new(),
            failed: false,
        },
        draining: None,
        degraded: false,
    };
    let mut group = supervisor.group(&executable, 0)?;
    if let Err(error) = supervisor.wait_ready(&mut group) {
        group.stop();
        return Err(error);
    }
    supervisor.current = group;
    let path = directory().join("upgrade.sock");
    // Exclusive service ownership is established by systemd. A leftover socket
    // is removed only after proving that no living coordinator accepts it.
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err(io::Error::other("coordinator already running"));
        }
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    log::info!(
        "[reload] resident primary and {} workers ready",
        super::expected_worker_count()
    );
    loop {
        let serving = supervisor.current.healthy()?;
        if !supervisor.degraded && (!serving || active() != supervisor.current.generation) {
            supervisor.degraded = true;
            unsafe {
                native::sow_reload_pause(1);
            }
            if let Err(error) = supervisor.revert() {
                log::error!("[reload] relay degraded: {error}; surviving workers remain running");
            }
        }
        if let Some(old) = &mut supervisor.draining {
            if !old.healthy()? && !supervisor.degraded {
                supervisor.degraded = true;
                unsafe {
                    native::sow_reload_pause(1);
                }
                log::error!("[reload] draining worker failed; preserving all surviving matches");
            }
            if unsafe { native::sow_reload_empty(old.generation) } != 0 {
                old.stop();
                unsafe {
                    native::sow_reload_clear(old.generation);
                }
                supervisor.draining = None;
                unsafe {
                    native::sow_reload_commit();
                }
                log::info!("[reload] old matches and deliveries finished; handover complete");
            }
        }
        match listener.accept() {
            Ok((mut socket, _)) => {
                socket.set_read_timeout(Some(Duration::from_secs(5)))?;
                socket.set_write_timeout(Some(Duration::from_secs(5)))?;
                if !same_user(&socket) {
                    continue;
                }
                let mut bytes = Vec::new();
                let request = Read::by_ref(&mut socket)
                    .take(65537)
                    .read_to_end(&mut bytes)
                    .and_then(|_| {
                        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(io::Error::from)
                    });
                let result = request.and_then(|value| {
                    if let Some(port) = value["port_owner"].as_u64().and_then(|port| u16::try_from(port).ok()) {
                        return Ok(serde_json::json!({"ok":true,"owner":unsafe { native::sow_reload_owner(port) },
                            "active":active(),"foundation":FOUNDATION}));
                    }
                    let binary = value["binary"].as_str().map(PathBuf::from)
                        .ok_or_else(|| io::Error::other("binary required"))?;
                    supervisor.upgrade(binary)
                });
                let response = result.unwrap_or_else(
                    |error| serde_json::json!({"ok":false,"error":error.to_string()}),
                );
                if let Err(error) = socket.write_all(response.to_string().as_bytes()) {
                    log::error!("[reload] failed to report upgrade result: {error}");
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
}

pub async fn management_listener(listen: &str, port: u16) -> io::Result<tokio::net::TcpListener> {
    if !managed() {
        return tokio::net::TcpListener::bind((listen, port)).await;
    }
    let address: std::net::IpAddr = listen.parse().map_err(io::Error::other)?;
    let socket = if address.is_ipv4() {
        tokio::net::TcpSocket::new_v4()?
    } else {
        tokio::net::TcpSocket::new_v6()?
    };
    socket.set_reuseaddr(true)?;
    socket.set_reuseport(true)?;
    socket.bind(std::net::SocketAddr::new(address, port))?;
    socket.listen(1024)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ManagementRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

async fn forward(
    worker_generation: i32,
    queue: u16,
    method: &str,
    path: &str,
    body: &[u8],
) -> io::Result<(String, serde_json::Value)> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let pid = unsafe { native::sow_reload_pid(worker_generation, queue.into()) };
    if pid == 0 {
        return Err(io::Error::other("match owner is unavailable"));
    }
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut stream = tokio::net::UnixStream::connect(worker_socket(pid)).await?;
        let request = serde_json::to_vec(&ManagementRequest {
            method: method.into(),
            path: path.into(),
            body: body.into(),
        })?;
        if request.len() > super::MAX_MGMT_BODY_BYTES * 4 + 4096 {
            return Err(io::Error::other("management request too large"));
        }
        stream.write_all(&request).await?;
        stream.shutdown().await?;
        let mut response = Vec::new();
        stream
            .take((super::MAX_MGMT_BODY_BYTES * 4 + 4097) as u64)
            .read_to_end(&mut response)
            .await?;
        serde_json::from_slice(&response).map_err(io::Error::from)
    })
    .await
    .map_err(|_| io::Error::other("match owner timed out"))?
}

pub async fn start_management(registry: super::Registry) -> io::Result<()> {
    if !managed() {
        return Ok(());
    }
    let path = worker_socket(std::process::id());
    // PIDs cannot be reused while their previous process is alive. The
    // supervisor also removes these sockets when it reaps its workers.
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let listener = tokio::net::UnixListener::bind(&path)?;
    let counts = registry.clone();
    tokio::spawn(async move {
        loop {
            let count = counts.read().await.len().min(u32::MAX as usize) as u32;
            unsafe {
                native::sow_reload_lobbies(count);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    });
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(value) => value,
                Err(error) => {
                    log::error!("[reload] management accept failed: {error}");
                    continue;
                }
            };
            let registry = registry.clone();
            tokio::spawn(async move {
                if !stream
                    .peer_cred()
                    .is_ok_and(|credential| credential.uid() == unsafe { libc::geteuid() })
                {
                    return;
                }
                let result = tokio::time::timeout(Duration::from_secs(30), async {
                    let mut stream = stream;
                    let mut bytes = Vec::new();
                    (&mut stream)
                        .take((super::MAX_MGMT_BODY_BYTES * 4 + 4097) as u64)
                        .read_to_end(&mut bytes)
                        .await?;
                    if bytes.len() > super::MAX_MGMT_BODY_BYTES * 4 + 4096 {
                        return Err(io::Error::other("management request too large"));
                    }
                    let request: ManagementRequest = serde_json::from_slice(&bytes)?;
                    let response = super::handle_http_local(
                        &registry,
                        &request.method,
                        &request.path,
                        &request.body,
                    )
                    .await;
                    stream.write_all(&serde_json::to_vec(&response)?).await?;
                    Ok::<_, io::Error>(())
                })
                .await;
                if !matches!(result, Ok(Ok(()))) {
                    log::error!("[reload] management request failed: {result:?}");
                }
            });
        }
    });
    unsafe {
        native::sow_reload_worker_ready();
    }
    Ok(())
}

fn stable_status(value: &str) -> &'static str {
    match value {
        "200 OK" => "200 OK",
        "400 Bad Request" => "400 Bad Request",
        "401 Unauthorized" => "401 Unauthorized",
        "404 Not Found" => "404 Not Found",
        "409 Conflict" => "409 Conflict",
        "413 Payload Too Large" => "413 Payload Too Large",
        _ => "503 Service Unavailable",
    }
}

pub async fn route_management(
    registry: &super::Registry,
    method: &str,
    path: &str,
    body: &[u8],
) -> (&'static str, serde_json::Value) {
    if !managed() || path == "/healthz" || path == "/internal/drain" {
        return super::handle_http_local(registry, method, path, body).await;
    }
    let queue = super::WORKER_QUEUE_ID.load(super::Ordering::Relaxed);
    let other = 1 - generation();
    let other_ready = unsafe { native::sow_reload_ready(other, queue.into()) != 0 };
    let request = serde_json::from_slice::<serde_json::Value>(body).unwrap_or_default();
    let lobby = request
        .get("lobby_id")
        .or_else(|| request.get("match_id"))
        .and_then(|id| {
            id.as_u64()
                .or_else(|| id.as_str().and_then(|id| id.parse().ok()))
        });
    let mut destination = generation();
    if path == "/internal/lobby/register" || path == "/internal/metrics" {
        destination = active();
        if let Some(port) = request["relay_port"]
            .as_u64()
            .and_then(|port| u16::try_from(port).ok())
        {
            let owner = unsafe { native::sow_reload_owner(port) };
            if owner >= 0 {
                destination = owner;
            }
        }
    }
    if let Some(id) = lobby {
        if !registry.read().await.contains_key(&id)
            && other_ready
            && path != "/internal/lobby/register"
        {
            match forward(other, queue, "GET", "/internal/lobbies", &[]).await {
                Ok((status, value)) if status == "200 OK" => {
                    if value["lobbies"].as_array().is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| item["lobby_id"].as_u64() == Some(id))
                    }) {
                        destination = other;
                    }
                }
                result => {
                    return (
                        "503 Service Unavailable",
                        serde_json::json!({"error":format!("cannot locate match owner: {result:?}")}),
                    );
                }
            }
        }
    }
    if path == "/internal/lobbies" && other_ready {
        let (status, mut local) = super::handle_http_local(registry, method, path, body).await;
        match forward(other, queue, method, path, body).await {
            Ok((remote_status, value)) if remote_status == "200 OK" => {
                if let (Some(local), Some(remote)) =
                    (local["lobbies"].as_array_mut(), value["lobbies"].as_array())
                {
                    local.extend(remote.iter().cloned());
                } else {
                    return (
                        "503 Service Unavailable",
                        serde_json::json!({"error":"invalid match registry from peer"}),
                    );
                }
                return (status, local);
            }
            result => {
                return (
                    "503 Service Unavailable",
                    serde_json::json!({"error":format!("cannot inspect draining matches: {result:?}")}),
                );
            }
        }
    }
    if destination != generation() {
        return match forward(destination, queue, method, path, body).await {
            Ok((status, value)) => (stable_status(&status), value),
            Err(error) => (
                "503 Service Unavailable",
                serde_json::json!({"error":error.to_string()}),
            ),
        };
    }
    super::handle_http_local(registry, method, path, body).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // Run the REAL C adapter in fresh processes so its library-global mapping
    // cannot contaminate other relay tests. The peer reattaches across exec.
    #[test]
    fn shared_reload_protocol() {
        let stage = std::env::var("SOW_RELOAD_PROTOCOL_TEST").ok();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "supervisor::tests::shared_reload_protocol",
            "--nocapture",
        ]);
        if stage.is_none() {
            assert!(
                command
                    .env("SOW_RELOAD_PROTOCOL_TEST", "root")
                    .status()
                    .unwrap()
                    .success()
            );
            return;
        }
        let digest = [7u8; 32];
        let peer_digest = [9u8; 32];
        let port = 29_102;
        if stage.as_deref() == Some("peer") {
            let address = std::env::var("SOW_RELOAD_PROTOCOL_ADDRESS")
                .unwrap()
                .parse()
                .unwrap();
            attach(200, address, 1, 0).unwrap();
            unsafe {
                assert_eq!(native::sow_reload_owner(port), 0);
                assert_eq!(native::sow_reload_claim(port), -1);
                assert_eq!(native::sow_reload_route(port, 2), -6);
                native::sow_reload_release(port);
                assert_eq!(native::sow_reload_owner(port), 0);
                assert_eq!(native::sow_reload_nonce(digest.as_ptr(), 100, 120), 0);
                assert_eq!(native::sow_reload_nonce(peer_digest.as_ptr(), 100, 120), 1);
            }
            return;
        }
        let fd = attach(-1, 0, 0, 0).unwrap();
        let address = unsafe { native::sow_reload_address() };
        unsafe {
            assert_eq!(native::sow_reload_claim(port), 1);
            assert_eq!(native::sow_reload_claim(port), 0);
            assert_eq!(native::sow_reload_route(port, 2), 2);
            assert_eq!(native::sow_reload_nonce(digest.as_ptr(), 100, 120), 1);
        }
        command
            .env("SOW_RELOAD_PROTOCOL_TEST", "peer")
            .env("SOW_RELOAD_PROTOCOL_ADDRESS", address.to_string());
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 200) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        assert!(command.status().unwrap().success());
        unsafe {
            assert_eq!(native::sow_reload_nonce(peer_digest.as_ptr(), 100, 120), 0);
            native::sow_reload_release(port);
            assert_eq!(native::sow_reload_owner(port), -1);
        }
    }
}
