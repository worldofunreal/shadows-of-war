use super::feedback;
use crate::app::{InputState, MapContextMenuView, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::game::BuildingKind;
use sow_core::protocol::SimSnapshot;
use sow_render::text::NameplateStatusSprite;
use std::collections::HashMap;
use web_time::{Duration, Instant};

const BUILDING_CULL_FLOOR: f32 = 0.25;
const BUILDING_FULL_DETAIL_SIZE: f32 = 28.0;
const BUILDING_MIN_MARKER_SIZE: f32 = 14.0;
const BUILDING_CLUSTER_TARGET_SIZE: f32 = 40.0;
const BUILDING_ZOOM_STEPS_PER_OCTAVE: f32 = 8.0;
const BUILDING_FOOTPRINT_FILL: f32 = 0.9;
const BUILDING_UPGRADE_SCALES: [f32; 4] = [1.0, 1.15, 1.3, 1.5];
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
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    sf: f32,
    now: Instant,
    placement_can_confirm: bool,
    building_panel: Option<&(
        crate::ui::building_panel::BuildingPanel,
        crate::ui::building_panel::BuildingPanelLayout,
    )>,
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
    render_building_placement_preview(text, sim, ui, input, sf, placement_can_confirm);
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
        ui.app.settings_state.reduced_motion,
    );
    feedback::render(
        text,
        snapshot,
        sim,
        ui,
        input,
        &dev,
        campaign_avatar_slots,
        sf,
        now,
    );
    render_building_placement_controls(text, sim, ui, input, sf, placement_can_confirm);
    if let Some((panel, layout)) = building_panel {
        render_building_panel(text, panel, layout, &dev, sf);
    }

    if !ui.tutorial_active {
        ui.tutorial_marker_player_id = None;
        ui.dialog_border_highlight = None;
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
    target_level.map_or_else(|| time.clone(), |level| format!("{level} · {time}"))
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
        let building_size = building_sprite_size(marker_size, sf, building.level.max(1));
        let half_width = building_size[0] * 0.5;
        let half_height = building_size[1] * 0.5;
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
        // Every kind and level has art in assets/art/buildings (checked by tests).
        if let Some(uv_rect) =
            crate::render::gpu::building_sprite_uv(building.kind, building.level.max(1))
        {
            text.push_building_sprite(
                building_center,
                building_size,
                uv_rect,
                [1.0, 1.0, 1.0, alpha],
            );
        }

        if building.detail == BuildingDetail::Full
            && let Some(tile_idx) = building.tile_idx
            && let Some(started) = building_upgrade_flashes.get(&tile_idx)
        {
            let t = (now.duration_since(*started).as_secs_f32() / 0.3).clamp(0.0, 1.0);
            let alpha = 1.0 - t;
            text.push_status_sprite(
                NameplateStatusSprite::UpgradeSparkle,
                [center[0] * sf, center[1] * sf],
                2.0 * marker_extent * sf * (0.5 + 0.35 * t),
                [1.0, 0.88, 0.46, alpha],
            );
            if building.owner_id == my_id
                && !reduced_motion
                && celebration_budget >= BUILDING_CELEBRATION_OFFSETS.len()
            {
                let radius = marker_extent * sf * (0.3 + 0.55 * t);
                let sparkle_size = (marker_extent * sf * 0.18).clamp(4.0 * sf, 14.0 * sf);
                for offset in BUILDING_CELEBRATION_OFFSETS {
                    text.push_status_sprite(
                        NameplateStatusSprite::UpgradeSparkle,
                        [
                            center[0] * sf + offset[0] * radius,
                            center[1] * sf + offset[1] * radius,
                        ],
                        2.0 * sparkle_size,
                        [1.0, 0.84, 0.36, alpha * 0.9],
                    );
                }
                celebration_budget -= BUILDING_CELEBRATION_OFFSETS.len();
            }
        }

        if (building.detail == BuildingDetail::Full || building.count > 1)
            && let Some((icon, label)) = building_badge_label(building)
        {
            let level_font_size = (marker_extent * BUILDING_LEVEL_FONT_RATIO)
                .clamp(8.0, 18.0)
                .round()
                * dev.font_size_scale.max(0.1);
            let label_center = [
                center[0] + half_width * 0.78,
                center[1] - half_height * 0.78,
            ];
            push_icon_label(
                text,
                icon,
                &label,
                [
                    label_center[0] * sf,
                    (label_center[1] + level_font_size * 0.25) * sf,
                ],
                level_font_size * sf,
                text_style,
                dev.font_char_spacing.max(0.1),
                crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]),
            );
        }

        if let Some(status) = &building.status
            && let Some(label) = &status.label
        {
            render_building_preview_badge(text, [center[0] * sf, center[1] * sf], label, dev, sf);
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
            if pointer_world_x < building.bx - width as f32 * 0.5
                || pointer_world_x >= building.bx + width as f32 * 0.5
                || pointer_world_y < building.by - height as f32 * 0.5
                || pointer_world_y >= building.by + height as f32 * 0.5
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
        let minimum = if building.status.is_some() {
            BUILDING_MIN_MARKER_SIZE
        } else {
            0.0
        };
        building_footprint_marker_size(building.kind, zoom_scaled, minimum)
    } else {
        let size = (building_icon_size(zoom_scaled) * if building.count > 1 { 0.6 } else { 0.5 })
            .max(BUILDING_MIN_MARKER_SIZE);
        [size; 2]
    }
}

fn building_footprint_marker_size(kind: BuildingKind, zoom_scaled: f32, minimum: f32) -> [f32; 2] {
    let (width, height) = kind.footprint_dimensions();
    [
        (width as f32 * zoom_scaled * BUILDING_FOOTPRINT_FILL).max(minimum),
        (height as f32 * zoom_scaled * BUILDING_FOOTPRINT_FILL).max(minimum),
    ]
}

fn building_sprite_size(marker_size: [f32; 2], sf: f32, level: u8) -> [f32; 2] {
    let scale = BUILDING_UPGRADE_SCALES
        .get(usize::from(level.saturating_sub(1)))
        .copied()
        .unwrap_or(1.5);
    marker_size.map(|size| size * sf * scale)
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
    let anchor_center = [
        input.camera_x + (tile_x as f32 + 0.5) * zoom,
        input.camera_y + (tile_y as f32 + 0.5) * zoom,
    ];

    let colors = placement_preview_colors(true, true);
    let metrics = placement_grid_metrics(zoom, sf);
    render_building_footprint_fill(text, [left, top], [width, height], colors.fill);
    render_building_footprint_grid(
        text,
        footprint,
        [left, top],
        [width, height],
        zoom,
        metrics,
        colors.line,
    );

    if kind == BuildingKind::Tower && menu.building_level > 0 && !menu.building_under_construction {
        let range_multiplier = sim
            .engine
            .as_ref()
            .and_then(|engine| {
                engine
                    .buildings
                    .iter()
                    .find(|building| building.tile_idx == menu.tile_idx)
                    .and_then(|building| engine.state.player(building.owner_id))
                    .map(|player| player.leader.tower_range_multiplier())
            })
            .unwrap_or(1.0);
        let range = sow_core::building::tower_range_for_level(
            menu.building_level,
            &sim.config,
            range_multiplier,
        );
        text.push_ring(
            anchor_center,
            range as f32 * zoom,
            [0.91, 0.71, 0.34, 0.30],
            (1.4 * sf).max(1.0),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PlacementPreviewColors {
    fill: [f32; 4],
    line: [f32; 4],
    building: [f32; 4],
}

const PLACEMENT_CONFLICT_COLOR: [f32; 4] = [0.35, 0.02, 0.03, 1.0];

#[derive(Clone, Copy, Debug, PartialEq)]
struct PlacementFootprintGeometry {
    top_left: [f32; 2],
    center: [f32; 2],
    size: [f32; 2],
}

fn placement_footprint_geometry(
    footprint: sow_core::building::BuildingFootprint,
    camera: [f32; 2],
    zoom: f32,
) -> PlacementFootprintGeometry {
    let top_left = [
        camera[0] + footprint.left as f32 * zoom,
        camera[1] + footprint.top as f32 * zoom,
    ];
    let size = [
        footprint.width as f32 * zoom,
        footprint.height as f32 * zoom,
    ];
    PlacementFootprintGeometry {
        top_left,
        center: [top_left[0] + size[0] * 0.5, top_left[1] + size[1] * 0.5],
        size,
    }
}

fn placement_preview_colors(site_valid: bool, can_confirm: bool) -> PlacementPreviewColors {
    if !site_valid {
        PlacementPreviewColors {
            fill: [0.75, 0.10, 0.08, 1.0],
            line: [1.0, 0.55, 0.48, 1.0],
            building: [1.0, 0.28, 0.28, 0.94],
        }
    } else if can_confirm {
        PlacementPreviewColors {
            fill: [0.035, 0.26, 0.06, 1.0],
            line: [0.66, 0.97, 0.45, 1.0],
            building: [1.0, 1.0, 1.0, 0.94],
        }
    } else {
        PlacementPreviewColors {
            fill: [0.32, 0.36, 0.38, 1.0],
            line: [0.75, 0.80, 0.82, 1.0],
            building: [1.0, 1.0, 1.0, 0.94],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PlacementGridMetrics {
    inner_line_px: f32,
    outer_border_px: f32,
    draw_inner_grid: bool,
}

fn placement_grid_metrics(zoom: f32, sf: f32) -> PlacementGridMetrics {
    let sf = sf.max(0.01);
    PlacementGridMetrics {
        inner_line_px: sf,
        outer_border_px: 2.0 * sf,
        draw_inner_grid: zoom / sf >= 4.0,
    }
}

fn render_building_footprint_fill(
    text: &mut TextRenderer,
    top_left: [f32; 2],
    size: [f32; 2],
    fill: [f32; 4],
) {
    text.push_rect(top_left, size, fill);
}

fn render_building_footprint_grid(
    text: &mut TextRenderer,
    footprint: sow_core::building::BuildingFootprint,
    top_left: [f32; 2],
    size: [f32; 2],
    zoom: f32,
    metrics: PlacementGridMetrics,
    color: [f32; 4],
) {
    let [left, top] = top_left;
    let [width, height] = size;
    if metrics.draw_inner_grid {
        for i in 1..footprint.width {
            let x = left + i as f32 * zoom - metrics.inner_line_px * 0.5;
            text.push_rect([x, top], [metrics.inner_line_px, height], color);
        }
        for i in 1..footprint.height {
            let y = top + i as f32 * zoom - metrics.inner_line_px * 0.5;
            text.push_rect([left, y], [width, metrics.inner_line_px], color);
        }
    }
    let rim = metrics.outer_border_px;
    for (pos, border_size) in [
        ([left - rim, top - rim], [width + 2.0 * rim, rim]),
        ([left - rim, top + height], [width + 2.0 * rim, rim]),
        ([left - rim, top], [rim, height]),
        ([left + width, top], [rim, height]),
    ] {
        text.push_rect(pos, border_size, color);
    }
}

fn render_building_placement_preview(
    text: &mut TextRenderer,
    sim: &SimState,
    ui: &UiState,
    input: &InputState,
    sf: f32,
    can_confirm: bool,
) {
    let Some(placement) = ui.app.hud_state.building_placement.filter(|p| !p.pending) else {
        return;
    };
    let kind = placement.kind;
    if sim.map_w == 0 || !input.camera_zoom.is_finite() || input.camera_zoom <= 0.0 {
        return;
    }
    let Some(engine) = sim.engine.as_ref() else {
        return;
    };
    let my_id = sim.my_player_id.unwrap_or(0);
    let site_valid = sow_core::building::validate_structure_site(
        &engine.state.map,
        my_id,
        kind,
        placement.tile_idx,
        &engine.building_grid,
    )
    .is_ok();
    let zoom = input.camera_zoom;
    let (tile_x, tile_y) = (
        placement.tile_idx % sim.map_w,
        placement.tile_idx / sim.map_w,
    );
    let footprint = sow_core::building::BuildingFootprint::at(kind, tile_x, tile_y);
    let geometry = placement_footprint_geometry(footprint, [input.camera_x, input.camera_y], zoom);
    let center_px = geometry.center;
    let footprint_size_px = geometry.size;
    let footprint_top_left = geometry.top_left;
    let colors = placement_preview_colors(site_valid, can_confirm);
    let metrics = placement_grid_metrics(zoom, sf);
    let map = &engine.state.map;
    render_building_footprint_fill(text, footprint_top_left, footprint_size_px, colors.fill);
    if metrics.draw_inner_grid && !site_valid {
        for y in footprint.top..footprint.top + footprint.height as i32 {
            for x in footprint.left..footprint.left + footprint.width as i32 {
                // Mark only cells rejected by the same ownership, terrain, or overlap rules.
                let conflict = !map.is_valid_coord(x, y)
                    || sow_core::building::validate_building_cell(
                        kind,
                        my_id,
                        map.owner_id(x as u32, y as u32),
                        map.terrain[map.ref_id(x as u32, y as u32)].as_byte(),
                    )
                    .is_err()
                    || engine
                        .building_grid
                        .iter_all_in_range(tile_x, tile_y, 3)
                        .any(|b| {
                            sow_core::building::BuildingFootprint::at(b.kind, b.x, b.y)
                                .contains(x, y)
                        });
                if conflict {
                    text.push_rect(
                        [
                            input.camera_x + x as f32 * zoom,
                            input.camera_y + y as f32 * zoom,
                        ],
                        [zoom; 2],
                        PLACEMENT_CONFLICT_COLOR,
                    );
                }
            }
        }
    }
    render_building_footprint_grid(
        text,
        footprint,
        footprint_top_left,
        footprint_size_px,
        zoom,
        metrics,
        colors.line,
    );
    let dragging = input.building_drag_offset.is_some();
    let sprite_size = building_sprite_size(
        building_footprint_marker_size(kind, zoom / sf, BUILDING_MIN_MARKER_SIZE),
        sf,
        1,
    );
    let lift = if dragging && !ui.app.settings_state.reduced_motion {
        5.0 * sf
    } else {
        0.0
    };
    if dragging {
        text.push_rect(footprint_top_left, footprint_size_px, [0.0, 0.0, 0.0, 0.16]);
    }
    let sprite_center = [center_px[0], center_px[1] - lift];
    if let Some(uv_rect) = crate::render::gpu::building_sprite_uv(kind, 1) {
        text.push_building_sprite(sprite_center, sprite_size, uv_rect, colors.building);
    }
    if kind == BuildingKind::Tower {
        let range_multiplier = engine
            .state
            .player(my_id)
            .map_or(1.0, |player| player.leader.tower_range_multiplier());
        let range = sow_core::building::tower_range_for_level(1, &sim.config, range_multiplier);
        text.push_ring(
            center_px,
            range as f32 * input.camera_zoom,
            [0.75, 0.85, 1.0, 0.32],
            (1.5 * sf).max(1.0),
        );
    }
}

/// Drag-hint arrows and the cancel/confirm pair, drawn last so they sit above the world.
fn render_building_placement_controls(
    text: &mut TextRenderer,
    sim: &SimState,
    ui: &UiState,
    input: &InputState,
    sf: f32,
    can_confirm: bool,
) {
    use crate::input::placement::PlacementDirection;
    let Some(placement) = ui.app.hud_state.building_placement.filter(|p| !p.pending) else {
        return;
    };
    if sim.map_w == 0 || !input.camera_zoom.is_finite() || input.camera_zoom <= 0.0 {
        return;
    }
    let controls = crate::input::placement::placement_controls(
        placement.kind,
        placement.tile_idx,
        sim.map_w,
        [input.camera_x, input.camera_y],
        input.camera_zoom,
        sf,
    );
    for (direction, center) in controls.arrows {
        let sprite = match direction {
            PlacementDirection::Up => NameplateStatusSprite::ArrowUp,
            PlacementDirection::Right => NameplateStatusSprite::ArrowRight,
            PlacementDirection::Down => NameplateStatusSprite::ArrowDown,
            PlacementDirection::Left => NameplateStatusSprite::ArrowLeft,
        };
        text.push_status_sprite(sprite, center, controls.arrow, [1.0; 4]);
    }
    text.push_status_sprite(
        NameplateStatusSprite::Cancel,
        controls.cancel,
        controls.button,
        [1.0; 4],
    );
    if can_confirm {
        text.push_status_sprite(
            NameplateStatusSprite::Confirm,
            controls.confirm,
            controls.button,
            [1.0; 4],
        );
    } else {
        text.push_status_sprite_desaturated(
            NameplateStatusSprite::Confirm,
            controls.confirm,
            controls.button,
            [0.5, 0.5, 0.5, 0.75],
        );
    }
}

/// Keep cluster counts visible without overlaying building levels on the art.
fn building_badge_label(
    building: &RenderedBuilding,
) -> Option<(Option<NameplateStatusSprite>, String)> {
    if building.under_construction {
        return None;
    }
    if building.level == 0 {
        return (building.count > 1).then(|| (None, format!("× {}", building.count)));
    }
    if building.level == 1 && building.count == 1 {
        return None;
    }
    (building.count > 1).then(|| (None, format!("× {}", building.count)))
}

/// Text centered on `center`, with an optional badge icon before it.
/// `center[1]` is the text baseline, as in `push_string`.
#[allow(clippy::too_many_arguments)]
fn push_icon_label(
    text: &mut TextRenderer,
    icon: Option<NameplateStatusSprite>,
    label: &str,
    center: [f32; 2],
    font_size: f32,
    style: crate::render::gpu::TextPaintStyle,
    char_spacing: f32,
    outline: crate::render::gpu::OutlineStyle,
) {
    let width = if label.is_empty() {
        0.0
    } else {
        text.measure_string(label, font_size, char_spacing).width
    };
    let (icon_px, gap) = icon_label_spacing(icon.is_some(), label, font_size);
    let left = center[0] - (icon_px + gap + width) * 0.5;
    if let Some(icon) = icon {
        text.push_icon(
            icon,
            [left + icon_px * 0.5, center[1] - font_size * 0.3],
            icon_px * 0.5,
            [1.0; 4],
            outline,
        );
    }
    if !label.is_empty() {
        text.push_string(
            label,
            [left + icon_px + gap + width * 0.5, center[1]],
            font_size,
            [1.0; 4],
            style,
            (0.5, char_spacing),
        );
    }
}

fn icon_label_spacing(has_icon: bool, label: &str, font_size: f32) -> (f32, f32) {
    match (has_icon, label.is_empty()) {
        (true, true) => (font_size, 0.0),
        (true, false) => (font_size, font_size * 0.25),
        (false, _) => (0.0, 0.0),
    }
}

/// Full width `push_icon_label` draws for this icon and label.
fn icon_label_width(
    text: &mut TextRenderer,
    has_icon: bool,
    label: &str,
    font_size: f32,
    char_spacing: f32,
) -> f32 {
    let (icon_px, gap) = icon_label_spacing(has_icon, label, font_size);
    let width = if label.is_empty() {
        0.0
    } else {
        text.measure_string(label, font_size, char_spacing).width
    };
    icon_px + gap + width
}

/// Width of each `push_icon_label_row` item: icon, text and pill padding.
fn icon_label_row_widths(
    text: &mut TextRenderer,
    items: &[(NameplateStatusSprite, String)],
    font_size: f32,
    spacing: f32,
    padding: f32,
) -> Vec<f32> {
    items
        .iter()
        .map(|(_, label)| icon_label_width(text, true, label, font_size, spacing) + 2.0 * padding)
        .collect()
}

/// Letter size at which a row (`widths` measured at `font_size`, plus `gaps`) fits `max_width`.
// ponytail: letters stop at 40% of their size; a longer row needs a wider button.
fn fitted_row_font(font_size: f32, widths: f32, gaps: f32, max_width: f32) -> f32 {
    let room = (max_width - gaps).max(0.0);
    if widths <= room || widths <= 0.0 {
        font_size
    } else {
        font_size * (room / widths).max(0.4)
    }
}

/// Lays `items` out side by side, centered on `center` with `gap` between them.
#[allow(clippy::too_many_arguments)]
fn push_icon_label_row(
    text: &mut TextRenderer,
    items: &[(NameplateStatusSprite, String)],
    center: [f32; 2],
    font_size: f32,
    gap: f32,
    pill: Option<(f32, f32, [f32; 4], [f32; 4])>,
    dev: &DevConfig,
    sf: f32,
) {
    let spacing = dev.font_char_spacing.max(0.1);
    let style = crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let outline = crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let padding = pill.map_or(0.0, |(padding, ..)| padding);
    let widths = icon_label_row_widths(text, items, font_size, spacing, padding);
    let total = widths.iter().sum::<f32>() + gap * items.len().saturating_sub(1) as f32;
    let mut left = center[0] - total * 0.5;
    for ((icon, label), width) in items.iter().zip(widths) {
        let item_center = [left + width * 0.5, center[1]];
        if let Some((_, height, fill, edge)) = pill {
            text.push_rounded_rect(item_center, [width, height], 0.5, fill, edge, 1.5 * sf);
        }
        push_icon_label(
            text,
            Some(*icon),
            label,
            [item_center[0], item_center[1] + font_size * 0.3],
            font_size,
            style,
            spacing,
            outline,
        );
        left += width + gap;
    }
}

/// What the next level adds, and the one-tap upgrade button, under your tapped
/// building. While it builds, the row shows the seconds left instead.
fn render_building_panel(
    text: &mut TextRenderer,
    panel: &crate::ui::building_panel::BuildingPanel,
    layout: &crate::ui::building_panel::BuildingPanelLayout,
    dev: &DevConfig,
    sf: f32,
) {
    let scale = dev.font_size_scale.max(0.1);
    let mut chips: Vec<(NameplateStatusSprite, String)> = panel
        .remaining_seconds
        .map(|seconds| {
            (
                NameplateStatusSprite::Clock,
                crate::utils::format_number(seconds.ceil().max(0.0)),
            )
        })
        .into_iter()
        .collect();
    chips.extend(
        panel
            .chips
            .iter()
            .map(|metric| (metric.icon, metric.label())),
    );
    if !chips.is_empty() {
        push_icon_label_row(
            text,
            &chips,
            layout.chips_center,
            15.0 * sf * scale,
            6.0 * sf,
            Some((
                8.0 * sf,
                layout.chip_height,
                [0.13, 0.09, 0.05, 0.9],
                [0.86, 0.66, 0.28, 1.0],
            )),
            dev,
            sf,
        );
    }
    let Some(offer) = panel.upgrade else {
        return;
    };
    let (fill, edge) = if offer.enabled {
        ([0.98, 0.74, 0.22, 1.0], [0.36, 0.2, 0.05, 1.0])
    } else {
        ([0.52, 0.5, 0.47, 1.0], [0.2, 0.19, 0.18, 1.0])
    };
    text.push_rounded_rect(
        layout.button_center,
        layout.button_size,
        0.3,
        fill,
        edge,
        3.0 * sf,
    );
    let items = [
        (NameplateStatusSprite::ArrowUp, offer.next_level.to_string()),
        (
            NameplateStatusSprite::Gold,
            crate::utils::format_number(offer.cost),
        ),
        (
            NameplateStatusSprite::Clock,
            crate::utils::format_number(offer.seconds.ceil()),
        ),
    ];
    // The tap area is the button, so the letters shrink to fit inside it.
    let font_size = 18.0 * sf * scale;
    let gap = 14.0 * sf;
    let spacing = dev.font_char_spacing.max(0.1);
    let widths = icon_label_row_widths(text, &items, font_size, spacing, 0.0);
    let fitted = fitted_row_font(
        font_size,
        widths.iter().sum(),
        gap * (items.len() - 1) as f32,
        layout.button_size[0] - 24.0 * sf,
    );
    push_icon_label_row(
        text,
        &items,
        layout.button_center,
        fitted,
        gap,
        None,
        dev,
        sf,
    );
}

fn render_building_preview_badge(
    text: &mut TextRenderer,
    center: [f32; 2],
    label: &str,
    dev: &DevConfig,
    sf: f32,
) {
    let font_size = 14.0 * dev.font_size_scale.max(0.1) * sf;
    let measure = text.measure_string(label, font_size, dev.font_char_spacing.max(0.1));
    let top = center[1] - measure.height - 8.0 * sf;
    push_icon_label(
        text,
        Some(NameplateStatusSprite::Construction),
        label,
        [center[0], top + measure.height * 0.5 + font_size * 0.3],
        font_size,
        crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]),
        dev.font_char_spacing.max(0.1),
        crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]),
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
        let keep_local_progress = building.owner_id == my_id && building.under_construction;
        if detail == BuildingDetail::Cluster && !keep_local_progress {
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
                detail == BuildingDetail::Full || keep_local_progress,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitted_row_font_keeps_letters_that_fit_and_shrinks_the_rest_into_the_button() {
        assert_eq!(fitted_row_font(18.0, 100.0, 20.0, 200.0), 18.0);
        let fitted = fitted_row_font(18.0, 300.0, 20.0, 200.0);
        // Widths scale with the letters, so the shrunk row ends exactly at the button edge.
        assert!((300.0 * fitted / 18.0 + 20.0 - 200.0).abs() < 1e-3);
        assert_eq!(fitted_row_font(18.0, 1000.0, 20.0, 200.0), 18.0 * 0.4);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn placement_sprite_keeps_built_scale_and_remains_visible_when_zooming_out() {
        for kind in BuildingKind::ALL {
            let footprint = sow_core::building::BuildingFootprint::at(kind, 17, 23);
            let expected = if kind == BuildingKind::City { 4.0 } else { 2.0 };
            assert_eq!([footprint.width, footprint.height], [expected as u32; 2]);
            for zoom in [0.5, 4.0, 12.0] {
                let geometry = placement_footprint_geometry(footprint, [31.0, -12.0], zoom);
                assert_eq!(geometry.size, [expected * zoom, expected * zoom]);
                assert_eq!(
                    geometry.center,
                    [
                        geometry.top_left[0] + geometry.size[0] * 0.5,
                        geometry.top_left[1] + geometry.size[1] * 0.5,
                    ]
                );
            }
            let building = RenderedBuilding {
                bx: 0.0,
                by: 0.0,
                kind,
                level: 1,
                under_construction: false,
                count: 1,
                owner_id: 7,
                tile_idx: Some(0),
                detail: BuildingDetail::Full,
                status: None,
            };
            for zoom in [4.0, 8.0, 12.0, 32.0] {
                assert_eq!(
                    building_footprint_marker_size(kind, zoom, BUILDING_MIN_MARKER_SIZE),
                    building_marker_size(&building, zoom),
                    "the preview preserves the actual building's scale at placement zoom"
                );
            }
            for zoom in [0.25, 1.0, 2.0] {
                assert_eq!(
                    building_footprint_marker_size(kind, zoom, BUILDING_MIN_MARKER_SIZE),
                    [BUILDING_MIN_MARKER_SIZE; 2],
                    "the selected building remains individually visible at overview zoom"
                );
            }
        }

        for (zoom, sf, expected_grid) in [
            (3.99, 1.0, false),
            (4.0, 1.0, true),
            (6.0, 2.0, false),
            (8.0, 2.0, true),
        ] {
            let metrics = placement_grid_metrics(zoom, sf);
            assert_eq!(metrics.inner_line_px, sf);
            assert_eq!(metrics.outer_border_px, 2.0 * sf);
            assert_eq!(metrics.draw_inner_grid, expected_grid);
        }

        let ready = placement_preview_colors(true, true);
        assert_eq!(ready.fill, [0.035, 0.26, 0.06, 1.0]);
        assert_eq!(ready.line, [0.66, 0.97, 0.45, 1.0]);
        assert_eq!(ready.building, [1.0, 1.0, 1.0, 0.94]);
        let invalid = placement_preview_colors(false, true);
        assert_eq!(invalid.fill, [0.75, 0.10, 0.08, 1.0]);
        assert_eq!(invalid.building, [1.0, 0.28, 0.28, 0.94]);
        let blocked = placement_preview_colors(true, false);
        assert_eq!(blocked.fill, [0.32, 0.36, 0.38, 1.0]);
        assert_eq!(blocked.line, [0.75, 0.80, 0.82, 1.0]);
        assert_eq!(blocked.building, [1.0, 1.0, 1.0, 0.94]);
    }

    #[test]
    fn zero_level_cluster_keeps_count_without_hammer_badge() {
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
        assert_eq!(
            building_badge_label(&building),
            Some((None, "× 4".to_string()))
        );

        building.level = 3;
        assert_eq!(
            building_badge_label(&building),
            Some((None, "× 4".to_string()))
        );
        building.count = 1;
        assert_eq!(building_badge_label(&building), None);
    }

    #[test]
    fn building_upgrade_visual_scale_never_exceeds_one_hundred_fifty_percent() {
        assert_eq!(BUILDING_UPGRADE_SCALES, [1.0, 1.15, 1.3, 1.5]);
        assert!(
            BUILDING_UPGRADE_SCALES
                .into_iter()
                .all(|scale| scale <= 1.5)
        );
        assert_eq!(building_sprite_size([100.0; 2], 1.0, 4), [150.0; 2]);
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
    fn distant_lod_keeps_only_local_construction_progress_visible() {
        use sow_core::game::GamePhase;
        use sow_core::protocol::SimSnapshot;
        use std::sync::Arc;

        let mut local_construction = building_snapshot(BuildingKind::City, 1, true, 20);
        local_construction.tile_idx = 0;
        let mut enemy_construction = local_construction.clone();
        enemy_construction.owner_id = 8;
        enemy_construction.tile_idx = 1;
        let mut completed = building_snapshot(BuildingKind::City, 1, false, 0);
        completed.tile_idx = 2;
        let snapshot = SimSnapshot {
            tick: 1,
            phase: GamePhase::Playing,
            spawn_timer_secs: None,
            players: Vec::new(),
            dirty_tiles: Vec::new(),
            fleets: Vec::new(),
            attacks: Vec::new(),
            buildings: vec![local_construction, enemy_construction, completed],
            projectiles: Vec::new(),
            nuke_alerts: Vec::new(),
            resource_transfers: Vec::new(),
            resource_rejections: Vec::new(),
            resource_transfer_rejections: Vec::new(),
            winner: None,
            winning_team: None,
            defense_posts: Vec::new(),
            defense_dirty: false,
            total_land_tiles: 0,
            sea_lanes: Arc::new(Vec::new()),
            debug_mem_info: String::new(),
        };
        let mut buildings = Vec::new();
        let mut clusters = HashMap::new();

        collect_buildings(
            &snapshot,
            100,
            BuildingLod::for_zoom(1.0),
            7,
            100.0,
            &mut buildings,
            &mut clusters,
        );

        assert_eq!(buildings.len(), 3);
        let local = buildings
            .iter()
            .find(|building| building.tile_idx == Some(0))
            .expect("local construction remains individual at distant zoom");
        let status = local
            .status
            .as_ref()
            .expect("local progress remains visible");
        assert_eq!(status.label.as_deref(), Some("2s"));
        assert_eq!(
            building_marker_size(local, 1.0),
            [BUILDING_MIN_MARKER_SIZE; 2]
        );

        let clustered: Vec<_> = buildings
            .iter()
            .filter(|building| building.tile_idx.is_none())
            .collect();
        assert_eq!(clustered.len(), 2);
        assert!(clustered.iter().all(|building| building.status.is_none()));
    }

    #[test]
    fn building_status_distinguishes_new_construction_from_upgrade() {
        let new_build = building_snapshot(BuildingKind::City, 1, true, 20);
        let status = building_visual_status(&new_build, new_build.active_level(), 7, 100.0, true)
            .expect("new construction status");
        assert_eq!(status.label.as_deref(), Some("2s"));
        assert_eq!(status.color, [0.0, 0.86, 1.0, 1.0]);
        assert_eq!(status.progress, 0.0);

        let upgrade_ticks = sow_core::building::core::upgrade_duration_ticks(BuildingKind::City, 2);
        let upgrade = building_snapshot(BuildingKind::City, 2, true, upgrade_ticks);
        let status = building_visual_status(&upgrade, upgrade.active_level(), 7, 100.0, true)
            .expect("upgrade status");
        assert_eq!(status.label.as_deref(), Some("2 · 2.2s"));
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
                "2 · {}",
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
        assert_eq!(status.label.as_deref(), Some("5s"));
        let compact = building_visual_status(&building, building.active_level(), 7, 250.0, false)
            .expect("compact construction status");
        assert_eq!(compact.label, None);

        let ready = building_snapshot(BuildingKind::City, 1, false, 0);
        assert!(building_visual_status(&ready, ready.active_level(), 7, 100.0, true).is_none());
    }

    #[test]
    fn every_building_kind_and_level_has_packed_art() {
        for kind in BuildingKind::ALL {
            for level in 0..=kind.max_level() {
                assert!(
                    crate::render::gpu::building_sprite_uv(kind, level).is_some(),
                    "missing assets/art/buildings art for {kind:?} level {level}"
                );
            }
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
