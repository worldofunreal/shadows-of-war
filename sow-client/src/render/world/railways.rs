use crate::app::{InputState, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::protocol::SimSnapshot;
use sow_core::building::RailwayLink;
use std::collections::HashMap;
use web_time::Instant;

const MAX_RENDERED_TRAINS: usize = 60;
const TRACK_BED_COLOR: [f32; 3] = [0.12, 0.11, 0.10];
const TRACK_RAIL_COLOR: [f32; 3] = [0.63, 0.54, 0.40];

struct RailSegment {
    a: [f32; 2],
    corner: [f32; 2],
    b: [f32; 2],
    path_tiles: Vec<u32>,
    owner_id: u16,
    level: u8,
    period_ticks: u32,
    opened_at: u64,
    blocked: bool,
    length: f32,
}

#[derive(Default)]
pub(crate) struct RailwayRenderState {
    snapshot_tick: Option<u64>,
    railway_links: Vec<RailwayLink>,
    segments: Vec<RailSegment>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    seen: Vec<u64>,
    frame: u64,
}

impl RailwayRenderState {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn invalidate_snapshot(&mut self) {
        self.snapshot_tick = None;
    }

    fn sync(
        &mut self,
        snapshot: &SimSnapshot,
        map_w: u32,
        map_h: u32,
    ) {
        if self.snapshot_tick == Some(snapshot.tick) {
            return;
        }
        self.snapshot_tick = Some(snapshot.tick);

        if self.railway_links == snapshot.railway_links {
            return;
        }
        self.railway_links.clone_from(&snapshot.railway_links);
        self.segments.clear();
        self.cells.clear();
        for link in &snapshot.railway_links {
            let a = crate::render::world::movers::tile_to_world(link.from_tile, map_w);
            let b = crate::render::world::movers::tile_to_world(link.to_tile, map_w);
            let (ax, ay) = ((link.from_tile % map_w) as i32, (link.from_tile / map_w) as i32);
            let (bx, by) = ((link.to_tile % map_w) as i32, (link.to_tile / map_w) as i32);
            let (cx, cy) = if link.horizontal_first { (bx, ay) } else { (ax, by) };
            let mut path_tiles = Vec::new();
            append_axis_tiles(ax, ay, cx, cy, map_w, map_h, &mut path_tiles);
            append_axis_tiles(cx, cy, bx, by, map_w, map_h, &mut path_tiles);
            let index = self.segments.len();
            for y in ay.min(by).div_euclid(CHUNK)..=ay.max(by).div_euclid(CHUNK) {
                for x in ax.min(bx).div_euclid(CHUNK)..=ax.max(bx).div_euclid(CHUNK) {
                    self.cells.entry((x, y)).or_default().push(index);
                }
            }
            self.segments.push(RailSegment {
                a: [a.0, a.1], b: [b.0, b.1], corner: [cx as f32 + 0.5, cy as f32 + 0.5],
                path_tiles, owner_id: link.owner_id, level: link.level, period_ticks: link.period_ticks,
                opened_at: link.opened_at, blocked: link.blocked,
                length: (ax.abs_diff(bx) + ay.abs_diff(by)) as f32,
            });
        }
        self.seen.resize(self.segments.len(), 0);
    }
}

const CHUNK: i32 = 64;
const MAX_VISIBLE_ROUTES: usize = 256;

fn append_axis_tiles(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    map_w: u32,
    map_h: u32,
    out: &mut Vec<u32>,
) {
    let (mut x, mut y) = (x0, y0);
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    loop {
        if x >= 0 && y >= 0 && x < map_w as i32 && y < map_h as i32 {
            let tile = y as u32 * map_w + x as u32;
            if out.last().copied() != Some(tile) {
                out.push(tile);
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        x += sx;
        y += sy;
    }
}

pub(crate) fn render(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    zoom_scaled: f32,
    _now: Instant,
) {
    let state = &mut ui.railway_render_state;
    let min_zoom = dev.railway_min_zoom.max(0.01);
    if !dev.vfx_railways || zoom_scaled < min_zoom || sim.map_w == 0 || sim.map_h == 0 {
        return;
    }
    state.sync(snapshot, sim.map_w, sim.map_h);
    if state.segments.is_empty() { return; }
    state.frame = state.frame.wrapping_add(1).max(1);
    let vehicle_limit = if dev.vfx_trains { (dev.train_max_count as usize).min(MAX_RENDERED_TRAINS) } else { 0 };
    let screen_w = input.screen_w;
    let screen_h = input.screen_h;
    let points = [[-8.0, -8.0], [screen_w + 8.0, -8.0], [-8.0, screen_h + 8.0], [screen_w + 8.0, screen_h + 8.0]]
        .map(|point| input.screen_to_world(point));
    let bound = |axis: usize, min: bool, cap: u32| {
        let coordinates = points.iter().map(|point| point[axis]);
        let edge = if min { coordinates.fold(f32::INFINITY, f32::min) } else { coordinates.fold(f32::NEG_INFINITY, f32::max) };
        (edge.clamp(0.0, cap.saturating_sub(1) as f32) as i32).div_euclid(CHUNK)
    };
    let width = dev.railway_track_width.clamp(0.5, 6.0) * sf.max(0.01);
    let opacity = dev.railway_opacity.clamp(0.0, 1.0) * ((zoom_scaled - min_zoom) / min_zoom).clamp(0.25, 1.0);
    let vehicle_size = dev.train_size.clamp(3.0, 24.0) * sf.max(0.01);
    let mut drawn = 0;
    let mut vehicles = 0;
    for y in bound(1, true, sim.map_h)..=bound(1, false, sim.map_h) {
        for x in bound(0, true, sim.map_w)..=bound(0, false, sim.map_w) {
            let Some(indices) = state.cells.get(&(x, y)) else { continue };
            for &index in indices {
                if state.seen[index] == state.frame { continue; }
                state.seen[index] = state.frame;
                let segment = &state.segments[index];
                let a = input.world_to_screen(segment.a);
                let corner = input.world_to_screen(segment.corner);
                let b = input.world_to_screen(segment.b);
                if !route_in_view(a, corner, b, screen_w, screen_h, width * 2.0)
                    || (dev.fog_of_war && !segment.path_tiles.iter().all(|&tile| sim.fog_visible.contains(tile)))
                { continue; }
                let railway = segment.level >= 6;
                draw_track_leg(text, a, corner, width, opacity, railway);
                draw_track_leg(text, corner, b, width, opacity, railway);
                drawn += 1;
                if vehicles < vehicle_limit && !segment.blocked && segment.period_ticks != 0 {
                    let elapsed = snapshot.tick.saturating_sub(segment.opened_at) % u64::from(segment.period_ticks);
                    let phase = (elapsed as f32 / segment.period_ticks as f32) * segment.length * 2.0;
                    let color = snapshot.players.iter().find(|player| player.id == segment.owner_id)
                        .map(crate::render::world::nameplates::player_color).unwrap_or([0.85, 0.8, 0.66, 1.0]);
                    for &gap in if railway { &[0.0, 1.15, 2.3][..] } else { &[0.0][..] } {
                        let (position, horizontal) = train_position(segment, phase, gap);
                        let size = if horizontal { [vehicle_size * 1.2, vehicle_size * 0.58] } else { [vehicle_size * 0.58, vehicle_size * 1.2] };
                        text.push_rounded_rect(input.world_to_screen(position), size, 0.25, color, [0.08, 0.07, 0.06, 0.95], (sf * 0.7).max(0.5));
                    }
                    vehicles += 1;
                }
                if drawn >= MAX_VISIBLE_ROUTES { return; }
            }
        }
    }
}

fn route_in_view(
    a: [f32; 2],
    corner: [f32; 2],
    b: [f32; 2],
    screen_w: f32,
    screen_h: f32,
    margin: f32,
) -> bool {
    let min_x = a[0].min(corner[0]).min(b[0]);
    let max_x = a[0].max(corner[0]).max(b[0]);
    let min_y = a[1].min(corner[1]).min(b[1]);
    let max_y = a[1].max(corner[1]).max(b[1]);
    max_x >= -margin && min_x <= screen_w + margin && max_y >= -margin && min_y <= screen_h + margin
}

fn draw_track_leg(text: &mut TextRenderer, a: [f32; 2], b: [f32; 2], width: f32, opacity: f32, railway: bool) {
    if opacity <= 0.0 || ((a[0] - b[0]).abs() + (a[1] - b[1]).abs()) < 0.5 {
        return;
    }
    let rail_width = (width * 0.28).max(0.7);
    let rail_offset = width * 0.4;
    let horizontal = (a[0] - b[0]).abs() >= (a[1] - b[1]).abs();
    let (start, end) = if horizontal {
        (a[0].min(b[0]), a[0].max(b[0]))
    } else {
        (a[1].min(b[1]), a[1].max(b[1]))
    };
    let length = end - start;
    let center = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    if !railway {
        let size = if horizontal { [length, width] } else { [width, length] };
        text.push_rect([center[0] - size[0] * 0.5, center[1] - size[1] * 0.5], size, [0.48, 0.34, 0.20, opacity]);
        return;
    }
    if horizontal {
        text.push_rect(
            [start, center[1] - width * 0.625],
            [length, width * 1.25],
            [
                TRACK_BED_COLOR[0],
                TRACK_BED_COLOR[1],
                TRACK_BED_COLOR[2],
                opacity * 0.42,
            ],
        );
        for offset in [-rail_offset, rail_offset] {
            text.push_rect(
                [start, center[1] + offset - rail_width * 0.5],
                [length, rail_width],
                [
                    TRACK_RAIL_COLOR[0],
                    TRACK_RAIL_COLOR[1],
                    TRACK_RAIL_COLOR[2],
                    opacity,
                ],
            );
        }
    } else {
        text.push_rect(
            [center[0] - width * 0.625, start],
            [width * 1.25, length],
            [
                TRACK_BED_COLOR[0],
                TRACK_BED_COLOR[1],
                TRACK_BED_COLOR[2],
                opacity * 0.42,
            ],
        );
        for offset in [-rail_offset, rail_offset] {
            text.push_rect(
                [center[0] + offset - rail_width * 0.5, start],
                [rail_width, length],
                [
                    TRACK_RAIL_COLOR[0],
                    TRACK_RAIL_COLOR[1],
                    TRACK_RAIL_COLOR[2],
                    opacity,
                ],
            );
        }
    }
}

fn train_position(segment: &RailSegment, phase: f32, gap: f32) -> ([f32; 2], bool) {
    let cycle = segment.length * 2.0;
    let unfolded = (phase - gap).rem_euclid(cycle);
    let distance = if unfolded <= segment.length {
        unfolded
    } else {
        cycle - unfolded
    };
    let (start, end, remaining) = if distance <= segment_leg_length(segment, true) {
        (segment.a, segment.corner, distance)
    } else {
        (
            segment.corner,
            segment.b,
            distance - segment_leg_length(segment, true),
        )
    };
    let leg_length = (end[0] - start[0]).hypot(end[1] - start[1]);
    let t = if leg_length > 0.0 {
        (remaining / leg_length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (
        [
            start[0] + (end[0] - start[0]) * t,
            start[1] + (end[1] - start[1]) * t,
        ],
        (end[0] - start[0]).abs() >= (end[1] - start[1]).abs(),
    )
}

fn segment_leg_length(segment: &RailSegment, first: bool) -> f32 {
    let (a, b) = if first {
        (segment.a, segment.corner)
    } else {
        (segment.corner, segment.b)
    };
    (b[0] - a[0]).hypot(b[1] - a[1])
}
