use crate::app::SowApp;
use crate::app::{TutorialObservation, TutorialPausedAction};
use sow_core::protocol::GameplayIntent;

fn is_tutorial_control_intent(intent: &GameplayIntent) -> bool {
    matches!(
        intent,
        GameplayIntent::Resign | GameplayIntent::ResolveCampaignDiplomacy { .. }
    )
}

fn is_emote_intent(intent: &GameplayIntent) -> bool {
    matches!(
        intent,
        GameplayIntent::ExpressEmote { .. }
            | GameplayIntent::ClearEmote
            | GameplayIntent::SetEmotePinned { .. }
    )
}

fn paused_tutorial_intent_allowed(
    action: Option<TutorialPausedAction>,
    marker_player_id: Option<u16>,
    intent: &GameplayIntent,
) -> bool {
    is_tutorial_control_intent(intent)
        || match action {
            Some(TutorialPausedAction::ExpandOnceThenResume) => {
                matches!(intent, GameplayIntent::Attack(attack) if attack.target_owner == 0)
            }
            Some(TutorialPausedAction::AttackOnceThenResume) => marker_player_id.is_some_and(|target| {
                matches!(intent, GameplayIntent::Attack(attack) if attack.target_owner == target)
            }),
            Some(TutorialPausedAction::SendResourcesStayPaused) => {
                matches!(intent, GameplayIntent::SendResources { .. })
            }
            Some(TutorialPausedAction::RequestResourcesStayPaused) => {
                matches!(intent, GameplayIntent::RequestResources { .. })
            }
            Some(TutorialPausedAction::RequestAllianceStayPaused) => {
                matches!(intent, GameplayIntent::ProposeAlliance { .. })
            }
            Some(TutorialPausedAction::BuildUntilStarted) => match intent {
                GameplayIntent::BuildStructure { kind, .. } => matches!(
                    *kind,
                    sow_core::game::BuildingKind::City
                        | sow_core::game::BuildingKind::Farm
                        | sow_core::game::BuildingKind::Factory
                        | sow_core::game::BuildingKind::Port
                        | sow_core::game::BuildingKind::Tower
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
        GameplayIntent::Attack(attack) if attack.target_owner == 0 => {
            Some(observation.wilderness_orders_accepted)
        }
        GameplayIntent::Attack(_) => Some(
            observation
                .attacks_by_faction_id
                .values()
                .fold(0, |total, count| total.saturating_add(*count)),
        ),
        GameplayIntent::SendResources { .. } => Some(observation.resource_transfers),
        GameplayIntent::RequestResources { .. } => Some(observation.resource_requests),
        GameplayIntent::ProposeAlliance { .. } => Some(observation.alliance_requests_submitted),
        GameplayIntent::BuildStructure { kind, .. } => {
            let key = match *kind {
                sow_core::game::BuildingKind::City => "cities",
                sow_core::game::BuildingKind::Farm => "farms",
                sow_core::game::BuildingKind::Factory => "factories",
                sow_core::game::BuildingKind::Port => "ports",
                sow_core::game::BuildingKind::Tower => "towers",
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
    /// Applies a tutorial emote in its own turn. The offline queue is not safe for emotes: pause
    /// commands clear it, which silently dropped the step emote that arrived just before a pause.
    fn apply_tutorial_emote(&mut self, intent: sow_core::protocol::GameplayIntent) -> bool {
        let Some(player_id) = self.sim.my_player_id else {
            return false;
        };
        if self.sim.paused {
            self.apply_paused_tutorial_intent(intent);
            return true;
        }
        self.dispatch_sim_command(sow_core::protocol::SimCommand::Turn(
            sow_core::protocol::Turn {
                turn_number: 0,
                intents: vec![sow_core::protocol::StampedIntent { player_id, intent }],
            },
        ));
        true
    }

    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) -> bool {
        if self.ui.tutorial_active && self.net.is_offline && is_emote_intent(&intent) {
            return self.apply_tutorial_emote(intent);
        }
        let tutorial_control = is_tutorial_control_intent(&intent);
        if self.ui.tutorial_camera_only && !tutorial_control {
            return false;
        }
        let paused_tutorial = self.ui.tutorial_active && self.net.is_offline && self.sim.paused;
        if paused_tutorial
            && self.ui.tutorial_paused_action
                == Some(TutorialPausedAction::RequestAllianceStayPaused)
            && matches!(&intent, GameplayIntent::ProposeAlliance { target_player } if Some(*target_player) != self.ui.tutorial_marker_player_id)
        {
            return false;
        }
        if paused_tutorial
            && !paused_tutorial_intent_allowed(
                self.ui.tutorial_paused_action,
                self.ui.tutorial_marker_player_id,
                &intent,
            )
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
                [
                    self.input.last_mouse_x as f32,
                    self.input.last_mouse_y as f32,
                ],
            );
            return false;
        }
        match &intent {
            sow_core::protocol::GameplayIntent::LaunchFleet { target_tile, .. } => {
                let wx = (*target_tile % self.sim.map_w) as f32 + 0.5;
                let wy = (*target_tile / self.sim.map_w) as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                    kind: crate::app::ClickMarkerKind::TransportOrder,
                });
            }
            sow_core::protocol::GameplayIntent::MoveWarships { target_tile, .. } => {
                let wx = (*target_tile % self.sim.map_w) as f32 + 0.5;
                let wy = (*target_tile / self.sim.map_w) as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                    kind: crate::app::ClickMarkerKind::WarshipOrder,
                });
            }
            sow_core::protocol::GameplayIntent::Spawn { x, y } => {
                let wx = *x as f32 + 0.5;
                let wy = *y as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                    kind: crate::app::ClickMarkerKind::Generic,
                });
                self.sfx
                    .play_deploy(web_time::Instant::now(), self.spatial_sound_params(wx, wy));
            }
            sow_core::protocol::GameplayIntent::Attack(attack) => {
                let now = web_time::Instant::now();
                // Flash enemy borders red
                self.ui
                    .border_flashes
                    .push(crate::app::BorderFlashInstance {
                        player_id: attack.target_owner,
                        start_time: now,
                        attacker_count: 1,
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

        // A dialogue can pause immediately after sending its outcome, clearing the
        // offline queue. Apply campaign diplomacy without waiting for a world tick.
        let campaign_diplomacy = self.ui.tutorial_active
            && self.net.is_offline
            && matches!(&intent, GameplayIntent::ResolveCampaignDiplomacy { .. });
        if paused_tutorial || campaign_diplomacy {
            if tutorial_control {
                self.apply_paused_tutorial_intent(intent);
                return true;
            }
            let before = paused_tutorial_progress(&self.sim.tutorial_observation, &intent);
            self.apply_paused_tutorial_intent(intent.clone());
            let after = paused_tutorial_progress(&self.sim.tutorial_observation, &intent);
            let accepted = paused_tutorial_action_accepted(before, after);
            if accepted {
                let resume_after_attack = self.ui.tutorial_paused_action
                    == Some(TutorialPausedAction::AttackOnceThenResume);
                self.ui.tutorial_paused_action = None;
                if resume_after_attack {
                    self.sim.paused = false;
                    self.sim.offline_tick_timer = 0.0;
                }
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
        amount_text: format!("+{}", crate::utils::format_number(troops)),
        world_x,
        world_y,
        start_time: now,
    });
}

