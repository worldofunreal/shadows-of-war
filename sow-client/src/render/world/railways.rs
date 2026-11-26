use crate::app::{InputState, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::building::{
    AStarScratch, RailPlacementPreview, RailwayLink, RailwayTrack, compress_path, expand_path,
    index_tracks, preview_building_rail_connection,
};
use sow_core::game::BuildingKind;
use sow_core::protocol::SimSnapshot;
use std::collections::HashMap;
use web_time::Instant;

const MAX_RENDERED_TRAINS: usize = 60;
/// Full road width in tiles per level (1 footpath … 5 highway, 6 steam … 8 maglev).
const ROUTE_WIDTH_TILES: [f32; 8] = [0.20, 0.26, 0.32, 0.36, 0.46, 0.38, 0.42, 0.46];

struct RailSegment {
    key: (u64, u64),
    points: Vec<[f32; 2]>,
    path_tiles: Vec<u32>,
    length: f32,
    level: u8,
}

struct RailTrainRoute {
    points: Vec<[f32; 2]>,
    path_tiles: Vec<u32>,
    track_keys: Vec<(u64, u64)>,
    /// Track levels along the route: (end as a fraction of the route, level).
    track_levels: Vec<(f32, u8)>,
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
    track_index_by_key: HashMap<(u64, u64), usize>,
    track_index_by_tile: HashMap<u32, Vec<(u64, u64)>>,
    routes_by_track_key: HashMap<(u64, u64), Vec<usize>>,
    segments: Vec<RailSegment>,
    train_routes: Vec<RailTrainRoute>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    train_cells: HashMap<(i32, i32), Vec<usize>>,
    seen: Vec<u64>,
    train_seen: Vec<u64>,
    fog_of_war: bool,
    frame: u64,
    station_tiles: Vec<(u64, u32)>,
    preview_astar: AStarScratch,
    cached_preview_key: Option<(BuildingKind, u32, usize, usize)>,
    cached_preview: RailPlacementPreview,
    cached_ghost_points: Vec<Vec<[f32; 2]>>,
    city_levels_hash: u64,
    epoch: Option<Instant>,
    pub(crate) hovered_segment: Option<usize>,
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

        let fog_changed = self.fog_of_war != fog_of_war;
        let tracks_dirty =
            fog_changed || !std::sync::Arc::ptr_eq(&self.railway_tracks, &snapshot.railway_tracks);
        let links_dirty =
            fog_changed || !std::sync::Arc::ptr_eq(&self.railway_links, &snapshot.railway_links);
        let city_levels_hash = snapshot
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::City)
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, b| {
                (hash ^ b.id.rotate_left(8) ^ u64::from(b.level)).wrapping_mul(0x0100_0000_01b3)
            });
        let cities_dirty = city_levels_hash != self.city_levels_hash;
        self.city_levels_hash = city_levels_hash;
        if !tracks_dirty && !links_dirty && !cities_dirty {
            return;
        }
        self.fog_of_war = fog_of_war;

        if tracks_dirty {
            self.railway_tracks.clone_from(&snapshot.railway_tracks);
            self.segments.clear();
            self.cells.clear();
            self.track_index_by_key.clear();
            self.track_index_by_tile = index_tracks(&snapshot.railway_tracks, map_w);
            self.cached_preview_key = None;
            self.hovered_segment = None;
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
                let key = track_key(track.from_node_id, track.to_node_id);
                assign_chunks(&mut self.cells, index, &track.path, map_w);
                self.track_index_by_key.insert(key, index);
                self.segments.push(RailSegment {
                    key,
                    length: polyline_length(&points),
                    points,
                    path_tiles,
                    level: 1,
                });
            }
            self.seen.resize(self.segments.len(), 0);
        }

        if links_dirty {
            self.train_routes.clear();
            self.train_cells.clear();
            self.routes_by_track_key.clear();
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
                let track_keys: Vec<_> = link
                    .station_path
                    .windows(2)
                    .map(|pair| track_key(pair[0], pair[1]))
                    .collect();
                for &key in &track_keys {
                    self.routes_by_track_key.entry(key).or_default().push(index);
                }
                self.train_routes.push(RailTrainRoute {
                    length: polyline_length(&points),
                    points,
                    path_tiles,
                    track_keys,
                    track_levels: Vec::new(),
                    owner_id: link.owner_id,
                    level: link.level,
                    period_ticks: link.period_ticks,
                    opened_at: link.opened_at,
                });
            }
            self.train_seen.resize(self.train_routes.len(), 0);
        }

        self.railway_links.clone_from(&snapshot.railway_links);
        self.assign_levels(&snapshot.buildings);
    }

    /// Track level for drawing. Every track inherits the best City it reaches
    /// without passing through another City, so a City upgrade restyles all
    /// tracks out to the next City. Tracks that reach no City keep the level of
    /// the best Trading Post route that runs on them.
    fn assign_levels(&mut self, buildings: &[sow_core::protocol::BuildingSnapshot]) {
        for segment in &mut self.segments {
            segment.level = 1;
        }
        for route in self.railway_links.iter() {
            for pair in route.station_path.windows(2) {
                if let Some(&seg_idx) = self.track_index_by_key.get(&track_key(pair[0], pair[1])) {
                    let level = &mut self.segments[seg_idx].level;
                    *level = (*level).max(route.level);
                }
            }
        }

        let cities: HashMap<u64, u8> = buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::City && b.level > 0)
            .map(|b| (b.id, b.level))
            .collect();
        let mut by_node: HashMap<u64, Vec<usize>> = HashMap::new();
        for (index, segment) in self.segments.iter().enumerate() {
            by_node.entry(segment.key.0).or_default().push(index);
            by_node.entry(segment.key.1).or_default().push(index);
        }
        // Flood each stretch of track between Cities; it takes the best City on its border.
        let mut visited = vec![false; self.segments.len()];
        let mut stretch = Vec::new();
        for start in 0..self.segments.len() {
            if visited[start] {
                continue;
            }
            visited[start] = true;
            stretch.clear();
            stretch.push(start);
            let mut best = 0;
            let mut next = 0;
            while let Some(&index) = stretch.get(next) {
                next += 1;
                let (from, to) = self.segments[index].key;
                for node in [from, to] {
                    if let Some(&level) = cities.get(&node) {
                        best = best.max(level);
                        continue;
                    }
                    for &other in by_node.get(&node).into_iter().flatten() {
                        if !visited[other] {
                            visited[other] = true;
                            stretch.push(other);
                        }
                    }
                }
            }
            if best > 0 {
                for &index in &stretch {
                    self.segments[index].level = best;
                }
            }
        }

        for route in &mut self.train_routes {
            route.track_levels.clear();
            let mut total = 0.0;
            for key in &route.track_keys {
                if let Some(&index) = self.track_index_by_key.get(key) {
                    total += self.segments[index].length;
                    route.track_levels.push((total, self.segments[index].level));
                }
            }
            if total > 0.0 {
                for (end, _) in &mut route.track_levels {
                    *end /= total;
                }
            }
        }
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
        let (x0, y0) = ((pair[0] % map_w) as i32, (pair[0] / map_w) as i32);
        let (x1, y1) = ((pair[1] % map_w) as i32, (pair[1] / map_w) as i32);
        let (cx0, cy0) = (x0.div_euclid(CHUNK), y0.div_euclid(CHUNK));
        let (cx1, cy1) = (x1.div_euclid(CHUNK), y1.div_euclid(CHUNK));
        for cy in cy0.min(cy1)..=cy0.max(cy1) {
            for cx in cx0.min(cx1)..=cx0.max(cx1) {
                chunks.push((cx, cy));
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
    now: Instant,
) {
    let state = &mut ui.railway_render_state;
    let min_zoom = dev.railway_min_zoom.max(0.01);
    if !dev.vfx_railways || sim.map_w == 0 || sim.map_h == 0 {
        state.hovered_segment = None;
        return;
    }
    state.sync(snapshot, sim.map_w, dev.fog_of_war);
    let placement = ui.app.hud_state.building_placement.filter(|p| !p.pending);
    if zoom_scaled < min_zoom && placement.is_none() {
        state.hovered_segment = None;
        return;
    }
    if state.segments.is_empty() && state.train_routes.is_empty() && placement.is_none() {
        state.hovered_segment = None;
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
    let seconds = now
        .duration_since(*state.epoch.get_or_insert(now))
        .as_secs_f32()
        % 600.0;
    let base_vehicle_size =
        dev.train_size.clamp(3.0, 24.0) * sf.max(0.01) * (zoom_scaled * 0.16).clamp(0.9, 1.45);
    if zoom_scaled >= min_zoom {
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
                    let full_width = (input.camera_zoom
                        * ROUTE_WIDTH_TILES[usize::from(segment.level.clamp(1, 8) - 1)])
                    .max(width * (1.0 + 0.08 * f32::from(segment.level)))
                    .min(20.0 * sf.max(0.01));
                    draw_route(
                        text,
                        input,
                        &segment.points,
                        full_width * 0.5,
                        segment.level,
                        opacity,
                        [screen_w, screen_h],
                        seconds,
                    );
                }
            }
        }
    }

    if let Some(placement) = placement {
        state.hovered_segment = None;
        render_placement_rail_highlight(
            text,
            state,
            snapshot,
            sim,
            input,
            placement.kind,
            placement.tile_idx,
            width,
            sf,
            [screen_w, screen_h],
        );
    } else if zoom_scaled >= min_zoom {
        let desktop_hover = crate::web_canvas::supports_desktop_hover()
            && matches!(input.hover_pointer, crate::app::HoverPointer::Mouse)
            && !input.is_pointer_gesture_active()
            && !input.dragging
            && input.building_drag.is_none()
            && ui.hovered_building.is_none()
            && input.camera_zoom > 0.0
            && input.last_mouse_x >= 0.0
            && input.last_mouse_y >= 0.0;
        if desktop_hover {
            let pointer_world =
                input.screen_to_world([input.last_mouse_x as f32, input.last_mouse_y as f32]);
            let max_world_dist = (10.0 * sf / input.camera_zoom).clamp(0.45, 2.2);
            state.hovered_segment = pick_hovered_rail_segment(
                state,
                pointer_world,
                max_world_dist,
                dev.fog_of_war.then_some(&sim.fog_visible),
            );
            if let Some(seg_idx) = state.hovered_segment {
                render_hover_rail_highlight(
                    text,
                    state,
                    seg_idx,
                    sim,
                    input,
                    dev.fog_of_war,
                    width,
                    [screen_w, screen_h],
                );
            }
        } else {
            state.hovered_segment = None;
        }
    } else {
        state.hovered_segment = None;
    }

    if vehicle_limit == 0 || zoom_scaled < min_zoom {
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
                // Vehicles match the track under the lead car, not the Trading Post.
                let lead = phase.rem_euclid(route.length * 2.0);
                let fraction = lead.min(route.length * 2.0 - lead) / route.length.max(1e-3);
                let level = route
                    .track_levels
                    .iter()
                    .find(|&&(end, _)| fraction <= end)
                    .map_or(route.level, |&(_, level)| level);
                if level >= 6 {
                    let gaps: &[f32] = match level {
                        6 => &[0.0, 1.32, 2.64],
                        7 => &[0.0, 1.18, 2.36, 3.54],
                        _ => &[0.0, 1.05, 2.10, 3.15],
                    };
                    for (car_index, &gap) in gaps.iter().enumerate() {
                        let (position, dir, loaded) =
                            train_position(&route.points, route.length, phase, gap);
                        draw_train_car(
                            text,
                            input.world_to_screen(position),
                            dir,
                            base_vehicle_size,
                            level,
                            car_index == 0,
                            loaded,
                            color,
                            snapshot.tick,
                            sf,
                        );
                    }
                } else {
                    let (position, dir, loaded) =
                        train_position(&route.points, route.length, phase, 0.0);
                    draw_trade_cart(
                        text,
                        input.world_to_screen(position),
                        dir,
                        base_vehicle_size,
                        level,
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

#[inline]
fn point_to_segment_dist_sq(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let len_sq = vx * vx + vy * vy;
    let t = if len_sq > 1e-6 {
        (((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (px, py) = (a[0] + vx * t - p[0], a[1] + vy * t - p[1]);
    px * px + py * py
}

fn pick_hovered_rail_segment(
    state: &RailwayRenderState,
    pointer_world: [f32; 2],
    max_world_dist: f32,
    fog_visible: Option<&sow_core::bitset::DenseBitSet>,
) -> Option<usize> {
    if state.segments.is_empty() || max_world_dist <= 0.0 {
        return None;
    }
    let cx = (pointer_world[0].floor() as i32).div_euclid(CHUNK);
    let cy = (pointer_world[1].floor() as i32).div_euclid(CHUNK);
    let max_sq = max_world_dist * max_world_dist;
    let mut best: Option<(f32, usize)> = None;

    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(indices) = state.cells.get(&(cx + dx, cy + dy)) else {
                continue;
            };
            for &seg_idx in indices {
                let seg = &state.segments[seg_idx];
                if let Some(visible) = fog_visible
                    && !seg.path_tiles.iter().all(|&tile| visible.contains(tile))
                {
                    continue;
                }
                for pair in seg.points.windows(2) {
                    let d_sq = point_to_segment_dist_sq(pointer_world, pair[0], pair[1]);
                    if d_sq <= max_sq && best.is_none_or(|(b_sq, _)| d_sq < b_sq) {
                        best = Some((d_sq, seg_idx));
                    }
                }
            }
        }
    }
    best.map(|(_, idx)| idx)
}

#[allow(clippy::too_many_arguments)]
fn draw_highlight_leg(
    text: &mut TextRenderer,
    a: [f32; 2],
    b: [f32; 2],
    halo_width: f32,
    core_width: f32,
    halo_color: [f32; 4],
    core_color: [f32; 4],
    dashed: bool,
    viewport: [f32; 2],
) {
    if ((a[0] - b[0]).abs() + (a[1] - b[1]).abs()) < 0.5 {
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

    push_axis_strip(text, center, horizontal, length, halo_width, halo_color);
    if dashed {
        draw_leg_marks(
            text,
            center,
            horizontal,
            start,
            end,
            viewport_limit,
            (core_width * 4.2).max(10.0),
            (core_width * 2.2).max(5.5),
            core_width,
            core_color,
        );
    } else {
        push_axis_strip(text, center, horizontal, length, core_width, core_color);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_hover_rail_highlight(
    text: &mut TextRenderer,
    state: &RailwayRenderState,
    seg_idx: usize,
    sim: &SimState,
    input: &InputState,
    fog_of_war: bool,
    width: f32,
    viewport: [f32; 2],
) {
    let seg = &state.segments[seg_idx];
    if let Some(route_indices) = state.routes_by_track_key.get(&seg.key) {
        for &route_idx in route_indices {
            let route = &state.train_routes[route_idx];
            if !route_visible(route, input, viewport[0], viewport[1], width * 2.0)
                || (fog_of_war
                    && !route
                        .path_tiles
                        .iter()
                        .all(|&tile| sim.fog_visible.contains(tile)))
            {
                continue;
            }
            for pair in route.points.windows(2) {
                let a = input.world_to_screen(pair[0]);
                let b = input.world_to_screen(pair[1]);
                if route_in_view(a, a, b, viewport[0], viewport[1], width * 2.0) {
                    draw_highlight_leg(
                        text,
                        a,
                        b,
                        (width * 2.1).max(3.5),
                        (width * 1.15).max(1.8),
                        [1.0, 0.84, 0.28, 0.16],
                        [1.0, 0.88, 0.42, 0.55],
                        false,
                        viewport,
                    );
                }
            }
        }
    }

    for pair in seg.points.windows(2) {
        let a = input.world_to_screen(pair[0]);
        let b = input.world_to_screen(pair[1]);
        if route_in_view(a, a, b, viewport[0], viewport[1], width * 3.0) {
            draw_highlight_leg(
                text,
                a,
                b,
                (width * 2.7).max(4.5),
                (width * 1.4).max(2.2),
                [1.0, 0.84, 0.25, 0.34],
                [1.0, 0.92, 0.52, 0.96],
                false,
                viewport,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_placement_rail_highlight(
    text: &mut TextRenderer,
    state: &mut RailwayRenderState,
    snapshot: &SimSnapshot,
    sim: &SimState,
    input: &InputState,
    kind: BuildingKind,
    tile_idx: u32,
    width: f32,
    sf: f32,
    viewport: [f32; 2],
) {
    let Some(engine) = sim.engine.as_ref() else {
        return;
    };
    let tracks_ptr = std::sync::Arc::as_ptr(&snapshot.railway_tracks) as usize;
    let key = (kind, tile_idx, tracks_ptr, snapshot.buildings.len());
    if state.cached_preview_key != Some(key) {
        state.station_tiles.clear();
        state.station_tiles.extend(
            snapshot
                .buildings
                .iter()
                .filter(|b| b.kind.is_enabled() && b.level > 0 && !b.under_construction)
                .map(|b| (b.id, b.tile_idx)),
        );
        state.cached_preview = preview_building_rail_connection(
            &state.track_index_by_tile,
            &state.station_tiles,
            &engine.state.map,
            kind,
            tile_idx,
            &mut state.preview_astar,
        );
        state.cached_ghost_points.clear();
        for path in &state.cached_preview.ghost_paths {
            let compressed = compress_path(path, sim.map_w);
            let pts: Vec<[f32; 2]> = compressed
                .iter()
                .map(|&tile| world_point(tile, sim.map_w))
                .collect();
            if pts.len() >= 2 {
                state.cached_ghost_points.push(pts);
            }
        }
        state.cached_preview_key = Some(key);
    }

    for seg_key in &state.cached_preview.snap_tracks {
        let Some(&seg_idx) = state.track_index_by_key.get(seg_key) else {
            continue;
        };
        let segment = &state.segments[seg_idx];
        for pair in segment.points.windows(2) {
            let a = input.world_to_screen(pair[0]);
            let b = input.world_to_screen(pair[1]);
            if route_in_view(a, a, b, viewport[0], viewport[1], width * 3.0) {
                draw_highlight_leg(
                    text,
                    a,
                    b,
                    (width * 2.8).max(4.5 * sf),
                    (width * 1.45).max(2.2 * sf),
                    [0.18, 0.88, 0.34, 0.34],
                    [0.24, 0.96, 0.42, 0.95],
                    false,
                    viewport,
                );
            }
        }
    }

    for pts in &state.cached_ghost_points {
        for pair in pts.windows(2) {
            let a = input.world_to_screen(pair[0]);
            let b = input.world_to_screen(pair[1]);
            if route_in_view(a, a, b, viewport[0], viewport[1], width * 3.0) {
                draw_highlight_leg(
                    text,
                    a,
                    b,
                    (width * 2.1).max(3.5 * sf),
                    (width * 1.2).max(1.8 * sf),
                    [0.25, 0.90, 0.55, 0.20],
                    [0.55, 0.98, 0.72, 0.78],
                    true,
                    viewport,
                );
            }
        }
        if let Some(&end_pt) = pts.last() {
            let end_screen = input.world_to_screen(end_pt);
            text.push_ring(
                end_screen,
                (width * 1.9).max(4.0 * sf),
                [0.24, 0.96, 0.45, 0.88],
                (1.8 * sf).max(1.0),
            );
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

/// Queues one shader-styled instance per leg of a track polyline.
fn draw_route(
    text: &mut TextRenderer,
    input: &InputState,
    points: &[[f32; 2]],
    half_width: f32,
    level: u8,
    opacity: f32,
    viewport: [f32; 2],
    seconds: f32,
) {
    if opacity <= 0.0 || points.len() < 2 {
        return;
    }
    let margin = half_width * 2.0 + 4.0;
    let mut along = 0.0;
    let mut prev = None;
    let mut a = input.world_to_screen(points[0]);
    let mut rest = points[1..]
        .iter()
        .map(|&point| input.world_to_screen(point))
        .peekable();
    while let Some(b) = rest.next() {
        if route_in_view(a, a, b, viewport[0], viewport[1], margin) {
            text.push_route(
                a,
                b,
                prev,
                rest.peek().copied(),
                along,
                half_width,
                level,
                opacity,
                seconds,
            );
        }
        along += (b[0] - a[0]).hypot(b[1] - a[1]);
        prev = Some(a);
        a = b;
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

fn train_position(
    points: &[[f32; 2]],
    length: f32,
    phase: f32,
    gap: f32,
) -> ([f32; 2], [f32; 2], bool) {
    let cycle = length * 2.0;
    if cycle <= 0.0 {
        return (*points.first().unwrap_or(&[0.0, 0.0]), [0.0, 0.0], false);
    }
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
        let length = polyline_length(&route);
        let (pos_out, dir_out, loaded_out) = train_position(&route, length, 5.0, 0.0);
        assert!(!loaded_out);
        assert_eq!(dir_out, [1.0, 0.0]);
        assert!((pos_out[0] - 5.5).abs() < 1e-4);

        let (pos_ret, dir_ret, loaded_ret) = train_position(&route, length, 25.0, 0.0);
        assert!(loaded_ret);
        assert_eq!(dir_ret, [0.0, -1.0]);
        assert!((pos_ret[1] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn assign_chunks_covers_long_axis_legs_in_o_chunks() {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let w = 256u32;
        // Horizontal leg across chunks 0, 1, 2 then vertical across chunks 0, 1.
        let path = [10, 140, 90 * w + 140];
        assign_chunks(&mut cells, 42, &path, w);
        for chunk in [(0, 0), (1, 0), (2, 0), (2, 1)] {
            assert_eq!(cells.get(&chunk).map(Vec::as_slice), Some(&[42][..]));
        }
        assert!(!cells.contains_key(&(0, 1)));
    }

    #[test]
    fn sync_links_only_preserves_track_cells_and_updates_levels() {
        let map_w = 128u32;
        let tracks = std::sync::Arc::new(vec![RailwayTrack {
            from_node_id: 1,
            to_node_id: 2,
            path: vec![2 * map_w + 2, 2 * map_w + 40],
        }]);
        let link1 = RailwayLink {
            from_building_id: 1,
            to_building_id: 2,
            from_tile: 2 * map_w + 2,
            owner_id: 1,
            level: 1,
            distance: 38,
            station_path: vec![1, 2],
            route_path: vec![2 * map_w + 2, 2 * map_w + 40],
            station_path_edge_distances: vec![38],
            station_stop_count: 1,
            period_ticks: 100,
            opened_at: 0,
        };
        let mut snap = SimSnapshot {
            tick: 1,
            phase: sow_core::game::GamePhase::Playing,
            spawn_timer_secs: None,
            players: Vec::new(),
            dirty_tiles: Vec::new(),
            fleets: Vec::new(),
            attacks: Vec::new(),
            buildings: Vec::new(),
            railway_links: std::sync::Arc::new(vec![link1.clone()]),
            railway_tracks: std::sync::Arc::clone(&tracks),
            projectiles: Vec::new(),
            nuke_alerts: Vec::new(),
            resource_transfers: Vec::new(),
            resource_rejections: Vec::new(),
            help_call_rejections: Vec::new(),
            resource_transfer_rejections: Vec::new(),
            winner: None,
            winning_team: None,
            defense_posts: Vec::new(),
            defense_dirty: false,
            total_land_tiles: 0,
            sea_lanes: std::sync::Arc::new(Vec::new()),
        };

        let mut render_state = RailwayRenderState::default();
        render_state.sync(&snap, map_w, false);
        assert_eq!(render_state.segments.len(), 1);
        assert_eq!(render_state.segments[0].level, 1);
        let orig_cells_len = render_state.cells.len();

        let mut upgraded = link1;
        upgraded.level = 7;
        snap.tick = 2;
        snap.railway_links = std::sync::Arc::new(vec![upgraded]);
        render_state.sync(&snap, map_w, false);
        assert_eq!(render_state.segments.len(), 1);
        assert_eq!(render_state.segments[0].level, 7);
        assert_eq!(render_state.cells.len(), orig_cells_len);
    }

    #[test]
    fn pick_hovered_rail_segment_hits_chunk_local_track_and_ignores_distant() {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let w = 128u32;
        let path = [10 * w + 5, 10 * w + 30];
        assign_chunks(&mut cells, 0, &path, w);
        let state = RailwayRenderState {
            cells,
            segments: vec![RailSegment {
                key: (1, 2),
                points: vec![[5.5, 10.5], [30.5, 10.5]],
                path_tiles: Vec::new(),
                length: 25.0,
                level: 1,
            }],
            ..Default::default()
        };
        assert_eq!(
            pick_hovered_rail_segment(&state, [15.0, 11.1], 1.25, None),
            Some(0)
        );
        assert_eq!(
            pick_hovered_rail_segment(&state, [15.0, 18.0], 1.25, None),
            None
        );
    }
}
