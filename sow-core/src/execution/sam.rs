use crate::building::income_level_weight;
use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, ProjectileKind};
use crate::game_config::GameConfig;
use std::cmp::Reverse;
use std::collections::HashSet;

/// SAM steps per tick (fast interceptor).
const SAM_STEPS_PER_TICK: u8 = 4;

/// Level-one SAM cooldown in ticks.
pub(crate) const SAM_COOLDOWN_TICKS: u32 = 90;

pub fn sam_cooldown_ticks(level: u8, cfg: &GameConfig) -> u32 {
    if cfg.progressive_defense_upgrades {
        (f64::from(SAM_COOLDOWN_TICKS) / income_level_weight(level.max(1)))
            .round()
            .max(1.0) as u32
    } else {
        SAM_COOLDOWN_TICKS
    }
}

pub fn sam_salvo_size(level: u8) -> usize {
    [0, 1, 3, 6, 10][usize::from(level.min(4))]
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
        if self.state.phase != GamePhase::Playing {
            return;
        }
        if !self.projectiles.iter().any(|projectile| {
            projectile.active && matches!(projectile.kind, ProjectileKind::Nuke { .. })
        }) {
            return;
        }

        let width = self.state.map.width;

        // Completed standalone SAMs intercept incoming nuclear strikes at their active level.
        let sams: Vec<(u64, u16, u32, u8)> = self
            .buildings
            .iter()
            .filter(|b| {
                b.kind == BuildingKind::SAM && b.active_level() > 0
            })
            .map(|b| (b.id, b.owner_id, b.tile_idx, b.active_level()))
            .collect();

        let mut sam_missiles = Vec::new();
        let mut assigned = HashSet::new();
        let mut candidates = Vec::new();

        for (sam_id, sam_owner, sam_tile, level) in sams {
            if self.silo_cooldowns.get(&sam_id).copied().unwrap_or(0) > 0 {
                continue;
            }

            let range = sam_range(level) as u32;
            candidates.clear();
            for proj in &self.projectiles {
                let ProjectileKind::Nuke { level } = proj.kind else {
                    continue;
                };
                if !proj.active
                    || proj.owner_id == sam_owner
                    || assigned.contains(&(sam_owner, proj.id))
                {
                    continue;
                }
                let distance = sam_tile_distance(sam_tile, proj.dst_tile, width);
                if distance <= range {
                    candidates.push((Reverse(level), distance, proj.id, proj.dst_tile));
                }
            }

            candidates.sort_unstable();
            let shots = sam_salvo_size(level).min(candidates.len());
            for &(_, _, target_id, target_tile) in &candidates[..shots] {
                assigned.insert((sam_owner, target_id));
                let id = self.state.next_projectile_id;
                self.state.next_projectile_id = id.wrapping_add(1).max(1);
                sam_missiles.push((id, sam_owner, sam_tile, target_tile));
            }
            if shots > 0 {
                let interval = sam_cooldown_ticks(level, &self.state.config);
                self.silo_cooldowns.insert(sam_id, interval * shots as u32);
            }
        }

        // Spawn SAM missiles
        for (id, owner, src_tile, dst_tile) in sam_missiles {
            let path = crate::pathfinding::bresenham_line(src_tile, dst_tile, width);
            self.projectiles.push(crate::game::Projectile {
                id,
                kind: ProjectileKind::SAMMissile,
                owner_id: owner,
                src_tile,
                dst_tile,
                path,
                path_cursor: 0,
                steps_per_tick: SAM_STEPS_PER_TICK,
                active: true,
                source_fleet_id: None,
                target_fleet_id: None,
                damage: 0.0,
                expires_at_tick: u64::MAX,
            });
        }

        // Check SAM missile proximity to their targets → intercept (same tile = hit)
        let mut intercepted_nukes = HashSet::new();
        let mut intercepted_sams = Vec::new();
        for sam in &self.projectiles {
            if !sam.active || !matches!(sam.kind, ProjectileKind::SAMMissile) {
                continue;
            }
            let sam_tile = sam
                .path
                .get(sam.path_cursor)
                .copied()
                .unwrap_or(sam.dst_tile);

            for nuke in &self.projectiles {
                if !nuke.active
                    || nuke.owner_id == sam.owner_id
                    || intercepted_nukes.contains(&nuke.id)
                {
                    continue;
                }
                let is_nuke = matches!(nuke.kind, ProjectileKind::Nuke { .. });
                if !is_nuke {
                    continue;
                }
                let nuke_tile = nuke
                    .path
                    .get(nuke.path_cursor)
                    .copied()
                    .unwrap_or(nuke.dst_tile);

                // Intercept if on the same tile or within 1 hex neighbor
                if sam_tile_distance(sam_tile, nuke_tile, width) <= 1 {
                    intercepted_nukes.insert(nuke.id);
                    intercepted_sams.push(sam.id);
                    break;
                }
            }
        }

        for id in intercepted_nukes.iter().chain(intercepted_sams.iter()) {
            if let Some(p) = self.projectiles.iter_mut().find(|p| p.id == *id) {
                p.active = false;
            }
        }

        // Cleanup
        self.projectiles.retain(|p| p.active);
    }
}
