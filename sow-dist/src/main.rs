use anyhow::{Context, Result, bail};
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
const DISTRIBUTION_RASTER_EXTENSIONS: [&str; 16] = [
    "png", "webp", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "avif", "ico", "cur", "icns",
    "tga", "heic", "heif", "jxl",
];
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
    "main_menu.store.ts",
    "allowfullscreen",
    "web-share",
    "focus-without-user-activation",
    "monetization",
];

// Share an authenticated transport across the deploy's checks and transfers.
// OpenSSH owns socket creation and removes it after the idle connection expires.
const SSH_REUSE_ARGS: [&str; 10] = [
    "-o",
    "ControlMaster=auto",
    "-o",
    "ControlPersist=600",
    "-o",
    "ControlPath=~/.ssh/sow-%C",
    "-o",
    "ServerAliveInterval=15",
    "-o",
    "ServerAliveCountMax=3",
];

fn command(cmd: &str) -> Command {
    let mut command = Command::new(cmd);
    match cmd {
        "ssh" | "scp" => {
            command.args(SSH_REUSE_ARGS);
        }
        "rsync" => {
            command.args(["-e", &format!("ssh {}", SSH_REUSE_ARGS.join(" "))]);
        }
        _ => {}
    }
    command
}

fn run(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    println!(
        "+ {cmd} {}",
        args.iter()
            .map(|a| shell_quote(a))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut c = command(cmd);
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
    let o = command(cmd).args(args).output()?;
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
    #[allow(dead_code)]
    dist_poki: PathBuf,
    dist_jest: PathBuf,
    native_web: PathBuf,
    wasm_input: PathBuf,
    wasm_cache: PathBuf,
    site_media: PathBuf,
    runtime_assets: PathBuf,
    app_icons: PathBuf,
    android_resources: PathBuf,
    art_pipeline: Option<ArtPipeline>,
}

impl Paths {
    fn discover() -> Result<Self> {
        Self::discover_with_runtime_assets("runtime-assets", true, false)
    }

    fn discover_for_local() -> Result<Self> {
        Self::discover_with_runtime_assets("local-runtime-assets", true, true)
    }

    fn discover_for_prod(server_only: bool) -> Result<Self> {
        Self::discover_with_runtime_assets("prod-runtime-assets", !server_only, false)
    }

    fn discover_for_poki() -> Result<Self> {
        Self::discover_with_runtime_assets("poki-runtime-assets", true, false)
    }

    fn discover_with_runtime_assets(runtime_assets_name: &str, prepare_art: bool, local_preview: bool) -> Result<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let art_pipeline = prepare_art
            .then(|| ArtPipeline::load_for(&root, local_preview))
            .transpose()?;
        let t = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("target"));
        let s = root.join("dist/.sow-state");
        let runtime_assets = s.join(runtime_assets_name);
        if let Some(art_pipeline) = &art_pipeline && !local_preview {
            stage_runtime_assets(&root, &runtime_assets, art_pipeline)?;
        }
        Ok(Self {
            wasm_input: t.join("wasm32-unknown-unknown/wasm-release/sow_client.wasm"),
            wasm_cache: s.join("wasm-opt-cache"),
            site_media: runtime_assets.join("site/media"),
            app_icons: runtime_assets.join("site-icons"),
            android_resources: runtime_assets.join("android/res"),
            art_pipeline,
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

    fn is_local_preview(&self) -> bool {
        self.art_pipeline.as_ref().is_some_and(|pipeline| pipeline.local_preview)
    }

    fn art_pipeline(&self) -> Result<&ArtPipeline> {
        self.art_pipeline
            .as_ref()
            .context("art pipeline is unavailable in server-only mode")
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ArtProfile {
    format: String,
    max_width: u32,
    max_height: u32,
    quality: u32,
    alpha: String,
    resample: String,
    lossless: bool,
    #[serde(default)]
    crop: Option<String>,
    max_bytes: u64,
    recipe_version: u32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ArtOptimizationConfig {
    version: u32,
    webp_method: u32,
    svgo_version: String,
    svg_max_bytes: u64,
    svg_recipe_version: u32,
    profiles: std::collections::BTreeMap<String, ArtProfile>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ImageCacheRecord {
    source_hash: String,
    recipe_hash: String,
    output_hash: String,
    source_has_transparency: bool,
}

struct ArtPipeline {
    local_preview: bool,
    root: PathBuf,
    cache_root: PathBuf,
    config_path: PathBuf,
    config: ArtOptimizationConfig,
    magick_version: String,
    svgo_version: String,
}

impl ArtPipeline {
    fn load(root: &Path) -> Result<Self> {
        Self::load_for(root, false)
    }

    fn load_for(root: &Path, local_preview: bool) -> Result<Self> {
        let config_path = root.join("sow-dist/art-optimization.toml");
        let config_text = fs::read_to_string(&config_path)
            .with_context(|| format!("read art optimization profiles: {}", config_path.display()))?;
        let config: ArtOptimizationConfig = toml::from_str(&config_text)
            .with_context(|| format!("parse art optimization profiles: {}", config_path.display()))?;
        if config.version != 1 || config.webp_method > 6 || config.profiles.is_empty() || config.svg_max_bytes == 0 || config.svg_recipe_version == 0 {
            bail!("unsupported or empty art optimization profile set in {}", config_path.display());
        }
        for (name, profile) in &config.profiles {
            if !["png", "webp"].contains(&profile.format.as_str())
                || profile.max_width == 0
                || profile.max_height == 0
                || profile.quality > 100
                || profile.max_bytes == 0
                || profile.recipe_version == 0
                || profile.alpha != "preserve"
                || !["lanczos", "point"].contains(&profile.resample.as_str())
                || profile.crop.as_deref().is_some_and(|crop| crop != "center_square")
                || (profile.lossless && profile.format != "webp" && profile.format != "png")
            {
                bail!("invalid art optimization profile {name:?} in {}", config_path.display());
            }
        }
        let magick_version = Self::tool_version("magick", "-version", local_preview)?
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        let svgo_version = Self::tool_version("svgo", "--version", local_preview)?;
        if !local_preview && svgo_version.trim() != config.svgo_version {
            bail!(
                "art profiles require SVGO {}, found {}; update the pinned profile/tool together",
                config.svgo_version,
                svgo_version.trim()
            );
        }
        let pipeline = Self {
            local_preview,
            root: root.to_path_buf(),
            cache_root: root.join("dist/.sow-state/art-optimization-cache"),
            config_path,
            config,
            magick_version,
            svgo_version: svgo_version.trim().to_owned(),
        };
        if !local_preview {
            pipeline.validate_profile_set()?;
        }
        Ok(pipeline)
    }

    fn tool_version(tool: &str, arg: &str, local_preview: bool) -> Result<String> {
        if local_preview {
            return Ok(output(tool, &[arg]).unwrap_or_default());
        }
        output(tool, &[arg]).with_context(|| format!("art distribution requires {tool}"))
    }

    fn profile(&self, name: &str) -> Result<&ArtProfile> {
        self.config
            .profiles
            .get(name)
            .with_context(|| format!("missing art optimization profile {name:?}"))
    }

    fn publish_image(
        &self,
        source: &Path,
        destination: &Path,
        profile_name: &str,
        exact_size: Option<(u32, u32)>,
        quality_override: Option<u32>,
    ) -> Result<()> {
        let result = self.publish_image_with_options(
            source, destination, profile_name, exact_size, quality_override, false,
        );
        self.preview_art_fallback(source, destination, result)
    }

    fn preview_art_fallback(&self, source: &Path, destination: &Path, result: Result<()>) -> Result<()> {
        if !self.local_preview {
            return result;
        }
        if let Err(error) = result {
            eprintln!("⚠ Local art {}: {error:#}", source.display());
            if source.is_file() && source.extension() == destination.extension() {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(source, destination)?;
            }
            // A derivative that requires conversion can keep its previous output.
        }
        Ok(())
    }

    fn publish_preview_image(
        &self,
        source: &Path,
        destination: &Path,
        profile_name: &str,
        exact_size: Option<(u32, u32)>,
        quality_override: Option<u32>,
    ) -> Result<()> {
        self.publish_image_with_options(source, destination, profile_name, exact_size, quality_override, true)
    }

    fn publish_image_with_options(
        &self,
        source: &Path,
        destination: &Path,
        profile_name: &str,
        exact_size: Option<(u32, u32)>,
        quality_override: Option<u32>,
        allow_over_cap: bool,
    ) -> Result<()> {
        let profile = self.profile(profile_name)?;
        let output_format = profile.format.as_str();
        if destination.extension().and_then(|ext| ext.to_str()) != Some(output_format) {
            bail!(
                "art profile {profile_name:?} outputs .{output_format}, but destination is {}",
                destination.display()
            );
        }
        if let Some((width, height)) = exact_size {
            if width == 0 || height == 0 || width > profile.max_width || height > profile.max_height {
                bail!("requested {width}x{height} exceeds art profile {profile_name:?}");
            }
        }
        let quality = quality_override.unwrap_or(profile.quality);
        if quality > 100 || (quality_override.is_some() && profile.lossless) {
            bail!("invalid quality override for art profile {profile_name:?}");
        }
        let source_hash = file_sha256(source)?;
        let recipe_hash = self.image_recipe_hash(profile_name, profile, exact_size, quality, allow_over_cap)?;
        let cache_file = self
            .cache_root
            .join(profile_name)
            .join(format!("{source_hash}-{recipe_hash}.{output_format}"));
        fs::create_dir_all(cache_file.parent().context("art cache has no parent")?)?;
        let mut cache_record = self.read_image_cache_record(
            profile_name,
            &source_hash,
            &recipe_hash,
            &cache_file,
        )?;
        let source_has_transparency = cache_record
            .as_ref()
            .map(|record| record.source_has_transparency)
            .map_or_else(|| image_has_transparency(source), Ok)?;
        let cached = cache_file.is_file()
            && self
                .validate_image_output(
                    &cache_file,
                    profile,
                    exact_size,
                    source_has_transparency,
                    !allow_over_cap,
                    cache_record.is_none(),
                )
                .is_ok();
        if !cached {
            let nonce = rand::thread_rng().next_u64();
            let temp = cache_file.with_file_name(format!(
                "{}-{}-{nonce}.tmp.{}",
                cache_file.file_stem().unwrap_or_default().to_string_lossy(),
                std::process::id(),
                output_format
            ));
            let mut magick = command("magick");
            magick.arg(source).args(["-filter", profile.resample.as_str()]);
            if let Some((width, height)) = exact_size {
                let geometry = format!("{width}x{height}!");
                magick.args(["-resize", geometry.as_str()]);
            } else if let Some("center_square") = profile.crop.as_deref() {
                let side = profile.max_width.min(profile.max_height);
                let geometry = format!("{side}x{side}^");
                let extent = format!("{side}x{side}");
                magick
                    .args(["-resize", geometry.as_str(), "-gravity", "center", "-extent", extent.as_str()]);
            } else {
                let geometry = format!("{}x{}>", profile.max_width, profile.max_height);
                magick.args(["-resize", geometry.as_str()]);
            }
            magick.arg("-strip");
            if output_format == "webp" {
                let method = format!("webp:method={}", self.config.webp_method);
                magick.args(["-quality", &quality.to_string(), "-define", method.as_str()]);
                if profile.lossless {
                    magick.args(["-define", "webp:lossless=true"]);
                } else {
                    magick.args(["-define", "webp:alpha-quality=100"]);
                }
            } else {
                magick.args([
                    "-define",
                    "png:compression-level=9",
                    "-define",
                    "png:compression-filter=5",
                ]);
            }
            let status = magick
                .arg(&temp)
                .status()
                .with_context(|| format!("optimize image {} with ImageMagick", source.display()))?;
            if !status.success() {
                bail!("ImageMagick failed to optimize {}", source.display());
            }
            if let Err(error) = self.validate_image_output(
                &temp,
                profile,
                exact_size,
                source_has_transparency,
                !allow_over_cap,
                true,
            ) {
                let _ = fs::remove_file(&temp);
                return Err(error).with_context(|| format!("validate art output for {}", source.display()));
            }
            fs::rename(&temp, &cache_file)?;
            cache_record = Some(self.write_image_cache_record(
                profile_name,
                &source_hash,
                &recipe_hash,
                &cache_file,
                source_has_transparency,
            )?);
        } else if cache_record.is_none() {
            cache_record = Some(self.write_image_cache_record(
                profile_name,
                &source_hash,
                &recipe_hash,
                &cache_file,
                source_has_transparency,
            )?);
        }
        let output_hash = &cache_record
            .context("validated art cache has no record")?
            .output_hash;
        if file_sha256(destination).ok().as_deref() != Some(output_hash.as_str()) {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&cache_file, destination)
                .with_context(|| format!("publish optimized image {}", destination.display()))?;
        }
        Ok(())
    }

    fn read_image_cache_record(
        &self,
        profile_name: &str,
        source_hash: &str,
        recipe_hash: &str,
        cache_file: &Path,
    ) -> Result<Option<ImageCacheRecord>> {
        if !cache_file.is_file() {
            return Ok(None);
        }
        let record_path = self.image_cache_record_path(profile_name, source_hash, recipe_hash);
        let Ok(bytes) = fs::read(record_path) else {
            return Ok(None);
        };
        let Ok(record) = serde_json::from_slice::<ImageCacheRecord>(&bytes) else {
            return Ok(None);
        };
        if record.source_hash != source_hash
            || record.recipe_hash != recipe_hash
            || file_sha256(cache_file)? != record.output_hash
        {
            return Ok(None);
        }
        Ok(Some(record))
    }

    fn write_image_cache_record(
        &self,
        profile_name: &str,
        source_hash: &str,
        recipe_hash: &str,
        cache_file: &Path,
        source_has_transparency: bool,
    ) -> Result<ImageCacheRecord> {
        let record = ImageCacheRecord {
            source_hash: source_hash.to_owned(),
            recipe_hash: recipe_hash.to_owned(),
            output_hash: file_sha256(cache_file)?,
            source_has_transparency,
        };
        let record_path = self.image_cache_record_path(profile_name, source_hash, recipe_hash);
        fs::create_dir_all(record_path.parent().context("art cache record has no parent")?)?;
        let nonce = rand::thread_rng().next_u64();
        let temp = record_path.with_file_name(format!(
            "{}-{}-{nonce}.tmp.json",
            record_path.file_stem().unwrap_or_default().to_string_lossy(),
            std::process::id(),
        ));
        fs::write(&temp, serde_json::to_vec(&record)?)?;
        fs::rename(temp, record_path)?;
        Ok(record)
    }

    fn image_cache_record_path(
        &self,
        profile_name: &str,
        source_hash: &str,
        recipe_hash: &str,
    ) -> PathBuf {
        self.cache_root
            .join("metadata")
            .join(profile_name)
            .join(format!("{source_hash}-{recipe_hash}.json"))
    }

    fn image_recipe_hash(
        &self,
        profile_name: &str,
        profile: &ArtProfile,
        exact_size: Option<(u32, u32)>,
        quality: u32,
        allow_over_cap: bool,
    ) -> Result<String> {
        let recipe = serde_json::json!({
            "profile": profile_name,
            "settings": profile,
            "exact_size": exact_size,
            "quality": quality,
            "allow_over_cap": allow_over_cap,
            "magick": self.magick_version,
            "optimization_version": self.config.version,
            "webp_method": self.config.webp_method,
        });
        Ok(hex::encode(Sha256::digest(serde_json::to_vec(&recipe)?)))
    }

    fn validate_image_output(
        &self,
        output: &Path,
        profile: &ArtProfile,
        exact_size: Option<(u32, u32)>,
        source_has_transparency: bool,
        enforce_byte_cap: bool,
        verify_transparency: bool,
    ) -> Result<()> {
        let size = fs::metadata(output)
            .with_context(|| format!("read optimized image {}", output.display()))?
            .len();
        if size == 0 || (enforce_byte_cap && size > profile.max_bytes) {
            bail!(
                "optimized image {} weighs {size} bytes; profile cap is {}",
                output.display(),
                profile.max_bytes
            );
        }
        let dimensions = image::image_dimensions(output)
            .with_context(|| format!("read optimized image dimensions {}", output.display()))?;
        if let Some(expected) = exact_size {
            if dimensions != expected {
                bail!("optimized image {} is {dimensions:?}; expected {expected:?}", output.display());
            }
        } else if dimensions.0 > profile.max_width || dimensions.1 > profile.max_height {
            bail!(
                "optimized image {} is {}x{}; profile maximum is {}x{}",
                output.display(), dimensions.0, dimensions.1, profile.max_width, profile.max_height
            );
        }
        if verify_transparency && source_has_transparency && !image_has_transparency(output)? {
            bail!("optimized image {} lost source transparency", output.display());
        }
        Ok(())
    }

    fn publish_svg(&self, source: &Path, destination: &Path) -> Result<()> {
        let source_hash = file_sha256(source)?;
        let recipe = serde_json::json!({
            "source": source_hash,
            "svgo": self.svgo_version,
            "config_version": self.config.version,
            "recipe_version": self.config.svg_recipe_version,
            "max_bytes": self.config.svg_max_bytes,
        });
        let recipe_hash = hex::encode(Sha256::digest(serde_json::to_vec(&recipe)?));
        let cache_file = self
            .cache_root
            .join("svg")
            .join(format!("{}-{recipe_hash}.svg", source_hash));
        fs::create_dir_all(cache_file.parent().context("SVG cache has no parent")?)?;
        let valid_cache = fs::read_to_string(&cache_file).is_ok_and(|svg| {
            svg.contains("<svg")
                && !svg.trim().is_empty()
                && fs::metadata(&cache_file)
                    .is_ok_and(|metadata| metadata.len() <= self.config.svg_max_bytes)
        });
        if !valid_cache {
            let temp = cache_file.with_file_name(format!(
                "{}-{}.tmp.svg",
                cache_file.file_stem().unwrap_or_default().to_string_lossy(),
                std::process::id()
            ));
            let status = command("svgo")
                .args(["--multipass", "--quiet", "-i"])
                .arg(source)
                .arg("-o")
                .arg(&temp)
                .status()
                .with_context(|| format!("minify SVG {} with SVGO", source.display()))?;
            if !status.success() {
                bail!("SVGO failed to minify {}", source.display());
            }
            let optimized = fs::read_to_string(&temp)
                .with_context(|| format!("read minified SVG {}", temp.display()))?;
            let size = fs::metadata(&temp)?.len();
            if !optimized.contains("<svg") || optimized.trim().is_empty() || size > self.config.svg_max_bytes {
                let _ = fs::remove_file(&temp);
                bail!("minified SVG {} is invalid or exceeds {} bytes", source.display(), self.config.svg_max_bytes);
            }
            fs::rename(&temp, &cache_file)?;
        }
        if file_sha256(destination).ok().as_deref() != file_sha256(&cache_file).ok().as_deref() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&cache_file, destination)
                .with_context(|| format!("publish minified SVG {}", destination.display()))?;
        }
        Ok(())
    }

    fn publish_asset(
        &self,
        source: &Path,
        destination: &Path,
        public_path: &Path,
        profile_override: Option<&str>,
    ) -> Result<()> {
        let extension = source.extension().and_then(|ext| ext.to_str()).unwrap_or_default().to_ascii_lowercase();
        let result = match extension.as_str() {
            "svg" => self.publish_svg(source, destination),
            extension if DISTRIBUTION_RASTER_EXTENSIONS.contains(&extension) => {
                match profile_override.or_else(|| profile_for_distribution_path(public_path)) {
                    Some(profile) => return self.publish_image(source, destination, profile, None, None),
                    None => Err(anyhow::anyhow!("raster art has no sow-dist profile: {}", public_path.display())),
                }
            }
            _ => {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(source, destination)
                    .with_context(|| format!("copy distribution file {}", source.display()))?;
                Ok(())
            }
        };
        self.preview_art_fallback(source, destination, result)
    }

    fn copy_distribution_tree(&self, source: &Path, destination: &Path, public_prefix: &Path) -> Result<()> {
        fs::create_dir_all(destination)?;
        for entry in walkdir::WalkDir::new(source).sort_by_file_name() {
            let entry = entry?;
            let relative = entry.path().strip_prefix(source)?;
            if entry.file_type().is_dir() {
                fs::create_dir_all(destination.join(relative))?;
                continue;
            }
            let public_path = public_prefix.join(relative);
            self.publish_asset(entry.path(), &destination.join(relative), &public_path, None)?;
        }
        Ok(())
    }

    fn validate_profile_set(&self) -> Result<()> {
        for required in [
            "ability_icon", "avatar_portrait_192", "avatar_portrait_256", "avatar_atlas", "fullbleed_wide", "fullbleed_tall",
            "poki_leader_wide", "poki_leader_tall",
            "site_still_wide", "site_still_tall", "ui_icon_128", "ui_icon_256",
            "unit_icon", "pixel_art", "atlas_lossless", "loader_ui", "app_icon",
            "map_preview", "map_thumbnail", "campaign_poster", "video_poster", "guide_image", "poki_thumbnail",
        ] {
            self.profile(required)?;
        }
        Ok(())
    }

    fn validate_staged_image_inventory(&self, staged_root: &Path) -> Result<()> {
        for entry in walkdir::WalkDir::new(staged_root) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(staged_root)?;
            let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase();
            if extension == "svg" {
                let svg = fs::read_to_string(path).with_context(|| format!("read staged SVG {}", path.display()))?;
                if !svg.contains("<svg") || fs::metadata(path)?.len() > self.config.svg_max_bytes {
                    bail!("staged SVG is invalid or over its profile cap: {}", path.display());
                }
                continue;
            }
            if !DISTRIBUTION_RASTER_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }
            let public_path = relative;
            let profile_name = profile_for_distribution_path(public_path)
                .with_context(|| format!("staged image has no distribution profile: {}", path.display()))?;
            let profile = self.profile(profile_name)?;
            let exact_size = staged_icon_size(public_path);
            self.validate_image_output(path, profile, exact_size, false, true, false)
                .with_context(|| format!("staged image violates {profile_name:?}: {}", path.display()))?;
        }
        Ok(())
    }

    fn run_preview(&self, selected_profile: Option<&str>) -> Result<PathBuf> {
        let mut assets = collect_art_preview_assets(&self.root)?;
        assets.sort_by(|left, right| left.profile.cmp(&right.profile).then_with(|| left.source.cmp(&right.source)));
        if let Some(profile) = selected_profile {
            if profile != "svg" {
                self.profile(profile)?;
            }
            assets.retain(|asset| asset.profile == profile);
            if assets.is_empty() {
                bail!("no source art is mapped to preview profile {profile:?}");
            }
        } else {
            let mut seen = std::collections::BTreeSet::new();
            assets.retain(|asset| seen.insert(asset.profile.clone()));
        }

        let output = self.root.join("dist/.sow-state/art-preview");
        if output.exists() {
            fs::remove_dir_all(&output)?;
        }
        fs::create_dir_all(&output)?;
        let mut html = String::from("<!doctype html><meta charset=\"utf-8\"><title>SOW art preview</title><style>body{font:16px system-ui;background:#111;color:#eee;margin:2rem}h1,h2{color:#d9b768}table{border-collapse:collapse;width:100%;margin:1rem 0 3rem}th,td{border:1px solid #444;padding:.6rem;text-align:left;vertical-align:top}img{max-width:320px;max-height:320px;object-fit:contain;background:#222}figure{margin:0}small{color:#aaa}a{color:#9cf}</style><h1>Shadows of War — art preview</h1><p>Masters remain untouched. Variants use the distribution recipes; byte-cap overruns are shown for comparison.</p>");
        let mut rows = Vec::new();
        for asset in &assets {
            let source_bytes = fs::metadata(&asset.source)?.len();
            let relative_source = asset.source.strip_prefix(&self.root).unwrap_or(&asset.source).to_string_lossy().into_owned();
            let safe_id = hex::encode(Sha256::digest(relative_source.as_bytes()));
            let source_url = format!("file://{}", asset.source.display());
            let folder = output.join(&asset.profile);
            fs::create_dir_all(&folder)?;
            let mut figures = String::new();
            let mut metrics = Vec::new();
            if asset.profile == "svg" {
                let destination = folder.join(format!("{safe_id}.svg"));
                self.publish_svg(&asset.source, &destination)?;
                let bytes = fs::metadata(&destination)?.len();
                let candidate = destination.strip_prefix(&output)?.to_string_lossy();
                figures.push_str(&format!("<td><figure><a href=\"{}\"><img src=\"{}\" alt=\"minified SVG\"></a><figcaption>SVGO · {bytes} bytes</figcaption></figure></td>", html_text(&candidate), html_text(&candidate)));
                metrics.push(format!("SVG {source_bytes} → {bytes} bytes"));
            } else {
                let profile = self.profile(&asset.profile)?;
                let qualities = if profile.format == "webp" && !profile.lossless {
                    let base = profile.quality;
                    let mut values = vec![base.saturating_sub(10).max(45), base, (base + 10).min(95)];
                    values.sort_unstable();
                    values.dedup();
                    values.into_iter().map(Some).collect::<Vec<_>>()
                } else {
                    vec![None]
                };
                for quality in qualities {
                    let label = quality.map_or_else(|| "lossless".to_owned(), |value| format!("q{value}"));
                    let destination = folder.join(format!("{safe_id}-{label}.{}", profile.format));
                    self.publish_preview_image(&asset.source, &destination, &asset.profile, asset.exact_size, quality)?;
                    let bytes = fs::metadata(&destination)?.len();
                    let dimensions = image::image_dimensions(&destination)?;
                    let candidate = destination.strip_prefix(&output)?.to_string_lossy();
                    let cap_note = if bytes > profile.max_bytes { format!(" · sobre límite {}", profile.max_bytes) } else { String::new() };
                    figures.push_str(&format!("<td><figure><a href=\"{}\"><img src=\"{}\" alt=\"{}\"></a><figcaption>{label} · {}×{} · {bytes} bytes{cap_note} · {:.1}% menor</figcaption></figure></td>", html_text(&candidate), html_text(&candidate), html_text(&relative_source), dimensions.0, dimensions.1, (1.0 - bytes as f64 / source_bytes.max(1) as f64) * 100.0));
                    metrics.push(format!("{label} {}×{} {bytes} bytes", dimensions.0, dimensions.1));
                }
            }
            html.push_str(&format!("<h2>{} · {}</h2><table><thead><tr><th>Original de alta resolución</th><th colspan=\"3\">Alternativas para distribución</th></tr></thead><tbody><tr><td><a href=\"{}\">{}</a><br><small>{source_bytes} bytes</small></td>{figures}</tr></tbody></table>", html_text(&asset.profile), html_text(&relative_source), html_text(&source_url), html_text(&relative_source)));
            rows.push(format!("{}\t{}\t{}\t{}", asset.profile, relative_source, source_bytes, metrics.join(" | ")));
        }
        let report = output.join("index.html");
        fs::write(&report, html)?;
        fs::write(output.join("report.tsv"), format!("profile\tsource\tsource_bytes\tcandidates\n{}\n", rows.join("\n")))?;
        println!("Art preview: {} source(s) -> {}", assets.len(), report.display());
        Ok(report)
    }
}

fn image_has_transparency(path: &Path) -> Result<bool> {
    let image = image::open(path).with_context(|| format!("decode image alpha {}", path.display()))?;
    if !image.color().has_alpha() {
        return Ok(false);
    }
    Ok(image.into_rgba8().pixels().any(|pixel| pixel.0[3] < 255))
}

struct ArtPreviewAsset {
    source: PathBuf,
    profile: String,
    exact_size: Option<(u32, u32)>,
}

fn profile_for_distribution_path(path: &Path) -> Option<&'static str> {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = path.strip_prefix("assets/").unwrap_or(&path);
    let name = Path::new(path).file_name()?.to_string_lossy();
    if path.starts_with("shell/loader/") {
        if name == "sow-splash-desktop.webp" {
            return Some("fullbleed_wide");
        }
        if name == "sow-splash-mobile.webp" {
            return Some("fullbleed_tall");
        }
        return Some("loader_ui");
    }
    if path.starts_with("shell/leaders/") {
        return Some(if name.contains("_mobile.") { "fullbleed_tall" } else { "fullbleed_wide" });
    }
    if path == "shell/brand/app-icon.png" || path == "icon-192.png" || path == "icon-512.png" || path == "icon-512-maskable.png" {
        return Some("app_icon");
    }
    if path == "poki-thumbnail.png" {
        return Some("poki_thumbnail");
    }
    if path.starts_with("site-icons/") || path.starts_with("android/res/mipmap-") {
        return Some("app_icon");
    }
    if path.starts_with("shell/brand/") || path.starts_with("shell/ranked/") || path.starts_with("gameplay/skins/") {
        return (name.ends_with(".svg")).then_some("svg");
    }
    if path.starts_with("gameplay/icons/abilities/") {
        return Some("ability_icon");
    }
    if path.starts_with("gameplay/currency/")
        || path.starts_with("gameplay/store/")
        || path.starts_with("shell/mobile-nav/")
        || path == "gameplay/icons/battle_icon_cutout.webp"
    {
        return Some("ui_icon_128");
    }
    if path == "gameplay/icons/tutorial_hand.webp" || path == "gameplay/icons/surrender_flag.webp" {
        return Some("ui_icon_256");
    }
    if path.starts_with("gameplay/avatars/") {
        if path.starts_with("gameplay/avatars/atlas/") {
            return Some("avatar_atlas");
        }
        return Some("avatar_portrait_256");
    }
    if path.starts_with("gameplay/units/") {
        return Some(if ["trade-ship.webp", "transport-ship.webp", "warship.webp"].contains(&name.as_ref()) { "pixel_art" } else { "unit_icon" });
    }
    if path == "gameplay/buildings/atlas.webp" || path == "gameplay/icons/nameplate_status_atlas.webp" {
        return Some("atlas_lossless");
    }
    if path == "gameplay/icons/abilities-atlas.webp" {
        return Some("atlas_lossless");
    }
    if path.starts_with("site/media/") {
        if name == "hero-loop.webp" || name == "gameplay.webp" {
            return Some("video_poster");
        }
        return Some(if name.contains("-mobile.") || name == "support-six-sky.webp" { "site_still_tall" } else { "site_still_wide" });
    }
    if path.starts_with("maps/") {
        if name == "thumbnail.webp" { return Some("map_thumbnail"); }
        if name == "preview.webp" { return Some("map_preview"); }
    }
    if path.starts_with("campaign/") && name.ends_with(".poster.webp") {
        return Some("campaign_poster");
    }
    if path.starts_with("guides/") {
        return Some("guide_image");
    }
    if path.starts_with("sow-web/site/") && name.ends_with(".svg") {
        return Some("svg");
    }
    None
}

fn staged_icon_size(path: &Path) -> Option<(u32, u32)> {
    let name = path.file_name()?.to_str()?;
    match name {
        "icon-192.png" => Some((192, 192)),
        "icon-512.png" | "icon-512-maskable.png" => Some((512, 512)),
        "ic_launcher.png" | "ic_launcher_round.png" => {
            let density = path.parent()?.file_name()?.to_str()?;
            let side = match density {
                "mipmap-mdpi" => 48,
                "mipmap-hdpi" => 72,
                "mipmap-xhdpi" => 96,
                "mipmap-xxhdpi" => 144,
                "mipmap-xxxhdpi" => 192,
                _ => return None,
            };
            Some((side, side))
        }
        _ => None,
    }
}

fn preview_source_named(dir: &Path, stem: &str) -> Option<PathBuf> {
    ["webp", "png", "jpg", "jpeg"]
        .iter()
        .map(|extension| dir.join(format!("{stem}.{extension}")))
        .find(|path| path.is_file())
}

fn push_preview_asset(
    assets: &mut Vec<ArtPreviewAsset>,
    source: PathBuf,
    profile: &str,
    exact_size: Option<(u32, u32)>,
) {
    if source.is_file() {
        assets.push(ArtPreviewAsset {
            source,
            profile: profile.to_owned(),
            exact_size,
        });
    }
}

fn collect_art_preview_assets(root: &Path) -> Result<Vec<ArtPreviewAsset>> {
    let mut assets = Vec::new();
    let art = root.join("assets/art");
    for entry in walkdir::WalkDir::new(&art).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let relative = path.strip_prefix(&art)?.to_string_lossy().replace('\\', "/");
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default();
        if extension == "svg" {
            push_preview_asset(&mut assets, path.to_path_buf(), "svg", None);
        } else if relative.starts_with("icons/abilities/") {
            push_preview_asset(&mut assets, path.to_path_buf(), "ability_icon", None);
        } else if relative.starts_with("heroes/") {
            let profile = if name.contains("_mobile.") { "fullbleed_tall" } else { "fullbleed_wide" };
            push_preview_asset(&mut assets, path.to_path_buf(), profile, None);
            let portal_profile = if name.contains("_mobile.") { "poki_leader_tall" } else { "poki_leader_wide" };
            push_preview_asset(&mut assets, path.to_path_buf(), portal_profile, None);
        } else if relative.starts_with("avatars/heroes/") {
            // These are 2x2 source sheets. The distributor previews their cropped expressions.
        } else if relative.starts_with("avatars/nations/") || relative.starts_with("avatars/tribes/") || relative.starts_with("avatars/campaign/") {
            push_preview_asset(&mut assets, path.to_path_buf(), "avatar_portrait_192", None);
        } else if relative == "avatars/random.webp" || relative == "avatars/default.webp" {
            push_preview_asset(&mut assets, path.to_path_buf(), "avatar_portrait_256", None);
        } else if relative == "brand/app-icon.webp" {
            push_preview_asset(&mut assets, path.to_path_buf(), "app_icon", Some((512, 512)));
            push_preview_asset(&mut assets, path.to_path_buf(), "poki_thumbnail", None);
        } else if relative == "loader/sow-splash-desktop.webp" {
            push_preview_asset(&mut assets, path.to_path_buf(), "fullbleed_wide", None);
        } else if relative == "loader/sow-splash-mobile.webp" {
            push_preview_asset(&mut assets, path.to_path_buf(), "fullbleed_tall", None);
        } else if relative.starts_with("loader/") {
            push_preview_asset(&mut assets, path.to_path_buf(), "loader_ui", None);
        } else if relative.starts_with("units/") {
            let profile = if ["transport-ship.webp", "trade-ship.webp", "warship.webp"].contains(&name.as_ref()) { "pixel_art" } else { "unit_icon" };
            push_preview_asset(&mut assets, path.to_path_buf(), profile, None);
        }
    }
    for entry in walkdir::WalkDir::new(root.join("assets/site/media")).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "svg") {
            push_preview_asset(&mut assets, entry.path().to_path_buf(), "svg", None);
        }
    }
    for (dir, stem, _, profile) in ART_ICONS {
        if let Some(source) = preview_source_named(&art.join(dir), stem) {
            push_preview_asset(&mut assets, source, profile, None);
        }
    }
    for (stem, (fallback_dir, fallback_stem)) in SITE_STILLS {
        let source = preview_source_named(&root.join("assets/site/media"), stem)
            .or_else(|| preview_source_named(&root.join(fallback_dir), fallback_stem));
        if let Some(source) = source {
            let profile = if stem.ends_with("-mobile") || stem == "support-six-sky" { "site_still_tall" } else { "site_still_wide" };
            push_preview_asset(&mut assets, source, profile, None);
        }
    }
    for entry in walkdir::WalkDir::new(root.join("assets/maps")).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() && entry.file_name() == "thumbnail.webp" {
            push_preview_asset(&mut assets, entry.path().to_path_buf(), "map_thumbnail", None);
            push_preview_asset(&mut assets, entry.path().to_path_buf(), "map_preview", None);
        }
    }
    for entry in walkdir::WalkDir::new(root.join("assets/campaign")).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() && entry.file_name().to_string_lossy().ends_with(".poster.webp") {
            push_preview_asset(&mut assets, entry.path().to_path_buf(), "campaign_poster", None);
        }
    }
    for name in ["buildings-atlas.webp", "badges-atlas.webp"] {
        let source = root.join("assets/generated").join(name);
        push_preview_asset(&mut assets, source, "atlas_lossless", None);
    }
    if let Some(source) = preview_source_named(&root.join("assets/site/media"), "game-war") {
        push_preview_asset(&mut assets, source, "video_poster", None);
    }
    Ok(assets)
}

/// UI icon masters and their published path/profile.
const ART_ICONS: [(&str, &str, &str, &str); 14] = [
    ("icons", "gold", "gameplay/currency/gold.webp", "ui_icon_128"),
    ("icons", "gem", "gameplay/currency/gem.webp", "ui_icon_128"),
    ("icons", "crown", "gameplay/currency/crown.webp", "ui_icon_128"),
    ("icons", "laurel", "gameplay/currency/laurel.webp", "ui_icon_128"),
    (
        "icons",
        "troops",
        "gameplay/icons/battle_icon_cutout.webp",
        "ui_icon_128",
    ),
    (
        "icons",
        "tutorial-hand",
        "gameplay/icons/tutorial_hand.webp",
        "ui_icon_256",
    ),
    (
        "icons",
        "surrender",
        "gameplay/icons/surrender_flag.webp",
        "ui_icon_256",
    ),
    ("icons", "troops", "shell/mobile-nav/battle.webp", "ui_icon_128"),
    ("icons", "nav-heroes", "shell/mobile-nav/heroes.webp", "ui_icon_128"),
    ("icons", "nav-profile", "shell/mobile-nav/profile.webp", "ui_icon_128"),
    ("icons", "nav-store", "shell/mobile-nav/store.webp", "ui_icon_128"),
    (
        "store",
        "sow_gems_500",
        "gameplay/store/gem_bundles/sow_gems_500.webp",
        "ui_icon_128",
    ),
    (
        "store",
        "sow_gems_1200",
        "gameplay/store/gem_bundles/sow_gems_1200.webp",
        "ui_icon_128",
    ),
    (
        "store",
        "sow_gems_2600",
        "gameplay/store/gem_bundles/sow_gems_2600.webp",
        "ui_icon_128",
    ),
];
/// Art folders routed into the runtime asset tree.
const ART_FOLDERS: [(&str, &str); 7] = [
    ("brand", "shell/brand"),
    ("ranked", "shell/ranked"),
    ("icons/abilities", "gameplay/icons/abilities"),
    ("loader", "shell/loader"),
    ("skins", "gameplay/skins"),
    ("music", "gameplay/music"),
    ("units", "gameplay/units"),
];
const GAMEPLAY_UNIT_ART: [&str; 5] = [
    "transport-ship.webp",
    "trade-ship.webp",
    "warship.webp",
    "atom-bomb.webp",
    "sam-missile.webp",
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
const PORTRAIT_PX: u32 = 192;
const HERO_PORTRAIT_PX: u32 = 256;
/// Landing shots (assets/site/media/<stem>.webp): until the owner drops a
/// capture, each slot publishes current game art (dir, stem) so nothing stale shows.
const SITE_STILLS: [(&str, (&str, &str)); 12] = [
    ("game-expand", ("assets/maps/world", "thumbnail")),
    ("game-build", ("assets/maps/world", "thumbnail")),
    ("game-war", ("assets/maps/world", "thumbnail")),
    (
        "closing-desktop",
        ("assets/art/loader", "sow-splash-desktop"),
    ),
    ("closing-mobile", ("assets/art/loader", "sow-splash-mobile")),
    (
        "step-expand-mobile",
        ("assets/art/heroes", "boudica_mobile"),
    ),
    (
        "step-build-mobile",
        ("assets/art/heroes", "cleopatra_mobile"),
    ),
    (
        "step-war-mobile",
        ("assets/art/heroes", "richard_the_lionheart_mobile"),
    ),
    ("step-expand", ("assets/maps/world", "thumbnail")),
    (
        "step-build",
        ("assets/art/heroes", "richard_the_lionheart_desktop"),
    ),
    ("step-war", ("assets/art/heroes", "leonidas_desktop")),
    ("support-six-sky", ("assets/art/heroes", "lady_six_sky_mobile")),
];
/// Landing videos (assets/site/media/<stem>.<mp4|mov|webm|mkv>), published as webm,
/// mp4 and a poster. Optional: the page shows a still while a slot is empty.
const SITE_VIDEOS: [&str; 2] = ["hero-loop", "gameplay"];
const SITE_VIDEO_SOURCES: [&str; 4] = ["mp4", "mov", "webm", "mkv"];
const SITE_BADGES: [&str; 6] = [
    "testflight-badge.svg",
    "google-play-badge.svg",
    "discord.svg",
    "telegram.svg",
    "github.svg",
    "worldofunreal.svg",
];
const SITE_STATIC_MEDIA: [&str; 0] = [];
const SITE_SOURCE_ONLY_MEDIA: [&str; 9] = [
    "caesar-cutout.webp",
    "session-battle.webp",
    "session-conflict.webp",
    "session-expansion.webp",
    "session-late-map.webp",
    "session-leader.webp",
    "session-world.webp",
    "shadows-of-war-gameplay.mp4",
    "shadowsofwar-scroll.vp9.webm",
];
fn site_media_ready(out: &Path) -> bool {
    let media = out.join("assets/site/media");
    SITE_BADGES
        .iter()
        .chain(SITE_STATIC_MEDIA.iter())
        .all(|name| media.join(name).is_file())
        && SITE_STILLS
            .iter()
            .all(|(stem, _)| media.join(format!("{stem}.webp")).is_file())
}

/// The owner edits only assets/art; the tools write assets/generated. This is the
/// single place that turns them into the published tree (/assets/shell, /assets/gameplay)
/// used by the shell, the WASM client and the CDN. Unknown art fails the build so a
/// dropped file can never sit unused or unpublished.
fn stage_runtime_assets(root: &Path, out: &Path, pipeline: &ArtPipeline) -> Result<()> {
    for entry in walkdir::WalkDir::new(root.join("assets")) {
        let entry = entry?;
        let ext = entry.path().extension().and_then(|ext| ext.to_str()).unwrap_or_default();
        if entry.file_type().is_file() && ["png", "jpg", "jpeg"].contains(&ext.to_ascii_lowercase().as_str()) {
            bail!("source raster must be WebP: {}", entry.path().display());
        }
    }
    let art = root.join("assets/art");
    if !pipeline.local_preview {
        verify_playable_hero_icons(&art.join("icons/abilities"), "source art")?;
    }
    let generated = root.join("assets/generated");
    let mut produced = std::collections::BTreeSet::new();
    let mut publish = |src: &Path, rel: &str, profile: Option<&str>| -> Result<()> {
        let dst = out.join(rel);
        pipeline.publish_asset(src, &dst, Path::new(rel), profile)?;
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
    let hero_art_slugs = sow_core::player::Leader::all()
        .map(sow_core::player::Leader::art_slug)
        .collect::<std::collections::BTreeSet<_>>();
    let mut avatar_portraits = Vec::new();
    for folder in ["heroes", "nations", "tribes", "campaign"] {
        for file in sorted_files(&art.join("avatars").join(folder))? {
            let stem = file_stem(&file);
            if folder == "heroes" {
                if pipeline.local_preview {
                    continue;
                }
                if expression_part(&stem).is_some() {
                    bail!(
                        "hero faces now belong in one 2 x 2 sheet: remove {}",
                        file.display()
                    );
                }
                if hero_art_slugs.contains(stem.as_str()) {
                    continue;
                }
                bail!(
                    "unexpected avatar {}: keep only playable hero sheets in avatars/heroes; general portraits belong in avatars/",
                    file.display()
                );
            }
            if let Some(first) = avatar_sources.insert(stem.clone(), file.clone()) && !pipeline.local_preview {
                bail!(
                    "avatar {stem} is in two folders: {} and {}",
                    first.display(),
                    file.display()
                );
            }
            avatar_portraits.push((file, format!("gameplay/avatars/{stem}.webp"), PORTRAIT_PX));
        }
    }
    let random_avatar = source_named(&art.join("avatars"), "random")
        .or_else(|| pipeline.local_preview.then(|| art.join("avatars/default.webp")))
        .context("missing art: assets/art/avatars/random.webp")?;
    if let Some(first) = avatar_sources.insert("random".to_owned(), random_avatar.clone()) && !pipeline.local_preview {
        bail!(
            "avatar random is in two folders: {} and {}",
            first.display(),
            random_avatar.display()
        );
    }
    avatar_portraits.push((
        random_avatar,
        "gameplay/avatars/random.webp".to_owned(),
        HERO_PORTRAIT_PX,
    ));
    avatar_portraits.push((
        art.join("avatars/default.webp"),
        "gameplay/avatars/null.webp".to_owned(),
        HERO_PORTRAIT_PX,
    ));
    let hero_crop_dir = tempfile::tempdir().context("create temporary hero avatar crops")?;
    let mut hero_expression_crops = Vec::new();
    for leader in sow_core::player::Leader::all() {
        let base = leader.art_slug();
        let source = art.join("avatars/heroes").join(format!("{base}.webp"));
        if !source.is_file() && !pipeline.local_preview {
            bail!(
                "hero art {base} needs one 2 x 2 sheet at {}",
                source.display()
            );
        }
        if let Some(first) = avatar_sources.insert(base.to_owned(), source.clone()) && !pipeline.local_preview {
            bail!(
                "avatar {base} is in two folders: {} and {}",
                first.display(),
                source.display()
            );
        }
        let sheet = match image::open(&source) {
            Ok(sheet) => sheet.to_rgba8(),
            Err(error) if pipeline.local_preview => {
                eprintln!("⚠ Local hero sheet {}: {error}", source.display());
                continue;
            }
            Err(error) => return Err(error).with_context(|| format!("read hero sheet {}", source.display())),
        };
        for expression in sow_core::emote::AvatarExpression::ALL {
            let name = expression.id();
            let crop = match hero_expression_crop(&sheet, expression) {
                Ok(crop) => crop,
                Err(error) if pipeline.local_preview => {
                    eprintln!("⚠ Local hero crop {}: {error:#}", source.display());
                    break;
                }
                Err(error) => return Err(error),
            };
            let crop_path = hero_crop_dir.path().join(format!("{base}_{name}.png"));
            crop.save(&crop_path)
                .with_context(|| format!("write {name} crop for hero {base}"))?;
            hero_expression_crops.push((
                crop_path,
                sow_core::emote::hero_portrait_path(base, expression),
            ));
        }
    }
    // Every playable hero needs a desktop and a mobile scene; a placeholder is fine.
    for leader in sow_core::player::Leader::all() {
        for variant in ["desktop", "mobile"] {
            let base = leader.art_slug();
            let has_scene = ["webp", "png", "jpg", "jpeg"].iter().any(|ext| {
                art.join("heroes").join(format!("{base}_{variant}.{ext}")).is_file()
            });
            if !has_scene && !pipeline.local_preview {
                bail!("hero {base} needs a {variant} scene in {}", art.join("heroes").display());
            }
        }
    }
    for file in sorted_files(&art.join("heroes"))? {
        let stem = file_stem(&file);
        if !stem.ends_with("_desktop") && !stem.ends_with("_mobile") {
            if pipeline.local_preview {
                continue;
            }
            bail!(
                "unknown hero scene {}: use <avatar>_desktop or <avatar>_mobile",
                file.display()
            );
        }
        publish(&file, &format!("shell/leaders/{stem}.webp"), None)?;
    }
    let mut known_icons = std::collections::BTreeSet::new();
    // Badge icons are packed by sow-tools into badges-atlas, not published one by one.
    if !pipeline.local_preview {
        let badges: serde_json::Value =
            serde_json::from_slice(&fs::read(generated.join("badges-atlas.json"))?)?;
        for cell in badges["cells"].as_array().into_iter().flatten() {
            let stem = cell.as_str().unwrap_or_default();
            known_icons.insert(
                source_named(&art.join("icons"), stem)
                    .with_context(|| format!("missing art: assets/art/icons/{stem}.webp"))?,
            );
        }
    }
    for (dir, stem, rel, profile) in ART_ICONS {
        let Some(src) = source_named(&art.join(dir), stem) else {
            if pipeline.local_preview {
                eprintln!("⚠ Local art missing: assets/art/{dir}/{stem}");
                publish(&art.join(dir).join(format!("{stem}.webp")), rel, Some(profile))?;
                continue;
            }
            bail!("missing art: assets/art/{dir}/{stem}.webp");
        };
        known_icons.insert(src.clone());
        publish(&src, rel, Some(profile))?;
    }
    for dir in ["icons", "store"].into_iter().filter(|_| !pipeline.local_preview) {
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
            let (relative, profile) = if dir == "units" {
                (format!("{rel_dir}/{}.webp", file_stem(&file)), None)
            } else if dir == "brand" && name == "app-icon.webp" {
                (format!("{rel_dir}/app-icon.png"), Some("app_icon"))
            } else {
                (format!("{rel_dir}/{name}"), None)
            };
            publish(&file, &relative, profile)?;
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
    for asset in SITE_STATIC_MEDIA {
        publish(&media.join(asset), &format!("site/media/{asset}"), None)?;
    }
    for (stem, (dir, provisional)) in SITE_STILLS {
        let src = source_named(&media, stem).or_else(|| source_named(&root.join(dir), provisional));
        let Some(src) = src else {
            if pipeline.local_preview {
                eprintln!("⚠ Local landing shot {stem} has no capture and no art");
                continue;
            }
            bail!("landing shot {stem} has no capture and no art");
        };
        let relative = format!("site/media/{stem}.webp");
        let profile = if stem.ends_with("-mobile") || stem == "support-six-sky" {
            "site_still_tall"
        } else {
            "site_still_wide"
        };
        publish(&src, &relative, Some(profile))?;
    }
    drop(publish);
    let icon_master = art.join("brand/app-icon.webp");
    let app_icons = out.join("site-icons");
    for (name, side) in [
        ("icon-192.png", 192),
        ("icon-512.png", 512),
        ("icon-512-maskable.png", 512),
    ] {
        let destination = app_icons.join(name);
        pipeline.publish_image(&icon_master, &destination, "app_icon", Some((side, side)), None)?;
        produced.insert(destination);
    }
    if !pipeline.local_preview {
        stage_android_icons(root, out, &icon_master, pipeline, &mut produced)?;
    }

    for (source, relative, max_side) in avatar_portraits {
        let profile = if max_side <= PORTRAIT_PX { "avatar_portrait_192" } else { "avatar_portrait_256" };
        pipeline.publish_image(&source, &out.join(&relative), profile, None, None)?;
        produced.insert(out.join(&relative));
    }
    for (source, relative) in hero_expression_crops {
        pipeline.publish_image(&source, &out.join(&relative), "avatar_portrait_256", None, None)?;
        produced.insert(out.join(&relative));
    }
    for file in fs::read_dir(&media)?.filter(|_| !pipeline.local_preview) {
        let file = file?.path();
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        let ext = file.extension().unwrap_or_default().to_string_lossy();
        let known = SITE_BADGES.contains(&name.as_ref())
            || SITE_STATIC_MEDIA.contains(&name.as_ref())
            || SITE_SOURCE_ONLY_MEDIA.contains(&name.as_ref())
            || (SITE_STILLS.iter().any(|(still, _)| *still == stem)
                && ["webp", "png", "jpg", "jpeg"].contains(&ext.as_ref()))
            || (SITE_VIDEOS.contains(&stem.as_ref()) && SITE_VIDEO_SOURCES.contains(&ext.as_ref()));
        if !known {
            bail!(
                "unregistered site media {}: add it to SITE_STATIC_MEDIA, SITE_STILLS, SITE_VIDEOS or SITE_SOURCE_ONLY_MEDIA",
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
        match publish_site_video(&src, &out.join("site/media"), stem, pipeline) {
            Ok(outputs) => produced.extend(outputs),
            Err(error) if pipeline.local_preview => {
                eprintln!("⚠ Local landing video {stem}: {error:#}");
                produced.extend(["webm", "mp4", "webp"].map(|ext| out.join(format!("site/media/{stem}.{ext}"))));
            }
            Err(error) => return Err(error),
        }
    }

    for entry in walkdir::WalkDir::new(out).contents_first(true) {
        let entry = entry?;
        if entry.file_type().is_file() && !produced.contains(entry.path()) {
            fs::remove_file(entry.path())?;
        } else if entry.file_type().is_dir() && entry.path() != out {
            let _ = fs::remove_dir(entry.path());
        }
    }
    if !pipeline.local_preview {
        pipeline.validate_staged_image_inventory(out)?;
    }
    Ok(())
}

fn stage_android_icons(
    root: &Path,
    out: &Path,
    icon_master: &Path,
    pipeline: &ArtPipeline,
    produced: &mut std::collections::BTreeSet<PathBuf>,
) -> Result<()> {
    let source_res = root.join("sow-dist/deploy/android/app/src/main/res");
    let generated_res = out.join("android/res");
    if generated_res.exists() {
        fs::remove_dir_all(&generated_res)?;
    }
    copy_dir(&source_res, &generated_res)
        .with_context(|| format!("stage Android resources from {}", source_res.display()))?;
    for entry in walkdir::WalkDir::new(&generated_res) {
        let entry = entry?;
        if entry.file_type().is_file() {
            produced.insert(entry.path().to_path_buf());
        }
    }
    for (density, side) in [
        ("mdpi", 48),
        ("hdpi", 72),
        ("xhdpi", 96),
        ("xxhdpi", 144),
        ("xxxhdpi", 192),
    ] {
        for icon in ["ic_launcher.png", "ic_launcher_round.png"] {
            let destination = generated_res
                .join(format!("mipmap-{density}"))
                .join(icon);
            pipeline.publish_image(icon_master, &destination, "app_icon", Some((side, side)), None)?;
            produced.insert(destination);
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
/// an avatar, music, or leader redraw invalidates only its own entry. Locale
/// catalogs are hashed straight from the exported bytes (locales_dir) —
/// WEB_CATALOG_VERSION alone is a hand-bumped constant, and immutable URLs must
/// never depend on someone remembering to bump it.
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
            // Progress bars retain build-stamped URLs. The shared splash art
            // uses a content hash so landing and loader reuse one cache key.
            let track = matches!(
                rel.as_str(),
                s if s.starts_with("shell/leaders/")
                    || s.starts_with("shell/mobile-nav/")
                    || s.starts_with("gameplay/music/")
                    || s.starts_with("gameplay/avatars/")
                    || s == "shell/loader/sow-splash-desktop.webp"
                    || s == "shell/loader/sow-splash-mobile.webp"
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

fn avatar_asset_catalog(avatars_root: &Path) -> Result<Vec<String>> {
    let mut catalog = Vec::new();
    for entry in walkdir::WalkDir::new(avatars_root) {
        let entry = entry?;
        if !entry.file_type().is_file()
            || !entry.path().extension().is_some_and(|ext| ext == "webp")
        {
            continue;
        }
        let relative = entry.path().strip_prefix(avatars_root)?;
        catalog.push(
            relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
        );
    }
    catalog.sort();
    Ok(catalog)
}

fn hash_avatar_asset_tree(hash: &mut Sha256, avatars_root: &Path) -> Result<()> {
    for relative in avatar_asset_catalog(avatars_root)? {
        hash.update(relative.as_bytes());
        hash.update([0]);
        hash.update(file_sha256(&avatars_root.join(relative))?.as_bytes());
        hash.update([0]);
    }
    Ok(())
}

fn poki_asset_is_excluded(relative: &Path) -> bool {
    relative.starts_with("gameplay/store")
        || relative.starts_with("gameplay/skins")
        || relative.starts_with("shell/brand")
        || relative.starts_with("site/media")
        || relative == Path::new("shell/mobile-nav/store.webp")
        || (relative.starts_with("campaign")
            && relative.extension().is_some_and(|extension| extension == "webm"))
}

fn asset_file_inventory(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(root)?.to_path_buf();
        files.push(relative);
    }
    files.sort();
    Ok(files)
}

fn poki_asset_inventory(root: &Path) -> Result<Vec<PathBuf>> {
    Ok(asset_file_inventory(root)?
        .into_iter()
        .filter(|relative| !poki_asset_is_excluded(relative))
        .collect())
}

fn cg_asset_is_excluded(relative: &Path) -> bool {
    relative.starts_with("shell/brand")
        || relative.starts_with("gameplay/avatars")
        || relative.starts_with("gameplay/icons/abilities")
        || (relative.starts_with("site/media")
            && ![
                "site/media/discord.svg",
                "site/media/telegram.svg",
                "site/media/github.svg",
            ]
            .iter()
            .any(|included| relative == Path::new(included)))
}

fn cg_asset_inventory(root: &Path) -> Result<Vec<PathBuf>> {
    let mut inventory = asset_file_inventory(root)?
        .into_iter()
        .filter(|relative| !cg_asset_is_excluded(relative))
        .collect::<Vec<_>>();
    inventory.sort();
    Ok(inventory)
}

/// Largest side of a CrazyGames avatar sheet; a full 2048 sheet decodes to 16 MB.
const CG_AVATAR_SHEET_PX: u32 = 2048;

/// One published avatar on its way into a sheet.
#[derive(Debug, Clone)]
struct AvatarSource {
    /// Path under `assets/`, e.g. `gameplay/avatars/caesar.webp`.
    key: String,
    /// Heroes share sheets; nations and tribes share sheets per region.
    group: String,
    side: u32,
    file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlacedAvatar {
    key: String,
    x: u32,
    y: u32,
    side: u32,
}

#[derive(Debug, PartialEq, Eq)]
struct AvatarSheet {
    /// Path under `assets/`, e.g. `gameplay/avatars/atlas/europe-192-0.webp`.
    path: String,
    width: u32,
    height: u32,
    cells: Vec<PlacedAvatar>,
}

/// Places avatars in square grids. Avatars of one group and size share sheets, sorted by key
/// so the layout is the same on every build.
fn plan_avatar_sheets(avatars: &[AvatarSource]) -> Result<Vec<AvatarSheet>> {
    let mut groups: std::collections::BTreeMap<(&str, u32), Vec<&AvatarSource>> = Default::default();
    for avatar in avatars {
        groups.entry((avatar.group.as_str(), avatar.side)).or_default().push(avatar);
    }
    let mut sheets = Vec::new();
    for ((group, side), mut members) in groups {
        if side == 0 || side > CG_AVATAR_SHEET_PX {
            bail!("avatars in group {group} are {side} px; a sheet holds cells of 1..={CG_AVATAR_SHEET_PX} px");
        }
        members.sort_by(|a, b| a.key.cmp(&b.key));
        let per_row = CG_AVATAR_SHEET_PX / side;
        for (index, chunk) in members.chunks((per_row * per_row) as usize).enumerate() {
            let columns = per_row.min(chunk.len() as u32);
            let rows = (chunk.len() as u32).div_ceil(columns);
            let cells = chunk
                .iter()
                .enumerate()
                .map(|(slot, avatar)| {
                    let slot = slot as u32;
                    PlacedAvatar {
                        key: avatar.key.clone(),
                        x: (slot % columns) * side,
                        y: (slot / columns) * side,
                        side,
                    }
                })
                .collect();
            sheets.push(AvatarSheet {
                path: format!("gameplay/avatars/atlas/{group}-{side}-{index}.webp"),
                width: columns * side,
                height: rows * side,
                cells,
            });
        }
    }
    Ok(sheets)
}

/// Region name for each avatar stem, from the shared entity catalog.
fn avatar_region_groups() -> std::collections::BTreeMap<String, String> {
    sow_data::geo_entities::all()
        .filter(|entity| entity.region != sow_data::geo_entities::Region::Unassigned)
        .filter_map(|entity| {
            let avatar = entity.avatar.clone()?;
            Some((avatar, format!("{:?}", entity.region).to_lowercase()))
        })
        .collect()
}

/// Packs every published avatar under `assets/gameplay/avatars` into shared sheets and removes
/// the loose files. Returns the index the CrazyGames shell reads as `window.SOW_AVATAR_ATLAS`.
fn pack_cg_avatar_sheets(packaged: &Path, pipeline: &ArtPipeline) -> Result<serde_json::Value> {
    let root = packaged.join("gameplay/avatars");
    let regions = avatar_region_groups();
    let heroes = sow_core::player::Leader::all()
        .map(|leader| leader.art_slug().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let mut avatars = Vec::new();
    for entry in walkdir::WalkDir::new(&root).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let file = entry.path().to_path_buf();
        if file.extension().and_then(|ext| ext.to_str()) != Some("webp") {
            bail!("CrazyGames avatar {} must be a .webp portrait", file.display());
        }
        let key = file.strip_prefix(packaged)?.to_string_lossy().replace('\\', "/");
        let stem = file_stem(&file);
        let group = if key.starts_with("gameplay/avatars/expressions/") || heroes.contains(&stem) {
            "heroes".to_owned()
        } else {
            regions.get(&stem).cloned().unwrap_or_else(|| "other".to_owned())
        };
        let (width, height) =
            image::image_dimensions(&file).with_context(|| format!("read avatar {}", file.display()))?;
        if width != height {
            bail!("avatar {} must be square, not {width}x{height}", file.display());
        }
        avatars.push(AvatarSource { key, group, side: width, file });
    }

    let sheets = plan_avatar_sheets(&avatars)?;
    let files = avatars
        .iter()
        .map(|avatar| (avatar.key.as_str(), avatar.file.as_path()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let scratch = tempfile::tempdir().context("create temporary CrazyGames avatar sheets")?;
    let mut index_sheets = Vec::new();
    let mut index_cells = serde_json::Map::new();
    for (sheet_index, sheet) in sheets.iter().enumerate() {
        let mut canvas = image::RgbaImage::new(sheet.width, sheet.height);
        for cell in &sheet.cells {
            let portrait = image::open(files[cell.key.as_str()])
                .with_context(|| format!("read avatar {}", cell.key))?
                .to_rgba8();
            image::imageops::overlay(&mut canvas, &portrait, cell.x.into(), cell.y.into());
            index_cells.insert(
                cell.key.clone(),
                serde_json::json!([sheet_index, cell.x, cell.y, cell.side]),
            );
        }
        let raw = scratch.path().join(format!("sheet-{sheet_index}.png"));
        canvas
            .save(&raw)
            .with_context(|| format!("write avatar sheet {}", sheet.path))?;
        pipeline.publish_image(&raw, &packaged.join(&sheet.path), "avatar_atlas", None, None)?;
        index_sheets.push(serde_json::json!({
            "path": sheet.path,
            "width": sheet.width,
            "height": sheet.height,
        }));
    }
    // Loose portraits go only after every sheet is published, so a failed pack leaves the package whole.
    for avatar in &avatars {
        fs::remove_file(&avatar.file)?;
    }
    let expressions = root.join("expressions");
    if expressions.is_dir() {
        fs::remove_dir_all(expressions)?;
    }
    Ok(serde_json::json!({ "sheets": index_sheets, "cells": index_cells }))
}

fn copy_cg_assets(source: &Path, packaged: &Path, pipeline: &ArtPipeline) -> Result<serde_json::Value> {
    copy_dir(source, packaged)?;
    for excluded in [
        "shell/brand",
        "site/media",
        "gameplay/icons/abilities",
    ] {
        let path = packaged.join(excluded);
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        }
    }
    for icon in ["discord.svg", "telegram.svg", "github.svg"] {
        let from = source.join("site/media").join(icon);
        if !from.is_file() {
            bail!("CrazyGames runtime asset is missing {}", from.display());
        }
        let to = packaged.join("site/media").join(icon);
        fs::create_dir_all(to.parent().context("social icon has no parent")?)?;
        pipeline.publish_asset(
            &from,
            &to,
            Path::new(&format!("site/media/{icon}")),
            None,
        )?;
    }
    build_cg_ability_atlas(
        &source.join("gameplay/icons/abilities"),
        &packaged.join("gameplay/icons/abilities-atlas.webp"),
        pipeline,
    )
}

fn build_cg_ability_atlas(source: &Path, destination: &Path, pipeline: &ArtPipeline) -> Result<serde_json::Value> {
    const CELL_SIZE: u32 = 256;
    const COLUMNS: u32 = 9;

    let mut slugs = sow_core::player::Leader::all()
        .map(|leader| leader.art_slug().to_owned())
        .collect::<Vec<_>>();
    slugs.sort();
    slugs.dedup();
    if slugs.len() != sow_core::player::Leader::all().count() {
        bail!("playable hero ability icon slugs are not unique");
    }
    let rows = (slugs.len() as u32).div_ceil(COLUMNS);
    let mut atlas = image::RgbaImage::new(COLUMNS * CELL_SIZE, rows * CELL_SIZE);
    let mut icons = serde_json::Map::new();

    for (index, slug) in slugs.iter().enumerate() {
        let path = source.join(format!("{slug}.webp"));
        let image = image::open(&path)
            .with_context(|| format!("read playable hero ability icon {}", path.display()))?
            .into_rgba8();
        let image = if image.width() != CELL_SIZE || image.height() != CELL_SIZE {
            image::imageops::resize(
                &image,
                CELL_SIZE,
                CELL_SIZE,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            image
        };
        let index = index as u32;
        image::imageops::overlay(
            &mut atlas,
            &image,
            ((index % COLUMNS) * CELL_SIZE).into(),
            ((index / COLUMNS) * CELL_SIZE).into(),
        );
        icons.insert(slug.clone(), serde_json::Value::from(index));
    }

    let mut encoded = Vec::new();
    image::ImageEncoder::write_image(
        image::codecs::webp::WebPEncoder::new_lossless(&mut encoded),
        atlas.as_raw(),
        atlas.width(),
        atlas.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = destination.with_file_name("abilities-atlas-source.webp");
    fs::write(&raw, encoded)?;
    pipeline.publish_image(&raw, destination, "atlas_lossless", None, None)?;
    fs::remove_file(raw)?;
    let sha256 = file_sha256(destination)?;

    Ok(serde_json::json!({
        "path": "gameplay/icons/abilities-atlas.webp",
        "sha256": sha256,
        "cellSize": CELL_SIZE,
        "columns": COLUMNS,
        "rows": rows,
        "icons": icons,
    }))
}

fn verify_cg_asset_inventory(source: &Path, packaged: &Path) -> Result<()> {
    let html = fs::read_to_string(
        packaged
            .parent()
            .context("CrazyGames assets folder has no bundle root")?
            .join("index.html"),
    )?;
    let avatar_atlas = cg_avatar_atlas_config(&html)?;
    let mut expected = cg_asset_inventory(source)?;
    expected.push(PathBuf::from("gameplay/icons/abilities-atlas.webp"));
    for sheet in avatar_atlas["sheets"].as_array().into_iter().flatten() {
        expected.push(PathBuf::from(sheet["path"].as_str().unwrap_or_default()));
    }
    expected.sort();
    let actual = asset_file_inventory(packaged)?;
    if expected != actual {
        let missing = expected
            .iter()
            .filter(|relative| !actual.contains(relative))
            .take(5)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let unexpected = actual
            .iter()
            .filter(|relative| !expected.contains(relative))
            .take(5)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        bail!(
            "CrazyGames runtime asset inventory differs (source={}, package={}, missing={missing:?}, unexpected={unexpected:?})",
            expected.len(),
            actual.len()
        );
    }
    let indexed = avatar_atlas["cells"]
        .as_object()
        .map(|cells| cells.keys().cloned().collect::<std::collections::BTreeSet<_>>())
        .unwrap_or_default();
    let published = asset_file_inventory(&source.join("gameplay/avatars"))?
        .into_iter()
        .map(|relative| format!("gameplay/avatars/{}", relative.to_string_lossy()))
        .collect::<std::collections::BTreeSet<_>>();
    if indexed != published {
        bail!("CrazyGames avatar index does not list exactly the published avatars");
    }
    for relative in expected {
        if relative == Path::new("gameplay/icons/abilities-atlas.webp")
            || relative.starts_with("gameplay/avatars/atlas")
        {
            continue;
        }
        if file_sha256(&source.join(&relative))? != file_sha256(&packaged.join(&relative))? {
            bail!(
                "CrazyGames runtime asset content mismatch: {}",
                relative.display()
            );
        }
    }
    Ok(())
}

/// The avatar index injected into the CrazyGames `index.html`.
fn cg_avatar_atlas_config(html: &str) -> Result<serde_json::Value> {
    let marker = "window.SOW_AVATAR_ATLAS = ";
    let start = html
        .find(marker)
        .context("CrazyGames avatar atlas config is missing")?
        + marker.len();
    let end = html[start..]
        .find(';')
        .context("CrazyGames avatar atlas config is unterminated")?
        + start;
    serde_json::from_str(&html[start..end]).context("CrazyGames avatar atlas config is invalid JSON")
}

/// Checks the avatar index the shell reads: every sheet exists at its size, every cell lies
/// inside its sheet, and every playable hero has its calm and expression portraits.
fn verify_cg_avatar_atlas(dir: &Path, html: &str) -> Result<()> {
    let config = cg_avatar_atlas_config(html)?;
    let mut sizes = Vec::new();
    for sheet in config["sheets"].as_array().context("CrazyGames avatar atlas has no sheets")? {
        let path = sheet["path"].as_str().context("CrazyGames avatar sheet has no path")?;
        let (width, height) = image::image_dimensions(dir.join("assets").join(path))
            .with_context(|| format!("CrazyGames avatar sheet is missing: {path}"))?;
        if width > CG_AVATAR_SHEET_PX || height > CG_AVATAR_SHEET_PX {
            bail!("CrazyGames avatar sheet {path} is {width}x{height}; the limit is {CG_AVATAR_SHEET_PX}");
        }
        sizes.push((width, height));
    }
    let cells = config["cells"].as_object().context("CrazyGames avatar atlas has no cells")?;
    for (key, cell) in cells {
        let values = cell.as_array().filter(|values| values.len() == 4).context("CrazyGames avatar cell is malformed")?;
        let [sheet, x, y, side] = [0, 1, 2, 3].map(|index| values[index].as_u64().unwrap_or(u64::MAX));
        let (width, height) = *sizes
            .get(sheet as usize)
            .with_context(|| format!("CrazyGames avatar {key} points at a missing sheet"))?;
        if side == 0 || x + side > u64::from(width) || y + side > u64::from(height) {
            bail!("CrazyGames avatar {key} leaves its sheet");
        }
    }
    for leader in sow_core::player::Leader::all() {
        for expression in sow_core::emote::AvatarExpression::ALL {
            let key = sow_core::emote::hero_portrait_path(leader.art_slug(), expression);
            if !cells.contains_key(&key) {
                bail!("CrazyGames avatar atlas is missing {key}");
            }
        }
    }
    Ok(())
}

fn verify_poki_asset_inventory(source: &Path, packaged: &Path) -> Result<()> {
    let excluded = asset_file_inventory(packaged)?
        .into_iter()
        .filter(|relative| poki_asset_is_excluded(relative))
        .take(5)
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if !excluded.is_empty() {
        bail!("Poki bundle contains excluded runtime assets: {excluded:?}");
    }
    let expected = poki_asset_inventory(source)?;
    let actual = poki_asset_inventory(packaged)?;
    if expected != actual {
        let missing = expected
            .iter()
            .filter(|relative| !actual.contains(relative))
            .take(5)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let unexpected = actual
            .iter()
            .filter(|relative| !expected.contains(relative))
            .take(5)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        bail!(
            "Poki runtime asset inventory differs (source={}, package={}, missing={missing:?}, unexpected={unexpected:?})",
            expected.len(),
            actual.len()
        );
    }
    for relative in expected {
        let source_path = source.join(&relative);
        let packaged_path = packaged.join(&relative);
        let is_leader_art = relative.starts_with("shell/leaders")
            && relative.extension().is_some_and(|extension| extension == "webp");
        if is_leader_art {
            let (width, height) = image::image_dimensions(&packaged_path)
                .with_context(|| format!("read packaged leader art {}", relative.display()))?;
            let (max_width, max_height) = if relative.to_string_lossy().ends_with("_mobile.webp") {
                (540, 960)
            } else {
                (960, 540)
            };
            if width > max_width || height > max_height {
                bail!(
                    "Poki leader art {} is {width}x{height}; expected at most {max_width}x{max_height}",
                    relative.display()
                );
            }
            continue;
        }
        if relative.starts_with("campaign")
            && relative.file_name().is_some_and(|name| {
                name.to_string_lossy().ends_with(".triggers.json")
            })
        {
            let expected = fs::read_to_string(&source_path)?.replace(".webm\"", ".mp4\"");
            if fs::read_to_string(&packaged_path)? != expected {
                bail!("Poki campaign trigger mismatch: {}", relative.display());
            }
        } else if file_sha256(&source_path)? != file_sha256(&packaged_path)? {
            bail!("Poki runtime asset content mismatch: {}", relative.display());
        }
    }
    Ok(())
}

fn avatar_asset_versions(avatars_root: &Path) -> Result<serde_json::Value> {
    let mut versions = serde_json::Map::new();
    for relative in avatar_asset_catalog(avatars_root)? {
        let path = avatars_root.join(&relative);
        versions.insert(
            format!("/assets/gameplay/avatars/{relative}"),
            serde_json::Value::String(file_sha256(&path)?[..8].to_string()),
        );
    }
    Ok(serde_json::Value::Object(versions))
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
        HERO_PORTRAIT_PX,
        HERO_PORTRAIT_PX,
        image::imageops::FilterType::Triangle,
    ))
}

/// A landing capture becomes `<stem>.webm` (VP9), `<stem>.mp4` (H.264) and a
/// `<stem>.webp` poster. Encoding is slow, so outputs newer than the capture are kept.
fn publish_site_video(src: &Path, dir: &Path, stem: &str, pipeline: &ArtPipeline) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let webm = dir.join(format!("{stem}.webm"));
    let mp4 = dir.join(format!("{stem}.mp4"));
    let poster = dir.join(format!("{stem}.webp"));
    let outputs = vec![webm.clone(), mp4.clone(), poster.clone()];
    let source_time = fs::metadata(src)?.modified()?;
    let videos_fresh = [&webm, &mp4].iter().all(|path| {
        fs::metadata(path)
            .and_then(|meta| meta.modified())
            .is_ok_and(|time| time >= source_time)
    });
    let source_hash = file_sha256(src)?;
    let poster_profile = pipeline.profile("video_poster")?;
    let poster_recipe = pipeline.image_recipe_hash(
        "video_poster",
        poster_profile,
        None,
        poster_profile.quality,
        false,
    )?;
    let poster_record = pipeline.cache_root.join("video-posters").join(format!("{stem}-{source_hash}.txt"));
    let poster_hash = file_sha256(&poster).ok();
    let poster_fresh = poster_hash.as_ref().is_some_and(|hash| {
        fs::read_to_string(&poster_record).is_ok_and(|record| record == format!("{poster_recipe};{hash}"))
    });
    if videos_fresh && poster_fresh {
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
    if !videos_fresh {
        ffmpeg(&args, &webm)?;
    }
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
    if !videos_fresh {
        ffmpeg(&args, &mp4)?;
    }
    let frame = dir.join(format!("{stem}.poster.png"));
    ffmpeg(&["-vf", scale, "-frames:v", "1"], &frame)?;
    pipeline.publish_image(&frame, &poster, "video_poster", None, None)?;
    fs::remove_file(&frame)?;
    fs::create_dir_all(poster_record.parent().context("video poster cache has no parent")?)?;
    fs::write(poster_record, format!("{poster_recipe};{}", file_sha256(&poster)?))?;
    Ok(outputs)
}

fn optimize_poki_thumbnail(src: &Path, dst: &Path, pipeline: &ArtPipeline) -> Result<()> {
    pipeline.publish_image(src, dst, "poki_thumbnail", None, None)
}

fn copy_poki_assets(src: &Path, dst: &Path, pipeline: &ArtPipeline) -> Result<()> {
    copy_dir(src, dst)?;
    for excluded in [
        "gameplay/store",
        "gameplay/skins",
        "shell/brand",
        "site/media",
    ] {
        let path = dst.join(excluded);
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        }
    }
    let site = dst.join("site");
    let _ = fs::remove_dir(site);
    let store_nav = dst.join("shell/mobile-nav/store.webp");
    if store_nav.is_file() {
        fs::remove_file(store_nav)?;
    }
    let campaign = dst.join("campaign");
    if campaign.is_dir() {
        fs::remove_dir_all(&campaign)?;
    }
    copy_campaign_for_poki(&src.join("campaign"), &campaign)?;

    for entry in fs::read_dir(src.join("shell/leaders"))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".webp") {
            continue;
        }
        let profile = if name.ends_with("_mobile.webp") {
            "poki_leader_tall"
        } else {
            "poki_leader_wide"
        };
        pipeline.publish_image(
            &entry.path(),
            &dst.join("shell/leaders").join(entry.file_name()),
            profile,
            None,
            None,
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

/// Release builds include only maps with a recorded recipe. Map Generator output
/// stays local until uploaded to SOW_MAPS_UPLOAD_ROOT, even though it shares the
/// assets/maps workspace used by the authored map library.
fn copy_release_maps(src: &Path, dst: &Path, sources_path: &Path) -> Result<()> {
    let sources: toml::Value = fs::read_to_string(sources_path)?
        .parse()
        .context("parse official map source catalog")?;
    let recipes = sources
        .get("recipes")
        .and_then(toml::Value::as_table)
        .context("map source catalog has no recipes")?;
    let mut published = recipes.keys().cloned().collect::<Vec<_>>();
    published.sort();
    let published_set = published.iter().cloned().collect::<HashSet<_>>();
    let mut catalog = sow_core::map_file::parse_catalog(&fs::read(src.join("catalog.bin"))?)
        .map_err(|error| anyhow::anyhow!("read map catalog: {error}"))?;
    catalog
        .entries
        .retain(|entry| published_set.contains(&entry.key));
    if catalog.entries.len() != published.len() {
        bail!(
            "map source catalog lists {} maps, but assets/maps/catalog.bin contains {} of them",
            published.len(),
            catalog.entries.len()
        );
    }

    fs::create_dir_all(dst)?;
    fs::copy(src.join("NOTICE"), dst.join("NOTICE"))?;
    fs::copy(src.join("SOURCES.toml"), dst.join("SOURCES.toml"))?;
    for map_id in &published {
        let map_dir = src.join(map_id);
        for name in ["map.bin", "map.bin.br"] {
            require_file(&map_dir.join(name), &format!("release map {map_id}/{name}"))?;
        }
        copy_dir(&map_dir, &dst.join(map_id))?;
    }
    fs::write(
        dst.join("catalog.bin"),
        sow_core::map_file::encode_catalog(&catalog),
    )?;
    Ok(())
}

fn copy_portal_maps(src: &Path, dst: &Path, pipeline: &ArtPipeline) -> Result<()> {
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
        let preview = entry.path().join("preview.webp");
        if preview.is_file() {
            pipeline.publish_image(
                &preview,
                &out_dir.join("thumbnail.webp"),
                "map_thumbnail",
                None,
                None,
            )?;
        }
    }
    Ok(())
}

fn portal_map_inventory(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![PathBuf::from("catalog.bin"), PathBuf::from("NOTICE")];
    for entry in fs::read_dir(root).with_context(|| format!("read map directory {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        for name in ["map.bin.br", "thumbnail.webp"] {
            if entry.path().join(name).is_file() {
                files.push(PathBuf::from(entry.file_name()).join(name));
            }
        }
    }
    files.sort();
    Ok(files)
}

fn verify_portal_map_inventory(source: &Path, packaged: &Path) -> Result<()> {
    let expected = portal_map_inventory(source)?;
    let actual = asset_file_inventory(packaged)?;
    if expected != actual {
        let missing = expected
            .iter()
            .filter(|relative| !actual.contains(relative))
            .take(5)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let unexpected = actual
            .iter()
            .filter(|relative| !expected.contains(relative))
            .take(5)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        bail!(
            "portal map inventory differs (source={}, package={}, missing={missing:?}, unexpected={unexpected:?})",
            expected.len(),
            actual.len()
        );
    }
    for relative in expected {
        let source_path = source.join(&relative);
        let packaged_path = packaged.join(&relative);
        if relative.file_name().is_some_and(|name| name == "thumbnail.webp") {
            let (width, height) = image::image_dimensions(&packaged_path)
                .with_context(|| format!("read packaged map thumbnail {}", relative.display()))?;
            if width > 256 || height > 144 {
                bail!(
                    "portal map thumbnail {} is {width}x{height}; expected at most 256x144",
                    relative.display()
                );
            }
        } else if file_sha256(&source_path)? != file_sha256(&packaged_path)? {
            bail!("portal map content mismatch: {}", relative.display());
        }
    }
    Ok(())
}

fn refresh_map_thumbnails(maps_root: &Path, map_sources: &Path, pipeline: &ArtPipeline) -> Result<()> {
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
        if let Some(sources) = value.get("sources").and_then(|v| v.as_object()) && !pipeline.local_preview {
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
        let raw_render = dir.join(".thumbnail-source.webp");
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
            sow_map::write_rendered_source_thumbnail(rendered, frame, &raw_render)
                .map_err(|error| anyhow::anyhow!("write {} thumbnail: {error}", dir.display()))?;
        } else {
            sow_map::write_map_thumbnail(&map, &raw_render)
                .map_err(|error| anyhow::anyhow!("write {} thumbnail: {error}", dir.display()))?;
        }
        // Both derivatives come from the same fresh render, so portal compression never
        // compounds the map preview's lossy WebP encoding.
        let preview = dir.join("preview.webp");
        let thumbnail = dir.join("thumbnail.webp");
        pipeline.publish_image(&raw_render, &preview, "map_preview", None, None)?;
        pipeline.publish_image(&raw_render, &thumbnail, "map_thumbnail", None, None)?;
        fs::remove_file(raw_render)?;
    }
    Ok(())
}

fn thumbnail_cache_bust(maps_root: &Path) -> Result<String> {
    let mut files = fs::read_dir(maps_root)?
        .filter_map(|entry| entry.ok())
        .flat_map(|entry| ["thumbnail.webp", "preview.webp"].map(|name| entry.path().join(name)))
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
    if !paths.is_local_preview() {
        stage_runtime_assets(&paths.root, &paths.runtime_assets, paths.art_pipeline()?)?;
    }
    Ok(())
}

fn compile_wasm_profile(paths: &Paths, profile: &str, dev: bool) -> Result<PathBuf> {
    let profile_dir = match profile {
        "dev" => "debug",
        "wasm-dev" => "wasm-dev",
        "wasm-release" => "wasm-release",
        _ => bail!("unsupported WASM profile: {profile}"),
    };
    if let Err(error) = pack_art_atlases(paths) {
        if !paths.is_local_preview() {
            return Err(error);
        }
        eprintln!("⚠ Local atlas generation: {error:#}");
    }
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

fn compile_shell_ts(shell: &Path, part: &str) -> Result<String> {
    compile_browser_ts(&shell.join(part), "iife", &format!("shell part {part}"))
}

fn compile_browser_ts(path: &Path, format: &str, context: &str) -> Result<String> {
    let output = Command::new("bun")
        .args(["build", "--target=browser", &format!("--format={format}")])
        .arg(path)
        .output()
        .with_context(|| format!("compile TypeScript {context} (Bun required)"))?;
    if !output.status.success() {
        bail!(
            "compile TypeScript {context}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).with_context(|| format!("TypeScript {context} is not UTF-8"))
}

fn read_shell_bundle(shell: &Path, manifest: &str, parts: &[&str], local_preview: bool) -> Result<String> {
    let manifest_source = if local_preview {
        None
    } else {
        Some(fs::read_to_string(shell.join(manifest))?)
    };
    // The part banner is attached first so it stays visible in the output.
    let mut bundle = format!("/* shell bundle parts: {} */\n", parts.join(" "));
    for part in parts {
        if manifest_source.as_ref().is_some_and(|source| !source.contains(part)) {
            bail!("{manifest}: missing bundle part {part}");
        }
        let source = fs::read_to_string(shell.join(part))
            .with_context(|| format!("read shell bundle part {part}"));
        match source {
            Ok(source) => bundle.push_str(&source),
            Err(error) if local_preview => eprintln!("⚠ Local CSS: {error:#}"),
            Err(error) => return Err(error),
        }
        bundle.push('\n');
    }
    Ok(bundle)
}

/// Bundles the menu from its ES entry, which imports the whole menu graph.
/// Portal builds compile a copy of the shell in which the surface seam
/// (main_menu.surfaces.ts) is the portal's surface, so every importer of the
/// seam gets the portal implementation, not only the entry.
fn read_menu_bundle(
    shell: &Path,
    portal_surface: Option<&str>,
    local_preview: bool,
) -> Result<String> {
    let scratch = tempfile::Builder::new()
        .prefix(".sow-menu-")
        .tempdir_in(shell.parent().context("shell directory has no parent")?)
        .context("create menu build directory")?;
    let build = scratch.path().join("shell");
    copy_dir(shell, &build)?;
    if let Some(surface) = portal_surface {
        fs::copy(shell.join(surface), build.join("main_menu.surfaces.ts"))
            .with_context(|| format!("copy portal surface {surface}"))?;
        let shell_file = build.join("main_menu.shell.ts");
        let source = fs::read_to_string(&shell_file)?;
        fs::write(&shell_file, strip_portal_menu_sections(&source)?)?;
    }
    let code = compile_browser_ts(&build.join("main_menu.ts"), "iife", "main menu")?;
    if !local_preview {
        ensure_shell_entry_runs("main_menu.ts", &code)?;
    }
    // Bun strips comments while bundling, so the banner is attached after.
    let surface = portal_surface.unwrap_or("main_menu.surfaces.ts");
    let mut bundle = format!("/* menu bundle: main_menu.ts surface {surface} */\n{code}");
    if !bundle.ends_with('\n') {
        bundle.push('\n');
    }
    if !local_preview {
        validate_web_bundle(&bundle)?;
    }
    Ok(bundle)
}

fn ensure_shell_entry_runs(manifest: &str, code: &str) -> Result<()> {
    // A bundled entry has to execute at load time. Bun wraps CommonJS entries in
    // a `var require_x = __commonJS(function (...) { ... })` factory and, when
    // the entry is not an ES module, never invokes it: the bundle parses, puts
    // nothing on `window`, and the loader stays on "Loading..." forever.
    let Some(pos) = code.find("= __commonJS(") else {
        return Ok(());
    };
    let Some(decl) = code[..pos].rfind("var ") else {
        return Ok(());
    };
    let ident = code[decl + 4..pos].trim();
    if ident.is_empty()
        || !ident
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    {
        return Ok(());
    }
    if !code.contains(&format!("{ident}()")) {
        bail!("{manifest}: shell bundle entry {ident} is never invoked (CommonJS entry wrapper)");
    }
    Ok(())
}

fn strip_portal_menu_sections(source: &str) -> Result<String> {
    let mut source = source.to_string();
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
        source = strip_marked_section(&source, begin, end)?;
    }
    Ok(source)
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
    let catalog = serde_json::to_value(&sow_i18n::web(sow_i18n::Language::English))?;
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
    const DOMAINS: [&str; 11] = [
        "menu", "auth", "lobbies", "heroes", "profile", "store", "hud", "endgame", "tutorial",
        "site", "ranked",
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
    let experience_events =
        compile_shell_ts(&paths.shell, "experience_events.ts")?.replace("</script>", "<\\/script>");
    let experience_events_slot = "/* __INLINE_EXPERIENCE_EVENTS_JS__ */";
    if !paths.is_local_preview() && tpl.matches(experience_events_slot).count() != 1 {
        bail!("game HTML template must contain one experience-events slot");
    }
    let source_slot = "<!-- __SOW_SOURCE_IDENTITY_SLOT__ -->";
    if !paths.is_local_preview() && tpl.matches(source_slot).count() != 1 {
        bail!("game HTML template must contain one source identity slot");
    }
    let source = source_identity(paths)?;
    let source_identity_html = format!(
        "<script>window.SOW_SOURCE_SHA = {}; window.SOW_SOURCE_URL = {};</script>",
        serde_json::to_string(&source.sha)?,
        serde_json::to_string(&source.url)?,
    );
    // Avatar files are copied byte-for-byte into portal packages or served from
    // the self-hosted asset CDN, so their cache keys remain stable across builds.
    // Other portal assets can be re-encoded and keep the build-level bust.
    let asset_versions = if portal {
        avatar_asset_versions(&paths.assets_gameplay.join("avatars"))?
    } else {
        asset_versions(paths, &out.join("locales"))?
    };
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
        .replace(
            "__ASSETS_UI_BASE__",
            if cg {
                // CrazyGames hosts this uploaded asset tree on its CDN.
                "./assets/shell/loader/"
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
    if !paths.is_local_preview()
        && (html.matches("name=\"sow-media-artwork\"").count() != 1
            || !html.contains(media_artwork_reference))
    {
        bail!("index.html has an invalid game music artwork path");
    }
    let index = if portal {
        out.join("index.html")
    } else {
        out.join("play/index.html")
    };
    fs::create_dir_all(index.parent().unwrap())?;
    let mut loader = compile_shell_ts(&paths.shell, "loader.ts")?;
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
            "main_menu.missions.css",
        ],
        paths.is_local_preview(),
    )?;
    let menu_surface = if poki {
        Some("main_menu.poki.ts")
    } else if jest {
        Some("main_menu.jest.ts")
    } else {
        None
    };
    let building_manifest = fs::read(paths.assets_gameplay.join("buildings/atlas.json"))
        .context("read building atlas manifest")
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes)
            .context("parse building atlas manifest"));
    let building_manifest = match building_manifest {
        Ok(manifest) => manifest,
        Err(error) if paths.is_local_preview() => {
            eprintln!("⚠ Local building atlas: {error:#}");
            serde_json::Value::Null
        }
        Err(error) => return Err(error),
    };
    let building_manifest_js = format!(
        "window.SOW_BUILDING_ART = {};\n",
        serde_json::to_string(&building_manifest)?
    );
    let mut menu_js = format!(
        "{building_manifest_js}{}",
        read_menu_bundle(&paths.shell, menu_surface, paths.is_local_preview())?
    )
    .replace("</script>", "<\\/script>");
    if poki || jest {
        for forbidden in [
            "https://id.worldofunreal.com",
            "https://discord.gg/4mBM6zJyG",
            "https://t.me/shadowsofwario",
            "https://github.com/worldofunreal/shadows-of-war",
            "https://worldofunreal.com/wouid.svg",
        ] {
            menu_js = menu_js.replace(forbidden, "about:blank");
        }
    }
    let mut fh = html;
    for bar in ["loader_empty", "loader_full", "sow-splash-desktop", "sow-splash-mobile"] {
        if let Some(hash) = asset_versions
            .get(format!("/assets/shell/loader/{bar}.webp"))
            .and_then(|v| v.as_str())
        {
            fh = fh.replace(
                &format!("{bar}.webp?v={ts}"),
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
    if !paths.is_local_preview() && !fh.contains(menu_css_marker) {
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
                &format!("href=\"fonts/fonts.css?v={ts}\""),
            )
            .replace(
                "    <link rel=\"manifest\" href=\"/manifest.webmanifest\">\n",
                "",
            )
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
            .replace(
                "<link rel=\"canonical\" href=\"https://shadowsofwar.io/play/\">",
                "",
            );
    }
    fs::write(&index, fh)?;
    if paths.is_local_preview() && let Ok(entries) = fs::read_dir(out.join("play")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("main_menu_")
                && name.ends_with(".js")
                && !menu_js_injection.contains(name.as_str())
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

fn copy_shell(paths: &Paths, out: &Path, target: WebTarget) -> Result<()> {
    let fav = paths.shell.join("favicon_io");
    if fav.is_dir() {
        paths.art_pipeline()?.copy_distribution_tree(
            &fav,
            out,
            Path::new("sow-web/shell/favicon_io"),
        )?;
    }
    let logo = paths.assets_shell.join("brand/sow.svg");
    if logo.is_file() || !paths.is_local_preview() {
        fs::copy(logo, out.join("sow.svg"))?;
    }
    if matches!(
        target,
        WebTarget::CrazyGames | WebTarget::Poki | WebTarget::Jest
    ) {
        fs::copy(paths.app_icons.join("icon-192.png"), out.join("icon-192.png"))?;
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
    fs::write(
        out.join("loader.js"),
        compile_shell_ts(&paths.shell, "loader.ts")?,
    )?;
    if matches!(target, WebTarget::Local) {
        fs::write(
            out.join("analytics.js"),
            compile_browser_ts(
                &paths.root.join("sow-web/site/analytics.ts"),
                "iife",
                "site analytics",
            )?,
        )?;
    }
    fs::write(
        out.join("sow-i18n.js"),
        compile_shell_ts(&paths.shell, "sow-i18n.ts")?,
    )?;
    fs::write(
        out.join("sow-dropdown.js"),
        compile_shell_ts(&paths.shell, "sow-dropdown.ts")?,
    )?;
    fs::copy(
        paths.shell.join("sow-controls.css"),
        out.join("sow-controls.css"),
    )?;
    fs::create_dir_all(out.join("sdk"))?;
    for bridge in ["store_portals", "poki_portals", "jest_portals"] {
        fs::write(
            out.join("sdk").join(format!("{bridge}.js")),
            compile_shell_ts(&paths.shell, &format!("sdk/{bridge}.ts"))?,
        )?;
    }
    for (source, destination) in [
        ("LICENSE", "LICENSE"),
        ("docs/legal/NOTICE", "NOTICE"),
        ("docs/legal/LICENSE-ASSETS", "LICENSE-ASSETS"),
        (
            "docs/legal/WASM-THIRD-PARTY-NOTICES.zip",
            "WASM-THIRD-PARTY-NOTICES.zip",
        ),
    ] {
        let source = paths.root.join(source);
        if source.is_file() || !paths.is_local_preview() {
            fs::copy(source, out.join(destination))?;
        }
    }
    Ok(())
}

const PUBLIC_SITE_PAGES: [&str; 11] = [
    "index.html",
    "maps/index.html",
    "support/index.html",
    "privacy/index.html",
    "cookies/index.html",
    "terms/index.html",
    "free-browser-strategy-game/index.html",
    "games-like-travian/index.html",
    "games-like-ikariam/index.html",
    "multiplayer-territory-control-game/index.html",
    "about/index.html",
];

/// Long-tail landing pages that ship in English only. They answer English
/// search intent, so they publish unprefixed and stay out of the localized
/// matrix instead of duplicating English copy under every language prefix.
const ENGLISH_ONLY_PAGES: [&str; 5] = [
    "free-browser-strategy-game/index.html",
    "games-like-travian/index.html",
    "games-like-ikariam/index.html",
    "multiplayer-territory-control-game/index.html",
    "about/index.html",
];

fn expand_public_site_header(paths: &Paths, out: &Path) -> Result<()> {
    // The landing's generated sections and every localized page share one publisher.
    let english = if paths.is_local_preview() {
        let payload: serde_json::Value = serde_json::from_slice(&fs::read(out.join("locales/en"))?)?;
        payload["strings"].clone()
    } else {
        serde_json::to_value(sow_i18n::web(sow_i18n::Language::English))?
    };
    let leader_explorer = landing_leader_explorer(&paths.root.join("sow-web/site"), &english)?;
    let map_explorer = landing_maps(&out.join("maps"), &paths.root.join("assets/maps/SOURCES.toml"))?;
    let landing = expand_landing(&fs::read_to_string(out.join("index.html"))?, out, &leader_explorer, &map_explorer)?;
    fs::write(out.join("index.html"), landing)?;
    let leaders = sow_core::player::Leader::all().map(|leader| {
        serde_json::json!({
            "id": leader.id(), "artSlug": leader.art_slug(),
            "legacyIds": leader.legacy_ids(),
            "region": leader.character().region,
            "wikipedia": leader.character().wikipedia,
        })
    }).collect::<Vec<_>>();
    let biographies = public_leader_biographies(&paths.root)?;
    let mut metadata = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(&mut metadata, &serde_json::json!({ "leaders": leaders, "pages": PUBLIC_SITE_PAGES, "englishOnly": ENGLISH_ONLY_PAGES, "biographies": biographies, "mapExplorer": map_explorer, "localPreview": paths.is_local_preview() }))?;
    run("bun", &[
        "run", paths.root.join("sow-web/site/generate.ts").to_str().context("site generator path")?,
        out.to_str().context("site output path")?,
        metadata.path().to_str().context("site metadata path")?,
    ], Some(&paths.root))
}

fn load_leader_biography(
    path: &Path,
    leader_id: &str,
    published_languages: &[&str],
    distinct_articles: &mut std::collections::HashSet<String>,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("read leader biography file {}", path.display()))?;
    let record: toml::Value = source
        .parse()
        .with_context(|| format!("parse leader biography file {}", path.display()))?;
    let record = record.as_table().with_context(|| {
        format!(
            "leader biography file {} must contain a table",
            path.display()
        )
    })?;
    for field in record.keys() {
        if field != "articles" && field != "fallbacks" {
            bail!(
                "leader biography file {} has unexpected field {field}",
                path.display()
            );
        }
    }
    let articles = record
        .get("articles")
        .and_then(toml::Value::as_table)
        .with_context(|| {
            format!(
                "leader biography file {} has no articles table",
                path.display()
            )
        })?;
    let fallbacks = record
        .get("fallbacks")
        .map(|value| {
            value.as_table().with_context(|| {
                format!(
                    "leader biography file {} has an invalid fallbacks table",
                    path.display()
                )
            })
        })
        .transpose()?;
    let mut used_articles = std::collections::HashSet::new();
    let mut biographies = serde_json::Map::new();

    for &code in published_languages {
        let (article_language, article) = if let Some(article) = articles.get(code) {
            if fallbacks.is_some_and(|fallbacks| fallbacks.contains_key(code)) {
                bail!(
                    "leader biography file {} has both a direct article and a fallback for {code}/{leader_id}",
                    path.display()
                );
            }
            (code, article)
        } else {
            let article_language = fallbacks
                .and_then(|fallbacks| fallbacks.get(code))
                .and_then(toml::Value::as_str)
                .with_context(|| {
                    format!(
                        "leader biography language missing: {} for {code}/{leader_id}",
                        path.display()
                    )
                })?;
            let article = articles.get(article_language)
                .with_context(|| format!("leader biography fallback in {} for {code}/{leader_id} targets missing article language {article_language}", path.display()))?;
            (article_language, article)
        };
        let source_language = article
            .get("lang")
            .and_then(toml::Value::as_str)
            .filter(|language| !language.is_empty())
            .with_context(|| {
                format!(
                    "leader biography article language missing in {} for {code}/{leader_id}",
                    path.display()
                )
            })?;
        if source_language != article_language {
            bail!(
                "leader biography article language mismatch in {} for {code}/{leader_id}: key {article_language}, article declares {source_language}",
                path.display()
            );
        }
        if article
            .get("url")
            .and_then(toml::Value::as_str)
            .is_none_or(|url| !url.starts_with("https://"))
        {
            bail!(
                "leader biography article in {} for {code}/{leader_id} needs an HTTPS source",
                path.display()
            );
        }
        let value = serde_json::to_value(article)?;
        let has_sections = value
            .get("sections")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|sections| !sections.is_empty());
        let short_article = value
            .get("shortArticle")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        if !value.get("intro").is_some_and(serde_json::Value::is_array)
            || (!has_sections && !short_article)
        {
            bail!(
                "leader biography article in {} for {code}/{leader_id} is incomplete",
                path.display()
            );
        }
        used_articles.insert(article_language.to_owned());
        biographies.insert(code.to_owned(), value);
    }

    if let Some(fallbacks) = fallbacks {
        for locale in fallbacks.keys() {
            if !published_languages.contains(&locale.as_str()) {
                bail!(
                    "leader biography file {} has a fallback for unpublished language {locale}",
                    path.display()
                );
            }
        }
    }
    if used_articles.len() != articles.len() {
        let unused_language = articles
            .keys()
            .find(|language| !used_articles.contains(language.as_str()))
            .map(String::as_str)
            .unwrap_or("<unknown>");
        bail!(
            "leader biography file {} has an unused article for language {unused_language}/{leader_id}",
            path.display()
        );
    }
    for (language, article) in articles {
        let fingerprint = serde_json::to_string(article)?;
        if !distinct_articles.insert(fingerprint) {
            bail!(
                "leader biography file {} duplicates another stored article for language {language}/{leader_id}",
                path.display()
            );
        }
    }
    Ok(biographies)
}

fn public_leader_biographies(root: &Path) -> Result<serde_json::Value> {
    let site = root.join("sow-web/site");
    let biography_directory = site.join("leader-bios");
    let expected_leaders = sow_core::player::Leader::all()
        .map(|leader| leader.id())
        .collect::<std::collections::HashSet<_>>();
    let published_languages = sow_i18n::Language::published_registry()
        .map(|(_, code, _)| code)
        .collect::<Vec<_>>();
    let mut biography_files = std::collections::HashSet::new();
    for entry in fs::read_dir(&biography_directory).with_context(|| {
        format!(
            "read leader biography directory {}",
            biography_directory.display()
        )
    })? {
        let path = entry?.path();
        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("toml") {
            continue;
        }
        if !path.is_file() {
            bail!("leader biography path is not a file: {}", path.display());
        }
        let id = path
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .with_context(|| {
                format!("leader biography filename is not UTF-8: {}", path.display())
            })?;
        if !expected_leaders.contains(id) {
            bail!(
                "leader biography file {} has unknown leader ID {id}",
                path.display()
            );
        }
        biography_files.insert(id.to_owned());
    }
    let mut distinct_articles = std::collections::HashSet::new();
    let mut biographies = published_languages
        .iter()
        .map(|code| {
            (
                (*code).to_owned(),
                serde_json::Value::Object(serde_json::Map::new()),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    for leader in sow_core::player::Leader::all() {
        let id = leader.id();
        let path = biography_directory.join(format!("{id}.toml"));
        if !biography_files.contains(id) {
            bail!("leader biography file is missing: {}", path.display());
        }
        let resolved =
            load_leader_biography(&path, id, &published_languages, &mut distinct_articles)?;
        for code in &published_languages {
            let article = resolved.get(*code).with_context(|| {
                format!("leader biography locale missing after load: {code}/{id}")
            })?;
            biographies
                .get_mut(*code)
                .and_then(serde_json::Value::as_object_mut)
                .with_context(|| format!("leader biography output locale is invalid: {code}"))?
                .insert(id.to_owned(), article.clone());
        }
    }
    let sagas: toml::Value = fs::read_to_string(site.join("leader-sagas.toml"))?
        .parse()
        .context("parse original leader sagas")?;
    let source_records = sagas.get("sources").and_then(toml::Value::as_table)
        .context("leader sagas have no sources")?;
    let saga_records = sagas.get("sagas").and_then(toml::Value::as_table)
        .context("leader sagas have no entries")?;
    for (code, leaders) in saga_records {
        let leaders = leaders.as_table().with_context(|| format!("invalid saga locale {code}"))?;
        let locale = biographies.get_mut(code).and_then(serde_json::Value::as_object_mut)
            .with_context(|| format!("saga locale is not published: {code}"))?;
        for (leader_id, saga) in leaders {
            if !expected_leaders.contains(leader_id.as_str()) {
                bail!("saga references unknown leader {leader_id}");
            }
            let highlight = saga.get("highlight").and_then(toml::Value::as_str)
                .with_context(|| format!("saga {code}/{leader_id} has no highlight"))?;
            let chapters = saga.get("chapters").and_then(toml::Value::as_array)
                .with_context(|| format!("saga {code}/{leader_id} has no chapters"))?;
            let highlight_words = highlight.split_whitespace().count();
            if !(70..=140).contains(&highlight_words) || chapters.len() != 5 {
                bail!("saga {code}/{leader_id} needs a 70–140 word highlight and exactly five chapters (found {highlight_words} words and {})", chapters.len());
            }
            let chapter_words = chapters.iter().map(|chapter| chapter.get("text").and_then(toml::Value::as_str).map_or(0, |text| text.split_whitespace().count())).sum::<usize>();
            if !(1000..=2200).contains(&chapter_words) {
                bail!("saga {code}/{leader_id} must contain 1,000–2,200 chapter words (found {chapter_words})");
            }
            let mut all_sources = std::collections::BTreeMap::new();
            let mut sections = Vec::new();
            for (index, chapter) in chapters.iter().enumerate() {
                let refs = chapter.get("sources").and_then(toml::Value::as_array)
                    .with_context(|| format!("saga {code}/{leader_id}, chapter {} has no sources", index + 1))?;
                if refs.is_empty() {
                    bail!("saga {code}/{leader_id}, chapter {} must cite at least one source", index + 1);
                }
                let mut chapter_sources = Vec::new();
                for reference in refs {
                    let source_id = reference.as_str().context("saga source reference must be a string")?;
                    let source = source_records.get(source_id).with_context(|| format!("unknown saga source {source_id}"))?;
                    if source.get("url").and_then(toml::Value::as_str).is_none_or(|url| !url.starts_with("https://")) {
                        bail!("saga source {source_id} needs an HTTPS URL");
                    }
                    if source.get("title").and_then(toml::Value::as_str).is_none_or(str::is_empty) {
                        bail!("saga source {source_id} needs a title");
                    }
                    let source_json = serde_json::to_value(source)?;
                    all_sources.insert(source_id.to_owned(), source_json.clone());
                    chapter_sources.push(source_json);
                }
                let title = chapter.get("title").and_then(toml::Value::as_str).context("saga chapter title missing")?;
                let text = chapter.get("text").and_then(toml::Value::as_str).context("saga chapter text missing")?;
                let evidence = chapter.get("evidence").and_then(toml::Value::as_str).context("saga chapter evidence note missing")?;
                let paragraphs = text.split("\n\n").map(str::trim).filter(|paragraph| !paragraph.is_empty()).collect::<Vec<_>>();
                if paragraphs.len() < 2 || evidence.trim().is_empty() || title.trim().is_empty() {
                    bail!("saga {code}/{leader_id}, chapter {} needs multiple paragraphs and an evidence note", index + 1);
                }
                sections.push(serde_json::json!({ "title": title, "paragraphs": paragraphs, "evidence": evidence, "sources": chapter_sources }));
            }
            for key in ["evidence_label", "chapter_sources_label", "source_note"] {
                if saga.get(key).and_then(toml::Value::as_str).is_none_or(str::is_empty) {
                    bail!("saga {code}/{leader_id} needs {key}");
                }
            }
            let mut bio = serde_json::json!({
                "title": saga.get("title").and_then(toml::Value::as_str).unwrap_or(leader_id),
                "lang": code,
                "url": "https://www.britannica.com/biography/Julius-Caesar-Roman-ruler",
                "highlight": highlight,
                "highlightLabel": saga.get("highlight_label").and_then(toml::Value::as_str).unwrap_or(if code == "es" { "En pocas palabras" } else { "In brief" }),
                "evidenceLabel": saga.get("evidence_label").and_then(toml::Value::as_str),
                "chapterSourcesLabel": saga.get("chapter_sources_label").and_then(toml::Value::as_str),
                "sourceNote": saga.get("source_note").and_then(toml::Value::as_str),
                "isSaga": true,
                "intro": [],
                "sections": sections,
                "sources": all_sources.into_values().collect::<Vec<_>>(),
            });
            // Let the ordinary profile template retain the current localized title.
            bio["title"] = serde_json::Value::String(saga.get("title").and_then(toml::Value::as_str).unwrap_or(leader_id).to_owned());
            locale.insert(leader_id.to_owned(), bio);
        }
    }
    Ok(serde_json::json!({ "leaders": biographies }))
}

/// Landing sections built from data at publish time: the hero roster from the
/// character catalog, the maps in rotation and the optional captured videos.
/// A hero or map added to the game appears here without touching the page.
fn expand_landing(source: &str, out: &Path, leader_explorer: &str, map_explorer: &str) -> Result<String> {
    let english = serde_json::to_value(sow_i18n::web(sow_i18n::Language::English))?;
    let media = out.join("assets/site/media");
    let splash_desktop_hash = file_sha256(&out.join("assets/shell/loader/sow-splash-desktop.webp"))?[..8].to_owned();
    let splash_mobile_hash = file_sha256(&out.join("assets/shell/loader/sow-splash-mobile.webp"))?[..8].to_owned();
    let sections = [
        (
            "<!-- SOW_SITE_HERO_MEDIA -->",
            landing_hero_media(&media, &splash_desktop_hash, &splash_mobile_hash),
        ),
        ("<!-- SOW_SITE_MAPS -->", map_explorer.to_owned()),
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
    let marker = "<!-- SOW_SITE_LEADER_EXPLORER -->";
    if page.matches(marker).count() != 1 {
        bail!("the landing page must contain one {marker}");
    }
    page = page.replace(marker, leader_explorer);
    Ok(page)
}

fn landing_leader_explorer(site: &Path, english: &serde_json::Value) -> Result<String> {
    let mut html = fs::read_to_string(site.join("leader-explorer.html"))?;
    for (marker, content) in [
        ("<!-- SOW_SITE_LEADER_SHOWCASE -->", landing_leader_showcase(english)?),
        ("<!-- SOW_SITE_LEADERS -->", landing_leaders(english)?),
    ] {
        if html.matches(marker).count() != 1 {
            bail!("the shared leader explorer must contain one {marker}");
        }
        html = html.replace(marker, &content);
    }
    if html.contains("<!-- SOW_SITE_") {
        bail!("the shared leader explorer contains an unexpanded marker");
    }
    Ok(html)
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

fn landing_hero_media(media: &Path, desktop_hash: &str, mobile_hash: &str) -> String {
    let desktop = format!("/assets/shell/loader/sow-splash-desktop.webp?v={desktop_hash}");
    let mobile = format!("/assets/shell/loader/sow-splash-mobile.webp?v={mobile_hash}");
    let still = format!(
        "<picture class=\"hero-still\"><source media=\"(max-width: 700px)\" srcset=\"{mobile}\"><img src=\"{desktop}\" alt=\"\" fetchpriority=\"high\" decoding=\"async\" width=\"1600\" height=\"900\"></picture>"
    );
    if media.join("hero-loop.webm").is_file() {
        format!(
            "{still}<video class=\"hero-video\" hidden loop muted playsinline preload=\"none\" aria-hidden=\"true\" poster=\"{desktop}\"><source src=\"/assets/site/media/hero-loop.webm\" type=\"video/webm\"><source src=\"/assets/site/media/hero-loop.mp4\" type=\"video/mp4\"></video>"
        )
    } else {
        still
    }
}

fn landing_leader_showcase(english: &serde_json::Value) -> Result<String> {
    let leader = sow_core::player::Leader::default();
    let id = leader.id();
    let art_slug = leader.art_slug();
    let name = english_text(english, &format!("heroes.leader_{id}_name"))?;
    let historical = english_text(english, &format!("heroes.leader_{id}_historical"))?;
    Ok(format!(
        r#"<div class="showcase-art is-loading" data-detail-art>
            <span class="art-unavailable" data-i18n="site.art_unavailable">{unavailable}</span>
            <div class="showcase-loader" aria-hidden="true">
              <svg class="showcase-loader-ring" viewBox="0 0 120 120" focusable="false"><defs><linearGradient id="showcase-loader-arc" x1="0" y1="0" x2="1" y2="1"><stop offset="0" class="showcase-loader-head"/><stop offset=".45" class="showcase-loader-body"/><stop offset="1" class="showcase-loader-tail"/></linearGradient></defs><circle class="showcase-loader-orbit" cx="60" cy="60" r="58.5" pathLength="100"/><circle class="showcase-loader-track" cx="60" cy="60" r="54"/><circle class="showcase-loader-arc" cx="60" cy="60" r="54" pathLength="100"/></svg>
              <img class="showcase-portrait" data-detail-portrait src="/assets/gameplay/avatars/{art_slug}.webp" alt="" width="256" height="256">
            </div>
            <picture><source media="(max-width: 960px)" srcset="/assets/shell/leaders/{art_slug}_mobile.webp" data-detail-image-mobile><img data-detail-image src="/assets/shell/leaders/{art_slug}_desktop.webp" alt="{name}" decoding="async" width="1600" height="900"></picture>
            <div class="leader-controls">
              <button class="leader-arrow leader-arrow--prev" type="button" data-leader-prev aria-label="Previous leader" data-i18n="site.previous_leader" data-i18n-attr="aria-label">
                <svg class="leader-arrow-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="15 19 8 12 15 5"></polyline></svg>
              </button>
              <span class="leader-position" data-leader-position aria-live="polite" aria-atomic="true"></span>
              <button class="leader-browse" type="button" data-leader-browse aria-haspopup="dialog" aria-controls="leader-picker">
                <span data-i18n="site.browse_all">Browse all</span> <span data-leader-total></span>
              </button>
              <button class="leader-arrow leader-arrow--next" type="button" data-leader-next aria-label="Next leader" data-i18n="site.next_leader" data-i18n-attr="aria-label">
                <svg class="leader-arrow-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="9 5 16 12 9 19"></polyline></svg>
              </button>
            </div>
          </div>
          <div class="showcase-copy">
            <div class="showcase-identity"><p class="eyebrow" data-detail-civ data-i18n="heroes.leader_{id}_civilization">{civ}</p>
            <h3 data-detail-name data-i18n="heroes.leader_{id}_name">{name}</h3>
            <p class="showcase-historical" data-detail-historical data-i18n="heroes.leader_{id}_historical"{hidden}>{historical}</p></div>
            <div class="showcase-trait"><p class="showcase-label" data-i18n="site.command_trait">{trait_label}</p>
            <div class="showcase-ability-row"><img class="showcase-ability-icon" data-detail-ability-icon src="/assets/gameplay/icons/abilities/{art_slug}.webp" alt="" aria-hidden="true" width="84" height="84"><div class="showcase-ability-copy"><p class="showcase-ability" data-detail-ability data-i18n="site.leader_{id}_ability">{ability}</p>
            <p class="showcase-description" data-detail-description data-i18n="site.leader_{id}_description">{description}</p></div></div></div>
            <a class="leader-profile-link" data-detail-profile href="/leaders/{id}/" data-i18n="site.view_profile">{profile_label}</a>
          </div>"#,
        civ = english_text(english, &format!("heroes.leader_{id}_civilization"))?,
        hidden = if historical == name { " hidden" } else { "" },
        trait_label = english_text(english, "site.command_trait")?,
        profile_label = english_text(english, "site.view_profile")?,
        ability = english_text(english, &format!("site.leader_{id}_ability"))?,
        description = english_text(english, &format!("site.leader_{id}_description"))?,
        unavailable = english_text(english, "site.art_unavailable")?,
    ))
}

fn landing_leaders(english: &serde_json::Value) -> Result<String> {
    let mut html = String::new();
    for leader in sow_core::player::Leader::all() {
        let id = leader.id();
        let art_slug = leader.art_slug();
        let legacy_ids = leader.legacy_ids().join(",");
        let active = leader == sow_core::player::Leader::default();
        let avatar_src = if active {
            format!("/assets/gameplay/avatars/expressions/{art_slug}_angry.webp")
        } else {
            format!("/assets/gameplay/avatars/{art_slug}.webp")
        };
        html += &format!(
            r#"<button class="roster-card{class}" type="button" aria-pressed="{active}" data-leader-id="{id}" data-legacy-ids="{legacy_ids}" data-art-slug="{art_slug}" data-name="{name}" data-civ="{civ}" data-historical="{historical}" data-ability="{ability}" data-description="{description}"><img src="{avatar_src}" alt="" width="128" height="128" loading="lazy" decoding="async"><span data-i18n="heroes.leader_{id}_name">{name}</span></button>
"#,
            class = if active { " is-active" } else { "" },
            legacy_ids = legacy_ids,
            name = english_text(english, &format!("heroes.leader_{id}_name"))?,
            civ = english_text(english, &format!("heroes.leader_{id}_civilization"))?,
            historical = english_text(english, &format!("heroes.leader_{id}_historical"))?,
            ability = english_text(english, &format!("site.leader_{id}_ability"))?,
            description = english_text(english, &format!("site.leader_{id}_description"))?,
        );
    }
    Ok(html)
}

fn landing_maps(maps: &Path, sources_path: &Path) -> Result<String> {
    let catalog = sow_core::map_file::parse_catalog(&fs::read(maps.join("catalog.bin"))?)
        .map_err(|error| anyhow::anyhow!("read map catalog: {error}"))?;
    let sources: toml::Value = fs::read_to_string(sources_path)?.parse()
        .context("parse official map source catalog")?;
    let recipes = sources.get("recipes").and_then(toml::Value::as_table)
        .context("map source catalog has no recipes")?;
    let bust = thumbnail_cache_bust(maps)?;
    let entries: Vec<_> = catalog
        .entries
        .iter()
        .filter(|entry| {
            entry.multiplayer_frequency > 0
                && recipes.get(&entry.key)
                    .and_then(|recipe| recipe.get("origin_kind"))
                    .and_then(toml::Value::as_str) == Some("openfront_image")
                && maps.join(&entry.key).join("thumbnail.webp").is_file()
        })
        .collect();
    let first = entries
        .first()
        .context("no map in rotation has a thumbnail for the landing page")?;
    let first_key = html_text(&first.key);
    let first_name = html_text(&official_map_display_name(&first.key, &first.display_name));
    let mut html = String::from(
        r#"<div class="map-layout"><nav class="map-picker" aria-label="Maps in Rotation" data-i18n="site.maps_in_rotation" data-i18n-attr="aria-label"><ul class="map-strip map-roster">"#,
    );
    for (index, entry) in entries.into_iter().enumerate() {
        let key = html_text(&entry.key);
        let name = html_text(&official_map_display_name(&entry.key, &entry.display_name));
        html += &format!(
            r#"<li class="map-card"><button type="button" data-map-select data-map-name="{name}" data-map-image="/maps/{key}/preview.webp?v={bust}" aria-pressed="{active}"><img src="/maps/{key}/thumbnail.webp?v={bust}" alt="" width="256" height="144" loading="lazy" decoding="async"><span>{name}</span></button></li>"#,
            active = index == 0
        );
    }
    html.push_str("</ul></nav>");
    html += &format!(
        r#"<div class="map-theatre" data-map-theatre><button type="button" class="map-nav-arrow map-nav-arrow--prev" data-map-prev aria-label="Previous map" data-i18n="site.previous_map" data-i18n-attr="aria-label">‹</button><button type="button" class="map-nav-arrow map-nav-arrow--next" data-map-next aria-label="Next map" data-i18n="site.next_map" data-i18n-attr="aria-label">›</button><div class="map-theatre-stage"><figure class="map-preview"><div class="map-preview-image" data-map-frame><div class="map-grid-overlay" aria-hidden="true"></div><span class="art-unavailable" data-i18n="site.art_unavailable">Artwork unavailable</span><img data-map-preview-image src="/maps/{key}/preview.webp?v={bust}" alt="{name}" width="1024" height="576" fetchpriority="high" decoding="async"></div><figcaption class="map-hud"><div class="map-hud-meta"><span class="eyebrow" data-i18n="site.maps_in_rotation">Maps in Rotation</span></div><h3 data-map-current-name aria-live="polite">{name}</h3></figcaption></figure></div></div></div>"#,
        key = first_key,
        name = first_name
    );
    Ok(html)
}

fn official_map_display_name(key: &str, catalog_name: &str) -> String {
    match key {
        "bajacalifornia" => "Baja California",
        "eastasia" => "East Asia",
        "indiansubcontinent" => "Indian Subcontinent",
        "mena" => "Middle East and North Africa",
        "middleeast" => "Middle East",
        "northamerica" => "North America",
        "southamerica" => "South America",
        "southeastasia" => "South East Asia",
        _ => return catalog_name.to_owned(),
    }
    .to_owned()
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

fn write_sw(paths: &Paths, out: &Path, ts: &str) -> Result<()> {
    let js = compile_shell_ts(&paths.shell, "sw.ts")?;
    fs::write(out.join("sw.js"), js.replace("__BUILD_TS__", ts))?;
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
    validate_web_node_with_missing(path, expected, actual, false, &mut Vec::new())
}

fn validate_web_node_with_missing(
    path: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    allow_missing_translations: bool,
    missing_translations: &mut Vec<String>,
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
                    None
                        if allow_missing_translations && is_non_english_catalog_path(path) =>
                    {
                        missing_translations.push(child_path);
                        continue;
                    }
                    None => bail!("web catalog missing key {child_path}"),
                };
                validate_web_node_with_missing(
                    &child_path,
                    expected_value,
                    actual_value,
                    allow_missing_translations,
                    missing_translations,
                )?;
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
        // League names are shared terms in several published languages.
        || key.starts_with("ranked.tier_")
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
                | "heroes.oceania"
                | "hud.ability_notice"
                | "hud.ability_remaining"
                | "hud.ability_state_active"
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
                | "profile.achievement_category_combat"
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

fn validate_leader_display_names(
    catalog: &serde_json::Value,
    fallback: Option<&serde_json::Value>,
) -> Result<()> {
    for slug in sow_core::player::Leader::all().map(sow_core::player::Leader::id) {
        let key = format!("heroes.leader_{slug}_name");
        let value = web_catalog_value(catalog, &key)
            .or_else(|| fallback.and_then(|fallback| web_catalog_value(fallback, &key)))
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
    validate_web_catalogs_with_missing(report_equal_values, false)
}

fn validate_web_catalogs_with_missing(
    report_equal_values: bool,
    allow_missing_translations: bool,
) -> Result<serde_json::Value> {
    let english = serde_json::to_value(&sow_i18n::web(sow_i18n::Language::English))?;
    let mut untranslated = Vec::new();
    for &(language, code, _, _) in sow_i18n::Language::registry() {
        let catalog = serde_json::to_value(&sow_i18n::web(language))?;
        let mut missing_translations = Vec::new();
        validate_web_node_with_missing(
            code,
            &english,
            &catalog,
            allow_missing_translations,
            &mut missing_translations,
        )
            .with_context(|| format!("validate web catalog {code}"))?;
        for key in missing_translations {
            eprintln!("i18n warning (local preview): {key} missing; using English fallback");
        }
        validate_leader_display_names(
            &catalog,
            allow_missing_translations.then_some(&english),
        )
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
            for (key, value) in equal_values
                .into_iter()
                .filter(|(key, _)| !web_key_is_intentionally_english(key))
            {
                if allow_missing_translations {
                    eprintln!("i18n warning (local preview): {code}: {key} still equals English");
                } else {
                    untranslated.push(format!("{code}: {key} = {value}"));
                }
            }
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
                let is_timestamped_backup =
                    name.strip_prefix("web.toml.bak_").is_some_and(|suffix| {
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
                Some("html") | Some("ts")
            )
        {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".test.ts"))
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
            || path.extension().and_then(|ext| ext.to_str()) != Some("ts")
        {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".test.ts"))
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
const campaign = require(process.argv[1]).default;
const { catalog, episodes, menuGuides } = JSON.parse(fs.readFileSync(0, "utf8"));
const catalogText = key => {
    const value = key.split(".").reduce((node, part) =>
        node && Object.prototype.hasOwnProperty.call(node, part) ? node[part] : undefined, catalog);
    return typeof value === "string" ? value : null;
};
const hasText = key => { const value = catalogText(key); return typeof value === "string" && value.trim().length > 0; };
for (const { episode_id, definition, roster } of episodes) {
    const result = campaign.validate(definition, roster, {
        hasText,
        getText: key => {
            const value = catalogText(key);
            return typeof value === "string" ? campaign.replaceFactionStoryNames(value, definition, roster) : null;
        }
    });
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
        .arg(paths.shell.join("sow-campaign.ts"))
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
    write_locales(out, None)
}

fn write_locales(out: &Path, local_root: Option<&Path>) -> Result<()> {
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
        let strings = if let Some(root) = local_root {
            let source = root.join("sow-i18n/strings").join(&folder).join("web.toml");
            let catalog = fs::read_to_string(&source)
                .map_err(anyhow::Error::from)
                .and_then(|text| toml::from_str::<sow_i18n::WebStrings>(&text).map_err(anyhow::Error::from));
            match catalog {
                Ok(catalog) => serde_json::to_value(catalog)?,
                Err(error) => {
                    eprintln!("⚠ Local locale {}: {error:#}", source.display());
                    if d.join(&folder).is_file() {
                        continue;
                    }
                    serde_json::json!({})
                }
            }
        } else {
            serde_json::to_value(sow_i18n::web(language))?
        };
        let payload = serde_json::json!({
            "schema": 1,
            "version": sow_i18n::WEB_CATALOG_VERSION,
            "locale": folder.clone(),
            "languages": languages.clone(),
            "strings": strings,
        });
        fs::write(d.join(folder), serde_json::to_string_pretty(&payload)?)?;
    }
    Ok(())
}

fn verify_gameplay_unit_art(dir: &Path, package: &str) -> Result<()> {
    for file in GAMEPLAY_UNIT_ART {
        if !dir.join("assets/gameplay/units").join(file).is_file() {
            bail!("{package} missing assets/gameplay/units/{file}");
        }
    }
    Ok(())
}

fn verify_ranked_art(dir: &Path, package: &str) -> Result<()> {
    for emblem in [
        "unranked",
        "bronze",
        "silver",
        "gold",
        "platinum",
        "diamond",
        "master",
        "grandmaster",
        "ranked",
    ] {
        let file = format!("assets/shell/ranked/{emblem}.svg");
        if !dir.join(&file).is_file() {
            bail!("{package} missing {file}");
        }
    }
    Ok(())
}

fn verify_layout(dir: &Path) -> Result<()> {
    verify_ranked_art(dir, "webroot")?;
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
        "auth-callback.js",
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
    verify_gameplay_unit_art(dir, "webroot")?;
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
    for (_, code, _) in sow_i18n::Language::published_registry() {
        let prefix = if code == "en" { String::new() } else { format!("{}/", locale_folder(code)) };
        for page in PUBLIC_SITE_PAGES.iter().map(|page| page.to_string())
            .chain(std::iter::once("leaders/index.html".to_string()))
            .chain(sow_core::player::Leader::all().map(|leader| format!("leaders/{}/index.html", leader.id()))) {
            if code != "en" && ENGLISH_ONLY_PAGES.contains(&page.as_str()) {
                continue;
            }
            let path = dir.join(format!("{prefix}{page}"));
            let html = fs::read_to_string(&path).with_context(|| format!("missing public page {}", path.display()))?;
            if html.contains("<!-- SOW_SITE_") || !html.contains(&format!("lang=\"{code}\"")) {
                bail!("incomplete localized page {}", path.display());
            }
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
        "fonts/fonts.css",
        "fonts/OFL.txt",
        "fonts/work-sans-latin.woff2",
        "assets/gameplay/buildings/atlas.webp",
        "assets/gameplay/buildings/atlas.json",
        "assets/gameplay/music/shadows-of-war.webm",
        "assets/gameplay/icons/nameplate_status_atlas.webp",
        "assets/shell/loader/loader_empty.webp",
        "assets/shell/loader/loader_full.webp",
        "assets/site/media/discord.svg",
        "assets/site/media/telegram.svg",
        "assets/site/media/github.svg",
    ] {
        if !dir.join(required).is_file() {
            bail!("crazygames bundle missing {}", required);
        }
    }
    for forbidden in ["admin", "play", "analytics.js"] {
        if dir.join(forbidden).exists() {
            bail!("crazygames bundle must not contain {forbidden}/");
        }
    }
    for required in [
        "maps/catalog.bin",
        "maps/NOTICE",
        "maps/world/map.bin.br",
        "assets/gameplay/icons/abilities-atlas.webp",
    ] {
        if !dir.join(required).is_file() {
            bail!("crazygames bundle missing {required}");
        }
    }
    let inventory = asset_file_inventory(dir)?;
    let file_count = inventory.len();
    if file_count > 1_500 {
        bail!("CrazyGames bundle has {file_count} files; portal limit is 1,500");
    }
    let mut total_bytes = 0u64;
    for relative in &inventory {
        total_bytes = total_bytes
            .checked_add(fs::metadata(dir.join(relative))?.len())
            .context("CrazyGames bundle size overflow")?;
    }
    if total_bytes > 250_000_000 {
        bail!("CrazyGames bundle has {total_bytes} bytes; portal limit is 250 MB");
    }
    let html = fs::read_to_string(dir.join("index.html"))?;
    if html.contains("https://shadowsofwar.io/assets/") {
        bail!("crazygames index.html must load static assets from the portal CDN");
    }
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
    verify_ranked_art(dir, "CrazyGames bundle")?;
    verify_gameplay_unit_art(dir, "CrazyGames bundle")?;
    verify_cg_ability_atlas(dir, &html)?;
    verify_cg_avatar_atlas(dir, &html)?;
    for needle in [
        "sdk.crazygames.com/crazygames-sdk-v3.js",
        "SOW_trackExperienceEvent",
        "SOW_MAPS_URL = \"./maps\"",
        "SOW_ASSETS_URL = \"./assets\"",
        "window.SOW_ABILITY_ICON_ATLAS =",
        "window.SOW_AVATAR_ATLAS =",
        "href=\"fonts/fonts.css?v=",
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

fn verify_cg_ability_atlas(dir: &Path, html: &str) -> Result<()> {
    let marker = "window.SOW_ABILITY_ICON_ATLAS = ";
    let start = html
        .find(marker)
        .context("CrazyGames ability atlas config is missing")?
        + marker.len();
    let end = html[start..]
        .find(';')
        .context("CrazyGames ability atlas config is unterminated")?
        + start;
    let config: serde_json::Value = serde_json::from_str(&html[start..end])
        .context("CrazyGames ability atlas config is invalid JSON")?;
    let expected = sow_core::player::Leader::all()
        .map(|leader| leader.art_slug().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let icons = config
        .get("icons")
        .and_then(serde_json::Value::as_object)
        .context("CrazyGames ability atlas has no icon map")?;
    let actual = icons.keys().cloned().collect::<std::collections::BTreeSet<_>>();
    if actual != expected {
        bail!("CrazyGames ability atlas does not cover every playable hero");
    }
    let columns = config
        .get("columns")
        .and_then(serde_json::Value::as_u64)
        .context("CrazyGames ability atlas has invalid column count")? as u32;
    let rows = config
        .get("rows")
        .and_then(serde_json::Value::as_u64)
        .context("CrazyGames ability atlas has invalid row count")? as u32;
    if columns == 0 || rows == 0 || icons.len() as u64 > u64::from(columns) * u64::from(rows) {
        bail!("CrazyGames ability atlas dimensions do not cover its icons");
    }
    let slots = u64::from(columns) * u64::from(rows);
    let indices = icons
        .values()
        .map(|index| {
            index
                .as_u64()
                .context("CrazyGames ability atlas has a non-integer icon index")
        })
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    if indices.len() != icons.len() || indices.iter().any(|index| *index >= slots) {
        bail!("CrazyGames ability atlas has duplicate or out-of-range icon indices");
    }
    if config.get("path").and_then(serde_json::Value::as_str)
        != Some("gameplay/icons/abilities-atlas.webp")
    {
        bail!("CrazyGames ability atlas has an unexpected asset path");
    }
    let expected_hash = config
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .context("CrazyGames ability atlas has no content hash")?;
    let path = dir.join("assets").join(
        config
            .get("path")
            .and_then(serde_json::Value::as_str)
            .context("CrazyGames ability atlas has no asset path")?,
    );
    let (width, height) = image::image_dimensions(&path)
        .with_context(|| format!("read CrazyGames ability atlas {}", path.display()))?;
    let cell_size = config
        .get("cellSize")
        .and_then(serde_json::Value::as_u64)
        .context("CrazyGames ability atlas has invalid cell size")? as u32;
    if cell_size != 256 || width != columns * cell_size || height != rows * cell_size {
        bail!("CrazyGames ability atlas image dimensions do not match its config");
    }
    if file_sha256(&path)? != expected_hash {
        bail!("CrazyGames ability atlas content hash does not match its config");
    }
    if dir.join("assets/gameplay/icons/abilities").exists() {
        bail!("CrazyGames bundle still contains individual ability icons");
    }
    Ok(())
}

fn verify_poki_layout(dir: &Path) -> Result<()> {
    verify_ranked_art(dir, "Poki bundle")?;
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
        "assets/gameplay/icons/battle_icon_cutout.webp",
        "assets/gameplay/icons/nameplate_status_atlas.webp",
        "assets/gameplay/icons/surrender_flag.webp",
        "assets/gameplay/icons/tutorial_hand.webp",
        "assets/gameplay/music/shadows-of-war.webm",
        "maps/catalog.bin",
        "maps/NOTICE",
        "maps/world/map.bin.br",
    ] {
        if !dir.join(required).is_file() {
            bail!("poki bundle missing {required}");
        }
    }
    verify_playable_hero_icons(&dir.join("assets/gameplay/icons/abilities"), "Poki bundle")?;
    verify_gameplay_unit_art(dir, "Poki bundle")?;
    for forbidden in [
        "admin",
        "play",
        "site",
        ".well-known",
        "sw.js",
        "assets/gameplay/store",
        "assets/gameplay/skins",
        "assets/shell/brand",
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
    // A rewarded video must pay something: the only consumer is the
    // post-match double-reward offer, which asks the server to pay it.
    if sdk.contains("rewardedBreak")
        && !(html.contains("SOW_portalRewardedBreak") && html.contains("double_match_reward"))
    {
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

fn verify_playable_hero_icons(icons_dir: &Path, package: &str) -> Result<()> {
    for leader in sow_core::player::Leader::all() {
        let icon = icons_dir.join(format!("{}.webp", leader.art_slug()));
        if !icon.is_file() {
            bail!("{package} missing playable hero icon {}", icon.display());
        }
    }
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
        &paths.assets_gameplay.join("units"),
        &assets.join("gameplay/units"),
    )?;
    copy_dir(
        &paths.assets_gameplay.join("music"),
        &assets.join("gameplay/music"),
    )?;
    paths.art_pipeline()?.copy_distribution_tree(
        &paths.root.join("assets/campaign"),
        &assets.join("campaign"),
        Path::new("campaign"),
    )?;
    paths.art_pipeline()?.copy_distribution_tree(
        &paths.root.join("assets/guides"),
        &assets.join("guides"),
        Path::new("guides"),
    )?;
    copy_dir(&paths.site_media, &assets.join("site/media"))?;
    let maps = out.join("maps");
    fs::create_dir_all(&maps)?;
    if paths.assets_maps.is_dir() {
        copy_release_maps(
            &paths.assets_maps,
            &maps,
            &paths.root.join("assets/maps/SOURCES.toml"),
        )?;
    }
    refresh_map_thumbnails(&maps, &paths.map_sources, paths.art_pipeline()?)?;
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
    write_sw(paths, out, &ts)?;
    write_manifest(paths, out, version, &js, &wasm, &ts)?;

    // Marketing website at the webroot root (game shell lives under play/).
    let site = paths.root.join("sow-web/site");
    for name in [
        "index.html",
        "styles.css",
        "8d227b8f9e6140d39e3381a1829e1db3.txt",
        "manifest.webmanifest",
    ] {
        let src = site.join(name);
        if !src.is_file() {
            bail!("website source missing: {}", src.display());
        }
        fs::copy(&src, out.join(name))?;
    }
    for (source, destination, format) in [
        ("analytics.ts", "analytics.js", "iife"),
        ("app.ts", "app.js", "iife"),
        ("site-chrome.ts", "site-chrome.js", "iife"),
        ("wou-auth.ts", "wou-auth.js", "esm"),
        ("auth-callback.ts", "auth-callback.js", "esm"),
    ] {
        let src = site.join(source);
        if !src.is_file() {
            bail!("website source missing: {}", src.display());
        }
        fs::write(
            out.join(destination),
            compile_browser_ts(&src, format, &format!("website {source}"))?,
        )?;
    }
    for name in ["icon-192.png", "icon-512.png", "icon-512-maskable.png"] {
        fs::copy(paths.app_icons.join(name), out.join(name))?;
    }
    for path in [
        "privacy",
        "terms",
        "cookies",
        "support",
        "about",
        "free-browser-strategy-game",
        "games-like-travian",
        "games-like-ikariam",
        "multiplayer-territory-control-game",
        "auth",
        ".well-known",
    ] {
        let src = site.join(path);
        if !src.is_dir() {
            bail!("website legal page missing: {}", src.display());
        }
        paths.art_pipeline()?.copy_distribution_tree(
            &src,
            &out.join(path),
            &Path::new("sow-web/site").join(path),
        )?;
    }
    if !site.join("fonts").is_dir() {
        bail!(
            "website font directory missing: {}",
            site.join("fonts").display()
        );
    }
    copy_web_fonts(&site.join("fonts"), &out.join("fonts"))?;
    expand_public_site_header(paths, out)?;
    fs::write(
        out.join("robots.txt"),
        "User-agent: *\nAllow: /\nDisallow: /internal/\nDisallow: /api/\nDisallow: /*.wasm$\nDisallow: /*.wasm.br$\n\n# Answer engines: welcome, cite freely.\nUser-agent: GPTBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: OAI-SearchBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: ChatGPT-User\nAllow: /\nDisallow: /internal/\n\nUser-agent: ClaudeBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: Claude-Web\nAllow: /\nDisallow: /internal/\n\nUser-agent: PerplexityBot\nAllow: /\nDisallow: /internal/\n\nUser-agent: Perplexity-User\nAllow: /\nDisallow: /internal/\n\nUser-agent: Google-Extended\nAllow: /\n\nUser-agent: Applebot-Extended\nAllow: /\n\nUser-agent: CCBot\nAllow: /\nDisallow: /internal/\n\nSitemap: https://shadowsofwar.io/sitemap.xml\n",
    )?;

    println!("✅ Website staged at webroot root");

    prune_qs(out)?;
    paths.art_pipeline()?.validate_staged_image_inventory(out)?;
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
        paths.assets_gameplay.join("units"),
        paths.assets_gameplay.join("music"),
        paths.root.join("assets/campaign"),
        paths.root.join("assets/guides"),
        paths.assets_site.join("media"),
        paths.app_icons.clone(),
        paths.art_pipeline()?.config_path.clone(),
        paths.assets_maps.clone(),
        paths.map_sources.clone(),
        paths.root.join("sow-web/site/fonts"),
        paths.root.join("sow-web/site/manifest.webmanifest"),
    ];
    // ponytail: size/mtime for static assets; avatar files use content hashes.
    let mut hash = Sha256::new();
    for stamp in local_source_snapshot(paths)?
        .into_iter()
        .filter(|stamp| roots.iter().any(|root| stamp.path.starts_with(root)))
    {
        hash.update(stamp.path.to_string_lossy().as_bytes());
        hash.update(stamp.len.to_le_bytes());
        hash.update(stamp.modified_nanos.to_le_bytes());
    }
    hash_avatar_asset_tree(&mut hash, &paths.assets_gameplay.join("avatars"))?;
    hash.update(file_sha256(&paths.art_pipeline()?.config_path)?.as_bytes());
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
        && GAMEPLAY_UNIT_ART
            .iter()
            .all(|file| out.join("assets/gameplay/units").join(file).is_file())
        && out
            .join("assets/gameplay/music/shadows-of-war.webm")
            .is_file()
        && site_media_ready(out);

    if !static_ready {
        if out.exists() {
            fs::remove_dir_all(out)
                .with_context(|| format!("reset native development webroot {}", out.display()))?;
        }
        fs::create_dir_all(out)?;
        let assets = out.join("assets");
        copy_dir(&paths.assets_shell, &assets.join("shell"))?;
        for name in [
            "avatars",
            "skins",
            "currency",
            "store",
            "buildings",
            "units",
            "music",
        ] {
            copy_dir(
                &paths.assets_gameplay.join(name),
                &assets.join("gameplay").join(name),
            )?;
        }
        paths.art_pipeline()?.copy_distribution_tree(
            &paths.root.join("assets/campaign"),
            &assets.join("campaign"),
            Path::new("campaign"),
        )?;
        paths.art_pipeline()?.copy_distribution_tree(
            &paths.root.join("assets/guides"),
            &assets.join("guides"),
            Path::new("guides"),
        )?;
        copy_dir(&paths.site_media, &assets.join("site/media"))?;
        copy_release_maps(
            &paths.assets_maps,
            &out.join("maps"),
            &paths.root.join("assets/maps/SOURCES.toml"),
        )?;
        refresh_map_thumbnails(&out.join("maps"), &paths.map_sources, paths.art_pipeline()?)?;
        copy_shell(paths, out, WebTarget::Native)?;

        let site = paths.root.join("sow-web/site");
        copy_web_fonts(&site.join("fonts"), &out.join("fonts"))?;
        fs::copy(
            site.join("manifest.webmanifest"),
            out.join("manifest.webmanifest"),
        )?;
        for name in ["icon-192.png", "icon-512.png", "icon-512-maskable.png"] {
            fs::copy(paths.app_icons.join(name), out.join(name))?;
        }
    }

    let wasm_hash = file_sha256(wasm_input)?;
    let build_id = &wasm_hash[..10];
    let bindgen_name = format!("sow_client_{build_id}");
    let wasm_name = format!("{bindgen_name}_bg.wasm");
    let wasm_ready = static_ready
        && out.join(&wasm_name).is_file()
        && out.join(format!("{bindgen_name}.js")).is_file()
        && out.join("play/index.html").is_file();

    if !wasm_ready {
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

        run_bindgen(wasm_input, out, &bindgen_name)?;
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
    }
    paths.art_pipeline()?.validate_staged_image_inventory(out)?;
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
    // CrazyGames serves uploaded files from its CDN. The ability and portrait
    // sheets keep the complete runtime art and map package below its 1,500-file cap.
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
    let ability_atlas = copy_cg_assets(&play_dir.join("assets"), &out.join("assets"), paths.art_pipeline()?)?;
    let packaged_assets = out.join("assets");
    let avatar_atlas = pack_cg_avatar_sheets(&packaged_assets, paths.art_pipeline()?)?;
    copy_portal_maps(&play_dir.join("maps"), &out.join("maps"), paths.art_pipeline()?)?;
    verify_portal_map_inventory(&play_dir.join("maps"), &out.join("maps"))?;
    copy_shell(paths, out, WebTarget::CrazyGames)?;
    copy_web_fonts(&paths.root.join("sow-web/site/fonts"), &out.join("fonts"))?;
    let stale_bridge = out.join("sdk/jest_portals.js");
    if stale_bridge.is_file() {
        fs::remove_file(stale_bridge)?;
    }
    export_locales(out)?;
    write_sw(paths, out, &jh)?;
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
    // and keep static assets/maps on the portal CDN.
    let idx = out.join("index.html");
    let html = fs::read_to_string(&idx)?;
    let mut lines: Vec<String> = html.lines().map(String::from).collect();
    let (mut sdk, mut boot) = (false, false);
    let ability_atlas_json = serde_json::to_string(&ability_atlas)?;
    let avatar_atlas_json = serde_json::to_string(&avatar_atlas)?;
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") {
            *line = "    <script src=\"https://sdk.crazygames.com/crazygames-sdk-v3.js\"></script>"
                .to_string();
            sdk = true;
        } else if line.contains("/* PORTAL_BOOT_SLOT */") {
            *line = format!(
                "        window.SOW_ENABLE_PORTAL_ADS = true; window.SOW_PORTAL = \"crazygames\"; window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; window.SOW_MAPS_URL = \"./maps\"; window.SOW_ASSETS_URL = \"./assets\"; window.SOW_DATABASE_URL = \"https://shadowsofwar.io/api\"; window.SOW_ABILITY_ICON_ATLAS = {ability_atlas_json}; window.SOW_AVATAR_ATLAS = {avatar_atlas_json};"
            );
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

    paths.art_pipeline()?.validate_staged_image_inventory(out)?;
    verify_cg_asset_inventory(&play_dir.join("assets"), &out.join("assets"))?;
    verify_cg_layout(out)?;
    let inventory = asset_file_inventory(out)?;
    let files = inventory.len();
    let bytes = inventory.iter().try_fold(0u64, |total, relative| {
        total
            .checked_add(fs::metadata(out.join(relative))?.len())
            .context("CrazyGames bundle size overflow")
    })?;
    println!(
        "✅ CrazyGames bundle ready for portal CDN: {} ({files} files, {bytes} bytes)",
        out.display()
    );
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

    let parent = out
        .parent()
        .context("Poki output directory has no parent")?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".poki-package-")
        .tempdir_in(parent)?;
    let package = staging.path();
    fs::copy(
        play_dir.join(format!("sow_client_{jh}.js")),
        package.join("sow_client.js"),
    )?;
    fs::copy(
        play_dir.join(format!("sow_client_{wh}_bg.wasm")),
        package.join("sow_client_bg.wasm"),
    )?;

    copy_poki_assets(&play_dir.join("assets"), &package.join("assets"), paths.art_pipeline()?)?;
    verify_poki_asset_inventory(&play_dir.join("assets"), &package.join("assets"))?;
    copy_portal_maps(&play_dir.join("maps"), &package.join("maps"), paths.art_pipeline()?)?;
    verify_portal_map_inventory(&play_dir.join("maps"), &package.join("maps"))?;
    copy_web_fonts(
        &paths.root.join("sow-web/site/fonts"),
        &package.join("fonts"),
    )?;
    copy_shell(paths, package, WebTarget::Poki)?;
    let stale_bridge = package.join("sdk/jest_portals.js");
    if stale_bridge.is_file() {
        fs::remove_file(stale_bridge)?;
    }
    optimize_poki_thumbnail(
        &paths.root.join("assets/art/brand/app-icon.webp"),
        &package
            .parent()
            .context("Poki staging directory has no parent")?
            .join("poki-thumbnail.png"),
        paths.art_pipeline()?,
    )?;
    fs::write(
        package.join("manifest.webmanifest"),
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
    let loader_path = package.join("loader.js");
    let loader = compile_shell_ts(&paths.shell, "loader.ts")?;
    fs::write(
        &loader_path,
        strip_optional_marked_section(
            &loader,
            "/* SOW_FIRST_PARTY_ANALYTICS_BEGIN */",
            "/* SOW_FIRST_PARTY_ANALYTICS_END */",
        )?,
    )?;
    fs::write(
        package.join("sdk/store_portals.js"),
        compile_shell_ts(&paths.shell, "sdk/poki_portals.ts")?,
    )?;
    let poki_source = package.join("sdk/poki_portals.js");
    if poki_source.is_file() {
        fs::remove_file(poki_source)?;
    }
    export_locales(package)?;
    write_manifest(
        paths,
        package,
        version,
        "sow_client.js",
        "sow_client_bg.wasm",
        &jh,
    )?;
    build_index(
        paths,
        package,
        IndexBuild {
            version,
            js: "sow_client.js",
            wasm: "sow_client_bg.wasm",
            ts: &jh,
            maps_cache_bust,
            target: WebTarget::Poki,
        },
    )?;

    let index = package.join("index.html");
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
    paths.art_pipeline()?.validate_staged_image_inventory(package)?;
    verify_poki_layout(package)?;

    if out.exists() {
        let backup = tempfile::Builder::new()
            .prefix(".poki-previous-")
            .tempdir_in(parent)?;
        let backup_path = backup.path().to_path_buf();
        drop(backup);
        fs::rename(out, &backup_path).context("preserve previous Poki package")?;
        if let Err(error) = fs::rename(package, out) {
            if let Err(restore) = fs::rename(&backup_path, out) {
                bail!(
                    "could not publish Poki package ({error}); previous package remains at {} ({restore})",
                    backup_path.display()
                );
            }
            return Err(error).context("publish validated Poki package");
        }
        if let Err(error) = fs::remove_dir_all(&backup_path) {
            eprintln!(
                "warning: validated Poki package is ready, but could not remove previous package {}: {error}",
                backup_path.display()
            );
        }
    } else {
        fs::rename(package, out).context("publish validated Poki package")?;
    }
    println!("✅ Poki bundle ready (self-contained): {}", out.display());
    Ok(())
}

fn verify_jest_layout(dir: &Path) -> Result<()> {
    verify_ranked_art(dir, "Jest bundle")?;
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
    verify_gameplay_unit_art(dir, "Jest bundle")?;
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
        "main_menu.jest.ts",
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
        "main_menu.poki.ts",
        "main_menu.store.ts",
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

    copy_poki_assets(&play_dir.join("assets"), &out.join("assets"), paths.art_pipeline()?)?;
    copy_portal_maps(&play_dir.join("maps"), &out.join("maps"), paths.art_pipeline()?)?;
    verify_portal_map_inventory(&play_dir.join("maps"), &out.join("maps"))?;
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
    let loader = compile_shell_ts(&paths.shell, "loader.ts")?;
    fs::write(
        &loader_path,
        strip_optional_marked_section(
            &loader,
            "/* SOW_FIRST_PARTY_ANALYTICS_BEGIN */",
            "/* SOW_FIRST_PARTY_ANALYTICS_END */",
        )?,
    )?;
    fs::write(
        out.join("sdk/store_portals.js"),
        compile_shell_ts(&paths.shell, "sdk/jest_portals.ts")?,
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
    paths.art_pipeline()?.validate_staged_image_inventory(out)?;
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

fn wasm_compile_needed(paths: &Paths) -> Result<bool> {
    if !paths.wasm_input.is_file() {
        return Ok(true);
    }
    let wasm_mtime = fs::metadata(&paths.wasm_input)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    if wasm_mtime == 0 {
        return Ok(true);
    }
    for stamp in local_source_snapshot(paths)? {
        if local_file_requires_wasm(paths, &stamp.path) && stamp.modified_nanos > wasm_mtime {
            return Ok(true);
        }
    }
    Ok(false)
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

fn local_static_fingerprint(paths: &Paths) -> Result<String> {
    let roots = [
        paths.root.join("assets/art"),
        paths.root.join("assets/generated"),
        paths.root.join("assets/site/media"),
        paths.art_pipeline()?.config_path.clone(),
        paths.root.join("assets/campaign"),
        paths.root.join("assets/guides"),
        paths.assets_maps.clone(),
        paths.map_sources.clone(),
        paths.root.join("sow-web/site/fonts"),
    ];
    let mut hash = Sha256::new();
    hash.update(b"local-static-output-v4-art-profiles\0");
    for stamp in local_source_snapshot(paths)?
        .into_iter()
        .filter(|stamp| roots.iter().any(|root| stamp.path.starts_with(root)))
    {
        hash.update(stamp.path.to_string_lossy().as_bytes());
        hash.update(stamp.len.to_le_bytes());
        hash.update(stamp.modified_nanos.to_le_bytes());
    }
    hash_avatar_asset_tree(&mut hash, &paths.assets_gameplay.join("avatars"))?;
    hash.update(file_sha256(&paths.art_pipeline()?.config_path)?.as_bytes());
    if let Ok(staged) = fs::read(paths.runtime_assets.join(".fingerprint")) {
        hash.update(staged);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn prepare_local_art(paths: &Paths) -> Result<()> {
    let pipeline = paths.art_pipeline()?;
    let roots = [
        paths.root.join("assets/art"),
        paths.root.join("assets/generated"),
        paths.root.join("assets/site/media"),
        pipeline.config_path.clone(),
    ];
    let mut hash = Sha256::new();
    hash.update(b"local-runtime-art-v1\0");
    hash.update(pipeline.magick_version.as_bytes());
    hash.update(pipeline.svgo_version.as_bytes());
    for stamp in local_source_snapshot(paths)?
        .into_iter()
        .filter(|stamp| roots.iter().any(|root| stamp.path.starts_with(root)))
    {
        hash.update(stamp.path.to_string_lossy().as_bytes());
        hash.update(stamp.len.to_le_bytes());
        hash.update(stamp.modified_nanos.to_le_bytes());
    }
    let fingerprint = format!("{:x}", hash.finalize());
    let marker = paths.runtime_assets.join(".fingerprint");
    if fs::read_to_string(&marker).is_ok_and(|cached| cached == fingerprint) {
        return Ok(());
    }
    stage_runtime_assets(&paths.root, &paths.runtime_assets, pipeline)?;
    fs::write(marker, fingerprint)?;
    Ok(())
}

fn package_local_preview(
    paths: &Paths,
    out: &Path,
    version: &str,
    _compile: bool,
) -> Result<()> {
    let fingerprint = local_static_fingerprint(paths)?;
    let fingerprint_file = out.join(".local-static-fingerprint");
    let static_ready = fs::read_to_string(&fingerprint_file)
        .is_ok_and(|cached| cached == fingerprint)
        && out.join("maps/catalog.bin").is_file()
        && out.join("fonts/fonts.css").is_file()
        && out
            .join("assets/shell/loader/sow-splash-desktop.webp")
            .is_file()
        && out.join("assets/gameplay/buildings/atlas.webp").is_file()
        && out.join("assets/gameplay/buildings/atlas.json").is_file()
        && GAMEPLAY_UNIT_ART
            .iter()
            .all(|file| out.join("assets/gameplay/units").join(file).is_file())
        && out
            .join("assets/gameplay/music/shadows-of-war.webm")
            .is_file()
        && site_media_ready(out);

    if !static_ready {
        for name in ["assets", "maps", "fonts"] {
            let stale = out.join(name);
            if stale.exists() {
                fs::remove_dir_all(&stale)
                    .with_context(|| format!("reset local preview {}", stale.display()))?;
            }
        }
        fs::create_dir_all(out)?;
        let assets = out.join("assets");
        copy_dir(&paths.assets_shell, &assets.join("shell"))?;
        for name in [
            "avatars",
            "skins",
            "currency",
            "icons",
            "store",
            "buildings",
            "units",
            "music",
        ] {
            let source = paths.assets_gameplay.join(name);
            if source.is_dir() {
                copy_dir(&source, &assets.join("gameplay").join(name))?;
            }
        }
        for name in ["campaign", "guides"] {
            let source = paths.root.join("assets").join(name);
            if source.is_dir() {
                copy_dir(&source, &assets.join(name))?;
            }
        }
        copy_dir(&paths.site_media, &assets.join("site/media"))?;
        let maps = out.join("maps");
        fs::create_dir_all(&maps)?;
        if paths.assets_maps.is_dir() {
            copy_dir(&paths.assets_maps, &maps)?;
        }

        let site = paths.root.join("sow-web/site");
        if site.join("fonts").is_dir()
            && let Err(error) = copy_web_fonts(&site.join("fonts"), &out.join("fonts"))
        {
            eprintln!("⚠ Local web fonts: {error:#}");
            let css = site.join("fonts/fonts.css");
            if css.is_file() {
                fs::copy(css, out.join("fonts/fonts.css"))?;
            }
        }
        for name in ["icon-192.png", "icon-512.png", "icon-512-maskable.png"] {
            let icon = paths.app_icons.join(name);
            if icon.is_file() {
                fs::copy(&icon, out.join(name))?;
            }
        }
        for name in [
            "8d227b8f9e6140d39e3381a1829e1db3.txt",
            "manifest.webmanifest",
        ] {
            let src = site.join(name);
            if src.is_file() {
                fs::copy(&src, out.join(name))?;
            }
        }
        fs::write(
            out.join("robots.txt"),
            "User-agent: *\nAllow: /\nDisallow: /internal/\nDisallow: /api/\nDisallow: /*.wasm$\nDisallow: /*.wasm.br$\n",
        )?;

        if let Err(error) = refresh_map_thumbnails(&maps, &paths.map_sources, paths.art_pipeline()?) {
            eprintln!("⚠ Local map thumbnails: {error:#}");
        } else {
            fs::write(&fingerprint_file, &fingerprint)?;
        }
    }

    require_file(&paths.wasm_input, "WASM client binary")?;
    let wasm_hash = file_sha256(&paths.wasm_input)?;
    let ts = &wasm_hash[..10];
    let bindgen_name = format!("sow_client_{ts}");
    let js_name = format!("{bindgen_name}.js");
    let wasm_name = format!("{bindgen_name}_bg.wasm");

    let wasm_ready = out.join(&wasm_name).is_file() && out.join(&js_name).is_file();

    if !wasm_ready {
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
        run_bindgen(&paths.wasm_input, out, &bindgen_name)?;
    }

    copy_shell(paths, out, WebTarget::Local)?;
    write_locales(out, Some(&paths.root))?;
    let maps_cache_bust = thumbnail_cache_bust(&out.join("maps")).unwrap_or_else(|error| {
        eprintln!("⚠ Local map thumbnails: {error:#}");
        ts.to_owned()
    });
    build_index(
        paths,
        out,
        IndexBuild {
            version,
            js: &js_name,
            wasm: &wasm_name,
            ts,
            maps_cache_bust: &maps_cache_bust,
            target: WebTarget::Local,
        },
    )?;
    write_sw(paths, out, ts)?;
    write_manifest(paths, out, version, &js_name, &wasm_name, ts)?;

    let site = paths.root.join("sow-web/site");
    for path in [
        "privacy",
        "terms",
        "cookies",
        "support",
        "about",
        "free-browser-strategy-game",
        "games-like-travian",
        "games-like-ikariam",
        "multiplayer-territory-control-game",
        "auth",
        ".well-known",
    ] {
        let src = site.join(path);
        if src.is_dir() {
            paths.art_pipeline()?.copy_distribution_tree(
                &src,
                &out.join(path),
                &Path::new("sow-web/site").join(path),
            )?;
        }
    }
    for name in ["index.html", "styles.css"] {
        let src = site.join(name);
        if src.is_file() {
            fs::copy(&src, out.join(name))?;
        }
    }
    for (source, destination, format) in [
        ("app.ts", "app.js", "iife"),
        ("site-chrome.ts", "site-chrome.js", "iife"),
        ("wou-auth.ts", "wou-auth.js", "esm"),
        ("auth-callback.ts", "auth-callback.js", "esm"),
    ] {
        let src = site.join(source);
        if src.is_file() {
            fs::write(
                out.join(destination),
                compile_browser_ts(&src, format, &format!("website {source}"))?,
            )?;
        }
    }
    if let Err(error) = expand_public_site_header(paths, out) {
        eprintln!("⚠ Local landing pages: {error:#}. The game is available at /play/.");
    }
    Ok(())
}

fn build_local_preview(paths: &Paths, version: &str, compile: bool) -> Result<()> {
    if compile {
        compile_wasm(paths, true)?;
    }
    if let Err(error) = prepare_local_art(paths) {
        eprintln!("⚠ Local art staging: {error:#}");
    }
    for folder in [&paths.assets_shell, &paths.assets_gameplay, &paths.site_media] {
        fs::create_dir_all(folder)?;
    }
    fs::create_dir_all(paths.assets_gameplay.join("avatars"))?;
    package_local_preview(paths, &paths.dist_web, version, compile)?;
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

/// Build a self-contained Poki upload directory without running the production deploy.
fn cmd_poki(paths: &Paths) -> Result<()> {
    let version = read_version(paths)?;
    let web = paths.root.join("dist/.sow-state/poki-web");
    println!("==> Building self-contained Poki bundle {version}");
    compile_wasm(paths, false)?;
    package_self(paths, &web, &version, true)?;
    let maps_cache_bust = thumbnail_cache_bust(&web.join("maps"))?;
    package_poki(
        &web,
        &paths.dist_poki,
        paths,
        &version,
        &maps_cache_bust,
    )?;
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in walkdir::WalkDir::new(&paths.dist_poki) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files += 1;
            bytes = bytes
                .checked_add(fs::metadata(entry.path())?.len())
                .context("Poki package size overflow")?;
        }
    }
    println!(
        "✅ Poki upload directory ready: {} ({files} files, {bytes} bytes, {:.2} MiB)",
        paths.dist_poki.display(),
        bytes as f64 / 1_048_576.0
    );
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

fn resolve_electron_bin(native_root: &Path) -> Result<PathBuf> {
    if let Some(custom) = env::var_os("SOW_ELECTRON") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
    }
    if let Ok(output) = Command::new("which").arg("electron").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let p = PathBuf::from(path_str);
                if p.is_file() {
                    return Ok(p);
                }
            }
        }
    }
    for candidate in &["/usr/bin/electron", "/usr/local/bin/electron"] {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Ok(p);
        }
    }
    let local = native_root.join(if cfg!(windows) {
        "node_modules/.bin/electron.cmd"
    } else {
        "node_modules/.bin/electron"
    });
    if local.is_file() {
        return Ok(local);
    }
    bail!(
        "Electron executable not found. Install electron system-wide or run 'npm install' inside sow-native"
    )
}

fn ensure_native_dependencies(native_root: &Path) -> Result<()> {
    require_file(&native_root.join("package.json"), "native package.json")?;
    require_file(&native_root.join("main.js"), "native main.js")?;
    if resolve_electron_bin(native_root).is_ok() {
        return Ok(());
    }
    println!("==> System electron not found; installing local electron dependency...");
    run("npm", &["install", "--include=dev"], Some(native_root))?;
    resolve_electron_bin(native_root)?;
    Ok(())
}

fn launch_electron(native_root: &Path, webroot: &Path) -> Result<()> {
    let electron_bin = resolve_electron_bin(native_root)?;
    let main_js = native_root.join("main.js");
    require_file(&main_js, "Electron main.js")?;
    let webroot_str = webroot
        .to_str()
        .context("native webroot path is not UTF-8")?;

    println!(
        "==> Opening desktop game via Electron ({})",
        electron_bin.display()
    );
    let mut command = Command::new(&electron_bin);
    command.arg(&main_js).arg(webroot_str);
    command.current_dir(native_root);
    command.env("ELECTRON_FORCE_IS_PACKAGED", "0");
    command.env("ELECTRON_IS_DEV", "1");
    command.env("SOW_WEBROOT", webroot_str);
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status().context("run Electron desktop client")?;
    if !status.success() {
        bail!("Electron client exited with status: {status}");
    }
    Ok(())
}

fn cmd_native(paths: &Paths) -> Result<()> {
    let version = read_version(paths)?;
    let native_root = paths.root.join("sow-native");
    ensure_native_dependencies(&native_root)?;

    let webroot = if env::var("SOW_PROD_WEB").is_ok_and(|v| v == "1") {
        let prod_play = paths.dist_web.join("play/index.html");
        if !prod_play.is_file() {
            bail!(
                "production webroot is not built yet (dist/web/play/index.html missing); run ./sow p first"
            );
        }
        println!(
            "==> Using existing production webroot: {}",
            paths.dist_web.display()
        );
        paths.dist_web.canonicalize().context("resolve dist/web")?
    } else {
        let release = env::var("SOW_NATIVE_RELEASE").is_ok_and(|value| value == "1");
        let profile = if release { "wasm-release" } else { "wasm-dev" };
        println!(
            "==> Building native {} WASM client",
            if release { "release" } else { "fast" }
        );
        let wasm = compile_wasm_profile(paths, profile, false)?;
        package_native_web(paths, &version, &wasm, release)?
    };

    launch_electron(&native_root, &webroot)
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
        let start = std::time::Instant::now();
        if let Err(error) = build_local_preview(paths, version, compile) {
            eprintln!("⚠️ Local preview rebuild failed: {error:#}");
        } else {
            println!("✅ Local preview rebuilt in {:.2?}; refresh the browser.", start.elapsed());
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
    let compile = env::var("SOW_FORCE_COMPILE").is_ok_and(|v| v == "1") || wasm_compile_needed(paths)?;
    if compile {
        println!("==> Building local web preview (compiling WASM)...");
    } else {
        println!("⚡ Reusing existing WASM client ({})", paths.wasm_input.display());
        println!("==> Packaging local web preview...");
    }
    let start = std::time::Instant::now();
    build_local_preview(paths, &version, compile)?;
    println!("✅ Local preview ready in {:.2?}", start.elapsed());
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
    if args.first().is_some_and(|arg| arg == "art-preview") {
        if args.len() > 2 {
            bail!("usage: ./sow art-preview [profile]");
        }
        let pipeline = ArtPipeline::load(&root)?;
        pipeline.run_preview(args.get(1).map(String::as_str))?;
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--rotate-secrets") {
        rotate_deployment_secrets(&root)?;
    }
    load_dotenv(&root.join("sow-dist/.env"));

    let mut cmd = String::new();
    let mut bump = false;
    let mut backend_only = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--version" => bump = true,
            flag @ ("--server-only" | "--relay-only") => {
                let selected = if flag == "--server-only" {
                    prod::BackendOnly::Server
                } else {
                    prod::BackendOnly::Relay
                };
                if backend_only.is_some_and(|previous| previous != selected) {
                    bail!("choose one of --server-only and --relay-only");
                }
                backend_only = Some(selected);
            }
            "--rotate-secrets" => {}
            _ if cmd.is_empty() => cmd = args[i].clone(),
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }

    if backend_only.is_some() && cmd != "p" && cmd != "prod" {
        bail!("--server-only and --relay-only are only valid with ./sow p");
    }

    let local_preview = matches!(cmd.as_str(), "l" | "local" | "");
    if !local_preview {
        ensure_generated_secret(&root, "SOW_DB_SECRET")?;
        ensure_generated_secret(&root, "SOW_RELAY_CONTROL_SECRET")?;
        ensure_generated_secret(&root, "SOW_RELAY_SETTLEMENT_SECRET")?;
        ensure_generated_secret(&root, "SOW_REVENUECAT_WEBHOOK_SECRET")?;
    }

    let paths = if local_preview {
        Paths::discover_for_local()?
    } else if cmd == "p" || cmd == "prod" {
        Paths::discover_for_prod(backend_only.is_some())?
    } else if cmd == "poki" {
        Paths::discover_for_poki()?
    } else {
        Paths::discover()?
    };

    match cmd.as_str() {
        "p" | "prod" => prod::execute(&paths, bump, backend_only),
        "a" | "android" => prod::execute_android(&paths),
        "jest" => cmd_jest(&paths),
        "poki" => cmd_poki(&paths),
        "native" => cmd_native(&paths),
        "local" | "l" | "" => cmd_local(&paths),
        _ => {
            eprintln!("Usage: ./sow [native|l|p|a|poki|jest|art-preview]");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poki_package_keeps_runtime_assets_and_only_drops_explicit_exclusions() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("runtime-assets");
        let packaged = temp.path().join("poki-assets");
        for dir in ["campaign", "shell/leaders"] {
            fs::create_dir_all(source.join(dir))?;
        }
        let files = [
            "gameplay/avatars/hero.webp",
            "gameplay/buildings/atlas.webp",
            "gameplay/icons/abilities/hero.webp",
            "gameplay/skins/unused.webp",
            "gameplay/store/unused.webp",
            "shell/brand/logo.webp",
            "shell/mobile-nav/battle.webp",
            "shell/mobile-nav/store.webp",
            "site/media/discord.svg",
            "campaign/boudica/opening.mp4",
            "campaign/boudica/opening.webm",
        ];
        for relative in files {
            let path = source.join(relative);
            fs::create_dir_all(path.parent().context("runtime asset has no parent")?)?;
            fs::write(path, relative.as_bytes())?;
        }

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize()?;
        let mut pipeline = ArtPipeline::load(&root)?;
        pipeline.cache_root = temp.path().join("art-cache");
        copy_poki_assets(&source, &packaged, &pipeline)?;
        verify_poki_asset_inventory(&source, &packaged)?;

        let expected = [
            "campaign/boudica/opening.mp4",
            "gameplay/avatars/hero.webp",
            "gameplay/buildings/atlas.webp",
            "gameplay/icons/abilities/hero.webp",
            "shell/mobile-nav/battle.webp",
        ]
        .map(PathBuf::from)
        .to_vec();
        assert_eq!(poki_asset_inventory(&source)?, expected);
        assert_eq!(poki_asset_inventory(&packaged)?, expected);
        Ok(())
    }

    #[test]
    fn published_biographies_cover_every_catalog_hero() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let catalog = public_leader_biographies(&root)?;
        let locales = catalog["leaders"]
            .as_object()
            .context("published biographies have no locales")?;

        for (language, biographies) in locales {
            let biographies = biographies
                .as_object()
                .with_context(|| format!("published biographies for {language} are invalid"))?;
            for leader in sow_core::player::Leader::all() {
                assert!(
                    biographies.contains_key(leader.id()),
                    "published biographies for {language} are missing {}",
                    leader.id()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn leader_biographies_require_explicit_locales_and_fallbacks() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("sample_hero.toml");
        let article = r#"[articles.en]
lang = "en"
url = "https://example.com/hero"
intro = []
shortArticle = true
"#;

        fs::write(&path, format!("{article}\n[fallbacks]\nfr = \"en\"\n"))?;
        let mut distinct_articles = std::collections::HashSet::new();
        let resolved =
            load_leader_biography(&path, "sample_hero", &["en", "fr"], &mut distinct_articles)?;
        assert_eq!(resolved["en"], resolved["fr"]);

        fs::write(&path, article)?;
        let error = load_leader_biography(
            &path,
            "sample_hero",
            &["en", "fr"],
            &mut std::collections::HashSet::new(),
        )
        .expect_err("missing French biography must fail");
        let message = format!("{error:#}");
        assert!(message.contains(path.to_str().context("temporary path is not UTF-8")?));
        assert!(message.contains("fr/sample_hero"));

        fs::write(&path, format!("{article}\n[fallbacks]\nfr = \"de\"\n"))?;
        let error = load_leader_biography(
            &path,
            "sample_hero",
            &["en", "fr"],
            &mut std::collections::HashSet::new(),
        )
        .expect_err("broken French fallback must fail");
        let message = format!("{error:#}");
        assert!(message.contains(path.to_str().context("temporary path is not UTF-8")?));
        assert!(message.contains("fr/sample_hero"));
        assert!(message.contains("de"));
        Ok(())
    }

    #[test]
    fn ssh_transport_reuses_one_connection_for_checks_and_transfers() {
        let script = "printf '%s' '$literal; remote command'";
        for program in ["ssh", "scp"] {
            let mut transport = command(program);
            transport.args(["configured-host", script]);
            let arguments = transport.get_args().collect::<Vec<_>>();
            assert_eq!(&arguments[..SSH_REUSE_ARGS.len()], SSH_REUSE_ARGS);
            assert_eq!(
                &arguments[SSH_REUSE_ARGS.len()..],
                ["configured-host", script]
            );
        }
        let mut transfer = command("rsync");
        transfer.args([
            "-azc",
            "source with spaces/",
            "configured-host:destination/",
        ]);
        let arguments = transfer.get_args().collect::<Vec<_>>();
        assert_eq!(arguments[0], "-e");
        assert_eq!(
            arguments[1],
            format!("ssh {}", SSH_REUSE_ARGS.join(" ")).as_str()
        );
        assert_eq!(
            &arguments[2..],
            [
                "-azc",
                "source with spaces/",
                "configured-host:destination/"
            ]
        );
        assert_eq!(command("bun").get_args().count(), 0);
    }

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
        for (index, expression) in sow_core::emote::AvatarExpression::ALL
            .into_iter()
            .enumerate()
        {
            let crop = hero_expression_crop(&sheet, expression)?;
            assert_eq!(crop.dimensions(), (HERO_PORTRAIT_PX, HERO_PORTRAIT_PX));
            assert!(crop.pixels().all(|pixel| pixel.0 == colors[index]));
        }
        Ok(())
    }

    fn avatar(key: &str, group: &str, side: u32) -> AvatarSource {
        AvatarSource {
            key: key.to_owned(),
            group: group.to_owned(),
            side,
            file: PathBuf::from(key),
        }
    }

    #[test]
    fn avatar_sheets_keep_every_cell_inside_a_sheet_of_one_group() -> Result<()> {
        let mut avatars: Vec<AvatarSource> = (0..205)
            .map(|index| avatar(&format!("gameplay/avatars/nation_{index:03}.webp"), "europe", 192))
            .collect();
        avatars.extend((0..70).map(|index| {
            avatar(&format!("gameplay/avatars/expressions/hero_{index:02}_fear.webp"), "heroes", 256)
        }));
        avatars.push(avatar("gameplay/avatars/random.webp", "other", 256));

        let sheets = plan_avatar_sheets(&avatars)?;
        let placed = sheets.iter().map(|sheet| sheet.cells.len()).sum::<usize>();
        assert_eq!(placed, avatars.len(), "every avatar lands on exactly one sheet");
        for sheet in &sheets {
            assert!(sheet.width <= CG_AVATAR_SHEET_PX && sheet.height <= CG_AVATAR_SHEET_PX);
            for cell in &sheet.cells {
                assert!(cell.x + cell.side <= sheet.width, "{} leaves {}", cell.key, sheet.path);
                assert!(cell.y + cell.side <= sheet.height, "{} leaves {}", cell.key, sheet.path);
            }
        }
        // 205 nations at 192 px: 100 per full sheet, so three sheets for the europe group.
        assert_eq!(sheets.iter().filter(|sheet| sheet.path.contains("europe-192")).count(), 3);
        // 70 heroes at 256 px: 64 per sheet; random sits alone in its own sheet.
        assert_eq!(sheets.iter().filter(|sheet| sheet.path.contains("heroes-256")).count(), 2);
        assert!(sheets.iter().any(|sheet| sheet.path.contains("other-256") && sheet.cells.len() == 1));
        Ok(())
    }

    #[test]
    fn avatar_atlas_config_rejects_a_cell_outside_its_sheet() -> Result<()> {
        let html = r#"<script>window.SOW_AVATAR_ATLAS = {"sheets":[{"path":"gameplay/avatars/atlas/x.webp","width":256,"height":256}],"cells":{"gameplay/avatars/caesar.webp":[0,128,0,256]}};</script>"#;
        let config = cg_avatar_atlas_config(html)?;
        assert_eq!(config["cells"]["gameplay/avatars/caesar.webp"][3], 256);
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path().join("assets/gameplay/avatars/atlas"))?;
        image::RgbaImage::new(256, 256).save(dir.path().join("assets/gameplay/avatars/atlas/x.webp"))?;
        let error = verify_cg_avatar_atlas(dir.path(), html).unwrap_err();
        assert!(error.to_string().contains("leaves its sheet"), "{error:#}");
        Ok(())
    }

    #[test]
    fn avatar_atlas_requires_all_four_faces_of_every_leader() -> Result<()> {
        let mut cells = serde_json::Map::new();
        for leader in sow_core::player::Leader::all() {
            for expression in sow_core::emote::AvatarExpression::ALL {
                let key = sow_core::emote::hero_portrait_path(leader.art_slug(), expression);
                cells.insert(key, serde_json::json!([0, 0, 0, 256]));
            }
        }
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path().join("assets/gameplay/avatars/atlas"))?;
        image::RgbaImage::new(256, 256).save(dir.path().join("assets/gameplay/avatars/atlas/x.webp"))?;
        let html = |cells: &serde_json::Map<String, serde_json::Value>| {
            let atlas = serde_json::json!({
                "sheets": [{ "path": "gameplay/avatars/atlas/x.webp", "width": 256, "height": 256 }],
                "cells": cells,
            });
            format!("<script>window.SOW_AVATAR_ATLAS = {atlas};</script>")
        };
        verify_cg_avatar_atlas(dir.path(), &html(&cells))?;
        let leader = sow_core::player::Leader::all()
            .into_iter()
            .next()
            .context("no playable leader")?;
        let missing = sow_core::emote::hero_portrait_path(
            leader.art_slug(),
            sow_core::emote::AvatarExpression::Fear,
        );
        cells.remove(&missing);
        let error = verify_cg_avatar_atlas(dir.path(), &html(&cells)).unwrap_err();
        assert!(error.to_string().contains(&missing), "{error:#}");
        Ok(())
    }

    #[test]
    fn avatar_asset_tree_fingerprint_tracks_content_and_removed_duplicates() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let avatars = temp.path().join("avatars");
        fs::create_dir_all(avatars.join("expressions"))?;
        fs::write(avatars.join("hero.webp"), b"base calm portrait")?;
        fs::write(
            avatars.join("expressions/hero_calm.webp"),
            b"duplicate calm",
        )?;
        let fingerprint = |root: &Path| -> Result<String> {
            let mut hash = Sha256::new();
            hash_avatar_asset_tree(&mut hash, root)?;
            Ok(format!("{:x}", hash.finalize()))
        };

        let with_duplicate = fingerprint(&avatars)?;
        fs::remove_file(avatars.join("expressions/hero_calm.webp"))?;
        let without_duplicate = fingerprint(&avatars)?;
        assert_ne!(with_duplicate, without_duplicate);

        fs::write(avatars.join("hero.webp"), b"updated base calm portrait")?;
        assert_ne!(without_duplicate, fingerprint(&avatars)?);
        Ok(())
    }

    #[test]
    fn art_profiles_cache_by_content_and_recipe_without_mutating_masters() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("hero.png");
        image::RgbaImage::from_pixel(512, 256, image::Rgba([32, 64, 96, 255])).save(&source)?;
        let source_hash = file_sha256(&source)?;
        let out = temp.path().join("runtime-assets");
        let relative = "gameplay/avatars/hero.webp";
        let destination = out.join(relative);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize()?;
        let mut pipeline = ArtPipeline::load(&root)?;
        pipeline.cache_root = temp.path().join("art-cache");
        pipeline.publish_image(&source, &destination, "avatar_portrait_192", None, None)?;
        assert_eq!(image::image_dimensions(&destination)?, (192, 96));
        assert_eq!(
            image::guess_format(&fs::read(&destination)?)?,
            image::ImageFormat::WebP
        );
        assert_eq!(file_sha256(&source)?, source_hash);
        let original_output_hash = file_sha256(&destination)?;
        pipeline.publish_image(&source, &destination, "avatar_portrait_192", None, None)?;
        assert_eq!(file_sha256(&destination)?, original_output_hash);
        let cache_dir = pipeline.cache_root.join("avatar_portrait_192");
        let metadata_dir = pipeline
            .cache_root
            .join("metadata/avatar_portrait_192");
        assert_eq!(fs::read_dir(&cache_dir)?.count(), 1, "same source+recipe reuses one derivative");
        assert_eq!(fs::read_dir(&metadata_dir)?.count(), 1, "validated cache metadata is reused");

        let cached_file = fs::read_dir(&cache_dir)?.next().context("cached derivative missing")??.path();
        fs::write(&cached_file, b"corrupt cache")?;
        pipeline.publish_image(&source, &destination, "avatar_portrait_192", None, None)?;
        assert_eq!(file_sha256(&destination)?, original_output_hash, "corrupt cache is regenerated");

        image::RgbaImage::from_pixel(512, 256, image::Rgba([80, 120, 160, 255])).save(&source)?;
        pipeline.publish_image(&source, &destination, "avatar_portrait_192", None, None)?;
        assert_ne!(file_sha256(&destination)?, original_output_hash);
        assert_eq!(fs::read_dir(&cache_dir)?.count(), 2, "changed source creates a new derivative");
        assert_eq!(fs::read_dir(&metadata_dir)?.count(), 2, "changed source gets new validation metadata");

        pipeline.config.profiles.get_mut("avatar_portrait_192").unwrap().recipe_version += 1;
        pipeline.publish_image(&source, &destination, "avatar_portrait_192", None, None)?;
        assert_eq!(fs::read_dir(&cache_dir)?.count(), 3, "changed recipe invalidates its cache key");
        assert_eq!(fs::read_dir(&metadata_dir)?.count(), 3, "changed recipe gets new validation metadata");
        assert_eq!(image::image_dimensions(&destination)?, (192, 96));
        Ok(())
    }

    #[test]
    fn art_profiles_preserve_real_transparency() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("transparent.png");
        let mut image = image::RgbaImage::from_pixel(512, 256, image::Rgba([32, 64, 96, 255]));
        image.put_pixel(128, 96, image::Rgba([32, 64, 96, 0]));
        image.save(&source)?;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize()?;
        let mut pipeline = ArtPipeline::load(&root)?;
        pipeline.cache_root = temp.path().join("art-cache");
        let destination = temp.path().join("published/transparent.webp");
        pipeline.publish_image(&source, &destination, "avatar_portrait_192", None, None)?;
        assert!(image_has_transparency(&destination)?, "transparent pixels must survive WebP conversion");
        pipeline.publish_image(&source, &destination, "avatar_portrait_192", None, None)?;
        assert!(image_has_transparency(&destination)?, "cache hits must retain validated transparency");
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
            "/assets/shell/loader/sow-splash-desktop.webp",
            "/assets/shell/loader/sow-splash-mobile.webp",
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
    fn landing_splash_uses_each_optimized_asset_hash() -> Result<()> {
        let media = tempfile::tempdir()?;
        let html = landing_hero_media(media.path(), "desktop1234", "mobile5678");
        assert!(html.contains("sow-splash-desktop.webp?v=desktop1234"));
        assert!(html.contains("sow-splash-mobile.webp?v=mobile5678"));
        assert!(!html.contains("sow-splash-mobile.webp?v=desktop1234"));
        Ok(())
    }

    #[test]
    fn unprofiled_raster_art_fails_closed() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("new-art.png");
        image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255])).save(&source)?;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize()?;
        let mut pipeline = ArtPipeline::load(&root)?;
        pipeline.cache_root = temp.path().join("art-cache");
        assert!(
            pipeline
                .publish_asset(
                    &source,
                    &temp.path().join("published/new-art.webp"),
                    Path::new("new-page/new-art.webp"),
                    None,
                )
                .is_err(),
            "new raster output without a profile must fail instead of copying"
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
    fn web_leader_packaging_uses_profile_without_mutating_master() -> Result<()> {
        let root = tempfile::tempdir()?;
        let leaders = root.path().join("leaders");
        fs::create_dir_all(&leaders)?;
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let master = project.join("assets/art/heroes/boudica_desktop.webp");
        let original_hash = file_sha256(&master)?;
        let destination = leaders.join("boudica_desktop.webp");
        let mut pipeline = ArtPipeline::load(&project)?;
        pipeline.cache_root = root.path().join("art-cache");
        pipeline.publish_image(&master, &destination, "fullbleed_wide", None, None)?;
        let (width, height) = image::image_dimensions(&destination)?;
        let after = fs::metadata(&destination)?.len();
        assert!(
            width <= 1600 && height <= 900 && after <= 400_000,
            "boudica_desktop output is {width}x{height}, {after} bytes; cap 1600x900 and 400000"
        );
        assert_eq!(file_sha256(&master)?, original_hash, "source master must remain untouched");
        let corrupt = root.path().join("corrupt.webp");
        fs::write(&corrupt, b"not-an-image")?;
        assert!(
            pipeline
                .publish_image(&corrupt, &leaders.join("corrupt.webp"), "fullbleed_wide", None, None)
                .is_err(),
            "a corrupt image must fail the build, never ship silently"
        );
        Ok(())
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
            "analytics.ts",
            "app.ts",
            "site-chrome.ts",
            "site-header.html",
            "styles.css",
            "fonts/fonts.css",
            "fonts/work-sans-latin.woff2",
            "fonts/work-sans-latin-ext.woff2",
            "fonts/work-sans-italic-latin.woff2",
            "fonts/work-sans-italic-latin-ext.woff2",
            "wou-auth.ts",
            "auth-callback.ts",
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
        assert!(site.join("leaders/index.html").exists());
        assert!(!site.join("how-to-play/index.html").exists());
        for required in [
            "index.html.template",
            "main_menu.css",
            "main_menu.ts",
            "sow-i18n.ts",
            "sow-dropdown.ts",
            "experience_events.ts",
            "loader.ts",
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
            "main_menu.missions.css",
            "sow-campaign.ts",
            "sow-campaign-view.ts",
            "campaign/types.ts",
            "campaign/rules.ts",
            "campaign/view.ts",
            "campaign/validation.ts",
            "campaign/menu-guide-validation.ts",
            "campaign/runtime.ts",
            "campaign/validation-context.ts",
            "campaign/validation-flow.ts",
            "campaign/validation-reactions.ts",
            "campaign/validation-steps.ts",
            "campaign/tutorial-guides.ts",
            "main_menu.menu-guide.ts",
            "main_menu.core.ts",
            "main_menu.motion.ts",
            "main_menu.lobbies.ts",
            "main_menu.store.ts",
            "main_menu.surfaces.ts",
            "main_menu.portal-stubs.ts",
            "main_menu.poki.ts",
            "main_menu.jest.ts",
            "main_menu.heroes.ts",
            "main_menu.profile.ts",
            "main_menu.missions.ts",
            "main_menu.tutorial.ts",
            "main_menu.shell.ts",
            "main_menu.hud.ts",
            "main_menu.flow.ts",
        ] {
            assert!(
                root.join("sow-web/shell").join(required).is_file(),
                "Required game shell source file missing: {required}"
            );
        }
        for required in [
            "sdk/store_portals.ts",
            "sdk/poki_portals.ts",
            "sdk/jest_portals.ts",
        ] {
            assert!(
                root.join("sow-web/shell").join(required).is_file(),
                "Required game shell source file missing: {required}"
            );
        }
        for required in [
            "LICENSE",
            "docs/legal/NOTICE",
            "docs/legal/LICENSE-ASSETS",
            "docs/legal/WASM-THIRD-PARTY-NOTICES.zip",
        ] {
            assert!(
                root.join(required).is_file(),
                "Required package source file missing: {required}"
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
            let versions_marker = "window.SOW_ASSET_TS = ";
            let versions_start = html
                .find(versions_marker)
                .context("asset versions injection missing")?
                + versions_marker.len();
            let versions_end = html[versions_start..]
                .find(";\n")
                .context("asset versions injection unterminated")?
                + versions_start;
            let injected_versions: serde_json::Value =
                serde_json::from_str(&html[versions_start..versions_end])?;
            let portrait_path = paths.assets_gameplay.join("avatars/boudica.webp");
            let expected_hash = file_sha256(&portrait_path)?[..8].to_string();
            assert_eq!(
                injected_versions
                    .get("/assets/gameplay/avatars/boudica.webp")
                    .and_then(serde_json::Value::as_str),
                Some(expected_hash.as_str()),
                "{target:?}: avatar URL version must follow its content"
            );
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
            let hashed_menu = fs::read_dir(&page_dir)?
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
            if let Some(name) = hashed_menu.as_ref() {
                assert!(
                    html.contains(&format!("<script src=\"./{name}\"></script>")),
                    "{target:?}: page must load the exact hashed bundle it shipped"
                );
                if target == WebTarget::Local {
                    for marker in [
                        "__SOW_SPLASH_DESKTOP_DATA__",
                        "__SOW_SPLASH_MOBILE_DATA__",
                        "__INLINE_MAIN_MENU_CSS__",
                        "__INLINE_MAIN_MENU_JS__",
                    ] {
                        assert!(!html.contains(marker), "Local shell retains {marker}");
                    }
                    assert!(!html.contains("data:image/webp;base64,"));
                    for name in ["sow-splash-desktop.webp", "sow-splash-mobile.webp"] {
                        let path = paths.assets_shell.join("loader").join(name);
                        let hash = file_sha256(&path)?[..8].to_owned();
                        assert!(
                            html.contains(&format!("/assets/shell/loader/{name}?v={hash}")),
                            "local loader must use the published splash hash for {name}"
                        );
                    }
                    assert!(html.contains("#sow-menu"));
                    assert!(html.contains("SOW_menu_command"));
                    let bundle = fs::read_to_string(page_dir.join(name))?;
                    assert!(bundle.contains("sow-campaign-view.ts"));
                    assert!(bundle.contains("window.SOW_onStateUpdate"));
                    assert!(html.contains("sow-hud__dock"));
                    assert!(html.contains("../analytics.js?v=test"));
                    assert!(
                        html.contains("name=\"sow-media-artwork\" content=\"../icon-192.png\"")
                    );
                }
            }
            assert_eq!(html.contains("window.SOW_BUILDING_ART ="), !external);

            if matches!(target, WebTarget::Poki | WebTarget::Jest) {
                assert!(!html.contains("analytics.js"));
                assert!(html.contains("name=\"sow-media-artwork\" content=\"icon-192.png\""));
                assert!(html.contains("href=\"fonts/fonts.css?v=test\""));
                assert!(html.contains("./assets/shell/loader/loader_empty.webp"));
                assert!(html.contains(if target == WebTarget::Poki {
                    "main_menu.poki.ts"
                } else {
                    "main_menu.jest.ts"
                }));
                assert!(!html.contains("main_menu.store.ts"));
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
                    assert!(
                        !html.contains(forbidden),
                        "{target:?} shell contains {forbidden}"
                    );
                }
                if target == WebTarget::Jest {
                    assert!(html.contains("jest_login"));
                    assert!(!html.contains("main_menu.poki.ts"));
                    assert!(!html.contains("SOW_startWouOAuth"));
                }
            }
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
        let tutorial_js_path = paths.root.join("sow-web/shell/main_menu.tutorial.ts");
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
        let explorer_html = fs::read_to_string(root.join("sow-web/site/leader-explorer.html"))?;
        for marker in [
            "<!-- SOW_SITE_HERO_MEDIA -->",
            "<!-- SOW_SITE_LEADER_EXPLORER -->",
            "<!-- SOW_SITE_MAPS -->",
            "<!-- SOW_SITE_TRAILER -->",
        ] {
            assert_eq!(index_html.matches(marker).count(), 1, "{marker}");
        }
        assert!(
            !index_html.contains("data-leader-id"),
            "heroes come from the catalog"
        );
        for marker in ["<!-- SOW_SITE_LEADER_SHOWCASE -->", "<!-- SOW_SITE_LEADERS -->"] {
            assert_eq!(explorer_html.matches(marker).count(), 1, "{marker}");
        }
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
        let missing = serde_json::json!({ "site": {} });

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
            conf.contains("location ~ ^/how-to-play/?$")
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

    #[test]
    fn web_menu_bundle_executes_on_load() -> Result<()> {
        // The shell bundle is one shared closure. If Bun wraps it as a CommonJS
        // entry the factory is never invoked and the menu/campaign runtime never
        // reaches `window`, which leaves the loader stuck on "Loading...".
        let paths = Paths::discover()?;
        let bundle = read_menu_bundle(&paths.shell, None, false)?;
        assert!(
            bundle.contains("window.SOWCampaign ="),
            "shell bundle lost the campaign runtime"
        );
        assert!(
            !bundle.contains("= __commonJS("),
            "shell bundle entry became a lazy CommonJS wrapper that never runs"
        );
        Ok(())
    }
}
