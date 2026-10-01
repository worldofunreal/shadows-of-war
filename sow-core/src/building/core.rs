use super::placement::hex_distance;
use crate::game::BuildingKind;
use serde::{Deserialize, Serialize};
use wyrand::WyRand;

#[inline]
pub fn upgrade_duration_ticks(kind: BuildingKind, target_level: u8) -> u32 {
    let base_dur = kind.construction_duration_ticks();
    let mut dur = base_dur;
    for _ in 1..target_level {
        dur = (dur as f64 * 1.1) as u32;
    }
    dur.max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModuleKind {
    Port,
    Foundry,
    Armory,
    Intel,
    Arsenal,
    Shield,
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 6] = [
        ModuleKind::Port,
        ModuleKind::Foundry,
        ModuleKind::Armory,
        ModuleKind::Intel,
        ModuleKind::Arsenal,
        ModuleKind::Shield,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ModuleKind::Port => "Port",
            ModuleKind::Foundry => "Foundry",
            ModuleKind::Armory => "Armory",
            ModuleKind::Intel => "Intel",
            ModuleKind::Arsenal => "Arsenal",
            ModuleKind::Shield => "Shield",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CityModules {
    pub port: u8,
    pub foundry: u8,
    pub armory: u8,
    pub intel: u8,
    pub arsenal: u8,
    pub shield: u8,
}

impl CityModules {
    pub fn get_level(&self, kind: ModuleKind) -> u8 {
        match kind {
            ModuleKind::Port => self.port,
            ModuleKind::Foundry => self.foundry,
            ModuleKind::Armory => self.armory,
            ModuleKind::Intel => self.intel,
            ModuleKind::Arsenal => self.arsenal,
            ModuleKind::Shield => self.shield,
        }
    }

    pub fn set_level(&mut self, kind: ModuleKind, level: u8) {
        match kind {
            ModuleKind::Port => self.port = level,
            ModuleKind::Foundry => self.foundry = level,
            ModuleKind::Armory => self.armory = level,
            ModuleKind::Intel => self.intel = level,
            ModuleKind::Arsenal => self.arsenal = level,
            ModuleKind::Shield => self.shield = level,
        }
    }
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
    pub modules: CityModules,
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
        if self.kind == BuildingKind::Bunker && !self.under_construction {
            (cfg.bunker_range.round() as i32 + (self.active_level().saturating_sub(1) as i32 * 2))
                .min(20)
        } else {
            0
        }
    }
}

/// Per-player totals for income / fleet gates (only **ready** structures count).
#[derive(Clone, Copy, Debug, Default)]
pub struct BuildingAggregate {
    pub city_levels: u32,
    pub bunker_levels: u32,
    pub factory_levels: u32,
    pub port_levels: u32,
    pub farm_levels: u32,
    pub farm_slots: u32,
    pub factory_time_levels: u32,
    pub factory_upgrade_discount_levels: u32,
    pub factory_trade_income_levels: u32,
    pub foundry_levels: u32,
    pub armory_levels: u32,
    pub intel_levels: u32,
    pub arsenal_levels: u32,
    pub shield_levels: u32,
    pub has_completed_port: bool,
    /// Ready cities only (for bot `city_equivalent` base).
    pub ready_city_count: u32,
    /// Ready factories only.
    pub ready_factory_count: u32,
    /// Total instances per kind, **including** under construction (for bot build quotas).
    pub count_city: u32,
    pub count_bunker: u32,
    pub count_factory: u32,
    pub count_port: u32,
    pub count_farm: u32,
}

impl BuildingAggregate {
    #[inline]
    pub fn total_structures_of_kind(self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::City => self.count_city,
            BuildingKind::Bunker => self.count_bunker,
            BuildingKind::Factory => self.count_factory,
            BuildingKind::Port => self.count_port,
            BuildingKind::Farm => self.count_farm,
        }
    }

    /// Sum of active levels for `kind` (matches `count_kind` / cost scaling).
    #[inline]
    pub fn levels_of_kind(self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::City => self.city_levels,
            BuildingKind::Bunker => self.bunker_levels,
            BuildingKind::Factory => self.factory_levels,
            BuildingKind::Port => self.port_levels,
            BuildingKind::Farm => self.farm_levels,
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
            BuildingKind::Bunker => a.count_bunker += 1,
            BuildingKind::Factory => a.count_factory += 1,
            BuildingKind::Port => a.count_port += 1,
            BuildingKind::Farm => a.count_farm += 1,
        }
        let active_lvl = b.active_level();
        if active_lvl == 0 {
            continue;
        }
        let a = &mut out[i];
        match b.kind {
            BuildingKind::City => {
                a.city_levels += active_lvl as u32;
                a.ready_city_count += 1;
                a.farm_slots += crate::building::cost::farm_slots_for_city_level(active_lvl);
            }
            BuildingKind::Bunker => {
                a.bunker_levels += active_lvl as u32;
            }
            BuildingKind::Factory => {
                a.factory_levels += active_lvl as u32;
                a.ready_factory_count += 1;
                a.factory_time_levels += u32::from(active_lvl >= 2);
                a.factory_upgrade_discount_levels += u32::from(active_lvl >= 3);
                a.factory_trade_income_levels += u32::from(active_lvl >= 4);
            }
            BuildingKind::Port => {
                a.port_levels += active_lvl as u32;
                a.has_completed_port = true;
            }
            BuildingKind::Farm => {
                a.farm_levels += active_lvl as u32;
            }
        }
    }
    out
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

#[derive(Clone, Copy)]
struct DefenseTower {
    owner_id: u16,
    x: u32,
    y: u32,
    active_level: u8,
    range: i32,
}

/// A spatial grid that limits defense queries to nearby cells.
#[derive(Default, Clone)]
pub struct DefenseGrid {
    cells: Vec<Vec<DefenseTower>>,
    pub grid_w: u32,
    pub grid_h: u32,
    pub cell_size: u32,
}

pub const DEFENSE_GRID_CELL_SIZE: u32 = 16;

#[inline]
fn defense_sample(
    match_seed: u64,
    attacker_id: u16,
    target_owner: u16,
    tile_idx: u64,
) -> f64 {
    let key = match_seed
        .wrapping_add((attacker_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((target_owner as u64).wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add(tile_idx.wrapping_mul(0x94D0_49BB_1331_11EB));
    let mut rng = WyRand::new(key);
    (rng.rand() >> 11) as f64 / 9_007_199_254_740_992.0
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
            if b.kind == BuildingKind::Bunker && b.active_level() > 0 {
                let bx = b.tile_idx % map_width;
                let by = b.tile_idx / map_width;
                let cx = bx / cell_size;
                let cy = by / cell_size;
                let active_level = b.active_level();
                let range = b.defense_range_cfg(cfg);
                if range > 0 && cx < grid_w && cy < grid_h {
                    self.cells[(cy * grid_w + cx) as usize].push(DefenseTower {
                        owner_id: b.owner_id,
                        x: bx,
                        y: by,
                        active_level,
                        range,
                    });
                }
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

        let max_range = 20;
        let cx_min = tile_x.saturating_sub(max_range) / self.cell_size;
        let cx_max = ((tile_x + max_range) / self.cell_size).min(self.grid_w - 1);
        let cy_min = tile_y.saturating_sub(max_range) / self.cell_size;
        let cy_max = ((tile_y + max_range) / self.cell_size).min(self.grid_h - 1);

        let mut weighted_levels = 0.0;
        let mut strongest_coverage: f64 = 0.0;
        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                for b in &self.cells[(cy * self.grid_w + cx) as usize] {
                    if b.owner_id != target_owner {
                        continue;
                    }
                    let d = hex_distance(tile_x as i32, tile_y as i32, b.x as i32, b.y as i32);
                    let range = b.range;
                    if range > 0 && d <= range {
                        let coverage = if d * 4 <= range * 3 {
                            1.0
                        } else {
                            (4 * (range - d)) as f64 / range as f64
                        };
                        weighted_levels += coverage * b.active_level as f64;
                        if NEED_CAPTURE_FALLOFF {
                            strongest_coverage = strongest_coverage.max(coverage);
                        }
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
        cfg.bunker_priority * weighted_levels * (0.5 + sample)
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
            priority_bonus: cfg.bunker_priority * weighted_levels * (0.5 + sample),
            attacker_loss_multiplier: 1.0
                + (cfg.bunker_strength * weighted_levels * (0.5 + sample)).clamp(0.0, 0.50),
        }
    }
}

/// Cell side length for spatial indexing of structure centers. Must match `STRUCTURE_MIN_DIST` in `placement.rs`.
pub const BUILDING_GRID_CELL_SIZE: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingGridEntry {
    pub x: u32,
    pub y: u32,
    pub kind: BuildingKind,
}

/// Spatial grid of structure tile coordinates `(x, y)` for O(local) minimum-distance checks during placement.
#[derive(Clone)]
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
}

#[cfg(test)]
mod defense_influence_tests {
    use super::*;

    fn tower(x: u32, y: u32, width: u32, owner_id: u16, level: u8) -> Building {
        Building {
            id: 1,
            owner_id,
            tile_idx: y * width + x,
            kind: BuildingKind::Bunker,
            level,
            under_construction: false,
            ticks_until_complete: 0,
            modules: CityModules::default(),
        }
    }

    #[test]
    fn influence_matches_weighted_formula_and_stays_repeatable() {
        let width = 64;
        let tower = tower(32, 32, width, 2, 2);
        let cfg = crate::game_config::GameConfig::default();
        assert_eq!(tower.defense_range_cfg(&cfg), 16);
        let mut grid = DefenseGrid::default();
        grid.rebuild(&[tower], width, width, DEFENSE_GRID_CELL_SIZE, &cfg);

        let seed = 0x1234_5678;
        let center = grid.influence(32, 32, width, 2, 1, seed, &cfg);
        assert_eq!(center, grid.influence(32, 32, width, 2, 1, seed, &cfg));
        let center_sample = defense_sample(seed, 1, 2, 32 * width as u64 + 32);
        assert_eq!(center.capture_multiplier, 1.0 + 3.0 * center_sample);
        assert_eq!(
            center.priority_bonus,
            cfg.bunker_priority * 2.0 * (0.5 + center_sample)
        );
        assert_eq!(
            center.priority_bonus,
            grid.priority_bonus(32, 32, width, 2, 1, seed, &cfg)
        );
        assert_eq!(
            center.attacker_loss_multiplier,
            1.0 + (cfg.bunker_strength * 2.0 * (0.5 + center_sample)).clamp(0.0, 0.50)
        );
        assert!((1.0..=4.0).contains(&center.capture_multiplier));

        let at_three_quarters = grid.influence(44, 32, width, 2, 1, seed, &cfg);
        let sample = defense_sample(seed, 1, 2, 32 * width as u64 + 44);
        assert_eq!(at_three_quarters.capture_multiplier, 1.0 + 3.0 * sample);

        let fading = grid.influence(45, 32, width, 2, 1, seed, &cfg);
        let sample = defense_sample(seed, 1, 2, 32 * width as u64 + 45);
        assert_eq!(fading.capture_multiplier, 1.0 + 3.0 * 0.75 * sample);
        assert_eq!(
            fading.priority_bonus,
            cfg.bunker_priority * 1.5 * (0.5 + sample)
        );
        assert_eq!(
            fading.attacker_loss_multiplier,
            1.0 + (cfg.bunker_strength * 1.5 * (0.5 + sample)).clamp(0.0, 0.50)
        );

        let edge = grid.influence(48, 32, width, 2, 1, seed, &cfg);
        assert_eq!(edge, DefenseInfluence::NONE);
        let outside = grid.influence(49, 32, width, 2, 1, seed, &cfg);
        assert_eq!(outside, DefenseInfluence::NONE);

        let mut no_towers = DefenseGrid::default();
        no_towers.rebuild(&[], width, width, DEFENSE_GRID_CELL_SIZE, &cfg);
        assert_eq!(
            no_towers.influence(32, 32, width, 2, 1, seed, &cfg),
            DefenseInfluence::NONE
        );
        assert_eq!(
            grid.influence(32, 32, width, 3, 1, seed, &cfg),
            DefenseInfluence::NONE
        );
    }

    #[test]
    fn influence_keeps_the_existing_capped_tower_range() {
        let cfg = crate::game_config::GameConfig::default();
        let tower = tower(32, 32, 64, 2, 4);
        assert_eq!(tower.defense_range_cfg(&cfg), 20);
    }
}
