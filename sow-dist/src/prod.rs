use super::*;
use hmac::{Hmac, Mac};
use serde_json::json;
use std::collections::BTreeMap;

const BUILD_HOST: &str = "freebsd";
const BUILD_ROOT: &str = "/home/bizkit/shadows-of-war";
const CONTROL_HOST: &str = "ionos";
const RELAY_HOST: &str = "relay";
const RELAY_USER: &str = "sowrelay";
const RELAY_GROUP: &str = "sowrelay";
const RELAY_EXEC: &str = "/usr/local/libexec/sow-relay/sow-relay";
const RELAY_CONFIG: &str = "/usr/local/etc/sow/echo-vf.ini";
const RELAY_STATE: &str = "/var/lib/sow-relay";
const RELAY_MANIFEST: &str = "/var/lib/sow-relay/manifest.json";
const RELAY_STAGE: &str = "/home/azureuser/.sow-deploy/relay";
const FSTACK_LOCAL_REPO: &str = "/home/bizkit/Github/infra/f-stack";
const FSTACK_RELEASE_REV: &str = "6a520b386d3f15265e54d87fe0e9fdc83d4da794";
const DPDK_RELEASE: &str = "24.11.8";
const FSTACK_BUILD_FLAGS: &str = "FF_ZC_RECV=1 FF_KERNEL_COEXIST=1";
const REMOTE_STAGE: &str = "/home/bizkit/.sow-deploy";
const REMOTE_RELEASES: &str = "/srv/sow/releases";
const PUBLIC_ORIGIN: &str = "https://shadowsofwar.io";
const SERVER_JAIL_IP: &str = "127.0.0.1";
const DATABASE_JAIL_IP: &str = "127.0.0.1";
const ANDROID_PROJECT: &str = "sow-dist/deploy/android";
const ANDROID_BUNDLE: &str = "dist/android/com.shadowsofwar.aab";
const ANDROID_VERSION_CODE: &str = ".android-version-code";

struct Config {
    build_host: String,
    build_root: String,
    control_host: String,
    relay_host: String,
    fstack_repo: String,
    remote_stage: String,
    public_origin: String,
}

impl Config {
    fn load() -> Self {
        Self {
            build_host: env_or("SOW_FREEBSD_BUILDER_HOST", BUILD_HOST),
            build_root: env_or("SOW_FREEBSD_BUILDER_ROOT", BUILD_ROOT),
            control_host: env_or("SOW_CONTROL_HOST", CONTROL_HOST),
            relay_host: env_or("SOW_RELAY_DEPLOY_HOST", RELAY_HOST),
            fstack_repo: env_or("SOW_FSTACK_REPO", FSTACK_LOCAL_REPO),
            remote_stage: env_or("SOW_REMOTE_STAGE", REMOTE_STAGE),
            public_origin: env_or("SOW_PUBLIC_ORIGIN", PUBLIC_ORIGIN)
                .trim_end_matches('/')
                .to_string(),
        }
    }
}

struct Release {
    id: String,
    version: String,
    dir: PathBuf,
}

#[derive(Default, Debug)]
struct ComponentPlan {
    web: bool,
    maps: bool,
    server: bool,
    database: bool,
    ops: bool,
    relay: bool,
}

impl ComponentPlan {
    fn any(&self) -> bool {
        self.web || self.maps || self.server || self.database || self.ops || self.relay
    }
}

struct RelayAdmissionPause<'a> {
    config: &'a Config,
    paused: bool,
}

impl<'a> RelayAdmissionPause<'a> {
    fn acquire(config: &'a Config, require_empty: bool) -> Result<Self> {
        if relay_workers_stopped(config)? {
            verify_no_pending_reward_calculations(config)?;
            return Ok(Self {
                config,
                paused: false,
            });
        }
        if !relay_supports_lobby_drain(config)? {
            bail!("backend update requires relay matchmaking pause support");
        }
        if let Err(error) = relay_set_lobby_drain(config, true) {
            return Err(resume_relay_after_error(config, error));
        }
        let pause = Self {
            config,
            paused: true,
        };
        if require_empty {
            let active = relay_active_match_count(config)?;
            if active != 0 {
                bail!(
                    "network-library replacement postponed: {active} active or closing matches remain"
                );
            }
            verify_no_pending_reward_calculations(config)?;
        }
        Ok(pause)
    }

    fn resume(&mut self) -> Result<()> {
        if self.paused {
            relay_set_lobby_drain(self.config, false)?;
            self.paused = false;
        }
        Ok(())
    }
}

fn verify_no_pending_reward_calculations(config: &Config) -> Result<()> {
    let script = r#"set -eu
secret=$(sudo awk -F= '$1 == "SOW_DB_SECRET" { sub(/^[^=]*=/, ""); print; exit }' /zroot/jails/sow-database/usr/local/etc/sow/sow.env)
test -n "$secret"
printf 'header = "Authorization: Bearer %s"\n' "$secret" | curl --noproxy '*' -fsS --max-time 10 --config - http://127.0.0.1:25585/internal/stats"#;
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

listing = jail("sow-server", "/bin/sh", "-c", '''
set -eu
. /usr/local/etc/sow/sow.env
root=${SOW_VERIFIER_DIR:-$(dirname "$SOW_MAPS_CATALOG_PATH")/verified}
for file in "$root"/*.checkpoint "$root"/*.checkpoint.previous; do
    [ ! -f "$file" ] || basename "$file"
done
''').splitlines()
ids = sorted({re.fullmatch(r"([0-9]+)\.checkpoint(?:\.previous)?", name).group(1) for name in listing})
if ids:
    secret = jail("sow-database", "/bin/sh", "-c", '. /usr/local/etc/sow/sow.env; printf "%s" "$SOW_DB_SECRET"')
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    for match_id in ids:
        request = urllib.request.Request("http://127.0.0.1:25585/internal/stats?match_id=" + match_id,
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

pub(super) fn execute(paths: &Paths, bump: bool) -> Result<()> {
    let config = Config::load();
    ensure_shared_identity_secret()?;
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
    require_secret("WOU_SOW_IDENTITY_SECRET")?;
    println!("==> 1/8 Preflight (read-only)");
    preflight(paths, &config)?;
    let version = version(paths, bump)?;
    println!("==> Production {version}");
    let runtime_inputs = [
        "Cargo.toml",
        "Cargo.lock",
        "sow-core",
        "sow-data",
        "sow-net",
        "sow-server",
        "sow-relay",
        "fstack-bridge",
        "assets/geo_entities.json",
        "assets/guides",
        "assets/maps/world/map.bin.br",
        "sow-dist/src",
        "sow-dist/deploy/freebsd",
        "sow-dist/deploy/linux",
    ]
    .map(|path| paths.root.join(path));
    let runtime_inputs = runtime_inputs
        .iter()
        .map(PathBuf::as_path)
        .collect::<Vec<_>>();
    let runtime_snapshot = input_fingerprint("production-runtime-v1", "", &runtime_inputs)?;
    // Android is deliberately NOT part of this pipeline: every Play upload
    // resets Google's review clock, so AAB publication lives in `./sow a`
    // and runs only when a new build is actually ready for review.

    println!("==> 2/8 Build candidates");
    let (web, backend, _relay) = std::thread::scope(|scope| {
        let web = scope.spawn(|| build_web(paths, &version));
        let backend = scope.spawn(|| build_freebsd(paths, &config));
        let relay = scope.spawn(|| build_relay(paths, &config));
        let web = web
            .join()
            .map_err(|_| anyhow::anyhow!("web build panicked"))??;
        let backend = backend
            .join()
            .map_err(|_| anyhow::anyhow!("FreeBSD build panicked"))??;
        let relay = relay
            .join()
            .map_err(|_| anyhow::anyhow!("relay build panicked"))??;
        Ok::<_, anyhow::Error>((web, backend, relay))
    })?;

    println!("==> 3/8 Package immutable release");
    let release = assemble_release(paths, &web, &backend, &version, &config)?;
    println!("  release {}", release.id);
    if input_fingerprint("production-runtime-v1", "", &runtime_inputs)? != runtime_snapshot {
        bail!(
            "runtime sources or deployment templates changed during the build; refusing a mixed release"
        );
    }

    println!("==> Runtime prerequisites");
    let relay_stopped = relay_workers_stopped(&config)?;
    if !relay_stopped {
        ensure_relay_runtime(&config)?;
        verify_relay_control_path(&config)?;
    }
    ensure_control_clock(&config)?;
    verify_control_runtime_secret(&config)?;

    println!("==> 4/8 Compare deployed manifest");
    let mut plan = remote_plan(&config, &release)?;
    if relay_stopped {
        println!("  relay workers stopped; recovering with the current release");
        plan.relay = true;
    }
    if control_certificate_automation_drift(&config)? {
        println!("  runtime drift: IONOS certificate renewal automation");
        plan.ops = true;
    }
    if maps_catalog_path_drift(&config)? {
        println!("  runtime env drift: SOW_MAPS_CATALOG_PATH");
        plan.ops = true;
    }
    if wou_id_config_drift(&config)? {
        println!("  runtime env drift: WOU_ID_URL/WOU_ID_RESOLVE_IP");
        plan.database = true;
    }
    if wou_identity_config_drift(&config)? {
        println!("  runtime env drift: WOU_SOW_IDENTITY_SECRET");
        plan.database = true;
    }
    if play_games_config_drift(&config)? {
        println!("  runtime env drift: Play Games Services configuration");
        plan.database = true;
    }
    if stripe_env_leftover(&config)? {
        println!("  runtime env drift: retired Stripe checkout keys still on the server");
        plan.database = true;
    }
    if relay_settlement_auth_drift(&config)? {
        println!("  runtime auth drift: relay-only reward settlement credential");
        plan.database = true;
        plan.relay = true;
    }
    println!("  plan: {plan:?}");

    if !plan.any() {
        println!("  no production component changed; no restart performed");
        verify_control_certificates(&config)?;
        verify_origin_tls_from_relay(&config)?;
        ensure_relay_certificate_automation(paths, &config)?;
        verify_relay_certificate_served(&config)?;
        verify_control_host(&config, &plan)?;
        verify_relay_runtime(&config)?;
        verify_control_runtime_secret(&config)?;
        verify_relay_settlement_auth(&config)?;
        verify_relay_control_path(&config)?;
        verify_relay_safe_lobby_drain(&config)?;
        verify_relay_identity(&config, &release)?;
        retain_releases(&config)?;
        retain_local_builds(paths)?;
        verify_public(paths, &config, &release)?;
        let handover_pending = relay_handover_pending(&config)?;
        verify_live_reward_return(paths, &config, handover_pending.then_some(RELAY_EXEC))?;
        if handover_pending {
            mark_relay_handover_verified(&config, &release)?;
        }
        println!("✅ Production already serves the requested content");
        return Ok(());
    }

    println!("==> 5/8 Stage release (no service mutation)");
    stage_release(&config, &release)?;
    if plan.relay {
        stage_relay(&config, &release)?;
    }
    if input_fingerprint("production-runtime-v1", "", &runtime_inputs)? != runtime_snapshot {
        bail!(
            "runtime sources or deployment templates changed before activation; refusing a mixed release"
        );
    }

    println!("==> 6/8 Activate changed components only");
    let hot = plan.relay && relay_can_handover(&config, &release)?;
    let mut admission = if plan.server || plan.database || plan.maps || plan.ops || plan.relay {
        Some(RelayAdmissionPause::acquire(
            &config,
            plan.server || plan.database || plan.maps || plan.ops || !hot,
        )?)
    } else {
        None
    };
    activate_control_host(paths, &config, &release, &plan)?;
    if !plan.relay || !relay_workers_stopped(&config)? {
        verify_relay_settlement_auth(&config)?;
    }
    verify_control_certificates(&config)?;
    verify_origin_tls_from_relay(&config)?;
    ensure_relay_certificate_automation(paths, &config)?;
    // Relay last, so the control host is healthy before relay workers change.
    if hot {
        verify_control_host(&config, &plan)?;
        if let Some(pause) = admission.as_mut() {
            pause.resume()?;
        }
        activate_relay_handover(paths, &config, &release)?;
    } else if plan.relay {
        activate_relay_host(&config, &release)?;
    }
    verify_relay_certificate_served(&config)?;

    println!("==> 7/8 Healthcheck and retain");
    verify_control_host(&config, &plan)?;
    verify_relay_runtime(&config)?;
    verify_control_runtime_secret(&config)?;
    verify_relay_settlement_auth(&config)?;
    verify_relay_control_path(&config)?;
    if let Some(pause) = admission.as_mut() {
        pause.resume()?;
    }
    verify_relay_safe_lobby_drain(&config)?;
    verify_relay_identity(&config, &release)?;
    retain_releases(&config)?;
    retain_local_builds(paths)?;

    println!("==> 8/8 Public verification");
    verify_public(paths, &config, &release)?;
    if !hot {
        let handover_pending = relay_handover_pending(&config)?;
        verify_live_reward_return(paths, &config, handover_pending.then_some(RELAY_EXEC))?;
        if handover_pending {
            mark_relay_handover_verified(&config, &release)?;
        }
    }
    println!("✅ Production {} ready as {}", release.version, release.id);
    Ok(())
}

fn verify_live_reward_return(
    paths: &Paths,
    config: &Config,
    handover_binary: Option<&str>,
) -> Result<()> {
    let mut websocket = url::Url::parse(&config.public_origin)?;
    let scheme = if websocket.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    websocket
        .set_scheme(scheme)
        .map_err(|_| anyhow::anyhow!("invalid public WebSocket scheme"))?;
    websocket.set_path("/ws/");
    let database = format!("{}/api", config.public_origin);
    let accounts = paths
        .root
        .join("dist/.sow-state/reward-audit-accounts.json");
    let mut args = vec![
        "run",
        "--locked",
        "-p",
        "sow-tools",
        "--bin",
        "test-relay",
        "--",
        "--verify-rewards",
        "--url",
        websocket.as_str(),
        "--database-url",
        &database,
        "--audit-accounts-file",
        accounts.to_str().context("invalid audit accounts path")?,
    ];
    if let Some(binary) = handover_binary {
        args.extend([
            "--verify-handover-relay-host",
            config.relay_host.as_str(),
            "--verify-handover-relay-binary",
            binary,
        ]);
    }
    run("cargo", &args, Some(&paths.root))
        .context("real multiplayer exit and reward presentation failed")
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

fn android_inputs(paths: &Paths) -> Vec<PathBuf> {
    let project = paths.root.join(ANDROID_PROJECT);
    vec![
        project.join("app/build.gradle"),
        project.join("app/proguard-rules.pro"),
        project.join("app/src/main"),
        project.join("build.gradle"),
        project.join("settings.gradle"),
        project.join("gradle.properties"),
        project.join("gradle/wrapper/gradle-wrapper.jar"),
        project.join("gradle/wrapper/gradle-wrapper.properties"),
        project.join("gradlew"),
        project.join("key.properties"),
    ]
}

fn android_fingerprint(paths: &Paths, version: &str, version_code: u32) -> Result<String> {
    let inputs = android_inputs(paths);
    let input_refs = inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    input_fingerprint(
        "android-v1",
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

fn preflight(paths: &Paths, config: &Config) -> Result<()> {
    for command in [
        "bun", "cargo", "curl", "magick", "node", "rsync", "rustc", "scp", "ssh", "wasm-opt",
    ] {
        if !Command::new("/bin/sh")
            .args(["-c", &format!("command -v {command} >/dev/null")])
            .status()?
            .success()
        {
            bail!("{command} is required");
        }
    }
    for (entry, context) in [
        (
            "sow-web/shell/sow-campaign.ts",
            "campaign engine TypeScript check failed",
        ),
        (
            "sow-web/shell/sow-campaign-view.ts",
            "campaign view TypeScript check failed",
        ),
        (
            "sow-web/shell/main_menu.menu-guide.ts",
            "menu guide TypeScript check failed",
        ),
    ] {
        run(
            "bun",
            &[
                "build",
                "--check",
                "--target=browser",
                "--format=iife",
                "--outfile=/dev/null",
                entry,
            ],
            Some(&paths.root),
        )
        .context(context)?;
    }
    super::pack_art_atlases(paths)?;
    super::validate_current_ui_contract(paths)?;
    run(
        "bun",
        &[
            "test",
            "sow-web/shell/menu_flow.test.ts",
            "sow-web/shell/map_interaction.test.ts",
            "sow-web/shell/loader_boot.test.ts",
            "sow-web/shell/menu_presentation.test.ts",
            "sow-web/shell/campaign_view.test.ts",
            "sow-web/shell/campaign_runtime.test.ts",
            "sow-web/shell/gameplay_ui.test.ts",
            "sow-web/shell/gameplay_interaction.test.ts",
        ],
        Some(&paths.root),
    )
    .context("shell UI regression tests failed")?;
    for filter in ["test_build_index_", "test_nginx_conf_", "ssh_transport_"] {
        run(
            "cargo",
            &["test", "--locked", "-p", "sow-dist", "--bin", "sow", filter],
            Some(&paths.root),
        )
        .context("distribution packaging regression tests failed")?;
    }
    run(
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "sow-core",
            "-p",
            "sow-data",
            "-p",
            "sow-server",
            "-p",
            "sow-render",
            "--features",
            "sow-data/server",
            "--",
            "--test-threads=1",
        ],
        Some(&paths.root),
    )
    .context("core economy, simulation and render regression tests failed")?;
    for (filter, failure) in [
        (
            "reward_exit_tests",
            "server-confirmed reward exit tests failed",
        ),
        ("hero_ability", "hero ability client tests failed"),
        ("status_badge", "nameplate status badge tests failed"),
        (
            "emote_tests",
            "player avatar expression and emote tests failed",
        ),
        ("troop_transfer", "troop transfer nameplate tests failed"),
        (
            "death_avatar",
            "death avatar presentation and loading tests failed",
        ),
    ] {
        run(
            "cargo",
            &[
                "test",
                "--locked",
                "-p",
                "sow-client",
                "--target",
                "wasm32-unknown-unknown",
                "--config",
                "target.wasm32-unknown-unknown.runner=\"wasm-bindgen-test-runner\"",
                "--lib",
                filter,
            ],
            Some(&paths.root),
        )
        .context(failure)?;
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
            "test -d /srv/sow/releases && command -v sudo >/dev/null",
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
    .context("FreeBSD builder preflight failed")?;
    run(
        "ssh",
        &[
            &config.relay_host,
            "command -v sudo >/dev/null && command -v systemctl >/dev/null && command -v curl >/dev/null",
        ],
        None,
    )
    .context("relay host preflight failed")
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
    let root = paths.root.to_str().context("workspace path is not UTF-8")?;
    let source = super::source_identity(paths)?;
    // AGPL §5(a): identify the modified source revision and its relevant date.
    let source_date = super::output("git", &["-C", root, "show", "-s", "--format=%cs", "HEAD"])?;
    let readme = fs::read_to_string(paths.root.join("README.md"))?;
    let modified_source_notice =
        format!("Modified from OpenFrontIO. Source revision date: `{source_date}`.");
    if !readme.contains(&modified_source_notice) {
        bail!(
            "README must identify the modified OpenFront source and its revision date ({source_date})"
        );
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

fn relay_worker_count() -> Result<usize> {
    let count = env_or("SOW_RELAY_WORKER_COUNT", "3")
        .parse::<usize>()
        .context("SOW_RELAY_WORKER_COUNT must be an integer")?;
    if !(1..=64).contains(&count) {
        bail!("SOW_RELAY_WORKER_COUNT must be between 1 and 64");
    }
    Ok(count)
}

fn relay_runtime_command(start_missing: bool) -> Result<String> {
    let recovery = if start_missing {
        "if [ \"$active\" -eq 0 ]; then for unit in $relay_units; do timeout 90s sudo systemctl start \"$unit\"; done; elif [ \"$active\" -ne \"$total\" ]; then echo 'partial relay failure; preserving surviving matches' >&2; exit 78; fi;"
    } else {
        ""
    };
    Ok(format!(
        r#"set -eu
relay_units=
active=0
total=0
for unit in $(systemctl show sow-relay.service -p Wants --value); do
  case "$unit" in sow-relay@*.service) relay_units="$relay_units $unit"; total=$((total+1));
    if systemctl is-active --quiet "$unit"; then active=$((active+1)); fi;; esac
done
test "$total" -gt 0
{recovery}
systemctl is-enabled --quiet sow-relay.service
systemctl is-active --quiet sow-relay.service
systemctl is-active --quiet chrony.service
test "$(timedatectl show -p NTPSynchronized --value)" = yes
for unit in $relay_units; do
  test "$(systemctl show "$unit" -p ActiveState --value)" = active
  test "$(systemctl show "$unit" -p Result --value)" = success
  test "$(systemctl show "$unit" -p ExecMainStatus --value)" = 0
done
count=$total
if grep -q -- --fstack-supervisor /usr/local/sbin/sow-relay-worker; then
  pid=$(systemctl show sow-relay@0.service -p MainPID --value)
  count=$(sudo cat "/proc/$pid/environ" | tr '\0' '\n' | sed -n 's/^SOW_RELAY_WORKER_COUNT=//p')
fi
test "$count" -gt 0
id=0
while [ "$id" -lt "$count" ]; do
  port=$((8080+id))
  curl -kfsS --max-time 5 https://127.0.0.1:$port/healthz >/dev/null
  id=$((id+1))
done
"#
    ))
}

fn ensure_relay_runtime(config: &Config) -> Result<()> {
    let command = relay_runtime_command(true)?;
    run("ssh", &[&config.relay_host, &command], None)
        .context("relay workers could not be made healthy")?;
    println!("  relay workers healthy");
    Ok(())
}

fn relay_workers_stopped(config: &Config) -> Result<bool> {
    let active = output(
        "ssh",
        &[
            &config.relay_host,
            "systemctl list-units --type=service --state=running --no-legend 'sow-relay@*.service'",
        ],
    )?;
    Ok(active.is_empty())
}

fn relay_current_worker_count(config: &Config) -> Result<usize> {
    let value = output(
        "ssh",
        &[
            &config.relay_host,
            r#"set -eu
if grep -q -- --fstack-supervisor /usr/local/sbin/sow-relay-worker; then
  pid=$(systemctl show sow-relay@0.service -p MainPID --value)
  if [ "$pid" -gt 1 ]; then
    sudo cat "/proc/$pid/environ" | tr '\0' '\n' | sed -n 's/^SOW_RELAY_WORKER_COUNT=//p'
  fi
else
  count=0
  for unit in $(systemctl show sow-relay.service -p Wants --value); do
    case "$unit" in sow-relay@*.service) count=$((count+1));; esac
  done
  echo "$count"
fi"#,
        ],
    )?;
    if value.is_empty() {
        return relay_worker_count();
    }
    let count: usize = value
        .parse()
        .context("running relay worker count is invalid")?;
    if !(1..=64).contains(&count) {
        bail!("running relay worker count is out of range");
    }
    Ok(count)
}

fn verify_relay_runtime(config: &Config) -> Result<()> {
    let command = relay_runtime_command(false)?;
    run("ssh", &[&config.relay_host, &command], None).context("relay worker healthcheck failed")
}

fn clock_skew_seconds(config: &Config) -> Result<u64> {
    let (control, relay) = std::thread::scope(|scope| {
        let control = scope.spawn(|| output("ssh", &[&config.control_host, "date -u +%s"]));
        let relay = scope.spawn(|| output("ssh", &[&config.relay_host, "date -u +%s"]));
        let control = control
            .join()
            .map_err(|_| anyhow::anyhow!("control clock check panicked"))??;
        let relay = relay
            .join()
            .map_err(|_| anyhow::anyhow!("relay clock check panicked"))??;
        Ok::<_, anyhow::Error>((control, relay))
    })?;
    let control = control
        .parse::<u64>()
        .context("control host returned an invalid UNIX timestamp")?;
    let relay = relay
        .parse::<u64>()
        .context("relay host returned an invalid UNIX timestamp")?;
    Ok(control.abs_diff(relay))
}

fn control_clock_configured(config: &Config) -> Result<bool> {
    let check = r#"cfg=/etc/rc.conf.d/ntpd; if sudo test -f "$cfg" && sudo grep -qx 'ntpd_enable="YES"' "$cfg" && sudo grep -qx 'ntpd_sync_on_start="YES"' "$cfg" && sudo service ntpd onestatus >/dev/null 2>&1; then echo ok; else echo drift; fi"#;
    Ok(output("ssh", &[&config.control_host, check])? == "ok")
}

fn ensure_control_clock(config: &Config) -> Result<()> {
    let configured = control_clock_configured(config)?;
    let skew = clock_skew_seconds(config)?;
    if !configured || skew > 5 {
        println!("  control clock drift detected ({skew}s); synchronizing through pipeline");
        let configure = r#"set -eu
cfg=/etc/rc.conf.d/ntpd
if ! sudo test -f "$cfg" || ! sudo grep -qx 'ntpd_enable="YES"' "$cfg" || ! sudo grep -qx 'ntpd_sync_on_start="YES"' "$cfg"; then
    if sudo test -e "$cfg"; then sudo cp -p "$cfg" "$cfg.bak_$(date +%s)"; fi
    tmp=$(mktemp /tmp/sow-ntpd.XXXXXX)
    trap 'rm -f "$tmp"' EXIT
    if sudo test -f "$cfg"; then sudo grep -v -E '^ntpd_(enable|sync_on_start)=' "$cfg" > "$tmp" || true; fi
    printf '%s\n' 'ntpd_enable="YES"' 'ntpd_sync_on_start="YES"' >> "$tmp"
    sudo install -o root -g wheel -m 0644 "$tmp" "$cfg"
fi
sudo service ntpd onestop >/dev/null 2>&1 || true
sudo /bin/timeout 60 /usr/sbin/ntpd -gq
sudo service ntpd onestart
sudo service ntpd onestatus >/dev/null"#;
        run("ssh", &[&config.control_host, configure], None)
            .context("control-host clock synchronization failed")?;
    }
    if !control_clock_configured(config)? {
        bail!("control-host NTP is not configured and running");
    }
    let skew = clock_skew_seconds(config)?;
    if skew > 5 {
        bail!("control/relay clock skew remains {skew}s after synchronization");
    }
    println!("  control/relay clock skew {skew}s");
    Ok(())
}

fn relay_management_workers() -> Result<Vec<(String, u16)>> {
    let raw = env::var("SOW_RELAY_WORKERS")
        .context("SOW_RELAY_WORKERS must configure every relay worker")?;
    let mut workers = Vec::new();
    for (id, spec) in raw.split(',').enumerate() {
        let fields = spec.trim().split(':').collect::<Vec<_>>();
        if fields.len() != 3 || fields[1].is_empty() {
            bail!("SOW_RELAY_WORKERS contains an invalid entry at index {id}");
        }
        let port = fields[2]
            .parse::<u16>()
            .with_context(|| format!("invalid relay management port at index {id}"))?;
        workers.push((fields[1].to_string(), port));
    }
    if workers.len() != relay_worker_count()? {
        bail!(
            "SOW_RELAY_WORKERS has {} entries but SOW_RELAY_WORKER_COUNT is {}",
            workers.len(),
            relay_worker_count()?
        );
    }
    Ok(workers)
}

fn verify_control_runtime_secret(config: &Config) -> Result<()> {
    let secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
    let expected = format!("{:x}", Sha256::digest(secret.as_bytes()));
    let command = r#"set -eu
pid=$(sudo jexec sow-server pgrep -xo sow-server)
value=$(sudo procstat -e "$pid" | tr ' ' '\n' | sed -n 's/^SOW_RELAY_CONTROL_SECRET=//p')
test -n "$value"
printf %s "$value" | sha256 -q"#;
    let actual = output("ssh", &[&config.control_host, command])?;
    if actual != expected {
        bail!("running sow-server relay control secret does not match deployment secret");
    }
    Ok(())
}

fn relay_settlement_auth_drift(config: &Config) -> Result<bool> {
    let secret = env::var("SOW_RELAY_SETTLEMENT_SECRET")?;
    let expected = format!("{:x}", Sha256::digest(secret.as_bytes()));
    let control_command = r#"set -eu
for spec in 'host:/usr/local/etc/sow/sow.env' 'server:/zroot/jails/sow-server/usr/local/etc/sow/sow.env' 'database:/zroot/jails/sow-database/usr/local/etc/sow/sow.env'; do
    label=${spec%%:*}
    file=${spec#*:}
    value=$(sudo awk -F= -v k='SOW_RELAY_SETTLEMENT_SECRET' '$1 == k { sub(/^[^=]*=/, ""); print; exit }' "$file" 2>/dev/null || true)
    if [ -n "$value" ]; then
        hash=$(printf %s "$value" | sha256 -q)
        printf '%s=%s\n' "$label" "$hash"
    else
        printf '%s=missing\n' "$label"
    fi
done"#;
    let control = output("ssh", &[&config.control_host, control_command])?;
    let expected_control = format!("host=missing\nserver=missing\ndatabase={expected}");

    let relay_env = RelayEnv::load()?;
    let ids = (0..relay_env.count)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let url = url::Url::parse(&relay_env.db_url)?;
    let host = url.host_str().context("SOW_DB_URL has no hostname")?;
    let port = url
        .port_or_known_default()
        .context("SOW_DB_URL has no port")?;
    let base = url.join("/")?.to_string();
    let base = base.trim_end_matches('/');
    let resolve = format!("{host}:{port}:{}", relay_env.db_resolve_ip);
    let relay_command = format!(
        r#"set -eu
base={base}
resolve={resolve}
for id in {ids}; do
    unit="sow-relay@$id.service"
    if grep -q -- --fstack-supervisor /usr/local/sbin/sow-relay-worker; then unit=sow-relay@0.service; fi
    pid=$(sudo systemctl show "$unit" -p MainPID --value)
    value=
    shared=missing
    if [ "$pid" -gt 1 ] && sudo test -r "/proc/$pid/environ"; then
        value=$(sudo cat "/proc/$pid/environ" | tr '\0' '\n' | awk -F= '$1 == "SOW_RELAY_SETTLEMENT_SECRET" {{ sub(/^[^=]*=/, ""); print; exit }}')
        if sudo cat "/proc/$pid/environ" | tr '\0' '\n' | grep -q '^SOW_DB_SECRET='; then shared=present; fi
    fi
    if [ -n "$value" ]; then
        hash=$(printf %s "$value" | sha256sum | awk '{{print $1}}')
        printf 'worker%s=%s\n' "$id" "$hash"
    else
        printf 'worker%s=missing\n' "$id"
    fi
    printf 'worker%s_shared=%s\n' "$id" "$shared"
done
pid=$(sudo systemctl show sow-relay@0.service -p MainPID --value)
probe=missing-worker
if [ "$pid" -gt 1 ] && sudo test -r "/proc/$pid/environ"; then
    secret=$(sudo cat "/proc/$pid/environ" | tr '\0' '\n' | awk -F= '$1 == "SOW_RELAY_SETTLEMENT_SECRET" {{ sub(/^[^=]*=/, ""); print; exit }}')
    if [ -n "$secret" ]; then
        probe=$(printf 'header = "Authorization: Bearer %s"\n' "$secret" | curl --noproxy '*' -sS --max-time 10 --config - --resolve "$resolve" -o /dev/null -w '%{{http_code}}|%{{remote_ip}}' -H 'Content-Type: application/octet-stream' --data-binary 'invalid' "$base/internal/match-turns" || true)
        probe=${{probe:-unreachable}}
    else
        probe=missing-secret
    fi
fi
printf 'active_match_turns=%s\n' "$probe"
"#,
        base = shell_quote(base),
        resolve = shell_quote(&resolve),
    );
    let relay = output("ssh", &[&config.relay_host, &relay_command])?;
    let expected_relay = (0..relay_env.count)
        .flat_map(|id| {
            [
                format!("worker{id}={expected}"),
                format!("worker{id}_shared=missing"),
            ]
        })
        .chain(std::iter::once(format!(
            "active_match_turns=400|{}",
            relay_env.db_resolve_ip
        )))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(control.trim() != expected_control || relay.trim() != expected_relay)
}

fn verify_relay_settlement_auth(config: &Config) -> Result<()> {
    if relay_settlement_auth_drift(config)? {
        bail!("relay-only reward settlement credential is missing, misplaced, or inactive");
    }
    let relay_env = RelayEnv::load()?;
    let url = url::Url::parse(&relay_env.db_url)?;
    let host = url.host_str().context("SOW_DB_URL has no hostname")?;
    let port = url
        .port_or_known_default()
        .context("SOW_DB_URL has no port")?;
    let base = url.join("/")?.to_string();
    let base = base.trim_end_matches('/');
    let resolve = format!("{host}:{port}:{}", relay_env.db_resolve_ip);
    // Invalid identifiers stop both handlers before any storage operation.
    let relay_command = format!(
        r#"set -eu
base={base}
resolve={resolve}
origin_ip={origin_ip}
pid=$(systemctl show sow-relay@0.service -p MainPID --value)
secret=$(sudo cat "/proc/$pid/environ" | tr '\0' '\n' | sed -n 's/^SOW_RELAY_SETTLEMENT_SECRET=//p')
test -n "$secret"
for route in match-turns; do
    status=$(printf 'header = "Authorization: Bearer %s"\n' "$secret" | curl --noproxy '*' -sS --max-time 10 --config - --resolve "$resolve" -o /dev/null -w '%{{http_code}}|%{{remote_ip}}' -H 'Content-Type: application/octet-stream' --data-binary 'invalid' "$base/internal/$route")
    if [ "$status" != "400|$origin_ip" ]; then echo "relay reward route rejected its dedicated credential or did not reach the pinned origin: $route ($status)" >&2; exit 1; fi
    status=$(curl --noproxy '*' -sS --max-time 10 --resolve "$resolve" -o /dev/null -w '%{{http_code}}' -H 'Content-Type: application/octet-stream' --data-binary 'invalid' "$base/internal/$route")
    if [ "$status" != 401 ]; then echo "relay reward route accepted a missing credential: $route ($status)" >&2; exit 1; fi
done"#,
        base = shell_quote(base),
        resolve = shell_quote(&resolve),
        origin_ip = shell_quote(&relay_env.db_resolve_ip),
    );
    run("ssh", &[&config.relay_host, &relay_command], None)
        .context("direct relay result route authentication failed")?;
    let control_command = format!(
        r#"set -eu
base={base}
resolve={resolve}
shared=$(sudo awk -F= '$1 == "SOW_DB_SECRET" {{ sub(/^[^=]*=/, ""); print; exit }}' /zroot/jails/sow-server/usr/local/etc/sow/sow.env)
test -n "$shared"
for route in match-turns; do
    status=$(printf 'header = "Authorization: Bearer %s"\n' "$shared" | curl --noproxy '*' -sS --max-time 10 --config - -o /dev/null -w '%{{http_code}}' -H 'Content-Type: application/octet-stream' --data-binary 'invalid' "http://{database_ip}:25585/internal/$route")
    if [ "$status" != 401 ]; then echo "reward route accepted the server's general database credential: $route ($status)" >&2; exit 1; fi
    status=$(curl --noproxy '*' -sS --max-time 10 --resolve "$resolve" -o /dev/null -w '%{{http_code}}' -H 'Content-Type: application/octet-stream' --data-binary 'invalid' "$base/internal/$route")
    if [ "$status" != 403 ]; then echo "reward route allowed a non-relay network source: $route ($status)" >&2; exit 1; fi
done"#,
        base = shell_quote(base),
        resolve = shell_quote(&format!("{host}:{port}:127.0.0.1")),
        database_ip = DATABASE_JAIL_IP,
    );
    run("ssh", &[&config.control_host, &control_command], None)
        .context("relay result route isolation failed")?;
    run("ssh", &[&config.control_host, r#"set -eu
for retired in 25585/internal/match-legacy-journal/invalid 25566/internal/replay/verify/invalid; do
    status=$(curl -sS --max-time 10 -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$retired")
    test "$status" = 404 || { echo "retired reward route still exists: $retired ($status)" >&2; exit 1; }
done"#], None).context("retired reward routes are still reachable")?;
    println!(
        "  relay reward routes use the pinned origin and reject missing, server, and non-relay access"
    );
    Ok(())
}

/// Authenticated GET to every relay worker's management port, executed from
/// the control host (the only host the Azure NSG lets through). Returns one
/// response body per worker, in worker-id order.
fn relay_authed_get(config: &Config, path: &str) -> Result<Vec<(usize, String)>> {
    relay_authed_request(config, "GET", path, "")
}

fn relay_authed_post_json(config: &Config, path: &str, body: &str) -> Result<Vec<(usize, String)>> {
    relay_authed_request(config, "POST", path, body)
}

fn relay_authed_request(
    config: &Config,
    method: &str,
    path: &str,
    body: &str,
) -> Result<Vec<(usize, String)>> {
    type HmacSha256 = Hmac<Sha256>;

    let secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
    if env_or("SOW_RELAY_MGMT_SCHEME", "https") != "https" {
        bail!("SOW_RELAY_MGMT_SCHEME must be https in production");
    }
    let resolve_ip = env::var("SOW_RELAY_MGMT_RESOLVE_IP")
        .context("SOW_RELAY_MGMT_RESOLVE_IP is required for relay verification")?;
    let mut responses = Vec::new();
    let mut workers = relay_management_workers()?;
    let count = relay_current_worker_count(config)?;
    let host = workers
        .first()
        .context("relay management workers missing")?
        .0
        .clone();
    workers.truncate(count);
    while workers.len() < count {
        workers.push((host.clone(), 8080 + workers.len() as u16));
    }
    for (worker_id, (host, port)) in workers.into_iter().enumerate() {
        let timestamp = output("ssh", &[&config.control_host, "date -u +%s"])?;
        let mut nonce_bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = hex::encode(nonce_bytes);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| anyhow::anyhow!("invalid relay control secret"))?;
        mac.update(method.as_bytes());
        mac.update(b"\n");
        mac.update(path.as_bytes());
        mac.update(b"\n");
        mac.update(timestamp.as_bytes());
        mac.update(b"\n");
        mac.update(nonce.as_bytes());
        mac.update(b"\n");
        mac.update(body.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let url = format!("https://{host}:{port}{path}");
        let resolve = format!("{host}:{port}:{resolve_ip}");
        let request = if method == "GET" {
            String::new()
        } else {
            format!(
                " -X {} -H {} --data-binary {}",
                shell_quote(method),
                shell_quote("Content-Type: application/json"),
                shell_quote(body)
            )
        };
        let command = format!(
            "curl --noproxy '*' -fsS --max-time 5 --resolve {} -H {} -H {} -H {}{} {}",
            shell_quote(&resolve),
            shell_quote(&format!("X-SOW-Timestamp: {timestamp}")),
            shell_quote(&format!("X-SOW-Nonce: {nonce}")),
            shell_quote(&format!("X-SOW-Signature: {signature}")),
            request,
            shell_quote(&url),
        );
        let response = output("ssh", &[&config.control_host, &command])
            .with_context(|| format!("authenticated relay probe failed for worker port {port}"))?;
        responses.push((worker_id, response));
    }
    Ok(responses)
}

fn relay_supports_lobby_drain(config: &Config) -> Result<bool> {
    let mut supported = Vec::new();
    for (worker_id, body) in relay_authed_get(config, "/internal/metrics")? {
        let metrics: serde_json::Value = serde_json::from_str(&body)
            .with_context(|| format!("worker port {worker_id} returned invalid metrics"))?;
        supported.push(
            metrics
                .get("lobby_drain_supported")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        );
    }
    if supported.iter().all(|value| *value) {
        return Ok(true);
    }
    if supported.iter().all(|value| !*value) {
        return Ok(false);
    }
    bail!("relay workers disagree about safe matchmaking pause support")
}

fn relay_active_match_count(config: &Config) -> Result<usize> {
    let mut count = 0;
    for (_, body) in relay_authed_get(config, "/internal/lobbies")? {
        let value: serde_json::Value = serde_json::from_str(&body)?;
        count += value["lobbies"]
            .as_array()
            .context("relay did not confirm its match registry")?
            .len();
    }
    Ok(count)
}

fn relay_can_handover(config: &Config, release: &Release) -> Result<bool> {
    // A saved manifest describes installed content, not a living coordinator.
    // Stopped services must use controlled recovery rather than live handover.
    if relay_workers_stopped(config)? {
        return Ok(false);
    }
    let candidate: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let Some(running) = relay_manifest_remote(config)? else {
        return Ok(false);
    };
    let expected = candidate["relay"]["foundation"]
        .as_str()
        .context("relay foundation missing")?;
    Ok(running["foundation"].as_str() == Some(expected)
        && running["dpdk"].as_str() == Some(DPDK_RELEASE))
}

fn relay_handover_pending(config: &Config) -> Result<bool> {
    let manifest = relay_manifest_remote(config)?.context("relay manifest missing")?;
    Ok(manifest["handover_verified"] != true)
}

fn mark_relay_handover_verified(config: &Config, release: &Release) -> Result<()> {
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let component = metadata["relay"]["sha256"]
        .as_str()
        .context("relay component hash missing")?;
    let binary = metadata["relay"]["bin_sha256"]
        .as_str()
        .context("relay binary hash missing")?;
    let command = format!(
        r#"sudo python3 - {manifest} {component} {binary} {exec} <<'VERIFY_RELAY_HANDOVER'
import hashlib, json, os, pathlib, shutil, sys, time
path = pathlib.Path(sys.argv[1])
manifest = json.loads(path.read_text())
if manifest.get("relay_sha256") != sys.argv[2] or manifest.get("relay_bin_sha256") != sys.argv[3]:
    raise RuntimeError("relay changed before handover acceptance was recorded")
if hashlib.sha256(pathlib.Path(sys.argv[4]).read_bytes()).hexdigest() != sys.argv[3]:
    raise RuntimeError("installed relay differs from the verified binary")
shutil.copy2(path, str(path) + ".bak_" + str(int(time.time())))
manifest["handover_verified"] = True
temporary = path.with_name(path.name + ".next")
temporary.write_text(json.dumps(manifest) + "\n")
os.chmod(temporary, 0o644)
os.replace(temporary, path)
VERIFY_RELAY_HANDOVER
"#,
        manifest = shell_quote(RELAY_MANIFEST),
        component = shell_quote(component),
        binary = shell_quote(binary),
        exec = shell_quote(RELAY_EXEC)
    );
    run("ssh", &[&config.relay_host, &command], None)
        .context("record relay connection handover acceptance")
}

fn relay_set_lobby_drain(config: &Config, draining: bool) -> Result<()> {
    let body = serde_json::json!({"draining":draining}).to_string();
    for (worker_id, response) in relay_authed_post_json(config, "/internal/drain", &body)? {
        let value: serde_json::Value = serde_json::from_str(&response)
            .with_context(|| format!("worker port {worker_id} returned invalid drain status"))?;
        if value.get("draining").and_then(serde_json::Value::as_bool) != Some(draining) {
            bail!("relay worker port {worker_id} did not apply matchmaking drain={draining}");
        }
    }
    Ok(())
}

fn resume_relay_after_error(config: &Config, error: anyhow::Error) -> anyhow::Error {
    if let Err(resume_error) = relay_set_lobby_drain(config, false) {
        return error.context(format!(
            "failed to resume relay matchmaking after deployment precondition failed: {resume_error:#}"
        ));
    }
    error
}

fn verify_relay_control_path(config: &Config) -> Result<()> {
    let path = "/internal/metrics";
    let responses = relay_authed_get(config, path)?;
    let worker_count = responses.len();
    for (worker_id, body) in responses {
        let metrics: serde_json::Value = serde_json::from_str(&body)
            .with_context(|| format!("worker port {worker_id} returned invalid metrics"))?;
        if metrics.get("queue_id").and_then(serde_json::Value::as_u64) != Some(worker_id as u64)
            || metrics
                .get("queue_count")
                .and_then(serde_json::Value::as_u64)
                != Some(worker_count as u64)
        {
            bail!(
                "relay worker port {worker_id} reports queue_id/queue_count {:?}/{:?}, expected {worker_id}/{worker_count}",
                metrics.get("queue_id"),
                metrics.get("queue_count")
            );
        }
    }
    println!("  authenticated IONOS -> relay control path verified");
    Ok(())
}

fn verify_relay_safe_lobby_drain(config: &Config) -> Result<()> {
    if !relay_supports_lobby_drain(config)? {
        bail!("relay workers do not support safe matchmaking pause");
    }
    relay_set_lobby_drain(config, false)?;
    println!("  authenticated relay matchmaking pause verified");
    Ok(())
}

fn stage_relay(config: &Config, release: &Release) -> Result<()> {
    let source = format!("{}/", release.dir.join("relay").display());
    let destination = format!("{}:{}", config.relay_host, RELAY_STAGE);
    run(
        "ssh",
        &[
            &config.relay_host,
            &format!("install -d -m 0700 {}", shell_quote(RELAY_STAGE)),
        ],
        None,
    )?;
    run(
        "rsync",
        &["-azc", "--delete", &source, &destination],
        Some(&release.dir),
    )?;
    Ok(())
}

fn activate_relay_handover(paths: &Paths, config: &Config, release: &Release) -> Result<()> {
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let relay = &metadata["relay"];
    let binary = relay["bin_sha256"]
        .as_str()
        .context("relay binary hash missing")?;
    if binary.len() != 64 || !binary.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid relay binary hash");
    }
    let component = relay["sha256"]
        .as_str()
        .context("relay component hash missing")?;
    let release_dir = format!("/usr/local/libexec/sow-relay/releases/{component}");
    let destination = format!("{release_dir}/sow-relay");
    let revision = relay["git"].as_str().context("relay revision missing")?;
    let manifest = json!({
        "version":release.version,"release":release.id,"git":revision,
        "fstack":relay["fstack"],"dpdk":relay["dpdk"],"foundation":relay["foundation"],
        "worker_count":relay["worker_count"],"relay_sha256":relay["sha256"],
        "relay_bin_sha256":binary,"ws_write_timeout_ms":relay["ws_write_timeout_ms"],
        "runtime_libraries_sha256":relay["runtime_libraries_sha256"],
        "drain":"match-owned generations","handover_verified":true,
        "deployed_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string(),
    }).to_string();
    let command = format!(
        r#"set -eu
stage={stage}
candidate_ldd=$(LD_LIBRARY_PATH="$stage/lib" ldd "$stage/bin/sow-relay")
case "$candidate_ldd" in *"not found"*) echo "$candidate_ldd" >&2; exit 78;; esac
test "$(sha256sum "$stage/bin/sow-relay" | cut -d ' ' -f 1)" = {binary}
sudo install -d -o root -g {group} -m 0750 {release_dir} {release_dir}/lib
if ! sudo test -f {destination}; then sudo install -o root -g {group} -m 0550 "$stage/bin/sow-relay" {destination}; fi
for lib in "$stage"/lib/*.so*; do sudo install -o root -g {group} -m 0440 "$lib" {release_dir}/lib/"$(basename "$lib")"; done
test "$(sudo sha256sum {destination} | cut -d ' ' -f 1)" = {binary}
"#,
        stage = shell_quote(RELAY_STAGE),
        binary = binary,
        group = RELAY_GROUP,
        destination = shell_quote(&destination),
        release_dir = shell_quote(&release_dir)
    );
    run("ssh", &[&config.relay_host, &command], None).context("prepare relay live handover")?;
    verify_live_reward_return(paths, config, Some(&destination))?;
    let command = format!(
        r#"set -eu
sudo ln -sfn releases/{component}/sow-relay {exec}.next
sudo mv -Tf {exec}.next {exec}
if sudo test -f {manifest_path}; then sudo cp -p {manifest_path} {manifest_path}.bak_$(date +%s); fi
printf '%s\n' {manifest_json} | sudo tee {manifest_path} >/dev/null
sudo chmod 0644 {manifest_path}
"#,
        component = component,
        exec = RELAY_EXEC,
        manifest_path = RELAY_MANIFEST,
        manifest_json = shell_quote(&manifest)
    );
    run("ssh", &[&config.relay_host, &command], None).context("relay live handover failed")?;
    println!("  relay replacement is serving; previous matches retain their workers");
    Ok(())
}

/// A library replacement must first prove that every running generation has
/// completed its matches and delivery. Compatible updates use the live fixture.
fn activate_relay_host(config: &Config, release: &Release) -> Result<()> {
    let active = if relay_workers_stopped(config)? {
        0
    } else {
        relay_active_match_count(config)?
    };
    if active != 0 {
        bail!("refusing network-library replacement while {active} matches remain");
    }
    let env = RelayEnv::load()?;
    let relay_fstack = fstack_version(config)?;
    let release_json: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let revision = release_json["relay"]["git"]
        .as_str()
        .context("relay git missing")?;
    let relay_component = release_json
        .get("relay")
        .and_then(|relay| relay.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .context("release.json relay sha256 missing")?;
    let relay_bin_sha = release_json
        .get("relay")
        .and_then(|relay| relay.get("bin_sha256"))
        .and_then(serde_json::Value::as_str)
        .context("release.json relay bin_sha256 missing")?;
    let relay_release = relay_component;
    let control_secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
    let settlement_secret = env::var("SOW_RELAY_SETTLEMENT_SECRET")?;
    let control_path = format!("/tmp/sow-relay-control-secret-{}", std::process::id());
    let settlement_path = format!("/tmp/sow-relay-settlement-secret-{}", std::process::id());
    stage_secret(&config.relay_host, &control_secret, &control_path)?;
    stage_secret(&config.relay_host, &settlement_secret, &settlement_path)?;

    let ids = (0..1)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let units = (0..1)
        .map(|id| format!("sow-relay@{id}.service"))
        .collect::<Vec<_>>()
        .join(" ");
    let remote = format!(
        r#"set -eu
stage={stage}
candidate_ldd=$(LD_LIBRARY_PATH="$stage/lib" ldd "$stage/bin/sow-relay")
case "$candidate_ldd" in *"not found"*) echo "$candidate_ldd" >&2; exit 78;; esac
sudo env RUNTIME_DIRECTORY=/run/sow-relay LD_LIBRARY_PATH="$stage/lib" "$stage/bin/sow-relay" --fstack-identity >/dev/null
ts=$(date +%s)
for f in {exec} {config} /usr/local/sbin/sow-relay-worker /etc/systemd/system/sow-relay.service /etc/systemd/system/sow-relay@.service /etc/systemd/system/sow-relay@.service.d/override.conf /etc/tmpfiles.d/sow-relay.conf /etc/systemd/journald.conf.d/30-sow-relay.conf; do
  if sudo test -e "$f"; then sudo cp -p "$f" "$f.bak_$ts"; fi
done
if ! getent group {group} >/dev/null; then sudo groupadd --system {group}; fi
if ! id -u {user} >/dev/null 2>&1; then sudo useradd --system --gid {group} --home-dir /nonexistent --shell /usr/sbin/nologin {user}; fi
sudo install -d -o root -g {group} -m 0750 /usr/local/libexec/sow-relay /usr/local/etc/sow
sudo install -d -o {user} -g {group} -m 0750 {state}
restore_workers() {{
  for id in {ids}; do
    sudo systemctl start "sow-relay@$id.service" || true
    port=$((8080+id)); i=0
    until curl -kfsS --max-time 5 "https://127.0.0.1:$port/healthz" >/dev/null; do
      i=$((i+1)); if [ "$i" -ge 60 ]; then echo "old relay worker $id failed recovery" >&2; exit 78; fi
      sleep 1
    done
  done
}}
sudo systemctl stop sow-relay.service
sudo systemctl stop 'sow-relay@*.service'
for id in {ids}; do
  if ! sudo systemctl stop "sow-relay@$id.service" 2>/dev/null; then
    restore_workers
    rm -f {control_path} {settlement_path}
    echo "relay worker $id failed to stop; old workers were restored" >&2
    exit 78
  fi
done
for id in {ids}; do
  if sudo systemctl is-active --quiet "sow-relay@$id.service"; then
    restore_workers
    rm -f {control_path} {settlement_path}
    echo "relay worker $id did not stop; old workers were restored" >&2
    exit 78
  fi
done
sudo install -d -o {user} -g {group} -m 0700 /var/run/dpdk
sudo chown -R {user}:{group} /var/run/dpdk /dev/hugepages 2>/dev/null || true
sudo tee /etc/tmpfiles.d/sow-relay.conf >/dev/null <<'EOF'
d /var/run/dpdk 0700 sowrelay sowrelay -
Z /dev/hugepages 0700 sowrelay sowrelay -
EOF
sudo install -d -m 0755 /etc/systemd/journald.conf.d
sudo tee /etc/systemd/journald.conf.d/30-sow-relay.conf >/dev/null <<'EOF'
[Journal]
SystemMaxUse=256M
RuntimeMaxUse=128M
MaxRetentionSec=7day
RateLimitIntervalSec=30s
RateLimitBurst=10000
EOF
sudo install -d -o root -g {group} -m 0750 /usr/local/libexec/sow-relay/releases/{relay_release} /usr/local/libexec/sow-relay/releases/{relay_release}/lib
sudo install -o root -g {group} -m 0550 "$stage/bin/sow-relay" /usr/local/libexec/sow-relay/releases/{relay_release}/sow-relay
for lib in "$stage"/lib/*.so*; do sudo install -o root -g {group} -m 0440 "$lib" /usr/local/libexec/sow-relay/releases/{relay_release}/lib/"$(basename "$lib")"; done
sudo ln -sfn releases/{relay_release}/sow-relay {exec}.next
sudo mv -Tf {exec}.next {exec}
sudo install -o root -g {group} -m 0640 "$stage/conf/echo-vf.ini" {config}
sudo install -o root -g root -m 0755 "$stage/ops/linux/sow-relay-worker" /usr/local/sbin/sow-relay-worker
sudo install -o root -g root -m 0644 "$stage/ops/linux/sow-relay.service" /etc/systemd/system/sow-relay.service
sudo install -o root -g root -m 0644 "$stage/ops/linux/sow-relay@.service" /etc/systemd/system/sow-relay@.service
sudo mkdir -p /etc/systemd/system/sow-relay@.service.d
ctl=$(cat {control_path}); settlement=$(cat {settlement_path}); rm -f {control_path} {settlement_path}
sed -e "s|__SOW_RELAY_SETTLEMENT_SECRET__|$settlement|" -e "s|__SOW_RELAY_CONTROL_SECRET__|$ctl|" "$stage/ops/linux/sow-relay-override.conf.tmpl" | sudo tee /etc/systemd/system/sow-relay@.service.d/override.conf >/dev/null
sudo chmod 0600 /etc/systemd/system/sow-relay@.service.d/override.conf
sudo test -s /usr/local/etc/sow/relay.crt
sudo test -s /usr/local/etc/sow/relay.key
sudo openssl x509 -in /usr/local/etc/sow/relay.crt -noout -checkend 86400 >/dev/null
sudo systemctl daemon-reload
sudo systemctl enable sow-relay.service >/dev/null
sudo systemctl start sow-relay.service
for id in {ids}; do
  timeout 90s sudo systemctl start "sow-relay@$id.service"
  port=$((8080+id))
  i=0
  until curl -kfsS --max-time 5 "https://127.0.0.1:$port/healthz" >/dev/null; do
    i=$((i+1))
    if [ "$i" -ge 60 ]; then echo "relay worker $id failed healthz after start" >&2; exit 78; fi
    sleep 1
  done
done
for port in {health_ports}; do
  curl -kfsS --max-time 5 "https://127.0.0.1:$port/healthz" >/dev/null
done
test "$(sudo stat -Lc '%a %U:%G' {exec})" = '550 root:{group}'
test "$(sudo stat -c '%a %U:%G' {config})" = '640 root:{group}'
test "$(sudo stat -c '%a' /etc/systemd/system/sow-relay@.service.d/override.conf)" = 600
for u in {units}; do
  test "$(systemctl show "$u" -p User --value)" = {user}
  test "$(systemctl show "$u" -p ActiveState --value)" = active
  test "$(systemctl show "$u" -p Result --value)" = success
  test "$(systemctl show "$u" -p ExecMainStatus --value)" = 0
done
sudo tee {manifest} >/dev/null <<EOF
{{"version":"{version}","release":"{release_id}","git":"{revision}","fstack":"{relay_fstack}","dpdk":"{dpdk}","foundation":"{foundation}","worker_count":{worker_count},"relay_sha256":"{relay_component}","relay_bin_sha256":"{relay_bin_sha}","runtime_libraries_sha256":"{runtime_libraries_sha}","ws_write_timeout_ms":{knob},"drain":"match-owned generations","handover_verified":false,"deployed_at":"$ts"}}
EOF
sudo chown root:root {manifest}
sudo chmod 0644 {manifest}
"#,
        stage = RELAY_STAGE,
        exec = RELAY_EXEC,
        config = RELAY_CONFIG,
        group = RELAY_GROUP,
        user = RELAY_USER,
        state = RELAY_STATE,
        settlement_path = shell_quote(&settlement_path),
        manifest = RELAY_MANIFEST,
        control_path = control_path,
        ids = ids,
        units = units,
        version = release.version,
        release_id = release.id,
        revision = revision,
        relay_fstack = relay_fstack,
        relay_component = relay_component,
        relay_bin_sha = relay_bin_sha,
        relay_release = relay_release,
        runtime_libraries_sha = release_json["relay"]["runtime_libraries_sha256"]
            .as_str()
            .context("relay runtime libraries hash missing")?,
        knob = env.knob,
        dpdk = DPDK_RELEASE,
        foundation = release_json["relay"]["foundation"]
            .as_str()
            .context("relay foundation missing")?,
        worker_count = env.count,
        health_ports = (8080..8080 + env.count)
            .map(|port| port.to_string())
            .collect::<Vec<_>>()
            .join(" "),
    );
    run("ssh", &[&config.relay_host, &remote], None).context("relay activation failed")?;
    println!(
        "  relay activated and registered ({})",
        &relay_component[..12]
    );
    Ok(())
}

/// The deployed relay must match the release exactly: manifest fields and the
/// [BOOT] identity line of the running worker. No manifest = the relay was
/// never deployed through the pipeline.
fn verify_relay_identity(config: &Config, release: &Release) -> Result<()> {
    let release_json: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let relay = release_json
        .get("relay")
        .context("release.json has no relay metadata")?;
    let expected_sha = relay
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .context("release relay sha256 missing")?;
    let expected_knob = relay
        .get("ws_write_timeout_ms")
        .and_then(serde_json::Value::as_u64)
        .context("release relay knob missing")?;
    let expected_git = relay
        .get("git")
        .and_then(serde_json::Value::as_str)
        .context("release git missing")?;
    let expected_fstack = relay
        .get("fstack")
        .and_then(serde_json::Value::as_str)
        .context("release relay fstack missing")?;
    let expected_bin_sha = relay
        .get("bin_sha256")
        .and_then(serde_json::Value::as_str)
        .context("release relay bin_sha256 missing")?;
    let expected_runtime_libraries_sha = relay
        .get("runtime_libraries_sha256")
        .and_then(serde_json::Value::as_str)
        .context("release relay runtime libraries hash missing")?;
    if directory_sha256(&release.dir.join("relay/lib"))? != expected_runtime_libraries_sha {
        bail!("release relay runtime library hash mismatch");
    }
    let manifest = relay_manifest_remote(config)?
        .context("relay host has no registered manifest — relay never deployed through ./sow p")?;
    for (key, expected) in [
        ("relay_sha256", expected_sha),
        ("relay_bin_sha256", expected_bin_sha),
        ("runtime_libraries_sha256", expected_runtime_libraries_sha),
        ("git", expected_git),
        ("fstack", expected_fstack),
        ("dpdk", DPDK_RELEASE),
        (
            "foundation",
            relay["foundation"]
                .as_str()
                .context("relay foundation missing")?,
        ),
    ] {
        let actual = manifest
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if actual != expected {
            bail!("relay manifest {key}={actual}, release expects {expected}");
        }
    }
    if manifest
        .get("ws_write_timeout_ms")
        .and_then(serde_json::Value::as_u64)
        != Some(expected_knob)
    {
        bail!("relay manifest knob mismatch");
    }
    for (queue, body) in relay_authed_get(config, "/internal/metrics")? {
        let metrics: serde_json::Value = serde_json::from_str(&body)?;
        if metrics["managed_reload"] != true
            || metrics["foundation"] != relay["foundation"]
            || metrics["dpdk"].as_str() != Some(DPDK_RELEASE)
            || metrics["git"].as_str() != Some(expected_git)
            || metrics["queue_count"].as_u64() != relay["worker_count"].as_u64()
        {
            bail!("relay queue {queue} does not run the requested F-Stack 2.0 foundation");
        }
    }
    let boot = output(
        "ssh",
        &[
            &config.relay_host,
            "sudo journalctl -u sow-relay@0.service --no-pager -n 8000 2>/dev/null | grep '\\[BOOT\\] git=' | tail -1",
        ],
    )?;
    if !boot.contains(&format!("git={expected_git}"))
        || !boot.contains(&format!("ws_write_timeout_ms={expected_knob}"))
    {
        bail!("relay worker 0 [BOOT] identity mismatch: {boot}");
    }
    // Content check: hash the binary the workers actually exec. Component
    // hashes and [BOOT] env stamps can be consistent while the deployed file
    // is stale (cargo not relinking against a rebuilt libfstack.a), so the
    // deployed file itself must match the release's recorded binary sha.
    let deployed = output(
        "ssh",
        &[&config.relay_host, &format!("sudo sha256sum {RELAY_EXEC}")],
    )?;
    if !deployed.starts_with(expected_bin_sha) {
        bail!("relay deployed binary mismatch: {deployed} expected {expected_bin_sha}");
    }
    for name in ["libfdt.so.1", "libxdp.so.1", "libisal.so.2"] {
        let expected = file_sha256(&release.dir.join("relay/lib").join(name))?;
        let path = format!("/usr/local/libexec/sow-relay/releases/{expected_sha}/lib/{name}");
        let deployed = output(
            "ssh",
            &[&config.relay_host, &format!("sudo sha256sum {path}")],
        )?;
        if !deployed.starts_with(&expected) {
            bail!("relay deployed runtime library mismatch: {name}");
        }
    }
    println!(
        "  relay identity verified (git={expected_git} fstack={expected_fstack} bin={expected_bin_sha} knob={expected_knob})"
    );
    Ok(())
}

fn build_web(paths: &Paths, version: &str) -> Result<PathBuf> {
    compile_wasm(paths, true)?;
    let fingerprint = web_fingerprint(paths, version)?;
    let candidate_root = paths.root.join("dist/.sow-state/prod-web");
    let web = candidate_root.join("web");
    let poki = candidate_root.join("poki");
    let cache = paths.root.join("dist/.sow-state/prod-web-package");
    let cached = fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
        && web.join("play/index.html").is_file()
        && paths.dist_cg.join("index.html").is_file()
        && poki.join("index.html").is_file()
        && paths.dist_jest.join("index.html").is_file()
        && verify_layout(&web).is_ok()
        && verify_cg_layout(&paths.dist_cg).is_ok()
        && verify_poki_layout(&poki).is_ok()
        && verify_jest_layout(&paths.dist_jest).is_ok();
    if cached {
        println!("==> Web package unchanged — reusing production candidate");
        return Ok(web);
    }
    package_self(paths, &web, version, true)?;
    let maps_cache_bust = thumbnail_cache_bust(&web.join("maps"))?;
    package_cg(&web, &paths.dist_cg, paths, version, &maps_cache_bust)?;
    package_poki(&web, &poki, paths, version, &maps_cache_bust)?;
    package_jest(&web, &paths.dist_jest, paths, version, &maps_cache_bust)?;
    write_jest_zip(paths)?;
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
        "web-v10-jest",
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
        ],
    )
}

fn freebsd_build_fingerprint(root: &Path) -> Result<String> {
    input_fingerprint(
        "freebsd-v6",
        "",
        &[
            &root.join("Cargo.toml"),
            &root.join("Cargo.lock"),
            &root.join("sow-core"),
            &root.join("sow-data"),
            &root.join("sow-net"),
            &root.join("sow-server"),
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
    if ["sow-server", "sow-database"]
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
            "--exclude=sow-dist/.env",
            &source,
            &destination,
        ],
        Some(&paths.root),
    )?;

    let root = shell_quote(&config.build_root);
    // Synced inputs can predate the last build; discard crates embedding game data.
    let command = format!(
        "set -eu; cd {root}; cargo clean --profile deploy -p sow-core -p sow-data; cargo test --locked -p sow-core -- --test-threads=1; cargo test --locked -p sow-data --features server; cargo test --locked -p sow-server; cargo build --locked --profile deploy -p sow-server; cargo build --locked --profile deploy -p sow-data --features server --bin sow-database"
    );
    run("ssh", &[&config.build_host, &command], None)?;

    if local.exists() {
        fs::remove_dir_all(&local)?;
    }
    fs::create_dir_all(&local)?;
    for name in ["sow-server", "sow-database"] {
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

/// Registered WS write timeout (ms) for the relay. The pipeline stamps it into
/// the unit drop-in and the relay manifest, and the relay prints it in [BOOT].
fn relay_knob_ms() -> u64 {
    env_or("SOW_WS_WRITE_TIMEOUT_MS", "15000")
        .parse::<u64>()
        .unwrap_or(15000)
}

/// Relay runtime environment rendered into the unit drop-in. Every value here
/// is release content: it participates in the relay component hash, so a knob
/// or admission change re-deploys the relay instead of ghosting.
struct RelayEnv {
    count: usize,
    max_connections: String,
    max_connections_per_ip: String,
    handshakes_per_ip: String,
    db_url: String,
    db_resolve_ip: String,
    knob: u64,
}

impl RelayEnv {
    fn load() -> Result<Self> {
        let count = relay_worker_count()?;
        let max_connections = env_or("SOW_RELAY_MAX_CONNECTIONS", "32768");
        let max_connections_per_ip = env_or("SOW_RELAY_MAX_CONNECTIONS_PER_IP", "4096");
        let handshakes_per_ip = env_or("SOW_RELAY_HANDSHAKES_PER_IP", "512");
        let max_connections = max_connections
            .parse::<usize>()
            .context("SOW_RELAY_MAX_CONNECTIONS must be an integer")?;
        let max_connections_per_ip = max_connections_per_ip
            .parse::<usize>()
            .context("SOW_RELAY_MAX_CONNECTIONS_PER_IP must be an integer")?;
        let handshakes_per_ip = handshakes_per_ip
            .parse::<u32>()
            .context("SOW_RELAY_HANDSHAKES_PER_IP must be an integer")?;
        if max_connections == 0 || max_connections_per_ip == 0 || handshakes_per_ip == 0 {
            bail!("relay admission limits must be positive");
        }
        if max_connections_per_ip > max_connections {
            bail!("SOW_RELAY_MAX_CONNECTIONS_PER_IP must not exceed SOW_RELAY_MAX_CONNECTIONS");
        }
        let db_url = env_or("SOW_DB_URL", "https://shadowsofwar.io");
        if !db_url.starts_with("https://") {
            bail!("SOW_DB_URL must use https for relay production deploys");
        }
        let db_resolve_ip = env_or("SOW_DB_RESOLVE_IP", "74.208.246.177");
        db_resolve_ip
            .parse::<std::net::IpAddr>()
            .with_context(|| format!("invalid SOW_DB_RESOLVE_IP={db_resolve_ip}"))?;
        Ok(Self {
            count,
            max_connections: max_connections.to_string(),
            max_connections_per_ip: max_connections_per_ip.to_string(),
            handshakes_per_ip: handshakes_per_ip.to_string(),
            db_url,
            db_resolve_ip,
            knob: relay_knob_ms(),
        })
    }
}

/// Registered identity of the f-stack tree used for the relay build.
fn fstack_version(config: &Config) -> Result<String> {
    let rev = output(
        "git",
        &["-C", &config.fstack_repo, "rev-parse", "--short=12", "HEAD"],
    )
    .with_context(|| format!("f-stack repo has no HEAD: {}", config.fstack_repo))?;
    let changes = output(
        "git",
        &["-C", &config.fstack_repo, "diff", "--binary", "HEAD"],
    )?;
    let extra = output(
        "git",
        &[
            "-C",
            &config.fstack_repo,
            "ls-files",
            "--others",
            "--exclude-standard",
        ],
    )?;
    let mut hash = Sha256::new();
    hash.update(changes.as_bytes());
    for file in extra.lines() {
        hash.update(file.as_bytes());
        hash.update(fs::read(Path::new(&config.fstack_repo).join(file))?);
    }
    Ok(if changes.is_empty() && extra.is_empty() {
        rev
    } else {
        format!("{rev}-{:x}", hash.finalize())
    })
}

/// Build the DPDK bundled with the pinned F-Stack release. A private prefix
/// keeps the builder's unrelated installed libraries out of the relay.
fn prepare_relay_dpdk(paths: &Paths, config: &Config, source: &str) -> Result<PathBuf> {
    let revision = output("git", &["-C", &config.fstack_repo, "rev-parse", "HEAD"])?;
    if revision != FSTACK_RELEASE_REV {
        bail!("relay requires F-Stack 2.0 ({FSTACK_RELEASE_REV}), found {revision}");
    }
    let version = fs::read_to_string(Path::new(&config.fstack_repo).join("dpdk/VERSION"))?;
    if version.trim() != DPDK_RELEASE {
        bail!(
            "F-Stack 2.0 requires bundled DPDK {DPDK_RELEASE}, found {}",
            version.trim()
        );
    }
    let root = paths.root.join("dist/.sow-state/dpdk").join(source);
    let prefix = root.join("installed");
    let pc = prefix.join("lib/pkgconfig/libdpdk.pc");
    let marker = root.join("complete");
    if marker.is_file() && pc.is_file() && prefix.join("lib/librte_eal.a").is_file() {
        let metadata = fs::read_to_string(&pc)?;
        if metadata
            .lines()
            .any(|line| line == format!("Version: {DPDK_RELEASE}"))
        {
            return Ok(prefix);
        }
    }
    fs::create_dir_all(&root)?;
    let script = format!(
        r#"set -eu
if ! /usr/bin/python3 -c 'import elftools' >/dev/null 2>&1; then
  apt-get update
  apt-get install -y --no-install-recommends python3-pyelftools
fi
configure=
if [ -f {build}/build.ninja ]; then configure=--reconfigure; fi
meson setup $configure {build} {src} --prefix {prefix} --libdir lib -Dplatform=generic -Ddefault_library=static -Denable_kmods=false -Dtests=false -Dexamples=
meson compile -C {build} -j 8
meson install -C {build}
test "$(PKG_CONFIG_PATH={pc} pkg-config --modversion libdpdk)" = {version}
"#,
        build = shell_quote(&root.join("build").display().to_string()),
        src = shell_quote(&format!("{}/dpdk", config.fstack_repo)),
        prefix = shell_quote(&prefix.display().to_string()),
        pc = shell_quote(&prefix.join("lib/pkgconfig").display().to_string()),
        version = shell_quote(DPDK_RELEASE),
    );
    println!("==> Building bundled DPDK {DPDK_RELEASE} for F-Stack 2.0");
    run(
        "sudo",
        &[
            "systemd-nspawn",
            "-q",
            "-D",
            "/var/lib/machines/debian12",
            "--bind",
            &paths.root.display().to_string(),
            "--bind",
            &config.fstack_repo,
            "--as-pid2",
            "bash",
            "-c",
            &script,
        ],
        None,
    )?;
    fs::write(marker, format!("{source}:DPDK={DPDK_RELEASE}\n"))?;
    Ok(prefix)
}

/// Keep the DPDK shared-library dependencies beside the relay generation that
/// was linked against them. Generations can then coexist during a handover.
fn collect_relay_runtime_libraries(paths: &Paths) -> Result<PathBuf> {
    let local = paths.root.join("dist/relay-bin");
    let libraries = local.join("lib");
    if libraries.exists() {
        fs::remove_dir_all(&libraries)?;
    }
    fs::create_dir_all(&libraries)?;
    let repo_mount = paths.root.display().to_string();
    let script = format!(
        r#"set -eu
repo={repo}
source=/usr/lib/x86_64-linux-gnu
destination="$repo/dist/relay-bin/lib"
for name in libfdt.so.1 libxdp.so.1 libisal.so.2; do
  test -r "$source/$name"
  cp -L "$source/$name" "$destination/$name"
done
chmod 0444 "$destination"/*
chown -R 1000:1000 "$destination"
"#,
        repo = shell_quote(&repo_mount),
    );
    run(
        "sudo",
        &[
            "systemd-nspawn",
            "-q",
            "-D",
            "/var/lib/machines/debian12",
            "--bind",
            &repo_mount,
            "--as-pid2",
            "bash",
            "-c",
            &script,
        ],
        None,
    )
    .context("collect DPDK runtime libraries from the Debian 12 build environment")?;
    for name in ["libfdt.so.1", "libxdp.so.1", "libisal.so.2"] {
        require_file(&libraries.join(name), "relay runtime library")?;
    }
    Ok(libraries)
}

/// Build the relay locally in the Debian 12 (bookworm) systemd-nspawn container
/// (f-stack lib + binary) to produce a glibc 2.36 binary compatible with Ubuntu 24.04.
/// Azure receives the binary and its DPDK runtime libraries, not source or a compiler.
fn build_relay(paths: &Paths, config: &Config) -> Result<PathBuf> {
    let patch = paths.root.join("fstack-bridge/patches/fstack-2.0.patch");
    let applied = Command::new("git")
        .args(["-C", &config.fstack_repo, "apply", "--reverse", "--check"])
        .arg(&patch)
        .output()?
        .status
        .success();
    if !applied {
        run(
            "git",
            &[
                "-C",
                &config.fstack_repo,
                "apply",
                "--check",
                &patch.display().to_string(),
            ],
            None,
        )?;
        run(
            "git",
            &[
                "-C",
                &config.fstack_repo,
                "apply",
                &patch.display().to_string(),
            ],
            None,
        )?;
    }
    let fstack_hash = fstack_version(config)?;
    let dpdk_prefix = prepare_relay_dpdk(paths, config, &fstack_hash)?;
    let local = paths.root.join("dist/relay-bin");
    let runtime_libraries = collect_relay_runtime_libraries(paths)?;
    let runtime_libraries_sha = directory_sha256(&runtime_libraries)?;
    let pkg_config_path = dpdk_prefix.join("lib/pkgconfig").display().to_string();
    let dependency_identity = format!("{fstack_hash}:DPDK={DPDK_RELEASE}:{FSTACK_BUILD_FLAGS}");
    let relay_env = RelayEnv::load()?;
    let foundation = input_fingerprint(
        "relay-foundation-v1",
        &format!(
            "{dependency_identity}:{runtime_libraries_sha}:{}:{}:{}:{}:{}:{}:{}",
            relay_env.count,
            relay_env.max_connections,
            relay_env.max_connections_per_ip,
            relay_env.handshakes_per_ip,
            relay_env.db_url,
            relay_env.db_resolve_ip,
            relay_env.knob
        ),
        &[
            &paths.root.join("sow-relay/src/reload.c"),
            &paths.root.join("sow-relay/src/supervisor.rs"),
            &paths.root.join("sow-relay/build.rs"),
            &paths.root.join("fstack-bridge/echo-vf.ini"),
            &paths.root.join("sow-dist/deploy/linux"),
        ],
    )?;
    let revision = output("git", &["rev-parse", "--short=12", "HEAD"])?;
    let repo_mount = paths.root.display().to_string();
    let fstack_mount = config.fstack_repo.clone();
    let fingerprint = input_fingerprint(
        "relay-v2",
        &format!("{dependency_identity}:{runtime_libraries_sha}:{foundation}"),
        &[
            &paths.root.join("Cargo.toml"),
            &paths.root.join("Cargo.lock"),
            &paths.root.join("sow-relay"),
            &paths.root.join("sow-data"),
            &paths.root.join("sow-core"),
            &paths.root.join("sow-net"),
            &paths.root.join("fstack-bridge"),
            &paths.root.join("sow-dist/deploy/linux"),
            &paths.root.join("fstack-bridge/echo-vf.ini"),
        ],
    )?;
    let cache = paths.root.join("dist/.sow-state/relay-build");
    if local.join("sow-relay").is_file()
        && local.join("foundation").is_file()
        && local.join("git").is_file()
        && fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
    {
        println!("==> Relay unchanged — reusing binary");
        return Ok(local);
    }

    run("sudo", &["test", "-d", "/var/lib/machines/debian12"], None)
        .context("Debian 12 nspawn container (/var/lib/machines/debian12) missing")?;
    println!("==> Building relay in local Debian 12 container (nspawn)...");

    let fstack_cache = paths.root.join("dist/.sow-state/fstack-build");
    let fstack_key = dependency_identity;
    if !fs::read_to_string(&fstack_cache).is_ok_and(|value| value.trim() == fstack_key) {
        println!(
            "==> F-Stack changed ({}) — rebuilding libfstack.a in Debian 12 container",
            &fstack_hash[..12]
        );
        let prepare = format!(
            "set -eu; export PKG_CONFIG_PATH={}; cd {}/lib; make clean; make -j8 {FSTACK_BUILD_FLAGS}",
            shell_quote(&pkg_config_path),
            shell_quote(&config.fstack_repo)
        );
        run(
            "sudo",
            &[
                "systemd-nspawn",
                "-D",
                "/var/lib/machines/debian12",
                "--bind",
                &repo_mount,
                "--bind",
                &fstack_mount,
                "--as-pid2",
                "bash",
                "-c",
                &prepare,
            ],
            None,
        )?;
        fs::create_dir_all(
            fstack_cache
                .parent()
                .context("fstack cache parent missing")?,
        )?;
        fs::write(&fstack_cache, format!("{fstack_key}\n"))?;
    }

    let build = format!(
        "set -eu; export PATH=$HOME/.cargo/bin:$PATH; \
         export PKG_CONFIG_PATH={}; export SOW_DPDK_VERSION={DPDK_RELEASE}; \
         export SOW_RELAY_FOUNDATION_ID={foundation}; export SOW_RELAY_BUILD_GIT={revision}; \
         export FSTACK_LIB_DIR={}; \
         cd {}; cargo test --locked --release -p fstack-bridge -p sow-relay -- --test-threads=1; \
         cargo build --locked --release -p sow-relay; \
         chown -R 1000:1000 {}/target {}/lib 2>/dev/null || true",
        shell_quote(&pkg_config_path),
        shell_quote(&format!("{}/lib", config.fstack_repo)),
        shell_quote(&paths.root.display().to_string()),
        shell_quote(&paths.root.display().to_string()),
        shell_quote(&config.fstack_repo)
    );
    run(
        "sudo",
        &[
            "systemd-nspawn",
            "-D",
            "/var/lib/machines/debian12",
            "--bind",
            &repo_mount,
            "--bind",
            &fstack_mount,
            "--as-pid2",
            "bash",
            "-c",
            &build,
        ],
        None,
    )?;

    if local.exists() {
        fs::remove_dir_all(&local)?;
    }
    fs::create_dir_all(&local)?;
    let compiled = paths.root.join("target/release/sow-relay");
    let destination = local.join("sow-relay");
    fs::copy(&compiled, &destination)?;
    fs::write(local.join("foundation"), format!("{foundation}\n"))?;
    fs::write(local.join("git"), format!("{revision}\n"))?;
    require_file(&destination, "relay binary")?;
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o550))?;
    let packaged_libraries = collect_relay_runtime_libraries(paths)?;
    if directory_sha256(&packaged_libraries)? != runtime_libraries_sha {
        bail!("DPDK runtime libraries changed while building the relay");
    }
    fs::create_dir_all(cache.parent().context("relay cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    println!(
        "  relay binary built ({} bytes)",
        fs::metadata(&destination)?.len()
    );
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
                && !path.contains("/dpdk/build/")
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

fn assemble_release(
    paths: &Paths,
    web: &Path,
    binaries: &Path,
    version: &str,
    config: &Config,
) -> Result<Release> {
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
    if web_maps.is_dir() {
        fs::rename(&web_maps, work.join("maps"))?;
    } else {
        copy_dir(&paths.assets_maps, &work.join("maps"))?;
    }
    for name in ["sow-server", "sow-database"] {
        let destination = work.join("bin").join(name);
        let source = binaries.join(name);
        fs::copy(&source, &destination)
            .with_context(|| format!("copy {} to {}", source.display(), destination.display()))?;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o550))?;
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
    // Relay component: binary, runtime libraries, config, systemd units, wrapper, and the rendered
    // drop-in template (secrets stay as placeholders, injected on the host).
    // Everything here is release content — the relay component hash changes
    // when the knob, admission limits, units, binary, or runtime libraries change, so the plan
    // diff detects drift instead of ghosting.
    let relay = work.join("relay");
    fs::create_dir_all(relay.join("bin"))?;
    fs::create_dir_all(relay.join("conf"))?;
    copy_dir(&paths.root.join("dist/relay-bin/lib"), &relay.join("lib"))?;
    let runtime_libraries_sha = directory_sha256(&relay.join("lib"))?;
    let relay_source = paths.root.join("dist/relay-bin/sow-relay");
    let relay_destination = relay.join("bin/sow-relay");
    fs::copy(&relay_source, &relay_destination).with_context(|| {
        format!(
            "copy {} to {}",
            relay_source.display(),
            relay_destination.display()
        )
    })?;
    fs::set_permissions(
        relay.join("bin/sow-relay"),
        fs::Permissions::from_mode(0o550),
    )?;
    let relay_config_source = paths.root.join("fstack-bridge/echo-vf.ini");
    let relay_config_destination = relay.join("conf/echo-vf.ini");
    fs::copy(&relay_config_source, &relay_config_destination).with_context(|| {
        format!(
            "copy {} to {}",
            relay_config_source.display(),
            relay_config_destination.display()
        )
    })?;
    let ops = relay.join("ops/linux");
    fs::create_dir_all(&ops)?;
    let env = RelayEnv::load()?;
    let wants = "sow-relay@0.service";
    let stops = "sow-relay@0.service";
    let secondary_ids = if env.count > 1 {
        (1..env.count)
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join("|")
    } else {
        String::new()
    };
    for (name, tokens) in [
        (
            "sow-relay.service",
            &[("__WANTS__", wants), ("__STOPS__", stops)][..],
        ),
        (
            "sow-relay-worker",
            &[("__SECONDARY_IDS__", secondary_ids.as_str())][..],
        ),
        ("sow-relay@.service", &[][..]),
    ] {
        let src = paths
            .root
            .join("sow-dist/deploy/linux")
            .join(format!("{name}.tmpl"));
        let mut content = fs::read_to_string(&src)
            .with_context(|| format!("read relay template {}", src.display()))?;
        for (token, value) in tokens {
            content = content.replace(token, value);
        }
        fs::write(ops.join(name), content)?;
    }
    let override_source = paths
        .root
        .join("sow-dist/deploy/linux/sow-relay-override.conf.tmpl");
    let mut override_tpl = fs::read_to_string(&override_source)
        .with_context(|| format!("read relay override template {}", override_source.display()))?;
    let relay_fstack = fstack_version(config)?;
    for (token, value) in [
        ("__SOW_RELAY_WORKER_COUNT__", env.count.to_string()),
        ("__SOW_RELAY_MAX_CONNECTIONS__", env.max_connections.clone()),
        (
            "__SOW_RELAY_MAX_CONNECTIONS_PER_IP__",
            env.max_connections_per_ip.clone(),
        ),
        (
            "__SOW_RELAY_HANDSHAKES_PER_IP__",
            env.handshakes_per_ip.clone(),
        ),
        ("__SOW_DB_URL__", env.db_url.clone()),
        ("__SOW_DB_RESOLVE_IP__", env.db_resolve_ip.clone()),
        ("__SOW_WS_WRITE_TIMEOUT_MS__", env.knob.to_string()),
        (
            "__SOW_RELAY_GIT__",
            fs::read_to_string(paths.root.join("dist/relay-bin/git"))?
                .trim()
                .to_string(),
        ),
        ("__SOW_FSTACK_VERSION__", relay_fstack.clone()),
    ] {
        override_tpl = override_tpl.replace(token, &value);
    }
    // The two secret placeholders are injected on the host (sed) — they are
    // the only tokens allowed to survive this check.
    let secrets_only = override_tpl
        .replace("__SOW_RELAY_SETTLEMENT_SECRET__", "")
        .replace("__SOW_RELAY_CONTROL_SECRET__", "");
    if secrets_only.contains("__SOW_") {
        bail!("relay override template has an unrendered token");
    }
    fs::write(ops.join("sow-relay-override.conf.tmpl"), override_tpl)?;
    require_file(&relay.join("bin/sow-relay"), "relay binary")?;
    require_file(&relay.join("conf/echo-vf.ini"), "relay config")?;

    let nginx_source = paths
        .root
        .join("sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf");
    let nginx_site = fs::read_to_string(&nginx_source)
        .with_context(|| format!("read nginx template {}", nginx_source.display()))?;
    let relay_ip = env_or("SOW_RELAY_DB_SOURCE_IP", "20.230.49.9");
    let nginx_site = nginx_site
        .replace("__SOW_RELAY_DB_SOURCE_IP__", &relay_ip)
        .replace("__SOW_DB_LISTEN_HOST__", DATABASE_JAIL_IP)
        .replace("__SOW_SERVER_LISTEN_HOST__", SERVER_JAIL_IP)
        .replace("__SOW_MAPS_LISTEN_HOST__", SERVER_JAIL_IP);
    if nginx_site.contains("__SOW_") {
        bail!("nginx placeholder was not rendered");
    }
    fs::write(work.join("ops/conf.d/shadowsofwar.io.conf"), nginx_site)?;

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
        ("relay", relay_component_hash(&work.join("relay"))?),
    ];
    let component_text = components
        .iter()
        .map(|(name, hash)| format!("{name}={hash}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(work.join("COMPONENTS"), format!("{component_text}\n"))?;
    fs::write(work.join("VERSION"), format!("{version}\n"))?;
    let relay_component = components
        .iter()
        .find(|(name, _)| *name == "relay")
        .map(|(_, hash)| hash.clone())
        .context("relay component missing")?;
    let relay_bin_sha = file_sha256(&work.join("relay/bin/sow-relay"))?;
    fs::write(
        work.join("release.json"),
        serde_json::to_vec_pretty(&json!({
            "version": version,
            "git": revision,
            "source_sha": source.sha,
            "source_url": source.url,
            "dirty": !dirty_files.is_empty(),
            "dirty_files": dirty_files,
            "components": components.iter().map(|(name, hash)| json!({"name": name, "sha256": hash})).collect::<Vec<_>>(),
            "relay": json!({
                "sha256": relay_component,
                "bin_sha256": relay_bin_sha,
                "runtime_libraries_sha256": runtime_libraries_sha,
                "fstack": relay_fstack,
                "fstack_release": "2.0",
                "git": fs::read_to_string(paths.root.join("dist/relay-bin/git"))?.trim(),
                "dpdk": DPDK_RELEASE,
                "foundation": fs::read_to_string(paths.root.join("dist/relay-bin/foundation"))?.trim(),
                "worker_count": env.count,
                "ws_write_timeout_ms": env.knob,
                "drain": "live handover with matching foundation; empty matches for library replacement",
            }),
        }))?,
    )?;
    let manifest = write_manifest(&work)?;
    let id = format!("{version}-{}", &file_sha256(&manifest)?[..12]);
    let releases = paths.root.join("dist/releases");
    fs::create_dir_all(&releases)?;
    let dir = releases.join(&id);
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    if fs::rename(&work, &dir).is_err() {
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        copy_dir(&work, &dir)?;
        let _ = fs::remove_dir_all(&work);
    }
    Ok(Release {
        id,
        version: version.to_string(),
        dir,
    })
}

/// Relay component hash with the git-revision stamp normalized out: the
/// revision changes on every commit and used to force relay redeploys
/// (worker drain included) with byte-identical binaries.
fn relay_component_hash(relay: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut files: Vec<_> = walkdir::WalkDir::new(relay)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    files.retain(|entry| entry.file_type().is_file());
    files.sort_by_key(|entry| entry.path().to_path_buf());
    let mut hash = Sha256::new();
    for entry in files {
        hash.update(
            entry
                .path()
                .strip_prefix(relay)?
                .to_string_lossy()
                .as_bytes(),
        );
        let mut bytes = fs::read(entry.path())?;
        if entry
            .path()
            .file_name()
            .is_some_and(|n| n.to_string_lossy().contains("override"))
        {
            let text = String::from_utf8_lossy(&bytes).to_string();
            let normalized: String = text
                .lines()
                .map(|line| match line.find("SOW_RELAY_GIT=") {
                    Some(idx) => format!("{}SOW_RELAY_GIT=NORM\n", &line[..idx]),
                    None => format!("{line}\n"),
                })
                .collect();
            bytes = normalized.into_bytes();
        }
        hash.update(&bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
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

/// The relay host's registered deploy manifest (written by activate_relay_host).
fn relay_manifest_remote(config: &Config) -> Result<Option<serde_json::Value>> {
    let raw = output(
        "ssh",
        &[
            &config.relay_host,
            &format!("sudo cat {RELAY_MANIFEST} 2>/dev/null || true"),
        ],
    )?;
    if raw.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&raw)
        .map(Some)
        .context("relay host manifest is not valid JSON")
}

fn relay_boot_git_remote(config: &Config) -> Result<Option<String>> {
    let boot = output(
        "ssh",
        &[
            &config.relay_host,
            "sudo journalctl -u sow-relay@0.service --no-pager -n 8000 2>/dev/null | grep '\\[BOOT\\] git=' | tail -1",
        ],
    )?;
    Ok(boot
        .split_whitespace()
        .find_map(|part| part.strip_prefix("git=").map(ToString::to_string)))
}

fn remote_plan(config: &Config, release: &Release) -> Result<ComponentPlan> {
    let remote = output(
        "ssh",
        &[
            &config.control_host,
            "if test -f /srv/sow/current/COMPONENTS; then cat /srv/sow/current/COMPONENTS; fi",
        ],
    )?;
    let current = parse_components(&remote);
    let local = parse_components(&fs::read_to_string(release.dir.join("COMPONENTS"))?);
    let relay_remote = relay_manifest_remote(config)?;
    let relay_boot_git = relay_boot_git_remote(config)?;
    let release_json: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let relay = release_json
        .get("relay")
        .context("release.json has no relay metadata")?;
    let expected_relay_sha = relay
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .context("release relay sha256 missing")?;
    let expected_bin_sha = relay
        .get("bin_sha256")
        .and_then(serde_json::Value::as_str)
        .context("release relay bin_sha256 missing")?;
    let expected_git = relay
        .get("git")
        .and_then(serde_json::Value::as_str)
        .context("release git missing")?;
    let expected_fstack = relay
        .get("fstack")
        .and_then(serde_json::Value::as_str)
        .context("release relay fstack missing")?;
    let expected_knob = relay
        .get("ws_write_timeout_ms")
        .and_then(serde_json::Value::as_u64)
        .context("release relay knob missing")?;
    let relay_identity_drift = match &relay_remote {
        None => true,
        Some(manifest) => {
            [
                ("relay_sha256", expected_relay_sha),
                ("relay_bin_sha256", expected_bin_sha),
                (
                    "runtime_libraries_sha256",
                    relay
                        .get("runtime_libraries_sha256")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(""),
                ),
                ("git", expected_git),
                ("fstack", expected_fstack),
            ]
            .iter()
            .any(|(key, expected)| {
                manifest.get(*key).and_then(serde_json::Value::as_str) != Some(*expected)
            }) || manifest
                .get("ws_write_timeout_ms")
                .and_then(serde_json::Value::as_u64)
                != Some(expected_knob)
                || relay_boot_git.as_deref() != Some(expected_git)
        }
    };
    Ok(ComponentPlan {
        web: current.get("web") != local.get("web"),
        maps: current.get("maps") != local.get("maps"),
        server: current.get("server") != local.get("server"),
        database: current.get("database") != local.get("database"),
        ops: current.get("ops") != local.get("ops"),
        relay: relay_identity_drift
            || relay_remote.and_then(|manifest| {
                manifest
                    .get("relay_sha256")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            }) != local.get("relay").cloned(),
    })
}

fn control_certificate_automation_drift(config: &Config) -> Result<bool> {
    let command = r#"if [ "$(sudo sysrc -n -f /etc/periodic.conf.local weekly_certbot_enable 2>/dev/null || true)" = YES ] \
        && [ "$(sudo sysrc -n -f /etc/periodic.conf.local weekly_certbot_deploy_hook 2>/dev/null || true)" = 'service nginx reload' ] \
        && sudo grep -Fq 'location ^~ /.well-known/acme-challenge/' /usr/local/etc/nginx/conf.d/shadowsofwar.io.conf \
        && sudo grep -Fq 'root /usr/local/www/letsencrypt;' /usr/local/etc/nginx/conf.d/shadowsofwar.io.conf \
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
    /usr/local/etc/letsencrypt/live/shadowsofwar.io/cert.pem; do
    sudo openssl x509 -in "$cert" -noout -checkend 2592000 >/dev/null || {
        echo "certificate is missing or expires within 30 days: $cert" >&2
        exit 1
    }
done
sudo openssl x509 -in /usr/local/etc/letsencrypt/live/shadowsofwar.io/cert.pem \
    -noout -checkhost shadowsofwar.io >/dev/null
sudo openssl x509 -in /usr/local/etc/letsencrypt/live/shadowsofwar.io/cert.pem \
    -noout -checkhost ws.shadowsofwar.io >/dev/null"#;
    run("ssh", &[&config.control_host, command], None)
        .context("IONOS certificate validity check failed")?;
    println!("  IONOS certificates valid; renewal is handled by the weekly certbot schedule");
    Ok(())
}

fn verify_origin_tls_from_relay(config: &Config) -> Result<()> {
    let relay_env = RelayEnv::load()?;
    let url = url::Url::parse(&relay_env.db_url).context("invalid SOW_DB_URL")?;
    let host = url.host_str().context("SOW_DB_URL has no hostname")?;
    let port = url
        .port_or_known_default()
        .context("SOW_DB_URL has no port")?;
    let root_url = url.join("/")?.to_string();
    let resolve = format!("{host}:{port}:{}", relay_env.db_resolve_ip);
    let command = format!(
        "curl --noproxy '*' -fsS --max-time 10 --resolve {} -o /dev/null {}",
        shell_quote(&resolve),
        shell_quote(&root_url)
    );
    run("ssh", &[&config.relay_host, &command], None)
        .context("relay-to-origin HTTPS certificate verification failed")?;
    let expected = output(
        "ssh",
        &[
            &config.control_host,
            "sudo openssl x509 -in /usr/local/etc/letsencrypt/live/shadowsofwar.io/cert.pem -noout -fingerprint -sha256",
        ],
    )?;
    let command = format!(
        r#"set -eu
set -o pipefail
certificate=$(openssl s_client -connect {address} -servername {host} -verify_hostname {host} -verify_return_error </dev/null 2>/dev/null | openssl x509 -outform PEM)
printf '%s\n' "$certificate" | openssl x509 -noout -checkend 2592000 >/dev/null
printf '%s\n' "$certificate" | openssl x509 -noout -fingerprint -sha256"#,
        address = shell_quote(&format!("{}:{port}", relay_env.db_resolve_ip)),
        host = shell_quote(host),
    );
    let served = output("ssh", &[&config.relay_host, &command])?;
    if served != expected {
        bail!("IONOS is not serving its installed certificate to the relay");
    }
    println!("  relay-to-origin HTTPS certificate verified with the production IP pin");
    Ok(())
}

fn relay_dns_credentials() -> Result<String> {
    let cert_path = env::var("SOW_RELAY_TLS_CERT_LOCAL").unwrap_or_else(|_| {
        "/etc/letsencrypt/live/relay.shadowsofwar.io/fullchain.pem".to_string()
    });
    let lineage = Path::new(&cert_path)
        .parent()
        .context("SOW_RELAY_TLS_CERT_LOCAL must point to fullchain.pem")?;
    if lineage.file_name().and_then(|name| name.to_str()) != Some("relay.shadowsofwar.io") {
        bail!("SOW_RELAY_TLS_CERT_LOCAL must point to the relay.shadowsofwar.io lineage");
    }
    let config_root = lineage
        .parent()
        .and_then(Path::parent)
        .context("could not locate the local Certbot config directory")?;
    let renewal = fs::read_to_string(
        config_root
            .join("renewal")
            .join("relay.shadowsofwar.io.conf"),
    )
    .context("local Certbot relay renewal config is unavailable")?;
    let credentials_path = renewal
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(key, value)| {
            (key.trim() == "dns_cloudflare_credentials")
                .then(|| PathBuf::from(value.trim().trim_matches('"')))
        })
        .context("local Certbot relay config has no Cloudflare credentials path")?;
    let metadata = fs::metadata(&credentials_path).with_context(|| {
        format!(
            "read local DNS credentials metadata at {}",
            credentials_path.display()
        )
    })?;
    if metadata.permissions().mode() & 0o077 != 0 {
        bail!("local DNS credentials file must not be readable by group or other users");
    }
    let credentials = fs::read_to_string(&credentials_path).with_context(|| {
        format!(
            "read local DNS credentials at {}",
            credentials_path.display()
        )
    })?;
    let mut token_present = false;
    let mut global_key_present = false;
    for (key, value) in credentials.lines().filter_map(|line| line.split_once('=')) {
        match key.trim() {
            "dns_cloudflare_api_token" => token_present |= !value.trim().is_empty(),
            "dns_cloudflare_api_key" => global_key_present |= !value.trim().is_empty(),
            _ => {}
        }
    }
    if !token_present || global_key_present {
        bail!("local DNS credentials must contain the existing scoped Cloudflare API token");
    }
    Ok(credentials)
}

fn ensure_relay_certificate_automation(paths: &Paths, config: &Config) -> Result<()> {
    let credentials = relay_dns_credentials()?;
    let expected_credentials_sha = format!("{:x}", Sha256::digest(credentials.as_bytes()));
    let current_credentials_sha = output(
        "ssh",
        &[
            &config.relay_host,
            "if sudo test -s /etc/letsencrypt/cloudflare.ini; then sudo sha256sum /etc/letsencrypt/cloudflare.ini | awk '{print $1}'; else echo missing; fi",
        ],
    )?;
    let credential_stage = format!("/tmp/sow-relay-cloudflare-{}", std::process::id());
    let stage_credentials = current_credentials_sha.trim() != expected_credentials_sha;
    if stage_credentials {
        stage_secret(&config.relay_host, &credentials, &credential_stage)?;
    }

    let hook_source = paths
        .root
        .join("sow-dist/deploy/linux/sow-relay-cert-deploy");
    require_file(&hook_source, "relay certificate deploy hook")?;
    let hook_sha = file_sha256(&hook_source)?;
    let current_hook_sha = output(
        "ssh",
        &[
            &config.relay_host,
            "if sudo test -s /etc/letsencrypt/renewal-hooks/deploy/50-sow-relay; then sudo sha256sum /etc/letsencrypt/renewal-hooks/deploy/50-sow-relay | awk '{print $1}'; else echo missing; fi",
        ],
    )?;
    let hook_stage = format!("/tmp/sow-relay-cert-hook-{}", std::process::id());
    let stage_hook = current_hook_sha.trim() != hook_sha;
    if stage_hook {
        let contents = fs::read_to_string(&hook_source)?;
        stage_secret(&config.relay_host, &contents, &hook_stage)?;
    }

    let credential_update = if stage_credentials {
        format!(
            r#"
if sudo test -e /etc/letsencrypt/cloudflare.ini; then sudo cp -p /etc/letsencrypt/cloudflare.ini /etc/letsencrypt/cloudflare.ini.bak_$(date +%s); fi
sudo install -o root -g root -m 0600 {credential_stage} /etc/letsencrypt/cloudflare.ini
"#,
            credential_stage = shell_quote(&credential_stage),
        )
    } else {
        ":".to_string()
    };
    let hook_update = if stage_hook {
        format!(
            r#"
if sudo test -e /etc/letsencrypt/renewal-hooks/deploy/50-sow-relay; then sudo cp -p /etc/letsencrypt/renewal-hooks/deploy/50-sow-relay /etc/letsencrypt/renewal-hooks/deploy/50-sow-relay.bak_$(date +%s); fi
sudo install -o root -g root -m 0755 {hook_stage} /etc/letsencrypt/renewal-hooks/deploy/50-sow-relay
"#,
            hook_stage = shell_quote(&hook_stage),
        )
    } else {
        ":".to_string()
    };
    let credential_stage_arg = if stage_credentials {
        shell_quote(&credential_stage)
    } else {
        String::new()
    };
    let hook_stage_arg = if stage_hook {
        shell_quote(&hook_stage)
    } else {
        String::new()
    };
    let remote = format!(
        r#"set -eu
credential_stage={credential_stage_arg}
hook_stage={hook_stage_arg}
cert_test_stage=
certbot_cli_stage=
cleanup() {{
    [ -z "$credential_stage" ] || rm -f "$credential_stage"
    [ -z "$hook_stage" ] || rm -f "$hook_stage"
    [ -z "$certbot_cli_stage" ] || rm -f "$certbot_cli_stage"
    [ -z "$cert_test_stage" ] || sudo rm -rf -- "$cert_test_stage"
}}
trap cleanup EXIT
if ! dpkg-query -W -f='${{Status}}' certbot 2>/dev/null | grep -qx 'install ok installed' \
    || ! dpkg-query -W -f='${{Status}}' python3-certbot-dns-cloudflare 2>/dev/null | grep -qx 'install ok installed'; then
    sudo apt-get update
    sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y certbot python3-certbot-dns-cloudflare
fi
sudo install -d -o root -g root -m 0755 /etc/letsencrypt/renewal-hooks/deploy
sudo install -d -o root -g sowrelay -m 0750 /usr/local/etc/sow
{credential_update}
{hook_update}
dns_cloudflare_propagation_seconds=60
# Certbot 2.9 deactivates reused dry-run authorizations. Keep each simulation's
# account state separate so a previous check cannot leave this one unusable.
cert_test_stage=$(sudo mktemp -d /tmp/sow-relay-cert-test.XXXXXX)
sudo certbot certonly --non-interactive --agree-tos --register-unsafely-without-email \
    --config /dev/null --config-dir "$cert_test_stage/config" --work-dir "$cert_test_stage/work" \
    --dns-cloudflare --dns-cloudflare-credentials /etc/letsencrypt/cloudflare.ini \
    --dns-cloudflare-propagation-seconds "$dns_cloudflare_propagation_seconds" \
    --cert-name relay.shadowsofwar.io -d relay.shadowsofwar.io --dry-run
certbot_cli_ini=/etc/letsencrypt/cli.ini
if ! sudo grep -Fxq "dns-cloudflare-propagation-seconds = $dns_cloudflare_propagation_seconds" "$certbot_cli_ini"; then
    if sudo test -e "$certbot_cli_ini"; then
        sudo cp -p "$certbot_cli_ini" "$certbot_cli_ini.bak_$(date +%s)"
    fi
    certbot_cli_stage=$(mktemp /tmp/sow-relay-certbot-cli.XXXXXX)
    if sudo test -f "$certbot_cli_ini"; then
        sudo grep -Ev '^[[:space:]]*dns-cloudflare-propagation-seconds[[:space:]]*=' "$certbot_cli_ini" > "$certbot_cli_stage" || true
    fi
    printf '%s\n' "dns-cloudflare-propagation-seconds = $dns_cloudflare_propagation_seconds" >> "$certbot_cli_stage"
    sudo install -o root -g root -m 0644 "$certbot_cli_stage" "$certbot_cli_ini"
    rm -f "$certbot_cli_stage"
    certbot_cli_stage=
fi
sudo systemctl enable --now certbot.timer
renewal=/etc/letsencrypt/renewal/relay.shadowsofwar.io.conf
if ! sudo test -s "$renewal" \
    || ! sudo grep -Fxq 'authenticator = dns-cloudflare' "$renewal" \
    || ! sudo grep -Fxq 'dns_cloudflare_credentials = /etc/letsencrypt/cloudflare.ini' "$renewal"; then
    sudo certbot certonly --non-interactive --agree-tos --register-unsafely-without-email \
        --dns-cloudflare --dns-cloudflare-credentials /etc/letsencrypt/cloudflare.ini \
        --dns-cloudflare-propagation-seconds "$dns_cloudflare_propagation_seconds" \
        --cert-name relay.shadowsofwar.io -d relay.shadowsofwar.io --force-renewal
else
    sudo certbot renew --no-random-sleep-on-renew \
        --dns-cloudflare-propagation-seconds "$dns_cloudflare_propagation_seconds" \
        --cert-name relay.shadowsofwar.io
fi
sudo /etc/letsencrypt/renewal-hooks/deploy/50-sow-relay
sudo openssl x509 -in /usr/local/etc/sow/relay.crt -noout -checkend 2592000 >/dev/null
sudo openssl x509 -in /usr/local/etc/sow/relay.crt -noout -checkhost relay.shadowsofwar.io >/dev/null
sudo test "$(sudo stat -c '%a %U:%G' /etc/letsencrypt/cloudflare.ini)" = '600 root:root'
systemctl is-enabled --quiet certbot.timer
systemctl is-active --quiet certbot.timer
sudo grep -Fxq 'authenticator = dns-cloudflare' "$renewal"
sudo grep -Fxq 'dns_cloudflare_credentials = /etc/letsencrypt/cloudflare.ini' "$renewal"
sudo grep -Fxq "dns-cloudflare-propagation-seconds = $dns_cloudflare_propagation_seconds" "$certbot_cli_ini"
"#,
        credential_stage_arg = credential_stage_arg,
        hook_stage_arg = hook_stage_arg,
        credential_update = credential_update,
        hook_update = hook_update,
    );
    run("ssh", &[&config.relay_host, &remote], None)
        .context("relay DNS certificate automation or renewal failed")?;
    println!("  relay Certbot DNS issuance dry-run passed and automatic renewal verified");
    Ok(())
}

fn verify_relay_certificate_served(config: &Config) -> Result<()> {
    let workers = relay_management_workers()?;
    let ports = workers
        .iter()
        .map(|(_, port)| port.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let command = format!(
        r#"set -eu
set -o pipefail
for port in {ports}; do
    curl -fsS --max-time 10 --resolve "relay.shadowsofwar.io:$port:127.0.0.1" "https://relay.shadowsofwar.io:$port/healthz" >/dev/null
    served=$(openssl s_client -connect "127.0.0.1:$port" -servername relay.shadowsofwar.io -verify_hostname relay.shadowsofwar.io -verify_return_error </dev/null 2>/dev/null | openssl x509 -noout -fingerprint -sha256)
    configured=$(sudo openssl x509 -in /usr/local/etc/sow/relay.crt -noout -fingerprint -sha256)
    test "$served" = "$configured"
done
"#
    );
    run("ssh", &[&config.relay_host, &command], None)
        .context("live relay TLS certificate verification failed")?;
    println!("  relay public TLS certificate verified on every worker");
    Ok(())
}

/// Web checkout was retired (gem packs are sold only through Google Play);
/// the old Stripe keys must not stay in the runtime env files.
const STRIPE_ENV_KEYS: &str = "^(SOW_STRIPE_[A-Z_]+|SOW_REVENUECAT_STRIPE_API_KEY)=";

fn stripe_env_leftover(config: &Config) -> Result<bool> {
    let command = format!(
        r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do
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
            r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do if sudo test -f "$f"; then sudo awk -F= '$1=="SOW_MAPS_CATALOG_PATH"{print $2}' "$f"; fi; done"#,
        ],
    )?;
    let values = remote.lines().map(str::trim).collect::<Vec<_>>();
    Ok(values.len() != 3 || values.iter().any(|value| *value != expected))
}

fn wou_id_config_drift(config: &Config) -> Result<bool> {
    let expected_url = env_or("WOU_ID_URL", "https://id.worldofunreal.com");
    let expected_resolve_ip = env_or("WOU_ID_RESOLVE_IP", "104.21.87.220");
    let remote = output(
        "ssh",
        &[
            &config.control_host,
            r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do if sudo test -f "$f"; then sudo awk -F= '$1=="WOU_ID_URL" || $1=="WOU_ID_RESOLVE_IP" {print $1 "=" $2}' "$f"; fi; done"#,
        ],
    )?;
    let actual = remote.lines().map(str::trim).collect::<Vec<_>>();
    let expected = (0..3)
        .flat_map(|_| {
            [
                format!("WOU_ID_URL={expected_url}"),
                format!("WOU_ID_RESOLVE_IP={expected_resolve_ip}"),
            ]
        })
        .collect::<Vec<_>>();
    Ok(actual != expected)
}

fn wou_identity_config_drift(config: &Config) -> Result<bool> {
    let remote = output(
        "ssh",
        &[
            &config.control_host,
            r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do if sudo test -f "$f"; then sudo awk -F= '$1=="WOU_SOW_IDENTITY_SECRET" && length($2)>0 {print "set"}' "$f"; fi; done"#,
        ],
    )?;
    Ok(remote.lines().map(str::trim).collect::<Vec<_>>() != vec!["set"; 3])
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
            r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do if sudo test -f "$f"; then app=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_APP_ID"{print $2}' "$f"); client=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_WEB_CLIENT_ID"{print $2}' "$f"); event=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_MATCH_EVENT_ID"{print $2}' "$f"); first=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID"{print $2}' "$f"); battle=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID"{print $2}' "$f"); victory=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID"{print $2}' "$f"); laurel=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID"{print $2}' "$f"); command=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID"{print $2}' "$f"); commander=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID"{print $2}' "$f"); veteran=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID"{print $2}' "$f"); banner=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID"{print $2}' "$f"); leader=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID"{print $2}' "$f"); leaderboard=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID"{print $2}' "$f"); secret=$(sudo awk -F= '$1=="SOW_PLAY_GAMES_WEB_CLIENT_SECRET" && length($2)>0{print "set"}' "$f"); printf '%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s\n' "$app" "$client" "$event" "$first" "$battle" "$victory" "$laurel" "$command" "$commander" "$veteran" "$banner" "$leader" "$leaderboard" "$secret"; fi; done"#,
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
    let target = format!("{REMOTE_RELEASES}/{}", release.id);
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
    let wou_identity_secret_file = if runtime_env {
        let secret = env::var("WOU_SOW_IDENTITY_SECRET")?;
        let path = format!("/tmp/sow-wou-identity-secret-{}", std::process::id());
        stage_secret(&config.control_host, &secret, &path)?;
        Some(path)
    } else {
        None
    };
    let relay_host = env_or("SOW_RELAY_HOST", "relay.shadowsofwar.io");
    let relay_workers = env::var("SOW_RELAY_WORKERS").unwrap_or_default();
    let relay_worker_count = env_or("SOW_RELAY_WORKER_COUNT", "3");
    let relay_mgmt_url = env_or("SOW_RELAY_MGMT_URL", "https://127.0.0.1:8080");
    let relay_mgmt_scheme = env_or("SOW_RELAY_MGMT_SCHEME", "https");
    let relay_mgmt_resolve_ip = env_or("SOW_RELAY_MGMT_RESOLVE_IP", "20.230.49.9");
    let maps_root = env_or("SOW_MAPS_ROOT", "/srv/sow/current/maps");
    let maps_catalog_path = env_or("SOW_MAPS_CATALOG_PATH", "/var/db/sow/server/catalog.bin");
    let wou_id_url = env_or("WOU_ID_URL", "https://id.worldofunreal.com");
    let wou_id_resolve_ip = env_or("WOU_ID_RESOLVE_IP", "104.21.87.220");
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
    let env_update_base = if runtime_env {
        format!(
            "mkdir -p /tmp/sow-env-update; for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do t=$(mktemp /tmp/sow.env.XXXXXX); if sudo test -f \"$f\"; then sudo grep -v -E '^(SOW_MAPS_ROOT|SOW_MAPS_CATALOG_PATH|SOW_DB_SECRET|SOW_RELAY_CONTROL_SECRET|SOW_RELAY_HOST|SOW_RELAY_WORKERS|SOW_RELAY_WORKER_COUNT|SOW_RELAY_MGMT_URL|SOW_RELAY_MGMT_SCHEME|SOW_RELAY_MGMT_RESOLVE_IP|WOU_ID_URL|WOU_ID_RESOLVE_IP|SOW_PLAY_GAMES_APP_ID|SOW_PLAY_GAMES_WEB_CLIENT_ID|SOW_PLAY_GAMES_MATCH_EVENT_ID|SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID|SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID|SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID|SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID|SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID|SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID|SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID|SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID|SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID|SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID|SOW_PLAY_GAMES_WEB_CLIENT_SECRET)=|^[0-9a-fA-F]{{64}}$' \"$f\" > \"$t\" || true; else : > \"$t\"; fi; printf '%s\\n' SOW_MAPS_ROOT={maps_root} SOW_MAPS_CATALOG_PATH={maps_catalog_path} SOW_RELAY_HOST={relay_host} SOW_RELAY_WORKERS={relay_workers} SOW_RELAY_WORKER_COUNT={relay_worker_count} SOW_RELAY_MGMT_URL={relay_mgmt_url} SOW_RELAY_MGMT_SCHEME={relay_mgmt_scheme} SOW_RELAY_MGMT_RESOLVE_IP={relay_mgmt_resolve_ip} WOU_ID_URL={wou_id_url} WOU_ID_RESOLVE_IP={wou_id_resolve_ip} SOW_PLAY_GAMES_APP_ID={play_games_app_id} SOW_PLAY_GAMES_WEB_CLIENT_ID={play_games_client_id} SOW_PLAY_GAMES_MATCH_EVENT_ID={play_games_match_event_id} SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID={play_games_achievement_id} SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID={play_games_battle_hardened_id} SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID={play_games_victory_march_id} SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID={play_games_laurel_hoard_id} SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID={play_games_first_command_id} SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID={play_games_commander_victorious_id} SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID={play_games_veteran_commander_id} SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID={play_games_banner_collector_id} SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID={play_games_leader_path_id} SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID={play_games_leaderboard_id} | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_DB_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {db_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_RELAY_CONTROL_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {control_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_PLAY_GAMES_WEB_CLIENT_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {play_games_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; done; rm -rf /tmp/sow-env-update",
            maps_root = shell_quote(&maps_root),
            maps_catalog_path = shell_quote(&maps_catalog_path),
            relay_host = shell_quote(&relay_host),
            relay_workers = shell_quote(&relay_workers),
            relay_worker_count = shell_quote(&relay_worker_count),
            relay_mgmt_url = shell_quote(&relay_mgmt_url),
            relay_mgmt_scheme = shell_quote(&relay_mgmt_scheme),
            relay_mgmt_resolve_ip = shell_quote(&relay_mgmt_resolve_ip),
            wou_id_url = shell_quote(&wou_id_url),
            wou_id_resolve_ip = shell_quote(&wou_id_resolve_ip),
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
    let env_update = if runtime_env {
        format!(
            "{env_update_base}; for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do if sudo test -f \"$f\"; then sudo cp -p \"$f\" \"$f.bak_$(date +%s)\"; fi; t=$(mktemp /tmp/sow-wou-env.XXXXXX); if sudo test -f \"$f\"; then sudo grep -v '^WOU_SOW_IDENTITY_SECRET=' \"$f\" > \"$t\" || true; else : > \"$t\"; fi; printf '%s' 'WOU_SOW_IDENTITY_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {secret_file} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; done",
            secret_file = shell_quote(wou_identity_secret_file.as_deref().unwrap()),
        )
    } else {
        env_update_base
    };
    let settlement_secret_update = if let Some(secret_file) = settlement_secret_file.as_deref() {
        format!(
            r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do
    t=$(mktemp /tmp/sow-settlement-env.XXXXXX)
    if sudo test -f "$f"; then sudo grep -v '^SOW_RELAY_SETTLEMENT_SECRET=' "$f" > "$t" || true; else : > "$t"; fi
    if [ "$f" = /zroot/jails/sow-database/usr/local/etc/sow/sow.env ]; then
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
            "for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do if sudo test -f \"$f\"; then sudo cp -p \"$f\" \"$f.bak_$(date +%s)\"; fi; t=$(mktemp /tmp/sow.env.XXXXXX); if sudo test -f \"$f\"; then sudo grep -v '^SOW_REVENUECAT_WEBHOOK_SECRET=' \"$f\" > \"$t\" || true; else : > \"$t\"; fi; printf '%s' 'SOW_REVENUECAT_WEBHOOK_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {secret_file} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; done; rm -f {secret_file}",
            secret_file = shell_quote(secret_file),
        )
    } else {
        ":".to_string()
    };
    let stripe_scrub = if runtime_env {
        format!(
            r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do
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
        wou_identity_secret_file.as_deref(),
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
        r#"db_env=/zroot/jails/sow-database/usr/local/etc/sow/sow.env
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
    let resolver_update = if server_restart {
        r#"server_resolver=/zroot/jails/sow-server/etc/resolv.conf
if sudo test -f "$server_resolver"; then
    sudo cp -p "$server_resolver" "$server_resolver.bak_$(date +%s)"
fi
sudo install -o root -g wheel -m 0644 /etc/resolv.conf "$server_resolver""#
            .to_string()
    } else {
        ":".to_string()
    };
    let env_backups = if runtime_env {
        r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do
    if sudo test -f "$f"; then sudo cp -p "$f" "$f.bak_$(date +%s)"; fi
done"#.to_string()
    } else {
        ":".to_string()
    };
    let mut remote = String::from(
        r#"set -eu
old=$(sudo readlink /srv/sow/current 2>/dev/null || true)
target=__TARGET__
stage=__STAGE__
rollback() {
    set +e
    sudo rm -f /srv/sow/current
    if [ -n "$old" ]; then sudo ln -s "$old" /srv/sow/current; fi
    if __DB_RESTART__; then sudo jexec sow-database service sow_database restart || true; fi
    if __SERVER_RESTART__; then sudo jexec sow-server service sow_server restart || true; fi
    __SECRET_CLEANUP__
    exit 78
}
__SECRET_TRAP__
sudo install -d -m 0755 __RELEASES__
sudo rm -rf "$target"
sudo mkdir -p "$target"
sudo cp -Rp "$stage/." "$target/"
sudo chown -R root:sow "$target"
sudo find "$target" -type d -exec chmod 0755 {} +
sudo find "$target" -type f -exec chmod 0644 {} +
sudo chmod 0550 "$target"/bin/*
if __SERVER_RESTART__; then
    sudo jexec sow-server service sow_server stop || rollback
    if ! sudo jexec sow-server /bin/sh -s <<'ARCHIVE_SETTLED_STATES'
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
sudo install -o root -g wheel -m 0555 "$target/ops/rc.d/sow_server" /zroot/jails/sow-server/usr/local/etc/rc.d/sow_server
sudo install -o root -g wheel -m 0555 "$target/ops/rc.d/sow_database" /zroot/jails/sow-database/usr/local/etc/rc.d/sow_database
link="/srv/sow/.current.$$"
sudo ln -s "releases/__ID__" "$link"
sudo mv -hf "$link" /srv/sow/current
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
__ENV_BACKUPS__
__ENV_UPDATE__
__SETTLEMENT_SECRET_UPDATE__
__REVENUECAT_UPDATE__
__STRIPE_SCRUB__
__DATABASE_SECRET_SCRUB__
__RESOLVER_UPDATE__
if __DB_RESTART__; then
    sudo jexec sow-database service sow_database restart || rollback
    i=0
    until curl -fsS --max-time 5 http://127.0.0.1:25585/healthz >/dev/null; do
        i=$((i+1))
        if [ "$i" -ge 180 ]; then echo "database healthz did not become ready before server restart" >&2; rollback; fi
        sleep 1
    done
fi
if __SERVER_RESTART__; then sudo jexec sow-server service sow_server restart || rollback; fi
if ! sudo jexec sow-database service sow_database status >/dev/null 2>&1; then rollback; fi
if ! sudo jexec sow-server service sow_server status >/dev/null 2>&1; then rollback; fi
__SECRET_CLEANUP__
"#,
    );
    let replacements = [
        ("__TARGET__", shell_quote(&target)),
        ("__STAGE__", shell_quote(&stage)),
        ("__RELEASES__", shell_quote(REMOTE_RELEASES)),
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
        ("__SETTLEMENT_SECRET_UPDATE__", settlement_secret_update),
        ("__ENV_BACKUPS__", env_backups),
        ("__REVENUECAT_UPDATE__", revenuecat_update),
        ("__STRIPE_SCRUB__", stripe_scrub),
        ("__DATABASE_SECRET_SCRUB__", database_secret_scrub),
        ("__RESOLVER_UPDATE__", resolver_update),
        (
            "__SECRET_TRAP__",
            if runtime_env {
                format!("trap 'rm -f {}' EXIT", secret_files.join(" "))
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
    sudo tail -80 /var/log/sow/database.log >&2 || true
    echo "--- server.log (tail) ---" >&2
    sudo tail -120 /var/log/sow/server.log >&2 || true
    exit 1
}
if ! sudo jexec sow-database service sow_database status; then fail "database service is not running"; fi
if ! sudo jexec sow-server service sow_server status; then fail "server service is not running"; fi
i=0
until curl -fsS --max-time 5 http://127.0.0.1:25585/healthz >/dev/null; do
    i=$((i+1))
    if [ "$i" -ge 180 ]; then fail "database healthz timeout"; fi
    sleep 1
done
if ! /usr/local/bin/valkey-cli -h 127.0.0.1 ping | grep -q PONG; then fail "valkey ping failed"; fi
j=0
until sudo sockstat -4l | grep -q '127.0.0.1:25564'; do
    j=$((j+1))
    if [ "$j" -ge 180 ]; then fail "server websocket port timeout"; fi
    sleep 1
done
"#,
    );
    if plan.server {
        checks.push_str(
            r#"
bot_line=$(sudo tail -500 /var/log/sow/server.log | grep '\[BOT_POOL\].*installed with' | tail -1 || true)
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

fn retain_releases(config: &Config) -> Result<()> {
    let command = format!(
        "set -eu; current=$(sudo readlink /srv/sow/current 2>/dev/null || true); case \"$current\" in releases/*) current_path=\"/srv/sow/$current\" ;; *) current_path=\"$current\" ;; esac; i=0; for dir in $(sudo ls -dt {REMOTE_RELEASES}/* 2>/dev/null || true); do i=$((i+1)); [ $i -le 5 ] && continue; [ \"$current_path\" = \"$dir\" ] && continue; case \"$dir\" in {REMOTE_RELEASES}/*) sudo rm -rf -- \"$dir\" ;; esac; done; sudo find {REMOTE_RELEASES} -maxdepth 2 -type l -name '.current.*' -delete"
    );
    run("ssh", &[&config.control_host, &command], None).context("release retention failed")
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
mod build_fingerprint_tests {
    use super::freebsd_build_fingerprint;
    use std::fs;

    #[test]
    fn freebsd_cache_tracks_atlas_and_bundled_world_map() {
        let root = tempfile::tempdir().unwrap();
        let atlas = root.path().join("assets/geo_entities.json");
        let map = root.path().join("assets/maps/world/map.bin.br");
        fs::create_dir_all(map.parent().unwrap()).unwrap();
        fs::write(&atlas, b"atlas-v1").unwrap();
        fs::write(&map, b"map-v1").unwrap();

        let before = freebsd_build_fingerprint(root.path()).unwrap();
        fs::write(&atlas, b"atlas-v2").unwrap();
        let after_atlas = freebsd_build_fingerprint(root.path()).unwrap();
        assert_ne!(before, after_atlas);

        fs::write(&map, b"map-v2").unwrap();
        assert_ne!(after_atlas, freebsd_build_fingerprint(root.path()).unwrap());
    }

    #[test]
    fn relay_handover_hashes_root_owned_candidate_with_sudo() {
        let source = include_str!("prod.rs");
        assert!(source.contains("test \"$(sudo sha256sum {destination}"));
    }
}
