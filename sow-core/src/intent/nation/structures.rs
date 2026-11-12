use crate::building::find_structure_site_near;
use crate::engine::SowEngine;

use crate::game::BuildingKind;
use crate::protocol::GameplayIntent;

use super::profile::{AiTier, BotDecision, BotDecisionKind};

pub(super) fn bot_structure_target_count(
    kind: BuildingKind,
    city_equivalent: u32,
    bot_iq: u32,
) -> u32 {
    let factor = if bot_iq >= 130 {
        1.0
    } else if bot_iq >= 100 {
        0.5
    } else {
        0.1
    };

    match kind {
        BuildingKind::Tower => ((city_equivalent as f64) * 0.35 * factor).floor() as u32,
        BuildingKind::City => {
            let base = (city_equivalent.saturating_add(2) as f64) * factor;
            (base.floor() as u32).max(1)
        }
        BuildingKind::Factory => {
            let val = ((city_equivalent as f64) * 0.65 * factor).floor() as u32;
            if factor > 0.4 { val.max(1) } else { val }
        }
        BuildingKind::Port => {
            let val = ((city_equivalent as f64) * 0.30 * factor).floor() as u32;
            if factor > 0.4 { val.max(1) } else { val }
        }
        BuildingKind::Farm => {
            let val = ((city_equivalent as f64) * 0.45 * factor).floor() as u32;
            if factor > 0.4 { val.max(1) } else { val }
        }
    }
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

pub(super) fn pick_stack_click_tile(
    buildings: &[crate::building::Building],
    bot_id: u16,
    kind: BuildingKind,
    max_level: u8,
) -> Option<u32> {
    let mut best: Option<(u8, u64, u32)> = None;
    for b in buildings {
        if b.owner_id != bot_id
            || b.kind != kind
            || b.under_construction
            || b.level >= kind.max_level()
            || b.level.saturating_add(1) > max_level
        {
            continue;
        }
        let cand = (b.level, b.id, b.tile_idx);
        match best {
            None => best = Some(cand),
            Some((bl, bid, _)) if b.level < bl || (b.level == bl && b.id < bid) => {
                best = Some(cand);
            }
            _ => {}
        }
    }
    best.map(|(_, _, tile)| tile)
}

pub(super) fn stack_build_decision(
    engine: &SowEngine,
    bot_id: u16,
    kind: BuildingKind,
    player_gold: f64,
    max_level: u8,
) -> Option<BotDecision> {
    let stack_tile = pick_stack_click_tile(&engine.buildings, bot_id, kind, max_level)?;
    let building = engine
        .buildings
        .iter()
        .find(|b| b.owner_id == bot_id && b.kind == kind && b.tile_idx == stack_tile)?;
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

pub(super) fn resolve_farm_from_candidates(
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
                BuildingKind::Farm,
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
    cfg.cost_city
        .min(cfg.cost_tower)
        .min(cfg.cost_factory)
        .min(cfg.cost_port)
        .min(cfg.cost_farm)
}
