//! Every playable hero's catalog ability must change the game. Each hero is
//! measured through the real game path twice: once as itself and once as a
//! hero whose ability is another kind. A hero added to the catalog is covered
//! here without writing a new test; an ability that does nothing fails.
use crate::building::{Building, BuildingAggregate};
use crate::engine::SowEngine;
use crate::execution::income_rates::{
    alliance_gold_per_second, gold_income_per_second, troop_income_per_second,
};
use crate::game::{BuildingKind, GamePhase, GameState};
use crate::game_config::GameConfig;
use crate::geo_entities::{Ability, District, Resource};
use crate::map::MapTile;
use crate::player::{Leader, Player, PlayerType};
use crate::protocol::{AttackIntent, CampaignRelation, GameplayIntent, StampedIntent};
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
        Ability::TerritoryGold { .. } => 6,
        Ability::EnemyTroopStrength { .. } => 7,
        Ability::CityDefense { .. } => 8,
        Ability::CityAssault { .. } => 9,
        Ability::AllianceCost { .. } => 10,
        Ability::AllianceGold { .. } => 11,
        Ability::DamageToTraitors { .. } => 12,
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

fn territory_gold(leader: Leader) -> f64 {
    gold_income_per_second(2_048, BuildingAggregate::default(), leader, &GameConfig::default())
}

fn alliance_gold(leader: Leader) -> f64 {
    alliance_gold_per_second(3, leader, &GameConfig::default())
}

/// Troops a traitor keeps after `attacker` hits its border for 20 ticks.
fn traitor_defender_troops(attacker: Leader) -> f64 {
    let mut engine = arena(24, 4, attacker, Some(Leader::of("caesar")));
    let rival = engine.state.player_mut(2).unwrap();
    rival.traitor = true;
    rival.traitor_tick = 10_000;
    attack(&mut engine, 2, 20);
    engine.state.player(2).unwrap().troops
}

/// Troops the attacker keeps after hitting a rival's border (no bunkers).
fn attacker_troops_against_rival(attacker: Leader) -> i64 {
    let mut engine = arena(24, 4, attacker, Some(Leader::of("caesar")));
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

/// Tiles the attacker holds after hitting a city of `defender` (the city sits on the border).
fn tiles_taken_from_city(attacker: Leader, defender: Leader) -> u32 {
    let mut engine = arena(24, 4, attacker, Some(defender));
    add_building(&mut engine, 2, 6, 12, BuildingKind::City);
    engine.refresh_building_grid();
    attack(&mut engine, 2, 20);
    engine.state.player(1).unwrap().tile_count
}

/// Gold the human pays for a campaign alliance that asks 200 gold.
fn alliance_price(human: Leader) -> f64 {
    let mut engine = arena(12, 6, human, Some(Leader::of("caesar")));
    engine.campaign_relations.insert(2, CampaignRelation::Neutral);
    engine.state.player_mut(1).unwrap().gold = 1_000.0;
    engine.apply_intents(&[StampedIntent {
        player_id: 1,
        intent: GameplayIntent::ResolveCampaignDiplomacy {
            target_player: 2,
            relation: CampaignRelation::Allied,
            gold_cost: 200.0,
        },
    }]);
    1_000.0 - engine.state.player(1).unwrap().gold
}

#[test]
fn every_hero_ability_changes_the_game() {
    let plain = Leader::of("ragnar");
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
            Ability::TerritoryGold { .. } => territory_gold(hero) > territory_gold(base),
            Ability::EnemyTroopStrength { .. } => {
                attacker_troops_against_rival(hero) > attacker_troops_against_rival(plain)
            }
            Ability::CityDefense { .. } => {
                tiles_taken_from_city(plain, hero) < tiles_taken_from_city(plain, plain)
            }
            Ability::CityAssault { .. } => {
                tiles_taken_from_city(hero, plain) > tiles_taken_from_city(plain, plain)
            }
            Ability::AllianceCost { .. } => alliance_price(hero) < alliance_price(base),
            Ability::AllianceGold { .. } => alliance_gold(hero) > alliance_gold(base),
            Ability::DamageToTraitors { .. } => {
                traitor_defender_troops(hero) < traitor_defender_troops(plain)
            }
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

fn send(engine: &mut SowEngine, player_id: u16, intent: GameplayIntent) {
    engine.apply_intents(&[StampedIntent { player_id, intent }]);
}

/// The owner's live test, start to finish through real intents: player 1 (`hero`)
/// and player 2 ally, player 2 betrays (break + attack, as the betrayal button
/// does), `delay` ticks pass, then player 1 counterattacks with half its army
/// for 50 ticks (5 s). Returns (tiles player 1 gained, troops player 2 lost).
fn revenge(hero: Leader, delay: u32) -> (i64, i64) {
    let mut engine = arena(32, 16, hero, Some(Leader::of("ragnar")));
    engine.execute_income();
    send(&mut engine, 1, GameplayIntent::ProposeAlliance { target_player: 2 });
    send(&mut engine, 2, GameplayIntent::AcceptAlliance { target_player: 1 });
    assert!(engine.state.player(1).unwrap().alliances.contains(&2), "alliance never formed");
    send(&mut engine, 2, GameplayIntent::BreakAlliance { target_player: 1 });
    let half = engine.state.player(2).unwrap().troops * 0.5;
    send(&mut engine, 2, GameplayIntent::Attack(AttackIntent { target_owner: 1, troops: Some(half) }));
    for _ in 0..delay {
        engine.tick();
    }
    let tiles_before = engine.state.player(1).unwrap().tile_count as i64;
    let traitor_before = engine.state.player(2).unwrap().troops;
    let half = engine.state.player(1).unwrap().troops * 0.5;
    send(&mut engine, 1, GameplayIntent::Attack(AttackIntent { target_owner: 2, troops: Some(half) }));
    for _ in 0..50 {
        engine.tick();
    }
    (
        engine.state.player(1).unwrap().tile_count as i64 - tiles_before,
        (traitor_before - engine.state.player(2).unwrap().troops).round() as i64,
    )
}

/// Boudica, as an online match builds her (account choice → match roster →
/// engine), against a traitor: she must clearly out-fight a hero without the
/// ability in the same scene, and only while the traitor mark lasts.
#[test]
fn boudica_revenge_in_a_real_betrayal() {
    let boudica = Leader::of("boudica");
    assert_eq!(boudica.ability(), Ability::DamageToTraitors { percent: 100 });
    // The account's pick survives the database check (Boudica is free) and the match roster.
    let pick = crate::commerce::resolve_leader(
        Some("boudica"),
        "any-account",
        &Default::default(),
        crate::commerce::current_rotation_period(),
    );
    assert_eq!(pick.resolved, boudica);
    let info = crate::protocol::PlayerInfo {
        id: 1,
        name: "B".into(),
        player_type: PlayerType::Human,
        color: [0.5; 3],
        team: None,
        spawn_x: 0,
        spawn_y: 0,
        leader: pick.resolved,
        skin_style: 0,
        is_ai_controlled: false,
    };
    let info: crate::protocol::PlayerInfo =
        serde_json::from_str(&serde_json::to_string(&info).unwrap()).unwrap();
    let mut config = GameConfig::default();
    config.map_width = 32;
    config.map_height = 32;
    let engine = crate::engine::initialize_match_engine(
        config,
        7,
        &[0x80; 32 * 32],
        vec![info],
        vec![],
        None,
        vec![],
        0,
    );
    assert_eq!(engine.state.player(1).unwrap().leader, boudica);

    let plain = Leader::of("ragnar");
    let (b_tiles, b_kills) = revenge(boudica, 0);
    let (p_tiles, p_kills) = revenge(plain, 0);
    let late_b = revenge(boudica, TRAITOR_WINDOW_PLUS_ONE);
    let late_p = revenge(plain, TRAITOR_WINDOW_PLUS_ONE);
    eprintln!("right after betrayal: Boudica {b_tiles} tiles / {b_kills} kills, plain {p_tiles} tiles / {p_kills} kills");
    eprintln!("after the traitor mark ends: Boudica {late_b:?}, plain {late_p:?}");
    assert!(
        b_tiles > 0 && b_tiles >= p_tiles.max(1) * 3 / 2,
        "Boudica took {b_tiles} tiles vs {p_tiles} without the ability"
    );
    assert_eq!(late_b, late_p, "the bonus outlived the traitor mark");
}

const TRAITOR_WINDOW_PLUS_ONE: u32 = crate::diplomacy::TRAITOR_STATUS_TICKS + 1;

/// The real betrayal path (`BreakAlliance` intent) must mark the breaker as an
/// active traitor, and the snapshot the map and combat read must carry it.
#[test]
fn breaking_an_alliance_marks_the_breaker_as_traitor() {
    let mut engine = arena(24, 4, Leader::of("caesar"), Some(Leader::of("caesar")));
    engine.apply_intents(&[StampedIntent {
        player_id: 2,
        intent: GameplayIntent::BreakAlliance { target_player: 1 },
    }]);
    let breaker = engine.state.player(2).unwrap();
    assert!(crate::diplomacy::is_traitor_active(
        breaker,
        engine.current_tick_u32()
    ));
    assert!(!engine.state.player(1).unwrap().traitor);
    let snapshot = engine.build_snapshot();
    assert!(snapshot.players.iter().any(|p| p.id == 2 && p.traitor));
}

/// The same betrayal scene for every hero: only heroes whose ability makes their
/// troops fight harder win the counterattack clash; nobody else gains anything.
#[test]
fn every_hero_in_the_betrayal_scene() {
    for hero in Leader::all() {
        let (tiles, kills) = revenge(hero, 0);
        let fights_harder = matches!(
            hero.ability(),
            Ability::CombatStrength { .. }
                | Ability::EnemyTroopStrength { .. }
                | Ability::DamageToTraitors { .. }
        );
        eprintln!("{hero:?}: {:?} → {tiles} tiles, {kills} troops", hero.ability());
        assert_eq!(tiles > 0, fights_harder, "{hero:?} in the betrayal scene");
    }
}
