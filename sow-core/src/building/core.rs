use super::placement::{BuildingFootprint, hex_distance};
use crate::game::{BuildingKind, CityArsenalKind};
use crate::rng::stable_tile_sample;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum Euclidean tile distance used when connecting railway stations.
pub const RAILWAY_LINK_DISTANCE: u32 = 72;
/// ponytail: cap station candidates per station at 64; keep nearest candidates when a cell is dense.
pub const RAILWAY_MAX_CANDIDATES_PER_STATION: usize = 64;
/// Maximum rail path length for an edge or a Trading Post delivery route.
pub const RAILWAY_MAX_ROUTE_DISTANCE: u32 = 102;
pub const RAILWAY_STOP_GOLD: f64 = 2.0;
pub const RAILWAY_STOP_GOLD_PER_TRIP_CAP: f64 = 8.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RailwayLink {
    pub from_building_id: u64,
    pub to_building_id: u64,
    pub from_tile: u32,
    pub owner_id: u16,
    pub level: u8,
    #[serde(default)]
    pub distance: u32,
    /// Building and junction node IDs from this Trading Post to the destination.
    #[serde(default)]
    pub station_path: Vec<u64>,
    /// Turn points for the complete trip, retained if the network is rebuilt mid-trip.
    #[serde(default)]
    pub route_path: Vec<u32>,
    /// Lengths of the rail segments between consecutive station_path nodes.
    #[serde(default)]
    pub station_path_edge_distances: Vec<u16>,
    /// Number of active City/Port stops in this itinerary, used to split the per-trip cap.
    #[serde(default)]
    pub station_stop_count: u16,
    pub period_ticks: u32,
    pub opened_at: u64,
}

impl RailwayLink {
    pub fn distance_bonus(&self, _map_width: u32) -> f64 {
        railway_distance_bonus(self.distance)
    }

    pub fn cargo_gold(&self, cfg: &crate::game_config::GameConfig, map_width: u32) -> f64 {
        cfg.per_tick(f64::from(self.period_ticks))
            * cfg.trading_post_gold_income
            * self.weight(map_width)
    }

    pub fn weight(&self, _map_width: u32) -> f64 {
        if self.period_ticks == 0 {
            0.0
        } else {
            trading_post_gold_level_weight(self.level) * self.distance_bonus(0)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RailwayTrack {
    pub from_node_id: u64,
    pub to_node_id: u64,
    /// Turn points along a land path. Endpoints are included.
    pub path: Vec<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeRouteProgress {
    /// Last arrived stop, or route endpoint from which the current leg departed.
    #[serde(default)]
    pub from_station_index: u16,
    pub next_station_index: u16,
    pub returning: bool,
    #[serde(default)]
    pub cargo_loaded: bool,
    pub next_arrival_tick: u64,
    /// Fractional tick carry keeps many stop arrivals aligned to the trip period.
    #[serde(default)]
    pub tick_remainder_micros: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingTradeRoute {
    /// `true` distinguishes a pending removal (`route == None`) from no change.
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub route: Option<RailwayLink>,
}

fn railway_distance_bonus(distance: u32) -> f64 {
    // Linear premium from 1x at zero distance to almost 2x at the legal L-route maximum.
    let max_route_distance = (f64::from(RAILWAY_LINK_DISTANCE) * std::f64::consts::SQRT_2).ceil();
    1.0 + (f64::from(distance) / max_route_distance).clamp(0.0, 1.0)
}

/// Maximum number of warships that can be queued at a single Port.
pub const MAX_WARSHIP_QUEUE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CityArsenalBuild {
    pub kind: CityArsenalKind,
    pub ticks_until_complete: u32,
    pub total_ticks: u32,
    #[serde(default)]
    pub warship_source_tile: Option<u32>,
    #[serde(default)]
    pub warship_target_tile: Option<u32>,
    #[serde(default)]
    pub paid_gold_bits: u64,
}

impl CityArsenalBuild {
    #[inline]
    pub fn paid_gold(self) -> f64 {
        f64::from_bits(self.paid_gold_bits)
    }

    #[inline]
    pub fn set_paid_gold(&mut self, gold: f64) {
        self.paid_gold_bits = gold.to_bits();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CityArsenalState {
    pub has_anti_nuke: bool,
    pub nuke_ready: bool,
    pub arsenal_build: Option<CityArsenalBuild>,
    #[serde(default)]
    pub warship_queue: Vec<CityArsenalBuild>,
}

impl CityArsenalState {
    #[inline]
    pub fn is_building(&self) -> bool {
        self.arsenal_build.is_some() || !self.warship_queue.is_empty()
    }

    #[inline]
    pub fn warship_queue_len(&self) -> usize {
        self.warship_queue.len()
            + usize::from(
                self.arsenal_build
                    .is_some_and(|b| b.kind == CityArsenalKind::Warship),
            )
    }

    #[inline]
    pub fn active_warship_build(&self) -> Option<CityArsenalBuild> {
        self.warship_queue.first().copied().or_else(|| {
            self.arsenal_build
                .filter(|b| b.kind == CityArsenalKind::Warship)
        })
    }

    /// Migrates a legacy single-slot `arsenal_build` Warship into `warship_queue[0]`.
    #[inline]
    pub fn normalize_legacy_warship(&mut self) {
        if let Some(build) = self.arsenal_build
            && build.kind == CityArsenalKind::Warship
        {
            self.arsenal_build = None;
            self.warship_queue.insert(0, build);
        }
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

/// City map art uses its completed tier; levels above four reuse the level-four sprite.
#[inline]
pub fn city_visual_level(buildings: &[Building], tile_idx: u32) -> u8 {
    compute_city_visual_level(
        buildings
            .iter()
            .map(|b| (b.tile_idx, b.kind, b.active_level())),
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
        buildings
            .iter()
            .map(|b| (b.tile_idx, b.kind, b.active_level())),
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
/// Widest reach a tower can have: its 28-tile cap times the strongest tower-range hero (Sun Tzu, 1.5x).
pub(crate) const MAX_TOWER_REACH: u32 = 42;

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
    let through_level_four = i32::from(level.saturating_sub(1).min(3)) * 2;
    let late_upgrade_bonus = i32::from(level.saturating_sub(4)) * 4;
    let range = (base + through_level_four + late_upgrade_bonus).min(28);
    (f64::from(range) * range_multiplier).round() as i32
}

/// Cumulative weights multiplying the configured level-one income rates.
pub fn trading_post_gold_level_weight(level: u8) -> f64 {
    [0.0, 1.0, 3.75, 8.25, 17.25, 65.25, 161.25, 353.25, 737.25][usize::from(level.min(8))]
}

/// Staged cart/train travel speed multiplier per TradingPost level (`1..=8`).
pub fn trading_post_speed_multiplier(level: u8) -> f64 {
    [1.0, 1.0, 1.15, 1.32, 1.52, 1.75, 2.05, 2.45, 2.90][usize::from(level.min(8))]
}

pub const fn cart_stop_multiplier(kind: BuildingKind) -> f64 {
    match kind {
        BuildingKind::TradingPost => 1.50,
        BuildingKind::Port => 1.25,
        BuildingKind::City => 1.00,
        BuildingKind::Farm => 0.75,
        BuildingKind::MilitaryCamp => 0.50,
        BuildingKind::Tower => 0.25,
        BuildingKind::SAM | BuildingKind::MissileSilo => 0.0,
    }
}

pub fn military_camp_troop_level_weight(level: u8) -> f64 {
    [0.0, 1.0, 3.5, 8.0, 17.0, 53.0, 125.0, 269.0, 557.0][usize::from(level.min(8))]
}

pub fn farm_troop_level_weight(level: u8) -> f64 {
    [0.0, 1.0, 3.0, 6.5, 13.5, 38.0, 88.0, 188.0, 388.0][usize::from(level.min(8))]
}

pub fn port_income_level_weight(level: u8) -> f64 {
    [0.0, 1.0, 3.5, 8.0, 17.0, 49.0, 141.0, 353.0, 777.0][usize::from(level.min(8))]
}

/// City capacity weights use stronger marginal gains than city income so upgrades
/// keep the City's reserve advantage over newly built level-one structures.
pub fn city_capacity_level_weight(level: u8) -> f64 {
    [0.0, 1.0, 3.5, 7.25, 13.25, 43.25, 103.25, 223.25, 463.25][usize::from(level.min(8))]
}

pub fn city_upgrade_reinforcements(level: u8, base_capacity: f64) -> f64 {
    match level {
        5 => return 100_000.0,
        6 => return 350_000.0,
        7 => return 1_000_000.0,
        8 => return 2_000_000.0,
        _ => {}
    }
    let share = match level {
        2 => 0.20,
        3 => 0.30,
        4 => 0.40,
        _ => return 0.0,
    };
    let capacity_added = city_capacity_level_weight(level) - city_capacity_level_weight(level - 1);
    share * capacity_added * base_capacity
}

pub fn tower_capture_weight(level: u8) -> f64 {
    [0.0, 1.0, 1.70, 2.55, 3.75, 5.0, 7.0, 10.0, 14.0][usize::from(level.min(8))]
}

pub const NUCLEAR_CITY_LEVEL: u8 = 6;

pub fn nuclear_city_level(campaign: bool) -> u8 {
    if campaign { 4 } else { NUCLEAR_CITY_LEVEL }
}

pub fn camp_capture_level_weight(level: u8) -> f64 {
    [0.0, 2.0, 4.0, 6.5, 9.5, 13.0, 16.5, 20.5, 25.0][usize::from(level.min(8))]
}

pub fn camp_conquest_multiplier(points: f64) -> f64 {
    1.0 + points.max(0.0) / 300.0
}

pub fn player_has_nuclear_launch_city(
    buildings: &[Building],
    owner_id: u16,
    campaign: bool,
) -> bool {
    has_nuclear_launch_city(
        buildings
            .iter()
            .map(|building| (building.owner_id, building.kind, building.active_level())),
        owner_id,
        campaign,
    )
}

pub fn snapshot_has_nuclear_launch_city(
    buildings: &[crate::protocol::BuildingSnapshot],
    owner_id: u16,
    campaign: bool,
) -> bool {
    has_nuclear_launch_city(
        buildings
            .iter()
            .map(|building| (building.owner_id, building.kind, building.active_level())),
        owner_id,
        campaign,
    )
}

fn has_nuclear_launch_city(
    buildings: impl Iterator<Item = (u16, BuildingKind, u8)>,
    owner_id: u16,
    _campaign: bool,
) -> bool {
    buildings.into_iter().any(|(owner, kind, level)| {
        owner == owner_id
            && ((kind == BuildingKind::MilitaryCamp && level >= 5)
                || (kind == BuildingKind::MissileSilo && level > 0))
    })
}

pub fn city_income_level_weight(level: u8) -> f64 {
    [0.0, 1.0, 2.5, 4.75, 8.125, 14.875, 28.375, 55.375, 109.375][usize::from(level.min(8))]
}

/// Gold progression keeps N2 at its current weight, then grows 2.2x per upgrade
/// so a higher-tier City beats two buildings of the previous tier for gold output.
pub fn city_gold_level_weight(level: u8) -> f64 {
    gold_upgrade_level_weight(level)
}

/// The Camp's secondary gold income follows the same upgrade curve as City gold.
pub fn military_camp_gold_level_weight(level: u8) -> f64 {
    gold_upgrade_level_weight(level)
}

pub fn farm_gold_level_weight(level: u8) -> f64 {
    gold_upgrade_level_weight(level)
}

fn gold_upgrade_level_weight(level: u8) -> f64 {
    match level.min(8) {
        0 => 0.0,
        1 => 1.0,
        2 => 2.5,
        level => 2.5 * 2.2_f64.powi(i32::from(level - 2)),
    }
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

/// Role-weighted troop capacity multiplier for districts that house troops.
pub fn district_capacity_weight_for_kind(kind: BuildingKind, level: u8) -> f64 {
    let base = income_level_weight(level);
    match kind {
        BuildingKind::MilitaryCamp => base * 2.0,
        BuildingKind::Farm => base * 1.25,
        BuildingKind::Port => base * 0.5,
        _ => 0.0,
    }
}

/// Per-player totals for income / fleet gates (only **ready** structures count).
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BuildingAggregate {
    pub highest_city_level: u8,
    pub camp_conquest_weight: f64,
    pub city_levels: u32,
    pub tower_levels: u32,
    pub trading_post_levels: u32,
    pub port_levels: u32,
    pub port_fleet_slots: u32,
    pub military_camp_levels: u32,
    pub farm_levels: u32,
    pub sam_levels: u32,
    pub silo_levels: u32,
    pub military_camp_troop_level_weight: f64,
    pub military_camp_gold_level_weight: f64,
    pub farm_troop_level_weight: f64,
    pub farm_gold_level_weight: f64,
    pub port_income_level_weight: f64,
    /// Sum of the Trading Post's gold-specific marginal weights.
    pub trading_post_income_level_weight: f64,
    /// Sum of 1.5x marginal level weights across active Cities.
    pub city_income_level_weight: f64,
    /// Sum of City gold weights, kept separate from troop production.
    pub city_gold_level_weight: f64,
    /// City troop capacity continues growing beyond the level-four income cap.
    pub city_capacity_level_weight: f64,
    /// Port levels used by hero-only income bonuses, capped at level four each.
    pub port_income_levels: u32,
    /// Shared small capacity bonus from completed non-City upgrades.
    pub district_capacity_level_weight: f64,
    pub has_completed_port: bool,
    /// Ready Cities, retained for AI and railway calculations.
    pub ready_city_count: u32,
    /// Ready trading_posts only.
    pub ready_trading_post_count: u32,
    /// Total instances per kind, **including** under construction (for bot build quotas).
    pub count_city: u32,
    pub count_tower: u32,
    pub count_trading_post: u32,
    pub count_port: u32,
    pub count_military_camp: u32,
    pub count_farm: u32,
    pub count_sam: u32,
    pub count_silo: u32,
    /// Number of owned TradingPost stations connected to another building over land.
    pub railway_connections: u32,
    pub land_trade_income_weight: f64,
}

impl BuildingAggregate {
    #[inline]
    pub fn total_structures_of_kind(self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::City => self.count_city,
            BuildingKind::Tower => self.count_tower,
            BuildingKind::TradingPost => self.count_trading_post,
            BuildingKind::Port => self.count_port,
            BuildingKind::MilitaryCamp => self.count_military_camp,
            BuildingKind::Farm => self.count_farm,
            BuildingKind::SAM => self.count_sam,
            BuildingKind::MissileSilo => self.count_silo,
        }
    }

    /// Sum of active levels for `kind` (matches `count_kind` / cost scaling).
    #[inline]
    pub fn levels_of_kind(self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::City => self.city_levels,
            BuildingKind::Tower => self.tower_levels,
            BuildingKind::TradingPost => self.trading_post_levels,
            BuildingKind::Port => self.port_levels,
            BuildingKind::MilitaryCamp => self.military_camp_levels,
            BuildingKind::Farm => self.farm_levels,
            BuildingKind::SAM => self.sam_levels,
            BuildingKind::MissileSilo => self.silo_levels,
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
            BuildingKind::TradingPost => a.count_trading_post += 1,
            BuildingKind::Port => a.count_port += 1,
            BuildingKind::MilitaryCamp => a.count_military_camp += 1,
            BuildingKind::Farm => a.count_farm += 1,
            BuildingKind::SAM => a.count_sam += 1,
            BuildingKind::MissileSilo => a.count_silo += 1,
        }
        let active_lvl = b.active_level();
        if active_lvl == 0 {
            continue;
        }
        let a = &mut out[i];
        if b.kind.grants_building_troop_capacity() {
            a.district_capacity_level_weight += district_capacity_weight_for_kind(b.kind, active_lvl);
        }
        match b.kind {
            BuildingKind::City => {
                a.highest_city_level = a.highest_city_level.max(active_lvl);
                a.city_levels += active_lvl as u32;
                a.city_income_level_weight += city_income_level_weight(active_lvl);
                a.city_gold_level_weight += city_gold_level_weight(active_lvl);
                a.city_capacity_level_weight += city_capacity_level_weight(active_lvl);
                a.ready_city_count += 1;
            }
            BuildingKind::Tower => {
                a.tower_levels += active_lvl as u32;
                if active_lvl >= 5 {
                    a.sam_levels += u32::from(active_lvl - 4);
                    a.count_sam += 1;
                }
            }
            BuildingKind::TradingPost => {
                a.trading_post_levels += active_lvl as u32;
                a.trading_post_income_level_weight += trading_post_gold_level_weight(active_lvl);
                a.ready_trading_post_count += 1;
            }
            BuildingKind::Port => {
                a.port_levels += active_lvl as u32;
                a.port_income_level_weight += port_income_level_weight(active_lvl);
                a.port_income_levels += u32::from(active_lvl.min(4));
                a.port_fleet_slots += super::cost::port_fleet_slots(active_lvl);
                a.has_completed_port = true;
            }
            BuildingKind::MilitaryCamp => {
                a.camp_conquest_weight += camp_capture_level_weight(active_lvl);
                a.military_camp_levels += active_lvl as u32;
                a.military_camp_troop_level_weight += military_camp_troop_level_weight(active_lvl);
                a.military_camp_gold_level_weight += military_camp_gold_level_weight(active_lvl);
                if active_lvl >= 5 {
                    a.silo_levels += u32::from(active_lvl - 4);
                    a.count_silo += 1;
                }
            }
            BuildingKind::Farm => {
                a.farm_levels += active_lvl as u32;
                a.farm_troop_level_weight += farm_troop_level_weight(active_lvl);
                a.farm_gold_level_weight += farm_gold_level_weight(active_lvl);
            }
            BuildingKind::SAM => a.sam_levels += u32::from(active_lvl),
            BuildingKind::MissileSilo => a.silo_levels += u32::from(active_lvl),
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

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct DefenseTower {
    owner_id: u16,
    x: u32,
    y: u32,
    active_level: u8,
    defense_weight: f64,
    capture_weight: f64,
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
                        capture_weight: tower_capture_weight(active_level),
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
                *strongest_coverage = strongest_coverage.max(coverage * tower.capture_weight);
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
        assert_eq!(center.capture_multiplier, 1.0 + 3.0 * 1.70 * center_sample);
        assert_eq!(
            center.priority_bonus,
            cfg.tower_priority * 2.5 * (0.5 + center_sample)
        );
        assert_eq!(
            center.priority_bonus,
            grid.priority_bonus(32, 32, width, 2, 1, seed, &cfg)
        );
        assert_eq!(
            center.attacker_loss_multiplier,
            1.0 + (cfg.tower_strength * 2.5 * (0.5 + center_sample)).clamp(0.0, 0.50)
        );
        assert!((1.0..=6.1).contains(&center.capture_multiplier));

        let at_three_quarters = grid.influence(38, 32, width, 2, 1, seed, &cfg);
        let sample = defense_sample(seed, 1, 2, 32 * width as u64 + 38);
        assert_eq!(
            at_three_quarters.capture_multiplier,
            1.0 + 3.0 * 1.70 * sample
        );

        let fading = grid.influence(39, 32, width, 2, 1, seed, &cfg);
        let sample = defense_sample(seed, 1, 2, 32 * width as u64 + 39);
        assert_eq!(fading.capture_multiplier, 1.0 + 3.0 * (0.5 * 1.70) * sample);
        assert_eq!(
            fading.priority_bonus,
            cfg.tower_priority * 1.25 * (0.5 + sample)
        );
        assert_eq!(
            fading.attacker_loss_multiplier,
            1.0 + (cfg.tower_strength * 1.25 * (0.5 + sample)).clamp(0.0, 0.50)
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

#[cfg(test)]
mod trading_post_route_tests {
    use super::*;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
    use crate::map::MapTile;

    fn land_state(w: u32, h: u32) -> GameState {
        let mut state = GameState::new(1, w, h, GameConfig::default());
        for y in 0..h {
            for x in 0..w {
                state.map.set_owner_id(x, y, 1);
                let ref_id = state.map.ref_id(x, y);
                state.map.terrain[ref_id] = MapTile::from_byte(0b1000_0000);
            }
        }
        state
    }

    fn station(id: u64, kind: BuildingKind, x: u32, y: u32, w: u32, level: u8) -> Building {
        Building {
            id,
            owner_id: 1,
            tile_idx: y * w + x,
            kind,
            level,
            under_construction: false,
            ticks_until_complete: 0,
        }
    }

    fn build_test_routes(stations: &[Building], state: &GameState) -> Vec<RailwayLink> {
        let mut track_index = HashMap::new();
        let mut index_ready = false;
        let tracks = crate::building::populate_railway_network(
            stations,
            &[],
            state,
            &[],
            &mut track_index,
            &mut index_ready,
        );
        crate::building::populate_trading_post_routes(stations, &tracks, state, &[])
    }

    #[test]
    fn trading_post_routes_use_cities_ports_and_fallback_posts() {
        let state = land_state(32, 32);
        for target_kind in [
            BuildingKind::City,
            BuildingKind::Port,
            BuildingKind::TradingPost,
        ] {
            let stations = [
                station(10, BuildingKind::TradingPost, 8, 8, 32, 1),
                station(20, target_kind, 18, 8, 32, 1),
            ];
            let links = build_test_routes(&stations, &state);
            let link = links
                .iter()
                .find(|link| link.from_building_id == 10)
                .unwrap_or_else(|| panic!("expected link from TradingPost to {target_kind:?}"));
            assert_eq!(link.to_building_id, 20);
        }

        let mut coast = land_state(32, 20);
        for y in 7..=13 {
            let tile = coast.map.ref_id(13, y);
            coast.map.terrain[tile] = MapTile::from_byte(0);
        }
        let stations = [
            station(10, BuildingKind::TradingPost, 4, 10, 32, 1),
            station(20, BuildingKind::City, 24, 10, 32, 1),
        ];
        let link = build_test_routes(&stations, &coast)
            .into_iter()
            .find(|link| link.from_building_id == 10)
            .unwrap();
        assert!(
            link.distance > 20,
            "the shared route should go around the water barrier"
        );

        let prebuilt = [
            station(20, BuildingKind::City, 18, 8, 32, 1),
            station(30, BuildingKind::Port, 26, 8, 32, 1),
        ];
        let mut track_index = HashMap::new();
        let mut index_ready = false;
        let existing_tracks = crate::building::populate_railway_network(
            &prebuilt,
            &[],
            &state,
            &[],
            &mut track_index,
            &mut index_ready,
        );
        let all_stations = [
            station(10, BuildingKind::TradingPost, 8, 8, 32, 1),
            prebuilt[0],
            prebuilt[1],
        ];
        let connected_tracks = crate::building::populate_railway_network(
            &all_stations,
            &prebuilt,
            &state,
            &existing_tracks,
            &mut track_index,
            &mut index_ready,
        );
        let link = crate::building::populate_trading_post_routes(
            &all_stations,
            &connected_tracks,
            &state,
            &[],
        )
        .into_iter()
        .find(|link| link.from_building_id == 10)
        .expect("new Trading Post should activate prebuilt stations");
        assert_eq!(link.to_building_id, 30);
        assert_eq!(link.station_path, [10, 20, 30]);
    }

    #[test]
    fn trading_post_prefers_commercial_stations_over_sibling_posts() {
        let state = land_state(32, 32);
        let stations = [
            station(1, BuildingKind::TradingPost, 8, 8, 32, 1),
            station(2, BuildingKind::TradingPost, 12, 8, 32, 1),
            station(3, BuildingKind::City, 18, 8, 32, 1),
        ];
        let links = build_test_routes(&stations, &state);
        let post1 = links
            .iter()
            .find(|link| link.from_building_id == 1)
            .unwrap();
        assert_eq!(post1.to_building_id, 3);
    }

    #[test]
    fn trading_post_speed_stages_across_levels_without_changing_gold_rate() {
        let state = land_state(32, 32);
        let cfg = &state.config;
        let mut prev_period = u32::MAX;
        for level in 1..=8 {
            let stations = [
                station(1, BuildingKind::TradingPost, 8, 8, 32, level),
                station(2, BuildingKind::City, 18, 8, 32, 1),
            ];
            let links = build_test_routes(&stations, &state);
            assert_eq!(links.len(), 1);
            let link = &links[0];
            assert!(
                link.period_ticks < prev_period,
                "level {level} period {} must be faster than previous {prev_period}",
                link.period_ticks
            );
            prev_period = link.period_ticks;
            let expected_gold = cfg.per_tick(f64::from(link.period_ticks))
                * cfg.trading_post_gold_income
                * trading_post_gold_level_weight(level);
            assert!(
                (link.cargo_gold(cfg, state.map.width)
                    - expected_gold * link.distance_bonus(state.map.width))
                .abs()
                    < 1e-9
            );
        }
        assert!(prev_period >= 60);
    }

    #[test]
    fn military_camp_capture_scales_per_tier_and_stacks_without_cap() {
        let default_points = crate::game_config::GameConfig::default().camp_capture_base_points;
        assert_eq!(default_points, 0.0);
        for level in 1..=8 {
            assert_eq!(
                camp_conquest_multiplier(camp_capture_level_weight(level) * default_points),
                1.0
            );
        }

        // Explicitly configured games retain the camp capture scaling behavior.
        let base_points = 3.0;
        let expected_pcts = [2.0, 4.0, 6.5, 9.5, 13.0, 16.5, 20.5, 25.0];
        let mut prev_mult = 1.0;
        for (idx, expected_pct) in expected_pcts.iter().enumerate() {
            let level = (idx + 1) as u8;
            let mult = camp_conquest_multiplier(camp_capture_level_weight(level) * base_points);
            assert!(
                mult > prev_mult,
                "level {level} multiplier {mult} must exceed previous {prev_mult}"
            );
            prev_mult = mult;
            let pct = (mult - 1.0) * 100.0;
            assert!(
                (pct - expected_pct).abs() < 1e-9,
                "level {level}: expected +{expected_pct}%, got +{pct}%"
            );
        }
        // Multiple N8 camps stack linearly with no ceiling.
        let two_n8 = camp_conquest_multiplier(2.0 * camp_capture_level_weight(8) * base_points);
        let four_n8 = camp_conquest_multiplier(4.0 * camp_capture_level_weight(8) * base_points);
        assert!((two_n8 - 1.50).abs() < 1e-9);
        assert!((four_n8 - 2.00).abs() < 1e-9);
    }
}
