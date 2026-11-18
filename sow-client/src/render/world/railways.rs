use crate::app::{InputState, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::game::BuildingKind;
use sow_core::protocol::{BuildingSnapshot, SimSnapshot};
use sow_core::building::RailwayLink;
use std::collections::HashMap;
use web_time::Instant;

const MAX_RENDERED_TRAINS: usize = 60;
const TRACK_BED_COLOR: [f32; 3] = [0.12, 0.11, 0.10];
const TRACK_RAIL_COLOR: [f32; 3] = [0.63, 0.54, 0.40];

#[derive(Clone, Copy)]
struct RailBuilding {
    id: u64,
    tile_idx: u32,
    owner_id: u16,
    kind: BuildingKind,
    level: u8,
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
    buildings: Vec<RailBuilding>,
    railway_links: Vec<RailwayLink>,
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
    ) {
        if self.snapshot_tick == Some(snapshot.tick) {
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
                        && old.level == new.level
                        && old.under_construction == new.under_construction
                });
        if buildings_match && self.railway_links == snapshot.railway_links {
            return;
        }

        self.buildings.clear();
        self.buildings
            .extend(snapshot.buildings.iter().map(|building| RailBuilding {
                id: building.id,
                tile_idx: building.tile_idx,
                owner_id: building.owner_id,
                kind: building.kind,
                level: building.level,
                under_construction: building.under_construction,
            }));
        self.railway_links.clone_from(&snapshot.railway_links);
        self.segments = build_segments(
            &snapshot.buildings,
            &snapshot.railway_links,
            map_w,
            map_h,
        );
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
    links: &[RailwayLink],
    map_w: u32,
    map_h: u32,
) -> Vec<RailSegment> {
    if map_w == 0 || map_h == 0 || links.is_empty() {
        return Vec::new();
    }
    let by_id: HashMap<_, _> = buildings.iter().map(|building| (building.id, building)).collect();
    let mut segments = Vec::with_capacity(links.len());
    for link in links {
        let (Some(a_building), Some(b_building)) = (
            by_id.get(&link.from_building_id),
            by_id.get(&link.to_building_id),
        ) else {
            continue;
        };
        let (a_building, b_building) = (*a_building, *b_building);
        if u64::from(a_building.tile_idx) >= u64::from(map_w) * u64::from(map_h)
            || u64::from(b_building.tile_idx) >= u64::from(map_w) * u64::from(map_h)
        {
            continue;
        }
        let ax = (a_building.tile_idx % map_w) as i32;
        let ay = (a_building.tile_idx / map_w) as i32;
        let bx = (b_building.tile_idx % map_w) as i32;
        let by = (b_building.tile_idx / map_w) as i32;
        let a = crate::render::world::movers::tile_to_world(a_building.tile_idx, map_w);
        let b = crate::render::world::movers::tile_to_world(b_building.tile_idx, map_w);
        let corner = if link.horizontal_first { [b.0, a.1] } else { [a.0, b.1] };
        let mut path_tiles = Vec::new();
        if link.horizontal_first {
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
        segments.push(RailSegment {
            a: [a.0, a.1],
            corner,
            b: [b.0, b.1],
            path_tiles,
            owner_id: if a_building.kind == BuildingKind::Factory {
                a_building.owner_id
            } else {
                b_building.owner_id
            },
            length,
        });
    }
    segments
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

    state.sync(snapshot, sim.map_w, sim.map_h);
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
        let a = world_screen(segment.a, camera, input.camera_zoom);
        let corner = world_screen(segment.corner, camera, input.camera_zoom);
        let b = world_screen(segment.b, camera, input.camera_zoom);
        if !route_in_view(a, corner, b, screen_w, screen_h, line_width * 2.0) {
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
