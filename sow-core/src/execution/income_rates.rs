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

/// Per-second troop income before bot penalty and before `per_tick()` scaling.
pub fn troop_income_per_second(
    tiles_owned: u32,
    agg: BuildingAggregate,
    leader: Leader,
    cfg: &GameConfig,
) -> f64 {
    let vercingetorix_mult = if leader == Leader::Vercingetorix {
        1.50
    } else {
        1.0
    };

    cfg.troop_base_income
        + cfg.city_troop_income * agg.city_levels as f64 * vercingetorix_mult
        + cfg.farm_troop_income * agg.farm_income_level_weight
        + territory_rate(
            tiles_owned,
            cfg.territory_troop_amount,
            cfg.territory_troop_tiles,
        )
}

/// Per-game-second income before NPC handicap and `per_tick()` scaling.
pub fn gold_income_per_second(
    tiles: u32,
    agg: BuildingAggregate,
    leader: Leader,
    cfg: &GameConfig,
) -> f64 {
    let ragnar_mult = if leader == Leader::Ragnar { 1.50 } else { 1.0 };
    cfg.gold_base_income
        + cfg.factory_gold_income * agg.factory_levels as f64
        + cfg.farm_gold_income * agg.farm_income_level_weight
        + cfg.port_gold_income * agg.port_income_level_weight * ragnar_mult
        + territory_rate(tiles, cfg.territory_gold_amount, cfg.territory_gold_tiles)
}

#[inline]
pub fn trade_income_per_second(trade_ships: u32, cfg: &GameConfig) -> f64 {
    f64::from(trade_ships) * cfg.trade_ship_gold_income
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
            troop_income_per_second(0, agg, Leader::Caesar, &cfg),
            cfg.troop_base_income
        );
        assert_eq!(
            gold_income_per_second(0, agg, Leader::Caesar, &cfg),
            cfg.gold_base_income
        );
    }

    #[test]
    fn territory_generates_one_gold_per_second_per_2048_tiles() {
        let cfg = default_cfg();
        let agg = BuildingAggregate::default();
        assert_eq!(
            gold_income_per_second(128, agg, Leader::Caesar, &cfg),
            cfg.gold_base_income + 0.0625
        );
        assert_eq!(
            gold_income_per_second(2_048, agg, Leader::Caesar, &cfg),
            cfg.gold_base_income + 1.0
        );
        assert_eq!(
            gold_income_per_second(20_480, agg, Leader::Caesar, &cfg),
            cfg.gold_base_income + 10.0
        );
        assert_eq!(
            troop_income_per_second(400, agg, Leader::Caesar, &cfg),
            cfg.troop_base_income + 400.0 / 24.0
        );
    }

    #[test]
    fn standard_speed_base_and_factory_gold_match_balance_targets() {
        let cfg = default_cfg();
        let ticks_per_second = 1_000.0 / f64::from(cfg.tick_rate_ms);
        let gold_per_minute = |per_second| cfg.per_tick(per_second) * ticks_per_second * 60.0;

        assert_eq!(gold_per_minute(cfg.gold_base_income), 21.0);
        assert!((gold_per_minute(cfg.factory_gold_income) - 24.0).abs() < 1e-12);
        assert_eq!(cfg.troop_base_income * 21.0, 3_675.0);
        assert_eq!(cfg.territory_troop_tiles, 24);
        assert_eq!(cfg.territory_gold_tiles, 2_048);
        assert_eq!(cfg.cost_city, 200.0);
        assert_eq!(cfg.cost_bunker, 100.0);
        assert_eq!(cfg.cost_factory, 175.0);
        assert_eq!(cfg.cost_port, 200.0);
        assert_eq!(cfg.cost_farm, 125.0);
    }

    #[test]
    fn trade_income_uses_game_seconds() {
        let cfg = default_cfg();
        assert!((trade_income_per_second(2, &cfg) * 21.0 - 24.0).abs() < 1e-12);
    }

    #[test]
    fn city_troop_income_from_config() {
        let mut cfg = default_cfg();
        cfg.city_troop_income = 25.0;
        let agg = BuildingAggregate {
            city_levels: 2,
            ..Default::default()
        };
        let rate = troop_income_per_second(0, agg, Leader::Caesar, &cfg);
        assert_eq!(rate, cfg.troop_base_income + 50.0);
    }

    #[test]
    fn vercingetorix_multiplies_city_but_not_farm_troops() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            city_levels: 2,
            farm_income_level_weight: 2.0,
            ..Default::default()
        };
        let base = troop_income_per_second(0, agg, Leader::Caesar, &cfg);
        let verc = troop_income_per_second(0, agg, Leader::Vercingetorix, &cfg);
        let city_bonus = cfg.city_troop_income * 2.0;
        let farm_bonus = cfg.farm_troop_income * 2.0;
        assert_eq!(base, cfg.troop_base_income + city_bonus + farm_bonus);
        assert_eq!(verc, cfg.troop_base_income + city_bonus * 1.5 + farm_bonus);
    }

    #[test]
    fn farm_and_port_marginal_income_grows_by_one_and_a_half_per_level() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            farm_income_level_weight: 4.75,
            port_income_level_weight: 4.75,
            ..Default::default()
        };
        let troop_rate = troop_income_per_second(0, agg, Leader::Caesar, &cfg);
        let gold_rate = gold_income_per_second(0, agg, Leader::Caesar, &cfg);
        assert!((troop_rate - (175.0 + 4.75 * 40.0 / 21.0)).abs() < 1e-12);
        assert!((gold_rate - (1.0 + 4.75 * (4.0 + 12.0) / 21.0)).abs() < 1e-12);

        let ragnar_gold = gold_income_per_second(0, agg, Leader::Ragnar, &cfg);
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
            troop_income_per_second(0, agg, Leader::Caesar, &cfg),
            cfg.troop_base_income
        );
        assert_eq!(
            troop_income_per_second(0, agg, Leader::Ragnar, &cfg),
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
