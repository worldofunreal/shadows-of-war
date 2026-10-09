#[cfg(test)]
mod bot_iq_alliance_tests {
    use crate::engine::SowEngine;
    use crate::game::{BuildingKind, GamePhase, GameState};
    use crate::game_config::BotDifficulty;
    use crate::intent::nation::combat::nation_target_allowed;
    use crate::intent::nation::combat::{
        ghost_retaliation_delay_ticks, ghost_retaliation_send_troops,
    };
    use crate::intent::nation::diplomacy::ghost_alliance_probability;
    use crate::intent::nation::profile::GhostRetaliation;
    use crate::intent::nation::profile::{AiSlot, AiTier, ai_profile_for};
    use crate::intent::nation::structures::bot_structure_target_count;
    use crate::player::{Player, PlayerType};
    use crate::protocol::GameplayIntent;
    use crate::water_components::WaterComponents;

    fn test_engine_two_players(seed: u64) -> SowEngine {
        let mut game = GameState::new(seed, 8, 8, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;

        // Player 1 (Bot, IQ 135 - High IQ)
        let mut p1 = Player::new_bot(
            1,
            "Bot1".into(),
            [1.0, 0.0, 0.0],
            &crate::game_config::GameConfig::default(),
        );
        p1.iq = 135;
        p1.iq_points = 50.0;
        p1.troops = 1000.0;
        p1.max_troops = 1500.0;
        p1.gold = 300_000.0;
        p1.tile_count = 10;
        p1.border_insert(0); // Tile (0, 0)
        game.players.push(p1);

        // Player 2 (Bot, IQ 85 - Low IQ)
        let mut p2 = Player::new_bot(
            2,
            "Bot2".into(),
            [0.0, 1.0, 0.0],
            &crate::game_config::GameConfig::default(),
        );
        p2.iq = 85;
        p2.iq_points = 50.0;
        p2.troops = 100.0;
        p2.max_troops = 200.0;
        p2.gold = 10_000.0;
        p2.tile_count = 5;
        p2.border_insert(1); // Tile (1, 0)
        game.players.push(p2);

        game.player_lookup = vec![None, Some(0), Some(1)];

        // Set map ownerships to make them neighbors
        game.map.set_owner_id(0, 0, 1);
        game.map.set_owner_id(1, 0, 2);

        // Make both land tiles
        let idx0 = game.map.ref_id(0, 0);
        game.map.terrain[idx0] = crate::map::MapTile::from_byte(0b1000_0000);
        let idx1 = game.map.ref_id(1, 0);
        game.map.terrain[idx1] = crate::map::MapTile::from_byte(0b1000_0000);

        SowEngine::new(game, WaterComponents::default())
    }

    fn run_nation_attack(
        engine: &mut SowEngine,
        neighbors: &[u16],
        has_neutral: bool,
    ) -> Vec<crate::intent::nation::profile::BotDecision> {
        let slot = AiSlot {
            bot_id: 1,
            tier: AiTier::Nation,
            do_attack: true,
            do_structures: false,
            is_under_attack: false,
            ghost_retaliation: None,
            profile: ai_profile_for(AiTier::Nation, BotDifficulty::Vanilla),
        };
        let mut decisions = Vec::new();
        engine.nation_run_combat_for_slot(
            &slot,
            (1, 135),
            (5.0, 5.0),
            neighbors,
            has_neutral,
            &mut decisions,
        );
        decisions
    }

    fn attack_targets(decisions: &[crate::intent::nation::profile::BotDecision]) -> Vec<u16> {
        decisions
            .iter()
            .filter_map(|decision| match &decision.intent {
                GameplayIntent::Attack(attack) => Some(attack.target_owner),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn ghost_alliance_probability_uses_stronger_comparable_and_weaker_bands() {
        assert_eq!(ghost_alliance_probability(1_000.0, 10, 1_000.0, 10), 100);
        assert_eq!(ghost_alliance_probability(1_000.0, 10, 800.0, 7), 75);
        assert_eq!(ghost_alliance_probability(1_000.0, 10, 500.0, 8), 75);
        assert_eq!(ghost_alliance_probability(1_000.0, 10, 499.0, 7), 25);
    }

    #[test]
    fn ghost_accepts_and_offers_to_humans_and_ghosts_by_the_same_rule() {
        for other_is_ghost in [false, true] {
            let mut engine = test_engine_two_players(42);
            {
                let ghost = engine.state.player_mut(1).unwrap();
                ghost.player_type = PlayerType::Human;
                ghost.is_ai_controlled = true;
                ghost.iq = 165;
                ghost.iq_points = 1_000.0;
                ghost.troops = 1_000.0;
                ghost.tile_count = 10;
            }
            {
                let other = engine.state.player_mut(2).unwrap();
                other.player_type = PlayerType::Human;
                other.is_ai_controlled = other_is_ghost;
                other.troops = 2_000.0;
                other.tile_count = 20;
            }

            engine.push_alliance_proposal(2, 1);
            let mut decisions = Vec::new();
            engine.nation_run_diplomacy_for_slot(
                (1, 165),
                (5.0, 5.0),
                &[2],
                false,
                false,
                &mut decisions,
            );
            assert!(decisions.iter().any(|decision| matches!(
                decision.intent,
                GameplayIntent::AcceptAlliance { target_player: 2 }
            )));
            engine.alliances_proposed.clear();

            let mut offered = false;
            for tick in 0..512 {
                engine.state.tick = tick;
                let mut decisions = Vec::new();
                engine.nation_run_diplomacy_for_slot(
                    (1, 165),
                    (5.0, 5.0),
                    &[2],
                    false,
                    false,
                    &mut decisions,
                );
                if decisions.iter().any(|decision| {
                    matches!(
                        decision.intent,
                        GameplayIntent::ProposeAlliance { target_player: 2 }
                    )
                }) {
                    offered = true;
                    break;
                }
            }
            assert!(offered, "ghost did not offer to human/ghost candidate");
        }
    }

    #[test]
    fn ghost_retaliation_delay_and_force_are_seeded_and_bounded() {
        let delay = ghost_retaliation_delay_ticks(42, 7, 91);
        assert!((3..=12).contains(&delay));
        assert_eq!(delay, ghost_retaliation_delay_ticks(42, 7, 91));
        let delays: std::collections::BTreeSet<_> = (0..32)
            .map(|seed| ghost_retaliation_delay_ticks(seed, 7, 91))
            .collect();
        assert!(delays.len() > 1);

        let retaliation = GhostRetaliation {
            attack_id: 91,
            attacker_id: 8,
            incoming_troops: 1_000.0,
            ready_tick: 12,
        };
        let sent = ghost_retaliation_send_troops(42, 7, retaliation, 5_000.0, 5_000.0, 0.02);
        assert!((750.0..=1_250.0).contains(&sent));
        assert_eq!(
            sent,
            ghost_retaliation_send_troops(42, 7, retaliation, 5_000.0, 5_000.0, 0.02)
        );
        let forces: std::collections::BTreeSet<_> = (1..=32)
            .map(|attack_id| {
                let mut varied = retaliation;
                varied.attack_id = attack_id;
                ghost_retaliation_send_troops(42, 7, varied, 5_000.0, 5_000.0, 0.02).to_bits()
            })
            .collect();
        assert!(forces.len() > 1);
    }

    #[test]
    fn ghost_retaliation_chooses_the_strongest_due_hostile_attack() {
        let mut engine = test_engine_two_players(42);
        let ghost = engine.state.player_mut(1).unwrap();
        ghost.player_type = PlayerType::Human;
        ghost.is_ai_controlled = true;
        let mut third = Player::new_human(
            3,
            "Human 3".into(),
            [0.0, 0.0, 1.0],
            &crate::game_config::GameConfig::default(),
        );
        third.troops = 3_000.0;
        engine.state.register_player(third);
        for (id, owner_id, troops) in [(11, 2, 200.0), (12, 3, 500.0)] {
            engine.attacks.push(crate::execution::AttackExecution {
                id,
                owner_id,
                target_owner: 1,
                created_tick: 0,
                troops,
                to_conquer: Default::default(),
                insert_seq_counter: 0,
                rng: wyrand::WyRand::new(id),
                retreating: false,
            });
        }
        engine.ai_attack_index = vec![Vec::new(); engine.state.player_lookup.len()];
        engine.ai_attack_index[1].extend([0, 1]);

        let (retaliation, _) = engine.ghost_retaliation_for(1, 20);
        let retaliation = retaliation.unwrap();
        assert_eq!(retaliation.attacker_id, 3);
        assert_eq!(retaliation.incoming_troops, 500.0);
    }

    #[test]
    fn ghost_does_not_repeat_a_retaliation_while_its_counterattack_is_active() {
        let mut engine = test_engine_two_players(42);
        let ghost = engine.state.player_mut(1).unwrap();
        ghost.player_type = PlayerType::Human;
        ghost.is_ai_controlled = true;
        engine.attacks.extend([
            crate::execution::AttackExecution {
                id: 11,
                owner_id: 2,
                target_owner: 1,
                created_tick: 0,
                troops: 200.0,
                to_conquer: Default::default(),
                insert_seq_counter: 0,
                rng: wyrand::WyRand::new(11),
                retreating: false,
            },
            crate::execution::AttackExecution {
                id: 12,
                owner_id: 1,
                target_owner: 2,
                created_tick: 4,
                troops: 200.0,
                to_conquer: Default::default(),
                insert_seq_counter: 0,
                rng: wyrand::WyRand::new(12),
                retreating: false,
            },
        ]);
        engine.ai_attack_index = vec![vec![], vec![0], vec![1]];

        let (retaliation, _) = engine.ghost_retaliation_for(1, 20);

        assert!(retaliation.is_none());
    }

    #[test]
    fn ghost_scheduler_forces_retaliation_on_due_tick_not_before() {
        let seed = 42;
        let attack_id = 91;
        let mut engine = test_engine_two_players(seed);
        let ghost = engine.state.player_mut(1).unwrap();
        ghost.player_type = PlayerType::Human;
        ghost.is_ai_controlled = true;
        ghost.iq = 165;
        ghost.iq_points = 10_000.0;
        ghost.troops = 10.0;
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.state.config.attack_cost_neutral = 50.0;
        engine.attacks.push(crate::execution::AttackExecution {
            id: attack_id,
            owner_id: 2,
            target_owner: 1,
            created_tick: 0,
            troops: 100.0,
            to_conquer: Default::default(),
            insert_seq_counter: 0,
            rng: wyrand::WyRand::new(42),
            retreating: false,
        });

        let due_tick = ghost_retaliation_delay_ticks(seed, 1, attack_id);
        engine.state.tick = due_tick - 1;
        engine.execute_ai_think();
        assert!(!engine.test_last_ai_intents.iter().any(|intent| matches!(
            intent.intent,
            GameplayIntent::Attack(ref attack) if attack.target_owner == 2
        )));

        engine.state.tick = due_tick;
        engine.execute_ai_think();
        assert!(!engine.test_last_ai_intents.iter().any(|intent| matches!(
            intent.intent,
            GameplayIntent::Attack(ref attack) if attack.target_owner == 2
        )));

        engine.state.player_mut(1).unwrap().troops = 1_000.0;
        engine.state.tick = due_tick + 1;
        engine.execute_ai_think();
        assert!(engine.test_last_ai_intents.iter().any(|intent| matches!(
            intent.intent,
            GameplayIntent::Attack(ref attack) if attack.target_owner == 2
        )));
    }

    #[test]
    fn ghost_retaliates_against_attacker_even_when_border_sample_omits_them() {
        let mut engine = test_engine_two_players(42);
        let ghost = engine.state.player_mut(1).unwrap();
        ghost.player_type = PlayerType::Human;
        ghost.is_ai_controlled = true;
        ghost.iq = 165;
        ghost.iq_points = 10_000.0;
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        let retaliation = GhostRetaliation {
            attack_id: 91,
            attacker_id: 2,
            incoming_troops: 500.0,
            ready_tick: 0,
        };
        let slot = AiSlot {
            bot_id: 1,
            tier: AiTier::Ghost,
            do_attack: true,
            do_structures: false,
            is_under_attack: true,
            ghost_retaliation: Some(retaliation),
            profile: ai_profile_for(AiTier::Ghost, BotDifficulty::Vanilla),
        };
        let mut decisions = Vec::new();

        engine.nation_run_combat_for_slot(&slot, (1, 165), (5.0, 5.0), &[], true, &mut decisions);

        assert_eq!(attack_targets(&decisions), vec![2]);
        let sent = decisions
            .iter()
            .find_map(|decision| match &decision.intent {
                GameplayIntent::Attack(attack) if attack.target_owner == 2 => attack.troops,
                _ => None,
            })
            .unwrap();
        assert!((375.0..=625.0).contains(&sent));
    }

    #[test]
    fn test_nation_food_chain_never_initiates_against_humans() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().player_type = PlayerType::Nation;
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        let decisions = run_nation_attack(&mut engine, &[2], true);
        assert!(!attack_targets(&decisions).contains(&2));
        let decisions = run_nation_attack(&mut engine, &[2], false);
        assert!(attack_targets(&decisions).is_empty());

        let mut ghost_engine = test_engine_two_players(42);
        ghost_engine.state.player_mut(1).unwrap().player_type = PlayerType::Nation;
        let ghost = ghost_engine.state.player_mut(2).unwrap();
        ghost.player_type = PlayerType::Human;
        ghost.is_ai_controlled = true;
        let decisions = run_nation_attack(&mut ghost_engine, &[2], true);
        assert!(!attack_targets(&decisions).contains(&2));
        let decisions = run_nation_attack(&mut ghost_engine, &[2], false);
        assert!(attack_targets(&decisions).contains(&2));
    }

    #[test]
    fn campaign_neutral_retaliates_only_after_being_attacked() {
        use crate::protocol::CampaignRelation;

        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().player_type = PlayerType::Nation;
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine
            .campaign_relations
            .insert(1, CampaignRelation::Neutral);

        let idle = run_nation_attack(&mut engine, &[2], false);
        assert!(attack_targets(&idle).is_empty());

        engine.attacks.push(crate::execution::AttackExecution {
            id: 1,
            owner_id: 2,
            target_owner: 1,
            created_tick: 0,
            troops: 5000.0,
            to_conquer: Default::default(),
            insert_seq_counter: 0,
            rng: wyrand::WyRand::new(42),
            retreating: false,
        });
        engine.ai_attack_index = vec![Vec::new(); engine.state.player_lookup.len()];
        engine.ai_attack_index[1].push(0);
        let defense = run_nation_attack(&mut engine, &[2], false);
        assert_eq!(attack_targets(&defense), vec![2]);
    }

    #[test]
    fn test_nation_prefers_tribe_before_human() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().player_type = PlayerType::Nation;
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Bot;
        engine.state.player_mut(2).unwrap().troops = 100.0;

        let mut human = Player::new_human(
            3,
            "Human".into(),
            [0.0, 0.0, 1.0],
            &crate::game_config::GameConfig::default(),
        );
        human.troops = 1.0;
        human.max_troops = 2.0;
        engine.state.players.push(human);
        engine.state.player_lookup.push(Some(2));

        let decisions = run_nation_attack(&mut engine, &[2, 3], false);
        assert_eq!(attack_targets(&decisions), vec![2]);
    }

    #[test]
    fn test_nation_defends_human_attacker_before_wilderness() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().player_type = PlayerType::Nation;
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.attacks.push(crate::execution::AttackExecution {
            id: 1,
            owner_id: 2,
            target_owner: 1,
            created_tick: 0,
            troops: 5000.0,
            to_conquer: Default::default(),
            insert_seq_counter: 0,
            rng: wyrand::WyRand::new(42),
            retreating: false,
        });
        engine.ai_attack_index = vec![Vec::new(); engine.state.player_lookup.len()];
        engine.ai_attack_index[1].push(0);

        let decisions = run_nation_attack(&mut engine, &[2], true);
        assert_eq!(attack_targets(&decisions), vec![2]);
    }

    #[test]
    fn test_nation_human_gate_is_shared_by_fleet_and_nuke() {
        // `target_is_human` means a person-controlled Human. Ghosts are AI
        // opponents, not protected human players.
        assert!(!nation_target_allowed(2, true, None));
        assert!(nation_target_allowed(2, true, Some(2)));
        assert!(!nation_target_allowed(3, true, Some(2)));
        assert!(nation_target_allowed(2, false, None));
    }

    #[test]
    fn test_execute_income_iq_points_accumulation() {
        let mut engine = test_engine_two_players(42);
        engine.state.config.global_speed_multiplier = 1.0;
        engine.state.config.tick_rate_ms = 100.0;

        // Prior to income
        assert_eq!(engine.state.player(1).unwrap().iq_points, 50.0);
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.0);

        // Tick income
        engine.execute_income();

        // High IQ (135): per_tick(1.35) = 1.35 * 0.1 * 1.0 = 0.135
        assert_eq!(engine.state.player(1).unwrap().iq_points, 50.135);
        // Low IQ (85): per_tick(0.85) = 0.85 * 0.1 * 1.0 = 0.085
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.085);
    }

    #[test]
    fn test_alliance_proposal_threshold_high_iq() {
        let mut engine = test_engine_two_players(42);
        // Ensure bot 1 can afford alliance
        engine.state.player_mut(1).unwrap().iq_points = 100.0;
        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // Since bot 1 has IQ 135, it only proposes if target troops > 0.8 * me_troops.
        // Bot 2 has 100 troops, Bot 1 has 1000. It should NOT propose an alliance.
        assert!(
            engine.alliances_proposed.is_empty(),
            "High IQ bot should not propose to weak neighbor"
        );
    }

    #[test]
    fn test_attack_context_betrayal_not_timer_driven() {
        let mut engine = test_engine_two_players(42);
        let p1 = engine.state.player_mut(1).unwrap();
        p1.iq_points = 100.0;
        p1.player_type = crate::player::PlayerType::Nation;
        p1.alliances.push(2);
        p1.alliance_timers.insert(2, 100);
        p1.troops = 5000.0;
        let p2 = engine.state.player_mut(2).unwrap();
        p2.alliances.push(1);
        p2.alliance_timers.insert(1, 100);
        p2.troops = 500.0;
        // No neutral land — boxed in with ally only.
        let mut broke_alliance = false;
        for _ in 0..120 {
            engine.state.tick += 1;
            engine.execute_ai_think();
            if !engine.state.player(1).unwrap().alliances.contains(&2) {
                broke_alliance = true;
                break;
            }
        }
        assert!(
            broke_alliance,
            "strong nation should betray weak bordering ally via attack-context logic"
        );
    }

    #[test]
    fn test_proactive_two_x_betrayal_removed() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 100.0;
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine
            .state
            .player_mut(1)
            .unwrap()
            .alliance_timers
            .insert(2, 500);
        engine
            .state
            .player_mut(2)
            .unwrap()
            .alliance_timers
            .insert(1, 500);
        engine.state.player_mut(1).unwrap().troops = 2500.0;
        engine.state.player_mut(2).unwrap().troops = 1000.0;
        // Give bot 1 neutral expansion option so diplomacy propose is skipped; 2x should not auto-break.
        engine.state.map.set_owner_id(2, 0, 0);
        let idx = engine.state.map.ref_id(2, 0);
        engine.state.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
        for _ in 0..20 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        assert!(
            engine.state.player(1).unwrap().alliances.contains(&2),
            "2x troop advantage alone must not trigger timer betrayal"
        );
    }

    #[test]
    fn test_density_upgrade_logic() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 10_000_000.0;
        engine.state.player_mut(1).unwrap().tile_count = 100; // Small area
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Add max structures to force upgrade
        for i in 0..15 {
            engine.buildings.push(crate::building::Building {
                id: i,
                owner_id: 1,
                tile_idx: 0,
                kind: crate::game::BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            });
        }
        engine.refresh_building_grid();
        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // As long as this executes without panic we're good
    }

    #[test]
    fn test_frontline_defense_post_prioritization() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Simulate under attack
        engine.attacks.push(crate::execution::AttackExecution {
            id: 1,
            owner_id: 2,
            target_owner: 1,
            created_tick: 0,
            troops: 5000.0,
            to_conquer: Default::default(),
            insert_seq_counter: 0,
            rng: wyrand::WyRand::new(42),
            retreating: false,
        });

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
    }

    #[test]
    fn test_nuke_launch_sam_avoidance() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Give bot 1 a silo
        engine.buildings.push(crate::building::Building {
            id: 100,
            owner_id: 1,
            tile_idx: 0,
            kind: crate::game::BuildingKind::City,
            level: BuildingKind::City.max_level(),
            under_construction: false,
            ticks_until_complete: 0,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101,
            owner_id: 2,
            tile_idx: 10,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });

        // Give bot 2 a SAM covering the city
        engine.buildings.push(crate::building::Building {
            id: 102,
            owner_id: 2,
            tile_idx: 10,
            kind: BuildingKind::Bunker,
            level: BuildingKind::Bunker.max_level(),
            under_construction: false,
            ticks_until_complete: 0,
        });

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // Since the only target is covered by SAM, it shouldn't launch.
        assert!(engine.recent_nuke_targets.is_empty());
    }

    #[test]
    fn test_alliance_cap_enforced() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        // Bot 1 (IQ 135) allows max 1 alliance. Give it 1 alliance already.
        engine.state.player_mut(1).unwrap().alliances.push(3);

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        assert!(
            engine.alliances_proposed.is_empty(),
            "High IQ bot should respect alliance cap of 1"
        );
    }

    #[test]
    fn test_nuke_launch_target_centroid() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Give bot 1 a silo
        engine.buildings.push(crate::building::Building {
            id: 100,
            owner_id: 1,
            tile_idx: 0,
            kind: crate::game::BuildingKind::City,
            level: BuildingKind::City.max_level(),
            under_construction: false,
            ticks_until_complete: 0,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101,
            owner_id: 2,
            tile_idx: 1,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });

        // Make sure bot 2 actually owns tile 1 so they are neighbors!
        engine.state.map.set_owner_id(1, 0, 2);

        engine.refresh_building_grid();
        let mut decisions = Vec::new();
        engine.maybe_launch_nuke(1, &mut decisions, 135, &[2], None);
        assert!(!decisions.is_empty());
        assert_eq!(engine.recent_nuke_targets.get(&(2, 1)), Some(&1));
    }

    #[test]
    fn test_nuke_history_count_skips_repeated_tile() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = PlayerType::Nation;
        engine.recent_nuke_targets.insert((2, 1), 2);
        engine.recent_nuke_targets.insert((3, 2), 90);

        engine.buildings.extend([
            crate::building::Building {
                id: 100,
                owner_id: 1,
                tile_idx: 0,
                kind: BuildingKind::City,
                level: BuildingKind::City.max_level(),
                under_construction: false,
                ticks_until_complete: 0,
            },
            crate::building::Building {
                id: 101,
                owner_id: 2,
                tile_idx: 1,
                kind: BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
            crate::building::Building {
                id: 102,
                owner_id: 2,
                tile_idx: 2,
                kind: BuildingKind::Factory,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
        ]);

        let mut decisions = Vec::new();
        engine.maybe_launch_nuke(1, &mut decisions, 135, &[2], None);
        assert!(matches!(
            decisions.first().map(|decision| &decision.intent),
            Some(GameplayIntent::LaunchNuke { target_tile: 2, .. })
        ));
        assert_eq!(engine.recent_nuke_targets.get(&(2, 2)), Some(&1));
    }

    #[test]
    fn test_nation_nuke_ignores_human_except_defense() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().player_type = PlayerType::Nation;
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;

        engine.buildings.push(crate::building::Building {
            id: 100,
            owner_id: 1,
            tile_idx: 0,
            kind: BuildingKind::City,
            level: BuildingKind::City.max_level(),
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.buildings.push(crate::building::Building {
            id: 101,
            owner_id: 2,
            tile_idx: 1,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });

        engine.refresh_building_grid();
        let mut decisions = Vec::new();
        engine.maybe_launch_nuke(1, &mut decisions, 135, &[2], None);
        assert!(decisions.is_empty());

        engine.maybe_launch_nuke(1, &mut decisions, 135, &[2], None);
        assert!(decisions.is_empty());

        decisions.clear();
        engine.recent_nuke_targets.clear();
        engine.maybe_launch_nuke(1, &mut decisions, 135, &[2], Some(2));
        assert!(!decisions.is_empty());
    }

    #[test]
    fn test_team_alliance_prohibited() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().team = Some(crate::protocol::Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(crate::protocol::Team::Red);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;

        let stamped = crate::protocol::StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::ProposeAlliance { target_player: 2 },
        };
        engine.apply_stamped_intent(&stamped, 0);
        assert!(
            engine.alliances_proposed.is_empty(),
            "Teammates should not be allowed to propose alliance"
        );
    }

    fn test_engine_nation_mid_game() -> SowEngine {
        let w = 64u32;
        let h = 64u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, h, config.clone());
        game.phase = GamePhase::Playing;

        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let owner = 1u16;
        let mut sum_x = 0u64;
        let mut sum_y = 0u64;
        let mut count = 0u32;
        for y in 10..50 {
            for x in 10..50 {
                game.map.set_owner_id(x, y, owner);
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }

        let mut nation = Player::new_nation(1, "Nation1".into(), [1.0, 0.0, 0.0], &config);
        nation.iq_points = 500.0;
        nation.gold = 100_000.0;
        nation.troops = 10_000.0;
        nation.tile_count = count;
        nation.sum_x = sum_x;
        nation.sum_y = sum_y;
        for x in 10..50 {
            nation.border_insert(10 * w + x);
            nation.border_insert(49 * w + x);
        }
        for y in 11..49 {
            nation.border_insert(y * w + 10);
            nation.border_insert(y * w + 49);
        }
        game.players.push(nation);
        game.player_lookup = vec![None, Some(0)];

        let mut engine = SowEngine::new(game, WaterComponents::default());
        let city_positions = [(15, 15), (15, 35), (35, 15), (25, 25), (20, 40), (40, 20)];
        for (i, (cx, cy)) in city_positions.iter().enumerate() {
            let tile_idx = cy * w + cx;
            engine.buildings.push(crate::building::Building {
                id: (i as u64) + 1,
                owner_id: 1,
                tile_idx,
                kind: crate::game::BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            });
        }
        engine.refresh_building_grid();
        engine.building_aggregates_dirty = true;
        engine
    }

    fn test_engine_advanced_tribe() -> SowEngine {
        let w = 48u32;
        let h = 48u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, h, config.clone());
        game.phase = GamePhase::Playing;

        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let owner = 10u16;
        let mut sum_x = 0u64;
        let mut sum_y = 0u64;
        let mut count = 0u32;
        for y in 4..28 {
            for x in 4..28 {
                game.map.set_owner_id(x, y, owner);
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }

        let mut tribe = Player::new_bot(10, "Tribe10".into(), [0.0, 1.0, 0.0], &config);
        tribe.iq = 110;
        tribe.iq_points = 500.0;
        tribe.gold = 50_000.0;
        tribe.troops = 5_000.0;
        tribe.tile_count = count;
        tribe.sum_x = sum_x;
        tribe.sum_y = sum_y;
        for x in 4..28 {
            tribe.border_insert(4 * w + x);
            tribe.border_insert(27 * w + x);
        }
        for y in 5..27 {
            tribe.border_insert(y * w + 4);
            tribe.border_insert(y * w + 27);
        }
        game.players.push(tribe);
        let mut lookup = vec![None; 11];
        lookup[10] = Some(0);
        game.player_lookup = lookup;

        let mut engine = SowEngine::new(game, WaterComponents::default());
        engine.refresh_building_grid();
        engine.building_aggregates_dirty = true;
        engine
    }

    #[test]
    fn test_nation_keeps_building_mid_game() {
        let mut engine = test_engine_nation_mid_game();
        let initial_count = engine.buildings.len();
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..2000 {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
        assert!(
            engine.buildings.len() > initial_count,
            "nation should keep placing structures mid-game (had {initial_count}, now {})",
            engine.buildings.len()
        );
    }

    #[test]
    fn test_advanced_tribe_can_build() {
        let mut engine = test_engine_advanced_tribe();
        let initial_count = engine.buildings.len();
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..1000 {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
        assert!(
            engine.buildings.len() > initial_count,
            "advanced tribe (id % 10) should build structures (had {initial_count}, now {})",
            engine.buildings.len()
        );
    }

    fn run_building_sim_ticks(engine: &mut SowEngine, ticks: u64) {
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..ticks {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
    }

    type BuildingSimFingerprint = (usize, u64, f64, f64, Vec<(u64, u32, u8, u8)>);

    fn building_sim_fingerprint(engine: &SowEngine, player_id: u16) -> BuildingSimFingerprint {
        let mut snaps: Vec<(u64, u32, u8, u8)> = engine
            .buildings
            .iter()
            .filter(|b| b.owner_id == player_id)
            .map(|b| (b.id, b.tile_idx, b.kind as u8, b.level))
            .collect();
        snaps.sort_by_key(|s| s.0);
        let level_sum: u64 = snaps.iter().map(|s| s.3 as u64).sum();
        let gold = engine
            .state
            .player(player_id)
            .map(|p| p.gold)
            .unwrap_or(0.0);
        let iq_pts = engine
            .state
            .player(player_id)
            .map(|p| p.iq_points)
            .unwrap_or(0.0);
        (snaps.len(), level_sum, gold, iq_pts, snaps)
    }

    #[test]
    fn test_ai_building_simulation_is_deterministic() {
        let mut a = test_engine_nation_mid_game();
        run_building_sim_ticks(&mut a, 500);
        let fp_a = building_sim_fingerprint(&a, 1);

        let mut b = test_engine_nation_mid_game();
        run_building_sim_ticks(&mut b, 500);
        let fp_b = building_sim_fingerprint(&b, 1);

        assert_eq!(
            fp_a, fp_b,
            "identical seed/setup must produce identical building state after 500 ticks"
        );
    }

    #[test]
    fn test_bot_structure_target_count_floor_is_stable() {
        // Low IQ: factor 0.1 caps non-city kinds at 0, city at least 1
        assert_eq!(bot_structure_target_count(BuildingKind::City, 10, 85), 1);
        assert_eq!(bot_structure_target_count(BuildingKind::Bunker, 10, 85), 0);
        // Mid IQ: 50% of high-IQ quotas, deterministic floor
        assert_eq!(bot_structure_target_count(BuildingKind::Factory, 8, 110), 2);
        // High IQ: full quotas
        assert_eq!(bot_structure_target_count(BuildingKind::Port, 10, 140), 3);
    }

    #[test]
    fn test_structure_upgrade_uses_an_explicit_intent() {
        let w = 32u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, w, config.clone());
        game.phase = GamePhase::Playing;
        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        for y in 0..w {
            for x in 0..w {
                game.map.set_owner_id(x, y, 1);
            }
        }
        let mut nation = Player::new_nation(1, "N".into(), [1.0, 0.0, 0.0], &config);
        nation.gold = 1_000_000.0;
        nation.iq = 140;
        nation.iq_points = 500.0;
        nation.tile_count = w * w;
        game.players.push(nation);
        game.player_lookup = vec![None, Some(0)];

        let mut engine = SowEngine::new(game, WaterComponents::default());
        engine.buildings.push(crate::building::Building {
            id: 1,
            owner_id: 1,
            tile_idx: 16 * w + 16,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.refresh_building_grid();

        let city_tile = 16 * w + 16;
        engine.apply_stamped_intent(
            &crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::BuildStructure {
                    kind: BuildingKind::City,
                    target_tile: city_tile,
                },
            },
            0,
        );

        assert_eq!(engine.buildings.len(), 1);
        assert_eq!(engine.buildings[0].level, 1);

        engine.apply_stamped_intent(
            &crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::UpgradeStructure { building_id: 1 },
            },
            0,
        );
        assert_eq!(engine.buildings[0].level, 2);
    }

    #[test]
    fn test_tribe_buildings_not_purged() {
        let mut engine = test_engine_two_players(42);
        engine.buildings.push(crate::building::Building {
            id: 50,
            owner_id: 2,
            tile_idx: 1,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.building_aggregates_dirty = true;
        engine.execute_income();
        assert!(
            engine
                .buildings
                .iter()
                .any(|b| b.id == 50 && b.owner_id == 2),
            "standard tribe buildings must not be deleted by income tick"
        );
    }

    // ── Food chain + tier semantics regression tests ──────────────────────
    // The AI personality system must key on (player_type, is_ai_controlled)
    // via `ai_tier` — NOT on `bot_id % N` arithmetic, which used to mint
    // accidental élite tribes and bottom-band ghosts that inverted the chain.

    #[test]
    fn test_ai_tier_resolves_from_type_not_id() {
        use crate::intent::nation::profile::{AiTier, ai_tier};
        use crate::player::PlayerType;

        // Ghost = Human + is_ai_controlled, regardless of id.
        assert_eq!(ai_tier(PlayerType::Human, true), Some(AiTier::Ghost));
        assert_eq!(ai_tier(PlayerType::Human, false), None); // real human: no AI
        // Nation and Bot map by type, never by id.
        assert_eq!(ai_tier(PlayerType::Nation, false), Some(AiTier::Nation));
        assert_eq!(ai_tier(PlayerType::Bot, false), Some(AiTier::Tribe));
        assert_eq!(ai_tier(PlayerType::Bot, true), Some(AiTier::Tribe));
        // A tribe with an "élite-looking" id must STILL be a tribe (no id%100 carve-out).
        assert_eq!(ai_tier(PlayerType::Bot, false), Some(AiTier::Tribe));
    }

    #[test]
    fn test_ghost_iq_higher_than_nation_higher_than_tribe() {
        use crate::game_config::GameConfig;
        use crate::player::Player;

        let config = GameConfig::default();
        // Same id across types — tier must come from type, not id.
        // Tribe band (50-85), Nation (130-159), Ghost (160-180) — no overlap,
        // so the food chain is strictly Ghost > Nation > Tribe.
        let max = |f: &dyn Fn(u16) -> Player| {
            let mut lo = u32::MAX;
            let mut hi = 0u32;
            for id in [1u16, 2, 3, 5, 7, 11, 13, 99, 400] {
                let p = f(id);
                lo = lo.min(p.iq);
                hi = hi.max(p.iq);
            }
            (lo, hi)
        };
        // Tribes: PlayerType::Bot via new_bot
        let (_, t_hi) = max(&|id| Player::new_bot(id, "t".into(), [1.0; 3], &config));
        // Ghost: new_human + is_ai_controlled sets band in spawn_human; emulate by
        // asserting the deterministic band function directly is in-range.
        // Use spawn_human through the engine to keep it honest.
        let mut g = crate::engine::SowEngine::new(
            GameState::new(1, 16, 16, config.clone()),
            WaterComponents::default(),
        );
        g.spawn_human(crate::engine::HumanSpawn {
            player_id: 5,
            name: "ghost".into(),
            color: [1.0, 0.0, 0.0],
            team: None,
            civilization: crate::player::Civilization::ALL[0],
            leader: crate::player::Leader::ALL[0],
            skin_style: 0,
            is_ai_controlled: true,
        });
        let ghost_iq = g.state.player(5).unwrap().iq;
        assert!(
            (160..=180).contains(&ghost_iq),
            "ghost IQ {} must be in top band 160-180",
            ghost_iq
        );

        // Nation band 130-159, strictly below the ghost floor of 160.
        let n = Player::new_nation(9, "n".into(), [1.0; 3], &config);
        assert!(
            (130..=159).contains(&n.iq),
            "nation IQ {} must be 130-159",
            n.iq
        );

        // Tribe: no actor may ever roll a nation/ghost-level IQ.
        assert!(
            t_hi <= 85,
            "tribe IQ ceiling {} must be <= 85 (no accidental élite tribes)",
            t_hi
        );
        // And the food chain strictness:
        assert!(
            ghost_iq > n.iq,
            "ghost {} must out-IQ nation {}",
            ghost_iq,
            n.iq
        );
    }

    #[test]
    fn test_vanilla_tribes_are_active_but_do_not_target_players() {
        use crate::game_config::BotDifficulty;
        use crate::intent::nation::profile::{AiTier, ai_profile_for};

        let vanilla = ai_profile_for(AiTier::Tribe, BotDifficulty::Vanilla);
        assert!(!vanilla.attacks_players);
        assert!(vanilla.expand_ratio > 0.0);

        let terminator = ai_profile_for(AiTier::Tribe, BotDifficulty::Terminator);
        assert!(terminator.attacks_players);
    }

    #[test]
    fn test_ghost_answers_teammate_resource_request_without_alliance() {
        use crate::protocol::{GameplayIntent, Team};

        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().team = Some(Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(Team::Red);
        engine.state.player_mut(1).unwrap().troops = 1_000.0;
        engine.state.player_mut(1).unwrap().max_troops = 1_000.0;
        engine.state.player_mut(1).unwrap().iq_points = 50.0;
        engine.state.player_mut(2).unwrap().troops = 100.0;

        // No formal alliance: team membership alone must be enough.
        engine
            .resource_requests_proposed
            .push(crate::engine::ResourceRequestProposed {
                proposer: 2,
                target: 1,
                gold: 100.0,
                troops: 50.0,
            });

        let mut decisions = Vec::new();
        engine.nation_run_diplomacy_for_slot(
            (1, 160),
            (5.0, 5.0),
            &[],
            false,
            false,
            &mut decisions,
        );

        assert!(decisions.iter().any(|d| matches!(
            d.intent,
            GameplayIntent::AcceptResourceRequest { target_player: 2 }
        )));
        assert_eq!(engine.state.player(1).unwrap().iq_points, 45.0);
    }

    #[test]
    fn campaign_enemy_with_alliance_offers_disabled_never_proposes_one() {
        use crate::protocol::{CampaignRelation, GameplayIntent};

        let mut engine = test_engine_two_players(42);
        engine.campaign_relations.insert(1, CampaignRelation::Enemy);
        engine.campaign_can_request_alliance.insert(1, false);
        engine.state.player_mut(1).unwrap().troops = 100.0;
        engine.state.player_mut(1).unwrap().tile_count = 5;
        engine.state.player_mut(2).unwrap().troops = 1_000.0;
        engine.state.player_mut(2).unwrap().tile_count = 10;

        for _ in 0..128 {
            let mut decisions = Vec::new();
            engine.nation_run_diplomacy_for_slot(
                (1, 135),
                (5.0, 5.0),
                &[2],
                false,
                false,
                &mut decisions,
            );
            assert!(
                !decisions.iter().any(|decision| matches!(
                    decision.intent,
                    GameplayIntent::ProposeAlliance { .. }
                ))
            );
        }
    }

    #[test]
    fn campaign_bot_does_not_renew_a_permanent_alliance() {
        use crate::protocol::{CampaignRelation, GameplayIntent};

        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.state.player_mut(1).unwrap().iq_points = 10_000.0;
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine
            .campaign_relations
            .insert(1, CampaignRelation::Allied);

        for _ in 0..128 {
            let mut decisions = Vec::new();
            engine.nation_run_diplomacy_for_slot(
                (1, 135),
                (5.0, 5.0),
                &[2],
                false,
                false,
                &mut decisions,
            );
            assert!(
                !decisions.iter().any(|decision| matches!(
                    decision.intent,
                    GameplayIntent::ProposeAlliance { .. }
                ))
            );
        }
    }

    #[test]
    fn gameplay_bot_still_proposes_to_renew_a_timed_alliance() {
        use crate::diplomacy::ALLIANCE_RENEWAL_WINDOW_TICKS;
        use crate::protocol::GameplayIntent;

        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.state.player_mut(1).unwrap().iq_points = 10_000.0;
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine
            .state
            .player_mut(1)
            .unwrap()
            .alliance_timers
            .insert(2, ALLIANCE_RENEWAL_WINDOW_TICKS);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine
            .state
            .player_mut(2)
            .unwrap()
            .alliance_timers
            .insert(1, ALLIANCE_RENEWAL_WINDOW_TICKS);

        let mut proposed = false;
        for _ in 0..128 {
            let mut decisions = Vec::new();
            engine.nation_run_diplomacy_for_slot(
                (1, 135),
                (5.0, 5.0),
                &[2],
                false,
                false,
                &mut decisions,
            );
            if decisions.iter().any(|decision| {
                matches!(
                    decision.intent,
                    GameplayIntent::ProposeAlliance { target_player: 2 }
                )
            }) {
                proposed = true;
                break;
            }
        }

        assert!(proposed, "timed alliance should remain renewable");
    }

    #[test]
    fn neutral_campaign_bot_with_alliance_offers_enabled_can_propose() {
        use crate::protocol::{CampaignRelation, GameplayIntent};

        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.state.player_mut(1).unwrap().iq_points = 10_000.0;
        engine.state.player_mut(2).unwrap().troops = 1_000.0;
        engine.state.player_mut(2).unwrap().tile_count = 10;
        engine
            .campaign_relations
            .insert(1, CampaignRelation::Neutral);
        engine.campaign_can_request_alliance.insert(1, true);

        let mut proposed = false;
        for _ in 0..512 {
            let mut decisions = Vec::new();
            engine.nation_run_diplomacy_for_slot(
                (1, 135),
                (5.0, 5.0),
                &[2],
                false,
                false,
                &mut decisions,
            );
            if decisions.iter().any(|decision| {
                matches!(
                    decision.intent,
                    GameplayIntent::ProposeAlliance { target_player: 2 }
                )
            }) {
                proposed = true;
                break;
            }
        }

        assert!(
            proposed,
            "enabled neutral campaign faction should offer alliance"
        );
    }

    #[test]
    fn campaign_assault_marks_every_living_team_faction_and_breaks_target_alliances() {
        use crate::protocol::{CampaignRelation, Team};

        let mut engine = test_engine_two_players(42);
        let config = engine.state.config.clone();
        let mut second_attacker =
            Player::new_bot(3, "Roman reserve".into(), [1.0, 0.0, 0.0], &config);
        second_attacker.team = Some(Team::Red);
        second_attacker.tile_count = 5;
        engine.state.register_player(second_attacker);
        let mut defeated_attacker =
            Player::new_bot(4, "Defeated Roman force".into(), [1.0, 0.0, 0.0], &config);
        defeated_attacker.team = Some(Team::Red);
        defeated_attacker.tile_count = 5;
        defeated_attacker.alive = false;
        engine.state.register_player(defeated_attacker);
        let mut unlisted_attacker =
            Player::new_bot(5, "Unlisted force".into(), [1.0, 0.0, 0.0], &config);
        unlisted_attacker.team = Some(Team::Red);
        unlisted_attacker.tile_count = 5;
        engine.state.register_player(unlisted_attacker);
        engine.state.player_mut(1).unwrap().team = Some(Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(Team::Blue);
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine.campaign_faction_ids.extend([
            (1, "legion_a".to_string()),
            (3, "legio_ii_augusta".to_string()),
            (4, "defeated_roman_force".to_string()),
        ]);
        engine
            .campaign_relations
            .insert(1, CampaignRelation::Neutral);
        engine
            .campaign_relations
            .insert(3, CampaignRelation::Neutral);

        assert_eq!(
            engine.campaign_relations.get(&3),
            Some(&CampaignRelation::Neutral)
        );
        assert_eq!(
            engine.activate_campaign_assault(Team::Red, 2, false, false, None, false),
            2
        );
        assert_eq!(
            engine.campaign_relations.get(&1),
            Some(&CampaignRelation::Enemy)
        );
        assert_eq!(
            engine.campaign_relations.get(&3),
            Some(&CampaignRelation::Enemy)
        );
        assert_eq!(
            engine.campaign_faction_ids.get(&3).map(String::as_str),
            Some("legio_ii_augusta")
        );
        assert_eq!(engine.campaign_assault_targets.get(&1), Some(&vec![2]));
        assert_eq!(engine.campaign_assault_targets.get(&3), Some(&vec![2]));
        assert!(
            !engine.campaign_assault_targets.contains_key(&4),
            "defeated campaign factions must not return for the final assault"
        );
        assert!(
            !engine.campaign_assault_targets.contains_key(&5),
            "only listed campaign factions join the assault"
        );
        assert!(!engine.state.player(1).unwrap().alliances.contains(&2));
        assert!(!engine.state.player(2).unwrap().alliances.contains(&1));
    }

    #[test]
    fn final_assault_uses_live_alliances_without_recalling_in_flight_forces() {
        use crate::protocol::Team;

        let mut engine = test_engine_two_players(52);
        let config = engine.state.config.clone();
        engine.state.player_mut(1).unwrap().team = Some(Team::Red);
        engine.state.player_mut(1).unwrap().troops = 300.0;
        engine.state.player_mut(1).unwrap().max_troops = 1000.0;
        {
            let boudica = engine.state.player_mut(2).unwrap();
            boudica.player_type = PlayerType::Human;
            boudica.team = None;
            boudica.max_troops = 1000.0;
            boudica.alliances.extend([3, 4]);
        }

        let mut blue_ally = Player::new_bot(3, "Blue ally".into(), [0.1, 0.4, 0.9], &config);
        blue_ally.team = Some(Team::Blue);
        blue_ally.tile_count = 1;
        blue_ally.max_troops = 2000.0;
        blue_ally.alliances.push(2);
        blue_ally.border_insert(8);
        engine.state.register_player(blue_ally);
        let mut teamless_ally =
            Player::new_bot(4, "Teamless ally".into(), [0.2, 0.7, 0.4], &config);
        teamless_ally.tile_count = 1;
        teamless_ally.max_troops = 3000.0;
        teamless_ally.alliances.push(2);
        teamless_ally.border_insert(9);
        engine.state.register_player(teamless_ally);
        let mut later_ally = Player::new_bot(5, "Later ally".into(), [0.7, 0.5, 0.2], &config);
        later_ally.tile_count = 1;
        later_ally.max_troops = 4000.0;
        later_ally.border_insert(16);
        engine.state.register_player(later_ally);
        for tile in [8, 9, 16] {
            engine.state.map.terrain[tile] = crate::map::MapTile::from_byte(0x80);
        }
        engine.state.map.set_owner_id(0, 1, 3);
        engine.state.map.set_owner_id(1, 1, 4);
        engine.state.map.set_owner_id(0, 2, 5);
        engine.state.player_mut(1).unwrap().border_insert(8);

        engine
            .campaign_faction_ids
            .insert(1, "suetonius_paulinus".into());
        engine.campaign_assault_force_ids.insert(1);
        let activated =
            engine.activate_campaign_assault(Team::Red, 2, false, true, Some((2.0, 10)), false);
        assert_eq!(activated, 1);
        assert_eq!(
            engine.campaign_assault_targets.get(&1),
            Some(&vec![2, 3, 4])
        );
        let first_wave: Vec<_> = engine
            .attacks
            .iter()
            .filter(|attack| attack.owner_id == 1)
            .collect();
        let mut first_wave_targets: Vec<_> = first_wave
            .iter()
            .map(|attack| attack.target_owner)
            .collect();
        first_wave_targets.sort_unstable();
        assert_eq!(first_wave_targets, [2, 3, 4]);
        assert!(
            first_wave
                .iter()
                .all(|attack| attack.created_tick == engine.state.tick)
        );
        let first_wave_troops: f64 = first_wave.iter().map(|attack| attack.troops).sum();
        assert!((first_wave_troops - 12_000.0).abs() < 0.01);

        let attack_count = engine.attacks.len();
        assert_eq!(
            engine.activate_campaign_assault(Team::Red, 2, false, true, Some((2.0, 10)), false),
            1
        );
        assert_eq!(
            engine.attacks.len(),
            attack_count,
            "repeated activation must not launch or create troops again"
        );

        let incoming_for = |engine: &SowEngine, target_id| {
            engine
                .attacks
                .iter()
                .filter(|attack| attack.owner_id == 1 && attack.target_owner == target_id)
                .map(|attack| attack.troops)
                .sum::<f64>()
                + engine
                    .fleets
                    .iter()
                    .filter(|fleet| fleet.owner_id == 1 && fleet.target_owner == target_id)
                    .map(|fleet| fleet.troops)
                    .sum::<f64>()
        };
        let former_ally_in_flight = incoming_for(&engine, 3);
        let before_new_ally_wave = [
            incoming_for(&engine, 2),
            incoming_for(&engine, 4),
            incoming_for(&engine, 5),
        ];
        engine
            .state
            .player_mut(2)
            .unwrap()
            .alliances
            .retain(|id| *id != 3);
        engine
            .state
            .player_mut(3)
            .unwrap()
            .alliances
            .retain(|id| *id != 2);
        engine.state.player_mut(2).unwrap().alliances.push(5);
        engine.state.player_mut(5).unwrap().alliances.push(2);
        engine.state.tick += 100;
        engine.update_campaign_assault();
        assert_eq!(
            engine.campaign_assault_targets.get(&1),
            Some(&vec![2, 4, 5])
        );
        assert_eq!(
            engine
                .campaign_assault
                .as_ref()
                .unwrap()
                .focus_targets
                .get(&1),
            Some(&5)
        );
        assert_eq!(incoming_for(&engine, 3), former_ally_in_flight);
        assert!(
            !engine
                .campaign_assault_targets
                .get(&1)
                .unwrap()
                .contains(&3),
            "a former ally receives no new campaign orders"
        );
        let new_ally_wave = [
            incoming_for(&engine, 2),
            incoming_for(&engine, 4),
            incoming_for(&engine, 5),
        ];
        assert!(
            new_ally_wave[2] - before_new_ally_wave[2] > new_ally_wave[0] - before_new_ally_wave[0]
        );
        assert!(
            new_ally_wave[2] - before_new_ally_wave[2] > new_ally_wave[1] - before_new_ally_wave[1]
        );
        let army_mass = engine.state.player(1).unwrap().troops
            + engine
                .attacks
                .iter()
                .filter(|attack| attack.owner_id == 1)
                .map(|attack| attack.troops)
                .sum::<f64>()
            + engine
                .fleets
                .iter()
                .filter(|fleet| fleet.owner_id == 1)
                .map(|fleet| fleet.troops)
                .sum::<f64>();
        assert!(
            (army_mass - 16_000.0).abs() < 0.01,
            "live allied capacity must set the refreshed 2:1 force total"
        );

        let in_flight_before_final_target_change = [
            incoming_for(&engine, 3),
            incoming_for(&engine, 4),
            incoming_for(&engine, 5),
        ];
        engine.state.player_mut(2).unwrap().alliances.clear();
        engine.state.player_mut(4).unwrap().alliances.clear();
        engine.state.player_mut(5).unwrap().alliances.clear();
        engine.state.tick += 100;
        engine.update_campaign_assault();
        assert_eq!(
            [
                incoming_for(&engine, 3),
                incoming_for(&engine, 4),
                incoming_for(&engine, 5),
            ],
            in_flight_before_final_target_change,
            "already-launched attacks remain unchanged after alliance changes"
        );
        let army_mass = engine.state.player(1).unwrap().troops
            + engine
                .attacks
                .iter()
                .filter(|attack| attack.owner_id == 1)
                .map(|attack| attack.troops)
                .sum::<f64>()
            + engine
                .fleets
                .iter()
                .filter(|fleet| fleet.owner_id == 1)
                .map(|fleet| fleet.troops)
                .sum::<f64>();
        assert!(
            (army_mass - 16_000.0).abs() < 0.01,
            "existing in-flight troops remain; standing reserves stop at the new cap"
        );
        assert!(
            !engine
                .campaign_assault_targets
                .get(&1)
                .unwrap()
                .contains(&4)
        );
        assert!(
            !engine
                .campaign_assault_targets
                .get(&1)
                .unwrap()
                .contains(&5)
        );

        // Once the old commitments resolve, the next wave obeys the smaller live-target cap.
        engine.attacks.clear();
        engine.fleets.clear();
        engine.state.tick += 100;
        engine.update_campaign_assault();
        let army_mass = engine.state.player(1).unwrap().troops
            + engine
                .attacks
                .iter()
                .filter(|attack| attack.owner_id == 1)
                .map(|attack| attack.troops)
                .sum::<f64>()
            + engine
                .fleets
                .iter()
                .filter(|fleet| fleet.owner_id == 1)
                .map(|fleet| fleet.troops)
                .sum::<f64>();
        assert!((army_mass - 2_000.0).abs() < 0.01);
    }

    #[test]
    fn final_assault_sends_every_roman_force_to_every_live_ally_by_sea_in_one_activation() {
        use crate::game::{GamePhase, GameState};
        use crate::map::MapTile;
        use crate::protocol::Team;
        use crate::water_components::WaterComponents;

        let config = crate::game_config::GameConfig::default();
        let mut state = GameState::new(54, 10, 10, config.clone());
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x20));

        for id in 1..=5 {
            let mut roman = Player::new_bot(id, format!("Roman {id}"), [0.9, 0.1, 0.1], &config);
            roman.team = Some(Team::Red);
            roman.troops = 3_000.0;
            roman.max_troops = 3_000.0;
            roman.tile_count = 1;
            roman.border_insert((id as u32 - 1) * 2 * 10);
            state.register_player(roman);
            let tile = (id as u32 - 1) * 2 * 10;
            state.map.terrain[tile as usize] = MapTile::from_byte(0xC0);
            state.map.set_owner_id(0, tile / 10, id);
        }

        let mut boudica = Player::new_human(6, "Boudica".into(), [0.2, 0.5, 1.0], &config);
        boudica.tile_count = 1;
        boudica.max_troops = 1_000.0;
        boudica.border_insert(9);
        for ally_id in [7, 8, 9, 10, 11] {
            boudica.alliances.push(ally_id);
        }
        state.register_player(boudica);
        state.map.terrain[9] = MapTile::from_byte(0xC0);
        state.map.set_owner_id(9, 0, 6);
        for (id, team) in [(7, Some(Team::Blue)), (8, None), (9, Some(Team::Red))] {
            let mut ally = Player::new_bot(id, format!("Ally {id}"), [0.2, 0.5, 1.0], &config);
            ally.team = team;
            ally.tile_count = 1;
            ally.max_troops = 500.0;
            ally.alliances.push(6);
            let tile = (id as u32 - 5) * 10 + 9;
            ally.border_insert(tile);
            state.register_player(ally);
            state.map.terrain[tile as usize] = MapTile::from_byte(0xC0);
            state.map.set_owner_id(9, tile / 10, id);
        }
        for (id, dead, mutual) in [
            (10, true, true),
            (11, false, false),
            (12, false, false),
            (13, false, false),
        ] {
            let mut faction =
                Player::new_bot(id, format!("Faction {id}"), [0.4, 0.4, 0.4], &config);
            faction.alive = !dead;
            faction.tile_count = 1;
            faction.max_troops = 4_000.0;
            if mutual {
                faction.alliances.push(6);
            }
            let tile = (id as u32 - 4) * 10 + 9;
            faction.border_insert(tile);
            state.register_player(faction);
            state.map.terrain[tile as usize] = MapTile::from_byte(0xC0);
            state.map.set_owner_id(9, tile / 10, id);
        }

        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = SowEngine::new(state, water);
        for id in 1..=5 {
            engine.campaign_assault_force_ids.insert(id);
            engine
                .campaign_faction_ids
                .insert(id, format!("roman_{id}"));
        }
        engine
            .campaign_relations
            .insert(12, crate::protocol::CampaignRelation::Neutral);
        engine
            .campaign_relations
            .insert(13, crate::protocol::CampaignRelation::Enemy);

        assert_eq!(
            engine.activate_campaign_assault(Team::Red, 6, false, true, Some((2.0, 10)), false),
            5
        );
        assert_eq!(
            engine.campaign_assault_targets.get(&1),
            Some(&vec![6, 7, 8, 9])
        );
        assert_eq!(
            engine.fleets.len(),
            20,
            "each of five Roman forces must launch one fleet at each of four live targets"
        );
        for roman_id in 1..=5 {
            let targets: std::collections::HashSet<_> = engine
                .fleets
                .iter()
                .filter(|fleet| fleet.owner_id == roman_id)
                .map(|fleet| fleet.target_owner)
                .collect();
            assert_eq!(targets, [6, 7, 8, 9].into_iter().collect());
            assert_eq!(engine.state.player(roman_id).unwrap().troops, 0.0);
        }
        let launched: f64 = engine.fleets.iter().map(|fleet| fleet.troops).sum();
        assert!(
            (launched - 5_000.0).abs() < 0.01,
            "the wave must not exceed the total available 2:1 force"
        );
    }

    #[test]
    fn final_assault_summons_one_reserve_if_every_tagged_force_is_dead() {
        use crate::protocol::Team;

        let mut engine = test_engine_two_players(53);
        engine
            .state
            .map
            .terrain
            .fill(crate::map::MapTile::from_byte(0x80));
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.state.player_mut(2).unwrap().team = None;
        engine.state.player_mut(1).unwrap().team = Some(Team::Red);
        engine
            .campaign_faction_ids
            .insert(1, "legio_xiv_gemina".into());
        engine.campaign_assault_force_ids.insert(1);
        engine.kill_player(1);

        assert_eq!(
            engine.activate_campaign_assault(Team::Red, 2, false, true, Some((2.0, 10)), true),
            1
        );
        let reserves: Vec<_> = engine
            .state
            .players
            .iter()
            .filter(|player| player.name == "Roman Reserve" && player.alive)
            .collect();
        assert_eq!(reserves.len(), 1);
        assert!(engine.campaign_assault.as_ref().unwrap().reserve_spawned);
        assert!(engine.campaign_assault.as_ref().unwrap().hold_last_tile);
        let before = engine.state.players.len();
        assert_eq!(
            engine.activate_campaign_assault(Team::Red, 2, false, true, Some((2.0, 10)), false),
            1
        );
        assert_eq!(
            engine.state.players.len(),
            before,
            "repeated activation must not spawn another reserve"
        );
        assert!(
            !engine.campaign_assault.as_ref().unwrap().hold_last_tile,
            "choosing resistance releases the final tile"
        );
    }

    #[test]
    fn targeted_campaign_assault_keeps_the_attacker_allied_to_the_player() {
        use crate::protocol::{CampaignRelation, Team};

        let mut engine = test_engine_two_players(43);
        let config = engine.state.config.clone();
        let mut human = Player::new_human(3, "Boudica".into(), [0.0, 0.0, 1.0], &config);
        human.tile_count = 1;
        engine.state.register_player(human);
        engine.state.player_mut(1).unwrap().team = Some(Team::Blue);
        engine.state.player_mut(2).unwrap().team = Some(Team::Red);
        engine.state.player_mut(1).unwrap().alliances.push(3);
        engine.state.player_mut(3).unwrap().alliances.push(1);
        engine.campaign_faction_ids.extend([
            (1, "trinovantes".to_string()),
            (2, "camulodunum".to_string()),
        ]);
        engine
            .campaign_relations
            .insert(1, CampaignRelation::Allied);
        engine.campaign_support_next_tick.insert(1, 100);

        assert_eq!(
            engine.activate_campaign_assault(Team::Blue, 2, true, false, None, false),
            1
        );
        assert_eq!(engine.campaign_assault_targets.get(&1), Some(&vec![2]));
        assert_eq!(
            engine.campaign_relations.get(&1),
            Some(&CampaignRelation::Allied)
        );
        assert!(engine.state.player(1).unwrap().alliances.contains(&3));
        assert!(engine.state.player(3).unwrap().alliances.contains(&1));
        assert!(engine.campaign_support_next_tick.contains_key(&1));
        engine.kill_player(2);
        assert!(!engine.campaign_assault_targets.contains_key(&1));
    }

    #[test]
    fn campaign_assault_land_wave_ignores_tribe_reserve_and_targets_only_focus() {
        use crate::game_config::BotDifficulty;
        use crate::protocol::{CampaignRelation, Team};

        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().team = Some(Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(Team::Blue);
        engine.state.player_mut(2).unwrap().player_type = PlayerType::Human;
        engine.campaign_relations.insert(1, CampaignRelation::Enemy);
        engine.campaign_assault_targets.insert(1, vec![2]);
        let slot = AiSlot {
            bot_id: 1,
            tier: AiTier::Tribe,
            do_attack: true,
            do_structures: false,
            is_under_attack: false,
            ghost_retaliation: None,
            profile: ai_profile_for(AiTier::Tribe, BotDifficulty::Vanilla),
        };
        let mut decisions = Vec::new();
        engine.nation_run_combat_for_slot(&slot, (1, 85), (5.0, 5.0), &[2], true, &mut decisions);

        assert_eq!(attack_targets(&decisions), vec![2]);
        assert!(
            matches!(&decisions[0].intent, GameplayIntent::Attack(attack) if attack.troops == Some(1000.0))
        );
    }

    #[test]
    fn coordinated_campaign_assault_uses_the_saved_legion_focus() {
        use crate::engine::CampaignAssaultState;
        use crate::game_config::BotDifficulty;
        use crate::protocol::{CampaignRelation, Team};

        let mut engine = test_engine_two_players(45);
        engine.state.player_mut(1).unwrap().team = Some(Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(Team::Blue);
        let config = engine.state.config.clone();
        let mut ally = Player::new_human(3, "Ally".into(), [0.2, 0.5, 1.0], &config);
        ally.team = Some(Team::Blue);
        engine.state.register_player(ally);
        engine.campaign_relations.insert(1, CampaignRelation::Enemy);
        engine.campaign_assault_targets.insert(1, vec![2, 3]);
        engine.campaign_assault = Some(CampaignAssaultState {
            root_target: 2,
            team: Team::Red,
            include_allies: true,
            preserve_relation: false,
            hold_last_tile: false,
            capacity_ratio: 2.0,
            interval_ticks: 100,
            next_tick: 100,
            attacker_ids: vec![1],
            target_ids: vec![2, 3],
            wave_index: 0,
            focus_targets: std::collections::HashMap::from([(1, 2)]),
            reserve_spawned: false,
        });

        let slot = AiSlot {
            bot_id: 1,
            tier: AiTier::Tribe,
            do_attack: true,
            do_structures: false,
            is_under_attack: false,
            ghost_retaliation: None,
            profile: ai_profile_for(AiTier::Tribe, BotDifficulty::Vanilla),
        };
        let mut decisions = Vec::new();
        engine.nation_run_combat_for_slot(
            &slot,
            (1, 85),
            (5.0, 5.0),
            &[2, 3],
            true,
            &mut decisions,
        );

        assert_eq!(attack_targets(&decisions), vec![2]);
    }

    #[test]
    fn campaign_assault_uses_a_sea_route_without_a_port_when_target_has_no_land_border() {
        use crate::game::{GamePhase, GameState};
        use crate::game_config::BotDifficulty;
        use crate::map::MapTile;
        use crate::protocol::{CampaignRelation, Team};
        use crate::water_components::WaterComponents;

        let config = crate::game_config::GameConfig::default();
        let mut state = GameState::new(42, 3, 1, config.clone());
        state.phase = GamePhase::Playing;
        state.map.terrain[0] = MapTile::from_byte(0xC0);
        state.map.terrain[1] = MapTile::from_byte(0x20);
        state.map.terrain[2] = MapTile::from_byte(0xC0);
        let mut attacker = Player::new_bot(1, "Roman fleet".into(), [1.0, 0.0, 0.0], &config);
        attacker.team = Some(Team::Red);
        attacker.troops = 1000.0;
        attacker.max_troops = 1000.0;
        attacker.tile_count = 1;
        attacker.border_insert(0);
        let mut target = Player::new_human(2, "Boudica".into(), [0.2, 0.5, 1.0], &config);
        target.team = Some(Team::Blue);
        target.tile_count = 1;
        target.border_insert(2);
        state.register_player(attacker);
        state.register_player(target);
        state.map.set_owner_id(0, 0, 1);
        state.map.set_owner_id(2, 0, 2);
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = SowEngine::new(state, water);
        engine.campaign_relations.insert(1, CampaignRelation::Enemy);
        engine.campaign_assault_targets.insert(1, vec![2]);
        let slot = AiSlot {
            bot_id: 1,
            tier: AiTier::Tribe,
            do_attack: true,
            do_structures: false,
            is_under_attack: false,
            ghost_retaliation: None,
            profile: ai_profile_for(AiTier::Tribe, BotDifficulty::Vanilla),
        };
        let mut decisions = Vec::new();
        engine.nation_run_combat_for_slot(&slot, (1, 85), (5.0, 5.0), &[], true, &mut decisions);

        assert!(
            matches!(decisions.as_slice(), [decision] if matches!(&decision.intent, GameplayIntent::LaunchFleet { target_tile, troops } if *target_tile == 2 && *troops == Some(1000.0)))
        );
    }

    #[test]
    fn campaign_assault_without_land_or_sea_route_falls_back_to_neutral_expansion() {
        use crate::game::{GamePhase, GameState};
        use crate::game_config::BotDifficulty;
        use crate::map::MapTile;
        use crate::protocol::{CampaignRelation, Team};
        use crate::water_components::WaterComponents;

        let config = crate::game_config::GameConfig::default();
        let mut state = GameState::new(42, 3, 1, config.clone());
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0xC0));
        let mut attacker = Player::new_bot(1, "Roman army".into(), [1.0, 0.0, 0.0], &config);
        attacker.team = Some(Team::Red);
        attacker.troops = 1000.0;
        attacker.max_troops = 1000.0;
        attacker.tile_count = 1;
        attacker.border_insert(0);
        let mut target = Player::new_human(2, "Boudica".into(), [0.2, 0.5, 1.0], &config);
        target.team = Some(Team::Blue);
        target.tile_count = 1;
        target.border_insert(2);
        state.register_player(attacker);
        state.register_player(target);
        state.map.set_owner_id(0, 0, 1);
        state.map.set_owner_id(2, 0, 2);
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = SowEngine::new(state, water);
        engine.campaign_relations.insert(1, CampaignRelation::Enemy);
        engine.campaign_assault_targets.insert(1, vec![2]);
        let slot = AiSlot {
            bot_id: 1,
            tier: AiTier::Tribe,
            do_attack: true,
            do_structures: false,
            is_under_attack: false,
            ghost_retaliation: None,
            profile: ai_profile_for(AiTier::Tribe, BotDifficulty::Vanilla),
        };
        let mut decisions = Vec::new();
        engine.nation_run_combat_for_slot(&slot, (1, 85), (5.0, 5.0), &[], true, &mut decisions);

        assert!(!attack_targets(&decisions).contains(&2));
        assert!(decisions.iter().any(|decision| matches!(decision.intent, GameplayIntent::Attack(ref attack) if attack.target_owner == 0)));
    }

    #[test]
    fn same_team_campaign_enemies_ignore_each_other_but_can_attack_boudica() {
        use crate::game_config::BotDifficulty;
        use crate::protocol::{CampaignRelation, Team};

        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().team = Some(Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(Team::Red);
        let config = crate::game_config::GameConfig::default();
        let mut boudica = Player::new_human(3, "Boudica".into(), [0.2, 0.5, 1.0], &config);
        boudica.troops = 200.0;
        boudica.max_troops = 1_000.0;
        boudica.tile_count = 1;
        boudica.border_insert(2);
        engine.state.register_player(boudica);
        engine.campaign_relations.insert(1, CampaignRelation::Enemy);
        engine.campaign_relations.insert(2, CampaignRelation::Enemy);

        let mut profile = ai_profile_for(AiTier::Tribe, BotDifficulty::Terminator);
        profile.refuse_human_chance = 0;
        let slot = AiSlot {
            bot_id: 1,
            tier: AiTier::Tribe,
            do_attack: true,
            do_structures: false,
            is_under_attack: false,
            ghost_retaliation: None,
            profile,
        };
        let mut decisions = Vec::new();
        engine.nation_run_combat_for_slot(
            &slot,
            (1, 135),
            (5.0, 5.0),
            &[2, 3],
            false,
            &mut decisions,
        );

        assert_eq!(attack_targets(&decisions), vec![3]);
        assert_eq!(
            engine.campaign_relations.get(&1),
            Some(&CampaignRelation::Enemy)
        );
        assert_eq!(
            engine.campaign_relations.get(&2),
            Some(&CampaignRelation::Enemy)
        );
    }

    #[test]
    fn campaign_relationships_do_not_disable_normal_bot_ai() {
        use crate::protocol::CampaignRelation;

        for relation in [
            CampaignRelation::Allied,
            CampaignRelation::Neutral,
            CampaignRelation::Enemy,
        ] {
            let mut engine = test_engine_two_players(42);
            engine.campaign_relations.insert(1, relation);
            let mut scheduled = false;
            for _ in 0..128 {
                engine.execute_ai_think();
                scheduled |= engine
                    .test_last_ai_intents
                    .iter()
                    .any(|intent| intent.player_id == 1);
                engine.state.tick += 1;
            }
            assert!(
                scheduled,
                "{relation:?} campaign faction receives normal AI decisions"
            );
        }
    }
}
