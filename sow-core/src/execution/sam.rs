use crate::building::income_level_weight;
use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, ProjectileKind};
use crate::game_config::GameConfig;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

/// SAM steps per tick (fast interceptor).
const SAM_STEPS_PER_TICK: u8 = 4;

/// Level-one SAM cooldown in ticks.
pub(crate) const SAM_COOLDOWN_TICKS: u32 = 90;

/// Coarse grid bucket sizes matching SAM target & flight-intercept scales.
const NUKE_DST_CELL_SIZE: u32 = 256;
const NUKE_CUR_CELL_SIZE: u32 = 16;

#[derive(Clone, Copy)]
struct ActiveNuke {
    proj_idx: usize,
    id: u64,
    owner_id: u16,
    level: u8,
    dst_tile: u32,
    dst_x: u32,
    dst_y: u32,
    cur_x: u32,
    cur_y: u32,
}

#[derive(Clone, Copy)]
struct ActiveSamMissile {
    proj_idx: usize,
    owner_id: u16,
    cur_x: u32,
    cur_y: u32,
}

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
        let width = self.state.map.width;
        if width == 0 {
            return;
        }

        // Partition active nukes and active SAM missiles in a single pass so inner
        // loops never scan Shells or re-filter `self.projectiles`.
        let mut active_nukes: Vec<ActiveNuke> = Vec::new();
        let mut active_sam_missiles: Vec<ActiveSamMissile> = Vec::new();
        for (proj_idx, proj) in self.projectiles.iter().enumerate() {
            if !proj.active {
                continue;
            }
            match proj.kind {
                ProjectileKind::Nuke { level } => {
                    let cur_tile = proj
                        .path
                        .get(proj.path_cursor)
                        .copied()
                        .unwrap_or(proj.dst_tile);
                    active_nukes.push(ActiveNuke {
                        proj_idx,
                        id: proj.id,
                        owner_id: proj.owner_id,
                        level,
                        dst_tile: proj.dst_tile,
                        dst_x: proj.dst_tile % width,
                        dst_y: proj.dst_tile / width,
                        cur_x: cur_tile % width,
                        cur_y: cur_tile / width,
                    });
                }
                ProjectileKind::SAMMissile => {
                    let cur_tile = proj
                        .path
                        .get(proj.path_cursor)
                        .copied()
                        .unwrap_or(proj.dst_tile);
                    active_sam_missiles.push(ActiveSamMissile {
                        proj_idx,
                        owner_id: proj.owner_id,
                        cur_x: cur_tile % width,
                        cur_y: cur_tile / width,
                    });
                }
                ProjectileKind::Shell => {}
            }
        }
        if active_nukes.is_empty() {
            return;
        }

        // Completed standalone SAMs ready to fire this tick.
        let sams: Vec<(u64, u16, u32, u8)> = self
            .buildings
            .iter()
            .filter(|b| {
                b.kind == BuildingKind::SAM
                    && b.active_level() > 0
                    && self.silo_cooldowns.get(&b.id).copied().unwrap_or(0) == 0
            })
            .map(|b| (b.id, b.owner_id, b.tile_idx, b.active_level()))
            .collect();

        if !sams.is_empty() {
            let mut sam_missiles = Vec::new();
            let mut assigned: HashSet<(u16, u64)> = HashSet::new();
            let mut candidates = Vec::new();

            // Index nuke destinations on a coarse spatial grid when nuke count is large.
            let dst_grid: Option<HashMap<(u32, u32), Vec<usize>>> = if active_nukes.len() > 16 {
                let mut map: HashMap<(u32, u32), Vec<usize>> =
                    HashMap::with_capacity(active_nukes.len());
                for (ni, nuke) in active_nukes.iter().enumerate() {
                    map.entry((
                        nuke.dst_x / NUKE_DST_CELL_SIZE,
                        nuke.dst_y / NUKE_DST_CELL_SIZE,
                    ))
                    .or_default()
                    .push(ni);
                }
                Some(map)
            } else {
                None
            };

            for (sam_id, sam_owner, sam_tile, level) in sams {
                let range = sam_range(level) as u32;
                let sam_x = sam_tile % width;
                let sam_y = sam_tile / width;
                candidates.clear();

                if let Some(grid) = dst_grid.as_ref() {
                    let gx0 = sam_x.saturating_sub(range) / NUKE_DST_CELL_SIZE;
                    let gx1 = sam_x.saturating_add(range) / NUKE_DST_CELL_SIZE;
                    let gy0 = sam_y.saturating_sub(range) / NUKE_DST_CELL_SIZE;
                    let gy1 = sam_y.saturating_add(range) / NUKE_DST_CELL_SIZE;
                    for gy in gy0..=gy1 {
                        for gx in gx0..=gx1 {
                            let Some(bucket) = grid.get(&(gx, gy)) else {
                                continue;
                            };
                            for &ni in bucket {
                                let nuke = &active_nukes[ni];
                                if nuke.owner_id == sam_owner
                                    || assigned.contains(&(sam_owner, nuke.id))
                                {
                                    continue;
                                }
                                let distance = nuke
                                    .dst_x
                                    .abs_diff(sam_x)
                                    .max(nuke.dst_y.abs_diff(sam_y));
                                if distance <= range {
                                    candidates.push((
                                        Reverse(nuke.level),
                                        distance,
                                        nuke.id,
                                        nuke.dst_tile,
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    for nuke in &active_nukes {
                        if nuke.owner_id == sam_owner || assigned.contains(&(sam_owner, nuke.id)) {
                            continue;
                        }
                        let distance = nuke.dst_x.abs_diff(sam_x).max(nuke.dst_y.abs_diff(sam_y));
                        if distance <= range {
                            candidates.push((
                                Reverse(nuke.level),
                                distance,
                                nuke.id,
                                nuke.dst_tile,
                            ));
                        }
                    }
                }

                // `nuke.id` is unique per projectile, so `sort_unstable` defines a strict total order.
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

            // Spawn SAM missiles and register them in `active_sam_missiles` for immediate contact checks.
            for (id, owner, src_tile, dst_tile) in sam_missiles {
                let path = std::sync::Arc::new(crate::pathfinding::bresenham_line(
                    src_tile, dst_tile, width,
                ));
                let cur_tile = path.first().copied().unwrap_or(dst_tile);
                let proj_idx = self.projectiles.len();
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
                active_sam_missiles.push(ActiveSamMissile {
                    proj_idx,
                    owner_id: owner,
                    cur_x: cur_tile % width,
                    cur_y: cur_tile / width,
                });
            }
        }

        if active_sam_missiles.is_empty() {
            return;
        }

        // Check SAM missile proximity to active nukes -> intercept (within 1 tile Chebyshev),
        // preserving exact first-matching `self.projectiles` order in O(1) per hit.
        let mut deactivated = vec![false; self.projectiles.len()];
        let cur_grid: Option<HashMap<(u32, u32), Vec<usize>>> =
            if active_nukes.len() * active_sam_missiles.len() > 256 {
                let mut map: HashMap<(u32, u32), Vec<usize>> =
                    HashMap::with_capacity(active_nukes.len());
                for (ni, nuke) in active_nukes.iter().enumerate() {
                    map.entry((
                        nuke.cur_x / NUKE_CUR_CELL_SIZE,
                        nuke.cur_y / NUKE_CUR_CELL_SIZE,
                    ))
                    .or_default()
                    .push(ni);
                }
                Some(map)
            } else {
                None
            };

        let mut any_intercepted = false;
        for sam in &active_sam_missiles {
            if let Some(grid) = cur_grid.as_ref() {
                let gx0 = sam.cur_x.saturating_sub(1) / NUKE_CUR_CELL_SIZE;
                let gx1 = sam.cur_x.saturating_add(1) / NUKE_CUR_CELL_SIZE;
                let gy0 = sam.cur_y.saturating_sub(1) / NUKE_CUR_CELL_SIZE;
                let gy1 = sam.cur_y.saturating_add(1) / NUKE_CUR_CELL_SIZE;
                let mut best_nuke_idx: Option<usize> = None;
                for gy in gy0..=gy1 {
                    for gx in gx0..=gx1 {
                        let Some(bucket) = grid.get(&(gx, gy)) else {
                            continue;
                        };
                        for &ni in bucket {
                            let nuke = &active_nukes[ni];
                            if nuke.owner_id == sam.owner_id
                                || deactivated[nuke.proj_idx]
                                || nuke.cur_x.abs_diff(sam.cur_x) > 1
                                || nuke.cur_y.abs_diff(sam.cur_y) > 1
                            {
                                continue;
                            }
                            if best_nuke_idx.is_none_or(|best| nuke.proj_idx < best) {
                                best_nuke_idx = Some(nuke.proj_idx);
                            }
                        }
                    }
                }
                if let Some(nuke_proj_idx) = best_nuke_idx {
                    deactivated[nuke_proj_idx] = true;
                    deactivated[sam.proj_idx] = true;
                    any_intercepted = true;
                }
            } else {
                for nuke in &active_nukes {
                    if nuke.owner_id == sam.owner_id || deactivated[nuke.proj_idx] {
                        continue;
                    }
                    if nuke.cur_x.abs_diff(sam.cur_x) <= 1 && nuke.cur_y.abs_diff(sam.cur_y) <= 1 {
                        deactivated[nuke.proj_idx] = true;
                        deactivated[sam.proj_idx] = true;
                        any_intercepted = true;
                        break;
                    }
                }
            }
        }

        if any_intercepted {
            let mut idx = 0usize;
            self.projectiles.retain(|p| {
                let keep = p.active && !deactivated[idx];
                idx += 1;
                keep
            });
        }
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
