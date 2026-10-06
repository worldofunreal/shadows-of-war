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
    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) -> bool {
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
    use super::{is_waiting_for_roman_attack, record_attack_launch_notice};
    use crate::campaign::CampaignId;
    use sow_core::protocol::{AttackIntent, GameplayIntent};
    use wasm_bindgen_test::wasm_bindgen_test;

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
