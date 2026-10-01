use crate::render::gpu::{MoverInstanceGpu, MoverSpriteId, TrailSegmentGpu};
use sow_core::game::{ProjectileKind, UnitType};
use sow_core::protocol::{FleetSnapshot, ProjectileSnapshot, SimSnapshot};
use std::collections::{HashMap, HashSet};

const TRAIL_CAP: usize = 32;
const NUKE_ARC_PEAK: f32 = 4.0;
const NUKE_ARC_MIN_HEIGHT: f32 = 50.0;
const NUKE_ARC_SAMPLES: usize = 128;
const NUKE_CURVE_MIN_SAMPLES: usize = 64;
const NUKE_CURVE_MAX_SAMPLES: usize = 4096;
const MIN_PROJECTILE_SCREEN_PX: f32 = 11.0;
const MIN_TRANSPORT_SCREEN_PX: f32 = 12.0;
const PROJECTILE_TRAIL_WIDTH_MIN: f32 = 3.0;
const PROJECTILE_TRAIL_WIDTH_MAX: f32 = 10.0;

#[inline]
pub fn world_to_tile(wx: f32, wy: f32) -> (i32, i32) {
    (wx.floor() as i32, wy.floor() as i32)
}

#[inline]
pub fn tile_to_world(tile: u32, map_w: u32) -> (f32, f32) {
    let tx = (tile % map_w) as f32;
    let ty = (tile / map_w) as f32;
    let wx = tx + 0.5;
    let wy = ty + 0.5;
    (wx, wy)
}

#[inline]
fn flight_progress(path_len: usize, path_index: f32) -> f32 {
    if path_len <= 1 {
        1.0
    } else {
        (path_index / (path_len - 1) as f32).clamp(0.0, 1.0)
    }
}

#[inline]
fn nuke_arc_height(progress: f32) -> f32 {
    NUKE_ARC_PEAK * progress * (1.0 - progress)
}

#[inline]
fn path_world_at(path: &[u32], map_w: u32, index_f: f32) -> (f32, f32) {
    if path.is_empty() {
        return (0.0, 0.0);
    }
    let max_idx = path.len() - 1;
    let idx = (index_f.floor() as usize).min(max_idx);
    let next = (idx + 1).min(max_idx);
    let frac = (index_f - idx as f32).clamp(0.0, 1.0);
    let (x0, y0) = tile_to_world(path[idx], map_w);
    if idx == next || frac <= 0.0 {
        (x0, y0)
    } else {
        let (x1, y1) = tile_to_world(path[next], map_w);
        (x0 + (x1 - x0) * frac, y0 + (y1 - y0) * frac)
    }
}

#[inline]
fn bezier_point(points: [[f32; 2]; 4], t: f32) -> [f32; 2] {
    let one_minus_t = 1.0 - t;
    let a = one_minus_t * one_minus_t * one_minus_t;
    let b = 3.0 * one_minus_t * one_minus_t * t;
    let c = 3.0 * one_minus_t * t * t;
    let d = t * t * t;
    [
        a * points[0][0] + b * points[1][0] + c * points[2][0] + d * points[3][0],
        a * points[0][1] + b * points[1][1] + c * points[2][1] + d * points[3][1],
    ]
}

#[inline]
fn sampled_path_at(path: &[[f32; 2]], index_f: f32) -> [f32; 2] {
    if path.is_empty() {
        return [0.0, 0.0];
    }
    let idx = (index_f.floor() as usize).min(path.len() - 1);
    let next = (idx + 1).min(path.len() - 1);
    let frac = (index_f - idx as f32).clamp(0.0, 1.0);
    let a = path[idx];
    let b = path[next];
    [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]
}

struct NukeArcPath {
    points: Vec<[f32; 2]>,
    bounds: [f32; 4], // min x, min y, max x, max y
}

impl NukeArcPath {
    fn new(src_tile: u32, dst_tile: u32, map_w: u32, map_h: u32) -> Self {
        let start = tile_to_world(src_tile, map_w);
        let end = tile_to_world(dst_tile, map_w);
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance <= f32::EPSILON {
            return Self {
                points: vec![[start.0, start.1]],
                bounds: [start.0, start.1, start.0, start.1],
            };
        }
        let height = (distance / 3.0).max(NUKE_ARC_MIN_HEIGHT);
        let min_y = 0.5;
        let max_y = (map_h.max(1) as f32 - 0.5).max(min_y);
        let controls = [
            [start.0, start.1],
            [
                start.0 + dx / 4.0,
                (start.1 + dy / 4.0 - height).clamp(min_y, max_y),
            ],
            [
                start.0 + dx * 3.0 / 4.0,
                (start.1 + dy * 3.0 / 4.0 - height).clamp(min_y, max_y),
            ],
            [end.0, end.1],
        ];

        // Approximate the curve, then resample it at equal world-space distances.
        let raw_steps = ((distance * 2.0).ceil() as usize)
            .clamp(NUKE_CURVE_MIN_SAMPLES, NUKE_CURVE_MAX_SAMPLES);
        let mut raw_points = Vec::with_capacity(raw_steps + 1);
        let mut cumulative = Vec::with_capacity(raw_steps + 1);
        raw_points.push(bezier_point(controls, 0.0));
        cumulative.push(0.0);
        let mut total_length = 0.0;
        for i in 1..=raw_steps {
            let point = bezier_point(controls, i as f32 / raw_steps as f32);
            let previous = raw_points[i - 1];
            let dx = point[0] - previous[0];
            let dy = point[1] - previous[1];
            total_length += (dx * dx + dy * dy).sqrt();
            raw_points.push(point);
            cumulative.push(total_length);
        }

        let uniform_steps = total_length.ceil().max(1.0) as usize;
        let mut points = Vec::with_capacity(uniform_steps + 1);
        let mut raw_idx = 1;
        for i in 0..=uniform_steps {
            let target_distance = total_length * i as f32 / uniform_steps as f32;
            while raw_idx + 1 < cumulative.len() && cumulative[raw_idx] < target_distance {
                raw_idx += 1;
            }
            let from_distance = cumulative[raw_idx - 1];
            let segment_length = cumulative[raw_idx] - from_distance;
            let t = ((target_distance - from_distance) / segment_length.max(f32::EPSILON))
                .clamp(0.0, 1.0);
            let from = raw_points[raw_idx - 1];
            let to = raw_points[raw_idx];
            points.push([
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            ]);
        }

        let first = points[0];
        let mut bounds = [first[0], first[1], first[0], first[1]];
        for point in &points[1..] {
            bounds[0] = bounds[0].min(point[0]);
            bounds[1] = bounds[1].min(point[1]);
            bounds[2] = bounds[2].max(point[0]);
            bounds[3] = bounds[3].max(point[1]);
        }
        Self { points, bounds }
    }

    #[inline]
    fn point_at(&self, progress: f32) -> [f32; 2] {
        if self.points.len() == 1 {
            return self.points[0];
        }
        sampled_path_at(
            &self.points,
            progress.clamp(0.0, 1.0) * (self.points.len() - 1) as f32,
        )
    }
}

fn sample_nuke_arc(path: &NukeArcPath, progress: f32, out: &mut Vec<[f32; 2]>) {
    out.clear();
    if path.points.len() <= 1 || progress <= 0.0 {
        return;
    }
    for s in 0..=NUKE_ARC_SAMPLES {
        out.push(path.point_at(progress * s as f32 / NUKE_ARC_SAMPLES as f32));
    }
}

#[inline]
fn screen_margin() -> f32 {
    64.0
}

#[inline]
fn in_viewport(sx: f32, sy: f32, min_sx: f32, min_sy: f32, max_sx: f32, max_sy: f32) -> bool {
    sx >= min_sx && sx <= max_sx && sy >= min_sy && sy <= max_sy
}

#[inline]
fn projectile_trail_width(zoom: f32) -> f32 {
    (zoom * 0.5 + 2.0).clamp(PROJECTILE_TRAIL_WIDTH_MIN, PROJECTILE_TRAIL_WIDTH_MAX)
}

#[derive(Clone, Copy)]
struct MoverSlot {
    prev_x: f32,
    prev_y: f32,
    curr_x: f32,
    curr_y: f32,
    path_progress_prev: f32,
    path_progress_curr: f32,
    size: f32,
    color: [f32; 4],
    trail_color: [f32; 4],
    sprite: MoverSpriteId,
    trail_start: u32,
    trail_len: u32,
    is_fleet: bool,
    arc_trail: bool,
}

pub struct MoverScene {
    id_to_idx: HashMap<u64, u32>,
    slots: Vec<MoverSlot>,
    trail_points: Vec<[f32; 2]>,
    arc_scratch: Vec<[f32; 2]>,
    arc_paths: HashMap<u64, Vec<u32>>,
    nuke_arcs: HashMap<u64, NukeArcPath>,
    player_colors: HashMap<u16, [f32; 3]>,
    last_snap_tick: u64,
    map_w: u32,
}

pub struct MoverPackParams {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub linear_alpha: f32,
}

impl MoverScene {
    pub fn new() -> Self {
        Self {
            id_to_idx: HashMap::new(),
            slots: Vec::new(),
            trail_points: Vec::new(),
            arc_scratch: Vec::with_capacity(NUKE_ARC_SAMPLES + 1),
            arc_paths: HashMap::new(),
            nuke_arcs: HashMap::new(),
            player_colors: HashMap::new(),
            last_snap_tick: u64::MAX,
            map_w: 1,
        }
    }

    pub fn on_snapshot(
        &mut self,
        snap: &SimSnapshot,
        map_w: u32,
        map_h: u32,
        fog_of_war_enabled: bool,
        my_id: u16,
        fog_visible: &sow_core::bitset::DenseBitSet,
    ) {
        if snap.tick == self.last_snap_tick {
            return;
        }
        self.last_snap_tick = snap.tick;
        self.map_w = map_w.max(1);
        self.trail_points.clear();
        self.player_colors.clear();
        self.player_colors.reserve(snap.players.len());
        for player in &snap.players {
            let rgb = player
                .team
                .map_or(player.color, sow_core::player::team_territory_rgb);
            self.player_colors.insert(player.id, rgb);
        }

        let mut alive: HashSet<u64> = HashSet::new();

        for fleet in &snap.fleets {
            let is_visible = !fog_of_war_enabled
                || fleet.owner_id == my_id
                || fog_visible.contains(fleet.current_tile);
            if is_visible {
                alive.insert(fleet.id);
                self.ingest_fleet(fleet, map_w);
            }
        }
        for proj in &snap.projectiles {
            if proj.path.is_empty() || matches!(proj.kind, ProjectileKind::Shell) {
                continue;
            }
            let is_visible = !fog_of_war_enabled
                || fog_visible.contains(proj.src_tile)
                || fog_visible.contains(proj.dst_tile);
            if is_visible {
                let key = proj.id | (1u64 << 63);
                alive.insert(key);
                self.ingest_projectile(proj, map_w, map_h);
            }
        }

        let dead: Vec<u64> = self
            .id_to_idx
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect();
        for id in dead {
            self.arc_paths.remove(&id);
            self.nuke_arcs.remove(&id);
            if let Some(idx) = self.id_to_idx.remove(&id) {
                let rem = idx as usize;
                if rem < self.slots.len() {
                    let last = self.slots.len() - 1;
                    if rem != last {
                        self.slots.swap(rem, last);
                        if let Some(moved_id) = self
                            .id_to_idx
                            .iter()
                            .find(|(_, i)| **i as usize == last)
                            .map(|(k, _)| *k)
                        {
                            self.id_to_idx.insert(moved_id, rem as u32);
                        }
                    }
                    self.slots.pop();
                }
            }
        }
    }

    fn ingest_fleet(&mut self, fleet: &FleetSnapshot, map_w: u32) {
        let (prev_x, prev_y, curr_x, curr_y, progress) = if fleet.unit_type
            == UnitType::TransportShip
            && fleet.path_cursor > 0
            && !fleet.path.is_empty()
        {
            let from_idx = fleet
                .path_cursor
                .saturating_sub(1)
                .min(fleet.path.len() - 1);
            let (from_x, from_y) = tile_to_world(fleet.path[from_idx], map_w);
            let (to_x, to_y) = fleet
                .path
                .get(fleet.path_cursor)
                .copied()
                .map(|tile| tile_to_world(tile, map_w))
                .unwrap_or((from_x, from_y));
            let progress = fleet.movement_progress.clamp(0.0, 1.0);
            (
                from_x,
                from_y,
                from_x + (to_x - from_x) * progress,
                from_y + (to_y - from_y) * progress,
                progress,
            )
        } else {
            let (curr_x, curr_y) = tile_to_world(fleet.current_tile, map_w);
            let (prev_x, prev_y) = if fleet.path_cursor > 1 && !fleet.path.is_empty() {
                let prev_idx = fleet
                    .path_cursor
                    .saturating_sub(2)
                    .min(fleet.path.len().saturating_sub(1));
                tile_to_world(fleet.path[prev_idx], map_w)
            } else {
                (curr_x, curr_y)
            };
            (prev_x, prev_y, curr_x, curr_y, 0.0)
        };

        let sprite = match fleet.unit_type {
            UnitType::TransportShip => MoverSpriteId::TransportShip,
            UnitType::TradeShip => MoverSpriteId::TradeShip,
            UnitType::Warship => MoverSpriteId::Warship,
        };

        let rgb = self
            .player_colors
            .get(&fleet.owner_id)
            .copied()
            .unwrap_or([0.5, 0.5, 0.5]);
        let color = [rgb[0], rgb[1], rgb[2], 1.0];
        let trail_color = [
            rgb[0] * 0.7 + 0.3,
            rgb[1] * 0.7 + 0.3,
            rgb[2] * 0.7 + 0.3,
            0.75,
        ];

        let trail_start = self.trail_points.len() as u32;
        let traveled = fleet.path_cursor.saturating_sub(1);
        if traveled > 0 {
            let start = traveled.saturating_sub(TRAIL_CAP - 1);
            for &tile in &fleet.path[start..=traveled] {
                let (wx, wy) = tile_to_world(tile, map_w);
                self.trail_points.push([wx, wy]);
            }
        }
        let trail_len = self.trail_points.len() as u32 - trail_start;

        let entry = MoverSlot {
            prev_x,
            prev_y,
            curr_x,
            curr_y,
            path_progress_prev: 0.0,
            path_progress_curr: progress,
            size: 0.7,
            color,
            trail_color,
            sprite,
            trail_start,
            trail_len,
            is_fleet: true,
            arc_trail: false,
        };
        self.upsert_slot(fleet.id, entry);
    }

    fn ingest_projectile(&mut self, proj: &ProjectileSnapshot, map_w: u32, map_h: u32) {
        let cursor = proj.path_cursor.min(proj.path.len().saturating_sub(1));
        let prev_idx = cursor.saturating_sub(proj.steps_per_tick as usize);
        let (curr_x, curr_y) = tile_to_world(proj.path[cursor], map_w);
        let (prev_x, prev_y) = tile_to_world(proj.path[prev_idx], map_w);

        let path_len = proj.path.len();
        let progress_curr = flight_progress(path_len, cursor as f32);
        let progress_prev = flight_progress(path_len, prev_idx as f32);

        let (sprite, size, trail_color) = match proj.kind {
            ProjectileKind::Nuke { level } => {
                let sprite = MoverSpriteId::AtomBomb;
                let tc = if level >= 3 {
                    [1.0, 0.667, 0.0, 0.95]
                } else if level == 2 {
                    [1.0, 0.196, 0.0, 0.92]
                } else {
                    [1.0, 0.353, 0.0, 0.88]
                };
                (sprite, 0.65 + level as f32 * 0.12, tc)
            }
            ProjectileKind::SAMMissile => (MoverSpriteId::SamMissile, 0.6, [0.39, 0.78, 1.0, 0.85]),
            ProjectileKind::Shell => return,
        };

        let is_nuke = matches!(proj.kind, ProjectileKind::Nuke { .. });
        let key = proj.id | (1u64 << 63);
        if is_nuke {
            self.nuke_arcs
                .entry(key)
                .or_insert_with(|| NukeArcPath::new(proj.src_tile, proj.dst_tile, map_w, map_h));
        } else {
            self.arc_paths.insert(key, proj.path.clone());
        }

        let (trail_start, trail_len) = if is_nuke {
            (0, 0)
        } else {
            let trail_start = self.trail_points.len() as u32;
            let traveled = cursor;
            if traveled > 0 {
                let start = traveled.saturating_sub(TRAIL_CAP);
                let stride = ((traveled - start) / 12).max(1);
                for i in (start..=traveled).step_by(stride) {
                    let (wx, wy) = tile_to_world(proj.path[i], map_w);
                    self.trail_points.push([wx, wy]);
                }
            }
            let trail_len = self.trail_points.len() as u32 - trail_start;
            (trail_start, trail_len)
        };

        let entry = MoverSlot {
            prev_x,
            prev_y,
            curr_x,
            curr_y,
            path_progress_prev: progress_prev,
            path_progress_curr: progress_curr,
            size,
            color: [1.0, 1.0, 1.0, 1.0],
            trail_color,
            sprite,
            trail_start,
            trail_len,
            is_fleet: false,
            arc_trail: is_nuke,
        };
        self.upsert_slot(key, entry);
    }

    fn upsert_slot(&mut self, id: u64, mut entry: MoverSlot) {
        if let Some(&idx) = self.id_to_idx.get(&id) {
            let old = &self.slots[idx as usize];
            entry.prev_x = old.curr_x;
            entry.prev_y = old.curr_y;
            entry.path_progress_prev = old.path_progress_curr;
            self.slots[idx as usize] = entry;
        } else {
            let idx = self.slots.len() as u32;
            self.id_to_idx.insert(id, idx);
            self.slots.push(entry);
        }
    }

    fn push_trail_segments(
        &self,
        renderer: &mut crate::render::gpu::MoverRenderer,
        points: &[[f32; 2]],
        head: [f32; 2],
        width: f32,
        color: [f32; 4],
        fade_tail: bool,
    ) {
        if points.is_empty() {
            return;
        }
        let mut prev = points[0];
        for (i, pt) in points[1..].iter().enumerate() {
            let mut segment_color = color;
            if fade_tail {
                segment_color[3] *= (i + 1) as f32 / points.len() as f32;
            }
            renderer.push_trail_segment(TrailSegmentGpu {
                p0: prev,
                p1: *pt,
                width,
                color: segment_color,
            });
            prev = *pt;
        }
        renderer.push_trail_segment(TrailSegmentGpu {
            p0: prev,
            p1: head,
            width,
            color,
        });
    }

    fn arc_visible(
        &self,
        points: &[[f32; 2]],
        head: [f32; 2],
        params: &MoverPackParams,
        bounds: (f32, f32, f32, f32),
    ) -> bool {
        let (min_sx, min_sy, max_sx, max_sy) = bounds;
        for pt in points {
            let sx = params.camera_x + pt[0] * params.camera_zoom;
            let sy = params.camera_y + pt[1] * params.camera_zoom;
            if in_viewport(sx, sy, min_sx, min_sy, max_sx, max_sy) {
                return true;
            }
        }
        let sx = params.camera_x + head[0] * params.camera_zoom;
        let sy = params.camera_y + head[1] * params.camera_zoom;
        in_viewport(sx, sy, min_sx, min_sy, max_sx, max_sy)
    }

    pub fn pack_gpu(
        &mut self,
        params: &MoverPackParams,
        renderer: &mut crate::render::gpu::MoverRenderer,
    ) {
        renderer.begin_frame();
        let margin = screen_margin();
        let min_sx = -margin;
        let min_sy = -margin;
        let max_sx = params.screen_w + margin;
        let max_sy = params.screen_h + margin;
        let mut arc_scratch = std::mem::take(&mut self.arc_scratch);

        for (id, &idx) in &self.id_to_idx {
            arc_scratch.clear();
            let slot = &self.slots[idx as usize];

            let (wx, wy, progress) = if slot.is_fleet {
                let wx = slot.prev_x + (slot.curr_x - slot.prev_x) * params.linear_alpha;
                let wy = slot.prev_y + (slot.curr_y - slot.prev_y) * params.linear_alpha;
                let progress = slot.path_progress_prev
                    + (slot.path_progress_curr - slot.path_progress_prev) * params.linear_alpha;
                (wx, wy, progress)
            } else {
                let progress = slot.path_progress_prev
                    + (slot.path_progress_curr - slot.path_progress_prev) * params.linear_alpha;
                if slot.arc_trail
                    && let Some(path) = self.nuke_arcs.get(id)
                {
                    let [wx, wy] = path.point_at(progress);
                    (wx, wy, progress)
                } else if let Some(path) = self.arc_paths.get(id) {
                    let path_len = path.len();
                    if path_len > 0 {
                        let idx_f = progress * (path_len - 1) as f32;
                        let pos = path_world_at(path, self.map_w, idx_f);
                        (pos.0, pos.1, progress)
                    } else {
                        let wx = slot.prev_x + (slot.curr_x - slot.prev_x) * params.linear_alpha;
                        let wy = slot.prev_y + (slot.curr_y - slot.prev_y) * params.linear_alpha;
                        (wx, wy, progress)
                    }
                } else {
                    let wx = slot.prev_x + (slot.curr_x - slot.prev_x) * params.linear_alpha;
                    let wy = slot.prev_y + (slot.curr_y - slot.prev_y) * params.linear_alpha;
                    (wx, wy, progress)
                }
            };

            let height = if slot.is_fleet {
                0.0
            } else {
                nuke_arc_height(progress)
            };
            let world_pos = [wx, wy];

            let sx = params.camera_x + wx * params.camera_zoom;
            let sy = params.camera_y + world_pos[1] * params.camera_zoom;
            let sprite_visible = in_viewport(sx, sy, min_sx, min_sy, max_sx, max_sy);

            let trail_width = if slot.is_fleet {
                (params.camera_zoom * 0.4).clamp(1.0, 6.0)
            } else {
                projectile_trail_width(params.camera_zoom)
            };

            if slot.arc_trail {
                if let Some(path) = self.nuke_arcs.get(id) {
                    let bounds = path.bounds;
                    let arc_visible = params.camera_x + bounds[2] * params.camera_zoom >= -margin
                        && params.camera_x + bounds[0] * params.camera_zoom
                            <= params.screen_w + margin
                        && params.camera_y + bounds[3] * params.camera_zoom >= -margin
                        && params.camera_y + bounds[1] * params.camera_zoom
                            <= params.screen_h + margin;
                    if arc_visible {
                        sample_nuke_arc(path, progress, &mut arc_scratch);
                        if self.arc_visible(
                            &arc_scratch,
                            world_pos,
                            params,
                            (min_sx, min_sy, max_sx, max_sy),
                        ) {
                            self.push_trail_segments(
                                renderer,
                                &arc_scratch,
                                world_pos,
                                trail_width,
                                slot.trail_color,
                                false,
                            );
                        }
                    }
                }
            } else if slot.trail_len > 0 {
                let start = slot.trail_start as usize;
                let end = start + slot.trail_len as usize;
                let trail_points = &self.trail_points[start..end];
                if self.arc_visible(
                    trail_points,
                    world_pos,
                    params,
                    (min_sx, min_sy, max_sx, max_sy),
                ) {
                    self.push_trail_segments(
                        renderer,
                        trail_points,
                        world_pos,
                        trail_width,
                        slot.trail_color,
                        slot.is_fleet,
                    );
                }
            }

            if !sprite_visible {
                continue;
            }

            let dx = slot.curr_x - slot.prev_x;
            let dy = slot.curr_y - slot.prev_y;
            let rotation = if slot.arc_trail && arc_scratch.len() >= 2 {
                let last = arc_scratch[arc_scratch.len() - 2];
                let dir_x = world_pos[0] - last[0];
                let dir_y = world_pos[1] - last[1];
                if dir_x * dir_x + dir_y * dir_y > 1e-8 {
                    dir_y.atan2(dir_x) + std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                }
            } else if slot.trail_len > 0 {
                let last = self.trail_points[(slot.trail_start + slot.trail_len - 1) as usize];
                let dir_x = world_pos[0] - last[0];
                let dir_y = world_pos[1] - last[1];
                if dir_x * dir_x + dir_y * dir_y > 1e-8 {
                    dir_y.atan2(dir_x) + std::f32::consts::FRAC_PI_2
                } else if dx * dx + dy * dy > 1e-8 {
                    dy.atan2(dx) + std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                }
            } else if dx * dx + dy * dy > 1e-8 {
                dy.atan2(dx) + std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };

            let scale = if slot.is_fleet {
                1.0
            } else {
                (1.0 + height * 0.5).min(2.0)
            };

            let mut sprite_size = slot.size * scale;
            if slot.is_fleet && slot.sprite == MoverSpriteId::TransportShip {
                let screen_size = sprite_size * params.camera_zoom;
                if screen_size < MIN_TRANSPORT_SCREEN_PX {
                    sprite_size = MIN_TRANSPORT_SCREEN_PX / params.camera_zoom;
                }
            } else if !slot.is_fleet {
                let screen_size = sprite_size * params.camera_zoom;
                if screen_size < MIN_PROJECTILE_SCREEN_PX {
                    sprite_size = MIN_PROJECTILE_SCREEN_PX / params.camera_zoom;
                }
            }

            renderer.push_sprite(MoverInstanceGpu {
                world_pos: [wx, wy],
                size: sprite_size,
                rotation,
                color: slot.color,
                uv_rect: slot.sprite.uv_rect(),
                height: 0.0,
            });
        }
        self.arc_scratch = arc_scratch;
    }
}

impl Default for MoverScene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::NukeArcPath;

    #[test]
    fn nuke_arc_is_distance_spaced_bounded_and_distance_scaled() {
        let short = NukeArcPath::new(100 * 256 + 20, 100 * 256 + 40, 256, 256);
        let long = NukeArcPath::new(100 * 256 + 20, 100 * 256 + 220, 256, 256);

        let start = short.point_at(0.0);
        let end = short.point_at(1.0);
        assert!((start[0] - 20.5).abs() < 0.001 && (start[1] - 100.5).abs() < 0.001);
        assert!((end[0] - 40.5).abs() < 0.001 && (end[1] - 100.5).abs() < 0.001);
        assert!(long.point_at(0.5)[1] < short.point_at(0.5)[1]);
        assert!(
            long.points
                .iter()
                .all(|point| point[1] >= 0.5 && point[1] <= 255.5)
        );

        let mut min_step = f32::INFINITY;
        let mut max_step: f32 = 0.0;
        for pair in long.points.windows(2) {
            let dx = pair[1][0] - pair[0][0];
            let dy = pair[1][1] - pair[0][1];
            let step = (dx * dx + dy * dy).sqrt();
            min_step = min_step.min(step);
            max_step = max_step.max(step);
        }
        assert!(max_step - min_step < 0.05);
    }
}
