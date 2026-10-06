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
                sow_core::game::GameEvent::TransportShipLanded { owner_id, x, y } => {
                    let color = snap
                        .players
                        .iter()
                        .find(|player| player.id == owner_id)
                        .map(|player| {
                            player
                                .team
                                .map_or(player.color, sow_core::player::team_territory_rgb)
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
                            start_time: now_instant,
                        });
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
