#[cfg(test)]
mod alliance_lifecycle_tests {
    use crate::diplomacy::{ALLIANCE_REQUEST_TTL_TICKS, AllianceProposal};
    use crate::engine::SowEngine;
    use crate::game::{GamePhase, GameState};
    use crate::game_config::GameConfig;
    use crate::player::{Player, PlayerType};
    use crate::protocol::{GameplayIntent, StampedIntent};
    use crate::water_components::WaterComponents;

    fn minimal_engine() -> SowEngine {
        let mut game = GameState::new(1, 4, 4, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;
        SowEngine::new(game, WaterComponents::default())
    }

    fn campaign_contact_engine(gold: f64, contact: bool) -> SowEngine {
        let config = GameConfig::default();
        let mut game = GameState::new(1, 4, 4, config.clone());
        game.phase = GamePhase::Playing;
        let mut human = Player::new_human(1, "Boudica".into(), [1.0; 3], &config);
        human.gold = gold;
        if contact { human.border_tiles.insert(5); }
        game.register_player(human);
        let mut tribe = Player::new_human(2, "Venta Icenorum".into(), [0.5; 3], &config);
        tribe.player_type = PlayerType::Bot;
        tribe.gold = 50.0;
        game.register_player(tribe);
        game.map.set_owner_id(2, 1, 2);
        let mut engine = SowEngine::new(game, WaterComponents::default());
        engine.campaign_relations.insert(2, crate::protocol::CampaignRelation::Neutral);
        engine
    }

    #[test]
    fn campaign_contact_choice_pays_atomically_and_updates_the_relationship() {
        let mut engine = campaign_contact_engine(250.0, true);
        engine.apply_stamped_intent(&StampedIntent {
            player_id: 1,
            intent: GameplayIntent::ResolveCampaignDiplomacy {
                target_player: 2,
                relation: crate::protocol::CampaignRelation::Allied,
                gold_cost: 200.0,
            },
        }, 0);

        assert_eq!(engine.state.player(1).unwrap().gold, 50.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 250.0);
        assert!(engine.state.player(1).unwrap().alliances.contains(&2));
        assert!(engine.state.player(2).unwrap().alliances.contains(&1));
        assert_eq!(engine.campaign_relations.get(&2), Some(&crate::protocol::CampaignRelation::Allied));
        let snapshot = engine.build_snapshot();
        assert_eq!(snapshot.players.iter().find(|player| player.id == 2).unwrap().color, [0.2, 0.5, 1.0]);
    }

    #[test]
    fn campaign_contact_choice_rejects_missing_gold_or_contact_without_partial_changes() {
        for (gold, contact) in [(199.0, true), (500.0, false)] {
            let mut engine = campaign_contact_engine(gold, contact);
            engine.apply_stamped_intent(&StampedIntent {
                player_id: 1,
                intent: GameplayIntent::ResolveCampaignDiplomacy {
                    target_player: 2,
                    relation: crate::protocol::CampaignRelation::Allied,
                    gold_cost: 200.0,
                },
            }, 0);
            assert_eq!(engine.state.player(1).unwrap().gold, gold);
            assert_eq!(engine.state.player(2).unwrap().gold, 50.0);
            assert!(engine.state.player(1).unwrap().alliances.is_empty());
            assert_eq!(engine.campaign_relations.get(&2), Some(&crate::protocol::CampaignRelation::Neutral));
            assert!(!engine.campaign_contact_resolved.contains(&2));
        }
    }

    #[test]
    fn campaign_contact_choice_rejects_gold_above_the_editor_limit() {
        let mut engine = campaign_contact_engine(1_000_001.0, true);
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::ResolveCampaignDiplomacy {
                    target_player: 2,
                    relation: crate::protocol::CampaignRelation::Allied,
                    gold_cost: 1_000_000.01,
                },
            },
            0,
        );

        assert_eq!(engine.state.player(1).unwrap().gold, 1_000_001.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 50.0);
        assert!(engine.state.player(1).unwrap().alliances.is_empty());
        assert_eq!(
            engine.campaign_relations.get(&2),
            Some(&crate::protocol::CampaignRelation::Neutral)
        );
        assert!(!engine.campaign_contact_resolved.contains(&2));
    }

    #[test]
    fn refusing_campaign_terms_marks_the_faction_enemy_and_red_without_charging_gold() {
        let mut engine = campaign_contact_engine(250.0, true);
        engine.apply_stamped_intent(&StampedIntent {
            player_id: 1,
            intent: GameplayIntent::ResolveCampaignDiplomacy {
                target_player: 2,
                relation: crate::protocol::CampaignRelation::Enemy,
                gold_cost: 0.0,
            },
        }, 0);

        assert_eq!(engine.state.player(1).unwrap().gold, 250.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 50.0);
        assert!(engine.state.player(1).unwrap().alliances.is_empty());
        assert!(engine.state.player(2).unwrap().alliances.is_empty());
        assert_eq!(engine.campaign_relations.get(&2), Some(&crate::protocol::CampaignRelation::Enemy));
        let snapshot = engine.build_snapshot();
        assert_eq!(snapshot.players.iter().find(|player| player.id == 2).unwrap().color, [1.0, 0.2, 0.2]);
    }

    #[test]
    fn declining_scripted_terms_still_allows_the_player_to_form_a_neutral_alliance() {
        let mut engine = campaign_contact_engine(250.0, true);
        engine.campaign_can_request_alliance.insert(2, false);
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::ResolveCampaignDiplomacy {
                    target_player: 2,
                    relation: crate::protocol::CampaignRelation::Neutral,
                    gold_cost: 0.0,
                },
            },
            0,
        );

        assert!(engine.campaign_contact_resolved.contains(&2));
        assert_eq!(engine.campaign_relations.get(&2), Some(&crate::protocol::CampaignRelation::Neutral));

        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::ProposeAlliance { target_player: 2 },
            },
            1,
        );
        assert!(engine.alliances_proposed.iter().any(|proposal| proposal.proposer == 1 && proposal.target == 2));

        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 2,
                intent: GameplayIntent::AcceptAlliance { target_player: 1 },
            },
            2,
        );
        assert_eq!(engine.campaign_relations.get(&2), Some(&crate::protocol::CampaignRelation::Allied));
        assert!(engine.state.player(1).unwrap().alliances.contains(&2));
        assert!(engine.state.player(2).unwrap().alliances.contains(&1));
    }

    #[test]
    fn campaign_alliance_break_marks_the_faction_as_an_enemy() {
        let mut engine = campaign_contact_engine(100.0, true);
        engine.campaign_relations.insert(2, crate::protocol::CampaignRelation::Allied);
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine.apply_stamped_intent(&StampedIntent {
            player_id: 2,
            intent: GameplayIntent::BreakAlliance { target_player: 1 },
        }, 0);
        assert_eq!(engine.campaign_relations.get(&2), Some(&crate::protocol::CampaignRelation::Enemy));
        let snapshot = engine.build_snapshot();
        assert_eq!(snapshot.players.iter().find(|player| player.id == 2).unwrap().color, [1.0, 0.2, 0.2]);
    }

    #[test]
    fn proposal_expires_and_sets_cooldown() {
        let mut engine = minimal_engine();
        engine.state.tick = 0;
        engine.push_alliance_proposal(1, 2);
        engine.state.tick = ALLIANCE_REQUEST_TTL_TICKS as u64 + 1;
        engine.prune_alliance_diplomacy();
        assert!(engine.alliances_proposed.is_empty());
        assert!(engine.alliance_request_cooldown_until.contains_key(&(1, 2)));
        assert!(!engine.can_send_alliance_request(1, 2));
        let until = *engine.alliance_request_cooldown_until.get(&(1, 2)).unwrap();
        engine.state.tick = until as u64 + 1;
        engine.prune_alliance_diplomacy();
        assert!(engine.can_send_alliance_request(1, 2));
    }

    #[test]
    fn reject_marks_cooldown() {
        let mut engine = minimal_engine();
        engine.push_alliance_proposal(1, 2);
        let stamped = StampedIntent {
            player_id: 2,
            intent: GameplayIntent::RejectAlliance { target_player: 1 },
        };
        engine.apply_stamped_intent(&stamped, 0);
        assert!(!engine.can_send_alliance_request(1, 2));
    }

    #[test]
    fn proposal_records_created_tick() {
        let mut engine = minimal_engine();
        engine.state.tick = 42;
        engine.push_alliance_proposal(3, 4);
        assert_eq!(
            engine.alliances_proposed[0],
            AllianceProposal {
                proposer: 3,
                target: 4,
                created_tick: 42,
            }
        );
    }
}
