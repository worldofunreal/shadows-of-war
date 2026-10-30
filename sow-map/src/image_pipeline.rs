//! Categorical image pipeline: classify encoded RGBA, cleanup, downscale, ocean/shore, pack.
//!
//! Same algorithm as Go `map_generator.go` and `sow-tools image-map`: water when
//! `alpha < 20` or `blue == 106`; land elevation from blue channel 140..200.

use image::RgbaImage;
use sow_core::maps;
use std::collections::VecDeque;

/// Per-map water controls. `Default` reproduces the canonical recipes in assets/maps/SOURCES.toml.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterSettings {
    /// Inland water bodies smaller than this many map tiles become land.
    pub min_lake_tiles: usize,
    /// Land masses smaller than this many source pixels become water.
    pub min_island_pixels: usize,
    /// When the source is enlarged, trace smooth coasts instead of square steps.
    pub smooth_coasts: bool,
    /// With smooth coasts: 0.5 follows the source; higher widens rivers, lakes and bays.
    pub water_level: f32,
    /// Open a tile where two water tiles touch only at a corner, so rivers stay navigable.
    pub connect_diagonals: bool,
}

impl Default for WaterSettings {
    fn default() -> Self {
        Self {
            min_lake_tiles: 16,
            min_island_pixels: 30,
            smooth_coasts: false,
            water_level: 0.5,
            connect_diagonals: false,
        }
    }
}

const LAND_BIT: u8 = 0b1000_0000;
const SHORE_BIT: u8 = 0b0100_0000;
const OCEAN_BIT: u8 = 0b0010_0000;

#[derive(Clone)]
struct Terrain {
    width: usize,
    height: usize,
    is_land: Vec<bool>,
    is_ocean: Vec<bool>,
    is_shore: Vec<bool>,
    /// Land: elevation 0..=30. Water: Manhattan distance to land.
    magnitude: Vec<f32>,
}

impl Terrain {
    fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            width,
            height,
            is_land: vec![false; n],
            is_ocean: vec![false; n],
            is_shore: vec![false; n],
            magnitude: vec![0.0; n],
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

pub struct ImagePipelineResult {
    pub width: u32,
    pub height: u32,
    pub map_data: Vec<u8>,
    pub num_land_tiles: u32,
}

/// Run the image pipeline on source-encoded RGBA pixels.
///
/// When `target_dims` is `Some((w, h))`, downscale to that size (editor OSM path).
/// When `None`, downscale with `mobile_safe_dims` preserving aspect (CLI image-map path).
pub fn generate_from_rgba(
    img: &RgbaImage,
    target_dims: Option<(u32, u32)>,
) -> Result<ImagePipelineResult, String> {
    generate_with_water(img, target_dims, &WaterSettings::default())
}

/// `generate_from_rgba` with per-map water controls (map generator path).
pub fn generate_with_water(
    img: &RgbaImage,
    target_dims: Option<(u32, u32)>,
    water: &WaterSettings,
) -> Result<ImagePipelineResult, String> {
    let src_w = img.width() as usize;
    let src_h = img.height() as usize;
    if src_w == 0 || src_h == 0 {
        return Err("empty image".into());
    }

    log::info!("image_pipeline: source {src_w}x{src_h}");

    let mut full = Terrain::new(src_w, src_h);
    classify_rgba_into(img, &mut full);

    remove_small_islands(&mut full, water.min_island_pixels);

    let (dst_w, dst_h) = match target_dims {
        Some((w, h)) => {
            let w = maps::align_map_dim(w.max(4));
            let h = maps::align_map_dim(h.max(4));
            (w, h)
        }
        None => mobile_safe_dims(src_w as u32, src_h as u32),
    };

    log::info!(
        "image_pipeline: downscale to {dst_w}x{dst_h} ({} pixels)",
        dst_w as u64 * dst_h as u64
    );

    let enlarging = dst_w as usize > src_w || dst_h as usize > src_h;
    let mut small = if water.smooth_coasts && enlarging {
        upscale_smooth(&full, dst_w as usize, dst_h as usize, water.water_level)
    } else {
        downscale(&full, dst_w as usize, dst_h as usize)
    };
    if water.connect_diagonals {
        connect_diagonal_water(&mut small);
    }
    remove_small_lakes(&mut small, water.min_lake_tiles);
    mark_ocean(&mut small);
    let shoreline_waters = process_shore(&mut small);
    process_dist_to_land(&mut small, &shoreline_waters);

    let (map_data, num_land_tiles) = pack(&small);
    Ok(ImagePipelineResult {
        width: dst_w,
        height: dst_h,
        map_data,
        num_land_tiles,
    })
}

fn classify_rgba_into(img: &RgbaImage, terrain: &mut Terrain) {
    for y in 0..terrain.height {
        for x in 0..terrain.width {
            let px = img.get_pixel(x as u32, y as u32).0;
            let blue = px[2];
            let alpha = px[3];
            let i = terrain.idx(x, y);
            if alpha < 20 || blue == 106 {
                terrain.is_land[i] = false;
            } else {
                terrain.is_land[i] = true;
                terrain.magnitude[i] = (blue.clamp(140, 200) as f32 - 140.0) / 2.0;
            }
        }
    }
}

pub fn mobile_safe_dims(src_w: u32, src_h: u32) -> (u32, u32) {
    let w = src_w.max(1) as f64;
    let h = src_h.max(1) as f64;
    let pixel_factor = (maps::MOBILE_SAFE_MAX_PIXELS as f64 / (w * h)).sqrt();
    let axis_factor = (maps::MOBILE_SAFE_MAX_AXIS as f64 / w).min(maps::MOBILE_SAFE_MAX_AXIS as f64 / h);
    let factor = pixel_factor.min(axis_factor).min(1.0);
    let dst_w = maps::align_map_dim((w * factor).floor() as u32);
    let dst_h = maps::align_map_dim((h * factor).floor() as u32);
    (dst_w, dst_h)
}

fn neighbors(x: usize, y: usize, w: usize, h: usize) -> impl Iterator<Item = (usize, usize)> {
    [
        (x > 0).then(|| (x - 1, y)),
        (x + 1 < w).then_some((x + 1, y)),
        (y > 0).then(|| (x, y - 1)),
        (y + 1 < h).then_some((x, y + 1)),
    ]
    .into_iter()
    .flatten()
}

fn flood_region(t: &Terrain, start: usize, want_land: bool, visited: &mut [bool]) -> Vec<usize> {
    let mut region = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited[start] = true;
    while let Some(cur) = queue.pop_front() {
        if t.is_land[cur] != want_land {
            continue;
        }
        region.push(cur);
        let x = cur % t.width;
        let y = cur / t.width;
        for (nx, ny) in neighbors(x, y, t.width, t.height) {
            let ni = ny * t.width + nx;
            if !visited[ni] && t.is_land[ni] == want_land {
                visited[ni] = true;
                queue.push_back(ni);
            }
        }
    }
    region
}

fn remove_small_islands(t: &mut Terrain, min_size: usize) {
    let mut visited = vec![false; t.is_land.len()];
    let mut removed = 0usize;
    for start in 0..t.is_land.len() {
        if visited[start] || !t.is_land[start] {
            continue;
        }
        let region = flood_region(t, start, true, &mut visited);
        if region.len() < min_size {
            removed += 1;
            for &i in &region {
                t.is_land[i] = false;
                t.magnitude[i] = 0.0;
            }
        }
    }
    log::info!("Removed {removed} islands smaller than {min_size} tiles");
}

fn downscale(src: &Terrain, dst_w: usize, dst_h: usize) -> Terrain {
    let mut dst = Terrain::new(dst_w, dst_h);
    let fx = src.width as f64 / dst_w as f64;
    let fy = src.height as f64 / dst_h as f64;
    for ty in 0..dst_h {
        let y0 = (ty as f64 * fy).floor() as usize;
        let y1 = (((ty + 1) as f64 * fy).ceil() as usize)
            .min(src.height)
            .max(y0 + 1);
        for tx in 0..dst_w {
            let x0 = (tx as f64 * fx).floor() as usize;
            let x1 = (((tx + 1) as f64 * fx).ceil() as usize)
                .min(src.width)
                .max(x0 + 1);
            let mut land_count = 0usize;
            let mut total = 0usize;
            let mut mag_sum = 0.0f32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let si = sy * src.width + sx;
                    total += 1;
                    if src.is_land[si] {
                        land_count += 1;
                        mag_sum += src.magnitude[si];
                    }
                }
            }
            let di = ty * dst_w + tx;
            if land_count == total {
                dst.is_land[di] = true;
                dst.magnitude[di] = mag_sum / land_count as f32;
            }
        }
    }
    dst
}

/// Bilinear land coverage per tile, cut at `water_level`: smooth coasts when the source is enlarged.
fn upscale_smooth(src: &Terrain, dst_w: usize, dst_h: usize, water_level: f32) -> Terrain {
    let mut dst = Terrain::new(dst_w, dst_h);
    let fx = src.width as f32 / dst_w as f32;
    let fy = src.height as f32 / dst_h as f32;
    let max_x = src.width as f32 - 1.0;
    let max_y = src.height as f32 - 1.0;
    for ty in 0..dst_h {
        let sy = ((ty as f32 + 0.5) * fy - 0.5).clamp(0.0, max_y);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(src.height - 1);
        let wy = sy - y0 as f32;
        for tx in 0..dst_w {
            let sx = ((tx as f32 + 0.5) * fx - 0.5).clamp(0.0, max_x);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(src.width - 1);
            let wx = sx - x0 as f32;
            let corners = [
                (y0 * src.width + x0, (1.0 - wx) * (1.0 - wy)),
                (y0 * src.width + x1, wx * (1.0 - wy)),
                (y1 * src.width + x0, (1.0 - wx) * wy),
                (y1 * src.width + x1, wx * wy),
            ];
            let mut land = 0.0f32;
            let mut mag_sum = 0.0f32;
            for (si, weight) in corners {
                if src.is_land[si] {
                    land += weight;
                    mag_sum += weight * src.magnitude[si];
                }
            }
            if land > water_level {
                let di = ty * dst_w + tx;
                dst.is_land[di] = true;
                dst.magnitude[di] = mag_sum / land;
            }
        }
    }
    dst
}

/// Where two water tiles meet only at a corner, flood one of the land tiles between them.
fn connect_diagonal_water(t: &mut Terrain) {
    let mut opened = 0usize;
    for y in 0..t.height.saturating_sub(1) {
        for x in 0..t.width.saturating_sub(1) {
            let a = t.idx(x, y);
            let b = a + 1;
            let c = a + t.width;
            let d = c + 1;
            let land = &t.is_land;
            let open = if !land[a] && !land[d] && land[b] && land[c] {
                Some(b)
            } else if !land[b] && !land[c] && land[a] && land[d] {
                Some(a)
            } else {
                None
            };
            if let Some(i) = open {
                t.is_land[i] = false;
                t.magnitude[i] = 0.0;
                opened += 1;
            }
        }
    }
    log::info!("Opened {opened} diagonal water links");
}

fn remove_small_lakes(t: &mut Terrain, min_size: usize) {
    let mut visited = vec![false; t.is_land.len()];
    let mut removed = 0usize;
    for start in 0..t.is_land.len() {
        if visited[start] || t.is_land[start] {
            continue;
        }
        let region = flood_region(t, start, false, &mut visited);
        if region.len() >= min_size {
            continue;
        }
        removed += 1;
        for &i in &region {
            t.is_land[i] = true;
            t.magnitude[i] = 0.0;
        }
    }
    log::info!("Removed {removed} inland water bodies smaller than {min_size} tiles");
}

fn mark_ocean(t: &mut Terrain) {
    let mut visited = vec![false; t.is_land.len()];
    let mut largest: Vec<usize> = Vec::new();
    for start in 0..t.is_land.len() {
        if visited[start] || t.is_land[start] {
            continue;
        }
        let region = flood_region(t, start, false, &mut visited);
        if region.len() > largest.len() {
            largest = region;
        }
    }
    for &i in &largest {
        t.is_ocean[i] = true;
    }
    log::info!("Marked ocean with {} water tiles", largest.len());
}

fn process_shore(t: &mut Terrain) -> Vec<usize> {
    let mut shoreline_waters = Vec::new();
    for y in 0..t.height {
        for x in 0..t.width {
            let i = t.idx(x, y);
            let mut shore = false;
            for (nx, ny) in neighbors(x, y, t.width, t.height) {
                let ni = ny * t.width + nx;
                if t.is_land[ni] != t.is_land[i] {
                    shore = true;
                    break;
                }
            }
            if shore {
                t.is_shore[i] = true;
                if !t.is_land[i] {
                    shoreline_waters.push(i);
                }
            }
        }
    }
    shoreline_waters
}

fn process_dist_to_land(t: &mut Terrain, shoreline_waters: &[usize]) {
    let mut visited = vec![false; t.is_land.len()];
    let mut queue: VecDeque<(usize, u32)> = VecDeque::new();
    for &i in shoreline_waters {
        visited[i] = true;
        t.magnitude[i] = 0.0;
        queue.push_back((i, 0));
    }
    while let Some((cur, dist)) = queue.pop_front() {
        let x = cur % t.width;
        let y = cur / t.width;
        for (nx, ny) in neighbors(x, y, t.width, t.height) {
            let ni = ny * t.width + nx;
            if !visited[ni] && !t.is_land[ni] {
                visited[ni] = true;
                t.magnitude[ni] = (dist + 1) as f32;
                queue.push_back((ni, dist + 1));
            }
        }
    }
}

fn pack(t: &Terrain) -> (Vec<u8>, u32) {
    let mut data = Vec::with_capacity(t.is_land.len());
    let mut num_land = 0u32;
    for i in 0..t.is_land.len() {
        let mut byte = 0u8;
        if t.is_land[i] {
            byte |= LAND_BIT;
            num_land += 1;
        }
        if t.is_shore[i] {
            byte |= SHORE_BIT;
        }
        if t.is_ocean[i] {
            byte |= OCEAN_BIT;
        }
        let mag = if t.is_land[i] {
            t.magnitude[i].ceil().min(31.0)
        } else {
            (t.magnitude[i] / 2.0).ceil().min(31.0)
        };
        byte |= mag as u8 & 0b0001_1111;
        data.push(byte);
    }
    (data, num_land)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn encoded_coast(w: u32, h: u32) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let px = if x < w / 2 {
                    Rgba([0, 0, 106, 255])
                } else {
                    Rgba([0, 0, 140, 255])
                };
                img.put_pixel(x, y, px);
            }
        }
        img
    }

    #[test]
    fn coast_produces_ocean_and_shore() {
        let img = encoded_coast(40, 40);
        let result = generate_from_rgba(&img, Some((20, 20))).expect("pipeline");
        assert_eq!(result.width, 20);
        assert!(result.num_land_tiles > 0);
        let ocean = result
            .map_data
            .iter()
            .filter(|b| **b & OCEAN_BIT != 0)
            .count();
        let shore = result
            .map_data
            .iter()
            .filter(|b| **b & SHORE_BIT != 0)
            .count();
        assert!(ocean > 0, "expected ocean tiles");
        assert!(shore > 0, "expected shoreline tiles");
    }

    #[test]
    fn downscale_water_wins_mixed_block() {
        let mut source = Terrain::new(2, 2);
        source.is_land.fill(true);
        source.is_land[0] = false;

        let result = downscale(&source, 1, 1);

        assert!(!result.is_land[0], "water must win a mixed source block");
    }

    #[test]
    fn diagonal_water_gets_a_cardinal_link() {
        let mut t = Terrain::new(2, 2);
        t.is_land = vec![false, true, true, false];

        connect_diagonal_water(&mut t);

        assert!(!t.is_land[1], "the corner between two water tiles must open");
        assert!(t.is_land[2]);
    }

    #[test]
    fn higher_water_level_floods_more_tiles_when_enlarging() {
        let img = encoded_coast(8, 8);
        let water_tiles = |level: f32| {
            let water = WaterSettings {
                smooth_coasts: true,
                water_level: level,
                min_lake_tiles: 0,
                ..WaterSettings::default()
            };
            let result = generate_with_water(&img, Some((32, 32)), &water).expect("pipeline");
            result.map_data.iter().filter(|b| **b & LAND_BIT == 0).count()
        };
        assert!(water_tiles(0.8) > water_tiles(0.2));
    }

    #[test]
    fn generation_is_deterministic() {
        let img = encoded_coast(40, 40);
        let first = generate_from_rgba(&img, Some((20, 20))).expect("first pipeline run");
        let second = generate_from_rgba(&img, Some((20, 20))).expect("second pipeline run");

        assert_eq!(first.width, second.width);
        assert_eq!(first.height, second.height);
        assert_eq!(first.num_land_tiles, second.num_land_tiles);
        assert_eq!(first.map_data, second.map_data);
    }

    #[test]
    fn plains_land_mag_zero() {
        let mut img = RgbaImage::new(8, 8);
        for p in img.pixels_mut() {
            *p = Rgba([0, 0, 140, 255]);
        }
        let result = generate_from_rgba(&img, Some((8, 8))).expect("pipeline");
        for &b in &result.map_data {
            assert!(b & LAND_BIT != 0);
            assert_eq!(b & 0b0001_1111, 0, "flat plains should have mag 0");
        }
    }

    #[test]
    fn mobile_safe_dims_preserve_aspect_and_budget() {
        let (w, h) = mobile_safe_dims(2800, 1448);
        assert!(w <= maps::MOBILE_SAFE_MAX_AXIS && h <= maps::MOBILE_SAFE_MAX_AXIS);
        assert!((w as u64) * (h as u64) <= maps::MOBILE_SAFE_MAX_PIXELS as u64);
        let src_ar = 2800.0 / 1448.0;
        let dst_ar = w as f64 / h as f64;
        assert!((src_ar - dst_ar).abs() < 0.05);
    }
}
