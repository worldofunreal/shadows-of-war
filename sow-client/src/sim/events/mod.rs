mod combat;
mod elimination;

use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

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
                _ => {}
            }
        }
        self.sfx.flush_tick(now_instant);
        turn_defeats
    }
}
