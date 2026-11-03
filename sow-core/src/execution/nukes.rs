use crate::engine::SowEngine;
use crate::game::{GamePhase, ProjectileKind, TerritoryChange};

/// Silo cooldown in ticks (90 ticks ≈ 9 seconds at 100ms/tick).
pub const SILO_COOLDOWN_TICKS: u32 = 90;

impl SowEngine {
    pub fn execute_projectiles(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // Tick down silo cooldowns
        self.silo_cooldowns.values_mut().for_each(|cd| {
            if *cd > 0 {
                *cd -= 1;
            }
        });
        self.silo_cooldowns.retain(|_, cd| *cd > 0); // Advance all active projectiles
        let mut detonations = Vec::new();
        let mut shell_impacts = Vec::new();
        let fleet_targets: std::collections::HashMap<_, _> = self
            .fleets
            .iter()
            .filter(|fleet| fleet.troops > 0.0)
            .map(|fleet| (fleet.id, (fleet.owner_id, fleet.unit_type, fleet.current_tile)))
            .collect();

        for proj in &mut self.projectiles {
            if !proj.active {
                continue;
            }

            if proj.kind == ProjectileKind::Shell {
                let Some(target_id) = proj.target_fleet_id else {
                    proj.active = false;
                    continue;
                };
                let Some(&(target_owner, target_type, target_tile)) = fleet_targets.get(&target_id)
                else {
                    proj.active = false;
                    continue;
                };
                if !matches!(
                    target_type,
                    crate::game::UnitType::TransportShip | crate::game::UnitType::Warship
                ) || naval_players_friendly(&self.state, proj.owner_id, target_owner)
                    || self.state.tick >= proj.expires_at_tick
                {
                    proj.active = false;
                    continue;
                }
                let current_tile = proj
                    .path
                    .get(proj.path_cursor.min(proj.path.len().saturating_sub(1)))
                    .copied()
                    .unwrap_or(proj.src_tile);
                proj.dst_tile = target_tile;
                proj.path = crate::pathfinding::bresenham_line(
                    current_tile,
                    target_tile,
                    self.state.map.width,
                );
                proj.path_cursor = (proj.steps_per_tick as usize).min(proj.path.len());
                if proj.path_cursor >= proj.path.len() {
                    proj.active = false;
                    shell_impacts.push((
                        proj.owner_id,
                        proj.source_fleet_id,
                        target_id,
                        target_owner,
                        target_type,
                        target_tile,
                        proj.damage,
                    ));
                }
                continue;
            }

            proj.path_cursor += proj.steps_per_tick as usize;

            if proj.path_cursor >= proj.path.len() {
                proj.path_cursor = proj.path.len().saturating_sub(1);
                proj.active = false;
                let dst_x = proj.dst_tile % self.state.map.width;
                let dst_y = proj.dst_tile / self.state.map.width;
                match proj.kind {
                    ProjectileKind::Nuke { level } => {
                        let inner = crate::game::nuke_inner_radius(level);
                        let outer = crate::game::nuke_outer_radius(level);
                        detonations.push((dst_x, dst_y, inner, outer, proj.owner_id));
                    }
                    ProjectileKind::SAMMissile => {
                        // SAM missiles delete their target on contact — handled in execute_sam
                    }
                    ProjectileKind::Shell => unreachable!("shells resolve before map projectiles"),
                }
            }
        }

        // Cleanup inactive projectiles
        self.projectiles.retain(|p| p.active);

        for (owner_id, source_id, target_id, target_owner, target_type, tile, damage) in
            shell_impacts
        {
            if !self.naval_target_allowed(owner_id, target_owner) {
                continue;
            }
            let Some(target_index) = self
                .fleets
                .iter()
                .position(|fleet| fleet.id == target_id && fleet.troops > 0.0)
            else {
                continue;
            };
            let sunk = {
                let target = &mut self.fleets[target_index];
                target.troops = (target.troops - damage.max(0.0)).max(0.0);
                if target.troops <= 0.0 {
                    target.retreating = false;
                    if let Some(warship) = target.warship.as_mut() {
                        warship.target_fleet = None;
                    }
                }
                target.troops <= 0.0
            };
            if sunk && let Some(source_id) = source_id {
                if let Some(source) = self.fleets.iter_mut().find(|fleet| fleet.id == source_id) {
                    if let Some(warship) = source.warship.as_mut() {
                        warship.record_progress(if target_type
                            == crate::game::UnitType::Warship
                        {
                            250
                        } else {
                            25
                        });
                    }
                }
            }
            self.state.events.push(crate::game::GameEvent::NavalImpact {
                owner_id,
                target_owner,
                tile,
                target_type,
                sunk,
            });
        }

        // Process detonations
        let mut territory_changes = Vec::new();
        for (dx, dy, inner, outer, owner_id) in detonations {
            territory_changes.extend(self.detonate_nuke(dx, dy, inner, outer, owner_id));
        }
        let elimination_candidates = self.annex_enclosed_components(territory_changes);
        for (victim, conqueror, x, y) in elimination_candidates {
            self.eliminate_player(victim, conqueror, x, y, false);
        }
    }

    fn detonate_nuke(
        &mut self,
        cx: u32,
        cy: u32,
        inner: u32,
        outer: u32,
        owner_id: u16,
    ) -> Vec<TerritoryChange> {
        let w = self.state.map.width;
        let h = self.state.map.height;
        let outer_sq = (outer * outer) as i64;
        let inner_sq = (inner * inner) as i64;
        let cx_i = cx as i32;
        let cy_i = cy as i32;

        let x_min = (cx_i - outer as i32).max(0) as u32;
        let x_max = (cx_i + outer as i32).min(w as i32 - 1) as u32;
        let y_min = (cy_i - outer as i32).max(0) as u32;
        let y_max = (cy_i + outer as i32).min(h as i32 - 1) as u32;

        let mut troops_damage: Vec<(u16, f64)> = Vec::new();
        let mut territory_changes = Vec::new();

        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let dx = x as i64 - cx as i64;
                let dy = y as i64 - cy as i64;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq > outer_sq {
                    continue;
                }

                let tile_owner = self.state.map.owner_id(x, y);

                if dist_sq <= inner_sq {
                    // Inner radius: wipe ownership
                    if tile_owner != 0 {
                        let (_, change) = self.state.set_tile_owner_for_execution(x, y, 0);
                        if let Some(change) = change {
                            territory_changes.push(change);
                        }
                    }
                } else {
                    // Outer radius: troop damage proportional to proximity
                    if tile_owner != 0 && tile_owner != owner_id {
                        let ratio = 1.0 - (dist_sq as f64 / outer_sq as f64);
                        let damage = ratio * 5.0;
                        let entry = troops_damage.iter_mut().find(|(pid, _)| *pid == tile_owner);
                        if let Some(e) = entry {
                            e.1 += damage;
                        } else {
                            troops_damage.push((tile_owner, damage));
                        }
                    }
                }
            }
        }

        // Apply troop damage
        for (pid, dmg) in &troops_damage {
            if let Some(p) = self.state.player_mut(*pid) {
                p.troops = (p.troops - dmg).max(0.0);
            }
        }

        // Destroy buildings in inner radius
        self.buildings.retain(|b| {
            let bx = b.tile_idx % w;
            let by = b.tile_idx / w;
            let dx = bx as i64 - cx as i64;
            let dy = by as i64 - cy as i64;
            dx * dx + dy * dy > inner_sq
        });
        self.building_grid.dirty = true;
        self.building_aggregates_dirty = true;
        self.bot_sam_tiles_cache = None;
        self.defense_grid_dirty = true;
        self.render_defense_dirty = true;
        self.sea_lanes_dirty = true;

        self.state
            .events
            .push(crate::game::GameEvent::NukeDetonated {
                tile_x: cx,
                tile_y: cy,
                inner_radius: inner,
                outer_radius: outer,
                owner_id,
            });

        let mut eliminated = Vec::new();
        for p in &self.state.players {
            if p.alive && p.tile_count == 0 {
                eliminated.push(p.id);
            }
        }
        for victim_id in eliminated {
            self.eliminate_player(victim_id, owner_id, cx, cy, true);
        }
        territory_changes
    }
}

fn naval_players_friendly(state: &crate::game::GameState, first_id: u16, second_id: u16) -> bool {
    let (Some(first), Some(second)) = (state.player(first_id), state.player(second_id)) else {
        return false;
    };
    first.alliances.contains(&second_id)
        || second.alliances.contains(&first_id)
        || (first.team.is_some() && first.team == second.team)
}

#[cfg(test)]
mod tests {
    use super::SowEngine;
    use crate::game::{GameEvent, GamePhase, GameState, Projectile, ProjectileKind};
    use crate::game_config::GameConfig;
    use crate::map::MapTile;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    #[test]
    fn nuke_territory_loss_rechecks_and_annexes_a_sealed_region() {
        const WIDTH: u32 = 100;
        let mut state = GameState::new(WIDTH as u64, WIDTH, WIDTH, GameConfig::default());
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x80));
        state.register_player(Player::new_human(
            1,
            "Captor".into(),
            [1.0; 3],
            &state.config,
        ));
        state.register_player(Player::new_human(
            2,
            "Victim".into(),
            [1.0; 3],
            &state.config,
        ));

        for y in 10..=13 {
            for x in 10..=13 {
                if !((11..=12).contains(&x) && (11..=12).contains(&y)) {
                    state.set_tile_owner(x, y, 1);
                }
            }
        }
        for y in 11..=12 {
            for x in 11..=12 {
                state.set_tile_owner(x, y, 2);
            }
        }
        state.set_tile_owner(80, 80, 2);
        state.map.compute_shorelines();

        let mut engine = SowEngine::new(state, WaterComponents::default());
        let tile = 80 * WIDTH + 80;
        engine.projectiles.push(Projectile {
            id: 1,
            kind: ProjectileKind::Nuke { level: 0 },
            owner_id: 1,
            src_tile: tile,
            dst_tile: tile,
            path: vec![tile],
            path_cursor: 0,
            steps_per_tick: 1,
            active: true,
                    source_fleet_id: None,
                    target_fleet_id: None,
                    damage: 0.0,
                    expires_at_tick: u64::MAX,
        });

        engine.execute_projectiles();

        for y in 11..=12 {
            for x in 11..=12 {
                assert_eq!(engine.state.map.owner_id(x, y), 1);
            }
        }
        let victim = engine.state.player(2).unwrap();
        assert!(!victim.alive);
        assert_eq!(victim.deaths, 1);
        assert_eq!(engine.state.player(1).unwrap().kills, 1);
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::TileCaptured { .. }))
                .count(),
            4
        );
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::PlayerEliminated { player_id: 2, .. }))
                .count(),
            1
        );
    }
}
