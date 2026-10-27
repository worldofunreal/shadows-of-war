use image::ImageEncoder;
use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const BUILDING_CELL_PX: u32 = 128;
const UNIT_CELL_PX: u32 = 64;
const BADGE_CELL_PX: u32 = 128;
const GUTTER_PX: u32 = 2;
const ART_MARGIN_PX: u32 = 2;
/// Every building and level needs art: `assets/art/buildings/<kind>-<level>.png`.
const BUILDING_KINDS: [&str; 5] = ["City", "Factory", "Port", "Tower", "Farm"];
const BUILDING_LEVELS: u8 = 4;
/// Nameplate and HUD badges, one strip in this order: `assets/art/icons/<name>.png`.
/// The cell order is read by sow-render (`badge_atlas::CELLS`) and the HUD (`STATUS_CELLS`).
// ponytail: one 128px strip, 27 cells = 3456px; switch to a grid before it passes 4096px.
pub const BADGES: [&str; 27] = [
    "request",
    "allied",
    "traitor",
    "rank-1",
    "rank-2",
    "rank-3",
    "you",
    "disconnected",
    "upgrade-sparkle",
    "wilderness",
    "troops",
    "death",
    "radiation",
    "construction",
    "hammer",
    "arrow-up",
    "arrow-right",
    "arrow-down",
    "arrow-left",
    "cancel",
    "confirm",
    "gold",
    "boat",
    "clock",
    "shield",
    "range",
    "speed",
];
/// Map movers, `assets/art/units/<name>.png`, packed 4x2 in sow-render
/// `MoverSpriteId` order (embedded in the game, never published).
const UNITS: [&str; 5] = [
    "transport-ship",
    "trade-ship",
    "warship",
    "atom-bomb",
    "sam-missile",
];
const UNIT_COLUMNS: u32 = 4;
/// Runtime URLs the pipeline publishes the generated atlases under.
const BUILDING_ATLAS_URL: &str = "gameplay/buildings/atlas.webp";
const BADGE_ATLAS_URL: &str = "gameplay/icons/nameplate_status_atlas.webp";

pub struct PackBuildingAtlasArgs {
    pub art_root: PathBuf,
    pub generated_root: PathBuf,
    pub rust_manifest: PathBuf,
    pub badge_manifest: PathBuf,
}

fn art_source(dir: &Path, stem: &str) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    ["png", "webp"]
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|path| path.is_file())
        .ok_or_else(|| format!("missing art: {}/{stem}.png", dir.display()).into())
}

pub fn pack(args: PackBuildingAtlasArgs) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut entries = Vec::new();
    let mut metadata = Vec::new();
    for kind in BUILDING_KINDS {
        for level in 1..=BUILDING_LEVELS {
            let source = art_source(
                &args.art_root.join("buildings"),
                &format!("{}-{level}", kind.to_lowercase()),
            )?;
            let bytes = fs::read(&source)?;
            let image = image::load_from_memory(&bytes)?.to_rgba8();
            entries.push((
                format!("{kind}:{level}"),
                pixel_cell(&image, BUILDING_CELL_PX)?,
            ));
            let filename = source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            metadata.push((kind, level, filename, sha256(&bytes)));
        }
    }

    let packed = pack_grid(&entries, BUILDING_CELL_PX, GUTTER_PX)?;
    let atlas_path = args.generated_root.join("buildings-atlas.webp");
    write_webp_if_changed(&packed.atlas, &atlas_path)?;

    let mut sprites = serde_json::Map::new();
    for ((kind, level, filename, digest), rect) in metadata.iter().zip(&packed.rects) {
        if rect.name != format!("{kind}:{level}")
            || rect.w != BUILDING_CELL_PX
            || rect.h != BUILDING_CELL_PX
            || rect.x + rect.w > packed.atlas.width()
            || rect.y + rect.h > packed.atlas.height()
        {
            return Err(format!("invalid building atlas slot for {kind} level {level}").into());
        }
        let levels = sprites
            .entry((*kind).to_string())
            .or_insert_with(|| json!({}));
        let Some(levels) = levels.as_object_mut() else {
            return Err("building sprite levels are not an object".into());
        };
        levels.insert(
            level.to_string(),
            json!({
                "x": rect.x,
                "y": rect.y,
                "width": rect.w,
                "height": rect.h,
                "source": filename,
                "sha256": digest,
            }),
        );
    }

    let manifest = json!({
        "schema_version": 1,
        "atlas": BUILDING_ATLAS_URL,
        "atlas_width": packed.atlas.width(),
        "atlas_height": packed.atlas.height(),
        "cell_size": BUILDING_CELL_PX,
        "sprites": sprites,
    });
    write_if_changed(
        &args.generated_root.join("buildings-atlas.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?).as_bytes(),
    )?;
    write_if_changed(
        &args.rust_manifest,
        rust_registry(&packed.rects, packed.atlas.width(), packed.atlas.height())?.as_bytes(),
    )?;

    pack_badges(&args)?;
    pack_units(&args)?;
    println!(
        "✅ Atlases ready: {} ({}x{}, {} building levels), {} badges and {} units",
        atlas_path.display(),
        packed.atlas.width(),
        packed.atlas.height(),
        entries.len(),
        BADGES.len(),
        UNITS.len(),
    );
    Ok(())
}

fn pack_units(args: &PackBuildingAtlasArgs) -> Result<(), Box<dyn Error + Send + Sync>> {
    let rows = (UNITS.len() as u32).div_ceil(UNIT_COLUMNS);
    let mut grid = ImageBuffer::from_pixel(
        UNIT_CELL_PX * UNIT_COLUMNS,
        UNIT_CELL_PX * rows,
        Rgba([0, 0, 0, 0]),
    );
    for (index, name) in UNITS.iter().enumerate() {
        let source = art_source(&args.art_root.join("units"), name)?;
        let image = image::load_from_memory(&fs::read(&source)?)?.to_rgba8();
        let index = index as u32;
        image::imageops::overlay(
            &mut grid,
            &pixel_cell(&image, UNIT_CELL_PX)?,
            i64::from(index % UNIT_COLUMNS * UNIT_CELL_PX),
            i64::from(index / UNIT_COLUMNS * UNIT_CELL_PX),
        );
    }
    write_webp_if_changed(&grid, &args.generated_root.join("units-atlas.webp"))
}

/// One horizontal strip so the HUD can address a badge with a single CSS
/// background-position and the renderer with `index / count`.
fn pack_badges(args: &PackBuildingAtlasArgs) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut strip = ImageBuffer::from_pixel(
        BADGE_CELL_PX * BADGES.len() as u32,
        BADGE_CELL_PX,
        Rgba([0, 0, 0, 0]),
    );
    for (index, name) in BADGES.iter().enumerate() {
        let source = art_source(&args.art_root.join("icons"), name)?;
        let image = image::load_from_memory(&fs::read(&source)?)?.to_rgba8();
        let cell = pixel_cell(&image, BADGE_CELL_PX)?;
        image::imageops::overlay(
            &mut strip,
            &cell,
            i64::from(BADGE_CELL_PX) * index as i64,
            0,
        );
    }
    write_webp_if_changed(&strip, &args.generated_root.join("badges-atlas.webp"))?;
    let manifest = json!({
        "schema_version": 1,
        "atlas": BADGE_ATLAS_URL,
        "cell_size": BADGE_CELL_PX,
        "cells": BADGES,
    });
    write_if_changed(
        &args.generated_root.join("badges-atlas.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?).as_bytes(),
    )?;
    let cells = BADGES
        .iter()
        .map(|name| format!("    \"{name}\",\n"))
        .collect::<String>();
    write_if_changed(
        &args.badge_manifest,
        format!(
            "// @generated by sow-tools pack-building-atlas; do not edit.\n\
             pub(crate) const CELLS: [&str; {}] = [\n{cells}];\n",
            BADGES.len()
        )
        .as_bytes(),
    )?;
    Ok(())
}

fn pixel_cell(source: &RgbaImage, cell_px: u32) -> Result<RgbaImage, Box<dyn Error + Send + Sync>> {
    let mut bounds = (source.width(), source.height(), 0, 0);
    let mut found = false;
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel.0[3] <= 8 {
            continue;
        }
        found = true;
        bounds.0 = bounds.0.min(x);
        bounds.1 = bounds.1.min(y);
        bounds.2 = bounds.2.max(x + 1);
        bounds.3 = bounds.3.max(y + 1);
    }
    if !found {
        return Err("art source image is fully transparent".into());
    }

    let cropped = image::imageops::crop_imm(
        source,
        bounds.0,
        bounds.1,
        bounds.2 - bounds.0,
        bounds.3 - bounds.1,
    )
    .to_image();
    let max_side = cell_px - GUTTER_PX * 2 - ART_MARGIN_PX * 2;
    let scale =
        (max_side as f32 / cropped.width() as f32).min(max_side as f32 / cropped.height() as f32);
    let width = ((cropped.width() as f32 * scale).round() as u32).clamp(1, max_side);
    let height = ((cropped.height() as f32 * scale).round() as u32).clamp(1, max_side);
    let resized = image::imageops::resize(&cropped, width, height, FilterType::Lanczos3);

    let mut cell = ImageBuffer::from_pixel(cell_px, cell_px, Rgba([0, 0, 0, 0]));
    let x = (cell_px - width) / 2;
    let y = (cell_px - height) / 2;
    image::imageops::overlay(&mut cell, &resized, x.into(), y.into());
    Ok(cell)
}

fn write_webp_if_changed(
    atlas: &RgbaImage,
    path: &Path,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    write_if_changed(path, &encode_webp(atlas)?)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
    if fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn rust_registry(
    rects: &[PackedGlyph],
    atlas_width: u32,
    atlas_height: u32,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let slots = rects
        .iter()
        .map(|rect| -> Result<String, Box<dyn Error + Send + Sync>> {
            let Some((kind, raw_level)) = rect.name.split_once(':') else {
                return Err("building atlas key must contain kind:level".into());
            };
            if !matches!(kind, "City" | "Tower" | "Factory" | "Port" | "Farm") {
                return Err(format!("unknown building kind in atlas: {kind}").into());
            }
            let level: u8 = raw_level.parse()?;
            Ok(format!(
                "        (sow_core::game::BuildingKind::{kind}, {level}) => Some([{}, {}, {}, {}]),\n",
                rect.x, rect.y, rect.w, rect.h
            ))
    })
        .collect::<Result<String, _>>()?;
    Ok(format!(
        concat!(
            "// @generated by sow-tools pack-building-atlas; do not edit.\n",
            "pub(crate) const ATLAS_WIDTH: u32 = {atlas_width};\n",
            "pub(crate) const ATLAS_HEIGHT: u32 = {atlas_height};\n",
            "pub(crate) fn uv_rect(kind: sow_core::game::BuildingKind, level: u8) -> Option<[f32; 4]> {{\n",
            "    let Some([x, y, width, height]) = (match (kind, level.max(1)) {{\n",
            "{slots}",
            "        _ => None,\n",
            "    }}) else {{\n",
            "        return None;\n",
            "    }};\n",
            "    Some([\n",
            "        x as f32 / ATLAS_WIDTH as f32,\n",
            "        y as f32 / ATLAS_HEIGHT as f32,\n",
            "        (x + width) as f32 / ATLAS_WIDTH as f32,\n",
            "        (y + height) as f32 / ATLAS_HEIGHT as f32,\n",
            "    ])\n",
            "}}\n"
        ),
        atlas_width = atlas_width,
        atlas_height = atlas_height,
        slots = slots
    ))
}

pub(crate) struct PackedGlyph {
    pub(crate) name: String,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

pub(crate) struct PackAtlasResult {
    pub(crate) atlas: RgbaImage,
    pub(crate) rects: Vec<PackedGlyph>,
}

pub(crate) fn pack_grid(
    entries: &[(String, RgbaImage)],
    cell: u32,
    gutter: u32,
) -> Result<PackAtlasResult, Box<dyn std::error::Error + Send + Sync>> {
    let n = entries.len() as u32;
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = n.div_ceil(cols);
    let tile = cell + gutter * 2;
    let w = cols * tile;
    let h = rows * tile;
    let mut atlas: RgbaImage = ImageBuffer::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    let mut rects = Vec::new();
    for (i, (name, img)) in entries.iter().enumerate() {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        let x = col * tile + gutter;
        let y = row * tile + gutter;
        image::imageops::overlay(&mut atlas, img, x.into(), y.into());
        rects.push(PackedGlyph {
            name: name.clone(),
            x,
            y,
            w: cell,
            h: cell,
        });
    }
    Ok(PackAtlasResult { atlas, rects })
}

pub(crate) fn encode_webp(
    atlas: &RgbaImage,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut bytes = Vec::new();
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut bytes);
    encoder.write_image(
        atlas.as_raw(),
        atlas.width(),
        atlas.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}
