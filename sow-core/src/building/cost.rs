use super::core::Building;
use crate::game_config::GameConfig;
use crate::game::BuildingKind;

fn ownership_cost_multiplier(owned_levels: u32, cfg: &GameConfig) -> f64 {
    let multiplier = 1.0 + 0.1 * (f64::from(owned_levels) + 1.0).log2();
    multiplier.min(cfg.cost_scale_cap_multiplier.max(1.0))
}

/// Total effective building count (sum of levels) owned by `owner` of exact `kind`.
/// A stacked level-3 building counts as 3 for cost scaling.
pub fn count_kind(buildings: &[Building], owner_id: u16, kind: BuildingKind) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == owner_id && b.kind == kind)
        .map(|b| b.level as u32)
        .sum()
}

/// Gold price for a new structure, scaled by total levels of the same kind.
#[inline]
pub fn structure_build_cost_gold(kind: BuildingKind, count: u32, cfg: &GameConfig) -> f64 {
    let base_cost = match kind {
        BuildingKind::City => cfg.cost_city,
        BuildingKind::Tower => cfg.cost_tower,
        BuildingKind::Factory => cfg.cost_factory,
        BuildingKind::Port => cfg.cost_port,
        BuildingKind::Farm => cfg.cost_farm,
        BuildingKind::SAM => cfg.cost_sam,
        BuildingKind::MissileSilo => 375.0,
    };
    base_cost * ownership_cost_multiplier(count, cfg)
}

/// Each upgrade is priced for one structure, then adjusted for additional owned levels.
#[inline]
pub fn structure_upgrade_cost_gold(
    kind: BuildingKind,
    target_level: u8,
    owned_levels: u32,
    cfg: &GameConfig,
) -> f64 {
    if !(2..=kind.max_level()).contains(&target_level) {
        return f64::INFINITY;
    }
    let prices: &[f64] = match kind {
        BuildingKind::City => &[115.0, 175.0, 250.0, 4_000.0, 13_000.0, 40_000.0],
        BuildingKind::Farm => &[125.0, 190.0, 280.0],
        BuildingKind::Factory => &[125.0, 165.0, 220.0],
        BuildingKind::Port => &[130.0, 205.0, 295.0, 5_000.0, 16_000.0, 48_000.0],
        BuildingKind::Tower => &[100.0, 140.0, 190.0, 3_000.0, 10_000.0, 32_000.0],
        BuildingKind::SAM => &[250.0, 285.0, 325.0],
        BuildingKind::MissileSilo => &[265.0, 305.0, 345.0],
    };
    let baseline = ownership_cost_multiplier(u32::from(target_level - 1), cfg);
    let scale = ownership_cost_multiplier(owned_levels, cfg) / baseline;
    prices
        .get(usize::from(target_level - 2))
        .copied()
        .unwrap_or(f64::INFINITY)
        * scale
}

#[inline]
pub fn structure_build_duration_ticks(kind: BuildingKind) -> u32 {
    kind.construction_duration_ticks().max(1)
}

#[inline]
pub fn structure_upgrade_duration_ticks(kind: BuildingKind, target_level: u8) -> u32 {
    crate::building::core::upgrade_duration_ticks(kind, target_level)
}

/// Cumulative cost and duration for upgrading one structure from `from_level` through
/// `target_level` (inclusive), advancing `owned_levels` by `+1` on each step.
pub fn structure_multi_upgrade_cost_and_ticks(
    kind: BuildingKind,
    from_level: u8,
    target_level: u8,
    mut owned_levels: u32,
    free_lv2_tutorial: bool,
    discount_pct: u32,
    tutorial_half_time: bool,
    cfg: &GameConfig,
) -> Option<(f64, u32, bool)> {
    if target_level <= from_level || target_level > kind.max_level() {
        return None;
    }
    let discount_pct = discount_pct.min(100);
    let mut total_cost = 0.0;
    let mut total_ticks = 0u32;
    let mut any_discounted = false;
    for lvl in from_level.saturating_add(1)..=target_level {
        let base_ticks = structure_upgrade_duration_ticks(kind, lvl);
        let step_ticks = if tutorial_half_time {
            (base_ticks / 2).max(1)
        } else {
            base_ticks
        };
        total_ticks = total_ticks.saturating_add(step_ticks);

        if free_lv2_tutorial && kind == BuildingKind::City && lvl == 2 {
            owned_levels = owned_levels.saturating_add(1);
            continue;
        }
        let raw_cost = structure_upgrade_cost_gold(kind, lvl, owned_levels, cfg);
        if !raw_cost.is_finite() {
            return None;
        }
        let step_cost = if discount_pct > 0 && raw_cost > 0.0 {
            any_discounted = true;
            raw_cost * f64::from(100 - discount_pct) / 100.0
        } else {
            raw_cost
        };
        total_cost += step_cost;
        owned_levels = owned_levels.saturating_add(1);
    }
    Some((total_cost, total_ticks, any_discounted))
}

/// Highest target level in `(from_level + 1)..=max_allowed_level` affordable with `gold`,
/// returning `(target_level, cumulative_cost, total_ticks)`.
pub fn structure_max_affordable_upgrade(
    kind: BuildingKind,
    from_level: u8,
    max_allowed_level: u8,
    owned_levels: u32,
    free_lv2_tutorial: bool,
    discount_pct: u32,
    tutorial_half_time: bool,
    gold: f64,
    cfg: &GameConfig,
) -> Option<(u8, f64, u32)> {
    let cap = max_allowed_level.min(kind.max_level());
    if cap <= from_level || !gold.is_finite() {
        return None;
    }
    let mut best: Option<(u8, f64, u32)> = None;
    for target in from_level.saturating_add(1)..=cap {
        let Some((cost, ticks, _)) = structure_multi_upgrade_cost_and_ticks(
            kind,
            from_level,
            target,
            owned_levels,
            free_lv2_tutorial,
            discount_pct,
            tutorial_half_time,
            cfg,
        ) else {
            break;
        };
        if cost.is_finite() && gold + 1e-6 >= cost {
            best = Some((target, cost, ticks));
        } else {
            break;
        }
    }
    best
}

pub fn player_fleet_capacity(buildings: &[Building], player_id: u16) -> u32 {
    let slots = buildings
        .iter()
        .filter(|building| building.owner_id == player_id && building.kind == BuildingKind::Port)
        .map(|building| port_fleet_slots(building.active_level()))
        .sum();
    player_fleet_capacity_from_port_slots(slots)
}

#[inline]
pub fn player_fleet_capacity_from_port_slots(port_slots: u32) -> u32 {
    3 + port_slots
}

pub fn port_fleet_slots(active_level: u8) -> u32 {
    [0, 1, 3, 6, 11, 20, 40, 80][usize::from(active_level.min(7))]
}

pub fn port_dock_capacity(active_level: u8) -> u32 {
    [0, 1, 2, 3, 4, 6, 10, 16][usize::from(active_level.min(7))]
}

pub fn port_repair_per_second(active_level: u8) -> f64 {
    [0.0, 50.0, 100.0, 150.0, 200.0, 400.0, 800.0, 1_600.0]
        [usize::from(active_level.min(7))]
}

#[inline]
pub fn player_trade_ship_capacity(buildings: &[Building], player_id: u16) -> u32 {
    player_port_levels(buildings, player_id)
}

pub fn player_port_levels(buildings: &[Building], player_id: u16) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == player_id && b.kind == BuildingKind::Port)
        .map(|b| u32::from(b.active_level()))
        .sum()
}

#[inline]
pub fn player_boat_speed_bonus(buildings: &[Building], player_id: u16) -> f64 {
    boat_speed_bonus_from_port_levels(player_port_levels(buildings, player_id))
}

#[inline]
pub fn boat_speed_bonus_from_port_levels(port_levels: u32) -> f64 {
    (port_levels as f64 * 0.01).min(0.30)
}

/// Canonical gold cost for a queued strategic product.
#[inline]
pub fn city_arsenal_cost_gold(
    kind: crate::game::CityArsenalKind,
    owned_warships: u32,
    leader: crate::player::Leader,
    cfg: &crate::game_config::GameConfig,
) -> f64 {
    match kind {
        crate::game::CityArsenalKind::Warship => crate::game::UnitType::Warship
            .gold_cost_with_owned_count_for_leader(owned_warships, leader),
        crate::game::CityArsenalKind::Nuke | crate::game::CityArsenalKind::AntiNuke => cfg.nuke_cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(id: u64, level: u8, under_construction: bool) -> Building {
        Building {
            id,
            owner_id: 7,
            tile_idx: id as u32,
            kind: BuildingKind::Port,
            level,
            under_construction,
            ticks_until_complete: 0,
        }
    }

    #[test]
    fn level_one_ports_add_one_slot_each_to_the_base_capacity() {
        assert_eq!(player_fleet_capacity(&[], 7), 3);
        assert_eq!(player_trade_ship_capacity(&[], 7), 0);

        let ports: Vec<_> = (0..400).map(|id| port(id, 1, false)).collect();
        assert_eq!(player_fleet_capacity(&ports, 7), 403);
        assert_eq!(player_trade_ship_capacity(&ports, 7), 400);
        assert_eq!(player_boat_speed_bonus(&ports, 7), 0.30);

        let unfinished = [port(1, 1, true)];
        assert_eq!(player_fleet_capacity(&unfinished, 7), 3);
        assert_eq!(player_boat_speed_bonus(&unfinished, 7), 0.0);

        let upgrading = [port(2, 4, true)];
        assert_eq!(player_fleet_capacity(&upgrading, 7), 9);
        assert_eq!(player_boat_speed_bonus(&upgrading, 7), 0.03);
    }

    #[test]
    fn boat_capacity_tracks_port_completion_upgrade_and_removal() {
        let mut ports = [port(1, 1, true)];
        assert_eq!(player_fleet_capacity(&ports, 7), 3);

        ports[0].under_construction = false;
        assert_eq!(player_fleet_capacity(&ports, 7), 4);

        ports[0].level = 2;
        ports[0].under_construction = true;
        assert_eq!(player_fleet_capacity(&ports, 7), 4);
        ports[0].under_construction = false;
        assert_eq!(player_fleet_capacity(&ports, 7), 6);

        assert_eq!(player_fleet_capacity(&[], 7), 3);
    }

    #[test]
    fn economy_buildings_have_seven_levels_and_other_buildings_have_four() {
        for kind in [BuildingKind::City, BuildingKind::Tower, BuildingKind::Port] {
            assert_eq!(kind.max_level(), 7);
        }
        for kind in [BuildingKind::Farm, BuildingKind::Factory, BuildingKind::SAM, BuildingKind::MissileSilo] {
            assert_eq!(kind.max_level(), 4);
        }
    }

    #[test]
    fn initial_building_prices_match_the_balance_table() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, 100.0),
            (BuildingKind::Tower, 70.0),
            (BuildingKind::Factory, 95.0),
            (BuildingKind::Port, 110.0),
            (BuildingKind::Farm, 105.0),
            (BuildingKind::SAM, 350.0),
            (BuildingKind::MissileSilo, 375.0),
        ] {
            assert_eq!(structure_build_cost_gold(kind, 0, &cfg), expected);
        }
    }

    #[test]
    fn every_upgrade_level_applies_ownership_growth_to_its_base_price() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, vec![115.0, 175.0, 250.0, 4_000.0, 13_000.0, 40_000.0]),
            (BuildingKind::Farm, vec![125.0, 190.0, 280.0]),
            (BuildingKind::Factory, vec![125.0, 165.0, 220.0]),
            (BuildingKind::Port, vec![130.0, 205.0, 295.0, 5_000.0, 16_000.0, 48_000.0]),
            (BuildingKind::Tower, vec![100.0, 140.0, 190.0, 3_000.0, 10_000.0, 32_000.0]),
            (BuildingKind::SAM, vec![250.0, 285.0, 325.0]),
            (BuildingKind::MissileSilo, vec![265.0, 305.0, 345.0]),
        ] {
            for target_level in 2..=kind.max_level() {
                let previous_owned_levels = u32::from(target_level - 1);
                let upgrade =
                    structure_upgrade_cost_gold(kind, target_level, previous_owned_levels, &cfg);
                assert_eq!(upgrade, expected[usize::from(target_level - 2)]);
                for owned_levels in [10, 100, 1_000] {
                    let cost = structure_upgrade_cost_gold(kind, target_level, owned_levels, &cfg);
                    let scale = structure_build_cost_gold(kind, owned_levels, &cfg)
                        / structure_build_cost_gold(kind, previous_owned_levels, &cfg);
                    assert!((cost - upgrade * scale).abs() < 1e-9);
                }
            }
        }
    }
}
