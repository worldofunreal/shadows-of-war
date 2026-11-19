use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, Projectile, ProjectileKind};
use crate::game_config::GameConfig;
use std::cmp::Reverse;
use std::collections::HashSet;

const SAM_STEPS_PER_TICK: u8 = 4;
pub(crate) const SAM_COOLDOWN_TICKS: u32 = 90;

pub fn sam_cooldown_ticks(_level: u8, _cfg: &GameConfig) -> u32 {
    SAM_COOLDOWN_TICKS
}

pub fn sam_salvo_size(level: u8) -> usize {
    usize::from(level)
}

/// Base SAM range constant.
const SAM_MAX_RANGE: f32 = 150.0;

/// SAM effective range: 150 - 480/(level+5)
pub fn sam_range(level: u8) -> f32 {
    SAM_MAX_RANGE - 480.0 / (level as f32 + 5.0)
}

/// Orthogonal Chebyshev distance between two tile indices on a square grid.
pub fn sam_tile_distance(a: u32, b: u32, width: u32) -> u32 {
    if width == 0 {
        return u32::MAX;
    }
    let ax = (a % width) as i32;
    let ay = (a / width) as i32;
    let bx = (b % width) as i32;
    let by = (b / width) as i32;

    let dx = (ax - bx).abs();
    let dy = (ay - by).abs();
    dx.max(dy) as u32
}

pub fn sam_covers_tile(sam_tile: u32, target_tile: u32, width: u32, level: u8) -> bool {
    sam_tile_distance(sam_tile, target_tile, width) <= sam_range(level) as u32
}

impl SowEngine {
    pub fn execute_sam(&mut self) {
        if self.state.phase != GamePhase::Playing || self.state.map.width == 0 {
            return;
        }

        let width = self.state.map.width;
        let tick = self.state.tick;
        let mut intercepted = HashSet::new();
        let mut spent = HashSet::new();
        let mut reserved = HashSet::new();
        let targets: std::collections::HashMap<_, _> = self.projectiles.iter()
            .filter(|p| p.active && matches!(p.kind, ProjectileKind::Nuke { .. }))
            .map(|p| (p.id, p))
            .collect();

        // SAMs store the assigned projectile in the existing target slot.
        for interceptor in self.projectiles.iter().filter(|p| p.active && p.kind == ProjectileKind::SAMMissile) {
            let Some(target) = interceptor.target_fleet_id.and_then(|id| targets.get(&id)) else {
                spent.insert(interceptor.id);
                continue;
            };
            let current = interceptor.path.get(interceptor.path_cursor).copied().unwrap_or(interceptor.dst_tile);
            let incoming = target.path.get(target.path_cursor).copied().unwrap_or(target.dst_tile);
            if sam_tile_distance(current, incoming, width) <= 1 {
                spent.insert(interceptor.id);
                intercepted.insert(target.id);
            } else if tick >= interceptor.expires_at_tick {
                spent.insert(interceptor.id);
            } else {
                reserved.insert(target.id);
            }
        }

        let batteries: Vec<_> = self.buildings.iter().copied()
            .filter(|b| b.kind == BuildingKind::SAM && self.nuclear_charges(b.id, b.active_level()) > 0)
            .collect();
        let mut shots = Vec::new();
        let mut reloads = Vec::new();
        for battery in batteries {
            let mut candidates = Vec::new();
            for target in targets.values() {
                if intercepted.contains(&target.id) || reserved.contains(&target.id)
                    || !self.naval_target_allowed(battery.owner_id, target.owner_id)
                {
                    continue;
                }
                let ProjectileKind::Nuke { level } = target.kind else { continue };
                let range = sam_range(battery.active_level()) as u32;
                let stride = usize::from(target.steps_per_tick.max(1));
                let future = target.path.iter().skip(target.path_cursor).step_by(stride);
                if let Some((time, &tile)) = future.enumerate().find(|&(time, &tile)| {
                    let distance = sam_tile_distance(battery.tile_idx, tile, width);
                    distance <= range && distance.div_ceil(u32::from(SAM_STEPS_PER_TICK)) as usize == time
                }) {
                    candidates.push((Reverse(level), time, target.id, tile));
                }
            }
            candidates.sort_unstable();
            for &(_, travel, target_id, tile) in candidates.iter()
                .take(usize::from(self.nuclear_charges(battery.id, battery.active_level())))
            {
                reserved.insert(target_id);
                reloads.push(battery.id);
                if travel == 0 {
                    intercepted.insert(target_id);
                    continue;
                }
                let id = self.state.next_projectile_id;
                self.state.next_projectile_id = id.wrapping_add(1).max(1);
                shots.push(Projectile {
                    id, kind: ProjectileKind::SAMMissile, owner_id: battery.owner_id,
                    src_tile: battery.tile_idx, dst_tile: tile,
                    path: std::sync::Arc::new(crate::pathfinding::bresenham_line(battery.tile_idx, tile, width)),
                    path_cursor: 0, steps_per_tick: SAM_STEPS_PER_TICK, active: true,
                    source_fleet_id: None, target_fleet_id: Some(target_id), damage: 0.0,
                    expires_at_tick: tick.saturating_add(travel as u64),
                });
            }
        }
        for id in reloads {
            self.spend_nuclear_charge(id);
        }
        self.projectiles.retain(|p| !spent.contains(&p.id) && !intercepted.contains(&p.id));
        self.projectiles.extend(shots);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{BuildingKind, GameState, Projectile};
    use crate::map::MapTile;
    use crate::player::Player;
    use crate::water_components::WaterComponents;
    use std::sync::Arc;

    fn test_sam_engine(w: u32, h: u32) -> SowEngine {
        let config = GameConfig::default();
        let mut state = GameState::new(1, w, h, config.clone());
        state.phase = GamePhase::Playing;
        let mut defender = Player::new_human(1, "Defender".into(), [0.0, 0.4, 1.0], &config);
        defender.alive = true;
        defender.tile_count = 20;
        let mut attacker = Player::new_human(2, "Attacker".into(), [1.0, 0.2, 0.2], &config);
        attacker.alive = true;
        attacker.tile_count = 20;
        state.register_player(defender);
        state.register_player(attacker);
        for i in 0..(w * h) as usize {
            state.map.terrain[i] = MapTile::from_byte(0b1000_0000);
        }
        SowEngine::new(state, WaterComponents::default())
    }

    #[test]
    fn sam_spatial_targeting_prioritizes_higher_level_nukes_without_double_targeting() {
        let mut engine = test_sam_engine(512, 512);
        // Two Lv1 SAMs belonging to owner 1 at (100, 100) and (102, 100)
        for (id, x) in [(1, 100), (2, 102)] {
            engine.buildings.push(crate::building::Building {
                id,
                tile_idx: 100 * 512 + x,
                owner_id: 1,
                kind: BuildingKind::SAM,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            });
        }
        // 20 enemy nukes so the dst_grid (>16) path is exercised:
        // 19 Lv1 nukes at distance ~10..=28, and 1 Lv2 nuke at distance 40.
        for i in 0..19u32 {
            let dst = (100 + i) * 512 + (110 + i);
            engine.projectiles.push(Projectile {
                id: u64::from(i + 1),
                kind: ProjectileKind::Nuke { level: 1 },
                owner_id: 2,
                src_tile: dst,
                dst_tile: dst,
                path: Arc::new(vec![dst]),
                path_cursor: 0,
                steps_per_tick: 2,
                active: true,
                source_fleet_id: None,
                target_fleet_id: None,
                damage: 0.0,
                expires_at_tick: u64::MAX,
            });
        }
        let prio_dst = 140 * 512 + 100;
        engine.projectiles.push(Projectile {
            id: 99,
            kind: ProjectileKind::Nuke { level: 2 },
            owner_id: 2,
            src_tile: prio_dst,
            dst_tile: prio_dst,
            path: Arc::new(vec![prio_dst]),
            path_cursor: 0,
            steps_per_tick: 3,
            active: true,
            source_fleet_id: None,
            target_fleet_id: None,
            damage: 0.0,
            expires_at_tick: u64::MAX,
        });

        engine.execute_sam();

        let sam_shots: Vec<u32> = engine
            .projectiles
            .iter()
            .filter(|p| matches!(p.kind, ProjectileKind::SAMMissile))
            .map(|p| p.dst_tile)
            .collect();
        assert_eq!(sam_shots.len(), 2);
        // First SAM takes Lv2 nuke; second SAM must NOT double-target Lv2, picks closest Lv1.
        assert_eq!(sam_shots[0], prio_dst);
        assert_eq!(sam_shots[1], 100 * 512 + 110);
    }

    #[test]
    fn sam_missile_intercepts_lowest_proj_index_enemy_nuke_only() {
        let mut engine = test_sam_engine(128, 128);
        let sam_tile = 32 * 128 + 32;
        let nuke_adj_1 = 32 * 128 + 33;
        let nuke_adj_2 = 33 * 128 + 32;
        let nuke_far = 80 * 128 + 80;

        for (id, dst) in [(10, nuke_adj_1), (11, nuke_adj_2), (12, nuke_far)] {
            engine.projectiles.push(Projectile {
                id,
                kind: ProjectileKind::Nuke { level: 1 },
                owner_id: 2,
                src_tile: dst,
                dst_tile: dst,
                path: Arc::new(vec![dst]),
                path_cursor: 0,
                steps_per_tick: 2,
                active: true,
                source_fleet_id: None,
                target_fleet_id: None,
                damage: 0.0,
                expires_at_tick: u64::MAX,
            });
        }
        engine.projectiles.push(Projectile {
            id: 50,
            kind: ProjectileKind::SAMMissile,
            owner_id: 1,
            src_tile: sam_tile,
            dst_tile: nuke_adj_1,
            path: Arc::new(vec![sam_tile]),
            path_cursor: 0,
            steps_per_tick: SAM_STEPS_PER_TICK,
            active: true,
            source_fleet_id: None,
            target_fleet_id: None,
            damage: 0.0,
            expires_at_tick: u64::MAX,
        });

        engine.execute_sam();

        let remaining_ids: Vec<u64> = engine.projectiles.iter().map(|p| p.id).collect();
        assert_eq!(remaining_ids, vec![11, 12]);
    }
}
