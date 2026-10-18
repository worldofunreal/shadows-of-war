//! Every playable hero's catalog ability must change the game. Each hero is
//! measured through the real game path twice: once as itself and once as a
//! hero whose ability is another kind. A hero added to the catalog is covered
//! here without writing a new test; an ability that does nothing fails.
use crate::building::{Building, BuildingAggregate};
use crate::engine::SowEngine;
use crate::execution::income_rates::{gold_income_per_second, troop_income_per_second};
use crate::game::{BuildingKind, GamePhase, GameState};
use crate::game_config::GameConfig;
use crate::geo_entities::{Ability, District, Resource};
use crate::map::MapTile;
use crate::player::{Leader, Player, PlayerType};
use crate::protocol::{AttackIntent, GameplayIntent, StampedIntent};
use crate::water_components::WaterComponents;

/// Abilities measured by the same experiment (conquest speed and combat
/// strength both show up as faster, cheaper expansion).
fn measurement(ability: Ability) -> u8 {
    match ability {
        Ability::CombatStrength { .. } | Ability::ExpansionSpeed { .. } => 0,
        Ability::DistrictIncome { .. } => 1,
        Ability::CityTroopCapacity { .. } => 2,
        Ability::BunkerRange { .. } => 3,
        Ability::BunkerDefense { .. } => 4,
        Ability::EliminationGold { .. } => 5,
    }
}

/// A hero whose ability cannot influence `hero`'s measurement.
fn baseline_for(hero: Leader) -> Leader {
    let measured = measurement(hero.ability());
    Leader::all()
        .find(|other| measurement(other.ability()) != measured)
        .expect("the catalog needs heroes measured by at least two experiments")
}

/// All-land map; player 1 owns the left `own` columns, player 2 the rest
/// when `defender` is set (otherwise the rest is neutral land).
fn arena(width: u32, own: u32, attacker: Leader, defender: Option<Leader>) -> SowEngine {
    let mut config = GameConfig::default();
    config.tick_rate_ms = 100.0;
    // Nobody wins mid-measurement.
    config.map_control_win_percentage = 2.0;
    let mut state = GameState::new(91, width, width, config);
    state.phase = GamePhase::Playing;
    state.map.terrain.fill(MapTile::from_byte(0x80));
    let mut hero = Player::new_human(1, "Hero".into(), [0.5; 3], &state.config);
    hero.leader = attacker;
    hero.has_spawned = true;
    state.register_player(hero);
    if let Some(leader) = defender {
        let mut rival = Player::new_human(2, "Rival".into(), [0.2; 3], &state.config);
        rival.leader = leader;
        rival.has_spawned = true;
        state.register_player(rival);
    }
    // A distant third player keeps the match from ending (last one standing).
    let mut watcher = Player::new_bot(3, "Watcher".into(), [0.9; 3], &state.config);
    watcher.has_spawned = true;
    state.register_player(watcher);
    for y in 0..width {
        for x in 0..width {
            if (x, y) == (width - 1, width - 1) {
                state.set_tile_owner(x, y, 3);
            } else if x < own {
                state.set_tile_owner(x, y, 1);
            } else if defender.is_some() {
                state.set_tile_owner(x, y, 2);
            }
        }
    }
    SowEngine::new(state, WaterComponents::default())
}

fn attack(engine: &mut SowEngine, target_owner: u16, ticks: u32) {
    engine.state.player_mut(1).unwrap().troops = 20_000.0;
    if let Some(defender) = engine.state.player_mut(2) {
        defender.troops = 20_000.0;
    }
    engine.apply_intents(&[StampedIntent {
        player_id: 1,
        intent: GameplayIntent::Attack(AttackIntent {
            target_owner,
            troops: Some(10_000.0),
        }),
    }]);
    for _ in 0..ticks {
        engine.tick();
    }
}

/// (tiles owned, troops still attacking or at home) after attacking neutral land.
fn neutral_conquest(leader: Leader) -> (u32, i64) {
    let mut engine = arena(48, 2, leader, None);
    attack(&mut engine, 0, 30);
    let player = engine.state.player(1).unwrap();
    let attacking: f64 = engine
        .attacks
        .iter()
        .filter(|attack| attack.owner_id == 1)
        .map(|attack| attack.troops)
        .sum();
    (
        player.tile_count,
        (player.troops + attacking).round() as i64,
    )
}

fn district_income(leader: Leader, district: District, resource: Resource) -> f64 {
    let mut agg = BuildingAggregate::default();
    match district {
        District::City => agg.city_levels = 3,
        District::Factory => agg.factory_levels = 3,
        District::Port => agg.port_income_level_weight = 3.0,
        District::Farm => agg.farm_income_level_weight = 3.0,
    }
    let cfg = GameConfig::default();
    match resource {
        Resource::Gold => gold_income_per_second(0, agg, leader, &cfg),
        Resource::Troops => troop_income_per_second(0, agg, leader, &cfg),
    }
}

fn add_building(engine: &mut SowEngine, owner_id: u16, x: u32, y: u32, kind: BuildingKind) {
    let width = engine.state.map.width;
    engine.add_building(Building {
        id: engine.buildings.len() as u64 + 1,
        owner_id,
        tile_idx: y * width + x,
        kind,
        level: 2,
        under_construction: false,
        ticks_until_complete: 0,
    });
}

fn max_troops_with_cities(leader: Leader) -> f64 {
    let mut engine = arena(24, 24, leader, None);
    add_building(&mut engine, 1, 4, 4, BuildingKind::City);
    engine.execute_income();
    engine.state.player(1).unwrap().max_troops
}

/// Whether a bunker of player 2 still defends a tile 17 hexes away.
fn bunker_reaches(defender: Leader) -> bool {
    let mut engine = arena(48, 1, Leader::of("caesar"), Some(defender));
    add_building(&mut engine, 2, 10, 10, BuildingKind::Bunker);
    engine.refresh_defense_grid();
    let width = engine.state.map.width;
    engine
        .defense_grid
        .influence(27, 10, width, 2, 1, engine.state.seed, &engine.state.config)
        .priority_bonus
        > 0.0
}

/// Troops the attacker has left after hitting a bunker-defended border.
fn attacker_troops_against_bunker(defender: Leader) -> i64 {
    let mut engine = arena(24, 4, Leader::of("ragnar"), Some(defender));
    add_building(&mut engine, 2, 6, 12, BuildingKind::Bunker);
    engine.refresh_defense_grid();
    attack(&mut engine, 2, 20);
    let player = engine.state.player(1).unwrap();
    let attacking: f64 = engine
        .attacks
        .iter()
        .filter(|attack| attack.owner_id == 1)
        .map(|attack| attack.troops)
        .sum();
    (player.troops + attacking).round() as i64
}

fn elimination_gold(conqueror: Leader) -> f64 {
    let mut engine = arena(12, 6, conqueror, Some(Leader::of("ragnar")));
    engine.state.player_mut(2).unwrap().player_type = PlayerType::Bot;
    let before = engine.state.player(1).unwrap().gold;
    engine.eliminate_player(2, 1, 0, 0, false);
    engine.state.player(1).unwrap().gold - before
}

#[test]
fn every_hero_ability_changes_the_game() {
    for hero in Leader::all() {
        let base = baseline_for(hero);
        let ability = hero.ability();
        let improved = match ability {
            Ability::DistrictIncome {
                district, resource, ..
            } => {
                district_income(hero, district, resource)
                    > district_income(base, district, resource)
            }
            Ability::CombatStrength { .. } | Ability::ExpansionSpeed { .. } => {
                neutral_conquest(hero) > neutral_conquest(base)
            }
            Ability::CityTroopCapacity { .. } => {
                max_troops_with_cities(hero) > max_troops_with_cities(base)
            }
            Ability::BunkerRange { .. } => bunker_reaches(hero) && !bunker_reaches(base),
            Ability::BunkerDefense { .. } => {
                attacker_troops_against_bunker(hero) < attacker_troops_against_bunker(base)
            }
            Ability::EliminationGold { .. } => elimination_gold(hero) > elimination_gold(base),
        };
        assert!(
            improved,
            "{hero:?}'s ability {ability:?} has no effect in the game"
        );
    }
}

#[test]
fn every_hero_has_a_catalog_ability() {
    for hero in Leader::all() {
        let ability = hero.ability();
        assert!(
            ability.percent() > 0,
            "{hero:?} has a zero-strength ability"
        );
    }
}
