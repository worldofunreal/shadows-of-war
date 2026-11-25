use sow_core::bitset::DenseBitSet;
use sow_core::game::BuildingKind;
use sow_core::protocol::BuildingSnapshot;
use sow_render::{
    BLAST_KIND_BUILDING_POP, BLAST_KIND_ENTITY_DEATH, BLAST_KIND_GROUND_SCORCH, BLAST_KIND_NUKE_AIR,
    BLAST_KIND_SAM_AIRBURST, BlastInstanceGpu, BlastRenderer, MAX_BLAST_INSTANCES,
    crater_front_fraction,
};
use web_time::{Duration, Instant};

const GROUND_SCORCH_LIFETIME_SECS: f32 = 5.5;
const NUKE_AIR_LIFETIME_SECS: f32 = 2.35;
const SAM_AIRBURST_LIFETIME_SECS: f32 = 0.48;
const SAM_AIRBURST_RADIUS: f32 = 6.5;
const BUILDING_POP_LIFETIME_SECS: f32 = 0.72;
pub const BUILDING_COLLAPSE_SECS: f32 = 0.62;
const ENTITY_DEATH_LIFETIME_SECS: f32 = 0.88;
const CAMERA_SHAKE_DURATION_SECS: f32 = 0.30;
const MAX_DYING_BUILDINGS: usize = 256;

#[inline]
pub fn pack_rgb_f32(rgb: [f32; 3]) -> f32 {
    let r = (rgb[0].clamp(0.0, 1.0) * 63.0).round() as u32;
    let g = (rgb[1].clamp(0.0, 1.0) * 63.0).round() as u32;
    let b = (rgb[2].clamp(0.0, 1.0) * 63.0).round() as u32;
    ((r << 12) | (g << 6) | b) as f32
}

#[inline]
fn building_demolition_tier_and_radius(kind: BuildingKind, level: u8) -> (f32, f32) {
    let lvl = level.max(1) as f32;
    match kind {
        BuildingKind::City | BuildingKind::MissileSilo => (2.0, (6.8 + lvl * 0.85).min(10.5)),
        BuildingKind::Port | BuildingKind::SAM | BuildingKind::TradingPost => {
            (1.0, (5.6 + lvl * 0.65).min(8.5))
        }
        BuildingKind::Tower | BuildingKind::MilitaryCamp => (0.0, (4.6 + lvl * 0.50).min(6.8)),
    }
}

#[inline]
fn building_footprint_world_center(kind: BuildingKind, tile_x: u32, tile_y: u32) -> (f32, f32) {
    let fp = sow_core::building::BuildingFootprint::at(kind, tile_x, tile_y);
    (
        fp.left as f32 + fp.width as f32 * 0.5,
        fp.top as f32 + fp.height as f32 * 0.5,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlastKind {
    GroundScorch,
    NukeAir,
    SamAirburst,
    BuildingPop,
    Launch,
    Split,
    EntityDeath,
}

impl BlastKind {
    #[inline]
    fn gpu_code(self) -> f32 {
        match self {
            Self::GroundScorch => BLAST_KIND_GROUND_SCORCH,
            Self::NukeAir => BLAST_KIND_NUKE_AIR,
            Self::SamAirburst => BLAST_KIND_SAM_AIRBURST,
            Self::Launch => sow_render::BLAST_KIND_NUKE_LAUNCH,
            Self::Split => sow_render::BLAST_KIND_MIRV_SPLIT,
            Self::BuildingPop => BLAST_KIND_BUILDING_POP,
            Self::EntityDeath => BLAST_KIND_ENTITY_DEATH,
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
            BlastKind::Launch | BlastKind::Split => self.outer_radius,
            BlastKind::BuildingPop | BlastKind::EntityDeath => {
                let t = (age / self.lifetime_secs.max(0.001)).clamp(0.0, 1.0);
                let ease = 1.0 - (1.0 - t) * (1.0 - t);
                self.outer_radius * (0.38 + 0.62 * ease)
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

/// A building destroyed by a nuke or combat demolition, kept visible until the outward-expanding
/// shockwave front reaches its footprint, then animated through an impact flash, squash-stretch,
/// ballistic shard shatter, and structural collapse.
#[derive(Clone, Debug)]
pub struct DyingBuilding {
    pub building: BuildingSnapshot,
    pub world_x: f32,
    pub world_y: f32,
    pub owner_id: u16,
    pub blast_dir: [f32; 2],
    pub spawn_time: Instant,
    pub trigger_at: Instant,
    pub popped: bool,
    pub seed: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum DyingBuildingPhase {
    /// Shockwave is approaching; `0.0..1.0` proximity factor drives pre-impact seismic tremor.
    PreWave(f32),
    /// Shockwave has struck; `0.0..1.0` progress through the 620ms impact flash, shatter & collapse.
    Collapsing(f32),
}

#[derive(Clone, Copy, Debug)]
pub struct DyingBuildingRender<'a> {
    pub building: &'a BuildingSnapshot,
    pub world_x: f32,
    pub world_y: f32,
    pub blast_dir: [f32; 2],
    pub seed: f32,
    pub phase: DyingBuildingPhase,
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
    pub reduced_motion: bool,
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

    pub fn spawn_staging(&mut self, tile: u32, width: u32, owner: u16, split: bool, now: Instant) {
        let w = width.max(1);
        let world_x = (tile % w) as f32 + 0.5;
        let world_y = (tile / w) as f32 + 0.5;
        self.spawn_staging_world([world_x, world_y], tile, owner, split, now);
    }

    pub fn spawn_staging_world(
        &mut self,
        world_pos: [f32; 2],
        tile: u32,
        owner: u16,
        split: bool,
        now: Instant,
    ) {
        let seed = self.alloc_seed();
        Self::push_capped(
            &mut self.air_blasts,
            ActiveBlast {
                world_x: world_pos[0],
                world_y: world_pos[1],
                tile_idx: tile,
                owner_id: owner,
                inner_radius: 1.0,
                outer_radius: if split { 16.0 } else { 10.0 },
                start_time: now,
                expand_dur: 0.0,
                lifetime_secs: if split { 0.55 } else { 0.42 },
                kind: if split {
                    BlastKind::Split
                } else {
                    BlastKind::Launch
                },
                seed,
            },
        );
    }

    /// Spawn a localized demolition shockwave & debris burst on an existing building (e.g. CityDenial downgrade).
    pub fn spawn_building_demolition(
        &mut self,
        building: &BuildingSnapshot,
        map_w: u32,
        now: Instant,
    ) {
        if map_w == 0 {
            return;
        }
        let bx = building.tile_idx % map_w;
        let by = building.tile_idx / map_w;
        let (world_x, world_y) = building_footprint_world_center(building.kind, bx, by);
        let (tier, outer_radius) =
            building_demolition_tier_and_radius(building.kind, building.active_level().max(1));
        let seed = self.alloc_seed();
        Self::push_capped(
            &mut self.air_blasts,
            ActiveBlast {
                world_x,
                world_y,
                tile_idx: building.tile_idx,
                owner_id: building.owner_id,
                inner_radius: outer_radius * 0.48,
                outer_radius,
                start_time: now,
                expand_dur: tier,
                lifetime_secs: BUILDING_POP_LIFETIME_SECS,
                kind: BlastKind::BuildingPop,
                seed,
            },
        );
    }

    /// Spawn an entity elimination / sovereign collapse or naval sinking burst in the air pass.
    /// `mode`: `0.0` = Tribe/Bot, `1.0` = Sovereign Player/Empire, `2.0` = Nuclear Kill, `3.0` = Naval Sunk.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_entity_death(
        &mut self,
        world_x: f32,
        world_y: f32,
        tile_idx: u32,
        owner_id: u16,
        rgb: [f32; 3],
        mode: f32,
        now: Instant,
    ) {
        let outer_radius = if mode > 2.5 {
            8.0
        } else if mode > 1.5 {
            15.5
        } else if mode > 0.5 {
            13.5
        } else {
            9.5
        };
        let seed = self.alloc_seed();
        Self::push_capped(
            &mut self.air_blasts,
            ActiveBlast {
                world_x,
                world_y,
                tile_idx,
                owner_id,
                inner_radius: pack_rgb_f32(rgb),
                outer_radius,
                start_time: now,
                expand_dur: mode,
                lifetime_secs: ENTITY_DEATH_LIFETIME_SECS,
                kind: BlastKind::EntityDeath,
                seed,
            },
        );
    }

    /// Match vanished buildings against this tick's nuke detonations (or direct combat demolition)
    /// so they remain intact until the shockwave crosses their footprint, then animate through
    /// impact flash, squash-stretch, shard shatter, and structural collapse.
    pub fn register_dying_buildings(
        &mut self,
        vanished: &[BuildingSnapshot],
        map_w: u32,
        now: Instant,
        allow_non_nuke: bool,
    ) {
        if map_w == 0 || (self.recent_detonations.is_empty() && !allow_non_nuke) {
            self.recent_detonations.clear();
            return;
        }
        for b in vanished {
            if b.kind != BuildingKind::City
                && vanished
                    .iter()
                    .any(|other| other.kind == BuildingKind::City && other.tile_idx == b.tile_idx)
            {
                continue;
            }
            let bx = b.tile_idx % map_w;
            let by = b.tile_idx / map_w;
            let (world_x, world_y) = building_footprint_world_center(b.kind, bx, by);
            let mut best_hit: Option<(Instant, Instant, u16, [f32; 2])> = None;
            for det in &self.recent_detonations {
                let dx = bx as i64 - det.tile_x as i64;
                let dy = by as i64 - det.tile_y as i64;
                let dist_sq = dx * dx + dy * dy;
                let reach = i64::from(det.inner_radius) + 2;
                if dist_sq > reach * reach {
                    continue;
                }
                let dist = (dist_sq as f32).sqrt();
                let trigger_at = building_wave_hit_time(
                    det.start_time,
                    det.expand_dur,
                    dist,
                    det.inner_radius as f32,
                );
                let vx = world_x - (det.tile_x as f32 + 0.5);
                let vy = world_y - (det.tile_y as f32 + 0.5);
                let vlen = vx.hypot(vy);
                let dir = if vlen > 0.05 {
                    [vx / vlen, vy / vlen]
                } else {
                    [0.0, -1.0]
                };
                if best_hit.is_none_or(|(prev, _, _, _)| trigger_at < prev) {
                    best_hit = Some((trigger_at, det.start_time, det.owner_id, dir));
                }
            }
            let resolved = best_hit.or_else(|| {
                allow_non_nuke.then(|| {
                    let ang = ((b.id as u32).wrapping_mul(2654435761) as f32)
                        * (std::f32::consts::TAU / 4_294_967_295.0);
                    (now, now, b.owner_id, [ang.cos(), ang.sin()])
                })
            });
            if let Some((trigger_at, spawn_time, owner_id, blast_dir)) = resolved {
                let seed = self.alloc_seed();
                if self.dying_buildings.len() >= MAX_DYING_BUILDINGS {
                    self.dying_buildings.remove(0);
                }
                self.dying_buildings.push(DyingBuilding {
                    building: b.clone(),
                    world_x,
                    world_y,
                    owner_id,
                    blast_dir,
                    spawn_time,
                    trigger_at,
                    popped: false,
                    seed,
                });
            }
        }
        self.recent_detonations.clear();
    }

    /// Active dying buildings for `overlays.rs` to draw through pre-wave tremor and post-impact collapse.
    pub fn dying_buildings_for_render(
        &self,
        now: Instant,
    ) -> impl Iterator<Item = DyingBuildingRender<'_>> {
        self.dying_buildings.iter().filter_map(move |db| {
            if now < db.trigger_at {
                let total_wait = db
                    .trigger_at
                    .saturating_duration_since(db.spawn_time)
                    .as_secs_f32()
                    .max(0.001);
                let elapsed = now
                    .saturating_duration_since(db.spawn_time)
                    .as_secs_f32();
                let pre_t = (elapsed / total_wait).clamp(0.0, 1.0);
                Some(DyingBuildingRender {
                    building: &db.building,
                    world_x: db.world_x,
                    world_y: db.world_y,
                    blast_dir: db.blast_dir,
                    seed: db.seed,
                    phase: DyingBuildingPhase::PreWave(pre_t),
                })
            } else {
                let collapse_age = now.saturating_duration_since(db.trigger_at).as_secs_f32();
                if collapse_age < BUILDING_COLLAPSE_SECS {
                    Some(DyingBuildingRender {
                        building: &db.building,
                        world_x: db.world_x,
                        world_y: db.world_y,
                        blast_dir: db.blast_dir,
                        seed: db.seed,
                        phase: DyingBuildingPhase::Collapsing(
                            (collapse_age / BUILDING_COLLAPSE_SECS).clamp(0.0, 1.0),
                        ),
                    })
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
        self.pack_instances(params, |air, inst| {
            if air {
                renderer.push_air(inst);
            } else {
                renderer.push_ground(inst);
            }
        });
    }

    fn pack_instances(
        &mut self,
        params: &BlastPackParams<'_>,
        mut push: impl FnMut(bool, BlastInstanceGpu),
    ) {
        self.advance_dying_buildings(params.now);

        self.ground_blasts.retain(|b| {
            params
                .now
                .saturating_duration_since(b.start_time)
                .as_secs_f32()
                < b.lifetime_secs
        });
        self.air_blasts.retain(|b| {
            params
                .now
                .saturating_duration_since(b.start_time)
                .as_secs_f32()
                < b.lifetime_secs
        });

        if self.ground_blasts.is_empty() && self.air_blasts.is_empty() {
            return;
        }

        let zoom = params.camera_zoom.max(0.0001);
        let view_min_x = -params.camera_x / zoom;
        let view_max_x = (params.screen_w - params.camera_x) / zoom;
        let view_min_y = -params.camera_y / zoom;
        let view_max_y = (params.screen_h - params.camera_y) / zoom;

        let view = (view_min_x, view_max_x, view_min_y, view_max_y);
        let viewport_area = params.screen_w * params.screen_h;

        // 1. Ground pass: scorched craters & cooling magma fissures.
        // Keep full craters up to a tactical salvo (40); only downgrade to ThermalGlint under
        // extreme multi-MIRV saturation, while switching shader to fast_lod above 10 for GPU speed.
        {
            let fast_lod = self.ground_blasts.len() > 10;
            let dense = self.ground_blasts.len() > 40;
            let mut budget = viewport_area * 0.70;
            let mut occupied = [0u32; 24];
            for b in self.ground_blasts.iter().rev() {
                let Some(mut inst) = Self::build_gpu_instance(b, params, view) else {
                    continue;
                };
                if fast_lod || params.reduced_motion {
                    inst.seed += 2.0;
                }
                if dense
                    && !Self::filter_dense_glint(
                        &mut inst,
                        &mut budget,
                        &mut occupied,
                        params,
                        viewport_area,
                        10.0 / zoom,
                        0.0,
                    )
                {
                    continue;
                }
                push(false, inst);
            }
        }

        // 2. Air pass: separate primary nuke blasts from secondary building/SAM pops and entity
        //    deaths so building pops never starve or overwrite NukeAir fireballs, and draw
        //    building pops + entity deaths ON TOP of NukeAir smoke.
        if !self.air_blasts.is_empty() {
            let mut nuke_count = 0usize;
            let mut pop_count = 0usize;
            let mut death_count = 0usize;
            for b in &self.air_blasts {
                match b.kind {
                    BlastKind::NukeAir | BlastKind::Launch | BlastKind::Split => nuke_count += 1,
                    BlastKind::BuildingPop | BlastKind::SamAirburst => pop_count += 1,
                    BlastKind::EntityDeath => death_count += 1,
                    BlastKind::GroundScorch => {}
                }
            }

            // 2a. Primary nuke fireballs, shockwaves, launches & MIRV splits (base air layer).
            let nuke_fast_lod = nuke_count > 10;
            let nuke_dense = nuke_count > 40;
            let mut nuke_budget = viewport_area * 0.70;
            let mut nuke_occupied = [0u32; 24];
            for b in self.air_blasts.iter().rev() {
                if !matches!(
                    b.kind,
                    BlastKind::NukeAir | BlastKind::Launch | BlastKind::Split
                ) {
                    continue;
                }
                let Some(mut inst) = Self::build_gpu_instance(b, params, view) else {
                    continue;
                };
                if nuke_fast_lod || params.reduced_motion {
                    inst.seed += 2.0;
                }
                if nuke_dense
                    && b.kind == BlastKind::NukeAir
                    && !Self::filter_dense_glint(
                        &mut inst,
                        &mut nuke_budget,
                        &mut nuke_occupied,
                        params,
                        viewport_area,
                        15.0 / zoom,
                        1.0,
                    )
                {
                    continue;
                }
                push(true, inst);
            }

            // 2b. Wave-synced building demolition pops & SAM airbursts (drawn over NukeAir).
            let pop_dense = pop_count > 24;
            let mut pop_budget = viewport_area * 0.06;
            let mut pop_occupied = [0u32; 24];
            for b in self.air_blasts.iter().rev() {
                if !matches!(b.kind, BlastKind::BuildingPop | BlastKind::SamAirburst) {
                    continue;
                }
                let Some(mut inst) = Self::build_gpu_instance(b, params, view) else {
                    continue;
                };
                if pop_dense || params.reduced_motion {
                    inst.seed += 2.0;
                }
                if pop_count > 48 {
                    let area = (inst.bound_radius * zoom * 2.0).powi(2).min(viewport_area);
                    if area > pop_budget {
                        let x = inst.world_pos[0] * zoom + params.camera_x;
                        let y = inst.world_pos[1] * zoom + params.camera_y;
                        let col = ((x * 32.0 / params.screen_w.max(1.0)) as usize).min(31);
                        let row = ((y * 24.0 / params.screen_h.max(1.0)) as usize).min(23);
                        if pop_occupied[row] & (1 << col) != 0 {
                            continue;
                        }
                        pop_occupied[row] |= 1 << col;
                        inst.bound_radius = inst.bound_radius.min(16.0 / zoom);
                    } else {
                        pop_budget -= area;
                    }
                }
                push(true, inst);
            }

            // 2c. Sovereign entity elimination & naval sinking bursts (top air layer).
            let death_dense = death_count > 12;
            for b in self.air_blasts.iter().rev() {
                if b.kind != BlastKind::EntityDeath {
                    continue;
                }
                let Some(mut inst) = Self::build_gpu_instance(b, params, view) else {
                    continue;
                };
                if death_dense || params.reduced_motion {
                    inst.seed += 2.0;
                }
                if death_dense {
                    inst.bound_radius = inst.bound_radius.min(22.0 / zoom);
                }
                push(true, inst);
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
                newly_popped.push((
                    db.world_x,
                    db.world_y,
                    db.building.tile_idx,
                    db.owner_id,
                    db.building.kind,
                    db.building.active_level().max(1),
                ));
            }
        }
        for (world_x, world_y, tile_idx, owner_id, kind, level) in newly_popped {
            let (tier, outer_radius) = building_demolition_tier_and_radius(kind, level);
            let seed = self.alloc_seed();
            Self::push_capped(
                &mut self.air_blasts,
                ActiveBlast {
                    world_x,
                    world_y,
                    tile_idx,
                    owner_id,
                    inner_radius: outer_radius * 0.48,
                    outer_radius,
                    start_time: now,
                    expand_dur: tier,
                    lifetime_secs: BUILDING_POP_LIFETIME_SECS,
                    kind: BlastKind::BuildingPop,
                    seed,
                },
            );
        }
        self.dying_buildings.retain(|db| {
            now < db.trigger_at
                || now.saturating_duration_since(db.trigger_at).as_secs_f32()
                    < BUILDING_COLLAPSE_SECS
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
        let age = params
            .now
            .saturating_duration_since(b.start_time)
            .as_secs_f32();
        if age >= b.lifetime_secs {
            return None;
        }
        let zoom = params.camera_zoom.max(0.01);
        let bound_radius = match b.kind {
            BlastKind::EntityDeath => b
                .bound_radius(age)
                .max((26.0 / zoom).min(b.outer_radius * 3.2)),
            BlastKind::BuildingPop => b
                .bound_radius(age)
                .max((13.0 / zoom).min(b.outer_radius * 2.2)),
            _ => b.bound_radius(age),
        };
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

    fn filter_dense_glint(
        inst: &mut BlastInstanceGpu,
        budget: &mut f32,
        occupied: &mut [u32; 24],
        params: &BlastPackParams<'_>,
        viewport_area: f32,
        glint_max_radius: f32,
        expand_dur: f32,
    ) -> bool {
        let zoom = params.camera_zoom.max(0.01);
        let area = (inst.bound_radius * zoom * 2.0).powi(2).min(viewport_area);
        if area <= *budget {
            *budget -= area;
            return true;
        }
        let x = inst.world_pos[0] * zoom + params.camera_x;
        let y = inst.world_pos[1] * zoom + params.camera_y;
        let col = ((x * 32.0 / params.screen_w.max(1.0)) as usize).min(31);
        let row = ((y * 24.0 / params.screen_h.max(1.0)) as usize).min(23);
        if occupied[row] & (1 << col) != 0 {
            return false;
        }
        occupied[row] |= 1 << col;
        inst.bound_radius = inst.bound_radius.min(glint_max_radius);
        inst.kind = sow_render::BLAST_KIND_THERMAL_GLINT;
        inst.expand_dur = expand_dur;
        true
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

        let packed = pack_rgb_f32([1.0, 0.5, 0.0]);
        let p = packed.round() as u32;
        assert_eq!(p >> 12, 63);
        assert_eq!((p >> 6) & 63, 32);
        assert_eq!(p & 63, 0);
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
        assert_eq!(
            scene.camera_shake_offset(t0 + Duration::from_millis(50), true),
            [0.0, 0.0]
        );
        assert!(!scene.has_recent_detonations());

        // When motion is allowed, vanished buildings register wave-synced PreWave -> Collapsing phases
        // and deduplicate non-City sub-buildings sharing a City's tile.
        scene.spawn_nuke_detonation(
            50,
            50,
            22,
            55,
            1,
            200,
            0.45,
            t0,
            false,
            (0.0, 0.0, 2.0),
            (1280.0, 720.0),
            1.0,
        );
        let city_tile = 50 * 200 + 60;
        let vanished = vec![
            BuildingSnapshot {
                id: 1,
                kind: BuildingKind::City,
                tile_idx: city_tile,
                owner_id: 2,
                level: 2,
                under_construction: false,
                ticks_until_complete: 0,
                has_anti_nuke: false,
                nuke_ready: false,
                arsenal_build: None,
                warship_queue: Vec::new(),
                sam_cooldown_ticks: 0,
                upgrade_target_level: 0,
            },
            BuildingSnapshot {
                id: 2,
                kind: BuildingKind::TradingPost,
                tile_idx: city_tile,
                owner_id: 2,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
                has_anti_nuke: false,
                nuke_ready: false,
                arsenal_build: None,
                warship_queue: Vec::new(),
                sam_cooldown_ticks: 0,
                upgrade_target_level: 0,
            },
        ];
        scene.register_dying_buildings(&vanished, 200, t0, true);
        let pre: Vec<_> = scene.dying_buildings_for_render(t0).collect();
        assert_eq!(pre.len(), 1);
        assert!(matches!(pre[0].phase, DyingBuildingPhase::PreWave(_)));
        let mid: Vec<_> = scene
            .dying_buildings_for_render(t0 + Duration::from_millis(350))
            .collect();
        assert_eq!(mid.len(), 1);
        assert!(matches!(mid[0].phase, DyingBuildingPhase::Collapsing(_)));
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
        scene.spawn_entity_death(20.5, 20.5, 2020, 1, [0.9, 0.2, 0.2], 1.0, t0);
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
        assert!(
            scene
                .air_blasts
                .iter()
                .any(|b| b.kind == BlastKind::EntityDeath),
            "entity death blast should survive SAM burst overflow"
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
            reduced_motion: false,
            fog_of_war: false,
            my_player_id: 1,
            fog_visible: &fog,
            now: t0 + Duration::from_millis(50),
        };
        let offscreen = BlastScene::build_gpu_instance(&blast, &params, (0.0, 200.0, 0.0, 200.0));
        assert!(offscreen.is_none());
        let onscreen =
            BlastScene::build_gpu_instance(&blast, &params, (450.0, 550.0, 450.0, 550.0));
        assert!(onscreen.is_some());

        // Far-zoom EntityDeath and direct non-nuke demolition building pops stay legible and advance cleanly.
        let mut scene = BlastScene::new();
        let tower = BuildingSnapshot {
            id: 7,
            kind: BuildingKind::Tower,
            tile_idx: 2050,
            owner_id: 2,
            level: 3,
            under_construction: false,
            ticks_until_complete: 0,
            has_anti_nuke: false,
            nuke_ready: false,
            arsenal_build: None,
            warship_queue: Vec::new(),
            sam_cooldown_ticks: 0,
            upgrade_target_level: 0,
        };
        scene.register_dying_buildings(&[tower.clone()], 100, t0, true);
        let immediate: Vec<_> = scene.dying_buildings_for_render(t0).collect();
        assert_eq!(immediate.len(), 1);
        assert!(matches!(
            immediate[0].phase,
            DyingBuildingPhase::Collapsing(_)
        ));
        scene.advance_dying_buildings(t0);
        assert_eq!(scene.air_blasts.len(), 1);
        assert_eq!(scene.air_blasts[0].kind, BlastKind::BuildingPop);

        scene.spawn_building_demolition(&tower, 100, t0);
        assert_eq!(scene.air_blasts.len(), 2);

        scene.spawn_entity_death(50.5, 20.5, 2050, 2, [0.2, 0.8, 0.4], 1.0, t0);
        let far_params = BlastPackParams {
            camera_zoom: 0.25,
            ..params
        };
        let death_inst = BlastScene::build_gpu_instance(
            scene.air_blasts.last().unwrap(),
            &far_params,
            (0.0, 200.0, 0.0, 200.0),
        )
        .expect("onscreen entity death instance");
        assert_eq!(death_inst.kind, BLAST_KIND_ENTITY_DEATH);
        assert!(death_inst.bound_radius >= 13.5);

        // Single nuke destroying 16 buildings must keep full NukeAir fireball at base of air layer
        // and draw BuildingPop + EntityDeath instances on top without downgrading to ThermalGlint.
        let mut strike_scene = BlastScene::new();
        strike_scene.spawn_nuke_detonation(
            50,
            50,
            22,
            55,
            1,
            100,
            0.35,
            t0,
            false,
            (0.0, 0.0, 4.0),
            (1280.0, 720.0),
            1.0,
        );
        for i in 0..16u32 {
            let mut b = tower.clone();
            b.id = 100 + u64::from(i);
            b.tile_idx = (46 + (i / 4) * 2) * 100 + (46 + (i % 4) * 2);
            strike_scene.spawn_building_demolition(&b, 100, t0);
        }
        strike_scene.spawn_entity_death(52.5, 52.5, 5252, 2, [0.9, 0.2, 0.1], 2.0, t0);
        let strike_params = BlastPackParams {
            camera_x: 400.0,
            camera_y: 160.0,
            camera_zoom: 4.0,
            screen_w: 1280.0,
            screen_h: 720.0,
            reduced_motion: false,
            fog_of_war: false,
            my_player_id: 1,
            fog_visible: &fog,
            now: t0 + Duration::from_millis(120),
        };
        let mut ground_out = Vec::new();
        let mut air_out = Vec::new();
        strike_scene.pack_instances(&strike_params, |air, inst| {
            if air {
                air_out.push(inst);
            } else {
                ground_out.push(inst);
            }
        });
        assert_eq!(ground_out.len(), 1);
        assert_eq!(ground_out[0].kind, BLAST_KIND_GROUND_SCORCH);
        assert_eq!(air_out.len(), 18);
        assert_eq!(air_out[0].kind, BLAST_KIND_NUKE_AIR);
        assert!(air_out[0].bound_radius > 20.0);
        assert!(air_out[0].seed < 1.5, "single nuke should not be forced to fast_lod");
        assert!(
            air_out[1..17]
                .iter()
                .all(|inst| inst.kind == BLAST_KIND_BUILDING_POP),
            "building pops must render as BLAST_KIND_BUILDING_POP over NukeAir"
        );
        assert_eq!(air_out[17].kind, BLAST_KIND_ENTITY_DEATH);

        // Dense MIRV saturation: 12 nukes + 100 buildings + 15 entity deaths stay bounded and
        // preserve distinct BuildingPop and EntityDeath kinds on top of NukeAir/ThermalGlint.
        let mut mirv_scene = BlastScene::new();
        for i in 0..12u32 {
            mirv_scene.spawn_nuke_detonation(
                20 + (i % 4) * 18,
                20 + (i / 4) * 18,
                22,
                55,
                1,
                100,
                0.35,
                t0,
                false,
                (0.0, 0.0, 4.0),
                (1280.0, 720.0),
                1.0,
            );
        }
        for i in 0..100u32 {
            let mut b = tower.clone();
            b.id = 1000 + u64::from(i);
            b.tile_idx = (10 + (i / 10) * 6) * 100 + (10 + (i % 10) * 6);
            mirv_scene.spawn_building_demolition(&b, 100, t0);
        }
        for i in 0..15u32 {
            let tx = 15 + (i % 5) * 12;
            let ty = 15 + (i / 5) * 12;
            mirv_scene.spawn_entity_death(
                tx as f32 + 0.5,
                ty as f32 + 0.5,
                ty * 100 + tx,
                2,
                [0.8, 0.3, 0.2],
                2.0,
                t0,
            );
        }
        ground_out.clear();
        air_out.clear();
        mirv_scene.pack_instances(&strike_params, |air, inst| {
            if air {
                air_out.push(inst);
            } else {
                ground_out.push(inst);
            }
        });
        assert!(
            air_out.iter().any(|inst| inst.kind == BLAST_KIND_NUKE_AIR),
            "at least one full NukeAir shockwave must render under MIRV saturation"
        );
        let pop_instances: Vec<_> = air_out
            .iter()
            .filter(|inst| inst.kind == BLAST_KIND_BUILDING_POP)
            .collect();
        assert!(!pop_instances.is_empty() && pop_instances.len() < 100);
        assert!(pop_instances.iter().all(|inst| inst.seed >= 1.5));
        let death_instances: Vec<_> = air_out
            .iter()
            .filter(|inst| inst.kind == BLAST_KIND_ENTITY_DEATH)
            .collect();
        assert_eq!(death_instances.len(), 15);
        assert!(death_instances.iter().all(|inst| inst.seed >= 1.5));
    }
}
