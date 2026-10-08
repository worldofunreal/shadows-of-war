use crate::app::SowApp;
use crate::app::{TutorialObservation, TutorialPausedAction};
use sow_core::protocol::GameplayIntent;

fn is_waiting_for_campaign_attack(
    waiting: bool,
    campaign: crate::campaign::CampaignId,
    intent: &sow_core::protocol::GameplayIntent,
) -> bool {
    waiting
        && campaign == crate::campaign::CampaignId::Boudica
        && matches!(intent, sow_core::protocol::GameplayIntent::Attack(_))
}

fn paused_tutorial_intent_allowed(
    action: Option<TutorialPausedAction>,
    intent: &GameplayIntent,
) -> bool {
    matches!(intent, GameplayIntent::Resign)
        || match action {
            Some(TutorialPausedAction::ExpandOnceThenResume) => {
                matches!(intent, GameplayIntent::Spawn { .. })
            }
            Some(TutorialPausedAction::SendResourcesStayPaused) => {
                matches!(intent, GameplayIntent::SendResources { .. })
            }
            Some(TutorialPausedAction::BuildUntilStarted) => match intent {
                GameplayIntent::BuildStructure { kind, .. } => matches!(
                    *kind,
                    sow_core::game::BuildingKind::City
                        | sow_core::game::BuildingKind::Factory
                        | sow_core::game::BuildingKind::Bunker
                ),
                _ => false,
            },
            Some(TutorialPausedAction::UpgradeUntilStarted) => {
                matches!(intent, GameplayIntent::UpgradeStructure { .. })
            }
            None => false,
        }
}

fn paused_tutorial_progress(
    observation: &TutorialObservation,
    intent: &GameplayIntent,
) -> Option<u64> {
    match intent {
        GameplayIntent::Spawn { .. } => Some(observation.tiles_gained),
        GameplayIntent::SendResources { .. } => Some(observation.resource_transfers),
        GameplayIntent::BuildStructure { kind, .. } => {
            let key = match *kind {
                sow_core::game::BuildingKind::City => "cities",
                sow_core::game::BuildingKind::Factory => "factories",
                sow_core::game::BuildingKind::Bunker => "bunkers",
                _ => return None,
            };
            Some(
                observation
                    .seen_buildings_by_kind
                    .get(key)
                    .map_or(0, |buildings| buildings.len() as u64),
            )
        }
        GameplayIntent::UpgradeStructure { .. } => Some(observation.structure_upgrades),
        _ => None,
    }
}

fn paused_tutorial_action_accepted(before: Option<u64>, after: Option<u64>) -> bool {
    matches!((before, after), (Some(before), Some(after)) if after > before)
}

impl SowApp {
    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) -> bool {
        if self.ui.tutorial_waiting_for_first_attack {
            if !is_waiting_for_campaign_attack(
                self.ui.tutorial_waiting_for_first_attack,
                self.ui.tutorial_campaign,
                &intent,
            ) {
                return false;
            }
            self.sim.offline_intents.clear();
            self.ui.tutorial_waiting_for_first_attack = false;
            self.sim.paused = false;
            self.sim.offline_tick_timer = 0.0;
        }
        if self.ui.tutorial_camera_only
            && !matches!(&intent, sow_core::protocol::GameplayIntent::Resign)
        {
            return false;
        }
        let paused_tutorial = self.ui.tutorial_active && self.net.is_offline && self.sim.paused;
        if paused_tutorial
            && !paused_tutorial_intent_allowed(self.ui.tutorial_paused_action, &intent)
        {
            return false;
        }
        if matches!(&intent, sow_core::protocol::GameplayIntent::Spawn { .. })
            && !paused_tutorial
            && let Some(snapshot) = self.sim.current_snapshot.as_ref()
            && matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. })
            && !self
                .input
                .spawn_intent_rate
                .admit_at_tick(snapshot.tick, self.sim.config.tick_rate_ms)
        {
            self.ui.app.hud_state.push_map_feedback(
                crate::ui::UiText::new("hud.spawn_rate_limited"),
                [self.input.last_mouse_x as f32, self.input.last_mouse_y as f32],
            );
            return false;
        }
        match &intent {
            sow_core::protocol::GameplayIntent::LaunchFleet { target_tile, .. }
            | sow_core::protocol::GameplayIntent::MoveWarships { target_tile, .. } => {
                let wx = (*target_tile % self.sim.map_w) as f32 + 0.5;
                let wy = (*target_tile / self.sim.map_w) as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });
            }
            sow_core::protocol::GameplayIntent::Spawn { x, y } => {
                let wx = *x as f32 + 0.5;
                let wy = *y as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });
                self.sfx
                    .play_deploy(web_time::Instant::now(), self.spatial_sound_params(wx, wy));
            }
            sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile } => {
                let wx = (*target_tile % self.sim.map_w) as f32 + 0.5;
                let wy = (*target_tile / self.sim.map_w) as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });
                self.sfx.play_placement(
                    web_time::Instant::now(),
                    crate::building_sound_kind(*kind),
                    self.spatial_sound_params(wx, wy),
                );
            }
            sow_core::protocol::GameplayIntent::Attack(attack) => {
                let now = web_time::Instant::now();
                // Flash enemy borders red
                self.ui
                    .border_flashes
                    .push(crate::app::BorderFlashInstance {
                        player_id: attack.target_owner,
                        start_time: now,
                        max_intensity: 1.0,
                    });
                record_attack_launch_notice(
                    &mut self.ui.attack_launch_notices,
                    &intent,
                    self.input.last_mouse_x,
                    self.input.last_mouse_y,
                    self.input.camera_x,
                    self.input.camera_y,
                    self.input.camera_zoom,
                    now,
                );
            }
            _ => {}
        }

        if paused_tutorial {
            if matches!(&intent, GameplayIntent::Resign) {
                self.apply_paused_tutorial_intent(intent);
                return true;
            }
            let before = paused_tutorial_progress(&self.sim.tutorial_observation, &intent);
            self.apply_paused_tutorial_intent(intent.clone());
            let after = paused_tutorial_progress(&self.sim.tutorial_observation, &intent);
            let accepted = paused_tutorial_action_accepted(before, after);
            if accepted {
                self.ui.tutorial_paused_action = None;
            }
            return accepted;
        }

        if let Some(c) = self.net.client.as_ref() {
            let msg = sow_core::protocol::ClientMessage::Gameplay {
                intent: intent.clone(),
            };
            if let Ok(json) = bincode::serialize(&msg) {
                c.send(json);
                true
            } else {
                false
            }
        } else {
            self.sim.offline_intents.push(intent);
            true
        }
    }
}

fn record_attack_launch_notice(
    notices: &mut Vec<crate::app::AttackLaunchNotice>,
    intent: &sow_core::protocol::GameplayIntent,
    mouse_x: f64,
    mouse_y: f64,
    camera_x: f32,
    camera_y: f32,
    camera_zoom: f32,
    now: web_time::Instant,
) {
    let sow_core::protocol::GameplayIntent::Attack(attack) = intent else {
        return;
    };
    let Some(troops) = attack
        .troops
        .filter(|troops| troops.is_finite() && *troops > 0.0)
    else {
        return;
    };

    let world_x = (mouse_x as f32 - camera_x) / camera_zoom;
    // Keep the notice above the pointer, matching the old click feedback.
    let pointer_y = mouse_y as f32 - 60.0;
    let world_y = (pointer_y - camera_y) / camera_zoom;
    notices.push(crate::app::AttackLaunchNotice {
        text: format!("⚔️ +{}", crate::utils::format_number(troops)),
        world_x,
        world_y,
        start_time: now,
    });
}

#[cfg(test)]
mod tests {
    use super::{
        is_waiting_for_campaign_attack, paused_tutorial_intent_allowed,
        paused_tutorial_action_accepted, record_attack_launch_notice,
    };
    use crate::app::TutorialPausedAction;
    use crate::campaign::CampaignId;
    use sow_core::protocol::{AttackIntent, GameplayIntent};
    use wasm_bindgen_test::wasm_bindgen_test;

    #[test]
    fn first_boudica_attack_unpauses_without_a_fixed_faction_id() {
        let attack = GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(100.0),
        });
        assert!(is_waiting_for_campaign_attack(true, CampaignId::Boudica, &attack));
        assert!(!is_waiting_for_campaign_attack(true, CampaignId::SixSkyEp1, &attack));
        assert!(!is_waiting_for_campaign_attack(false, CampaignId::Boudica, &attack));
        assert!(!is_waiting_for_campaign_attack(
            true,
            CampaignId::Boudica,
            &GameplayIntent::Resign
        ));
    }

    #[test]
    fn paused_tutorial_actions_are_limited_to_the_current_step() {
        let spawn = GameplayIntent::Spawn { x: 4, y: 5 };
        let send = GameplayIntent::SendResources {
            target_player: 2,
            gold: 1.0,
            troops: 0.0,
        };
        let build = GameplayIntent::BuildStructure {
            kind: sow_core::game::BuildingKind::City,
            target_tile: 20,
        };
        let upgrade = GameplayIntent::UpgradeStructure { building_id: 1 };

        assert!(paused_tutorial_intent_allowed(
            Some(TutorialPausedAction::ExpandOnceThenResume),
            &spawn
        ));
        assert!(!paused_tutorial_intent_allowed(
            Some(TutorialPausedAction::ExpandOnceThenResume),
            &send
        ));
        assert!(paused_tutorial_intent_allowed(
            Some(TutorialPausedAction::SendResourcesStayPaused),
            &send
        ));
        assert!(!paused_tutorial_intent_allowed(
            Some(TutorialPausedAction::SendResourcesStayPaused),
            &spawn
        ));
        assert!(paused_tutorial_intent_allowed(
            Some(TutorialPausedAction::BuildUntilStarted),
            &build
        ));
        assert!(paused_tutorial_intent_allowed(
            Some(TutorialPausedAction::UpgradeUntilStarted),
            &upgrade
        ));
        assert!(!paused_tutorial_intent_allowed(
            Some(TutorialPausedAction::BuildUntilStarted),
            &upgrade
        ));
        assert!(!paused_tutorial_intent_allowed(None, &spawn));
        assert!(paused_tutorial_intent_allowed(None, &GameplayIntent::Resign));
    }

    #[test]
    fn paused_tutorial_only_advances_after_observed_progress() {
        assert!(!paused_tutorial_action_accepted(Some(12), Some(12)));
        assert!(!paused_tutorial_action_accepted(Some(12), Some(11)));
        assert!(!paused_tutorial_action_accepted(Some(12), None));
        assert!(!paused_tutorial_action_accepted(None, Some(13)));
        assert!(paused_tutorial_action_accepted(Some(12), Some(13)));
    }

    #[wasm_bindgen_test]
    fn repeated_attacks_each_get_a_notice_even_for_the_same_target() {
        let intent = GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(13_000.0),
        });
        let mut notices = Vec::new();
        let now = web_time::Instant::now();
        for _ in 0..2 {
            record_attack_launch_notice(&mut notices, &intent, 100.0, 200.0, 10.0, 20.0, 2.0, now);
        }
        assert_eq!(notices.len(), 2);
        assert_eq!(notices[0].text, "⚔️ +13.0K");
        assert_eq!(notices[1].text, "⚔️ +13.0K");
        assert_eq!((notices[0].world_x, notices[0].world_y), (45.0, 60.0));
    }

    #[wasm_bindgen_test]
    fn invalid_amounts_and_non_attack_intents_create_no_notice() {
        let now = web_time::Instant::now();
        let mut notices = Vec::new();
        for troops in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            let intent = GameplayIntent::Attack(AttackIntent {
                target_owner: 2,
                troops,
            });
            record_attack_launch_notice(&mut notices, &intent, 100.0, 200.0, 0.0, 0.0, 1.0, now);
        }
        record_attack_launch_notice(
            &mut notices,
            &GameplayIntent::Resign,
            100.0,
            200.0,
            0.0,
            0.0,
            1.0,
            now,
        );
        assert!(notices.is_empty());
    }
}
