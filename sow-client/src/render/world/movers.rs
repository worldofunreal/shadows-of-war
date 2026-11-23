use crate::render::gpu::{MoverInstanceGpu, MoverSpriteId, TrailSegmentGpu};
use sow_core::game::{BuildingKind, ProjectileKind, UnitType};
use sow_core::protocol::{BuildingSnapshot, FleetSnapshot, ProjectileSnapshot, SimSnapshot};
use std::collections::{HashMap, HashSet};

const TRAIL_CAP: usize = 32;
const SHELL_SPRITE_SCALE: f32 = 0.8;
const NUKE_ARC_MIN_HEIGHT: f32 = 50.0;
const NUKE_ARC_SAMPLES: usize = 128;
const MIN_TRANSPORT_SCREEN_PX: f32 = 12.0;
const FLEET_LOCATE_DURATION_SECS: f32 = 3.0;
const FLEET_LOCATE_RING_SEGMENTS: usize = 24;

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
fn nuke_arc_height(progress: f32, peak: f32) -> f32 {
    peak * progress * (1.0 - progress)
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

#[derive(Clone, Copy)]
pub(crate) struct NukeArcPath {
    controls: [[f32; 2]; 4],
    lengths: [f32; 17],
    bounds: [f32; 4], // min x, min y, max x, max y
    src_tile: u32,
    dst_tile: u32,
    map_w: u32,
    map_h: u32,
}

impl NukeArcPath {
    pub(crate) fn new(
        src_tile: u32,
        dst_tile: u32,
        map_w: u32,
        map_h: u32,
        min_height: f32,
    ) -> Self {
        let start = tile_to_world(src_tile, map_w);
        let end = tile_to_world(dst_tile, map_w);
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        let distance = (dx * dx + dy * dy).sqrt();
        let height = if distance == 0.0 {
            0.0
        } else {
            (distance / 3.0).max(min_height)
        };
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

        let mut lengths = [0.0; 17];
        let mut prev = controls[0];
        for i in 1..=16 {
            let point = bezier_point(controls, i as f32 / 16.0);
            lengths[i] = lengths[i - 1] + (point[0] - prev[0]).hypot(point[1] - prev[1]);
            prev = point;
        }
        let mut bounds = [start.0, start.1, start.0, start.1];
        for p in controls {
            bounds = [
                bounds[0].min(p[0]),
                bounds[1].min(p[1]),
                bounds[2].max(p[0]),
                bounds[3].max(p[1]),
            ];
        }
        Self {
            controls,
            lengths,
            bounds,
            src_tile,
            dst_tile,
            map_w,
            map_h,
        }
    }

    fn rebuild(&mut self, min_height: f32) {
        *self = Self::new(
            self.src_tile,
            self.dst_tile,
            self.map_w,
            self.map_h,
            min_height,
        );
    }

    #[inline]
    fn parameter(&self, progress: f32) -> f32 {
        let d = progress.clamp(0.0, 1.0) * self.lengths[16];
        let i = self.lengths.partition_point(|&s| s < d).clamp(1, 16);
        let fraction =
            (d - self.lengths[i - 1]) / (self.lengths[i] - self.lengths[i - 1]).max(f32::EPSILON);
        (i as f32 - 1.0 + fraction) / 16.0
    }

    pub(crate) fn point_at(&self, progress: f32) -> [f32; 2] {
        bezier_point(self.controls, self.parameter(progress))
    }

    fn heading(&self, progress: f32) -> f32 {
        let t = self.parameter(progress);
        let u = 1.0 - t;
        let [a, b, c, d] = self.controls;
        let axis =
            |i: usize| u * u * (b[i] - a[i]) + 2.0 * u * t * (c[i] - b[i]) + t * t * (d[i] - c[i]);
        axis(1).atan2(axis(0)) + std::f32::consts::FRAC_PI_2
    }
}

fn sample_nuke_arc(
    path: &NukeArcPath,
    start: f32,
    progress: f32,
    samples: usize,
    out: &mut Vec<[f32; 2]>,
) {
    out.clear();
    if progress <= start {
        return;
    }
    let samples = samples.clamp(2, 512);
    for s in 0..=samples {
        out.push(path.point_at(start + (progress - start) * s as f32 / samples as f32));
    }
}

#[inline]
fn adaptive_nuke_arc_samples(bounds: [f32; 4], camera_zoom: f32, max_samples: usize) -> usize {
    let max_samples = max_samples.clamp(12, 512);
    let arc_px = (bounds[2] - bounds[0]).hypot(bounds[3] - bounds[1]) * camera_zoom.max(0.01);
    ((arc_px / 6.0).ceil() as usize).clamp(12, max_samples)
}

#[inline]
fn in_viewport(sx: f32, sy: f32, min_sx: f32, min_sy: f32, max_sx: f32, max_sy: f32) -> bool {
    sx >= min_sx && sx <= max_sx && sy >= min_sy && sy <= max_sy
}

fn locate_highlight_color(color: [f32; 4], intensity: f32) -> [f32; 4] {
    let amount = intensity.clamp(0.0, 1.0) * 0.35;
    [
        color[0] + (1.0 - color[0]) * amount,
        color[1] + (1.0 - color[1]) * amount,
        color[2] + (1.0 - color[2]) * amount,
        color[3] + (1.0 - color[3]) * amount,
    ]
}

#[derive(Clone, Copy)]
struct MoverSlot {
    id: u64,
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
    is_shell: bool,
    warhead: bool,
    arc_trail: bool,
    is_owned_warship: bool,
    health_fraction: f32,
    veterancy: u8,
    warship_mode: Option<sow_core::warp_fleet::WarshipMode>,
    in_combat: bool,
}

struct WarshipLaunchPulse {
    center: [f32; 2],
    owner_id: u16,
    started: web_time::Instant,
}

pub struct MoverScene {
    launch_pulses: Vec<WarshipLaunchPulse>,
    id_to_idx: HashMap<u64, u32>,
    slots: Vec<MoverSlot>,
    trail_points: Vec<[f32; 2]>,
    arc_scratch: Vec<[f32; 2]>,
    alive_scratch: HashSet<u64>,
    dead_scratch: Vec<u64>,
    arc_paths: HashMap<u64, std::sync::Arc<Vec<u32>>>,
    nuke_arcs: HashMap<u64, NukeArcPath>,
    player_colors: HashMap<u16, [f32; 3]>,
    last_snap_tick: u64,
    last_curve_tuning: u32,
    map_w: u32,
    located_fleet: Option<(u64, web_time::Instant)>,
}

pub struct MoverPackParams<'a> {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub linear_alpha: f32,
    pub shader_params: &'a [[f32; 4]; sow_render::shader_params::PARAM_CAPACITY],
    pub sprites_enabled: bool,
    pub trails_enabled: bool,
}

impl MoverScene {
    pub fn new() -> Self {
        Self {
            id_to_idx: HashMap::new(),
            slots: Vec::new(),
            trail_points: Vec::new(),
            arc_scratch: Vec::with_capacity(513),
            alive_scratch: HashSet::new(),
            dead_scratch: Vec::new(),
            arc_paths: HashMap::new(),
            nuke_arcs: HashMap::new(),
            player_colors: HashMap::new(),
            last_snap_tick: u64::MAX,
            last_curve_tuning: 0,
            map_w: 1,
            located_fleet: None,
            launch_pulses: Vec::new(),
        }
    }

    pub fn warship_launch_pulse(
        &mut self,
        center: [f32; 2],
        owner_id: u16,
        started: web_time::Instant,
    ) {
        self.launch_pulses
            .retain(|pulse| started.duration_since(pulse.started).as_secs_f32() < 0.6);
        self.launch_pulses.push(WarshipLaunchPulse {
            center,
            owner_id,
            started,
        });
    }

    pub fn highlight_fleet(&mut self, fleet_id: u64) {
        self.located_fleet = Some((fleet_id, web_time::Instant::now()));
    }

    pub fn on_snapshot(
        &mut self,
        snap: &SimSnapshot,
        map_w: u32,
        map_h: u32,
        fog_of_war_enabled: bool,
        my_id: u16,
        fog_visible: &sow_core::bitset::DenseBitSet,
        shader_params: &[[f32; 4]; sow_render::shader_params::PARAM_CAPACITY],
    ) {
        let setting = |id: &str, fallback: f32| {
            sow_render::shader_params::shader_slot(id)
                .map(|slot| shader_params[slot][0])
                .unwrap_or(fallback)
        };
        let min_height = setting("mover.nuke.arc_height", NUKE_ARC_MIN_HEIGHT);
        let curve_tuning = min_height.to_bits();
        if curve_tuning != self.last_curve_tuning {
            for path in self.nuke_arcs.values_mut() {
                path.rebuild(min_height);
            }
            self.last_curve_tuning = curve_tuning;
        }
        if snap.tick == self.last_snap_tick {
            return;
        }
        self.last_snap_tick = snap.tick;
        self.last_curve_tuning = curve_tuning;
        self.map_w = map_w.max(1);
        self.trail_points.clear();
        self.player_colors.clear();
        self.player_colors.reserve(snap.players.len());
        for player in &snap.players {
            let rgb = sow_core::player::player_territory_rgb(
                player.player_type,
                player.team,
                player.color,
                player.is_campaign_faction,
            );
            self.player_colors.insert(player.id, rgb);
        }

        self.alive_scratch.clear();
        let mut alive = std::mem::take(&mut self.alive_scratch);

        for fleet in &snap.fleets {
            let is_visible = !fog_of_war_enabled
                || fleet.owner_id == my_id
                || fog_visible.contains(fleet.current_tile);
            if is_visible {
                alive.insert(fleet.id);
                self.ingest_fleet(fleet, map_w, my_id);
            }
        }
        for building in &snap.buildings {
            if building.kind != BuildingKind::SAM {
                continue;
            }
            let visible = !fog_of_war_enabled
                || building.owner_id == my_id
                || fog_visible.contains(building.tile_idx);
            if visible {
                let key = building.id | (1u64 << 62);
                alive.insert(key);
                self.ingest_sam_building(building, map_w);
            }
        }
        for proj in &snap.projectiles {
            if proj.path.is_empty() {
                continue;
            }
            let is_visible = !fog_of_war_enabled
                || fog_visible.contains(proj.src_tile)
                || fog_visible.contains(proj.dst_tile);
            if is_visible {
                let key = proj.id | (1u64 << 63);
                alive.insert(key);
                self.ingest_projectile(proj, map_w, map_h, min_height);
            }
        }

        let mut dead = std::mem::take(&mut self.dead_scratch);
        dead.clear();
        dead.extend(
            self.id_to_idx
                .keys()
                .copied()
                .filter(|id| !alive.contains(id)),
        );
        self.remove_dead_slots(&dead);
        self.dead_scratch = dead;
        self.alive_scratch = alive;
    }

    fn remove_dead_slots(&mut self, dead: &[u64]) {
        for &id in dead {
            self.arc_paths.remove(&id);
            self.nuke_arcs.remove(&id);
            if let Some(idx) = self.id_to_idx.remove(&id) {
                let rem = idx as usize;
                if rem < self.slots.len() {
                    let last = self.slots.len() - 1;
                    if rem != last {
                        self.slots.swap(rem, last);
                        let moved_id = self.slots[rem].id;
                        self.id_to_idx.insert(moved_id, rem as u32);
                    }
                    self.slots.pop();
                }
            }
        }
    }

    fn ingest_sam_building(&mut self, building: &BuildingSnapshot, map_w: u32) {
        let (x, y) = tile_to_world(building.tile_idx, map_w.max(1));
        let color = self
            .player_colors
            .get(&building.owner_id)
            .copied()
            .unwrap_or([0.5, 0.7, 0.8]);
        self.upsert_slot(
            building.id | (1u64 << 62),
            MoverSlot {
                id: 0,
                prev_x: x,
                prev_y: y,
                curr_x: x,
                curr_y: y,
                path_progress_prev: 0.0,
                path_progress_curr: 0.0,
                size: 0.9,
                color: [
                    color[0],
                    color[1],
                    color[2],
                    if building.under_construction {
                        0.55
                    } else {
                        1.0
                    },
                ],
                trail_color: [0.0; 4],
                sprite: MoverSpriteId::SamMissile,
                trail_start: 0,
                trail_len: 0,
                is_fleet: true,
                is_shell: false,
                warhead: false,
                arc_trail: false,
                is_owned_warship: false,
                health_fraction: 1.0,
                veterancy: 0,
                warship_mode: None,
                in_combat: false,
            },
        );
    }

    fn ingest_fleet(&mut self, fleet: &FleetSnapshot, map_w: u32, my_id: u16) {
        let (prev_x, prev_y, curr_x, curr_y, progress) = if matches!(
            fleet.unit_type,
            UnitType::TransportShip | UnitType::TradeShip
        ) && fleet.path_cursor > 0
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
        let traveled = fleet.path_cursor.min(fleet.path.len()).saturating_sub(1);
        if fleet.unit_type != UnitType::TradeShip && traveled > 0 {
            let start = traveled.saturating_sub(TRAIL_CAP - 1);
            for &tile in &fleet.path[start..=traveled] {
                let (wx, wy) = tile_to_world(tile, map_w);
                self.trail_points.push([wx, wy]);
            }
        }
        let trail_len = self.trail_points.len() as u32 - trail_start;

        let entry = MoverSlot {
            id: 0,
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
            is_shell: false,
            warhead: false,
            arc_trail: false,
            is_owned_warship: fleet.unit_type == UnitType::Warship && fleet.owner_id == my_id,
            health_fraction: (fleet.troops / fleet.max_health.max(1.0)).clamp(0.0, 1.0) as f32,
            veterancy: fleet.warship.as_ref().map_or(0, |state| state.veterancy),
            warship_mode: fleet.warship.as_ref().map(|state| state.mode),
            in_combat: fleet
                .warship
                .as_ref()
                .is_some_and(|state| state.target_fleet.is_some()),
        };
        self.upsert_slot(fleet.id, entry);
    }

    fn ingest_projectile(
        &mut self,
        proj: &ProjectileSnapshot,
        map_w: u32,
        map_h: u32,
        min_height: f32,
    ) {
        let cursor = proj.path_cursor.min(proj.path.len().saturating_sub(1));
        let prev_idx = cursor.saturating_sub(proj.steps_per_tick as usize);
        let (curr_x, curr_y) = tile_to_world(proj.path[cursor], map_w);
        let (prev_x, prev_y) = tile_to_world(proj.path[prev_idx], map_w);

        let path_len = proj.path.len();
        let progress_curr = flight_progress(path_len, cursor as f32);
        let progress_prev = flight_progress(path_len, prev_idx as f32);

        let (sprite, size, trail_color) = match proj.kind {
            ProjectileKind::Nuke { level } => {
                let sprite = match level {
                    3.. => MoverSpriteId::Mirv,
                    2 => MoverSpriteId::HydrogenBomb,
                    _ => MoverSpriteId::AtomBomb,
                };
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
            ProjectileKind::Shell => (
                MoverSpriteId::SamMissile,
                0.22 * SHELL_SPRITE_SCALE,
                [0.94, 0.97, 1.0, 0.82],
            ),
        };

        let is_nuke = matches!(proj.kind, ProjectileKind::Nuke { .. });
        let key = proj.id | (1u64 << 63);
        if is_nuke {
            self.nuke_arcs.entry(key).or_insert_with(|| {
                NukeArcPath::new(proj.src_tile, proj.dst_tile, map_w, map_h, min_height)
            });
        } else {
            self.arc_paths
                .insert(key, std::sync::Arc::clone(&proj.path));
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
            id: 0,
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
            is_shell: matches!(proj.kind, ProjectileKind::Shell),
            warhead: matches!(proj.kind, ProjectileKind::Nuke { level: 4 }),
            arc_trail: is_nuke,
            is_owned_warship: false,
            health_fraction: 1.0,
            veterancy: 0,
            warship_mode: None,
            in_combat: false,
        };
        self.upsert_slot(key, entry);
    }

    fn upsert_slot(&mut self, id: u64, mut entry: MoverSlot) {
        entry.id = id;
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
        thermal: bool,
        trail_length: f32,
        tail_fade: f32,
    ) {
        if points.is_empty() || trail_length <= 0.0 {
            return;
        }
        let count = ((points.len() as f32 * trail_length.clamp(0.0, 1.0)).ceil() as usize)
            .clamp(1, points.len());
        let points = &points[points.len() - count..];
        let mut prev = points[0];
        for (i, pt) in points[1..].iter().enumerate() {
            let mut segment_color = color;
            let fraction = (i + 1) as f32 / (count - 1).max(1) as f32;
            if thermal {
                let heat = fraction * fraction * fraction;
                segment_color[0] = 1.0;
                segment_color[1] += (0.94 - color[1]) * heat;
                segment_color[2] += (0.67 - color[2]) * heat;
            }
            if fade_tail {
                let fade = (i + 1) as f32 / points.len() as f32;
                segment_color[3] = (segment_color[3] * (1.0 + (fade - 1.0) * tail_fade)).max(0.0);
            }
            renderer.push_trail_segment(TrailSegmentGpu {
                p0: prev,
                p1: *pt,
                width: if thermal {
                    width * (0.35 + 0.8 * fraction)
                } else {
                    width
                },
                color: segment_color,
            });
            prev = *pt;
        }
        if prev == head {
            return;
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
        params: &MoverPackParams<'_>,
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
        params: &MoverPackParams<'_>,
        renderer: &mut crate::render::gpu::MoverRenderer,
    ) {
        let setting = |id: &str, fallback: f32| {
            sow_render::shader_params::shader_slot(id)
                .map(|slot| params.shader_params[slot][0])
                .unwrap_or(fallback)
        };
        let margin = setting("mover.viewport_margin", 64.0);
        let sprite_uniform_scale = setting("mover.sprite.scale", 1.0);
        let trail_length = setting("mover.trail.length", 1.0);
        let tail_fade = setting("mover.trail.fade", 1.0);
        let fleet_trail_gain = setting("mover.trail.fleet_width", 0.4);
        let fleet_trail_min = setting("mover.trail.fleet_min", 1.0);
        let fleet_trail_max = setting("mover.trail.fleet_max", 6.0);
        let projectile_trail_zoom = setting("mover.trail.projectile_zoom", 0.5);
        let projectile_trail_base = setting("mover.trail.projectile_base", 2.0);
        let projectile_trail_min = setting("mover.trail.projectile_min", 3.0);
        let projectile_trail_max = setting("mover.trail.projectile_max", 10.0);
        let (fleet_trail_min, fleet_trail_max) = (
            fleet_trail_min.min(fleet_trail_max),
            fleet_trail_min.max(fleet_trail_max),
        );
        let (projectile_trail_min, projectile_trail_max) = (
            projectile_trail_min.min(projectile_trail_max),
            projectile_trail_min.max(projectile_trail_max),
        );
        let arc_samples = setting("mover.nuke.arc_samples", NUKE_ARC_SAMPLES as f32)
            .round()
            .clamp(8.0, 512.0) as usize;
        let arc_peak = setting("mover.nuke.arc_peak", 4.0);
        let height_scale = setting("mover.height_scale", 0.5);
        let height_limit = setting("mover.height_limit", 2.0);
        let min_transport_px = setting("mover.transport.minimum_px", MIN_TRANSPORT_SCREEN_PX);
        let min_trade_ship_px = setting("mover.trade_ship.minimum_px", 16.0);
        let min_warship_px = setting("mover.warship.minimum_px", 16.0);
        let min_nuke_px = setting("mover.nuke.minimum_px", 16.0);
        let min_sam_px = setting("mover.sam.minimum_px", 11.0);
        let transport_scale = setting("mover.transport.scale", 1.0);
        let trade_ship_scale = setting("mover.trade_ship.scale", 1.0);
        let warship_scale = setting("mover.warship.scale", 1.0);
        let projectile_scale = setting("mover.projectile.scale", 1.0);
        let nuke_scale = setting("mover.nuke.scale", 1.0);
        let sam_scale = setting("mover.sam.scale", 1.0);
        renderer.begin_frame();
        self.launch_pulses
            .retain(|pulse| pulse.started.elapsed().as_secs_f32() < 0.6);
        if params.trails_enabled {
            for pulse in &self.launch_pulses {
                let sx = params.camera_x + pulse.center[0] * params.camera_zoom;
                let sy = params.camera_y + pulse.center[1] * params.camera_zoom;
                if !in_viewport(
                    sx,
                    sy,
                    -24.0,
                    -24.0,
                    params.screen_w + 24.0,
                    params.screen_h + 24.0,
                ) {
                    continue;
                }
                let t = pulse.started.elapsed().as_secs_f32().min(0.6) / 0.6;
                let radius = (9.0 + 13.0 * t) / params.camera_zoom.max(0.001);
                let [r, g, b] = self
                    .player_colors
                    .get(&pulse.owner_id)
                    .copied()
                    .unwrap_or([1.0; 3]);
                let point = |segment: usize| {
                    let angle =
                        std::f32::consts::TAU * segment as f32 / FLEET_LOCATE_RING_SEGMENTS as f32;
                    [
                        pulse.center[0] + angle.cos() * radius,
                        pulse.center[1] + angle.sin() * radius,
                    ]
                };
                for segment in 0..FLEET_LOCATE_RING_SEGMENTS {
                    renderer.push_trail_segment(TrailSegmentGpu {
                        p0: point(segment),
                        p1: point(segment + 1),
                        width: 2.0,
                        color: [r, g, b, 0.85 * (1.0 - t)],
                    });
                }
            }
        }
        let mut arc_scratch = std::mem::take(&mut self.arc_scratch);
        let located_fleet = self.located_fleet.as_ref().and_then(|(fleet_id, started)| {
            let elapsed = started.elapsed().as_secs_f32();
            (elapsed < FLEET_LOCATE_DURATION_SECS).then_some((*fleet_id, elapsed))
        });
        if located_fleet.is_none() {
            self.located_fleet = None;
        }

        // At salvo density, silhouettes closer than one sector read as one formation.
        let dense = self.nuke_arcs.len() > 128;
        let mut occupied = [0u32; 16];
        for slot in &self.slots {
            let id = &slot.id;
            arc_scratch.clear();

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

            if dense && slot.arc_trail {
                let x = wx * params.camera_zoom + params.camera_x;
                let y = wy * params.camera_zoom + params.camera_y;
                if !in_viewport(
                    x,
                    y,
                    -40.0,
                    -40.0,
                    params.screen_w + 40.0,
                    params.screen_h + 40.0,
                ) {
                    continue;
                }
                let col = ((x * 24.0 / params.screen_w.max(1.0)) as usize).min(23);
                let row = ((y * 16.0 / params.screen_h.max(1.0)) as usize).min(15);
                let bit = 1 << col;
                if occupied[row] & bit != 0 {
                    continue;
                }
                occupied[row] |= bit;
            }
            let height = if slot.is_fleet {
                0.0
            } else {
                nuke_arc_height(progress, arc_peak)
            };
            let world_pos = [wx, wy];

            let scale = if slot.is_fleet {
                1.0
            } else {
                (1.0 + height * height_scale).min(height_limit)
            };
            let class_scale = match slot.sprite {
                MoverSpriteId::TransportShip => transport_scale,
                MoverSpriteId::TradeShip => trade_ship_scale,
                MoverSpriteId::Warship => warship_scale,
                MoverSpriteId::AtomBomb | MoverSpriteId::HydrogenBomb | MoverSpriteId::Mirv => {
                    projectile_scale * nuke_scale
                }
                MoverSpriteId::SamMissile => projectile_scale * sam_scale,
            };
            let mut sprite_size = slot.size * scale * class_scale;
            let minimum_screen_size = match slot.sprite {
                MoverSpriteId::TransportShip => min_transport_px,
                MoverSpriteId::TradeShip => min_trade_ship_px,
                MoverSpriteId::Warship => min_warship_px,
                MoverSpriteId::AtomBomb | MoverSpriteId::HydrogenBomb | MoverSpriteId::Mirv => {
                    min_nuke_px
                }
                MoverSpriteId::SamMissile if slot.is_shell => min_sam_px * SHELL_SPRITE_SCALE,
                MoverSpriteId::SamMissile => min_sam_px,
            };
            if minimum_screen_size > 0.0 && sprite_size * params.camera_zoom < minimum_screen_size {
                sprite_size = minimum_screen_size / params.camera_zoom;
            }
            let locate_age = located_fleet
                .filter(|(fleet_id, _)| slot.is_fleet && fleet_id == id)
                .map(|(_, age)| age);
            let locate_pulse = locate_age
                .map(|age| 0.5 + 0.5 * (age * std::f32::consts::TAU * 1.2).sin())
                .unwrap_or(0.0);
            let locate_strength = locate_age
                .map(|age| {
                    (1.0 - age / FLEET_LOCATE_DURATION_SECS).clamp(0.0, 1.0)
                        * (0.7 + 0.3 * locate_pulse)
                })
                .unwrap_or(0.0);
            sprite_size *= 1.0 + locate_strength * 0.08;

            let sx = params.camera_x + wx * params.camera_zoom;
            let sy = params.camera_y + world_pos[1] * params.camera_zoom;
            let sprite_margin = margin
                + (sprite_size * params.camera_zoom * sprite_uniform_scale).abs()
                    * std::f32::consts::FRAC_1_SQRT_2;
            let sprite_visible = in_viewport(
                sx,
                sy,
                -sprite_margin,
                -sprite_margin,
                params.screen_w + sprite_margin,
                params.screen_h + sprite_margin,
            );

            let trail_width = if slot.is_fleet {
                (params.camera_zoom * fleet_trail_gain).clamp(fleet_trail_min, fleet_trail_max)
            } else {
                (params.camera_zoom * projectile_trail_zoom + projectile_trail_base)
                    .clamp(projectile_trail_min, projectile_trail_max)
            };
            let trail_margin = margin + trail_width.abs() * 0.5;
            let trail_bounds = (
                -trail_margin,
                -trail_margin,
                params.screen_w + trail_margin,
                params.screen_h + trail_margin,
            );

            if params.trails_enabled && slot.arc_trail {
                if let Some(path) = self.nuke_arcs.get(id) {
                    let bounds = path.bounds;
                    let arc_visible = params.camera_x + bounds[2] * params.camera_zoom
                        >= -trail_margin
                        && params.camera_x + bounds[0] * params.camera_zoom
                            <= params.screen_w + trail_margin
                        && params.camera_y + bounds[3] * params.camera_zoom >= -trail_margin
                        && params.camera_y + bounds[1] * params.camera_zoom
                            <= params.screen_h + trail_margin;
                    if arc_visible {
                        let (samples, window) = if dense {
                            // Constant screen-space exhaust length even for map-spanning flights.
                            let screen_length = path.lengths[16] * params.camera_zoom;
                            (3, (32.0 / screen_length.max(1.0)).min(0.35))
                        } else {
                            let cap = arc_samples.min(12000 / self.nuke_arcs.len().max(1));
                            (
                                adaptive_nuke_arc_samples(bounds, params.camera_zoom, cap),
                                if slot.warhead { 0.35 } else { 1.0 },
                            )
                        };
                        sample_nuke_arc(
                            path,
                            (progress - window).max(0.0),
                            progress,
                            samples,
                            &mut arc_scratch,
                        );
                        if self.arc_visible(&arc_scratch, world_pos, params, trail_bounds) {
                            self.push_trail_segments(
                                renderer,
                                &arc_scratch,
                                world_pos,
                                trail_width,
                                slot.trail_color,
                                true,
                                true,
                                trail_length,
                                tail_fade,
                            );
                        }
                    }
                }
            } else if params.trails_enabled && slot.trail_len > 0 {
                let start = slot.trail_start as usize;
                let end = start + slot.trail_len as usize;
                let trail_points = &self.trail_points[start..end];
                if self.arc_visible(trail_points, world_pos, params, trail_bounds) {
                    self.push_trail_segments(
                        renderer,
                        trail_points,
                        world_pos,
                        trail_width,
                        locate_highlight_color(slot.trail_color, locate_strength),
                        slot.is_fleet,
                        false,
                        trail_length,
                        tail_fade,
                    );
                }
            }

            let ring_margin = margin + (sprite_size * 1.55 * params.camera_zoom).abs();
            let ring_visible = in_viewport(
                sx,
                sy,
                -ring_margin,
                -ring_margin,
                params.screen_w + ring_margin,
                params.screen_h + ring_margin,
            );

            if locate_strength > 0.02 && ring_visible {
                arc_scratch.clear();
                let radius = sprite_size * (0.78 + locate_pulse * 0.16);
                for segment in 0..FLEET_LOCATE_RING_SEGMENTS {
                    let angle =
                        std::f32::consts::TAU * segment as f32 / FLEET_LOCATE_RING_SEGMENTS as f32;
                    arc_scratch.push([wx + angle.cos() * radius, wy + angle.sin() * radius]);
                }
                let ring_head = arc_scratch[0];
                self.push_trail_segments(
                    renderer,
                    &arc_scratch,
                    ring_head,
                    1.2 + locate_strength,
                    [1.0, 0.92, 0.68, 0.72 * locate_strength],
                    false,
                    false,
                    1.0,
                    1.0,
                );
                arc_scratch.clear();
            }

            if slot.is_owned_warship {
                if ring_visible {
                    arc_scratch.clear();
                    let status_color = if slot.in_combat {
                        [1.0, 0.48, 0.28, 0.9]
                    } else {
                        match slot.warship_mode {
                            Some(sow_core::warp_fleet::WarshipMode::Retreating) => {
                                [1.0, 0.35, 0.28, 0.82]
                            }
                            Some(sow_core::warp_fleet::WarshipMode::Docked) => {
                                [0.35, 0.95, 0.78, 0.78]
                            }
                            _ => [0.78, 0.9, 1.0, 0.66],
                        }
                    };
                    let radius = sprite_size * 0.80;
                    let segments = 14;
                    for segment in 0..=segments {
                        let angle = std::f32::consts::TAU * segment as f32 / segments as f32;
                        arc_scratch.push([wx + angle.cos() * radius, wy + angle.sin() * radius]);
                    }
                    self.push_trail_segments(
                        renderer,
                        &arc_scratch,
                        arc_scratch[0],
                        0.9,
                        status_color,
                        false,
                        false,
                        1.0,
                        0.0,
                    );
                    arc_scratch.clear();
                    if slot.health_fraction < 0.999 {
                        let segments = 12;
                        for segment in 0..=segments {
                            let angle = std::f32::consts::TAU * segment as f32 / segments as f32;
                            let radius = radius * (1.18 + f32::from(slot.veterancy) * 0.06);
                            arc_scratch
                                .push([wx + angle.cos() * radius, wy + angle.sin() * radius]);
                        }
                        let visible = ((segments as f32) * slot.health_fraction).ceil() as usize;
                        if visible > 0 {
                            self.push_trail_segments(
                                renderer,
                                &arc_scratch[..=visible.min(segments)],
                                arc_scratch[visible.min(segments)],
                                1.4,
                                [
                                    0.3 + 0.7 * (1.0 - slot.health_fraction),
                                    0.85 * slot.health_fraction,
                                    0.35,
                                    0.9,
                                ],
                                false,
                                false,
                                1.0,
                                0.0,
                            );
                        }
                        arc_scratch.clear();
                    }
                }
            }

            if !params.sprites_enabled || !sprite_visible {
                continue;
            }

            let dx = slot.curr_x - slot.prev_x;
            let dy = slot.curr_y - slot.prev_y;
            let rotation = if slot.is_fleet {
                0.0
            } else if let Some(path) = self.nuke_arcs.get(id) {
                path.heading(progress)
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

            renderer.push_sprite(MoverInstanceGpu {
                world_pos: [wx, wy],
                size: sprite_size,
                rotation,
                color: locate_highlight_color(slot.color, locate_strength),
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
    use super::*;

    fn dummy_slot(id: u64) -> MoverSlot {
        MoverSlot {
            id,
            prev_x: 0.0,
            prev_y: 0.0,
            curr_x: id as f32,
            curr_y: 0.0,
            path_progress_prev: 0.0,
            path_progress_curr: 0.0,
            size: 1.0,
            color: [1.0; 4],
            trail_color: [1.0; 4],
            sprite: MoverSpriteId::AtomBomb,
            trail_start: 0,
            trail_len: 0,
            is_fleet: false,
            is_shell: false,
            warhead: false,
            arc_trail: true,
            is_owned_warship: false,
            health_fraction: 1.0,
            veterancy: 0,
            warship_mode: None,
            in_combat: false,
        }
    }

    #[test]
    fn adaptive_nuke_arc_samples_scales_down_on_far_zoom_and_caps_at_max() {
        let bounds = [0.0, 0.0, 120.0, 0.0];
        // Tiny on-screen arc (120 * 0.25 = 30px) clamps to 12 segments instead of 128.
        assert_eq!(adaptive_nuke_arc_samples(bounds, 0.25, 128), 12);
        // Medium on-screen arc (120 * 2.0 = 240px -> 40 segments).
        assert_eq!(adaptive_nuke_arc_samples(bounds, 2.0, 128), 40);
        // Zoomed close respects max_samples cap.
        assert_eq!(adaptive_nuke_arc_samples(bounds, 10.0, 128), 128);
    }

    #[test]
    fn swap_remove_dead_slots_keeps_id_to_idx_consistent_in_o1() {
        let mut scene = MoverScene::new();
        for id in 10..=14u64 {
            scene.upsert_slot(id, dummy_slot(id));
        }
        scene.remove_dead_slots(&[10, 12]);
        assert_eq!(scene.slots.len(), 3);
        for id in [11u64, 13, 14] {
            let idx = *scene.id_to_idx.get(&id).expect("surviving id present");
            assert_eq!(scene.slots[idx as usize].id, id);
            assert_eq!(scene.slots[idx as usize].curr_x, id as f32);
        }
        assert!(!scene.id_to_idx.contains_key(&10));
        assert!(!scene.id_to_idx.contains_key(&12));
    }
}
