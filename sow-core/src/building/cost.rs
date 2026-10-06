use super::core::Building;
use crate::config;
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
    factory_discount_levels: u32,
    cfg: &crate::game_config::GameConfig,
) -> f64 {
    if target_level == 0 || target_level > kind.max_level() {
        return f64::INFINITY;
    }
    let foundation = structure_build_cost_gold(kind, owned_levels, cfg);
    let discount = if kind == BuildingKind::Factory {
        0.0
    } else {
        (factory_discount_levels.min(5) as f64 * 0.05).min(0.25)
    };
    foundation * 0.85 * (1.0 - discount)
}

#[inline]
pub fn structure_build_duration_ticks(kind: BuildingKind, factory_time_levels: u32) -> u32 {
    let ticks = kind.construction_duration_ticks();
    let reduction = (factory_time_levels.min(5) as f64 * 0.05).min(0.25);
    ((ticks as f64 * (1.0 - reduction)).ceil() as u32).max(1)
}

#[inline]
pub fn structure_upgrade_duration_ticks(
    kind: BuildingKind,
    target_level: u8,
    factory_time_levels: u32,
) -> u32 {
    let ticks = crate::building::core::upgrade_duration_ticks(kind, target_level);
    let reduction = (factory_time_levels.min(5) as f64 * 0.05).min(0.25);
    ((ticks as f64 * (1.0 - reduction)).ceil() as u32).max(1)
}

#[inline]
pub fn farm_slots_for_city_level(level: u8) -> u32 {
    if level == 0 || level > BuildingKind::City.max_level() {
        0
    } else {
        1u32 << (level - 1)
    }
}

pub fn player_farm_slots(buildings: &[Building], player_id: u16) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == player_id && b.kind == BuildingKind::City)
        .map(|b| farm_slots_for_city_level(b.active_level()))
        .sum()
}

pub fn factory_perk_counts(buildings: &[Building], player_id: u16) -> (u32, u32, u32) {
    buildings
        .iter()
        .filter(|b| b.owner_id == player_id && b.kind == BuildingKind::Factory)
        .filter_map(|b| {
            let level = b.active_level();
            (level > 0).then_some((
                u32::from(level >= 2),
                u32::from(level >= 3),
                u32::from(level >= 4),
            ))
        })
        .fold((0, 0, 0), |(time, cost, trade), (t, c, r)| {
            (time + t, cost + c, trade + r)
        })
}

pub fn player_fleet_capacity(buildings: &[Building], player_id: u16) -> u32 {
    1 + player_port_levels(buildings, player_id)
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

/// City milestones unlock structure types consistently for the server and client.
#[inline]
pub const fn structure_kind_unlocked(kind: BuildingKind, city_level: u8) -> bool {
    !matches!(kind, BuildingKind::Factory) || city_level >= 3
}

#[cfg(test)]
mod structure_unlock_tests {
    use super::structure_kind_unlocked;
    use crate::game::BuildingKind;

    #[test]
    fn only_factory_requires_city_level_three() {
        assert!(!structure_kind_unlocked(BuildingKind::Factory, 2));
        assert!(structure_kind_unlocked(BuildingKind::Factory, 3));
        assert!(BuildingKind::ALL
            .into_iter()
            .filter(|kind| *kind != BuildingKind::Factory)
            .all(|kind| structure_kind_unlocked(kind, 0)));
    }
}

/// Whether `player_id` owns a finished Port; the AI uses this for naval strategy.
#[inline]
pub fn player_has_completed_port(buildings: &[Building], player_id: u16) -> bool {
    buildings
        .iter()
        .any(|b| b.owner_id == player_id && (b.kind == BuildingKind::Port && !b.under_construction))
}

pub fn module_upgrade_cost_gold(kind: crate::building::ModuleKind, level: u8) -> f64 {
    let s = config::GOLD_SCALE.max(1.0);
    let base = match kind {
        crate::building::ModuleKind::Port => match level {
            1 => 100_000.0,
            2 => 200_000.0,
            3 => 400_000.0,
            4 => 800_000.0,
            _ => 1_600_000.0,
        },
        crate::building::ModuleKind::Foundry => match level {
            1 => 75_000.0,
            2 => 150_000.0,
            3 => 300_000.0,
            4 => 600_000.0,
            _ => 1_200_000.0,
        },
        crate::building::ModuleKind::Armory => match level {
            1 => 75_000.0,
            2 => 150_000.0,
            3 => 300_000.0,
            4 => 600_000.0,
            _ => 1_200_000.0,
        },
        crate::building::ModuleKind::Intel => match level {
            1 => 50_000.0,
            2 => 100_000.0,
            3 => 200_000.0,
            4 => 400_000.0,
            _ => 800_000.0,
        },
        crate::building::ModuleKind::Arsenal => match level {
            1 => 500_000.0,
            2 => 1_000_000.0,
            _ => 2_000_000.0,
        },
        crate::building::ModuleKind::Shield => match level {
            1 => 100_000.0,
            2 => 200_000.0,
            3 => 400_000.0,
            4 => 800_000.0,
            _ => 1_600_000.0,
        },
    };
    base / s
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
            modules: crate::building::CityModules::default(),
        }
    }

    #[test]
    fn boat_capacity_starts_at_one_and_adds_one_per_finished_port_level() {
        assert_eq!(player_fleet_capacity(&[], 7), 1);

        let ports: Vec<_> = (0..400).map(|id| port(id, 1, false)).collect();
        assert_eq!(player_fleet_capacity(&ports, 7), 401);
        assert_eq!(player_boat_speed_bonus(&ports, 7), 0.30);

        let unfinished = [port(1, 1, true)];
        assert_eq!(player_fleet_capacity(&unfinished, 7), 1);
        assert_eq!(player_boat_speed_bonus(&unfinished, 7), 0.0);

        let upgrading = [port(2, 5, true)];
        assert_eq!(player_fleet_capacity(&upgrading, 7), 5);
        assert_eq!(player_boat_speed_bonus(&upgrading, 7), 0.04);
    }

    #[test]
    fn upgrades_cost_less_than_a_new_building_and_factory_discount_caps_at_25_percent() {
        let cfg = crate::game_config::GameConfig::default();
        let next_foundation = structure_build_cost_gold(BuildingKind::City, 2, &cfg);
        let normal = structure_upgrade_cost_gold(BuildingKind::City, 2, 2, 0, &cfg);
        let discounted = structure_upgrade_cost_gold(BuildingKind::City, 2, 2, 10, &cfg);

        assert_eq!(normal, next_foundation * 0.85);
        assert_eq!(discounted, next_foundation * 0.85 * 0.75);
    }

    #[test]
    fn city_level_farm_plot_limits_double_by_tier() {
        assert_eq!(farm_slots_for_city_level(0), 0);
        assert_eq!(farm_slots_for_city_level(1), 1);
        assert_eq!(farm_slots_for_city_level(2), 2);
        assert_eq!(farm_slots_for_city_level(6), 32);
    }
}
