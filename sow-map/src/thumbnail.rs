//! Build-time map thumbnails for lobby previews.

use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, RgbaImage};
use std::collections::VecDeque;
use std::path::Path;

use sow_core::map_file::MapFile;

/// Lobby / catalog preview edge length (keep small for WASM/catalog fetch).
pub const THUMBNAIL_SIZE: u32 = 512;
pub const THUMBNAIL_WIDTH: u32 = 512;
pub const THUMBNAIL_HEIGHT: u32 = 288;

/// A 16:9 source rectangle in authoring-image pixels.
///
/// Coordinates may extend beyond the source vertically or horizontally. The
/// renderer fills vertical overflow with ocean and wraps horizontal overflow,
/// which preserves a complete world map without cutting its antimeridian.
#[derive(Clone, Copy, Debug)]
pub struct SourceFrame {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

pub fn encode_square_thumbnail_webp(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let cropped = center_crop_square(img);
    let resized = cropped.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    encode_lossless_webp(&rgba)
}

pub fn write_square_thumbnail(img: &DynamicImage, path: &Path) -> Result<(), String> {
    let bytes = encode_square_thumbnail_webp(img)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// Generate the canonical lobby thumbnail from packed terrain data.
pub fn write_map_thumbnail(map: &MapFile, path: &Path) -> Result<(), String> {
    let preview = terrain_preview_image(map.width, map.height, &map.terrain);
    write_wide_thumbnail(&preview, path)
}

/// Generate a canonical 16:9 thumbnail from an authoring source image.
///
/// This is intentionally build-time only. The source image is never copied
/// into the web bundle or shipped to Android.
pub fn write_source_thumbnail(
    source_path: &Path,
    frame: SourceFrame,
    path: &Path,
) -> Result<(), String> {
    let source = image::open(source_path).map_err(|e| format!("open source image: {e}"))?;
    let rendered = render_source_image(&source)?;
    write_rendered_source_thumbnail(&rendered, frame, path)
}

/// Render a categorical authoring image once so multiple map frames can reuse it.
pub fn render_source_image(source: &DynamicImage) -> Result<RgbaImage, String> {
    render_source_image_impl(source)
}

pub fn render_source_file(source_path: &Path) -> Result<RgbaImage, String> {
    let source = image::open(source_path).map_err(|e| format!("open source image: {e}"))?;
    render_source_image(&source)
}

pub fn write_rendered_source_thumbnail(
    source: &RgbaImage,
    frame: SourceFrame,
    path: &Path,
) -> Result<(), String> {
    let framed = sample_source_frame(source, frame);
    write_wide_thumbnail(&DynamicImage::ImageRgba8(framed), path)
}

pub const BACKGROUND_RGB: [u8; 3] = [60, 60, 60];
pub const OCEAN_RGB: [u8; 3] = [71, 133, 181];
/// Shore water color: `round(0.7 * OCEAN_RGB + 76.5)` matching OpenFront `ColorUtils.ts`.
pub const SHORE_WATER_RGB: [u8; 3] = [126, 170, 203];
pub const SAND_RGB: [u8; 3] = [204, 203, 158];
pub const PLAINS_RGB: [u8; 3] = [190, 220, 138];
pub const HIGHLAND_RGB: [u8; 3] = [220, 203, 158];
pub const MOUNTAIN_RGB: [u8; 3] = [230, 230, 230];
const DEEP_OCEAN_RGBA: [u8; 4] = [OCEAN_RGB[0] - 10, OCEAN_RGB[1] - 10, OCEAN_RGB[2] - 10, 255];

/// Encode one packed `MapTile` byte into an sRGB RGBA8 pixel using OpenFront's `encodeTerrainTile` rules.
pub fn encode_terrain_tile(byte: u8) -> [u8; 4] {
    let is_land = (byte & 0x80) != 0;
    let is_shoreline = (byte & 0x40) != 0;
    let magnitude = byte & 0x1f;

    let [r, g, b] = if is_land && magnitude == 31 {
        BACKGROUND_RGB
    } else if is_land && is_shoreline {
        SAND_RGB
    } else if is_land {
        if magnitude < 10 {
            [
                PLAINS_RGB[0],
                PLAINS_RGB[1].saturating_sub(2 * magnitude),
                PLAINS_RGB[2],
            ]
        } else if magnitude < 20 {
            let m = magnitude - 10;
            [
                HIGHLAND_RGB[0].saturating_add(2 * m),
                HIGHLAND_RGB[1].saturating_add(2 * m),
                HIGHLAND_RGB[2].saturating_add(2 * m),
            ]
        } else {
            let m = magnitude / 2;
            [
                MOUNTAIN_RGB[0].saturating_add(m),
                MOUNTAIN_RGB[1].saturating_add(m),
                MOUNTAIN_RGB[2].saturating_add(m),
            ]
        }
    } else if is_shoreline {
        SHORE_WATER_RGB
    } else {
        let m = magnitude.min(10);
        [
            OCEAN_RGB[0].saturating_sub(m),
            OCEAN_RGB[1].saturating_sub(m),
            OCEAN_RGB[2].saturating_sub(m),
        ]
    };

    [r, g, b, 255]
}

/// Write a 16:9 thumbnail without cropping the input image.
pub fn write_wide_thumbnail(img: &DynamicImage, path: &Path) -> Result<(), String> {
    let background =
        RgbaImage::from_pixel(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, image::Rgba(DEEP_OCEAN_RGBA));
    let resized = img.resize(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, FilterType::Lanczos3);
    let mut canvas = DynamicImage::ImageRgba8(background).to_rgba8();
    let x = (THUMBNAIL_WIDTH.saturating_sub(resized.width())) / 2;
    let y = (THUMBNAIL_HEIGHT.saturating_sub(resized.height())) / 2;
    image::imageops::overlay(&mut canvas, &resized.to_rgba8(), x.into(), y.into());
    let bytes = encode_lossless_webp(&canvas)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn write_square_thumbnail_from_rgba(rgba: &RgbaImage, path: &Path) -> Result<(), String> {
    write_square_thumbnail(&DynamicImage::ImageRgba8(rgba.clone()), path)
}

pub fn write_square_thumbnail_from_pixels(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
    path: &Path,
) -> Result<(), String> {
    let rgba = RgbaImage::from_raw(
        width,
        height,
        pixels.iter().flat_map(|p| p.iter().copied()).collect(),
    )
    .ok_or_else(|| "thumbnail pixel buffer size mismatch".to_string())?;
    write_square_thumbnail_from_rgba(&rgba, path)
}

/// Render packed terrain bytes into an RGBA preview (downscaled if very large).
pub fn terrain_preview_image(width: u32, height: u32, terrain: &[u8]) -> DynamicImage {
    let max_dim = 2048u32;
    let longest = width.max(height).max(1);
    let scale = if longest > max_dim {
        max_dim as f64 / longest as f64
    } else {
        1.0
    };
    let pw = ((width as f64 * scale).ceil() as u32).max(1);
    let ph = ((height as f64 * scale).ceil() as u32).max(1);
    let mut rgba = vec![0u8; (pw * ph * 4) as usize];

    for py in 0..ph {
        for px in 0..pw {
            let sx = ((px as f64 / scale).floor() as u32).min(width.saturating_sub(1));
            let sy = ((py as f64 / scale).floor() as u32).min(height.saturating_sub(1));
            let idx = (sy * width + sx) as usize;
            let color = terrain
                .get(idx)
                .copied()
                .map(encode_terrain_tile)
                .unwrap_or(DEEP_OCEAN_RGBA);
            let o = ((py * pw + px) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&color);
        }
    }

    DynamicImage::ImageRgba8(
        RgbaImage::from_raw(pw, ph, rgba).expect("terrain preview buffer size"),
    )
}

fn center_crop_square(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    img.crop_imm(x, y, side, side)
}

fn render_source_image_impl(source: &DynamicImage) -> Result<RgbaImage, String> {
    let source = source.to_rgba8();
    let (width, height) = source.dimensions();
    let count = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| "source image is too large".to_string())?;
    let mut land = vec![false; count];
    let mut magnitude = vec![0u8; count];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let px = source.get_pixel(x, y).0;
            land[idx] = px[3] >= 20 && px[2] != 106;
            magnitude[idx] = (px[2].clamp(140, 200) - 140) / 2;
        }
    }
    Ok(render_categorical(width, height, &land, &magnitude))
}

/// Render generated map bytes with the same palette as `render_source_image`,
/// so a thumbnail shows the water the map really has. Sampled to at most
/// twice the thumbnail width, which is all the 16:9 thumbnail can show.
pub fn render_terrain_image(width: u32, height: u32, terrain: &[u8]) -> RgbaImage {
    let scale = (f64::from(THUMBNAIL_WIDTH * 2) / f64::from(width.max(height).max(1))).min(1.0);
    let out_w = ((f64::from(width) * scale).round() as u32).max(1);
    let out_h = ((f64::from(height) * scale).round() as u32).max(1);
    let count = (out_w * out_h) as usize;
    let mut land = Vec::with_capacity(count);
    let mut magnitude = Vec::with_capacity(count);
    for y in 0..out_h {
        let sy = ((f64::from(y) / scale) as u32).min(height.saturating_sub(1));
        for x in 0..out_w {
            let sx = ((f64::from(x) / scale) as u32).min(width.saturating_sub(1));
            let byte = terrain.get((sy * width + sx) as usize).copied().unwrap_or(0);
            let is_land = byte & 0x80 != 0;
            land.push(is_land);
            magnitude.push(if is_land { byte & 0x1f } else { 0 });
        }
    }
    render_categorical(out_w, out_h, &land, &magnitude)
}

fn render_categorical(width: u32, height: u32, land: &[bool], magnitude: &[u8]) -> RgbaImage {
    let count = land.len();
    // The source encodes only land/elevation. A bounded BFS adds the same
    // shallow-water band used by the accepted visual proof without inventing
    // a second map palette or changing map.bin.
    let mut water_distance = vec![u8::MAX; count];
    let mut queue = VecDeque::new();
    for (idx, &is_land) in land.iter().enumerate() {
        if is_land {
            queue.push_back(idx);
        }
    }
    while let Some(idx) = queue.pop_front() {
        let x = idx % width as usize;
        let y = idx / width as usize;
        let next_distance = if land[idx] {
            0
        } else {
            water_distance[idx].saturating_add(1)
        };
        if next_distance > 20 {
            continue;
        }
        visit_neighbors(x, y, width as usize, height as usize, |nx, ny| {
            let nidx = ny * width as usize + nx;
            if !land[nidx] && water_distance[nidx] == u8::MAX {
                water_distance[nidx] = next_distance;
                queue.push_back(nidx);
            }
        });
    }

    let mut out = RgbaImage::from_pixel(width, height, image::Rgba(DEEP_OCEAN_RGBA));
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let tile_byte = if land[idx] {
                let mut shoreline = false;
                visit_neighbors(
                    x as usize,
                    y as usize,
                    width as usize,
                    height as usize,
                    |nx, ny| {
                        shoreline |= !land[ny * width as usize + nx];
                    },
                );
                let mag = magnitude[idx] & 0x1f;
                0x80 | if shoreline && mag != 31 { 0x40 } else { 0 } | mag
            } else if water_distance[idx] == 0 {
                0x40
            } else if water_distance[idx] <= 20 {
                ((water_distance[idx] + 1) / 2).min(10)
            } else {
                10
            };
            out.put_pixel(x, y, image::Rgba(encode_terrain_tile(tile_byte)));
        }
    }
    out
}

fn sample_source_frame(source: &RgbaImage, frame: SourceFrame) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(frame.width, frame.height, image::Rgba(DEEP_OCEAN_RGBA));
    let source_width = source.width() as i64;
    let source_height = source.height() as i64;
    for y in 0..frame.height {
        let sy = frame.y + y as i64;
        if !(0..source_height).contains(&sy) {
            continue;
        }
        for x in 0..frame.width {
            let sx = (frame.x + x as i64).rem_euclid(source_width);
            out.put_pixel(x, y, *source.get_pixel(sx as u32, sy as u32));
        }
    }
    out
}

fn visit_neighbors(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    mut visit: impl FnMut(usize, usize),
) {
    if x > 0 {
        visit(x - 1, y);
    }
    if x + 1 < width {
        visit(x + 1, y);
    }
    if y > 0 {
        visit(x, y - 1);
    }
    if y + 1 < height {
        visit(x, y + 1);
    }
}

fn encode_lossless_webp(rgba: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out)
        .encode(rgba, rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        .map_err(|e| format!("webp encode: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[test]
    fn square_thumbnail_is_512_and_compact() {
        let img = DynamicImage::new_rgba8(2800, 1448);
        let bytes = encode_square_thumbnail_webp(&img).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.dimensions(), (THUMBNAIL_SIZE, THUMBNAIL_SIZE));
        // Lossless 512² should stay well under legacy 1MB 1024² blobs.
        assert!(
            bytes.len() < 400_000,
            "thumbnail too large: {} bytes",
            bytes.len()
        );
    }

    #[test]
    fn wide_thumbnail_is_512_by_288_without_crop() {
        let img = DynamicImage::new_rgba8(2800, 1448);
        let path =
            std::env::temp_dir().join(format!("sow-thumbnail-test-{}.webp", std::process::id()));
        write_wide_thumbnail(&img, &path).unwrap();
        let decoded = image::open(&path).unwrap();
        assert_eq!(decoded.dimensions(), (THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT));
        let _ = std::fs::remove_file(path);
    }
}
