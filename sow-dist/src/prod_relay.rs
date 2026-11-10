use super::*;
use anyhow::ensure;
use std::net::Ipv4Addr;

pub(super) const NAME: &str = "shadowsofwar.io";
pub(super) const PORT: u16 = 8083;
pub(super) const JAIL_ROOT: &str = "/zroot/jails/sow-server";
const LOCAL: Ipv4Addr = Ipv4Addr::LOCALHOST;
const TLS_LINEAGE: &str = "/usr/local/etc/letsencrypt/live/shadowsofwar.io";
const TLS_HOOK: &str = "/usr/local/etc/letsencrypt/renewal-hooks/deploy/50-sow-relay";

pub(super) struct Control {
    ip: Ipv4Addr,
    workers: Vec<(String, u16)>,
}

impl Control {
    pub fn local() -> Self {
        Self {
            ip: LOCAL,
            workers: vec![(NAME.into(), PORT)],
        }
    }

    pub fn deployed(config: &Config) -> Result<Self> {
        let manifest: serde_json::Value = serde_json::from_str(&output(
            "ssh",
            &[&config.control_host, "cat /srv/sow/current/release.json"],
        )?)?;
        if manifest["relay"]["backend"] == "tokio" {
            return Ok(Self::local());
        }
        // The outgoing relay keeps its deployed pins even after its public DNS changes.
        let settings = output(
            "ssh",
            &[
                &config.control_host,
                r#"sudo awk -F= '
$1=="SOW_RELAY_WORKERS" || $1=="SOW_RELAY_MGMT_RESOLVE_IP" {print}
' /zroot/jails/sow-server/usr/local/etc/sow/sow.env"#,
            ],
        )?;
        let settings = parse_components(&settings);
        let ip = settings
            .get("SOW_RELAY_MGMT_RESOLVE_IP")
            .context("installed Azure management pin missing")?
            .parse()?;
        let workers = settings
            .get("SOW_RELAY_WORKERS")
            .context("installed Azure catalog missing")?
            .split(',')
            .map(|entry| {
                let [_, host, port] = entry.split(':').collect::<Vec<_>>()[..] else {
                    bail!("invalid installed relay catalog entry");
                };
                Ok((host.to_string(), port.parse()?))
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            ip != LOCAL && !workers.is_empty(),
            "invalid Azure control route"
        );
        Ok(Self { ip, workers })
    }

    pub fn is_local(&self) -> bool {
        self.ip == LOCAL
    }

    pub fn request(
        &self,
        config: &Config,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let timestamp = output("ssh", &[&config.control_host, "date -u +%s"])?;
        let secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
        self.workers
            .iter()
            .map(|(host, port)| {
                let mut random = [0u8; 16];
                rand::rngs::OsRng.fill_bytes(&mut random);
                let nonce = hex::encode(random);
                let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
                    .map_err(|_| anyhow::anyhow!("invalid relay control secret"))?;
                mac.update(format!("{method}\n{path}\n{timestamp}\n{nonce}\n").as_bytes());
                mac.update(body.as_bytes());
                let signature = hex::encode(mac.finalize().into_bytes());
                let body = if method == "POST" {
                    format!(
                        " -H 'Content-Type: application/json' --data-binary {}",
                        shell_quote(body)
                    )
                } else {
                    String::new()
                };
                let command = format!(
                    "curl --noproxy '*' -fsS --max-time 25 --resolve {} -H {} -H {} -H {}{} {}",
                    shell_quote(&format!("{host}:{port}:{}", self.ip)),
                    shell_quote(&format!("X-SOW-Timestamp: {timestamp}")),
                    shell_quote(&format!("X-SOW-Nonce: {nonce}")),
                    shell_quote(&format!("X-SOW-Signature: {signature}")),
                    body,
                    shell_quote(&format!("https://{host}:{port}{path}")),
                );
                let response = output("ssh", &[&config.control_host, &command])
                    .with_context(|| format!("relay management {method} {path} on port {port}"))?;
                serde_json::from_str(&response).context("invalid relay management response")
            })
            .collect()
    }

    pub fn drain(&self, config: &Config, paused: bool) -> Result<()> {
        let responses = self.request(
            config,
            "POST",
            "/internal/drain",
            &json!({"draining": paused}).to_string(),
        )?;
        ensure!(
            responses.iter().all(|reply| reply["draining"] == paused),
            "relay did not confirm admission state"
        );
        Ok(())
    }

    pub fn lobbies(&self, config: &Config) -> Result<Vec<serde_json::Value>> {
        let mut lobbies = Vec::new();
        for reply in self.request(config, "GET", "/internal/lobbies", "")? {
            lobbies.extend(
                reply["lobbies"]
                    .as_array()
                    .context("relay did not report lobbies")?
                    .iter()
                    .cloned(),
            );
        }
        Ok(lobbies)
    }

    pub fn metrics(&self, config: &Config) -> Result<serde_json::Value> {
        let replies = self.request(config, "GET", "/internal/metrics", "")?;
        ensure!(
            replies
                .iter()
                .all(|reply| reply["lobby_drain_supported"] == true),
            "relay does not support admission pause"
        );
        replies.into_iter().next().context("relay metrics missing")
    }
}

fn run_root(config: &Config, script: &str, args: &[&str]) -> Result<String> {
    let args = args
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let command = format!(
        "sudo -n /bin/sh -eu -c {} sow-relay {args}",
        shell_quote(script)
    );
    output("ssh", &[&config.control_host, &command])
}

pub(super) fn settings(dir: &Path) -> Result<u64> {
    let number = |key, default: u64| -> Result<u64> {
        let value = env_or(key, &default.to_string())
            .parse::<u64>()
            .with_context(|| format!("{key} must be a positive integer"))?;
        ensure!(value > 0 && value <= u32::MAX as u64, "invalid {key}");
        Ok(value)
    };
    let total = number("SOW_RELAY_MAX_CONNECTIONS", 32768)?;
    let per_ip = number("SOW_RELAY_MAX_CONNECTIONS_PER_IP", 4096)?;
    let rate = number("SOW_RELAY_HANDSHAKES_PER_IP", 512)?;
    let timeout = number("SOW_WS_WRITE_TIMEOUT_MS", 15000)?;
    ensure!(per_ip <= total, "relay per-IP limit exceeds the host limit");
    fs::write(
        dir.join("relay.env"),
        format!(
            "RUST_LOG=info\nSOW_RELAY_BASE_MGMT_PORT={PORT}\nSOW_DB_URL=http://127.0.0.1:25585\nSOW_RELAY_TLS_CERT=/usr/local/etc/sow/relay-tls/current/relay.crt\nSOW_RELAY_TLS_KEY=/usr/local/etc/sow/relay-tls/current/relay.key\nSOW_RELAY_MAX_CONNECTIONS={total}\nSOW_RELAY_MAX_CONNECTIONS_PER_IP={per_ip}\nSOW_RELAY_HANDSHAKES_PER_IP={rate}\nSOW_WS_WRITE_TIMEOUT_MS={timeout}\n",
        ),
    )?;
    Ok(timeout)
}

const BACKUP_PATHS: &str = "srv/sow/current
usr/local/etc/sow/sow.env
usr/local/etc/nginx/conf.d/shadowsofwar.io.conf
usr/local/etc/nginx/conf.d/00-00-security.conf
usr/local/etc/nginx/snippets/proxy-headers.conf
usr/local/etc/nginx/snippets/proxy-websocket.conf
usr/local/etc/nginx/snippets/security-headers.conf
usr/local/etc/nginx/snippets/ssl-params.conf
usr/local/etc/letsencrypt/renewal-hooks/deploy/50-sow-relay
etc/periodic.conf.local
etc/rc.conf
zroot/jails/sow-server/etc/resolv.conf
zroot/jails/sow-server/usr/local/etc/sow/sow.env
zroot/jails/sow-server/usr/local/etc/sow/relay.env
zroot/jails/sow-server/usr/local/etc/rc.d/sow_server
zroot/jails/sow-server/usr/local/etc/rc.d/sow_relay
zroot/jails/sow-server/etc/rc.conf.d/sow_relay
zroot/jails/sow-database/usr/local/etc/sow/sow.env
zroot/jails/sow-database/usr/local/etc/rc.d/sow_database";

pub(super) struct Backup(String);

impl Backup {
    pub fn capture(config: &Config, legacy: bool) -> Result<Self> {
        let location = run_root(
            config,
            r#"
umask 077
backup=$(mktemp -d /srv/sow/.relay-rollback.XXXXXX)
printf '%s\n' "$1" | while IFS= read -r file; do
    if [ -e "/$file" ] || [ -L "/$file" ]; then printf '%s\n' "$file"; fi
done > "$backup/paths"
tar -C / -cpf "$backup/files.tar" -T "$backup/paths"
readlink /srv/sow/current > "$backup/current"
if [ "$2" = yes ]; then touch "$backup/azure"; fi
printf '%s\n' "$backup"
"#,
            &[BACKUP_PATHS, yes(legacy)],
        )?;
        ensure!(
            location.starts_with("/srv/sow/.relay-rollback."),
            "invalid rollback directory"
        );
        println!("  previous production settings saved at {location}");
        Ok(Self(location))
    }

    pub fn remove(&self, config: &Config) -> Result<()> {
        run_root(config, r#"rm -rf "$1""#, &[&self.0])?;
        Ok(())
    }

    pub fn restore(&self, config: &Config, plan: &ComponentPlan, legacy: bool) -> Result<()> {
        run_root(
            config,
            r#"
backup=$1
test -r "$backup/files.tar"
if [ "$3" = yes ]; then
    if jexec sow-server service sow_relay onestatus >/dev/null 2>&1; then
        jexec sow-server service sow_relay stop
    fi
    rm -f /zroot/jails/sow-server/var/run/sow-relay/paused
fi
printf '%s\n' "$2" | while IFS= read -r file; do
    [ "$file" = srv/sow/current ] || rm -f "/$file"
done
tar -C / -xpf "$backup/files.tar" --exclude srv/sow/current
link=/srv/sow/.rollback.$$
ln -s "$(cat "$backup/current")" "$link"
mv -hf "$link" /srv/sow/current
nginx -t
service nginx reload
if [ "$4" = yes ]; then
    jexec sow-database service sow_database restart
    i=0
    until curl -fsS --max-time 3 http://127.0.0.1:25585/healthz >/dev/null; do
        i=$((i + 1)); [ "$i" -lt 60 ]; sleep 1
    done
fi
if [ "$5" = yes ]; then jexec sow-server service sow_server restart; fi
if [ "$3" = no ] && [ "$6" = yes ]; then jexec sow-server service sow_relay restart; fi
"#,
            &[
                &self.0,
                BACKUP_PATHS,
                yes(legacy),
                yes(plan.database || plan.ops),
                yes(plan.server || plan.maps || plan.ops),
                yes(plan.relay),
            ],
        )?;
        Ok(())
    }
}

fn yes(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub(super) fn verify_control_port_available(config: &Config) -> Result<()> {
    let command = format!(
        "sockstat -46l -p {PORT} | awk 'NR > 1 {{ print; occupied = 1 }} END {{ exit occupied }}'"
    );
    run("ssh", &[&config.control_host, &command], None)
        .with_context(|| format!("IONOS relay control port {PORT} is already in use"))
}

pub(super) fn prepare_host(config: &Config) -> Result<()> {
    let ops = format!(
        "{}/release/relay-ops",
        config.remote_stage.trim_end_matches('/')
    );
    run_root(config, r#"
ops=$1
jail=/zroot/jails/sow-server
if ! jexec sow-server pw groupshow sowrelay >/dev/null 2>&1; then jexec sow-server pw groupadd sowrelay; fi
if ! jexec sow-server id sowrelay >/dev/null 2>&1; then
    jexec sow-server pw useradd sowrelay -g sowrelay -G sow -d /nonexistent -s /usr/sbin/nologin
fi
jexec sow-server id -Gn sowrelay | tr ' ' '\n' | grep -qx sow
install -d -m 0755 "$jail/etc/rc.conf.d"
install -d -m 0755 /usr/local/etc/letsencrypt/renewal-hooks/deploy
install -m 0555 "$ops/sow-relay-cert-deploy" /usr/local/etc/letsencrypt/renewal-hooks/deploy/50-sow-relay

"#, &[&ops]).context("prepare IONOS relay control service")?;
    Ok(())
}

pub(super) fn certificate(config: &Config) -> Result<()> {
    run_root(config, r#"
lineage=$1
hook=$2
openssl x509 -in "$lineage/fullchain.pem" -noout -checkend 2592000 >/dev/null
installed=/zroot/jails/sow-server/usr/local/etc/sow/relay-tls/current/relay.crt
if ! cmp -s "$lineage/fullchain.pem" "$installed"; then "$hook"; fi
"#, &[TLS_LINEAGE, TLS_HOOK]).context("install site certificate for local relay control")?;
    Ok(())
}

pub(super) fn activate(config: &Config, initial: bool) -> Result<()> {
    let ops = format!(
        "{}/release/relay-ops",
        config.remote_stage.trim_end_matches('/')
    );
    let control_file = format!("/tmp/sow-relay-local-control-{}", std::process::id());
    let settlement_file = format!("/tmp/sow-relay-local-settlement-{}", std::process::id());
    stage_secret(
        &config.control_host,
        &env::var("SOW_RELAY_CONTROL_SECRET")?,
        &control_file,
    )?;
    stage_secret(
        &config.control_host,
        &env::var("SOW_RELAY_SETTLEMENT_SECRET")?,
        &settlement_file,
    )?;
    run_root(
        config,
        r#"
ops=$1
jail=/zroot/jails/sow-server
envfile=$(mktemp /tmp/sow-relay.env.XXXXXX)
trap 'rm -f "$envfile" "$2" "$3"' EXIT HUP INT TERM
chmod 0600 "$envfile"
cat "$ops/relay.env" > "$envfile"
printf 'SOW_RELAY_CONTROL_SECRET=' >> "$envfile"
cat "$2" >> "$envfile"
printf '\nSOW_RELAY_SETTLEMENT_SECRET=' >> "$envfile"
cat "$3" >> "$envfile"
printf '\n' >> "$envfile"
install -m 0600 "$envfile" "$jail/usr/local/etc/sow/relay.env"
install -m 0555 "$ops/sow_relay" "$jail/usr/local/etc/rc.d/sow_relay"
sysrc -f "$jail/etc/rc.conf.d/sow_relay" sow_relay_enable=YES >/dev/null
if [ "$4" = yes ]; then
    jexec sow-server install -d -o sowrelay -g sowrelay -m 0700 /var/run/sow-relay
    jexec sow-server install -o sowrelay -g sowrelay -m 0600 /dev/null /var/run/sow-relay/paused
fi
if jexec sow-server service sow_relay onestatus >/dev/null 2>&1; then
    jexec sow-server service sow_relay restart
else
    jexec sow-server service sow_relay start
fi
"#,
        &[&ops, &control_file, &settlement_file, yes(initial)],
    )
    .context("activate FreeBSD relay control service")?;
    wait_ready(config)
}

pub(super) fn wait_ready(config: &Config) -> Result<()> {
    let command = format!(
        r#"
i=0
until curl --noproxy '*' -fsS --max-time 5 --resolve '{NAME}:{PORT}:127.0.0.1' 'https://{NAME}:{PORT}/healthz' >/dev/null; do
    i=$((i+1)); [ "$i" -lt 30 ] || exit 1; sleep 1
done
"#
    );
    run("ssh", &[&config.control_host, &command], None)
        .context("FreeBSD relay control health check")
}

pub(super) fn ensure_running(config: &Config) -> Result<()> {
    run(
        "ssh",
        &[
            &config.control_host,
            "sudo jexec sow-server service sow_relay onestatus >/dev/null 2>&1 || sudo jexec sow-server service sow_relay start",
        ],
        None,
    )?;
    wait_ready(config)
}

pub(super) fn verify_identity(config: &Config, release: &Release) -> Result<()> {
    let metrics = Control::local().metrics(config)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let executable = metrics["executable"]
        .as_str()
        .context("relay executable missing")?;
    let pid = metrics["proc_id"].as_u64().context("relay pid missing")?;
    ensure!(
        executable.starts_with("/srv/sow/releases/") && executable.ends_with("/bin/sow-relay"),
        "relay control is not running from an immutable release: {executable}"
    );
    let command = format!(
        "test \"$(sudo cat {JAIL_ROOT}/var/run/sow_relay.child.pid)\" = {pid} && sudo sha256 -q {}",
        shell_quote(executable),
    );
    let sha = output("ssh", &[&config.control_host, &command])?;
    ensure!(
        manifest["relay"]["bin_sha256"] == sha
            && manifest["relay"]["ws_write_timeout_ms"] == metrics["ws_write_timeout_ms"],
        "relay control binary or runtime setting differs from the candidate release"
    );
    Ok(())
}

pub(super) fn verify_settlement(config: &Config) -> Result<()> {
    let script = r#"
db=http://127.0.0.1:25585/internal/match-turns
relay=/zroot/jails/sow-server/usr/local/etc/sow/relay.env
server=/zroot/jails/sow-server/usr/local/etc/sow/sow.env
dedicated=$(awk -F= '$1=="SOW_RELAY_SETTLEMENT_SECRET" {print substr($0,index($0,"=")+1)}' "$relay")
general=$(awk -F= '$1=="SOW_DB_SECRET" {print substr($0,index($0,"=")+1)}' "$server")
test -n "$dedicated" && test -n "$general" && test "$dedicated" != "$general"
relay_pid=$(cat /zroot/jails/sow-server/var/run/sow_relay.child.pid)
server_pid=$(jexec sow-server pgrep -xo sow-server)
relay_env=$(procstat -e "$relay_pid")
server_env=$(procstat -e "$server_pid")
test "$(printf '%s\n' "$relay_env" | tr ' ' '\n' | sed -n 's/^SOW_RELAY_SETTLEMENT_SECRET=//p')" = "$dedicated"
if printf '%s\n' "$relay_env" | tr ' ' '\n' | grep -q '^SOW_DB_SECRET='; then exit 78; fi
if printf '%s\n' "$server_env" | tr ' ' '\n' | grep -q '^SOW_RELAY_SETTLEMENT_SECRET='; then exit 78; fi
probe() {
    printf 'header = "Authorization: Bearer %s"\n' "$1" | curl --noproxy '*' -sS --max-time 10 --config - \
        -o /dev/null -w '%{http_code}' -H 'Content-Type: application/octet-stream' --data-binary invalid "$db"
}
test "$(probe "$dedicated")" = 400
test "$(probe "$general")" = 401
test "$(probe '')" = 401
for retired in 25585/internal/match-legacy-journal/invalid 25566/internal/replay/verify/invalid; do
    test "$(curl -sS --max-time 10 -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$retired")" = 404
done
test "$(sockstat -4l -p "$1" | awk '/sow-relay/ {print $6}')" = "127.0.0.1:$1"
"#;
    run_root(config, script, &[&PORT.to_string()])
        .context("verify loopback relay settlement authorization and listeners")?;
    let public = format!("{}/internal/match-turns", config.public_origin);
    let status = output(
        "curl",
        &[
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--data-binary",
            "invalid",
            &public,
        ],
    )?;
    ensure!(
        matches!(status.as_str(), "403" | "404"),
        "relay settlement route is public ({status})"
    );
    Ok(())
}

pub(super) fn verify_certificate(config: &Config) -> Result<()> {
    let command = format!(
        r#"
set -eu
sudo openssl x509 -in {TLS_LINEAGE}/fullchain.pem -noout -checkend 2592000 >/dev/null
expected=$(sudo openssl x509 -in {TLS_LINEAGE}/fullchain.pem -noout -fingerprint -sha256)
actual=$(openssl s_client -connect 127.0.0.1:{PORT} -servername {NAME} -verify_hostname {NAME} -verify_return_error </dev/null 2>/dev/null | openssl x509 -noout -fingerprint -sha256)
test "$expected" = "$actual"
sudo jexec sow-server sockstat -4l -p {PORT} | awk -v endpoint="127.0.0.1:{PORT}" '
NR > 1 {{ if ($6 != endpoint || $1 != "sowrelay") exit 1; found = 1 }}
END {{ if (!found) exit 1 }}
'
sudo test -x {TLS_HOOK}
"#
    );
    run("ssh", &[&config.control_host, &command], None)
        .context("verify served relay certificate and renewal hook")
}
