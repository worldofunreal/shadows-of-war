use crate::app::{InputState, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::game::BuildingKind;
use sow_core::map::MapTile;
use sow_core::protocol::{BuildingSnapshot, SimSnapshot};
use std::collections::{HashMap, HashSet};
use web_time::Instant;

const GRID_CELL_SIZE: i32 = 32;
const MAX_CANDIDATES_PER_BUILDING: usize = 64;
const MAX_RENDERED_TRAINS: usize = 60;
const TRACK_BED_COLOR: [f32; 3] = [0.12, 0.11, 0.10];
const TRACK_RAIL_COLOR: [f32; 3] = [0.63, 0.54, 0.40];

#[derive(Clone, Copy)]
struct RailBuilding {
    id: u64,
    tile_idx: u32,
    owner_id: u16,
    kind: BuildingKind,
    under_construction: bool,
}

struct RailSegment {
    a: [f32; 2],
    corner: [f32; 2],
    b: [f32; 2],
    path_tiles: Vec<u32>,
    owner_id: u16,
    length: f32,
}

struct VisualTrain {
    edge: usize,
    phase: f32,
}

#[derive(Default)]
pub(crate) struct RailwayRenderState {
    snapshot_tick: Option<u64>,
    link_distance_bits: Option<u32>,
    buildings: Vec<RailBuilding>,
    segments: Vec<RailSegment>,
    trains: Vec<VisualTrain>,
    train_count: usize,
    last_frame: Option<Instant>,
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
        terrain: Option<&[MapTile]>,
        link_distance: f32,
    ) {
        let distance_bits = link_distance.to_bits();
        if self.snapshot_tick == Some(snapshot.tick)
            && self.link_distance_bits == Some(distance_bits)
        {
            return;
        }
        self.snapshot_tick = Some(snapshot.tick);

        let buildings_match = self.buildings.len() == snapshot.buildings.len()
            && self
                .buildings
                .iter()
                .zip(&snapshot.buildings)
                .all(|(old, new)| {
                    old.id == new.id
                        && old.tile_idx == new.tile_idx
                        && old.owner_id == new.owner_id
                        && old.kind == new.kind
                        && old.under_construction == new.under_construction
                });
        if buildings_match && self.link_distance_bits == Some(distance_bits) {
            return;
        }

        self.buildings.clear();
        self.buildings
            .extend(snapshot.buildings.iter().map(|building| RailBuilding {
                id: building.id,
                tile_idx: building.tile_idx,
                owner_id: building.owner_id,
                kind: building.kind,
                under_construction: building.under_construction,
            }));
        self.link_distance_bits = Some(distance_bits);
        self.segments = build_segments(&snapshot.buildings, map_w, map_h, terrain, link_distance);
        self.trains.clear();
        self.train_count = 0;
    }

    fn advance_trains(&mut self, now: Instant, speed: f32, count: usize) {
        let dt = self
            .last_frame
            .map(|last| now.duration_since(last).as_secs_f32())
            .unwrap_or(0.0)
            .clamp(0.0, 0.1);
        self.last_frame = Some(now);

        let count = count.min(MAX_RENDERED_TRAINS).min(self.segments.len());
        if count != self.train_count {
            self.trains.clear();
            if count > 0 {
                for index in 0..count {
                    let edge = index * self.segments.len() / count;
                    let cycle = self.segments[edge].length * 2.0;
                    self.trains.push(VisualTrain {
                        edge,
                        phase: cycle * ((index as f32 * 0.618_034).fract()),
                    });
                }
            }
            self.train_count = count;
        }

        for train in &mut self.trains {
            let cycle = self.segments[train.edge].length * 2.0;
            if cycle > 0.0 {
                train.phase = (train.phase + speed * dt).rem_euclid(cycle);
            }
        }
    }
}

fn build_segments(
    buildings: &[BuildingSnapshot],
    map_w: u32,
    map_h: u32,
    terrain: Option<&[MapTile]>,
    max_distance: f32,
) -> Vec<RailSegment> {
    if map_w == 0 || map_h == 0 {
        return Vec::new();
    }

    let eligible: Vec<_> = buildings
        .iter()
        .filter(|building| {
            !building.under_construction
                && u64::from(building.tile_idx) < u64::from(map_w) * u64::from(map_h)
        })
        .copied()
        .collect();
    if eligible.len() < 2 {
        return Vec::new();
    }

    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (index, building) in eligible.iter().enumerate() {
        let x = (building.tile_idx % map_w) as i32;
        let y = (building.tile_idx / map_w) as i32;
        grid.entry((x / GRID_CELL_SIZE, y / GRID_CELL_SIZE))
            .or_default()
            .push(index);
    }

    let cell_radius = (max_distance / GRID_CELL_SIZE as f32).ceil() as i32;
    let mut offsets: Vec<_> = (-cell_radius..=cell_radius)
        .flat_map(|dy| (-cell_radius..=cell_radius).map(move |dx| (dx, dy)))
        .collect();
    offsets.sort_by_key(|(dx, dy)| dx * dx + dy * dy);

    let max_distance_sq = max_distance * max_distance;
    let mut edges = Vec::with_capacity(eligible.len());
    let mut seen = HashSet::with_capacity(eligible.len());
    for (index, building) in eligible.iter().enumerate() {
        let ax = (building.tile_idx % map_w) as i32;
        let ay = (building.tile_idx / map_w) as i32;
        let cell = (ax / GRID_CELL_SIZE, ay / GRID_CELL_SIZE);
        let mut best: Option<(usize, f32, bool)> = None;
        let mut checked = 0;

        'cells: for (dx, dy) in &offsets {
            let Some(candidates) = grid.get(&(cell.0 + dx, cell.1 + dy)) else {
                continue;
            };
            for &other_index in candidates {
                if other_index == index {
                    continue;
                }
                checked += 1;
                if checked > MAX_CANDIDATES_PER_BUILDING {
                    break 'cells;
                }

                let other = eligible[other_index];
                let bx = (other.tile_idx % map_w) as i32;
                let by = (other.tile_idx / map_w) as i32;
                let distance_sq = ((ax - bx) * (ax - bx) + (ay - by) * (ay - by)) as f32;
                if distance_sq > max_distance_sq
                    || best.is_some_and(|(_, best_distance, _)| distance_sq >= best_distance)
                {
                    continue;
                }

                let horizontal_first = if route_is_land(ax, ay, bx, by, true, map_w, map_h, terrain)
                {
                    true
                } else if route_is_land(ax, ay, bx, by, false, map_w, map_h, terrain) {
                    false
                } else {
                    continue;
                };
                best = Some((other_index, distance_sq, horizontal_first));
            }
        }

        let Some((other_index, _, horizontal_first)) = best else {
            continue;
        };
        let other = eligible[other_index];
        let pair = if building.id < other.id {
            (building.id, other.id)
        } else {
            (other.id, building.id)
        };
        if !seen.insert(pair) {
            continue;
        }

        let bx = (other.tile_idx % map_w) as i32;
        let by = (other.tile_idx / map_w) as i32;
        let a = crate::render::world::movers::tile_to_world(building.tile_idx, map_w);
        let b = crate::render::world::movers::tile_to_world(other.tile_idx, map_w);
        let corner = if horizontal_first {
            [b.0, a.1]
        } else {
            [a.0, b.1]
        };
        let mut path_tiles = Vec::new();
        if horizontal_first {
            append_axis_tiles(ax, ay, bx, ay, map_w, map_h, &mut path_tiles);
            append_axis_tiles(bx, ay, bx, by, map_w, map_h, &mut path_tiles);
        } else {
            append_axis_tiles(ax, ay, ax, by, map_w, map_h, &mut path_tiles);
            append_axis_tiles(ax, by, bx, by, map_w, map_h, &mut path_tiles);
        }
        let length =
            (corner[0] - a.0).hypot(corner[1] - a.1) + (b.0 - corner[0]).hypot(b.1 - corner[1]);
        if length <= 0.0 {
            continue;
        }
        edges.push(RailSegment {
            a: [a.0, a.1],
            corner,
            b: [b.0, b.1],
            path_tiles,
            owner_id: building.owner_id,
            length,
        });
    }
    edges
}

fn route_is_land(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    horizontal_first: bool,
    map_w: u32,
    map_h: u32,
    terrain: Option<&[MapTile]>,
) -> bool {
    if horizontal_first {
        axis_is_land(ax, ay, bx, ay, map_w, map_h, terrain)
            && axis_is_land(bx, ay, bx, by, map_w, map_h, terrain)
    } else {
        axis_is_land(ax, ay, ax, by, map_w, map_h, terrain)
            && axis_is_land(ax, by, bx, by, map_w, map_h, terrain)
    }
}

fn axis_is_land(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    map_w: u32,
    map_h: u32,
    terrain: Option<&[MapTile]>,
) -> bool {
    let (mut x, mut y) = (x0, y0);
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    loop {
        if x < 0 || y < 0 || x >= map_w as i32 || y >= map_h as i32 {
            return false;
        }
        let tile = y as u32 * map_w + x as u32;
        if terrain.is_some_and(|tiles| !tiles.get(tile as usize).is_some_and(MapTile::is_land)) {
            return false;
        }
        if x == x1 && y == y1 {
            return true;
        }
        x += sx;
        y += sy;
    }
}

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
    now: Instant,
) {
    let state = &mut ui.railway_render_state;
    let count = dev
        .train_max_count
        .round()
        .clamp(0.0, MAX_RENDERED_TRAINS as f32) as usize;
    let min_zoom = dev.railway_min_zoom.max(0.01);
    if !dev.vfx_railways || zoom_scaled < min_zoom || sim.map_w == 0 || sim.map_h == 0 {
        state.advance_trains(now, dev.train_speed.max(0.0), 0);
        return;
    }

    let terrain = sim
        .engine
        .as_ref()
        .map(|engine| engine.state.map.terrain.as_slice());
    state.sync(
        snapshot,
        sim.map_w,
        sim.map_h,
        terrain,
        dev.railway_link_distance.clamp(4.0, 150.0),
    );
    state.advance_trains(
        now,
        dev.train_speed.max(0.0),
        if dev.vfx_trains { count } else { 0 },
    );

    let fade = ((zoom_scaled - min_zoom) / min_zoom).clamp(0.25, 1.0);
    let opacity = dev.railway_opacity.clamp(0.0, 1.0) * fade;
    let line_width = dev.railway_track_width.clamp(0.5, 6.0) * sf.max(0.01);
    let screen_w = input.screen_w;
    let screen_h = input.screen_h;
    let camera = [input.camera_x, input.camera_y];
    for segment in &state.segments {
        if dev.fog_of_war
            && !segment
                .path_tiles
                .iter()
                .all(|&tile| sim.fog_visible.contains(tile))
        {
            continue;
        }
        let a = world_screen(segment.a, camera, input.camera_zoom);
        let corner = world_screen(segment.corner, camera, input.camera_zoom);
        let b = world_screen(segment.b, camera, input.camera_zoom);
        if !route_in_view(a, corner, b, screen_w, screen_h, line_width * 2.0) {
            continue;
        }
        draw_track_leg(text, a, corner, line_width, opacity);
        draw_track_leg(text, corner, b, line_width, opacity);
    }

    if !dev.vfx_trains {
        return;
    }
    let train_size = dev.train_size.clamp(3.0, 24.0) * sf.max(0.01);
    for train in &state.trains {
        let segment = &state.segments[train.edge];
        let a = world_screen(segment.a, camera, input.camera_zoom);
        let corner = world_screen(segment.corner, camera, input.camera_zoom);
        let b = world_screen(segment.b, camera, input.camera_zoom);
        if !route_in_view(a, corner, b, screen_w, screen_h, train_size * 2.0) {
            continue;
        }
        if dev.fog_of_war
            && !segment
                .path_tiles
                .iter()
                .all(|&tile| sim.fog_visible.contains(tile))
        {
            continue;
        }
        let color = snapshot
            .players
            .iter()
            .find(|player| player.id == segment.owner_id)
            .map(crate::render::world::nameplates::player_color)
            .unwrap_or([0.85, 0.8, 0.66, 1.0]);
        for gap in [0.0, train_size * 1.35, train_size * 2.7] {
            let (position, horizontal) =
                train_position(segment, train.phase, gap / input.camera_zoom.max(0.01));
            let center = world_screen(position, camera, input.camera_zoom);
            let size = if horizontal {
                [train_size * 1.2, train_size * 0.58]
            } else {
                [train_size * 0.58, train_size * 1.2]
            };
            let alpha = if gap == 0.0 { 1.0 } else { 0.8 };
            text.push_rounded_rect(
                center,
                size,
                0.25,
                [color[0], color[1], color[2], alpha],
                [0.08, 0.07, 0.06, 0.95],
                (sf * 0.7).max(0.5),
            );
        }
    }
}

fn world_screen(point: [f32; 2], camera: [f32; 2], zoom: f32) -> [f32; 2] {
    [camera[0] + point[0] * zoom, camera[1] + point[1] * zoom]
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

fn draw_track_leg(text: &mut TextRenderer, a: [f32; 2], b: [f32; 2], width: f32, opacity: f32) {
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
