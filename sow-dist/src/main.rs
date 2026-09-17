use anyhow::{Context, Result, bail};
use base64::Engine as _;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::{collections::{HashMap, HashSet}, env, fs, thread};

mod prod;

const WASM_OPT_TAG: &str = "oz-cli-v1";
const POKI_FORBIDDEN_MARKERS: &[&str] = &[
    "sdk.crazygames.com",
    "js.stripe.com",
    "stripe",
    "id.worldofunreal.com",
    "worldofunreal.com/wouid.svg",
    "discord.gg",
    "t.me/shadowsofwar",
    "github.com/worldofunreal",
    "SOW_startAndroidPlayGamesAutoAuth",
    "SOW_prepareAndroidAuthState",
    "SOW_ensureWouAnonymousSession",
    "SOW_getWouSession",
    "SOW_getAuthState",
    "SOW_isSowProductionHost",
    "SOW_portalShowAuthPrompt",
    "SOW_portalSignOut",
    "SOW_startWouOAuth",
    "SOW_signOutWou",
    "/api/v1/auth/",
    "wou_session_token",
    "wou_user_data",
    "class='sow-menu__signin'",
    "SIGN IN",
    "/store/",
    "main_menu.store.js",
    "allowfullscreen",
    "web-share",
    "focus-without-user-activation",
    "monetization",
];

fn run(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    println!(
        "+ {cmd} {}",
        args.iter()
            .map(|a| shell_quote(a))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut c = Command::new(cmd);
    c.args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    if !c.spawn()?.wait()?.success() {
        bail!("{cmd} failed");
    }
    Ok(())
}

fn output(cmd: &str, args: &[&str]) -> Result<String> {
    let o = Command::new(cmd).args(args).output()?;
    if !o.status.success() {
        bail!("{cmd} failed: {}", String::from_utf8_lossy(&o.stderr));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "./_=-+:,".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

struct Paths {
    root: PathBuf,
    shell: PathBuf,
    assets_shell: PathBuf,
    assets_gameplay: PathBuf,
    assets_site: PathBuf,
    assets_maps: PathBuf,
    map_sources: PathBuf,
    dist_web: PathBuf,
    dist_cg: PathBuf,
    dist_poki: PathBuf,
    wasm_input: PathBuf,
    wasm_cache: PathBuf,
}

impl Paths {
    fn discover() -> Result<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let t = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("target"));
        let s = root.join("dist/.sow-state");
        Ok(Self {
            wasm_input: t.join("wasm32-unknown-unknown/wasm-release/sow_client.wasm"),
            wasm_cache: s.join("wasm-opt-cache"),
            shell: root.join("sow-web/shell"),
            assets_shell: root.join("assets/shell"),
            assets_gameplay: root.join("assets/gameplay"),
            assets_site: root.join("assets/site"),
            assets_maps: root.join("assets/maps"),
            map_sources: root.join("assets/map_sources"),
            dist_web: root.join("dist/web"),
            dist_cg: root.join("dist/crazygames"),
            dist_poki: root.join("dist/poki"),
            root,
        })
    }
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0u8; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("create directory {}", dst.display()))?;
    for e in fs::read_dir(src).with_context(|| format!("read directory {}", src.display()))? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), &to)
                .with_context(|| format!("copy {} to {}", e.path().display(), to.display()))?;
        }
    }
    Ok(())
}

fn optimize_poki_webp(src: &Path, dst: &Path, geometry: &str) -> Result<()> {
    let status = Command::new("magick")
        .arg(src)
        .args(["-resize", geometry, "-strip", "-quality", "70"])
        .args(["-define", "webp:method=6"])
        .arg(dst)
        .status()
        .with_context(|| "Poki packaging requires ImageMagick (magick)")?;
    if !status.success() {
        bail!("Poki image optimization failed for {}", src.display());
    }
    Ok(())
}

fn optimize_poki_thumbnail(src: &Path, dst: &Path) -> Result<()> {
    let status = Command::new("magick")
        .arg(src)
        .args([
            "-resize",
            "628x628^",
            "-gravity",
            "center",
            "-extent",
            "628x628",
            "-strip",
            "-define",
            "png:compression-level=9",
        ])
        .arg(dst)
        .status()
        .with_context(|| "Poki packaging requires ImageMagick (magick)")?;
    if !status.success() {
        bail!("Poki thumbnail optimization failed for {}", src.display());
    }
    Ok(())
}

fn copy_poki_assets(src: &Path, dst: &Path) -> Result<()> {
    copy_dir(&src.join("shell/loader"), &dst.join("shell/loader"))?;
    fs::create_dir_all(dst.join("shell/leaders"))?;
    for entry in fs::read_dir(src.join("shell/leaders"))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let geometry = if name.ends_with("_mobile.webp") {
            "540x960"
        } else {
            "960x540"
        };
        optimize_poki_webp(
            &entry.path(),
            &dst.join("shell/leaders").join(entry.file_name()),
            geometry,
        )?;
    }
    copy_dir(&src.join("gameplay/avatars"), &dst.join("gameplay/avatars"))?;
    copy_dir(&src.join("gameplay/currency"), &dst.join("gameplay/currency"))?;
    let mobile_nav = dst.join("shell/mobile-nav");
    fs::create_dir_all(&mobile_nav)?;
    for file in ["heroes.webp", "battle.webp", "profile.webp"] {
        fs::copy(src.join("shell/mobile-nav").join(file), mobile_nav.join(file))?;
    }
    Ok(())
}

fn copy_poki_maps(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    fs::copy(src.join("catalog.bin"), dst.join("catalog.bin"))?;
    for entry in fs::read_dir(src).with_context(|| format!("read directory {}", src.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let out_dir = dst.join(entry.file_name());
        fs::create_dir_all(&out_dir)?;
        let map = entry.path().join("map.bin.br");
        if map.is_file() {
            fs::copy(&map, out_dir.join("map.bin.br"))?;
        }
        let thumbnail = entry.path().join("thumbnail.webp");
        if thumbnail.is_file() {
            optimize_poki_webp(&thumbnail, &out_dir.join("thumbnail.webp"), "256x144")?;
        }
    }
    Ok(())
}

fn refresh_map_thumbnails(maps_root: &Path, map_sources: &Path) -> Result<()> {
    let manifest_path = map_sources.join("thumbnail_frames.json");
    let manifest = if manifest_path.is_file() {
        let value: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&manifest_path)
                .with_context(|| format!("read {}", manifest_path.display()))?,
        )
        .with_context(|| format!("parse {}", manifest_path.display()))?;
        if value.get("version").and_then(|v| v.as_u64()) != Some(1) {
            bail!(
                "unsupported thumbnail frame manifest: {}",
                manifest_path.display()
            );
        }
        if let Some(sources) = value.get("sources").and_then(|v| v.as_object()) {
            for (name, metadata) in sources {
                let path = map_sources.join(name);
                require_file(&path, "thumbnail source")?;
                if let Some(expected) = metadata.get("sha256").and_then(|v| v.as_str()) {
                    let actual = file_sha256(&path)?;
                    if actual != expected {
                        bail!("thumbnail source hash mismatch for {}", path.display());
                    }
                }
            }
        }
        Some(value)
    } else {
        None
    };

    let mut rendered_sources = HashMap::new();
    let mut map_dirs = fs::read_dir(maps_root)
        .with_context(|| format!("read maps directory {}", maps_root.display()))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    map_dirs.sort_by_key(|entry| entry.file_name());

    for entry in map_dirs {
        let dir = entry.path();
        let map_path = dir.join("map.bin");
        if !map_path.is_file() {
            continue;
        }
        let bytes =
            fs::read(&map_path).with_context(|| format!("read map {}", map_path.display()))?;
        let map = sow_core::map_file::parse(&bytes)
            .map_err(|error| anyhow::anyhow!("parse {}: {error}", map_path.display()))?;
        let thumbnail = dir.join("thumbnail.webp");
        let frame = manifest
            .as_ref()
            .and_then(|value| value.get("maps"))
            .and_then(|maps| maps.get(entry.file_name().to_string_lossy().as_ref()))
            .and_then(|map| map.get("frame"))
            .and_then(|frame| frame.as_array())
            .and_then(|frame| {
                let values = frame
                    .iter()
                    .map(|value| value.as_i64())
                    .collect::<Option<Vec<_>>>()?;
                (values.len() == 4 && values[2] > 0 && values[3] > 0).then_some(
                    sow_map::SourceFrame {
                        x: values[0],
                        y: values[1],
                        width: values[2] as u32,
                        height: values[3] as u32,
                    },
                )
            });
        let source = manifest
            .as_ref()
            .and_then(|value| value.get("maps"))
            .and_then(|maps| maps.get(entry.file_name().to_string_lossy().as_ref()))
            .and_then(|map| map.get("source"))
            .and_then(|source| source.as_str());

        if let (Some(frame), Some(source)) = (frame, source) {
            let source_path = map_sources.join(source);
            if !rendered_sources.contains_key(&source_path) {
                let rendered = sow_map::render_source_file(&source_path).map_err(|error| {
                    anyhow::anyhow!("render {}: {error}", source_path.display())
                })?;
                rendered_sources.insert(source_path.clone(), rendered);
            }
            let rendered = rendered_sources
                .get(&source_path)
                .expect("source inserted above");
            sow_map::write_rendered_source_thumbnail(rendered, frame, &thumbnail)
                .map_err(|error| anyhow::anyhow!("write {} thumbnail: {error}", dir.display()))?;
        } else {
            sow_map::write_map_thumbnail(&map, &thumbnail)
                .map_err(|error| anyhow::anyhow!("write {} thumbnail: {error}", dir.display()))?;
        }
    }
    Ok(())
}

fn thumbnail_cache_bust(maps_root: &Path) -> Result<String> {
    let mut files = fs::read_dir(maps_root)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("thumbnail.webp"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        bail!(
            "no generated map thumbnails found in {}",
            maps_root.display()
        );
    }
    let mut hash = Sha256::new();
    for path in files {
        hash.update(path.strip_prefix(maps_root)?.to_string_lossy().as_bytes());
        hash.update(fs::read(path)?);
    }
    Ok(format!("{:x}", hash.finalize())[..12].to_string())
}

fn require_file(path: &Path, label: &str) -> Result<()> {
    if !path.is_file() {
        bail!("{label} missing: {}", path.display());
    }
    if fs::metadata(path)?.len() == 0 {
        bail!("{label} empty: {}", path.display());
    }
    Ok(())
}

fn brotli_dst(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.br", path.display()))
}

fn compress_brotli(src: &Path, dst: &Path) -> Result<()> {
    let input = fs::read(src)?;
    if input.is_empty() {
        bail!("brotli source empty");
    }
    let mut out = Vec::new();
    let mut w = brotli::CompressorWriter::new(&mut out, 4096, 9, 22);
    w.write_all(&input)?;
    w.flush()?;
    drop(w);
    fs::write(dst, &out)?;
    Ok(())
}

fn prune_qs(root: &Path) -> Result<()> {
    let mut n = 0u32;
    for e in walkdir::WalkDir::new(root) {
        let e = e?;
        if e.file_type().is_file() && e.file_name().to_string_lossy().contains("?v=") {
            fs::remove_file(e.path())?;
            n += 1;
        }
    }
    if n > 0 {
        println!("Removed {n} querystring artifact(s)");
    }
    Ok(())
}

fn compile_wasm(paths: &Paths, dev: bool) -> Result<()> {
    println!("==> Compiling WASM (wasm-release)...");
    let mut a = vec![
        "build",
        "--profile",
        "wasm-release",
        "-p",
        "sow-client",
        "--target",
        "wasm32-unknown-unknown",
    ];
    if dev {
        a.extend_from_slice(&["--features", "dev"]);
        println!("==> (local) dev tools enabled");
    }
    let mut c = Command::new("cargo");
    c.args(&a)
        .current_dir(&paths.root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    c.env("RUSTFLAGS", "-C target-feature=-bulk-memory");
    c.env("CARGO_BUILD_JOBS", "4");
    if !c.spawn()?.wait()?.success() {
        bail!("WASM compile failed");
    }
    require_file(&paths.wasm_input, "WASM output")?;
    Ok(())
}

fn run_bindgen(wasm: &Path, out: &Path, name: &str) -> Result<()> {
    println!("==> Running wasm-bindgen for {name}...");
    wasm_bindgen_cli_support::Bindgen::new()
        .input_path(wasm)
        .web(true)?
        .out_name(name)
        .typescript(false)
        .generate(out)?;
    println!("✅ wasm-bindgen finished");
    Ok(())
}

fn run_wasm_opt(path: &Path, cache: &Path) -> Result<()> {
    require_file(path, "wasm-opt input")?;
    let hash = file_sha256(path)?;
    fs::create_dir_all(cache)?;
    let c = cache.join(format!("{WASM_OPT_TAG}-{hash}.wasm"));
    let cb = cache.join(format!("{WASM_OPT_TAG}-{hash}.wasm.br"));
    let br = brotli_dst(path);
    if c.is_file() {
        fs::copy(&c, path)?;
        if cb.is_file() {
            fs::copy(&cb, &br)?;
        } else {
            compress_brotli(path, &br)?;
            fs::copy(&br, &cb)?;
        }
        require_file(path, "wasm-opt")?;
        require_file(&br, "brotli")?;
        return Ok(());
    }
    println!("==> wasm-opt -O3 ({})...", path.display());
    let parent = path.parent().unwrap();
    let tmp = tempfile::NamedTempFile::new_in(parent)?;
    run(
        "wasm-opt",
        &[
            "-O3",
            "--enable-bulk-memory",
            "--enable-nontrapping-float-to-int",
            "--vacuum",
            path.to_str().context("WASM path is not UTF-8")?,
            "-o",
            tmp.path().to_str().context("temporary path is not UTF-8")?,
        ],
        None,
    )?;
    tmp.persist(path)
        .map_err(|e| anyhow::anyhow!("persist: {}", e.error))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    require_file(path, "wasm-opt")?;
    fs::copy(path, &c)?;
    compress_brotli(path, &br)?;
    fs::copy(&br, &cb)?;
    println!("✅ wasm-opt finished");
    Ok(())
}

fn brotli_file(path: &Path) -> Result<()> {
    require_file(path, "brotli source")?;
    let dst = brotli_dst(path);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if dst.is_file() && fs::metadata(&dst)?.len() > 0 {
        println!("==> Brotli cache hit for {name}");
        return Ok(());
    }
    compress_brotli(path, &dst)?;
    println!(
        "✅ Brotli {name} → {} ({}b)",
        dst.display(),
        fs::metadata(&dst)?.len()
    );
    Ok(())
}

fn minify_js(path: &Path) -> Result<()> {
    require_file(path, "minify input")?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    println!("==> Minifying {name}...");
    let src = fs::read_to_string(path)?;
    if src.trim().is_empty() {
        bail!("minify input empty");
    }
    let m = minifier::js::minify(&src).to_string();
    if m.is_empty() {
        bail!("minify empty");
    }
    fs::write(path, m)?;
    println!("✅ Minifying {name} finished");
    Ok(())
}

fn read_shell_bundle(shell: &Path, manifest: &str, parts: &[&str]) -> Result<String> {
    let manifest_source = fs::read_to_string(shell.join(manifest))?;
    let mut bundle = String::new();
    for part in parts {
        if !manifest_source.contains(part) {
            bail!("{manifest}: missing bundle part {part}");
        }
        let source = fs::read_to_string(shell.join(part))
            .with_context(|| format!("read shell bundle part {part}"))?;
        bundle.push_str("\n/* --- ");
        bundle.push_str(part);
        bundle.push_str(" --- */\n");
        bundle.push_str(&source);
        if !source.ends_with('\n') {
            bundle.push('\n');
        }
    }
    if manifest == "main_menu.js" {
        validate_web_bundle(&bundle)?;
    }
    Ok(bundle)
}

fn web_catalog_value<'a>(catalog: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut parts = key.split('.');
    let domain = parts.next()?;
    let name = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    catalog.get(domain)?.get(name)
}

fn web_catalog_has_prefix(catalog: &serde_json::Value, prefix: &str) -> bool {
    let Some((domain, key_prefix)) = prefix.split_once('.') else {
        return false;
    };
    catalog
        .get(domain)
        .and_then(serde_json::Value::as_object)
        .is_some_and(|values| values.keys().any(|key| key.starts_with(key_prefix)))
}

fn validate_web_bundle(bundle: &str) -> Result<()> {
    let catalog = validate_web_catalogs()?;
    let mut offset = 0;
    while let Some(found) = bundle[offset..].find("SOW_t(") {
        let start = offset + found + "SOW_t(".len();
        let rest = &bundle[start..];
        let Some(quote) = rest.as_bytes().first().copied() else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            offset = start;
            continue;
        }
        let quote = quote as char;
        let value = &rest[1..];
        let Some(end) = value.find(quote) else {
            bail!("unterminated SOW_t key in web bundle");
        };
        let key = &value[..end];
        if key.ends_with('_') {
            if !web_catalog_has_prefix(&catalog, key) {
                bail!("web bundle uses unknown localization key prefix {key}");
            }
        } else if web_catalog_value(&catalog, key).is_none() {
            bail!("web bundle uses unknown localization key {key}");
        }
        offset = start + 1 + end;
    }
    Ok(())
}

fn strip_marked_section(source: &str, begin: &str, end: &str) -> Result<String> {
    let start = source
        .find(begin)
        .with_context(|| format!("missing bundle marker {begin}"))?;
    let end_start = source[start..]
        .find(end)
        .map(|offset| start + offset)
        .with_context(|| format!("missing bundle marker {end}"))?;
    let end_after = end_start + end.len();
    let mut output = String::with_capacity(source.len());
    output.push_str(&source[..start]);
    output.push_str(&source[end_after..]);
    Ok(output)
}

struct IndexBuild<'a> {
    version: &'a str,
    js: &'a str,
    wasm: &'a str,
    ts: &'a str,
    maps_cache_bust: &'a str,
    target: WebTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebTarget {
    Local,
    CrazyGames,
    Poki,
}

fn build_index(paths: &Paths, out: &Path, build: IndexBuild<'_>) -> Result<()> {
    let IndexBuild {
        version,
        js,
        wasm,
        ts,
        maps_cache_bust,
        target,
    } = build;
    let cg = target == WebTarget::CrazyGames;
    let poki = target == WebTarget::Poki;
    let portal = cg || poki;
    let tpl = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let splash_desktop = inline_webp(&paths.assets_shell.join("loader/sow-splash-desktop.webp"))?;
    let splash_mobile = inline_webp(&paths.assets_shell.join("loader/sow-splash-mobile.webp"))?;
    let locale_codes = serde_json::to_string(
        &sow_i18n::Language::registry()
            .iter()
            .map(|(_, code, _)| *code)
            .collect::<Vec<_>>(),
    )?;
    let locale_base = if portal { "locales" } else { "../locales" };
    let store_portals_template = format!("src=\"./sdk/store_portals.js?v={ts}\"");
    let store_portals_src = if portal {
        format!("src=\"sdk/store_portals.js?v={ts}\"")
    } else {
        format!("src=\"../sdk/store_portals.js?v={ts}\"")
    };
    let mut html = tpl
        .replace("__VERSION__", version)
        .replace(
            "./__JS_FILE__",
            &if portal {
                format!("./{js}")
            } else {
                "../__JS_FILE__".to_string()
            },
        )
        .replace(
            "./__WASM_FILE__",
            &if portal {
                format!("./{wasm}")
            } else {
                "../__WASM_FILE__".to_string()
            },
        )
        .replace("__JS_FILE__", js)
        .replace("__WASM_FILE__", wasm)
        .replace("__BUILD_TS__", ts)
        .replace("__SOW_LOCALE_BASE__", locale_base)
        .replace(
            "__SOW_LOCALE_CATALOG_VERSION__",
            &sow_i18n::WEB_CATALOG_VERSION.to_string(),
        )
        .replace("__SOW_LOCALE_CODES__", &locale_codes)
        .replace("__MAPS_CACHE_BUST__", maps_cache_bust)
        .replace("__SOW_SPLASH_DESKTOP_DATA__", &splash_desktop)
        .replace("__SOW_SPLASH_MOBILE_DATA__", &splash_mobile)
        .replace(
            "__ASSETS_UI_BASE__",
            if cg {
                // Portal iframe: relative paths resolve against the game CDN,
                // but the whitelist bundle ships no assets — point straight at
                // the production CDN (served with ACAO for cross-origin reads).
                "https://shadowsofwar.io/assets/shell/loader/"
            } else if poki {
                "./assets/shell/loader/"
            } else {
                "/assets/shell/loader/"
            },
        )
        .replace(
            "href=\"./sow.svg\"",
            if portal {
                "href=\"sow.svg\""
            } else {
                "href=\"../sow.svg\""
            },
        )
        .replace(
            "href=\"./favicon.ico\"",
            if portal {
                "href=\"favicon.ico\""
            } else {
                "href=\"../favicon.ico\""
            },
        )
        .replace(
            "src=\"./loader.js\"",
            if portal {
                "src=\"loader.js\""
            } else {
                "src=\"../loader.js\""
            },
        )
        .replace(&store_portals_template, &store_portals_src)
        .replace(
            "<!-- __SOW_SERVICE_WORKER_SLOT__ -->",
            match target {
                WebTarget::Local => {
                    "if ('serviceWorker' in navigator && window.location.hostname !== \"appassets.androidplatform.net\" && !isPortal) { navigator.serviceWorker.register('/sw.js', { scope: './' }).catch(function (err) { console.warn('Service worker registration failed:', err); }); }"
                }
                WebTarget::CrazyGames => {
                    "if ('serviceWorker' in navigator && !isPortal) { navigator.serviceWorker.register('sw.js', { scope: '/' }).catch(function (err) { console.warn('Service worker registration failed:', err); }); }"
                }
                WebTarget::Poki => "",
            },
        )
        .replace(
            "/* PORTAL_BOOT_SLOT: SOW_PORTAL / SOW_WS_URL overrides injected by sow-dist crazygames. */",
            &if portal {
                // CG keeps the marker: package_cg injects the portal boot line.
                "/* PORTAL_BOOT_SLOT */".to_string()
            } else {
                // Production shell declares every endpoint explicitly — the
                // client resolves strict config only (no fallbacks).
                if target == WebTarget::Local {
                    concat!(
                        "window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; ",
                        "window.SOW_MAPS_URL = \"/maps\"; ",
                        "window.SOW_ASSETS_URL = \"/assets\"; ",
                        "window.SOW_DATABASE_URL = \"https://shadowsofwar.io/api\";"
                    )
                    .to_string()
                } else {
                    concat!(
                        "window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; ",
                        "window.SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"; ",
                        "window.SOW_ASSETS_URL = \"https://shadowsofwar.io/assets\"; ",
                        "window.SOW_DATABASE_URL = \"https://shadowsofwar.io/api\";"
                    )
                    .to_string()
                }
            },
        );
    if poki {
        for (begin, end) in [
            (
                "/* POKI_ANDROID_AUTH_BEGIN */",
                "/* POKI_ANDROID_AUTH_END */",
            ),
            (
                "/* POKI_ANDROID_AUTH_STATE_BEGIN */",
                "/* POKI_ANDROID_AUTH_STATE_END */",
            ),
            (
                "/* POKI_WOU_AUTH_BEGIN */",
                "/* POKI_WOU_AUTH_END */",
            ),
        ] {
            html = strip_marked_section(&html, begin, end)?;
        }
    }
    let index = if portal {
        out.join("index.html")
    } else {
        out.join("play/index.html")
    };
    fs::create_dir_all(index.parent().unwrap())?;
    fs::write(&index, &html)?;
    let mut loader = fs::read_to_string(paths.shell.join("loader.js"))?;
    if poki {
        loader = strip_marked_section(
            &loader,
            "/* SOW_FIRST_PARTY_ANALYTICS_BEGIN */",
            "/* SOW_FIRST_PARTY_ANALYTICS_END */",
        )?;
    }
    let loader = loader.replace("</script>", "<\\/script>");
    let menu_css = read_shell_bundle(
        &paths.shell,
        "main_menu.css",
        &[
            "main_menu.base.css",
            "main_menu.hud.css",
            "main_menu.profile.css",
        ],
    )?;
    let menu_parts: Vec<&str> = if poki {
        vec![
            "main_menu.i18n.js",
            "main_menu.core.js",
            "main_menu.motion.js",
            "main_menu.lobbies.js",
            "main_menu.heroes.js",
            "main_menu.profile.js",
            "main_menu.poki.js",
            "main_menu.tutorial.js",
            "main_menu.shell.js",
            "main_menu.hud.js",
        ]
    } else {
        vec![
            "main_menu.i18n.js",
            "main_menu.core.js",
            "main_menu.motion.js",
            "main_menu.lobbies.js",
            "main_menu.store.js",
            "main_menu.heroes.js",
            "main_menu.profile.js",
            "main_menu.tutorial.js",
            "main_menu.shell.js",
            "main_menu.hud.js",
        ]
    };
    let mut menu_js = read_shell_bundle(
        &paths.shell,
        "main_menu.js",
        &menu_parts,
    )?
    .replace("</script>", "<\\/script>");
    if poki {
        menu_js = strip_marked_section(
            &menu_js,
            "/* POKI_RENDER_REPLACEMENT_BEGIN */",
            "/* POKI_RENDER_REPLACEMENT_END */",
        )?;
        menu_js = strip_marked_section(
            &menu_js,
            "/* POKI_STRIPE_STATE_BEGIN */",
            "/* POKI_STRIPE_STATE_END */",
        )?;
        for (begin, end) in [
            (
                "/* POKI_SHARED_UPDATE_TOPBAR_BEGIN */",
                "/* POKI_SHARED_UPDATE_TOPBAR_END */",
            ),
            (
                "/* POKI_SHARED_STORE_ACTIONS_BEGIN */",
                "/* POKI_SHARED_STORE_ACTIONS_END */",
            ),
            (
                "/* POKI_SHARED_STORE_UNLOCK_BEGIN */",
                "/* POKI_SHARED_STORE_UNLOCK_END */",
            ),
            (
                "/* POKI_SHARED_AUTH_ACTIONS_BEGIN */",
                "/* POKI_SHARED_AUTH_ACTIONS_END */",
            ),
            (
                "/* POKI_SHARED_AUTH_EVENTS_BEGIN */",
                "/* POKI_SHARED_AUTH_EVENTS_END */",
            ),
            (
                "/* POKI_SHARED_STORE_EVENTS_BEGIN */",
                "/* POKI_SHARED_STORE_EVENTS_END */",
            ),
            (
                "/* POKI_SHARED_AUTH_SUBMIT_BEGIN */",
                "/* POKI_SHARED_AUTH_SUBMIT_END */",
            ),
        ] {
            menu_js = strip_marked_section(&menu_js, begin, end)?;
        }
        for forbidden in [
            "https://id.worldofunreal.com",
            "https://discord.gg/d6ZDeChSE",
            "https://t.me/shadowsofwario",
            "https://github.com/worldofunreal/shadows-of-war",
            "https://worldofunreal.com/wouid.svg",
        ] {
            menu_js = menu_js.replace(forbidden, "about:blank");
        }
    }
    let mut fh = fs::read_to_string(&index)?;
    let marker = "/* __INLINE_LOADER_JS__ */";
    if fh.contains(marker) {
        fh = fh.replacen(marker, &loader, 1);
    } else if fh.contains(r#"<script src="../loader.js"></script>"#) {
        fh = fh.replacen(
            r#"<script src="../loader.js"></script>"#,
            &format!("<script>\n{loader}\n</script>"),
            1,
        );
    } else if fh.contains(r#"<script src="./loader.js"></script>"#) {
        fh = fh.replacen(
            r#"<script src="./loader.js"></script>"#,
            &format!("<script>\n{loader}\n</script>"),
            1,
        );
    } else {
        bail!("index.html: no loader injection point");
    }
    let menu_css_marker = "/* __INLINE_MAIN_MENU_CSS__ */";
    if !fh.contains(menu_css_marker) {
        bail!("index.html: no main menu CSS injection point");
    }
    fh = fh.replacen(menu_css_marker, &menu_css, 1);
    let menu_js_marker = "/* __INLINE_MAIN_MENU_JS__ */";
    if !fh.contains(menu_js_marker) {
        bail!("index.html: no main menu JS injection point");
    }
    fh = fh.replacen(menu_js_marker, &menu_js, 1);
    if poki {
        fh = fh
            .replace("href=\"/fonts/fonts.css\"", "href=\"fonts/fonts.css\"")
            .replace("href=\"/manifest.webmanifest\"", "href=\"manifest.webmanifest\"")
            .replace(
                "<meta property=\"og:url\" content=\"https://shadowsofwar.io/play/\">",
                "",
            )
            .replace(
                "<meta property=\"twitter:url\" content=\"https://shadowsofwar.io/play/\">",
                "",
            )
            .replace("<meta property=\"og:image\" content=\"https://shadowsofwar.io/assets/shell/loader/sow-splash-desktop.webp\">", "")
            .replace("<meta property=\"twitter:image\" content=\"https://shadowsofwar.io/assets/shell/loader/sow-splash-desktop.webp\">", "")
            .replace("<link rel=\"canonical\" href=\"https://shadowsofwar.io/play/\">", "");
    }
    fs::write(&index, fh)?;
    Ok(())
}

fn inline_webp(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .with_context(|| format!("read critical loader asset {}", path.display()))?;
    Ok(format!(
        "data:image/webp;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn copy_shell(paths: &Paths, out: &Path) -> Result<()> {
    let fav = paths.shell.join("favicon_io");
    if fav.is_dir() {
        for e in fs::read_dir(&fav)? {
            let e = e?;
            let d = out.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &d)?;
            } else {
                fs::copy(e.path(), d)?;
            }
        }
    }
    fs::copy(
        paths.assets_shell.join("brand/sow.svg"),
        out.join("sow.svg"),
    )?;
    if paths.assets_shell.join("brand/sow-long.svg").is_file() {
        fs::copy(
            paths.assets_shell.join("brand/sow-long.svg"),
            out.join("sow-long.svg"),
        )?;
    }
    if paths.assets_shell.join("brand/tower.svg").is_file() {
        fs::copy(
            paths.assets_shell.join("brand/tower.svg"),
            out.join("tower.svg"),
        )?;
    }
    fs::copy(paths.shell.join("loader.js"), out.join("loader.js"))?;
    copy_dir(&paths.shell.join("sdk"), &out.join("sdk"))?;
    Ok(())
}

fn write_sw(out: &Path, version: &str, js: &str, wasm: &str, ts: &str) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let tpl = fs::read_to_string(root.join("sow-web/shell/sw.js.template"))?;
    fs::write(
        out.join("sw.js"),
        tpl.replace("__VERSION__", version)
            .replace("__JS_FILE__", js)
            .replace("__WASM_FILE__", wasm)
            .replace("__BUILD_TS__", ts),
    )?;
    Ok(())
}

fn write_manifest(out: &Path, version: &str, js: &str, wasm: &str, ts: &str) -> Result<()> {
    fs::write(
        out.join("game-manifest.json"),
        format!(r#"{{"js":"{js}","wasm":"{wasm}","build_ts":"{ts}","version":"{version}"}}"#),
    )?;
    Ok(())
}

fn placeholder_names(value: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut remainder = value;
    while let Some(start) = remainder.find('{') {
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('}') else {
            break;
        };
        let name = after_start[..end].trim();
        if !name.is_empty() {
            names.insert(name.to_string());
        }
        remainder = &after_start[end + 1..];
    }
    names
}

fn validate_web_node(path: &str, expected: &serde_json::Value, actual: &serde_json::Value) -> Result<()> {
    match (expected, actual) {
        (serde_json::Value::Object(expected), serde_json::Value::Object(actual)) => {
            for (key, expected_value) in expected {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                let actual_value = actual
                    .get(key)
                    .with_context(|| format!("web catalog missing key {child_path}"))?;
                validate_web_node(&child_path, expected_value, actual_value)?;
            }
            for key in actual.keys() {
                if !expected.contains_key(key) {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    bail!("web catalog has unknown key {child_path}");
                }
            }
            Ok(())
        }
        (serde_json::Value::String(expected), serde_json::Value::String(actual)) => {
            if actual.trim().is_empty() {
                bail!("web catalog has an empty value at {path}");
            }
            if placeholder_names(expected) != placeholder_names(actual) {
                bail!("web catalog placeholder mismatch at {path}");
            }
            Ok(())
        }
        _ => bail!("web catalog shape mismatch at {path}"),
    }
}

fn validate_web_catalogs() -> Result<serde_json::Value> {
    let english = serde_json::to_value(&sow_i18n::web(sow_i18n::Language::English))?;
    for &(language, code, _) in sow_i18n::Language::registry() {
        let catalog = serde_json::to_value(&sow_i18n::web(language))?;
        validate_web_node(code, &english, &catalog)
            .with_context(|| format!("validate web catalog {code}"))?;
    }
    Ok(english)
}

fn validate_current_ui_contract(paths: &Paths) -> Result<()> {
    let root = &paths.root;
    let catalog = validate_web_catalogs()?;
    let strings_root = root.join("sow-i18n/strings");
    let registered = sow_i18n::Language::registry()
        .iter()
        .map(|(_, code, _)| *code)
        .collect::<HashSet<_>>();
    let mut found = HashSet::new();
    for entry in fs::read_dir(&strings_root)
        .with_context(|| format!("read {}", strings_root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            bail!("web locale entry is not a directory: {}", entry.path().display());
        }
        let code = entry.file_name().to_string_lossy().into_owned();
        if !registered.contains(code.as_str()) {
            bail!("unregistered web locale directory: {code}");
        }
        let files = fs::read_dir(entry.path())?
            .map(|entry| {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    bail!("web locale entry is not a file: {}", entry.path().display());
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if name != "web.toml" {
                    bail!("unexpected web locale file: {}/{}", code, name);
                }
                Ok(name)
            })
            .collect::<Result<Vec<_>>>()?;
        if files.len() != 1 {
            bail!("web locale {code} must contain exactly one catalog");
        }
        found.insert(code);
    }
    for &(_, code, _) in sow_i18n::Language::registry() {
        if !found.contains(code) {
            bail!("registered web locale is missing: {code}");
        }
    }

    let source_root = root.join("sow-client/src");
    for entry in walkdir::WalkDir::new(&source_root) {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(path)
            .with_context(|| format!("read client source {}", path.display()))?;
        let mut offset = 0;
        while let Some(found) = source[offset..].find("UiText::new(") {
            let start = offset + found + "UiText::new(".len();
            let rest = &source[start..];
            let trimmed = rest.trim_start();
            let Some(quote) = trimmed.as_bytes().first().copied() else {
                bail!("unterminated UiText key in {}", path.display());
            };
            if quote != b'"' && quote != b'\'' {
                bail!("UiText::new requires a literal key in {}", path.display());
            }
            let value = &trimmed[1..];
            let end = value
                .find(quote as char)
                .with_context(|| format!("unterminated UiText key in {}", path.display()))?;
            let key = &value[..end];
            if web_catalog_value(&catalog, key).is_none() {
                bail!("client source uses unknown localization key {key}: {}", path.display());
            }
            let key_start = start + rest.len() - trimmed.len() + 1;
            offset = key_start + end;
        }
    }
    Ok(())
}

fn verify_exported_locales(dir: &Path) -> Result<()> {
    let expected = validate_web_catalogs()?;
    for &(_, code, _) in sow_i18n::Language::registry() {
        let path = dir.join("locales").join(code);
        if !path.is_file() {
            bail!("missing exported web catalog {code}");
        }
        let payload: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path)?)
            .with_context(|| format!("parse exported web catalog {code}"))?;
        if payload.get("schema").and_then(serde_json::Value::as_u64) != Some(1)
            || payload.get("version").and_then(serde_json::Value::as_u64)
                != Some(sow_i18n::WEB_CATALOG_VERSION as u64)
            || payload.get("locale").and_then(serde_json::Value::as_str) != Some(code)
        {
            bail!("exported web catalog metadata is invalid for {code}");
        }
        let strings = payload
            .get("strings")
            .with_context(|| format!("exported web catalog {code} has no strings"))?;
        validate_web_node(code, &expected, strings)?;
    }
    Ok(())
}

fn validate_campaign_assets(paths: &Paths) -> Result<()> {
    let dir = paths.root.join("assets/campaign");
    let mut rosters = HashMap::new();
    let mut triggers = HashMap::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if let Some(id) = name.strip_suffix(".triggers.json") {
            triggers.insert(id.to_string(), path);
        } else if let Some(id) = name.strip_suffix(".json") {
            rosters.insert(id.to_string(), path);
        }
    }
    let trigger_types = ["territory", "kills", "defeated", "contact", "attack", "troops", "building", "fleet", "nuke", "elapsed"];
    let action_types = ["show_dialog", "set_objective", "emote", "pause", "resume", "set_flag"];
    for episode_id in triggers.keys() {
        if !rosters.contains_key(episode_id) {
            bail!("campaign triggers have no roster: {episode_id}");
        }
    }
    for (episode_id, roster_path) in &rosters {
        let roster: serde_json::Value = serde_json::from_str(&fs::read_to_string(roster_path)?)
            .with_context(|| format!("parse {}", roster_path.display()))?;
        let factions = roster.get("factions").and_then(serde_json::Value::as_array)
            .filter(|factions| !factions.is_empty()).context("campaign roster has no factions")?;
        let names = factions.iter().map(|faction| faction.get("name").and_then(serde_json::Value::as_str).unwrap_or_default()).collect::<HashSet<_>>();
        if names.len() != factions.len() || names.iter().any(|name| name.is_empty()) {
            bail!("campaign roster has duplicate or empty faction names: {episode_id}");
        }
        let trigger_path = triggers.get(episode_id).with_context(|| format!("missing triggers for {episode_id}"))?;
        let definition: serde_json::Value = serde_json::from_str(&fs::read_to_string(trigger_path)?)
            .with_context(|| format!("parse {}", trigger_path.display()))?;
        let expected_episode_id = episode_id.strip_prefix("lady_").unwrap_or(episode_id);
        if definition.get("version").and_then(serde_json::Value::as_u64) != Some(1)
            || definition.get("episode_id").and_then(serde_json::Value::as_str) != Some(expected_episode_id) {
            bail!("campaign trigger header is invalid: {episode_id}");
        }
        let settings = definition.get("settings").context("campaign has no settings")?;
        if settings.get("buildings_enabled").and_then(serde_json::Value::as_bool).is_none()
            || !settings.get("starting_troops").and_then(serde_json::Value::as_f64).is_some_and(|value| value.is_finite() && (1.0..=100_000.0).contains(&value)) {
            bail!("campaign settings are invalid: {episode_id}");
        }
        let steps = definition.get("steps").and_then(serde_json::Value::as_array)
            .filter(|steps| !steps.is_empty()).context("campaign has no steps")?;
        let mut ids = HashSet::new();
        for step in steps {
            let id = step.get("id").and_then(serde_json::Value::as_str).context("campaign step has no id")?;
            if !ids.insert(id) { bail!("duplicate campaign step id: {id}"); }
            let trigger = step.get("trigger").context("campaign step has no trigger")?;
            let trigger_type = trigger.get("type").and_then(serde_json::Value::as_str).context("campaign trigger has no type")?;
            if !trigger_types.contains(&trigger_type) { bail!("unknown campaign trigger: {trigger_type}"); }
            if ["territory", "kills", "attack", "troops", "building", "fleet", "nuke", "elapsed"].contains(&trigger_type)
                && !trigger.get("value").and_then(serde_json::Value::as_f64).is_some_and(|value| value.is_finite()) {
                bail!("campaign trigger value is invalid: {id}");
            }
            for key in ["title_key", "body_key", "hint_key"] {
                if !step.get(key).and_then(serde_json::Value::as_str).is_some_and(|value| value.starts_with("tutorial.")) {
                    bail!("campaign translation key is invalid: {id}.{key}");
                }
            }
            for target in [trigger.get("target").and_then(serde_json::Value::as_str), step.get("marker").and_then(|marker| marker.get("target")).and_then(serde_json::Value::as_str)]
                .into_iter().flatten().filter(|target| *target != "player") {
                if !names.contains(target) { bail!("campaign references unknown faction: {target}"); }
            }
            for phase in ["on_enter", "on_complete"] {
                if let Some(actions) = step.get(phase).and_then(serde_json::Value::as_array) {
                    for action in actions {
                        if !action_types.contains(&action.get("type").and_then(serde_json::Value::as_str).unwrap_or_default()) {
                            bail!("unknown campaign action in {id}");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn export_locales(out: &Path) -> Result<()> {
    validate_web_catalogs()?;
    let d = out.join("locales");
    fs::create_dir_all(&d)?;
    for &(language, code, _) in sow_i18n::Language::registry() {
        let payload = serde_json::json!({
            "schema": 1,
            "version": sow_i18n::WEB_CATALOG_VERSION,
            "locale": code,
            "strings": sow_i18n::web(language),
        });
        fs::write(d.join(code), serde_json::to_string_pretty(&payload)?)?;
    }
    Ok(())
}

fn verify_layout(dir: &Path) -> Result<()> {
    verify_exported_locales(dir)?;
    let (mut wn, mut jn) = (None, None);
    for e in fs::read_dir(dir)? {
        let e = e?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with("_bg.wasm") && !n.ends_with(".br") {
            wn = Some(n.clone());
        }
        if n.starts_with("sow_client_") && n.ends_with(".js") && !n.ends_with(".br") {
            jn = Some(n);
        }
    }
    let w = wn.as_ref().context("missing _bg.wasm")?;
    let j = jn.as_ref().context("missing sow_client_*.js")?;
    if !dir.join(format!("{w}.br")).is_file() {
        bail!("missing {w}.br");
    }
    if !dir.join(format!("{j}.br")).is_file() {
        bail!("missing {j}.br");
    }
    // Webroot contract: marketing site at the root, game under play/.
    for required in [
        "index.html",
        "play/index.html",
        "robots.txt",
        "sitemap.xml",
        "app.js",
        "site-chrome.js",
        "styles.css",
        "legal.css",
        "fonts/fonts.css",
        "fonts/work-sans-latin.woff2",
        "fonts/work-sans-latin-ext.woff2",
        "fonts/work-sans-italic-latin.woff2",
        "fonts/work-sans-italic-latin-ext.woff2",
        "wou-auth.js",
        "privacy/index.html",
        "terms/index.html",
        "cookies/index.html",
        "support/index.html",
        "how-to-play/index.html",
        "leaders/index.html",
        "8d227b8f9e6140d39e3381a1829e1db3.txt",
        "sow.svg",
        "manifest.webmanifest",
        "icon-192.png",
        "icon-512.png",
        "icon-512-maskable.png",
        ".well-known/assetlinks.json",
    ] {
        if !dir.join(required).is_file() {
            bail!("webroot missing {}", required);
        }
    }
    if dir.join("admin").exists() {
        bail!("webroot must not contain admin/ (dashboard was removed)");
    }
    println!("✅ Dist layout OK ({})", dir.display());
    Ok(())
}

fn verify_cg_layout(dir: &Path) -> Result<()> {
    verify_exported_locales(dir)?;
    // Portal entry points are the UNCOMPRESSED pair (restored June design):
    // a native `import()` of a `.br` URL only works if the CDN serves it with
    // Content-Encoding: br + a JS MIME, which the CrazyGames CDN does not.
    for required in [
        "index.html",
        "sow_client.js",
        "sow_client_bg.wasm",
        "sow.svg",
        "loader.js",
        "sw.js",
        "game-manifest.json",
        "sdk/store_portals.js",
    ] {
        if !dir.join(required).is_file() {
            bail!("crazygames bundle missing {}", required);
        }
    }
    // The bundle is a whitelist; these must never ride along again.
    for forbidden in ["maps", "assets", "admin", "play"] {
        if dir.join(forbidden).exists() {
            bail!("crazygames bundle must not contain {forbidden}/");
        }
    }
    let html = fs::read_to_string(dir.join("index.html"))?;
    for needle in [
        "sdk.crazygames.com/crazygames-sdk-v3.js",
        "SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"",
        "SOW_ASSETS_URL = \"https://shadowsofwar.io/assets\"",
        "sow_client.js",
        "sow_client_bg.wasm",
    ] {
        if !html.contains(needle) {
            bail!("crazygames index.html missing: {}", needle);
        }
    }
    Ok(())
}

fn verify_poki_layout(dir: &Path) -> Result<()> {
    verify_exported_locales(dir)?;
    for required in [
        "index.html",
        "sow_client.js",
        "sow_client_bg.wasm",
        "sow.svg",
        "loader.js",
        "game-manifest.json",
        "manifest.webmanifest",
        "sdk/store_portals.js",
        "fonts/fonts.css",
        "fonts/work-sans-latin.woff2",
        "assets/shell/loader/loader_empty.webp",
        "assets/shell/loader/loader_full.webp",
        "maps/catalog.bin",
        "maps/world/map.bin.br",
    ] {
        if !dir.join(required).is_file() {
            bail!("poki bundle missing {required}");
        }
    }
    for forbidden in [
        "admin",
        "play",
        "site",
        ".well-known",
        "sw.js",
        "assets/gameplay/store",
        "assets/gameplay/skins",
        "assets/shell/mobile-nav/store.webp",
    ] {
        if dir.join(forbidden).exists() {
            bail!("poki bundle must not contain {forbidden}");
        }
    }
    let html = fs::read_to_string(dir.join("index.html"))?;
    let loader = fs::read_to_string(dir.join("loader.js"))?;
    let sdk = fs::read_to_string(dir.join("sdk/store_portals.js"))?;
    let manifest = fs::read_to_string(dir.join("manifest.webmanifest"))?;
    for needle in [
        "https://game-cdn.poki.com/scripts/v2/poki-sdk.js",
        "window.SOW_PORTAL = \"poki\"",
        "window.SOW_MAPS_URL = \"./maps\"",
        "window.SOW_ASSETS_URL = \"./assets\"",
        "window.SOW_DISABLE_CHAT = true",
        "sdk.init",
        "sdk.gameLoadingFinished",
        "sdk.gameplayStart",
        "sdk.gameplayStop",
        "sdk.commercialBreak",
        "sdk.measure",
        "sdk.openExternalLink",
    ] {
        if !html.contains(needle) && !sdk.contains(needle) {
            bail!("poki bundle missing {needle}");
        }
    }
    if html.matches("function selfCreds()").count() != 1 {
        bail!("poki bundle must define shared selfCreds exactly once");
    }
    for required in [
        "<canvas id=\"blade\"",
        "tabindex=\"0\"",
        "touch-action: none",
    ] {
        if !html.contains(required) {
            bail!("poki bundle missing responsive/focus contract {required}");
        }
    }
    if !manifest.contains("\"display\": \"fullscreen\"") {
        bail!("poki manifest missing responsive/focus contract \"display\": \"fullscreen\"");
    }
    if html.contains("<iframe") || html.contains("href=\"https://") {
        bail!("poki bundle must not own an iframe or direct external href");
    }
    if sdk.contains("rewardedBreak") {
        bail!("poki bridge must not request rewardedBreak without a product reward");
    }
    for forbidden in POKI_FORBIDDEN_MARKERS.iter().copied().chain([
        "SOW_MAPS_URL = \"https://",
        "SOW_ASSETS_URL = \"https://",
        "register('/sw.js'",
    ]) {
        if html.contains(forbidden) || loader.contains(forbidden) || sdk.contains(forbidden) {
            bail!("poki bundle contains forbidden content {forbidden}");
        }
    }
    if loader.contains("SOW_FIRST_PARTY_ANALYTICS_BEGIN") || loader.contains("/event") {
        bail!("Poki loader contains first-party analytics");
    }
    if html.contains("SOW_FIRST_PARTY_ANALYTICS_BEGIN") || html.contains("/event") {
        bail!("Poki index.html contains first-party analytics");
    }
    if html.matches("window.SOW_ENABLE_PORTAL_ADS = true;").count() != 1 {
        bail!("Poki index.html must contain exactly one portal boot block");
    }
    if html.contains("/* PORTAL_BOOT_SLOT */") {
        bail!("Poki index.html still contains the portal boot marker");
    }
    for entry in fs::read_dir(dir.join("maps"))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join("map.bin").exists() {
            bail!("Poki bundle contains an uncompressed map: {}", entry.path().display());
        }
    }
    if dir.join("sdk/poki_portals.js").exists() {
        bail!("Poki bundle contains the source-only Poki bridge");
    }
    println!("✅ Poki layout OK ({})", dir.display());
    Ok(())
}

fn package_self(paths: &Paths, out: &Path, version: &str, compile: bool) -> Result<()> {
    validate_campaign_assets(paths)?;
    let previous_artifacts = if !compile && out.is_dir() {
        let js = fs::read_dir(out)?.filter_map(Result::ok).find_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (name.starts_with("sow_client_") && name.ends_with(".js") && !name.ends_with(".br"))
                .then_some((name, fs::read(entry.path()).ok()?))
        });
        let wasm = fs::read_dir(out)?.filter_map(Result::ok).find_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (name.starts_with("sow_client_") && name.ends_with("_bg.wasm") && !name.ends_with(".br"))
                .then_some((name, fs::read(entry.path()).ok()?))
        });
        js.zip(wasm).map(|(js, wasm)| {
            let js_br = fs::read(out.join(format!("{}.br", js.0))).ok();
            let wasm_br = fs::read(out.join(format!("{}.br", wasm.0))).ok();
            (js, wasm, js_br, wasm_br)
        })
    } else {
        None
    };
    if out.exists() {
        for e in fs::read_dir(out)? {
            let e = e?;
            let p = e.path();
            if p.is_dir() {
                fs::remove_dir_all(&p)?;
            } else {
                fs::remove_file(&p)?;
            }
        }
    } else {
        fs::create_dir_all(out)?;
    }

    let ts = prod::web_fingerprint(paths, version)?[..10].to_string();
    let (js, wasm) = previous_artifacts
        .as_ref()
        .map(|(js, wasm, _, _)| (js.0.clone(), wasm.0.clone()))
        .unwrap_or_else(|| (format!("sow_client_{ts}.js"), format!("sow_client_{ts}_bg.wasm")));

    let assets = out.join("assets");
    copy_dir(&paths.assets_shell, &assets.join("shell"))?;
    copy_dir(
        &paths.assets_gameplay.join("avatars"),
        &assets.join("gameplay/avatars"),
    )?;
    copy_dir(
        &paths.assets_gameplay.join("skins"),
        &assets.join("gameplay/skins"),
    )?;
    copy_dir(
        &paths.assets_gameplay.join("currency"),
        &assets.join("gameplay/currency"),
    )?;
    copy_dir(
        &paths.assets_gameplay.join("store"),
        &assets.join("gameplay/store"),
    )?;
    copy_dir(
        &paths.root.join("assets/campaign"),
        &assets.join("campaign"),
    )?;
    copy_dir(&paths.assets_site.join("media"), &assets.join("site/media"))?;
    let maps = out.join("maps");
    fs::create_dir_all(&maps)?;
    if paths.assets_maps.is_dir() {
        copy_dir(&paths.assets_maps, &maps)?;
    }
    refresh_map_thumbnails(&maps, &paths.map_sources)?;
    let maps_cache_bust = thumbnail_cache_bust(&maps)?;

    if let Some((previous_js, previous_wasm, previous_js_br, previous_wasm_br)) = previous_artifacts {
        let js_name = previous_js.0;
        let wasm_name = previous_wasm.0;
        fs::write(out.join(&js_name), previous_js.1)?;
        fs::write(out.join(&wasm_name), previous_wasm.1)?;
        if let Some(bytes) = previous_js_br {
            fs::write(out.join(format!("{js_name}.br")), bytes)?;
        }
        if let Some(bytes) = previous_wasm_br {
            fs::write(out.join(format!("{wasm_name}.br")), bytes)?;
        }
    } else {
        run_bindgen(&paths.wasm_input, out, &format!("sow_client_{ts}"))?;
    }
    copy_shell(paths, out)?;
    build_index(
        paths,
        out,
        IndexBuild {
            version,
            js: &js,
            wasm: &wasm,
            ts: &ts,
            maps_cache_bust: &maps_cache_bust,
            target: WebTarget::Local,
        },
    )?;
    export_locales(out)?;

    if compile {
        minify_js(&out.join(&js))?;
        run_wasm_opt(&out.join(&wasm), &paths.wasm_cache)?;
        brotli_file(&out.join(&wasm))?;
        brotli_file(&out.join(&js))?;
    } else {
        if !brotli_dst(&out.join(&wasm)).is_file() {
            brotli_file(&out.join(&wasm))?;
        }
        if !brotli_dst(&out.join(&js)).is_file() {
            brotli_file(&out.join(&js))?;
        }
    }
    write_sw(out, version, &js, &wasm, &ts)?;
    write_manifest(out, version, &js, &wasm, &ts)?;

    // Marketing website at the webroot root (game shell lives under play/).
    let site = paths.root.join("sow-web/site");
    for name in [
        "index.html",
        "app.js",
        "site-chrome.js",
        "styles.css",
        "legal.css",
        "wou-auth.js",
        "8d227b8f9e6140d39e3381a1829e1db3.txt",
        "manifest.webmanifest",
    ] {
        let src = site.join(name);
        if !src.is_file() {
            bail!("website source missing: {}", src.display());
        }
        fs::copy(&src, out.join(name))?;
    }
    for name in ["icon-192.png", "icon-512.png", "icon-512-maskable.png"] {
        fs::copy(paths.assets_site.join("icons").join(name), out.join(name))?;
    }
    for path in [
        "privacy",
        "terms",
        "cookies",
        "support",
        "how-to-play",
        "leaders",
        "auth",
        "fonts",
        ".well-known",
    ] {
        let src = site.join(path);
        if !src.is_dir() {
            bail!("website legal page missing: {}", src.display());
        }
        copy_dir(&src, &out.join(path))?;
    }
    // Fingerprint site assets (styles/app/site-chrome/legal/wou-auth) with a content hash so edge and
    // browser caches never serve a stale version after a redeploy.
    for name in [
        "styles.css",
        "app.js",
        "site-chrome.js",
        "legal.css",
        "wou-auth.js",
    ] {
        let hash = file_sha256(&out.join(name))?;
        let versioned = format!("{name}?v={}", &hash[..10]);
        let html = out.join("index.html");
        let content = fs::read_to_string(&html)?;
        fs::write(
            &html,
            content.replace(&format!("./{name}"), &format!("./{versioned}")),
        )?;
    }
    fs::write(
        out.join("robots.txt"),
        "User-agent: *\nAllow: /\nDisallow: /internal/\nDisallow: /api/\nDisallow: /*.wasm$\nDisallow: /*.wasm.br$\n\nSitemap: https://shadowsofwar.io/sitemap.xml\n",
    )?;
    fs::write(
        out.join("sitemap.xml"),
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
            "  <url>\n    <loc>https://shadowsofwar.io/</loc>\n    <lastmod>2026-08-25</lastmod>\n    <changefreq>weekly</changefreq>\n    <priority>1.0</priority>\n  </url>\n",
            "  <url>\n    <loc>https://shadowsofwar.io/play/</loc>\n    <lastmod>2026-08-25</lastmod>\n    <changefreq>weekly</changefreq>\n    <priority>0.9</priority>\n  </url>\n",
            "  <url>\n    <loc>https://shadowsofwar.io/leaders/</loc>\n    <lastmod>2026-08-25</lastmod>\n    <changefreq>monthly</changefreq>\n    <priority>0.8</priority>\n  </url>\n",
            "  <url>\n    <loc>https://shadowsofwar.io/how-to-play/</loc>\n    <lastmod>2026-08-25</lastmod>\n    <changefreq>monthly</changefreq>\n    <priority>0.8</priority>\n  </url>\n",
            "  <url>\n    <loc>https://shadowsofwar.io/support/</loc>\n    <lastmod>2026-09-03</lastmod>\n    <changefreq>monthly</changefreq>\n    <priority>0.4</priority>\n  </url>\n",
            "  <url>\n    <loc>https://shadowsofwar.io/privacy/</loc>\n    <lastmod>2026-09-03</lastmod>\n    <changefreq>yearly</changefreq>\n    <priority>0.3</priority>\n  </url>\n",
            "  <url>\n    <loc>https://shadowsofwar.io/terms/</loc>\n    <lastmod>2026-09-03</lastmod>\n    <changefreq>yearly</changefreq>\n    <priority>0.3</priority>\n  </url>\n",
            "  <url>\n    <loc>https://shadowsofwar.io/cookies/</loc>\n    <lastmod>2026-09-03</lastmod>\n    <changefreq>yearly</changefreq>\n    <priority>0.3</priority>\n  </url>\n",
            "</urlset>\n",
        ),
    )?;
    println!("✅ Website staged at webroot root");

    prune_qs(out)?;
    verify_layout(out)?;
    println!("Load paths: {wasm}, {js}");
    Ok(())
}

fn prepare_native_webroot(out: &Path) -> Result<()> {
    let index = out.join("play/index.html");
    let html = fs::read_to_string(&index)
        .with_context(|| format!("read native entrypoint {}", index.display()))?;
    let maps = "window.SOW_MAPS_URL = \"/maps\";";
    let assets = "window.SOW_ASSETS_URL = \"/assets\";";
    if html.matches(maps).count() != 1 || html.matches(assets).count() != 1 {
        bail!("native entrypoint endpoints are missing or duplicated");
    }
    let html = html
        .replace(maps, "window.SOW_MAPS_URL = \"../maps\";")
        .replace(assets, "window.SOW_NATIVE = true; window.SOW_ASSETS_URL = \"../assets\";");
    fs::write(&index, html)?;
    Ok(())
}

fn package_cg(
    play_dir: &Path,
    out: &Path,
    paths: &Paths,
    version: &str,
    maps_cache_bust: &str,
) -> Result<()> {
    // The portal bundle is a strict whitelist: index.html, the brotli client
    // pair, and the shell essentials. Everything else (maps, assets, admin)
    // streams from the production CDN at runtime. Never clone dist/web here.
    let (mut jh, mut wh) = (String::new(), String::new());
    for e in fs::read_dir(play_dir)? {
        let n = e?.file_name().to_string_lossy().into_owned();
        if n.starts_with("sow_client_") && n.ends_with(".js") && !n.ends_with(".br") {
            jh = n
                .trim_start_matches("sow_client_")
                .trim_end_matches(".js")
                .to_string();
        }
        if n.ends_with("_bg.wasm") && !n.ends_with(".br") {
            wh = n
                .trim_end_matches("_bg.wasm")
                .trim_start_matches("sow_client_")
                .to_string();
        }
    }
    if jh.is_empty() || wh.is_empty() {
        bail!("CrazyGames package is missing hashed client artifacts");
    }

    if out.exists() {
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)?;

    // Portal entry points are the UNCOMPRESSED pair (restored June design):
    // CG's CDN does not serve `.br` as an importable module. The .js/.wasm
    // plain files load natively via import()/fetch on any static host.
    fs::copy(
        play_dir.join(format!("sow_client_{jh}.js")),
        out.join("sow_client.js"),
    )?;
    fs::copy(
        play_dir.join(format!("sow_client_{wh}_bg.wasm")),
        out.join("sow_client_bg.wasm"),
    )?;
    copy_shell(paths, out)?;
    export_locales(out)?;
    write_sw(out, version, "sow_client.js", "sow_client_bg.wasm", &jh)?;
    fs::write(
        out.join("game-manifest.json"),
        format!(
            r#"{{"js":"sow_client.js","wasm":"sow_client_bg.wasm","build_ts":"{jh}","version":"{version}"}}"#
        ),
    )?;
    build_index(
        paths,
        out,
        IndexBuild {
            version,
            js: &format!("sow_client_{jh}.js"),
            wasm: &format!("sow_client_{wh}_bg.wasm"),
            ts: &jh,
            maps_cache_bust,
            target: WebTarget::CrazyGames,
        },
    )?;

    // Patch index.html: hashed names -> stable PLAIN names, inject portal SDK
    // and boot overrides (maps AND assets resolve against the prod CDN).
    let idx = out.join("index.html");
    let html = fs::read_to_string(&idx)?;
    let mut lines: Vec<String> = html.lines().map(String::from).collect();
    let (mut sdk, mut boot) = (false, false);
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") {
            *line = "    <script src=\"https://sdk.crazygames.com/crazygames-sdk-v3.js\"></script>"
                .to_string();
            sdk = true;
        } else if line.contains("/* PORTAL_BOOT_SLOT */") {
            *line = "        window.SOW_ENABLE_PORTAL_ADS = true; window.SOW_PORTAL = \"crazygames\"; window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; window.SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"; window.SOW_ASSETS_URL = \"https://shadowsofwar.io/assets\"; window.SOW_DATABASE_URL = \"https://shadowsofwar.io/api\";".to_string();
            boot = true;
        }
    }
    if !sdk || !boot {
        bail!("CrazyGames index.html is missing portal slots (sdk={sdk} boot={boot})");
    }
    let html_out = lines
        .join("\n")
        .replace(&format!("sow_client_{jh}.js"), "sow_client.js")
        .replace(&format!("sow_client_{wh}_bg.wasm"), "sow_client_bg.wasm");
    fs::write(&idx, html_out)?;

    verify_cg_layout(out)?;
    println!("✅ CrazyGames bundle ready (whitelist): {}", out.display());
    Ok(())
}

fn package_poki(
    play_dir: &Path,
    out: &Path,
    paths: &Paths,
    version: &str,
    maps_cache_bust: &str,
) -> Result<()> {
    let (mut jh, mut wh) = (String::new(), String::new());
    for e in fs::read_dir(play_dir)? {
        let n = e?.file_name().to_string_lossy().into_owned();
        if n.starts_with("sow_client_") && n.ends_with(".js") && !n.ends_with(".br") {
            jh = n
                .trim_start_matches("sow_client_")
                .trim_end_matches(".js")
                .to_string();
        }
        if n.ends_with("_bg.wasm") && !n.ends_with(".br") {
            wh = n
                .trim_end_matches("_bg.wasm")
                .trim_start_matches("sow_client_")
                .to_string();
        }
    }
    if jh.is_empty() || wh.is_empty() {
        bail!("Poki package is missing hashed client artifacts");
    }

    if out.exists() {
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)?;
    fs::copy(
        play_dir.join(format!("sow_client_{jh}.js")),
        out.join("sow_client.js"),
    )?;
    fs::copy(
        play_dir.join(format!("sow_client_{wh}_bg.wasm")),
        out.join("sow_client_bg.wasm"),
    )?;

    copy_poki_assets(&play_dir.join("assets"), &out.join("assets"))?;
    copy_poki_maps(&play_dir.join("maps"), &out.join("maps"))?;
    copy_dir(
        &paths.root.join("sow-web/site/fonts"),
        &out.join("fonts"),
    )?;
    copy_shell(paths, out)?;
    optimize_poki_thumbnail(
        &paths.assets_shell.join("brand/app-icon.png"),
        &out
            .parent()
            .context("Poki output directory has no parent")?
            .join("poki-thumbnail.png"),
    )?;
    fs::write(
        out.join("manifest.webmanifest"),
        r##"{
  "name": "Shadows of War",
  "short_name": "Shadows of War",
  "start_url": "./",
  "scope": "./",
  "display": "fullscreen",
  "orientation": "landscape",
  "background_color": "#0a0a0f",
  "theme_color": "#0a0a0f",
  "icons": [{"src": "sow.svg", "sizes": "any", "type": "image/svg+xml", "purpose": "any"}]
}
"##,
    )?;
    let loader_path = out.join("loader.js");
    let loader = fs::read_to_string(&loader_path)?;
    fs::write(
        &loader_path,
        strip_marked_section(
            &loader,
            "/* SOW_FIRST_PARTY_ANALYTICS_BEGIN */",
            "/* SOW_FIRST_PARTY_ANALYTICS_END */",
        )?,
    )?;
    fs::copy(
        paths.shell.join("sdk/poki_portals.js"),
        out.join("sdk/store_portals.js"),
    )?;
    let poki_source = out.join("sdk/poki_portals.js");
    if poki_source.is_file() {
        fs::remove_file(poki_source)?;
    }
    export_locales(out)?;
    write_manifest(out, version, "sow_client.js", "sow_client_bg.wasm", &jh)?;
    build_index(
        paths,
        out,
        IndexBuild {
            version,
            js: "sow_client.js",
            wasm: "sow_client_bg.wasm",
            ts: &jh,
            maps_cache_bust,
            target: WebTarget::Poki,
        },
    )?;

    let index = out.join("index.html");
    let html = fs::read_to_string(&index)?;
    let mut lines = Vec::new();
    let mut sdk = false;
    let mut boot = false;
    for line in html.lines() {
        if line.contains("PORTAL_SDK_SLOT") {
            lines.push(
                "    <script src=\"https://game-cdn.poki.com/scripts/v2/poki-sdk.js\"></script>"
                    .to_string(),
            );
            sdk = true;
        } else if line.trim() == "/* PORTAL_BOOT_SLOT */" {
            lines.push(concat!(
                "        window.SOW_ENABLE_PORTAL_ADS = true; ",
                "window.SOW_PORTAL = \"poki\"; ",
                "window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; ",
                "window.SOW_MAPS_URL = \"./maps\"; ",
                "window.SOW_ASSETS_URL = \"./assets\"; ",
                "window.SOW_DATABASE_URL = \"https://shadowsofwar.io/api\"; ",
                "window.SOW_DISABLE_CHAT = true;"
            ).to_string());
            boot = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !sdk || !boot {
        bail!("Poki index.html is missing portal slots (sdk={sdk} boot={boot})");
    }
    fs::write(&index, lines.join("\n") + "\n")?;
    verify_poki_layout(out)?;
    println!("✅ Poki bundle ready (self-contained): {}", out.display());
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalFileStamp {
    path: PathBuf,
    len: u64,
    modified_nanos: u128,
}

struct LocalWatchLock {
    path: PathBuf,
    pid: u32,
}

impl Drop for LocalWatchLock {
    fn drop(&mut self) {
        let owned = fs::read_to_string(&self.path)
            .ok()
            .is_some_and(|contents| contents.trim() == self.pid.to_string());
        if owned {
            let _ = fs::remove_file(&self.path);
        }
    }
}

struct LocalPreviewServer {
    child: Child,
}

impl Drop for LocalPreviewServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

const LOCAL_WASM_CRATES: &[&str] = &[
    "sow-audio",
    "sow-client",
    "sow-core",
    "sow-data",
    "sow-i18n",
    "sow-net",
    "sow-render",
];

fn local_watch_roots(paths: &Paths) -> Vec<PathBuf> {
    vec![
        paths.root.join("Cargo.toml"),
        paths.root.join("Cargo.lock"),
        paths.root.join(".version"),
        paths.root.join("sow-audio"),
        paths.root.join("sow-client"),
        paths.root.join("sow-core"),
        paths.root.join("sow-data"),
        paths.root.join("sow-i18n"),
        paths.root.join("sow-net"),
        paths.root.join("sow-render"),
        paths.root.join("sow-web"),
        paths.root.join("assets"),
    ]
}

fn local_source_snapshot(paths: &Paths) -> Result<Vec<LocalFileStamp>> {
    let mut files = Vec::new();
    for root in local_watch_roots(paths) {
        if root.is_file() {
            let metadata = fs::metadata(&root)?;
            files.push(LocalFileStamp {
                path: root,
                len: metadata.len(),
                modified_nanos: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_nanos()),
            });
            continue;
        }
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| {
                !matches!(
                    entry.file_name().to_str(),
                    Some(".git" | "target" | "dist" | "node_modules")
                )
            })
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error)
                    if error
                        .io_error()
                        .map_or(false, |io_error| io_error.kind() == std::io::ErrorKind::NotFound) =>
                {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            files.push(LocalFileStamp {
                path: entry.path().to_path_buf(),
                len: metadata.len(),
                modified_nanos: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_nanos()),
            });
        }
    }
    files.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn local_file_requires_wasm(paths: &Paths, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(&paths.root) else {
        return false;
    };
    let Some(crate_name) = relative.components().next() else {
        return false;
    };
    let crate_name = crate_name.as_os_str().to_string_lossy();
    let is_manifest = relative.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
        matches!(name, "Cargo.toml" | "Cargo.lock" | "build.rs")
    });
    is_manifest
        || (LOCAL_WASM_CRATES.iter().any(|name| crate_name == *name)
            && relative.extension().and_then(|extension| extension.to_str()) == Some("rs"))
}

fn local_change_requires_wasm(
    previous: &[LocalFileStamp],
    current: &[LocalFileStamp],
    paths: &Paths,
) -> bool {
    let mut previous_by_path = previous
        .iter()
        .map(|stamp| (&stamp.path, (stamp.len, stamp.modified_nanos)))
        .collect::<HashMap<_, _>>();
    for stamp in current {
        let unchanged = previous_by_path
            .remove(&stamp.path)
            .is_some_and(|old| old == (stamp.len, stamp.modified_nanos));
        if !unchanged && local_file_requires_wasm(paths, &stamp.path) {
            return true;
        }
    }
    previous_by_path
        .keys()
        .any(|path| local_file_requires_wasm(paths, path))
}

fn local_watch_process(paths: &Paths, pid: u32) -> bool {
    if pid == std::process::id() {
        return false;
    }
    let proc_dir = Path::new("/proc").join(pid.to_string());
    let Ok(cwd) = fs::canonicalize(proc_dir.join("cwd")) else {
        return false;
    };
    if cwd != paths.root {
        return false;
    }
    let Ok(cmdline) = fs::read(proc_dir.join("cmdline")) else {
        return false;
    };
    let mut args = cmdline.split(|byte| *byte == 0).filter(|arg| !arg.is_empty());
    let Some(executable) = args.next() else {
        return false;
    };
    let executable = String::from_utf8_lossy(executable);
    let command = Path::new(executable.as_ref())
        .file_name()
        .and_then(|name| name.to_str());
    let subcommand = args
        .next()
        .map(|arg| String::from_utf8_lossy(arg).into_owned());
    command == Some("sow") && matches!(subcommand.as_deref(), None | Some("l") | Some("local"))
}

fn local_process_group(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat")).ok()?;
    let (_, fields) = stat.rsplit_once(") ")?;
    fields.split_whitespace().nth(2)?.parse().ok()
}

fn stop_local_process(pid: u32) -> Result<()> {
    if pid == std::process::id() {
        bail!("refusing to stop the current local preview process");
    }
    let pid_arg = pid.to_string();
    let status = if local_process_group(pid) == Some(pid) {
        let group_arg = format!("-{pid}");
        Command::new("kill")
            .args(["-KILL", "--", group_arg.as_str()])
            .status()
    } else {
        Command::new("kill")
            .args(["-KILL", pid_arg.as_str()])
            .status()
    }
    .with_context(|| format!("stop local preview process {pid}"))?;
    if !status.success() && Path::new("/proc").join(pid.to_string()).exists() {
        bail!("could not stop local preview process {pid}");
    }
    Ok(())
}

fn stop_local_port(port: u16) -> Result<()> {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    if TcpListener::bind(address).is_ok() {
        return Ok(());
    }
    let socket = format!("{port}/tcp");
    let status = Command::new("fuser")
        .args(["-k", "-KILL", &socket])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("free the local preview port (fuser is required when it is occupied)")?;
    if !status.success() && status.code() != Some(1) {
        bail!("could not free local preview port {port}");
    }
    if TcpListener::bind(address).is_err() {
        bail!("local preview port {port} is still occupied");
    }
    Ok(())
}

fn acquire_local_watch_lock(paths: &Paths, port: u16) -> Result<LocalWatchLock> {
    let dir = paths.root.join("dist/.sow-state");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("local-{port}.lock"));
    loop {
        if path.exists() {
            let pid = fs::read_to_string(&path)
                .ok()
                .and_then(|value| value.trim().parse::<u32>().ok());
            if let Some(pid) = pid.filter(|pid| local_watch_process(paths, *pid)) {
                stop_local_process(pid)?;
            }
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("remove previous local preview lock {}", path.display())
                    });
                }
            }
        }
        stop_local_port(port)?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let pid = std::process::id();
                writeln!(file, "{pid}")?;
                file.sync_all()?;
                return Ok(LocalWatchLock { path, pid });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create local preview lock {}", path.display()));
            }
        }
    }
}

fn start_local_server(port: u16, webroot: &str) -> Result<LocalPreviewServer> {
    stop_local_port(port)?;
    let port_arg = port.to_string();
    let child = Command::new("python3")
        .args([
            "-m",
            "http.server",
            &port_arg,
            "--bind",
            "127.0.0.1",
            "--directory",
            webroot,
        ])
        .spawn()
        .context("start local web server (python3 is required)")?;
    Ok(LocalPreviewServer {
        child,
    })
}

fn build_local_preview(paths: &Paths, version: &str, compile: bool) -> Result<()> {
    if compile {
        compile_wasm(paths, false)?;
    }
    package_self(paths, &paths.dist_web, version, compile)?;
    let maps_cache_bust = thumbnail_cache_bust(&paths.dist_web.join("maps"))?;
    package_poki(
        &paths.dist_web,
        &paths.dist_poki,
        paths,
        version,
        &maps_cache_bust,
    )?;
    Ok(())
}

fn read_version(paths: &Paths) -> Result<String> {
    let version_path = paths.root.join(".version");
    let version = fs::read_to_string(&version_path)
        .with_context(|| format!("read {}", version_path.display()))?
        .trim()
        .to_string();
    if version.is_empty() {
        bail!(".version must not be empty");
    }
    Ok(version)
}

fn ensure_native_dependencies(native_root: &Path) -> Result<()> {
    require_file(&native_root.join("package.json"), "native package.json")?;
    require_file(&native_root.join("package-lock.json"), "native package-lock.json")?;
    let cli = native_root.join(if cfg!(windows) {
        "node_modules/.bin/tauri.cmd"
    } else {
        "node_modules/.bin/tauri"
    });
    if !cli.is_file() {
        run("npm", &["ci", "--include=dev"], Some(native_root))?;
    }
    require_file(&cli, "Tauri CLI")?;
    Ok(())
}

fn build_native(paths: &Paths, native_root: &Path, version: &str) -> Result<()> {
    let config = serde_json::json!({ "version": version }).to_string();
    let bundles = if cfg!(target_os = "linux") {
        "deb"
    } else if cfg!(target_os = "macos") {
        "app,dmg"
    } else {
        "nsis"
    };
    println!("+ npm run tauri -- build --no-sign --bundles {bundles} --config {config}");
    let status = Command::new("npm")
        .args([
            "run",
            "tauri",
            "--",
            "build",
            "--no-sign",
            "--bundles",
            bundles,
            "--config",
            config.as_str(),
        ])
        .current_dir(native_root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("build native desktop bundle (npm is required)")?;
    if !status.success() {
        bail!("Tauri native build failed");
    }
    launch_native(paths)
}

fn launch_native(paths: &Paths) -> Result<()> {
    let target = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.root.join("target"));
    if cfg!(target_os = "macos") {
        let app = target.join("release/bundle/macos/Shadows of War.app");
        if !app.is_dir() {
            bail!("native macOS app missing: {}", app.display());
        }
        run(
            "open",
            &[app.to_str().context("native app path is not UTF-8")?],
            None,
        )?;
    } else {
        let name = if cfg!(windows) {
            "sow-native.exe"
        } else {
            "sow-native"
        };
        let executable = target.join("release").join(name);
        require_file(&executable, "native executable")?;
        println!("✅ Native game ready: {}", executable.display());
        let mut command = Command::new(&executable);
        command.current_dir(&paths.root);
        if cfg!(target_os = "linux")
            && env::var_os("WEBKIT_DMABUF_RENDERER_FORCE_SHM").is_none()
            && Path::new("/sys/module/nvidia").is_dir()
        {
            command.env("WEBKIT_DMABUF_RENDERER_FORCE_SHM", "1");
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("open native game {}", executable.display()))?;
        child.wait().context("wait for native game")?;
    }
    Ok(())
}

fn cmd_native(paths: &Paths) -> Result<()> {
    let version = read_version(paths)?;
    println!("==> Building native JavaScript/WASM game");
    compile_wasm(paths, false)?;
    package_self(paths, &paths.dist_web, &version, true)?;
    prepare_native_webroot(&paths.dist_web)?;
    let native_root = paths.root.join("sow-native");
    ensure_native_dependencies(&native_root)?;
    build_native(paths, &native_root, &version)
}

fn watch_local_preview(
    paths: &Paths,
    version: &str,
    port: u16,
    webroot: &str,
    _lock: LocalWatchLock,
) -> Result<()> {
    let mut server = start_local_server(port, webroot)?;
    println!("✅ Local webroot ready at http://127.0.0.1:{port}/");
    println!("   Backend: https://shadowsofwar.io (Ctrl-C to stop)");
    println!("   Watching source files; refresh the browser after a rebuild.");
    let mut previous = local_source_snapshot(paths)?;
    loop {
        if let Some(status) = server.child.try_wait()? {
            bail!("local web server stopped unexpectedly: {status}");
        }
        thread::sleep(Duration::from_millis(250));
        let current = local_source_snapshot(paths)?;
        if current == previous {
            continue;
        }
        let compile = local_change_requires_wasm(&previous, &current, paths);
        previous = current;
        println!(
            "==> Local source change detected; rebuilding{}...",
            if compile { " WASM" } else { " web files" }
        );
        if let Err(error) = build_local_preview(paths, version, compile) {
            eprintln!("⚠️ Local preview rebuild failed: {error:#}");
        } else {
            println!("✅ Local preview rebuilt; refresh the browser.");
        }
    }
}

fn cmd_local(paths: &Paths) -> Result<()> {
    let version = read_version(paths)?;

    let port = env::var("SOW_LOCAL_PORT").unwrap_or_else(|_| "4173".to_string());
    let port_number = port
        .parse::<u16>()
        .with_context(|| format!("SOW_LOCAL_PORT must be a valid port: {port}"))?;
    if port_number == 0 {
        bail!("SOW_LOCAL_PORT must not be 0");
    }
    let webroot = paths
        .dist_web
        .to_str()
        .context("local webroot path is not UTF-8")?;
    let lock = acquire_local_watch_lock(paths, port_number)?;
    println!("==> Building local web preview");
    // Local preview must package the client from the current source tree. Reusing a stale
    // wasm artifact makes UI/WASM work appear successful while the browser is running old code.
    build_local_preview(paths, &version, true)?;
    watch_local_preview(paths, &version, port_number, webroot, lock)
}

fn load_dotenv(path: &Path) {
    if path.is_file()
        && let Ok(c) = fs::read_to_string(path)
    {
        for line in c.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(eq) = line.find('=') {
                unsafe {
                    env::set_var(line[..eq].trim(), line[eq + 1..].trim().trim_matches('"'));
                }
            }
        }
    }
}

/// Create the machine-to-machine relay control secret once, locally, when
/// the ignored deployment environment does not have one yet.  The value is
/// persisted only in sow-dist/.env (mode 0600) and is staged to both ends by
/// ./sow p; it is never included in a command line or pipeline output.
fn ensure_generated_secret(root: &Path, key: &str) -> Result<()> {
    if env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        return Ok(());
    }

    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let value = hex::encode(bytes);
    let path = root.join("sow-dist/.env");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {} for generated secret", path.display()))?;
    file.write_all(format!("\n{key}={value}\n").as_bytes())?;
    file.sync_all()?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    unsafe { env::set_var(key, value) };
    println!("✅ generated and persisted {key} in ignored sow-dist/.env");
    Ok(())
}

fn shared_identity_secret_path() -> Result<PathBuf> {
    env::var_os("WOU_SOW_IDENTITY_SECRET_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".config/shadows-of-war/wou_sow_identity_secret"))
        })
        .ok_or_else(|| anyhow::anyhow!("cannot determine the shared WOU-ID secret path"))
}

fn ensure_shared_identity_secret() -> Result<()> {
    let path = shared_identity_secret_path()?;
    let configured = env::var("WOU_SOW_IDENTITY_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let stored = if path.is_file() {
        Some(
            fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?
                .trim()
                .to_string(),
        )
    } else {
        None
    };
    if let (Some(configured), Some(stored)) = (&configured, &stored)
        && configured != stored
    {
        bail!(
            "WOU_SOW_IDENTITY_SECRET differs from {}",
            path.display()
        );
    }
    let value = stored.or(configured).unwrap_or_else(|| {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    });
    if value.len() < 32 || value.len() > 256 || value.chars().any(char::is_whitespace) {
        bail!("WOU_SOW_IDENTITY_SECRET must be 32-256 non-whitespace characters");
    }
    if !path.is_file() {
        let parent = path
            .parent()
            .context("shared WOU-ID secret path has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("create {}", path.display()))?;
        file.write_all(value.as_bytes())?;
        file.sync_all()?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    unsafe { env::set_var("WOU_SOW_IDENTITY_SECRET", value) };
    Ok(())
}

/// Rotate both deployment credentials in the ignored local environment.
/// Values never enter command-line arguments or stdout; the caller must run
/// the normal production pipeline immediately afterwards.
fn rotate_deployment_secrets(root: &Path) -> Result<()> {
    let path = root.join("sow-dist/.env");
    let input = fs::read_to_string(&path)
        .with_context(|| format!("read {} for secret rotation", path.display()))?;
    let mut output = String::with_capacity(input.len() + 160);
    let mut replaced = [false; 2];
    for line in input.lines() {
        let key = line.split_once('=').map(|(key, _)| key.trim());
        let slot = match key {
            Some("SOW_DB_SECRET") => Some(0),
            Some("SOW_RELAY_CONTROL_SECRET") => Some(1),
            _ => None,
        };
        if let Some(slot) = slot {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let name = if slot == 0 {
                "SOW_DB_SECRET"
            } else {
                "SOW_RELAY_CONTROL_SECRET"
            };
            output.push_str(name);
            output.push('=');
            output.push_str(&hex::encode(bytes));
            output.push('\n');
            replaced[slot] = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    for (slot, present) in replaced.iter().enumerate() {
        if !present {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let name = if slot == 0 {
                "SOW_DB_SECRET"
            } else {
                "SOW_RELAY_CONTROL_SECRET"
            };
            output.push_str(name);
            output.push('=');
            output.push_str(&hex::encode(bytes));
            output.push('\n');
        }
    }
    let parent = path
        .parent()
        .context("secret environment has no parent directory")?;
    let mut temp =
        tempfile::NamedTempFile::new_in(parent).context("create temporary secret environment")?;
    temp.write_all(output.as_bytes())?;
    temp.as_file().sync_all()?;
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o600))?;
    temp.persist(&path)
        .map_err(|e| anyhow::anyhow!("persist rotated secret environment: {}", e.error))?;
    println!("✅ rotated deployment secrets locally (values redacted)");
    Ok(())
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()?;
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--rotate-secrets") {
        rotate_deployment_secrets(&root)?;
    }
    load_dotenv(&root.join("sow-dist/.env"));
    ensure_generated_secret(&root, "SOW_DB_SECRET")?;
    ensure_generated_secret(&root, "SOW_RELAY_CONTROL_SECRET")?;
    ensure_generated_secret(&root, "SOW_REVENUECAT_WEBHOOK_SECRET")?;

    let paths = Paths::discover()?;
    let mut cmd = String::new();
    let mut bump = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--version" => bump = true,
            "--rotate-secrets" => {}
            _ if cmd.is_empty() => cmd = args[i].clone(),
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }

    match cmd.as_str() {
        "p" | "prod" => prod::execute(&paths, bump),
        "a" | "android" => prod::execute_android(&paths),
        "native" => cmd_native(&paths),
        "local" | "l" | "" => cmd_local(&paths),
        _ => {
            eprintln!("Usage: ./sow [native|l|p|a]");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_site_sources_present() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let site = root.join("sow-web/site");
        for required in [
            "index.html",
            "app.js",
            "site-chrome.js",
            "styles.css",
            "legal.css",
            "fonts/fonts.css",
            "fonts/work-sans-latin.woff2",
            "fonts/work-sans-latin-ext.woff2",
            "fonts/work-sans-italic-latin.woff2",
            "fonts/work-sans-italic-latin-ext.woff2",
            "wou-auth.js",
            "how-to-play/index.html",
        "leaders/index.html",
        "auth/callback/index.html",
        "8d227b8f9e6140d39e3381a1829e1db3.txt",
            "privacy/index.html",
            "terms/index.html",
            "support/index.html",
        ] {
            assert!(
                site.join(required).is_file(),
                "Required site source file missing: {required}"
            );
        }
        for required in [
            "index.html.template",
            "main_menu.css",
            "main_menu.js",
            "main_menu.i18n.js",
            "main_menu.base.css",
            "main_menu.hud.css",
            "main_menu.profile.css",
            "main_menu.core.js",
            "main_menu.motion.js",
            "main_menu.lobbies.js",
            "main_menu.store.js",
            "main_menu.poki.js",
            "main_menu.heroes.js",
            "main_menu.profile.js",
            "main_menu.tutorial.js",
            "main_menu.shell.js",
            "main_menu.hud.js",
        ] {
            assert!(
                root.join("sow-web/shell").join(required).is_file(),
                "Required game shell source file missing: {required}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_build_index_inlines_web_menu_shell() -> Result<()> {
        let paths = Paths::discover()?;
        let out = tempfile::tempdir()?;
        build_index(
            &paths,
            out.path(),
            IndexBuild {
                version: "test",
                js: "sow_client_test.js",
                wasm: "sow_client_test_bg.wasm",
                ts: "test",
                maps_cache_bust: "test-maps",
                target: WebTarget::Local,
            },
        )?;
        let html = fs::read_to_string(out.path().join("play/index.html"))?;
        assert!(!html.contains("__SOW_SPLASH_DESKTOP_DATA__"));
        assert!(!html.contains("__SOW_SPLASH_MOBILE_DATA__"));
        assert!(html.contains("data:image/webp;base64,"));
        assert!(!html.contains("__INLINE_MAIN_MENU_CSS__"));
        assert!(!html.contains("__INLINE_MAIN_MENU_JS__"));
        assert!(html.contains("#sow-menu"));
        assert!(html.contains("SOW_menu_command"));
        assert!(html.contains("SOW_onStateUpdate"));
        assert!(html.contains("sow-hud__dock"));
        Ok(())
    }

    #[test]
    fn test_build_index_inlines_poki_shell_without_store_or_service_worker() -> Result<()> {
        let paths = Paths::discover()?;
        let out = tempfile::tempdir()?;
        build_index(
            &paths,
            out.path(),
            IndexBuild {
                version: "test",
                js: "sow_client.js",
                wasm: "sow_client_bg.wasm",
                ts: "test",
                maps_cache_bust: "test-maps",
                target: WebTarget::Poki,
            },
        )?;
        let html = fs::read_to_string(out.path().join("index.html"))?;
        assert!(html.contains("href=\"fonts/fonts.css\""));
        assert!(html.contains("./assets/shell/loader/loader_empty.webp"));
        assert!(html.contains("main_menu.poki.js"));
        assert!(!html.contains("main_menu.store.js"));
        assert_eq!(html.matches("function selfCreds()").count(), 1);
        assert!(!html.contains("<iframe"));
        assert!(!html.contains("href=\"https://"));
        assert!(!html.contains("allowfullscreen"));
        assert!(!html.contains("web-share"));
        assert!(!html.contains("focus-without-user-activation"));
        assert!(!html.contains("monetization"));
        assert!(!html.contains("__SOW_SERVICE_WORKER_SLOT__"));
        assert!(!html.contains("register('/sw.js'"));
        for forbidden in POKI_FORBIDDEN_MARKERS {
            assert!(!html.contains(forbidden), "Poki shell contains {forbidden}");
        }
        Ok(())
    }

    #[test]
    fn test_local_watcher_rebuilds_wasm_only_for_game_rust() -> Result<()> {
        let paths = Paths::discover()?;
        let rust_path = paths.root.join("sow-client/src/lib.rs");
        let css_path = paths.root.join("sow-web/shell/main_menu.base.css");
        let previous = vec![
            LocalFileStamp {
                path: rust_path.clone(),
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: css_path.clone(),
                len: 1,
                modified_nanos: 1,
            },
        ];
        let mut current = previous.clone();
        current[0].len = 2;
        assert!(local_change_requires_wasm(&previous, &current, &paths));

        current = previous.clone();
        current[1].len = 2;
        assert!(!local_change_requires_wasm(&previous, &current, &paths));
        Ok(())
    }

    #[test]
    fn test_leader_compendium_contains_all_twelve_leaders() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let leaders_html = fs::read_to_string(root.join("sow-web/site/leaders/index.html"))?;
        for leader_id in [
            "caesar",
            "cleopatra",
            "ragnar",
            "suntzu",
            "alexander",
            "genghiskhan",
            "richard",
            "vercingetorix",
            "boudica",
            "ladysixsky",
            "leonidas",
            "napoleon",
        ] {
            assert!(
                leaders_html.contains(&format!("id=\"{leader_id}\"")),
                "leaders/index.html missing section for leader id: {leader_id}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_index_html_prerenders_all_twelve_leaders() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let index_html = fs::read_to_string(root.join("sow-web/site/index.html"))?;
        for leader_id in [
            "caesar",
            "cleopatra",
            "ragnar",
            "suntzu",
            "alexander",
            "genghiskhan",
            "richard",
            "vercingetorix",
            "boudica",
            "ladysixsky",
            "leonidas",
            "napoleon",
        ] {
            assert!(
                index_html.contains(&format!("data-leader-id=\"{leader_id}\"")),
                "index.html missing prerendered card for leader id: {leader_id}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_marketing_mechanics_match_engine_terms() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let leaders_html = fs::read_to_string(root.join("sow-web/site/leaders/index.html"))?;
        let how_to_play = fs::read_to_string(root.join("sow-web/site/how-to-play/index.html"))?;
        assert!(leaders_html.contains("Armory modules grant +50% max troop capacity."));
        assert!(!leaders_html.contains("Armory / Bunker districts"));
        assert!(!how_to_play.contains("raise garrison limits"));
        Ok(())
    }

    #[test]
    fn test_nginx_conf_redirects_www() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let conf =
            fs::read_to_string(root.join("sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf"))?;
        assert!(
            conf.contains("server_name www.shadowsofwar.io;"),
            "Nginx missing dedicated www server_name"
        );
        assert!(
            conf.contains("return 301 https://shadowsofwar.io$request_uri;"),
            "Nginx missing 301 redirect to canonical root"
        );
        let security =
            fs::read_to_string(root.join("sow-dist/deploy/freebsd/conf.d/00-00-security.conf"))?;
        assert!(
            security.contains("log_format origin_bypass"),
            "Nginx security config missing origin_bypass log format"
        );
        assert!(
            security.contains("map $is_cloudflare_or_local $is_origin_bypass"),
            "Nginx security config missing origin cloaking map"
        );
        Ok(())
    }
}
