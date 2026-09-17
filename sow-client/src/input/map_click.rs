use super::placement::resolve_build_target_tile;
use crate::app::SowApp;

impl SowApp {
    pub(crate) fn handle_map_click(&mut self, x: f64, y: f64) {
        if self.ui.observing {
            self.ui.app.hud_state.selected_building_kind = None;
            self.ui.app.hud_state.selected_nuke_kind = None;
            self.input.hold_build_active = false;
            self.input.hold_build_accum = 0.0;
            return;
        }
        let phase = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| &s.phase)
            .unwrap_or(&sow_core::game::GamePhase::Lobby);

        let (col, row) = match self.mouse_to_tile(x, y) {
            Some(res) => res,
            None => return,
        };

        if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
            let idx = (row * self.sim.map_w as i32 + col) as usize;
            let terrain_byte = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.terrain[idx])
                .unwrap_or(0);
            let is_land = (terrain_byte & 0x80) != 0;

            if !is_land {
                let wx = col as f32 + 0.5;
                let wy = row as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });

                return;
            }

            let owner = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners[idx])
                .unwrap_or(0);

            let mut target_col = col;
            let mut target_row = row;

            if owner != 0 {
                let mut best_tile = None;
                let mut best_dist = i32::MAX;
                let search_radius = 5;

                for dy in -search_radius..=search_radius {
                    for dx in -search_radius..=search_radius {
                        let tx = col + dx;
                        let ty = row + dy;
                        if tx >= 0
                            && tx < self.sim.map_w as i32
                            && ty >= 0
                            && ty < self.sim.map_h as i32
                        {
                            let dist = sow_core::building::hex_distance(col, row, tx, ty);
                            if dist <= search_radius {
                                let n_idx = (ty * self.sim.map_w as i32 + tx) as usize;
                                let n_owner = self
                                    .gfx
                                    .map_renderer
                                    .as_ref()
                                    .map(|mr| mr.owners[n_idx])
                                    .unwrap_or(0);
                                let n_terrain = self
                                    .gfx
                                    .map_renderer
                                    .as_ref()
                                    .map(|mr| mr.terrain[n_idx])
                                    .unwrap_or(0);
                                let n_is_land = (n_terrain & 0x80) != 0;

                                if n_owner == 0 && n_is_land && dist < best_dist {
                                    best_dist = dist;
                                    best_tile = Some((tx, ty));
                                }
                            }
                        }
                    }
                }

                if let Some((bx, by)) = best_tile {
                    target_col = bx;
                    target_row = by;
                } else {
                    let wx = col as f32 + 0.5;
                    let wy = row as f32 + 0.5;
                    self.ui.click_markers.push(crate::app::ClickMarker {
                        world_x: wx,
                        world_y: wy,
                        start_time: web_time::Instant::now(),
                    });
                    return;
                }
            }

            let intent = sow_core::protocol::GameplayIntent::Spawn {
                x: target_col as u32,
                y: target_row as u32,
            };
            self.send_intent(intent);
        } else if let Some(nuke_kind) = self.ui.app.hud_state.selected_nuke_kind {
            let tile_idx = (row * self.sim.map_w as i32 + col) as u32;
            let intent = sow_core::protocol::GameplayIntent::LaunchNuke {
                kind: nuke_kind,
                target_tile: tile_idx,
            };
            self.send_intent(intent);
            self.ui.app.hud_state.selected_nuke_kind = None;
        } else if let Some(kind) = self.ui.app.hud_state.selected_building_kind {
            if let Some(snap) = &self.sim.current_snapshot {
                let my_id = self.sim.my_player_id.unwrap_or(0);
                let owners = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners.as_slice())
                    .unwrap_or(&[]);
                let terrain = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.terrain.as_slice())
                    .unwrap_or(&[]);

                let target_res = resolve_build_target_tile(&super::placement::PlacementQuery {
                    kind,
                    click_x: col,
                    click_y: row,
                    map_w: self.sim.map_w,
                    map_h: self.sim.map_h,
                    owners,
                    terrain,
                    my_id,
                    buildings: &snap.buildings,
                });

                let cost = {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    self.ui.app.hud_state.building_costs[i]
                };

                if self.ui.app.hud_state.gold < cost {
                    return;
                }
                let Ok(target_tile) = target_res else { return; };
                let intent =
                    sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile };
                self.send_intent(intent);
                self.ui.last_build_confirm_time = Some(web_time::Instant::now());
            }
        } else {
            // Check if we clicked on a Warship we own
            let mut clicked_warships = Vec::new();
            if let Some(snap) = &self.sim.current_snapshot {
                let my_pid = self.sim.my_player_id.unwrap_or(0);
                let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
                let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;
                for f in &snap.fleets {
                    if f.unit_type == sow_core::game::UnitType::Warship && f.owner_id == my_pid {
                        let col = (f.current_tile % self.sim.map_w) as f32;
                        let row = (f.current_tile / self.sim.map_w) as f32;
                        let wx = col + 0.5;
                        let wy = row + 0.5;
                        // Click tolerance (half a tile)
                        if (wx - world_x).abs() < 0.5 && (wy - world_y).abs() < 0.5 {
                            clicked_warships.push(f.id);
                        }
                    }
                }
            }
            if !clicked_warships.is_empty() {
                self.input.selected_warships = clicked_warships;
            } else {
                self.input.selected_warships.clear();

                let idx = (row * self.sim.map_w as i32 + col) as usize;
                let Some(renderer) = self.gfx.map_renderer.as_ref() else { return; };
                let owner = renderer.owners.get(idx).copied().unwrap_or(0);
                let my_id = self.sim.my_player_id.unwrap_or(0);
                let is_land = renderer
                    .terrain
                    .get(idx)
                    .is_some_and(|terrain| terrain & 0x80 != 0);
                let is_betrayer = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|s| s.players.iter().find(|p| p.id == owner))
                    .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                    .unwrap_or(false);
                let is_allied = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                    .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                    .unwrap_or(false);
                let is_teammate = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .map(|s| {
                        let my_team = s
                            .players
                            .iter()
                            .find(|p| p.id == my_id)
                            .and_then(|p| p.team);
                        let other_team = s
                            .players
                            .iter()
                            .find(|p| p.id == owner)
                            .and_then(|p| p.team);
                        my_team.is_some() && my_team == other_team
                    })
                    .unwrap_or(false);

                if matches!(phase, sow_core::game::GamePhase::Playing)
                    && is_land
                    && owner != 0
                    && owner != my_id
                {
                    if is_allied || is_teammate {
                        self.ui.app.hud_state.show_ask_panel = Some(owner);
                        self.ui.app.hud_state.transfer_confirm_pending = false;
                    } else if shares_land_border(
                        &renderer.owners,
                        &renderer.terrain,
                        self.sim.map_w,
                        self.sim.map_h,
                        my_id,
                        owner,
                    ) {
                        let troops = self.ui.app.hud_state.troops
                            * self.ui.app.hud_state.attack_ratio as f64;
                        self.send_intent(sow_core::protocol::GameplayIntent::Attack(
                            sow_core::protocol::AttackIntent {
                                target_owner: owner,
                                troops: Some(troops),
                            },
                        ));
                    }
                }
            }
        }
    }
}

fn shares_land_border(
    owners: &[u16],
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
    my_id: u16,
    target_owner: u16,
) -> bool {
    if my_id == 0 || target_owner == 0 || my_id == target_owner || map_w == 0 {
        return false;
    }
    let width = map_w as i32;
    let height = map_h as i32;
    let neighbors = [(1, 0), (-1, 0), (0, -1), (0, 1), (1, -1), (-1, -1), (1, 1), (-1, 1)];

    for row in 0..height {
        for col in 0..width {
            let index = (row * width + col) as usize;
            if owners.get(index).copied() != Some(my_id) {
                continue;
            }
            for (dc, dr) in neighbors {
                let neighbor_col = col + dc;
                let neighbor_row = row + dr;
                if neighbor_col < 0
                    || neighbor_col >= width
                    || neighbor_row < 0
                    || neighbor_row >= height
                {
                    continue;
                }
                let neighbor = (neighbor_row * width + neighbor_col) as usize;
                if owners.get(neighbor).copied() == Some(target_owner)
                    && terrain
                        .get(neighbor)
                        .is_some_and(|value| value & 0x80 != 0)
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::shares_land_border;

    #[test]
    fn land_border_is_required_for_a_click_attack() {
        let owners = [1, 2, 0, 0];
        let terrain = [0x80, 0x80, 0x80, 0x80];
        assert!(shares_land_border(&owners, &terrain, 2, 2, 1, 2));
        assert!(!shares_land_border(&owners, &terrain, 2, 2, 1, 3));
    }
}
