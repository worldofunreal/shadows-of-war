use super::placement::{BuildingFootprint, hex_distance};
use crate::execution::hero_effects::allied;
use crate::game::{BuildingKind, CityArsenalKind, GameState};
use crate::map::MapTile;
use crate::rng::stable_tile_sample;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Maximum Euclidean tile distance between eligible railway station buildings.
pub const RAILWAY_LINK_DISTANCE: u32 = 36;
/// Minimum active Factory level required to create a railway station node.
pub const RAILWAY_REQUIRED_FACTORY_LEVEL: u8 = 2;
pub const RAILWAY_MAX_CANDIDATES_PER_STATION: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RailwayLink {
    pub from_building_id: u64,
    pub to_building_id: u64,
    pub horizontal_first: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CityArsenalBuild {
    pub kind: CityArsenalKind,
    pub ticks_until_complete: u32,
    pub total_ticks: u32,
    #[serde(default)]
    pub warship_source_tile: Option<u32>,
    #[serde(default)]
    pub warship_target_tile: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CityArsenalState {
    pub has_anti_nuke: bool,
    pub nuke_ready: bool,
    pub arsenal_build: Option<CityArsenalBuild>,
}

impl CityArsenalState {
    #[inline]
    pub const fn is_building(self) -> bool {
        self.arsenal_build.is_some()
    }
}

fn compute_city_visual_level(
    items: impl Iterator<Item = (u32, BuildingKind, u8)>,
    tile_idx: u32,
) -> u8 {
    items
        .filter(|(tile, kind, _)| *tile == tile_idx && *kind == BuildingKind::City)
        .map(|(_, _, level)| level)
        .max()
        .unwrap_or(0)
}

/// Visual tier `0..=4` for a City on the map (`0` while the City base is still under construction,
/// otherwise the City's own completed level `1..=4`).
#[inline]
pub fn city_visual_level(buildings: &[Building], tile_idx: u32) -> u8 {
    compute_city_visual_level(
        buildings.iter().map(|b| (b.tile_idx, b.kind, b.active_level())),
        tile_idx,
    )
}

/// Snapshot equivalent of [`city_visual_level`] for client UI and map overlays.
#[inline]
pub fn city_snapshot_visual_level(
    buildings: &[crate::protocol::BuildingSnapshot],
    tile_idx: u32,
) -> u8 {
    compute_city_visual_level(
        buildings.iter().map(|b| (b.tile_idx, b.kind, b.active_level())),
        tile_idx,
    )
}

/// True when any structure or old city-tile arsenal task at `tile_idx` is under construction.
#[inline]
pub fn city_has_active_build(
    buildings: &[Building],
    city_arsenals: &HashMap<u64, CityArsenalState>,
    tile_idx: u32,
) -> bool {
    buildings.iter().any(|b| {
        b.tile_idx == tile_idx
            && (b.under_construction
                || (b.kind == BuildingKind::City
                    && city_arsenals
                        .get(&b.id)
                        .is_some_and(|state| state.arsenal_build.is_some())))
    })
}

fn railway_axis_is_land_with(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    map_w: u32,
    map_h: u32,
    is_land: Option<&dyn Fn(u32) -> bool>,
) -> bool {
    let (mut x, mut y) = (x0, y0);
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    loop {
        if x < 0 || y < 0 || x >= map_w as i32 || y >= map_h as i32 {
            return false;
        }
        let tile = y as u32 * map_w + x as u32;
        if is_land.is_some_and(|check| !check(tile)) {
            return false;
        }
        if x == x1 && y == y1 {
            return true;
        }
        x += sx;
        y += sy;
    }
}

fn railway_route_is_land_with(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    map_w: u32,
    map_h: u32,
    is_land: Option<&dyn Fn(u32) -> bool>,
) -> Option<bool> {
    if railway_axis_is_land_with(ax, ay, bx, ay, map_w, map_h, is_land)
        && railway_axis_is_land_with(bx, ay, bx, by, map_w, map_h, is_land)
    {
        Some(true)
    } else if railway_axis_is_land_with(ax, ay, ax, by, map_w, map_h, is_land)
        && railway_axis_is_land_with(ax, by, bx, by, map_w, map_h, is_land)
    {
        Some(false)
    } else {
        None
    }
}

/// Canonical check for whether an L-shaped land railway axis crosses only land tiles.
pub fn railway_axis_is_land(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    map_w: u32,
    map_h: u32,
    terrain: Option<&[MapTile]>,
) -> bool {
    let checker = terrain.map(|tiles| {
        move |tile: u32| tiles.get(tile as usize).is_some_and(MapTile::is_land)
    });
    railway_axis_is_land_with(
        x0,
        y0,
        x1,
        y1,
        map_w,
        map_h,
        checker.as_ref().map(|f| f as &dyn Fn(u32) -> bool),
    )
}

/// Returns `Some(horizontal_first)` if `(ax, ay)` and `(bx, by)` connect over land by railway.
pub fn railway_route_is_land(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    map_w: u32,
    map_h: u32,
    terrain: Option<&[MapTile]>,
) -> Option<bool> {
    let checker = terrain.map(|tiles| {
        move |tile: u32| tiles.get(tile as usize).is_some_and(MapTile::is_land)
    });
    railway_route_is_land_with(
        ax,
        ay,
        bx,
        by,
        map_w,
        map_h,
        checker.as_ref().map(|f| f as &dyn Fn(u32) -> bool),
    )
}

/// Snapshot check matching [`populate_railway_connections`] for a City's active railway link.
pub fn city_snapshot_has_railway(
    snapshot: &crate::protocol::SimSnapshot,
    city_tile: u32,
    map_w: u32,
    map_h: u32,
    _is_land: Option<&dyn Fn(u32) -> bool>,
) -> bool {
    if map_w == 0 || map_h == 0 || u64::from(city_tile) >= u64::from(map_w) * u64::from(map_h) {
        return false;
    }
    let Some(city) = snapshot.buildings.iter().find(|building| {
        building.tile_idx == city_tile
            && building.kind == BuildingKind::City
            && building.active_level() > 0
    }) else {
        return false;
    };
    snapshot.railway_links.iter().any(|link| {
        link.from_building_id == city.id || link.to_building_id == city.id
    })
}

#[inline]
pub fn upgrade_duration_ticks(kind: BuildingKind, target_level: u8) -> u32 {
    let base_dur = kind.construction_duration_ticks();
    let mut dur = base_dur;
    for _ in 1..target_level {
        dur = (dur as f64 * 1.1) as u32;
    }
    dur.max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Building {
    pub id: u64,
    pub owner_id: u16,
    /// Linear index `y * width + x`.
    pub tile_idx: u32,
    pub kind: BuildingKind,
    pub level: u8,
    pub under_construction: bool,
    pub ticks_until_complete: u32,
}

impl Building {
    #[inline]
    pub fn active_level(&self) -> u8 {
        if self.under_construction {
            self.level.saturating_sub(1)
        } else {
            self.level
        }
    }

    #[inline]
    pub fn defense_range_cfg(&self, cfg: &crate::game_config::GameConfig) -> i32 {
        if self.kind == BuildingKind::Tower && !self.under_construction {
            tower_range_for_level(self.active_level(), cfg, 1.0)
        } else {
            0
        }
    }
}

/// Tower range uses the same base, level increments, cap, and hero bonus in
/// combat and every client range display.
/// Widest reach a tower can have: its 20-tile cap times the strongest tower-range hero (Sun Tzu, 1.5x).
pub(crate) const MAX_TOWER_REACH: u32 = 30;

#[inline]
pub fn tower_range_for_level(
    level: u8,
    cfg: &crate::game_config::GameConfig,
    range_multiplier: f64,
) -> i32 {
    if level == 0 {
        return 0;
    }
    let base = cfg.tower_range.round().clamp(0.0, 20.0) as i32;
    let range = (base + i32::from(level.saturating_sub(1)) * 2).min(20);
    (f64::from(range) * range_multiplier).round() as i32
}

const GOLD_INCOME_LEVEL_GROWTH: f64 = 1.1948581737791502;

pub fn gold_income_level_weight(level: u8) -> f64 {
    (0..level)
        .map(|n| GOLD_INCOME_LEVEL_GROWTH.powi(i32::from(n)))
        .sum()
}

pub fn income_level_weight(level: u8) -> f64 {
    (0..level).map(|n| 1.5_f64.powi(i32::from(n))).sum()
}

pub fn tower_defense_weight(level: u8, cfg: &crate::game_config::GameConfig) -> f64 {
    if cfg.progressive_defense_upgrades {
        income_level_weight(level)
    } else {
        f64::from(level)
    }
}

/// Capacity starts with a non-City building's first completed upgrade, not its foundation.
pub fn district_capacity_level_weight(level: u8) -> f64 {
    income_level_weight(level.saturating_sub(1))
}

/// Per-player totals for income / fleet gates (only **ready** structures count).
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BuildingAggregate {
    pub city_levels: u32,
    pub tower_levels: u32,
    pub factory_levels: u32,
    pub port_levels: u32,
    pub farm_levels: u32,
    pub sam_levels: u32,
    /// Sum of 1.5x marginal level weights across active Farms.
    pub farm_income_level_weight: f64,
    /// Sum of 1.5x marginal level weights across active Ports.
    pub port_income_level_weight: f64,
    /// Sum of the factory's gold-specific marginal weights.
    pub factory_income_level_weight: f64,
    /// Sum of 1.5x marginal level weights across active Cities.
    pub city_income_level_weight: f64,
    /// Shared small capacity bonus from completed non-City upgrades.
    pub district_capacity_level_weight: f64,
    pub has_completed_port: bool,
    /// Ready Cities, retained for AI and railway calculations.
    pub ready_city_count: u32,
    /// Ready factories only.
    pub ready_factory_count: u32,
    /// Total instances per kind, **including** under construction (for bot build quotas).
    pub count_city: u32,
    pub count_tower: u32,
    pub count_factory: u32,
    pub count_port: u32,
    pub count_farm: u32,
    pub count_sam: u32,
    /// Number of owned Factory stations connected to a City or Port over land.
    pub railway_connections: u32,
}

impl BuildingAggregate {
    #[inline]
    pub fn total_structures_of_kind(self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::City => self.count_city,
            BuildingKind::Tower => self.count_tower,
            BuildingKind::Factory => self.count_factory,
            BuildingKind::Port => self.count_port,
            BuildingKind::Farm => self.count_farm,
            BuildingKind::SAM => self.count_sam,
        }
    }

    /// Sum of active levels for `kind` (matches `count_kind` / cost scaling).
    #[inline]
    pub fn levels_of_kind(self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::City => self.city_levels,
            BuildingKind::Tower => self.tower_levels,
            BuildingKind::Factory => self.factory_levels,
            BuildingKind::Port => self.port_levels,
            BuildingKind::Farm => self.farm_levels,
            BuildingKind::SAM => self.sam_levels,
        }
    }
}

/// `out[v]` is aggregate for owner id `v` (resize to `max_player_id + 1`).
pub fn aggregate_buildings_per_player(
    buildings: impl Iterator<Item = Building>,
    max_player_id: usize,
) -> Vec<BuildingAggregate> {
    let mut out = vec![BuildingAggregate::default(); max_player_id.saturating_add(1)];
    for b in buildings {
        let i = b.owner_id as usize;
        if i >= out.len() {
            continue;
        }
        let a = &mut out[i];
        match b.kind {
            BuildingKind::City => a.count_city += 1,
            BuildingKind::Tower => a.count_tower += 1,
            BuildingKind::Factory => a.count_factory += 1,
            BuildingKind::Port => a.count_port += 1,
            BuildingKind::Farm => a.count_farm += 1,
            BuildingKind::SAM => a.count_sam += 1,
        }
        let active_lvl = b.active_level();
        if active_lvl == 0 {
            continue;
        }
        let a = &mut out[i];
        if b.kind.grants_building_troop_capacity() {
            a.district_capacity_level_weight += district_capacity_level_weight(active_lvl);
        }
        match b.kind {
            BuildingKind::City => {
                a.city_levels += active_lvl as u32;
                a.city_income_level_weight += income_level_weight(active_lvl);
                a.ready_city_count += 1;
            }
            BuildingKind::Tower => {
                a.tower_levels += active_lvl as u32;
            }
            BuildingKind::Factory => {
                a.factory_levels += active_lvl as u32;
                a.factory_income_level_weight += gold_income_level_weight(active_lvl);
                a.ready_factory_count += 1;
            }
            BuildingKind::Port => {
                a.port_levels += active_lvl as u32;
                a.port_income_level_weight += income_level_weight(active_lvl);
                a.has_completed_port = true;
            }
            BuildingKind::Farm => {
                a.farm_levels += active_lvl as u32;
                a.farm_income_level_weight += income_level_weight(active_lvl);
            }
            BuildingKind::SAM => {
                a.sam_levels += active_lvl as u32;
            }
        }
    }
    out
}

/// Builds the single railway graph used by passive income, the building panel and map rendering.
/// Every edge includes a level-two Factory station; Cities and Ports are trade stations.
pub fn populate_railway_connections(
    buildings: &[Building],
    state: &GameState,
    aggregates: &mut [BuildingAggregate],
) -> Vec<RailwayLink> {
    for agg in aggregates.iter_mut() {
        agg.railway_connections = 0;
    }
    let map_w = state.map.width;
    let map_h = state.map.height;
    if map_w == 0 || map_h == 0 {
        return Vec::new();
    }
    let total_tiles = u64::from(map_w) * u64::from(map_h);
    let nodes: Vec<Building> = buildings
        .iter()
        .copied()
        .filter(|building| {
            let is_station = match building.kind {
                BuildingKind::City | BuildingKind::Port => building.active_level() > 0,
                BuildingKind::Factory => {
                    building.active_level() >= RAILWAY_REQUIRED_FACTORY_LEVEL
                }
                _ => false,
            };
            is_station && u64::from(building.tile_idx) < total_tiles
        })
        .collect();
    if nodes.len() < 2 {
        return Vec::new();
    }
    let max_dist_sq = (RAILWAY_LINK_DISTANCE as i32) * (RAILWAY_LINK_DISTANCE as i32);
    let terrain = Some(state.map.terrain.as_slice());
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let x = (node.tile_idx % map_w) as i32;
        let y = (node.tile_idx / map_w) as i32;
        grid.entry((x / RAILWAY_LINK_DISTANCE as i32, y / RAILWAY_LINK_DISTANCE as i32))
            .or_default()
            .push(index);
    }
    let mut offsets = (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .collect::<Vec<_>>();
    offsets.sort_by_key(|(dx, dy)| dx * dx + dy * dy);
    let mut links = Vec::with_capacity(nodes.len());
    let mut seen = HashSet::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let ax = (node.tile_idx % map_w) as i32;
        let ay = (node.tile_idx / map_w) as i32;
        let cell = (ax / RAILWAY_LINK_DISTANCE as i32, ay / RAILWAY_LINK_DISTANCE as i32);
        let mut checked = 0;
        let mut best: Option<(usize, i32)> = None;
        'cells: for (dx, dy) in &offsets {
            let Some(candidates) = grid.get(&(cell.0 + dx, cell.1 + dy)) else {
                continue;
            };
            for &other_index in candidates {
                if other_index == index {
                    continue;
                }
                checked += 1;
                if checked > RAILWAY_MAX_CANDIDATES_PER_STATION {
                    break 'cells;
                }
                let other = nodes[other_index];
                if node.kind != BuildingKind::Factory && other.kind != BuildingKind::Factory {
                    continue;
                }
                if node.owner_id != other.owner_id
                    && !allied(state, node.owner_id, other.owner_id)
                {
                    continue;
                }
                let bx = (other.tile_idx % map_w) as i32;
                let by = (other.tile_idx / map_w) as i32;
                let distance_sq = (ax - bx).pow(2) + (ay - by).pow(2);
                if distance_sq > max_dist_sq
                    || best.is_some_and(|(_, best_distance)| distance_sq >= best_distance)
                    || railway_route_is_land(ax, ay, bx, by, map_w, map_h, terrain).is_none()
                {
                    continue;
                }
                best = Some((other_index, distance_sq));
            }
        }
        let Some((other_index, _)) = best else {
            continue;
        };
        let other = nodes[other_index];
        let (from, to) = if node.id < other.id {
            (*node, other)
        } else {
            (other, *node)
        };
        if !seen.insert((from.id, to.id)) {
            continue;
        }
        let from_x = (from.tile_idx % map_w) as i32;
        let from_y = (from.tile_idx / map_w) as i32;
        let to_x = (to.tile_idx % map_w) as i32;
        let to_y = (to.tile_idx / map_w) as i32;
        let Some(horizontal_first) =
            railway_route_is_land(from_x, from_y, to_x, to_y, map_w, map_h, terrain)
        else {
            continue;
        };
        links.push(RailwayLink {
            from_building_id: from.id,
            to_building_id: to.id,
            horizontal_first,
        });
    }
    links.sort_by_key(|link| (link.from_building_id, link.to_building_id));

    let indices: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect();
    let mut adjacency = vec![Vec::new(); nodes.len()];
    for link in &links {
        let (Some(&from), Some(&to)) = (
            indices.get(&link.from_building_id),
            indices.get(&link.to_building_id),
        ) else {
            continue;
        };
        adjacency[from].push(to);
        adjacency[to].push(from);
    }
    let mut visited = vec![false; nodes.len()];
    for start in 0..nodes.len() {
        if visited[start] {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![start];
        visited[start] = true;
        while let Some(index) = stack.pop() {
            component.push(index);
            for &neighbor in &adjacency[index] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }
        let has_trade_station = component.iter().any(|&index| {
            matches!(nodes[index].kind, BuildingKind::City | BuildingKind::Port)
        });
        if !has_trade_station {
            continue;
        }
        for index in component {
            let factory = nodes[index];
            if factory.kind == BuildingKind::Factory
                && let Some(aggregate) = aggregates.get_mut(factory.owner_id as usize)
            {
                aggregate.railway_connections = aggregate.railway_connections.saturating_add(1);
            }
        }
    }
    links
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefenseInfluence {
    pub capture_multiplier: f64,
    pub priority_bonus: f64,
    pub attacker_loss_multiplier: f64,
}

impl DefenseInfluence {
    const NONE: Self = Self {
        capture_multiplier: 1.0,
        priority_bonus: 0.0,
        attacker_loss_multiplier: 1.0,
    };
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct DefenseTower {
    owner_id: u16,
    x: u32,
    y: u32,
    active_level: u8,
    defense_weight: f64,
    range: i32,
}

/// A spatial grid that limits defense queries to nearby cells.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct DefenseGrid {
    cells: Vec<Vec<DefenseTower>>,
    pub grid_w: u32,
    pub grid_h: u32,
    pub cell_size: u32,
}

pub const DEFENSE_GRID_CELL_SIZE: u32 = 16;
const DEFENSE_OWNER_INDEX_THRESHOLD: usize = 8;

#[inline]
fn defense_sample(match_seed: u64, attacker_id: u16, target_owner: u16, tile_idx: u64) -> f64 {
    stable_tile_sample(match_seed, attacker_id, target_owner, tile_idx, 0)
}

impl DefenseGrid {
    /// Rebuild the grid with the specified player's defense posts.
    /// This is allocation-free after the first few calls because the `cells` array
    /// simply clears its internal `Vec`s without dropping capacity.
    pub fn rebuild(
        &mut self,
        buildings: &[Building],
        map_width: u32,
        map_height: u32,
        cell_size: u32,
        cfg: &crate::game_config::GameConfig,
    ) {
        self.rebuild_with_range(buildings, map_width, map_height, cell_size, cfg, |_| 1.0);
    }

    /// Like [`Self::rebuild`], scaling each tower's range by its owner's
    /// tower-range hero bonus.
    pub fn rebuild_with_range(
        &mut self,
        buildings: &[Building],
        map_width: u32,
        map_height: u32,
        cell_size: u32,
        cfg: &crate::game_config::GameConfig,
        range_multiplier: impl Fn(u16) -> f64,
    ) {
        let grid_w = map_width.div_ceil(cell_size);
        let grid_h = map_height.div_ceil(cell_size);

        self.grid_w = grid_w;
        self.grid_h = grid_h;
        self.cell_size = cell_size;

        let num_cells = (grid_w * grid_h) as usize;
        if self.cells.len() < num_cells {
            self.cells.resize(num_cells, Vec::new());
        }
        for cell in self.cells.iter_mut() {
            cell.clear();
        }

        for &b in buildings {
            if b.kind == BuildingKind::Tower && b.active_level() > 0 {
                let bx = b.tile_idx % map_width;
                let by = b.tile_idx / map_width;
                let cx = bx / cell_size;
                let cy = by / cell_size;
                let active_level = b.active_level();
                let range = tower_range_for_level(active_level, cfg, range_multiplier(b.owner_id));
                if range > 0 && cx < grid_w && cy < grid_h {
                    self.cells[(cy * grid_w + cx) as usize].push(DefenseTower {
                        owner_id: b.owner_id,
                        x: bx,
                        y: by,
                        active_level,
                        defense_weight: tower_defense_weight(active_level, cfg),
                        range,
                    });
                }
            }
        }

        for cell in &mut self.cells[..num_cells] {
            cell.sort_unstable_by_key(|tower| {
                (
                    tower.owner_id,
                    tower.x,
                    tower.y,
                    tower.active_level,
                    tower.range,
                )
            });
        }
    }

    #[inline]
    fn add_weighted_coverage<const NEED_CAPTURE_FALLOFF: bool>(
        tower: &DefenseTower,
        tile_x: u32,
        tile_y: u32,
        weighted_levels: &mut f64,
        strongest_coverage: &mut f64,
    ) {
        let distance = hex_distance(tile_x as i32, tile_y as i32, tower.x as i32, tower.y as i32);
        if tower.range > 0 && distance <= tower.range {
            let coverage = if distance * 4 <= tower.range * 3 {
                1.0
            } else {
                (4 * (tower.range - distance)) as f64 / tower.range as f64
            };
            *weighted_levels += coverage * tower.defense_weight;
            if NEED_CAPTURE_FALLOFF {
                *strongest_coverage = strongest_coverage.max(coverage);
            }
        }
    }

    #[inline]
    fn weighted_coverage<const NEED_CAPTURE_FALLOFF: bool>(
        &self,
        tile_x: u32,
        tile_y: u32,
        map_width: u32,
        target_owner: u16,
    ) -> (f64, f64) {
        if map_width == 0 || self.cell_size == 0 || self.grid_w == 0 {
            return (0.0, 0.0);
        }

        let max_range = MAX_TOWER_REACH;
        let cx_min = tile_x.saturating_sub(max_range) / self.cell_size;
        let cx_max = ((tile_x + max_range) / self.cell_size).min(self.grid_w - 1);
        let cy_min = tile_y.saturating_sub(max_range) / self.cell_size;
        let cy_max = ((tile_y + max_range) / self.cell_size).min(self.grid_h - 1);

        let mut weighted_levels = 0.0;
        let mut strongest_coverage: f64 = 0.0;
        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                let cell = &self.cells[(cy * self.grid_w + cx) as usize];
                if cell.len() < DEFENSE_OWNER_INDEX_THRESHOLD {
                    for tower in cell {
                        if tower.owner_id == target_owner {
                            Self::add_weighted_coverage::<NEED_CAPTURE_FALLOFF>(
                                tower,
                                tile_x,
                                tile_y,
                                &mut weighted_levels,
                                &mut strongest_coverage,
                            );
                        }
                    }
                } else {
                    let owner_start = cell.partition_point(|tower| tower.owner_id < target_owner);
                    for tower in cell[owner_start..]
                        .iter()
                        .take_while(|tower| tower.owner_id == target_owner)
                    {
                        Self::add_weighted_coverage::<NEED_CAPTURE_FALLOFF>(
                            tower,
                            tile_x,
                            tile_y,
                            &mut weighted_levels,
                            &mut strongest_coverage,
                        );
                    }
                }
            }
        }

        (weighted_levels, strongest_coverage)
    }

    /// Calculate only the defense contribution needed to order a frontier tile.
    #[inline]
    pub fn priority_bonus(
        &self,
        tile_x: u32,
        tile_y: u32,
        map_width: u32,
        target_owner: u16,
        attacker_id: u16,
        match_seed: u64,
        cfg: &crate::game_config::GameConfig,
    ) -> f64 {
        let (weighted_levels, _) =
            self.weighted_coverage::<false>(tile_x, tile_y, map_width, target_owner);
        if weighted_levels == 0.0 {
            return 0.0;
        }

        let tile_idx = u64::from(tile_y) * u64::from(map_width) + u64::from(tile_x);
        let sample = defense_sample(match_seed, attacker_id, target_owner, tile_idx);
        cfg.tower_priority * weighted_levels * (0.5 + sample)
    }

    /// Calculate one deterministic, distance-weighted defense sample for a tile.
    #[inline]
    pub fn influence(
        &self,
        tile_x: u32,
        tile_y: u32,
        map_width: u32,
        target_owner: u16,
        attacker_id: u16,
        match_seed: u64,
        cfg: &crate::game_config::GameConfig,
    ) -> DefenseInfluence {
        let (weighted_levels, strongest_coverage) =
            self.weighted_coverage::<true>(tile_x, tile_y, map_width, target_owner);

        if weighted_levels == 0.0 {
            return DefenseInfluence::NONE;
        }

        let tile_idx = u64::from(tile_y) * u64::from(map_width) + u64::from(tile_x);
        let sample = defense_sample(match_seed, attacker_id, target_owner, tile_idx);

        DefenseInfluence {
            capture_multiplier: 1.0 + 3.0 * strongest_coverage * sample,
            priority_bonus: cfg.tower_priority * weighted_levels * (0.5 + sample),
            attacker_loss_multiplier: 1.0
                + (cfg.tower_strength * weighted_levels * (0.5 + sample)).clamp(0.0, 0.50),
        }
    }
}

/// Spatial index cell size. This is not a minimum construction distance.
pub const BUILDING_GRID_CELL_SIZE: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BuildingGridEntry {
    pub x: u32,
    pub y: u32,
    pub kind: BuildingKind,
}

/// Spatial grid of structure tile coordinates `(x, y)` for O(local) minimum-distance checks during placement.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildingGrid {
    /// City positions, kept separate because cities use the larger spacing rule.
    pub cells: Vec<Vec<BuildingGridEntry>>,
    /// Non-city positions, which share the smaller spacing rule.
    pub non_city_cells: Vec<Vec<BuildingGridEntry>>,
    pub grid_w: u32,
    pub grid_h: u32,
    pub cell_size: u32,
    /// When false and `grid_w > 0`, [`DarkRiftEngine::refresh_building_grid`] may skip work.
    pub dirty: bool,
}

impl Default for BuildingGrid {
    fn default() -> Self {
        Self {
            cells: Vec::new(),
            non_city_cells: Vec::new(),
            grid_w: 0,
            grid_h: 0,
            cell_size: BUILDING_GRID_CELL_SIZE,
            dirty: true,
        }
    }
}

impl BuildingGrid {
    /// Grid with no buildings: dimensions and cells initialized, [`Self::dirty`] false.
    pub fn rebuild_empty(map_w: u32, map_h: u32) -> Self {
        let mut g = Self::default();
        g.rebuild_from_pairs(map_w, map_h, &[]);
        g
    }

    #[inline]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Fill grid from all buildings, including those under construction.
    pub fn rebuild<'a>(
        &mut self,
        buildings: impl Iterator<Item = &'a Building>,
        map_w: u32,
        map_h: u32,
    ) {
        let cell_size = BUILDING_GRID_CELL_SIZE;
        let grid_w = map_w.div_ceil(cell_size);
        let grid_h = map_h.div_ceil(cell_size);

        self.grid_w = grid_w;
        self.grid_h = grid_h;
        self.cell_size = cell_size;

        let num_cells = (grid_w * grid_h) as usize;
        if self.cells.len() < num_cells {
            self.cells.resize(num_cells, Vec::new());
        }
        if self.non_city_cells.len() < num_cells {
            self.non_city_cells.resize(num_cells, Vec::new());
        }
        for cell in self.cells.iter_mut() {
            cell.clear();
        }
        for cell in self.non_city_cells.iter_mut() {
            cell.clear();
        }

        for b in buildings {
            let bx = b.tile_idx % map_w;
            let by = b.tile_idx / map_w;
            let cx = bx / cell_size;
            let cy = by / cell_size;
            if cx < grid_w && cy < grid_h {
                let cells = if b.kind == BuildingKind::City {
                    &mut self.cells
                } else {
                    &mut self.non_city_cells
                };
                cells[(cy * grid_w + cx) as usize].push(BuildingGridEntry {
                    x: bx,
                    y: by,
                    kind: b.kind,
                });
            }
        }
        self.dirty = false;
    }

    pub fn insert(&mut self, tile_idx: u32, kind: BuildingKind, map_w: u32, map_h: u32) {
        let grid_w = map_w.div_ceil(self.cell_size);
        let grid_h = map_h.div_ceil(self.cell_size);
        if self.dirty
            || self.grid_w != grid_w
            || self.grid_h != grid_h
            || map_w == 0
            || tile_idx >= map_w.saturating_mul(map_h)
        {
            self.mark_dirty();
            return;
        }

        let x = tile_idx % map_w;
        let y = tile_idx / map_w;
        let cells = if kind == BuildingKind::City {
            &mut self.cells
        } else {
            &mut self.non_city_cells
        };
        cells[((y / self.cell_size) * grid_w + x / self.cell_size) as usize]
            .push(BuildingGridEntry { x, y, kind });
    }

    /// Rebuild from raw tile coordinates (tests / tooling; no `Building` structs).
    pub fn rebuild_from_pairs(&mut self, map_w: u32, map_h: u32, pairs: &[(u32, u32)]) {
        let cell_size = BUILDING_GRID_CELL_SIZE;
        let grid_w = map_w.div_ceil(cell_size);
        let grid_h = map_h.div_ceil(cell_size);
        self.grid_w = grid_w;
        self.grid_h = grid_h;
        self.cell_size = cell_size;
        let num_cells = (grid_w * grid_h) as usize;
        if self.cells.len() < num_cells {
            self.cells.resize(num_cells, Vec::new());
        }
        if self.non_city_cells.len() < num_cells {
            self.non_city_cells.resize(num_cells, Vec::new());
        }
        for cell in self.cells.iter_mut() {
            cell.clear();
        }
        for cell in self.non_city_cells.iter_mut() {
            cell.clear();
        }
        for &(bx, by) in pairs {
            let cx = bx / cell_size;
            let cy = by / cell_size;
            if cx < grid_w && cy < grid_h {
                self.cells[(cy * grid_w + cx) as usize].push(BuildingGridEntry {
                    x: bx,
                    y: by,
                    kind: BuildingKind::City,
                });
            }
        }
        self.dirty = false;
    }

    /// All stored structure positions in cells overlapping the Euclidean disk of radius `range` around `(tile_x, tile_y)`.
    pub fn iter_in_range(
        &self,
        tile_x: u32,
        tile_y: u32,
        range: u32,
    ) -> impl Iterator<Item = BuildingGridEntry> + '_ {
        let cx_min = tile_x.saturating_sub(range) / self.cell_size;
        let cx_max = (tile_x + range) / self.cell_size;
        let cy_min = tile_y.saturating_sub(range) / self.cell_size;
        let cy_max = (tile_y + range) / self.cell_size;
        let cx_max = cx_max.min(self.grid_w.saturating_sub(1));
        let cy_max = cy_max.min(self.grid_h.saturating_sub(1));

        (cy_min..=cy_max).flat_map(move |cy| {
            (cx_min..=cx_max).flat_map(move |cx| {
                let idx = (cy * self.grid_w + cx) as usize;
                self.cells[idx].iter().copied()
            })
        })
    }

    /// All stored non-city positions in cells overlapping the Euclidean disk.
    pub fn iter_non_city_in_range(
        &self,
        tile_x: u32,
        tile_y: u32,
        range: u32,
    ) -> impl Iterator<Item = BuildingGridEntry> + '_ {
        let cx_min = tile_x.saturating_sub(range) / self.cell_size;
        let cx_max = (tile_x + range) / self.cell_size;
        let cy_min = tile_y.saturating_sub(range) / self.cell_size;
        let cy_max = (tile_y + range) / self.cell_size;
        let cx_max = cx_max.min(self.grid_w.saturating_sub(1));
        let cy_max = cy_max.min(self.grid_h.saturating_sub(1));

        (cy_min..=cy_max).flat_map(move |cy| {
            (cx_min..=cx_max).flat_map(move |cx| {
                let idx = (cy * self.grid_w + cx) as usize;
                self.non_city_cells[idx].iter().copied()
            })
        })
    }

    pub fn iter_all_in_range(
        &self,
        tile_x: u32,
        tile_y: u32,
        range: u32,
    ) -> impl Iterator<Item = BuildingGridEntry> + '_ {
        self.iter_in_range(tile_x, tile_y, range)
            .chain(self.iter_non_city_in_range(tile_x, tile_y, range))
    }

    /// Whether a city's footprint covers this tile. The grid must be refreshed first.
    pub fn city_covers_tile(&self, tile_x: u32, tile_y: u32) -> bool {
        self.iter_in_range(tile_x, tile_y, 2).any(|city| {
            BuildingFootprint::at(BuildingKind::City, city.x, city.y)
                .contains(tile_x as i32, tile_y as i32)
        })
    }
}

#[cfg(test)]
mod defense_influence_tests {
    use super::*;

    #[test]
    fn every_tower_reach_fits_the_search_window() {
        for hero in crate::player::Leader::all() {
            let reach = 20.0 * hero.tower_range_multiplier();
            assert!(
                reach <= f64::from(MAX_TOWER_REACH),
                "{hero:?} reaches {reach} tiles, past the search window"
            );
        }
    }

    fn tower(x: u32, y: u32, width: u32, owner_id: u16, level: u8) -> Building {
        Building {
            id: 1,
            owner_id,
            tile_idx: y * width + x,
            kind: BuildingKind::Tower,
            level,
            under_construction: false,
            ticks_until_complete: 0,
        }
    }

    #[test]
    fn influence_matches_weighted_formula_and_stays_repeatable() {
        let width = 64;
        let target_tower = tower(32, 32, width, 2, 2);
        let other_owners = [tower(32, 32, width, 1, 4), tower(32, 32, width, 3, 4)];
        let cfg = crate::game_config::GameConfig::default();
        assert_eq!(target_tower.defense_range_cfg(&cfg), 8);
        let mut grid = DefenseGrid::default();
        grid.rebuild(
            &[target_tower, other_owners[0], other_owners[1]],
            width,
            width,
            DEFENSE_GRID_CELL_SIZE,
            &cfg,
        );

        let seed = 0x1234_5678;
        let center = grid.influence(32, 32, width, 2, 1, seed, &cfg);
        assert_eq!(center, grid.influence(32, 32, width, 2, 1, seed, &cfg));
        let center_sample = defense_sample(seed, 1, 2, 32 * width as u64 + 32);
        assert_eq!(center.capture_multiplier, 1.0 + 3.0 * center_sample);
        assert_eq!(
            center.priority_bonus,
            cfg.tower_priority * 2.0 * (0.5 + center_sample)
        );
        assert_eq!(
            center.priority_bonus,
            grid.priority_bonus(32, 32, width, 2, 1, seed, &cfg)
        );
        assert_eq!(
            center.attacker_loss_multiplier,
            1.0 + (cfg.tower_strength * 2.0 * (0.5 + center_sample)).clamp(0.0, 0.50)
        );
        assert!((1.0..=4.0).contains(&center.capture_multiplier));

        let at_three_quarters = grid.influence(38, 32, width, 2, 1, seed, &cfg);
        let sample = defense_sample(seed, 1, 2, 32 * width as u64 + 38);
        assert_eq!(at_three_quarters.capture_multiplier, 1.0 + 3.0 * sample);

        let fading = grid.influence(39, 32, width, 2, 1, seed, &cfg);
        let sample = defense_sample(seed, 1, 2, 32 * width as u64 + 39);
        assert_eq!(fading.capture_multiplier, 1.0 + 3.0 * 0.5 * sample);
        assert_eq!(fading.priority_bonus, cfg.tower_priority * (0.5 + sample));
        assert_eq!(
            fading.attacker_loss_multiplier,
            1.0 + (cfg.tower_strength * (0.5 + sample)).clamp(0.0, 0.50)
        );

        let edge = grid.influence(40, 32, width, 2, 1, seed, &cfg);
        assert_eq!(edge, DefenseInfluence::NONE);
        let outside = grid.influence(41, 32, width, 2, 1, seed, &cfg);
        assert_eq!(outside, DefenseInfluence::NONE);

        let mut no_towers = DefenseGrid::default();
        no_towers.rebuild(&[], width, width, DEFENSE_GRID_CELL_SIZE, &cfg);
        assert_eq!(
            no_towers.influence(32, 32, width, 2, 1, seed, &cfg),
            DefenseInfluence::NONE
        );
        assert_eq!(
            grid.influence(32, 32, width, 4, 1, seed, &cfg),
            DefenseInfluence::NONE
        );
    }

    #[test]
    fn influence_keeps_the_existing_capped_tower_range() {
        let mut cfg = crate::game_config::GameConfig::default();
        cfg.tower_range = 18.0;
        let tower = tower(32, 32, 64, 2, 4);
        assert_eq!(tower.defense_range_cfg(&cfg), 20);
    }

    #[test]
    fn tower_range_uses_base_level_increments_and_hero_bonus() {
        let cfg = crate::game_config::GameConfig::default();
        assert_eq!(tower_range_for_level(0, &cfg, 1.0), 0);
        assert_eq!(
            (1..=4)
                .map(|level| tower_range_for_level(level, &cfg, 1.0))
                .collect::<Vec<_>>(),
            [6, 8, 10, 12]
        );
        assert_eq!(
            (1..=4)
                .map(|level| tower_range_for_level(level, &cfg, 1.5))
                .collect::<Vec<_>>(),
            [9, 12, 15, 18]
        );
    }
}
