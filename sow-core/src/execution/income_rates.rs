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
            * agg.farm_levels as f64
            * leader.resource_bonus_multiplier(Source::District(District::Farm), Resource::Troops)
        + hero_only_income(
            leader.resource_bonus_hero_bonus(Source::District(District::Factory), Resource::Troops),
            cfg.hero_factory_troop_income * agg.factory_levels as f64,
        )
        + hero_only_income(
            leader.resource_bonus_hero_bonus(Source::District(District::Port), Resource::Troops),
            cfg.hero_port_troop_income * agg.port_levels as f64,
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
            * agg.factory_income_level_weight
            * leader.resource_bonus_multiplier(Source::District(District::Factory), Resource::Gold)
        + cfg.farm_gold_income
            * agg.farm_income_level_weight
            * leader.resource_bonus_multiplier(Source::District(District::Farm), Resource::Gold)
        + cfg.port_gold_income
            * agg.port_income_level_weight
            * leader.resource_bonus_multiplier(Source::District(District::Port), Resource::Gold)
        + hero_only_income(
            leader.resource_bonus_hero_bonus(Source::District(District::City), Resource::Gold),
            cfg.hero_city_gold_income * agg.city_income_level_weight,
        )
        + territory_rate(tiles, cfg.territory_gold_amount, cfg.territory_gold_tiles)
            * leader.resource_bonus_multiplier(Source::Territory, Resource::Gold)
}

/// Multiplier for the ordinary income owned by Nafanua's player. Alliance
/// income, trade income, and other hero income remain separate from this rate.
pub fn tafaifa_council_multiplier(leader: Leader, living_allies: usize) -> f64 {
    match leader.ability() {
        crate::geo_entities::Ability::TafaifaCouncil { percent } if living_allies >= 3 => {
            1.0 + f64::from(percent) / 100.0
        }
        _ => 1.0,
    }
}

/// Extra gold and troops per game second from balanced active districts.
pub fn imperial_standard_income(
    agg: BuildingAggregate,
    leader: Leader,
    cfg: &GameConfig,
) -> (f64, f64) {
    let crate::geo_entities::Ability::ImperialStandards { percent } = leader.ability() else {
        return (0.0, 0.0);
    };
    if !cfg.buildings_enabled
        || agg.city_levels == 0
        || agg.city_levels != agg.factory_levels
        || agg.city_levels != agg.farm_levels
    {
        return (0.0, 0.0);
    }
    let multiplier = f64::from(percent) / 100.0;
    (
        cfg.factory_gold_income * agg.factory_income_level_weight * multiplier,
        cfg.farm_troop_income * f64::from(agg.farm_levels) * multiplier,
    )
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
    fn territory_generates_four_gold_per_second_per_1024_tiles() {
        let cfg = default_cfg();
        let agg = BuildingAggregate::default();
        assert_eq!(
            gold_income_per_second(128, agg, Leader::of("caesar"), &cfg),
            cfg.gold_base_income + 0.5
        );
        assert_eq!(
            gold_income_per_second(1_024, agg, Leader::of("caesar"), &cfg),
            cfg.gold_base_income + 4.0
        );
        assert_eq!(
            gold_income_per_second(10_240, agg, Leader::of("caesar"), &cfg),
            cfg.gold_base_income + 40.0
        );
        assert_eq!(
            troop_income_per_second(400, agg, Leader::of("caesar"), &cfg),
            cfg.troop_base_income + 400.0 / 16.0 * 3.0
        );
    }

    #[test]
    fn base_and_factory_gold_match_game_time_balance_targets() {
        let cfg = default_cfg();
        let ticks_per_second = 1_000.0 / f64::from(cfg.tick_rate_ms);
        let gold_per_game_minute = |per_second| {
            cfg.per_tick(per_second) * ticks_per_second * 60.0 / cfg.global_speed_multiplier
        };

        assert!((gold_per_game_minute(cfg.gold_base_income) - 600.0).abs() < 1e-9);
        assert!(
            (gold_per_game_minute(cfg.factory_gold_income) - 60.0 * (32.0 / 7.0)).abs()
                < 1e-9
        );
        assert_eq!(
            cfg.global_speed_multiplier,
            crate::game_config::NORMAL_GAME_SPEED
        );
        assert_eq!(cfg.troop_base_income, 1_600.0);
        assert_eq!(cfg.city_troop_income, 600.0);
        assert_eq!(cfg.farm_troop_income, 600.0);
        assert_eq!(cfg.hero_factory_troop_income, 600.0);
        assert_eq!(cfg.hero_port_troop_income, 600.0);
        assert_eq!(cfg.city_max_troops, 3_630.0);
        assert_eq!(cfg.territory_troop_amount, 3.0);
        assert_eq!(cfg.max_troops_base, 5_000.0);
        assert_eq!(cfg.max_troops_scale, 55.0);
        assert_eq!(cfg.territory_troop_tiles, 16);
        assert_eq!(cfg.territory_gold_tiles, 1_024);
        assert_eq!(cfg.cost_city, 200.0);
        assert_eq!(cfg.cost_tower, 100.0);
        assert_eq!(cfg.cost_factory, 175.0);
        assert_eq!(cfg.cost_port, 200.0);
        assert_eq!(cfg.cost_farm, 125.0);
    }

    #[test]
    fn level_four_city_matches_tuned_troop_rate() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            city_levels: 4,
            ..Default::default()
        };
        let troop_rate = troop_income_per_second(0, agg, Leader::of("caesar"), &cfg);
        // Four levels at 600 troops/s, scaled by the shared 0.18 Normal speed.
        assert!(
            ((troop_rate - cfg.troop_base_income) * cfg.global_speed_multiplier - 432.0).abs()
                < 1e-10
        );
    }

    #[test]
    fn trade_income_uses_game_seconds() {
        let cfg = default_cfg();
        assert!(
            (trade_income_per_second(2, Leader::of("caesar"), &cfg) * 21.0 - 96.0).abs() < 1e-12
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
    fn farm_troops_scale_linearly_while_farm_and_port_gold_are_weighted() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            farm_levels: 2,
            farm_income_level_weight: 4.75,
            port_income_level_weight: 4.75,
            ..Default::default()
        };
        let caesar = Leader::of("caesar");
        let troop_rate = troop_income_per_second(0, agg, caesar, &cfg);
        let gold_rate = gold_income_per_second(0, agg, Leader::of("caesar"), &cfg);
        let farm_troop_bonus = caesar.resource_bonus_multiplier(
            crate::geo_entities::Source::District(crate::geo_entities::District::Farm),
            crate::geo_entities::Resource::Troops,
        );
        assert!(
            (troop_rate - (1_600.0 + cfg.farm_troop_income * 2.0 * farm_troop_bonus)).abs()
                < 1e-12
        );
        assert!((gold_rate - (10.0 + 4.75 * (16.0 + 48.0) / 21.0)).abs() < 1e-12);

        let ragnar_gold = gold_income_per_second(0, agg, Leader::of("cleopatra"), &cfg);
        assert!((ragnar_gold - (10.0 + 4.75 * (16.0 + 72.0) / 21.0)).abs() < 1e-12);
    }

    #[test]
    fn port_troops_are_hero_only_and_scale_linearly() {
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
        let artemisia = Leader::of("artemisia_i");
        let port_troop_bonus = artemisia
            .resource_bonus_hero_bonus(
                crate::geo_entities::Source::District(crate::geo_entities::District::Port),
                crate::geo_entities::Resource::Troops,
            )
            .unwrap();
        assert_eq!(
            troop_income_per_second(0, agg, artemisia, &cfg),
            cfg.troop_base_income + cfg.hero_port_troop_income * 4.0 * port_troop_bonus
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
