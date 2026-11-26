use super::core::{Building, BuildingGrid};
use crate::game::BuildingKind;
use crate::game_config::GameConfig;
use crate::map::{GameMap, TerrainType};
use crate::protocol::BuildingSnapshot;

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
        Self::with_size(anchor_x, anchor_y, width, height)
    }

    #[inline]
    pub(crate) const fn legacy_at(anchor_x: u32, anchor_y: u32) -> Self {
        Self::with_size(anchor_x, anchor_y, 4, 4)
    }

    #[inline]
    const fn with_size(anchor_x: u32, anchor_y: u32, width: u32, height: u32) -> Self {
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
        if matches!(self, BuildingKind::City) {
            (4, 4)
        } else {
            (2, 2)
        }
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
    CityRequired,
    TooCloseToCity,
    OutsideCityRange,
}

impl PlacementError {
    pub const fn reason_key(self) -> &'static str {
        match self {
            Self::OutsideMap => "hud.build_outside_map",
            Self::OwnLand => "hud.build_owned_land",
            Self::Occupied => "hud.build_occupied",
            Self::CoastRequired => "hud.build_coast_required",
            Self::LowlandRequired => "hud.build_military_camp_lowland_required",
            Self::CityRequired => "hud.build_occupied",
            Self::TooCloseToCity => "hud.build_too_close_to_city",
            Self::OutsideCityRange => "hud.build_outside_city_range",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CityAnchor {
    pub id: u64,
    pub owner_id: u16,
    pub x: u32,
    pub y: u32,
    pub active_level: u8,
    pub level: u8,
}

impl CityAnchor {
    #[inline]
    pub fn from_building(b: &Building, map_width: u32) -> Option<Self> {
        if b.kind != BuildingKind::City || map_width == 0 {
            return None;
        }
        Some(Self {
            id: b.id,
            owner_id: b.owner_id,
            x: b.tile_idx % map_width,
            y: b.tile_idx / map_width,
            active_level: b.active_level(),
            level: b.level,
        })
    }

    #[inline]
    pub fn from_snapshot(b: &BuildingSnapshot, map_width: u32) -> Option<Self> {
        if b.kind != BuildingKind::City || map_width == 0 {
            return None;
        }
        Some(Self {
            id: b.id,
            owner_id: b.owner_id,
            x: b.tile_idx % map_width,
            y: b.tile_idx / map_width,
            active_level: b.active_level(),
            level: b.level,
        })
    }
}

pub fn city_anchors_from_buildings(buildings: &[Building], map_width: u32) -> Vec<CityAnchor> {
    buildings
        .iter()
        .filter_map(|b| CityAnchor::from_building(b, map_width))
        .collect()
}

pub fn city_anchors_from_snapshots(
    buildings: &[BuildingSnapshot],
    map_width: u32,
) -> Vec<CityAnchor> {
    buildings
        .iter()
        .filter_map(|b| CityAnchor::from_snapshot(b, map_width))
        .collect()
}

/// Construction radius in tiles for a City at `city_level`.
#[inline]
pub fn city_construction_radius(city_level: u8, cfg: &GameConfig) -> u32 {
    let base = cfg.city_base_radius.max(4);
    let step = cfg.city_radius_per_level;
    base.saturating_add(u32::from(city_level.saturating_sub(1)).saturating_mul(step))
}

/// Minimum spacing required between a newly founded City and an existing City of `existing_level`.
#[inline]
pub fn city_exclusion_radius(existing_level: u8, cfg: &GameConfig) -> u32 {
    city_construction_radius(existing_level.max(1), cfg).saturating_mul(2)
}

#[inline]
pub fn city_covers_coord(city: CityAnchor, x: u32, y: u32, cfg: &GameConfig) -> bool {
    if city.active_level == 0 {
        return false;
    }
    let radius = i64::from(city_construction_radius(city.active_level, cfg));
    euclid_sq(i64::from(x), i64::from(y), i64::from(city.x), i64::from(city.y))
        <= radius * radius
}

pub fn validate_city_spacing(
    x: u32,
    y: u32,
    cities: &[CityAnchor],
    cfg: &GameConfig,
) -> Result<(), PlacementError> {
    for &city in cities {
        let excl = i64::from(city_exclusion_radius(city.active_level.max(city.level).max(1), cfg));
        if euclid_sq(i64::from(x), i64::from(y), i64::from(city.x), i64::from(city.y))
            < excl * excl
        {
            return Err(PlacementError::TooCloseToCity);
        }
    }
    Ok(())
}

pub fn validate_district_city_range(
    owner_id: u16,
    x: u32,
    y: u32,
    cities: &[CityAnchor],
    required_city_id: Option<u64>,
    cfg: &GameConfig,
) -> Result<(), PlacementError> {
    let covered = cities.iter().any(|&city| {
        city.owner_id == owner_id
            && required_city_id.is_none_or(|id| city.id == id)
            && city_covers_coord(city, x, y, cfg)
    });
    if covered {
        Ok(())
    } else {
        Err(PlacementError::OutsideCityRange)
    }
}

pub fn max_covering_city_level(
    owner_id: u16,
    x: u32,
    y: u32,
    cities: &[CityAnchor],
    cfg: &GameConfig,
) -> u8 {
    cities
        .iter()
        .filter(|city| city.owner_id == owner_id && city_covers_coord(**city, x, y, cfg))
        .map(|city| city.active_level)
        .max()
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CitySiteTraits {
    pub has_coastal_port_access: bool,
    pub coastal_water_tiles: u32,
    pub land_tiles: u32,
    pub highland_tiles: u32,
    pub mountain_tiles: u32,
    pub total_buildable_tiles: u32,
    pub blocked_by_city_spacing: bool,
}

/// Evaluates terrain pros and cons inside the founding City's initial radius.
pub fn evaluate_city_site_traits(
    map: &GameMap,
    x: u32,
    y: u32,
    cities: &[CityAnchor],
    cfg: &GameConfig,
) -> CitySiteTraits {
    let mut traits = CitySiteTraits::default();
    if map.width == 0 || map.height == 0 || x >= map.width || y >= map.height {
        return traits;
    }
    let radius = city_construction_radius(1, cfg) as i32;
    let r_sq = i64::from(radius) * i64::from(radius);
    let min_x = (x as i32 - radius).max(0);
    let max_x = (x as i32 + radius).min(map.width as i32 - 1);
    let min_y = (y as i32 - radius).max(0);
    let max_y = (y as i32 + radius).min(map.height as i32 - 1);
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            if euclid_sq(i64::from(tx), i64::from(ty), i64::from(x), i64::from(y)) > r_sq {
                continue;
            }
            let (ux, uy) = (tx as u32, ty as u32);
            match map.terrain_type(ux, uy) {
                TerrainType::Water | TerrainType::Lake => {
                    traits.coastal_water_tiles += 1;
                }
                TerrainType::Land => {
                    traits.land_tiles += 1;
                    traits.total_buildable_tiles += 1;
                    if is_shore_land_tile(map, ux, uy) {
                        traits.has_coastal_port_access = true;
                    }
                }
                TerrainType::Highland => {
                    traits.highland_tiles += 1;
                    traits.total_buildable_tiles += 1;
                    if is_shore_land_tile(map, ux, uy) {
                        traits.has_coastal_port_access = true;
                    }
                }
                TerrainType::Mountain => {
                    traits.mountain_tiles += 1;
                    traits.total_buildable_tiles += 1;
                }
            }
        }
    }
    traits.blocked_by_city_spacing = validate_city_spacing(x, y, cities, cfg).is_err();
    traits
}

/// Calls `visit(water_tile)` for every water tile immediately bordering a structure footprint.
pub fn for_each_structure_coastal_water_tile(
    map: &GameMap,
    kind: BuildingKind,
    tile_idx: u32,
    mut visit: impl FnMut(u32),
) {
    if map.width == 0 || map.height == 0 || tile_idx >= map.width.saturating_mul(map.height) {
        return;
    }
    let (x, y) = idx_xy(tile_idx, map.width);
    let footprint = BuildingFootprint::at(kind, x, y);
    let left = footprint.left.saturating_sub(1).max(0);
    let top = footprint.top.saturating_sub(1).max(0);
    let right = (footprint.left + footprint.width as i32).min(map.width as i32 - 1);
    let bottom = (footprint.top + footprint.height as i32).min(map.height as i32 - 1);
    for fy in top..=bottom {
        for fx in left..=right {
            if footprint.contains(fx, fy) {
                continue;
            }
            let tile = fy as u32 * map.width + fx as u32;
            if map.terrain[tile as usize].is_water() {
                visit(tile);
            }
        }
    }
}

/// Returns true if the selected structure footprint centered around `(x, y)` touches water.
pub fn structure_footprint_touches_water(
    kind: BuildingKind,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    terrain_at: impl Fn(u32, u32) -> u8,
) -> bool {
    let footprint = BuildingFootprint::at(kind, x, y);
    for fy in footprint.top - 1..=footprint.top + footprint.height as i32 {
        for fx in footprint.left - 1..=footprint.left + footprint.width as i32 {
            if fx >= 0
                && fy >= 0
                && fx < width as i32
                && fy < height as i32
                && terrain_at(fx as u32, fy as u32) & 0x80 == 0
            {
                return true;
            }
        }
    }
    false
}

/// Shared per-cell rules also identify the exact conflicts in a selected foundation.
pub fn validate_building_cell(
    _kind: BuildingKind,
    owner_id: u16,
    tile_owner: u16,
    terrain: u8,
) -> Result<(), PlacementError> {
    if owner_id == 0 || tile_owner != owner_id || terrain & 0x80 == 0 {
        return Err(PlacementError::OwnLand);
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
    if kind == BuildingKind::Port
        && !structure_footprint_touches_water(kind, x, y, width, height, &terrain_at)
    {
        return Err(PlacementError::CoastRequired);
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
        |fx, fy| map.owner_id(fx, fy),
        |fx, fy| map.terrain[map.ref_id(fx, fy)].as_byte(),
        |footprint| {
            existing.iter_all_in_range(x, y, 3).any(|building| {
                footprint.intersects(BuildingFootprint::at(building.kind, building.x, building.y))
            })
        },
    )
}

/// Full match validation: footprint + City spacing or City construction radius.
pub fn validate_match_structure_site(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    tile: u32,
    existing: &BuildingGrid,
    cities: &[CityAnchor],
    required_city_id: Option<u64>,
    cfg: &GameConfig,
) -> Result<(), PlacementError> {
    validate_structure_site(map, owner_id, kind, tile, existing)?;
    let (x, y) = idx_xy(tile, map.width);
    if kind == BuildingKind::City {
        validate_city_spacing(x, y, cities, cfg)
    } else {
        validate_district_city_range(owner_id, x, y, cities, required_city_id, cfg)
    }
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

        let footprint = if legacy {
            BuildingFootprint::legacy_at(x, y)
        } else {
            BuildingFootprint::at(kind, x, y)
        };
        let valid_placement = if !legacy {
            validate_structure_site(map, owner_id, kind, idx, existing).is_ok()
        } else {
            footprint_fits(map, owner_id, footprint)
                && !existing
                    .iter_all_in_range(x, y, LEGACY_CITY_SPACING as u32)
                    .any(|building| {
                        let dx = xi - building.x as i64;
                        let dy = yi - building.y as i64;
                        let min_dist = legacy_minimum_building_spacing(kind, building.kind) as i64;
                        let too_close = dx * dx + dy * dy < min_dist * min_dist;
                        let other = BuildingFootprint::legacy_at(building.x, building.y);
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

/// Closest legal foundation to (x, y) within `radius` tiles under the live
/// rule, so a preview can only ever rest on a site the simulation accepts.
pub fn nearest_structure_site(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    x: i32,
    y: i32,
    radius: i32,
    existing: &BuildingGrid,
) -> Option<u32> {
    nearest_structure_site_in_bounds(
        map,
        owner_id,
        kind,
        x,
        y,
        radius,
        existing,
        (0, 0, map.width as i32 - 1, map.height as i32 - 1),
    )
}

/// Closest legal foundation within a bounded set of anchor tiles.
pub fn nearest_structure_site_in_bounds(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    x: i32,
    y: i32,
    radius: i32,
    existing: &BuildingGrid,
    bounds: (i32, i32, i32, i32),
) -> Option<u32> {
    nearest_site_in_bounds_with(map, x, y, radius, bounds, |tile| {
        validate_structure_site(map, owner_id, kind, tile, existing).is_ok()
    })
}

/// Closest legal foundation obeying City spacing / City radius rules within bounds.
pub fn nearest_match_structure_site_in_bounds(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    x: i32,
    y: i32,
    radius: i32,
    existing: &BuildingGrid,
    cities: &[CityAnchor],
    required_city_id: Option<u64>,
    cfg: &GameConfig,
    bounds: (i32, i32, i32, i32),
) -> Option<u32> {
    nearest_site_in_bounds_with(map, x, y, radius, bounds, |tile| {
        validate_match_structure_site(
            map,
            owner_id,
            kind,
            tile,
            existing,
            cities,
            required_city_id,
            cfg,
        )
        .is_ok()
    })
}

fn nearest_site_in_bounds_with(
    map: &GameMap,
    x: i32,
    y: i32,
    radius: i32,
    (bound_x_min, bound_y_min, bound_x_max, bound_y_max): (i32, i32, i32, i32),
    is_valid: impl Fn(u32) -> bool,
) -> Option<u32> {
    if radius < 0 || map.width == 0 || map.height == 0 {
        return None;
    }
    let w = map.width as i32;
    let h = map.height as i32;
    let x_min = x.saturating_sub(radius).max(0).max(bound_x_min);
    let x_max = x.saturating_add(radius).min(w - 1).min(bound_x_max);
    let y_min = y.saturating_sub(radius).max(0).max(bound_y_min);
    let y_max = y.saturating_add(radius).min(h - 1).min(bound_y_max);
    if x_min > x_max || y_min > y_max {
        return None;
    }

    let min_r = (x_min - x)
        .max(0)
        .max(x - x_max)
        .max(y_min - y)
        .max(y - y_max);
    let max_r = (x - x_min)
        .abs()
        .max((x_max - x).abs())
        .max((y - y_min).abs())
        .max((y_max - y).abs())
        .min(radius);

    let mut best: Option<(i64, u32)> = None;
    let consider = |best: &mut Option<(i64, u32)>, col: i32, row: i32| {
        let distance = euclid_sq(col as i64, row as i64, x as i64, y as i64);
        let tile = xy_idx(col as u32, row as u32, map.width);
        if best.is_some_and(|current| current <= (distance, tile)) {
            return;
        }
        if is_valid(tile) {
            *best = Some((distance, tile));
        }
    };

    for r in min_r..=max_r {
        // Every point on Chebyshev ring r has distance >= r^2; no farther ring can beat best_dist.
        if let Some((best_dist, _)) = best
            && (r as i64).saturating_mul(r as i64) > best_dist
        {
            break;
        }
        if r == 0 {
            consider(&mut best, x, y);
            continue;
        }
        let col_lo = (x - r).max(x_min);
        let col_hi = (x + r).min(x_max);
        let top_row = y - r;
        if top_row >= y_min && top_row <= y_max {
            for col in col_lo..=col_hi {
                consider(&mut best, col, top_row);
            }
        }
        let bot_row = y + r;
        if bot_row >= y_min && bot_row <= y_max {
            for col in col_lo..=col_hi {
                consider(&mut best, col, bot_row);
            }
        }
        let inner_lo = (y - r + 1).max(y_min);
        let inner_hi = (y + r - 1).min(y_max);
        if inner_lo <= inner_hi {
            let left_col = x - r;
            let right_col = x + r;
            let check_left = left_col >= x_min && left_col <= x_max;
            let check_right = right_col >= x_min && right_col <= x_max;
            for row in inner_lo..=inner_hi {
                if check_left {
                    consider(&mut best, left_col, row);
                }
                if check_right {
                    consider(&mut best, right_col, row);
                }
            }
        }
    }
    best.map(|(_, tile)| tile)
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
