use crate::building::BuildingAggregate;
use crate::game_config::GameConfig;
use crate::player::Leader;

#[inline]
fn territory_rate(tiles: u32, amount: f64, interval: u32) -> f64 {
    if interval == 0 {
        return 0.0;
    }
    tiles as f64 / interval as f64 * amount
}

/// Hero-only base income: `rate` times the hero's bonus, or nothing for other leaders.
#[inline]
fn hero_only_income(bonus: Option<f64>, rate: f64) -> f64 {
    bonus.map_or(0.0, |bonus| rate * bonus)
}

/// Per-second troop income before bot penalty and before `per_tick()` scaling.
pub fn troop_income_per_second(
    tiles_owned: u32,
    agg: BuildingAggregate,
    leader: Leader,
    cfg: &GameConfig,
) -> f64 {
    use crate::geo_entities::{District, Resource, Source};
    cfg.troop_base_income
        + cfg.city_troop_income
            * agg.city_levels as f64
            * leader.resource_bonus_multiplier(Source::District(District::City), Resource::Troops)
        + cfg.farm_troop_income
            * agg.farm_income_level_weight
            * leader.resource_bonus_multiplier(Source::District(District::Farm), Resource::Troops)
        + hero_only_income(
            leader.resource_bonus_hero_bonus(Source::District(District::Factory), Resource::Troops),
            cfg.hero_factory_troop_income * agg.factory_levels as f64,
        )
        + hero_only_income(
            leader.resource_bonus_hero_bonus(Source::District(District::Port), Resource::Troops),
            cfg.hero_port_troop_income * agg.port_income_level_weight,
        )
        + territory_rate(
            tiles_owned,
            cfg.territory_troop_amount,
            cfg.territory_troop_tiles,
        ) * leader.resource_bonus_multiplier(Source::Territory, Resource::Troops)
}

/// Per-game-second income before NPC handicap and `per_tick()` scaling.
pub fn gold_income_per_second(
    tiles: u32,
    agg: BuildingAggregate,
    leader: Leader,
    cfg: &GameConfig,
) -> f64 {
    use crate::geo_entities::{District, Resource, Source};
    cfg.gold_base_income
        + cfg.factory_gold_income
            * agg.factory_levels as f64
            * leader.resource_bonus_multiplier(Source::District(District::Factory), Resource::Gold)
        + cfg.farm_gold_income
            * agg.farm_income_level_weight
            * leader.resource_bonus_multiplier(Source::District(District::Farm), Resource::Gold)
        + cfg.port_gold_income
            * agg.port_income_level_weight
            * leader.resource_bonus_multiplier(Source::District(District::Port), Resource::Gold)
        + hero_only_income(
            leader.resource_bonus_hero_bonus(Source::District(District::City), Resource::Gold),
            cfg.hero_city_gold_income * agg.city_levels as f64,
        )
        + territory_rate(tiles, cfg.territory_gold_amount, cfg.territory_gold_tiles)
            * leader.resource_bonus_multiplier(Source::Territory, Resource::Gold)
}

/// Per-game-second gold from the player's active alliances, paid only to the hero
/// whose ability pays for them.
pub fn alliance_gold_per_second(alliances: usize, leader: Leader, cfg: &GameConfig) -> f64 {
    use crate::geo_entities::{Resource, Source};
    hero_only_income(
        leader.resource_bonus_hero_bonus(Source::Alliance, Resource::Gold),
        cfg.alliance_gold_income * alliances as f64,
    )
}

/// Per-game-second troops from the player's active alliances, paid only to the hero
/// whose ability pays for them.
pub fn alliance_troops_per_second(alliances: usize, leader: Leader, cfg: &GameConfig) -> f64 {
    use crate::geo_entities::{Resource, Source};
    hero_only_income(
        leader.resource_bonus_hero_bonus(Source::Alliance, Resource::Troops),
        cfg.alliance_troop_income * alliances as f64,
    )
}

#[inline]
pub fn trade_income_per_second(trade_ships: u32, leader: Leader, cfg: &GameConfig) -> f64 {
    use crate::geo_entities::{Resource, Source};
    f64::from(trade_ships)
        * cfg.trade_ship_gold_income
        * leader.resource_bonus_multiplier(Source::Trade, Resource::Gold)
}

/// Per-game-second troops from Trade Ships, paid only to the hero whose ability pays for them.
pub fn trade_troops_per_second(trade_ships: u32, leader: Leader, cfg: &GameConfig) -> f64 {
    use crate::geo_entities::{Resource, Source};
    hero_only_income(
        leader.resource_bonus_hero_bonus(Source::Trade, Resource::Troops),
        cfg.trade_troop_income * f64::from(trade_ships),
    )
}

/// Troops a conqueror gets for eliminating a player, paid only to the hero whose ability pays for them.
pub fn elimination_troop_bounty(leader: Leader, cfg: &GameConfig) -> f64 {
    use crate::geo_entities::{Resource, Source};
    hero_only_income(
        leader.resource_bonus_hero_bonus(Source::Elimination, Resource::Troops),
        cfg.elimination_troop_bounty,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::BuildingAggregate;

    fn default_cfg() -> GameConfig {
        GameConfig::default()
    }

    #[test]
    fn zero_tiles_base_only() {
        let cfg = default_cfg();
        let agg = BuildingAggregate::default();
        assert_eq!(
            troop_income_per_second(0, agg, Leader::of("caesar"), &cfg),
            cfg.troop_base_income
        );
        assert_eq!(
            gold_income_per_second(0, agg, Leader::of("caesar"), &cfg),
            cfg.gold_base_income
        );
    }

    #[test]
    fn territory_generates_one_gold_per_second_per_1024_tiles() {
        let cfg = default_cfg();
        let agg = BuildingAggregate::default();
        assert_eq!(
            gold_income_per_second(128, agg, Leader::of("caesar"), &cfg),
            cfg.gold_base_income + 0.125
        );
        assert_eq!(
            gold_income_per_second(1_024, agg, Leader::of("caesar"), &cfg),
            cfg.gold_base_income + 1.0
        );
        assert_eq!(
            gold_income_per_second(10_240, agg, Leader::of("caesar"), &cfg),
            cfg.gold_base_income + 10.0
        );
        assert_eq!(
            troop_income_per_second(400, agg, Leader::of("caesar"), &cfg),
            cfg.troop_base_income + 400.0 / 16.0
        );
    }

    #[test]
    fn base_and_factory_gold_match_game_time_balance_targets() {
        let cfg = default_cfg();
        let ticks_per_second = 1_000.0 / f64::from(cfg.tick_rate_ms);
        let gold_per_game_minute = |per_second| {
            cfg.per_tick(per_second) * ticks_per_second * 60.0 / cfg.global_speed_multiplier
        };

        assert!((gold_per_game_minute(cfg.gold_base_income) - 60.0).abs() < 1e-9);
        assert!((gold_per_game_minute(cfg.factory_gold_income) - 60.0 * (8.0 / 7.0)).abs() < 1e-9);
        assert_eq!(cfg.troop_base_income, 175.0);
        assert_eq!(cfg.territory_troop_tiles, 16);
        assert_eq!(cfg.territory_gold_tiles, 1_024);
        assert_eq!(cfg.cost_city, 200.0);
        assert_eq!(cfg.cost_tower, 100.0);
        assert_eq!(cfg.cost_factory, 175.0);
        assert_eq!(cfg.cost_port, 200.0);
        assert_eq!(cfg.cost_farm, 125.0);
    }

    #[test]
    fn trade_income_uses_game_seconds() {
        let cfg = default_cfg();
        assert!(
            (trade_income_per_second(2, Leader::of("caesar"), &cfg) * 21.0 - 24.0).abs() < 1e-12
        );
    }

    #[test]
    fn city_troop_income_from_config() {
        let mut cfg = default_cfg();
        cfg.city_troop_income = 25.0;
        let agg = BuildingAggregate {
            city_levels: 2,
            ..Default::default()
        };
        let rate = troop_income_per_second(0, agg, Leader::of("caesar"), &cfg);
        assert_eq!(rate, cfg.troop_base_income + 50.0);
    }

    #[test]
    fn vercingetorix_pays_alliance_troops_at_one_and_a_half() {
        let cfg = default_cfg();
        let verc = alliance_troops_per_second(2, Leader::of("vercingetorix"), &cfg);
        assert!((verc - cfg.alliance_troop_income * 2.0 * 1.5).abs() < 1e-12);
    }

    #[test]
    fn farm_and_port_marginal_income_grows_by_one_and_a_half_per_level() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            farm_income_level_weight: 4.75,
            port_income_level_weight: 4.75,
            ..Default::default()
        };
        let troop_rate = troop_income_per_second(0, agg, Leader::of("caesar"), &cfg);
        let gold_rate = gold_income_per_second(0, agg, Leader::of("caesar"), &cfg);
        assert!((troop_rate - (175.0 + 4.75 * 40.0 / 21.0)).abs() < 1e-12);
        assert!((gold_rate - (1.0 + 4.75 * (4.0 + 12.0) / 21.0)).abs() < 1e-12);

        let ragnar_gold = gold_income_per_second(0, agg, Leader::of("cleopatra"), &cfg);
        assert!((ragnar_gold - (1.0 + 4.75 * (4.0 + 18.0) / 21.0)).abs() < 1e-12);
    }

    #[test]
    fn port_generates_no_troops_for_any_leader() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            port_levels: 4,
            ..Default::default()
        };
        assert_eq!(
            troop_income_per_second(0, agg, Leader::of("caesar"), &cfg),
            cfg.troop_base_income
        );
        assert_eq!(
            troop_income_per_second(0, agg, Leader::of("cleopatra"), &cfg),
            cfg.troop_base_income
        );
    }

    #[test]
    fn aggregation_preserves_level_progression_for_each_building() {
        use crate::building::{Building, aggregate_buildings_per_player};
        use crate::game::BuildingKind;

        let make = |id, kind, level| Building {
            id,
            owner_id: 1,
            tile_idx: id as u32,
            kind,
            level,
            under_construction: false,
            ticks_until_complete: 0,
        };
        let aggregates = aggregate_buildings_per_player(
            [
                make(1, BuildingKind::Farm, 2),
                make(2, BuildingKind::Farm, 1),
                make(3, BuildingKind::Port, 4),
            ]
            .into_iter(),
            1,
        );
        let agg = aggregates[1];
        assert_eq!(agg.farm_levels, 3);
        assert_eq!(agg.farm_income_level_weight, 3.5);
        assert_eq!(agg.port_levels, 4);
        assert_eq!(agg.port_income_level_weight, 8.125);
    }
}
