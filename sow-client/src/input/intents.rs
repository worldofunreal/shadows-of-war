use crate::app::SowApp;

fn is_waiting_for_roman_attack(
    waiting: bool,
    campaign: crate::campaign::CampaignId,
    intent: &sow_core::protocol::GameplayIntent,
    faction_id: Option<&str>,
) -> bool {
    waiting
        && campaign == crate::campaign::CampaignId::Boudica
        && matches!(intent, sow_core::protocol::GameplayIntent::Attack(_))
        && faction_id == Some("roman_outpost")
}

impl SowApp {
    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) {
        if self.ui.tutorial_waiting_for_first_attack {
            let target_faction = match &intent {
                sow_core::protocol::GameplayIntent::Attack(attack) => self
                    .sim
                    .engine
                    .as_ref()
                    .and_then(|engine| engine.campaign_faction_ids.get(&attack.target_owner))
                    .map(String::as_str),
                _ => None,
            };
            if !is_waiting_for_roman_attack(
                self.ui.tutorial_waiting_for_first_attack,
                self.ui.tutorial_campaign,
                &intent,
                target_faction,
            ) {
                return;
            }
            self.sim.offline_intents.clear();
            self.ui.tutorial_waiting_for_first_attack = false;
            self.sim.paused = false;
            self.sim.offline_tick_timer = 0.0;
        }
        if self.ui.tutorial_camera_only
            && !matches!(&intent, sow_core::protocol::GameplayIntent::Resign)
        {
            return;
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

#[cfg(test)]
mod tests {
    use super::is_waiting_for_roman_attack;
    use crate::campaign::CampaignId;
    use sow_core::protocol::{AttackIntent, GameplayIntent};

    #[test]
    fn only_the_first_boudica_attack_on_roman_outpost_unpauses() {
        let attack = GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(100.0),
        });
        assert!(is_waiting_for_roman_attack(
            true,
            CampaignId::Boudica,
            &attack,
            Some("roman_outpost")
        ));
        assert!(!is_waiting_for_roman_attack(
            true,
            CampaignId::Boudica,
            &attack,
            Some("stonea")
        ));
        assert!(!is_waiting_for_roman_attack(
            true,
            CampaignId::SixSkyEp1,
            &attack,
            Some("roman_outpost")
        ));
        assert!(!is_waiting_for_roman_attack(
            false,
            CampaignId::Boudica,
            &attack,
            Some("roman_outpost")
        ));
        assert!(!is_waiting_for_roman_attack(
            true,
            CampaignId::Boudica,
            &GameplayIntent::Resign,
            Some("roman_outpost")
        ));
    }
}
