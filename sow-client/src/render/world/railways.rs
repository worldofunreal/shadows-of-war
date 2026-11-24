use crate::app::{InputState, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::building::{RailwayLink, RailwayTrack, expand_path};
use sow_core::protocol::SimSnapshot;
use std::collections::HashMap;
use web_time::Instant;

const MAX_RENDERED_TRAINS: usize = 60;
const TRACK_BED_COLOR: [f32; 3] = [0.12, 0.11, 0.10];
const TRACK_RAIL_COLOR: [f32; 3] = [0.63, 0.54, 0.40];

struct RailSegment {
    points: Vec<[f32; 2]>,
    path_tiles: Vec<u32>,
    level: u8,
}

struct RailTrainRoute {
    points: Vec<[f32; 2]>,
    path_tiles: Vec<u32>,
    owner_id: u16,
    level: u8,
    period_ticks: u32,
    opened_at: u64,
    length: f32,
}

#[derive(Default)]
pub(crate) struct RailwayRenderState {
    snapshot_tick: Option<u64>,
    railway_links: std::sync::Arc<Vec<RailwayLink>>,
    railway_tracks: std::sync::Arc<Vec<RailwayTrack>>,
    segments: Vec<RailSegment>,
    train_routes: Vec<RailTrainRoute>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    train_cells: HashMap<(i32, i32), Vec<usize>>,
    seen: Vec<u64>,
    train_seen: Vec<u64>,
    fog_of_war: bool,
    frame: u64,
}

impl RailwayRenderState {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn invalidate_snapshot(&mut self) {
        self.snapshot_tick = None;
    }

    fn sync(&mut self, snapshot: &SimSnapshot, map_w: u32, fog_of_war: bool) {
        if self.snapshot_tick == Some(snapshot.tick) && self.fog_of_war == fog_of_war {
            return;
        }
        self.snapshot_tick = Some(snapshot.tick);

        if std::sync::Arc::ptr_eq(&self.railway_links, &snapshot.railway_links)
            && std::sync::Arc::ptr_eq(&self.railway_tracks, &snapshot.railway_tracks)
            && self.fog_of_war == fog_of_war
        {
            return;
        }
        self.railway_links.clone_from(&snapshot.railway_links);
        self.railway_tracks.clone_from(&snapshot.railway_tracks);
        self.segments.clear();
        self.train_routes.clear();
        self.cells.clear();
        self.train_cells.clear();
        self.fog_of_war = fog_of_war;
        let mut track_levels = HashMap::<(u64, u64), u8>::new();
        for route in snapshot.railway_links.iter() {
            for pair in route.station_path.windows(2) {
                track_levels
                    .entry(track_key(pair[0], pair[1]))
                    .and_modify(|level| *level = (*level).max(route.level))
                    .or_insert(route.level);
            }
        }
        for track in snapshot.railway_tracks.iter() {
            let path_tiles = if fog_of_war {
                expand_path(&track.path, map_w)
            } else {
                Vec::new()
            };
            let points: Vec<_> = track
                .path
                .iter()
                .map(|&tile| world_point(tile, map_w))
                .collect();
            if points.len() < 2 {
                continue;
            }
            let index = self.segments.len();
            assign_chunks(&mut self.cells, index, &track.path, map_w);
            self.segments.push(RailSegment {
                points,
                path_tiles,
                level: track_levels
                    .get(&track_key(track.from_node_id, track.to_node_id))
                    .copied()
                    .unwrap_or(1),
            });
        }
        for link in snapshot.railway_links.iter() {
            if link.period_ticks == 0 {
                continue;
            }
            let points: Vec<_> = link
                .route_path
                .iter()
                .map(|&tile| world_point(tile, map_w))
                .collect();
            if points.len() < 2 {
                continue;
            }
            let path_tiles = if fog_of_war {
                expand_path(&link.route_path, map_w)
            } else {
                Vec::new()
            };
            let index = self.train_routes.len();
            assign_chunks(&mut self.train_cells, index, &link.route_path, map_w);
            self.train_routes.push(RailTrainRoute {
                length: polyline_length(&points),
                points,
                path_tiles,
                owner_id: link.owner_id,
                level: link.level,
                period_ticks: link.period_ticks,
                opened_at: link.opened_at,
            });
        }
        self.seen.resize(self.segments.len(), 0);
        self.train_seen.resize(self.train_routes.len(), 0);
    }
}

const CHUNK: i32 = 64;
fn track_key(from: u64, to: u64) -> (u64, u64) {
    (from.min(to), from.max(to))
}

fn world_point(tile: u32, map_w: u32) -> [f32; 2] {
    let (x, y) = crate::render::world::movers::tile_to_world(tile, map_w);
    [x, y]
}

fn polyline_length(points: &[[f32; 2]]) -> f32 {
    points
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).abs() + (pair[1][1] - pair[0][1]).abs())
        .sum()
}

fn assign_chunks(
    cells: &mut HashMap<(i32, i32), Vec<usize>>,
    index: usize,
    tiles: &[u32],
    map_w: u32,
) {
    if tiles.is_empty() || map_w == 0 {
        return;
    }
    let mut chunks = Vec::new();
    for pair in tiles.windows(2) {
        let (mut x, mut y) = ((pair[0] % map_w) as i32, (pair[0] / map_w) as i32);
        let (end_x, end_y) = ((pair[1] % map_w) as i32, (pair[1] / map_w) as i32);
        loop {
            chunks.push((x.div_euclid(CHUNK), y.div_euclid(CHUNK)));
            if x == end_x && y == end_y {
                break;
            }
            if x != end_x {
                x += (end_x - x).signum();
            } else {
                y += (end_y - y).signum();
            }
        }
    }
    if chunks.is_empty() {
        chunks.push((
            ((tiles[0] % map_w) as i32).div_euclid(CHUNK),
            ((tiles[0] / map_w) as i32).div_euclid(CHUNK),
        ));
    }
    chunks.sort_unstable();
    chunks.dedup();
    for chunk in chunks {
        cells.entry(chunk).or_default().push(index);
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
    state.sync(snapshot, sim.map_w, dev.fog_of_war);
    if state.segments.is_empty() && state.train_routes.is_empty() {
        return;
    }
    state.frame = state.frame.wrapping_add(1).max(1);
    let vehicle_limit = if dev.vfx_trains {
        (dev.train_max_count as usize).min(MAX_RENDERED_TRAINS)
    } else {
        0
    };
    let screen_w = input.screen_w;
    let screen_h = input.screen_h;
    let points = [
        [-8.0, -8.0],
        [screen_w + 8.0, -8.0],
        [-8.0, screen_h + 8.0],
        [screen_w + 8.0, screen_h + 8.0],
    ]
    .map(|point| input.screen_to_world(point));
    let bound = |axis: usize, min: bool, cap: u32| {
        let coordinates = points.iter().map(|point| point[axis]);
        let edge = if min {
            coordinates.fold(f32::INFINITY, f32::min)
        } else {
            coordinates.fold(f32::NEG_INFINITY, f32::max)
        };
        (edge.clamp(0.0, cap.saturating_sub(1) as f32) as i32).div_euclid(CHUNK)
    };
    let width = dev.railway_track_width.clamp(0.5, 6.0) * sf.max(0.01);
    let opacity = dev.railway_opacity.clamp(0.0, 1.0)
        * ((zoom_scaled - min_zoom) / min_zoom).clamp(0.25, 1.0);
    let base_vehicle_size =
        dev.train_size.clamp(3.0, 24.0) * sf.max(0.01) * (zoom_scaled * 0.16).clamp(0.9, 1.45);
    for y in bound(1, true, sim.map_h)..=bound(1, false, sim.map_h) {
        for x in bound(0, true, sim.map_w)..=bound(0, false, sim.map_w) {
            let Some(indices) = state.cells.get(&(x, y)) else {
                continue;
            };
            for &index in indices {
                if state.seen[index] == state.frame {
                    continue;
                }
                state.seen[index] = state.frame;
                let segment = &state.segments[index];
                if dev.fog_of_war
                    && !segment
                        .path_tiles
                        .iter()
                        .all(|&tile| sim.fog_visible.contains(tile))
                {
                    continue;
                }
                for pair in segment.points.windows(2) {
                    let a = input.world_to_screen(pair[0]);
                    let b = input.world_to_screen(pair[1]);
                    if route_in_view(a, a, b, screen_w, screen_h, width * 2.0) {
                        draw_track_leg(
                            text,
                            a,
                            b,
                            width,
                            opacity,
                            segment.level,
                            zoom_scaled,
                            [screen_w, screen_h],
                            snapshot.tick,
                        );
                    }
                }
            }
        }
    }
    if vehicle_limit == 0 {
        return;
    }
    let mut vehicles = 0;
    for y in bound(1, true, sim.map_h)..=bound(1, false, sim.map_h) {
        for x in bound(0, true, sim.map_w)..=bound(0, false, sim.map_w) {
            let Some(indices) = state.train_cells.get(&(x, y)) else {
                continue;
            };
            for &index in indices {
                if state.train_seen[index] == state.frame {
                    continue;
                }
                state.train_seen[index] = state.frame;
                let route = &state.train_routes[index];
                if !route_visible(route, input, screen_w, screen_h, width * 2.0)
                    || (dev.fog_of_war
                        && !route
                            .path_tiles
                            .iter()
                            .all(|&tile| sim.fog_visible.contains(tile)))
                {
                    continue;
                }
                let elapsed =
                    snapshot.tick.saturating_sub(route.opened_at) % u64::from(route.period_ticks);
                let phase = (elapsed as f32 / route.period_ticks as f32) * route.length * 2.0;
                let color = snapshot
                    .players
                    .iter()
                    .find(|player| player.id == route.owner_id)
                    .map(crate::render::world::nameplates::player_color)
                    .unwrap_or([0.85, 0.8, 0.66, 1.0]);
                if route.level >= 6 {
                    let gaps: &[f32] = match route.level {
                        6 => &[0.0, 1.32, 2.64],
                        7 => &[0.0, 1.18, 2.36, 3.54],
                        _ => &[0.0, 1.05, 2.10, 3.15],
                    };
                    for (car_index, &gap) in gaps.iter().enumerate() {
                        let (position, dir, loaded) = train_position(&route.points, phase, gap);
                        draw_train_car(
                            text,
                            input.world_to_screen(position),
                            dir,
                            base_vehicle_size,
                            route.level,
                            car_index == 0,
                            loaded,
                            color,
                            snapshot.tick,
                            sf,
                        );
                    }
                } else {
                    let (position, dir, loaded) = train_position(&route.points, phase, 0.0);
                    draw_trade_cart(
                        text,
                        input.world_to_screen(position),
                        dir,
                        base_vehicle_size,
                        route.level,
                        loaded,
                        color,
                        sf,
                    );
                }
                vehicles += 1;
                if vehicles >= vehicle_limit {
                    return;
                }
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

fn route_visible(
    route: &RailTrainRoute,
    input: &InputState,
    screen_w: f32,
    screen_h: f32,
    margin: f32,
) -> bool {
    route.points.windows(2).any(|pair| {
        let a = input.world_to_screen(pair[0]);
        let b = input.world_to_screen(pair[1]);
        route_in_view(a, a, b, screen_w, screen_h, margin)
    })
}

#[inline]
fn push_axis_strip(
    text: &mut TextRenderer,
    center: [f32; 2],
    horizontal: bool,
    length: f32,
    thickness: f32,
    color: [f32; 4],
) {
    let size = if horizontal {
        [length, thickness]
    } else {
        [thickness, length]
    };
    text.push_rect(
        [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
        size,
        color,
    );
}

#[inline]
fn offset_cross(center: [f32; 2], horizontal: bool, cross: f32) -> [f32; 2] {
    if horizontal {
        [center[0], center[1] + cross]
    } else {
        [center[0] + cross, center[1]]
    }
}

#[inline]
fn offset_along(center: [f32; 2], horizontal: bool, along: f32) -> [f32; 2] {
    if horizontal {
        [center[0] + along, center[1]]
    } else {
        [center[0], center[1] + along]
    }
}

const MAX_MARKS_PER_LEG: i32 = 48;

fn draw_leg_marks(
    text: &mut TextRenderer,
    center: [f32; 2],
    horizontal: bool,
    start: f32,
    end: f32,
    viewport_limit: f32,
    step: f32,
    mark_along: f32,
    mark_cross: f32,
    color: [f32; 4],
) {
    let vis_start = start.max(-8.0);
    let vis_end = end.min(viewport_limit + 8.0);
    if vis_end <= vis_start || step <= 0.5 {
        return;
    }
    let first = ((vis_start - start) / step).ceil().max(0.0) as i32;
    let last = ((vis_end - start) / step).floor().max(-1.0) as i32;
    if last < first {
        return;
    }
    let count = last - first + 1;
    let stride = (count + MAX_MARKS_PER_LEG - 1) / MAX_MARKS_PER_LEG;
    let mut index = first;
    while index <= last {
        let pos_along =
            start + index as f32 * step - if horizontal { center[0] } else { center[1] };
        push_axis_strip(
            text,
            offset_along(center, horizontal, pos_along),
            horizontal,
            mark_along,
            mark_cross,
            color,
        );
        index += stride;
    }
}

fn draw_track_leg(
    text: &mut TextRenderer,
    a: [f32; 2],
    b: [f32; 2],
    width: f32,
    opacity: f32,
    level: u8,
    zoom_scaled: f32,
    viewport: [f32; 2],
    tick: u64,
) {
    if opacity <= 0.0 || ((a[0] - b[0]).abs() + (a[1] - b[1]).abs()) < 0.5 {
        return;
    }
    let horizontal = (a[0] - b[0]).abs() >= (a[1] - b[1]).abs();
    let (start, end) = if horizontal {
        (a[0].min(b[0]), a[0].max(b[0]))
    } else {
        (a[1].min(b[1]), a[1].max(b[1]))
    };
    let length = end - start;
    let center = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let viewport_limit = if horizontal { viewport[0] } else { viewport[1] };
    let detail_zoom = zoom_scaled >= 1.1;

    if level < 6 {
        let (border_color, road_color, road_scale) = match level {
            0 | 1 => (
                [0.30, 0.21, 0.13, opacity * 0.44],
                [0.49, 0.34, 0.20, opacity * 0.92],
                0.85,
            ),
            2 => (
                [0.30, 0.29, 0.17, opacity * 0.52],
                [0.54, 0.39, 0.22, opacity * 0.95],
                0.98,
            ),
            3 => (
                [0.24, 0.22, 0.20, opacity * 0.65],
                [0.62, 0.56, 0.47, opacity * 0.95],
                1.06,
            ),
            4 => (
                [0.20, 0.19, 0.19, opacity * 0.72],
                [0.48, 0.45, 0.41, opacity * 0.95],
                1.14,
            ),
            _ => (
                [0.15, 0.15, 0.17, opacity * 0.80],
                [0.33, 0.32, 0.32, opacity * 0.96],
                1.20,
            ),
        };
        let inner = width * road_scale;
        let outer = inner * 1.32;
        push_axis_strip(text, center, horizontal, length, outer, border_color);
        push_axis_strip(text, center, horizontal, length, inner, road_color);
        if detail_zoom && level == 4 {
            draw_leg_marks(
                text,
                center,
                horizontal,
                start,
                end,
                viewport_limit,
                (inner * 1.6).max(6.0),
                (inner * 0.25).max(1.0),
                inner * 0.82,
                [0.28, 0.26, 0.23, opacity * 0.45],
            );
        } else if detail_zoom && level == 5 {
            draw_leg_marks(
                text,
                center,
                horizontal,
                start,
                end,
                viewport_limit,
                (inner * 2.2).max(8.0),
                (inner * 0.85).max(3.0),
                (inner * 0.18).max(0.8),
                [0.88, 0.80, 0.56, opacity * 0.75],
            );
        }
        return;
    }

    let (bed_color, bed_w, rail_color, rail_offset, rail_width) = match level {
        6 => (
            [
                TRACK_BED_COLOR[0],
                TRACK_BED_COLOR[1],
                TRACK_BED_COLOR[2],
                opacity * 0.46,
            ],
            width * 1.30,
            [
                TRACK_RAIL_COLOR[0],
                TRACK_RAIL_COLOR[1],
                TRACK_RAIL_COLOR[2],
                opacity,
            ],
            width * 0.42,
            (width * 0.28).max(0.75),
        ),
        7 => (
            [0.14, 0.15, 0.17, opacity * 0.54],
            width * 1.38,
            [0.78, 0.82, 0.86, opacity],
            width * 0.44,
            (width * 0.28).max(0.75),
        ),
        _ => (
            [0.10, 0.12, 0.17, opacity * 0.72],
            width * 1.48,
            [0.76, 0.85, 0.94, opacity],
            width * 0.46,
            (width * 0.26).max(0.75),
        ),
    };
    push_axis_strip(text, center, horizontal, length, bed_w, bed_color);
    if detail_zoom {
        let (tie_step, tie_thick, tie_span, tie_color) = match level {
            6 => (
                (width * 1.4).max(5.5),
                (width * 0.30).max(1.0),
                bed_w * 0.95,
                [0.38, 0.25, 0.14, opacity * 0.72],
            ),
            7 => (
                (width * 1.5).max(6.0),
                (width * 0.28).max(1.0),
                bed_w * 0.92,
                [0.54, 0.56, 0.60, opacity * 0.66],
            ),
            _ => (
                (width * 1.8).max(7.0),
                (width * 0.26).max(0.9),
                bed_w * 0.88,
                [0.24, 0.78, 0.95, opacity * 0.42],
            ),
        };
        draw_leg_marks(
            text,
            center,
            horizontal,
            start,
            end,
            viewport_limit,
            tie_step,
            tie_thick,
            tie_span,
            tie_color,
        );
    }
    for offset in [-rail_offset, rail_offset] {
        push_axis_strip(
            text,
            offset_cross(center, horizontal, offset),
            horizontal,
            length,
            rail_width,
            rail_color,
        );
    }
    if level >= 8 {
        let pulse = 0.72 + 0.16 * (tick as f32 * 0.25).sin();
        push_axis_strip(
            text,
            center,
            horizontal,
            length,
            (width * 0.24).max(0.8),
            [0.24, 0.86, 1.0, opacity * pulse],
        );
    }
}

#[inline]
fn oriented_size(along: f32, across: f32, dir: [f32; 2]) -> [f32; 2] {
    if dir[0].abs() >= dir[1].abs() {
        [along, across]
    } else {
        [across, along]
    }
}

#[inline]
fn offset_along_dir(center: [f32; 2], dir: [f32; 2], along: f32, across: f32) -> [f32; 2] {
    let perp = [-dir[1], dir[0]];
    [
        center[0] + dir[0] * along + perp[0] * across,
        center[1] + dir[1] * along + perp[1] * across,
    ]
}

fn draw_cart_wheels(
    text: &mut TextRenderer,
    center: [f32; 2],
    dir: [f32; 2],
    length: f32,
    breadth: f32,
    wheel_radius: f32,
    single_axle: bool,
) {
    let wheel_color = [0.16, 0.12, 0.09, 0.98];
    let axles: &[f32] = if single_axle {
        &[0.0]
    } else {
        &[-length * 0.28, length * 0.28]
    };
    for &along in axles {
        for across in [-breadth * 0.52, breadth * 0.52] {
            text.push_disc(
                offset_along_dir(center, dir, along, across),
                wheel_radius,
                wheel_color,
            );
        }
    }
}

fn draw_trade_cart(
    text: &mut TextRenderer,
    center: [f32; 2],
    dir: [f32; 2],
    base_size: f32,
    level: u8,
    loaded: bool,
    color: [f32; 4],
    sf: f32,
) {
    let scale = 1.0 + 0.06 * f32::from(level.saturating_sub(1).min(4));
    let length = base_size * 1.46 * scale;
    let breadth = base_size * 0.84 * scale;
    let outline = (sf * 0.75).max(0.6);

    text.push_rounded_rect(
        [center[0], center[1] + sf],
        oriented_size(length * 1.04, breadth * 1.12, dir),
        0.24,
        [0.0, 0.0, 0.0, 0.34],
        [0.0; 4],
        0.0,
    );
    text.push_rounded_rect(
        offset_along_dir(center, dir, length * 0.54, 0.0),
        oriented_size(length * 0.24, breadth * 0.22, dir),
        0.3,
        [0.36, 0.24, 0.14, 0.95],
        [0.08, 0.06, 0.04, 0.9],
        (sf * 0.5).max(0.5),
    );
    let wheel_r = if level <= 1 {
        (breadth * 0.26).max(1.8)
    } else {
        (breadth * 0.21).max(1.5)
    };
    draw_cart_wheels(text, center, dir, length, breadth, wheel_r, level <= 1);

    let frame_border = [0.12, 0.09, 0.06, 0.96];
    let frame_fill = match level {
        0 | 1 => [0.54, 0.35, 0.17, 1.0],
        2 | 3 => [0.60, 0.40, 0.21, 1.0],
        4 => [0.50, 0.34, 0.19, 1.0],
        _ => [0.24, 0.21, 0.19, 1.0],
    };
    text.push_rounded_rect(
        center,
        oriented_size(length, breadth, dir),
        0.20,
        frame_fill,
        frame_border,
        outline,
    );
    text.push_rounded_rect(
        center,
        oriented_size(length * 0.82, breadth * 0.72, dir),
        0.18,
        [color[0], color[1], color[2], 0.96],
        frame_border,
        (sf * 0.5).max(0.45),
    );

    match level {
        0 | 1 | 2 => {
            if loaded {
                text.push_rounded_rect(
                    offset_along_dir(center, dir, -length * 0.05, 0.0),
                    oriented_size(length * 0.48, breadth * 0.48, dir),
                    0.24,
                    [0.98, 0.80, 0.22, 1.0],
                    [0.45, 0.30, 0.05, 0.92],
                    (sf * 0.45).max(0.4),
                );
            } else {
                text.push_rounded_rect(
                    offset_along_dir(center, dir, -length * 0.05, 0.0),
                    oriented_size(length * 0.44, breadth * 0.44, dir),
                    0.20,
                    [0.42, 0.28, 0.16, 0.9],
                    [0.18, 0.12, 0.07, 0.85],
                    (sf * 0.35).max(0.35),
                );
            }
        }
        3 => {
            text.push_rounded_rect(
                offset_along_dir(center, dir, -length * 0.05, 0.0),
                oriented_size(length * 0.56, breadth * 0.52, dir),
                0.28,
                if loaded {
                    [0.98, 0.82, 0.26, 1.0]
                } else {
                    [0.85, 0.80, 0.70, 0.94]
                },
                frame_border,
                (sf * 0.4).max(0.35),
            );
        }
        4 => {
            text.push_rounded_rect(
                offset_along_dir(center, dir, -length * 0.06, 0.0),
                oriented_size(length * 0.64, breadth * 0.66, dir),
                0.38,
                [0.94, 0.90, 0.80, 0.98],
                [0.34, 0.28, 0.20, 0.85],
                (sf * 0.45).max(0.4),
            );
            if loaded {
                text.push_rounded_rect(
                    offset_along_dir(center, dir, length * 0.20, 0.0),
                    oriented_size(length * 0.20, breadth * 0.52, dir),
                    0.25,
                    [0.98, 0.80, 0.22, 1.0],
                    [0.42, 0.28, 0.05, 0.85],
                    (sf * 0.4).max(0.35),
                );
            }
        }
        _ => {
            text.push_rounded_rect(
                offset_along_dir(center, dir, -length * 0.04, 0.0),
                oriented_size(length * 0.58, breadth * 0.58, dir),
                0.24,
                [0.24, 0.21, 0.20, 0.98],
                frame_border,
                (sf * 0.4).max(0.35),
            );
            text.push_disc(
                offset_along_dir(center, dir, length * 0.36, 0.0),
                (breadth * 0.18).max(1.3),
                [0.99, 0.88, 0.36, 0.98],
            );
            if loaded {
                text.push_rounded_rect(
                    offset_along_dir(center, dir, -length * 0.08, 0.0),
                    oriented_size(length * 0.34, breadth * 0.38, dir),
                    0.22,
                    [0.98, 0.80, 0.22, 1.0],
                    [0.42, 0.28, 0.05, 0.85],
                    (sf * 0.35).max(0.35),
                );
            }
        }
    }
}

fn draw_train_car(
    text: &mut TextRenderer,
    center: [f32; 2],
    dir: [f32; 2],
    base_size: f32,
    level: u8,
    is_locomotive: bool,
    loaded: bool,
    color: [f32; 4],
    tick: u64,
    sf: f32,
) {
    let is_high_speed = level >= 8;
    let is_modern = level == 7;
    let length = base_size
        * if is_high_speed {
            1.48
        } else if is_locomotive {
            1.42
        } else {
            1.24
        };
    let breadth = base_size * if is_high_speed { 0.64 } else { 0.68 };
    let outline = (sf * 0.7).max(0.5);

    if is_high_speed {
        let shell_color = [0.91, 0.94, 0.98, 1.0];
        let edge = [0.16, 0.20, 0.28, 0.95];
        if is_locomotive {
            text.push_rounded_rect(
                offset_along_dir(center, dir, -length * 0.45, 0.0),
                oriented_size(length * 0.75, breadth * 0.36, dir),
                0.45,
                [0.24, 0.86, 1.0, 0.22],
                [0.0; 4],
                0.0,
            );
            text.push_rounded_rect(
                center,
                oriented_size(length, breadth, dir),
                0.48,
                shell_color,
                edge,
                outline,
            );
            text.push_rounded_rect(
                offset_along_dir(center, dir, -length * 0.08, 0.0),
                oriented_size(length * 0.56, breadth * 0.32, dir),
                0.3,
                [color[0], color[1], color[2], 0.95],
                [0.0; 4],
                0.0,
            );
            text.push_rounded_rect(
                offset_along_dir(center, dir, length * 0.26, 0.0),
                oriented_size(length * 0.26, breadth * 0.52, dir),
                0.45,
                [0.22, 0.88, 1.0, 0.96],
                [0.08, 0.14, 0.22, 0.9],
                (sf * 0.4).max(0.35),
            );
        } else {
            text.push_rounded_rect(
                center,
                oriented_size(length * 0.96, breadth * 0.94, dir),
                0.28,
                shell_color,
                edge,
                outline,
            );
            text.push_rounded_rect(
                center,
                oriented_size(length * 0.72, breadth * 0.40, dir),
                0.22,
                if loaded {
                    [0.98, 0.82, 0.24, 0.98]
                } else {
                    [0.24, 0.84, 0.98, 0.72]
                },
                [0.12, 0.16, 0.24, 0.75],
                (sf * 0.35).max(0.3),
            );
        }
        return;
    }

    let body_color = if is_locomotive {
        if is_modern {
            [0.22, 0.25, 0.30, 1.0]
        } else {
            [0.20, 0.18, 0.17, 1.0]
        }
    } else {
        color
    };
    text.push_rounded_rect(
        center,
        oriented_size(length, breadth, dir),
        if is_modern { 0.32 } else { 0.22 },
        body_color,
        [0.08, 0.07, 0.06, 0.95],
        outline,
    );
    if is_locomotive {
        text.push_rounded_rect(
            offset_along_dir(center, dir, -length * 0.12, 0.0),
            oriented_size(length * 0.48, breadth * 0.62, dir),
            0.2,
            color,
            [0.08, 0.07, 0.06, 0.8],
            (sf * 0.4).max(0.4),
        );
        text.push_disc(
            offset_along_dir(center, dir, length * 0.28, 0.0),
            (breadth * 0.25).max(1.5),
            if is_modern {
                [0.98, 0.95, 0.72, 0.98]
            } else {
                [0.98, 0.80, 0.24, 0.98]
            },
        );
        if !is_modern {
            let puff_drift = (tick % 8) as f32 * 0.08;
            for (idx, step) in [0.42, 0.78].iter().enumerate() {
                let alpha = (0.34 - idx as f32 * 0.12).max(0.1);
                text.push_disc(
                    offset_along_dir(center, dir, -length * (step + puff_drift), 0.0),
                    (breadth * (0.28 + idx as f32 * 0.08)).max(1.5),
                    [0.86, 0.86, 0.84, alpha],
                );
            }
        }
    } else if loaded {
        text.push_rounded_rect(
            center,
            oriented_size(length * 0.62, breadth * 0.46, dir),
            0.2,
            [0.98, 0.80, 0.22, 0.98],
            [0.42, 0.28, 0.05, 0.85],
            (sf * 0.35).max(0.35),
        );
    } else {
        text.push_rounded_rect(
            center,
            oriented_size(length * 0.56, breadth * 0.42, dir),
            0.18,
            [0.24, 0.22, 0.20, 0.72],
            [0.10, 0.09, 0.08, 0.65],
            (sf * 0.3).max(0.3),
        );
    }
}

fn train_position(points: &[[f32; 2]], phase: f32, gap: f32) -> ([f32; 2], [f32; 2], bool) {
    let length = polyline_length(points);
    let cycle = length * 2.0;
    let unfolded = (phase - gap).rem_euclid(cycle);
    let forward = unfolded <= length;
    let mut remaining = if forward { unfolded } else { cycle - unfolded };
    let pair_count = points.len().saturating_sub(1);
    for step in 0..pair_count {
        let index = if forward { step } else { pair_count - step - 1 };
        let (start, end) = if forward {
            (points[index], points[index + 1])
        } else {
            (points[index + 1], points[index])
        };
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let leg = dx.abs() + dy.abs();
        if remaining <= leg || leg == 0.0 {
            let t = if leg > 0.0 {
                (remaining / leg).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return (
                [start[0] + dx * t, start[1] + dy * t],
                [dx.signum(), dy.signum()],
                !forward,
            );
        }
        remaining -= leg;
    }
    (*points.last().unwrap_or(&[0.0, 0.0]), [0.0, 0.0], !forward)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn train_position_distinguishes_outbound_empty_and_return_loaded_trips() {
        let route = [[0.5, 0.5], [10.5, 0.5], [10.5, 5.5]];
        let (pos_out, dir_out, loaded_out) = train_position(&route, 5.0, 0.0);
        assert!(!loaded_out);
        assert_eq!(dir_out, [1.0, 0.0]);
        assert!((pos_out[0] - 5.5).abs() < 1e-4);

        let (pos_ret, dir_ret, loaded_ret) = train_position(&route, 25.0, 0.0);
        assert!(loaded_ret);
        assert_eq!(dir_ret, [0.0, -1.0]);
        assert!((pos_ret[1] - 0.5).abs() < 1e-4);
    }
}
