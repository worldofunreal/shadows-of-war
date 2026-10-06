use super::feedback;
use crate::app::{InputState, MapContextMenuView, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::game::BuildingKind;
use sow_core::player::Leader;
use sow_core::protocol::SimSnapshot;
use std::collections::HashMap;
use web_time::{Duration, Instant};

pub(crate) const INLINE_EMOJI_SCALE: f32 = 1.4;

const BUILDING_CULL_FLOOR: f32 = 0.25;
const BUILDING_FULL_DETAIL_SIZE: f32 = 28.0;
const BUILDING_MIN_MARKER_SIZE: f32 = 14.0;
const BUILDING_CLUSTER_TARGET_SIZE: f32 = 40.0;
const BUILDING_ZOOM_STEPS_PER_OCTAVE: f32 = 8.0;
const BUILDING_FOOTPRINT_FILL: f32 = 0.9;
const BUILDING_LEVEL_FONT_RATIO: f32 = 0.58;
const BUILDING_CELEBRATION_OFFSETS: [[f32; 2]; 4] =
    [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

#[inline]
pub(crate) fn world_to_screen(world_x: f32, world_y: f32, input: &InputState, sf: f32) -> [f32; 2] {
    world_to_screen_values(
        world_x,
        world_y,
        input.camera_x,
        input.camera_y,
        input.camera_zoom,
        sf,
    )
}

#[inline]
pub(super) fn world_to_screen_values(
    world_x: f32,
    world_y: f32,
    camera_x: f32,
    camera_y: f32,
    camera_zoom: f32,
    sf: f32,
) -> [f32; 2] {
    let sf = sf.max(0.01);
    [
        (camera_x + world_x * camera_zoom) / sf,
        (camera_y + world_y * camera_zoom) / sf,
    ]
}

pub(crate) fn render_overlays(
    text: &mut TextRenderer,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    map_renderer: Option<&crate::render::gpu::MapRenderer>,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    sf: f32,
    time_secs: f32,
    now: Instant,
) {
    let Some(snapshot) = sim.current_snapshot.as_ref() else {
        return;
    };
    let sf = sf.max(0.01);
    let dev = DevConfig::get();
    let zoom_scaled = input.camera_zoom / sf;

    render_building_selection_focus(text, sim, input, sf);
    if dev.vfx_world_buildings {
        render_buildings(text, snapshot, sim, ui, input, &dev, sf, zoom_scaled, now);
    }
    render_building_placement_preview(
        text,
        snapshot,
        sim,
        ui,
        input,
        map_renderer,
        &dev,
        sf,
        time_secs,
        now,
    );
    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    let leaderboard_top_three = ui.leaderboard_top_three;
    super::nameplates::render_nameplates(
        text,
        snapshot,
        sim,
        &mut ui.nameplates,
        input,
        &dev,
        campaign_avatar_slots,
        sf,
        zoom_scaled,
        now,
        my_id,
        leaderboard_top_three,
        ui.tutorial_active
            .then_some(ui.tutorial_marker_player_id)
            .flatten(),
    );
    feedback::render(text, snapshot, sim, ui, input, &dev, sf, now);

    if !ui.tutorial_active {
        ui.tutorial_marker_player_id = None;
        ui.dialog_border_highlight = None;
    }
}

pub(super) fn avatar_slot(leader: Option<Leader>) -> usize {
    match leader {
        Some(leader) => Leader::ALL
            .iter()
            .position(|value| *value == leader)
            .unwrap_or(0),
        None => Leader::ALL.len(),
    }
}

#[derive(Clone, Copy)]
struct BuildingLod {
    band: i16,
    quantized_zoom: f32,
    cluster_cell_size: f32,
}

impl BuildingLod {
    fn for_zoom(zoom_scaled: f32) -> Self {
        let zoom = zoom_scaled.max(BUILDING_CULL_FLOOR);
        let band = (zoom.log2() * BUILDING_ZOOM_STEPS_PER_OCTAVE).floor() as i16;
        let quantized_zoom = 2.0_f32.powf(band as f32 / BUILDING_ZOOM_STEPS_PER_OCTAVE);
        Self {
            band,
            quantized_zoom,
            cluster_cell_size: (BUILDING_CLUSTER_TARGET_SIZE / quantized_zoom).max(1.0),
        }
    }

    fn detail(self, kind: BuildingKind) -> BuildingDetail {
        let (width, height) = kind.footprint_dimensions();
        let projected_short_side = width.min(height) as f32 * self.quantized_zoom;
        if projected_short_side < BUILDING_MIN_MARKER_SIZE {
            BuildingDetail::Cluster
        } else if projected_short_side < BUILDING_FULL_DETAIL_SIZE {
            BuildingDetail::Compact
        } else {
            BuildingDetail::Full
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuildingDetail {
    Full,
    Compact,
    Cluster,
}

#[derive(Clone, Debug, PartialEq)]
struct BuildingVisualStatus {
    progress: f32,
    label: Option<String>,
    color: [f32; 4],
}

#[derive(Clone)]
struct RenderedBuilding {
    bx: f32,
    by: f32,
    kind: BuildingKind,
    level: u8,
    under_construction: bool,
    count: usize,
    owner_id: u16,
    tile_idx: Option<u32>,
    detail: BuildingDetail,
    status: Option<BuildingVisualStatus>,
}

#[derive(Hash, PartialEq, Eq)]
struct BuildingClusterKey {
    grid_x: i32,
    grid_y: i32,
    owner_id: u16,
    kind: BuildingKind,
    level: u8,
}

#[derive(Default)]
pub(crate) struct BuildingRenderCache {
    tick: Option<u64>,
    map_w: u32,
    lod_band: i16,
    player_id: u16,
    tick_rate_ms_bits: u32,
    buildings: Vec<RenderedBuilding>,
    clusters: HashMap<BuildingClusterKey, (f32, f32, usize)>,
}

impl BuildingRenderCache {
    #[inline]
    fn matches(
        &self,
        tick: u64,
        map_w: u32,
        lod_band: i16,
        player_id: u16,
        tick_rate_ms_bits: u32,
    ) -> bool {
        self.tick == Some(tick)
            && self.map_w == map_w
            && self.lod_band == lod_band
            && self.player_id == player_id
            && self.tick_rate_ms_bits == tick_rate_ms_bits
    }
}

fn format_construction_time(ticks: u32, tick_rate_ms: f32) -> String {
    let tick_rate_ms = if tick_rate_ms.is_finite() && tick_rate_ms > 0.0 {
        tick_rate_ms
    } else {
        100.0
    };
    let tenths = ((ticks as f32 * tick_rate_ms / 100.0).ceil() as u32).max(0);
    if tenths < 600 {
        let seconds = tenths / 10;
        let fraction = tenths % 10;
        if fraction == 0 {
            format!("{}s", seconds)
        } else {
            format!("{}.{}s", seconds, fraction)
        }
    } else {
        let total_seconds = (tenths + 9) / 10;
        format!("{}m {:02}s", total_seconds / 60, total_seconds % 60)
    }
}

fn construction_progress(
    kind: BuildingKind,
    active_level: u8,
    target_level: u8,
    ticks_until_complete: u32,
) -> Option<f32> {
    if ticks_until_complete == 0 {
        return None;
    }

    if active_level == 0 {
        let total_ticks = kind.construction_duration_ticks();
        return (total_ticks > 0)
            .then(|| 1.0 - (ticks_until_complete as f32 / total_ticks as f32).clamp(0.0, 1.0));
    }

    if target_level <= active_level {
        return None;
    }

    let duration_current =
        sow_core::building::core::upgrade_duration_ticks(kind, active_level.saturating_add(1));
    Some(1.0 - (ticks_until_complete as f32 / duration_current as f32).clamp(0.0, 1.0))
}

fn construction_status_label(
    target_level: Option<u8>,
    ticks_remaining: u32,
    tick_rate_ms: f32,
) -> String {
    let time = format_construction_time(ticks_remaining, tick_rate_ms);
    target_level.map_or_else(
        || format!("🏗️ {time}"),
        |level| format!("🏗️ {level} · {time}"),
    )
}

fn building_visual_status(
    building: &sow_core::protocol::BuildingSnapshot,
    active_level: u8,
    my_id: u16,
    tick_rate_ms: f32,
    show_label: bool,
) -> Option<BuildingVisualStatus> {
    if building.owner_id != my_id || !building.under_construction {
        return None;
    }
    let progress = construction_progress(
        building.kind,
        active_level,
        building.level,
        building.ticks_until_complete,
    )?;
    Some(BuildingVisualStatus {
        progress,
        label: show_label.then(|| {
            construction_status_label(
                (active_level > 0).then_some(building.level),
                building.ticks_until_complete,
                tick_rate_ms,
            )
        }),
        color: if active_level == 0 {
            [0.0, 0.86, 1.0, 1.0]
        } else {
            [1.0, 0.82, 0.22, 1.0]
        },
    })
}

fn render_buildings(
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
    if zoom_scaled < BUILDING_CULL_FLOOR {
        return;
    }
    let lod = BuildingLod::for_zoom(zoom_scaled);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let my_id = sim.my_player_id.unwrap_or(0);
    for building in &snapshot.buildings {
        let active_level = building.active_level();
        if let Some(previous) = ui
            .building_levels_seen
            .insert(building.tile_idx, active_level)
            && active_level > previous
        {
            ui.building_upgrade_flashes.insert(building.tile_idx, now);
        }
    }
    ui.building_upgrade_flashes
        .retain(|_, started| now.duration_since(*started) < Duration::from_millis(300));
    let building_upgrade_flashes = std::mem::take(&mut ui.building_upgrade_flashes);
    let reduced_motion = ui.app.settings_state.reduced_motion;
    let buildings = cached_buildings(ui, snapshot, sim.map_w, lod, my_id, sim.config.tick_rate_ms);
    let text_style = crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let mut celebration_budget = 24usize;

    for building in buildings {
        if dev.fog_of_war
            && building.owner_id != my_id
            && building
                .tile_idx
                .or_else(|| tile_at_world(building.bx, building.by, sim.map_w))
                .is_some_and(|tile| !sim.fog_visible.contains(tile))
        {
            continue;
        }

        let center = world_to_screen(building.bx, building.by, input, sf);
        let marker_size = building_marker_size(building, zoom_scaled);
        let half_width = marker_size[0] * 0.5;
        let half_height = marker_size[1] * 0.5;
        if center[0] + half_width < 0.0
            || center[0] - half_width > screen_w
            || center[1] + half_height < 0.0
            || center[1] - half_height > screen_h
        {
            continue;
        }
        let marker_extent = marker_size[0].max(marker_size[1]);

        if let Some(status) = &building.status {
            let center_px = [center[0] * sf, center[1] * sf];
            let radius = marker_extent * sf * 0.58;
            text.push_ring(
                center_px,
                radius,
                [0.0, 0.0, 0.0, 0.65],
                (2.0 * sf).max(1.0),
            );
            if status.progress > 0.0 {
                text.push_arc(
                    center_px,
                    radius,
                    status.progress,
                    status.color,
                    (2.5 * sf).max(1.0),
                );
            }
        }

        let alpha = if building.under_construction {
            0.5
        } else {
            1.0
        };
        let building_center = [center[0] * sf, center[1] * sf];
        let building_size = [
            marker_size[0] * sf * BUILDING_FOOTPRINT_FILL,
            marker_size[1] * sf * BUILDING_FOOTPRINT_FILL,
        ];
        let building_art = if building.detail == BuildingDetail::Full {
            crate::render::gpu::building_sprite_uv(building.kind, building.level.max(1))
        } else {
            None
        };
        if let Some(uv_rect) = building_art {
            text.push_building_sprite(
                building_center,
                building_size,
                uv_rect,
                [1.0, 1.0, 1.0, alpha],
            );
        } else {
            let icon_half_size =
                marker_size[0].min(marker_size[1]) * sf * BUILDING_FOOTPRINT_FILL * 0.5;
            let _ = text.push_emoji(
                building_kind_emoji(building.kind, building.level.max(1)),
                building_center,
                icon_half_size,
                [1.0, 1.0, 1.0, alpha],
                crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha]),
            );
        }

        if building.detail == BuildingDetail::Full
            && let Some(tile_idx) = building.tile_idx
            && let Some(started) = building_upgrade_flashes.get(&tile_idx)
        {
            let t = (now.duration_since(*started).as_secs_f32() / 0.3).clamp(0.0, 1.0);
            let alpha = 1.0 - t;
            let _ = text.push_emoji(
                "✨",
                [center[0] * sf, center[1] * sf],
                marker_extent * sf * (0.5 + 0.35 * t),
                [1.0, 0.88, 0.46, alpha],
                crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha * 0.7]),
            );
            if building.owner_id == my_id
                && !reduced_motion
                && celebration_budget >= BUILDING_CELEBRATION_OFFSETS.len()
            {
                let radius = marker_extent * sf * (0.3 + 0.55 * t);
                let sparkle_size = (marker_extent * sf * 0.18).clamp(4.0 * sf, 14.0 * sf);
                for offset in BUILDING_CELEBRATION_OFFSETS {
                    let _ = text.push_emoji(
                        "✨",
                        [
                            center[0] * sf + offset[0] * radius,
                            center[1] * sf + offset[1] * radius,
                        ],
                        sparkle_size,
                        [1.0, 0.84, 0.36, alpha * 0.9],
                        crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha * 0.6]),
                    );
                }
                celebration_budget -= BUILDING_CELEBRATION_OFFSETS.len();
            }
        }

        if (building.detail == BuildingDetail::Full || building.count > 1)
            && let Some(label) = building_badge_label(building)
        {
            let level_font_size = (marker_extent * BUILDING_LEVEL_FONT_RATIO)
                .clamp(8.0, 18.0)
                .round()
                * dev.font_size_scale.max(0.1);
            let label_center = [
                center[0] + half_width * 0.78,
                center[1] - half_height * 0.78,
            ];
            text.push_string(
                &label,
                [
                    label_center[0] * sf,
                    (label_center[1] + level_font_size * 0.25) * sf,
                ],
                level_font_size * sf,
                [1.0; 4],
                text_style,
                (0.5, dev.font_char_spacing.max(0.1), INLINE_EMOJI_SCALE),
            );
        }

        if building.detail == BuildingDetail::Full
            && let Some(status) = &building.status
            && let Some(label) = &status.label
        {
            render_building_preview_badge(
                text,
                [center[0] * sf, center[1] * sf],
                label,
                status.color,
                dev,
                sf,
            );
        }
    }
    ui.building_upgrade_flashes = building_upgrade_flashes;
}

pub(crate) fn building_at_pointer(
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    x: f64,
    y: f64,
) -> Option<u32> {
    let my_id = sim.my_player_id.unwrap_or(0);
    let sf = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    let zoom_scaled = input.camera_zoom / sf;
    if my_id == 0
        || sim.map_w == 0
        || !input.camera_zoom.is_finite()
        || input.camera_zoom <= 0.0
        || zoom_scaled < BUILDING_CULL_FLOOR
    {
        return None;
    }

    let lod = BuildingLod::for_zoom(zoom_scaled);
    let buildings = cached_buildings(ui, snapshot, sim.map_w, lod, my_id, sim.config.tick_rate_ms);
    let pointer_x = x as f32 / sf;
    let pointer_y = y as f32 / sf;
    let pointer_world_x = (x as f32 - input.camera_x) / input.camera_zoom;
    let pointer_world_y = (y as f32 - input.camera_y) / input.camera_zoom;
    let mut closest: Option<(f32, u32)> = None;

    for building in buildings {
        if building.owner_id != my_id {
            continue;
        }
        let center = world_to_screen(building.bx, building.by, input, sf);
        let dx = pointer_x - center[0];
        let dy = pointer_y - center[1];
        let distance_sq = dx * dx + dy * dy;
        if building.tile_idx.is_some() {
            let (width, height) = building.kind.footprint_dimensions();
            if (pointer_world_x - building.bx).abs() > width as f32 * 0.5
                || (pointer_world_y - building.by).abs() > height as f32 * 0.5
            {
                continue;
            }
        } else {
            let size = building_marker_size(building, zoom_scaled)[0];
            let hit_radius = (size * 0.5).max(12.0);
            if distance_sq > hit_radius * hit_radius {
                continue;
            }
        }

        let Some(tile_idx) = building.tile_idx.or_else(|| {
            nearest_building_in_cluster(
                snapshot,
                sim.map_w,
                lod.cluster_cell_size,
                building,
                pointer_world_x,
                pointer_world_y,
            )
        }) else {
            continue;
        };
        if closest.is_none_or(|(best_distance, best_tile)| {
            distance_sq < best_distance || (distance_sq == best_distance && tile_idx < best_tile)
        }) {
            closest = Some((distance_sq, tile_idx));
        }
    }
    closest.map(|(_, tile_idx)| tile_idx)
}

fn nearest_building_in_cluster(
    snapshot: &SimSnapshot,
    map_w: u32,
    cluster_cell_size: f32,
    marker: &RenderedBuilding,
    pointer_x: f32,
    pointer_y: f32,
) -> Option<u32> {
    let grid_x = (marker.bx / cluster_cell_size) as i32;
    let grid_y = (marker.by / cluster_cell_size) as i32;
    let mut closest: Option<(f32, u32)> = None;

    for building in &snapshot.buildings {
        if building.owner_id != marker.owner_id
            || building.kind != marker.kind
            || building.active_level() != marker.level
        {
            continue;
        }
        let tile_x = (building.tile_idx % map_w) as f32;
        let tile_y = (building.tile_idx / map_w) as f32;
        if (tile_x / cluster_cell_size) as i32 != grid_x
            || (tile_y / cluster_cell_size) as i32 != grid_y
        {
            continue;
        }
        let (world_x, world_y) =
            crate::render::world::movers::tile_to_world(building.tile_idx, map_w);
        let dx = pointer_x - world_x;
        let dy = pointer_y - world_y;
        let distance_sq = dx * dx + dy * dy;
        if closest.is_none_or(|(best_distance, best_tile)| {
            distance_sq < best_distance
                || (distance_sq == best_distance && building.tile_idx < best_tile)
        }) {
            closest = Some((distance_sq, building.tile_idx));
        }
    }
    closest.map(|(_, tile_idx)| tile_idx)
}

fn building_marker_size(building: &RenderedBuilding, zoom_scaled: f32) -> [f32; 2] {
    if building.tile_idx.is_some() {
        let (width, height) = building.kind.footprint_dimensions();
        [
            width as f32 * zoom_scaled * BUILDING_FOOTPRINT_FILL,
            height as f32 * zoom_scaled * BUILDING_FOOTPRINT_FILL,
        ]
    } else {
        let size = (building_icon_size(zoom_scaled) * if building.count > 1 { 0.6 } else { 0.5 })
            .max(BUILDING_MIN_MARKER_SIZE);
        [size; 2]
    }
}

fn render_building_selection_focus(
    text: &mut TextRenderer,
    sim: &SimState,
    input: &InputState,
    sf: f32,
) {
    let Some(menu) = input
        .map_context_menu
        .filter(|menu| menu.view == MapContextMenuView::BuildingDetails)
    else {
        return;
    };
    if sim.map_w == 0
        || sim.map_h == 0
        || !input.camera_zoom.is_finite()
        || input.camera_zoom <= 0.0
    {
        return;
    }
    let Some(kind) = menu.building_kind else {
        return;
    };

    let zoom = input.camera_zoom;
    let tile_x = menu.tile_idx % sim.map_w;
    let tile_y = menu.tile_idx / sim.map_w;
    let footprint = sow_core::building::BuildingFootprint::at(kind, tile_x, tile_y);
    let left = input.camera_x + footprint.left as f32 * zoom;
    let top = input.camera_y + footprint.top as f32 * zoom;
    let width = footprint.width as f32 * zoom;
    let height = footprint.height as f32 * zoom;
    let line = (1.5 * sf).min((zoom * 0.2).max(1.0)).max(1.0);
    let selected_center = [left + width * 0.5, top + height * 0.5];
    let anchor_center = [
        input.camera_x + (tile_x as f32 + 0.5) * zoom,
        input.camera_y + (tile_y as f32 + 0.5) * zoom,
    ];

    if zoom >= 4.0 * sf {
        text.push_rect([left, top], [width, height], [0.84, 0.68, 0.38, 0.045]);
        for (x, y, w, h) in [
            (left, top, width, line * 1.6),
            (left, top + height - line * 1.6, width, line * 1.6),
            (left, top, line * 1.6, height),
            (left + width - line * 1.6, top, line * 1.6, height),
        ] {
            text.push_rect([x, y], [w, h], [0.42, 0.91, 0.94, 0.88]);
        }
    } else {
        text.push_ring(
            selected_center,
            (width.max(height) * 0.5).max(9.0 * sf),
            [0.42, 0.91, 0.94, 0.85],
            (1.8 * sf).max(1.0),
        );
    }

    if kind == BuildingKind::Bunker && menu.building_level > 0 && !menu.building_under_construction
    {
        let range = (sim.config.bunker_range.round() as u32
            + u32::from(menu.building_level.saturating_sub(1)) * 2)
            .min(20);
        text.push_ring(
            anchor_center,
            range as f32 * zoom,
            [0.91, 0.71, 0.34, 0.30],
            (1.4 * sf).max(1.0),
        );
    }
}

fn render_building_placement_preview(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    map_renderer: Option<&crate::render::gpu::MapRenderer>,
    dev: &DevConfig,
    sf: f32,
    time_secs: f32,
    now: Instant,
) {
    let Some(kind) = ui.app.hud_state.selected_building_kind else {
        return;
    };
    let Some(map_renderer) = map_renderer else {
        return;
    };
    if !input.camera_zoom.is_finite() || input.camera_zoom <= 0.0 {
        return;
    }

    let world_x = (input.last_mouse_x as f32 - input.camera_x) / input.camera_zoom;
    let world_y = (input.last_mouse_y as f32 - input.camera_y) / input.camera_zoom;
    let (col, row) = crate::render::world::movers::world_to_tile(world_x, world_y);
    if col < 0 || row < 0 || col >= sim.map_w as i32 || row >= sim.map_h as i32 {
        return;
    }
    let hovered_tile = (row as u32) * sim.map_w + col as u32;
    let my_id = sim.my_player_id.unwrap_or(0);
    let target = ui.building_placement_cache.resolve(
        snapshot.tick,
        &crate::input::placement::PlacementQuery {
            kind,
            click_x: col,
            click_y: row,
            map_w: sim.map_w,
            map_h: sim.map_h,
            owners: &map_renderer.owners,
            terrain: &map_renderer.terrain,
            my_id,
            buildings: &snapshot.buildings,
        },
    );
    let preview_tile = target.unwrap_or(hovered_tile);
    let cost_index = sow_core::game::BuildingKind::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(0);
    let cost = ui.app.hud_state.building_costs[cost_index];
    let has_gold = ui.app.hud_state.gold >= cost;
    let can_place = target.is_ok() && has_gold;
    let footprint = sow_core::building::BuildingFootprint::at(
        kind,
        preview_tile % sim.map_w,
        preview_tile / sim.map_w,
    );
    let preview_x = footprint.left as f32 + footprint.width as f32 * 0.5;
    let preview_y = footprint.top as f32 + footprint.height as f32 * 0.5;
    let center = world_to_screen_values(
        preview_x,
        preview_y,
        input.camera_x,
        input.camera_y,
        input.camera_zoom,
        sf,
    );
    let center_px = [center[0] * sf, center[1] * sf];
    let footprint_size_px = [
        footprint.width as f32 * input.camera_zoom,
        footprint.height as f32 * input.camera_zoom,
    ];
    let footprint_top_left = [
        input.camera_x + footprint.left as f32 * input.camera_zoom,
        input.camera_y + footprint.top as f32 * input.camera_zoom,
    ];
    let color = if can_place {
        [0.13, 0.83, 0.94, 1.0]
    } else {
        [0.94, 0.27, 0.27, 1.0]
    };
    text.push_rect(
        footprint_top_left,
        footprint_size_px,
        [color[0], color[1], color[2], 0.16],
    );
    let grid_line = (1.0 * sf).max(1.0);
    for col in 0..=footprint.width {
        let x = footprint_top_left[0] + col as f32 * input.camera_zoom;
        text.push_rect(
            [x - grid_line * 0.5, footprint_top_left[1]],
            [grid_line, footprint_size_px[1]],
            [color[0], color[1], color[2], 0.58],
        );
    }
    for row in 0..=footprint.height {
        let y = footprint_top_left[1] + row as f32 * input.camera_zoom;
        text.push_rect(
            [footprint_top_left[0], y - grid_line * 0.5],
            [footprint_size_px[0], grid_line],
            [color[0], color[1], color[2], 0.58],
        );
    }
    if input.hold_build_active
        && let Some(start) = input.map_pointer_start.as_ref()
    {
        let held_secs = now.duration_since(start.started_at).as_secs_f32();
        let interval = crate::input::window::hold_build_repeat_interval(held_secs);
        let progress = (1.0 - input.hold_build_accum / interval).clamp(0.0, 1.0);
        let burst = held_secs >= crate::input::window::HOLD_BUILD_BURST_AFTER_SECS;
        let color = if burst {
            [1.0, 0.62, 0.16, 1.0]
        } else {
            [0.13, 0.83, 0.94, 1.0]
        };
        let pulse_rate = if burst { 28.0 } else { 8.0 };
        let pulse = (time_secs * pulse_rate).sin().max(0.0);
        let radius =
            footprint_size_px[0].max(footprint_size_px[1]) * 0.5 + (3.0 + pulse * 2.0) * sf;
        text.push_ring(
            center_px,
            radius,
            [color[0], color[1], color[2], 0.32],
            (2.0 * sf).max(1.0),
        );
        text.push_arc(center_px, radius, progress, color, (3.0 * sf).max(1.0));
    }

    if let Some(uv_rect) = crate::render::gpu::building_sprite_uv(kind, 1) {
        text.push_building_sprite(
            center_px,
            [
                footprint_size_px[0] * BUILDING_FOOTPRINT_FILL,
                footprint_size_px[1] * BUILDING_FOOTPRINT_FILL,
            ],
            uv_rect,
            [1.0, 1.0, 1.0, if can_place { 0.78 } else { 0.42 }],
        );
    } else {
        let icon_half_size =
            footprint_size_px[0].min(footprint_size_px[1]) * BUILDING_FOOTPRINT_FILL * 0.5;
        let _ = text.push_emoji(
            building_kind_emoji(kind, 1),
            center_px,
            icon_half_size,
            [1.0, 1.0, 1.0, if can_place { 0.78 } else { 0.42 }],
            crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.75]),
        );
    }
    let label = construction_status_label(
        None,
        kind.construction_duration_ticks(),
        sim.config.tick_rate_ms,
    );
    render_building_preview_badge(text, center_px, &label, color, dev, sf);

    if kind == BuildingKind::Bunker {
        let (anchor_x, anchor_y) =
            crate::render::world::movers::tile_to_world(preview_tile, sim.map_w);
        let anchor_center = world_to_screen_values(
            anchor_x,
            anchor_y,
            input.camera_x,
            input.camera_y,
            input.camera_zoom,
            sf,
        );
        text.push_ring(
            [anchor_center[0] * sf, anchor_center[1] * sf],
            sim.config.bunker_range as f32 * input.camera_zoom,
            [0.94, 0.27, 0.27, 0.38],
            (1.5 * sf).max(1.0),
        );
    }

    let balance = if has_gold {
        crate::utils::format_number(ui.app.hud_state.gold - cost)
    } else {
        format!(
            "-{}",
            crate::utils::format_number(cost - ui.app.hud_state.gold)
        )
    };
    render_building_gold_badge(
        text,
        [
            center_px[0],
            center_px[1] + footprint_size_px[1] * 0.5 + 18.0 * sf,
        ],
        &balance,
        has_gold,
        dev,
        sf,
    );
}

fn building_badge_label(building: &RenderedBuilding) -> Option<String> {
    if building.under_construction {
        return Some("🔨".to_string());
    }
    if building.level == 0 {
        return Some(if building.count > 1 {
            format!("🔨 × {}", building.count)
        } else {
            "🔨".to_string()
        });
    }
    if building.level == 1 && building.count == 1 {
        return None;
    }
    Some(if building.count > 1 {
        format!("{} × {}", building.level, building.count)
    } else {
        building.level.to_string()
    })
}

fn render_building_preview_badge(
    text: &mut TextRenderer,
    center: [f32; 2],
    label: &str,
    color: [f32; 4],
    dev: &DevConfig,
    sf: f32,
) {
    let font_size = 14.0 * dev.font_size_scale.max(0.1) * sf;
    let measure = text.measure_string(
        label,
        font_size,
        dev.font_char_spacing.max(0.1),
        INLINE_EMOJI_SCALE,
    );
    let padding = 8.0 * sf;
    let width = measure.width + padding * 2.0;
    let height = (measure.height + padding).max(22.0 * sf);
    let top = center[1] - height - 8.0 * sf;
    text.push_rect(
        [center[0] - width * 0.5, top],
        [width, height],
        [color[0], color[1], color[2], 0.8],
    );
    text.push_rect(
        [center[0] - width * 0.5 + sf, top + sf],
        [(width - 2.0 * sf).max(1.0), (height - 2.0 * sf).max(1.0)],
        [0.06, 0.09, 0.16, 0.92],
    );
    text.push_string(
        label,
        [center[0], top + height * 0.5 + font_size * 0.3],
        font_size,
        [1.0; 4],
        crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]),
        (0.5, dev.font_char_spacing.max(0.1), INLINE_EMOJI_SCALE),
    );
}

fn render_building_gold_badge(
    text: &mut TextRenderer,
    center: [f32; 2],
    balance: &str,
    positive: bool,
    dev: &DevConfig,
    sf: f32,
) {
    let font_size = 13.0 * dev.font_size_scale.max(0.1) * sf;
    let measure = text.measure_string(balance, font_size, dev.font_char_spacing.max(0.1), 1.0);
    let icon_size = font_size;
    let gap = 4.0 * sf;
    let width = icon_size + gap + measure.width + 12.0 * sf;
    let height = (measure.height.max(icon_size) + 8.0 * sf).max(20.0 * sf);
    let top = center[1] - height * 0.5;
    let left_edge = center[0] - width * 0.5;
    let border = if positive {
        [0.29, 0.87, 0.49, 0.8]
    } else {
        [0.97, 0.44, 0.44, 0.8]
    };
    text.push_rect([left_edge, top], [width, height], border);
    text.push_rect(
        [left_edge + sf, top + sf],
        [(width - 2.0 * sf).max(1.0), (height - 2.0 * sf).max(1.0)],
        [0.06, 0.09, 0.16, 0.92],
    );
    let left = center[0] - width * 0.5 + 6.0 * sf;
    let _ = text.push_emoji(
        "🪙",
        [left + icon_size * 0.5, center[1]],
        icon_size * 0.5,
        [1.0; 4],
        crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.75]),
    );
    text.push_string(
        balance,
        [left + icon_size + gap, center[1] + font_size * 0.3],
        font_size,
        if positive {
            [0.29, 0.87, 0.49, 1.0]
        } else {
            [0.97, 0.44, 0.44, 1.0]
        },
        crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]),
        (0.0, dev.font_char_spacing.max(0.1), INLINE_EMOJI_SCALE),
    );
}

fn cached_buildings<'a>(
    ui: &'a mut UiState,
    snapshot: &SimSnapshot,
    map_w: u32,
    lod: BuildingLod,
    my_id: u16,
    tick_rate_ms: f32,
) -> &'a [RenderedBuilding] {
    let map_w = map_w.max(1);
    let tick_rate_ms_bits = tick_rate_ms.to_bits();
    let cache = &mut ui.building_render_cache;
    if !cache.matches(snapshot.tick, map_w, lod.band, my_id, tick_rate_ms_bits) {
        collect_buildings(
            snapshot,
            map_w,
            lod,
            my_id,
            tick_rate_ms,
            &mut cache.buildings,
            &mut cache.clusters,
        );
        cache.buildings.sort_unstable_by(|a, b| {
            a.by.partial_cmp(&b.by)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.bx.partial_cmp(&b.bx).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.count.cmp(&b.count))
        });
        cache.tick = Some(snapshot.tick);
        cache.map_w = map_w;
        cache.lod_band = lod.band;
        cache.player_id = my_id;
        cache.tick_rate_ms_bits = tick_rate_ms_bits;
    }
    &cache.buildings
}

fn collect_buildings(
    snapshot: &SimSnapshot,
    map_w: u32,
    lod: BuildingLod,
    my_id: u16,
    tick_rate_ms: f32,
    out: &mut Vec<RenderedBuilding>,
    clusters: &mut HashMap<BuildingClusterKey, (f32, f32, usize)>,
) {
    let map_w = map_w.max(1);
    out.clear();
    clusters.clear();
    for building in &snapshot.buildings {
        let tile_x = building.tile_idx % map_w;
        let tile_y = building.tile_idx / map_w;
        let active_level = building.active_level();
        let detail = lod.detail(building.kind);
        if detail == BuildingDetail::Cluster {
            let (bx, by) = crate::render::world::movers::tile_to_world(building.tile_idx, map_w);
            let key = BuildingClusterKey {
                grid_x: (tile_x as f32 / lod.cluster_cell_size) as i32,
                grid_y: (tile_y as f32 / lod.cluster_cell_size) as i32,
                owner_id: building.owner_id,
                kind: building.kind,
                level: active_level,
            };
            let entry = clusters.entry(key).or_insert((0.0, 0.0, 0));
            entry.0 += bx;
            entry.1 += by;
            entry.2 += 1;
            continue;
        }

        let footprint = sow_core::building::BuildingFootprint::at(building.kind, tile_x, tile_y);
        out.push(RenderedBuilding {
            bx: footprint.left as f32 + footprint.width as f32 * 0.5,
            by: footprint.top as f32 + footprint.height as f32 * 0.5,
            kind: building.kind,
            level: active_level,
            under_construction: building.under_construction,
            count: 1,
            owner_id: building.owner_id,
            tile_idx: Some(building.tile_idx),
            detail,
            status: building_visual_status(
                building,
                active_level,
                my_id,
                tick_rate_ms,
                detail == BuildingDetail::Full,
            ),
        });
    }

    for (key, (sum_x, sum_y, count)) in clusters.drain() {
        out.push(RenderedBuilding {
            bx: sum_x / count as f32,
            by: sum_y / count as f32,
            kind: key.kind,
            level: key.level,
            under_construction: false,
            count,
            owner_id: key.owner_id,
            tile_idx: None,
            detail: BuildingDetail::Cluster,
            status: None,
        });
    }
}

fn tile_at_world(x: f32, y: f32, map_w: u32) -> Option<u32> {
    let col = x.floor() as i32;
    let row = y.floor() as i32;
    (col >= 0 && row >= 0).then_some((row as u32).checked_mul(map_w)?.checked_add(col as u32)?)
}

fn building_icon_size(zoom_scaled: f32) -> f32 {
    let size = if zoom_scaled < 10.0 {
        zoom_scaled * 2.0
    } else {
        zoom_scaled * 1.6
    };
    size.clamp(11.0, 96.0)
}

fn building_kind_emoji(kind: BuildingKind, level: u8) -> &'static str {
    match (kind, level.clamp(1, kind.max_level())) {
        (BuildingKind::City, 1) => "🏕️",
        (BuildingKind::City, 2) => "🏘️",
        (BuildingKind::City, 3) => "🏡",
        (BuildingKind::City, 4) => "🏙️",
        (BuildingKind::City, 5) => "🏛️",
        (BuildingKind::City, _) => "🌆",
        (BuildingKind::Factory, 1) => "🛠️",
        (BuildingKind::Factory, 2) => "🏗️",
        (BuildingKind::Factory, 3) => "🏭",
        (BuildingKind::Factory, _) => "🏭",
        (BuildingKind::Port, 1) => "⚓",
        (BuildingKind::Port, 2) => "🛶",
        (BuildingKind::Port, 3) => "🚢",
        (BuildingKind::Port, 4) => "⚓",
        (BuildingKind::Port, _) => "🛳️",
        (BuildingKind::Bunker, 1) => "👁️",
        (BuildingKind::Bunker, 2) => "🗼",
        (BuildingKind::Bunker, 3) => "🏰",
        (BuildingKind::Bunker, _) => "🏯",
        (BuildingKind::Farm, 1) => "🌱",
        (BuildingKind::Farm, 2) => "🌾",
        (BuildingKind::Farm, _) => "🚜",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_level_cluster_uses_construction_badge_and_keeps_count() {
        let mut building = RenderedBuilding {
            bx: 0.0,
            by: 0.0,
            kind: BuildingKind::City,
            level: 0,
            under_construction: false,
            count: 4,
            owner_id: 7,
            tile_idx: None,
            detail: BuildingDetail::Cluster,
            status: None,
        };
        assert_eq!(building_badge_label(&building).as_deref(), Some("🔨 × 4"));

        building.level = 3;
        assert_eq!(building_badge_label(&building).as_deref(), Some("3 × 4"));
    }

    #[test]
    fn standard_building_footprint_shares_lod_and_is_stable_inside_a_zoom_band() {
        let far = BuildingLod::for_zoom(1.0);
        for kind in BuildingKind::ALL {
            assert_eq!(far.detail(kind), BuildingDetail::Cluster);
        }
        assert_eq!(far.cluster_cell_size, BUILDING_CLUSTER_TARGET_SIZE);
        assert_eq!(far.band, BuildingLod::for_zoom(1.05).band);
        assert_eq!(
            far.cluster_cell_size,
            BuildingLod::for_zoom(1.05).cluster_cell_size
        );

        let mid = BuildingLod::for_zoom(8.0);
        for kind in BuildingKind::ALL {
            assert_eq!(mid.detail(kind), BuildingDetail::Full);
        }

        let close = BuildingLod::for_zoom(32.0);
        for kind in BuildingKind::ALL {
            assert_eq!(close.detail(kind), BuildingDetail::Full);
        }
    }

    #[test]
    fn building_cache_key_changes_with_tick_map_or_zoom_cluster() {
        let lod = BuildingLod::for_zoom(1.0);
        let mut cache = BuildingRenderCache::default();
        cache.tick = Some(4);
        cache.map_w = 800;
        cache.lod_band = lod.band;

        cache.player_id = 7;
        cache.tick_rate_ms_bits = 100.0f32.to_bits();

        assert!(cache.matches(4, 800, lod.band, 7, 100.0f32.to_bits()));
        assert!(!cache.matches(5, 800, lod.band, 7, 100.0f32.to_bits()));
        assert!(!cache.matches(4, 801, lod.band, 7, 100.0f32.to_bits()));
        assert!(!cache.matches(
            4,
            800,
            BuildingLod::for_zoom(2.0).band,
            7,
            100.0f32.to_bits()
        ));
    }

    fn building_snapshot(
        kind: BuildingKind,
        level: u8,
        under_construction: bool,
        ticks_until_complete: u32,
    ) -> sow_core::protocol::BuildingSnapshot {
        sow_core::protocol::BuildingSnapshot {
            id: 1,
            tile_idx: 0,
            owner_id: 7,
            kind,
            level,
            under_construction,
            ticks_until_complete,
        }
    }

    #[test]
    fn building_status_distinguishes_new_construction_from_upgrade() {
        let new_build = building_snapshot(BuildingKind::City, 1, true, 20);
        let status = building_visual_status(&new_build, new_build.active_level(), 7, 100.0, true)
            .expect("new construction status");
        assert_eq!(status.label.as_deref(), Some("🏗️ 2s"));
        assert_eq!(status.color, [0.0, 0.86, 1.0, 1.0]);
        assert_eq!(status.progress, 0.0);

        let upgrade_ticks = sow_core::building::core::upgrade_duration_ticks(BuildingKind::City, 2);
        let upgrade = building_snapshot(BuildingKind::City, 2, true, upgrade_ticks);
        let status = building_visual_status(&upgrade, upgrade.active_level(), 7, 100.0, true)
            .expect("upgrade status");
        assert_eq!(status.label.as_deref(), Some("🏗️ 2 · 2.2s"));
        assert_eq!(status.color, [1.0, 0.82, 0.22, 1.0]);
        assert_eq!(status.progress, 0.0);
    }

    #[test]
    fn building_status_reports_progress_for_one_upgrade() {
        let duration_two = sow_core::building::core::upgrade_duration_ticks(BuildingKind::City, 2);
        let remaining = duration_two / 2;
        let upgrade = building_snapshot(BuildingKind::City, 2, true, remaining);
        let status = building_visual_status(&upgrade, upgrade.active_level(), 7, 100.0, true)
            .expect("upgrade status");
        assert_eq!(
            status.label,
            Some(format!(
                "🏗️ 2 · {}",
                format_construction_time(remaining, 100.0)
            ))
        );
        let expected_progress = 1.0 - (remaining as f32 / duration_two as f32);
        assert!((status.progress - expected_progress).abs() < 0.001);
    }

    #[test]
    fn building_status_uses_configured_tick_rate_and_disappears_when_ready() {
        let building = building_snapshot(BuildingKind::City, 1, true, 20);
        let status = building_visual_status(&building, building.active_level(), 7, 250.0, true)
            .expect("construction status");
        assert_eq!(status.label.as_deref(), Some("🏗️ 5s"));
        let compact = building_visual_status(&building, building.active_level(), 7, 250.0, false)
            .expect("compact construction status");
        assert_eq!(compact.label, None);

        let ready = building_snapshot(BuildingKind::City, 1, false, 0);
        assert!(building_visual_status(&ready, ready.active_level(), 7, 100.0, true).is_none());
    }

    #[test]
    fn building_levels_restore_the_historical_emojis_and_all_are_packed() {
        let expected: &[(BuildingKind, &[&str])] = &[
            (BuildingKind::City, &["🏕️", "🏘️", "🏡", "🏙️", "🏛️", "🌆"]),
            (BuildingKind::Factory, &["🛠️", "🏗️", "🏭", "🏭"]),
            (BuildingKind::Port, &["⚓", "🛶", "🚢", "⚓", "🛳️"]),
            (BuildingKind::Bunker, &["👁️", "🗼", "🏰", "🏯"]),
            (BuildingKind::Farm, &["🌱", "🌾", "🚜"]),
        ];

        for (kind, emojis) in expected {
            assert_eq!(emojis.len(), kind.max_level() as usize);
            assert_eq!(building_kind_emoji(*kind, 0), emojis[0]);
            for (level, emoji) in emojis.iter().enumerate() {
                let level = level as u8 + 1;
                assert_eq!(building_kind_emoji(*kind, level), *emoji);
                assert!(
                    crate::render::gpu::emoji_uv_opt(emoji).is_some(),
                    "missing emoji atlas entry for {kind:?} level {level}: {emoji}"
                );
            }
        }
        for level in 1..=BuildingKind::Farm.max_level() {
            assert!(crate::render::gpu::building_sprite_uv(BuildingKind::Farm, level).is_some());
        }
        assert!(crate::render::gpu::building_sprite_uv(BuildingKind::Farm, 0).is_some());
        assert!(crate::render::gpu::building_sprite_uv(BuildingKind::Farm, 4).is_none());
        for kind in [
            BuildingKind::City,
            BuildingKind::Factory,
            BuildingKind::Port,
            BuildingKind::Bunker,
        ] {
            assert!(crate::render::gpu::building_sprite_uv(kind, 1).is_none());
        }
    }

    #[test]
    fn tutorial_hand_target_uses_camera_projection_in_css_pixels() {
        let world = (18.25, 7.75, 96.0, 48.0, 3.0);
        for sf in [1.0, 2.0] {
            let target =
                world_to_screen_values(world.0 + 0.5, world.1 + 0.5, world.2, world.3, world.4, sf);
            assert_eq!(target[0], (world.2 + (world.0 + 0.5) * world.4) / sf);
            assert_eq!(target[1], (world.3 + (world.1 + 0.5) * world.4) / sf);
        }
    }
}
