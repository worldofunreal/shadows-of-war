use crate::building::{
    CityAnchor, city_anchors_from_buildings, city_construction_radius, find_structure_site_near,
    nearest_match_structure_site_in_bounds, structure_build_cost_gold, validate_match_structure_site,
};
use crate::engine::SowEngine;

use crate::game::BuildingKind;
use crate::protocol::GameplayIntent;

use super::profile::{AiTier, BotDecision, BotDecisionKind};

pub(super) fn bot_structure_target_count(
    kind: BuildingKind,
    city_equivalent: u32,
    bot_iq: u32,
) -> u32 {
    if !kind.is_enabled() {
        return 0;
    }
    let factor = if bot_iq >= 130 {
        1.0
    } else if bot_iq >= 100 {
        0.5
    } else {
        0.4
    };

    match kind {
        BuildingKind::Tower => {
            let val = ((city_equivalent as f64) * 0.35 * factor).floor() as u32;
            if bot_iq < 100 { val.max(1) } else { val }
        }
        BuildingKind::City => {
            let base = (city_equivalent.saturating_add(2) as f64) * factor;
            (base.floor() as u32).max(1)
        }
        BuildingKind::TradingPost => {
            let val = ((city_equivalent as f64) * 0.55 * factor).floor() as u32;
            if factor >= 0.4 { val.max(1) } else { val }
        }
        BuildingKind::Farm => {
            let val = ((city_equivalent as f64) * 0.55 * factor).floor() as u32;
            if factor >= 0.4 { val.max(1) } else { val }
        }
        BuildingKind::Port => {
            let val = ((city_equivalent as f64) * 0.30 * factor).floor() as u32;
            if factor >= 0.4 { val.max(1) } else { val }
        }
        BuildingKind::MilitaryCamp => {
            let val = ((city_equivalent as f64) * 0.40 * factor).floor() as u32;
            if factor >= 0.4 { val.max(1) } else { val }
        }
        BuildingKind::SAM | BuildingKind::MissileSilo => 0,
    }
}

/// One construction or arsenal order per bot decision cycle avoids competing for a city queue.
pub(super) fn has_pending_build(decisions: &[BotDecision], bot_id: u16) -> bool {
    decisions.iter().rev()
        .take_while(|decision| decision.bot_id == bot_id)
        .any(|decision| decision.kind == BotDecisionKind::Build)
}

/// Decision window per tier from the match's tuning, in ticks, inclusive.
/// `under_attack` only applies to tiers that react (Nation, Ghost).
pub(super) fn think_window_ticks(
    ai: &crate::game_config::AiTuning,
    tier: AiTier,
    under_attack: bool,
) -> (u64, u64) {
    let [min, max] = match (tier, under_attack) {
        (AiTier::Tribe, _) => ai.think_tribe,
        (AiTier::Nation, false) => ai.think_nation,
        (AiTier::Nation, true) => ai.think_nation_attacked,
        (AiTier::Ghost, false) => ai.think_ghost,
        (AiTier::Ghost, true) => ai.think_ghost_attacked,
    };
    (min, max)
}

pub(super) fn stack_build_decision(
    engine: &SowEngine,
    bot_id: u16,
    kind: BuildingKind,
    player_gold: f64,
    max_level: u8,
) -> Option<BotDecision> {
    let mut best: Option<(u8, u64, &crate::building::Building)> = None;
    for b in &engine.buildings {
        if b.owner_id != bot_id
            || b.kind != kind
            || b.under_construction
            || engine
                .building_productions
                .get(&b.id)
                .is_some_and(|state| state.is_building())
        {
            continue;
        }
        let cap = max_level.min(engine.building_upgrade_level_cap(*b));
        if b.level >= cap {
            continue;
        }
        let cand = (b.level, b.id, b);
        if best.is_none_or(|(bl, bid, _)| b.level < bl || (b.level == bl && b.id < bid)) {
            best = Some(cand);
        }
    }
    let building = best?.2;
    let (cost, _) = engine.paid_structure_upgrade_quote(*building);
    if player_gold < cost {
        return None;
    }
    Some(BotDecision {
        bot_id,
        kind: BotDecisionKind::Build,
        intent: GameplayIntent::UpgradeStructure {
            building_id: building.id,
        },
    })
}

/// Finds a valid district placement inside an owned active City's construction radius,
/// preferring the City closest to `preferred_probe` (or any owned City if `None`).
pub(super) fn find_district_site_in_owned_cities(
    engine: &SowEngine,
    bot_id: u16,
    kind: BuildingKind,
    preferred_probe: Option<u32>,
) -> Option<u32> {
    let map = &engine.state.map;
    if map.width == 0 || map.height == 0 || !kind.is_non_city() {
        return None;
    }
    let cities = city_anchors_from_buildings(&engine.buildings, map.width);
    let mut owned_cities: Vec<CityAnchor> = cities
        .iter()
        .copied()
        .filter(|c| c.owner_id == bot_id && c.active_level >= 1)
        .collect();
    if owned_cities.is_empty() {
        return None;
    }
    if let Some(probe) = preferred_probe {
        let px = probe % map.width;
        let py = probe / map.width;
        owned_cities.sort_by_key(|c| {
            let dx = c.x.abs_diff(px);
            let dy = c.y.abs_diff(py);
            (dx * dx + dy * dy, c.id)
        });
    }
    let cfg = &engine.state.config;
    for city in owned_cities {
        let radius = city_construction_radius(city.active_level, cfg) as i32;
        let (ax, ay) = preferred_probe
            .map(|p| ((p % map.width) as i32, (p / map.width) as i32))
            .filter(|&(px, py)| {
                let dx = i64::from(px) - i64::from(city.x as i32);
                let dy = i64::from(py) - i64::from(city.y as i32);
                dx * dx + dy * dy <= i64::from(radius) * i64::from(radius)
            })
            .unwrap_or((city.x as i32, city.y as i32));
        if let Some(site) = nearest_match_structure_site_in_bounds(
            map,
            bot_id,
            kind,
            ax,
            ay,
            radius * 2,
            &engine.building_grid,
            &cities,
            Some(city.id),
            cfg,
            (
                city.x as i32 - radius,
                city.y as i32 - radius,
                city.x as i32 + radius,
                city.y as i32 + radius,
            ),
        ) {
            return Some(site);
        }
    }
    None
}

pub(super) fn find_city_site_from_candidates(
    engine: &SowEngine,
    bot_id: u16,
    candidates: StructureCandidates<'_>,
) -> Option<u32> {
    let map = &engine.state.map;
    let w = map.width;
    if w == 0 || map.height == 0 {
        return None;
    }
    let cities = city_anchors_from_buildings(&engine.buildings, w);
    let cfg = &engine.state.config;
    for &(nx, ny) in candidates.interior {
        if !map.is_valid_coord(nx, ny) {
            continue;
        }
        let idx = ny as u32 * w + nx as u32;
        if validate_match_structure_site(
            map,
            bot_id,
            BuildingKind::City,
            idx,
            &engine.building_grid,
            &cities,
            None,
            cfg,
        )
        .is_ok()
        {
            return Some(idx);
        }
        if let Some(site) = nearest_match_structure_site_in_bounds(
            map,
            bot_id,
            BuildingKind::City,
            nx,
            ny,
            8,
            &engine.building_grid,
            &cities,
            None,
            cfg,
            (0, 0, map.width as i32 - 1, map.height as i32 - 1),
        ) {
            return Some(site);
        }
    }
    for &idx in candidates.border {
        let (bx, by) = ((idx % w) as i32, (idx / w) as i32);
        if let Some(site) = nearest_match_structure_site_in_bounds(
            map,
            bot_id,
            BuildingKind::City,
            bx,
            by,
            8,
            &engine.building_grid,
            &cities,
            None,
            cfg,
            (0, 0, map.width as i32 - 1, map.height as i32 - 1),
        ) {
            return Some(site);
        }
    }
    None
}

pub(super) const PLACEMENT_ATTEMPTS: i32 = 8;

pub(super) struct StructureCandidates<'a> {
    pub(super) border: &'a [u32],
    pub(super) interior: &'a [(i32, i32)],
}

pub(super) fn resolve_structure_from_candidates(
    map: &crate::map::GameMap,
    owner_id: u16,
    kind: BuildingKind,
    candidates: StructureCandidates<'_>,
    existing: &crate::building::BuildingGrid,
    scratch: &mut crate::engine::PlacementScratch,
    legacy: bool,
) -> Option<u32> {
    let map_w = map.width;
    for &idx in candidates.border {
        if let Some(spawn) =
            find_structure_site_near(map, owner_id, idx, kind, existing, scratch, legacy)
        {
            return Some(spawn);
        }
    }
    for &(nx, ny) in candidates.interior {
        if !map.is_valid_coord(nx, ny) {
            continue;
        }
        let (ux, uy) = (nx as u32, ny as u32);
        if map.owner_id(ux, uy) != owner_id {
            continue;
        }
        let idx = uy * map_w + ux;
        if let Some(spawn) =
            find_structure_site_near(map, owner_id, idx, kind, existing, scratch, legacy)
        {
            return Some(spawn);
        }
    }
    None
}

pub(super) fn resolve_military_camp_from_candidates(
    map: &crate::map::GameMap,
    owner_id: u16,
    candidates: StructureCandidates<'_>,
    buildings: &[crate::building::Building],
    existing: &crate::building::BuildingGrid,
    legacy: bool,
) -> Option<u32> {
    let suitable = |idx: u32| {
        if !legacy {
            return crate::building::validate_structure_site(
                map,
                owner_id,
                BuildingKind::MilitaryCamp,
                idx,
                existing,
            )
            .is_ok();
        }
        let x = idx % map.width;
        let y = idx / map.width;
        map.owner_id(x, y) == owner_id
            && map.terrain_type(x, y) == crate::map::TerrainType::Land
            && !buildings.iter().any(|building| building.tile_idx == idx)
    };

    candidates
        .border
        .iter()
        .copied()
        .find(|&idx| suitable(idx))
        .or_else(|| {
            candidates.interior.iter().find_map(|&(x, y)| {
                map.is_valid_coord(x, y)
                    .then(|| y as u32 * map.width + x as u32)
                    .filter(|&idx| suitable(idx))
            })
        })
}

/// Cheapest possible gold cost for a building.
#[inline]
pub(super) fn cheapest_gold_cost(cfg: &crate::game_config::GameConfig) -> f64 {
    BuildingKind::ALL
        .into_iter()
        .filter(|&kind| kind.is_enabled() && kind != BuildingKind::SAM)
        .map(|kind| structure_build_cost_gold(kind, 0, cfg))
        .fold(f64::INFINITY, f64::min)
}
