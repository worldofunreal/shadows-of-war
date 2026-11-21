use super::movers::{NukeArcPath, tile_to_world};
use crate::app::{InputState, SimState};
use sow_core::game::{
    BuildingKind, NukeKind, ProjectileKind, nuke_inner_radius, nuke_outer_radius,
};
use sow_core::protocol::SimSnapshot;
use sow_render::{BlastInstanceGpu, BlastRenderer, MoverRenderer, TrailSegmentGpu};
use std::collections::HashMap;

#[derive(Default)]
pub struct TargetIndicators {
    buckets: HashMap<(u32, u32, u8, u8), usize>,
    markers: Vec<BlastInstanceGpu>,
    arrivals: Vec<f32>,
    relations: HashMap<u16, u8>,
    last_pointer: Option<[f32; 2]>,
    dial_phase: f32,
}

fn marker(tile: u32, width: u32, level: u8, zoom: f32) -> BlastInstanceGpu {
    let (x, y) = tile_to_world(tile, width);
    let outer = nuke_outer_radius(level) as f32;
    BlastInstanceGpu {
        world_pos: [x, y],
        inner_radius: nuke_inner_radius(level) as f32,
        outer_radius: outer,
        bound_radius: outer + 4.0 / zoom + 3.0,
        age_secs: 0.0,
        expand_dur: 0.0,
        lifetime_secs: level as f32,
        kind: sow_render::BLAST_KIND_NUKE_TELEGRAPH,
        seed: 0.0,
    }
}

impl TargetIndicators {
    pub fn pointer_phase(&mut self, input: &InputState, aiming: bool) -> f32 {
        let point = [input.last_mouse_x as f32, input.last_mouse_y as f32];
        if !aiming {
            self.last_pointer = None;
            return self.dial_phase;
        }
        if let Some(prev) = self.last_pointer.replace(point) {
            let travel = (point[0] - prev[0]).hypot(point[1] - prev[1]);
            self.dial_phase = (self.dial_phase + travel.min(80.0) / 540.0).fract();
        }
        self.dial_phase
    }

    pub fn pack(
        &mut self,
        sim: &SimState,
        snap: &SimSnapshot,
        input: &InputState,
        fog: bool,
        reduced: bool,
        alpha: f32,
        renderer: &mut BlastRenderer,
    ) {
        if snap.projectiles.is_empty() {
            return;
        }
        let width = sim.map_w.max(1);
        let zoom = input.camera_zoom.max(0.01);
        let me = sim
            .my_player_id
            .and_then(|id| snap.players.iter().find(|p| p.id == id));
        self.relations.clear();
        for p in &snap.players {
            let relation = match me {
                Some(m) if m.id == p.id => 0,
                Some(m)
                    if (m.team.is_some() && m.team == p.team)
                        || m.alliances.contains(&p.id)
                        || p.alliances.contains(&m.id) =>
                {
                    1
                }
                _ => 2,
            };
            self.relations.insert(p.id, relation);
        }
        self.markers.clear();
        self.buckets.clear();
        let cell = (7.0 / zoom).max(1.0) as u32;
        for p in &snap.projectiles {
            let ProjectileKind::Nuke { level } = p.kind else {
                continue;
            };
            let relation = *self.relations.get(&p.owner_id).unwrap_or(&2);
            if fog && relation != 0 && !sim.fog_visible.contains(p.dst_tile) {
                continue;
            }
            let mut m = marker(p.dst_tile, width, level, zoom);
            // A carrier separates. Mark its selected destination as a beacon, not a damage radius.
            if level == 3 {
                m.outer_radius = 18.0 / zoom;
                m.inner_radius = 6.0 / zoom;
                m.bound_radius = 23.0 / zoom;
            }
            let sx = m.world_pos[0] * zoom + input.camera_x;
            let sy = m.world_pos[1] * zoom + input.camera_y;
            let r = m.bound_radius * zoom;
            if sx < -r || sy < -r || sx > input.screen_w + r || sy > input.screen_h + r {
                continue;
            }
            let end = if level == 3 {
                p.path.len() / 2
            } else {
                p.path.len()
            };
            let prev = p.path_cursor.saturating_sub(p.steps_per_tick as usize);
            let cursor = prev as f32 + (p.path_cursor - prev) as f32 * alpha;
            let eta = (end as f32 - cursor).max(0.0) / f32::from(p.steps_per_tick.max(1));
            m.age_secs = (cursor / end.max(1) as f32).min(1.0);
            m.expand_dur = relation as f32;
            m.seed = if reduced { 2.0 } else { 0.0 };
            let key = (
                p.dst_tile % width / cell,
                p.dst_tile / width / cell,
                level,
                relation,
            );
            let index = *self.buckets.entry(key).or_insert(self.markers.len());
            if index == self.markers.len() {
                self.markers.push(m);
                self.arrivals.push(eta);
            } else {
                let old = &mut self.markers[index];
                let intensity = (old.lifetime_secs + 0.01).min(level as f32 + 0.08);
                if eta < self.arrivals[index] {
                    *old = m;
                    self.arrivals[index] = eta;
                }
                old.lifetime_secs = intensity;
            }
        }
        // Under saturation keep a threat beacon for every occupied screen region.
        // A capped prefix would silently hide entire sectors of the battlefield.
        let area: f32 = self
            .markers
            .iter()
            .map(|m| (2.0 * m.bound_radius * zoom).powi(2))
            .sum();
        if self.markers.len() > 900
            || (self.markers.len() > 4 && area > 0.85 * input.screen_w * input.screen_h)
        {
            let mut sectors = [None::<usize>; 32 * 24];
            for (i, m) in self.markers.iter().enumerate() {
                let col = ((m.world_pos[0] * zoom + input.camera_x) * 32.0
                    / input.screen_w.max(1.0)) as usize;
                let row = ((m.world_pos[1] * zoom + input.camera_y) * 24.0
                    / input.screen_h.max(1.0)) as usize;
                let slot = &mut sectors[row.min(23) * 32 + col.min(31)];
                if slot.is_none_or(|j| {
                    m.expand_dur > self.markers[j].expand_dur
                        || (m.expand_dur == self.markers[j].expand_dur
                            && self.arrivals[i] < self.arrivals[j])
                }) {
                    *slot = Some(i);
                }
            }
            for i in sectors.into_iter().flatten() {
                let mut m = self.markers[i];
                m.outer_radius = 6.0 / zoom;
                m.inner_radius = 1.75 / zoom;
                m.bound_radius = 9.0 / zoom;
                m.kind = sow_render::BLAST_KIND_SALVO_BEACON;
                renderer.push_ground(m);
            }
        } else {
            // Draw hostile rims last when multiple diplomatic classes share a target.
            for relation in [0.0, 1.0, 2.0] {
                for &m in &self.markers {
                    if m.expand_dur == relation {
                        renderer.push_ground(m);
                    }
                }
            }
        }
        self.arrivals.clear();
    }
}

pub struct Aim {
    target: BlastInstanceGpu,
    source: Option<BlastInstanceGpu>,
    arc: Option<NukeArcPath>,
    risk: Option<f32>,
}

impl Aim {
    pub fn at_pointer(
        kind: NukeKind,
        sim: &SimState,
        input: &InputState,
        reduced: bool,
        fog: bool,
        arc_height: f32,
        dial_phase: f32,
    ) -> Option<Self> {
        let engine = sim.engine.as_ref()?;
        let map = &engine.state.map;
        let zoom = input.camera_zoom.max(0.01);
        let x = (input.last_mouse_x as f32 - input.camera_x) / zoom;
        let y = (input.last_mouse_y as f32 - input.camera_y) / zoom;
        if x < 0.0 || y < 0.0 || x >= map.width as f32 || y >= map.height as f32 {
            return None;
        }
        let tile = y as u32 * map.width + x as u32;
        let level = match kind {
            NukeKind::AtomBomb => 1,
            NukeKind::HydrogenBomb => 2,
            NukeKind::MIRV => 3,
        };
        let id = sim.my_player_id.unwrap_or(0);
        let silo = engine.ready_nuclear_silo(id, tile).filter(|_| {
            engine
                .state
                .player(id)
                .is_some_and(|p| p.alive && p.gold >= engine.nuclear_price(kind))
                && engine.campaign_allows_action(sow_core::campaign::CampaignAction::Nuke)
        });
        let mut target = marker(tile, map.width, level, zoom);
        target.kind = sow_render::BLAST_KIND_NUKE_RETICLE;
        target.expand_dur = if silo.is_some() { 0.0 } else { 1.0 };
        target.seed = if reduced { 2.0 } else { dial_phase };
        if level == 3 {
            target.outer_radius = 1500.0;
            target.bound_radius = 1504.0 + 4.0 / zoom;
        }
        let source = silo.map(|s| {
            let (x, y) = tile_to_world(s.tile_idx, map.width);
            BlastInstanceGpu {
                world_pos: [x, y],
                inner_radius: 0.0,
                outer_radius: 10.0 / zoom,
                bound_radius: 14.0 / zoom,
                expand_dur: 2.0,
                ..target
            }
        });
        let arc =
            silo.map(|s| NukeArcPath::new(s.tile_idx, tile, map.width, map.height, arc_height));
        let risk = source.and_then(|src| {
            if level == 3 {
                return None;
            }
            let me = engine.state.player(id)?;
            engine
                .buildings
                .iter()
                .filter(|b| {
                    b.kind == BuildingKind::SAM
                        && b.owner_id != id
                        && (!fog || sim.fog_visible.contains(b.tile_idx))
                        && !me.alliances.contains(&b.owner_id)
                        && !engine
                            .state
                            .player(b.owner_id)
                            .is_some_and(|p| p.alliances.contains(&id))
                        && !(me.team.is_some()
                            && engine
                                .state
                                .player(b.owner_id)
                                .is_some_and(|p| p.team == me.team))
                        && engine.nuclear_charges(b.id, b.active_level()) > 0
                })
                .filter_map(|b| {
                    let (bx, by) = tile_to_world(b.tile_idx, map.width);
                    let r = sow_core::execution::sam::sam_range(b.active_level()).floor();
                    let mut enter = 0.0f32;
                    let mut leave = 1.0f32;
                    for (axis, center) in [bx, by].into_iter().enumerate() {
                        let start = src.world_pos[axis];
                        let d = target.world_pos[axis] - start;
                        if d == 0.0 {
                            if (center - start).abs() > r {
                                return None;
                            }
                        } else {
                            let a = (center - r - start) / d;
                            let b = (center + r - start) / d;
                            enter = enter.max(a.min(b));
                            leave = leave.min(a.max(b));
                        }
                    }
                    (enter <= leave).then_some(enter)
                })
                .min_by(f32::total_cmp)
        });
        Some(Self {
            target,
            source,
            arc,
            risk,
        })
    }

    pub fn ground(&self, r: &mut BlastRenderer) {
        r.push_ground(self.target);
        if let Some(s) = self.source {
            r.push_ground(s);
        }
        if let (Some(t), Some(arc), Some(mut s)) = (self.risk, self.arc, self.source) {
            s.world_pos = arc.point_at(t);
            s.expand_dur = 3.0;
            r.push_ground(s);
        }
    }

    pub fn trajectory(&self, r: &mut MoverRenderer) {
        let Some(arc) = self.arc else { return };
        for i in 0..32 {
            let t = i as f32 / 32.0;
            r.push_trail_segment(TrailSegmentGpu {
                p0: arc.point_at(t),
                p1: arc.point_at(t + 0.019),
                width: 1.8,
                color: if self.risk.is_some_and(|risk| t >= risk) {
                    [1.0, 0.19, 0.08, 0.85]
                } else {
                    [1.0, 0.82, 0.38, 0.76]
                },
            });
        }
    }
}
