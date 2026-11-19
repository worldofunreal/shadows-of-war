use crate::engine::SowEngine;
use crate::game::{GamePhase, ProjectileKind, TerritoryChange};

/// Silo cooldown in ticks (90 ticks ≈ 9 seconds at 100ms/tick).
pub const SILO_COOLDOWN_TICKS: u32 = 90;

impl SowEngine {
    fn release_mirv_warheads(&mut self, carrier: u64, owner: u16, split: u32, target: u32) {
        let width = self.state.map.width;
        let height = self.state.map.height;
        let x = target % width;
        let y = target / width;
        let mut targets = Vec::new();
        for dy in y.saturating_sub(100)..=y.saturating_add(100).min(height - 1) {
            for dx in x.saturating_sub(100)..=x.saturating_add(100).min(width - 1) {
                if dx.abs_diff(x).pow(2) + dy.abs_diff(y).pow(2) > 10_000 {
                    continue;
                }
                let defender = self.state.map.owner_id(dx, dy);
                if defender != 0 && defender != owner && self.naval_target_allowed(owner, defender) {
                    let tile = dy * width + dx;
                    let score = crate::rng::stable_tile_sample(self.state.seed ^ carrier, owner, defender, u64::from(tile), 0);
                    targets.push((score, tile));
                }
            }
        }
        targets.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for (_, tile) in targets.into_iter().take(350) {
            let id = self.state.next_projectile_id;
            self.state.next_projectile_id = id.wrapping_add(1).max(1);
            self.projectiles.push(crate::game::Projectile {
                id,
                kind: ProjectileKind::Nuke { level: 4 },
                owner_id: owner,
                src_tile: split,
                dst_tile: tile,
                path: std::sync::Arc::new(crate::pathfinding::bresenham_line(split, tile, width)),
                path_cursor: 0,
                steps_per_tick: 2,
                active: true,
                source_fleet_id: None,
                target_fleet_id: None,
                damage: 0.0,
                expires_at_tick: u64::MAX,
            });
        }
    }

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
        let tick = self.state.tick;
        self.nuclear_reload.retain(|_, ready| {
            ready.retain(|&deadline| deadline > tick);
            !ready.is_empty()
        });
        let mut separations = Vec::new();
        let mut detonations = Vec::new();
        let mut shell_impacts = Vec::new();
        let fleet_targets: std::collections::HashMap<_, _> = self
            .fleets
            .iter()
            .filter(|fleet| fleet.troops > 0.0)
            .map(|fleet| {
                (
                    fleet.id,
                    (fleet.owner_id, fleet.unit_type, fleet.current_tile),
                )
            })
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
                proj.path = std::sync::Arc::new(crate::pathfinding::bresenham_line(
                    current_tile,
                    target_tile,
                    self.state.map.width,
                ));
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
            if proj.kind == ProjectileKind::SAMMissile {
                proj.path_cursor = proj.path_cursor.min(proj.path.len().saturating_sub(1));
                continue;
            }

            if proj.kind == (ProjectileKind::Nuke { level: 3 })
                && proj.path_cursor >= proj.path.len() / 2
            {
                let split = proj.path.get(proj.path_cursor).copied().unwrap_or(proj.dst_tile);
                separations.push((proj.id, proj.owner_id, split, proj.dst_tile));
                proj.active = false;
                continue;
            }

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
        for (id, owner, split, target) in separations {
            self.release_mirv_warheads(id, owner, split, target);
        }

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
            // Earlier impacts in this batch may have taken this ship as a prize.
            // Recheck its current owner before applying another queued shell.
            let target_owner = self.fleets[target_index].owner_id;
            if !self.naval_target_allowed(owner_id, target_owner) {
                continue;
            }
            let (health_before, troops_lost, sunk) = {
                let target = &mut self.fleets[target_index];
                let before = target.troops;
                let cargo_lost = if target_type == crate::game::UnitType::TransportShip {
                    before
                } else {
                    0.0
                };
                target.troops = if target_type == crate::game::UnitType::TransportShip {
                    0.0
                } else {
                    (before - damage.max(0.0)).max(0.0)
                };
                if target.troops <= 0.0 {
                    target.retreating = false;
                    if let Some(warship) = target.warship.as_mut() {
                        warship.target_fleet = None;
                    }
                }
                (before, cargo_lost, target.troops <= 0.0)
            };
            if sunk && let Some(source_id) = source_id {
                if let Some(source) = self.fleets.iter_mut().find(|fleet| fleet.id == source_id) {
                    if let Some(warship) = source.warship.as_mut() {
                        warship.record_progress(if target_type == crate::game::UnitType::Warship {
                            250
                        } else {
                            25
                        });
                    }
                }
            }
            let captured = sunk && self.hero_warship_prize(owner_id, target_index);
            let health_after = self.fleets[target_index].troops;
            self.state.events.push(crate::game::GameEvent::NavalImpact {
                owner_id,
                target_owner,
                source_fleet_id: source_id,
                target_fleet_id: target_id,
                tile,
                target_type,
                damage,
                health_before,
                health_after,
                troops_lost,
                sunk: sunk && !captured,
                captured,
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
        let mut last_outer_owner: u16 = 0;
        let mut last_outer_idx: usize = 0;
        let mut territory_changes = Vec::new();
        let mut wiped_any_tile = false;

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
                        wiped_any_tile = true;
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
                        if tile_owner == last_outer_owner {
                            troops_damage[last_outer_idx].1 += damage;
                        } else if let Some(pos) =
                            troops_damage.iter().position(|(pid, _)| *pid == tile_owner)
                        {
                            troops_damage[pos].1 += damage;
                            last_outer_owner = tile_owner;
                            last_outer_idx = pos;
                        } else {
                            last_outer_owner = tile_owner;
                            last_outer_idx = troops_damage.len();
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

        // Destroy buildings in inner radius in O(B), removing only destroyed keys from O(1) maps.
        let mut destroyed_port = false;
        let mut destroyed_ids: Vec<u64> = Vec::new();
        self.buildings.retain(|b| {
            let bx = b.tile_idx % w;
            let by = b.tile_idx / w;
            let dx = bx as i64 - cx as i64;
            let dy = by as i64 - cy as i64;
            let keep = dx * dx + dy * dy > inner_sq;
            if !keep {
                destroyed_ids.push(b.id);
                if b.kind == crate::game::BuildingKind::Port {
                    destroyed_port = true;
                }
            }
            keep
        });
        for id in destroyed_ids {
            self.building_productions.remove(&id);
            self.silo_cooldowns.remove(&id);
            self.nuclear_reload.remove(&id);
            self.upgrade_targets.remove(&id);
        }
        self.building_grid.dirty = true;
        self.building_aggregates_dirty = true;
        self.bot_sam_tiles_cache = None;
        self.defense_grid_dirty = true;
        self.render_defense_dirty = true;
        if destroyed_port {
            self.sea_lanes_dirty = true;
        }

        self.state
            .events
            .push(crate::game::GameEvent::NukeDetonated {
                tile_x: cx,
                tile_y: cy,
                inner_radius: inner,
                outer_radius: outer,
                owner_id,
            });

        if wiped_any_tile {
            let mut eliminated = Vec::new();
            for p in &self.state.players {
                if p.alive && p.tile_count == 0 {
                    eliminated.push(p.id);
                }
            }
            for victim_id in eliminated {
                self.eliminate_player(victim_id, owner_id, cx, cy, true);
            }
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
            path: std::sync::Arc::new(vec![tile]),
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

    #[test]
    fn detonate_nuke_cleans_productions_and_silo_cooldowns_for_destroyed_buildings_only() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 64, 64, config.clone());
        state.phase = GamePhase::Playing;
        let mut p1 = Player::new_human(1, "Attacker".into(), [1.0, 0.0, 0.0], &config);
        p1.alive = true;
        p1.tile_count = 10;
        let mut p2 = Player::new_human(2, "Defender".into(), [0.0, 0.0, 1.0], &config);
        p2.alive = true;
        p2.tile_count = 10;
        state.register_player(p1);
        state.register_player(p2);
        for i in 0..64 * 64 {
            state.map.terrain[i] = MapTile::from_byte(0b1000_0000);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        let inside_tile = 20 * 64 + 20;
        let outside_tile = 60 * 64 + 60;
        engine.buildings.push(crate::building::Building {
            id: 101,
            tile_idx: inside_tile,
            owner_id: 2,
            kind: crate::game::BuildingKind::Factory,
            level: 4,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.buildings.push(crate::building::Building {
            id: 202,
            tile_idx: outside_tile,
            owner_id: 2,
            kind: crate::game::BuildingKind::SAM,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine
            .building_productions
            .insert(101, crate::building::CityArsenalState { nuke_ready: true, ..Default::default() });
        engine
            .building_productions
            .insert(202, crate::building::CityArsenalState { nuke_ready: true, ..Default::default() });
        engine.silo_cooldowns.insert(101, 45);
        engine.silo_cooldowns.insert(202, 45);

        engine.projectiles.push(Projectile {
            id: 1,
            kind: ProjectileKind::Nuke { level: 1 },
            owner_id: 1,
            src_tile: inside_tile,
            dst_tile: inside_tile,
            path: std::sync::Arc::new(vec![inside_tile]),
            path_cursor: 0,
            steps_per_tick: 1,
            active: true,
            source_fleet_id: None,
            target_fleet_id: None,
            damage: 0.0,
            expires_at_tick: u64::MAX,
        });

        engine.execute_projectiles();

        assert!(!engine.buildings.iter().any(|b| b.id == 101));
        assert!(engine.buildings.iter().any(|b| b.id == 202));
        assert!(!engine.building_productions.contains_key(&101));
        assert!(engine.building_productions.contains_key(&202));
        assert!(!engine.silo_cooldowns.contains_key(&101));
        assert_eq!(engine.silo_cooldowns.get(&202).copied(), Some(44));
    }
}
