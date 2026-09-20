use crate::app::SowApp;

impl SowApp {
    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) {
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
                // Flash enemy borders red
                self.ui
                    .border_flashes
                    .push(crate::app::BorderFlashInstance {
                        player_id: attack.target_owner,
                        start_time: web_time::Instant::now(),
                        max_intensity: 1.0,
                    });
            }
            _ => {}
        }

        if let Some(c) = self.net.client.as_ref() {
            let msg = sow_core::protocol::ClientMessage::Gameplay {
                intent: intent.clone(),
            };
            if let Ok(json) = bincode::serialize(&msg) {
                c.send(json);
            }
        } else {
            self.sim.offline_intents.push(intent);
        }
    }
}
