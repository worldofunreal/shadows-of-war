use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;
use std::collections::{HashMap, HashSet};

impl SowApp {
    pub(crate) fn apply_snapshot_fx(&mut self, snap: &mut SimSnapshot, my_id: u16) {
        let mut being_attacked_triggered = false;
        let mut under_attack_sound_triggered = false;
        if let Some(mut existing) = self.sim.current_snapshot.take() {
            let old_attack_troops: HashMap<u64, f64> = existing
                .attacks
                .iter()
                .map(|attack| (attack.id, attack.troops))
                .collect();
            let old_buildings: HashMap<u64, (u8, bool)> = existing
                .buildings
                .iter()
                .map(|building| (building.id, (building.level, building.under_construction)))
                .collect();
            let old_attackers: HashSet<u16> = existing
                .attacks
                .iter()
                .filter(|attack| attack.target_owner == my_id && attack.troops > 0.0)
                .map(|attack| attack.owner_id)
                .collect();
            let new_attackers: HashSet<u16> = snap
                .attacks
                .iter()
                .filter(|attack| attack.target_owner == my_id && attack.troops > 0.0)
                .map(|attack| attack.owner_id)
                .collect();
            if my_id != 0 {
                // 1. Detect incoming attacks (UnderAttack)
                for attack in &snap.attacks {
                    if attack.target_owner == my_id && attack.troops > 0.0 {
                        let is_new_or_increased = old_attack_troops
                            .get(&attack.id)
                            .is_none_or(|old_troops| attack.troops > *old_troops);
                        if is_new_or_increased {
                            being_attacked_triggered = true;
                        }
                    }
                }
                under_attack_sound_triggered = new_attackers
                    .iter()
                    .any(|attacker| !old_attackers.contains(attacker));

                // 2. Detect new alliance requests targeting us
                if let Some(my_info_new) = snap.players.iter().find(|p| p.id == my_id)
                    && let Some(my_info_old) = existing.players.iter().find(|p| p.id == my_id)
                {
                    for req in &my_info_new.alliance_requests {
                        if !my_info_old.alliance_requests.contains(req) {
                            self.ui.trigger_viewport_alert(
                                crate::app::ViewportAlertKind::AllianceRequest,
                            );
                        }
                    }
                }

                // 3. Detect betrayals (ally breaks alliance and is marked traitor)
                if let Some(my_info_new) = snap.players.iter().find(|p| p.id == my_id)
                    && let Some(my_info_old) = existing.players.iter().find(|p| p.id == my_id)
                {
                    for ally_id in &my_info_old.alliances {
                        if !my_info_new.alliances.contains(ally_id)
                            && let Some(other_player) =
                                snap.players.iter().find(|p| p.id == *ally_id)
                            && (other_player.traitor
                                || other_player.active_emoji.as_deref() == Some("🗡️"))
                        {
                            self.ui
                                .trigger_viewport_alert(crate::app::ViewportAlertKind::Betrayal);
                        }
                    }
                }
            }
            // Count unique attackers targeting us in the new snapshot
            let unique_attackers = new_attackers.len();
            let now = web_time::Instant::now();
            let under_attack_spatial = snap
                .attacks
                .iter()
                .find(|attack| attack.target_owner == my_id && attack.troops > 0.0)
                .map(|attack| self.spatial_sound_params(attack.front_cx, attack.front_cy));

            if unique_attackers > 0 {
                let should_flash = self
                    .ui
                    .last_player_attack_flash_time
                    .get(&my_id)
                    .copied()
                    .map(|last| now.duration_since(last).as_secs_f32() >= 1.0)
                    .unwrap_or(true);

                if should_flash || being_attacked_triggered {
                    // Compose intensity: 1.0 base + 0.2 per attacker, clamped between 1.0 and 1.5
                    let intensity = (1.0 + (unique_attackers as f32 - 1.0) * 0.2).clamp(1.0, 1.5);
                    self.ui
                        .border_flashes
                        .push(crate::app::BorderFlashInstance {
                            player_id: my_id,
                            start_time: now,
                            max_intensity: intensity,
                        });
                    self.ui.last_player_attack_flash_time.insert(my_id, now);
                }
            } else {
                self.ui.last_player_attack_flash_time.remove(&my_id);
            }

            if being_attacked_triggered {
                self.ui
                    .trigger_viewport_alert(crate::app::ViewportAlertKind::UnderAttack);
            }
            self.sfx.update_under_attack(
                now,
                unique_attackers > 0,
                under_attack_sound_triggered,
                under_attack_spatial,
            );

            // Detect building level upgrades and completions
            for b_new in &snap.buildings {
                if let Some((old_level, old_under_construction)) = old_buildings.get(&b_new.id) {
                    if b_new.level > *old_level
                        || (*old_under_construction && !b_new.under_construction)
                    {
                        // ponytail: active_upgrades animations removed as they were dead code
                    }
                    // ponytail: only play building completion sound for the local player
                    if *old_under_construction
                        && !b_new.under_construction
                        && b_new.owner_id == my_id
                        && my_id != 0
                    {
                        let wx = (b_new.tile_idx % self.sim.map_w) as f32 + 0.5;
                        let wy = (b_new.tile_idx / self.sim.map_w) as f32 + 0.5;
                        self.sfx.play_completion(
                            now,
                            crate::building_sound_kind(b_new.kind),
                            self.spatial_sound_params(wx, wy),
                        );
                    }
                }
            }

            if !existing.dirty_tiles.is_empty() {
                existing.dirty_tiles.append(&mut snap.dirty_tiles);
                snap.dirty_tiles = existing.dirty_tiles;
            }
        }
    }

    pub(crate) fn process_nuke_alerts(&mut self, snap: &SimSnapshot) {
        // Process nuke alerts into HUD notifications
        let my_id = self.sim.my_player_id.unwrap_or(0);
        for alert in &snap.nuke_alerts {
            let attacker_name = snap
                .players
                .iter()
                .find(|p| p.id == alert.owner_id)
                .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                .unwrap_or_else(|| format!("Player {}", alert.owner_id));

            // Determine victim from tile ownership in previous snapshot state
            let tile_idx = alert.tile_y * self.sim.map_w + alert.tile_x;
            let victim_id = self
                .gfx
                .map_renderer
                .as_ref()
                .and_then(|mr| mr.owners.get(tile_idx as usize).copied())
                .unwrap_or(0);
            let victim_name = if victim_id == 0 {
                "unclaimed territory".to_string()
            } else {
                snap.players
                    .iter()
                    .find(|p| p.id == victim_id)
                    .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                    .unwrap_or_else(|| format!("Player {}", victim_id))
            };

            let (text, color) = if victim_id == my_id && my_id != 0 {
                (
                    crate::ui::UiText::new("hud.nuke_incoming").with("name", attacker_name),
                    crate::rgb(239, 68, 68),
                )
            } else if alert.owner_id == my_id {
                (
                    crate::ui::UiText::new("hud.nuke_hit").with("name", victim_name),
                    crate::rgb(74, 222, 128),
                )
            } else if my_id != 0
                && snap
                    .players
                    .iter()
                    .find(|p| p.id == my_id)
                    .map(|p| p.alliances.contains(&victim_id))
                    .unwrap_or(false)
                && victim_id != 0
            {
                (
                    crate::ui::UiText::new("hud.nuke_ally_hit")
                        .with("attacker", attacker_name)
                        .with("victim", victim_name),
                    crate::rgb(251, 191, 36),
                )
            } else {
                (
                    crate::ui::UiText::new("hud.nuke_struck")
                        .with("attacker", attacker_name)
                        .with("victim", victim_name),
                    crate::rgb(180, 180, 200),
                )
            };

            self.ui.app.hud_state.push_notification(text, color);
        }
    }
}
