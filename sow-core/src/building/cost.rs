use super::core::Building;
use crate::game::BuildingKind;

/// Total effective building count (sum of levels) owned by `owner` of exact `kind`.
/// A stacked level-3 building counts as 3 for cost scaling.
pub fn count_kind(buildings: &[Building], owner_id: u16, kind: BuildingKind) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == owner_id && b.kind == kind)
        .map(|b| b.level as u32)
        .sum()
}

/// Gold price for a new structure.
#[inline]
pub fn structure_build_cost_gold(
    kind: BuildingKind,
    count: u32,
    cfg: &crate::game_config::GameConfig,
) -> f64 {
    let base_cost = match kind {
        BuildingKind::City => cfg.cost_city,
        BuildingKind::Bunker => cfg.cost_bunker,
        BuildingKind::Factory => cfg.cost_factory,
        BuildingKind::Port => cfg.cost_port,
        BuildingKind::Farm => cfg.cost_farm,
    };
    let cap_mult = cfg.cost_scale_cap_multiplier.max(1.0);
    let scaled = base_cost * 1.1f64.powi(count as i32);
    scaled.min(base_cost * cap_mult)
}

/// Gold price for moving one structure to its next level.
/// The level is authoritative; nearby structures never participate in this cost.
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
    foundation * 0.75
}

#[inline]
pub fn structure_build_duration_ticks(kind: BuildingKind) -> u32 {
    kind.construction_duration_ticks().max(1)
}

#[inline]
pub fn structure_upgrade_duration_ticks(
    kind: BuildingKind,
    target_level: u8,
) -> u32 {
    crate::building::core::upgrade_duration_ticks(kind, target_level)
}

pub fn player_fleet_capacity(buildings: &[Building], player_id: u16) -> u32 {
    1 + player_port_levels(buildings, player_id)
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

#[inline]
pub fn structure_kind_enabled(_kind: BuildingKind) -> bool {
    true
}

/// Whether `player_id` owns a finished Port; the AI uses this for naval strategy.
#[inline]
pub fn player_has_completed_port(buildings: &[Building], player_id: u16) -> bool {
    buildings
        .iter()
        .any(|b| b.owner_id == player_id && (b.kind == BuildingKind::Port && !b.under_construction))
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
    fn boat_capacity_starts_at_one_and_adds_one_per_finished_port_level() {
        assert_eq!(player_fleet_capacity(&[], 7), 1);
        assert_eq!(player_trade_ship_capacity(&[], 7), 0);

        let ports: Vec<_> = (0..400).map(|id| port(id, 1, false)).collect();
        assert_eq!(player_fleet_capacity(&ports, 7), 401);
        assert_eq!(player_trade_ship_capacity(&ports, 7), 400);
        assert_eq!(player_boat_speed_bonus(&ports, 7), 0.30);

        let unfinished = [port(1, 1, true)];
        assert_eq!(player_fleet_capacity(&unfinished, 7), 1);
        assert_eq!(player_boat_speed_bonus(&unfinished, 7), 0.0);

        let upgrading = [port(2, 4, true)];
        assert_eq!(player_fleet_capacity(&upgrading, 7), 4);
        assert_eq!(player_boat_speed_bonus(&upgrading, 7), 0.03);
    }

    #[test]
    fn upgrades_cost_less_than_a_new_building_with_no_hidden_discounts() {
        let cfg = crate::game_config::GameConfig::default();
        let next_foundation = structure_build_cost_gold(BuildingKind::City, 2, &cfg);
        let normal = structure_upgrade_cost_gold(BuildingKind::City, 2, 2, &cfg);

        assert_eq!(normal, next_foundation * 0.75);
    }

    #[test]
    fn every_building_has_three_upgrades_after_its_foundation() {
        assert!(BuildingKind::ALL.into_iter().all(|kind| kind.max_level() == 4));
    }

    #[test]
    fn initial_building_prices_match_the_balance_table() {
        let cfg = crate::game_config::GameConfig::default();
        for (kind, expected) in [
            (BuildingKind::City, 200.0),
            (BuildingKind::Bunker, 100.0),
            (BuildingKind::Factory, 175.0),
            (BuildingKind::Port, 200.0),
            (BuildingKind::Farm, 125.0),
        ] {
            assert_eq!(structure_build_cost_gold(kind, 0, &cfg), expected);
        }
    }

    #[test]
    fn every_upgrade_level_uses_seventy_five_percent_after_existing_cost_growth() {
        let cfg = crate::game_config::GameConfig::default();
        for kind in BuildingKind::ALL {
            let base = structure_build_cost_gold(kind, 0, &cfg);
            for target_level in 2..=kind.max_level() {
                let previous_owned_levels = u32::from(target_level - 1);
                let placement = structure_build_cost_gold(kind, previous_owned_levels, &cfg);
                let upgrade = structure_upgrade_cost_gold(
                    kind,
                    target_level,
                    previous_owned_levels,
                    &cfg,
                );
                assert!((placement / base - 1.1_f64.powi(previous_owned_levels as i32)).abs() < 1e-12);
                assert_eq!(upgrade, placement * 0.75);
            }
        }
    }
}
