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
    let prices = match kind {
        BuildingKind::City => [102.0, 154.0, 218.0],
        BuildingKind::Farm => [110.0, 162.0, 235.0],
        BuildingKind::Factory => [105.0, 111.0, 117.0],
        BuildingKind::Port => [112.0, 172.0, 248.0],
        BuildingKind::Tower => [89.0, 124.0, 176.0],
        BuildingKind::SAM => [220.0, 320.0, 455.0],
    };
    let baseline = ownership_cost_multiplier(u32::from(target_level - 1), cfg);
    let scale = ownership_cost_multiplier(owned_levels, cfg) / baseline;
    prices[usize::from(target_level - 2)] * scale
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
    [0, 1, 3, 6, 11][usize::from(active_level.min(4))]
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
            (BuildingKind::City, 95.0),
            (BuildingKind::Tower, 85.0),
            (BuildingKind::Factory, 90.0),
            (BuildingKind::Port, 105.0),
            (BuildingKind::Farm, 100.0),
            (BuildingKind::SAM, 350.0),
        ] {
            assert_eq!(structure_build_cost_gold(kind, 0, &cfg), expected);
        }
    }

    #[test]
    fn every_upgrade_level_applies_ownership_growth_to_its_base_price() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, [102.0, 154.0, 218.0]),
            (BuildingKind::Farm, [110.0, 162.0, 235.0]),
            (BuildingKind::Factory, [105.0, 111.0, 117.0]),
            (BuildingKind::Port, [112.0, 172.0, 248.0]),
            (BuildingKind::Tower, [89.0, 124.0, 176.0]),
            (BuildingKind::SAM, [220.0, 320.0, 455.0]),
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
