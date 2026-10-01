use crate::emoji_atlas::{encode_webp, pack_grid};
use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const CELL_PX: u32 = 64;
const GUTTER_PX: u32 = 2;
const ART_MARGIN_PX: u32 = 2;
const SOURCES: [(&str, u8, &str); 3] = [
    ("Farm", 1, "farm-level-1.png"),
    ("Farm", 2, "farm-level-2.png"),
    ("Farm", 3, "farm-level-3.png"),
];

pub struct PackBuildingAtlasArgs {
    pub source_root: PathBuf,
    pub asset_root: PathBuf,
    pub rust_manifest: PathBuf,
}

pub fn pack(args: PackBuildingAtlasArgs) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut entries = Vec::with_capacity(SOURCES.len());
    let mut metadata = Vec::with_capacity(SOURCES.len());
    for (kind, level, filename) in SOURCES {
        let source = args.source_root.join(filename);
        let bytes = fs::read(&source)?;
        let image = image::load_from_memory(&bytes)?.to_rgba8();
        entries.push((format!("{kind}:{level}"), pixel_cell(&image)?));
        metadata.push((kind, level, filename, sha256(&bytes)));
    }

    let packed = pack_grid(&entries, CELL_PX, GUTTER_PX)?;
    let atlas_path = args.asset_root.join("atlas.webp");
    write_webp_if_changed(&packed.atlas, &atlas_path)?;

    let mut sprites = serde_json::Map::new();
    for ((kind, level, filename, digest), rect) in metadata.iter().zip(&packed.rects) {
        if rect.name != format!("{kind}:{level}")
            || rect.w != CELL_PX
            || rect.h != CELL_PX
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
        "atlas": "gameplay/buildings/atlas.webp",
        "atlas_width": packed.atlas.width(),
        "atlas_height": packed.atlas.height(),
        "cell_size": CELL_PX,
        "sprites": sprites,
    });
    write_if_changed(
        &args.asset_root.join("atlas.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?).as_bytes(),
    )?;
    write_if_changed(
        &args.rust_manifest,
        rust_registry(&packed.rects, packed.atlas.width(), packed.atlas.height())?.as_bytes(),
    )?;

    println!(
        "✅ Building atlas ready: {} ({}x{}, {} pixel-art levels)",
        atlas_path.display(),
        packed.atlas.width(),
        packed.atlas.height(),
        SOURCES.len()
    );
    Ok(())
}

fn pixel_cell(source: &RgbaImage) -> Result<RgbaImage, Box<dyn Error + Send + Sync>> {
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
        return Err("building source image is fully transparent".into());
    }

    let cropped = image::imageops::crop_imm(
        source,
        bounds.0,
        bounds.1,
        bounds.2 - bounds.0,
        bounds.3 - bounds.1,
    )
    .to_image();
    let max_side = CELL_PX - GUTTER_PX * 2 - ART_MARGIN_PX * 2;
    let scale = (max_side as f32 / cropped.width() as f32)
        .min(max_side as f32 / cropped.height() as f32);
    let width = ((cropped.width() as f32 * scale).round() as u32).clamp(1, max_side);
    let height = ((cropped.height() as f32 * scale).round() as u32).clamp(1, max_side);
    let resized = image::imageops::resize(&cropped, width, height, FilterType::Lanczos3);

    let mut cell = ImageBuffer::from_pixel(CELL_PX, CELL_PX, Rgba([0, 0, 0, 0]));
    let x = (CELL_PX - width) / 2;
    let y = (CELL_PX - height) / 2;
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
    rects: &[crate::emoji_atlas::PackedGlyph],
    atlas_width: u32,
    atlas_height: u32,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let slots = rects
        .iter()
        .map(|rect| -> Result<String, Box<dyn Error + Send + Sync>> {
            let Some((kind, raw_level)) = rect.name.split_once(':') else {
                return Err("building atlas key must contain kind:level".into());
            };
            if !matches!(kind, "City" | "Bunker" | "Factory" | "Port" | "Farm") {
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
