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
use std::{
    collections::{HashMap, HashSet},
    env, fs, thread,
};

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

struct SourceIdentity {
    sha: String,
    url: String,
}

fn source_identity(paths: &Paths) -> Result<SourceIdentity> {
    let root = paths.root.to_str().context("workspace path is not UTF-8")?;
    let sha = output(
        "git",
        &["-C", root, "rev-parse", "--verify", "HEAD^{commit}"],
    )?;
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("source revision is not a full Git commit SHA");
    }
    Ok(SourceIdentity {
        url: format!("https://github.com/worldofunreal/shadows-of-war/tree/{sha}"),
        sha,
    })
}

struct Paths {
    root: PathBuf,
    target_dir: PathBuf,
    shell: PathBuf,
    assets_shell: PathBuf,
    assets_gameplay: PathBuf,
    assets_site: PathBuf,
    assets_maps: PathBuf,
    map_sources: PathBuf,
    dist_web: PathBuf,
    dist_cg: PathBuf,
    dist_poki: PathBuf,
    dist_jest: PathBuf,
    native_web: PathBuf,
    wasm_input: PathBuf,
    wasm_cache: PathBuf,
    site_media: PathBuf,
    runtime_assets: PathBuf,
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
        let runtime_assets = s.join("runtime-assets");
        stage_runtime_assets(&root, &runtime_assets)?;
        Ok(Self {
            wasm_input: t.join("wasm32-unknown-unknown/wasm-release/sow_client.wasm"),
            wasm_cache: s.join("wasm-opt-cache"),
            site_media: runtime_assets.join("site/media"),
            shell: root.join("sow-web/shell"),
            assets_shell: runtime_assets.join("shell"),
            assets_gameplay: runtime_assets.join("gameplay"),
            assets_site: root.join("assets/site"),
            assets_maps: root.join("assets/maps"),
            map_sources: root.join("assets/map_sources"),
            dist_web: root.join("dist/web"),
            dist_cg: root.join("dist/crazygames"),
            dist_poki: root.join("dist/poki"),
            dist_jest: root.join("dist/jest"),
            native_web: t.join("sow-native-dev/web"),
            target_dir: t,
            root,
            runtime_assets,
        })
    }
}

/// Icons and store art: (file stem in assets/art/<dir>/, published path, max side in px).
const ART_ICONS: [(&str, &str, &str, u32); 14] = [
    ("icons", "gold", "gameplay/currency/gold.webp", 128),
    ("icons", "gem", "gameplay/currency/gem.webp", 128),
    ("icons", "crown", "gameplay/currency/crown.webp", 128),
    ("icons", "laurel", "gameplay/currency/laurel.webp", 128),
    (
        "icons",
        "troops",
        "gameplay/icons/battle_icon_cutout.webp",
        128,
    ),
    (
        "icons",
        "tutorial-hand",
        "gameplay/icons/tutorial_hand.webp",
        256,
    ),
    (
        "icons",
        "surrender",
        "gameplay/icons/surrender_flag.webp",
        256,
    ),
    ("icons", "troops", "shell/mobile-nav/battle.webp", 128),
    ("icons", "nav-heroes", "shell/mobile-nav/heroes.webp", 128),
    ("icons", "nav-profile", "shell/mobile-nav/profile.webp", 128),
    ("icons", "nav-store", "shell/mobile-nav/store.webp", 128),
    (
        "store",
        "sow_gems_500",
        "gameplay/store/gem_bundles/sow_gems_500.webp",
        128,
    ),
    (
        "store",
        "sow_gems_1200",
        "gameplay/store/gem_bundles/sow_gems_1200.webp",
        128,
    ),
    (
        "store",
        "sow_gems_2600",
        "gameplay/store/gem_bundles/sow_gems_2600.webp",
        128,
    ),
];
/// Folders published byte-for-byte: (assets/art/<dir>, published folder).
const ART_FOLDERS: [(&str, &str); 6] = [
    ("brand", "shell/brand"),
    ("icons/abilities", "gameplay/icons/abilities"),
    ("loader", "shell/loader"),
    ("skins", "gameplay/skins"),
    ("music", "gameplay/music"),
    ("units", "gameplay/units"),
];
/// Tool output in assets/generated: (file, published path).
const GENERATED_FILES: [(&str, &str); 3] = [
    ("buildings-atlas.webp", "gameplay/buildings/atlas.webp"),
    ("buildings-atlas.json", "gameplay/buildings/atlas.json"),
    (
        "badges-atlas.webp",
        "gameplay/icons/nameplate_status_atlas.webp",
    ),
];
const PORTRAIT_PX: u32 = 256;
/// Landing shots (assets/site/media/<stem>.<png|jpg|webp>): until the owner drops a
/// capture, each slot publishes current game art (dir, stem) so nothing stale shows.
const SITE_STILLS: [(&str, (&str, &str)); 3] = [
    ("step-expand", ("assets/maps/world", "thumbnail")),
    (
        "step-build",
        ("assets/art/heroes", "richard_the_lionheart_desktop"),
    ),
    ("step-war", ("assets/art/heroes", "leonidas_desktop")),
];
/// Landing videos (assets/site/media/<stem>.<mp4|mov|webm|mkv>), published as webm,
/// mp4 and a poster. Optional: the page shows a still while a slot is empty.
const SITE_VIDEOS: [&str; 2] = ["hero-loop", "gameplay"];
const SITE_VIDEO_SOURCES: [&str; 4] = ["mp4", "mov", "webm", "mkv"];
const SITE_BADGES: [&str; 2] = ["testflight-badge.svg", "google-play-badge.svg"];
const SITE_SOURCE_ONLY_MEDIA: [&str; 8] = [
    "session-battle.webp",
    "session-conflict.webp",
    "session-expansion.webp",
    "session-late-map.webp",
    "session-leader.webp",
    "session-world.webp",
    "shadows-of-war-gameplay.mp4",
    "shadowsofwar-scroll.vp9.webm",
];
const SITE_STILL_PX: u32 = 1600;

/// The owner edits only assets/art; the tools write assets/generated. This is the
/// single place that turns them into the published tree (/assets/shell, /assets/gameplay)
/// used by the shell, the WASM client and the CDN. Unknown art fails the build so a
/// dropped file can never sit unused or unpublished.
fn stage_runtime_assets(root: &Path, out: &Path) -> Result<()> {
    let art = root.join("assets/art");
    let generated = root.join("assets/generated");
    let mut produced = std::collections::BTreeSet::new();
    let mut publish = |src: &Path, rel: &str, max_side: Option<u32>| -> Result<()> {
        let dst = out.join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        let is_webp = src.extension().is_some_and(|ext| ext == "webp");
        let oversized = match max_side {
            Some(max) => {
                let (w, h) = image::image_dimensions(src)
                    .with_context(|| format!("read art {}", src.display()))?;
                w > max || h > max
            }
            None => false,
        };
        let needs_webp = rel.ends_with(".webp") && !is_webp;
        if oversized || needs_webp {
            let fresh = fs::metadata(&dst)
                .and_then(|d| Ok(d.modified()? >= fs::metadata(src)?.modified()?))
                .unwrap_or(false);
            if !fresh {
                let max = max_side.unwrap_or(4096);
                optimize_webp(src, &dst, &format!("{max}x{max}>"), 85)?;
            }
        } else if fs::read(&dst).ok() != Some(fs::read(src)?) {
            fs::copy(src, &dst).with_context(|| format!("publish {}", src.display()))?;
        }
        produced.insert(dst);
        Ok(())
    };
    let source_named = |dir: &Path, stem: &str| -> Option<PathBuf> {
        ["webp", "png", "jpg", "jpeg"]
            .iter()
            .map(|ext| dir.join(format!("{stem}.{ext}")))
            .find(|path| path.is_file())
    };

    // Avatars: ordinary portraits plus one 2×2 source sheet per playable hero.
    let mut avatar_sources = std::collections::BTreeMap::new();
    let hero_ids = sow_core::player::Leader::all()
        .map(sow_core::player::Leader::id)
        .collect::<std::collections::BTreeSet<_>>();
    for folder in ["heroes", "nations", "tribes", "campaign"] {
        for file in sorted_files(&art.join("avatars").join(folder))? {
            let stem = file_stem(&file);
            if folder == "heroes" {
                if expression_part(&stem).is_some() {
                    bail!(
                        "hero faces now belong in one 2 x 2 sheet: remove {}",
                        file.display()
                    );
                }
                if hero_ids.contains(stem.as_str()) {
                    continue;
                }
                bail!(
                    "unexpected avatar {}: keep only playable hero sheets in avatars/heroes; general portraits belong in avatars/",
                    file.display()
                );
            }
            if let Some(first) = avatar_sources.insert(stem.clone(), file.clone()) {
                bail!(
                    "avatar {stem} is in two folders: {} and {}",
                    first.display(),
                    file.display()
                );
            }
            publish(
                &file,
                &format!("gameplay/avatars/{stem}.webp"),
                Some(PORTRAIT_PX),
            )?;
        }
    }
    let random_avatar = source_named(&art.join("avatars"), "random")
        .context("missing art: assets/art/avatars/random.webp")?;
    if let Some(first) = avatar_sources.insert("random".to_owned(), random_avatar.clone()) {
        bail!(
            "avatar random is in two folders: {} and {}",
            first.display(),
            random_avatar.display()
        );
    }
    publish(
        &random_avatar,
        "gameplay/avatars/random.webp",
        Some(PORTRAIT_PX),
    )?;
    publish(
        &art.join("avatars/default.webp"),
        "gameplay/avatars/null.webp",
        Some(PORTRAIT_PX),
    )?;
    let hero_crop_dir = tempfile::tempdir().context("create temporary hero avatar crops")?;
    for leader in sow_core::player::Leader::all() {
        let base = leader.id();
        let source = art.join("avatars/heroes").join(format!("{base}.webp"));
        if !source.is_file() {
            bail!("hero {base} needs one 2 x 2 sheet at {}", source.display());
        }
        if let Some(first) = avatar_sources.insert(base.to_owned(), source.clone()) {
            bail!(
                "avatar {base} is in two folders: {} and {}",
                first.display(),
                source.display()
            );
        }
        publish(
            &source,
            &format!("gameplay/avatars/spritesheets/{base}.webp"),
            None,
        )?;
        let sheet = image::open(&source)
            .with_context(|| format!("read hero sheet {}", source.display()))?
            .to_rgba8();
        for (expression, name) in sow_core::emote::AvatarExpression::ALL
            .into_iter()
            .zip(["happy", "calm", "fear", "angry"])
        {
            let crop = hero_expression_crop(&sheet, expression)?;
            let crop_path = hero_crop_dir
                .path()
                .join(format!("{base}_{name}.png"));
            crop.save(&crop_path)
                .with_context(|| format!("write {name} crop for hero {base}"))?;
            publish(
                &crop_path,
                &format!("gameplay/avatars/expressions/{base}_{name}.webp"),
                Some(PORTRAIT_PX),
            )?;
            if expression == sow_core::emote::AvatarExpression::Calm {
                publish(
                    &crop_path,
                    &format!("gameplay/avatars/{base}.webp"),
                    Some(PORTRAIT_PX),
                )?;
            }
        }
    }
    // Hero scenes are optional; publish whichever desktop/mobile art is present.
    for file in sorted_files(&art.join("heroes"))? {
        let stem = file_stem(&file);
        if !stem.ends_with("_desktop") && !stem.ends_with("_mobile") {
            bail!(
                "unknown hero scene {}: use <id>_desktop or <id>_mobile",
                file.display()
            );
        }
        publish(&file, &format!("shell/leaders/{stem}.webp"), None)?;
    }
    let mut known_icons = std::collections::BTreeSet::new();
    // Badge icons are packed by sow-tools into badges-atlas, not published one by one.
    let badges: serde_json::Value =
        serde_json::from_slice(&fs::read(generated.join("badges-atlas.json"))?)?;
    for cell in badges["cells"].as_array().into_iter().flatten() {
        let stem = cell.as_str().unwrap_or_default();
        known_icons.insert(
            source_named(&art.join("icons"), stem)
                .with_context(|| format!("missing art: assets/art/icons/{stem}.png"))?,
        );
    }
    for (dir, stem, rel, max) in ART_ICONS {
        let src = source_named(&art.join(dir), stem)
            .with_context(|| format!("missing art: assets/art/{dir}/{stem}.png"))?;
        known_icons.insert(src.clone());
        publish(&src, rel, Some(max))?;
    }
    for dir in ["icons", "store"] {
        for file in fs::read_dir(art.join(dir))? {
            let file = file?.path();
            if ART_FOLDERS
                .iter()
                .any(|(source, _)| art.join(source) == file)
            {
                continue;
            }
            if !known_icons.contains(&file) {
                bail!(
                    "unknown art {}: add it to ART_ICONS or ART_FOLDERS in sow-dist",
                    file.display()
                );
            }
        }
    }
    for (dir, rel_dir) in ART_FOLDERS {
        for file in fs::read_dir(art.join(dir))? {
            let file = file?.path();
            let name = file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            publish(&file, &format!("{rel_dir}/{name}"), None)?;
        }
    }
    // A fresh checkout may not have the packed atlases yet: `pack_art_atlases`
    // writes them and stages again, and fails if any is still missing.
    for (name, rel) in GENERATED_FILES {
        if !generated.join(name).is_file() {
            continue;
        }
        publish(&generated.join(name), rel, None)?;
    }
    let media = root.join("assets/site/media");
    for badge in SITE_BADGES {
        publish(&media.join(badge), &format!("site/media/{badge}"), None)?;
    }
    for (stem, (dir, provisional)) in SITE_STILLS {
        let src = source_named(&media, stem)
            .or_else(|| source_named(&root.join(dir), provisional))
            .with_context(|| format!("landing shot {stem} has no capture and no art"))?;
        publish(
            &src,
            &format!("site/media/{stem}.webp"),
            Some(SITE_STILL_PX),
        )?;
    }
    for file in fs::read_dir(&media)? {
        let file = file?.path();
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        let ext = file.extension().unwrap_or_default().to_string_lossy();
        let known = SITE_BADGES.contains(&name.as_ref())
            || SITE_SOURCE_ONLY_MEDIA.contains(&name.as_ref())
            || (SITE_STILLS.iter().any(|(still, _)| *still == stem)
                && ["webp", "png", "jpg", "jpeg"].contains(&ext.as_ref()))
            || (SITE_VIDEOS.contains(&stem.as_ref()) && SITE_VIDEO_SOURCES.contains(&ext.as_ref()));
        if !known {
            bail!(
                "unregistered site media {}: add it to SITE_STILLS, SITE_VIDEOS or SITE_SOURCE_ONLY_MEDIA",
                file.display()
            );
        }
    }
    for stem in SITE_VIDEOS {
        let Some(src) = SITE_VIDEO_SOURCES
            .iter()
            .map(|ext| media.join(format!("{stem}.{ext}")))
            .find(|path| path.is_file())
        else {
            continue;
        };
        produced.extend(publish_site_video(&src, &out.join("site/media"), stem)?);
    }

    for entry in walkdir::WalkDir::new(out).contents_first(true) {
        let entry = entry?;
        if entry.file_type().is_file() && !produced.contains(entry.path()) {
            fs::remove_file(entry.path())?;
        } else if entry.file_type().is_dir() && entry.path() != out {
            let _ = fs::remove_dir(entry.path());
        }
    }
    Ok(())
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

/// Content hashes keyed by the exact URL paths the shell builds (relative to
/// assets/, e.g. "shell/leaders/boudica_desktop.webp"). Injected as
/// window.SOW_ASSET_TS so ?v= cache keys stop changing on unrelated deploys:
/// a music/leader redraw alone invalidates its own entry. Locale catalogs are
/// hashed straight from the exported bytes (locales_dir) — WEB_CATALOG_VERSION
/// alone is a hand-bumped constant, and immutable URLs must never depend on
/// someone remembering to bump it.
fn asset_versions(paths: &Paths, locales_dir: &Path) -> Result<serde_json::Value> {
    let mut map = serde_json::Map::new();
    let mut roots = vec![
        (paths.assets_shell.clone(), "shell".to_string()),
        (paths.assets_gameplay.clone(), "gameplay".to_string()),
    ];
    while let Some((dir, prefix)) = roots.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = format!("{prefix}/{name}");
            if path.is_dir() {
                roots.push((path, rel));
                continue;
            }
            // shell/loader/ is deliberately NOT tracked: its URLs are baked into
            // index.html with ?v=__BUILD_TS__ and loader.js re-assigns the same
            // nodes; a content-hash variant here made the browser fetch both.
            let track = matches!(
                rel.as_str(),
                s if s.starts_with("shell/leaders/")
                    || s.starts_with("shell/mobile-nav/")
                    || s.starts_with("gameplay/music/")
            );
            if track && path.extension().is_some_and(|e| e == "webp" || e == "webm") {
                map.insert(
                    format!("/assets/{rel}"),
                    serde_json::Value::String(file_sha256(&path)?[..8].to_string()),
                );
            }
        }
    }
    if let Ok(entries) = fs::read_dir(locales_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            map.insert(
                format!("/locales/{name}"),
                serde_json::Value::String(file_sha256(&path)?[..8].to_string()),
            );
        }
    }
    Ok(serde_json::Value::Object(map))
}

/// UI fonts for every web package: only fonts.css, OFL.txt and the woff2
/// faces ship to the webroot. The Noto TTF/TTCs stay in the repo because the
/// atlas generator and the Arabic runtime rasterizer read them from there,
/// but as webfonts they cost a player up to 19.5 MB at boot (CJK), uncached
/// at the CDN edge. Each url() in fonts.css carries the face's content hash,
/// so an unchanged font is never re-downloaded; fonts.css itself rides
/// ?v=<build> from index.html, so deploys always refresh the stylesheet.
fn copy_web_fonts(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    let mut faces: Vec<(String, String)> = Vec::new();
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".woff2") {
            faces.push((name.clone(), file_sha256(&path)?[..8].to_string()));
            fs::copy(&path, dst.join(&name))?;
        } else if name == "OFL.txt" {
            fs::copy(&path, dst.join(&name))?;
        }
    }
    let mut css = fs::read_to_string(src.join("fonts.css"))?;
    for (name, hash) in &faces {
        css = css.replace(
            &format!("url(./{name})"),
            &format!("url(./{name}?v={hash})"),
        );
    }
    let mut rest = css.as_str();
    while let Some(i) = rest.find("url(./") {
        let tail = &rest[i + 6..];
        let end = tail.find(')').unwrap_or(tail.len());
        if !tail[..end].contains("?v=") {
            bail!("fonts.css references an unshipped face: {}", &tail[..end]);
        }
        rest = &tail[end..];
    }
    fs::write(dst.join("fonts.css"), css)?;
    Ok(())
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

fn optimize_webp(src: &Path, dst: &Path, geometry: &str, quality: u32) -> Result<()> {
    let status = Command::new("magick")
        .arg(src)
        .args([
            "-resize",
            geometry,
            "-strip",
            "-quality",
            &quality.to_string(),
        ])
        .args(["-define", "webp:method=6"])
        .arg(dst)
        .status()
        .with_context(|| "web packaging requires ImageMagick (magick)")?;
    if !status.success() {
        bail!("webp optimization failed for {}", src.display());
    }
    Ok(())
}

/// Files directly inside `dir`, sorted by name.
fn sorted_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(dir)
        .with_context(|| format!("read art {}", dir.display()))?
        .map(|entry| Ok(entry?.path()))
        .collect::<Result<Vec<_>>>()?;
    files.retain(|path| path.is_file());
    files.sort();
    Ok(files)
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Recognize removed per-expression files so the build rejects the old multi-file layout.
fn expression_part(stem: &str) -> Option<&str> {
    let (base, index) = stem.rsplit_once('_')?;
    matches!(index, "1" | "2" | "3" | "4").then_some(base)
}

/// Crop one named expression using the same quadrant and separator rules as the game client.
fn hero_expression_crop(
    sheet: &image::RgbaImage,
    expression: sow_core::emote::AvatarExpression,
) -> Result<image::RgbaImage> {
    let (width, height) = sheet.dimensions();
    if width < 4 || height < 4 {
        bail!("hero sheet must contain a 2 x 2 grid");
    }
    let half_width = width / 2;
    let half_height = height / 2;
    let inset = ((half_width.min(half_height) as f32 * 0.01).round() as u32).max(1);
    let index = expression as u32;
    let quadrant = image::imageops::crop_imm(
        sheet,
        (index % 2) * half_width + inset,
        (index / 2) * half_height + inset,
        half_width.saturating_sub(2 * inset).max(1),
        half_height.saturating_sub(2 * inset).max(1),
    )
    .to_image();
    let size = quadrant.width().min(quadrant.height()).max(1);
    let square = image::imageops::crop_imm(
        &quadrant,
        (quadrant.width() - size) / 2,
        (quadrant.height() - size) / 2,
        size,
        size,
    )
    .to_image();
    Ok(image::imageops::resize(
        &square,
        PORTRAIT_PX,
        PORTRAIT_PX,
        image::imageops::FilterType::Triangle,
    ))
}

/// A landing capture becomes `<stem>.webm` (VP9), `<stem>.mp4` (H.264) and a
/// `<stem>.webp` poster. Encoding is slow, so outputs newer than the capture are kept.
fn publish_site_video(src: &Path, dir: &Path, stem: &str) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let webm = dir.join(format!("{stem}.webm"));
    let mp4 = dir.join(format!("{stem}.mp4"));
    let poster = dir.join(format!("{stem}.webp"));
    let outputs = vec![webm.clone(), mp4.clone(), poster.clone()];
    let source_time = fs::metadata(src)?.modified()?;
    let fresh = outputs.iter().all(|path| {
        fs::metadata(path)
            .and_then(|meta| meta.modified())
            .is_ok_and(|time| time >= source_time)
    });
    if fresh {
        return Ok(outputs);
    }
    // The background loop plays muted; the gameplay video keeps its sound.
    let audio = stem != "hero-loop";
    let scale = "scale='min(1920,iw)':-2";
    let ffmpeg = |args: &[&str], dst: &Path| -> Result<()> {
        let status = Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(src)
            .args(args)
            .arg(dst)
            .status()
            .with_context(|| "landing videos require ffmpeg")?;
        if !status.success() {
            bail!("ffmpeg failed for {} -> {}", src.display(), dst.display());
        }
        Ok(())
    };
    let webm_audio: &[&str] = if audio {
        &["-c:a", "libopus", "-b:a", "96k"]
    } else {
        &["-an"]
    };
    let mp4_audio: &[&str] = if audio {
        &["-c:a", "aac", "-b:a", "128k"]
    } else {
        &["-an"]
    };
    let mut args = vec![
        "-vf",
        scale,
        "-c:v",
        "libvpx-vp9",
        "-b:v",
        "0",
        "-crf",
        "34",
        "-row-mt",
        "1",
    ];
    args.extend_from_slice(webm_audio);
    ffmpeg(&args, &webm)?;
    let mut args = vec![
        "-vf",
        scale,
        "-c:v",
        "libx264",
        "-crf",
        "26",
        "-preset",
        "slow",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
    ];
    args.extend_from_slice(mp4_audio);
    ffmpeg(&args, &mp4)?;
    let frame = dir.join(format!("{stem}.poster.png"));
    ffmpeg(&["-vf", scale, "-frames:v", "1"], &frame)?;
    optimize_webp(
        &frame,
        &poster,
        &format!("{SITE_STILL_PX}x{SITE_STILL_PX}>"),
        80,
    )?;
    fs::remove_file(&frame)?;
    Ok(outputs)
}

/// Full-bleed loader/menu leader art ships recompressed: the repo masters are
/// 400-950 KB each and the loader waits on this art at match exit. Cap keeps a
/// regression (a new unoptimized master) from silently re-inflating the boot.
fn compress_web_leaders(leaders: &Path) -> Result<()> {
    const MAX_LEADER_BYTES: u64 = 400_000;
    for entry in fs::read_dir(leaders)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".webp") {
            continue;
        }
        let path = entry.path();
        let geometry = if name.ends_with("_mobile.webp") {
            "900x1600>"
        } else {
            "1600x900>"
        };
        let tmp = path.with_extension("webp.optimizing");
        optimize_webp(&path, &tmp, geometry, 75)?;
        fs::rename(&tmp, &path)?;
        let size = fs::metadata(&path)?.len();
        if size > MAX_LEADER_BYTES {
            bail!(
                "leader art {name} weighs {size} bytes after optimization; cap is {MAX_LEADER_BYTES}"
            );
        }
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
        optimize_webp(
            &entry.path(),
            &dst.join("shell/leaders").join(entry.file_name()),
            geometry,
            70,
        )?;
    }
    copy_dir(&src.join("gameplay/avatars"), &dst.join("gameplay/avatars"))?;
    copy_dir(
        &src.join("gameplay/buildings"),
        &dst.join("gameplay/buildings"),
    )?;
    copy_dir(&src.join("gameplay/music"), &dst.join("gameplay/music"))?;
    copy_dir(
        &src.join("gameplay/currency"),
        &dst.join("gameplay/currency"),
    )?;
    copy_campaign_for_poki(&src.join("campaign"), &dst.join("campaign"))?;
    copy_dir(&src.join("guides"), &dst.join("guides"))?;
    let mobile_nav = dst.join("shell/mobile-nav");
    fs::create_dir_all(&mobile_nav)?;
    for file in ["heroes.webp", "battle.webp", "profile.webp"] {
        fs::copy(
            src.join("shell/mobile-nav").join(file),
            mobile_nav.join(file),
        )?;
    }
    Ok(())
}

/// Poki players are Chromium/Safari; the MP4 rendition covers both. Shipping
/// only the MP4 halves the campaign payload in the portal package. The
/// triggers JSON is rewritten to the MP4 stem so no source 404s on boot.
fn copy_campaign_for_poki(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let from = entry.path();
        if from.is_dir() {
            copy_campaign_for_poki(&from, &dst.join(&name))?;
            continue;
        }
        if name.ends_with(".webm") {
            continue;
        }
        let to = dst.join(&name);
        if name.ends_with(".triggers.json") {
            let text = fs::read_to_string(&from)?.replace(".webm\"", ".mp4\"");
            fs::write(&to, text)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn copy_poki_maps(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    fs::copy(src.join("catalog.bin"), dst.join("catalog.bin"))?;
    fs::copy(src.join("NOTICE"), dst.join("NOTICE"))
        .context("copy map attribution notice into portal package")?;
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
            optimize_webp(&thumbnail, &out_dir.join("thumbnail.webp"), "256x144", 70)?;
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

/// Packs assets/art/{buildings,badges} into assets/generated. Runs before
/// anything reads the atlases (tests and the WASM build), so replacing art and
/// running the pipeline is always enough.
fn pack_art_atlases(paths: &Paths) -> Result<()> {
    run(
        "cargo",
        &[
            "run",
            "-p",
            "sow-tools",
            "--bin",
            "sow-tools",
            "--",
            "pack-building-atlas",
        ],
        Some(&paths.root),
    )?;
    let generated = paths.root.join("assets/generated");
    for (name, _) in GENERATED_FILES {
        require_file(&generated.join(name), "packed atlas")?;
    }
    stage_runtime_assets(
        &paths.root,
        &paths.root.join("dist/.sow-state/runtime-assets"),
    )
}

fn compile_wasm_profile(paths: &Paths, profile: &str, dev: bool) -> Result<PathBuf> {
    let profile_dir = match profile {
        "dev" => "debug",
        "wasm-dev" => "wasm-dev",
        "wasm-release" => "wasm-release",
        _ => bail!("unsupported WASM profile: {profile}"),
    };
    pack_art_atlases(paths)?;
    println!("==> Compiling WASM ({profile})...");
    let mut a = vec!["build"];
    if profile != "dev" {
        a.extend_from_slice(&["--profile", profile]);
    }
    a.extend_from_slice(&["-p", "sow-client", "--target", "wasm32-unknown-unknown"]);
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
    let wasm = paths
        .target_dir
        .join("wasm32-unknown-unknown")
        .join(profile_dir)
        .join("sow_client.wasm");
    require_file(&wasm, "WASM output")?;
    Ok(wasm)
}

fn compile_wasm(paths: &Paths, dev: bool) -> Result<()> {
    compile_wasm_profile(paths, "wasm-release", dev)?;
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

fn run_wasm_opt(path: &Path, cache: &Path, compress: bool) -> Result<()> {
    require_file(path, "wasm-opt input")?;
    let hash = file_sha256(path)?;
    fs::create_dir_all(cache)?;
    let c = cache.join(format!("{WASM_OPT_TAG}-{hash}.wasm"));
    let cb = cache.join(format!("{WASM_OPT_TAG}-{hash}.wasm.br"));
    let br = brotli_dst(path);
    if c.is_file() {
        fs::copy(&c, path)?;
        // Keeps the entry alive for retain_local_builds.
        let now = std::time::SystemTime::now();
        fs::File::options()
            .append(true)
            .open(&c)?
            .set_modified(now)?;
        if compress {
            if cb.is_file() {
                fs::File::options()
                    .append(true)
                    .open(&cb)?
                    .set_modified(now)?;
                fs::copy(&cb, &br)?;
            } else {
                compress_brotli(path, &br)?;
                fs::copy(&br, &cb)?;
            }
        }
        require_file(path, "wasm-opt")?;
        if compress {
            require_file(&br, "brotli")?;
        }
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
    if compress {
        compress_brotli(path, &br)?;
        fs::copy(&br, &cb)?;
        require_file(&br, "brotli")?;
    }
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

fn web_catalog_value<'a>(
    catalog: &'a serde_json::Value,
    key: &str,
) -> Option<&'a serde_json::Value> {
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
    let catalog = validate_web_catalogs(false)?;
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

fn validate_source_localization_keys(
    source: &str,
    label: &str,
    catalog: &serde_json::Value,
) -> Result<()> {
    for needle in ["SOW_t(", "siteText(", "translate("] {
        let mut offset = 0;
        while let Some(found) = source[offset..].find(needle) {
            let start = offset + found + needle.len();
            let rest = &source[start..];
            let trimmed = rest.trim_start();
            let Some(quote) = trimmed.as_bytes().first().copied() else {
                break;
            };
            if quote != b'"' && quote != b'\'' {
                offset = start;
                continue;
            }
            let value = &trimmed[1..];
            let end = value
                .find(quote as char)
                .with_context(|| format!("unterminated localization key in {label}"))?;
            let key = &value[..end];
            if key.ends_with('_') {
                if !web_catalog_has_prefix(catalog, key) {
                    bail!("{label} uses unknown localization key prefix {key}");
                }
            } else if web_catalog_value(catalog, key).is_none() {
                bail!("{label} uses unknown localization key {key}");
            }
            offset = start + rest.len() - trimmed.len() + 1 + end;
        }
    }
    for needle in [
        "data-i18n=\"",
        "data-i18n='",
        "data-i18n-placeholder=\"",
        "data-i18n-placeholder='",
        "data-i18n-values=\"",
        "data-i18n-values='",
        "data-ability-key=\"",
        "data-ability-key='",
        "data-description-key=\"",
        "data-description-key='",
    ] {
        let quote = needle.as_bytes()[needle.len() - 1] as char;
        let mut offset = 0;
        while let Some(found) = source[offset..].find(needle) {
            let start = offset + found + needle.len();
            let rest = &source[start..];
            let end = rest
                .find(quote)
                .with_context(|| format!("unterminated localization attribute in {label}"))?;
            let key = &rest[..end];
            if needle.starts_with("data-i18n-values") {
                let values = serde_json::from_str::<HashMap<String, String>>(key)
                    .with_context(|| format!("invalid data-i18n-values in {label}"))?;
                for value_key in values.values() {
                    if web_catalog_value(catalog, value_key).is_none() {
                        bail!("{label} uses unknown localization value key {value_key}");
                    }
                }
            } else if web_catalog_value(catalog, key).is_none() {
                bail!("{label} uses unknown localization key {key}");
            }
            offset = start + end;
        }
    }
    validate_quoted_catalog_keys(source, label, catalog)?;
    Ok(())
}

fn validate_quoted_catalog_keys(
    source: &str,
    label: &str,
    catalog: &serde_json::Value,
) -> Result<()> {
    const DOMAINS: [&str; 10] = [
        "menu", "auth", "lobbies", "heroes", "profile", "store", "hud", "endgame", "tutorial",
        "site",
    ];
    for domain in DOMAINS {
        for quote in ['"', '\''] {
            let needle = format!("{quote}{domain}.");
            let mut offset = 0;
            while let Some(found) = source[offset..].find(&needle) {
                let start = offset + found + 1;
                let rest = &source[start..];
                let end = rest
                    .find(|c: char| {
                        !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
                    })
                    .unwrap_or(rest.len());
                let key = &rest[..end];
                // Menu journey copy has its own catalog, checked by validateMenuGuides.
                if key.starts_with("menu.guide.") {
                    offset = start + end;
                    continue;
                }
                if key.ends_with('.') || key.ends_with('_') {
                    if !web_catalog_has_prefix(catalog, key) {
                        bail!("{label} uses unknown localization key prefix {key}");
                    }
                } else if web_catalog_value(catalog, key).is_none() {
                    bail!("{label} uses unknown localization key {key}");
                }
                offset = start + end;
            }
        }
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

fn strip_optional_marked_section(source: &str, begin: &str, end: &str) -> Result<String> {
    if !source.contains(begin) && !source.contains(end) {
        return Ok(source.to_string());
    }
    strip_marked_section(source, begin, end)
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
    Native,
    CrazyGames,
    Poki,
    Jest,
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
    let jest = target == WebTarget::Jest;
    let portal = cg || poki || jest;
    let tpl = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let experience_events = fs::read_to_string(paths.shell.join("experience_events.js"))?
        .replace("</script>", "<\\/script>");
    let experience_events_slot = "/* __INLINE_EXPERIENCE_EVENTS_JS__ */";
    if tpl.matches(experience_events_slot).count() != 1 {
        bail!("game HTML template must contain one experience-events slot");
    }
    let source_slot = "<!-- __SOW_SOURCE_IDENTITY_SLOT__ -->";
    if tpl.matches(source_slot).count() != 1 {
        bail!("game HTML template must contain one source identity slot");
    }
    let source = source_identity(paths)?;
    let source_identity_html = format!(
        "<script>window.SOW_SOURCE_SHA = {}; window.SOW_SOURCE_URL = {};</script>",
        serde_json::to_string(&source.sha)?,
        serde_json::to_string(&source.url)?,
    );
    // Portal builds keep the whole-build bust (their asset bytes are
    // re-encoded at packaging time); self-hosted and native use per-file
    // content hashes so music/leader/loader cache keys survive unrelated deploys.
    let asset_versions = if portal {
        serde_json::json!({})
    } else {
        asset_versions(paths, &out.join("locales"))?
    };
    let splash_desktop = inline_webp(&paths.assets_shell.join("loader/sow-splash-desktop.webp"))?;
    let splash_mobile = inline_webp(&paths.assets_shell.join("loader/sow-splash-mobile.webp"))?;
    let locale_codes = serde_json::to_string(
        &sow_i18n::Language::published_registry()
            .map(|(_, code, _)| code)
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
        .replace(source_slot, &source_identity_html)
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
            } else if poki || jest {
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
            "content=\"./icon-192.png\"",
            if portal {
                "content=\"icon-192.png\""
            } else {
                "content=\"../icon-192.png\""
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
        .replace(experience_events_slot, &experience_events)
        .replace(
            "<!-- __SOW_ANALYTICS_SCRIPT_SLOT__ -->",
            &if matches!(target, WebTarget::Local) {
                format!("<script src=\"../analytics.js?v={ts}\"></script>")
            } else {
                String::new()
            },
        )
        .replace(
            "<!-- __SOW_SERVICE_WORKER_SLOT__ -->",
            match target {
                WebTarget::Local => {
                    "if ('serviceWorker' in navigator && window.location.hostname !== \"appassets.androidplatform.net\" && !isPortal) { navigator.serviceWorker.register('/sw.js', { scope: './' }).catch(function (err) { console.warn('Service worker registration failed:', err); }); }"
                }
                WebTarget::Native => "",
                WebTarget::CrazyGames => {
                    "if ('serviceWorker' in navigator && !isPortal) { navigator.serviceWorker.register('sw.js', { scope: '/' }).catch(function (err) { console.warn('Service worker registration failed:', err); }); }"
                }
                WebTarget::Poki | WebTarget::Jest => "",
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
                if matches!(target, WebTarget::Local | WebTarget::Native) {
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
    if poki || jest {
        for (begin, end) in [
            (
                "/* POKI_ANDROID_AUTH_BEGIN */",
                "/* POKI_ANDROID_AUTH_END */",
            ),
            (
                "/* POKI_ANDROID_AUTH_STATE_BEGIN */",
                "/* POKI_ANDROID_AUTH_STATE_END */",
            ),
            ("/* POKI_WOU_AUTH_BEGIN */", "/* POKI_WOU_AUTH_END */"),
        ] {
            html = strip_marked_section(&html, begin, end)?;
        }
    }
    let media_artwork_reference = if portal {
        "name=\"sow-media-artwork\" content=\"icon-192.png\""
    } else {
        "name=\"sow-media-artwork\" content=\"../icon-192.png\""
    };
    if html.matches("name=\"sow-media-artwork\"").count() != 1
        || !html.contains(media_artwork_reference)
    {
        bail!("index.html has an invalid game music artwork path");
    }
    let index = if portal {
        out.join("index.html")
    } else {
        out.join("play/index.html")
    };
    fs::create_dir_all(index.parent().unwrap())?;
    fs::write(&index, &html)?;
    let mut loader = fs::read_to_string(paths.shell.join("loader.js"))?;
    if poki || jest {
        loader = strip_optional_marked_section(
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
            "sow-controls.css",
            "main_menu.base.css",
            "main_menu.layout.css",
            "main_menu.lobbies.css",
            "main_menu.create.css",
            "main_menu.queue.css",
            "main_menu.settings.css",
            "main_menu.auth.css",
            "main_menu.campaign.css",
            "main_menu.hud.css",
            "main_menu.tutorial.css",
            "main_menu.profile.css",
        ],
    )?;
    let menu_parts: Vec<&str> = if poki {
        vec![
            "sow-i18n.js",
            "sow-dropdown.js",
            "sow-campaign.js",
            "sow-campaign-view.js",
            "main_menu.flow.js",
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
    } else if jest {
        vec![
            "sow-i18n.js",
            "sow-dropdown.js",
            "sow-campaign.js",
            "sow-campaign-view.js",
            "main_menu.flow.js",
            "main_menu.core.js",
            "main_menu.motion.js",
            "main_menu.lobbies.js",
            "main_menu.heroes.js",
            "main_menu.profile.js",
            "main_menu.jest.js",
            "main_menu.tutorial.js",
            "main_menu.shell.js",
            "main_menu.hud.js",
        ]
    } else {
        vec![
            "sow-i18n.js",
            "sow-dropdown.js",
            "sow-campaign.js",
            "sow-campaign-view.js",
            "main_menu.flow.js",
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
    let building_manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(paths.assets_gameplay.join("buildings/atlas.json"))
            .context("building atlas manifest missing; run pack-building-atlas")?,
    )
    .context("building atlas manifest is invalid JSON")?;
    let building_manifest_js = format!(
        "window.SOW_BUILDING_ART = {};\n",
        serde_json::to_string(&building_manifest)?
    );
    let mut menu_js = format!(
        "{building_manifest_js}{}",
        read_shell_bundle(&paths.shell, "main_menu.js", &menu_parts)?
    )
    .replace("</script>", "<\\/script>");
    if poki || jest {
        menu_js = strip_marked_section(
            &menu_js,
            "/* POKI_RENDER_REPLACEMENT_BEGIN */",
            "/* POKI_RENDER_REPLACEMENT_END */",
        )?;
        for (begin, end) in [
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
        while menu_js.contains("/* POKI_SHARED_AUTH_LOOKUP_BEGIN */") {
            menu_js = strip_marked_section(
                &menu_js,
                "/* POKI_SHARED_AUTH_LOOKUP_BEGIN */",
                "/* POKI_SHARED_AUTH_LOOKUP_END */",
            )?;
        }
        while menu_js.contains("/* POKI_SHARED_SIGNIN_MARKUP_BEGIN */") {
            menu_js = strip_marked_section(
                &menu_js,
                "/* POKI_SHARED_SIGNIN_MARKUP_BEGIN */",
                "/* POKI_SHARED_SIGNIN_MARKUP_END */",
            )?;
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
    for bar in ["loader_empty", "loader_full"] {
        if let Some(hash) = asset_versions
            .get(format!("/assets/shell/loader/{bar}.webp"))
            .and_then(|v| v.as_str())
        {
            fh = fh.replace(
                &format!("{bar}.webp?v=__BUILD_TS__"),
                &format!("{bar}.webp?v={hash}"),
            );
        }
    }
    let marker = "/* __INLINE_LOADER_JS__ */";
    let loader_with_versions = format!(
        "window.SOW_ASSET_TS = {};\n{loader}",
        serde_json::to_string(&asset_versions)?
    );
    if fh.contains(marker) {
        fh = fh.replacen(marker, &loader_with_versions, 1);
    } else if fh.contains(r#"<script src="../loader.js"></script>"#) {
        fh = fh.replacen(
            r#"<script src="../loader.js"></script>"#,
            &format!("<script>\n{loader_with_versions}\n</script>"),
            1,
        );
    } else if fh.contains(r#"<script src="./loader.js"></script>"#) {
        fh = fh.replacen(
            r#"<script src="./loader.js"></script>"#,
            &format!("<script>\n{loader_with_versions}\n</script>"),
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
    let menu_js_marker = "<script>/* __INLINE_MAIN_MENU_JS__ */</script>";
    if fh.matches(menu_js_marker).count() != 1 {
        bail!("index.html must contain one main menu script slot");
    }
    // Self-hosted web and native ship the menu bundle as a sibling file so a
    // returning visitor pulls it from cache instead of re-reading ~1.4 MB of
    // HTML on every boot. Portal builds (CrazyGames/Poki/Jest) keep inlining.
    let menu_js_injection = if !portal {
        let menu_hash = format!("{:x}", Sha256::digest(menu_js.as_bytes()))[..8].to_string();
        let menu_name = format!("main_menu_{menu_hash}.js");
        fs::write(index.with_file_name(&menu_name), &menu_js)
            .context("write externalized main menu bundle")?;
        format!(
            "<link rel=\"preload\" href=\"./{menu_name}\" as=\"script\">\n    <script src=\"./{menu_name}\"></script>"
        )
    } else {
        format!("<script>{menu_js}</script>")
    };
    fh = fh.replacen(menu_js_marker, &menu_js_injection, 1);
    if poki || jest {
        fh = fh
            .replace(
                &format!("href=\"/fonts/fonts.css?v={ts}\""),
                &format!("href=\"fonts/fonts.css?v={ts}\""),
            )
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
    } else if cg {
        fh = fh
            .replace(
                &format!("href=\"/fonts/fonts.css?v={ts}\""),
                &format!("href=\"https://shadowsofwar.io/fonts/fonts.css?v={ts}\""),
            )
            .replace(
                "    <link rel=\"manifest\" href=\"/manifest.webmanifest\">\n",
                "",
            );
    }
    fs::write(&index, fh)?;
    Ok(())
}

fn inline_webp(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("read critical loader asset {}", path.display()))?;
    Ok(format!(
        "data:image/webp;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn copy_shell(paths: &Paths, out: &Path, target: WebTarget) -> Result<()> {
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
    if matches!(
        target,
        WebTarget::CrazyGames | WebTarget::Poki | WebTarget::Jest
    ) {
        fs::copy(
            paths.assets_site.join("icons/icon-192.png"),
            out.join("icon-192.png"),
        )?;
    }
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
    if matches!(target, WebTarget::Local) {
        fs::copy(
            paths.root.join("sow-web/site/analytics.js"),
            out.join("analytics.js"),
        )?;
    }
    fs::copy(paths.shell.join("sow-i18n.js"), out.join("sow-i18n.js"))?;
    fs::copy(
        paths.shell.join("sow-dropdown.js"),
        out.join("sow-dropdown.js"),
    )?;
    fs::copy(
        paths.shell.join("sow-controls.css"),
        out.join("sow-controls.css"),
    )?;
    copy_dir(&paths.shell.join("sdk"), &out.join("sdk"))?;
    for (source, destination) in [
        ("LICENSE", "LICENSE"),
        ("docs/legal/NOTICE", "NOTICE"),
        ("docs/legal/LICENSE-ASSETS", "LICENSE-ASSETS"),
        (
            "docs/legal/WASM-THIRD-PARTY-NOTICES.zip",
            "WASM-THIRD-PARTY-NOTICES.zip",
        ),
    ] {
        fs::copy(paths.root.join(source), out.join(destination))?;
    }
    Ok(())
}

const PUBLIC_SITE_PAGES: [&str; 5] = [
    "index.html",
    "support/index.html",
    "privacy/index.html",
    "cookies/index.html",
    "terms/index.html",
];

fn expand_public_site_header(paths: &Paths, out: &Path) -> Result<()> {
    let header = fs::read_to_string(paths.root.join("sow-web/site/site-header.html"))?
        .trim()
        .to_string();
    let marker = "<!-- SOW_SITE_HEADER -->";
    let locale_marker = "<!-- SOW_SITE_LOCALE -->";
    let locale_codes = serde_json::to_string(
        &sow_i18n::Language::published_registry()
            .map(|(_, code, _)| code)
            .collect::<Vec<_>>(),
    )?;
    let mut locale_versions = asset_versions(paths, &out.join("locales"))?;
    locale_versions
        .as_object_mut()
        .context("invalid asset-version map")?
        .retain(|key, _| key.starts_with("/locales/"));
    let locale_versions = serde_json::to_string(&locale_versions)?;
    for relative in PUBLIC_SITE_PAGES {
        let path = out.join(relative);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read public page {}", path.display()))?;
        let source = if relative == "index.html" {
            expand_landing(&source, out)?
        } else {
            source
        };
        if source.matches(marker).count() != 1 {
            bail!(
                "public page {} must contain one site header marker",
                path.display()
            );
        }
        let head_end = source.find("</head>").context("public page has no head")?;
        if source.matches(locale_marker).count() != 1
            || source.find(locale_marker).is_none_or(|at| at > head_end)
        {
            bail!("public page {relative} must select its locale in the head");
        }
        let prefix = if relative == "index.html" {
            "./"
        } else {
            "../"
        };
        let locale_bootstrap = format!(
            "<script data-cfasync=\"false\">window.SOW_LOCALE_BEFORE_PAINT = true; window.SOW_LOCALE_CATALOG_VERSION = {}; window.SOW_LOCALE_CODES = {locale_codes}; window.SOW_ASSET_TS = {locale_versions};</script>\n  <script data-cfasync=\"false\" src=\"{prefix}sow-i18n.js\"></script>",
            sow_i18n::WEB_CATALOG_VERSION,
        );
        fs::write(
            &path,
            source
                .replace(marker, &header)
                .replace(locale_marker, &locale_bootstrap),
        )?;
    }
    Ok(())
}

/// Landing sections built from data at publish time: the hero roster from the
/// character catalog, the maps in rotation and the optional captured videos.
/// A hero or map added to the game appears here without touching the page.
fn expand_landing(source: &str, out: &Path) -> Result<String> {
    let english = serde_json::to_value(sow_i18n::web(sow_i18n::Language::English))?;
    let media = out.join("assets/site/media");
    let sections = [
        ("<!-- SOW_SITE_HERO_MEDIA -->", landing_hero_media(&media)),
        (
            "<!-- SOW_SITE_LEADER_SHOWCASE -->",
            landing_leader_showcase(&english)?,
        ),
        ("<!-- SOW_SITE_LEADERS -->", landing_leaders(&english)?),
        ("<!-- SOW_SITE_MAPS -->", landing_maps(&out.join("maps"))?),
        (
            "<!-- SOW_SITE_TRAILER -->",
            landing_trailer(&media, &english)?,
        ),
    ];
    let mut page = source.to_string();
    for (marker, html) in sections {
        if page.matches(marker).count() != 1 {
            bail!("the landing page must contain one {marker}");
        }
        page = page.replace(marker, &html);
    }
    Ok(page)
}

fn html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// English fallback shown before the visitor's language loads.
fn english_text(english: &serde_json::Value, key: &str) -> Result<String> {
    web_catalog_value(english, key)
        .and_then(serde_json::Value::as_str)
        .map(html_text)
        .with_context(|| format!("English text is missing: {key}"))
}

fn landing_hero_media(media: &Path) -> String {
    if media.join("hero-loop.webm").is_file() {
        r#"<video class="hero-video" loop muted playsinline preload="metadata" aria-hidden="true" poster="/assets/site/media/hero-loop.webp"><source src="/assets/site/media/hero-loop.webm" type="video/webm"><source src="/assets/site/media/hero-loop.mp4" type="video/mp4"></video>"#.to_string()
    } else {
        r#"<picture class="hero-still"><source media="(max-width: 700px)" srcset="/assets/shell/loader/sow-splash-mobile.webp"><img src="/assets/shell/loader/sow-splash-desktop.webp" alt="" fetchpriority="high" decoding="async"></picture>"#.to_string()
    }
}

fn landing_leader_showcase(english: &serde_json::Value) -> Result<String> {
    let id = sow_core::player::Leader::default().id();
    let name = english_text(english, &format!("heroes.leader_{id}_name"))?;
    let historical = english_text(english, &format!("heroes.leader_{id}_historical"))?;
    Ok(format!(
        r#"<picture class="showcase-art"><source media="(max-width: 700px)" srcset="/assets/shell/leaders/{id}_mobile.webp" data-detail-image-mobile><img data-detail-image src="/assets/shell/leaders/{id}_desktop.webp" alt="{name}" decoding="async"></picture>
          <div class="showcase-copy">
            <p class="eyebrow" data-detail-civ data-i18n="heroes.leader_{id}_civilization">{civ}</p>
            <h3 data-detail-name data-i18n="heroes.leader_{id}_name">{name}</h3>
            <p class="showcase-historical" data-detail-historical data-i18n="heroes.leader_{id}_historical"{hidden}>{historical}</p>
            <p class="showcase-label" data-i18n="site.command_trait">Command trait</p>
            <p class="showcase-ability" data-detail-ability data-i18n="site.leader_{id}_ability">{ability}</p>
            <p class="showcase-description" data-detail-description data-i18n="site.leader_{id}_description">{description}</p>
          </div>"#,
        civ = english_text(english, &format!("heroes.leader_{id}_civilization"))?,
        hidden = if historical == name { " hidden" } else { "" },
        ability = english_text(english, &format!("site.leader_{id}_ability"))?,
        description = english_text(english, &format!("site.leader_{id}_description"))?,
    ))
}

fn landing_leaders(english: &serde_json::Value) -> Result<String> {
    let mut html = String::new();
    for leader in sow_core::player::Leader::all() {
        let id = leader.id();
        let active = leader == sow_core::player::Leader::default();
        html += &format!(
            r#"<button class="roster-card{class}" type="button" aria-pressed="{active}" data-leader-id="{id}" data-name="{name}" data-civ="{civ}" data-historical="{historical}" data-ability="{ability}" data-description="{description}"><img src="/assets/gameplay/avatars/{id}.webp" alt="" width="128" height="128" loading="lazy" decoding="async"><span data-i18n="heroes.leader_{id}_name">{name}</span></button>
"#,
            class = if active { " is-active" } else { "" },
            name = english_text(english, &format!("heroes.leader_{id}_name"))?,
            civ = english_text(english, &format!("heroes.leader_{id}_civilization"))?,
            historical = english_text(english, &format!("heroes.leader_{id}_historical"))?,
            ability = english_text(english, &format!("site.leader_{id}_ability"))?,
            description = english_text(english, &format!("site.leader_{id}_description"))?,
        );
    }
    Ok(html)
}

fn landing_maps(maps: &Path) -> Result<String> {
    let catalog = sow_core::map_file::parse_catalog(&fs::read(maps.join("catalog.bin"))?)
        .map_err(|error| anyhow::anyhow!("read map catalog: {error}"))?;
    let bust = thumbnail_cache_bust(maps)?;
    let html: String = catalog
        .entries
        .iter()
        .filter(|entry| {
            entry.multiplayer_frequency > 0 && maps.join(&entry.key).join("thumbnail.webp").is_file()
        })
        .map(|entry| {
            format!(
                r#"<li class="map-card"><img src="/maps/{key}/thumbnail.webp?v={bust}" alt="{name}" width="512" height="288" loading="lazy" decoding="async"><span>{name}</span></li>
"#,
                key = html_text(&entry.key),
                name = html_text(&entry.display_name),
            )
        })
        .collect();
    if html.is_empty() {
        bail!("no map in rotation has a thumbnail for the landing page");
    }
    Ok(html)
}

fn landing_trailer(media: &Path, english: &serde_json::Value) -> Result<String> {
    if !media.join("gameplay.webm").is_file() {
        return Ok(String::new());
    }
    Ok(format!(
        r#"<section class="landing-section trailer" id="media">
      <div class="container">
        <header class="section-head">
          <p class="eyebrow" data-i18n="site.gameplay">{eyebrow}</p>
          <h2><span data-i18n="site.watch">{watch}</span> <em data-i18n="site.the_game">{game}</em></h2>
          <p data-i18n="site.media_summary">{summary}</p>
        </header>
        <video class="trailer-video" controls playsinline preload="none" poster="/assets/site/media/gameplay.webp" aria-label="{label}" data-i18n="site.gameplay_capture" data-i18n-attr="aria-label">
          <source src="/assets/site/media/gameplay.webm" type="video/webm">
          <source src="/assets/site/media/gameplay.mp4" type="video/mp4">
        </video>
      </div>
    </section>"#,
        eyebrow = english_text(english, "site.gameplay")?,
        watch = english_text(english, "site.watch")?,
        game = english_text(english, "site.the_game")?,
        summary = english_text(english, "site.media_summary")?,
        label = english_text(english, "site.gameplay_capture")?,
    ))
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

fn write_manifest(
    paths: &Paths,
    out: &Path,
    version: &str,
    js: &str,
    wasm: &str,
    ts: &str,
) -> Result<()> {
    let source = source_identity(paths)?;
    fs::write(
        out.join("game-manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "js": js,
            "wasm": wasm,
            "build_ts": ts,
            "version": version,
            "source_sha": source.sha,
            "source_url": source.url,
        }))?,
    )?;
    Ok(())
}

fn placeholder_names(value: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut offset = 0;
    while offset < value.len() {
        let remainder = &value[offset..];
        let open = remainder.find('{');
        let close = remainder.find('}');
        match (open, close) {
            (None, None) => break,
            (None, Some(_)) => bail!("unmatched closing placeholder brace"),
            (Some(open), Some(close)) if close < open => {
                bail!("unmatched closing placeholder brace")
            }
            (Some(open), _) => {
                let after_open = &remainder[open + 1..];
                let end = after_open.find('}').context("unterminated placeholder")?;
                let name = after_open[..end].trim();
                if name.is_empty()
                    || !name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
                {
                    bail!("invalid placeholder {name:?}");
                }
                names.push(name.to_string());
                offset += open + 1 + end + 1;
            }
        }
    }
    names.sort();
    Ok(names)
}

const APPROVED_CAMPAIGN_TRANSLATION_EPISODES: &[&str] = &[];
const APPROVED_CAMPAIGN_TRANSLATION_KEYS: &[&str] = &[
    "campaign_boudica_boudica_city_upgrade_ready_body",
    "campaign_boudica_boudica_city_upgrade_ready_title",
    "campaign_boudica_boudica_structure_upgrade_hint",
    "campaign_boudica_boudica_structure_upgrade_title",
];

const CAMPAIGN_TRANSLATION_NAMESPACES: &[(&str, &str)] = &[
    ("boudica", "boudica_"),
    ("boudica", "campaign_boudica_"),
    ("six_sky_ep1", "campaign_six_sky_ep1_"),
    ("six_sky_ep2", "campaign_six_sky_ep2_"),
    ("six_sky_ep3", "campaign_six_sky_ep3_"),
    ("six_sky_ep1", "six_sky_ep1_"),
    ("six_sky_ep2", "six_sky_ep2_"),
    ("six_sky_ep3", "six_sky_ep3_"),
];

fn is_campaign_translation_key(key: &str) -> bool {
    if matches!(
        key,
        "campaign_episode_locked"
            | "campaign_chronicles"
            | "campaign_unavailable"
            | "campaign_unlock_required"
    ) || key.starts_with("campaign_menu_guide_")
        || key.starts_with("campaign_studio_demo_")
    {
        return false;
    }
    key.starts_with("campaign_") || key.starts_with("boudica_") || key.starts_with("six_sky_ep")
}

fn is_locked_campaign_translation_key(key: &str, approved_episodes: &[&str]) -> bool {
    if !is_campaign_translation_key(key) || APPROVED_CAMPAIGN_TRANSLATION_KEYS.contains(&key) {
        return false;
    }
    let episode_id = CAMPAIGN_TRANSLATION_NAMESPACES
        .iter()
        .find_map(|(episode_id, prefix)| key.starts_with(prefix).then_some(*episode_id));
    episode_id.map_or(true, |episode_id| !approved_episodes.contains(&episode_id))
}

fn is_non_english_catalog_path(path: &str) -> bool {
    let locale = path.split('.').next().unwrap_or(path);
    sow_i18n::Language::registry()
        .iter()
        .any(|(_, code, _, _)| {
            code.eq_ignore_ascii_case(locale) && !code.eq_ignore_ascii_case("en")
        })
}

fn validate_web_node(
    path: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> Result<()> {
    match (expected, actual) {
        (serde_json::Value::Object(expected), serde_json::Value::Object(actual)) => {
            for (key, expected_value) in expected {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                let catalog_key = child_path.splitn(2, '.').nth(1).unwrap_or(&child_path);
                let actual_value = match actual.get(key) {
                    Some(value) => value,
                    None if is_non_english_catalog_path(path)
                        && is_locked_campaign_translation_key(
                            key,
                            APPROVED_CAMPAIGN_TRANSLATION_EPISODES,
                        ) =>
                    {
                        continue;
                    }
                    None if path.ends_with(".tutorial")
                        && key.starts_with("campaign_")
                        && is_non_english_catalog_path(path) =>
                    {
                        continue;
                    }
                    None if path.ends_with(".site")
                        && web_key_is_intentionally_english(catalog_key)
                        && !path
                            .split('.')
                            .next()
                            .unwrap_or("en")
                            .eq_ignore_ascii_case("en") =>
                    {
                        continue;
                    }
                    None if path.ends_with(".menu")
                        && web_key_is_intentionally_english(catalog_key)
                        && !path
                            .split('.')
                            .next()
                            .unwrap_or("en")
                            .eq_ignore_ascii_case("en") =>
                    {
                        continue;
                    }
                    None => bail!("web catalog missing key {child_path}"),
                };
                validate_web_node(&child_path, expected_value, actual_value)?;
            }
            for key in actual.keys() {
                if is_non_english_catalog_path(path)
                    && is_locked_campaign_translation_key(
                        key,
                        APPROVED_CAMPAIGN_TRANSLATION_EPISODES,
                    )
                {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    bail!("web catalog contains locked campaign text {child_path}");
                }
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
            if placeholder_names(expected)
                .with_context(|| format!("invalid placeholder at {path}"))?
                != placeholder_names(actual)
                    .with_context(|| format!("invalid placeholder at {path}"))?
            {
                bail!("web catalog placeholder mismatch at {path}");
            }
            Ok(())
        }
        _ => bail!("web catalog shape mismatch at {path}"),
    }
}

fn web_key_is_intentionally_english(key: &str) -> bool {
    key.starts_with("menu.language_")
        // Hero names are proper names in every language.
        || (key.starts_with("heroes.leader_")
            && (key.ends_with("_name")
                || key.ends_with("_historical")
                || key.ends_with("_civilization")))
        || matches!(
            key,
            "auth.discord_provider"
                | "auth.email_placeholder"
                | "auth.google"
                | "auth.meta"
                | "auth.x_provider"
                | "endgame.xp"
                | "heroes.africa"
                | "heroes.americas"
                | "heroes.random_leader"
                | "heroes.asia"
                | "heroes.leader_leonidas_civilization"
                | "heroes.europe"
                | "hud.ability_notice"
                | "hud.fps"
                | "hud.fps_ping"
                | "hud.nuke_struck"
                | "hud.pin"
                | "lobbies.hvn"
                | "lobbies.terminator"
                | "menu.brand"
                | "menu.cookies"
                | "menu.cookie_settings"
                | "menu.discord"
                | "menu.github"
                | "menu.google_play_games"
                | "menu.level_short"
                | "menu.source_code"
                | "menu.telegram"
                | "menu.xp"
                | "profile.android"
                | "profile.ffa"
                | "profile.google_play_games"
                | "profile.kda"
                | "profile.kda_value"
                | "profile.spam"
                | "site.copyright"
                | "site.cookies"
                | "site.discord"
                | "site.faq"
                | "site.github"
                | "site.leader_artigas_ability"
                | "site.leader_napoleon_ability"
                | "site.leader_tlacaelel_ability"
                | "site.terms_maps_license"
                | "site.telegram"
        )
}

fn validate_leader_display_names(catalog: &serde_json::Value) -> Result<()> {
    for slug in sow_core::player::Leader::all().map(sow_core::player::Leader::id) {
        let key = format!("heroes.leader_{slug}_name");
        let value = web_catalog_value(catalog, &key)
            .and_then(serde_json::Value::as_str)
            .with_context(|| format!("dynamic localization key is missing: {key}"))?;
        if [" / ", " · ", " | "]
            .iter()
            .any(|separator| value.contains(separator))
        {
            bail!("leader display name contains a compound separator: {key}");
        }
    }
    Ok(())
}

fn collect_equal_web_values(
    path: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    values: &mut Vec<(String, String)>,
) {
    match (expected, actual) {
        (serde_json::Value::Object(expected), serde_json::Value::Object(actual)) => {
            for (key, expected_value) in expected {
                if let Some(actual_value) = actual.get(key) {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    collect_equal_web_values(&child_path, expected_value, actual_value, values);
                }
            }
        }
        (serde_json::Value::String(expected), serde_json::Value::String(actual))
            if expected == actual =>
        {
            values.push((path.to_string(), actual.clone()));
        }
        _ => {}
    }
}

fn validate_web_catalogs(report_equal_values: bool) -> Result<serde_json::Value> {
    let english = serde_json::to_value(&sow_i18n::web(sow_i18n::Language::English))?;
    let mut untranslated = Vec::new();
    for &(language, code, _, _) in sow_i18n::Language::registry() {
        let catalog = serde_json::to_value(&sow_i18n::web(language))?;
        validate_web_node(code, &english, &catalog)
            .with_context(|| format!("validate web catalog {code}"))?;
        validate_leader_display_names(&catalog)
            .with_context(|| format!("validate leader display names for {code}"))?;
        if code != "en" {
            let mut equal_values = Vec::new();
            collect_equal_web_values("", &english, &catalog, &mut equal_values);
            let intentional = equal_values
                .iter()
                .filter(|(key, _)| web_key_is_intentionally_english(key))
                .count();
            let pending = equal_values.len() - intentional;
            if report_equal_values {
                println!(
                    "i18n audit {code}: {} equal values ({} intentional, {} require translation)",
                    equal_values.len(),
                    intentional,
                    pending
                );
            }
            untranslated.extend(
                equal_values
                    .into_iter()
                    .filter(|(key, _)| !web_key_is_intentionally_english(key))
                    .map(|(key, value)| format!("{code}: {key} = {value}")),
            );
        }
    }
    if !untranslated.is_empty() {
        for value in &untranslated {
            eprintln!("i18n untranslated: {value}");
        }
        bail!(
            "{} visible localization values still equal English",
            untranslated.len()
        );
    }
    Ok(english)
}

fn validate_dynamic_web_keys(catalog: &serde_json::Value) -> Result<()> {
    for key in ["menu.loading"] {
        if web_catalog_value(catalog, key).is_none() {
            bail!("dynamic localization key is missing: {key}");
        }
    }
    for &(_, code, _, _) in sow_i18n::Language::registry() {
        let token = code.to_ascii_lowercase().replace('-', "_");
        let key = format!("menu.language_{token}");
        if web_catalog_value(catalog, &key).is_none() {
            bail!("dynamic localization key is missing: {key}");
        }
    }
    for tab in ["overview", "commanders", "history", "achievements"] {
        let key = format!("profile.tab_{tab}");
        if web_catalog_value(catalog, &key).is_none() {
            bail!("dynamic localization key is missing: {key}");
        }
    }
    for category in ["battles", "victories", "commanders", "laurels"] {
        let key = format!("profile.achievement_category_{category}");
        if web_catalog_value(catalog, &key).is_none() {
            bail!("dynamic localization key is missing: {key}");
        }
    }
    for id in [
        "first_command",
        "first_victory",
        "battle_hardened",
        "victory_march",
        "laurel_hoard",
        "commander_victorious",
        "veteran_commander",
        "banner_collector",
        "leader_path",
    ] {
        for suffix in ["title", "description"] {
            let key = format!("profile.achievement_{id}_{suffix}");
            if web_catalog_value(catalog, &key).is_none() {
                bail!("dynamic localization key is missing: {key}");
            }
        }
    }
    for slug in sow_core::player::Leader::all().map(sow_core::player::Leader::id) {
        for key in ["name", "historical", "civilization"]
            .map(|suffix| format!("heroes.leader_{slug}_{suffix}"))
            .into_iter()
            .chain(["ability", "description"].map(|suffix| format!("site.leader_{slug}_{suffix}")))
        {
            if web_catalog_value(catalog, &key).is_none() {
                bail!("dynamic localization key is missing: {key}");
            }
        }
    }
    Ok(())
}

fn locale_folder(code: &str) -> String {
    code.to_ascii_lowercase()
}

fn validate_current_ui_contract(paths: &Paths) -> Result<()> {
    let root = &paths.root;
    let catalog = validate_web_catalogs(true)?;
    validate_dynamic_web_keys(&catalog)?;
    for definition in sow_core::emote::EMOTES {
        if web_catalog_value(&catalog, definition.bubble_key).and_then(serde_json::Value::as_str)
            != Some(definition.fallback)
        {
            bail!(
                "emote fallback differs from its English catalog: {}",
                definition.bubble_key
            );
        }
        for key in [
            definition.menu_key,
            definition.bubble_key,
            definition.notice_key,
        ] {
            if web_catalog_value(&catalog, key).is_none() {
                bail!("emote definition uses unknown localization key {key}");
            }
        }
    }
    let strings_root = root.join("sow-i18n/strings");
    let registered = sow_i18n::Language::registry()
        .iter()
        .map(|(_, code, _, _)| locale_folder(code))
        .collect::<HashSet<_>>();
    let mut found = HashSet::new();
    for entry in
        fs::read_dir(&strings_root).with_context(|| format!("read {}", strings_root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            bail!(
                "web locale entry is not a directory: {}",
                entry.path().display()
            );
        }
        let code = entry.file_name().to_string_lossy().into_owned();
        if !registered.contains(&code) {
            bail!("unregistered web locale directory: {code}");
        }
        let files = fs::read_dir(entry.path())?
            .map(|entry| {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    bail!("web locale entry is not a file: {}", entry.path().display());
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let is_timestamped_backup = name
                    .strip_prefix("web.toml.bak_")
                    .is_some_and(|suffix| {
                        !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
                    });
                if name != "web.toml" && !is_timestamped_backup {
                    bail!("unexpected web locale file: {}/{}", code, name);
                }
                Ok(name)
            })
            .collect::<Result<Vec<_>>>()?;
        if files
            .iter()
            .filter(|name| name.as_str() == "web.toml")
            .count()
            != 1
        {
            bail!("web locale {code} must contain exactly one catalog");
        }
        found.insert(code);
    }
    for &(_, code, _, _) in sow_i18n::Language::registry() {
        if !found.contains(&locale_folder(code)) {
            bail!("registered web locale is missing: {code}");
        }
    }

    let source_root = root.join("sow-client/src");
    for entry in walkdir::WalkDir::new(&source_root) {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("rs")
        {
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
                if path.ends_with("input/map_click.rs") && trimmed.starts_with("key)") {
                    offset = start;
                    continue;
                }
                bail!("UiText::new requires a literal key in {}", path.display());
            }
            let value = &trimmed[1..];
            let end = value
                .find(quote as char)
                .with_context(|| format!("unterminated UiText key in {}", path.display()))?;
            let key = &value[..end];
            if web_catalog_value(&catalog, key).is_none() {
                bail!(
                    "client source uses unknown localization key {key}: {}",
                    path.display()
                );
            }
            let key_start = start + rest.len() - trimmed.len() + 1;
            offset = key_start + end;
        }
        validate_quoted_catalog_keys(&source, &path.display().to_string(), &catalog)?;
    }
    let placement_source_path = root.join("sow-core/src/building/placement.rs");
    let placement_source = fs::read_to_string(&placement_source_path)
        .with_context(|| format!("read placement source {}", placement_source_path.display()))?;
    validate_quoted_catalog_keys(
        &placement_source,
        &placement_source_path.display().to_string(),
        &catalog,
    )?;
    for entry in walkdir::WalkDir::new(root.join("sow-web/site")) {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file()
            || !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("html") | Some("js")
            )
        {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".test.js"))
        {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read public source {}", path.display()))?;
        validate_source_localization_keys(&source, &relative, &catalog)?;
    }
    let shell_root = root.join("sow-web/shell");
    for entry in walkdir::WalkDir::new(&shell_root) {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("js")
        {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".test.js"))
        {
            continue;
        }
        let source = fs::read_to_string(path)
            .with_context(|| format!("read shell source {}", path.display()))?;
        validate_source_localization_keys(&source, &path.display().to_string(), &catalog)?;
    }
    validate_campaign_assets(paths, &catalog)?;
    Ok(())
}

fn verify_exported_locales(dir: &Path) -> Result<()> {
    let expected = serde_json::to_value(&sow_i18n::web(sow_i18n::Language::English))?;
    let expected_languages = serde_json::to_value(
        sow_i18n::Language::published_registry()
            .map(|(_, code, name)| serde_json::json!({ "code": code, "name": name }))
            .collect::<Vec<_>>(),
    )?;
    let registry_path = dir.join("locales/index.json");
    let registry: serde_json::Value = serde_json::from_str(&fs::read_to_string(&registry_path)?)
        .with_context(|| format!("parse exported locale registry {}", registry_path.display()))?;
    if registry.get("schema").and_then(serde_json::Value::as_u64) != Some(1)
        || registry.get("version").and_then(serde_json::Value::as_u64)
            != Some(sow_i18n::WEB_CATALOG_VERSION as u64)
        || registry.get("languages") != Some(&expected_languages)
    {
        bail!("exported locale registry metadata is invalid");
    }
    for (_, code, _) in sow_i18n::Language::published_registry() {
        let folder = locale_folder(code);
        let path = dir.join("locales").join(&folder);
        if !path.is_file() {
            bail!("missing exported web catalog {code}");
        }
        let payload: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path)?)
            .with_context(|| format!("parse exported web catalog {code}"))?;
        if payload.get("schema").and_then(serde_json::Value::as_u64) != Some(1)
            || payload.get("version").and_then(serde_json::Value::as_u64)
                != Some(sow_i18n::WEB_CATALOG_VERSION as u64)
            || payload.get("locale").and_then(serde_json::Value::as_str) != Some(folder.as_str())
        {
            bail!("exported web catalog metadata is invalid for {code}");
        }
        if payload.get("languages") != Some(&expected_languages) {
            bail!("exported web catalog language registry is invalid for {code}");
        }
        let strings = payload
            .get("strings")
            .with_context(|| format!("exported web catalog {code} has no strings"))?;
        validate_web_node(&folder, &expected, strings)?;
    }
    Ok(())
}

/// Every cinematic ships a WebM + MP4 pair with a poster under the episode's
/// own folder, small enough to start instantly and muxed with its index at
/// the front so the browser plays without fetching the tail of the file.
fn validate_cinematic_media(
    dir: &Path,
    episode_id: &str,
    definition: &serde_json::Value,
) -> Result<()> {
    const MAX_RENDITION_BYTES: u64 = 6 * 1024 * 1024;
    let Some(steps) = definition
        .get("steps")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    for step in steps {
        let video_src = step.get("video_src").and_then(serde_json::Value::as_str);
        if step.get("presentation").and_then(serde_json::Value::as_str) != Some("cinematic")
            || video_src.is_none()
        {
            continue;
        }
        let video_src = video_src.unwrap();
        let prefix = format!("/assets/campaign/{episode_id}/");
        let Some(relative) = video_src.strip_prefix(&prefix) else {
            bail!("campaign cinematic video_src must live under {prefix}: {episode_id}");
        };
        let video_path = dir.join(episode_id).join(relative);
        let stem = video_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        for extension in ["webm", "mp4"] {
            let path = video_path.with_file_name(format!("{stem}.{extension}"));
            if !path.is_file() {
                bail!(
                    "campaign cinematic rendition missing for {episode_id}: {}",
                    path.display()
                );
            }
            let size = fs::metadata(&path)?.len();
            if size > MAX_RENDITION_BYTES {
                bail!(
                    "campaign cinematic rendition is too heavy for instant start ({size} bytes, max {MAX_RENDITION_BYTES}): {}",
                    path.display()
                );
            }
            let mut head = fs::File::open(&path)?;
            let mut window = Vec::new();
            std::io::Read::by_ref(&mut head)
                .take(1024 * 1024)
                .read_to_end(&mut window)?;
            let streamable = match extension {
                "webm" => window
                    .windows(4)
                    .position(|bytes| bytes == [0x1C, 0x53, 0xBB, 0x6B])
                    .is_some_and(|position| position < 16 * 1024),
                _ => match (
                    window.windows(4).position(|bytes| bytes == *b"moov"),
                    window.windows(4).position(|bytes| bytes == *b"mdat"),
                ) {
                    (Some(moov), Some(mdat)) => moov < mdat,
                    (Some(_), None) => true,
                    _ => false,
                },
            };
            if !streamable {
                bail!(
                    "campaign cinematic must ship its index at the front (-cues_to_front 1 / +faststart) so playback never waits for the whole file: {}",
                    path.display()
                );
            }
        }
        let poster = video_path.with_file_name(format!("{stem}.poster.webp"));
        if !poster.is_file() {
            bail!(
                "campaign cinematic poster missing for {episode_id}: {}",
                poster.display()
            );
        }
        if fs::metadata(&poster)?.len() > 128 * 1024 {
            bail!(
                "campaign cinematic poster is too heavy: {}",
                poster.display()
            );
        }
    }
    Ok(())
}

fn validate_campaign_assets(paths: &Paths, catalog: &serde_json::Value) -> Result<()> {
    let dir = paths.root.join("assets/campaign");
    let mut rosters = HashMap::new();
    let mut triggers = HashMap::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if let Some(id) = name.strip_suffix(".triggers.json") {
            triggers.insert(id.to_string(), path);
        } else if let Some(id) = name.strip_suffix(".json") {
            rosters.insert(id.to_string(), path);
        }
    }
    for episode_id in triggers.keys() {
        if !rosters.contains_key(episode_id) {
            bail!("campaign triggers have no roster: {episode_id}");
        }
    }
    let mut episodes = Vec::new();
    for (episode_id, roster_path) in &rosters {
        let roster: serde_json::Value = serde_json::from_str(&fs::read_to_string(roster_path)?)
            .with_context(|| format!("parse {}", roster_path.display()))?;
        let map_id = roster
            .get("map")
            .and_then(serde_json::Value::as_str)
            .filter(|id| {
                !id.is_empty()
                    && id.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'-' | b'_')
                    })
            })
            .with_context(|| format!("campaign roster has invalid map id: {episode_id}"))?;
        let map_path = paths.assets_maps.join(map_id).join("map.bin.br");
        if !map_path.is_file() {
            bail!(
                "campaign map asset missing: {episode_id} -> {map_id} ({})",
                map_path.display()
            );
        }
        let trigger_path = triggers
            .get(episode_id)
            .with_context(|| format!("missing triggers for {episode_id}"))?;
        let definition: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(trigger_path)?)
                .with_context(|| format!("parse {}", trigger_path.display()))?;
        if definition
            .get("episode_id")
            .and_then(serde_json::Value::as_str)
            != Some(episode_id.as_str())
        {
            bail!("campaign episode_id does not match filename: {episode_id}");
        }
        if let Some(speakers) = definition
            .get("speakers")
            .and_then(serde_json::Value::as_object)
        {
            for (speaker, definition) in speakers {
                let Some(avatar) = definition.get("avatar").and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                if avatar.is_empty() {
                    continue;
                }
                if !avatar.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '_' || character == '-'
                }) || !paths
                    .assets_gameplay
                    .join("avatars")
                    .join(format!("{avatar}.webp"))
                    .is_file()
                {
                    bail!(
                        "campaign speaker avatar is invalid or missing: {episode_id}.{speaker}: {avatar}"
                    );
                }
            }
        }
        validate_cinematic_media(&dir, episode_id, &definition)?;
        episodes.push(serde_json::json!({
            "episode_id": episode_id,
            "definition": definition,
            "roster": roster,
        }));
    }
    // Episode-local strings and flow rules belong to the browser/editor's shared validator.
    let mut validator = Command::new("node")
        .args([
            "-e",
            r#"
const fs = require("node:fs");
const campaign = require(process.argv[1]);
const { catalog, episodes, menuGuides } = JSON.parse(fs.readFileSync(0, "utf8"));
const hasText = key => {
    const value = key.split(".").reduce((node, part) =>
        node && Object.prototype.hasOwnProperty.call(node, part) ? node[part] : undefined, catalog);
    return typeof value === "string" && value.trim().length > 0;
};
for (const { episode_id, definition, roster } of episodes) {
    const result = campaign.validate(definition, roster, { hasText });
    for (const issue of result.errors) {
        console.error([episode_id, issue.step, issue.field].filter(Boolean).join(".") + ": " + issue.message);
    }
    for (const issue of result.warnings) {
        console.warn(episode_id + ": " + issue.message);
    }
    if (result.errors.length) process.exitCode = 1;
}
const menuReport = campaign.validateMenuGuides(menuGuides);
for (const issue of menuReport.errors) {
    console.error(["menu", issue.step, issue.field].filter(Boolean).join(".") + ": " + issue.message);
}
if (menuReport.errors.length) process.exitCode = 1;
"#,
        ])
        .arg(paths.shell.join("sow-campaign.js"))
        .stdin(Stdio::piped())
        .spawn()
        .context("start shared campaign validator (Node.js required)")?;
    serde_json::to_writer(
        validator
            .stdin
            .take()
            .context("open campaign validator input")?,
        &serde_json::json!({ "catalog": catalog, "episodes": episodes,
            "menuGuides": serde_json::from_str::<serde_json::Value>(&fs::read_to_string(
                paths.root.join("assets/guides/menu.triggers.json"))?)? }),
    )
    .context("write campaign validator input")?;
    if !validator
        .wait()
        .context("wait for shared campaign validator")?
        .success()
    {
        bail!("shared campaign validation failed");
    }
    Ok(())
}

fn export_locales(out: &Path) -> Result<()> {
    validate_web_catalogs(false)?;
    let d = out.join("locales");
    fs::create_dir_all(&d)?;
    let languages = sow_i18n::Language::published_registry()
        .map(|(_, code, name)| serde_json::json!({ "code": code, "name": name }))
        .collect::<Vec<_>>();
    fs::write(
        d.join("index.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": 1,
            "version": sow_i18n::WEB_CATALOG_VERSION,
            "languages": languages.clone(),
        }))?,
    )?;
    for (language, code, _) in sow_i18n::Language::published_registry() {
        let folder = locale_folder(code);
        let payload = serde_json::json!({
            "schema": 1,
            "version": sow_i18n::WEB_CATALOG_VERSION,
            "locale": folder.clone(),
            "languages": languages.clone(),
            "strings": sow_i18n::web(language),
        });
        fs::write(d.join(folder), serde_json::to_string_pretty(&payload)?)?;
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
        "llms.txt",
        "app.js",
        "site-chrome.js",
        "sow-i18n.js",
        "sow-dropdown.js",
        "sow-controls.css",
        "locales/index.json",
        "styles.css",
        "fonts/fonts.css",
        "fonts/OFL.txt",
        "fonts/work-sans-latin.woff2",
        "fonts/work-sans-latin-ext.woff2",
        "fonts/work-sans-italic-latin.woff2",
        "fonts/work-sans-italic-latin-ext.woff2",
        "wou-auth.js",
        "privacy/index.html",
        "terms/index.html",
        "cookies/index.html",
        "support/index.html",
        "8d227b8f9e6140d39e3381a1829e1db3.txt",
        "sow.svg",
        "LICENSE",
        "NOTICE",
        "LICENSE-ASSETS",
        "WASM-THIRD-PARTY-NOTICES.zip",
        "manifest.webmanifest",
        "assets/gameplay/buildings/atlas.webp",
        "assets/gameplay/buildings/atlas.json",
        "assets/gameplay/music/shadows-of-war.webm",
        "maps/NOTICE",
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
    for relative in PUBLIC_SITE_PAGES {
        let html = fs::read_to_string(dir.join(relative))?;
        let (head, _) = html
            .split_once("</head>")
            .context("public page has no head")?;
        if !head.contains("window.SOW_LOCALE_BEFORE_PAINT = true")
            || !head.contains("window.SOW_LOCALE_CODES = [")
            || !head.contains("data-cfasync=\"false\" src=\"")
            || !head.contains("sow-i18n.js?v=")
        {
            bail!("public page {relative} is missing its early locale bootstrap");
        }
    }
    let play_html = fs::read_to_string(dir.join("play/index.html"))?;
    // Self-hosted web externalizes the menu bundle next to the page; the
    // registries it must carry live there instead of inside the HTML.
    let menu_bundle = fs::read_dir(dir.join("play"))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("main_menu") && n.ends_with(".js"))
        .map(|n| fs::read_to_string(dir.join("play").join(n)).unwrap_or_default())
        .collect::<String>();
    let shell_page = format!("{play_html}{menu_bundle}");
    if !play_html.contains("name=\"sow-media-artwork\" content=\"../icon-192.png\"") {
        bail!("web game page has no game music artwork reference");
    }
    if !shell_page.contains("window.SOW_BUILDING_ART =")
        || !shell_page.contains("gameplay/buildings/atlas.webp")
    {
        bail!("webroot game page is missing the building-art registry");
    }
    println!("✅ Dist layout OK ({})", dir.display());
    Ok(())
}

fn html_attribute_value<'a>(tag: &'a str, wanted: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut index = 1;
    while index < bytes.len()
        && !bytes[index].is_ascii_whitespace()
        && !matches!(bytes[index], b'/' | b'>')
    {
        index += 1;
    }

    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || matches!(bytes[index], b'/' | b'>') {
            break;
        }

        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(bytes[index], b'=' | b'/' | b'>')
        {
            index += 1;
        }
        if name_start == index {
            index += 1;
            continue;
        }
        let name = &tag[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || matches!(bytes[index], b'/' | b'>') {
            if name.eq_ignore_ascii_case(wanted) {
                return Some("");
            }
            break;
        }
        if bytes[index] != b'=' {
            if name.eq_ignore_ascii_case(wanted) {
                return Some("");
            }
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }

        let value = if index < bytes.len() && matches!(bytes[index], b'\'' | b'"') {
            let quote = bytes[index];
            index += 1;
            let value_start = index;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
            let value = &tag[value_start..index];
            if index < bytes.len() {
                index += 1;
            }
            value
        } else {
            let value_start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'>'
            {
                index += 1;
            }
            &tag[value_start..index]
        };
        if name.eq_ignore_ascii_case(wanted) {
            return Some(value);
        }
    }
    None
}

fn verify_cg_local_resources(dir: &Path, html: &str) -> Result<()> {
    let bytes = html.as_bytes();
    let mut offset = 0;
    while let Some(relative_start) = html[offset..].find('<') {
        let start = offset + relative_start;
        if html[start..].starts_with("<!--") {
            let end = html[start + 4..]
                .find("-->")
                .context("crazygames index.html has an unterminated comment")?;
            offset = start + 4 + end + 3;
            continue;
        }

        let mut quote = None;
        let mut end = start + 1;
        while end < bytes.len() {
            match (quote, bytes[end]) {
                (Some(current), byte) if current == byte => quote = None,
                (None, b'\'' | b'"') => quote = Some(bytes[end]),
                (None, b'>') => break,
                _ => {}
            }
            end += 1;
        }
        if end >= bytes.len() {
            bail!("crazygames index.html has an unterminated tag");
        }
        let tag = &html[start..=end];
        let mut name_end = 1;
        while name_end < tag.len()
            && !tag.as_bytes()[name_end].is_ascii_whitespace()
            && !matches!(tag.as_bytes()[name_end], b'/' | b'>')
        {
            name_end += 1;
        }
        let name = tag[1..name_end].to_ascii_lowercase();
        let attributes: &[&str] = match name.as_str() {
            "link" => &["href"],
            "script" | "img" | "iframe" | "embed" | "source" | "audio" | "track" => &["src"],
            "video" => &["src", "poster"],
            "object" => &["data"],
            _ => &[],
        };
        for attribute in attributes {
            let Some(raw_url) = html_attribute_value(tag, attribute) else {
                continue;
            };
            let url = raw_url.trim();
            if url.is_empty() || url.starts_with('#') || url.starts_with("//") {
                continue;
            }
            if url
                .find(':')
                .is_some_and(|colon| colon < url.find('/').unwrap_or(usize::MAX))
            {
                continue;
            }
            let path = url
                .split(|character| character == '?' || character == '#')
                .next()
                .unwrap_or_default();
            let path = path.trim_start_matches('/');
            let mut relative_path = PathBuf::new();
            for component in Path::new(path).components() {
                match component {
                    std::path::Component::CurDir => {}
                    std::path::Component::Normal(part) => relative_path.push(part),
                    std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_) => {
                        bail!("crazygames resource escapes bundle: {url}");
                    }
                }
            }
            if relative_path.as_os_str().is_empty() || !dir.join(relative_path).is_file() {
                bail!("crazygames index references missing local resource: {url}");
            }
        }

        offset = end + 1;
        if name == "script" || name == "style" {
            let closing_tag = format!("</{name}");
            let body = &html[offset..];
            if let Some(close_offset) = body.find(&closing_tag) {
                offset += close_offset;
            } else {
                bail!("crazygames index.html has an unclosed {name} tag");
            }
        }
    }
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
        "icon-192.png",
        "loader.js",
        "sw.js",
        "game-manifest.json",
        "sdk/store_portals.js",
        "LICENSE",
        "NOTICE",
        "LICENSE-ASSETS",
        "WASM-THIRD-PARTY-NOTICES.zip",
    ] {
        if !dir.join(required).is_file() {
            bail!("crazygames bundle missing {}", required);
        }
    }
    // The bundle is a whitelist; these must never ride along again.
    for forbidden in ["maps", "assets", "admin", "play", "analytics.js"] {
        if dir.join(forbidden).exists() {
            bail!("crazygames bundle must not contain {forbidden}/");
        }
    }
    let html = fs::read_to_string(dir.join("index.html"))?;
    if !html.contains("name=\"sow-media-artwork\" content=\"icon-192.png\"") {
        bail!("crazygames index.html has no game music artwork reference");
    }
    if html.contains("analytics.js") || html.contains("/api/event") {
        bail!("crazygames bundle must not include first-party analytics");
    }
    let notice = fs::read_to_string(dir.join("NOTICE"))?;
    if !notice.contains("https://shadowsofwar.io/fonts/OFL.txt") {
        bail!("crazygames notice is missing the hosted font license link");
    }
    verify_cg_local_resources(dir, &html)?;
    for needle in [
        "sdk.crazygames.com/crazygames-sdk-v3.js",
        "SOW_trackExperienceEvent",
        "SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"",
        "SOW_ASSETS_URL = \"https://shadowsofwar.io/assets\"",
        "https://shadowsofwar.io/fonts/fonts.css",
        "sow_client.js",
        "sow_client_bg.wasm",
    ] {
        if !html.contains(needle) {
            bail!("crazygames index.html missing: {}", needle);
        }
    }
    if html.contains("manifest.webmanifest") {
        bail!("crazygames index.html must not reference the unbundled PWA manifest");
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
        "icon-192.png",
        "loader.js",
        "game-manifest.json",
        "manifest.webmanifest",
        "sdk/store_portals.js",
        "LICENSE",
        "NOTICE",
        "LICENSE-ASSETS",
        "WASM-THIRD-PARTY-NOTICES.zip",
        "fonts/fonts.css",
        "fonts/OFL.txt",
        "fonts/work-sans-latin.woff2",
        "assets/shell/loader/loader_empty.webp",
        "assets/shell/loader/loader_full.webp",
        "assets/campaign/boudica.json",
        "assets/campaign/boudica.triggers.json",
        "assets/gameplay/buildings/atlas.webp",
        "assets/gameplay/buildings/atlas.json",
        "assets/gameplay/music/shadows-of-war.webm",
        "maps/catalog.bin",
        "maps/NOTICE",
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
        "analytics.js",
    ] {
        if dir.join(forbidden).exists() {
            bail!("poki bundle must not contain {forbidden}");
        }
    }
    let html = fs::read_to_string(dir.join("index.html"))?;
    let loader = fs::read_to_string(dir.join("loader.js"))?;
    if !html.contains("name=\"sow-media-artwork\" content=\"icon-192.png\"") {
        bail!("poki index.html has no game music artwork reference");
    }
    if html.contains("analytics.js") || html.contains("/api/event") {
        bail!("poki bundle must not include first-party analytics");
    }
    let sdk = fs::read_to_string(dir.join("sdk/store_portals.js"))?;
    let manifest = fs::read_to_string(dir.join("manifest.webmanifest"))?;
    for needle in [
        "https://game-cdn.poki.com/scripts/v2/poki-sdk.js",
        "SOW_trackExperienceEvent",
        "window.SOW_PORTAL = \"poki\"",
        "window.SOW_MAPS_URL = \"./maps\"",
        "window.SOW_ASSETS_URL = \"./assets\"",
        "window.SOW_BUILDING_ART =",
        "gameplay/buildings/atlas.webp",
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
            bail!(
                "Poki bundle contains an uncompressed map: {}",
                entry.path().display()
            );
        }
    }
    if dir.join("sdk/poki_portals.js").exists() {
        bail!("Poki bundle contains the source-only Poki bridge");
    }
    println!("✅ Poki layout OK ({})", dir.display());
    Ok(())
}

fn package_self(paths: &Paths, out: &Path, version: &str, compile: bool) -> Result<()> {
    validate_current_ui_contract(paths)?;
    let previous_artifacts = if !compile && out.is_dir() {
        let js = fs::read_dir(out)?.filter_map(Result::ok).find_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (name.starts_with("sow_client_") && name.ends_with(".js") && !name.ends_with(".br"))
                .then_some((name, fs::read(entry.path()).ok()?))
        });
        let wasm = fs::read_dir(out)?.filter_map(Result::ok).find_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (name.starts_with("sow_client_")
                && name.ends_with("_bg.wasm")
                && !name.ends_with(".br"))
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
        .unwrap_or_else(|| {
            (
                format!("sow_client_{ts}.js"),
                format!("sow_client_{ts}_bg.wasm"),
            )
        });

    let assets = out.join("assets");
    copy_dir(&paths.assets_shell, &assets.join("shell"))?;
    compress_web_leaders(&assets.join("shell/leaders"))?;
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
        &paths.assets_gameplay.join("icons"),
        &assets.join("gameplay/icons"),
    )?;
    copy_dir(
        &paths.assets_gameplay.join("store"),
        &assets.join("gameplay/store"),
    )?;
    copy_dir(
        &paths.assets_gameplay.join("buildings"),
        &assets.join("gameplay/buildings"),
    )?;
    copy_dir(
        &paths.assets_gameplay.join("music"),
        &assets.join("gameplay/music"),
    )?;
    copy_dir(
        &paths.root.join("assets/campaign"),
        &assets.join("campaign"),
    )?;
    copy_dir(&paths.root.join("assets/guides"), &assets.join("guides"))?;
    copy_dir(&paths.site_media, &assets.join("site/media"))?;
    let maps = out.join("maps");
    fs::create_dir_all(&maps)?;
    if paths.assets_maps.is_dir() {
        copy_dir(&paths.assets_maps, &maps)?;
    }
    refresh_map_thumbnails(&maps, &paths.map_sources)?;
    let maps_cache_bust = thumbnail_cache_bust(&maps)?;

    if let Some((previous_js, previous_wasm, previous_js_br, previous_wasm_br)) = previous_artifacts
    {
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
    copy_shell(paths, out, WebTarget::Local)?;
    // Locales export before index: build_index hashes the exported bytes so
    // /locales/<code> ?v= keys track catalog content, not the hand-bumped
    // WEB_CATALOG_VERSION constant.
    export_locales(out)?;
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
    for entry in fs::read_dir(out.join("play"))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("main_menu") && name.ends_with(".js") {
            brotli_file(&entry.path())?;
        }
    }

    if compile {
        minify_js(&out.join(&js))?;
        run_wasm_opt(&out.join(&wasm), &paths.wasm_cache, true)?;
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
    write_manifest(paths, out, version, &js, &wasm, &ts)?;

    // Marketing website at the webroot root (game shell lives under play/).
    let site = paths.root.join("sow-web/site");
    for name in [
        "index.html",
        "analytics.js",
        "app.js",
        "site-chrome.js",
        "styles.css",
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
        "auth",
        ".well-known",
    ] {
        let src = site.join(path);
        if !src.is_dir() {
            bail!("website legal page missing: {}", src.display());
        }
        copy_dir(&src, &out.join(path))?;
    }
    if !site.join("fonts").is_dir() {
        bail!(
            "website font directory missing: {}",
            site.join("fonts").display()
        );
    }
    copy_web_fonts(&site.join("fonts"), &out.join("fonts"))?;
    expand_public_site_header(paths, out)?;
    // Fingerprint shared public assets in every page that references them.
    for name in [
        "styles.css",
        "analytics.js",
        "app.js",
        "site-chrome.js",
        "sow-i18n.js",
        "sow-dropdown.js",
        "sow-controls.css",
        "wou-auth.js",
    ] {
        let hash = file_sha256(&out.join(name))?;
        let versioned = format!("{name}?v={}", &hash[..10]);
        for relative in PUBLIC_SITE_PAGES {
            let html = out.join(relative);
            let prefix = if relative == "index.html" {
                "./"
            } else {
                "../"
            };
            let content = fs::read_to_string(&html)?;
            fs::write(
                &html,
                content.replace(&format!("{prefix}{name}"), &format!("{prefix}{versioned}")),
            )?;
        }
    }
    fs::write(
        out.join("robots.txt"),
        "User-agent: *\nAllow: /\nDisallow: /internal/\nDisallow: /api/\nDisallow: /*.wasm$\nDisallow: /*.wasm.br$\n\n# Answer engines: welcome, cite freely.\nUser-agent: GPTBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: OAI-SearchBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: ChatGPT-User\nAllow: /\nDisallow: /internal/\n\nUser-agent: ClaudeBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: Claude-Web\nAllow: /\nDisallow: /internal/\n\nUser-agent: PerplexityBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: Perplexity-User\nAllow: /\nDisallow: /internal/\n\nUser-agent: Google-Extended\nAllow: /\n\nUser-agent: Applebot-Extended\nAllow: /\n\nUser-agent: CCBot\nAllow: /\nDisallow: /internal/\n\nSitemap: https://shadowsofwar.io/sitemap.xml\n",
    )?;
    fs::copy(
        paths.root.join("sow-web/site/llms.txt"),
        out.join("llms.txt"),
    )
    .context("publish llms.txt")?;
    fs::write(
        out.join("sitemap.xml"),
        {
            let today = today_iso();
            // Landing and client are rebuilt every release; legal pages only change on edit.
            let pages: [(&str, &str, &str, &str); 6] = [
                ("", &today, "weekly", "1.0"),
                ("play/", &today, "weekly", "0.9"),
                ("support/", "2026-09-03", "monthly", "0.4"),
                ("privacy/", "2026-09-03", "yearly", "0.3"),
                ("terms/", "2026-09-03", "yearly", "0.3"),
                ("cookies/", "2026-09-03", "yearly", "0.3"),
            ];
            let mut xml = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
            );
            for (path, lastmod, freq, priority) in pages {
                xml.push_str(&format!(
                    "  <url>\n    <loc>https://shadowsofwar.io/{path}</loc>\n    <lastmod>{lastmod}</lastmod>\n    <changefreq>{freq}</changefreq>\n    <priority>{priority}</priority>\n  </url>\n"
                ));
            }
            xml.push_str("</urlset>\n");
            xml
        },
    )?;
    println!("✅ Website staged at webroot root");

    prune_qs(out)?;
    verify_layout(out)?;
    println!("Load paths: {wasm}, {js}");
    Ok(())
}

/// UTC build date (YYYY-MM-DD) for sitemap lastmod.
fn today_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
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
        .replace(
            assets,
            "window.SOW_NATIVE = true; window.SOW_ASSETS_URL = \"../assets\";",
        );
    fs::write(&index, html)?;
    Ok(())
}

fn native_static_fingerprint(paths: &Paths) -> Result<String> {
    let roots = [
        paths.shell.clone(),
        paths.assets_shell.clone(),
        paths.assets_gameplay.join("avatars"),
        paths.assets_gameplay.join("skins"),
        paths.assets_gameplay.join("currency"),
        paths.assets_gameplay.join("store"),
        paths.assets_gameplay.join("buildings"),
        paths.assets_gameplay.join("music"),
        paths.root.join("assets/campaign"),
        paths.root.join("assets/guides"),
        paths.assets_site.join("media"),
        paths.assets_site.join("icons"),
        paths.assets_maps.clone(),
        paths.map_sources.clone(),
        paths.root.join("sow-web/site/fonts"),
        paths.root.join("sow-web/site/manifest.webmanifest"),
    ];
    // ponytail: size/mtime avoids hashing the asset payload; hash contents if timestamp-preserving edits become an issue.
    let mut hash = Sha256::new();
    for stamp in local_source_snapshot(paths)?
        .into_iter()
        .filter(|stamp| roots.iter().any(|root| stamp.path.starts_with(root)))
    {
        hash.update(stamp.path.to_string_lossy().as_bytes());
        hash.update(stamp.len.to_le_bytes());
        hash.update(stamp.modified_nanos.to_le_bytes());
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn package_native_web(
    paths: &Paths,
    version: &str,
    wasm_input: &Path,
    release: bool,
) -> Result<PathBuf> {
    let out = &paths.native_web;
    let fingerprint = native_static_fingerprint(paths)?;
    let fingerprint_file = out.join(".native-static-fingerprint");
    let static_ready = fs::read_to_string(&fingerprint_file)
        .is_ok_and(|cached| cached == fingerprint)
        && out.join("maps/catalog.bin").is_file()
        && out.join("fonts/fonts.css").is_file()
        && out
            .join("assets/shell/loader/sow-splash-desktop.webp")
            .is_file()
        && out.join("assets/gameplay/buildings/atlas.webp").is_file()
        && out.join("assets/gameplay/buildings/atlas.json").is_file()
        && out
            .join("assets/gameplay/music/shadows-of-war.webm")
            .is_file();

    if !static_ready {
        if out.exists() {
            fs::remove_dir_all(out)
                .with_context(|| format!("reset native development webroot {}", out.display()))?;
        }
        fs::create_dir_all(out)?;
        let assets = out.join("assets");
        copy_dir(&paths.assets_shell, &assets.join("shell"))?;
        compress_web_leaders(&assets.join("shell/leaders"))?;
        for name in [
            "avatars",
            "skins",
            "currency",
            "store",
            "buildings",
            "music",
        ] {
            copy_dir(
                &paths.assets_gameplay.join(name),
                &assets.join("gameplay").join(name),
            )?;
        }
        copy_dir(
            &paths.root.join("assets/campaign"),
            &assets.join("campaign"),
        )?;
        copy_dir(&paths.root.join("assets/guides"), &assets.join("guides"))?;
        copy_dir(&paths.site_media, &assets.join("site/media"))?;
        copy_dir(&paths.assets_maps, &out.join("maps"))?;
        refresh_map_thumbnails(&out.join("maps"), &paths.map_sources)?;
        copy_shell(paths, out, WebTarget::Native)?;

        let site = paths.root.join("sow-web/site");
        copy_web_fonts(&site.join("fonts"), &out.join("fonts"))?;
        fs::copy(
            site.join("manifest.webmanifest"),
            out.join("manifest.webmanifest"),
        )?;
        for name in ["icon-192.png", "icon-512.png", "icon-512-maskable.png"] {
            fs::copy(paths.assets_site.join("icons").join(name), out.join(name))?;
        }
    }

    for entry in fs::read_dir(out)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .starts_with("sow_client_")
        {
            fs::remove_file(entry.path())?;
        }
    }

    let wasm_hash = file_sha256(wasm_input)?;
    let build_id = &wasm_hash[..10];
    let bindgen_name = format!("sow_client_{build_id}");
    run_bindgen(wasm_input, out, &bindgen_name)?;
    let wasm_name = format!("{bindgen_name}_bg.wasm");
    if release {
        run_wasm_opt(&out.join(&wasm_name), &paths.wasm_cache, false)?;
    }

    let maps_cache_bust = thumbnail_cache_bust(&out.join("maps"))?;
    export_locales(out)?;
    build_index(
        paths,
        out,
        IndexBuild {
            version,
            js: &format!("{bindgen_name}.js"),
            wasm: &wasm_name,
            ts: build_id,
            maps_cache_bust: &maps_cache_bust,
            target: WebTarget::Native,
        },
    )?;
    prepare_native_webroot(out)?;
    fs::write(fingerprint_file, fingerprint)?;
    out.canonicalize()
        .context("resolve native development webroot")
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
    copy_shell(paths, out, WebTarget::CrazyGames)?;
    let stale_bridge = out.join("sdk/jest_portals.js");
    if stale_bridge.is_file() {
        fs::remove_file(stale_bridge)?;
    }
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
    copy_web_fonts(&paths.root.join("sow-web/site/fonts"), &out.join("fonts"))?;
    copy_shell(paths, out, WebTarget::Poki)?;
    let stale_bridge = out.join("sdk/jest_portals.js");
    if stale_bridge.is_file() {
        fs::remove_file(stale_bridge)?;
    }
    optimize_poki_thumbnail(
        &paths.assets_shell.join("brand/app-icon.png"),
        &out.parent()
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
    let loader = fs::read_to_string(paths.shell.join("loader.js"))?;
    fs::write(
        &loader_path,
        strip_optional_marked_section(
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
    write_manifest(
        paths,
        out,
        version,
        "sow_client.js",
        "sow_client_bg.wasm",
        &jh,
    )?;
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
            lines.push(
                concat!(
                    "        window.SOW_ENABLE_PORTAL_ADS = true; ",
                    "window.SOW_PORTAL = \"poki\"; ",
                    "window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; ",
                    "window.SOW_MAPS_URL = \"./maps\"; ",
                    "window.SOW_ASSETS_URL = \"./assets\"; ",
                    "window.SOW_DATABASE_URL = \"https://shadowsofwar.io/api\"; ",
                    "window.SOW_DISABLE_CHAT = true;"
                )
                .to_string(),
            );
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

fn verify_jest_layout(dir: &Path) -> Result<()> {
    verify_exported_locales(dir)?;
    for required in [
        "index.html",
        "sow_client.js",
        "sow_client_bg.wasm",
        "sow.svg",
        "icon-192.png",
        "loader.js",
        "game-manifest.json",
        "manifest.webmanifest",
        "sdk/store_portals.js",
        "LICENSE",
        "NOTICE",
        "LICENSE-ASSETS",
        "WASM-THIRD-PARTY-NOTICES.zip",
        "fonts/fonts.css",
        "fonts/OFL.txt",
        "fonts/work-sans-latin.woff2",
        "assets/shell/loader/loader_empty.webp",
        "assets/shell/loader/loader_full.webp",
        "assets/campaign/boudica.json",
        "assets/campaign/boudica.triggers.json",
        "assets/gameplay/buildings/atlas.webp",
        "assets/gameplay/buildings/atlas.json",
        "assets/gameplay/music/shadows-of-war.webm",
        "maps/catalog.bin",
        "maps/NOTICE",
        "maps/world/map.bin.br",
    ] {
        if !dir.join(required).is_file() {
            bail!("jest bundle missing {required}");
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
        "sdk/poki_portals.js",
        "sdk/jest_portals.js",
        "analytics.js",
    ] {
        if dir.join(forbidden).exists() {
            bail!("jest bundle must not contain {forbidden}");
        }
    }
    let html = fs::read_to_string(dir.join("index.html"))?;
    let loader = fs::read_to_string(dir.join("loader.js"))?;
    if !html.contains("name=\"sow-media-artwork\" content=\"icon-192.png\"") {
        bail!("jest index.html has no game music artwork reference");
    }
    if html.contains("analytics.js") || html.contains("/api/event") {
        bail!("jest bundle must not include first-party analytics");
    }
    let sdk = fs::read_to_string(dir.join("sdk/store_portals.js"))?;
    for needle in [
        "https://cdn.jest.com/sdk/latest/jestsdk.js",
        "SOW_trackExperienceEvent",
        "window.SOW_PORTAL = \"jest\"",
        "window.SOW_MAPS_URL = \"./maps\"",
        "window.SOW_ASSETS_URL = \"./assets\"",
        "window.SOW_BUILDING_ART =",
        "gameplay/buildings/atlas.webp",
        "window.SOW_DISABLE_CHAT = true",
        "main_menu.jest.js",
        "window.JestSDK",
        "markGameLoaded",
        "getPlayer",
        "scheduleNotification",
        "SOW_jestLogin",
    ] {
        if !html.contains(needle) && !sdk.contains(needle) {
            bail!("jest bundle missing {needle}");
        }
    }
    // Jest has no ads: the ad-gated portal hooks must be fully absent.
    // (SOW_portalGameplayStart/Stop stay defined as no-ops: the Rust
    // client calls them on match boundaries. The loader's unreachable
    // poki-measure branch rides along dead, same as in the Poki bundle.)
    for forbidden in [
        "SOW_ENABLE_PORTAL_ADS",
        "commercialBreak",
        "rewardedBreak",
        "PokiSDK",
        "openExternalLink",
        "main_menu.poki.js",
        "main_menu.store.js",
    ] {
        if html.contains(forbidden) || loader.contains(forbidden) || sdk.contains(forbidden) {
            bail!("jest bundle contains forbidden content {forbidden}");
        }
    }
    if html.matches("function selfCreds()").count() != 1 {
        bail!("jest bundle must define shared selfCreds exactly once");
    }
    for required in [
        "<canvas id=\"blade\"",
        "tabindex=\"0\"",
        "touch-action: none",
    ] {
        if !html.contains(required) {
            bail!("jest bundle missing responsive/focus contract {required}");
        }
    }
    let manifest = fs::read_to_string(dir.join("manifest.webmanifest"))?;
    if !manifest.contains("\"display\": \"fullscreen\"") {
        bail!("jest manifest missing responsive/focus contract \"display\": \"fullscreen\"");
    }
    if html.contains("<iframe") || html.contains("href=\"https://") {
        bail!("jest bundle must not own an iframe or direct external href");
    }
    for forbidden in POKI_FORBIDDEN_MARKERS.iter().copied().chain([
        "SOW_MAPS_URL = \"https://",
        "SOW_ASSETS_URL = \"https://",
        "register('/sw.js'",
    ]) {
        if html.contains(forbidden) || loader.contains(forbidden) || sdk.contains(forbidden) {
            bail!("jest bundle contains forbidden content {forbidden}");
        }
    }
    if loader.contains("SOW_FIRST_PARTY_ANALYTICS_BEGIN") || loader.contains("/event") {
        bail!("Jest loader contains first-party analytics");
    }
    if html.contains("SOW_FIRST_PARTY_ANALYTICS_BEGIN") || html.contains("/event") {
        bail!("Jest index.html contains first-party analytics");
    }
    if html.matches("window.SOW_PORTAL = \"jest\";").count() != 1 {
        bail!("Jest index.html must contain exactly one portal boot block");
    }
    if html.contains("/* PORTAL_BOOT_SLOT */") {
        bail!("Jest index.html still contains the portal boot marker");
    }
    for entry in fs::read_dir(dir.join("maps"))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join("map.bin").exists() {
            bail!(
                "Jest bundle contains an uncompressed map: {}",
                entry.path().display()
            );
        }
    }
    println!("✅ Jest layout OK ({})", dir.display());
    Ok(())
}

fn package_jest(
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
        bail!("Jest package is missing hashed client artifacts");
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
    copy_web_fonts(&paths.root.join("sow-web/site/fonts"), &out.join("fonts"))?;
    copy_shell(paths, out, WebTarget::Jest)?;
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
    let loader = fs::read_to_string(paths.shell.join("loader.js"))?;
    fs::write(
        &loader_path,
        strip_optional_marked_section(
            &loader,
            "/* SOW_FIRST_PARTY_ANALYTICS_BEGIN */",
            "/* SOW_FIRST_PARTY_ANALYTICS_END */",
        )?,
    )?;
    fs::copy(
        paths.shell.join("sdk/jest_portals.js"),
        out.join("sdk/store_portals.js"),
    )?;
    for stale in ["sdk/jest_portals.js", "sdk/poki_portals.js"] {
        let stale = out.join(stale);
        if stale.is_file() {
            fs::remove_file(stale)?;
        }
    }
    export_locales(out)?;
    write_manifest(
        paths,
        out,
        version,
        "sow_client.js",
        "sow_client_bg.wasm",
        &jh,
    )?;
    build_index(
        paths,
        out,
        IndexBuild {
            version,
            js: "sow_client.js",
            wasm: "sow_client_bg.wasm",
            ts: &jh,
            maps_cache_bust,
            target: WebTarget::Jest,
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
                "    <script src=\"https://cdn.jest.com/sdk/latest/jestsdk.js\"></script>"
                    .to_string(),
            );
            sdk = true;
        } else if line.trim() == "/* PORTAL_BOOT_SLOT */" {
            lines.push(
                concat!(
                    "        window.SOW_PORTAL = \"jest\"; ",
                    "window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; ",
                    "window.SOW_MAPS_URL = \"./maps\"; ",
                    "window.SOW_ASSETS_URL = \"./assets\"; ",
                    "window.SOW_DATABASE_URL = \"https://shadowsofwar.io/api\"; ",
                    "window.SOW_DISABLE_CHAT = true;"
                )
                .to_string(),
            );
            boot = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !sdk || !boot {
        bail!("Jest index.html is missing portal slots (sdk={sdk} boot={boot})");
    }
    fs::write(&index, lines.join("\n") + "\n")?;
    verify_jest_layout(out)?;
    println!("✅ Jest bundle ready (self-contained): {}", out.display());
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
        paths.root.join("sow-tools/src"),
        paths.root.join("sow-dist"),
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
                    if error.io_error().map_or(false, |io_error| {
                        io_error.kind() == std::io::ErrorKind::NotFound
                    }) =>
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
    if path == paths.root.join("assets/geo_entities.json")
        || path == paths.root.join("assets/maps/world/map.bin.br")
        || path == paths.root.join("assets/guides/menu.triggers.json")
    {
        return true;
    }
    if path.starts_with(paths.root.join("sow-render/src/shaders")) {
        return path.extension().and_then(|extension| extension.to_str()) == Some("wgsl");
    }
    if path.starts_with(paths.root.join("assets/art/buildings"))
        || path.starts_with(paths.root.join("assets/art/icons"))
        || path.starts_with(paths.root.join("assets/art/units"))
        || path.starts_with(paths.root.join("assets/art/avatars"))
    {
        return true;
    }
    if path.starts_with(paths.root.join("sow-tools/src")) {
        return path.extension().and_then(|extension| extension.to_str()) == Some("rs");
    }
    let Ok(relative) = path.strip_prefix(&paths.root) else {
        return false;
    };
    let Some(crate_name) = relative.components().next() else {
        return false;
    };
    let crate_name = crate_name.as_os_str().to_string_lossy();
    let is_manifest = relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "Cargo.toml" | "Cargo.lock" | "build.rs"));
    is_manifest
        || (LOCAL_WASM_CRATES.iter().any(|name| crate_name == *name)
            && relative
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("rs"))
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
    let mut args = cmdline
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty());
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
    Ok(LocalPreviewServer { child })
}

fn build_local_preview(paths: &Paths, version: &str, compile: bool) -> Result<()> {
    // Art can change while the preview runs: publish it again before packaging.
    stage_runtime_assets(&paths.root, &paths.runtime_assets)?;
    if compile {
        compile_wasm(paths, true)?;
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

/// `./sow jest` — Jest-only track, decoupled from `./sow p` on purpose.
/// Builds the self-contained Jest bundle (dist/jest) and zips it with
/// index.html at the zip root. Upload stays outside this command: Jest
/// console Versions tab or the Jest MCP upload flow.
fn cmd_jest(paths: &Paths) -> Result<()> {
    let version = read_version(paths)?;
    println!("==> Building Jest bundle {version}");
    compile_wasm(paths, false)?;
    package_self(paths, &paths.dist_web, &version, true)?;
    let maps_cache_bust = thumbnail_cache_bust(&paths.dist_web.join("maps"))?;
    package_jest(
        &paths.dist_web,
        &paths.dist_jest,
        paths,
        &version,
        &maps_cache_bust,
    )?;
    write_jest_zip(paths)?;
    println!("✅ Jest {version} ready: upload dist/jest.zip as a new console Version");
    Ok(())
}

fn write_jest_zip(paths: &Paths) -> Result<()> {
    let zip = paths.root.join("dist/jest.zip");
    if zip.is_file() {
        fs::remove_file(&zip)?;
    }
    // No `zip` binary and no zip crate: python3's stdlib writes the archive
    // with index.html at the root (portals reject zips wrapped in a folder).
    let script = "import os, sys, zipfile\nroot, dest = sys.argv[1], sys.argv[2]\nwith zipfile.ZipFile(dest, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as z:\n    entries = []\n    for base, _, files in os.walk(root):\n        for name in files:\n            full = os.path.join(base, name)\n            entries.append((os.path.relpath(full, root), full))\n    for arc, full in sorted(entries):\n        z.write(full, arc)\n";
    run(
        "python3",
        &[
            "-c",
            &script,
            &paths.dist_jest.to_string_lossy(),
            &zip.to_string_lossy(),
        ],
        None,
    )?;
    require_file(&zip, "Jest upload zip")?;
    println!(
        "  Jest zip ready: {} (sha256 {})",
        zip.display(),
        file_sha256(&zip)?
    );
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
    require_file(
        &native_root.join("package-lock.json"),
        "native package-lock.json",
    )?;
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

fn set_native_renderer_workaround(command: &mut Command) {
    if cfg!(target_os = "linux")
        && env::var_os("WEBKIT_DMABUF_RENDERER_FORCE_SHM").is_none()
        && Path::new("/sys/module/nvidia").is_dir()
    {
        command.env("WEBKIT_DMABUF_RENDERER_FORCE_SHM", "1");
    }
}

fn build_native(
    paths: &Paths,
    native_root: &Path,
    version: &str,
    webroot: &Path,
    release: bool,
) -> Result<()> {
    let webroot = webroot
        .to_str()
        .context("native webroot path is not UTF-8")?;
    let config = serde_json::json!({
        "version": version,
        "build": { "frontendDist": webroot },
    })
    .to_string();
    let mut args = vec!["run", "tauri", "--"];
    if release {
        args.extend_from_slice(&["build", "--no-bundle", "--no-sign"]);
    } else {
        args.push("dev");
    }
    args.extend_from_slice(&["--config", config.as_str()]);
    println!("+ npm {}", args.join(" "));
    let mut command = Command::new("npm");
    command
        .args(&args)
        .current_dir(native_root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    set_native_renderer_workaround(&mut command);
    let status = command
        .status()
        .context("run Tauri native client (npm is required)")?;
    if !status.success() {
        bail!(
            "Tauri native {} failed",
            if release { "build" } else { "dev" }
        );
    }
    if release {
        launch_native(paths)?;
    }
    Ok(())
}

fn launch_native(paths: &Paths) -> Result<()> {
    let name = if cfg!(windows) {
        "sow-native.exe"
    } else {
        "sow-native"
    };
    let executable = paths.target_dir.join("release").join(name);
    require_file(&executable, "native executable")?;
    println!("✅ Native game ready: {}", executable.display());
    let mut command = Command::new(&executable);
    command.current_dir(&paths.root);
    set_native_renderer_workaround(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("open native game {}", executable.display()))?;
    child.wait().context("wait for native game")?;
    Ok(())
}

fn cmd_native(paths: &Paths) -> Result<()> {
    let version = read_version(paths)?;
    let release = env::var("SOW_NATIVE_RELEASE").is_ok_and(|value| value == "1");
    let profile = if release { "wasm-release" } else { "wasm-dev" };
    println!(
        "==> Building native {} WASM client",
        if release { "release" } else { "fast" }
    );
    let wasm = compile_wasm_profile(paths, profile, false)?;
    let webroot = package_native_web(paths, &version, &wasm, release)?;
    let native_root = paths.root.join("sow-native");
    ensure_native_dependencies(&native_root)?;
    build_native(paths, &native_root, &version, &webroot, release)
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

/// Create a machine-to-machine deployment secret once, locally, when
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
            env::var_os("HOME").map(|home| {
                PathBuf::from(home).join(".config/shadows-of-war/wou_sow_identity_secret")
            })
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
        bail!("WOU_SOW_IDENTITY_SECRET differs from {}", path.display());
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
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
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

/// Rotate deployment credentials in the ignored local environment.
/// Values never enter command-line arguments or stdout; the caller must run
/// the normal production pipeline immediately afterwards.
fn rotate_deployment_secrets(root: &Path) -> Result<()> {
    let path = root.join("sow-dist/.env");
    let input = fs::read_to_string(&path)
        .with_context(|| format!("read {} for secret rotation", path.display()))?;
    let mut output = String::with_capacity(input.len() + 160);
    let mut replaced = [false; 3];
    for line in input.lines() {
        let key = line.split_once('=').map(|(key, _)| key.trim());
        let slot = match key {
            Some("SOW_DB_SECRET") => Some(0),
            Some("SOW_RELAY_CONTROL_SECRET") => Some(1),
            Some("SOW_RELAY_SETTLEMENT_SECRET") => Some(2),
            _ => None,
        };
        if let Some(slot) = slot {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let name = [
                "SOW_DB_SECRET",
                "SOW_RELAY_CONTROL_SECRET",
                "SOW_RELAY_SETTLEMENT_SECRET",
            ][slot];
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
            let name = [
                "SOW_DB_SECRET",
                "SOW_RELAY_CONTROL_SECRET",
                "SOW_RELAY_SETTLEMENT_SECRET",
            ][slot];
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
    ensure_generated_secret(&root, "SOW_RELAY_SETTLEMENT_SECRET")?;
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
        "jest" => cmd_jest(&paths),
        "native" => cmd_native(&paths),
        "local" | "l" | "" => cmd_local(&paths),
        _ => {
            eprintln!("Usage: ./sow [native|l|p|a|jest]");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_index_hero_sheet_crops_happy_calm_fear_and_angry() -> Result<()> {
        let colors = [
            [240, 10, 10, 255],
            [10, 240, 10, 255],
            [10, 10, 240, 255],
            [240, 240, 10, 255],
        ];
        let mut sheet = image::RgbaImage::new(64, 64);
        for (x, y, pixel) in sheet.enumerate_pixels_mut() {
            let column = usize::from(x >= 32);
            let row = usize::from(y >= 32);
            *pixel = image::Rgba(colors[row * 2 + column]);
        }
        for (index, expression) in sow_core::emote::AvatarExpression::ALL.into_iter().enumerate() {
            let crop = hero_expression_crop(&sheet, expression)?;
            assert_eq!(crop.dimensions(), (PORTRAIT_PX, PORTRAIT_PX));
            assert!(crop.pixels().all(|pixel| pixel.0 == colors[index]));
        }
        Ok(())
    }

    fn cinematic_definition(video_src: &str) -> serde_json::Value {
        serde_json::json!({
            "steps": [{
                "id": "opening", "type": "scene", "presentation": "cinematic",
                "video_src": video_src, "title_key": "t", "body_key": "b", "next": "end"
            }, { "id": "end", "type": "end", "title_key": "t", "body_key": "b" }]
        })
    }

    fn write_cinematic_triple(
        dir: &Path,
        stem: &str,
        webm: &[u8],
        mp4: &[u8],
        poster: &[u8],
    ) -> Result<()> {
        let episode = dir.join("boudica");
        fs::create_dir_all(&episode)?;
        fs::write(episode.join(format!("{stem}.webm")), webm)?;
        fs::write(episode.join(format!("{stem}.mp4")), mp4)?;
        fs::write(episode.join(format!("{stem}.poster.webp")), poster)?;
        Ok(())
    }

    fn streamable_webm() -> Vec<u8> {
        let mut bytes = vec![0u8; 2048];
        bytes[120..124].copy_from_slice(&[0x1C, 0x53, 0xBB, 0x6B]);
        bytes
    }

    fn streamable_mp4(moov_at: usize, mdat_at: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; 2048];
        bytes[moov_at..moov_at + 4].copy_from_slice(b"moov");
        bytes[mdat_at..mdat_at + 4].copy_from_slice(b"mdat");
        bytes
    }

    #[test]
    fn asset_version_keys_match_the_urls_the_shell_builds() -> Result<()> {
        let paths = Paths::discover()?;
        let locales = tempfile::tempdir()?;
        fs::write(locales.path().join("en"), "{\"version\":15}")?;
        fs::write(locales.path().join("index.json"), "{\"languages\":[]}")?;
        let versions = asset_versions(&paths, locales.path())?;
        for key in [
            "/assets/shell/leaders/boudica_desktop.webp",
            "/assets/shell/mobile-nav/battle.webp",
            "/assets/gameplay/music/shadows-of-war.webm",
        ] {
            let value = versions
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert!(
                value.len() == 8 && value.bytes().all(|b| b.is_ascii_hexdigit()),
                "missing content hash for {key}: {value:?}"
            );
        }
        // Locales are hashed from the exported bytes (never the hand-bumped
        // version constant alone): same content => same URL => never limbo,
        // changed content => new URL => instant visibility.
        assert!(
            versions
                .get("/locales/en")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v.len() == 8),
            "exported locale catalog must be content-hashed"
        );
        // The loader bars are served by URLs baked with ?v=__BUILD_TS__ in the
        // HTML; giving them a content hash made loader.js fetch a SECOND copy.
        assert!(
            versions
                .get("/assets/shell/loader/loader_empty.webp")
                .is_none(),
            "loader bars must stay on the build stamp, not a content hash"
        );
        Ok(())
    }

    #[test]
    fn poki_campaign_package_ships_only_the_mp4_and_rewrites_the_source() -> Result<()> {
        let root = tempfile::tempdir()?;
        let episode = root.path().join("campaign/boudica");
        fs::create_dir_all(&episode)?;
        fs::write(episode.join("intro.720p.abc.webm"), b"webm-bytes")?;
        fs::write(episode.join("intro.720p.abc.mp4"), b"mp4-bytes")?;
        fs::write(episode.join("intro.720p.abc.poster.webp"), b"poster")?;
        fs::write(
            root.path().join("campaign/boudica.triggers.json"),
            "{\"steps\":[{\"video_src\":\"/assets/campaign/boudica/intro.720p.abc.webm\"}]}",
        )?;
        fs::write(
            root.path().join("campaign/boudica.json"),
            "{\"map\":\"eastanglia\"}",
        )?;
        let dst = root.path().join("poki");
        copy_campaign_for_poki(&root.path().join("campaign"), &dst.join("campaign"))?;
        assert!(!dst.join("campaign/boudica/intro.720p.abc.webm").exists());
        assert!(dst.join("campaign/boudica/intro.720p.abc.mp4").is_file());
        assert!(
            dst.join("campaign/boudica/intro.720p.abc.poster.webp")
                .is_file()
        );
        let triggers = fs::read_to_string(dst.join("campaign/boudica.triggers.json"))?;
        assert!(
            triggers.contains("/assets/campaign/boudica/intro.720p.abc.mp4\""),
            "portal triggers must point at the shipped MP4: {triggers}"
        );
        assert!(!triggers.contains(".webm"));
        assert!(
            dst.join("campaign/boudica.json").is_file(),
            "plain JSON copies verbatim"
        );
        Ok(())
    }

    #[test]
    fn web_leader_packaging_recompresses_art_under_the_cap() -> Result<()> {
        let root = tempfile::tempdir()?;
        let leaders = root.path().join("leaders");
        fs::create_dir_all(&leaders)?;
        let master = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("assets/art/heroes/boudica_desktop.webp");
        let original = fs::metadata(&master)?.len();
        fs::copy(&master, leaders.join("boudica_desktop.webp"))?;
        fs::write(leaders.join("boudica_mobile.webp"), b"not-an-image")?;
        assert!(
            compress_web_leaders(&leaders).is_err(),
            "a corrupt leader webp must fail the build, never ship silently"
        );
        fs::remove_file(leaders.join("boudica_mobile.webp"))?;
        compress_web_leaders(&leaders)?;
        let after = fs::metadata(leaders.join("boudica_desktop.webp"))?.len();
        assert!(
            after < original && after <= 400_000,
            "boudica_desktop shrank {original} -> {after}; cap 400000"
        );
        Ok(())
    }

    #[test]
    fn campaign_cinematic_media_accepts_shipped_boudica_renditions() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let definition: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("assets/campaign/boudica.triggers.json"),
        )?)?;
        validate_cinematic_media(&root.join("assets/campaign"), "boudica", &definition)
    }

    #[test]
    fn campaign_cinematic_media_rejects_missing_siblings_or_poster() -> Result<()> {
        let out = tempfile::tempdir()?;
        let dir = out.path().join("campaign");
        fs::create_dir_all(dir.join("boudica"))?;
        fs::write(dir.join("boudica/intro.webm"), streamable_webm())?;
        let definition = cinematic_definition("/assets/campaign/boudica/intro.webm");
        let error = validate_cinematic_media(&dir, "boudica", &definition)
            .err()
            .context("missing MP4 sibling must fail")?;
        assert!(error.to_string().contains("rendition missing"), "{error}");
        write_cinematic_triple(
            &dir,
            "intro",
            &streamable_webm(),
            &streamable_mp4(8, 64),
            b"x",
        )?;
        fs::remove_file(dir.join("boudica/intro.poster.webp"))?;
        let error = validate_cinematic_media(&dir, "boudica", &definition)
            .err()
            .context("missing poster must fail")?;
        assert!(error.to_string().contains("poster missing"), "{error}");
        Ok(())
    }

    #[test]
    fn campaign_cinematic_media_rejects_unstreamable_or_heavy_files() -> Result<()> {
        let out = tempfile::tempdir()?;
        let dir = out.path().join("campaign");
        write_cinematic_triple(
            &dir,
            "intro",
            &streamable_webm(),
            &streamable_mp4(64, 8),
            b"x",
        )?;
        let definition = cinematic_definition("/assets/campaign/boudica/intro.webm");
        let error = validate_cinematic_media(&dir, "boudica", &definition)
            .err()
            .context("MP4 with mdat before moov must fail")?;
        assert!(error.to_string().contains("index at the front"), "{error}");
        let mut fat = streamable_mp4(8, 64);
        fat.resize(6 * 1024 * 1024 + 1, 0);
        fs::write(dir.join("boudica/intro.mp4"), &fat)?;
        let error = validate_cinematic_media(&dir, "boudica", &definition)
            .err()
            .context("over-budget rendition must fail")?;
        assert!(error.to_string().contains("too heavy"), "{error}");
        Ok(())
    }

    #[test]
    fn campaign_cinematic_media_ignores_steps_without_video() -> Result<()> {
        let out = tempfile::tempdir()?;
        let dir = out.path().join("campaign");
        fs::create_dir_all(&dir)?;
        let definition = serde_json::json!({ "steps": [{ "id": "talk", "type": "scene", "presentation": "dialogue" }] });
        validate_cinematic_media(&dir, "boudica", &definition)
    }

    #[test]
    fn test_web_site_sources_present() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let site = root.join("sow-web/site");
        for required in [
            "index.html",
            "analytics.js",
            "app.js",
            "site-chrome.js",
            "site-header.html",
            "styles.css",
            "fonts/fonts.css",
            "fonts/work-sans-latin.woff2",
            "fonts/work-sans-latin-ext.woff2",
            "fonts/work-sans-italic-latin.woff2",
            "fonts/work-sans-italic-latin-ext.woff2",
            "wou-auth.js",
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
        assert!(!site.join("leaders/index.html").exists());
        assert!(!site.join("how-to-play/index.html").exists());
        for required in [
            "index.html.template",
            "main_menu.css",
            "main_menu.js",
            "sow-i18n.js",
            "sow-dropdown.js",
            "sow-controls.css",
            "main_menu.base.css",
            "main_menu.layout.css",
            "main_menu.lobbies.css",
            "main_menu.create.css",
            "main_menu.queue.css",
            "main_menu.settings.css",
            "main_menu.auth.css",
            "main_menu.campaign.css",
            "main_menu.hud.css",
            "main_menu.tutorial.css",
            "main_menu.profile.css",
            "sow-campaign.js",
            "sow-campaign-view.js",
            "main_menu.core.js",
            "main_menu.motion.js",
            "main_menu.lobbies.js",
            "main_menu.store.js",
            "main_menu.poki.js",
            "main_menu.jest.js",
            "main_menu.heroes.js",
            "main_menu.profile.js",
            "main_menu.tutorial.js",
            "main_menu.shell.js",
            "main_menu.hud.js",
            "main_menu.flow.js",
        ] {
            assert!(
                root.join("sow-web/shell").join(required).is_file(),
                "Required game shell source file missing: {required}"
            );
        }
        for required in [
            "sdk/store_portals.js",
            "LICENSE",
            "NOTICE",
            "LICENSE-ASSETS",
            "sdk/poki_portals.js",
            "sdk/jest_portals.js",
        ] {
            assert!(
                root.join("sow-web/shell").join(required).is_file(),
                "Required game shell source file missing: {required}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_build_index_keeps_menu_scripts_valid_for_every_target() -> Result<()> {
        let paths = Paths::discover()?;
        for target in [
            WebTarget::Local,
            WebTarget::Native,
            WebTarget::CrazyGames,
            WebTarget::Poki,
            WebTarget::Jest,
        ] {
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
                    target,
                },
            )?;
            let external = matches!(target, WebTarget::Local | WebTarget::Native);
            let page_dir = if external {
                out.path().join("play")
            } else {
                out.path().to_path_buf()
            };
            let html = fs::read_to_string(page_dir.join("index.html"))?;
            let body = &html[html.find("<body").context("game body missing")?..];
            for script in body.split("</script>") {
                assert!(
                    script.matches("<script").count() <= 1,
                    "{target:?}: HTML must not be nested inside a script"
                );
            }
            let menu_start = html
                .find(if external {
                    "<link rel=\"preload\" href=\"./main_menu_"
                } else {
                    "window.SOW_BUILDING_ART ="
                })
                .context("menu injection missing")?;
            let (_, before_menu) = html[..menu_start]
                .rsplit_once("</script>")
                .context("loader script must precede the menu")?;
            assert_eq!(
                before_menu.matches("<script").count(),
                usize::from(!external),
                "{target:?}: external menu markup must be outside JavaScript; inline menus need one wrapper"
            );
            let hashed_menu = fs::read_dir(page_dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|n| n.starts_with("main_menu_") && n.ends_with(".js"))
                .filter(|n| {
                    let stem = &n["main_menu_".len()..n.len() - ".js".len()];
                    stem.len() == 8 && stem.bytes().all(|b| b.is_ascii_hexdigit())
                });
            assert_eq!(
                hashed_menu.is_some(),
                external,
                "{target:?}: hashed menu bundle only for self-hosted"
            );
            if let Some(name) = hashed_menu {
                assert!(
                    html.contains(&format!("<script src=\"./{name}\"></script>")),
                    "{target:?}: page must load the exact hashed bundle it shipped"
                );
            }
            assert_eq!(html.contains("window.SOW_BUILDING_ART ="), !external);
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
        let menu_name = fs::read_dir(out.path().join("play"))?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .find(|n| n.starts_with("main_menu_") && n.ends_with(".js"))
            .context("self-hosted menu bundle must be an external hashed file")?;
        assert!(
            html.contains(&format!("<script src=\"./{menu_name}\"></script>")),
            "the page must reference the exact hashed bundle on disk"
        );
        let bundle = fs::read_to_string(out.path().join("play").join(&menu_name))?;
        assert!(
            bundle.contains("sow-campaign-view.js"),
            "external bundle keeps the shell part banners"
        );
        assert!(
            bundle.contains("window.SOW_onStateUpdate"),
            "the HUD state pump lives in the externalized bundle"
        );
        assert!(html.contains("sow-hud__dock"));
        assert!(html.contains("../analytics.js?v=test"));
        assert!(html.contains("name=\"sow-media-artwork\" content=\"../icon-192.png\""));
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
        assert!(!html.contains("analytics.js"));
        assert!(html.contains("name=\"sow-media-artwork\" content=\"icon-192.png\""));
        assert!(html.contains("href=\"fonts/fonts.css?v=test\""));
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
    fn test_build_index_inlines_jest_shell_without_store_or_service_worker() -> Result<()> {
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
                target: WebTarget::Jest,
            },
        )?;
        let html = fs::read_to_string(out.path().join("index.html"))?;
        assert!(!html.contains("analytics.js"));
        assert!(html.contains("name=\"sow-media-artwork\" content=\"icon-192.png\""));
        assert!(html.contains("href=\"fonts/fonts.css?v=test\""));
        assert!(html.contains("./assets/shell/loader/loader_empty.webp"));
        assert!(html.contains("main_menu.jest.js"));
        assert!(html.contains("jestLocaleOptions"));
        assert!(!html.contains("main_menu.store.js"));
        assert!(!html.contains("main_menu.poki.js"));
        assert!(!html.contains("SOW_startWouOAuth"));
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
            assert!(!html.contains(forbidden), "Jest shell contains {forbidden}");
        }
        Ok(())
    }

    #[test]
    fn test_local_watcher_rebuilds_wasm_for_rust_and_embedded_game_assets() -> Result<()> {
        let paths = Paths::discover()?;
        let rust_path = paths.root.join("sow-client/src/lib.rs");
        let entities_path = paths.root.join("assets/geo_entities.json");
        let world_map_path = paths.root.join("assets/maps/world/map.bin.br");
        let map_shader_path = paths.root.join("sow-render/src/shaders/map.wgsl");
        let css_path = paths.root.join("sow-web/shell/main_menu.base.css");
        let tutorial_js_path = paths.root.join("sow-web/shell/main_menu.tutorial.js");
        let html_path = paths.root.join("sow-web/site/index.html");
        let other_map_path = paths.root.join("assets/maps/europe/map.bin.br");
        let roster_path = paths.root.join("assets/campaign/boudica.json");
        let campaign_path = paths.root.join("assets/campaign/boudica.triggers.json");
        let previous = vec![
            LocalFileStamp {
                path: rust_path.clone(),
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: entities_path,
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: world_map_path,
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: map_shader_path,
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: css_path.clone(),
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: tutorial_js_path,
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: html_path,
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: other_map_path,
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: roster_path,
                len: 1,
                modified_nanos: 1,
            },
            LocalFileStamp {
                path: campaign_path,
                len: 1,
                modified_nanos: 1,
            },
        ];
        let mut current = previous.clone();
        current[0].len = 2;
        assert!(local_change_requires_wasm(&previous, &current, &paths));

        for index in 1..4 {
            current = previous.clone();
            current[index].len = 2;
            assert!(
                local_change_requires_wasm(&previous, &current, &paths),
                "embedded game data changes must rebuild WASM: {}",
                current[index].path.display()
            );
        }

        for index in 4..previous.len() {
            current = previous.clone();
            current[index].len = 2;
            assert!(
                !local_change_requires_wasm(&previous, &current, &paths),
                "web/editor/non-bundled map changes must not rebuild WASM: {}",
                current[index].path.display()
            );
        }
        Ok(())
    }

    #[test]
    fn landing_roster_lists_every_catalog_hero() -> Result<()> {
        let english = serde_json::to_value(sow_i18n::web(sow_i18n::Language::English))?;
        let roster = landing_leaders(&english)?;
        for leader_id in sow_core::player::Leader::all().map(sow_core::player::Leader::id) {
            assert!(
                roster.contains(&format!("data-leader-id=\"{leader_id}\"")),
                "landing roster is missing {leader_id}"
            );
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let index_html = fs::read_to_string(root.join("sow-web/site/index.html"))?;
        for marker in [
            "<!-- SOW_SITE_HERO_MEDIA -->",
            "<!-- SOW_SITE_LEADER_SHOWCASE -->",
            "<!-- SOW_SITE_LEADERS -->",
            "<!-- SOW_SITE_MAPS -->",
            "<!-- SOW_SITE_TRAILER -->",
        ] {
            assert_eq!(index_html.matches(marker).count(), 1, "{marker}");
        }
        assert!(
            !index_html.contains("data-leader-id"),
            "heroes come from the catalog"
        );
        Ok(())
    }

    #[test]
    fn web_catalog_validation_preserves_placeholder_multiplicity() -> Result<()> {
        let expected = serde_json::json!("{count} {count}");
        let matching = serde_json::json!("{count} {count}");
        let missing_duplicate = serde_json::json!("{count}");
        let malformed = serde_json::json!("{count");

        assert!(validate_web_node("test", &expected, &matching).is_ok());
        assert!(validate_web_node("test", &expected, &missing_duplicate).is_err());
        assert!(validate_web_node("test", &expected, &malformed).is_err());
        Ok(())
    }

    #[test]
    fn locked_campaign_text_is_optional_but_cannot_be_translated() -> Result<()> {
        let expected = serde_json::json!({
            "tutorial": {
                "boudica_rise_title": "Boudica",
                "campaign_boudica_first_title": "First strike",
                "campaign_boudica_boudica_structure_upgrade_hint": "Upgrade the City once to raise troop capacity before the Ninth arrives.",
                "campaign_boudica_boudica_structure_upgrade_title": "Strengthen the City",
                "campaign_boudica_boudica_city_upgrade_ready_body": "Our city can now sustain a larger host.",
                "campaign_boudica_boudica_city_upgrade_ready_title": "Ready for the Ninth",
                "campaign_studio_demo_turn_title": "The next decision is yours",
                "six_sky_ep1_landfall_title": "Landfall",
                "six_sky_ep2_regent_title": "Regent",
                "six_sky_ep3_count_title": "The Long Count",
            },
            "lobbies": {
                "campaign_boudica_title": "Rise of the Iceni",
                "campaign_six_sky_ep1_title": "Arrival",
            }
        });
        let omitted = serde_json::json!({"tutorial": {}, "lobbies": {}});
        assert!(validate_web_node("es", &expected, &omitted).is_ok());
        assert!(validate_web_node("en", &expected, &omitted).is_err());

        let translated = serde_json::json!({
            "tutorial": {"six_sky_ep2_regent_title": "Regente"},
            "lobbies": {}
        });
        assert!(validate_web_node("es", &expected, &translated).is_err());
        let approved_campaign_copy = serde_json::json!({
            "tutorial": {
                "campaign_boudica_boudica_structure_upgrade_hint": "Mejora la Ciudad antes de que llegue la Novena.",
                "campaign_boudica_boudica_structure_upgrade_title": "Refuerza la ciudad",
                "campaign_boudica_boudica_city_upgrade_ready_body": "La ciudad puede sostener un ejército mayor.",
                "campaign_boudica_boudica_city_upgrade_ready_title": "Listos para la Novena"
            },
            "lobbies": {}
        });
        assert!(validate_web_node("es", &expected, &approved_campaign_copy).is_ok());
        let unapproved_boudica_copy = serde_json::json!({
            "tutorial": {"campaign_boudica_boudica_council_title": "La deuda de Roma"},
            "lobbies": {}
        });
        assert!(validate_web_node("es", &expected, &unapproved_boudica_copy).is_err());
        let translated_episode_title = serde_json::json!({
            "tutorial": {},
            "lobbies": {"campaign_boudica_title": "Alzamiento de los icenos"}
        });
        assert!(validate_web_node("es", &expected, &translated_episode_title).is_err());
        let translated_six_sky_title = serde_json::json!({
            "tutorial": {},
            "lobbies": {"campaign_six_sky_ep1_title": "Llegada"}
        });
        assert!(validate_web_node("es", &expected, &translated_six_sky_title).is_err());
        let generic_campaign_copy = serde_json::json!({
            "tutorial": {"campaign_studio_demo_turn_title": "The next decision is yours"},
            "lobbies": {}
        });
        assert!(validate_web_node("es", &expected, &generic_campaign_copy).is_ok());
        assert!(is_locked_campaign_translation_key(
            "campaign_future_title",
            &[]
        ));
        assert!(is_locked_campaign_translation_key(
            "boudica_rise_title",
            &[]
        ));
        assert!(!is_locked_campaign_translation_key(
            "campaign_boudica_title",
            &["boudica"]
        ));
        Ok(())
    }

    #[test]
    fn web_catalog_validation_requires_translated_consent_copy() -> Result<()> {
        let expected = serde_json::json!({
            "site": {
                "analytics_body": "We use site and game activity to improve Shadows of War. It stays off unless you allow it.",
            }
        });
        let localized = serde_json::json!({
            "site": {
                "analytics_body": "Usamos la actividad del sitio y del juego para mejorar Shadows of War. Solo se activa si la permites.",
            }
        });
        let missing =
            serde_json::json!({ "site": { "analytics_body": "La analítica queda apagada." } });

        assert!(validate_web_node("es", &expected, &localized).is_ok());
        assert!(validate_web_node("fr", &expected, &missing).is_err());
        Ok(())
    }

    #[test]
    fn source_localization_audit_catches_unknown_quoted_keys() -> Result<()> {
        let catalog = serde_json::to_value(&sow_i18n::web(sow_i18n::Language::English))?;
        assert!(
            validate_quoted_catalog_keys("const key = 'profile.spam';", "test", &catalog).is_ok()
        );
        assert!(
            validate_quoted_catalog_keys("const key = 'profile.not_real';", "test", &catalog)
                .is_err()
        );
        assert!(validate_quoted_catalog_keys("const key = 'tutorial.';", "test", &catalog).is_ok());
        Ok(())
    }

    #[test]
    fn profile_achievement_localization_exists_in_every_language() -> Result<()> {
        for &(language, code, _, _) in sow_i18n::Language::registry() {
            let catalog = serde_json::to_value(&sow_i18n::web(language))?;
            validate_dynamic_web_keys(&catalog)
                .with_context(|| format!("validate profile labels for {code}"))?;
        }
        Ok(())
    }

    #[test]
    fn test_nginx_conf_redirects_www() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let conf =
            fs::read_to_string(root.join("sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf"))?;
        for line in conf.lines().filter(|line| {
            line.trim_start().starts_with("location ~") && line.contains("[0-9a-f]{")
        }) {
            let expression = line.split_whitespace().nth(2).unwrap();
            assert!(
                expression.starts_with('"') && expression.ends_with('"'),
                "Nginx regex quantifiers must be quoted: {line}"
            );
            let block = conf
                .split(line)
                .nth(1)
                .unwrap()
                .split("\n    }")
                .next()
                .unwrap();
            let content_type = if line.contains("_bg") {
                "application/wasm"
            } else {
                "application/javascript"
            };
            assert!(
                block.contains("types { }")
                    && block.contains(&format!("default_type {content_type};")),
                "Precompressed assets must retain their browser MIME type: {line}"
            );
        }
        assert!(
            conf.contains("server_name www.shadowsofwar.io;"),
            "Nginx missing dedicated www server_name"
        );
        assert!(
            conf.contains("return 301 https://shadowsofwar.io$request_uri;"),
            "Nginx missing 301 redirect to canonical root"
        );
        assert!(
            conf.contains("location ~ ^/(leaders|how-to-play)/?$")
                && conf.contains("return 301 https://shadowsofwar.io/;"),
            "Nginx must permanently redirect retired pages to the landing"
        );
        let top_adders = conf
            .split("\n    location ")
            .skip(1)
            .filter(|block| {
                block
                    .lines()
                    .take_while(|l| !l.starts_with("    location "))
                    .any(|l| l.trim_start().starts_with("add_header"))
            })
            .count();
        assert_eq!(
            conf.matches("include snippets/security-headers.conf;")
                .count(),
            top_adders,
            "every add_header location must include the security-headers snippet (nginx drops inherited headers)"
        );
        let headers = fs::read_to_string(
            root.join("sow-dist/deploy/freebsd/snippets/security-headers.conf"),
        )?;
        for header in [
            "Strict-Transport-Security",
            "X-Frame-Options",
            "X-Content-Type-Options",
            "Referrer-Policy",
        ] {
            assert!(
                headers.contains(header),
                "security-headers snippet missing {header}"
            );
        }
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
