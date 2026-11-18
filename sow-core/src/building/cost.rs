use super::core::{Building, gold_income_level_weight, income_level_weight};
use crate::game::BuildingKind;

const COST_MULTIPLIER_PER_DOUBLING: f64 = 1.1;

/// Total effective building count (sum of levels) owned by `owner` of exact `kind`.
/// A stacked level-3 building counts as 3 for cost scaling.
pub fn count_kind(buildings: &[Building], owner_id: u16, kind: BuildingKind) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == owner_id && b.kind == kind)
        .map(|b| b.level as u32)
        .sum()
}

/// Gold price for a new structure, adding 10% of base cost per doubling of (owned levels + 1).
#[inline]
pub fn structure_build_cost_gold(
    kind: BuildingKind,
    count: u32,
    cfg: &crate::game_config::GameConfig,
) -> f64 {
    let base_cost = match kind {
        BuildingKind::City => cfg.cost_city,
        BuildingKind::Tower => cfg.cost_tower,
        BuildingKind::Factory => cfg.cost_factory,
        BuildingKind::Port => cfg.cost_port,
        BuildingKind::Farm => cfg.cost_farm,
        BuildingKind::SAM => cfg.cost_sam,
    };
    let cap_mult = cfg.cost_scale_cap_multiplier.max(1.0);
    let count_factor = 1.0
        + (COST_MULTIPLIER_PER_DOUBLING - 1.0) * (f64::from(count) + 1.0).log2();
    let scaled = base_cost * count_factor;
    scaled.min(base_cost * cap_mult)
}

/// Gold price for the next level, discounted 25% against equivalent new production.
#[inline]
pub fn structure_upgrade_cost_gold(
    kind: BuildingKind,
    target_level: u8,
    owned_levels: u32,
    cfg: &crate::game_config::GameConfig,
) -> f64 {
    if target_level == 0 || target_level > kind.max_level() {
        return f64::INFINITY;
    }
    let foundation = structure_build_cost_gold(kind, owned_levels, cfg);
    let marginal = match kind {
        BuildingKind::City | BuildingKind::Farm | BuildingKind::Port => {
            income_level_weight(target_level) - income_level_weight(target_level - 1)
        }
        BuildingKind::Factory => {
            gold_income_level_weight(target_level) - gold_income_level_weight(target_level - 1)
        }
        BuildingKind::Tower | BuildingKind::SAM => 1.0,
    };
    foundation * marginal * 0.75
}

#[inline]
pub fn structure_build_duration_ticks(kind: BuildingKind) -> u32 {
    kind.construction_duration_ticks().max(1)
}

#[inline]
pub fn structure_upgrade_duration_ticks(kind: BuildingKind, target_level: u8) -> u32 {
    crate::building::core::upgrade_duration_ticks(kind, target_level)
}

pub fn player_fleet_capacity(buildings: &[Building], player_id: u16) -> u32 {
    player_fleet_capacity_from_port_levels(player_port_levels(buildings, player_id))
}

#[inline]
pub fn player_fleet_capacity_from_port_levels(port_levels: u32) -> u32 {
    3 + port_levels
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
    fn boat_capacity_starts_at_three_and_adds_one_per_active_port_level() {
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
        assert_eq!(player_fleet_capacity(&upgrading, 7), 6);
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
        assert_eq!(player_fleet_capacity(&ports, 7), 5);

        assert_eq!(player_fleet_capacity(&[], 7), 3);
    }

    #[test]
    fn upgrades_discount_equivalent_production_with_no_hidden_discounts() {
        let cfg = crate::game_config::GameConfig::default();
        let next_foundation = structure_build_cost_gold(BuildingKind::Factory, 2, &cfg);
        let normal = structure_upgrade_cost_gold(BuildingKind::Factory, 2, 2, &cfg);

        let marginal = (1.45_f64 / 0.85).cbrt();
        assert!((normal - next_foundation * marginal * 0.75).abs() < 1e-9);
    }

    #[test]
    fn every_building_has_three_upgrades_after_its_foundation() {
        assert_eq!(BuildingKind::City.max_level(), 4);
        assert!(
            BuildingKind::NON_CITY
                .into_iter()
                .all(|kind| kind.max_level() == 4)
        );
    }

    #[test]
    fn initial_building_prices_match_the_balance_table() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, 125.0),
            (BuildingKind::Tower, 100.0),
            (BuildingKind::Factory, 100.0),
            (BuildingKind::Port, 200.0),
            (BuildingKind::Farm, 175.0),
            (BuildingKind::SAM, 500.0),
        ] {
            assert_eq!(structure_build_cost_gold(kind, 0, &cfg), expected);
        }
    }

    #[test]
    fn every_upgrade_level_prices_its_marginal_benefit_after_existing_cost_growth() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, [154.69, 244.37, 379.69]),
            (BuildingKind::Farm, [216.56, 342.12, 531.56]),
            (BuildingKind::Factory, [98.58, 124.05, 153.53]),
            (BuildingKind::Port, [247.50, 390.99, 607.50]),
            (BuildingKind::Tower, [82.50, 86.89, 90.0]),
            (BuildingKind::SAM, [412.50, 434.44, 450.0]),
        ] {
            for target_level in 2..=kind.max_level() {
                let previous_owned_levels = u32::from(target_level - 1);
                let upgrade =
                    structure_upgrade_cost_gold(kind, target_level, previous_owned_levels, &cfg);
                assert!((upgrade - expected[usize::from(target_level - 2)]).abs() < 0.005);
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
