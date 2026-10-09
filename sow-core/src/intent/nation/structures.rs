use crate::building::resolve_structure_spawn_tile;

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
        BuildingKind::Bunker => ((city_equivalent as f64) * 0.35 * factor).floor() as u32,
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

/// Cadence (base ticks between AI actions), keyed on TIER (not IQ).
///
/// Tier is the single source of truth (`ai_tier`); keying cadence on IQ
/// would let two tiers with overlapping bands cross into the wrong cadence.
///   Ghost  → 5 ticks   (0.5-1.0s after scheduler jitter) — top of chain
///   Nation → 30 ticks  (3-6s after scheduler jitter)     — mid tier
///   Tribe  → 100 ticks (10-20s after scheduler jitter)   — slowest, not brainrot
///
/// The scheduler adds deterministic seed/id jitter around this base. The ID
/// changes phase, never the entity's tier or personality.
pub(super) fn iq_build_interval_base(tier: AiTier) -> u64 {
    match tier {
        AiTier::Ghost => 5,
        AiTier::Nation => 30,
        AiTier::Tribe => 100,
    }
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
    buildings: &[crate::building::Building],
    bot_id: u16,
    kind: BuildingKind,
    player_gold: f64,
    cfg: &crate::game_config::GameConfig,
    max_level: u8,
) -> Option<BotDecision> {
    let stack_tile = pick_stack_click_tile(buildings, bot_id, kind, max_level)?;
    let building = buildings
        .iter()
        .find(|b| b.owner_id == bot_id && b.kind == kind && b.tile_idx == stack_tile)?;
    let owned_levels = crate::building::count_kind(buildings, bot_id, kind);
    let cost = crate::building::structure_upgrade_cost_gold(
        kind,
        building.level.saturating_add(1),
        owned_levels,
        cfg,
    );
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
) -> Option<u32> {
    let map_w = map.width;
    for &idx in candidates.border {
        if let Some(spawn) =
            resolve_structure_spawn_tile(map, owner_id, kind, idx, existing, scratch)
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
            resolve_structure_spawn_tile(map, owner_id, kind, idx, existing, scratch)
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
) -> Option<u32> {
    let suitable = |idx: u32| {
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
        .min(cfg.cost_bunker)
        .min(cfg.cost_factory)
        .min(cfg.cost_port)
        .min(cfg.cost_farm)
}
