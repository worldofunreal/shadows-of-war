//! Import map-source folders (PNG + info.json or map.bin).

use serde_json::Value;
use sow_core::map_file::{self, MapFile, MapSpawn};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct ImportArgs {
    pub input: PathBuf,
    pub name: Option<String>,
    pub maps_root: PathBuf,
    /// PNG to import instead of `<input>/image.png`.
    pub image: Option<PathBuf>,
    /// Crop rectangle `x, y, w, h` in source pixels.
    pub crop: Option<[u32; 4]>,
    /// Size preset (`small`, `regular`, `big`, `huge`); defaults to the mobile-safe size.
    pub size: Option<String>,
    /// Rotation weight; overrides `multiplayer_frequency` from info.json.
    pub frequency: Option<u32>,
}

pub fn run_import(args: ImportArgs) -> Result<(), Box<dyn std::error::Error>> {
    let input = args.input.canonicalize().unwrap_or(args.input.clone());
    if !input.is_dir() {
        return Err(format!("Input is not a directory: {}", input.display()).into());
    }

    let slug = args
        .name
        .clone()
        .unwrap_or_else(|| input.file_name().unwrap().to_string_lossy().to_string());
    let slug = sow_core::maps::map_key(&slug);

    // Display name from the source info.json ("Africa", "Europe", ...); fall back to slug.
    let display_name = read_info_json_name(&input).unwrap_or_else(|| slug.clone());

    let png_path = args
        .image
        .clone()
        .unwrap_or_else(|| input.join("image.png"));
    let (mut map_file, thumbnail_source) = if png_path.exists() {
        let (map_file, source) = import_from_png(
            &png_path,
            &input,
            &display_name,
            args.crop,
            args.size.as_deref(),
        )?;
        (map_file, Some(source))
    } else {
        (import_from_bin(&input)?, None)
    };

    fs::create_dir_all(&args.maps_root)?;
    let out_dir = args.maps_root.join(&slug);
    fs::create_dir_all(&out_dir)?;

    if out_dir.join("map.bin").is_file() || out_dir.join("map.bin.br").is_file() {
        let existing = map_file::parse(&read_map_payload(&out_dir)?)?;
        map_file::preserve_rosters(&existing, &mut map_file)
            .map_err(|message| format!("cannot replace map '{slug}': {message}"))?;
    }

    let encoded = map_file::encode(&map_file);
    fs::write(out_dir.join("map.bin"), &encoded)?;

    let mut brotli_out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut brotli_out, 4096, 11, 22);
    writer.write_all(&encoded)?;
    writer.flush()?;
    drop(writer);
    fs::write(out_dir.join("map.bin.br"), brotli_out)?;

    let thumb_path = out_dir.join("thumbnail.webp");
    match thumbnail_source {
        Some(img) => {
            sow_map::write_wide_thumbnail(&img, &thumb_path).map_err(|e| format!("thumbnail: {e}"))?
        }
        None => write_placeholder_thumbnail(&map_file, &thumb_path)?,
    }

    // Persist the source `multiplayer_frequency` as info.toml so the server's
    // catalog scan can build weighted playlists (0 = out of rotation).
    let frequency = args
        .frequency
        .unwrap_or_else(|| read_info_json_frequency(&input));
    fs::write(
        out_dir.join("info.toml"),
        format!("# Written by sow-tools import-map-source\nfrequency = {frequency}\n"),
    )?;

    refresh_catalog(&args.maps_root)?;

    println!(
        "Imported '{}' → {}/ ({}x{}, {} spawns, {} land tiles, freq {})",
        slug,
        out_dir.display(),
        map_file.width,
        map_file.height,
        map_file.spawns.len(),
        map_file.num_land_tiles,
        frequency
    );
    Ok(())
}

/// `multiplayer_frequency` from a source `info.json` (absent → 0, matching
/// the source schema: omitted/0 keeps the map out of the regular rotation).
fn read_info_json_frequency(dir: &Path) -> u32 {
    let Ok(blob) = fs::read_to_string(dir.join("info.json")) else {
        return 0;
    };
    let Ok(info) = serde_json::from_str::<Value>(&blob) else {
        return 0;
    };
    info.get("multiplayer_frequency")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
}

/// `name` from a source `info.json` (used as the catalog display name).
fn read_info_json_name(dir: &Path) -> Option<String> {
    let blob = fs::read_to_string(dir.join("info.json")).ok()?;
    let info: Value = serde_json::from_str(&blob).ok()?;
    info.get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Import a PNG (optionally cropped and resized). Returns the map and the
/// source image the thumbnail is drawn from.
fn import_from_png(
    png_path: &Path,
    info_dir: &Path,
    display_name: &str,
    crop: Option<[u32; 4]>,
    size: Option<&str>,
) -> Result<(MapFile, image::DynamicImage), Box<dyn std::error::Error>> {
    let original = image::open(png_path)?;
    let cropped = match crop {
        Some([x, y, w, h]) => {
            let inside = w > 0
                && h > 0
                && x.checked_add(w).is_some_and(|right| right <= original.width())
                && y.checked_add(h).is_some_and(|bottom| bottom <= original.height());
            if !inside {
                return Err(format!(
                    "crop {x},{y},{w},{h} is outside the {}x{} source image",
                    original.width(),
                    original.height()
                )
                .into());
            }
            original.crop_imm(x, y, w, h)
        }
        None => original,
    };
    let (src_w, src_h) = (cropped.width(), cropped.height());
    // Keep the full categorical source intact until the image pipeline's
    // water-aware downscale. Without a size preset, use the same
    // mobile-safe dimensions as the canonical image-map path.
    let (target_w, target_h) = match size {
        Some(preset) => sow_core::maps::map_size_for_preset(preset, src_w, src_h).ok_or_else(|| {
            format!("unknown size '{preset}' (use small, regular, big or huge)")
        })?,
        None => sow_map::image_pipeline::mobile_safe_dims(src_w, src_h),
    };
    if target_w < 4
        || target_h < 4
        || target_w > sow_core::maps::MAX_MAP_AXIS
        || target_h > sow_core::maps::MAX_MAP_AXIS
        || !sow_core::maps::map_within_pixel_budget(target_w, target_h)
    {
        return Err(format!(
            "size {target_w}x{target_h} is outside the map limits (4 to {} per side, {} tiles total)",
            sow_core::maps::MAX_MAP_AXIS,
            sow_core::maps::MAX_MAP_PIXELS
        )
        .into());
    }
    let rgba = cropped.to_rgba8();
    let result = sow_map::image_pipeline::generate_from_rgba(&rgba, Some((target_w, target_h)))
        .map_err(|e| format!("image pipeline: {e}"))?;
    // Same categorical render as the canonical thumbnails (world, asia...), so water matches the lobby blue.
    let thumbnail = sow_map::render_source_image(&cropped).map_err(|e| format!("thumbnail: {e}"))?;

    // info.json spawns are in full-image pixels, so a crop has no matching spawns.
    let mut spawns = if crop.is_some() {
        Vec::new()
    } else {
        load_info_json_spawns(info_dir)?
    };
    // Spawn coordinates come from the native-resolution info.json — scale them
    // down with the image so they stay on land after clamping.
    if src_w != target_w || src_h != target_h {
        for s in &mut spawns {
            let rx = (s.x as f64 * (target_w as f64 / src_w as f64)).round() as u32;
            let ry = (s.y as f64 * (target_h as f64 / src_h as f64)).round() as u32;
            s.x = rx.min(target_w.saturating_sub(1));
            s.y = ry.min(target_h.saturating_sub(1));
        }
    }

    let map_file = MapFile {
        display_name: display_name.to_string(),
        width: result.width,
        height: result.height,
        num_land_tiles: result.num_land_tiles,
        spawns,
        geo_bounds: None,
        default_roster: None,
        rosters: Vec::new(),
        terrain: result.map_data,
    };
    Ok((map_file, image::DynamicImage::ImageRgba8(thumbnail)))
}

fn load_info_json_spawns(dir: &Path) -> Result<Vec<MapSpawn>, Box<dyn std::error::Error>> {
    let info_path = dir.join("info.json");
    if !info_path.exists() {
        return Ok(Vec::new());
    }
    let info: Value = serde_json::from_str(&fs::read_to_string(&info_path)?)?;
    let mut spawns = Vec::new();
    if let Some(arr) = info.as_array() {
        for entry in arr {
            push_spawn(entry, &mut spawns);
        }
    } else if let Some(nations) = info.get("nations").and_then(|v| v.as_array()) {
        for entry in nations {
            push_spawn(entry, &mut spawns);
        }
    }
    Ok(spawns)
}

fn push_spawn(entry: &Value, spawns: &mut Vec<MapSpawn>) {
    let name = entry
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Nation")
        .to_string();
    let flag = entry
        .get("flag")
        .and_then(|v| v.as_str())
        .unwrap_or("🏳")
        .to_string();
    let coords = entry.get("coordinates").and_then(|v| v.as_array());
    let x = coords
        .and_then(|c| c.first())
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let y = coords
        .and_then(|c| c.get(1))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    spawns.push(MapSpawn { name, flag, x, y });
}

pub fn clamp_map_dimensions_proportional(width: u32, height: u32, max_pixels: u32) -> (u32, u32) {
    if (width as u64) * (height as u64) <= max_pixels as u64 {
        return (
            sow_core::maps::align_map_dim(width),
            sow_core::maps::align_map_dim(height),
        );
    }
    let aspect = width as f64 / height as f64;
    let h = (max_pixels as f64 / aspect).sqrt();
    let w = h * aspect;

    let mut target_w = sow_core::maps::align_map_dim(w.round() as u32);
    let mut target_h = sow_core::maps::align_map_dim(h.round() as u32);

    while (target_w as u64) * (target_h as u64) > max_pixels as u64 {
        if target_w > target_h {
            target_w = (target_w - 4).max(4);
        } else {
            target_h = (target_h - 4).max(4);
        }
    }
    (target_w, target_h)
}

fn import_from_bin(dir: &Path) -> Result<MapFile, Box<dyn std::error::Error>> {
    let raw = read_map_payload(dir)?;
    if raw.len() >= 4 && &raw[0..4] == map_file::MAP_MAGIC {
        let mut map = map_file::parse(&raw)?;
        let width = map.width;
        let height = map.height;
        if (width as u64) * (height as u64) > sow_core::maps::MAX_MAP_PIXELS as u64 {
            let (target_w, target_h) =
                clamp_map_dimensions_proportional(width, height, sow_core::maps::MAX_MAP_PIXELS);
            let mut rescaled = Vec::with_capacity((target_w * target_h) as usize);
            for ty in 0..target_h {
                for tx in 0..target_w {
                    let sx = (tx as f64 * (width as f64 / target_w as f64)).floor() as u32;
                    let sy = (ty as f64 * (height as f64 / target_h as f64)).floor() as u32;
                    let sx = sx.min(width - 1);
                    let sy = sy.min(height - 1);
                    rescaled.push(map.terrain[(sy * width + sx) as usize]);
                }
            }
            for s in &mut map.spawns {
                let rx = (s.x as f64 * (target_w as f64 / width as f64)).round() as u32;
                let ry = (s.y as f64 * (target_h as f64 / height as f64)).round() as u32;
                s.x = rx.min(target_w - 1);
                s.y = ry.min(target_h - 1);
            }
            for preset in &mut map.rosters {
                for entity in &mut preset.entries {
                    let rx = (entity.x as f64 * (target_w as f64 / width as f64)).round() as u32;
                    let ry = (entity.y as f64 * (target_h as f64 / height as f64)).round() as u32;
                    entity.x = rx.min(target_w - 1);
                    entity.y = ry.min(target_h - 1);
                }
            }
            map.width = target_w;
            map.height = target_h;
            map.num_land_tiles = rescaled.iter().filter(|&&b| (b & 0x80) != 0).count() as u32;
            map.terrain = rescaled;
            map_file::validate_rosters(&map, false)?;
        }
        return Ok(map);
    }

    Err("Need image.png or a valid map.bin".into())
}

fn read_map_payload(map_dir: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bin_path = map_dir.join("map.bin");
    let br_path = map_dir.join("map.bin.br");
    if bin_path.exists() {
        return Ok(fs::read(&bin_path)?);
    }
    if br_path.exists() {
        let br = fs::read(&br_path)?;
        return map_file::decompress_map_payload(&br).map_err(|e| e.to_string().into());
    }
    Err("missing map.bin and map.bin.br".into())
}

fn write_placeholder_thumbnail(
    map_file: &MapFile,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    sow_map::write_map_thumbnail(map_file, path).map_err(|e| e.into())
}

pub fn refresh_catalog(maps_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut items = Vec::new();
    for entry in fs::read_dir(maps_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let key = entry.file_name().to_string_lossy().to_string();
        if key.starts_with('.') {
            continue;
        }
        let map_path = entry.path().join("map.bin");
        if !map_path.exists() {
            continue;
        }
        let bytes = fs::read(&map_path)?;
        let header = map_file::parse_header(&bytes)?;
        let frequency = fs::read_to_string(entry.path().join("info.toml"))
            .map(|blob| map_file::parse_frequency_toml(&blob))
            .unwrap_or(1);
        items.push((sow_core::maps::map_key(&key), header, frequency));
    }
    let catalog = map_file::catalog_from_headers(items);
    let catalog_bytes = map_file::encode_catalog(&catalog);
    let path = maps_root.join("catalog.bin");
    let temp = maps_root.join(format!(".catalog.bin.{}.tmp", std::process::id()));
    if let Err(error) = fs::write(&temp, catalog_bytes) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    if let Err(error) = fs::rename(&temp, &path) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(())
}
