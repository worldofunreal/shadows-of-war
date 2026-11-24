use super::core::Building;
use crate::game_config::GameConfig;
use crate::game::BuildingKind;

fn building_cost_progression_multiplier(built_levels: u32, cfg: &GameConfig) -> f64 {
    let multiplier = 1.0 + 0.1 * (f64::from(built_levels) + 1.0).log2();
    multiplier.min(cfg.cost_scale_cap_multiplier.max(1.0))
}

/// Gold price for a new structure, scaled by levels the player has built or upgraded.
#[inline]
pub fn structure_build_cost_gold(kind: BuildingKind, built_levels: u32, cfg: &GameConfig) -> f64 {
    let base_cost = match kind {
        BuildingKind::City => cfg.cost_city,
        BuildingKind::Tower => cfg.cost_tower,
        BuildingKind::TradingPost => cfg.cost_trading_post,
        BuildingKind::Port => cfg.cost_port,
        BuildingKind::MilitaryCamp => cfg.cost_military_camp,
        BuildingKind::SAM => cfg.cost_sam,
        BuildingKind::MissileSilo => 375.0,
    };
    base_cost * building_cost_progression_multiplier(built_levels, cfg)
}

/// Each upgrade is priced for one structure, then adjusted for levels the player built or paid for.
#[inline]
pub fn structure_upgrade_cost_gold(
    kind: BuildingKind,
    target_level: u8,
    built_levels: u32,
    cfg: &GameConfig,
) -> f64 {
    if !(2..=kind.max_level()).contains(&target_level) {
        return f64::INFINITY;
    }
    let first_upgrade_cost = match kind {
        BuildingKind::City | BuildingKind::MilitaryCamp | BuildingKind::TradingPost => 110.0,
        BuildingKind::Port => 120.0,
        BuildingKind::Tower => 80.0,
        BuildingKind::SAM => 700.0,
        BuildingKind::MissileSilo => 750.0,
    };
    let baseline = building_cost_progression_multiplier(u32::from(target_level - 1), cfg);
    let own_progression = building_cost_progression_multiplier(
        built_levels.max(u32::from(target_level - 1)),
        cfg,
    );
    let tier_cost = first_upgrade_cost * 2.0_f64.powi(i32::from(target_level - 2));
    tier_cost * (own_progression / baseline)
}

#[inline]
pub fn structure_build_duration_ticks(kind: BuildingKind) -> u32 {
    kind.construction_duration_ticks().max(1)
}

pub fn structure_foundation_quote(
    kind: BuildingKind,
    level: u8,
    built_levels: u32,
    cfg: &GameConfig,
) -> Option<(f64, u32)> {
    if !(1..=kind.max_level()).contains(&level) {
        return None;
    }
    let mut cost = structure_build_cost_gold(kind, built_levels, cfg);
    let base_ticks = structure_build_duration_ticks(kind);
    let mut ticks = if cfg.tutorial { (base_ticks / 2).max(1) } else { base_ticks };
    if level > 1 {
        let (upgrade_cost, upgrade_ticks, _) = structure_multi_upgrade_cost_and_ticks(
            kind, 1, level, built_levels.saturating_add(1), false, 0, cfg.tutorial, cfg,
        )?;
        cost += upgrade_cost;
        ticks = ticks.saturating_add(upgrade_ticks);
    }
    cost.is_finite().then_some((cost, ticks))
}

#[inline]
pub fn structure_upgrade_duration_ticks(kind: BuildingKind, target_level: u8) -> u32 {
    crate::building::core::upgrade_duration_ticks(kind, target_level)
}

/// Cumulative cost and duration for upgrading one structure from `from_level` through
/// `target_level` (inclusive), advancing `built_levels` by `+1` on each step.
pub fn structure_multi_upgrade_cost_and_ticks(
    kind: BuildingKind,
    from_level: u8,
    target_level: u8,
    mut built_levels: u32,
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
            built_levels = built_levels.saturating_add(1);
            continue;
        }
        let raw_cost = structure_upgrade_cost_gold(kind, lvl, built_levels, cfg);
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
        built_levels = built_levels.saturating_add(1);
    }
    Some((total_cost, total_ticks, any_discounted))
}

/// Highest target level in `(from_level + 1)..=max_allowed_level` affordable with `gold`,
/// returning `(target_level, cumulative_cost, total_ticks)`.
pub fn structure_max_affordable_upgrade(
    kind: BuildingKind,
    from_level: u8,
    max_allowed_level: u8,
    built_levels: u32,
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
            built_levels,
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
    [0, 1, 3, 6, 11, 20, 40, 80, 120][usize::from(active_level.min(8))]
}

pub fn port_dock_capacity(active_level: u8) -> u32 {
    [0, 1, 2, 3, 4, 6, 10, 16, 24][usize::from(active_level.min(8))]
}

pub fn port_repair_per_second(active_level: u8) -> f64 {
    [0.0, 50.0, 100.0, 150.0, 200.0, 400.0, 800.0, 1_600.0, 3_200.0]
        [usize::from(active_level.min(8))]
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
    fn building_lines_have_eight_levels_and_nuclear_structures_have_four() {
        for kind in [BuildingKind::City, BuildingKind::Tower, BuildingKind::Port, BuildingKind::MilitaryCamp, BuildingKind::TradingPost] {
            assert_eq!(kind.max_level(), 8);
        }
        for kind in [BuildingKind::SAM, BuildingKind::MissileSilo] {
            assert_eq!(kind.max_level(), 4);
        }
    }

    #[test]
    fn initial_building_prices_match_the_balance_table() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, 80.0),
            (BuildingKind::Tower, 50.0),
            (BuildingKind::TradingPost, 75.0),
            (BuildingKind::Port, 90.0),
            (BuildingKind::MilitaryCamp, 85.0),
            (BuildingKind::SAM, 350.0),
            (BuildingKind::MissileSilo, 375.0),
        ] {
            assert_eq!(structure_build_cost_gold(kind, 0, &cfg), expected);
        }
    }

    #[test]
    fn every_upgrade_level_applies_built_level_growth_to_its_base_price() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, vec![110.0, 220.0, 440.0, 880.0, 1_760.0, 3_520.0, 7_040.0]),
            (BuildingKind::MilitaryCamp, vec![110.0, 220.0, 440.0, 880.0, 1_760.0, 3_520.0, 7_040.0]),
            (BuildingKind::TradingPost, vec![110.0, 220.0, 440.0, 880.0, 1_760.0, 3_520.0, 7_040.0]),
            (BuildingKind::Port, vec![120.0, 240.0, 480.0, 960.0, 1_920.0, 3_840.0, 7_680.0]),
            (BuildingKind::Tower, vec![80.0, 160.0, 320.0, 640.0, 1_280.0, 2_560.0, 5_120.0]),
            (BuildingKind::SAM, vec![700.0, 1_400.0, 2_800.0]),
            (BuildingKind::MissileSilo, vec![750.0, 1_500.0, 3_000.0]),
        ] {
            for target_level in 2..=kind.max_level() {
                let previous_built_levels = u32::from(target_level - 1);
                let upgrade =
                    structure_upgrade_cost_gold(kind, target_level, previous_built_levels, &cfg);
                assert_eq!(upgrade, expected[usize::from(target_level - 2)]);
                for built_levels in [10, 100, 1_000] {
                    let cost = structure_upgrade_cost_gold(kind, target_level, built_levels, &cfg);
                    let scale = structure_build_cost_gold(kind, built_levels, &cfg)
                        / structure_build_cost_gold(kind, previous_built_levels, &cfg);
                    assert!((cost - upgrade * scale).abs() < 1e-9);
                }
            }
        }
    }
}
