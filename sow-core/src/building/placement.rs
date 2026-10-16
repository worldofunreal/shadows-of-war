use super::core::BuildingGrid;
use crate::game::BuildingKind;
use crate::map::{GameMap, TerrainType};

/// Frozen spacing used exclusively when verifying pre-v3 recordings.
const LEGACY_CITY_SPACING: i32 = 6;
const LEGACY_BUILDING_SPACING: i32 = 4;
/// Bounded AI candidate search, independent of legal building adjacency.
const STRUCTURE_SEARCH_RADIUS_SQ: i64 = 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingFootprint {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

impl BuildingFootprint {
    #[inline]
    pub const fn at(kind: BuildingKind, anchor_x: u32, anchor_y: u32) -> Self {
        let (width, height) = kind.footprint_dimensions();
        Self {
            left: anchor_x as i32 - (width as i32 / 2),
            top: anchor_y as i32 - (height as i32 / 2),
            width,
            height,
        }
    }

    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        self.left < other.left + other.width as i32
            && other.left < self.left + self.width as i32
            && self.top < other.top + other.height as i32
            && other.top < self.top + self.height as i32
    }

    #[inline]
    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left
            && x < self.left + self.width as i32
            && y >= self.top
            && y < self.top + self.height as i32
    }
}

impl BuildingKind {
    #[inline]
    pub const fn footprint_dimensions(self) -> (u32, u32) {
        (4, 4)
    }
}

#[inline]
pub(crate) const fn legacy_minimum_building_spacing(a: BuildingKind, b: BuildingKind) -> i32 {
    if matches!(a, BuildingKind::City) || matches!(b, BuildingKind::City) {
        LEGACY_CITY_SPACING
    } else {
        LEGACY_BUILDING_SPACING
    }
}

#[inline]
pub fn idx_xy(idx: u32, w: u32) -> (u32, u32) {
    (idx % w, idx / w)
}

#[inline]
pub fn xy_idx(x: u32, y: u32, w: u32) -> u32 {
    y * w + x
}

#[inline]
pub fn euclid_sq(ax: i64, ay: i64, bx: i64, by: i64) -> i64 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

#[inline]
pub fn manhattan(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs() + (ay - by).abs()
}

/// Odd-r offset hex distance (matches map neighbor topology).
#[inline]
pub fn hex_distance(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let r1 = y1;
    let q1 = x1 - (y1 - (y1 & 1)) / 2;
    let s1 = -q1 - r1;
    let r2 = y2;
    let q2 = x2 - (y2 - (y2 & 1)) / 2;
    let s2 = -q2 - r2;
    ((q1 - q2).abs() + (r1 - r2).abs() + (s1 - s2).abs()) / 2
}

/// Land shoreline tile: land terrain with shoreline bit.
pub fn is_shore_land_tile(map: &GameMap, x: u32, y: u32) -> bool {
    let t = map.terrain[map.ref_id(x, y)];
    t.is_land() && t.is_shoreline()
}

fn is_land_structure_tile(map: &GameMap, x: u32, y: u32) -> bool {
    matches!(
        map.terrain_type(x, y),
        TerrainType::Land | TerrainType::Highland | TerrainType::Mountain
    )
}

pub fn footprint_fits(map: &GameMap, owner_id: u16, footprint: BuildingFootprint) -> bool {
    for y in footprint.top..footprint.top + footprint.height as i32 {
        for x in footprint.left..footprint.left + footprint.width as i32 {
            if !map.is_valid_coord(x, y)
                || map.owner_id(x as u32, y as u32) != owner_id
                || !is_land_structure_tile(map, x as u32, y as u32)
            {
                return false;
            }
        }
    }
    true
}

fn footprint_touches_water(map: &GameMap, footprint: BuildingFootprint) -> bool {
    let mut touches_water = false;
    for y in footprint.top..footprint.top + footprint.height as i32 {
        for x in footprint.left..footprint.left + footprint.width as i32 {
            if !map.is_valid_coord(x, y) {
                continue;
            }
            map.for_each_neighbor(x as u32, y as u32, |nx, ny| {
                if !map.terrain[map.ref_id(nx, ny)].is_land() {
                    touches_water = true;
                }
            });
        }
    }
    touches_water
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    OutsideMap,
    OwnLand,
    Occupied,
    CoastRequired,
    LowlandRequired,
}

impl PlacementError {
    pub const fn reason_key(self) -> &'static str {
        match self {
            Self::OutsideMap => "hud.build_outside_map",
            Self::OwnLand => "hud.build_owned_land",
            Self::Occupied => "hud.build_occupied",
            Self::CoastRequired => "hud.build_coast_required",
            Self::LowlandRequired => "hud.build_lowland_required",
        }
    }
}

/// Shared per-cell rules also identify the exact conflicts in a selected foundation.
pub fn validate_building_cell(
    kind: BuildingKind,
    owner_id: u16,
    tile_owner: u16,
    terrain: u8,
) -> Result<(), PlacementError> {
    if owner_id == 0 || tile_owner != owner_id || terrain & 0x80 == 0 {
        return Err(PlacementError::OwnLand);
    }
    if kind == BuildingKind::Farm && terrain & 0x1f >= 10 {
        return Err(PlacementError::LowlandRequired);
    }
    Ok(())
}

/// One placement rule for simulation, preview and guides. Adjoining bases are legal.
pub fn validate_building_site(
    kind: BuildingKind,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    owner_id: u16,
    owner_at: impl Fn(u32, u32) -> u16,
    terrain_at: impl Fn(u32, u32) -> u8,
    occupied: impl FnOnce(BuildingFootprint) -> bool,
) -> Result<(), PlacementError> {
    let footprint = BuildingFootprint::at(kind, x, y);
    if width == 0
        || height == 0
        || x >= width
        || y >= height
        || footprint.left < 0
        || footprint.top < 0
        || footprint.left + footprint.width as i32 > width as i32
        || footprint.top + footprint.height as i32 > height as i32
    {
        return Err(PlacementError::OutsideMap);
    }
    for fy in footprint.top..footprint.top + footprint.height as i32 {
        for fx in footprint.left..footprint.left + footprint.width as i32 {
            let (fx, fy) = (fx as u32, fy as u32);
            let terrain = terrain_at(fx, fy);
            validate_building_cell(kind, owner_id, owner_at(fx, fy), terrain)?;
        }
    }
    if occupied(footprint) {
        return Err(PlacementError::Occupied);
    }
    if kind == BuildingKind::Port {
        let mut coast = false;
        for fy in footprint.top - 1..=footprint.top + footprint.height as i32 {
            for fx in footprint.left - 1..=footprint.left + footprint.width as i32 {
                if fx >= 0
                    && fy >= 0
                    && fx < width as i32
                    && fy < height as i32
                    && terrain_at(fx as u32, fy as u32) & 0x80 == 0
                {
                    coast = true;
                }
            }
        }
        if !coast {
            return Err(PlacementError::CoastRequired);
        }
    }
    Ok(())
}

pub fn validate_structure_site(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    tile: u32,
    existing: &BuildingGrid,
) -> Result<(), PlacementError> {
    if map.width == 0 || tile >= map.width.saturating_mul(map.height) {
        return Err(PlacementError::OutsideMap);
    }
    let (x, y) = idx_xy(tile, map.width);
    validate_building_site(
        kind,
        x,
        y,
        map.width,
        map.height,
        owner_id,
        |x, y| map.owner_id(x, y),
        |x, y| map.terrain[map.ref_id(x, y)].as_byte(),
        |footprint| {
            existing.iter_all_in_range(x, y, 3).any(|building| {
                footprint.intersects(BuildingFootprint::at(building.kind, building.x, building.y))
            })
        },
    )
}

fn for_each_valid_land_structure_index(
    map: &GameMap,
    owner_id: u16,
    click_idx: u32,
    kind: BuildingKind,
    existing: &BuildingGrid,
    scratch: &mut crate::engine::PlacementScratch,
    legacy: bool,
    mut visit: impl FnMut(u32),
) {
    let w = map.width;
    let (cx, cy) = idx_xy(click_idx, w);
    if !is_land_structure_tile(map, cx, cy) {
        return;
    }
    if map.owner_id(cx, cy) != owner_id {
        return;
    }

    let cx_i = cx as i64;
    let cy_i = cy as i64;

    let stamp = scratch.stamp.wrapping_add(1);
    scratch.stamp = if stamp == 0 {
        scratch.visited_stamp.fill(0);
        1
    } else {
        stamp
    };
    let stamp = scratch.stamp;

    scratch.queue.clear();
    scratch.visited_stamp[480] = stamp;
    scratch.queue.push(click_idx);

    let mut qi = 0usize;
    while qi < scratch.queue.len() {
        let idx = scratch.queue[qi];
        qi += 1;
        let (x, y) = idx_xy(idx, w);
        let xi = x as i64;
        let yi = y as i64;
        if euclid_sq(xi, yi, cx_i, cy_i) >= STRUCTURE_SEARCH_RADIUS_SQ {
            continue;
        }
        if !is_land_structure_tile(map, x, y) {
            continue;
        }
        if map.owner_id(x, y) != owner_id {
            continue;
        }
        #[cfg(feature = "ai-metrics")]
        {
            scratch.candidates_examined += 1;
        }

        let footprint = BuildingFootprint::at(kind, x, y);
        let valid_placement = if !legacy {
            validate_structure_site(map, owner_id, kind, idx, existing).is_ok()
        } else {
            footprint_fits(map, owner_id, footprint)
                && !existing
                    .iter_all_in_range(x, y, LEGACY_CITY_SPACING as u32)
                    .any(|building| {
                        #[cfg(feature = "ai-metrics")]
                        {
                            scratch.building_checks += 1;
                        }
                        let dx = xi - building.x as i64;
                        let dy = yi - building.y as i64;
                        let min_dist = legacy_minimum_building_spacing(kind, building.kind) as i64;
                        let too_close = dx * dx + dy * dy < min_dist * min_dist;
                        let other = BuildingFootprint::at(building.kind, building.x, building.y);
                        too_close || footprint.intersects(other)
                    })
                && (kind != BuildingKind::Port || footprint_touches_water(map, footprint))
        };
        if valid_placement {
            visit(idx);
        }

        map.for_each_neighbor(x, y, |nx, ny| {
            let nxi = nx as i64;
            let nyi = ny as i64;
            if euclid_sq(nxi, nyi, cx_i, cy_i) >= STRUCTURE_SEARCH_RADIUS_SQ {
                return;
            }
            if map.owner_id(nx, ny) != owner_id {
                return;
            }
            let lx = (nx as i32 - cx as i32 + 15) as usize;
            let ly = (ny as i32 - cy as i32 + 15) as usize;
            let lidx = ly * 31 + lx;
            if scratch.visited_stamp[lidx] == stamp {
                return;
            }
            scratch.visited_stamp[lidx] = stamp;
            scratch.queue.push(xy_idx(nx, ny, w));
        });
    }
}

/// Bounded AI candidate list using the current exact footprint rules.
pub fn valid_land_structure_indices(
    map: &GameMap,
    owner_id: u16,
    click_idx: u32,
    kind: BuildingKind,
    existing: &BuildingGrid,
    scratch: &mut crate::engine::PlacementScratch,
) -> Vec<u32> {
    let w = map.width;
    let (cx, cy) = idx_xy(click_idx, w);
    let cx_i = cx as i64;
    let cy_i = cy as i64;
    let mut out = Vec::new();
    for_each_valid_land_structure_index(
        map,
        owner_id,
        click_idx,
        kind,
        existing,
        scratch,
        false,
        |idx| out.push(idx),
    );

    out.sort_by(|&a, &b| {
        let (ax, ay) = idx_xy(a, w);
        let (bx, by) = idx_xy(b, w);
        let da = euclid_sq(ax as i64, ay as i64, cx_i, cy_i);
        let db = euclid_sq(bx as i64, by as i64, cx_i, cy_i);
        da.cmp(&db).then_with(|| a.cmp(&b))
    });
    out
}

pub fn find_structure_site_near(
    map: &GameMap,
    owner_id: u16,
    click_idx: u32,
    kind: BuildingKind,
    existing: &BuildingGrid,
    scratch: &mut crate::engine::PlacementScratch,
    legacy: bool,
) -> Option<u32> {
    let w = map.width;
    let (cx, cy) = idx_xy(click_idx, w);
    let mut best: Option<(i64, u32)> = None;
    for_each_valid_land_structure_index(
        map,
        owner_id,
        click_idx,
        kind,
        existing,
        scratch,
        legacy,
        |idx| {
            let (x, y) = idx_xy(idx, w);
            let candidate = (euclid_sq(x as i64, y as i64, cx as i64, cy as i64), idx);
            if best.is_none_or(|current| candidate < current) {
                best = Some(candidate);
            }
        },
    );
    best.map(|(_, idx)| idx)
}

/// Resolve final spawn tile index for `kind` at `click_idx`, or `None` if illegal.
pub fn resolve_structure_spawn_tile(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    click_idx: u32,
    existing: &BuildingGrid,
    _scratch: &mut crate::engine::PlacementScratch,
) -> Option<u32> {
    let w = map.width;
    let h = map.height;
    let max_idx = w.saturating_mul(h);
    if click_idx >= max_idx {
        return None;
    }

    validate_structure_site(map, owner_id, kind, click_idx, existing)
        .ok()
        .map(|()| click_idx)
}
