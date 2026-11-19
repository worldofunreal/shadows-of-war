use sow_core::bitset::DenseBitSet;
use sow_core::protocol::BuildingSnapshot;
use sow_render::{
    BLAST_KIND_BUILDING_POP, BLAST_KIND_GROUND_SCORCH, BLAST_KIND_NUKE_AIR,
    BLAST_KIND_SAM_AIRBURST, BlastInstanceGpu, BlastRenderer, MAX_BLAST_INSTANCES,
    crater_front_fraction,
};
use web_time::{Duration, Instant};

const GROUND_SCORCH_LIFETIME_SECS: f32 = 5.5;
const NUKE_AIR_LIFETIME_SECS: f32 = 2.35;
const SAM_AIRBURST_LIFETIME_SECS: f32 = 0.48;
const SAM_AIRBURST_RADIUS: f32 = 6.5;
const BUILDING_POP_LIFETIME_SECS: f32 = 0.34;
const BUILDING_POP_RADIUS: f32 = 3.2;
const BUILDING_FLASH_SECS: f32 = 0.08;
const CAMERA_SHAKE_DURATION_SECS: f32 = 0.30;
const MAX_DYING_BUILDINGS: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlastKind {
    GroundScorch,
    NukeAir,
    SamAirburst,
    BuildingPop,
}

impl BlastKind {
    #[inline]
    fn gpu_code(self) -> f32 {
        match self {
            Self::GroundScorch => BLAST_KIND_GROUND_SCORCH,
            Self::NukeAir => BLAST_KIND_NUKE_AIR,
            Self::SamAirburst => BLAST_KIND_SAM_AIRBURST,
            Self::BuildingPop => BLAST_KIND_BUILDING_POP,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ActiveBlast {
    world_x: f32,
    world_y: f32,
    tile_idx: u32,
    owner_id: u16,
    inner_radius: f32,
    outer_radius: f32,
    start_time: Instant,
    expand_dur: f32,
    lifetime_secs: f32,
    kind: BlastKind,
    seed: f32,
}

impl ActiveBlast {
    fn bound_radius(&self, age: f32) -> f32 {
        match self.kind {
            BlastKind::GroundScorch => {
                let hole_frac = if self.expand_dur > 0.001 {
                    crater_front_fraction(age / self.expand_dur)
                } else {
                    1.0
                };
                (self.inner_radius * hole_frac + 2.8).min(self.inner_radius + 3.0)
            }
            BlastKind::NukeAir => {
                let eff_expand = self.expand_dur.max(0.28);
                let wave_dur = (eff_expand + 0.45).max(0.72);
                let core_bound = self.inner_radius * 1.52 + 3.0;
                if age < wave_dur {
                    let wt = (age / wave_dur).clamp(0.0, 1.0);
                    let wave_r = self.outer_radius * (1.0 - (1.0 - wt).powf(2.6));
                    let wave_pad = (self.outer_radius * 0.14).max(3.5);
                    core_bound.max(wave_r + wave_pad)
                } else {
                    core_bound
                }
            }
            BlastKind::SamAirburst => {
                let t = (age / self.lifetime_secs.max(0.001)).clamp(0.0, 1.0);
                let ease = 1.0 - (1.0 - t) * (1.0 - t);
                self.outer_radius * (0.35 + 0.65 * ease)
            }
            BlastKind::BuildingPop => {
                let t = (age / self.lifetime_secs.max(0.001)).clamp(0.0, 1.0);
                let ease = 1.0 - (1.0 - t) * (1.0 - t);
                self.outer_radius * (0.40 + 0.60 * ease)
            }
        }
    }
}

/// Snapshot metadata for a nuke detonation that occurred during the current tick,
/// consumed in `apply_snapshot_fx` to match destroyed buildings for wave-synced pops.
#[derive(Clone, Copy, Debug)]
pub struct RecentNukeDetonation {
    pub tile_x: u32,
    pub tile_y: u32,
    pub inner_radius: u32,
    pub owner_id: u16,
    pub start_time: Instant,
    pub expand_dur: f32,
}

/// A building destroyed by a nuke, kept briefly visible until the outward-expanding
/// shockwave front reaches its tile, then flashed incandescently and popped.
#[derive(Clone, Debug)]
pub struct DyingBuilding {
    pub building: BuildingSnapshot,
    pub world_x: f32,
    pub world_y: f32,
    pub owner_id: u16,
    pub trigger_at: Instant,
    pub popped: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct CameraShakeState {
    start_time: Option<Instant>,
    amplitude_px: f32,
    seed: f32,
}

pub struct BlastPackParams<'a> {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub fog_of_war: bool,
    pub my_player_id: u16,
    pub fog_visible: &'a DenseBitSet,
    pub now: Instant,
}

#[derive(Default)]
pub struct BlastScene {
    ground_blasts: Vec<ActiveBlast>,
    air_blasts: Vec<ActiveBlast>,
    dying_buildings: Vec<DyingBuilding>,
    recent_detonations: Vec<RecentNukeDetonation>,
    shake: CameraShakeState,
    next_seed: u32,
}

impl BlastScene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.ground_blasts.clear();
        self.air_blasts.clear();
        self.dying_buildings.clear();
        self.recent_detonations.clear();
        self.shake = CameraShakeState::default();
    }

    #[inline]
    pub fn has_active(&self) -> bool {
        !self.ground_blasts.is_empty()
            || !self.air_blasts.is_empty()
            || !self.dying_buildings.is_empty()
    }

    #[inline]
    pub fn has_recent_detonations(&self) -> bool {
        !self.recent_detonations.is_empty()
    }

    fn alloc_seed(&mut self) -> f32 {
        self.next_seed = self.next_seed.wrapping_add(1);
        let mut h = self.next_seed.wrapping_mul(0x9E37_79B9);
        h ^= h >> 15;
        h = h.wrapping_mul(0x85EB_CA6B);
        h ^= h >> 13;
        (h & 0x00FF_FFFF) as f32 * (1.0 / 16_777_215.0)
    }

    /// Spawn a nuke detonation's visual layers (ground scorch + aerial fireball/shockwave/embers)
    /// and trigger a subtle proximity-scaled camera impulse if visible and motion is allowed.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_nuke_detonation(
        &mut self,
        tile_x: u32,
        tile_y: u32,
        inner_radius: u32,
        outer_radius: u32,
        owner_id: u16,
        map_w: u32,
        expand_dur: f32,
        now: Instant,
        reduced_motion: bool,
        camera: (f32, f32, f32),
        screen: (f32, f32),
        sf: f32,
    ) {
        let world_x = tile_x as f32 + 0.5;
        let world_y = tile_y as f32 + 0.5;
        let tile_idx = tile_y.saturating_mul(map_w).saturating_add(tile_x);
        let inner_f = inner_radius.max(1) as f32;
        let outer_f = outer_radius.max(inner_radius.saturating_add(4)) as f32;
        let seed = self.alloc_seed();

        Self::push_capped(
            &mut self.ground_blasts,
            ActiveBlast {
                world_x,
                world_y,
                tile_idx,
                owner_id,
                inner_radius: inner_f,
                outer_radius: outer_f,
                start_time: now,
                expand_dur,
                lifetime_secs: GROUND_SCORCH_LIFETIME_SECS,
                kind: BlastKind::GroundScorch,
                seed,
            },
        );

        Self::push_capped(
            &mut self.air_blasts,
            ActiveBlast {
                world_x,
                world_y,
                tile_idx,
                owner_id,
                inner_radius: inner_f,
                outer_radius: outer_f,
                start_time: now,
                expand_dur,
                lifetime_secs: NUKE_AIR_LIFETIME_SECS,
                kind: BlastKind::NukeAir,
                seed,
            },
        );

        if expand_dur > 0.0 && !reduced_motion {
            self.recent_detonations.push(RecentNukeDetonation {
                tile_x,
                tile_y,
                inner_radius,
                owner_id,
                start_time: now,
                expand_dur,
            });
            self.trigger_camera_shake(world_x, world_y, inner_f, now, camera, screen, sf);
        }
    }

    /// Spawn a crisp white-cyan mid-air SAM interception burst at `tile_idx`.
    pub fn spawn_sam_airburst(&mut self, tile_idx: u32, map_w: u32, now: Instant) {
        if map_w == 0 {
            return;
        }
        let world_x = (tile_idx % map_w) as f32 + 0.5;
        let world_y = (tile_idx / map_w) as f32 + 0.5;
        let seed = self.alloc_seed();
        Self::push_capped(
            &mut self.air_blasts,
            ActiveBlast {
                world_x,
                world_y,
                tile_idx,
                owner_id: 0,
                inner_radius: SAM_AIRBURST_RADIUS * 0.5,
                outer_radius: SAM_AIRBURST_RADIUS,
                start_time: now,
                expand_dur: 0.0,
                lifetime_secs: SAM_AIRBURST_LIFETIME_SECS,
                kind: BlastKind::SamAirburst,
                seed,
            },
        );
    }

    /// Match vanished buildings against this tick's nuke detonations so they remain
    /// intact for ~50–500ms until the expanding shockwave crosses their tile.
    pub fn register_dying_buildings(
        &mut self,
        vanished: &[BuildingSnapshot],
        map_w: u32,
    ) {
        if self.recent_detonations.is_empty() || map_w == 0 {
            self.recent_detonations.clear();
            return;
        }
        for b in vanished {
            let bx = b.tile_idx % map_w;
            let by = b.tile_idx / map_w;
            let mut best_hit: Option<(Instant, u16)> = None;
            for det in &self.recent_detonations {
                let dx = bx as i64 - det.tile_x as i64;
                let dy = by as i64 - det.tile_y as i64;
                let dist_sq = dx * dx + dy * dy;
                let r_sq = i64::from(det.inner_radius) * i64::from(det.inner_radius);
                if dist_sq > r_sq {
                    continue;
                }
                let dist = (dist_sq as f32).sqrt();
                let trigger_at = building_wave_hit_time(
                    det.start_time,
                    det.expand_dur,
                    dist,
                    det.inner_radius as f32,
                );
                if best_hit.is_none_or(|(prev, _)| trigger_at < prev) {
                    best_hit = Some((trigger_at, det.owner_id));
                }
            }
            if let Some((trigger_at, owner_id)) = best_hit {
                if self.dying_buildings.len() >= MAX_DYING_BUILDINGS {
                    self.dying_buildings.remove(0);
                }
                self.dying_buildings.push(DyingBuilding {
                    building: b.clone(),
                    world_x: bx as f32 + 0.5,
                    world_y: by as f32 + 0.5,
                    owner_id,
                    trigger_at,
                    popped: false,
                });
            }
        }
        self.recent_detonations.clear();
    }

    /// Active dying buildings for `overlays.rs` to draw before they finish popping.
    /// Returns `(building, world_x, world_y, flash_progress)` where `flash_progress` is
    /// `None` before the shockwave hits and `Some(0.0..1.0)` during the 80ms pop flash.
    pub fn dying_buildings_for_render(
        &self,
        now: Instant,
    ) -> impl Iterator<Item = (&BuildingSnapshot, f32, f32, Option<f32>)> {
        self.dying_buildings.iter().filter_map(move |db| {
            if now < db.trigger_at {
                Some((&db.building, db.world_x, db.world_y, None))
            } else {
                let flash_age = now.saturating_duration_since(db.trigger_at).as_secs_f32();
                if flash_age < BUILDING_FLASH_SECS {
                    Some((
                        &db.building,
                        db.world_x,
                        db.world_y,
                        Some((flash_age / BUILDING_FLASH_SECS).clamp(0.0, 1.0)),
                    ))
                } else {
                    None
                }
            }
        })
    }

    /// Render-only camera shake offset in physical pixels (never mutates persistent camera state).
    pub fn camera_shake_offset(&mut self, now: Instant, reduced_motion: bool) -> [f32; 2] {
        if reduced_motion {
            self.shake.start_time = None;
            return [0.0, 0.0];
        }
        let Some(start) = self.shake.start_time else {
            return [0.0, 0.0];
        };
        let elapsed = now.saturating_duration_since(start).as_secs_f32();
        if elapsed >= CAMERA_SHAKE_DURATION_SECS {
            self.shake.start_time = None;
            return [0.0, 0.0];
        }
        let decay = (-11.0 * elapsed).exp() * (1.0 - elapsed / CAMERA_SHAKE_DURATION_SECS);
        let amp = self.shake.amplitude_px * decay;
        let phase = self.shake.seed * std::f32::consts::TAU;
        [
            (elapsed * 58.0 + phase).sin() * amp,
            (elapsed * 67.0 - phase * 1.3).cos() * amp,
        ]
    }

    /// Advance timers, trigger wave-synced building pops, frustum-cull, and upload active
    /// ground & aerial blast instances into `renderer`.
    pub fn pack_gpu(&mut self, params: &BlastPackParams<'_>, renderer: &mut BlastRenderer) {
        renderer.begin_frame();
        self.advance_dying_buildings(params.now);

        self.ground_blasts.retain(|b| {
            params.now.saturating_duration_since(b.start_time).as_secs_f32() < b.lifetime_secs
        });
        self.air_blasts.retain(|b| {
            params.now.saturating_duration_since(b.start_time).as_secs_f32() < b.lifetime_secs
        });

        if self.ground_blasts.is_empty() && self.air_blasts.is_empty() {
            return;
        }

        let zoom = params.camera_zoom.max(0.0001);
        let view_min_x = -params.camera_x / zoom;
        let view_max_x = (params.screen_w - params.camera_x) / zoom;
        let view_min_y = -params.camera_y / zoom;
        let view_max_y = (params.screen_h - params.camera_y) / zoom;

        for b in &self.ground_blasts {
            if let Some(inst) = Self::build_gpu_instance(
                b,
                params,
                (view_min_x, view_max_x, view_min_y, view_max_y),
            ) {
                renderer.push_ground(inst);
            }
        }
        for b in &self.air_blasts {
            if let Some(inst) = Self::build_gpu_instance(
                b,
                params,
                (view_min_x, view_max_x, view_min_y, view_max_y),
            ) {
                renderer.push_air(inst);
            }
        }
    }

    fn advance_dying_buildings(&mut self, now: Instant) {
        if self.dying_buildings.is_empty() {
            return;
        }
        let mut newly_popped = Vec::new();
        for db in &mut self.dying_buildings {
            if !db.popped && now >= db.trigger_at {
                db.popped = true;
                newly_popped.push((db.world_x, db.world_y, db.building.tile_idx, db.owner_id));
            }
        }
        for (world_x, world_y, tile_idx, owner_id) in newly_popped {
            let seed = self.alloc_seed();
            Self::push_capped(
                &mut self.air_blasts,
                ActiveBlast {
                    world_x,
                    world_y,
                    tile_idx,
                    owner_id,
                    inner_radius: BUILDING_POP_RADIUS * 0.5,
                    outer_radius: BUILDING_POP_RADIUS,
                    start_time: now,
                    expand_dur: 0.0,
                    lifetime_secs: BUILDING_POP_LIFETIME_SECS,
                    kind: BlastKind::BuildingPop,
                    seed,
                },
            );
        }
        self.dying_buildings.retain(|db| {
            now < db.trigger_at
                || now.saturating_duration_since(db.trigger_at).as_secs_f32() < BUILDING_FLASH_SECS
        });
    }

    fn build_gpu_instance(
        b: &ActiveBlast,
        params: &BlastPackParams<'_>,
        view_bounds: (f32, f32, f32, f32),
    ) -> Option<BlastInstanceGpu> {
        if params.fog_of_war
            && b.owner_id != 0
            && b.owner_id != params.my_player_id
            && !params.fog_visible.contains(b.tile_idx)
        {
            return None;
        }
        let age = params.now.saturating_duration_since(b.start_time).as_secs_f32();
        if age >= b.lifetime_secs {
            return None;
        }
        let bound_radius = b.bound_radius(age);
        let (min_x, max_x, min_y, max_y) = view_bounds;
        if b.world_x + bound_radius < min_x
            || b.world_x - bound_radius > max_x
            || b.world_y + bound_radius < min_y
            || b.world_y - bound_radius > max_y
        {
            return None;
        }
        Some(BlastInstanceGpu {
            world_pos: [b.world_x, b.world_y],
            bound_radius,
            inner_radius: b.inner_radius,
            outer_radius: b.outer_radius,
            age_secs: age,
            expand_dur: b.expand_dur,
            lifetime_secs: b.lifetime_secs,
            kind: b.kind.gpu_code(),
            seed: b.seed,
        })
    }

    fn trigger_camera_shake(
        &mut self,
        world_x: f32,
        world_y: f32,
        inner_radius: f32,
        now: Instant,
        camera: (f32, f32, f32),
        screen: (f32, f32),
        sf: f32,
    ) {
        let (cam_x, cam_y, zoom) = camera;
        let (screen_w, screen_h) = screen;
        if screen_w <= 1.0 || screen_h <= 1.0 {
            return;
        }
        let sx = world_x * zoom + cam_x;
        let sy = world_y * zoom + cam_y;
        let dist_px = (sx - screen_w * 0.5).hypot(sy - screen_h * 0.5);
        let diag = screen_w.hypot(screen_h).max(1.0);
        let proximity = (1.0 - dist_px / (diag * 0.85)).clamp(0.0, 1.0);
        if proximity <= 0.02 {
            return;
        }
        let base_css_px = (2.8 + (inner_radius - 12.0).max(0.0) * 0.11).clamp(2.8, 6.2);
        let amp = base_css_px * sf.max(1.0) * proximity;
        let current_amp = self.shake.start_time.map_or(0.0, |start| {
            let elapsed = now.saturating_duration_since(start).as_secs_f32();
            if elapsed >= CAMERA_SHAKE_DURATION_SECS {
                0.0
            } else {
                self.shake.amplitude_px * (1.0 - elapsed / CAMERA_SHAKE_DURATION_SECS)
            }
        });
        if amp >= current_amp * 0.75 {
            self.shake = CameraShakeState {
                start_time: Some(now),
                amplitude_px: amp.max(current_amp),
                seed: self.alloc_seed(),
            };
        }
    }

    fn push_capped(list: &mut Vec<ActiveBlast>, blast: ActiveBlast) {
        if list.len() >= MAX_BLAST_INSTANCES {
            // Prefer evicting secondary pops/airbursts before dropping a fresh primary nuke blast.
            if let Some(pos) = list
                .iter()
                .position(|b| matches!(b.kind, BlastKind::BuildingPop | BlastKind::SamAirburst))
            {
                list.remove(pos);
            } else {
                list.remove(0);
            }
        }
        list.push(blast);
    }
}

/// Exact inverse of `crater_front_fraction(t) = 1 - (1 - t)^2.2`:
/// computes the timestamp when the outward shockwave front reaches distance `dist`.
pub fn building_wave_hit_time(
    start_time: Instant,
    expand_dur: f32,
    dist: f32,
    inner_radius: f32,
) -> Instant {
    if expand_dur <= 0.0 || inner_radius <= 0.0 {
        return start_time;
    }
    let frac = (dist / inner_radius).clamp(0.0, 1.0);
    let t_hit = 1.0 - (1.0 - frac).powf(1.0 / 2.2);
    start_time + Duration::from_secs_f32((t_hit * expand_dur).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_hit_time_matches_crater_front_fraction_inverse() {
        let t0 = Instant::now();
        let dur = 0.5;
        let inner = 20.0;
        let hit = building_wave_hit_time(t0, dur, 10.0, inner);
        let elapsed = hit.duration_since(t0).as_secs_f32();
        let front = inner * crater_front_fraction(elapsed / dur);
        assert!((front - 10.0).abs() < 0.05, "front={front}");
    }

    #[test]
    fn reduced_motion_suppresses_camera_shake_and_dying_buildings() {
        let mut scene = BlastScene::new();
        let t0 = Instant::now();
        scene.spawn_nuke_detonation(
            50,
            50,
            22,
            55,
            1,
            200,
            0.0,
            t0,
            true,
            (0.0, 0.0, 2.0),
            (1280.0, 720.0),
            1.0,
        );
        assert_eq!(scene.camera_shake_offset(t0 + Duration::from_millis(50), true), [0.0, 0.0]);
        assert!(!scene.has_recent_detonations());
    }

    #[test]
    fn cap_evicts_secondary_pops_before_primary_nuke_blasts() {
        let mut scene = BlastScene::new();
        let t0 = Instant::now();
        scene.spawn_nuke_detonation(
            10,
            10,
            12,
            30,
            1,
            100,
            0.4,
            t0,
            false,
            (0.0, 0.0, 2.0),
            (800.0, 600.0),
            1.0,
        );
        for i in 0..(MAX_BLAST_INSTANCES + 8) {
            scene.spawn_sam_airburst(i as u32, 100, t0);
        }
        assert_eq!(scene.air_blasts.len(), MAX_BLAST_INSTANCES);
        assert!(
            scene
                .air_blasts
                .iter()
                .any(|b| b.kind == BlastKind::NukeAir),
            "primary nuke blast should survive SAM burst overflow"
        );
    }

    #[test]
    fn frustum_culling_skips_offscreen_blasts() {
        let t0 = Instant::now();
        let blast = ActiveBlast {
            world_x: 500.0,
            world_y: 500.0,
            tile_idx: 0,
            owner_id: 1,
            inner_radius: 12.0,
            outer_radius: 30.0,
            start_time: t0,
            expand_dur: 0.45,
            lifetime_secs: NUKE_AIR_LIFETIME_SECS,
            kind: BlastKind::NukeAir,
            seed: 0.5,
        };
        let fog = DenseBitSet::new();
        let params = BlastPackParams {
            camera_x: 0.0,
            camera_y: 0.0,
            camera_zoom: 2.0,
            screen_w: 400.0,
            screen_h: 400.0,
            fog_of_war: false,
            my_player_id: 1,
            fog_visible: &fog,
            now: t0 + Duration::from_millis(50),
        };
        let offscreen = BlastScene::build_gpu_instance(&blast, &params, (0.0, 200.0, 0.0, 200.0));
        assert!(offscreen.is_none());
        let onscreen = BlastScene::build_gpu_instance(&blast, &params, (450.0, 550.0, 450.0, 550.0));
        assert!(onscreen.is_some());
    }
}
