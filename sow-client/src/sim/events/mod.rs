mod combat;
mod elimination;

use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

fn suppress_submitted_notice_if_formed(
    status: sow_core::game::AllianceRequestStatus,
    was_allied: bool,
    is_allied: bool,
) -> bool {
    status == sow_core::game::AllianceRequestStatus::Submitted && !was_allied && is_allied
}

fn alliance_request_notification(
    proposer_id: u16,
    target_id: u16,
    status: sow_core::game::AllianceRequestStatus,
    my_id: u16,
    target_name: String,
) -> Option<(crate::ui::UiText, [Option<u16>; 2])> {
    if my_id == 0 || proposer_id != my_id {
        return None;
    }
    let text = match status {
        sow_core::game::AllianceRequestStatus::Submitted => {
            crate::ui::UiText::new("hud.alliance_request_sent")
        }
        sow_core::game::AllianceRequestStatus::Rejected => {
            crate::ui::UiText::new("hud.alliance_request_rejected")
        }
        sow_core::game::AllianceRequestStatus::Expired => {
            crate::ui::UiText::new("hud.alliance_request_expired")
        }
    };
    Some((
        text.with("name", target_name),
        [Some(proposer_id), Some(target_id)],
    ))
}

impl SowApp {
    pub(crate) fn process_tick_events(
        &mut self,
        events: Vec<sow_core::game::GameEvent>,
        snap: &SimSnapshot,
        my_id: u16,
    ) -> crate::player_progress::SessionDefeats {
        let mut turn_defeats = crate::player_progress::SessionDefeats::default();
        let mut played_combat_this_tick = false;
        let now_instant = web_time::Instant::now();
        self.sfx.sync_server_phase(&snap.phase);
        self.sfx.begin_tick();
        for event in events {
            match event {
                sow_core::game::GameEvent::ResourceTransferred {
                    receiver_id,
                    troops,
                    ..
                } => {
                    self.ui
                        .nameplates
                        .animate_troop_change(receiver_id, troops, now_instant);
                }
                sow_core::game::GameEvent::TransportShipLanded { owner_id, x, y } => {
                    let color = snap
                        .players
                        .iter()
                        .find(|player| player.id == owner_id)
                        .map(|player| {
                            sow_core::player::player_territory_rgb(
                                player.player_type,
                                player.team,
                                player.color,
                                player.is_campaign_faction,
                            )
                        })
                        .unwrap_or([0.13, 0.83, 0.94]);
                    if self.ui.transport_impacts.len() == crate::app::MAX_TRANSPORT_IMPACTS {
                        self.ui.transport_impacts.pop_front();
                    }
                    self.ui
                        .transport_impacts
                        .push_back(crate::app::TransportImpact {
                            tile_x: x,
                            tile_y: y,
                            color,
                            cue_color: None,
                            start_time: now_instant,
                        });
                }
                sow_core::game::GameEvent::AbilityTriggered { owner_id, tile, affected } => {
                    let is_mine = my_id != 0 && owner_id == my_id;
                    let is_affected = my_id != 0 && affected.contains(&my_id);
                    if !is_mine && !is_affected {
                        continue;
                    }
                    let Some(owner) = snap.players.iter().find(|player| player.id == owner_id)
                    else {
                        continue;
                    };
                    if let Some((x, y)) = tile {
                        let color = sow_core::player::player_territory_rgb(
                            owner.player_type,
                            owner.team,
                            owner.color,
                            owner.is_campaign_faction,
                        );
                        if self.ui.transport_impacts.len() == crate::app::MAX_TRANSPORT_IMPACTS {
                            self.ui.transport_impacts.pop_front();
                        }
                        self.ui
                            .transport_impacts
                            .push_back(crate::app::TransportImpact {
                                tile_x: x,
                                tile_y: y,
                                color,
                                cue_color: Some(crate::app::ability_cue_rgb(
                                    owner.leader.ability().cue(),
                                )),
                                start_time: now_instant,
                            });
                    }
                    // The notice shows the hero's icon and its own ability name.
                    let hero = sow_core::commerce::leader_id(owner.leader).to_string();
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.ability_notice")
                            .with("hero", hero.clone())
                            .with("ability_key", format!("site.leader_{hero}_ability")),
                        [Some(owner_id), None],
                        3,
                        Some(format!("ability:{owner_id}")),
                        false,
                    );
                }
                sow_core::game::GameEvent::PlayerEliminated {
                    player_id,
                    conqueror_id,
                    gold_bounty,
                    elimination_x,
                    elimination_y,
                    assists,
                    by_nuke,
                } => {
                    self.handle_player_eliminated(
                        snap,
                        my_id,
                        now_instant,
                        &mut turn_defeats,
                        &elimination::EliminationEventInfo {
                            player_id,
                            conqueror_id,
                            pos: (elimination_x, elimination_y),
                            gold_bounty,
                            assists: &assists,
                            by_nuke,
                        },
                    );
                    if conqueror_id == my_id && my_id != 0 {
                        self.ui
                            .trigger_viewport_alert(crate::app::ViewportAlertKind::ConquerPlayer);
                    }
                }
                sow_core::game::GameEvent::PlayerRebelled {
                    player_id,
                    rebel_id,
                    ..
                } if player_id == my_id && my_id != 0 => {
                    let name = snap
                        .players
                        .iter()
                        .find(|player| player.id == rebel_id)
                        .map(|player| player.name.clone())
                        .unwrap_or_else(|| "Rebels".to_string());
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.debt_rebellion").with("name", name),
                        [Some(my_id), Some(rebel_id)],
                        3,
                        Some(format!("debt-rebellion:{rebel_id}")),
                        false,
                    );
                }
                sow_core::game::GameEvent::TileCaptured {
                    x,
                    y,
                    new_owner,
                    previous_owner,
                    troops,
                } => {
                    self.handle_tile_captured(
                        snap,
                        my_id,
                        &mut played_combat_this_tick,
                        &combat::CaptureEventInfo {
                            pos: (x, y),
                            new_owner,
                            previous_owner,
                            troops,
                        },
                    );
                }
                sow_core::game::GameEvent::AllianceRequestLifecycle {
                    proposer_id,
                    target_id,
                    status,
                } => {
                    if proposer_id != my_id || my_id == 0 {
                        continue;
                    }
                    let was_allied = self
                        .sim
                        .current_snapshot
                        .as_ref()
                        .and_then(|previous| {
                            previous
                                .players
                                .iter()
                                .find(|player| player.id == proposer_id)
                        })
                        .is_some_and(|player| player.alliances.contains(&target_id));
                    let is_allied = snap
                        .players
                        .iter()
                        .find(|player| player.id == proposer_id)
                        .is_some_and(|player| player.alliances.contains(&target_id));
                    if suppress_submitted_notice_if_formed(status, was_allied, is_allied) {
                        continue;
                    }
                    let target_name = snap
                        .players
                        .iter()
                        .find(|player| player.id == target_id)
                        .map(|player| {
                            sow_core::player::display_name(
                                player.id,
                                &player.name,
                                player.player_type,
                            )
                        })
                        .unwrap_or_else(|| format!("Player {target_id}"));
                    if let Some((text, players)) = alliance_request_notification(
                        proposer_id,
                        target_id,
                        status,
                        my_id,
                        target_name,
                    ) {
                        self.ui
                            .app
                            .hud_state
                            .push_notification_for_players(text, players, 3, None, false);
                    }
                }
                _ => {}
            }
        }
        self.sfx.flush_tick(now_instant);
        turn_defeats
    }
}

#[cfg(test)]
mod tests {
    use super::{alliance_request_notification, suppress_submitted_notice_if_formed};
    use sow_core::game::AllianceRequestStatus;

    #[test]
    fn alliance_request_lifecycle_notices_only_reach_the_proposer() {
        for (status, key) in [
            (
                AllianceRequestStatus::Submitted,
                "hud.alliance_request_sent",
            ),
            (
                AllianceRequestStatus::Rejected,
                "hud.alliance_request_rejected",
            ),
            (
                AllianceRequestStatus::Expired,
                "hud.alliance_request_expired",
            ),
        ] {
            let (notice, players) = alliance_request_notification(1, 2, status, 1, "Caesar".into())
                .expect("the proposer should receive the lifecycle notice");
            assert_eq!(notice.key, key);
            assert_eq!(notice.values, vec![("name", "Caesar".into())]);
            assert_eq!(players, [Some(1), Some(2)]);
            assert!(alliance_request_notification(1, 2, status, 2, "Caesar".into()).is_none());
            assert!(alliance_request_notification(1, 2, status, 0, "Caesar".into()).is_none());
        }
    }

    #[test]
    fn submitted_notice_is_suppressed_only_for_a_new_alliance() {
        assert!(suppress_submitted_notice_if_formed(
            AllianceRequestStatus::Submitted,
            false,
            true,
        ));
        assert!(!suppress_submitted_notice_if_formed(
            AllianceRequestStatus::Submitted,
            true,
            true,
        ));
        assert!(!suppress_submitted_notice_if_formed(
            AllianceRequestStatus::Rejected,
            false,
            true,
        ));
    }
}

#[cfg(test)]
mod hero_ability_visibility_tests {
    use super::*;
    use sow_core::engine::SowEngine;
    use sow_core::game::{GameEvent, GamePhase, GameState};
    use sow_core::game_config::GameConfig;
    use sow_core::player::{Leader, Player};
    use sow_core::water_components::WaterComponents;

    fn app_and_snapshot() -> (SowApp, SimSnapshot) {
        let config = GameConfig::default();
        let mut state = GameState::new(7, 4, 4, config.clone());
        state.phase = GamePhase::Playing;
        for (id, leader) in [
            (1, Leader::of("burebista")),
            (2, Leader::of("caesar")),
            (3, Leader::of("cleopatra")),
        ] {
            let mut player = Player::new_human(id, format!("P{id}"), [0.5; 3], &config);
            player.has_spawned = true;
            player.leader = leader;
            state.register_player(player);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        (SowApp::new(), engine.build_snapshot())
    }

    fn cues_for(my_id: u16) -> (usize, usize) {
        let (mut app, snap) = app_and_snapshot();
        app.process_tick_events(
            vec![GameEvent::AbilityTriggered {
                owner_id: 1,
                tile: Some((1, 1)),
                affected: vec![2],
            }],
            &snap,
            my_id,
        );
        (
            app.ui.transport_impacts.len(),
            app.ui.app.hud_state.hud_notifications.len(),
        )
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn hero_ability_cue_is_private_to_its_owner_and_affected_players() {
        assert_eq!(cues_for(1), (1, 1), "the owner sees the ring and the notice");
        assert_eq!(cues_for(2), (1, 1), "the player it hit sees the ring and the notice");
        assert_eq!(cues_for(3), (0, 0), "an uninvolved player sees nothing");
        assert_eq!(cues_for(0), (0, 0), "a spectator sees nothing");
    }
}
