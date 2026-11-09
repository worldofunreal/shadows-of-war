//! Every playable hero's catalog ability must change the game and show its mark
//! in its own situation. Each hero is measured through the real game path against
//! a control hero that lacks that situation. A hero added to the catalog is covered
//! here without writing a new test; an ability that does nothing, or a mark that
//! never shows, fails.
use crate::building::construction::tests::{engine_with_ports, lane, port};
use crate::building::{Building, BuildingAggregate};
use crate::engine::SowEngine;
use crate::execution::combat::{attack_strength, targeted_multiplier};
use crate::execution::income_rates::{
    alliance_gold_per_second, alliance_troops_per_second, gold_income_per_second,
    trade_income_per_second, trade_troops_per_second, troop_income_per_second,
};
use crate::game::{BuildingKind, GameEvent, GamePhase, GameState, UnitType};
use crate::game_config::GameConfig;
use crate::geo_entities::{Ability, District, LossWhen, Resource, Side, Source, Target};
use crate::map::MapTile;
use crate::player::{Leader, Player, PlayerType};
use crate::protocol::{AttackIntent, CampaignRelation, GameplayIntent, StampedIntent};
use crate::warp_fleet::WarpFleet;
use crate::water_components::WaterComponents;

/// The experiment that measures an ability. Every concrete situation has one,
/// so a new situation cannot ship without a test that proves it changes the game.
/// Targeted kinds get one experiment per target, shared by all their situations.
fn measurement(ability: Ability) -> u16 {
    match ability.situation() {
        Ability::CoastalFounding => 400,
        Ability::AlliedCityGrant { .. } => 401,
        Ability::CityTowerUpgrade => 402,
        Ability::SharedCitySpoils { .. } => 403,
        Ability::CityEvacuation { .. } => 404,
        Ability::ReturnCampaign { .. } => 405,
        Ability::AllyPatronage { .. } => 406,
        Ability::SurplusTreasury { .. } => 407,
        Ability::GuildNetwork { .. } => 408,
        Ability::WarshipPrize { .. } => 409,
        Ability::Armistice { .. } => 410,
        Ability::CityFarmFounding => 411,
        Ability::CounterLevy { .. } => 412,
        Ability::CityIntercession { .. } => 413,
        Ability::RetreatHandover { .. } => 414,
        Ability::CityDenial => 415,
        Ability::PuntGardens => 416,
        Ability::LastCityRelocation => 417,
        Ability::WarRegalia { .. } => 418,
        Ability::Reunification { .. } => 419,
        Ability::FactoryCityUpgradeDiscount { .. } => 420,
        Ability::FactoryRaid { .. } => 421,
        Ability::ImperialStandards { .. } => 422,
        Ability::SiegeBreach => 423,
        Ability::CookhamExchange => 425,
        Ability::SecondCrown => 426,
        Ability::KitchenPhilosophy => 427,
        Ability::MountedCapture { .. } => 428,
        Ability::AksumRout => 429,
        Ability::TigranesAid { .. } => 430,
        Ability::AkkadGarrison => 431,
        Ability::SharedFeast => 432,
        Ability::GreatLawOfPeace => 433,
        Ability::ContestedSteppe { .. } => 434,
        Ability::ZagrosAscent { .. } => 436,
        Ability::RegionalCommands { .. } => 437,
        Ability::NyabingiNetwork { .. } => 438,
        Ability::AttackStrength { against, .. } => 100 + target_index(against),
        Ability::ConquestSpeed { against, .. } => 120 + target_index(against),
        Ability::DefenseStrength { against, .. } => 300 + target_index(against),
        Ability::EnemyRetreatPenalty { against, .. } => 140 + target_index(against),
        Ability::ResourceBonus {
            source: Source::District(_),
            ..
        } => 1,
        Ability::CityTroopCapacity { .. } => 2,
        Ability::TowerRange { .. } => 3,
        Ability::TowerLoss {
            side: Side::Defender,
            ..
        } => 4,
        Ability::ResourceBonus {
            source: Source::Elimination,
            resource: Resource::Gold,
            ..
        } => 5,
        Ability::ResourceBonus {
            source: Source::Elimination,
            resource: Resource::Troops,
            ..
        } => 27,
        Ability::ResourceBonus {
            source: Source::Territory,
            resource: Resource::Gold,
            ..
        } => 6,
        Ability::ResourceBonus {
            source: Source::Territory,
            resource: Resource::Troops,
            ..
        } => 26,
        Ability::CityCapture {
            side: Side::Defender,
            ..
        } => 8,
        Ability::CityCapture {
            side: Side::Attacker,
            ..
        } => 9,
        Ability::CityDefense { against, .. } => 240 + target_index(against),
        Ability::AllianceCost { against, .. } => 200 + target_index(against),
        Ability::WarshipCost { .. } => 42,
        Ability::ResourceBonus {
            source: Source::Alliance,
            resource: Resource::Gold,
            ..
        } => 11,
        Ability::ResourceBonus {
            source: Source::Alliance,
            resource: Resource::Troops,
            ..
        } => 25,
        Ability::AttackerLoss {
            when: LossWhen::Mountain,
            ..
        } => 15,
        Ability::TowerLoss {
            side: Side::Attacker,
            ..
        } => 16,
        Ability::AttackerLoss {
            when: LossWhen::RecaptureWindow,
            ..
        } => 17,
        Ability::AttackerLoss {
            when: LossWhen::Retreat,
            ..
        } => 22,
        Ability::AttackerLoss {
            when: LossWhen::Highland,
            ..
        } => 23,
        Ability::AttackerLoss {
            when: LossWhen::City,
            ..
        } => 29,
        Ability::AttackerLoss {
            when: LossWhen::Coast,
            ..
        } => 30,
        Ability::AttackerLoss {
            when: LossWhen::Landing,
            ..
        } => 31,
        Ability::AttackerLoss {
            when: LossWhen::JointFront,
            ..
        } => 424,
        Ability::AttackerLoss {
            when: LossWhen::SeaRetreat,
            ..
        } => 435,
        Ability::AttackerLoss {
            when: LossWhen::Against(against),
            ..
        } => 160 + target_index(against),
        Ability::ResourceBonus {
            source: Source::Trade,
            resource: Resource::Gold,
            ..
        } => 24,
        Ability::ResourceBonus {
            source: Source::Trade,
            resource: Resource::Troops,
            ..
        } => 28,
    }
}

/// Position of `target` in `Target::ALL`, so each target gets its own experiment.
fn target_index(target: Target) -> u16 {
    Target::ALL
        .iter()
        .position(|&listed| listed == target)
        .expect("every target is listed") as u16
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
    // Ability gaps are measured over a fixed tick window at the previous standard speed.
    config.global_speed_multiplier = 0.35;
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

/// Ability marks shown per owner, indexed by owner id (the arena has owners 0 to 3).
type Marks = [usize; 4];

/// The marks the events `engine` holds now show, per owner.
fn owner_marks(engine: &SowEngine) -> Marks {
    let mut marks = [0; 4];
    for event in &engine.state.events {
        if let GameEvent::AbilityTriggered { owner_id, .. } = event
            && let Some(count) = marks.get_mut(usize::from(*owner_id))
        {
            *count += 1;
        }
    }
    marks
}

/// Ticks `engine`, adding the marks each owner shows to `marks`. The events an action
/// left before the first tick count too, since a tick clears them.
fn tick_counting(engine: &mut SowEngine, ticks: u32, marks: &mut Marks) {
    add_marks(marks, owner_marks(engine));
    for _ in 0..ticks {
        engine.tick();
        add_marks(marks, owner_marks(engine));
    }
}

fn add_marks(total: &mut Marks, more: Marks) {
    for (count, extra) in total.iter_mut().zip(more) {
        *count += extra;
    }
}

/// Attacks from player 1 for `ticks` ticks and returns the marks shown.
fn attack(engine: &mut SowEngine, target_owner: u16, ticks: u32) -> Marks {
    attack_with_rival_troops(engine, target_owner, ticks, 20_000.0)
}

/// `attack` against a rival that holds `rival_troops`.
fn attack_with_rival_troops(
    engine: &mut SowEngine,
    target_owner: u16,
    ticks: u32,
    rival_troops: f64,
) -> Marks {
    attack_with_troops(
        engine,
        target_owner,
        ticks,
        20_000.0,
        10_000.0,
        rival_troops,
    )
}

fn attack_with_troops(
    engine: &mut SowEngine,
    target_owner: u16,
    ticks: u32,
    attacker_troops: f64,
    army: f64,
    rival_troops: f64,
) -> Marks {
    engine.state.player_mut(1).unwrap().troops = attacker_troops;
    if let Some(defender) = engine.state.player_mut(2) {
        defender.troops = rival_troops;
    }
    engine.apply_intents(&[StampedIntent {
        player_id: 1,
        intent: GameplayIntent::Attack(AttackIntent {
            target_owner,
            troops: Some(army),
        }),
    }]);
    let mut marks = [0; 4];
    tick_counting(engine, ticks, &mut marks);
    marks
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
        District::City => {
            agg.city_levels = 3;
            agg.city_income_level_weight = 3.0;
        }
        District::Factory => {
            agg.factory_levels = 3;
            agg.factory_income_level_weight = 3.0;
        }
        District::Port => {
            agg.port_levels = 3;
            agg.port_income_level_weight = 3.0;
        }
        District::Farm => {
            agg.farm_levels = 3;
            agg.farm_income_level_weight = 3.0;
        }
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

/// Whether a tower of player 2 still defends a tile eight hexes away.
fn tower_reaches(defender: Leader) -> bool {
    let mut engine = arena(48, 1, Leader::of("caesar"), Some(defender));
    add_building(&mut engine, 2, 10, 10, BuildingKind::Tower);
    engine.refresh_defense_grid();
    let width = engine.state.map.width;
    engine
        .defense_grid
        .influence(18, 10, width, 2, 1, engine.state.seed, &engine.state.config)
        .priority_bonus
        > 0.0
}

/// Troops the attacker has left after hitting a tower-defended border.
fn attacker_troops_against_tower(defender: Leader) -> (f64, Marks) {
    let mut engine = arena(24, 4, Leader::of("cleopatra"), Some(defender));
    // Amplify the tower signal so a 50% hero modifier is measurable over this short fight.
    engine.state.config.tower_strength = 1.0;
    for y in [2, 12, 22] {
        add_building(&mut engine, 2, 6, y, BuildingKind::Tower);
    }
    engine.refresh_defense_grid();
    let marks = attack(&mut engine, 2, 20);
    (attacker_troop_total(&engine), marks)
}

fn elimination_gold(conqueror: Leader) -> (f64, Marks) {
    let mut engine = arena(12, 6, conqueror, Some(Leader::of("cleopatra")));
    engine.state.player_mut(2).unwrap().player_type = PlayerType::Bot;
    let before = engine.state.player(1).unwrap().gold;
    engine.eliminate_player(2, 1, 0, 0, false);
    (
        engine.state.player(1).unwrap().gold - before,
        owner_marks(&engine),
    )
}

fn territory_gold(leader: Leader) -> f64 {
    gold_income_per_second(
        2_048,
        BuildingAggregate::default(),
        leader,
        &GameConfig::default(),
    )
}

fn alliance_gold(leader: Leader) -> f64 {
    alliance_gold_per_second(3, leader, &GameConfig::default())
}

fn alliance_troops(leader: Leader) -> f64 {
    alliance_troops_per_second(3, leader, &GameConfig::default())
}

fn territory_troops(leader: Leader) -> f64 {
    troop_income_per_second(
        2_048,
        BuildingAggregate::default(),
        leader,
        &GameConfig::default(),
    )
}

fn trade_gold(leader: Leader) -> f64 {
    trade_income_per_second(1, leader, &GameConfig::default())
}

fn trade_troops(leader: Leader) -> f64 {
    trade_troops_per_second(1, leader, &GameConfig::default())
}

/// Marks `leader` shows when it forms a campaign alliance with a rival.
fn alliance_marks(leader: Leader) -> usize {
    let mut engine = arena(32, 16, leader, Some(Leader::of("cleopatra")));
    send(
        &mut engine,
        1,
        GameplayIntent::ProposeAlliance { target_player: 2 },
    );
    send(
        &mut engine,
        2,
        GameplayIntent::AcceptAlliance { target_player: 1 },
    );
    owner_marks(&engine)[1]
}

fn armistice_refund(leader: Leader, attacker: u16) -> (f64, usize) {
    let mut engine = arena(32, 16, leader, Some(Leader::of("cleopatra")));
    engine.state.player_mut(attacker).unwrap().troops = 2_000.0;
    send(
        &mut engine,
        attacker,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 3 - attacker,
            troops: Some(1_000.0),
        }),
    );
    assert_eq!(engine.attacks.len(), 1);
    send(
        &mut engine,
        1,
        GameplayIntent::ProposeAlliance { target_player: 2 },
    );
    send(
        &mut engine,
        2,
        GameplayIntent::AcceptAlliance { target_player: 1 },
    );
    assert!(engine.attacks[0].retreating);
    let before = engine.state.player(attacker).unwrap().troops;
    engine.execute_combat();
    let refund = engine.state.player(attacker).unwrap().troops - before;
    if matches!(leader.ability(), Ability::Armistice { .. }) {
        assert_eq!(refund, 1_000.0);
        assert!(engine.state.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityTriggered { owner_id: 1, tile: Some(_), affected }
                if affected.contains(&2)
        )));
    }
    (refund, owner_marks(&engine)[1])
}

/// Marks `leader` shows when one of its trade ships leaves port.
fn trade_marks(leader: Leader) -> usize {
    let mut engine = engine_with_ports(
        &[port(1, 1, 10), port(2, 2, 20)],
        vec![lane(1, 1, 2, &[100, 101])],
    );
    engine.state.player_mut(1).unwrap().leader = leader;
    engine.execute_trade_ships();
    owner_marks(&engine)[1]
}

fn district_building(district: District) -> BuildingKind {
    match district {
        District::City => BuildingKind::City,
        District::Factory => BuildingKind::Factory,
        District::Port => BuildingKind::Port,
        District::Farm => BuildingKind::Farm,
    }
}

/// Marks `leader` shows when one of its own `kind` buildings finishes construction.
fn completion_marks(leader: Leader, kind: BuildingKind) -> usize {
    let mut engine = arena(24, 24, leader, None);
    engine.add_building(Building {
        id: 1,
        owner_id: 1,
        tile_idx: 10,
        kind,
        level: 1,
        under_construction: true,
        ticks_until_complete: 1,
    });
    engine.execute_construction();
    owner_marks(&engine)[1]
}

/// Troops the conqueror gains by eliminating a bot, and the marks shown.
fn elimination_troops(conqueror: Leader) -> (f64, Marks) {
    let mut engine = arena(12, 6, conqueror, Some(Leader::of("cleopatra")));
    engine.state.player_mut(2).unwrap().player_type = PlayerType::Bot;
    engine.state.player_mut(1).unwrap().max_troops = 1.0e9;
    let before = engine.state.player(1).unwrap().troops;
    engine.eliminate_player(2, 1, 0, 0, false);
    (
        engine.state.player(1).unwrap().troops - before,
        owner_marks(&engine),
    )
}

/// Makes player `subject` (1 or 2) fit `target` against the other player. Sizes are set by
/// the caller's map split; gold, type, traitor status and people are set here.
fn shape_subject(engine: &mut SowEngine, subject: u16, target: Target) {
    let actor = if subject == 1 { 2 } else { 1 };
    match target {
        // An AI rival (tribe or nation) is kept static so its counterattack cannot swamp
        // the attacker's own tile count during the measurement window.
        Target::Tribe | Target::Nation => {
            let kind = if target == Target::Tribe {
                PlayerType::Bot
            } else {
                PlayerType::Nation
            };
            engine.state.player_mut(subject).unwrap().player_type = kind;
            engine.campaign_halted_ids.insert(subject);
        }
        Target::ActiveTraitor => {
            let rival = engine.state.player_mut(subject).unwrap();
            rival.traitor = true;
            rival.traitor_tick = 10_000;
        }
        Target::Richer | Target::Poorer => {
            let (rich, poor) = if target == Target::Richer {
                (subject, actor)
            } else {
                (actor, subject)
            };
            engine.state.player_mut(rich).unwrap().gold = 1_000.0;
            engine.state.player_mut(poor).unwrap().gold = 0.0;
        }
        // The subject gets the actor's own hero, so both lead the same people.
        Target::Kin => {
            let leader = engine.state.player(actor).unwrap().leader;
            engine.state.player_mut(subject).unwrap().leader = leader;
        }
        // The subject keeps its hero unless that hero leads the actor's people.
        Target::Foreign => {
            let people = engine.state.player(actor).unwrap().leader.civilization();
            let foreigner = Leader::all()
                .find(|leader| leader.civilization() != people)
                .expect("the catalog has heroes of more than one people");
            let rival = engine.state.player_mut(subject).unwrap();
            if rival.leader.civilization() == people {
                rival.leader = foreigner;
            }
        }
        // Sizes and troops come from the caller; any player fits the rest.
        Target::Anyone
        | Target::Neutral
        | Target::OtherPlayer
        | Target::Human
        | Target::Outnumbering
        | Target::Outnumbered
        | Target::Larger
        | Target::Smaller => {}
    }
}

/// Attacks `target` from `attacker` for 20 ticks. Neutral land is attacked directly; any
/// other target is a rival (player 2) shaped to fit it, so the ability's rule decides.
/// Returns the engine and the marks shown.
fn attack_target(attacker: Leader, target: Target) -> (SowEngine, Marks) {
    attack_target_for_ticks(attacker, target, 20)
}

fn attack_target_for_ticks(attacker: Leader, target: Target, ticks: u32) -> (SowEngine, Marks) {
    let mut engine = launch_target(attacker, target);
    let mut marks = [0; 4];
    tick_counting(&mut engine, ticks, &mut marks);
    (engine, marks)
}

/// The engine right after `attacker` launches its attack on `target`, before any tick.
fn launch_target(attacker: Leader, target: Target) -> SowEngine {
    let (mut engine, target_owner, rival_troops) = if target == Target::Neutral {
        (arena(24, 4, attacker, None), 0, 20_000.0)
    } else {
        // A smaller rival needs the attacker to own more tiles. On a wider map the rival still
        // holds enough land that the 20-tick attack is not capped by the map's size.
        let (width, own) = if target == Target::Smaller {
            (40, 22)
        } else {
            (24, 4)
        };
        let mut engine = arena(width, own, attacker, Some(Leader::of("caesar")));
        shape_subject(&mut engine, 2, target);
        let rival_troops = if target == Target::Outnumbering {
            20_000.0
        } else if target == Target::Outnumbered {
            5_000.0
        } else {
            20_000.0
        };
        (engine, 2, rival_troops)
    };
    if let Some(defender) = engine.state.player_mut(2) {
        defender.troops = rival_troops;
    }
    engine.state.player_mut(1).unwrap().troops = 20_000.0;
    let army = if target == Target::Outnumbering {
        1_000.0
    } else {
        10_000.0
    };
    engine.apply_intents(&[StampedIntent {
        player_id: 1,
        intent: GameplayIntent::Attack(AttackIntent {
            target_owner,
            troops: Some(army),
        }),
    }]);
    engine
}

/// Tiles the attacker holds after attacking `target`, and the marks shown.
fn tiles_against(attacker: Leader, target: Target) -> (u32, Marks) {
    let (engine, marks) = attack_target(attacker, target);
    (engine.state.player(1).unwrap().tile_count, marks)
}

/// Tiles kept by a nation attacking this defender, with the defender's marks.
fn tiles_against_defense(defender: Leader, target: Target) -> (u32, Marks) {
    let own = if target == Target::Larger { 13 } else { 4 };
    let mut engine = arena(24, own, Leader::of("caesar"), Some(defender));
    shape_subject(&mut engine, 1, target);
    let marks = attack_with_troops(&mut engine, 2, 20, 1_000.0, 500.0, 2_000.0);
    (engine.state.player(1).unwrap().tile_count, marks)
}

/// Troops `retreater` gets back when it retreats at once from `defender`'s land, with
/// the retreating player fitting `target`, and the marks the retreat shows.
fn retreat_against(retreater: Leader, defender: Leader, target: Target) -> (f64, Marks) {
    // A larger retreater needs the map split the other way.
    let own = if target == Target::Larger { 20 } else { 4 };
    let mut engine = arena(24, own, retreater, Some(defender));
    // A city lifts the home cap so the refund is not clipped by it.
    add_building(&mut engine, 1, 0, 0, BuildingKind::City);
    shape_subject(&mut engine, 1, target);
    engine.state.player_mut(1).unwrap().troops = if target == Target::Outnumbered {
        500.0
    } else {
        2_000.0
    };
    send(
        &mut engine,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(1_000.0),
        }),
    );
    let attack_id = engine
        .attacks
        .iter()
        .find(|attack| attack.owner_id == 1)
        .unwrap()
        .id;
    let before = engine.state.player(1).unwrap().troops;
    send(&mut engine, 1, GameplayIntent::CancelAttack { attack_id });
    let mut marks = [0; 4];
    tick_counting(&mut engine, 1, &mut marks);
    (engine.state.player(1).unwrap().troops - before, marks)
}

/// Tiles the attacker holds after hitting a city of `defender` (the city sits on the border),
/// and the marks shown.
fn tiles_taken_from_city(attacker: Leader, defender: Leader) -> (f64, Marks) {
    let mut engine = arena(24, 4, attacker, Some(defender));
    add_building(&mut engine, 2, 6, 12, BuildingKind::City);
    engine.refresh_building_grid();
    let marks = attack(&mut engine, 2, 20);
    (f64::from(engine.state.player(1).unwrap().tile_count), marks)
}

/// City capture against a defender, with the attacker shaped to fit a targeted ability.
fn tiles_taken_from_city_against(
    attacker: Leader,
    defender: Leader,
    target: Target,
) -> (f64, Marks) {
    let mut engine = arena(24, 4, attacker, Some(defender));
    shape_subject(&mut engine, 1, target);
    add_building(&mut engine, 2, 6, 12, BuildingKind::City);
    engine.refresh_building_grid();
    let marks = attack(&mut engine, 2, 20);
    (f64::from(engine.state.player(1).unwrap().tile_count), marks)
}

/// Exact troops player 1 has left: at home plus still attacking.
fn attacker_troop_total(engine: &SowEngine) -> f64 {
    let player = engine.state.player(1).unwrap();
    let attacking: f64 = engine
        .attacks
        .iter()
        .filter(|attack| attack.owner_id == 1)
        .map(|attack| attack.troops)
        .sum();
    player.troops + attacking
}

/// Rounded troops player 1 has left, for checks where a whole-troop comparison is enough.
fn attacker_troops_left(engine: &SowEngine) -> i64 {
    attacker_troop_total(engine).round() as i64
}

/// Troops the attacker keeps after hitting a tower on a plain rival's border.
fn tower_siege_troops(attacker: Leader) -> (f64, Marks) {
    let mut engine = arena(24, 4, attacker, Some(Leader::of("caesar")));
    engine.state.config.tower_strength = 1.0;
    for y in [2, 12, 22] {
        add_building(&mut engine, 2, 6, y, BuildingKind::Tower);
    }
    engine.refresh_defense_grid();
    let marks = attack(&mut engine, 2, 20);
    (attacker_troop_total(&engine), marks)
}

/// Player 2 takes player 1's border city through real attack intents (`victim` is player 1).
fn lose_city(victim: Leader) -> SowEngine {
    let mut engine = arena(24, 4, victim, Some(Leader::of("caesar")));
    add_building(&mut engine, 1, 3, 12, BuildingKind::City);
    engine.refresh_building_grid();
    engine.state.player_mut(2).unwrap().troops = 20_000.0;
    send(
        &mut engine,
        2,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 1,
            troops: Some(10_000.0),
        }),
    );
    // Stop the rival's army once it holds the city, so the victim keeps land to counterattack from.
    for _ in 0..1_000 {
        engine.tick();
        if engine.state.map.owner_id(3, 12) == 2 {
            break;
        }
    }
    let rival_attacks: Vec<u64> = engine
        .attacks
        .iter()
        .filter(|attack| attack.owner_id == 2)
        .map(|attack| attack.id)
        .collect();
    for attack_id in rival_attacks {
        send(&mut engine, 2, GameplayIntent::CancelAttack { attack_id });
    }
    engine.tick();
    engine
}

/// Troops `victim` keeps when it counterattacks the player that just took its city.
fn recapture_troops(victim: Leader) -> (f64, Marks) {
    let mut engine = lose_city(victim);
    let marks = attack(&mut engine, 2, 20);
    (attacker_troop_total(&engine), marks)
}

/// Troops `attacker` gets back when it retreats at once from `defender`'s land, and the marks.
fn retreat_refund(attacker: Leader, defender: Leader) -> (f64, Marks) {
    retreat_against(attacker, defender, Target::Anyone)
}

/// Gold `human` pays for a campaign alliance with a player that fits `target`, for an
/// alliance that asks 200 gold, and the marks shown.
fn alliance_price_against(human: Leader, target: Target) -> (f64, Marks) {
    // A larger partner needs the human to hold fewer tiles, a smaller one more.
    let own = match target {
        Target::Larger => 3,
        Target::Smaller => 9,
        _ => 6,
    };
    let mut engine = arena(12, own, human, Some(Leader::of("caesar")));
    shape_subject(&mut engine, 2, target);
    engine
        .campaign_relations
        .insert(2, CampaignRelation::Neutral);
    // The human can pay in every case; the gold shape above decides the Richer/Poorer rule.
    engine.state.player_mut(1).unwrap().gold = 300.0;
    engine.apply_intents(&[StampedIntent {
        player_id: 1,
        intent: GameplayIntent::ResolveCampaignDiplomacy {
            target_player: 2,
            relation: CampaignRelation::Allied,
            gold_cost: 200.0,
        },
    }]);
    (
        300.0 - engine.state.player(1).unwrap().gold,
        owner_marks(&engine),
    )
}

/// Every hero's situation changes the game and shows its mark there. The control hero
/// lacks the situation, so its mark must stay hidden: the mark belongs to the situation.
#[test]
fn every_hero_ability_changes_the_game_and_shows_its_mark() {
    let plain = Leader::of("cleopatra");
    let idle = Leader::of("caesar");
    for hero in Leader::all() {
        let base = baseline_for(hero);
        let ability = hero.ability();
        let (changed, shown, control) = match ability.situation() {
            Ability::Armistice { .. } => {
                let mut changed = true;
                let mut shown = 0;
                let mut control = 0;
                for attacker in [1, 2] {
                    let (refund, marks) = armistice_refund(hero, attacker);
                    let (ordinary, baseline_marks) = armistice_refund(plain, attacker);
                    changed &= refund > ordinary;
                    shown += marks;
                    control += baseline_marks;
                }
                (changed, shown, control)
            }
            Ability::CoastalFounding
            | Ability::AlliedCityGrant { .. }
            | Ability::CityTowerUpgrade
            | Ability::CityFarmFounding
            | Ability::CityDenial
            | Ability::PuntGardens
            | Ability::LastCityRelocation
            | Ability::WarRegalia { .. }
            | Ability::Reunification { .. }
            | Ability::FactoryCityUpgradeDiscount { .. }
            | Ability::FactoryRaid { .. }
            | Ability::ImperialStandards { .. }
            | Ability::SiegeBreach
            | Ability::CookhamExchange
            | Ability::SecondCrown
            | Ability::KitchenPhilosophy
            | Ability::MountedCapture { .. }
            | Ability::AksumRout
            | Ability::TigranesAid { .. }
            | Ability::AkkadGarrison
            | Ability::SharedFeast
            | Ability::GreatLawOfPeace
            | Ability::ContestedSteppe { .. }
            | Ability::ZagrosAscent { .. }
            | Ability::RegionalCommands { .. }
            | Ability::NyabingiNetwork { .. }
            | Ability::CounterLevy { .. }
            | Ability::SharedCitySpoils { .. }
            | Ability::CityEvacuation { .. }
            | Ability::CityIntercession { .. }
            | Ability::RetreatHandover { .. }
            | Ability::ReturnCampaign { .. }
            | Ability::AllyPatronage { .. }
            | Ability::SurplusTreasury { .. }
            | Ability::GuildNetwork { .. }
            | Ability::WarshipPrize { .. } => {
                let (value, shown) = redesigned_effect(hero, ability);
                let (plain_value, control) = redesigned_effect(base, ability);
                (value > plain_value, shown, control)
            }

            Ability::AttackStrength { against, .. } => {
                let ticks = if against == Target::Neutral { 4 } else { 20 };
                let (strengthened, shown) = attack_target_for_ticks(hero, against, ticks);
                let (control, control_shown) = attack_target_for_ticks(plain, against, ticks);
                let changed = strengthened.state.player(1).unwrap().tile_count
                    > control.state.player(1).unwrap().tile_count
                    || attacker_troops_left(&strengthened) > attacker_troops_left(&control);
                (changed, shown[1], control_shown[1])
            }
            Ability::DefenseStrength { against, .. } => {
                let (tiles, shown) = tiles_against_defense(hero, against);
                let (control_tiles, control_shown) = tiles_against_defense(plain, against);
                (tiles < control_tiles, shown[2], control_shown[2])
            }
            Ability::ConquestSpeed { against, .. } => {
                // Measure early: otherwise a heavily favored attack can nearly clear the board
                // before a speed difference becomes visible, or the rival can stop qualifying.
                let (strengthened, shown) = attack_target_for_ticks(hero, against, 4);
                let (control, control_shown) = attack_target_for_ticks(plain, against, 4);
                let tiles = strengthened.state.player(1).unwrap().tile_count;
                let control_tiles = control.state.player(1).unwrap().tile_count;
                (tiles > control_tiles, shown[1], control_shown[1])
            }
            Ability::EnemyRetreatPenalty { against, .. } => {
                let (refund, shown) = retreat_against(plain, hero, against);
                let (control_refund, control_shown) = retreat_against(plain, plain, against);
                (refund < control_refund, shown[2], control_shown[2])
            }
            Ability::ResourceBonus {
                source: Source::District(district),
                resource,
                ..
            } => (
                district_income(hero, district, resource)
                    > district_income(base, district, resource),
                completion_marks(hero, district_building(district)),
                completion_marks(idle, district_building(district)),
            ),
            Ability::CityTroopCapacity { .. } => (
                max_troops_with_cities(hero) > max_troops_with_cities(base),
                completion_marks(hero, BuildingKind::City),
                completion_marks(idle, BuildingKind::City),
            ),
            Ability::TowerRange { .. } => (
                tower_reaches(hero) && !tower_reaches(base),
                completion_marks(hero, BuildingKind::Tower),
                completion_marks(idle, BuildingKind::Tower),
            ),
            Ability::TowerLoss {
                side: Side::Defender,
                ..
            } => {
                let (troops, shown) = attacker_troops_against_tower(hero);
                let (base_troops, _) = attacker_troops_against_tower(base);
                let (_, control_shown) = attacker_troops_against_tower(plain);
                (troops < base_troops, shown[2], control_shown[2])
            }
            Ability::ResourceBonus {
                source: Source::Elimination,
                resource: Resource::Gold,
                ..
            } => {
                let (gold, shown) = elimination_gold(hero);
                let (base_gold, _) = elimination_gold(base);
                let (_, control_shown) = elimination_gold(plain);
                (gold > base_gold, shown[1], control_shown[1])
            }
            Ability::ResourceBonus {
                source: Source::Elimination,
                resource: Resource::Troops,
                ..
            } => {
                let (troops, shown) = elimination_troops(hero);
                let (base_troops, _) = elimination_troops(base);
                let (_, control_shown) = elimination_troops(plain);
                (troops > base_troops, shown[1], control_shown[1])
            }
            Ability::ResourceBonus {
                source: Source::Territory,
                resource: Resource::Gold,
                ..
            } => (
                territory_gold(hero) > territory_gold(base),
                attack_target(hero, Target::Neutral).1[1],
                attack_target(plain, Target::Neutral).1[1],
            ),
            Ability::ResourceBonus {
                source: Source::Territory,
                resource: Resource::Troops,
                ..
            } => (
                territory_troops(hero) > territory_troops(base),
                attack_target(hero, Target::Neutral).1[1],
                attack_target(plain, Target::Neutral).1[1],
            ),
            Ability::CityCapture {
                side: Side::Defender,
                ..
            } => {
                let (taken, shown) = tiles_taken_from_city(plain, hero);
                let (base_taken, control_shown) = tiles_taken_from_city(plain, plain);
                (taken < base_taken, shown[2], control_shown[2])
            }
            Ability::CityCapture {
                side: Side::Attacker,
                ..
            } => {
                let (taken, shown) = tiles_taken_from_city(hero, plain);
                let (base_taken, control_shown) = tiles_taken_from_city(plain, plain);
                (taken > base_taken, shown[1], control_shown[1])
            }
            Ability::CityDefense { against, .. } => {
                let (taken, shown) = tiles_taken_from_city_against(plain, hero, against);
                let (base_taken, control_shown) =
                    tiles_taken_from_city_against(plain, plain, against);
                (taken < base_taken, shown[2], control_shown[2])
            }
            Ability::AllianceCost { against, .. } => {
                let (price, shown) = alliance_price_against(hero, against);
                let (plain_price, control_shown) = alliance_price_against(plain, against);
                (price < plain_price, shown[1], control_shown[1])
            }
            Ability::WarshipCost { .. } => {
                let (price, shown) = warship_purchase(hero);
                let (plain_price, control_shown) = warship_purchase(plain);
                (price < plain_price, shown, control_shown)
            }
            Ability::ResourceBonus {
                source: Source::Alliance,
                resource: Resource::Gold,
                ..
            } => (
                alliance_gold(hero) > alliance_gold(base),
                alliance_marks(hero),
                alliance_marks(plain),
            ),
            Ability::ResourceBonus {
                source: Source::Alliance,
                resource: Resource::Troops,
                ..
            } => (
                alliance_troops(hero) > alliance_troops(base),
                alliance_marks(hero),
                alliance_marks(plain),
            ),
            Ability::ResourceBonus {
                source: Source::Trade,
                resource: Resource::Gold,
                ..
            } => (
                trade_gold(hero) > trade_gold(base),
                trade_marks(hero),
                trade_marks(plain),
            ),
            Ability::ResourceBonus {
                source: Source::Trade,
                resource: Resource::Troops,
                ..
            } => (
                trade_troops(hero) > trade_troops(base),
                trade_marks(hero),
                trade_marks(plain),
            ),
            Ability::AttackerLoss {
                when: LossWhen::Mountain,
                ..
            } => {
                let (cost, shown) = cost_per_tile(hero, MOUNTAIN);
                let (plain_cost, control_shown) = cost_per_tile(plain, MOUNTAIN);
                (cost < plain_cost, shown[1], control_shown[1])
            }
            Ability::TowerLoss {
                side: Side::Attacker,
                ..
            } => {
                let (siege, shown) = tower_siege_troops(hero);
                let (plain_siege, control_shown) = tower_siege_troops(plain);
                (siege > plain_siege, shown[1], control_shown[1])
            }
            Ability::AttackerLoss {
                when: LossWhen::RecaptureWindow,
                ..
            } => {
                let (troops, shown) = recapture_troops(hero);
                let (plain_troops, control_shown) = recapture_troops(plain);
                (troops > plain_troops, shown[1], control_shown[1])
            }
            Ability::AttackerLoss {
                when: LossWhen::Retreat,
                ..
            } => {
                let (refund, shown) = retreat_refund(hero, plain);
                let (plain_refund, control_shown) = retreat_refund(plain, plain);
                (refund > plain_refund, shown[1], control_shown[1])
            }
            Ability::AttackerLoss {
                when: LossWhen::Highland,
                ..
            } => {
                let (cost, shown) = cost_per_tile(hero, HIGHLAND);
                let (plain_cost, control_shown) = cost_per_tile(plain, HIGHLAND);
                (cost < plain_cost, shown[1], control_shown[1])
            }
            Ability::AttackerLoss {
                when: LossWhen::City,
                ..
            } => {
                let (cost, shown) = city_cost_per_tile(hero);
                let (plain_cost, control_shown) = city_cost_per_tile(plain);
                (cost < plain_cost, shown[1], control_shown[1])
            }
            Ability::AttackerLoss {
                when: LossWhen::Coast,
                ..
            } => {
                let (cost, shown) = cost_per_tile(hero, COASTAL);
                let (plain_cost, control_shown) = cost_per_tile(plain, COASTAL);
                (cost < plain_cost, shown[1], control_shown[1])
            }
            Ability::AttackerLoss {
                when: LossWhen::Landing,
                ..
            } => {
                let (cost, shown) = landing_cost_per_tile(hero);
                let (plain_cost, control_shown) = landing_cost_per_tile(plain);
                (cost < plain_cost, shown[1], control_shown[1])
            }
            Ability::AttackerLoss {
                when: LossWhen::SeaRetreat,
                ..
            } => {
                let (troops, shown) = sea_retreat_refund(hero, true, false);
                let (ordinary, control_shown) = sea_retreat_refund(plain, true, false);
                assert_eq!(sea_retreat_refund(hero, false, false), (9_750.0, 0));
                assert_eq!(sea_retreat_refund(hero, true, true), (9_500.0, 0));
                (
                    troops == 10_000.0 && ordinary == 9_750.0,
                    shown,
                    control_shown,
                )
            }
            Ability::AttackerLoss {
                when: LossWhen::JointFront,
                ..
            } => {
                use super::hero_effects::AbilityState::{Conditional, Ready};
                let run = |leader: Leader, adjacent: bool, allied_attack: bool| {
                    let active =
                        leader.has_attacker_loss(LossWhen::JointFront) && adjacent && allied_attack;
                    let mut battle = arena(12, 6, leader, Some(Leader::of("cleopatra")));
                    battle.state.config.global_speed_multiplier = 100.0;
                    battle.state.config.attack_cost_enemy = 100.0;
                    for y in 0..12 {
                        for x in 6..9 {
                            if (x, y) != (6, 6) {
                                battle.state.set_tile_owner(x, y, 0);
                            }
                        }
                    }
                    if adjacent {
                        battle.state.set_tile_owner(6, 7, 3);
                    } else {
                        battle.state.set_tile_owner(8, 6, 3);
                    }
                    let attacker = battle.state.player_mut(1).unwrap();
                    attacker.troops = 10_000.0;
                    attacker.alliances.push(3);
                    let ally = battle.state.player_mut(3).unwrap();
                    ally.troops = 10_000.0;
                    ally.alliances.push(1);
                    battle.state.player_mut(2).unwrap().troops = 2000.0;
                    send(
                        &mut battle,
                        1,
                        GameplayIntent::Attack(AttackIntent {
                            target_owner: 2,
                            troops: Some(8000.0),
                        }),
                    );
                    if allied_attack {
                        send(
                            &mut battle,
                            3,
                            GameplayIntent::Attack(AttackIntent {
                                target_owner: 2,
                                troops: Some(8000.0),
                            }),
                        );
                        assert!(battle.attacks.iter().any(|attack| attack.owner_id == 3));
                    }
                    if leader.has_attacker_loss(LossWhen::JointFront) {
                        assert_eq!(
                            battle.hero_ability_feedback(1).unwrap().state,
                            if active { Ready } else { Conditional }
                        );
                    }
                    battle.execute_combat();
                    let troops = battle
                        .state
                        .events
                        .iter()
                        .find_map(|event| match event {
                            GameEvent::TileCaptured {
                                x: 6,
                                y: 6,
                                new_owner: 1,
                                troops,
                                ..
                            } => Some(*troops),
                            _ => None,
                        })
                        .expect("the land attack must conquer the shared frontier");
                    if active {
                        assert!(battle.state.events.iter().any(|event| matches!(
                            event,
                            GameEvent::AbilityTriggered { owner_id: 1, tile: Some((6, 6)), affected }
                                if affected == &[2, 3]
                        )));
                    }
                    (troops, owner_marks(&battle)[1])
                };
                let (supported, shown) = run(hero, true, true);
                let (ordinary, control) = run(plain, true, true);
                assert_eq!(supported - ordinary, 25.0);
                assert_eq!(run(hero, true, false), (ordinary, 0));
                assert_eq!(run(hero, false, true), (ordinary, 0));
                (supported > ordinary, shown, control)
            }
            Ability::AttackerLoss {
                when: LossWhen::Against(against),
                ..
            } => {
                // The mark comes from the tiles the loss discounts, not from the launch.
                let (cost, shown) = cost_per_tile_against(hero, against);
                let (plain_cost, control_shown) = cost_per_tile_against(plain, against);
                (cost < plain_cost, shown[1], control_shown[1])
            }
        };
        assert!(
            changed,
            "{hero:?}'s ability {ability:?} has no effect in the game"
        );
        assert!(
            shown >= 1,
            "{hero:?} does not show its mark in its situation {ability:?}"
        );
        assert_eq!(
            control, 0,
            "a hero without {ability:?} shows its mark there"
        );
    }
}

/// Build one real warship through the authoritative intent path and return its price
/// and ability marks. The all-water map gives the port and target one connected sea.
fn warship_purchase(leader: Leader) -> (f64, usize) {
    let mut engine = engine_with_ports(&[port(1, 1, 10 * 64 + 10)], vec![]);
    engine.state.player_mut(1).unwrap().leader = leader;
    engine.state.player_mut(1).unwrap().gold = 1_000.0;
    engine.state.map.terrain.fill(MapTile::from_byte(0x20));
    engine.state.map.terrain[10 * 64 + 10] = MapTile::from_byte(0x80);
    engine.state.map.compute_shorelines();
    engine.water = WaterComponents::compute(&engine.state.map, |_| {});
    send(
        &mut engine,
        1,
        GameplayIntent::BuildShip {
            target_tile: 20 * 64 + 20,
            kind: UnitType::Warship,
        },
    );
    assert!(
        engine
            .state
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::WarshipSpawned { owner_id: 1, .. })),
        "{leader:?} warship purchase failed: {:?}",
        engine.state.events
    );
    let cost = 1_000.0 - engine.state.player(1).unwrap().gold;
    let marks = owner_marks(&engine)[1];
    (cost, marks)
}

#[test]
fn every_hero_has_a_catalog_ability() {
    for hero in Leader::all() {
        let ability = hero.ability();
        // Fixed actions and resource amounts have no percentage. Validate their
        // actual magnitude instead of inventing a percentage for them.
        let magnitude = match ability {
            Ability::CoastalFounding
            | Ability::CityTowerUpgrade
            | Ability::CityFarmFounding
            | Ability::CityDenial
            | Ability::PuntGardens
            | Ability::LastCityRelocation
            | Ability::CookhamExchange
            | Ability::SecondCrown
            | Ability::KitchenPhilosophy
            | Ability::AksumRout
            | Ability::AkkadGarrison
            | Ability::SharedFeast
            | Ability::GreatLawOfPeace
            | Ability::SiegeBreach => 1,
            Ability::AlliedCityGrant { gold } | Ability::SharedCitySpoils { gold } => gold,
            Ability::CityEvacuation { troops }
            | Ability::CityIntercession { troops }
            | Ability::WarRegalia { troops } => troops,
            Ability::SurplusTreasury { troops_per_gold } => troops_per_gold,
            Ability::FactoryRaid { troops_per_level } => troops_per_level,
            Ability::GuildNetwork { gold_per_second } => gold_per_second,
            Ability::TigranesAid { max_troops } => max_troops,
            _ => ability.percent(),
        };
        assert!(magnitude > 0, "{hero:?} has a zero-strength ability");
    }
}

/// No two heroes share a situation (the same kind against the same target, side,
/// source or loss case, whatever the percent): a shared one would make the mark and
/// the balance ambiguous.
#[test]
fn every_hero_has_a_unique_situation() {
    let mut seen: Vec<(Ability, Leader)> = Vec::new();
    for hero in Leader::all() {
        let situation = hero.ability().situation();
        if let Some((_, other)) = seen.iter().find(|(taken, _)| *taken == situation) {
            panic!("{hero:?} and {other:?} share the situation {situation:?}");
        }
        seen.push((situation, hero));
    }
}

#[test]
fn al_ghafiqi_has_a_mountain_campaign_ability() {
    let hero = Leader::of("abd_al_rahman_al_ghafiqi");
    assert_eq!(
        hero.ability(),
        Ability::AttackerLoss {
            when: LossWhen::Mountain,
            percent: 25,
        }
    );
    assert_eq!(hero.attacker_loss_multiplier(LossWhen::Mountain), 0.75);
    assert_eq!(hero.attacker_loss_multiplier(LossWhen::Highland), 1.0);
}

#[test]
fn windradyne_defends_against_a_larger_rival() {
    let hero = Leader::of("tribe_wiradjuri");
    assert_eq!(
        hero.ability(),
        Ability::DefenseStrength {
            against: Target::Larger,
            percent: 15,
        }
    );

    let (tiles, marks) = tiles_against_defense(hero, Target::Larger);
    let (control_tiles, control_marks) =
        tiles_against_defense(Leader::of("caesar"), Target::Larger);
    assert!(
        tiles < control_tiles,
        "the bonus reduces the attacker's gains: hero {tiles} {marks:?}, control {control_tiles} {control_marks:?}"
    );
    assert!(marks[2] >= 1, "Windradyne shows the defense mark");
    assert_eq!(
        control_marks[2], 0,
        "a hero without the ability shows no mark"
    );
}

#[test]
fn yagan_attacks_foreign_people_with_strength() {
    let hero = Leader::of("nation_yagan");
    assert_eq!(
        hero.ability(),
        Ability::AttackStrength {
            against: Target::Foreign,
            percent: 15,
        }
    );
    assert_eq!(hero.civilization(), "Whadjuk Noongar");

    let (empowered, marks) = attack_target(hero, Target::Foreign);
    let (control, control_marks) = attack_target(Leader::of("caesar"), Target::Foreign);
    assert!(
        empowered.state.player(1).unwrap().tile_count > control.state.player(1).unwrap().tile_count
            || attacker_troops_left(&empowered) > attacker_troops_left(&control),
        "Yagan's attack bonus changes the battle"
    );
    assert_eq!(marks[1], 1, "Yagan shows the attack mark");
    assert_eq!(
        control_marks[1], 0,
        "a hero without the ability shows no mark"
    );
}

fn send(engine: &mut SowEngine, player_id: u16, intent: GameplayIntent) {
    engine.apply_intents(&[StampedIntent { player_id, intent }]);
}

/// The owner's live test, start to finish through real intents: player 1 (`hero`)
/// and player 2 ally, player 2 betrays (break, plus an attack when `traitor_attacks`,
/// as the betrayal button does), `delay` ticks pass, then player 1 attacks back with
/// half its army for 10 ticks (1 s). Returns (tiles player 1 gained, troops player 2
/// lost, whether player 2 wore the traitor badge — the red border — when hit).
fn revenge(hero: Leader, delay: u32, traitor_attacks: bool) -> (i64, i64, bool, bool) {
    let mut engine = arena(32, 16, hero, Some(Leader::of("cleopatra")));
    engine.execute_income();
    send(
        &mut engine,
        1,
        GameplayIntent::ProposeAlliance { target_player: 2 },
    );
    send(
        &mut engine,
        2,
        GameplayIntent::AcceptAlliance { target_player: 1 },
    );
    assert!(
        engine.state.player(1).unwrap().alliances.contains(&2),
        "alliance never formed"
    );
    send(
        &mut engine,
        2,
        GameplayIntent::BreakAlliance { target_player: 1 },
    );
    if traitor_attacks {
        let half = engine.state.player(2).unwrap().troops * 0.5;
        send(
            &mut engine,
            2,
            GameplayIntent::Attack(AttackIntent {
                target_owner: 1,
                troops: Some(half),
            }),
        );
    }
    for _ in 0..delay {
        engine.tick();
    }
    let tiles_before = engine.state.player(1).unwrap().tile_count as i64;
    let traitor_before = engine.state.player(2).unwrap().troops;
    let badge = engine
        .build_snapshot()
        .players
        .iter()
        .any(|p| p.id == 2 && p.traitor);
    let half = engine.state.player(1).unwrap().troops * 0.5;
    let fights_harder = targeted_multiplier(&engine.state, 1, 2, half, |target| {
        hero.attack_strength_multiplier(target)
    }) > 1.0;
    send(
        &mut engine,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(half),
        }),
    );
    for _ in 0..10 {
        engine.tick();
    }
    (
        engine.state.player(1).unwrap().tile_count as i64 - tiles_before,
        (traitor_before - engine.state.player(2).unwrap().troops).round() as i64,
        badge,
        fights_harder,
    )
}

/// Boudica, as an online match builds her (account choice → match roster → engine): her
/// attacks fight twice as strong against a rival with fewer troops than her army, and the
/// same attack from a plain hero does not.
#[test]
fn boudica_strikes_outnumbered_rivals_in_a_real_match() {
    let boudica = Leader::of("boudica");
    assert_eq!(
        boudica.ability(),
        Ability::AttackStrength {
            against: Target::Outnumbered,
            percent: 100
        }
    );
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

    let plain = Leader::of("cleopatra");
    let (strong, _) = attack_target(boudica, Target::Outnumbered);
    let (weak, _) = attack_target(plain, Target::Outnumbered);
    assert!(
        attacker_troops_left(&strong) > attacker_troops_left(&weak),
        "Boudica must keep more troops than a plain hero against an outnumbered rival"
    );
}

/// Artigas, "Broken Truce": his armies take a traitor's land faster, and only while the
/// traitor badge lasts.
#[test]
fn artigas_takes_traitor_land_faster_only_while_the_badge_lasts() {
    let artigas = Leader::of("artigas");
    let plain = Leader::of("cleopatra");
    let window = crate::diplomacy::TRAITOR_STATUS_TICKS;
    for (delay, marked) in [(window - 10, true), (window + 1, false)] {
        let (a, _, a_badge, _) = revenge(artigas, delay, false);
        let (p, _, p_badge, _) = revenge(plain, delay, false);
        assert_eq!(
            (a_badge, p_badge),
            (marked, marked),
            "badge at tick {delay}"
        );
        assert!(p > 0, "plain attack must take land at tick {delay}");
        if marked {
            assert!(
                a > p,
                "no faster conquest of a traitor while the badge is on"
            );
        } else {
            assert_eq!(a, p, "the bonus outlived the badge");
        }
    }
}

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

/// The same betrayal scene for every hero: only heroes whose attack strength applies to
/// the traitor in that scene win the counterattack clash; nobody else gains anything.
#[test]
fn every_hero_in_the_betrayal_scene() {
    for hero in Leader::all() {
        let (tiles, kills, _, fights_harder) = revenge(hero, 0, true);
        eprintln!(
            "{hero:?}: {:?} → {tiles} tiles, {kills} troops",
            hero.ability()
        );
        assert_eq!(tiles > 0, fights_harder, "{hero:?} in the betrayal scene");
    }
}

/// Player 1 (`hero`) attacks player 2 — a real tribe (`Bot`, tribe troops) or a
/// human — with half its army through the real attack intent, again and again
/// until player 2 falls. Returns (ticks to crush it, tiles taken in the first second).
fn crush(hero: Leader, victim: PlayerType) -> (u32, u32) {
    let mut engine = arena(32, 16, hero, Some(Leader::of("cleopatra")));
    let tribe = Player::new_bot(2, "Tribe".into(), [0.2; 3], &engine.state.config);
    let rival = engine.state.player_mut(2).unwrap();
    rival.player_type = victim;
    rival.troops = tribe.troops;
    engine.execute_income();
    let start = engine.state.player(1).unwrap().tile_count;
    let mut first_second = 0;
    for tick in 1..=600 {
        if !engine.attacks.iter().any(|a| a.owner_id == 1) {
            let half = engine.state.player(1).unwrap().troops * 0.5;
            send(
                &mut engine,
                1,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(half),
                }),
            );
        }
        engine.tick();
        if tick == 10 {
            first_second = engine.state.player(1).unwrap().tile_count - start;
        }
        if !engine.state.player(2).unwrap().alive || engine.state.player(2).unwrap().tile_count == 0
        {
            return (tick, first_second);
        }
    }
    (600, first_second)
}

/// Genghis Khan, "Horde Momentum": he fights tribes harder than a hero without
/// the ability and gets nothing extra against human players.
#[test]
fn genghis_khan_crushes_tribes() {
    let genghis = Leader::of("genghis_khan");
    assert_eq!(
        genghis.ability(),
        Ability::AttackStrength {
            against: Target::Tribe,
            percent: 50
        }
    );
    let plain = Leader::of("cleopatra");
    let (strong, _) = attack_target(genghis, Target::Tribe);
    let (weak, _) = attack_target(plain, Target::Tribe);
    assert!(
        attacker_troops_left(&strong) > attacker_troops_left(&weak),
        "no stronger fight against tribes"
    );

    let vs_human = crush(genghis, PlayerType::Human);
    let plain_vs_human = crush(plain, PlayerType::Human);
    eprintln!("vs human: Genghis Khan {vs_human:?}, plain {plain_vs_human:?}");
    assert_eq!(
        vs_human, plain_vs_human,
        "the tribe bonus leaked onto human players"
    );
}

const MOUNTAIN: u8 = 0x80 | 25;
const HIGHLAND: u8 = 0x80 | 15;
// Land with the shoreline bit: every tile is next to water.
const COASTAL: u8 = 0x80 | 0x40;

/// Tiles `attacker` (5,000 troops) takes from a rival holding `rival_troops`, in 20 ticks.
fn outnumbered_tiles(attacker: Leader, rival_troops: f64) -> u32 {
    let mut engine = arena(24, 4, attacker, Some(Leader::of("caesar")));
    engine.state.player_mut(1).unwrap().troops = 5_000.0;
    engine.state.player_mut(2).unwrap().troops = rival_troops;
    send(
        &mut engine,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(5_000.0),
        }),
    );
    for _ in 0..20 {
        engine.tick();
    }
    engine.state.player(1).unwrap().tile_count
}

/// Troops the attacking army loses per neutral tile taken on `terrain`, and the marks shown.
/// Only conquest takes from the army (income goes to the home pool), so a short raid isolates it.
fn cost_per_tile(leader: Leader, terrain: u8) -> (f64, Marks) {
    let mut engine = arena(48, 2, leader, None);
    engine.state.map.terrain.fill(MapTile::from_byte(terrain));
    engine.state.player_mut(1).unwrap().troops = 10_000.0;
    send(
        &mut engine,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 0,
            troops: Some(10_000.0),
        }),
    );
    army_loss_per_tile(&mut engine, 3)
}

/// Troops player 1's attacking army loses per tile it takes over `ticks` ticks of the attack
/// `engine` already launched, and the marks shown (the launch's marks included).
fn army_loss_per_tile(engine: &mut SowEngine, ticks: u32) -> (f64, Marks) {
    let army = |engine: &SowEngine| -> f64 {
        engine
            .attacks
            .iter()
            .filter(|a| a.owner_id == 1)
            .map(|a| a.troops)
            .sum()
    };
    let army_start = army(engine);
    let start_tiles = engine.state.player(1).unwrap().tile_count;
    let mut marks = [0; 4];
    tick_counting(engine, ticks, &mut marks);
    let tiles = engine.state.player(1).unwrap().tile_count - start_tiles;
    assert!(
        tiles > 0,
        "the army took no tile, so its loss per tile is undefined"
    );
    ((army_start - army(engine)) / f64::from(tiles), marks)
}

/// Troops the attacking army loses per tile taken from `target` (set up as in
/// `attack_target`), and the marks shown.
fn cost_per_tile_against(attacker: Leader, target: Target) -> (f64, Marks) {
    let mut engine = launch_target(attacker, target);
    let ticks = if target == Target::Neutral { 1 } else { 20 };
    army_loss_per_tile(&mut engine, ticks)
}

/// Troops the attacking army loses per tile when its first front runs through a plain
/// rival's city (the city sits on the border), and the marks shown.
fn city_cost_per_tile(attacker: Leader) -> (f64, Marks) {
    let mut engine = arena(24, 4, attacker, Some(Leader::of("caesar")));
    add_building(&mut engine, 2, 6, 12, BuildingKind::City);
    engine.refresh_building_grid();
    engine.state.player_mut(1).unwrap().troops = 20_000.0;
    engine.state.player_mut(2).unwrap().troops = 20_000.0;
    send(
        &mut engine,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(10_000.0),
        }),
    );
    army_loss_per_tile(&mut engine, 20)
}

/// Troops the attacking army loses per tile it takes after landing from ships on a rival's
/// shore, and the marks shown. The landing spawns the attack, so its losses start there.
fn landing_cost_per_tile(attacker: Leader) -> (f64, Marks) {
    let mut engine = arena(24, 4, attacker, Some(Leader::of("caesar")));
    engine.state.player_mut(2).unwrap().troops = 20_000.0;
    // The transport's path ends on the rival's first tile of row 12, so it lands this tick.
    let shore = 12 * 24 + 4;
    engine.add_fleet(WarpFleet::new(
        1,
        1,
        2,
        UnitType::TransportShip,
        10_000.0,
        (shore, shore),
        vec![shore],
    ));
    engine.execute_fleets();
    army_loss_per_tile(&mut engine, 20)
}

/// Launch, sail into open water, then either recall or arrive at a destination gained en route.
fn sea_retreat_refund(leader: Leader, recall: bool, limited_room: bool) -> (f64, usize) {
    use super::hero_effects::AbilityState::{Conditional, Ready};
    let active = leader.has_attacker_loss(LossWhen::SeaRetreat);
    let mut engine = arena(20, 2, leader, Some(Leader::of("cleopatra")));
    for y in 0..20 {
        for x in 2..12 {
            engine.state.set_tile_owner(x, y, 0);
            engine.state.map.terrain[(y * 20 + x) as usize] = MapTile::from_byte(0x20);
        }
    }
    engine.state.map.compute_shorelines();
    engine.water = WaterComponents::compute(&engine.state.map, |_| {});
    let p = engine.state.player_mut(1).unwrap();
    p.troops = 10_000.0;
    p.max_troops = 20_000.0;
    if active {
        assert_eq!(engine.hero_ability_feedback(1).unwrap().state, Conditional);
    }
    send(
        &mut engine,
        1,
        GameplayIntent::LaunchFleet {
            target_tile: 8 * 20 + 12,
            troops: Some(1_000.0),
        },
    );
    assert_eq!(engine.fleets.len(), 1);
    let id = engine.fleets[0].id;
    for _ in 0..4 {
        engine.execute_fleets();
    }
    assert!(engine.state.map.terrain[engine.fleets[0].current_tile as usize].is_water());
    if active {
        assert_eq!(engine.hero_ability_feedback(1).unwrap().state, Ready);
    }
    if limited_room {
        engine.state.player_mut(1).unwrap().max_troops = 9_500.0;
        if active {
            assert_eq!(engine.hero_ability_feedback(1).unwrap().state, Conditional);
        }
    }
    if recall {
        send(&mut engine, 1, GameplayIntent::RecallFleet { fleet_id: id });
        assert!(engine.fleets[0].retreating);
    } else {
        let dst = engine.fleets[0].dst_tile;
        engine.state.set_tile_owner(dst % 20, dst / 20, 1);
    }
    assert_eq!(owner_marks(&engine)[1], 0);
    for _ in 0..120 {
        engine.execute_fleets();
        if engine.fleets.is_empty() {
            break;
        }
    }
    assert!(
        engine.fleets.is_empty(),
        "the transport must complete its return"
    );
    if active && recall && !limited_room {
        assert!(engine.state.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityTriggered {
                owner_id: 1,
                tile: Some((1, _)),
                ..
            }
        )));
    }
    if active {
        assert_eq!(engine.hero_ability_feedback(1).unwrap().state, Conditional);
    }
    (
        engine.state.player(1).unwrap().troops,
        owner_marks(&engine)[1],
    )
}

/// Kin and Foreign follow the people each player is shown as, not the hero itself: a rival
/// led by another hero of the same people is kin, a rival of another people is foreign, a
/// catalog bot counts as its catalog people whatever random hero carries its ability, and
/// neutral land and the hero's own land are neither.
#[test]
fn kin_and_foreign_follow_the_people_each_hero_leads() {
    let (hero, kinsman) = Leader::all()
        .flat_map(|a| Leader::all().map(move |b| (a, b)))
        .find(|(a, b)| a != b && a.civilization() == b.civilization())
        .expect("the catalog has two heroes of one people");
    let stranger = Leader::all()
        .find(|other| other.civilization() != hero.civilization())
        .expect("the catalog has heroes of more than one people");
    // A probe worth 2 on `target` alone: the product is 2 only when `target` describes `subject`.
    // `bot_people` makes the rival a catalog bot shown as that hero's people, as its spawn does.
    let describes = |rival: Leader, bot_people: Option<Leader>, subject: u16, target: Target| {
        let mut engine = arena(24, 4, hero, Some(rival));
        if let Some(people) = bot_people {
            let bot = engine.state.player_mut(2).unwrap();
            bot.player_type = PlayerType::Nation;
            bot.realm = Some(people.civilization().to_owned());
        }
        targeted_multiplier(&engine.state, 1, subject, 10_000.0, |t| {
            if t == target { 2.0 } else { 1.0 }
        }) == 2.0
    };
    assert!(
        describes(kinsman, None, 2, Target::Kin),
        "same people, another hero"
    );
    assert!(!describes(kinsman, None, 2, Target::Foreign));
    assert!(
        describes(stranger, None, 2, Target::Foreign),
        "another people"
    );
    assert!(!describes(stranger, None, 2, Target::Kin));
    assert!(
        describes(stranger, Some(kinsman), 2, Target::Kin),
        "a bot shown as the same people"
    );
    assert!(!describes(stranger, Some(kinsman), 2, Target::Foreign));
    assert!(
        describes(kinsman, Some(stranger), 2, Target::Foreign),
        "a bot shown as another people"
    );
    assert!(!describes(kinsman, Some(stranger), 2, Target::Kin));
    for target in [Target::Kin, Target::Foreign] {
        assert!(
            !describes(kinsman, None, 0, target),
            "neutral land leads no people"
        );
        assert!(!describes(kinsman, None, 1, target), "the hero's own land");
    }

    // The real spawn gives a catalog bot its catalog people, not its random hero's.
    let mut engine = arena(24, 4, hero, None);
    engine.spawn_ai(1, 0);
    let bot = engine
        .state
        .players
        .iter()
        .find(|player| player.player_type == PlayerType::Nation)
        .expect("the nation bot spawned");
    assert!(
        crate::geo_entities::bots(crate::geo_entities::Role::Nation)
            .any(|character| character.name == bot.name && character.entity == bot.realm()),
        "{} is shown as its catalog people",
        bot.name
    );
}

/// The target loss mark comes from the tiles it discounts, during the attack's ticks: the
/// launch shows nothing, and the attacker's mark appears at a tile its army just took.
#[test]
fn target_loss_marks_the_discounted_tile_during_the_tick() {
    let (hero, against) = Leader::all()
        .find_map(|leader| match leader.ability() {
            Ability::AttackerLoss {
                when: LossWhen::Against(against),
                ..
            } => Some((leader, against)),
            _ => None,
        })
        .expect("the catalog has a target loss hero");
    let mut engine = launch_target(hero, against);
    assert_eq!(marks(&engine, 1), 0, "the launch shows no target loss mark");
    let marked_taken_tile = (0..20).any(|_| {
        engine.tick();
        engine.state.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AbilityTriggered { owner_id: 1, tile: Some((x, y)), .. }
                    if engine.state.map.owner_id(*x, *y) == 1
            )
        })
    });
    assert!(
        marked_taken_tile,
        "{hero:?} shows no mark at a tile its {against:?} loss discounts"
    );
}

/// Tenamaxtli, "Mixtón Stronghold": his attacks fight 25% stronger only against a player
/// whose troops outnumber the army he sends. Other heroes and neutral land get nothing.
#[test]
fn tenamaxtli_strength_counts_only_against_a_stronger_player() {
    let tenamaxtli = Leader::of("tenamaxtli");
    let mut engine = arena(24, 4, tenamaxtli, Some(Leader::of("caesar")));
    engine.state.player_mut(2).unwrap().troops = 20_000.0;
    let strength = |engine: &SowEngine, target: u16, army: f64| {
        attack_strength(&engine.state, 1, target, army)
    };
    assert_eq!(strength(&engine, 2, 5_000.0), 1.25);
    assert_eq!(strength(&engine, 2, 20_000.0), 1.0, "equal numbers");
    assert_eq!(strength(&engine, 2, 25_000.0), 1.0, "bigger army");
    assert_eq!(strength(&engine, 0, 5_000.0), 1.0, "neutral land");
    engine.state.player_mut(1).unwrap().leader = Leader::of("cleopatra");
    assert_eq!(strength(&engine, 2, 5_000.0), 1.0, "another hero");
}

#[test]
fn tenamaxtli_outnumbered_attack_takes_more_tiles() {
    let tenamaxtli = Leader::of("tenamaxtli");
    let plain = Leader::of("cleopatra");
    assert!(
        outnumbered_tiles(tenamaxtli, 20_000.0) > outnumbered_tiles(plain, 20_000.0),
        "Tenamaxtli's outnumbered attack must out-fight the same attack without the ability"
    );
    assert_eq!(
        outnumbered_tiles(tenamaxtli, 2_500.0),
        outnumbered_tiles(plain, 2_500.0),
        "no bonus when his army is the bigger one"
    );
}

/// The visual flag must follow the same rule as the combat: Tenamaxtli's attack on a
/// stronger player is flagged, the same attack by another hero or against a weaker
/// player is not.
#[test]
fn tenamaxtli_empowered_flag_reaches_the_snapshot() {
    let flagged = |attacker: Leader, rival_troops: f64| {
        let mut engine = arena(24, 4, attacker, Some(Leader::of("caesar")));
        engine.state.player_mut(1).unwrap().troops = 5_000.0;
        engine.state.player_mut(2).unwrap().troops = rival_troops;
        send(
            &mut engine,
            1,
            GameplayIntent::Attack(AttackIntent {
                target_owner: 2,
                troops: Some(5_000.0),
            }),
        );
        let snapshot = engine.build_snapshot();
        snapshot
            .players
            .iter()
            .find(|p| p.id == 1)
            .unwrap()
            .empowered_attack
    };
    assert!(flagged(Leader::of("tenamaxtli"), 20_000.0));
    assert!(!flagged(Leader::of("tenamaxtli"), 2_500.0));
    assert!(!flagged(Leader::of("cleopatra"), 20_000.0));
}

/// Napoleon, "Montenotte Blitz": his conquest runs 35% faster against other players' land,
/// and neutral land gets the plain speed.
#[test]
fn napoleon_blitz_speeds_up_other_players_not_neutral_land() {
    let napoleon = Leader::of("napoleon");
    let plain = Leader::of("cleopatra");
    assert!(
        tiles_against(napoleon, Target::OtherPlayer).0
            > tiles_against(plain, Target::OtherPlayer).0,
        "Napoleon must take more land from other players than a plain hero"
    );
    assert_eq!(
        tiles_against(napoleon, Target::Neutral).0,
        tiles_against(plain, Target::Neutral).0,
        "neutral land gets no bonus"
    );
}

/// Musa I, "Timbuktu Recapture": losing a city to player 2 opens a counterattack window against
/// player 2 only; the window closes on its own. Other leaders never open one.
#[test]
fn musa_recapture_window_opens_on_a_lost_city_and_closes_after_it() {
    let mut engine = lose_city(Leader::of("musa_i"));
    let victim = engine.state.player(1).unwrap();
    assert_eq!(
        victim.recapture_target, 2,
        "the city's captor is the target"
    );
    assert!(
        victim.recapture_ticks > 0,
        "losing the city opens the window"
    );

    let mut ticks = 0;
    while engine.state.player(1).unwrap().recapture_ticks > 0 && ticks < 1_000 {
        engine.tick();
        ticks += 1;
    }
    assert_eq!(
        engine.state.player(1).unwrap().recapture_ticks,
        0,
        "the window closes"
    );

    let plain = lose_city(Leader::of("cleopatra"));
    assert_eq!(plain.state.player(1).unwrap().recapture_ticks, 0);
}

/// Isabella, "Capitulations of Santa Fe": her attacks capture enemy cities faster, and
/// the same attacks on open land are no faster than anyone else's.
#[test]
fn isabella_captures_enemy_cities_faster_on_open_land_no_faster() {
    let isabella = Leader::of("isabella_of_castile");
    let plain = Leader::of("cleopatra");
    assert!(tiles_taken_from_city(isabella, plain).0 > tiles_taken_from_city(plain, plain).0);
    assert_eq!(
        neutral_conquest(isabella),
        neutral_conquest(plain),
        "open land is no faster"
    );
}

/// Ability marks (the visible effect events) that `owner` shows right now.
fn marks(engine: &SowEngine, owner: u16) -> usize {
    owner_marks(engine)[usize::from(owner)]
}

/// A mark needs its exact target, not just its hero: Yaa Asantewaa's attack marks on a nation and
/// stays silent on a human rival of the same army and land.
#[test]
fn a_mark_needs_its_target_not_just_its_hero() {
    let mut nation = arena(
        24,
        4,
        Leader::of("yaa_asantewaa"),
        Some(Leader::of("caesar")),
    );
    nation.state.player_mut(2).unwrap().player_type = PlayerType::Nation;
    let mut human = arena(
        24,
        4,
        Leader::of("yaa_asantewaa"),
        Some(Leader::of("caesar")),
    );
    for engine in [&mut nation, &mut human] {
        send(
            engine,
            1,
            GameplayIntent::Attack(AttackIntent {
                target_owner: 2,
                troops: Some(10_000.0),
            }),
        );
    }
    assert_eq!(
        marks(&nation, 1),
        1,
        "Yaa Asantewaa marks an attack on a nation"
    );
    assert_eq!(
        marks(&human, 1),
        0,
        "Yaa Asantewaa's mark needs a nation target"
    );
}

/// Burebista, "Carpathian Fortresses": the mark reaches the human attacker even when
/// the hero is a bot, because the ability changed that player's fight.
#[test]
fn burebista_defense_announces_to_the_attacker_even_from_a_bot() {
    let mut engine = arena(24, 4, Leader::of("caesar"), Some(Leader::of("burebista")));
    engine.state.player_mut(2).unwrap().player_type = PlayerType::Bot;
    engine.state.player_mut(1).unwrap().troops = 20_000.0;
    engine.state.player_mut(2).unwrap().troops = 20_000.0;
    send(
        &mut engine,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(10_000.0),
        }),
    );
    let mut affected = None;
    for _ in 0..20 {
        engine.tick();
        affected = engine.state.events.iter().find_map(|event| match event {
            GameEvent::AbilityTriggered {
                owner_id: 2,
                affected,
                ..
            } => Some(affected.clone()),
            _ => None,
        });
        if affected.is_some() {
            break;
        }
    }
    assert_eq!(
        affected.expect("the defense ability must announce to the human attacker"),
        vec![1]
    );
}

/// Bolesław II, "Kievan Expeditions": the mark names the foreign player it hit, and
/// stays silent against neutral land and against the hero's own people.
#[test]
fn boleslaw_conquest_announces_to_a_foreign_target_only() {
    let hero = Leader::of("boleslaw_ii");
    let foreign = launch_target(hero, Target::Foreign);
    let affected = foreign
        .state
        .events
        .iter()
        .find_map(|event| match event {
            GameEvent::AbilityTriggered {
                owner_id: 1,
                affected,
                ..
            } => Some(affected.clone()),
            _ => None,
        })
        .expect("a foreign conquest must announce");
    assert_eq!(affected, vec![2]);

    for target in [Target::Neutral, Target::Kin] {
        let engine = launch_target(hero, target);
        assert!(
            !engine
                .state
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityTriggered { owner_id: 1, .. })),
            "{target:?} must not announce"
        );
    }
}

/// Exercise the live completion, ownership, projectile and income paths for the
/// redesigned situations. Controls use the identical world and another leader.
fn redesigned_effect(leader: Leader, situation: Ability) -> (f64, usize) {
    let mut e = arena(64, 32, leader, Some(Leader::of("cleopatra")));
    let make = |id, owner_id, kind, x: u32, y: u32, level, under| Building {
        id,
        owner_id,
        kind,
        tile_idx: y * 64 + x,
        level,
        under_construction: under,
        ticks_until_complete: u32::from(under),
    };
    match situation {
        Ability::CoastalFounding => {
            for x in 0..64 {
                for y in 0..4 {
                    e.state.map.terrain[(y * 64 + x) as usize] = MapTile::from_byte(0x20);
                }
            }
            e.state.map.compute_shorelines();
            // An enemy-coast capture opens a valid port site on our bank.
            e.state.set_tile_owner(32, 4, 1);
            e.state.events.push(GameEvent::TileCaptured {
                x: 32,
                y: 4,
                new_owner: 1,
                previous_owner: 2,
                troops: 100.0,
            });
            e.hero_coastal_foundings();
            let count = e.buildings.len();
            e.hero_coastal_foundings();
            assert_eq!(e.buildings.len(), count, "founding is once per match");
            (count as f64, owner_marks(&e)[1])
        }
        Ability::AlliedCityGrant { .. } => {
            e.state.player_mut(1).unwrap().alliances.push(2);
            e.state.player_mut(2).unwrap().alliances.push(1);
            e.add_building(make(1, 2, BuildingKind::City, 40, 40, 1, true));
            let before = e.state.player(1).unwrap().gold;
            e.execute_construction();
            let value = e.state.player(1).unwrap().gold - before;
            assert_eq!(
                e.state.player(2).unwrap().gold - before,
                value,
                "both receive the grant"
            );
            (value, owner_marks(&e)[1])
        }
        Ability::CityTowerUpgrade => {
            e.add_building(make(1, 1, BuildingKind::City, 16, 16, 2, true));
            e.add_building(make(2, 1, BuildingKind::Tower, 24, 16, 1, false));
            e.execute_construction();
            (f64::from(e.buildings[1].level), owner_marks(&e)[1])
        }
        Ability::CityFarmFounding => {
            e.add_building(make(1, 1, BuildingKind::City, 16, 16, 1, false));
            e.state.next_building_id = 2;
            e.state.player_mut(1).unwrap().gold = 1_000_000.0;
            send(
                &mut e,
                1,
                GameplayIntent::UpgradeStructure { building_id: 1 },
            );
            let duration = e.buildings[0].ticks_until_complete;
            assert!(duration > 0);
            for _ in 0..duration {
                e.execute_construction();
            }
            let farms = e
                .buildings
                .iter()
                .filter(|b| b.kind == BuildingKind::Farm)
                .count();
            (farms as f64, owner_marks(&e)[1])
        }
        Ability::SiegeBreach => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = leader.ability() == Ability::SiegeBreach;
            e.add_building(make(1, 2, BuildingKind::Tower, 32, 16, 3, false));
            e.add_building(make(2, 2, BuildingKind::Tower, 35, 16, 3, false));
            e.add_building(make(3, 2, BuildingKind::Tower, 49, 16, 2, false));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.add_building(make(4, 2, BuildingKind::Tower, 32, 20, 2, false));
            e.add_building(make(5, 2, BuildingKind::Tower, 39, 16, 1, false));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            e.state.set_tile_owner(32, 16, 1);
            e.sync_building_ownership();
            let destroyed = 5 - e.buildings.len();
            assert_eq!(destroyed, usize::from(active));
            assert_eq!(e.buildings.iter().any(|tower| tower.id == 4), !active);
            assert_eq!(e.buildings[0].owner_id, 1);
            if active {
                assert!(e.building_grid.dirty && e.render_defense_dirty);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((32, 20)), affected }
                        if affected == &[2]
                )));
            }
            let shown = owner_marks(&e)[1];
            e.state.events.clear();
            e.state.set_tile_owner(32, 16, 2);
            e.sync_building_ownership();
            e.state.set_tile_owner(32, 16, 1);
            e.sync_building_ownership();
            assert_eq!(e.buildings.len(), 5 - destroyed);
            assert_eq!(owner_marks(&e)[1], 0);
            (destroyed as f64, shown)
        }
        Ability::ImperialStandards { .. } => {
            use super::hero_effects::AbilityState::{Active, Conditional};
            use super::income_rates::imperial_standard_income;
            let active = matches!(leader.ability(), Ability::ImperialStandards { .. });
            e.add_building(make(1, 1, BuildingKind::City, 8, 16, 2, false));
            e.add_building(make(2, 1, BuildingKind::Factory, 16, 16, 2, false));
            e.add_building(make(3, 1, BuildingKind::Farm, 24, 16, 2, true));
            e.execute_income();
            assert_eq!(e.hero_gold_per_second(1), 0.0);
            assert_eq!(owner_marks(&e)[1], 0);
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }

            let player = e.state.player_mut(1).unwrap();
            player.troops = 0.0;
            let before = player.gold;
            e.execute_construction();
            e.execute_income();
            let player = e.state.player(1).unwrap();
            let agg = e.building_aggregates[1];
            let cfg = &e.state.config;
            let (extra_gold, extra_troops) = imperial_standard_income(agg, leader, cfg);
            assert_eq!(
                (extra_gold, extra_troops),
                if active {
                    (
                        cfg.factory_gold_income * 2.5 * 0.25,
                        cfg.farm_troop_income * 2.0 * 0.25,
                    )
                } else {
                    (0.0, 0.0)
                }
            );
            assert_eq!(e.hero_gold_per_second(1), extra_gold);
            let base_gold = gold_income_per_second(player.tile_count, agg, leader, cfg);
            let base_troops = troop_income_per_second(player.tile_count, agg, leader, cfg);
            assert!((player.gold - before - cfg.per_tick(base_gold + extra_gold)).abs() < 1e-8);
            assert!((player.troops - cfg.per_tick(base_troops + extra_troops)).abs() < 1e-8);
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Active);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered {
                        owner_id: 1,
                        tile: Some((8, 16)),
                        ..
                    }
                )));
            }
            let shown = owner_marks(&e)[1];
            e.state.events.clear();
            e.execute_income();
            assert_eq!(owner_marks(&e)[1], 0);

            e.state.set_tile_owner(24, 16, 2);
            e.sync_building_ownership();
            e.execute_income();
            assert_eq!(e.hero_gold_per_second(1), 0.0);
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.state.set_tile_owner(24, 16, 1);
            e.sync_building_ownership();
            e.execute_income();
            assert_eq!(owner_marks(&e)[1], shown);
            (extra_gold + extra_troops, shown)
        }
        Ability::FactoryRaid { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::FactoryRaid { .. });
            let mut battle = arena(12, 6, leader, Some(Leader::of("cleopatra")));
            for y in 0..12 {
                for x in 6..9 {
                    if (x, y) != (6, 6) {
                        battle.state.set_tile_owner(x, y, 0);
                    }
                }
            }
            battle.add_building(Building {
                tile_idx: 6 * 12 + 6,
                ..make(1, 2, BuildingKind::Factory, 6, 6, 1, true)
            });
            let take_factory = |battle: &mut SowEngine| {
                battle.state.events.clear();
                battle.state.player_mut(1).unwrap().troops = 10_000.0;
                battle.state.player_mut(2).unwrap().troops = 2000.0;
                send(
                    battle,
                    1,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner: 2,
                        troops: Some(8000.0),
                    }),
                );
                let id = battle.attacks[0].id;
                for _ in 0..20 {
                    battle.execute_combat();
                    if battle.state.map.owner_id(6, 6) == 1 {
                        break;
                    }
                    battle.state.tick += 1;
                }
                let troops = battle
                    .state
                    .events
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::TileCaptured {
                            x: 6,
                            y: 6,
                            new_owner: 1,
                            troops,
                            ..
                        } => Some(*troops),
                        _ => None,
                    })
                    .expect("land attack must take the factory center");
                let marks = owner_marks(battle)[1];
                if !battle.attacks.is_empty() {
                    send(battle, 1, GameplayIntent::CancelAttack { attack_id: id });
                    battle.execute_combat();
                }
                battle.sync_building_ownership();
                (troops, marks)
            };
            if active {
                assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            let (ordinary, no_mark) = take_factory(&mut battle);
            assert_eq!(no_mark, 0);
            battle.state.set_tile_owner(6, 6, 2);
            battle.sync_building_ownership();
            battle.buildings[0].level = 3;
            battle.buildings[0].under_construction = false;
            if active {
                battle.state.set_tile_owner(6, 6, 0);
                assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Conditional);
                battle.state.set_tile_owner(6, 6, 2);
                assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Ready);
            }
            let (reinforced, shown) = take_factory(&mut battle);
            let gained = reinforced - ordinary;
            assert_eq!(gained, if active { 900.0 } else { 0.0 });
            if active {
                assert!(battle.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((6, 6)), affected }
                        if affected == &[2]
                )));
            }
            battle.state.set_tile_owner(6, 6, 2);
            battle.sync_building_ownership();
            if active {
                assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            assert_eq!(take_factory(&mut battle), (ordinary, 0));
            (reinforced, shown)
        }
        Ability::FactoryCityUpgradeDiscount { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::FactoryCityUpgradeDiscount { .. });
            let city = make(1, 1, BuildingKind::City, 16, 16, 1, false);
            e.add_building(city);
            let full_cost = crate::building::structure_upgrade_cost_gold(
                BuildingKind::City,
                2,
                1,
                &e.state.config,
            );
            e.state.player_mut(1).unwrap().gold = full_cost;
            e.add_building(make(2, 2, BuildingKind::Factory, 22, 16, 1, false));
            e.add_building(make(3, 1, BuildingKind::Factory, 16, 28, 1, true));
            e.add_building(make(4, 1, BuildingKind::Factory, 16, 29, 1, false));
            assert_eq!(e.structure_upgrade_quote(city.id), Some((full_cost, false)));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.execute_construction();
            let expected_cost = if active { full_cost * 0.65 } else { full_cost };
            let (price, discounted) = e.structure_upgrade_quote(city.id).unwrap();
            assert!((price - expected_cost).abs() < 1e-8);
            assert_eq!(discounted, active);
            if active {
                e.state.player_mut(1).unwrap().gold = price - 1.0;
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
                send(
                    &mut e,
                    1,
                    GameplayIntent::UpgradeStructure {
                        building_id: city.id,
                    },
                );
                assert!(!e.buildings[0].under_construction);
                assert_eq!(owner_marks(&e)[1], 0);
                e.state.player_mut(1).unwrap().gold = price;
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            let before = e.state.player(1).unwrap().gold;
            send(
                &mut e,
                1,
                GameplayIntent::UpgradeStructure {
                    building_id: city.id,
                },
            );
            assert_eq!(e.buildings[0].level, 2);
            assert!(e.buildings[0].under_construction);
            assert_eq!(e.state.player(1).unwrap().gold, 0.0);
            if active {
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered {
                        owner_id: 1,
                        tile: Some((16, 16)),
                        ..
                    }
                )));
            }
            (full_cost - before, owner_marks(&e)[1])
        }
        Ability::Reunification { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::Reunification { .. });
            let run = |room: f64| {
                let mut battle = arena(12, 6, leader, Some(Leader::of("cleopatra")));
                for y in 0..12 {
                    for x in 6..12 {
                        if (x, y) != (6, 6) && (x, y) != (11, 11) {
                            battle.state.set_tile_owner(x, y, 1);
                        }
                    }
                }
                battle.state.config.attack_cost_enemy = 0.0;
                let defender = battle.state.player_mut(2).unwrap();
                defender.troops = 2000.0;
                let attacker = battle.state.player_mut(1).unwrap();
                attacker.troops = 10_000.0;
                attacker.max_troops = 10_000.0 + room;
                if active {
                    assert_eq!(
                        battle.hero_ability_feedback(1).unwrap().state,
                        if room > 0.0 { Ready } else { Conditional }
                    );
                }
                send(
                    &mut battle,
                    1,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner: 2,
                        troops: Some(8000.0),
                    }),
                );
                if active && room > 0.0 {
                    assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Ready);
                }
                for _ in 0..20 {
                    battle.execute_combat();
                    if !battle.state.player(2).unwrap().alive {
                        break;
                    }
                    battle.state.tick += 1;
                }
                let defender = battle.state.player(2).unwrap();
                assert!(!defender.alive, "attack must take the last enemy tile");
                assert_eq!(defender.troops, 0.0);
                let recruited = battle.state.player(1).unwrap().troops - 10_000.0;
                assert_eq!(recruited, if active { room.min(500.0) } else { 0.0 });
                if active && room > 0.0 {
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event,
                        GameEvent::AbilityTriggered { owner_id: 1, tile: Some((6, 6)), affected }
                            if affected == &[2]
                    )));
                }
                (recruited, owner_marks(&battle)[1])
            };
            let result = run(750.0);
            run(200.0);
            assert_eq!(run(0.0).1, 0);
            result
        }
        Ability::WarRegalia { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::WarRegalia { .. });
            e.state.set_tile_owner(32, 16, 3);
            e.state.set_tile_owner(63, 63, 2);
            e.state.player_mut(1).unwrap().troops = 2000.0;
            e.state.player_mut(1).unwrap().alliances.push(2);
            e.state.player_mut(2).unwrap().alliances.push(1);
            e.state.player_mut(2).unwrap().troops = 10_000.0;
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.add_building(make(1, 1, BuildingKind::City, 16, 16, 1, false));
            if active {
                e.state.player_mut(1).unwrap().troops = 749.0;
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
                e.state.player_mut(1).unwrap().troops = 2000.0;
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            let order = GameplayIntent::Attack(AttackIntent {
                target_owner: 3,
                troops: Some(1000.0),
            });
            send(&mut e, 2, order.clone());
            let troops = e.attacks[0].troops;
            let reserve = 2000.0 - e.state.player(1).unwrap().troops;
            assert_eq!(reserve, if active { 750.0 } else { 0.0 });
            assert_eq!(troops, 1000.0 + reserve);
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((32, 16)), affected }
                        if affected == &[2, 3]
                )));
            }
            send(&mut e, 2, order.clone());
            assert_eq!(e.attacks[0].troops, troops + 1000.0);
            let attack_id = e.attacks[0].id;
            send(&mut e, 2, GameplayIntent::CancelAttack { attack_id });
            e.execute_combat();
            send(&mut e, 2, order);
            assert_eq!(e.attacks[0].troops, 1000.0);
            assert_eq!(2000.0 - e.state.player(1).unwrap().troops, reserve);
            (troops, owner_marks(&e)[1])
        }
        Ability::LastCityRelocation => {
            use super::hero_effects::AbilityState::{Conditional, Ready, Spent};
            let active = leader.ability() == Ability::LastCityRelocation;
            e.add_building(make(1, 1, BuildingKind::City, 8, 16, 1, false));
            e.add_building(make(2, 1, BuildingKind::City, 20, 16, 1, false));
            e.state.next_building_id = 3;
            e.state.player_mut(1).unwrap().cities = 2;
            let gold = e.state.player(1).unwrap().gold;
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.state.set_tile_owner(8, 16, 2);
            e.sync_building_ownership();
            assert_eq!(e.buildings.len(), 2);
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            e.state.set_tile_owner(20, 16, 2);
            e.sync_building_ownership();
            let founded = e.buildings.len() - 2;
            assert_eq!(founded, usize::from(active));
            assert_eq!(e.state.player(1).unwrap().gold, gold);
            assert_eq!(e.state.player(1).unwrap().cities, u32::from(active));
            if active {
                let city = e.buildings[2];
                assert_eq!(
                    (city.kind, city.owner_id, city.active_level()),
                    (BuildingKind::City, 1, 1)
                );
                let (x, y) = (city.tile_idx % 64, city.tile_idx / 64);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some(tile), .. }
                        if *tile == (x, y)
                )));
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Spent);
                e.state.set_tile_owner(x, y, 2);
                e.sync_building_ownership();
                assert_eq!(e.buildings.len(), 3);
            }
            (founded as f64, owner_marks(&e)[1])
        }
        Ability::NyabingiNetwork { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::NyabingiNetwork { .. });
            let (mut defended_tiles, mut marks) = (0, 0);
            for (defender, linked, allied) in [
                (1, true, false),
                (1, false, true),
                (1, true, true),
                (2, true, true),
            ] {
                let plain = Leader::of("cleopatra");
                let mut battle = arena(40, 8, leader, Some(plain));
                for y in 0..40 {
                    for x in 0..40 {
                        let owner = if x >= 8 { 3 } else { 1 + (y % 2) as u16 };
                        battle.state.set_tile_owner(x, y, owner);
                        if x < 8 && (owner == defender || linked) {
                            battle.state.map.terrain[(y * 40 + x) as usize] =
                                MapTile::from_byte(0x8a);
                        }
                    }
                }
                if allied {
                    send(
                        &mut battle,
                        1,
                        GameplayIntent::ProposeAlliance { target_player: 2 },
                    );
                    send(
                        &mut battle,
                        2,
                        GameplayIntent::AcceptAlliance { target_player: 1 },
                    );
                    assert!(super::hero_effects::allied(&battle.state, 1, 2));
                }
                if active {
                    assert_eq!(
                        battle.hero_ability_feedback(1).unwrap().state,
                        if linked && allied { Ready } else { Conditional },
                    );
                }
                battle.state.config.global_speed_multiplier = 2.0;
                for id in [1, 2, 3] {
                    battle.state.player_mut(id).unwrap().troops = 20_000.0;
                }
                battle.state.player_mut(3).unwrap().leader = plain;
                send(
                    &mut battle,
                    3,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner: defender,
                        troops: Some(10_000.0),
                    }),
                );
                assert_eq!(battle.attacks.len(), 1);
                assert_eq!(owner_marks(&battle)[1], 0);
                battle.execute_combat();
                let shown = owner_marks(&battle)[1];
                assert_eq!(shown, usize::from(active && linked && allied));
                if shown > 0 {
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event, GameEvent::AbilityTriggered { owner_id: 1, tile: Some((7, y)), affected }
                        if 1 + (*y % 2) as u16 == defender
                            && affected.contains(&3)
                            && (defender == 1 || affected.contains(&defender))
                    )));
                }
                if linked && allied {
                    defended_tiles += battle.state.player(defender).unwrap().tile_count;
                    marks += shown;
                }
            }
            (f64::from(defended_tiles), marks)
        }
        Ability::RegionalCommands { percent } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::RegionalCommands { .. });
            let mut result = (0.0, 0);
            for cancel in [true, false] {
                let plain = Leader::of("cleopatra");
                let mut battle = arena(24, 6, leader, Some(plain));
                battle.state.config.global_speed_multiplier = 2.0;
                battle.state.player_mut(1).unwrap().troops = 20_000.0;
                battle.state.config.attack_cost_enemy = 100.0;
                for y in 12..24 {
                    battle.state.set_tile_owner(6, y, 3);
                }
                for id in [2, 3] {
                    let p = battle.state.player_mut(id).unwrap();
                    p.leader = plain;
                    p.troops = 20_000.0;
                    send(
                        &mut battle,
                        id,
                        GameplayIntent::Attack(AttackIntent {
                            target_owner: 1,
                            troops: Some(10_000.0),
                        }),
                    );
                    if active {
                        assert_eq!(
                            battle.hero_ability_feedback(1).unwrap().state,
                            if id == 2 { Conditional } else { Ready },
                        );
                    }
                }
                assert_eq!(battle.attacks.len(), 2);
                let mut invaders = vec![2, 3];
                if cancel {
                    let id = battle.attacks.iter().find(|a| a.owner_id == 3).unwrap().id;
                    send(
                        &mut battle,
                        3,
                        GameplayIntent::CancelAttack { attack_id: id },
                    );
                    invaders.pop();
                    if active {
                        assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Conditional);
                    }
                }

                battle.execute_combat();
                let mut loss_per_tile = 0.0;
                for &invader in &invaders {
                    let expected = if active && !cancel {
                        100.0 + f64::from(percent)
                    } else {
                        100.0
                    };
                    let mut taken = 0;
                    let mut remaining = 10_000.0;
                    // Capture events record the army after losses, even if it
                    // finished the attack and returned home in the same tick.
                    for event in &battle.state.events {
                        if let GameEvent::TileCaptured {
                            new_owner,
                            previous_owner: 1,
                            troops,
                            ..
                        } = event
                            && *new_owner == invader
                        {
                            taken += 1;
                            assert!((remaining - troops - expected).abs() < 1e-8);
                            remaining = *troops;
                        }
                    }
                    assert!(taken > 0);
                    let loss = (10_000.0 - remaining) / f64::from(taken);
                    loss_per_tile += loss;
                }
                let shown = owner_marks(&battle)[1];
                assert_eq!(shown, usize::from(active && !cancel));
                if active && !cancel {
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event, GameEvent::AbilityTriggered { owner_id: 1, tile: Some((x,y)), affected }
                            if affected.len() == 2 && invaders.iter().all(|id| affected.contains(id))
                                && invaders.contains(&battle.state.map.owner_id(*x, *y))
                    )));
                }
                if !cancel {
                    result = (loss_per_tile, shown);
                }
            }
            result
        }
        Ability::ZagrosAscent { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::ZagrosAscent { .. });
            let (mut captured, mut marks) = (0, 0);
            for (approach, target_owner) in [(0x80, 2), (0x8a, 0), (0x8a, 2)] {
                let mut battle = arena(48, 6, leader, Some(Leader::of("cleopatra")));
                battle.state.config.global_speed_multiplier = 1.0;
                for y in 0..48 {
                    battle.state.map.terrain[(y * 48 + 5) as usize] = MapTile::from_byte(approach);
                    battle.state.map.terrain[(y * 48 + 6) as usize] = MapTile::from_byte(0x94);
                    if target_owner == 0 {
                        battle.state.set_tile_owner(6, y, 0);
                    }
                }
                let qualifies = approach == 0x8a && target_owner != 0;
                if active {
                    assert_eq!(
                        battle.hero_ability_feedback(1).unwrap().state,
                        if qualifies { Ready } else { Conditional },
                    );
                }
                battle.state.player_mut(1).unwrap().troops = 20_000.0;
                battle.state.player_mut(2).unwrap().troops = 20_000.0;
                send(
                    &mut battle,
                    1,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner,
                        troops: Some(10_000.0),
                    }),
                );
                assert_eq!(
                    owner_marks(&battle)[1],
                    0,
                    "launch alone never triggers the mark"
                );
                battle.execute_combat();
                let shown = owner_marks(&battle)[1];
                assert_eq!(shown, usize::from(active && qualifies));
                if active && qualifies {
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event, GameEvent::AbilityTriggered { owner_id: 1, tile: Some((6, y)), affected }
                        if affected == &[2] && battle.state.map.owner_id(6, *y) == 1
                    )));
                }
                if qualifies {
                    captured = (0..48)
                        .filter(|&y| battle.state.map.owner_id(6, y) == 1)
                        .count();
                    marks = shown;
                }
            }
            (captured as f64, marks)
        }
        Ability::ContestedSteppe { percent } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::ContestedSteppe { .. });
            let mut remaining = 0.0;
            let mut marks = 0;
            // Same isolated tile and army each time, with a rival, ally or highland.
            for (friendly, terrain) in [(true, 0x80), (false, 0x8a), (false, 0x80)] {
                let mut battle = arena(12, 6, leader, Some(Leader::of("cleopatra")));
                let (x, y) = (6, 6);
                battle.state.map.terrain[(y * 12 + x) as usize] = MapTile::from_byte(terrain);
                battle.state.set_tile_owner(x, y, 0);
                if friendly {
                    send(
                        &mut battle,
                        1,
                        GameplayIntent::ProposeAlliance { target_player: 2 },
                    );
                    send(
                        &mut battle,
                        2,
                        GameplayIntent::AcceptAlliance { target_player: 1 },
                    );
                    assert!(super::hero_effects::allied(&battle.state, 1, 2));
                }
                let eligible = active && !friendly && terrain == 0x80;
                if active {
                    assert_eq!(
                        battle.hero_ability_feedback(1).unwrap().state,
                        if eligible { Ready } else { Conditional },
                    );
                }
                let initial = 20_000.0;
                battle.state.player_mut(1).unwrap().troops = initial;
                send(
                    &mut battle,
                    1,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner: 0,
                        troops: Some(10_000.0),
                    }),
                );
                for _ in 0..40 {
                    battle.execute_combat();
                    if battle.state.map.owner_id(x, y) == 1 {
                        break;
                    }
                    battle.state.tick += 1;
                }
                assert_eq!(battle.state.map.owner_id(x, y), 1);
                let ordinary_loss = battle.state.config.attack_cost_neutral
                    * if terrain == 0x8a {
                        battle.state.config.terrain_multiplier_highland
                    } else {
                        1.0
                    };
                let troops = attacker_troop_total(&battle);
                let expected_loss = ordinary_loss
                    * if eligible {
                        1.0 - f64::from(percent) / 100.0
                    } else {
                        1.0
                    };
                assert!((initial - troops - expected_loss).abs() < 1e-8);
                assert_eq!(owner_marks(&battle)[1], usize::from(eligible));
                if eligible {
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event,
                        GameEvent::AbilityTriggered {
                            owner_id: 1,
                            tile: Some((6, 6)),
                            ..
                        }
                    )));
                }
                if !friendly && terrain == 0x80 {
                    remaining = troops;
                    marks = owner_marks(&battle)[1];
                }
            }
            (remaining, marks)
        }
        Ability::GreatLawOfPeace => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            use crate::diplomacy::ALLIANCE_DURATION_TICKS;
            let active = leader.ability() == Ability::GreatLawOfPeace;
            e.state.player_mut(1).unwrap().home = Some((12, 16));
            let form = |e: &mut SowEngine, a, b| {
                send(e, a, GameplayIntent::ProposeAlliance { target_player: b });
                send(e, b, GameplayIntent::AcceptAlliance { target_player: a });
                assert!(super::hero_effects::allied(&e.state, a, b));
            };
            form(&mut e, 1, 2);
            form(&mut e, 1, 3);
            form(&mut e, 2, 3);
            assert_eq!(owner_marks(&e)[1], 0, "full clocks do not use the ability");
            send(
                &mut e,
                2,
                GameplayIntent::BreakAlliance { target_player: 3 },
            );
            let clocks = [(1, 2, 600), (2, 1, 600), (1, 3, 1_800), (3, 1, 1_800)];
            for (player, ally, ticks) in clocks {
                e.state
                    .player_mut(player)
                    .unwrap()
                    .alliance_timers
                    .insert(ally, ticks);
            }
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            send(
                &mut e,
                3,
                GameplayIntent::ProposeAlliance { target_player: 2 },
            );
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            assert_eq!(e.state.player(1).unwrap().alliance_timers[&2], 600);
            send(
                &mut e,
                2,
                GameplayIntent::AcceptAlliance { target_player: 3 },
            );
            for (player, ally, ticks) in clocks {
                assert_eq!(
                    e.state.player(player).unwrap().alliance_timers[&ally],
                    if active {
                        ALLIANCE_DURATION_TICKS
                    } else {
                        ticks
                    }
                );
            }
            let gain = e.state.player(1).unwrap().alliance_timers[&2] - 600;
            let marks = owner_marks(&e)[1];
            if active {
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((12, 16)), affected }
                        if affected == &[2, 3]
                )));
            }
            send(
                &mut e,
                3,
                GameplayIntent::BreakAlliance { target_player: 2 },
            );
            for (player, ally, ticks) in clocks {
                e.state
                    .player_mut(player)
                    .unwrap()
                    .alliance_timers
                    .insert(ally, ticks);
            }
            form(&mut e, 2, 3);
            assert_eq!(e.state.player(1).unwrap().alliance_timers[&2], 600);
            assert_eq!(owner_marks(&e)[1], marks);
            (f64::from(gain), marks)
        }
        Ability::SharedFeast => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = leader.ability() == Ability::SharedFeast;
            let run = |cap: f64| {
                let mut battle = arena(64, 32, leader, Some(Leader::of("cleopatra")));
                let sender = battle.state.player_mut(1).unwrap();
                sender.troops = 8_000.0;
                sender.max_troops = 10_000.0;
                sender.alliances.push(2);
                let ally = battle.state.player_mut(2).unwrap();
                ally.troops = 1_000.0;
                ally.max_troops = cap;
                ally.alliances.push(1);
                battle.add_building(make(1, 1, BuildingKind::Farm, 16, 16, 1, false));
                battle.add_building(make(2, 2, BuildingKind::Farm, 40, 16, 1, true));
                let gift = |battle: &mut SowEngine, troops| {
                    send(
                        battle,
                        1,
                        GameplayIntent::SendResources {
                            target_player: 2,
                            gold: 0.0,
                            troops,
                        },
                    );
                };
                if active {
                    assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Conditional);
                }
                gift(&mut battle, 500.0);
                assert_eq!(owner_marks(&battle)[1], 0);
                battle.execute_construction();
                if active {
                    assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Ready);
                }
                gift(&mut battle, 499.0);
                assert_eq!(owner_marks(&battle)[1], 0);
                assert_eq!(battle.state.player(1).unwrap().troops, 7_001.0);
                assert_eq!(battle.state.player(2).unwrap().troops, 1_999.0);
                gift(&mut battle, 2_400.0);
                let sender_bonus = battle.state.player(1).unwrap().troops - 4_601.0;
                let receiver_bonus = battle.state.player(2).unwrap().troops - 4_399.0;
                assert_eq!(sender_bonus, if active { 500.0 } else { 0.0 });
                assert_eq!(
                    receiver_bonus,
                    if active {
                        (cap - 4_399.0).min(500.0)
                    } else {
                        0.0
                    }
                );
                let marks = owner_marks(&battle)[1];
                if active {
                    assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Conditional);
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event,
                        GameEvent::AbilityTriggered { owner_id: 1, tile: Some((16, 16)), affected }
                            if affected == &[2]
                    )));
                }
                let before = battle.state.player(1).unwrap().troops;
                gift(&mut battle, 600.0);
                let transferable = (cap - (4_399.0 + receiver_bonus)).min(600.0);
                assert_eq!(
                    battle.state.player(1).unwrap().troops,
                    before - transferable
                );
                assert_eq!(owner_marks(&battle)[1], marks);
                (sender_bonus + receiver_bonus, marks)
            };
            run(4_500.0);
            run(10_000.0)
        }
        Ability::AkkadGarrison => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = leader.ability() == Ability::AkkadGarrison;
            e.add_building(make(1, 2, BuildingKind::City, 32, 16, 1, false));
            e.add_building(make(2, 1, BuildingKind::Tower, 26, 16, 1, true));
            e.state.next_building_id = 3;
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.state.set_tile_owner(32, 16, 1);
            e.sync_building_ownership();
            assert_eq!(e.buildings.len(), 2);
            e.state.set_tile_owner(32, 16, 2);
            e.sync_building_ownership();
            e.buildings.pop();
            e.building_grid.mark_dirty();
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            e.state.config.global_speed_multiplier = 100.0;
            e.state.player_mut(1).unwrap().gold = 12.0;
            e.state.player_mut(1).unwrap().troops = 10_000.0;
            e.state.player_mut(2).unwrap().troops = 0.0;
            send(
                &mut e,
                1,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(8_000.0),
                }),
            );
            e.attacks[0]
                .to_conquer
                .retain(|tile| (tile.x, tile.y) == (32, 16));
            e.execute_combat();
            assert_eq!(e.state.map.owner_id(32, 16), 1);
            e.sync_building_ownership();
            assert_eq!(e.state.player(1).unwrap().gold, 12.0);
            let gained = e.buildings.len() - 1;
            let marks = owner_marks(&e)[1];
            assert_eq!(gained, usize::from(active));
            if active {
                let tower = e.buildings.pop().unwrap();
                let tile = (tower.tile_idx % 64, tower.tile_idx / 64);
                assert_eq!(tower.kind, BuildingKind::Tower);
                assert_eq!(tower.active_level(), 1);
                assert_eq!(e.state.map.owner_id(tile.0, tile.1), 1);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some(mark), .. }
                        if *mark == tile
                )));
                e.building_grid.mark_dirty();
                for owner in [2, 1] {
                    e.state.set_tile_owner(32, 16, owner);
                    e.sync_building_ownership();
                }
                assert_eq!(e.buildings.len(), 1);
                assert_eq!(owner_marks(&e)[1], marks);
            }
            (gained as f64, marks)
        }
        Ability::TigranesAid { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready, Spent};
            let active = matches!(leader.ability(), Ability::TigranesAid { .. });
            let run = |ally: bool, elapsed_ticks: u64, committed: f64, cap: f64| {
                let mut battle = arena(64, 32, leader, Some(Leader::of("cleopatra")));
                battle.state.set_tile_owner(10, 16, 3);
                battle.add_building(make(1, 3, BuildingKind::City, 10, 16, 1, false));
                battle.state.player_mut(1).unwrap().troops = 5_000.0;
                battle.state.player_mut(1).unwrap().max_troops = cap;
                if ally {
                    battle.state.player_mut(1).unwrap().alliances.push(3);
                    battle.state.player_mut(3).unwrap().alliances.push(1);
                }
                let donor_troops = battle.state.player(3).unwrap().troops;
                send(
                    &mut battle,
                    1,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner: 2,
                        troops: Some(committed),
                    }),
                );
                battle.state.tick += elapsed_ticks;
                if active {
                    assert_eq!(
                        battle.hero_ability_feedback(1).unwrap().state,
                        if ally && elapsed_ticks >= 30 && 5_000.0 - committed * 0.25 < cap {
                            Ready
                        } else {
                            Conditional
                        },
                    );
                }
                let attack_id = battle.attacks[0].id;
                send(&mut battle, 1, GameplayIntent::CancelAttack { attack_id });
                battle.execute_combat();
                assert_eq!(battle.state.player(3).unwrap().troops, donor_troops);
                let ordinary = (5_000.0 - committed * 0.25).min(cap);
                let gained = battle.state.player(1).unwrap().troops - ordinary;
                let marks = owner_marks(&battle)[1];
                assert_eq!(marks, usize::from(gained > 0.0));
                if gained > 0.0 {
                    assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Spent);
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event,
                        GameEvent::AbilityTriggered { owner_id: 1, tile: Some((10, 16)), affected }
                            if affected == &[3]
                    )));
                    let before = battle.state.player(1).unwrap().troops;
                    send(
                        &mut battle,
                        1,
                        GameplayIntent::Attack(AttackIntent {
                            target_owner: 2,
                            troops: Some(1_000.0),
                        }),
                    );
                    battle.state.tick += 30;
                    let attack_id = battle.attacks[0].id;
                    send(&mut battle, 1, GameplayIntent::CancelAttack { attack_id });
                    battle.execute_combat();
                    assert_eq!(battle.state.player(1).unwrap().troops, before - 250.0);
                    assert_eq!(owner_marks(&battle)[1], marks);
                }
                (gained, marks)
            };
            assert_eq!(run(false, 30, 800.0, 10_000.0), (0.0, 0));
            assert_eq!(run(true, 29, 800.0, 10_000.0), (0.0, 0));
            assert_eq!(run(true, 30, 800.0, 4_800.0), (0.0, 0));
            assert_eq!(
                run(true, 30, 800.0, 4_900.0),
                if active { (100.0, 1) } else { (0.0, 0) }
            );
            assert_eq!(
                run(true, 30, 1_600.0, 10_000.0),
                if active { (1_000.0, 1) } else { (0.0, 0) }
            );
            run(true, 30, 800.0, 10_000.0)
        }
        Ability::AksumRout => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = leader.ability() == Ability::AksumRout;
            let fourth = Player::new_bot(4, "Fourth".into(), [0.8; 3], &e.state.config);
            e.state.register_player(fourth);
            e.state.set_tile_owner(61, 12, 4);
            e.state.set_tile_owner(40, 12, 0);
            e.add_building(make(1, 2, BuildingKind::City, 32, 16, 1, false));
            e.add_building(make(2, 2, BuildingKind::City, 32, 20, 1, true));
            let rival = e.state.player_mut(2).unwrap();
            rival.troops = 10_000.0;
            rival.max_troops = 20_000.0;
            for (target_owner, troops) in [(0, 2_500.0), (3, 800.0), (4, 1_500.0)] {
                send(
                    &mut e,
                    2,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner,
                        troops: Some(troops),
                    }),
                );
                if active && target_owner == 0 {
                    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
                }
            }
            assert_eq!(e.attacks.len(), 3);
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            e.state.set_tile_owner(32, 20, 1);
            e.sync_building_ownership();
            assert!(!e.attacks.iter().any(|a| a.retreating));
            e.state.set_tile_owner(32, 16, 1);
            e.sync_building_ownership();
            let recalled = e.attacks.iter().filter(|a| a.retreating).count();
            assert_eq!(recalled, usize::from(active));
            assert!(
                e.attacks
                    .iter()
                    .all(|a| a.retreating == (active && a.target_owner == 4))
            );
            let marks = owner_marks(&e)[1];
            if active {
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((32, 16)), affected }
                        if affected == &[2]
                )));
            }
            for owner in [2, 1] {
                e.state.set_tile_owner(32, 16, owner);
                e.sync_building_ownership();
            }
            assert_eq!(e.attacks.iter().filter(|a| a.retreating).count(), recalled);
            assert_eq!(owner_marks(&e)[1], marks);
            (recalled as f64, marks)
        }
        Ability::MountedCapture { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::MountedCapture { .. });
            let run = |retreat: bool, cap: f64| {
                let mut battle = arena(64, 32, leader, Some(Leader::of("cleopatra")));
                let own = battle.state.player_mut(1).unwrap();
                own.troops = 2_000.0;
                own.max_troops = cap;
                battle.state.player_mut(2).unwrap().troops = 2_000.0;
                if active {
                    assert_eq!(battle.hero_ability_feedback(1).unwrap().state, Conditional);
                }
                send(
                    &mut battle,
                    2,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner: 1,
                        troops: Some(800.0),
                    }),
                );
                let attack = &mut battle.attacks[0];
                assert_eq!(attack.owner_id, 2);
                attack.retreating = retreat;
                let frontier = attack
                    .to_conquer
                    .peek()
                    .map(|tile| (tile.x, tile.y))
                    .unwrap();
                if active {
                    assert_eq!(
                        battle.hero_ability_feedback(1).unwrap().state,
                        if retreat { Conditional } else { Ready },
                    );
                }
                send(
                    &mut battle,
                    1,
                    GameplayIntent::Attack(AttackIntent {
                        target_owner: 2,
                        troops: Some(1_000.0),
                    }),
                );
                let reserve_gain = battle.state.player(1).unwrap().troops - 1_000.0;
                let own_army = battle.attacks.iter().find(|a| a.owner_id == 1).unwrap();
                assert_eq!(own_army.troops, if retreat { 1_000.0 } else { 200.0 });
                let marks = owner_marks(&battle)[1];
                assert_eq!(marks, usize::from(reserve_gain > 0.0));
                if reserve_gain > 0.0 {
                    assert!(battle.state.events.iter().any(|event| matches!(
                        event,
                        GameEvent::AbilityTriggered { owner_id: 1, tile: Some(tile), affected }
                            if *tile == frontier && affected == &[2]
                    )));
                }
                (reserve_gain, marks)
            };
            assert_eq!(run(true, 2_000.0), (0.0, 0));
            assert_eq!(run(false, 900.0), (0.0, 0));
            assert_eq!(
                run(false, 1_050.0),
                if active { (50.0, 1) } else { (0.0, 0) },
            );
            let result = run(false, 2_000.0);
            assert_eq!(result, if active { (160.0, 1) } else { (0.0, 0) });
            result
        }
        Ability::KitchenPhilosophy => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = leader.ability() == Ability::KitchenPhilosophy;
            e.state.player_mut(1).unwrap().gold = 1_000_000.0;
            e.add_building(make(1, 1, BuildingKind::Farm, 16, 16, 1, false));
            e.add_building(make(2, 1, BuildingKind::City, 24, 16, 1, false));
            e.add_building(make(3, 1, BuildingKind::City, 29, 16, 1, false));
            e.add_building(make(4, 1, BuildingKind::City, 20, 16, 1, false));
            send(
                &mut e,
                1,
                GameplayIntent::UpgradeStructure { building_id: 1 },
            );
            while e.buildings[0].ticks_until_complete > 5 {
                e.execute_construction();
            }
            send(
                &mut e,
                1,
                GameplayIntent::UpgradeStructure { building_id: 3 },
            );
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            for building_id in [2, 4] {
                send(&mut e, 1, GameplayIntent::UpgradeStructure { building_id });
            }
            e.add_building(make(5, 1, BuildingKind::Farm, 18, 16, 1, true));
            let mut same_tick = make(6, 1, BuildingKind::City, 17, 16, 2, true);
            same_tick.ticks_until_complete = 5;
            e.add_building(same_tick);
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            let normal_remaining = e.buildings[3].ticks_until_complete - 5;
            for _ in 0..4 {
                e.execute_construction();
            }
            assert_eq!(owner_marks(&e)[1], 0, "new farms do not accelerate");
            e.execute_construction();
            assert_eq!(e.buildings[0].active_level(), 2);
            assert_eq!(e.buildings[5].active_level(), 2);
            assert_eq!(e.buildings[1].ticks_until_complete, normal_remaining);
            assert_eq!(e.buildings[2].ticks_until_complete, normal_remaining);
            let remaining = e.buildings[3].ticks_until_complete;
            assert_eq!(
                remaining,
                if active {
                    normal_remaining.div_ceil(2)
                } else {
                    normal_remaining
                }
            );
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered {
                        owner_id: 1,
                        tile: Some((20, 16)),
                        ..
                    }
                )));
            }
            (f64::from(normal_remaining - remaining), owner_marks(&e)[1])
        }
        Ability::SecondCrown => {
            use super::hero_effects::AbilityState::{Conditional, Ready, Spent};
            let active = leader.ability() == Ability::SecondCrown;
            let mut oldest = make(1, 2, BuildingKind::City, 40, 16, 1, true);
            oldest.ticks_until_complete = 50;
            e.add_building(oldest);
            e.add_building(Building {
                id: 2,
                tile_idx: 16 * 64 + 50,
                ..oldest
            });
            send(
                &mut e,
                2,
                GameplayIntent::ProposeAlliance { target_player: 1 },
            );
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.add_building(make(3, 1, BuildingKind::City, 15, 16, 1, false));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            assert_eq!(e.buildings[0].ticks_until_complete, 50);
            send(
                &mut e,
                1,
                GameplayIntent::AcceptAlliance { target_player: 2 },
            );
            assert!(e.state.player(1).unwrap().alliances.contains(&2));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Spent);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((40, 16)), affected }
                        if affected == &[2]
                )));
            }
            e.execute_construction();
            assert_eq!(e.buildings[0].active_level(), u8::from(active));
            assert_eq!(e.buildings[1].ticks_until_complete, 49);
            if active {
                assert!(
                    e.state
                        .events
                        .iter()
                        .any(|event| matches!(event, GameEvent::StructureReady { id: 1, .. }))
                );
            }

            // A second otherwise eligible alliance cannot accelerate another city.
            e.state.set_tile_owner(31, 12, 3);
            e.add_building(Building {
                id: 4,
                owner_id: 3,
                tile_idx: 12 * 64 + 31,
                ..oldest
            });
            send(
                &mut e,
                1,
                GameplayIntent::ProposeAlliance { target_player: 3 },
            );
            send(
                &mut e,
                3,
                GameplayIntent::AcceptAlliance { target_player: 1 },
            );
            assert!(e.state.player(1).unwrap().alliances.contains(&3));
            assert_eq!(e.buildings[3].ticks_until_complete, 50);
            (f64::from(e.buildings[0].active_level()), owner_marks(&e)[1])
        }
        Ability::CookhamExchange => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = leader.ability() == Ability::CookhamExchange;
            e.add_building(make(1, 1, BuildingKind::Farm, 31, 16, 2, false));
            e.add_building(make(2, 1, BuildingKind::City, 18, 16, 1, false));
            e.add_building(make(3, 1, BuildingKind::City, 24, 16, 4, false));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.add_building(make(4, 1, BuildingKind::City, 26, 16, 2, false));
            e.add_building(make(5, 1, BuildingKind::City, 29, 16, 3, false));
            e.add_building(make(6, 1, BuildingKind::City, 25, 16, 1, false));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            e.state.player_mut(1).unwrap().alliances.push(2);
            e.state.player_mut(2).unwrap().alliances.push(1);
            e.state.set_tile_owner(31, 16, 2);
            e.sync_building_ownership();
            assert_eq!(e.buildings[3].level, 2);
            assert_eq!(owner_marks(&e)[1], 0);
            e.state.set_tile_owner(31, 16, 1);
            e.sync_building_ownership();
            e.state.player_mut(1).unwrap().alliances.clear();
            e.state.player_mut(2).unwrap().alliances.clear();
            e.state.set_tile_owner(25, 16, 2);
            e.state.set_tile_owner(31, 16, 2);
            e.sync_building_ownership();
            assert_eq!(e.buildings[0].owner_id, 2);
            assert_eq!(e.buildings[5].owner_id, 2);
            assert_eq!(e.buildings[5].level, 1);
            assert_eq!(e.buildings[1].level, 1);
            assert_eq!(e.buildings[4].level, 3);
            let gained = e.buildings[3].level - 2;
            assert_eq!(gained, u8::from(active));
            let marks = owner_marks(&e)[1];
            if active {
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((26, 16)), affected }
                        if affected == &[2]
                )));
            }
            e.state.set_tile_owner(31, 16, 1);
            e.sync_building_ownership();
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.state.set_tile_owner(31, 16, 2);
            e.sync_building_ownership();
            assert_eq!(e.buildings[3].level, 2 + gained);
            assert_eq!(owner_marks(&e)[1], marks);
            (f64::from(gained), marks)
        }
        Ability::PuntGardens => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = leader.ability() == Ability::PuntGardens;
            let mut trade = engine_with_ports(
                &[port(1, 1, 650), port(2, 2, 660)],
                vec![lane(
                    1,
                    1,
                    2,
                    &[651, 652, 653, 654, 655, 656, 657, 658, 659],
                )],
            );
            trade.state.set_tile_owner(10, 10, 1);
            trade.state.player_mut(1).unwrap().leader = leader;
            trade.add_building(make(3, 1, BuildingKind::Farm, 10, 12, 2, false));
            trade.add_building(make(4, 1, BuildingKind::Farm, 10, 14, 1, false));
            trade.add_building(make(5, 1, BuildingKind::Farm, 10, 23, 1, false));
            trade.execute_trade_ships();
            assert_eq!(trade.buildings[3].level, 1);
            assert_eq!(trade.fleets.len(), 1);
            if active {
                assert_eq!(trade.hero_ability_feedback(1).unwrap().state, Ready);
            }
            for _ in 0..32 {
                if trade.fleets[0].path.is_empty() {
                    break;
                }
                trade.execute_fleets();
            }
            assert!(trade.fleets[0].path.is_empty());
            trade.execute_trade_ships();
            let gained = trade.buildings[3].level - 1;
            assert_eq!(gained, u8::from(active));
            assert_eq!(trade.buildings[2].level, 2);
            assert_eq!(trade.buildings[4].level, 1);
            if active {
                assert_eq!(trade.hero_ability_feedback(1).unwrap().state, Conditional);
                assert!(trade.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered {
                        owner_id: 1,
                        tile: Some((10, 14)),
                        ..
                    }
                )));
            }
            for _ in 0..32 {
                if trade.fleets[0].path.is_empty() {
                    break;
                }
                trade.execute_fleets();
            }
            assert!(trade.fleets[0].path.is_empty());
            trade.execute_trade_ships();
            assert_eq!(trade.buildings[3].level, 1 + gained);
            (f64::from(gained), owner_marks(&trade)[1])
        }
        Ability::CityDenial => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let denies = leader.ability() == Ability::CityDenial;
            let (x, y) = (16, 16);
            e.add_building(make(1, 1, BuildingKind::City, x, y, 3, false));
            e.state.player_mut(1).unwrap().gold = 1_000_000.0;
            send(
                &mut e,
                1,
                GameplayIntent::UpgradeStructure { building_id: 1 },
            );
            assert!(e.buildings[0].under_construction);
            assert_eq!(e.buildings[0].active_level(), 3);
            if denies {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            e.state.set_tile_owner(x, y, 2);
            e.sync_building_ownership();
            assert_eq!(e.buildings[0].owner_id, 2);
            let denied = 3 - e.buildings[0].active_level();
            assert_eq!(denied, u8::from(denies));
            assert_eq!(e.buildings[0].under_construction, !denies);
            if denies {
                assert_eq!(e.buildings[0].ticks_until_complete, 0);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((16, 16)), affected }
                        if affected == &[2]
                )));
            }
            e.state.set_tile_owner(x, y, 1);
            e.sync_building_ownership();
            if denies {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.state.set_tile_owner(x, y, 2);
            e.sync_building_ownership();
            assert_eq!(e.buildings[0].active_level(), 3 - denied);
            (f64::from(denied), owner_marks(&e)[1])
        }
        Ability::RetreatHandover { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let has_handover = matches!(leader.ability(), Ability::RetreatHandover { .. });
            e.state.set_tile_owner(32, 16, 3);
            e.state.player_mut(1).unwrap().alliances.push(3);
            e.state.player_mut(3).unwrap().alliances.push(1);
            e.state.player_mut(2).unwrap().troops = 10_000.0;
            e.state.player_mut(2).unwrap().max_troops = 12_000.0;
            e.state.player_mut(3).unwrap().troops = 10_000.0;
            e.state.player_mut(3).unwrap().max_troops = 12_000.0;
            send(
                &mut e,
                2,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 1,
                    troops: Some(4000.0),
                }),
            );
            if has_handover {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            send(
                &mut e,
                3,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(4000.0),
                }),
            );
            // Preserve the active armies while measuring only the retreat.
            e.state.config.global_speed_multiplier = 0.0;
            let checkpoint = bincode::serialize(&e).unwrap();
            let mut received = 0.0;
            let mut shown = 0;
            for (cap, expected) in [(6300.0, 300.0), (12_000.0, 750.0)] {
                let mut run: SowEngine = bincode::deserialize(&checkpoint).unwrap();
                run.state.player_mut(3).unwrap().max_troops = cap;
                if has_handover {
                    assert_eq!(run.hero_ability_feedback(1).unwrap().state, Ready);
                }
                let attack_id = run.attacks.iter().find(|a| a.owner_id == 2).unwrap().id;
                send(&mut run, 2, GameplayIntent::CancelAttack { attack_id });
                run.execute_combat();
                let gained = run.state.player(3).unwrap().troops - 6000.0;
                assert_eq!(gained, if has_handover { expected } else { 0.0 });
                assert_eq!(run.state.player(2).unwrap().troops, 9000.0 - gained);
                if has_handover {
                    assert_eq!(run.hero_ability_feedback(1).unwrap().state, Conditional);
                    assert!(run.state.events.iter().any(|event| matches!(
                        event,
                        GameEvent::AbilityTriggered { owner_id: 1, tile: Some(_), affected }
                            if affected == &[3, 2]
                    )));
                }
                received += gained;
                shown += owner_marks(&run)[1];
            }
            (received, shown)
        }
        Ability::CityIntercession { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let active = matches!(leader.ability(), Ability::CityIntercession { .. });
            e.state.config.global_speed_multiplier = 100.0;
            e.state.set_tile_owner(32, 16, 3);
            e.state.player_mut(1).unwrap().troops = 3000.0;
            e.state.player_mut(2).unwrap().troops = 0.0;
            e.state.player_mut(3).unwrap().troops = 10_000.0;
            e.state.player_mut(1).unwrap().alliances.push(2);
            e.state.player_mut(2).unwrap().alliances.push(1);
            e.add_building(make(1, 2, BuildingKind::City, 33, 16, 1, false));
            if active {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
            }
            send(
                &mut e,
                3,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(5000.0),
                }),
            );
            let attack = e.attacks.iter_mut().find(|a| a.owner_id == 3).unwrap();
            attack
                .to_conquer
                .retain(|tile| (tile.x, tile.y) == (33, 16));
            assert!(!attack.to_conquer.is_empty());
            e.execute_combat();
            let saved = e.state.map.owner_id(33, 16) == 2;
            let shown = owner_marks(&e)[1];
            assert_eq!(saved, active);
            if active {
                assert_eq!(e.state.player(1).unwrap().troops, 1500.0);
                assert_eq!(
                    e.attacks.iter().find(|a| a.owner_id == 3).unwrap().troops,
                    3500.0
                );
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((33, 16)), affected }
                        if affected == &[2, 3]
                )));
                e.execute_combat();
                assert_eq!(e.state.map.owner_id(33, 16), 3);
                assert_eq!(e.state.player(1).unwrap().troops, 1500.0);
            }
            (f64::from(u8::from(saved)), shown)
        }
        Ability::CounterLevy { .. } => {
            use super::hero_effects::AbilityState::{Conditional, Ready};
            let has_levy = matches!(leader.ability(), Ability::CounterLevy { .. });
            let initial_gold = e.state.player(1).unwrap().gold;
            e.add_building(make(1, 2, BuildingKind::Tower, 32, 16, 1, false));
            if has_levy {
                assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
            }
            e.state.set_tile_owner(32, 16, 1);
            e.sync_building_ownership();
            assert_eq!(e.state.player(1).unwrap().gold, initial_gold);
            e.state.set_tile_owner(32, 16, 2);
            e.sync_building_ownership();

            e.state.player_mut(2).unwrap().troops = 10_000.0;
            send(
                &mut e,
                2,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 1,
                    troops: Some(1000.0),
                }),
            );
            for (id, x, rival_gold, expected) in [(1, 32, 600.0, 150.0), (2, 36, 2000.0, 250.0)] {
                if id == 2 {
                    e.add_building(make(id, 2, BuildingKind::Tower, x, 16, 1, false));
                }
                e.state.player_mut(2).unwrap().gold = rival_gold;
                if has_levy {
                    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
                }
                let before = e.state.player(1).unwrap().gold;
                e.state.set_tile_owner(x, 16, 1);
                e.sync_building_ownership();
                let gained = e.state.player(1).unwrap().gold - before;
                assert_eq!(gained, if has_levy { expected } else { 0.0 });
                assert_eq!(rival_gold - e.state.player(2).unwrap().gold, gained);
                e.state.set_tile_owner(x, 16, 2);
                e.sync_building_ownership();
                e.state.set_tile_owner(x, 16, 1);
                e.sync_building_ownership();
                assert_eq!(e.state.player(1).unwrap().gold - before, gained);
            }
            if has_levy {
                assert!(e.state.events.iter().any(|event| matches!(
                    event,
                    GameEvent::AbilityTriggered { owner_id: 1, tile: Some((32, 16)), affected }
                        if affected == &[2]
                )));
            }
            (
                e.state.player(1).unwrap().gold - initial_gold,
                owner_marks(&e)[1],
            )
        }
        Ability::SharedCitySpoils { .. } => {
            let before = e.state.player(1).unwrap().gold;
            e.add_building(make(1, 2, BuildingKind::City, 40, 40, 1, false));
            e.state.set_tile_owner(40, 40, 1);
            e.sync_building_ownership();
            let value = e.state.player(1).unwrap().gold - before;
            e.state.set_tile_owner(40, 40, 2);
            e.sync_building_ownership();
            e.state.set_tile_owner(40, 40, 1);
            e.sync_building_ownership();
            assert_eq!(
                e.state.player(1).unwrap().gold - before,
                value,
                "recapture cannot mint gold again"
            );
            (value, owner_marks(&e)[1])
        }
        Ability::CityEvacuation { .. } => {
            e.state.player_mut(1).unwrap().troops = 100.0;
            e.state.player_mut(1).unwrap().max_troops = 10_000.0;
            e.add_building(make(1, 1, BuildingKind::City, 16, 16, 1, false));
            e.state.set_tile_owner(16, 16, 2);
            e.sync_building_ownership();
            let value = e.state.player(1).unwrap().troops;
            e.state.set_tile_owner(16, 16, 1);
            e.sync_building_ownership();
            e.state.set_tile_owner(16, 16, 2);
            e.sync_building_ownership();
            assert_eq!(
                e.state.player(1).unwrap().troops,
                value,
                "each city has one reserve"
            );
            (value, owner_marks(&e)[1])
        }
        Ability::ReturnCampaign { .. } => {
            e.state.player_mut(1).unwrap().troops = 20_000.0;
            send(
                &mut e,
                1,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(5_000.0),
                }),
            );
            let id = e.attacks[0].id;
            e.state.tick = 31;
            send(&mut e, 1, GameplayIntent::CancelAttack { attack_id: id });
            e.execute_combat();
            e.state.events.clear();
            send(
                &mut e,
                1,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(1_000.0),
                }),
            );
            let strength = attack_strength(&e.state, 1, 2, 1_000.0);
            let shown = owner_marks(&e)[1];
            if matches!(leader.ability(), Ability::ReturnCampaign { .. }) {
                assert!(
                    e.build_snapshot()
                        .players
                        .iter()
                        .find(|p| p.id == 1)
                        .unwrap()
                        .empowered_attack
                );
                e.state.tick = 331;
                assert!(
                    !e.build_snapshot()
                        .players
                        .iter()
                        .find(|p| p.id == 1)
                        .unwrap()
                        .empowered_attack
                );
            }
            (strength, shown)
        }
        Ability::AllyPatronage { .. }
        | Ability::SurplusTreasury { .. }
        | Ability::GuildNetwork { .. } => {
            match situation {
                Ability::AllyPatronage { .. } => {
                    e.state.player_mut(1).unwrap().alliances.push(2);
                    e.state.player_mut(2).unwrap().alliances.push(1);
                    e.state.set_tile_owner(31, 20, 2);
                }
                Ability::SurplusTreasury { .. } => {
                    e.state.player_mut(1).unwrap().troops = 1e9;
                }
                Ability::GuildNetwork { .. } => {
                    e.add_building(make(1, 1, BuildingKind::City, 16, 16, 1, false));
                    e.add_building(make(2, 1, BuildingKind::Factory, 24, 16, 1, false));
                }
                _ => unreachable!(),
            }
            e.refresh_building_aggregates();
            let before = e.state.player(1).unwrap().gold;
            let agg = e.building_aggregates[1];
            let ordinary = e.state.config.per_tick(gold_income_per_second(
                e.state.player(1).unwrap().tile_count,
                agg,
                leader,
                &e.state.config,
            ));
            e.execute_income();
            let value = e.state.player(1).unwrap().gold - before - ordinary;
            let shown = owner_marks(&e)[1];
            e.state.events.clear();
            e.execute_income();
            assert_eq!(
                owner_marks(&e)[1],
                0,
                "continuous income does not spam activation every tick"
            );
            (value, shown)
        }
        Ability::WarshipPrize { .. } => {
            let tile = 8;
            e.state.map.terrain[tile as usize] = MapTile::from_byte(0x20);
            e.add_fleet(WarpFleet::new(
                1,
                2,
                1,
                UnitType::Warship,
                100.0,
                (tile, tile),
                vec![],
            ));
            e.projectiles.push(crate::game::Projectile {
                id: 1,
                kind: crate::game::ProjectileKind::Shell,
                owner_id: 1,
                src_tile: tile,
                dst_tile: tile,
                path: vec![tile],
                path_cursor: 0,
                steps_per_tick: 3,
                active: true,
                source_fleet_id: None,
                target_fleet_id: Some(1),
                damage: 200.0,
                expires_at_tick: 50,
            });
            e.execute_projectiles();
            (
                e.fleets
                    .iter()
                    .filter(|f| f.owner_id == 1)
                    .map(|f| f.troops)
                    .sum(),
                owner_marks(&e)[1],
            )
        }
        _ => panic!("not a redesigned situation"),
    }
}

#[test]
fn redesigned_heroes_require_their_actual_situation() {
    for id in ["puyi", "pertinax", "ewuare"] {
        let mut e = arena(64, 32, Leader::of(id), Some(Leader::of("cleopatra")));
        e.state.player_mut(1).unwrap().troops = 100.0;
        e.state.player_mut(1).unwrap().max_troops = 10_000.0;
        e.refresh_building_aggregates();
        assert_eq!(
            e.hero_gold_per_second(1),
            0.0,
            "{id} has no unconditional income bonus"
        );
        e.execute_income();
        assert_eq!(
            owner_marks(&e)[1],
            0,
            "{id} stays unmarked outside its situation"
        );
    }
    let mut e = arena(
        64,
        32,
        Leader::of("muhammad_of_ghor"),
        Some(Leader::of("cleopatra")),
    );
    e.state.player_mut(1).unwrap().return_campaign = Some((2, 300));
    assert_eq!(attack_strength(&e.state, 1, 2, 100.0), 1.35);
    assert_eq!(attack_strength(&e.state, 1, 0, 100.0), 1.0);
    e.state.tick = 300;
    assert_eq!(
        attack_strength(&e.state, 1, 2, 100.0),
        1.0,
        "the return window expires"
    );
    e.state.player_mut(1).unwrap().troops = 10_000.0;
    send(
        &mut e,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(1_000.0),
        }),
    );
    let id = e.attacks[0].id;
    send(&mut e, 1, GameplayIntent::CancelAttack { attack_id: id });
    e.execute_combat();
    assert_eq!(
        attack_strength(&e.state, 1, 2, 100.0),
        1.0,
        "an immediate cancellation grants no return bonus"
    );
}

fn redesigned_building(
    id: u64,
    owner_id: u16,
    kind: BuildingKind,
    x: u32,
    y: u32,
    level: u8,
) -> Building {
    Building {
        id,
        owner_id,
        kind,
        tile_idx: y * 64 + x,
        level,
        under_construction: false,
        ticks_until_complete: 0,
    }
}

#[test]
fn redesigned_peter_founds_a_legal_port_during_real_conquest() {
    for landed in [false, true] {
        let mut e = arena(
            64,
            32,
            Leader::of("peter_the_great"),
            Some(Leader::of("cleopatra")),
        );
        for y in 0..4 {
            for x in 0..64 {
                e.state.map.terrain[(y * 64 + x) as usize] = MapTile::from_byte(0x20);
            }
        }
        e.state.map.compute_shorelines();
        // An otherwise identical neutral coast cannot consume the one-use founding.
        e.state.events.push(GameEvent::TileCaptured {
            x: 31,
            y: 4,
            new_owner: 1,
            previous_owner: 0,
            troops: 100.0,
        });
        e.hero_coastal_foundings();
        assert!(e.buildings.is_empty());
        if landed {
            let shore = 4 * 64 + 32;
            e.add_fleet(WarpFleet::new(
                1,
                1,
                2,
                UnitType::TransportShip,
                10_000.0,
                (shore, shore),
                vec![shore],
            ));
            e.execute_fleets();
        } else {
            e.state.player_mut(1).unwrap().troops = 30_000.0;
            send(
                &mut e,
                1,
                GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(20_000.0),
                }),
            );
            for _ in 0..200 {
                e.state.tick += 1;
                e.execute_combat();
                if !e.buildings.is_empty() {
                    break;
                }
            }
        }
        assert_eq!(e.buildings.len(), 1);
        let port = e.buildings[0];
        assert_eq!(port.kind, BuildingKind::Port);
        assert!(!port.under_construction);
        assert!(
            crate::building::validate_structure_site(
                &e.state.map,
                1,
                BuildingKind::Port,
                port.tile_idx,
                &crate::building::BuildingGrid::default()
            )
            .is_ok()
        );
        let gold = e.state.player(1).unwrap().gold;
        e.hero_coastal_foundings();
        assert_eq!(e.buildings.len(), 1);
        assert_eq!(e.state.player(1).unwrap().gold, gold);
        assert_eq!(owner_marks(&e)[1], 1);
    }
}

#[test]
fn natavan_founds_one_farm_at_a_legal_site_when_the_city_reaches_level_two() {
    use super::hero_effects::AbilityState::{Conditional, Ready};

    let leader = Leader::of("natavan");
    let mut engine = arena(64, 32, leader, Some(Leader::of("cleopatra")));
    engine.state.map.terrain.fill(MapTile::from_byte(0x8a));
    let city = Building {
        id: 1,
        owner_id: 1,
        tile_idx: 16 * 64 + 16,
        kind: BuildingKind::City,
        level: 1,
        under_construction: false,
        ticks_until_complete: 0,
    };
    engine.add_building(city);
    engine.state.next_building_id = 2;
    engine.state.player_mut(1).unwrap().gold = 1_000_000.0;
    // This diagonal site fits inside the search box, but is over twelve tiles away.
    for y in 16..=17 {
        for x in 27..=28 {
            engine.state.map.terrain[y * 64 + x] = MapTile::from_byte(0x80);
        }
    }
    assert_eq!(engine.hero_ability_feedback(1).unwrap().state, Conditional);
    let bytes = bincode::serialize(&engine).unwrap();
    let mut blocked: SowEngine = bincode::deserialize(&bytes).unwrap();
    send(
        &mut blocked,
        1,
        GameplayIntent::UpgradeStructure {
            building_id: city.id,
        },
    );
    for _ in 0..blocked.buildings[0].ticks_until_complete {
        blocked.execute_construction();
    }
    assert_eq!(blocked.buildings[0].level, 2);
    assert_eq!(blocked.buildings.len(), 1);
    assert_eq!(owner_marks(&blocked)[1], 0);

    // The only other eligible foundation is exactly twelve tiles from the city.
    for y in 15..=16 {
        for x in 27..=28 {
            engine.state.map.terrain[y * 64 + x] = MapTile::from_byte(0x80);
        }
    }
    assert_eq!(engine.hero_ability_feedback(1).unwrap().state, Ready);
    send(
        &mut engine,
        1,
        GameplayIntent::UpgradeStructure {
            building_id: city.id,
        },
    );
    let gold_after_upgrade = engine.state.player(1).unwrap().gold;
    let ticks = engine.buildings[0].ticks_until_complete;
    assert!(ticks > 1);
    for _ in 0..ticks - 1 {
        engine.execute_construction();
    }
    assert_eq!(engine.buildings.len(), 1);
    engine.execute_construction();
    let farm = engine.buildings[1];
    assert_eq!(farm.kind, BuildingKind::Farm);
    assert_eq!(farm.tile_idx, 16 * 64 + 28);
    assert_eq!(farm.active_level(), 1);
    assert_eq!(engine.state.player(1).unwrap().gold, gold_after_upgrade);
    assert!(engine.state.events.iter().any(|event| matches!(
        event,
        GameEvent::AbilityTriggered {
            owner_id: 1,
            tile: Some((28, 16)),
            ..
        }
    )));
    assert_eq!(engine.hero_ability_feedback(1).unwrap().state, Conditional);
    engine.refresh_building_aggregates();
    assert_eq!(engine.building_aggregates[1].farm_levels, 1);

    send(
        &mut engine,
        1,
        GameplayIntent::UpgradeStructure {
            building_id: city.id,
        },
    );
    for _ in 0..=engine.buildings[0].ticks_until_complete {
        engine.execute_construction();
    }
    assert_eq!(engine.buildings[0].active_level(), 3);
    assert_eq!(engine.buildings.len(), 2);
}

#[test]
fn redesigned_city_rewards_require_allies_upgrades_and_range() {
    let mut e = arena(
        64,
        32,
        Leader::of("empress_suiko"),
        Some(Leader::of("cleopatra")),
    );
    let mut city = redesigned_building(1, 2, BuildingKind::City, 40, 40, 1);
    city.under_construction = true;
    city.ticks_until_complete = 1;
    e.add_building(city);
    let gold = e.state.player(1).unwrap().gold;
    e.execute_construction();
    assert_eq!(
        e.state.player(1).unwrap().gold,
        gold,
        "a rival's new city pays nothing"
    );
    e.state.player_mut(1).unwrap().alliances.push(2);
    e.state.player_mut(2).unwrap().alliances.push(1);
    e.buildings[0].level = 2;
    e.buildings[0].under_construction = true;
    e.buildings[0].ticks_until_complete = 1;
    e.execute_construction();
    assert_eq!(
        e.state.player(1).unwrap().gold,
        gold,
        "allied upgrades are not new foundations"
    );
    e.state.player_mut(1).unwrap().leader = Leader::of("nebuchadnezzar_ii");
    e.buildings.clear();
    e.building_grid = Default::default();
    city = redesigned_building(2, 1, BuildingKind::City, 10, 10, 2);
    e.add_building(city);
    e.add_building(redesigned_building(3, 1, BuildingKind::Tower, 31, 10, 1));
    e.hero_structure_ready(city);
    assert_eq!(e.buildings[1].level, 1, "a distant tower cannot be raised");
    e.buildings[1].tile_idx = 20 * 64 + 10;
    e.buildings[1].level = 4;
    e.hero_structure_ready(city);
    assert_eq!(
        e.buildings[1].level, 4,
        "a tower cannot exceed the level cap"
    );
    e.buildings[1].level = 2;
    e.hero_structure_ready(city);
    assert_eq!(e.buildings[1].level, 3);
}

#[test]
fn redesigned_city_spoils_are_shared_and_evacuation_respects_the_cap() {
    let mut e = arena(
        64,
        32,
        Leader::of("king_david"),
        Some(Leader::of("cleopatra")),
    );
    e.state.player_mut(1).unwrap().alliances.push(3);
    e.state.player_mut(3).unwrap().alliances.push(1);
    let mine = e.state.player(1).unwrap().gold;
    let ally = e.state.player(3).unwrap().gold;
    e.add_building(redesigned_building(1, 2, BuildingKind::City, 40, 40, 1));
    e.state.set_tile_owner(40, 40, 1);
    e.sync_building_ownership();
    assert_eq!(e.state.player(1).unwrap().gold - mine, 100.0);
    assert_eq!(e.state.player(3).unwrap().gold - ally, 100.0);
    assert!(e.state.events.iter().any(|event| matches!(event, GameEvent::AbilityTriggered { affected, .. } if affected.contains(&3))));
    let mut e = arena(
        64,
        32,
        Leader::of("sarraounia_mangou"),
        Some(Leader::of("cleopatra")),
    );
    e.state.player_mut(1).unwrap().troops = 900.0;
    e.state.player_mut(1).unwrap().max_troops = 1_000.0;
    e.add_building(redesigned_building(1, 1, BuildingKind::City, 16, 16, 1));
    e.state.set_tile_owner(16, 16, 2);
    e.sync_building_ownership();
    assert_eq!(e.state.player(1).unwrap().troops, 1_000.0);
    e.state.player_mut(1).unwrap().alive = false;
    e.add_building(redesigned_building(2, 1, BuildingKind::City, 20, 20, 1));
    e.state.set_tile_owner(20, 20, 2);
    e.sync_building_ownership();
    assert!(
        !e.state.player(1).unwrap().alive,
        "losing a city cannot resurrect a defeated player"
    );
}

#[test]
fn redesigned_income_stops_when_its_condition_ends_and_matches_the_hud_rate() {
    let mut e = arena(64, 16, Leader::of("puyi"), Some(Leader::of("cleopatra")));
    e.state.player_mut(1).unwrap().alliances.push(2);
    e.state.player_mut(2).unwrap().alliances.push(1);
    e.refresh_building_aggregates();
    let extra = e.hero_gold_per_second(1);
    assert!(extra > 0.0);
    let ordinary = gold_income_per_second(
        e.state.player(1).unwrap().tile_count,
        e.building_aggregates[1],
        Leader::of("puyi"),
        &e.state.config,
    );
    let before = e.state.player(1).unwrap().gold;
    e.execute_income();
    assert!(
        (e.state.player(1).unwrap().gold - before - e.state.config.per_tick(ordinary + extra))
            .abs()
            < 1e-9
    );
    e.state.player_mut(2).unwrap().alliances.clear();
    assert_eq!(e.hero_gold_per_second(1), 0.0);
    e.state.player_mut(1).unwrap().leader = Leader::of("pertinax");
    e.state.player_mut(1).unwrap().troops = e.state.player(1).unwrap().max_troops;
    assert!(e.hero_gold_per_second(1) > 0.0);
    e.state.player_mut(2).unwrap().troops = 10_000.0;
    send(
        &mut e,
        2,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 1,
            troops: Some(1_000.0),
        }),
    );
    assert_eq!(
        e.hero_gold_per_second(1),
        0.0,
        "an incoming attack ends austerity"
    );
    e.state.player_mut(1).unwrap().leader = Leader::of("ewuare");
    e.add_building(redesigned_building(1, 1, BuildingKind::City, 10, 10, 1));
    e.add_building(redesigned_building(2, 1, BuildingKind::Factory, 15, 10, 2));
    assert_eq!(e.hero_gold_per_second(1), 2.0);
    e.add_building(redesigned_building(3, 1, BuildingKind::City, 10, 15, 1));
    assert_eq!(
        e.hero_gold_per_second(1),
        2.0,
        "the same factory is counted only once"
    );
    e.buildings[0].owner_id = 2;
    e.buildings[2].owner_id = 2;
    assert_eq!(
        e.hero_gold_per_second(1),
        0.0,
        "losing the supporting cities ends the guild income"
    );
}

#[test]
fn redesigned_veleda_prizes_survive_other_shells_in_the_same_batch() {
    let mut e = arena(64, 32, Leader::of("veleda"), Some(Leader::of("cleopatra")));
    e.add_fleet(WarpFleet::new(
        1,
        2,
        1,
        UnitType::Warship,
        100.0,
        (8, 8),
        vec![],
    ));
    for id in 1..=2 {
        e.projectiles.push(crate::game::Projectile {
            id,
            kind: crate::game::ProjectileKind::Shell,
            owner_id: 1,
            src_tile: 8,
            dst_tile: 8,
            path: vec![8],
            path_cursor: 0,
            steps_per_tick: 3,
            active: true,
            source_fleet_id: None,
            target_fleet_id: Some(1),
            damage: 1_000.0,
            expires_at_tick: 50,
        });
    }
    e.execute_projectiles();
    assert_eq!(e.fleets[0].owner_id, 1);
    assert_eq!(e.fleets[0].troops, 250.0);
    assert_eq!(e.fleets[0].warship.as_ref().unwrap().veterancy, 0);
    assert!(
        !e.state
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::NavalImpact { sunk: true, .. }))
    );
}

#[test]
fn redesigned_hero_claims_and_return_windows_survive_checkpoints() {
    let mut e = arena(
        64,
        32,
        Leader::of("muhammad_of_ghor"),
        Some(Leader::of("cleopatra")),
    );
    e.hero_claims.insert((1, 10, 0));
    e.hero_income_active.insert(1);
    e.state.player_mut(1).unwrap().return_campaign = Some((2, 300));
    let bytes = bincode::serialize(&e).unwrap();
    let restored: SowEngine = bincode::deserialize(&bytes).unwrap();
    assert_eq!(e.hero_claims, restored.hero_claims);
    assert_eq!(e.hero_income_active, restored.hero_income_active);
    assert_eq!(attack_strength(&restored.state, 1, 2, 100.0), 1.35);
}

#[test]
fn every_hero_has_live_feedback_for_its_automatic_ability() {
    use super::hero_effects::{AbilityRestriction, AbilityState};
    for hero in Leader::all() {
        let e = arena(64, 32, hero, Some(Leader::of("cleopatra")));
        let status = e.hero_ability_feedback(1).expect("every hero has feedback");
        assert_ne!(
            status.state,
            AbilityState::Triggered,
            "a persistent state cannot invent an activation"
        );
        if matches!(hero.ability(), Ability::AllianceCost { .. }) {
            assert_eq!(status.state, AbilityState::Unavailable);
            assert_eq!(status.restriction, Some(AbilityRestriction::CampaignOnly));
        }
    }
}

#[test]
fn hero_feedback_tracks_consumption_return_timer_income_and_required_structures() {
    use super::hero_effects::AbilityState::*;
    let mut e = arena(
        64,
        32,
        Leader::of("peter_the_great"),
        Some(Leader::of("cleopatra")),
    );
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
    e.hero_claims.insert((1, 0, 0));
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Spent);
    e.state.player_mut(1).unwrap().leader = Leader::of("empress_suiko");
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.state.player_mut(1).unwrap().alliances.push(2);
    e.state.player_mut(2).unwrap().alliances.push(1);
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
    e.state.player_mut(1).unwrap().leader = Leader::of("muhammad_of_ghor");
    e.state.tick = 100;
    e.state.player_mut(1).unwrap().return_campaign = Some((2, 400));
    assert_eq!(
        e.hero_ability_feedback(1).unwrap().remaining_seconds,
        Some(30)
    );
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
    // End the alliance so the attack is legal and the timer can be used.
    e.state.player_mut(1).unwrap().alliances.clear();
    e.state.player_mut(2).unwrap().alliances.clear();
    e.state.player_mut(1).unwrap().troops = 10_000.0;
    send(
        &mut e,
        1,
        GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(1_000.0),
        }),
    );
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Active);
    e.state.tick = 200;
    assert_eq!(
        e.hero_ability_feedback(1).unwrap().remaining_seconds,
        Some(20)
    );
    e.state.tick = 400;
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    assert_eq!(e.hero_ability_feedback(1).unwrap().remaining_seconds, None);
    e.attacks.clear();
    e.state.player_mut(1).unwrap().leader = Leader::of("nebuchadnezzar_ii");
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.add_building(redesigned_building(1, 1, BuildingKind::City, 10, 10, 1));
    e.add_building(redesigned_building(2, 1, BuildingKind::Tower, 18, 10, 1));
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
    e.state.player_mut(1).unwrap().leader = Leader::of("sarraounia_mangou");
    e.state.player_mut(1).unwrap().troops = e.state.player(1).unwrap().max_troops;
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.state.player_mut(1).unwrap().troops = 1.0;
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
    e.hero_claims.insert((1, 1, 0));
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.state.player_mut(1).unwrap().leader = Leader::of("king_david");
    e.add_building(redesigned_building(3, 2, BuildingKind::City, 40, 40, 1));
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
    e.hero_claims.insert((1, 3, 0));
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.state.player_mut(1).unwrap().leader = Leader::of("ewuare");
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.add_building(redesigned_building(4, 1, BuildingKind::Factory, 12, 15, 1));
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Active);
    e.state.player_mut(1).unwrap().leader = Leader::of("pertinax");
    e.refresh_building_aggregates();
    e.state.player_mut(1).unwrap().troops = e.state.player(1).unwrap().max_troops;
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Active);
    e.state.player_mut(1).unwrap().troops = 0.0;
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.state.player_mut(1).unwrap().leader = Leader::of("puyi");
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.state.player_mut(1).unwrap().alliances.push(2);
    e.state.player_mut(2).unwrap().alliances.push(1);
    e.state.set_tile_owner(31, 30, 2);
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Active);
    e.state.player_mut(1).unwrap().leader = Leader::of("veleda");
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Conditional);
    e.add_fleet(WarpFleet::new(
        1,
        1,
        2,
        UnitType::Warship,
        1_000.0,
        (1, 1),
        vec![],
    ));
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Ready);
    e.state.config.buildings_enabled = false;
    assert_eq!(e.hero_ability_feedback(1).unwrap().state, Unavailable);
}
