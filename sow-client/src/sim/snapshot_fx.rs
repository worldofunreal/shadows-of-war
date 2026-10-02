use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;
use std::collections::{HashMap, HashSet};

impl SowApp {
    pub(crate) fn apply_snapshot_fx(&mut self, snap: &mut SimSnapshot, my_id: u16) {
        let mut being_attacked_triggered = false;
        let mut under_attack_sound_triggered = false;
        let now = web_time::Instant::now();
        if let Some(mut existing) = self.sim.current_snapshot.take() {
            for [a, b] in new_mutual_alliances(&existing.players, &snap.players) {
                self.ui.trigger_alliance_celebration(a, b, now);
            }
            let old_attack_troops: HashMap<u64, f64> = existing
                .attacks
                .iter()
                .map(|attack| (attack.id, attack.troops))
                .collect();
            let old_buildings: HashMap<u64, bool> = existing
                .buildings
                .iter()
                .map(|building| (building.id, building.under_construction))
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
            let mut notifications = Vec::new();
            if my_id != 0 {
                if let Some(attacker_id) = new_attackers.difference(&old_attackers).copied().min() {
                    notifications.push((
                        crate::ui::UiText::new("hud.attack_incoming")
                            .with("count", new_attackers.len().to_string()),
                        [Some(attacker_id), Some(my_id)],
                        4,
                        format!("incoming-attack:{my_id}"),
                        false,
                    ));
                }
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
                            if let Some(requester) = snap.players.iter().find(|p| p.id == *req) {
                                notifications.push((
                                    crate::ui::UiText::new("hud.alliance_request").with(
                                        "name",
                                        sow_core::player::display_name(
                                            requester.id,
                                            &requester.name,
                                            requester.player_type,
                                        ),
                                    ),
                                    [Some(*req), Some(my_id)],
                                    3,
                                    format!("alliance-request:{req}"),
                                    false,
                                ));
                            }
                        }
                    }
                    for request in &my_info_new.resource_requests {
                        if !my_info_old
                            .resource_requests
                            .iter()
                            .any(|old| old.requester == request.requester)
                            && let Some(requester) =
                                snap.players.iter().find(|p| p.id == request.requester)
                        {
                            notifications.push((
                                crate::ui::UiText::new("hud.resource_request")
                                    .with(
                                        "name",
                                        sow_core::player::display_name(
                                            requester.id,
                                            &requester.name,
                                            requester.player_type,
                                        ),
                                    )
                                    .with("gold", crate::utils::format_number(request.gold))
                                    .with("troops", crate::utils::format_number(request.troops)),
                                [Some(request.requester), Some(my_id)],
                                3,
                                format!("resource-request:{}", request.requester),
                                false,
                            ));
                        }
                    }
                    for ally_id in &my_info_new.alliances {
                        let Some(old_ally) = my_info_old.alliances.iter().find(|id| *id == ally_id)
                        else {
                            if let Some(ally) = snap.players.iter().find(|p| p.id == *ally_id) {
                                notifications.push((
                                    crate::ui::UiText::new("hud.alliance_formed").with(
                                        "name",
                                        sow_core::player::display_name(
                                            ally.id,
                                            &ally.name,
                                            ally.player_type,
                                        ),
                                    ),
                                    [Some(my_id), Some(*ally_id)],
                                    3,
                                    format!("alliance-formed:{ally_id}"),
                                    false,
                                ));
                            }
                            continue;
                        };
                        let old_timer = my_info_old
                            .alliance_timers
                            .get(old_ally)
                            .copied()
                            .unwrap_or_default();
                        let new_timer = my_info_new
                            .alliance_timers
                            .get(ally_id)
                            .copied()
                            .unwrap_or_default();
                        if old_timer <= 300
                            && new_timer > old_timer
                            && new_timer > 300
                            && let Some(ally) = snap.players.iter().find(|p| p.id == *ally_id)
                        {
                            notifications.push((
                                crate::ui::UiText::new("hud.alliance_renewed").with(
                                    "name",
                                    sow_core::player::display_name(
                                        ally.id,
                                        &ally.name,
                                        ally.player_type,
                                    ),
                                ),
                                [Some(my_id), Some(*ally_id)],
                                3,
                                format!("alliance-renewed:{ally_id}"),
                                false,
                            ));
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
                            notifications.push((
                                crate::ui::UiText::new("hud.betrayal").with(
                                    "name",
                                    sow_core::player::display_name(
                                        other_player.id,
                                        &other_player.name,
                                        other_player.player_type,
                                    ),
                                ),
                                [Some(*ally_id), Some(my_id)],
                                4,
                                format!("betrayal:{ally_id}"),
                                false,
                            ));
                        } else if !my_info_new.alliances.contains(ally_id)
                            && let Some(other_player) =
                                snap.players.iter().find(|p| p.id == *ally_id)
                        {
                            notifications.push((
                                crate::ui::UiText::new("hud.alliance_ended").with(
                                    "name",
                                    sow_core::player::display_name(
                                        other_player.id,
                                        &other_player.name,
                                        other_player.player_type,
                                    ),
                                ),
                                [Some(*ally_id), Some(my_id)],
                                3,
                                format!("alliance-ended:{ally_id}"),
                                false,
                            ));
                        }
                    }
                }
            }
            for (text, players, priority, group, sum_values) in notifications {
                self.ui.app.hud_state.push_notification_for_players(
                    text,
                    players,
                    priority,
                    Some(group),
                    sum_values,
                );
            }
            // Count unique attackers targeting us in the new snapshot
            let unique_attackers = new_attackers.len();
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

            // Play completion feedback for local structures.
            for b_new in &snap.buildings {
                if let Some(old_under_construction) = old_buildings.get(&b_new.id) {
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
            if !nuke_alert_targets_player(alert.owner_id, victim_id, my_id) {
                continue;
            }
            let victim_name = if victim_id == 0 {
                format!("({}, {})", alert.tile_x, alert.tile_y)
            } else {
                snap.players
                    .iter()
                    .find(|p| p.id == victim_id)
                    .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                    .unwrap_or_else(|| format!("Player {}", victim_id))
            };

            let text = crate::ui::UiText::new("hud.nuke_struck")
                .with("attacker", attacker_name)
                .with("victim", victim_name);
            let priority = if victim_id == my_id && my_id != 0 {
                4
            } else if alert.owner_id == my_id {
                3
            } else {
                2
            };
            self.ui.app.hud_state.push_notification_for_players(
                text,
                [Some(alert.owner_id), (victim_id != 0).then_some(victim_id)],
                priority,
                Some(format!("nuke:{}:{victim_id}", alert.owner_id)),
                false,
            );
        }
    }
}

fn new_mutual_alliances(
    old: &[sow_core::protocol::PlayerSnapshot],
    new: &[sow_core::protocol::PlayerSnapshot],
) -> Vec<[u16; 2]> {
    let old_by_id: HashMap<u16, &sow_core::protocol::PlayerSnapshot> =
        old.iter().map(|player| (player.id, player)).collect();
    let new_by_id: HashMap<u16, &sow_core::protocol::PlayerSnapshot> =
        new.iter().map(|player| (player.id, player)).collect();
    let mut pairs = Vec::new();
    for player in new {
        if player.id == 0 {
            continue;
        }
        for &other_id in &player.alliances {
            if other_id == 0
                || player.id >= other_id
                || !new_by_id
                    .get(&other_id)
                    .is_some_and(|other| other.alliances.contains(&player.id))
            {
                continue;
            }
            let was_mutual = old_by_id
                .get(&player.id)
                .is_some_and(|previous| previous.alliances.contains(&other_id))
                && old_by_id
                    .get(&other_id)
                    .is_some_and(|previous| previous.alliances.contains(&player.id));
            if !was_mutual {
                pairs.push([player.id, other_id]);
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

#[cfg(test)]
mod tests {
    use super::new_mutual_alliances;
    use sow_core::player::{Civilization, Leader, PlayerType};
    use sow_core::protocol::PlayerSnapshot;

    fn player(id: u16, alliances: &[u16]) -> PlayerSnapshot {
        PlayerSnapshot {
            id,
            name: format!("Player {id}"),
            troops: 100.0,
            max_troops: 100.0,
            gold: 100.0,
            tile_count: 1,
            centroid_x: 0.0,
            centroid_y: 0.0,
            player_type: PlayerType::Bot,
            color: [0.5; 3],
            team: None,
            has_spawned: true,
            alive: true,
            iq: 100,
            alliances: alliances.to_vec(),
            alliance_timers: Default::default(),
            alliance_requests: Vec::new(),
            resource_requests: Vec::new(),
            disconnected: false,
            active_emoji: None,
            traitor: false,
            civilization: Civilization::Rome,
            leader: Leader::Caesar,
            campaign_avatar: None,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            boats_in_use: 0,
            boat_capacity: 1,
        }
    }

    #[test]
    fn detects_only_new_mutual_alliances() {
        let old = vec![player(1, &[]), player(2, &[]), player(3, &[])];
        let new = vec![player(1, &[2]), player(2, &[1]), player(3, &[])];
        assert_eq!(new_mutual_alliances(&old, &new), vec![[1, 2]]);
    }

    #[test]
    fn ignores_requests_unilateral_alliances_and_existing_alliances() {
        let old = vec![player(1, &[2]), player(2, &[1]), player(3, &[])];
        let new = vec![player(1, &[2, 3]), player(2, &[1]), player(3, &[])];
        assert!(new_mutual_alliances(&old, &new).is_empty());
    }

    #[test]
    fn ignores_requests_and_alliance_renewals() {
        let old = vec![player(1, &[]), player(2, &[])];
        let mut one = player(1, &[]);
        one.alliance_requests.push(2);
        let mut two = player(2, &[]);
        two.alliance_requests.push(1);
        assert!(new_mutual_alliances(&old, &[one, two]).is_empty());

        let mut old_one = player(1, &[2]);
        old_one.alliance_timers.insert(2, 300);
        let mut old_two = player(2, &[1]);
        old_two.alliance_timers.insert(1, 300);
        let mut new_one = player(1, &[2]);
        new_one.alliance_timers.insert(2, 600);
        let mut new_two = player(2, &[1]);
        new_two.alliance_timers.insert(1, 600);
        assert!(new_mutual_alliances(&[old_one, old_two], &[new_one, new_two]).is_empty());
    }
}

fn nuke_alert_targets_player(attacker_id: u16, victim_id: u16, my_id: u16) -> bool {
    my_id != 0 && (attacker_id == my_id || victim_id == my_id)
}

#[cfg(test)]
mod tests {
    use super::nuke_alert_targets_player;

    #[test]
    fn nuke_alerts_reach_only_the_attacker_or_affected_player() {
        assert!(nuke_alert_targets_player(4, 7, 4));
        assert!(nuke_alert_targets_player(4, 7, 7));
        assert!(!nuke_alert_targets_player(4, 7, 8));
        assert!(!nuke_alert_targets_player(4, 0, 0));
        assert!(!nuke_alert_targets_player(4, 0, 9));
    }
}
