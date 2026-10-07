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
    let ragnar_mult = if leader == Leader::Ragnar { 1.50 } else { 1.0 };
    let vercingetorix_mult = if leader == Leader::Vercingetorix {
        1.50
    } else {
        1.0
    };

    cfg.troop_base_income
        + cfg.city_troop_income * agg.city_levels as f64 * vercingetorix_mult
        + cfg.farm_troop_income * agg.farm_levels as f64
        + cfg.port_troop_income * agg.port_levels as f64 * ragnar_mult
        + territory_rate(
            tiles_owned,
            cfg.territory_troop_amount,
            cfg.territory_troop_tiles,
        )
}

/// Per-second gold income before upkeep, NPC handicap and `per_tick()` scaling.
pub fn gold_income_per_second(tiles: u32, agg: BuildingAggregate, cfg: &GameConfig) -> f64 {
    cfg.gold_base_income
        + cfg.factory_gold_income * agg.factory_levels as f64
        + territory_rate(tiles, cfg.territory_gold_amount, cfg.territory_gold_tiles)
}

#[inline]
pub fn trade_income_per_second(trade_ships: u32, cfg: &GameConfig) -> f64 {
    f64::from(trade_ships) * cfg.trade_ship_gold_income
}

#[inline]
pub fn troop_upkeep_per_second(troops: f64, cfg: &GameConfig) -> f64 {
    troops.max(0.0) / 1_000.0 * cfg.troop_upkeep_per_1000
}

pub fn gold_net_income_per_second(
    troops: f64,
    tiles: u32,
    agg: BuildingAggregate,
    trade_ships: u32,
    charge_upkeep: bool,
    cfg: &GameConfig,
) -> f64 {
    gold_income_per_second(tiles, agg, cfg) + trade_income_per_second(trade_ships, cfg)
        - if charge_upkeep {
            troop_upkeep_per_second(troops, cfg)
        } else {
            0.0
        }
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
            gold_income_per_second(0, agg, &cfg),
            cfg.gold_base_income
        );
    }

    #[test]
    fn territory_generates_one_gold_per_second_per_1024_tiles() {
        let cfg = default_cfg();
        let agg = BuildingAggregate::default();
        assert_eq!(
            gold_income_per_second(128, agg, &cfg),
            cfg.gold_base_income + 0.125
        );
        assert_eq!(gold_income_per_second(1_024, agg, &cfg), cfg.gold_base_income + 1.0);
        assert_eq!(gold_income_per_second(10_240, agg, &cfg), cfg.gold_base_income + 10.0);
        assert_eq!(
            troop_income_per_second(400, agg, Leader::Caesar, &cfg),
            cfg.troop_base_income + 25.0
        );
    }

    #[test]
    fn trade_income_and_upkeep_share_the_game_clock() {
        let cfg = default_cfg();
        assert_eq!(trade_income_per_second(2, &cfg), 10.0);
        assert_eq!(troop_upkeep_per_second(100_000.0, &cfg), 20.0);
        assert_eq!(
            gold_net_income_per_second(100_000.0, 0, BuildingAggregate::default(), 2, true, &cfg),
            cfg.gold_base_income + 10.0 - 20.0
        );
        assert_eq!(
            gold_net_income_per_second(100_000.0, 0, BuildingAggregate::default(), 2, false, &cfg),
            cfg.gold_base_income + 10.0
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
        let rate = troop_income_per_second(0, agg, Leader::Caesar, &cfg);
        assert_eq!(rate, cfg.troop_base_income + 50.0);
    }

    #[test]
    fn vercingetorix_multiplies_city_troop_only() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            city_levels: 2,
            port_levels: 2,
            ..Default::default()
        };
        let base = troop_income_per_second(0, agg, Leader::Caesar, &cfg);
        let verc = troop_income_per_second(0, agg, Leader::Vercingetorix, &cfg);
        let city_bonus = cfg.city_troop_income * 2.0;
        let port_bonus = cfg.port_troop_income * 2.0;
        assert_eq!(base, cfg.troop_base_income + city_bonus + port_bonus);
        assert_eq!(verc, cfg.troop_base_income + city_bonus * 1.5 + port_bonus);
    }
}
