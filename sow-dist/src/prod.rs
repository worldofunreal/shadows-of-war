use super::*;
use hmac::{Hmac, Mac};
use serde_json::json;
use std::collections::BTreeMap;

#[path = "prod_relay.rs"]
mod relay;

const BUILD_HOST: &str = "freebsd";
const BUILD_ROOT: &str = "/home/bizkit/shadows-of-war";
const CONTROL_HOST: &str = "ionos";
const STRIPE_ENV_KEYS: &str = "^(SOW_STRIPE_[A-Z_]+|SOW_REVENUECAT_STRIPE_API_KEY)=";
const ANDROID_PROJECT: &str = "sow-dist/deploy/android";
const ANDROID_BUNDLE: &str = "dist/android/com.shadowsofwar.aab";
const ANDROID_VERSION_CODE: &str = ".android-version-code";

struct Config {
    build_host: String,
    build_root: String,
    control_host: String,
    remote_stage: String,
    public_origin: String,
}

impl Config {
    fn load() -> Self {
        Self {
            build_host: env_or("SOW_FREEBSD_BUILDER_HOST", BUILD_HOST),
            build_root: env_or("SOW_FREEBSD_BUILDER_ROOT", BUILD_ROOT),
            control_host: env_or("SOW_CONTROL_HOST", CONTROL_HOST),
            remote_stage: env_or("SOW_REMOTE_STAGE", site::current().remote_stage()),
            public_origin: env_or("SOW_PUBLIC_ORIGIN", &site::current().origin())
                .trim_end_matches('/')
                .to_string(),
        }
    }
}

/// `./sow config p|ptr`: edit the site's server rules in the repo
/// (`sow-dist/config/<site>.json`), check them with the parser the server uses,
/// and install them over SSH. The server reads the file at every lobby
/// creation, so new matches use it without a restart. The repo file is the
/// source of truth for site-specific rules. Shared speed is built from
/// `sow-dist/config/game-speed.json` and cannot be changed here.
pub(crate) fn edit_server_config(root: &Path) -> Result<()> {
    use std::io::Write;
    let config = Config::load();
    let site = site::current();
    let remote = format!("{}/usr/local/etc/sow/config.json", site.server_root());
    let local = root
        .join("sow-dist/config")
        .join(format!("{}.json", site.state_prefix()));
    if !local.exists() {
        fs::create_dir_all(local.parent().context("config dir")?)?;
        fs::write(&local, "{}\n")?;
    }
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let text = loop {
        let edited = Command::new("sh")
            .args(["-c", &format!("{editor} \"$1\""), "sh"])
            .arg(&local)
            .status()?;
        if !edited.success() {
            bail!("editor exited with an error; nothing was installed");
        }
        let text = fs::read_to_string(&local)?;
        if serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .is_some_and(|value| value.get("global_speed_multiplier").is_some())
        {
            eprintln!("❌ global_speed_multiplier is shared by both sites; edit sow-dist/config/game-speed.json and deploy with ./sow ptr or ./sow p");
            continue;
        }
        match sow_core::game_config::GameConfig::with_overrides(&text) {
            Ok(_) => break text,
            Err(error) => {
                eprintln!("❌ {error}");
                eprint!("Press Enter to fix it, or Ctrl-C to cancel: ");
                if std::io::stdin().read_line(&mut String::new())? == 0 {
                    bail!("cancelled; {} has the error and nothing was installed", local.display());
                }
            }
        }
    };
    let mut install = command("ssh")
        .args([
            &config.control_host,
            &format!(
                "t=$(mktemp) && cat > \"$t\" && sudo install -o root -g wheel -m 0644 \"$t\" {remote}; s=$?; rm -f \"$t\"; exit $s"
            ),
        ])
        .stdin(Stdio::piped())
        .spawn()?;
    install
        .stdin
        .take()
        .context("ssh stdin")?
        .write_all(text.as_bytes())?;
    if !install.wait()?.success() {
        bail!("could not install {remote}");
    }
    println!(
        "✅ {} installed as {remote}; matches created from now on use it.",
        local.display()
    );
    Ok(())
}

fn remote_releases() -> String {
    format!("{}/releases", site::current().release_root())
}

struct Release {
    id: String,
    version: String,
    dir: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BackendOnly {
    Server,
    Relay,
}

impl BackendOnly {
    fn name(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Relay => "relay",
        }
    }
}

#[derive(Default, Debug)]
struct ComponentPlan {
    web: bool,
    maps: bool,
    server: bool,
    database: bool,
    ops: bool,
    relay: bool,
    /// The multiplayer identity changed; never a restart on its own.
    client: bool,
}

impl ComponentPlan {
    fn any(&self) -> bool {
        self.web || self.maps || self.server || self.database || self.ops || self.relay
    }
}

struct RelayAdmissionPause<'a> {
    config: &'a Config,
    route: &'a relay::Control,
    paused: bool,
}

impl<'a> RelayAdmissionPause<'a> {
    fn acquire(config: &'a Config, route: &'a relay::Control, require_empty: bool) -> Result<Self> {
        route.metrics(config)?;
        let pause = Self { config, route, paused: true };
        route.drain(config, true)?;
        if require_empty {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
            loop {
                let active = route.lobbies(config)?.len();
                let pending = if active == 0 {
                    match verify_no_pending_reward_calculations(config) {
                        Ok(()) => break,
                        Err(error) => error,
                    }
                } else {
                    anyhow::anyhow!("{active} active or closing matches remain")
                };
                if std::time::Instant::now() >= deadline {
                    return Err(pending.context("timed out draining production matches"));
                }
                println!("  waiting for production drain: {pending:#}");
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
        }
        Ok(pause)
    }

    fn resume(&mut self) -> Result<()> {
        if self.paused {
            self.route.drain(self.config, false)?;
            self.paused = false;
        }
        Ok(())
    }
}

fn verify_no_pending_reward_calculations(config: &Config) -> Result<()> {
    let script = r#"set -eu
secret=$(sudo awk -F= '$1 == "SOW_DB_SECRET" { sub(/^[^=]*=/, ""); print; exit }' __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env)
test -n "$secret"
printf 'header = "Authorization: Bearer %s"\n' "$secret" | curl --noproxy '*' -fsS --max-time 10 --config - http://__SITE_LOOPBACK__:25585/internal/stats"#;
    let stats: serde_json::Value =
        serde_json::from_str(&output("ssh", &[&config.control_host, script])?)?;
    let pending = stats["rewards"]["pending_calculations"]
        .as_u64()
        .context("database did not confirm its pending reward calculations")?;
    let payments = stats["rewards"]["pending_payments"]
        .as_u64()
        .context("database did not confirm its pending reward payments")?;
    if pending != 0 || payments != 0 {
        bail!(
            "backend update postponed: {pending} match calculations and {payments} closing match payments remain; matches={}",
            stats["rewards"]["match_ids"]
        );
    }
    verify_settled_verifier_checkpoints(config)
}

fn verify_settled_verifier_checkpoints(config: &Config) -> Result<()> {
    let script = r#"sudo python3 - <<'VERIFY_SAVED_ENGINES'
import json, re, subprocess, urllib.request

def jail(name, *args):
    return subprocess.check_output(["jexec", name, *args], text=True)

listing = jail("__SITE_SERVER_JAIL__", "/bin/sh", "-c", '''
set -eu
. /usr/local/etc/sow/sow.env
root=${SOW_VERIFIER_DIR:-$(dirname "$SOW_MAPS_CATALOG_PATH")/verified}
for file in "$root"/*.checkpoint "$root"/*.checkpoint.previous; do
    [ ! -f "$file" ] || basename "$file"
done
''').splitlines()
ids = sorted({re.fullmatch(r"([0-9]+)\.checkpoint(?:\.previous)?", name).group(1) for name in listing})
if ids:
    secret = jail("__SITE_DATABASE_JAIL__", "/bin/sh", "-c", '. /usr/local/etc/sow/sow.env; printf "%s" "$SOW_DB_SECRET"')
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    for match_id in ids:
        request = urllib.request.Request("http://__SITE_LOOPBACK__:25585/internal/stats?match_id=" + match_id,
            headers={"Authorization": "Bearer " + secret})
        with opener.open(request, timeout=10) as response:
            proof = json.load(response).get("checkpoint", {})
        if proof.get("match_id") != match_id or proof.get("rewards_terminal") is not True:
            raise RuntimeError("saved match still requires its running verifier: " + match_id)
    print("  verified every human receipt for",len(ids),"saved match engines")
VERIFY_SAVED_ENGINES
"#;
    run("ssh", &[&config.control_host, script], None)
        .context("backend update postponed: a saved match still requires its running verifier")
}

impl Drop for RelayAdmissionPause<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.resume() {
            eprintln!("failed to restore relay matchmaking after deployment: {error:#}");
        }
    }
}

pub(super) fn execute(paths: &Paths, bump: bool, only: Option<BackendOnly>, client_update: bool) -> Result<()> {
    if let Some(only) = only && bump {
        bail!("--{}-only cannot bump the public version", only.name());
    }
    let config = Config::load();
    auth_env_contents()?;
    require_secret("SOW_DB_SECRET")?;
    require_secret("SOW_RELAY_CONTROL_SECRET")?;
    require_secret("SOW_RELAY_SETTLEMENT_SECRET")?;
    require_secret("SOW_REVENUECAT_WEBHOOK_SECRET")?;
    require_config("SOW_PLAY_GAMES_APP_ID")?;
    require_config("SOW_PLAY_GAMES_WEB_CLIENT_ID")?;
    require_config("SOW_PLAY_GAMES_MATCH_EVENT_ID")?;
    require_config("SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID")?;
    require_config("SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID")?;
    require_secret("SOW_PLAY_GAMES_WEB_CLIENT_SECRET")?;

    println!("==> 1/8 Preflight");
    let local = relay::Control::local();
    let fresh_site = !has_active_release(&config)?;
    preflight(paths, &config, only.is_some())?;
    let version = version(paths, bump)?;
    println!("==> {} {version}", site::current().label());
    // Android is deliberately NOT part of this pipeline: every Play upload
    // resets Google's review clock, so AAB publication lives in `./sow a`
    // and runs only when a new build is actually ready for review.

    println!("==> 2/8 Build candidates");
    let release = if let Some(only) = only {
        let backend = build_freebsd(paths, &config)?;
        println!("==> 3/8 Package {}-only immutable release", only.name());
        assemble_backend_only_release(paths, &backend, &version, &config, only)?
    } else {
        let (web, backend) = std::thread::scope(|scope| {
            let web = scope.spawn(|| build_web(paths, &version));
            let backend = scope.spawn(|| build_freebsd(paths, &config));
            let web = web.join().map_err(|_| anyhow::anyhow!("web build panicked"))??;
            let backend = backend.join().map_err(|_| anyhow::anyhow!("FreeBSD build panicked"))??;
            Ok::<_, anyhow::Error>((web, backend))
        })?;
        println!("==> 3/8 Package immutable release");
        assemble_release(paths, &web, &backend, &version)?
    };
    println!("  release {}", release.id);

    ensure_control_clock(&config)?;
    verify_control_runtime_secret(&config)?;
    if !fresh_site {
        relay::ensure_running(&config)?;
    }

    println!("==> 4/8 Compare deployed manifest");
    let mut plan = remote_plan(&config, &release)?;
    if relay::verify_identity(&config, &release).is_err() {
        plan.relay = true;
    }
    if control_certificate_automation_drift(&config)? || maps_catalog_path_drift(&config)? {
        plan.ops = true;
    }
    if auth_config_drift(&config)? || play_games_config_drift(&config)? {
        plan.database = true;
        plan.server = true;
    }
    if stripe_env_leftover(&config)? { plan.database = true; }
    if let Some(only) = only && (plan.web || plan.maps || plan.database || plan.ops
        || (plan.relay && only == BackendOnly::Server)
        || (plan.server && only == BackendOnly::Relay))
    {
        bail!("{}-only deployment refused: another component needs activation ({plan:?})", only.name());
    }
    println!("  plan: {plan:?}");
    if plan.client {
        let note = format!(
            "this release changes the multiplayer identity to {}: players on Poki, CrazyGames, Jest and Android cannot join until their store publishes the new build",
            sow_core::protocol::gameplay_build_identity()
        );
        if !site::current().is_production() {
            println!("⚠️  {note}");
        } else if !client_update {
            bail!("{note}. Run ./sow p --client-update to deploy it anyway");
        }
    }

    if !plan.any() {
        println!("  no production component changed; no restart performed");
        verify_ionos_runtime(&config, &release, &plan)?;
        local.drain(&config, false)?;
        verify_public(paths, &config, &release)?;
        join_smoke(paths, &config)?;
        retain_releases(&config)?;
        retain_local_builds(paths)?;
        println!("✅ {} already serves the requested content", site::current().label());
        return Ok(());
    }

    println!("==> 5/8 Stage release (no service mutation)");
    stage_release(&config, &release)?;
    let require_empty = plan.server || plan.database || plan.maps || plan.ops;
    let activate = || {
        println!("==> 6/8 Activate changed components only");
        // A first activation has no relay to pause and no matches to drain.
        let mut admission = (!fresh_site && (require_empty || plan.relay))
            .then(|| RelayAdmissionPause::acquire(&config, &local, require_empty))
            .transpose()?;
        let backup = relay::Backup::capture(&config)?;
        let mut opened_local = false;
        let result = (|| -> Result<()> {
            if plan.relay {
                relay::prepare_host(&config)?;
                relay::certificate(&config)?;
            }
            activate_control_host(paths, &config, &release, &plan)?;
            if plan.relay {
                relay::activate(&config)?;
            }
            println!("==> 7/8 Verify IONOS runtime");
            verify_ionos_runtime(&config, &release, &plan)?;
            if let Some(pause) = admission.as_mut() {
                opened_local = true;
                pause.resume()?;
            }
            Ok(())
        })();
        if result.is_err() && let Some(pause) = admission.as_mut() {
            pause.paused = false;
        }
        Ok::<_, anyhow::Error>((backup, opened_local, result))
    };

    let (backup, opened_local, activation_result) = activate()?;
    let result = activation_result.and_then(|()| {
        println!("==> 8/8 Public verification");
        verify_public(paths, &config, &release)?;
        join_smoke(paths, &config)
    });
    if let Err(error) = result {
        // A live match keeps its own process; a backend restore waits for an empty relay.
        let rollback = (|| -> Result<()> {
            if opened_local && require_empty {
                local.drain(&config, true)?;
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(240);
                loop {
                    if local.lobbies(&config)?.is_empty()
                        && verify_no_pending_reward_calculations(&config).is_ok()
                    {
                        break;
                    }
                    if std::time::Instant::now() >= deadline {
                        bail!("rollback waiting for live IONOS matches or durable settlements");
                    }
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
            }
            backup.restore(&config, &plan)?;
            local.metrics(&config)?;
            local.drain(&config, false)?;
            Ok(())
        })();
        return match rollback {
            Ok(()) => Err(error.context("previous production release restored")),
            Err(rollback) => Err(error.context(format!("could not complete rollback: {rollback:#}"))),
        };
    }
    backup.remove(&config)?;
    retain_releases(&config)?;
    retain_local_builds(paths)?;
    println!("✅ {} {} ready as {}", site::current().label(), release.version, release.id);
    Ok(())
}

fn verify_ionos_runtime(config: &Config, release: &Release, plan: &ComponentPlan) -> Result<()> {
    verify_control_host(config, plan)?;
    verify_map_catalog(config, release)?;
    verify_control_certificates(config)?;
    verify_control_runtime_secret(config)?;
    if !plan.relay {
        relay::certificate(config)?;
    }
    relay::wait_ready(config)?;
    relay::verify_identity(config, release)?;
    relay::verify_settlement(config)?;
    relay::verify_certificate(config)
}

/// A real client joins a private lobby on the public route with this release's identity.
fn join_smoke(paths: &Paths, config: &Config) -> Result<()> {
    let mut websocket = url::Url::parse(&config.public_origin)?;
    let scheme = if websocket.scheme() == "https" { "wss" } else { "ws" };
    websocket
        .set_scheme(scheme)
        .map_err(|_| anyhow::anyhow!("invalid public WebSocket scheme"))?;
    websocket.set_path("/ws/");
    let database = format!("{}/api", config.public_origin);
    let accounts = paths
        .root
        .join("dist/.sow-state")
        .join(site::current().reward_audit_accounts_file());
    run(
        "cargo",
        &[
            "run", "--locked", "-p", "sow-tools", "--bin", "test-relay", "--",
            "--smoke",
            "--url", websocket.as_str(),
            "--database-url", &database,
            "--audit-accounts-file", accounts.to_str().context("invalid audit accounts path")?,
        ],
        Some(&paths.root),
    )
    .context("join smoke failed")
}

/// `./sow a` — Android-only track, decoupled from `./sow p` on purpose.
/// Each Play upload restarts Google's review clock, so this runs only on
/// explicit request: versionCode bumps, AAB rebuilds, on-device smoke test,
/// then upload to the configured Play track.
pub(super) fn execute_android(paths: &Paths) -> Result<()> {
    println!("==> 1/3 Preflight (read-only)");
    preflight_android(paths)?;
    let source = require_published_source(paths)?;
    // Every Android publication is a user-visible release. Keep the marketing
    // version moving together with the Play versionCode instead of publishing
    // a new AAB forever as the same versionName.
    let version = version(paths, true)?;
    println!("==> Android {version}");
    let android_code = android_version_code(paths, &version, false)?;

    println!("==> 2/3 Build Android AAB");
    build_android(paths, &version, android_code, &source.sha, &source.url)?;

    println!("==> 3/3 Test and publish");
    test_android(paths, &version, android_code, &source.sha)?;
    publish_android(paths, android_code)?;
    println!("✅ Android {version} (code {android_code}) published");
    Ok(())
}

fn require_secret(key: &str) -> Result<()> {
    let value =
        env::var(key).with_context(|| format!("{key} must be provided via sow-dist/.env"))?;
    if value.trim().is_empty() {
        bail!("{key} must not be empty");
    }
    Ok(())
}

fn require_config(key: &str) -> Result<()> {
    let value =
        env::var(key).with_context(|| format!("{key} must be provided via sow-dist/.env"))?;
    if value.trim().is_empty() {
        bail!("{key} must not be empty");
    }
    Ok(())
}

/// Renders `__SITE_*` placeholders in release files that are installed as they are.
fn render_site_files(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            render_site_files(&path)?;
        } else if fs::read(&path)?.windows(7).any(|window| window == b"__SITE_") {
            let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            fs::write(&path, site::render(&text)?)?;
        }
    }
    Ok(())
}

/// Keeps `# sow:production-only` blocks in production's nginx file and drops the markers everywhere.
fn strip_production_only(text: &str) -> Result<String> {
    const BEGIN: &str = "# sow:production-only begin";
    const END: &str = "# sow:production-only end";
    let production = site::current().is_production();
    let mut kept = String::new();
    let mut inside = false;
    for line in text.lines() {
        match line.trim() {
            BEGIN if inside => bail!("nested sow:production-only block"),
            BEGIN => inside = true,
            END if !inside => bail!("sow:production-only end without begin"),
            END => inside = false,
            _ if inside && !production => {}
            _ => {
                kept.push_str(line);
                kept.push('\n');
            }
        }
    }
    if inside {
        bail!("unterminated sow:production-only block");
    }
    Ok(kept)
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn version(paths: &Paths, bump: bool) -> Result<String> {
    let path = paths.root.join(".version");
    let current = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?
        .trim()
        .to_string();
    if !bump {
        return Ok(current);
    }
    let mut parts = current
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context(".version must be major.minor.patch")?;
    if parts.len() != 3 {
        bail!(".version must be major.minor.patch");
    }
    parts[2] += 1;
    let next = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    fs::write(&path, format!("{next}\n"))?;
    println!("  version {current} -> {next}");
    Ok(next)
}

fn android_inputs(paths: &Paths) -> Result<Vec<PathBuf>> {
    let project = paths.root.join(ANDROID_PROJECT);
    Ok(vec![
        project.join("app/build.gradle"),
        project.join("app/proguard-rules.pro"),
        project.join("app/src/main"),
        paths.android_resources.clone(),
        paths.root.join("assets/art/brand/app-icon.webp"),
        paths.art_pipeline()?.config_path.clone(),
        project.join("build.gradle"),
        project.join("settings.gradle"),
        project.join("gradle.properties"),
        project.join("gradle/wrapper/gradle-wrapper.jar"),
        project.join("gradle/wrapper/gradle-wrapper.properties"),
        project.join("gradlew"),
        project.join("key.properties"),
    ])
}

fn android_fingerprint(paths: &Paths, version: &str, version_code: u32) -> Result<String> {
    let inputs = android_inputs(paths)?;
    let input_refs = inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    input_fingerprint(
        "android-v2",
        &format!("{version}:{version_code}"),
        &input_refs,
    )
}

fn android_play_key_path() -> Option<PathBuf> {
    env::var_os("SOW_PLAY_KEY").map(PathBuf::from).or_else(|| {
        env::var_os("HOME").map(|home| {
            PathBuf::from(home).join(".config/shadows-of-war/google-play-service-account.json")
        })
    })
}

fn fastlane_bin() -> String {
    if let Some(path) = env::var_os("SOW_FASTLANE") {
        return PathBuf::from(path).to_string_lossy().into_owned();
    }
    if let Some(home) = env::var_os("HOME") {
        let path = PathBuf::from(home).join(".local/share/gem/ruby/3.4.0/bin/fastlane");
        if path.is_file() {
            return path.to_string_lossy().into_owned();
        }
    }
    "fastlane".to_string()
}

fn play_highest_android_version_code() -> Result<u32> {
    let Some(play_key) = android_play_key_path() else {
        return Ok(0);
    };
    if !play_key.is_file() {
        return Ok(0);
    }
    let play_key = play_key
        .to_str()
        .context("SOW_PLAY_KEY path is not UTF-8")?;
    let fastlane = fastlane_bin();
    let mut highest = 0;
    for track in ["internal", "alpha", "beta", "open", "production"] {
        let track_arg = format!("track:{track}");
        let key_arg = format!("json_key:{play_key}");
        let output = output(
            &fastlane,
            &[
                "run",
                "google_play_track_version_codes",
                "package_name:com.shadowsofwar",
                &track_arg,
                &key_arg,
            ],
        )?;
        let result = output
            .lines()
            .rev()
            .find_map(|line| line.split_once("Result:").map(|(_, value)| value.trim()))
            .with_context(|| format!("Google Play returned no version list for track {track}"))?;
        let result = result
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .context("Google Play returned malformed version list")?;
        for value in result
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let code = value
                .parse::<u32>()
                .with_context(|| format!("Google Play returned invalid version code: {value}"))?;
            highest = highest.max(code);
        }
    }
    Ok(highest)
}

fn android_version_code(paths: &Paths, _version: &str, _bump: bool) -> Result<u32> {
    let path = paths.root.join(ANDROID_VERSION_CODE);
    let current = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?
        .trim()
        .parse::<u32>()
        .with_context(|| format!("{ANDROID_VERSION_CODE} must contain a positive integer"))?;
    if current == 0 {
        bail!("{ANDROID_VERSION_CODE} must contain a positive integer");
    }
    let highest_play_code = play_highest_android_version_code()?;
    let next = current
        .max(highest_play_code)
        .checked_add(1)
        .context("Android versionCode exhausted")?;
    fs::write(&path, format!("{next}\n"))?;
    println!("  Android versionCode local={current} play_max={highest_play_code} -> {next}");
    Ok(next)
}

fn build_android(
    paths: &Paths,
    version: &str,
    version_code: u32,
    source_sha: &str,
    source_url: &str,
) -> Result<()> {
    let project = paths.root.join(ANDROID_PROJECT);
    let gradlew = project.join("gradlew");
    let key_properties = project.join("key.properties");
    let bundle = project.join("app/build/outputs/bundle/release/app-release.aab");
    let output = paths.root.join(ANDROID_BUNDLE);
    let cache = paths.root.join("dist/.sow-state/android-build");

    require_file(&gradlew, "Android Gradle wrapper")?;
    require_file(&key_properties, "Android signing properties")?;
    let fingerprint = android_fingerprint(paths, version, version_code)?;
    let version_name_arg = format!("-PsowVersionName={version}");
    let version_code_arg = format!("-PsowVersionCode={version_code}");
    let source_sha_arg = format!("-PsowSourceSha={source_sha}");
    let source_url_arg = format!("-PsowSourceUrl={source_url}");
    let revenuecat_key = env::var("SOW_REVENUECAT_ANDROID_PUBLIC_KEY")
        .context("SOW_REVENUECAT_ANDROID_PUBLIC_KEY must be provided via sow-dist/.env")?;
    if !revenuecat_key.starts_with("goog_") {
        bail!("SOW_REVENUECAT_ANDROID_PUBLIC_KEY must be a Google Play public key");
    }
    let revenuecat_key_arg = format!("-PrevenueCatAndroidPublicKey={revenuecat_key}");
    let play_games_app_id = env::var("SOW_PLAY_GAMES_APP_ID")
        .context("SOW_PLAY_GAMES_APP_ID must be provided via sow-dist/.env")?;
    let play_games_client_id = env::var("SOW_PLAY_GAMES_WEB_CLIENT_ID")
        .context("SOW_PLAY_GAMES_WEB_CLIENT_ID must be provided via sow-dist/.env")?;
    let play_games_app_id_arg = format!("-PsowPlayGamesAppId={play_games_app_id}");
    let play_games_client_id_arg = format!("-PsowPlayGamesWebClientId={play_games_client_id}");
    let generated_res_arg = format!("-PsowGeneratedResDir={}", paths.android_resources.display());
    run(
        "./gradlew",
        &[
            "--warning-mode",
            "fail",
            "--no-daemon",
            "--no-configuration-cache",
            ":app:bundleRelease",
            ":app:assembleDebug",
            &version_name_arg,
            &version_code_arg,
            &source_sha_arg,
            &source_url_arg,
            &revenuecat_key_arg,
            &play_games_app_id_arg,
            &play_games_client_id_arg,
            &generated_res_arg,
        ],
        Some(&project),
    )?;
    require_file(&bundle, "Android release AAB")?;

    let metadata_path = [
        project.join(
            "app/build/intermediates/merged_manifests/release/processReleaseManifest/output-metadata.json",
        ),
        project.join("app/build/intermediates/merged_manifests/release/output-metadata.json"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .context("Android output metadata not found after release build")?;
    require_file(&metadata_path, "Android output metadata")?;
    let release_build_config = project
        .join("app/build/generated/source/buildConfig/release/com/shadowsofwar/BuildConfig.java");
    let build_config = fs::read_to_string(&release_build_config)
        .with_context(|| format!("read {}", release_build_config.display()))?;
    if !build_config.contains(&format!("SOW_SOURCE_SHA = \"{source_sha}\""))
        || !build_config.contains(&format!("SOW_SOURCE_URL = \"{source_url}\""))
        || !build_config.contains(&format!("VERSION_CODE = {version_code};"))
    {
        bail!("Android release artifact identity is not bound to source SHA/versionCode");
    }
    let release_manifest = project.join(
        "app/build/intermediates/merged_manifests/release/processReleaseManifest/AndroidManifest.xml",
    );
    let merged_manifest = fs::read_to_string(&release_manifest)
        .with_context(|| format!("read {}", release_manifest.display()))?;
    if !merged_manifest.contains("package=\"com.shadowsofwar\"")
        || !merged_manifest.contains(&format!("android:versionCode=\"{version_code}\""))
        || !merged_manifest.contains(&format!("sow_source_sha={source_sha}"))
        || !merged_manifest.contains(&format!("{source_url}"))
    {
        bail!("Android release manifest identity is not bound to source SHA/versionCode");
    }
    let metadata: serde_json::Value = serde_json::from_slice(&fs::read(&metadata_path)?)
        .context("parse Android output metadata")?;
    let application_id = metadata
        .get("applicationId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let element = metadata
        .get("elements")
        .and_then(serde_json::Value::as_array)
        .and_then(|elements| elements.first())
        .context("Android output metadata has no bundle element")?;
    let built_code = element
        .get("versionCode")
        .and_then(serde_json::Value::as_u64)
        .context("Android output metadata has no versionCode")?;
    let built_name = element
        .get("versionName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if application_id != "com.shadowsofwar" {
        bail!("Android applicationId mismatch: {application_id}");
    }
    if built_code != u64::from(version_code) || built_name != version {
        bail!(
            "Android version mismatch: built {built_name} (code {built_code}), expected {version} (code {version_code})"
        );
    }

    fs::create_dir_all(output.parent().context("Android output parent missing")?)?;
    fs::copy(&bundle, &output)
        .with_context(|| format!("copy {} to {}", bundle.display(), output.display()))?;
    fs::create_dir_all(cache.parent().context("Android cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    println!(
        "  Android AAB ready: {} (version {version}, code {version_code}, sha256 {})",
        output.display(),
        file_sha256(&bundle)?
    );
    Ok(())
}

fn test_android(paths: &Paths, version: &str, version_code: u32, source_sha: &str) -> Result<()> {
    let adb_state = Command::new("adb").args(["get-state"]).output();
    let has_adb_device = adb_state
        .as_ref()
        .map(|s| s.status.success() && String::from_utf8_lossy(&s.stdout).trim() == "device")
        .unwrap_or(false);
    if !has_adb_device {
        bail!("Android local device test requires an attached USB-debugging device");
    }
    let test_script = paths.root.join("scripts/android-local-test.sh");
    require_file(&test_script, "Android local test script")?;
    let play_track = env_or("SOW_ANDROID_PLAY_TRACK", "alpha");
    println!(
        "==> Android local smoke test (captured logcat): {}",
        test_script.display()
    );
    let test_status = Command::new(&test_script)
        .env("SOW_ANDROID_SKIP_BUILD", "1")
        .env("SOW_ANDROID_TEST_VARIANT", "debug")
        .env("SOW_ANDROID_TEST_VERSION_NAME", version)
        .env("SOW_ANDROID_TEST_VERSION_CODE", version_code.to_string())
        .env("SOW_ANDROID_TEST_SOURCE_SHA", source_sha)
        .env("SOW_ANDROID_TEST_TRACK", play_track)
        .status()?;
    if !test_status.success() {
        bail!("Android local device test failed");
    }
    Ok(())
}

fn publish_android(paths: &Paths, version_code: u32) -> Result<()> {
    let script = paths.root.join("scripts/android-release.sh");
    require_file(&script, "Android Play publication script")?;
    let track = env_or("SOW_ANDROID_PLAY_TRACK", "alpha");
    println!("==> Publish Android AAB to Google Play {track}");
    let status = Command::new(&script)
        .arg("--publish-existing")
        .env(
            "SOW_ANDROID_EXPECTED_VERSION_CODE",
            version_code.to_string(),
        )
        .status()
        .context("start Android Play publication")?;
    if !status.success() {
        bail!("Android Play publication failed");
    }
    Ok(())
}

fn preflight(paths: &Paths, config: &Config, reuse_web: bool) -> Result<()> {
    let commands: &[&str] = if reuse_web {
        &["bun", "cargo", "curl", "node", "rsync", "rustc", "scp", "ssh"]
    } else {
        &["bun", "cargo", "curl", "magick", "node", "rsync", "rustc", "scp", "ssh", "wasm-opt"]
    };
    for command in commands {
        if !Command::new("/bin/sh")
            .args(["-c", &format!("command -v {command} >/dev/null")])
            .status()?
            .success()
        {
            bail!("{command} is required");
        }
    }
    if !Command::new("rustc")
        .args([
            "--print",
            "target-libdir",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success()
    {
        bail!("Rust WASM standard library missing");
    }
    require_file(&paths.root.join("Cargo.toml"), "workspace Cargo.toml")?;
    // The blade graphics fork is gitignored, not vendored: a fresh clone has
    // no blade/ and every Rust build fails at workspace resolution. Fail fast
    // here with the one-command fix instead of mid-build.
    for vendored in [
        "blade/Cargo.toml",
        "blade/blade-graphics/Cargo.toml",
        "blade/blade-macros/Cargo.toml",
    ] {
        if !paths.root.join(vendored).is_file() {
            bail!("{vendored} missing (blade/ is gitignored) — run ./scripts/vendor-blade.sh");
        }
    }
    run(
        "ssh",
        &[
            &config.control_host,
            "test -d __SITE_RELEASE_ROOT__/releases && command -v sudo >/dev/null",
        ],
        None,
    )
    .context("control host preflight failed")?;
    run(
        "ssh",
        &[
            &config.build_host,
            &format!(
                "test -d {} && command -v cargo >/dev/null",
                shell_quote(&config.build_root)
            ),
        ],
        None,
    )
    .context("FreeBSD builder preflight failed")
}

fn preflight_android(paths: &Paths) -> Result<()> {
    require_published_source(paths)?;
    for command in ["adb", "java"] {
        if !Command::new("/bin/sh")
            .args(["-c", &format!("command -v {command} >/dev/null")])
            .status()?
            .success()
        {
            bail!("{command} is required for Android release");
        }
    }
    let fastlane = fastlane_bin();
    if !Command::new(&fastlane).arg("--version").status()?.success() {
        bail!("fastlane 2.238.0 is required for Android release");
    }
    require_file(
        &paths.root.join(ANDROID_PROJECT).join("gradlew"),
        "Android Gradle wrapper",
    )?;
    require_file(
        &paths.root.join(ANDROID_PROJECT).join("key.properties"),
        "Android signing properties",
    )?;
    let play_key = android_play_key_path().context(
        "Google Play service-account key path is not configured (set SOW_PLAY_KEY or use the default path)",
    )?;
    require_file(&play_key, "Google Play service-account key")?;
    require_secret("SOW_REVENUECAT_ANDROID_PUBLIC_KEY")?;
    if !env::var("SOW_REVENUECAT_ANDROID_PUBLIC_KEY")?.starts_with("goog_") {
        bail!("SOW_REVENUECAT_ANDROID_PUBLIC_KEY must be a Google Play public key");
    }
    require_config("SOW_PLAY_GAMES_APP_ID")?;
    require_config("SOW_PLAY_GAMES_WEB_CLIENT_ID")?;
    validate_android_release_inputs(paths)
}

fn validate_android_release_inputs(paths: &Paths) -> Result<()> {
    let project = paths.root.join(ANDROID_PROJECT);
    let build_gradle = fs::read_to_string(project.join("app/build.gradle"))?;
    if !build_gradle.contains("compileSdk 36") || !build_gradle.contains("targetSdk 36") {
        bail!("Android build must compile and target API 36");
    }
    if build_gradle.contains("suppressUnsupportedCompileSdk") {
        bail!("Android build contains a suppressed/unsupported compile SDK");
    }

    let strings = fs::read_to_string(project.join("app/src/main/res/values/strings.xml"))?;
    if !strings.contains("\\\"namespace\\\": \\\"web\\\"")
        || !strings.contains("\\\"site\\\": \\\"https://shadowsofwar.io\\\"")
    {
        bail!("Android asset_statements must delegate to https://shadowsofwar.io");
    }

    let assetlinks_path = paths.root.join("sow-web/site/.well-known/assetlinks.json");
    let assetlinks: serde_json::Value = serde_json::from_slice(&fs::read(&assetlinks_path)?)
        .with_context(|| format!("invalid {}", assetlinks_path.display()))?;
    let statements = assetlinks
        .as_array()
        .context("assetlinks.json must contain an array")?;
    for package in ["com.shadowsofwar", "com.shadowsofwar.debug"] {
        let found = statements.iter().any(|statement| {
            statement
                .get("relation")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|relations| {
                    relations.iter().any(|relation| {
                        relation.as_str() == Some("delegate_permission/common.handle_all_urls")
                    }) && relations.iter().any(|relation| {
                        relation.as_str() == Some("delegate_permission/common.use_as_origin")
                    })
                })
                && statement
                    .get("target")
                    .and_then(|target| target.get("namespace"))
                    .and_then(serde_json::Value::as_str)
                    == Some("android_app")
                && statement
                    .get("target")
                    .and_then(|target| target.get("package_name"))
                    .and_then(serde_json::Value::as_str)
                    == Some(package)
                && statement
                    .get("target")
                    .and_then(|target| target.get("sha256_cert_fingerprints"))
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|fingerprints| !fingerprints.is_empty())
        });
        if !found {
            bail!("assetlinks.json has no valid association for {package}");
        }
    }
    println!("  Android build, TWA statement, and assetlinks inputs audited");
    Ok(())
}

fn workspace_dirty_files(paths: &Paths) -> Result<Vec<String>> {
    let root = paths.root.to_str().context("workspace path is not UTF-8")?;
    Ok(output("git", &["-C", root, "status", "--porcelain=v1"])?
        .lines()
        .map(str::to_string)
        .collect())
}

fn require_published_source(paths: &Paths) -> Result<super::SourceIdentity> {
    if !workspace_dirty_files(paths)?.is_empty() {
        bail!("release source must be committed and publicly available before building");
    }
    let source = super::source_identity(paths)?;
    // AGPL §5(a): mark the OpenFront-derived work and when this project began.
    let readme = fs::read_to_string(paths.root.join("README.md"))?;
    let modified_source_notice = "Modified from OpenFrontIO since `2026-05-08`.";
    if !readme.contains(modified_source_notice) {
        bail!("README must identify the OpenFront origin and project start date");
    }
    let status = super::output(
        "curl",
        &[
            "-fsSL",
            "--max-time",
            "20",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            &source.url,
        ],
    )?;
    if status != "200" {
        bail!("release source is not publicly available: {}", source.url);
    }
    println!("  published source: {}", source.url);
    Ok(source)
}

fn control_clock_configured(config: &Config) -> Result<bool> {
    let check = r#"cfg=/etc/rc.conf.d/ntpd; if sudo test -f "$cfg" && sudo grep -qx 'ntpd_enable="YES"' "$cfg" && sudo grep -qx 'ntpd_sync_on_start="YES"' "$cfg" && sudo service ntpd onestatus >/dev/null 2>&1; then echo ok; else echo drift; fi"#;
    Ok(output("ssh", &[&config.control_host, check])? == "ok")
}

fn ensure_control_clock(config: &Config) -> Result<()> {
    if !control_clock_configured(config)? {
        let script = r#"set -eu
sudo install -d /etc/rc.conf.d
sudo sysrc -f /etc/rc.conf.d/ntpd ntpd_enable=YES ntpd_sync_on_start=YES >/dev/null
sudo service ntpd onestart >/dev/null"#;
        run("ssh", &[&config.control_host, script], None)
            .context("control host NTP configuration failed")?;
    }
    if !control_clock_configured(config)? {
        bail!("control-host NTP is not configured and running");
    }
    Ok(())
}

fn verify_control_runtime_secret(config: &Config) -> Result<()> {
    // Before a site's first activation nothing runs, so there is no running secret to match.
    if !has_active_release(config)? {
        return Ok(());
    }
    let secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
    let expected = format!("{:x}", Sha256::digest(secret.as_bytes()));
    let command = r#"set -eu
pid=$(sudo jexec __SITE_SERVER_JAIL__ pgrep -xo sow-server)
value=$(sudo procstat -e "$pid" | tr ' ' '\n' | sed -n 's/^SOW_RELAY_CONTROL_SECRET=//p')
test -n "$value"
printf %s "$value" | sha256 -q"#;
    let actual = output("ssh", &[&config.control_host, command])?;
    if actual != expected {
        bail!("running sow-server relay control secret does not match deployment secret");
    }
    Ok(())
}

fn build_web(paths: &Paths, version: &str) -> Result<PathBuf> {
    compile_wasm(paths, true)?;
    let fingerprint = web_fingerprint(paths, version)?;
    let state = site::current().state_prefix();
    let candidate_root = paths.root.join(format!("dist/.sow-state/{state}-web"));
    let web = candidate_root.join("web");
    let cache = paths.root.join(format!("dist/.sow-state/{state}-web-package"));
    let cached = fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
        && web.join("play/index.html").is_file();
    if cached {
        println!("==> Web package unchanged — reusing candidate");
        return Ok(web);
    }
    package_self(paths, &web, version, true)?;
    fs::create_dir_all(cache.parent().context("web cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    Ok(web)
}

pub(crate) fn web_fingerprint(paths: &Paths, version: &str) -> Result<String> {
    // The game HTML and manifest embed the public source commit, so doc-only
    // commits must still invalidate the packaged web candidate.
    let source = super::source_identity(paths)?;
    let source_version = format!("{version}:{}", source.sha);
    input_fingerprint(
        "web-v14-cg-only-sprite-atlas",
        &source_version,
        &[
            &paths.wasm_input,
            &paths.shell,
            &paths.assets_shell,
            &paths.assets_gameplay,
            &paths.assets_site,
            &paths.assets_maps,
            &paths.root.join("assets/campaign"),
            &paths.root.join("assets/guides"),
            &paths.map_sources,
            &paths.root.join("sow-i18n/src"),
            &paths.root.join("sow-i18n/strings"),
            &paths.root.join("sow-web/site"),
            &paths.root.join("LICENSE"),
            &paths.root.join("docs/legal/NOTICE"),
            &paths.root.join("docs/legal/LICENSE-ASSETS"),
            &paths.root.join("docs/legal/WASM-THIRD-PARTY-NOTICES.zip"),
            &paths.root.join("sow-dist/src/main.rs"),
            &paths.root.join("sow-dist/config/game-speed.json"),
            &paths.art_pipeline()?.config_path,
        ],
    )
}

fn freebsd_build_fingerprint(root: &Path) -> Result<String> {
    input_fingerprint(
        "freebsd-v7",
        "",
        &[
            &root.join("Cargo.toml"),
            &root.join("Cargo.lock"),
            &root.join("sow-core"),
            &root.join("sow-data"),
            &root.join("sow-net"),
            &root.join("sow-server"),
            &root.join("sow-relay"),
            &root.join("sow-dist/config/game-speed.json"),
            // Compile-time assets referenced from outside the Rust crates.
            &root.join("assets/geo_entities.json"),
            &root.join("assets/guides"),
            &root.join("assets/maps/world/map.bin.br"),
        ],
    )
}

fn build_freebsd(paths: &Paths, config: &Config) -> Result<PathBuf> {
    let local = paths.root.join("dist/freebsd-bin");
    let fingerprint = freebsd_build_fingerprint(&paths.root)?;
    let cache = paths.root.join("dist/.sow-state/freebsd-build");
    if ["sow-server", "sow-database", "sow-relay"]
        .iter()
        .all(|name| local.join(name).is_file())
        && fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
    {
        println!("==> FreeBSD backend unchanged — reusing binaries");
        return Ok(local);
    }

    let source = format!("{}/", paths.root.display());
    let destination = format!("{}:{}/", config.build_host, config.build_root);
    run(
        "rsync",
        &[
            "-azc",
            "--no-times",
            "--delete",
            "--exclude=.git",
            "--exclude=dist",
            "--exclude=target",
            "--exclude=sow-dist/.env*",
            &source,
            &destination,
        ],
        Some(&paths.root),
    )?;

    let root = shell_quote(&config.build_root);
    // Synced inputs can predate the last build; discard crates embedding game data.
    let command = format!(
        "set -eu; cd {root}; cargo clean --profile deploy -p sow-core -p sow-data; cargo build --locked --profile deploy -p sow-server -p sow-relay; cargo build --locked --profile deploy -p sow-data --features server --bin sow-database"
    );
    run("ssh", &[&config.build_host, &command], None)?;

    if local.exists() {
        fs::remove_dir_all(&local)?;
    }
    fs::create_dir_all(&local)?;
    for name in ["sow-server", "sow-database", "sow-relay"] {
        let remote = format!(
            "{}:{}/target/deploy/{name}",
            config.build_host, config.build_root
        );
        let destination = local.join(name);
        let temporary = local.join(format!(".{name}.{}.tmp", std::process::id()));
        if temporary.exists() {
            fs::remove_file(&temporary)?;
        }
        if let Err(error) = run(
            "scp",
            &[
                &remote,
                temporary.to_str().context("binary path is not UTF-8")?,
            ],
            None,
        ) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        require_file(&temporary, name)?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o550))?;
        fs::rename(temporary, destination)?;
    }
    fs::create_dir_all(cache.parent().context("FreeBSD cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    Ok(local)
}

fn input_fingerprint(tag: &str, value: &str, inputs: &[&Path]) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(tag.as_bytes());
    hash.update(value.as_bytes());
    for input in inputs {
        if input.is_file() {
            hash.update(input.to_string_lossy().as_bytes());
            hash.update(fs::read(input)?);
            continue;
        }
        if !input.is_dir() {
            hash.update(b"missing:");
            hash.update(input.to_string_lossy().as_bytes());
            continue;
        }
        let mut files = walkdir::WalkDir::new(input)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        files.retain(|entry| {
            if !entry.file_type().is_file() {
                return false;
            }
            let name = entry.file_name().to_string_lossy();
            let path = entry.path().to_string_lossy();
            !path.contains("/.git/")
                && !path.contains("/target/")
                && !name.ends_with(".o")
                && !name.ends_with(".a")
                && !name.ends_with(".tmp")
        });
        files.sort_by_key(|entry| entry.path().to_path_buf());
        for entry in files {
            hash.update(
                entry
                    .path()
                    .strip_prefix(input)?
                    .to_string_lossy()
                    .as_bytes(),
            );
            hash.update(fs::read(entry.path())?);
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// True once the site has activated a release. Before its first activation there is no
/// `current` link, nothing runs, and no relay exists to pause or compare against.
fn has_active_release(config: &Config) -> Result<bool> {
    let state = output(
        "ssh",
        &[&config.control_host, "if [ -L __SITE_RELEASE_ROOT__/current ]; then echo active; else echo none; fi"],
    )?;
    Ok(state.trim() == "active")
}

fn active_release_id(config: &Config) -> Result<String> {
    let current = output(
        "ssh",
        &[&config.control_host, "readlink __SITE_RELEASE_ROOT__/current"],
    )?;
    let current = current
        .trim()
        .strip_prefix("releases/")
        .context("current production link is not a release")?;
    let Some((release_version, release_hash)) = current.rsplit_once('-') else {
        bail!("current production release id is malformed: {current}");
    };
    if release_version.split('.').count() != 3
        || !release_version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        || release_hash.len() != 12
        || !release_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("current production release id is malformed: {current}");
    }
    Ok(current.to_owned())
}

fn assemble_backend_only_release(
    paths: &Paths,
    binaries: &Path,
    version: &str,
    config: &Config,
    only: BackendOnly,
) -> Result<Release> {
    let current = active_release_id(config)?;
    let work = paths.root.join("dist/.release-backend-only");
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    fs::create_dir_all(&work)?;
    let source = format!(
        "{}:{}/{current}/",
        config.control_host,
        remote_releases()
    );
    let destination = format!("{}/", work.display());
    run(
        "rsync",
        &[
            "-rzc",
            "--delete",
            "--rsync-path=sudo rsync",
            &source,
            &destination,
        ],
        Some(&paths.root),
    )
    .context("copy active immutable release for backend-only update")?;

    if fs::read_to_string(work.join("VERSION"))?.trim() != version {
        bail!("a backend-only release requires .version to match the active public version");
    }
    require_file(&work.join("web/game-manifest.json"), "active game manifest")?;
    require_file(&work.join("maps/world/map.bin"), "active server map")?;
    require_file(&work.join("bin/sow-database"), "active database binary")?;
    require_file(&work.join("bin/sow-relay"), "active relay binary")?;

    let mut components = parse_components(&fs::read_to_string(work.join("COMPONENTS"))?);
    let new_relay = match only {
        BackendOnly::Server => {
            install_release_binary(binaries, &work, "sow-server")?;
            components.insert("server".into(), file_sha256(&work.join("bin/sow-server"))?);
            None
        }
        BackendOnly::Relay => {
            for (component, binary) in [("server", "sow-server"), ("database", "sow-database")] {
                let candidate = file_sha256(&binaries.join(binary))?;
                if components.get(component) != Some(&candidate) {
                    bail!("relay-only build changes {component}; publish compatible components together with ./sow p");
                }
            }
            let (hash, manifest) = package_relay(paths, binaries, &work)?;
            components.insert("relay".into(), hash);
            Some(manifest)
        }
    };
    components.insert("client".into(), sow_core::protocol::gameplay_build_identity());
    let component_names = ["web", "maps", "server", "database", "ops", "relay", "client"];
    let component_text = component_names
        .iter()
        .map(|name| {
            components
                .get(*name)
                .map(|hash| format!("{name}={hash}"))
                .with_context(|| format!("active release is missing {name} component hash"))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    fs::write(work.join("COMPONENTS"), format!("{component_text}\n"))?;
    fs::write(work.join("VERSION"), format!("{version}\n"))?;

    let source = super::source_identity(paths)?;
    let revision = output("git", &["rev-parse", "--short=12", "HEAD"])?;
    let dirty_files = workspace_dirty_files(paths)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(work.join("web/game-manifest.json"))?)?;
    let web_source_sha = manifest
        .get("source_sha")
        .and_then(serde_json::Value::as_str)
        .context("active game manifest has no source revision")?;
    let release_json_path = work.join("release.json");
    let mut release_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&release_json_path)?)?;
    release_json["version"] = json!(version);
    release_json["git"] = json!(revision.trim());
    release_json["source_sha"] = json!(source.sha);
    release_json["source_url"] = json!(source.url);
    release_json["web_source_sha"] = json!(web_source_sha);
    release_json["server_only"] = json!(only == BackendOnly::Server);
    if let Some(relay) = new_relay {
        release_json["relay"] = relay;
    }
    release_json["dirty"] = json!(!dirty_files.is_empty());
    release_json["dirty_files"] = json!(dirty_files);
    release_json["components"] = json!(component_names
        .iter()
        .map(|name| json!({"name": name, "sha256": components[*name]}))
        .collect::<Vec<_>>());
    fs::write(&release_json_path, serde_json::to_vec_pretty(&release_json)?)?;

    println!("  carrying forward all components except {} from {current}", only.name());
    seal_release(paths, &work, version)
}

fn assemble_release(paths: &Paths, web: &Path, binaries: &Path, version: &str) -> Result<Release> {
    let revision = output("git", &["rev-parse", "--short=12", "HEAD"])?;
    let source = super::source_identity(paths)?;
    let dirty_files = workspace_dirty_files(paths)?;
    let work = paths.root.join("dist/.release");
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    fs::create_dir_all(work.join("bin"))?;
    copy_dir(web, &work.join("web"))?;
    let web_maps = work.join("web/maps");
    if !web_maps.is_dir() {
        bail!("release web package is missing optimized maps");
    }
    fs::rename(&web_maps, work.join("maps"))?;
    for name in ["sow-server", "sow-database"] {
        install_release_binary(binaries, &work, name)?;
    }
    copy_dir(
        &paths.root.join("sow-dist/deploy/freebsd/rc.d"),
        &work.join("ops/rc.d"),
    )?;
    copy_dir(
        &paths.root.join("sow-dist/deploy/freebsd/conf.d"),
        &work.join("ops/conf.d"),
    )?;
    copy_dir(
        &paths.root.join("sow-dist/deploy/freebsd/snippets"),
        &work.join("ops/snippets"),
    )?;
    let (relay_component, relay_manifest) = package_relay(paths, binaries, &work)?;

    let nginx_source = paths
        .root
        .join("sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf");
    let nginx_site = fs::read_to_string(&nginx_source)
        .with_context(|| format!("read nginx template {}", nginx_source.display()))?;
    let loopback = site::current().loopback();
    let nginx_site = nginx_site
        .replace("__SOW_DB_LISTEN_HOST__", loopback)
        .replace("__SOW_SERVER_LISTEN_HOST__", loopback)
        .replace("__SOW_MAPS_LISTEN_HOST__", loopback);
    let nginx_site = site::render(&strip_production_only(&nginx_site)?)?;
    if nginx_site.contains("__SOW_") {
        bail!("nginx placeholder was not rendered");
    }
    // conf.d was copied whole, including the production template. Install only this site's file.
    fs::remove_file(work.join("ops/conf.d/shadowsofwar.io.conf"))?;
    fs::write(work.join("ops/conf.d").join(site::current().nginx_conf()), nginx_site)?;
    render_site_files(&work.join("ops"))?;

    require_file(&work.join("web/index.html"), "website index")?;
    require_file(&work.join("web/play/index.html"), "game index")?;
    require_file(&work.join("web/robots.txt"), "robots.txt")?;
    require_file(&work.join("web/sitemap.xml"), "sitemap.xml")?;
    require_file(&work.join("web/game-manifest.json"), "game manifest")?;
    require_file(&work.join("maps/world/map.bin"), "server map")?;
    require_file(&work.join("maps/NOTICE"), "map attribution notice")?;
    let game_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(work.join("web/game-manifest.json"))?)?;
    if game_manifest
        .get("source_sha")
        .and_then(serde_json::Value::as_str)
        != Some(source.sha.as_str())
        || game_manifest
            .get("source_url")
            .and_then(serde_json::Value::as_str)
            != Some(source.url.as_str())
        || !fs::read_to_string(work.join("web/play/index.html"))?.contains(&source.url)
    {
        bail!("game source link does not match the release source revision");
    }

    let components = [
        ("web", component_hash(&work.join("web"))?),
        ("maps", component_hash(&work.join("maps"))?),
        // Server/database rows must be raw content hashes (like the relay bin):
        // component_hash() folds the absolute work path into its fingerprint and
        // can drift from the actual bytes, which silently skips the jail restart
        // that ships new AI or protocol code to players.
        ("server", file_sha256(&work.join("bin/sow-server"))?),
        ("database", file_sha256(&work.join("bin/sow-database"))?),
        ("ops", component_hash(&work.join("ops"))?),
        ("relay", relay_component),
        ("client", sow_core::protocol::gameplay_build_identity()),
    ];
    let component_text = components
        .iter()
        .map(|(name, hash)| format!("{name}={hash}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(work.join("COMPONENTS"), format!("{component_text}\n"))?;
    fs::write(work.join("VERSION"), format!("{version}\n"))?;
    fs::write(
        work.join("release.json"),
        serde_json::to_vec_pretty(&json!({
            "version": version,
            "git": revision,
            "source_sha": source.sha,
            "source_url": source.url,
            "web_source_sha": game_manifest["source_sha"],
            "dirty": !dirty_files.is_empty(),
            "dirty_files": dirty_files,
            "components": components.iter().map(|(name, hash)| json!({"name": name, "sha256": hash})).collect::<Vec<_>>(),
            "relay": relay_manifest,
        }))?,
    )?;
    seal_release(paths, &work, version)
}

fn install_release_binary(binaries: &Path, work: &Path, name: &str) -> Result<()> {
    let source = binaries.join(name);
    let destination = work.join("bin").join(name);
    let temporary = destination.with_extension("tmp");
    fs::copy(&source, &temporary)
        .with_context(|| format!("copy {} to {}", source.display(), temporary.display()))?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o550))?;
    fs::rename(temporary, destination)?;
    Ok(())
}

fn package_relay(paths: &Paths, binaries: &Path, work: &Path) -> Result<(String, serde_json::Value)> {
    install_release_binary(binaries, work, "sow-relay")?;
    let relay_ops = work.join("relay-ops");
    if relay_ops.exists() {
        fs::remove_dir_all(&relay_ops)?;
    }
    copy_dir(&paths.root.join("sow-dist/deploy/freebsd/relay"), &relay_ops)?;
    let timeout = relay::settings(&relay_ops)?;
    let bin_sha = file_sha256(&work.join("bin/sow-relay"))?;
    let component = format!("{:x}", Sha256::digest(
        format!("{bin_sha}:{}", directory_sha256(&relay_ops)?).as_bytes(),
    ));
    let manifest = json!({
        "backend": "tokio",
        "sha256": component,
        "bin_sha256": bin_sha,
        "ws_write_timeout_ms": timeout,
        "drain": "one immutable relay process per match",
    });
    Ok((component, manifest))
}

fn seal_release(paths: &Paths, work: &Path, version: &str) -> Result<Release> {
    let manifest = write_manifest(work)?;
    let id = format!("{version}-{}", &file_sha256(&manifest)?[..12]);
    let releases = paths.root.join("dist/releases");
    fs::create_dir_all(&releases)?;
    let dir = releases.join(&id);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    if fs::rename(work, &dir).is_err() {
        copy_dir(work, &dir)?;
        fs::remove_dir_all(work)?;
    }
    Ok(Release {
        id,
        version: version.to_string(),
        dir,
    })
}

fn directory_sha256(root: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    files.retain(|entry| entry.file_type().is_file());
    files.sort_by_key(|entry| entry.path().to_path_buf());
    let mut hash = Sha256::new();
    for entry in files {
        hash.update(
            entry
                .path()
                .strip_prefix(root)?
                .to_string_lossy()
                .as_bytes(),
        );
        hash.update(fs::read(entry.path())?);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn component_hash(path: &Path) -> Result<String> {
    input_fingerprint("component-v1", "", &[path])
}

fn write_manifest(root: &Path) -> Result<PathBuf> {
    let mut files = walkdir::WalkDir::new(root)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    files.retain(|entry| entry.file_type().is_file() && entry.file_name() != "SHA256");
    files.sort_by_key(|entry| entry.path().to_path_buf());
    let mut contents = String::new();
    for entry in files {
        let relative = entry
            .path()
            .strip_prefix(root)?
            .to_str()
            .context("release path is not UTF-8")?;
        contents.push_str(&format!("{}  {relative}\n", file_sha256(entry.path())?));
    }
    let path = root.join("SHA256");
    fs::write(&path, contents)?;
    Ok(path)
}

fn remote_plan(config: &Config, release: &Release) -> Result<ComponentPlan> {
    // A first activation has no active release to compare with, so every component is new.
    if !has_active_release(config)? {
        return Ok(ComponentPlan { web: true, maps: true, server: true, database: true, ops: true, relay: true, client: false });
    }
    let remote = output("ssh", &[&config.control_host, "cat __SITE_RELEASE_ROOT__/current/COMPONENTS"])?;
    let current = parse_components(&remote);
    let local = parse_components(&fs::read_to_string(release.dir.join("COMPONENTS"))?);
    let changed = |component| current.get(component) != local.get(component);
    Ok(ComponentPlan {
        web: changed("web"), maps: changed("maps"), server: changed("server"),
        database: changed("database"), ops: changed("ops"), relay: changed("relay"),
        client: changed("client"),
    })
}

fn control_certificate_automation_drift(config: &Config) -> Result<bool> {
    let command = r#"if [ "$(sudo sysrc -n -f /etc/periodic.conf.local weekly_certbot_enable 2>/dev/null || true)" = YES ] \
        && [ "$(sudo sysrc -n -f /etc/periodic.conf.local weekly_certbot_deploy_hook 2>/dev/null || true)" = 'service nginx reload' ] \
        && sudo grep -Fq 'location ^~ /.well-known/acme-challenge/' /usr/local/etc/nginx/conf.d/__SITE_NGINX_CONF__ \
        && sudo grep -Fq 'root /usr/local/www/letsencrypt;' /usr/local/etc/nginx/conf.d/__SITE_NGINX_CONF__ \
        && sudo grep -Fq 'periodic weekly' /etc/crontab \
        && sudo find /usr/local/etc/periodic/weekly -maxdepth 1 -type f -name '500.certbot-*' | grep -q . \
        && [ "$(sudo sysrc -n cron_enable 2>/dev/null || true)" = YES ] \
        && sudo service cron onestatus >/dev/null 2>&1; then echo clean; else echo drift; fi"#;
    let state = output("ssh", &[&config.control_host, command])?;
    Ok(state.trim() == "drift")
}

fn verify_control_certificates(config: &Config) -> Result<()> {
    if control_certificate_automation_drift(config)? {
        bail!("IONOS certificate renewal schedule or validation route is not installed");
    }
    let command = r#"set -eu
for cert in \
    /usr/local/etc/letsencrypt/live/ionicswap.com/cert.pem \
    /usr/local/etc/letsencrypt/live/mail.worldofunreal.com/cert.pem \
    /usr/local/etc/letsencrypt/live/__SITE_HOST__/cert.pem; do
    sudo openssl x509 -in "$cert" -noout -checkend 2592000 >/dev/null || {
        echo "certificate is missing or expires within 30 days: $cert" >&2
        exit 1
    }
done
for name in __SITE_TLS_NAMES__; do
    sudo openssl x509 -in /usr/local/etc/letsencrypt/live/__SITE_HOST__/cert.pem \
        -noout -checkhost "$name" >/dev/null
done"#;
    run("ssh", &[&config.control_host, command], None)
        .context("IONOS certificate validity check failed")?;
    println!("  IONOS certificates valid; renewal is handled by the weekly certbot schedule");
    Ok(())
}

fn stripe_env_leftover(config: &Config) -> Result<bool> {
    let command = format!(
        r#"for f in __SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env; do
    if sudo test -f "$f" && sudo grep -qE '{STRIPE_ENV_KEYS}' "$f"; then echo "$f"; fi
done"#
    );
    Ok(!output("ssh", &[&config.control_host, &command])?
        .trim()
        .is_empty())
}

fn maps_catalog_path_drift(config: &Config) -> Result<bool> {
    let expected = env_or("SOW_MAPS_CATALOG_PATH", "/var/db/sow/server/catalog.bin");
    let remote = output(
        "ssh",
        &[
            &config.control_host,
            r#"for f in __SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env; do if sudo test -f "$f"; then sudo awk -F= '$1=="SOW_MAPS_CATALOG_PATH"{print $2}' "$f"; fi; done"#,
        ],
    )?;
    let values = remote.lines().map(str::trim).collect::<Vec<_>>();
    Ok(values.len() != 3 || values.iter().any(|value| *value != expected))
}

fn auth_config_drift(config: &Config) -> Result<bool> {
    let expected = hex::encode(Sha256::digest(auth_env_contents()?.as_bytes()));
    let remote = output("ssh", &[&config.control_host,
        "sudo test -s __SITE_DATABASE_ROOT__/etc/resolv.conf && sudo test -s __SITE_DATABASE_ROOT__/etc/ssl/cert.pem && sudo sha256 -q __SITE_DATABASE_ROOT__/usr/local/etc/sow/auth.env 2>/dev/null || true"])?;
    if remote.trim() != expected { return Ok(true); }
    Ok(false)
}

fn play_games_config_drift(config: &Config) -> Result<bool> {
    let expected_app_id = env::var("SOW_PLAY_GAMES_APP_ID")?;
    let expected_client_id = env::var("SOW_PLAY_GAMES_WEB_CLIENT_ID")?;
    let expected_event_id = env::var("SOW_PLAY_GAMES_MATCH_EVENT_ID").unwrap_or_default();
    let expected_achievements = [
        "SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID",
        "SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID",
    ]
    .into_iter()
    .map(env::var)
    .collect::<Result<Vec<_>, _>>()?;
    let expected_leaderboard_id = env::var("SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID")?;
    let remote = output(
        "ssh",
        &[
            &config.control_host,
            r#"for f in __SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env; do if sudo test -f "$f"; then app=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_APP_ID"{print $2}' "$f"); client=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_WEB_CLIENT_ID"{print $2}' "$f"); event=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_MATCH_EVENT_ID"{print $2}' "$f"); first=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID"{print $2}' "$f"); battle=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID"{print $2}' "$f"); victory=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID"{print $2}' "$f"); laurel=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID"{print $2}' "$f"); command=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID"{print $2}' "$f"); commander=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID"{print $2}' "$f"); veteran=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID"{print $2}' "$f"); banner=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID"{print $2}' "$f"); leader=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID"{print $2}' "$f"); leaderboard=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID"{print $2}' "$f"); secret=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_WEB_CLIENT_SECRET" && length($2)>0{print "set"}' "$f"); printf '%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s\n' "$app" "$client" "$event" "$first" "$battle" "$victory" "$laurel" "$command" "$commander" "$veteran" "$banner" "$leader" "$leaderboard" "$secret"; fi; done"#,
        ],
    )?;
    let mut expected_fields = vec![expected_app_id, expected_client_id, expected_event_id];
    expected_fields.extend(expected_achievements);
    expected_fields.push(expected_leaderboard_id);
    expected_fields.push("set".to_string());
    let expected_line = expected_fields.join("|");
    let mut expected_without_secret = expected_fields;
    let _ = expected_without_secret.pop();
    expected_without_secret.push(String::new());
    let expected = vec![
        expected_line.clone(),
        expected_line,
        expected_without_secret.join("|"),
    ];
    Ok(remote.lines().map(str::trim).collect::<Vec<_>>() != expected)
}

fn parse_components(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn stage_release(config: &Config, release: &Release) -> Result<()> {
    let stage = format!("{}/release", config.remote_stage.trim_end_matches('/'));
    let prepare = format!(
        "install -d -m 0700 {0} {0}/bin {0}/maps {0}/ops {0}/web",
        shell_quote(&stage)
    );
    run("ssh", &[&config.control_host, &prepare], None)?;
    let source = format!("{}/", release.dir.display());
    let destination = format!("{}:{}/", config.control_host, stage);
    // The control host link resets now and then mid-copy; this copy only fills staging, so retry it.
    let mut attempt = 1;
    loop {
        match run(
            "rsync",
            &["-azc", "--delete", &source, &destination],
            Some(&release.dir),
        ) {
            Err(error) if attempt < 3 => {
                println!("  staging copy failed (attempt {attempt}/3): {error:#}; retrying");
                attempt += 1;
            }
            result => return result,
        }
    }
}

fn activate_control_host(
    paths: &Paths,
    config: &Config,
    release: &Release,
    plan: &ComponentPlan,
) -> Result<()> {
    let stage = format!("{}/release", config.remote_stage.trim_end_matches('/'));
    let target = format!("{}/{}", remote_releases(), release.id);
    let server_restart = plan.server || plan.maps || plan.ops;
    let db_restart = plan.database || plan.ops;
    let nginx_reload = plan.ops;
    let runtime_env = plan.server || plan.database || plan.ops;
    let (db_secret_file, control_secret_file) = if runtime_env {
        let db_secret = env::var("SOW_DB_SECRET")?;
        let control_secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
        let db_path = format!("/tmp/sow-db-secret-{}", std::process::id());
        let control_path = format!("/tmp/sow-relay-control-secret-{}", std::process::id());
        stage_secret(&config.control_host, &db_secret, &db_path)?;
        stage_secret(&config.control_host, &control_secret, &control_path)?;
        (Some(db_path), Some(control_path))
    } else {
        (None, None)
    };
    let settlement_secret_file = if runtime_env {
        let secret = env::var("SOW_RELAY_SETTLEMENT_SECRET")?;
        let path = format!("/tmp/sow-relay-settlement-secret-{}", std::process::id());
        stage_secret(&config.control_host, &secret, &path)?;
        Some(path)
    } else {
        None
    };
    let revenuecat_webhook_file = if runtime_env {
        env::var("SOW_REVENUECAT_WEBHOOK_SECRET")
            .ok()
            .filter(|secret| !secret.trim().is_empty())
            .map(|secret| {
                let path = format!("/tmp/sow-revenuecat-webhook-{}", std::process::id());
                stage_secret(&config.control_host, &secret, &path).map(|_| path)
            })
            .transpose()?
    } else {
        None
    };
    let play_games_web_client_secret_file = if runtime_env {
        let secret = env::var("SOW_PLAY_GAMES_WEB_CLIENT_SECRET")?;
        let path = format!("/tmp/sow-play-games-client-secret-{}", std::process::id());
        stage_secret(&config.control_host, &secret, &path)?;
        Some(path)
    } else {
        None
    };
    let auth_env_file = if runtime_env {
        let path = format!("/tmp/sow-auth-env-{}", std::process::id());
        stage_secret(&config.control_host, &auth_env_contents()?, &path)?;
        Some(path)
    } else { None };
    let relay_host = relay::name();
    let relay_workers = format!("-:{relay_host}:{}", relay::PORT);
    let relay_worker_count = "1";
    let relay_mgmt_url = format!("https://{relay_host}:{}", relay::PORT);
    let relay_mgmt_scheme = "https";
    let relay_mgmt_resolve_ip = "__SITE_LOOPBACK__";
    let maps_root = env_or("SOW_MAPS_ROOT", "/srv/sow/current/maps");
    let maps_catalog_path = env_or("SOW_MAPS_CATALOG_PATH", "/var/db/sow/server/catalog.bin");
    // Uploaded maps live outside the immutable release so a deploy never deletes them.
    let maps_upload_root = env_or("SOW_MAPS_UPLOAD_ROOT", "/var/db/sow/server/maps-uploaded");
    let play_games_app_id = env::var("SOW_PLAY_GAMES_APP_ID")?;
    let play_games_client_id = env::var("SOW_PLAY_GAMES_WEB_CLIENT_ID")?;
    let play_games_match_event_id = env::var("SOW_PLAY_GAMES_MATCH_EVENT_ID").unwrap_or_default();
    let play_games_achievement_id = env::var("SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID")?;
    let play_games_battle_hardened_id = env::var("SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID")?;
    let play_games_victory_march_id = env::var("SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID")?;
    let play_games_laurel_hoard_id = env::var("SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID")?;
    let play_games_first_command_id = env::var("SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID")?;
    let play_games_commander_victorious_id =
        env::var("SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID")?;
    let play_games_veteran_commander_id =
        env::var("SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID")?;
    let play_games_banner_collector_id =
        env::var("SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID")?;
    let play_games_leader_path_id = env::var("SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID")?;
    let play_games_leaderboard_id = env::var("SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID")?;
    let maps_reload_token_file = if runtime_env {
        let token = env::var("SOW_MAPS_RELOAD_TOKEN")?;
        let path = format!("/tmp/sow-maps-reload-token-{}", std::process::id());
        stage_secret(&config.control_host, &token, &path)?;
        Some(path)
    } else {
        None
    };
    let env_update_base = if runtime_env {
        format!(
            "mkdir -p /tmp/sow-env-update; for f in __SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env; do t=$(mktemp /tmp/sow.env.XXXXXX); if sudo test -f \"$f\"; then sudo grep -v -E '^(SOW_MAPS_ROOT|SOW_MAPS_CATALOG_PATH|SOW_MAPS_UPLOAD_ROOT|SOW_MAPS_RELOAD_TOKEN|SOW_DB_SECRET|SOW_RELAY_CONTROL_SECRET|SOW_RELAY_HOST|SOW_RELAY_WORKERS|SOW_RELAY_WORKER_COUNT|SOW_RELAY_MGMT_URL|SOW_RELAY_MGMT_SCHEME|SOW_RELAY_MGMT_RESOLVE_IP|SOW_SERVER_HTTP_URL|SOW_CRAZYGAMES_GAME_ID|WOU_[A-Z_]+|SOW_PLAY_GAMES_APP_ID|SOW_PLAY_GAMES_WEB_CLIENT_ID|SOW_PLAY_GAMES_MATCH_EVENT_ID|SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID|SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID|SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID|SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID|SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID|SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID|SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID|SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID|SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID|SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID|SOW_PLAY_GAMES_WEB_CLIENT_SECRET)=|^[0-9a-fA-F]{{64}}$' \"$f\" > \"$t\" || true; else : > \"$t\"; fi; printf '%s\\n' SOW_MAPS_ROOT={maps_root} SOW_MAPS_CATALOG_PATH={maps_catalog_path} SOW_MAPS_UPLOAD_ROOT={maps_upload_root} SOW_RELAY_HOST={relay_host} SOW_RELAY_WORKERS={relay_workers} SOW_RELAY_WORKER_COUNT={relay_worker_count} SOW_RELAY_MGMT_URL={relay_mgmt_url} SOW_RELAY_MGMT_SCHEME={relay_mgmt_scheme} SOW_RELAY_MGMT_RESOLVE_IP={relay_mgmt_resolve_ip} SOW_SERVER_HTTP_URL={server_http_url} SOW_CRAZYGAMES_GAME_ID={crazygames_game_id} SOW_PLAY_GAMES_APP_ID={play_games_app_id} SOW_PLAY_GAMES_WEB_CLIENT_ID={play_games_client_id} SOW_PLAY_GAMES_MATCH_EVENT_ID={play_games_match_event_id} SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID={play_games_achievement_id} SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID={play_games_battle_hardened_id} SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID={play_games_victory_march_id} SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID={play_games_laurel_hoard_id} SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID={play_games_first_command_id} SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID={play_games_commander_victorious_id} SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID={play_games_veteran_commander_id} SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID={play_games_banner_collector_id} SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID={play_games_leader_path_id} SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID={play_games_leaderboard_id} | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_DB_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {db_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_RELAY_CONTROL_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {control_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_PLAY_GAMES_WEB_CLIENT_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {play_games_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_MAPS_RELOAD_TOKEN=' | sudo tee -a \"$t\" >/dev/null; sudo cat {maps_reload_token} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; done; rm -rf /tmp/sow-env-update",
            maps_root = shell_quote(&maps_root),
            maps_catalog_path = shell_quote(&maps_catalog_path),
            maps_upload_root = shell_quote(&maps_upload_root),
            maps_reload_token = shell_quote(maps_reload_token_file.as_deref().unwrap()),
            relay_host = shell_quote(&relay_host),
            relay_workers = shell_quote(&relay_workers),
            relay_worker_count = shell_quote(&relay_worker_count),
            relay_mgmt_url = shell_quote(&relay_mgmt_url),
            relay_mgmt_scheme = shell_quote(&relay_mgmt_scheme),
            relay_mgmt_resolve_ip = shell_quote(&relay_mgmt_resolve_ip),
            server_http_url = shell_quote(&site::current().server_http_url()),
            crazygames_game_id = shell_quote(&env::var("SOW_CRAZYGAMES_GAME_ID").unwrap_or_default()),
            play_games_app_id = shell_quote(&play_games_app_id),
            play_games_client_id = shell_quote(&play_games_client_id),
            play_games_match_event_id = shell_quote(&play_games_match_event_id),
            play_games_achievement_id = shell_quote(&play_games_achievement_id),
            play_games_battle_hardened_id = shell_quote(&play_games_battle_hardened_id),
            play_games_victory_march_id = shell_quote(&play_games_victory_march_id),
            play_games_laurel_hoard_id = shell_quote(&play_games_laurel_hoard_id),
            play_games_first_command_id = shell_quote(&play_games_first_command_id),
            play_games_commander_victorious_id = shell_quote(&play_games_commander_victorious_id),
            play_games_veteran_commander_id = shell_quote(&play_games_veteran_commander_id),
            play_games_banner_collector_id = shell_quote(&play_games_banner_collector_id),
            play_games_leader_path_id = shell_quote(&play_games_leader_path_id),
            play_games_leaderboard_id = shell_quote(&play_games_leaderboard_id),
            db_secret = shell_quote(db_secret_file.as_deref().unwrap()),
            control_secret = shell_quote(control_secret_file.as_deref().unwrap()),
            play_games_secret = shell_quote(play_games_web_client_secret_file.as_deref().unwrap()),
        )
    } else {
        ":".to_string()
    };
    let env_update = env_update_base;
    let auth_update = if let Some(path) = &auth_env_file {
        format!("sudo install -o root -g wheel -m 0600 {} __SITE_DATABASE_ROOT__/usr/local/etc/sow/auth.env", shell_quote(path))
    } else { ":".to_owned() };
    let settlement_secret_update = if let Some(secret_file) = settlement_secret_file.as_deref() {
        format!(
            r#"for f in __SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env; do
    t=$(mktemp /tmp/sow-settlement-env.XXXXXX)
    if sudo test -f "$f"; then sudo grep -v '^SOW_RELAY_SETTLEMENT_SECRET=' "$f" > "$t" || true; else : > "$t"; fi
    if [ "$f" = __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env ]; then
        printf '%s' 'SOW_RELAY_SETTLEMENT_SECRET=' >> "$t"
        sudo cat {secret_file} >> "$t"
        printf '\n' >> "$t"
    fi
    sudo install -o root -g wheel -m 0600 "$t" "$f"
    rm -f "$t"
done"#,
            secret_file = shell_quote(secret_file),
        )
    } else {
        ":".to_string()
    };
    let revenuecat_update = if let Some(secret_file) = revenuecat_webhook_file.as_deref() {
        format!(
            "for f in __SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env; do if sudo test -f \"$f\"; then sudo cp -p \"$f\" \"$f.bak_$(date +%s)\"; fi; t=$(mktemp /tmp/sow.env.XXXXXX); if sudo test -f \"$f\"; then sudo grep -v '^SOW_REVENUECAT_WEBHOOK_SECRET=' \"$f\" > \"$t\" || true; else : > \"$t\"; fi; printf '%s' 'SOW_REVENUECAT_WEBHOOK_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {secret_file} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; done; rm -f {secret_file}",
            secret_file = shell_quote(secret_file),
        )
    } else {
        ":".to_string()
    };
    let stripe_scrub = if runtime_env {
        format!(
            r#"for f in __SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env; do
    if sudo test -f "$f" && sudo grep -qE '{STRIPE_ENV_KEYS}' "$f"; then
        sudo cp -p "$f" "$f.bak_$(date +%s)"
        t=$(mktemp /tmp/sow.env.XXXXXX)
        sudo grep -vE '{STRIPE_ENV_KEYS}' "$f" > "$t" || true
        sudo install -o root -g wheel -m 0600 "$t" "$f"
        rm -f "$t"
    fi
done"#
        )
    } else {
        ":".to_string()
    };
    let secret_files = [
        db_secret_file.as_deref(),
        control_secret_file.as_deref(),
        settlement_secret_file.as_deref(),
        revenuecat_webhook_file.as_deref(),
        play_games_web_client_secret_file.as_deref(),
        auth_env_file.as_deref(),
        maps_reload_token_file.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(shell_quote)
    .collect::<Vec<_>>();
    let secret_cleanup = if runtime_env {
        format!("rm -f {}", secret_files.join(" "))
    } else {
        ":".to_string()
    };
    let database_secret_scrub = if runtime_env {
        r#"db_env=__SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env
if sudo test -f "$db_env"; then
    sudo cp -p "$db_env" "$db_env.bak_$(date +%s)"
    db_tmp=$(mktemp /tmp/sow-db-env.XXXXXX)
    sudo grep -v '^SOW_PLAY_GAMES_WEB_CLIENT_SECRET=' "$db_env" > "$db_tmp" || true
    sudo install -o root -g wheel -m 0600 "$db_tmp" "$db_env"
    rm -f "$db_tmp"
fi"#
        .to_string()
    } else {
        ":".to_string()
    };
    let mut resolver_update = String::new();
    for (root, restart) in [(site::current().server_root(), server_restart), (site::current().database_root(), db_restart)] {
        if restart {
            resolver_update.push_str(&format!("sudo install -o root -g wheel -m 0644 /etc/resolv.conf {root}/etc/resolv.conf\n"));
            resolver_update.push_str(&format!("sudo install -d -m 0755 {root}/etc/ssl\n"));
            resolver_update.push_str(&format!("sudo install -o root -g wheel -m 0644 /etc/ssl/cert.pem {root}/etc/ssl/cert.pem\n"));
        }
    }
    if resolver_update.is_empty() { resolver_update.push(':'); }
    let config_files = "__SITE_HOST_ETC__/sow.env __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/sow.env __SITE_DATABASE_ROOT__/usr/local/etc/sow/auth.env __SITE_SERVER_ROOT__/etc/resolv.conf __SITE_SERVER_ROOT__/etc/ssl/cert.pem __SITE_DATABASE_ROOT__/etc/resolv.conf __SITE_DATABASE_ROOT__/etc/ssl/cert.pem __SITE_SERVER_ROOT__/usr/local/etc/rc.d/sow_server __SITE_DATABASE_ROOT__/usr/local/etc/rc.d/sow_database";
    let env_backups = if runtime_env {
        format!(r#"backup=$(sudo mktemp -d /tmp/sow-config-backup.XXXXXX)
for f in {config_files}; do
    if sudo test -f "$f"; then
        sudo mkdir -p "$backup$(dirname "$f")"
        sudo cp -p "$f" "$backup$f"
    fi
done
backup_ready=true"#)
    } else { ":".to_owned() };
    let env_restore = if runtime_env {
        format!(r#"if [ "$backup_ready" = true ]; then
    for f in {config_files}; do
        if sudo test -f "$backup$f"; then sudo cp -p "$backup$f" "$f"; else sudo rm -f "$f"; fi
    done
fi"#)
    } else { ":".to_owned() };
    let mut remote = String::from(
        r#"set -eu
old=$(sudo readlink __SITE_RELEASE_ROOT__/current 2>/dev/null || true)
target=__TARGET__
stage=__STAGE__
backup=
backup_ready=false
rollback() {
    trap - EXIT
    set +e
    __ENV_RESTORE__
    sudo rm -f __SITE_RELEASE_ROOT__/current
    if [ -n "$old" ]; then sudo ln -s "$old" __SITE_RELEASE_ROOT__/current; fi
    if __DB_RESTART__; then sudo jexec __SITE_DATABASE_JAIL__ service sow_database restart || true; fi
    if __SERVER_RESTART__; then sudo jexec __SITE_SERVER_JAIL__ service sow_server restart || true; fi
    __SECRET_CLEANUP__
    exit 78
}
__SECRET_TRAP__
sudo install -d -m 0755 __RELEASES__
if sudo test -d "$target"; then
    sudo cmp -s "$stage/SHA256" "$target/SHA256" || rollback
else
    pending=$(sudo mktemp -d "$target.pending.XXXXXX")
    sudo cp -Rp "$stage/." "$pending/"
    sudo chown -R root:sow "$pending"
    sudo find "$pending" -type d -exec chmod 0755 {} +
    sudo find "$pending" -type f -exec chmod 0644 {} +
    sudo chmod 0550 "$pending"/bin/*
    sudo mv "$pending" "$target"
fi
# Only a previous release has a running server and verifier checkpoints to retire.
if __SERVER_RESTART__ && [ -n "$old" ]; then
    sudo jexec __SITE_SERVER_JAIL__ service sow_server stop || rollback
    if ! sudo jexec __SITE_SERVER_JAIL__ /bin/sh -s <<'ARCHIVE_SETTLED_STATES'
set -eu
. /usr/local/etc/sow/sow.env
root=${SOW_VERIFIER_DIR:-$(dirname "$SOW_MAPS_CATALOG_PATH")/verified}
archive="$root/retired_$(date +%s)"
created=false
for file in "$root"/*.checkpoint "$root"/*.checkpoint.previous; do
    [ -f "$file" ] || continue
    if [ "$created" = false ]; then mkdir -m 0700 "$archive"; created=true; fi
    mv "$file" "$archive/"
done
ARCHIVE_SETTLED_STATES
    then
        rollback
    fi
fi
__ENV_BACKUPS__
sudo install -o root -g wheel -m 0555 "$target/ops/rc.d/sow_server" __SITE_SERVER_ROOT__/usr/local/etc/rc.d/sow_server
sudo install -o root -g wheel -m 0555 "$target/ops/rc.d/sow_database" __SITE_DATABASE_ROOT__/usr/local/etc/rc.d/sow_database
link="__SITE_RELEASE_ROOT__/.current.$$"
sudo ln -s "releases/__ID__" "$link"
sudo mv -hf "$link" __SITE_RELEASE_ROOT__/current
if __NGINX_RELOAD__; then
    for f in "$target"/ops/conf.d/*; do
        [ -f "$f" ] || continue
        dest="/usr/local/etc/nginx/conf.d/$(basename "$f")"
        if sudo test -f "$dest"; then sudo cp -p "$dest" "$dest.bak_$(date +%s)"; fi
        sudo install -o root -g wheel -m 0644 "$f" "$dest"
    done
    for f in "$target"/ops/snippets/*; do
        [ -f "$f" ] || continue
        dest="/usr/local/etc/nginx/snippets/$(basename "$f")"
        if sudo test -f "$dest"; then sudo cp -p "$dest" "$dest.bak_$(date +%s)"; fi
        sudo install -o root -g wheel -m 0644 "$f" "$dest"
    done
    dest=/etc/periodic.conf.local
    if [ "$(sudo sysrc -n -f "$dest" weekly_certbot_enable 2>/dev/null || true)" != YES ] \
        || [ "$(sudo sysrc -n -f "$dest" weekly_certbot_deploy_hook 2>/dev/null || true)" != 'service nginx reload' ]; then
        if sudo test -f "$dest"; then sudo cp -p "$dest" "$dest.bak_$(date +%s)"; fi
        sudo sysrc -f "$dest" weekly_certbot_enable=YES >/dev/null
        sudo sysrc -f "$dest" 'weekly_certbot_deploy_hook=service nginx reload' >/dev/null
    fi
    if [ "$(sudo sysrc -n cron_enable 2>/dev/null || true)" != YES ]; then
        if sudo test -f /etc/rc.conf; then sudo cp -p /etc/rc.conf "/etc/rc.conf.bak_$(date +%s)"; fi
        sudo sysrc cron_enable=YES >/dev/null
    fi
    sudo service cron onestatus >/dev/null 2>&1 || sudo service cron onestart
    sudo nginx -t || rollback
    sudo service nginx reload || rollback
fi
__ENV_UPDATE__
__AUTH_UPDATE__
__SETTLEMENT_SECRET_UPDATE__
__REVENUECAT_UPDATE__
__STRIPE_SCRUB__
__DATABASE_SECRET_SCRUB__
__RESOLVER_UPDATE__
if __DB_RESTART__; then
    sudo jexec __SITE_DATABASE_JAIL__ service sow_database restart || rollback
    i=0
    until curl -fsS --max-time 5 http://__SITE_LOOPBACK__:25585/healthz >/dev/null; do
        i=$((i+1))
        if [ "$i" -ge 180 ]; then echo "database healthz did not become ready before server restart" >&2; rollback; fi
        sleep 1
    done
fi
if __SERVER_RESTART__; then sudo jexec __SITE_SERVER_JAIL__ service sow_server restart || rollback; fi
if ! sudo jexec __SITE_DATABASE_JAIL__ service sow_database status >/dev/null 2>&1; then rollback; fi
if ! sudo jexec __SITE_SERVER_JAIL__ service sow_server status >/dev/null 2>&1; then rollback; fi
__SECRET_CLEANUP__
"#,
    );
    let replacements = [
        ("__TARGET__", shell_quote(&target)),
        ("__STAGE__", shell_quote(&stage)),
        ("__RELEASES__", shell_quote(&remote_releases())),
        ("__ID__", release.id.clone()),
        (
            "__DB_RESTART__",
            if db_restart {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        (
            "__SERVER_RESTART__",
            if server_restart {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        (
            "__NGINX_RELOAD__",
            if nginx_reload {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        ("__ENV_UPDATE__", env_update),
        ("__AUTH_UPDATE__", auth_update),
        ("__ENV_RESTORE__", env_restore),
        ("__SETTLEMENT_SECRET_UPDATE__", settlement_secret_update),
        ("__ENV_BACKUPS__", env_backups),
        ("__REVENUECAT_UPDATE__", revenuecat_update),
        ("__STRIPE_SCRUB__", stripe_scrub),
        ("__DATABASE_SECRET_SCRUB__", database_secret_scrub),
        ("__RESOLVER_UPDATE__", resolver_update),
        (
            "__SECRET_TRAP__",
            if runtime_env {
                format!("trap 'status=$?; if [ \"$status\" -ne 0 ]; then rollback; fi; rm -f {}' EXIT", secret_files.join(" "))
            } else {
                ":".to_string()
            },
        ),
        ("__SECRET_CLEANUP__", secret_cleanup),
    ];
    for (token, value) in replacements {
        remote = remote.replace(token, &value);
    }
    run("ssh", &[&config.control_host, &remote], Some(&paths.root))
        .context("granular control-host activation failed")
}

fn stage_secret(host: &str, secret: &str, remote_path: &str) -> Result<()> {
    let mut file = tempfile::NamedTempFile::new().context("create secret staging file")?;
    file.write_all(secret.as_bytes())?;
    file.as_file().sync_all()?;
    fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))?;
    let local_path = file.path().to_str().context("secret path is not UTF-8")?;
    run(
        "scp",
        &["-q", local_path, &format!("{host}:{remote_path}")],
        None,
    )
}

fn verify_control_host(config: &Config, plan: &ComponentPlan) -> Result<()> {
    let mut checks = String::from(
        r#"set -eu
fail() {
    echo "CONTROL HEALTHCHECK FAILED: $1" >&2
    echo "--- database.log (tail) ---" >&2
    sudo tail -80 __SITE_LOG_DIR__/database.log >&2 || true
    echo "--- server.log (tail) ---" >&2
    sudo tail -120 __SITE_LOG_DIR__/server.log >&2 || true
    exit 1
}
if ! sudo jexec __SITE_DATABASE_JAIL__ service sow_database status; then fail "database service is not running"; fi
if ! sudo jexec __SITE_SERVER_JAIL__ service sow_server status; then fail "server service is not running"; fi
i=0
until curl -fsS --max-time 5 http://__SITE_LOOPBACK__:25585/healthz >/dev/null; do
    i=$((i+1))
    if [ "$i" -ge 180 ]; then fail "database healthz timeout"; fi
    sleep 1
done
valkey_url=$(sudo awk -F= '$1=="SOW_VALKEY_URL"{print substr($0,index($0,"=")+1)}' __SITE_SERVER_ROOT__/usr/local/etc/sow/sow.env)
if ! /usr/local/bin/valkey-cli -u "$valkey_url" ping | grep -q PONG; then fail "valkey ping failed"; fi
j=0
until sudo sockstat -4l | grep -q '__SITE_LOOPBACK__:25564'; do
    j=$((j+1))
    if [ "$j" -ge 180 ]; then fail "server websocket port timeout"; fi
    sleep 1
done
"#,
    );
    if plan.server {
        checks.push_str(
            r#"
bot_line=$(sudo tail -500 __SITE_LOG_DIR__/server.log | grep '\[BOT_POOL\].*installed with' | tail -1 || true)
echo "  $bot_line"
if ! printf '%s\n' "$bot_line" | grep -Eq 'installed with [1-9][0-9]* identities'; then
    fail "Ghost bot pool was not installed with identities"
fi"#,
        );
    }
    if plan.web || plan.ops {
        checks.push_str("\nsudo nginx -t");
    }
    checks.push_str(
        r#"
test "$(sudo sysrc -n -f /etc/periodic.conf.local weekly_certbot_enable)" = YES
test "$(sudo sysrc -n -f /etc/periodic.conf.local weekly_certbot_deploy_hook)" = 'service nginx reload'
sudo service cron onestatus >/dev/null
"#,
    );
    run("ssh", &[&config.control_host, &checks], None).context("control-host healthcheck failed")
}

/// The live server must publish every map its release ships. A server that boots with an
/// empty catalog still answers health checks but aborts every lobby, so the deploy fails here
/// and the rollback restores the previous release.
fn verify_map_catalog(config: &Config, release: &Release) -> Result<()> {
    let expected = release_map_count(release)?;
    let server_log = format!("{}/server.log", site::current().log_dir());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    loop {
        let lines = output(
            "ssh",
            &[
                &config.control_host,
                &format!("sudo grep -E 'SERVER-BOOT|Map catalog: ' {server_log} | tail -n 200 || true"),
            ],
        )?;
        match latest_map_catalog_entries(&lines) {
            Some(count) if count >= expected => {
                println!("  map catalog: {count} entries (release ships {expected})");
                return Ok(());
            }
            Some(count) => bail!(
                "server published {count} map catalog entries; release {} ships {expected}",
                release.id
            ),
            None if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
            None => bail!("server published no map catalog after its latest boot"),
        }
    }
}

/// Entry count from the newest `Map catalog:` line since the latest `SERVER-BOOT`. `None` when
/// the current boot has not published a catalog yet, or the publish failed.
fn latest_map_catalog_entries(log: &str) -> Option<usize> {
    let mut entries = None;
    for line in log.lines() {
        if line.contains("[SERVER-BOOT]") {
            entries = None;
        }
        if let Some((_, report)) = line.split_once("Map catalog: ") {
            entries = report.split_whitespace().next().and_then(|count| count.parse().ok());
        }
    }
    entries
}

/// Number of maps the release ships: the `[recipes]` allowlist in its `maps/SOURCES.toml`.
fn release_map_count(release: &Release) -> Result<usize> {
    let sources: toml::Value = fs::read_to_string(release.dir.join("maps/SOURCES.toml"))
        .with_context(|| format!("read map allowlist of release {}", release.id))?
        .parse()
        .context("parse release map allowlist")?;
    let count = sources
        .get("recipes")
        .and_then(toml::Value::as_table)
        .map_or(0, |recipes| recipes.len());
    if count == 0 {
        bail!("release {} ships no maps", release.id);
    }
    Ok(count)
}

fn retain_releases(config: &Config) -> Result<()> {
    let command = r#"set -eu
current=$(sudo readlink __SITE_RELEASE_ROOT__/current)
live=$(sudo jexec __SITE_SERVER_JAIL__ sh -c 'pids=$(pgrep -x sow-relay || true); if [ -n "$pids" ]; then procstat -b $pids; fi' | awk '{ path=$NF; prefix="/srv/sow/releases/"; suffix="/bin/sow-relay"; if (substr(path,1,length(prefix)) == prefix && substr(path,length(path)-length(suffix)+1) == suffix) print "__SITE_RELEASE_ROOT__/releases/" substr(path,length(prefix)+1,length(path)-length(prefix)-length(suffix)); }')
i=0
for dir in $(sudo ls -dt __SITE_RELEASE_ROOT__/releases/* 2>/dev/null || true); do
    i=$((i+1))
    [ "$i" -le 5 ] && continue
    [ "__SITE_RELEASE_ROOT__/$current" = "$dir" ] && continue
    if printf '%s\n' "$live" | grep -Fxq "$dir"; then continue; fi
    case "$dir" in __SITE_RELEASE_ROOT__/releases/*) sudo rm -rf -- "$dir";; esac
done"#;
    run("ssh", &[&config.control_host, command], None).context("release retention failed")
}

/// Same rule as the server: keep the 5 newest local releases, and drop
/// wasm-opt cache entries not touched since the oldest kept release was
/// built (cache hits refresh their mtime).
fn retain_local_builds(paths: &Paths) -> Result<()> {
    let mtime = |p: &Path| fs::metadata(p).and_then(|m| m.modified());
    let mut releases: Vec<_> = fs::read_dir(paths.root.join("dist/releases"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter_map(|p| mtime(&p).ok().map(|t| (t, p)))
        .collect();
    releases.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, old) in releases.iter().skip(5) {
        fs::remove_dir_all(old)?;
    }
    let Some((cutoff, _)) = releases.iter().take(5).last() else {
        return Ok(());
    };
    if let Ok(entries) = fs::read_dir(&paths.wasm_cache) {
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            if mtime(&path).is_ok_and(|t| t < *cutoff) {
                fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

fn verify_public(paths: &Paths, config: &Config, release: &Release) -> Result<()> {
    let local_manifest = fs::read_to_string(release.dir.join("web/game-manifest.json"))?;
    let url = format!("{}/game-manifest.json", config.public_origin);
    let manifest = output("curl", &["-fsS", "--max-time", "20", &url])
        .context("public game manifest request failed")?;
    if manifest.trim() != local_manifest.trim() {
        bail!("public domain does not serve {}", release.id);
    }

    let local_assets: serde_json::Value = serde_json::from_str(&local_manifest)?;
    for (field, expected_type) in [
        ("js", "application/javascript"),
        ("wasm", "application/wasm"),
    ] {
        let asset = local_assets[field]
            .as_str()
            .with_context(|| format!("game manifest missing {field}"))?;
        let url = format!("{}/{asset}", config.public_origin);
        let headers = output(
            "curl",
            &[
                "-fsS",
                "--max-time",
                "20",
                "--head",
                "-H",
                "Accept-Encoding: br",
                &url,
            ],
        )?;
        let content_type = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-type")
                    .then(|| value.split(';').next().unwrap_or_default().trim())
            })
            .unwrap_or_default();
        if content_type != expected_type {
            bail!("public {asset} must be served as {expected_type}, got {content_type}");
        }
    }

    let local_assetlinks_path = release.dir.join("web/.well-known/assetlinks.json");
    let local_assetlinks = fs::read_to_string(&local_assetlinks_path)
        .with_context(|| format!("read {}", local_assetlinks_path.display()))?;
    serde_json::from_str::<serde_json::Value>(&local_assetlinks)
        .context("release assetlinks.json is not valid JSON")?;
    let assetlinks_url = format!("{}/.well-known/assetlinks.json", config.public_origin);
    let headers = output(
        "curl",
        &[
            "-fsS",
            "--max-time",
            "20",
            "-D",
            "-",
            "-o",
            "/dev/null",
            &assetlinks_url,
        ],
    )?;
    let content_type = headers
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("content-type:"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !content_type.contains("application/json") {
        bail!("public assetlinks.json must be served as application/json");
    }
    let public_assetlinks = output("curl", &["-fsS", "--max-time", "20", &assetlinks_url])
        .context("public assetlinks.json request failed")?;
    if public_assetlinks.trim() != local_assetlinks.trim() {
        bail!(
            "public assetlinks.json does not match release {}",
            release.id
        );
    }

    for url in [
        format!("{}/", config.public_origin),
        format!("{}/play/", config.public_origin),
    ] {
        run(
            "curl",
            &["-fsS", "--max-time", "20", "-o", "/dev/null", &url],
            Some(&paths.root),
        )
        .with_context(|| format!("public origin check failed for {url}"))?;
    }
    println!("  public origin and assetlinks verified for {}", release.id);
    Ok(())
}

#[cfg(test)]
mod map_catalog_gate_tests {
    use super::latest_map_catalog_entries;

    #[test]
    fn catalog_gate_reads_only_the_current_boot() {
        let empty = "[SERVER-BOOT] build=1\n[x INFO] Map catalog: 0 entries → /var/db/c\n";
        assert_eq!(latest_map_catalog_entries(empty), Some(0));
        let loaded = "[SERVER-BOOT] build=1\n[x INFO] Map catalog: 15 entries → /var/db/c\n";
        assert_eq!(latest_map_catalog_entries(loaded), Some(15));
        let rebooted = "[SERVER-BOOT] build=1\n[x INFO] Map catalog: 15 entries → p\n[SERVER-BOOT] build=2\n";
        assert_eq!(latest_map_catalog_entries(rebooted), None);
        let failed = "[SERVER-BOOT] build=2\n[x ERROR] Map catalog: failed to write /var/db/c\n";
        assert_eq!(latest_map_catalog_entries(failed), None);
    }
}

#[cfg(test)]
mod build_fingerprint_tests {
    use super::freebsd_build_fingerprint;
    use std::fs;

    #[test]
    fn freebsd_cache_tracks_relay_atlas_and_bundled_world_map() {
        let root = tempfile::tempdir().unwrap();
        let relay = root.path().join("sow-relay/src/main.rs");
        fs::create_dir_all(relay.parent().unwrap()).unwrap();
        fs::write(&relay, b"relay-v1").unwrap();
        let atlas = root.path().join("assets/geo_entities.json");
        let map = root.path().join("assets/maps/world/map.bin.br");
        let game_speed = root.path().join("sow-dist/config/game-speed.json");
        fs::create_dir_all(map.parent().unwrap()).unwrap();
        fs::create_dir_all(game_speed.parent().unwrap()).unwrap();
        fs::write(&atlas, b"atlas-v1").unwrap();
        fs::write(&map, b"map-v1").unwrap();
        fs::write(&game_speed, b"speed-v1").unwrap();

        let before = freebsd_build_fingerprint(root.path()).unwrap();
        fs::write(&atlas, b"atlas-v2").unwrap();
        let after_atlas = freebsd_build_fingerprint(root.path()).unwrap();
        assert_ne!(before, after_atlas);

        fs::write(&map, b"map-v2").unwrap();
        let after_map = freebsd_build_fingerprint(root.path()).unwrap();
        assert_ne!(after_atlas, after_map);

        fs::write(&game_speed, b"speed-v2").unwrap();
        let after_game_speed = freebsd_build_fingerprint(root.path()).unwrap();
        assert_ne!(after_map, after_game_speed);

        fs::write(&relay, b"relay-v2").unwrap();
        assert_ne!(
            after_game_speed,
            freebsd_build_fingerprint(root.path()).unwrap()
        );
    }
}
