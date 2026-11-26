use super::feedback;
use crate::app::{InputState, MapContextMenuView, SimState, UiState};
use crate::render::gpu::{BuildingMarkerInstance, BuildingMarkerShape, TextRenderer};
use crate::theme::dev_config::DevConfig;
use sow_core::game::BuildingKind;
use sow_core::protocol::SimSnapshot;
use sow_render::text::NameplateStatusSprite;
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

const BUILDING_CULL_FLOOR: f32 = 0.05;
const BUILDING_MIN_MARKER_SIZE: f32 = 14.0;
/// Short side, in tiles, of the standard building the Dev Tools zoom cutoffs describe.
const BUILDING_LOD_REFERENCE_SIDE: f32 = 2.0;
/// Quantized zoom bands per octave for world-anchored spatial thinning (zero panning flicker).
const BUILDING_ZOOM_STEPS_PER_OCTAVE: f32 = 4.0;
const BUILDING_FOOTPRINT_FILL: f32 = 0.9;
const BUILDING_UPGRADE_SCALES: [f32; 4] = [1.0, 1.15, 1.3, 1.5];
/// Cross-owner spacing ratio relative to `min_spacing_px` so frontline enemy structures stay visible.
const BUILDING_DIFF_OWNER_SPACING_RATIO: f32 = 0.45;
const BUILDING_MARKER_TRANSITION_RATIO: f32 = 0.65;
const BUILDING_CELEBRATION_OFFSETS: [[f32; 2]; 4] =
    [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

#[inline]
pub(crate) fn world_to_screen(world_x: f32, world_y: f32, input: &InputState, sf: f32) -> [f32; 2] {
    let [x, y] = input.world_to_screen([world_x, world_y]);
    let sf = sf.max(0.01);
    [x / sf, y / sf]
}

pub(crate) fn render_overlays(
    text: &mut TextRenderer,
    avatars: &crate::render::avatars::AvatarSlots,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    sf: f32,
    now: Instant,
    placement_can_confirm: bool,
) -> Option<usize> {
    let Some(snapshot) = sim.current_snapshot.as_ref() else {
        return None;
    };
    let sf = sf.max(0.01);
    let zoom_scaled = input.camera_zoom / sf;
    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    let leaderboard_top_three = ui.leaderboard_top_three;
    let mut foreground_start = None;

    DevConfig::with(|dev| {
        super::railways::render(
            text,
            snapshot,
            sim,
            ui,
            input,
            dev,
            sf,
            zoom_scaled,
            now,
        );
        render_building_selection_focus(
            text,
            sim,
            input,
            dev,
            sf,
            ui.app.settings_state.reduced_motion,
            now,
        );
        render_warship_selection_overlays(text, snapshot, sim, ui, input, sf, now);
        if dev.vfx_world_buildings {
            render_buildings(text, snapshot, sim, ui, input, dev, sf, zoom_scaled, now);
        } else {
            ui.hovered_building = None;
            ui.prev_hovered_building = None;
        }
        render_building_placement_preview(text, sim, ui, input, sf, placement_can_confirm);
        foreground_start = Some(text.instance_count());
        let nameplate_render_started_at = crate::diag::startup_perf_timer();
        super::nameplates::render_nameplates(
            text,
            snapshot,
            sim,
            &mut ui.nameplates,
            input,
            dev,
            avatars,
            sf,
            zoom_scaled,
            now,
            my_id,
            leaderboard_top_three,
            ui.tutorial_active
                .then_some(ui.tutorial_marker_player_id)
                .flatten(),
            ui.tutorial_active,
            ui.app.settings_state.reduced_motion,
        );
        if let Some(started_at) = nameplate_render_started_at {
            crate::diag::record_startup_perf(
                crate::diag::StartupMetric::NameplateRender,
                started_at,
            );
        }
        feedback::render(
            text,
            snapshot,
            sim,
            ui,
            input,
            dev,
            avatars,
            sf,
            now,
        );
    });
    super::nameplates::render_emote_bubbles(
        text,
        snapshot,
        &mut ui.nameplates,
        input,
        sf,
        now,
        my_id,
        ui.app.settings_state.reduced_motion,
    );

    if !ui.tutorial_active {
        ui.tutorial_marker_player_id = None;
        ui.tutorial_marker_action = None;
        ui.dialog_border_highlight = None;
    }
    foreground_start
}

#[derive(Clone, Copy, PartialEq)]
struct BuildingLod {
    band: i16,
    quantized_zoom: f32,
    sprite_zoom: f32,
    dot_zoom: f32,
    district_zoom: f32,
    show_districts: bool,
    icon_px: f32,
    dot_px: f32,
    min_spacing_px: f32,
}

#[derive(Clone, Copy)]
struct BuildingLodMetrics {
    /// Visible logical size in CSS pixels.
    size_css: [f32; 2],
    /// Inner silhouette strength (0 = pure dot shape, 1 = tactical icon badge).
    glyph_alpha: f32,
    /// Cross-fade factor (0 = pure SDF marker, 1 = pure world-anchored art sprite).
    sprite_blend: f32,
}

#[inline]
fn smoothstep_lod(edge0: f32, edge1: f32, x: f32) -> f32 {
    let span = edge1 - edge0;
    if span <= 1e-5 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / span).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn building_lod_icon(kind: BuildingKind, level: u8) -> NameplateStatusSprite {
    match kind {
        BuildingKind::City => NameplateStatusSprite::LodCity,
        BuildingKind::Tower if level >= 5 => NameplateStatusSprite::LodSam,
        BuildingKind::Tower => NameplateStatusSprite::LodTower,
        BuildingKind::TradingPost | BuildingKind::Farm => NameplateStatusSprite::LodTradingPost,
        BuildingKind::Port => NameplateStatusSprite::LodPort,
        BuildingKind::MilitaryCamp if level >= 5 => NameplateStatusSprite::LodSilo,
        BuildingKind::MilitaryCamp => NameplateStatusSprite::LodMilitaryCamp,
        BuildingKind::SAM => NameplateStatusSprite::LodSam,
        BuildingKind::MissileSilo => NameplateStatusSprite::LodSilo,
    }
}

#[inline]
fn building_marker_shape(kind: BuildingKind) -> BuildingMarkerShape {
    match kind {
        BuildingKind::City => BuildingMarkerShape::Circle,
        BuildingKind::MissileSilo | BuildingKind::Farm => BuildingMarkerShape::Triangle,
        BuildingKind::SAM => BuildingMarkerShape::Square,
        BuildingKind::Port => BuildingMarkerShape::Pentagon,
        BuildingKind::TradingPost => BuildingMarkerShape::Hexagon,
        BuildingKind::Tower => BuildingMarkerShape::Octagon,
        BuildingKind::MilitaryCamp => BuildingMarkerShape::Diamond,
    }
}

#[inline]
fn building_shape_scale(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::MissileSilo => 1.28,
        BuildingKind::City => 1.22,
        BuildingKind::Port | BuildingKind::TradingPost => 1.08,
        BuildingKind::SAM | BuildingKind::MilitaryCamp => 1.05,
        BuildingKind::Farm => 1.04,
        BuildingKind::Tower => 1.0,
    }
}

#[inline]
fn building_icon_fill(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::MissileSilo => 0.68,
        BuildingKind::MilitaryCamp => 0.72,
        BuildingKind::City => 0.80,
        _ => 0.76,
    }
}

/// Strategic priority for deterministic overlap resolution and painter's order.
/// Higher values win spatial thinning conflicts and render on top.
#[inline]
fn building_priority_rank(kind: BuildingKind) -> u8 {
    match kind {
        BuildingKind::MissileSilo => 7,
        BuildingKind::City => 6,
        BuildingKind::SAM => 5,
        BuildingKind::Port => 4,
        BuildingKind::TradingPost => 3,
        BuildingKind::Farm => 2,
        BuildingKind::Tower => 1,
        BuildingKind::MilitaryCamp => 0,
    }
}

#[inline]
fn building_upgrade_scale(level: u8) -> f32 {
    BUILDING_UPGRADE_SCALES
        .get(usize::from(level.saturating_sub(1)))
        .copied()
        .unwrap_or(1.5)
}

impl BuildingLod {
    /// Cutoffs come from the Dev Tools "Zoom LOD" tab and describe a standard 2×2 building;
    /// larger footprints (4×4 City) keep their detailed art proportionally longer.
    fn for_zoom(
        zoom_scaled: f32,
        params: &[[f32; 4]; sow_render::shader_params::PARAM_CAPACITY],
    ) -> Self {
        let setting = |id| sow_render::shader_params::number_value(params, id);
        let zoom = zoom_scaled.max(BUILDING_CULL_FLOOR);
        let band = (zoom.log2() * BUILDING_ZOOM_STEPS_PER_OCTAVE).floor() as i16;
        let quantized_zoom = 2.0_f32.powf(band as f32 / BUILDING_ZOOM_STEPS_PER_OCTAVE);
        let district_zoom = setting("lod.building.district_zoom").max(0.0);
        Self {
            band,
            quantized_zoom,
            sprite_zoom: setting("lod.building.sprite_zoom").max(2.0),
            dot_zoom: setting("lod.building.dot_zoom").max(0.0),
            district_zoom,
            show_districts: zoom_scaled >= district_zoom,
            icon_px: setting("lod.building.icon_px").max(8.0),
            dot_px: setting("lod.building.dot_px").max(6.0),
            min_spacing_px: setting("lod.building.min_spacing_px").max(0.0),
        }
    }

    fn metrics(
        self,
        kind: BuildingKind,
        level: u8,
        zoom_scaled: f32,
        has_construction_min: bool,
    ) -> BuildingLodMetrics {
        let (width, height) = kind.footprint_dimensions();
        let ref_zoom = width.min(height) as f32 / BUILDING_LOD_REFERENCE_SIDE * zoom_scaled;
        let sprite_zoom = self.sprite_zoom;
        let dot_zoom = self.dot_zoom.min(sprite_zoom * BUILDING_MARKER_TRANSITION_RATIO);
        let marker_zoom = (sprite_zoom * BUILDING_MARKER_TRANSITION_RATIO).max(dot_zoom);
        let shape_scale = building_shape_scale(kind);
        let footprint_css = building_footprint_marker_size(
            kind,
            zoom_scaled,
            if has_construction_min {
                BUILDING_MIN_MARKER_SIZE
            } else {
                0.0
            },
        );
        let upgrade_scale = building_upgrade_scale(level.max(1));
        let sprite_css = [
            footprint_css[0] * upgrade_scale,
            footprint_css[1] * upgrade_scale,
        ];
        let dot_size = self.dot_px * shape_scale;
        let icon_size = self.icon_px.max(self.dot_px) * shape_scale;

        if ref_zoom >= sprite_zoom {
            BuildingLodMetrics {
                size_css: sprite_css,
                glyph_alpha: 0.0,
                sprite_blend: 1.0,
            }
        } else if ref_zoom >= marker_zoom {
            let t = smoothstep_lod(marker_zoom, sprite_zoom, ref_zoom);
            BuildingLodMetrics {
                size_css: [
                    icon_size + (sprite_css[0] - icon_size) * t,
                    icon_size + (sprite_css[1] - icon_size) * t,
                ],
                glyph_alpha: 1.0 - t,
                sprite_blend: t,
            }
        } else if ref_zoom > dot_zoom {
            let t = ((ref_zoom - dot_zoom) / (marker_zoom - dot_zoom).max(1e-4)).clamp(0.0, 1.0);
            let diameter = dot_size + (icon_size - dot_size) * t;
            let glyph_alpha = if dot_zoom <= 0.001 {
                1.0
            } else {
                smoothstep_lod(0.0, 0.22, t)
            };
            BuildingLodMetrics {
                size_css: [diameter, diameter],
                glyph_alpha,
                sprite_blend: 0.0,
            }
        } else {
            BuildingLodMetrics {
                size_css: [dot_size, dot_size],
                glyph_alpha: 0.0,
                sprite_blend: 0.0,
            }
        }
    }

    #[inline]
    fn needs_spatial_thinning(self) -> bool {
        self.min_spacing_px > 0.0 && self.quantized_zoom < self.sprite_zoom
    }
}

#[derive(Clone, Copy)]
struct RenderedBuilding {
    bx: f32,
    by: f32,
    kind: BuildingKind,
    level: u8,
    under_construction: bool,
    keep_local_progress: bool,
    owner_id: u16,
    tile_idx: u32,
    owner_rgb: [f32; 3],
    stack_hint: bool,
}

#[derive(Default)]
pub(crate) struct BuildingRenderCache {
    tick: Option<u64>,
    map_w: u32,
    lod: Option<BuildingLod>,
    player_id: u16,
    fog_of_war: bool,
    selected_tile: Option<u32>,
    buildings: Vec<RenderedBuilding>,
    scratch_candidates: Vec<RenderedBuilding>,
    spatial_heads: HashMap<(i32, i32), usize>,
    spatial_next: Vec<usize>,
    owner_colors: HashMap<u16, [f32; 3]>,
    city_tiles: HashSet<u32>,
    under_construction_tiles: HashSet<u32>,
    flash_tick: Option<u64>,
    flash_tile_totals: HashMap<u32, (u8, u16, BuildingKind, u8)>,
}

impl BuildingRenderCache {
    #[inline]
    fn matches(
        &self,
        tick: u64,
        map_w: u32,
        lod: BuildingLod,
        player_id: u16,
        fog_of_war: bool,
        selected_tile: Option<u32>,
    ) -> bool {
        self.tick == Some(tick)
            && self.map_w == map_w
            && self.lod == Some(lod)
            && self.player_id == player_id
            && self.fog_of_war == fog_of_war
            && self.selected_tile == selected_tile
    }
}

pub(crate) fn construction_progress(
    kind: BuildingKind,
    active_level: u8,
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

    let duration_current =
        sow_core::building::core::upgrade_duration_ticks(kind, active_level.saturating_add(1));
    Some(1.0 - (ticks_until_complete as f32 / duration_current as f32).clamp(0.0, 1.0))
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
        ui.hovered_building = None;
        ui.prev_hovered_building = None;
        return;
    }
    let lod = BuildingLod::for_zoom(zoom_scaled, &dev.shader_params);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let my_id = sim.my_player_id.unwrap_or(0);
    if ui.building_render_cache.flash_tick != Some(snapshot.tick) {
        let tile_totals = &mut ui.building_render_cache.flash_tile_totals;
        tile_totals.clear();
        for building in &snapshot.buildings {
            let active_level = building.active_level();
            let entry = tile_totals.entry(building.tile_idx).or_insert((
                0,
                building.owner_id,
                building.kind,
                active_level,
            ));
            entry.0 = entry.0.saturating_add(active_level);
            entry.1 = building.owner_id;
            if building.kind != BuildingKind::City && active_level > 0 {
                entry.2 = building.kind;
                entry.3 = active_level;
            }
        }
        ui.building_levels_seen
            .retain(|tile_idx, _| tile_totals.contains_key(tile_idx));
        for (&tile_idx, &(total_level, owner_id, celeb_kind, celeb_level)) in tile_totals.iter() {
            if let Some(previous) = ui.building_levels_seen.insert(tile_idx, total_level)
                && total_level > previous
            {
                ui.building_upgrade_flashes.insert(tile_idx, now);
                if owner_id == my_id {
                    ui.building_celebrations
                        .insert(tile_idx, (celeb_kind, celeb_level, now));
                }
            }
        }
        ui.building_render_cache.flash_tick = Some(snapshot.tick);
    }
    ui.building_upgrade_flashes
        .retain(|_, started| now.duration_since(*started) < Duration::from_millis(420));
    ui.building_action_pulses
        .retain(|_, started| now.duration_since(*started) < Duration::from_millis(320));
    ui.building_celebrations
        .retain(|_, (_, _, started)| now.duration_since(*started) < Duration::from_millis(1600));
    let building_upgrade_flashes = std::mem::take(&mut ui.building_upgrade_flashes);
    let building_action_pulses = std::mem::take(&mut ui.building_action_pulses);
    let prev_hovered_building = ui.hovered_building;
    let prev_exiting_hover = ui.prev_hovered_building;
    let selected_menu = input
        .map_context_menu
        .filter(|menu| menu.view == MapContextMenuView::BuildingDetails);
    let selected_tile = selected_menu.map(|menu| menu.tile_idx);
    let desktop_hover_enabled = crate::web_canvas::supports_desktop_hover()
        && matches!(input.hover_pointer, crate::app::HoverPointer::Mouse)
        && !input.is_pointer_gesture_active()
        && input.camera_zoom > 0.0
        && input.last_mouse_x >= 0.0
        && input.last_mouse_y >= 0.0;
    let reduced_motion = ui.app.settings_state.reduced_motion;
    let buildings = cached_buildings(
        ui,
        snapshot,
        sim,
        lod,
        my_id,
        dev.fog_of_war,
        selected_tile,
    );
    let (next_hovered_building, next_exiting_hover) = if desktop_hover_enabled {
        let [wx, wy] = input.screen_to_world([input.last_mouse_x as f32, input.last_mouse_y as f32]);
        let pointer_x = input.last_mouse_x as f32 / sf;
        let pointer_y = input.last_mouse_y as f32 / sf;
        let hovered_tile = pick_owned_building_at_pointer(
            buildings,
            my_id,
            zoom_scaled,
            lod,
            input,
            sf,
            pointer_x,
            pointer_y,
            wx,
            wy,
        );
        let (hovered, mut exiting) = if hovered_tile == prev_hovered_building.map(|(t, _)| t) {
            (prev_hovered_building, prev_exiting_hover)
        } else {
            let exiting = prev_hovered_building
                .map(|(old_tile, _)| (old_tile, now))
                .or(prev_exiting_hover);
            (hovered_tile.map(|tile_idx| (tile_idx, now)), exiting)
        };
        if let Some((exit_tile, exited_at)) = exiting {
            let expired = now.saturating_duration_since(exited_at).as_secs_f32()
                >= crate::render::motion::BUILDING_HOVER_EXIT_SECS;
            if expired
                || Some(exit_tile) == hovered_tile
                || selected_tile == Some(exit_tile)
            {
                exiting = None;
            }
        }
        (hovered, exiting)
    } else {
        (None, None)
    };

    // Desktop-only ghost hover ground rings: emitted before all building sprites so they always
    // sit cleanly underneath building roofs with zero frame lag.
    if desktop_hover_enabled {
        let ghost_entries = [
            next_exiting_hover.map(|(tile, started)| (tile, started, true)),
            next_hovered_building.map(|(tile, started)| (tile, started, false)),
        ];
        for entry in ghost_entries.into_iter().flatten() {
            let (target_tile, started_at, exiting) = entry;
            if selected_tile == Some(target_tile) {
                continue;
            }
            let Some(building) = buildings
                .iter()
                .find(|b| b.owner_id == my_id && b.tile_idx == target_tile)
            else {
                continue;
            };
            let elapsed = now.saturating_duration_since(started_at).as_secs_f32();
            let Some(motion) =
                crate::render::motion::building_hover_ring_motion(elapsed, exiting, reduced_motion)
            else {
                continue;
            };
            let metrics = lod.metrics(
                building.kind,
                building.level,
                zoom_scaled,
                building.keep_local_progress,
            );
            let building_size = [metrics.size_css[0] * sf, metrics.size_css[1] * sf];
            let (ring_center, base_radius) =
                building_ground_ring_geometry(building.bx, building.by, building_size, input, sf);
            if ring_center[0] + base_radius[0] < 0.0
                || ring_center[0] - base_radius[0] > input.screen_w
                || ring_center[1] + base_radius[1] < 0.0
                || ring_center[1] - base_radius[1] > input.screen_h
            {
                continue;
            }
            text.push_selection_ring(
                ring_center,
                [base_radius[0] * motion.scale, base_radius[1] * motion.scale],
                0.0,
                motion.phase,
                motion.pulse,
                motion.acquire,
                [0.38, 0.96, 0.78, motion.alpha],
                [0.90, 1.0, 0.96, 0.72],
            );
        }
    }

    let mut celebration_budget = 24usize;
    let hovered_tile = next_hovered_building.map(|(tile, _)| tile);

    for building in buildings {
        let center = world_to_screen(building.bx, building.by, input, sf);
        let metrics = lod.metrics(
            building.kind,
            building.level,
            zoom_scaled,
            building.keep_local_progress,
        );
        // Conservative CSS-pixel frustum margin (accounts for upgrade bounce and SDF halo).
        let cull_half_w = metrics.size_css[0] * 0.75;
        let cull_half_h = metrics.size_css[1] * 0.75;
        if center[0] + cull_half_w < 0.0
            || center[0] - cull_half_w > screen_w
            || center[1] + cull_half_h < 0.0
            || center[1] - cull_half_h > screen_h
        {
            continue;
        }
        let building_size = [metrics.size_css[0] * sf, metrics.size_css[1] * sf];
        let marker_extent = metrics.size_css[0].max(metrics.size_css[1]);
        let tile_idx = building.tile_idx;
        let is_pure_sprite = metrics.sprite_blend >= 0.999;

        let alpha = if building.under_construction {
            if is_pure_sprite { 0.5 } else { 0.85 }
        } else {
            1.0
        };
        let building_center = [center[0] * sf, center[1] * sf];
        let mut sx = 1.0f32;
        let mut sy = 1.0f32;
        let mut lift_y = 0.0f32;
        let mut sprite_tint = [1.0, 1.0, 1.0, alpha];
        let is_hovered = hovered_tile == Some(tile_idx);
        let is_selected = selected_tile == Some(tile_idx);
        if !reduced_motion {
            // Desktop hover: gentle bubbly lift + subtle brightness on selectable owned buildings.
            if let Some((htile, hstart)) = next_hovered_building
                && htile == tile_idx
            {
                let elapsed = now.saturating_duration_since(hstart).as_secs_f32();
                let t = (elapsed / 0.16).clamp(0.0, 1.0);
                let wobble = (-6.0 * t).exp() * (t * 16.0).sin();
                let s = 1.03 + 0.04 * wobble;
                sx *= s;
                sy *= s;
                lift_y -= 2.2 * sf * t;
                sprite_tint = [1.08, 1.08, 1.04, alpha];
            }
            // Queue 1: Selection — Disney squash & stretch + damped spring wobble.
            if let Some(menu) = selected_menu
                && menu.tile_idx == tile_idx
            {
                let elapsed = now.duration_since(menu.opened_at).as_secs_f32();
                let t = (elapsed / crate::render::motion::BUILDING_SELECTION_TAP_SECS)
                    .clamp(0.0, 1.0);
                let (tap_scale, _) = crate::render::motion::building_selection_tap_motion(
                    elapsed,
                    reduced_motion,
                );
                // Anticipation squash (t < 0.14) -> stretch & damped wobble.
                let env = (-6.5 * t).exp();
                let osc = (t * 22.0 - 0.6).sin();
                let stretch = env * osc * 0.16;
                sx *= tap_scale * (1.0 - stretch * 0.85);
                sy *= tap_scale * (1.0 + stretch);
                lift_y -= (stretch.max(0.0)) * building_size[1] * 0.12;
                sprite_tint = [1.14, 1.12, 1.04, alpha];
            }
            // Queue 2: Upgrade sent — anticipation squash then upward launch impulse.
            if let Some(started) = building_action_pulses.get(&tile_idx) {
                let t = (now.duration_since(*started).as_secs_f32() / 0.32).clamp(0.0, 1.0);
                let env = (-5.5 * t).exp();
                let osc = (t * 18.0 - 0.8).sin();
                let stretch = env * osc * 0.20;
                sx *= 1.0 - stretch * 0.80;
                sy *= 1.0 + stretch;
                lift_y -= (stretch.max(0.0)) * building_size[1] * 0.18;
                if is_pure_sprite {
                    text.push_status_sprite(
                        NameplateStatusSprite::UpgradeSparkle,
                        building_center,
                        1.7 * marker_extent * sf * (0.45 + 0.45 * t),
                        [0.45, 0.92, 1.0, (1.0 - t) * 0.85],
                    );
                }
            }
            // Queue 3: Upgrade completed — celebratory elastic double-bounce + golden flash.
            if let Some(started) = building_upgrade_flashes.get(&tile_idx) {
                let t = (now.duration_since(*started).as_secs_f32() / 0.42).clamp(0.0, 1.0);
                let env = (-5.0 * t).exp();
                let osc = (t * 18.0).sin();
                let bounce = 1.0 + env * osc * 0.24;
                sx *= bounce;
                sy *= bounce;
                lift_y -= (env * osc).max(0.0) * building_size[1] * 0.16;
                let flash = (1.0 - t * 1.4).max(0.0);
                sprite_tint[0] += 0.32 * flash;
                sprite_tint[1] += 0.26 * flash;
                sprite_tint[2] += 0.08 * flash;
            }
        }
        let animated_center = [building_center[0], building_center[1] + lift_y];
        let animated_size = [building_size[0] * sx, building_size[1] * sy];
        // Every kind and level has art in assets/art/buildings (checked by tests).
        if let Some(uv_rect) =
            crate::render::gpu::building_sprite_uv(building.kind, building.level.max(1))
        {
            if is_pure_sprite {
                text.push_building_sprite(animated_center, animated_size, uv_rect, sprite_tint);
            } else {
                let (fill_rgb, border_rgb) = if building.under_construction {
                    (
                        [198.0 / 255.0, 198.0 / 255.0, 198.0 / 255.0],
                        [115.0 / 255.0, 115.0 / 255.0, 115.0 / 255.0],
                    )
                } else {
                    (building.owner_rgb, building.owner_rgb)
                };
                let highlight_width = if is_selected {
                    0.072
                } else if is_hovered {
                    0.052
                } else {
                    0.0
                };
                let icon_uv_rect = building_lod_icon(building.kind, building.level).uv_rect();
                text.push_building_marker(BuildingMarkerInstance {
                    center: animated_center,
                    size: animated_size,
                    uv_rect,
                    icon_uv: [icon_uv_rect[0], icon_uv_rect[2]],
                    fill_color: [fill_rgb[0], fill_rgb[1], fill_rgb[2], alpha],
                    border_color: border_rgb,
                    shape: building_marker_shape(building.kind),
                    glyph_alpha: metrics.glyph_alpha,
                    sprite_blend: metrics.sprite_blend,
                    icon_fill: building_icon_fill(building.kind),
                    stack_hint: building.stack_hint,
                    under_construction: building.under_construction,
                    highlight_width,
                });
            }
        }

        if is_pure_sprite
            && (building.kind == BuildingKind::MissileSilo
                || (building.kind == BuildingKind::MilitaryCamp && building.level >= 5))
        {
            text.push_status_sprite(
                NameplateStatusSprite::Radiation,
                building_center,
                (marker_extent * sf).max(9.0 * sf),
                [1.0, 0.78, 0.32, alpha],
            );
        }

        if is_pure_sprite
            && let Some(started) = building_upgrade_flashes.get(&tile_idx)
        {
            let t = (now.duration_since(*started).as_secs_f32() / 0.3).clamp(0.0, 1.0);
            let flash_alpha = 1.0 - t;
            text.push_status_sprite(
                NameplateStatusSprite::UpgradeSparkle,
                [center[0] * sf, center[1] * sf],
                2.0 * marker_extent * sf * (0.5 + 0.35 * t),
                [1.0, 0.88, 0.46, flash_alpha],
            );
            if building.owner_id == my_id
                && !reduced_motion
                && celebration_budget > 0
            {
                let stride = if crate::web_canvas::is_mobile_pointer() { 2 } else { 1 };
                let count = BUILDING_CELEBRATION_OFFSETS.len().div_ceil(stride);
                if celebration_budget >= count {
                    let radius = marker_extent * sf * (0.3 + 0.55 * t);
                    let sparkle_size = (marker_extent * sf * 0.18).clamp(4.0 * sf, 14.0 * sf);
                    for offset in BUILDING_CELEBRATION_OFFSETS.iter().step_by(stride) {
                        text.push_status_sprite(
                            NameplateStatusSprite::UpgradeSparkle,
                            [
                                center[0] * sf + offset[0] * radius,
                                center[1] * sf + offset[1] * radius,
                            ],
                            2.0 * sparkle_size,
                            [1.0, 0.84, 0.36, flash_alpha * 0.9],
                        );
                    }
                    celebration_budget -= count;
                }
            }
        }
    }
    // Render buildings destroyed by an active nuke or demolition through pre-wave seismic tremor,
    // impact squash-stretch & thermal flash, ballistic UV shard shatter, and structural collapse.
    if dev.vfx_blasts {
        let mut destruction_vfx_budget = 72usize;
        for dying in ui.blast_scene.dying_buildings_for_render(now) {
            let building = dying.building;
            if dev.fog_of_war
                && building.owner_id != my_id
                && !sim.fog_visible.contains(building.tile_idx)
            {
                continue;
            }
            let center = world_to_screen(dying.world_x, dying.world_y, input, sf);
            let min_marker = if building.owner_id == my_id && building.under_construction {
                BUILDING_MIN_MARKER_SIZE
            } else {
                0.0
            };
            let marker_size = building_footprint_marker_size(building.kind, zoom_scaled, min_marker);
            let level = building.active_level().max(1);
            let base_size = building_sprite_size(marker_size, sf, level);
            let margin_w = (base_size[0] / sf) * 1.1 + 18.0;
            let margin_h = (base_size[1] / sf) * 1.1 + 18.0;
            if center[0] + margin_w < 0.0
                || center[0] - margin_w > screen_w
                || center[1] + margin_h < 0.0
                || center[1] - margin_h > screen_h
            {
                continue;
            }
            let Some(uv_rect) = crate::render::gpu::building_sprite_uv(building.kind, level) else {
                continue;
            };
            let base_center = [center[0] * sf, center[1] * sf];
            let extent_px = base_size[0].max(base_size[1]).max(12.0 * sf);
            let phase_ang = dying.seed * std::f32::consts::TAU;

            match dying.phase {
                crate::render::world::blasts::DyingBuildingPhase::PreWave(pre_t) => {
                    let base_alpha = if building.under_construction { 0.5 } else { 1.0 };
                    let (jitter, tint) = if reduced_motion {
                        ([0.0, 0.0], [1.0, 1.0, 1.0, base_alpha])
                    } else {
                        let ramp = pre_t * pre_t;
                        let amp = (1.8 * sf * ramp).min(extent_px * 0.14);
                        let jx = (pre_t * 68.0 + phase_ang).sin() * amp;
                        let jy = (pre_t * 79.0 - phase_ang * 1.3).cos() * amp;
                        (
                            [jx, jy],
                            [
                                1.0 + 0.35 * ramp,
                                1.0 - 0.08 * ramp,
                                1.0 - 0.32 * ramp,
                                base_alpha,
                            ],
                        )
                    };
                    let pre_center = [base_center[0] + jitter[0], base_center[1] + jitter[1]];
                    text.push_building_sprite(
                        pre_center,
                        base_size,
                        uv_rect,
                        tint,
                    );
                    if building.kind == BuildingKind::MissileSilo
                        || (building.kind == BuildingKind::MilitaryCamp && building.level >= 5)
                    {
                        let marker_extent = marker_size[0].max(marker_size[1]);
                        text.push_status_sprite(
                            NameplateStatusSprite::Radiation,
                            pre_center,
                            (marker_extent * sf).max(9.0 * sf),
                            [1.0, 0.78, 0.32, base_alpha],
                        );
                    }
                }
                crate::render::world::blasts::DyingBuildingPhase::Collapsing(ct) => {
                    let dir = dying.blast_dir;
                    let perp = [-dir[1], dir[0]];
                    let is_strategic = matches!(
                        building.kind,
                        BuildingKind::City
                            | BuildingKind::MissileSilo
                            | BuildingKind::MilitaryCamp
                            | BuildingKind::Port
                            | BuildingKind::SAM
                    );

                    // 1. Tactical structural destruction indicator reticle & warning pulse.
                    if !reduced_motion
                        && ct < 0.72
                        && destruction_vfx_budget >= 3
                        && text.remaining_instance_capacity() >= 6
                    {
                        let rt = (ct / 0.72).clamp(0.0, 1.0);
                        let r_ease = 1.0 - (1.0 - rt) * (1.0 - rt);
                        let ring_fade = (1.0 - rt) * (1.0 - rt * 0.35);
                        let ring_r = extent_px * (0.42 + 0.88 * r_ease);
                        let ring_col = if is_strategic {
                            [1.0, 0.28, 0.12, 0.90 * ring_fade]
                        } else {
                            [1.0, 0.56, 0.16, 0.82 * ring_fade]
                        };
                        text.push_ring(
                            base_center,
                            ring_r,
                            ring_col,
                            (2.2 * (1.0 - 0.45 * rt) * sf).max(1.0),
                        );
                        if ct < 0.44 {
                            let cross_t = (ct / 0.44).clamp(0.0, 1.0);
                            text.push_cross(
                                base_center,
                                extent_px * (0.32 + 0.36 * cross_t),
                                [1.0, 0.90, 0.52, (1.0 - cross_t) * 0.88],
                                (1.8 * sf).max(1.0),
                            );
                        }
                        destruction_vfx_budget -= 3;
                    }

                    // 2. Impact squash-stretch (ct < 0.18) -> buckling structural collapse (ct >= 0.18).
                    if ct < 0.18 {
                        let u = (ct / 0.18).clamp(0.0, 1.0);
                        let (sx, sy, lift) = if reduced_motion {
                            (1.0 + 0.12 * u, 1.0 + 0.12 * u, 0.0)
                        } else if u < 0.42 {
                            let s = u / 0.42;
                            (1.0 + 0.34 * s, 1.0 - 0.28 * s, 0.08 * extent_px * s)
                        } else {
                            let s = (u - 0.42) / 0.58;
                            (
                                1.34 - 0.46 * s,
                                0.72 + 0.52 * s,
                                (0.08 - 0.24 * s) * extent_px,
                            )
                        };
                        let kick = if reduced_motion {
                            0.0
                        } else {
                            extent_px * 0.22 * u
                        };
                        let pos = [
                            base_center[0] + dir[0] * kick,
                            base_center[1] + dir[1] * kick + lift,
                        ];
                        text.push_building_sprite(
                            pos,
                            [base_size[0] * sx, base_size[1] * sy],
                            uv_rect,
                            [1.45, 1.24 - 0.22 * u, 0.92 - 0.45 * u, 1.0],
                        );
                    } else {
                        let p = ((ct - 0.18) / 0.82).clamp(0.0, 1.0);
                        let p_ease = 1.0 - (1.0 - p) * (1.0 - p);
                        let core_alpha = (1.0 - p * p).clamp(0.0, 1.0);

                        // Buckling foundation pancake collapse + charring from molten orange to soot.
                        let sx = if reduced_motion {
                            1.0 - 0.25 * p
                        } else {
                            0.92 + 0.26 * p_ease
                        };
                        let sy = if reduced_motion {
                            1.0 - 0.25 * p
                        } else {
                            1.18 * (1.0 - 0.70 * p * p)
                        };
                        let drift = if reduced_motion {
                            0.0
                        } else {
                            extent_px * (0.22 + 0.26 * p_ease)
                        };
                        let sink = if reduced_motion {
                            0.0
                        } else {
                            base_size[1] * 0.34 * (p * p)
                        };
                        let vibration = if reduced_motion {
                            0.0
                        } else {
                            (p * 42.0 + phase_ang).sin() * (1.0 - p) * 1.4 * sf
                        };
                        let char_mix = (p * 1.35).clamp(0.0, 1.0);
                        let core_tint = [
                            1.25 * (1.0 - char_mix) + 0.15 * char_mix,
                            0.44 * (1.0 - char_mix) + 0.12 * char_mix,
                            0.10 * (1.0 - char_mix) + 0.10 * char_mix,
                            core_alpha,
                        ];
                        text.push_building_sprite(
                            [
                                base_center[0] + dir[0] * drift + perp[0] * vibration,
                                base_center[1] + dir[1] * drift + sink,
                            ],
                            [base_size[0] * sx, base_size[1] * sy],
                            uv_rect,
                            core_tint,
                        );

                        // 3. Ballistic UV-split structural shatter shards (zero extra textures/passes).
                        if !reduced_motion
                            && extent_px >= 13.0 * sf
                            && destruction_vfx_budget >= 3
                            && text.remaining_instance_capacity() >= 4
                        {
                            let [u0, v0, u1, v1] = uv_rect;
                            let du = u1 - u0;
                            let dv = v1 - v0;
                            let shards: [([f32; 4], [f32; 2], f32, f32); 3] = [
                                // Upper-left roof/spire shard
                                ([0.0, 0.0, 0.55, 0.56], [-0.85, -0.55], 0.62, 0.44),
                                // Upper-right wall/tower shard
                                ([0.45, 0.0, 1.0, 0.56], [0.90, -0.45], 0.58, 0.52),
                                // Center superstructure chunk
                                ([0.16, 0.36, 0.84, 0.92], [0.15, 0.75], 0.52, 0.36),
                            ];
                            let shard_fade = (1.0 - p).powi(2);
                            for (i, (sub, spread, vel_scale, loft_scale)) in
                                shards.iter().enumerate()
                            {
                                let shard_uv = [
                                    u0 + du * sub[0],
                                    v0 + dv * sub[1],
                                    u0 + du * sub[2],
                                    v0 + dv * sub[3],
                                ];
                                let phase_sign = if (i as f32 + dying.seed * 7.0) as i32 % 2 == 0 {
                                    1.0
                                } else {
                                    -1.0
                                };
                                let vx = dir[0] * vel_scale + perp[0] * spread[0] * 0.52 * phase_sign;
                                let vy = dir[1] * vel_scale + perp[1] * spread[1] * 0.45;
                                let travel = extent_px * 1.15 * p_ease;
                                // Ballistic parabola: initial upward loft followed by gravity drop.
                                let gravity_y =
                                    extent_px * (-loft_scale * p + 0.95 * loft_scale * p * p);
                                let shrink = (1.0 - 0.48 * p).max(0.25);
                                let shard_w = base_size[0] * (sub[2] - sub[0]) * shrink;
                                let shard_h = base_size[1] * (sub[3] - sub[1]) * shrink;
                                let heat = (1.0 - p * 1.25).clamp(0.0, 1.0);
                                let shard_tint = [
                                    0.22 + 1.25 * heat,
                                    0.16 + 0.68 * heat * heat,
                                    0.14 + 0.25 * heat * heat,
                                    shard_fade * 0.94,
                                ];
                                text.push_building_sprite(
                                    [
                                        base_center[0] + vx * travel,
                                        base_center[1] + vy * travel + gravity_y,
                                    ],
                                    [shard_w, shard_h],
                                    shard_uv,
                                    shard_tint,
                                );
                            }
                            destruction_vfx_budget -= 3;
                        }
                    }
                }
            }
        }
    }
    ui.building_upgrade_flashes = building_upgrade_flashes;
    ui.building_action_pulses = building_action_pulses;
    ui.hovered_building = next_hovered_building;
    ui.prev_hovered_building = next_exiting_hover;
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

    let (lod, fog_of_war) = DevConfig::with(|dev| {
        (
            BuildingLod::for_zoom(zoom_scaled, &dev.shader_params),
            dev.fog_of_war,
        )
    });
    let selected_tile = input
        .map_context_menu
        .filter(|menu| menu.view == MapContextMenuView::BuildingDetails)
        .map(|menu| menu.tile_idx);
    let buildings = cached_buildings(
        ui,
        snapshot,
        sim,
        lod,
        my_id,
        fog_of_war,
        selected_tile,
    );
    let pointer_x = x as f32 / sf;
    let pointer_y = y as f32 / sf;
    let [pointer_world_x, pointer_world_y] = input.screen_to_world([x as f32, y as f32]);
    pick_owned_building_at_pointer(
        buildings,
        my_id,
        zoom_scaled,
        lod,
        input,
        sf,
        pointer_x,
        pointer_y,
        pointer_world_x,
        pointer_world_y,
    )
}

/// WYSIWYG hit-test matching the exact painter's order (reverse scan = topmost visible icon wins).
#[allow(clippy::too_many_arguments)]
fn pick_owned_building_at_pointer(
    buildings: &[RenderedBuilding],
    my_id: u16,
    zoom_scaled: f32,
    lod: BuildingLod,
    input: &InputState,
    sf: f32,
    pointer_x: f32,
    pointer_y: f32,
    pointer_world_x: f32,
    pointer_world_y: f32,
) -> Option<u32> {
    for building in buildings.iter().rev() {
        if building.owner_id != my_id {
            continue;
        }
        let metrics = lod.metrics(
            building.kind,
            building.level,
            zoom_scaled,
            building.keep_local_progress,
        );
        let (width, height) = building.kind.footprint_dimensions();
        let in_footprint = pointer_world_x >= building.bx - width as f32 * 0.5
            && pointer_world_x < building.bx + width as f32 * 0.5
            && pointer_world_y >= building.by - height as f32 * 0.5
            && pointer_world_y < building.by + height as f32 * 0.5;
        if in_footprint {
            return Some(building.tile_idx);
        }
        if metrics.sprite_blend < 0.999 {
            let center = world_to_screen(building.bx, building.by, input, sf);
            let dx = pointer_x - center[0];
            let dy = pointer_y - center[1];
            let visual_extent = metrics.size_css[0].max(metrics.size_css[1]);
            let hit_radius = (visual_extent * 0.5).max(12.0);
            if dx * dx + dy * dy <= hit_radius * hit_radius {
                return Some(building.tile_idx);
            }
        }
    }
    None
}

pub(crate) fn warship_at_pointer(
    snapshot: &SimSnapshot,
    sim: &SimState,
    input: &InputState,
    x: f64,
    y: f64,
) -> Option<u64> {
    let my_id = sim.my_player_id.unwrap_or(0);
    let sf = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    if my_id == 0 || sim.map_w == 0 || !input.camera_zoom.is_finite() || input.camera_zoom <= 0.0 {
        return None;
    }
    let pointer_x = x as f32 / sf;
    let pointer_y = y as f32 / sf;
    let map_w = sim.map_w as usize;
    let hit_radius_sq = 24.0_f32 * 24.0_f32;
    let mut closest: Option<(f32, u64)> = None;

    for fleet in &snapshot.fleets {
        if fleet.owner_id != my_id
            || fleet.unit_type != sow_core::game::UnitType::Warship
            || fleet.troops <= 0.0
        {
            continue;
        }
        let tile = fleet.current_tile as usize;
        let wx = (tile % map_w) as f32 + 0.5;
        let wy = (tile / map_w) as f32 + 0.5;
        let center = world_to_screen(wx, wy, input, sf);
        let dx = pointer_x - center[0];
        let dy = pointer_y - center[1];
        let distance_sq = dx * dx + dy * dy;
        if distance_sq > hit_radius_sq {
            continue;
        }
        if closest.is_none_or(|(best_dist, best_id)| {
            distance_sq < best_dist || (distance_sq == best_dist && fleet.id < best_id)
        }) {
            closest = Some((distance_sq, fleet.id));
        }
    }
    closest.map(|(_, id)| id)
}

fn render_warship_selection_overlays(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &UiState,
    input: &InputState,
    sf: f32,
    now: Instant,
) {
    let selected_ids = &ui.app.hud_state.selected_warship_ids;
    if selected_ids.is_empty()
        || sim.map_w == 0
        || !input.camera_zoom.is_finite()
        || input.camera_zoom <= 0.0
    {
        return;
    }
    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    let zoom = input.camera_zoom;
    let map_w = sim.map_w as usize;
    let elapsed = now.elapsed().as_secs_f32();
    let motion = crate::render::motion::building_selection_ring_motion(
        elapsed,
        ui.app.settings_state.reduced_motion,
    );

    for fleet in &snapshot.fleets {
        if fleet.owner_id != my_id
            || fleet.unit_type != sow_core::game::UnitType::Warship
            || fleet.troops <= 0.0
            || !selected_ids.contains(&fleet.id)
        {
            continue;
        }
        let tile = fleet.current_tile as usize;
        let wx = (tile % map_w) as f32 + 0.5;
        let wy = (tile / map_w) as f32 + 0.5;
        let center = input.world_to_screen([wx, wy]);
        let mode = fleet
            .warship
            .as_ref()
            .map_or(sow_core::warp_fleet::WarshipMode::Patrolling, |w| w.mode);
        let (ring_color, core_glow) = match mode {
            sow_core::warp_fleet::WarshipMode::Patrolling => (
                [0.24, 0.96, 0.58, motion.alpha],
                [0.88, 1.0, 0.94, 0.92],
            ),
            sow_core::warp_fleet::WarshipMode::Holding => (
                [0.35, 0.85, 0.98, motion.alpha],
                [0.90, 0.98, 1.0, 0.92],
            ),
            sow_core::warp_fleet::WarshipMode::Retreating
            | sow_core::warp_fleet::WarshipMode::Docked => (
                [0.96, 0.80, 0.34, motion.alpha],
                [1.0, 0.96, 0.85, 0.92],
            ),
        };
        let r = (14.0 * sf).max(zoom * 1.1);
        text.push_selection_ring(
            center,
            [r * motion.scale, r * 0.68 * motion.scale],
            1.0,
            motion.phase,
            motion.pulse,
            motion.acquire,
            ring_color,
            core_glow,
        );

        if mode == sow_core::warp_fleet::WarshipMode::Patrolling
            && let Some(ws) = fleet.warship.as_ref()
        {
            let p_tile = ws.patrol_tile as usize;
            let px = (p_tile % map_w) as f32 + 0.5;
            let py = (p_tile / map_w) as f32 + 0.5;
            let patrol_center = input.world_to_screen([px, py]);
            text.push_ring(
                patrol_center,
                (18.0 * zoom).max(16.0 * sf),
                [0.24, 0.96, 0.58, 0.22],
                (1.2 * sf).max(1.0),
            );
        }

        if fleet.path_cursor < fleet.path.len()
            && let Some(&dst_tile) = fleet.path.last()
        {
            let d_tile = dst_tile as usize;
            let dx = (d_tile % map_w) as f32 + 0.5;
            let dy = (d_tile / map_w) as f32 + 0.5;
            let dst_center = input.world_to_screen([dx, dy]);
            text.push_ring(
                dst_center,
                (8.0 * sf).max(zoom * 0.7),
                [0.96, 0.83, 0.47, 0.75],
                (1.6 * sf).max(1.0),
            );
        }
    }

    if ui.app.hud_state.warship_move_targeting {
        let px = input.last_mouse_x as f32;
        let py = input.last_mouse_y as f32;
        text.push_ring(
            [px, py],
            (14.0 * sf).max(10.0),
            [0.96, 0.83, 0.47, 0.85],
            (1.8 * sf).max(1.0),
        );
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
    let scale = building_upgrade_scale(level);
    marker_size.map(|size| size * sf * scale)
}

#[inline]
pub(crate) fn building_world_center(kind: BuildingKind, tile_idx: u32, map_w: u32) -> [f32; 2] {
    let map_w = map_w.max(1);
    let footprint =
        sow_core::building::BuildingFootprint::at(kind, tile_idx % map_w, tile_idx / map_w);
    [
        footprint.left as f32 + footprint.width as f32 * 0.5,
        footprint.top as f32 + footprint.height as f32 * 0.5,
    ]
}

pub(crate) fn building_screen_anchor_and_radius(
    kind: BuildingKind,
    tile_idx: u32,
    map_w: u32,
    input: &InputState,
    sf: f32,
) -> ([f32; 2], f32) {
    let sf = sf.max(0.01);
    let [bx, by] = building_world_center(kind, tile_idx, map_w);
    let center = input.world_to_screen([bx, by]);
    let (_, height) = kind.footprint_dimensions();
    let half_h_world = height as f32 * 0.5;
    let top_y = input.world_to_screen([bx, by - half_h_world])[1];
    let bottom_y = input.world_to_screen([bx, by + half_h_world])[1];
    let max_radius = (input.screen_h / sf * 0.24).max(48.0);
    let radius = ((bottom_y - top_y) * 0.5 / sf).clamp(24.0, max_radius);
    ([center[0] / sf, center[1] / sf], radius)
}

pub(crate) fn building_screen_anchor_bottom(
    bx: f32,
    by: f32,
    kind: BuildingKind,
    level: u8,
    input: &InputState,
    sf: f32,
) -> [f32; 2] {
    let center = world_to_screen(bx, by, input, sf);
    let zoom_scaled = input.camera_zoom / sf;
    let lod = DevConfig::with(|dev| BuildingLod::for_zoom(zoom_scaled, &dev.shader_params));
    let metrics = lod.metrics(kind, level.max(1), zoom_scaled, true);
    let bottom_y = center[1] + metrics.size_css[1] * 0.5 + 6.0;
    [center[0], bottom_y]
}

/// Ground-plane selection ring center and elliptical radii `[rx, ry]` in physical px.
/// Adapts automatically to tabletop perspective foreshortening and building upgrade scale.
fn building_ground_ring_geometry(
    bx: f32,
    by: f32,
    building_size: [f32; 2],
    input: &InputState,
    sf: f32,
) -> ([f32; 2], [f32; 2]) {
    let center = input.world_to_screen([bx, by]);
    let top_y = input.world_to_screen([bx, by - 0.5])[1];
    let bottom_y = input.world_to_screen([bx, by + 0.5])[1];
    let y_aspect = if input.camera_zoom > 0.001 {
        ((bottom_y - top_y) / input.camera_zoom).clamp(0.48, 1.0)
    } else {
        1.0
    };
    // Primary luminous ring lies at `d = 0.70` inside the SDF quad; frame the building's base.
    let visual_extent = building_size[0].max(building_size[1]);
    let main_radius = (visual_extent * 0.64).max(15.0 * sf);
    let quad_rx = main_radius / 0.70;
    let quad_ry = quad_rx * y_aspect;
    let base_shift_y = building_size[1] * 0.08 * (1.0 - y_aspect).max(0.0);
    ([center[0], center[1] + base_shift_y], [quad_rx, quad_ry])
}

fn render_building_selection_focus(
    text: &mut TextRenderer,
    sim: &SimState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    reduced_motion: bool,
    now: Instant,
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
    let zoom_scaled = zoom / sf;
    let tile_x = menu.tile_idx % sim.map_w;
    let tile_y = menu.tile_idx / sim.map_w;
    let footprint = sow_core::building::BuildingFootprint::at(kind, tile_x, tile_y);
    let bx = footprint.left as f32 + footprint.width as f32 * 0.5;
    let by = footprint.top as f32 + footprint.height as f32 * 0.5;
    let anchor_center = input.world_to_screen([bx, by]);

    let snapshot_buildings = sim
        .current_snapshot
        .as_ref()
        .map(|s| s.buildings.as_slice())
        .unwrap_or(&[]);
    let live_building = snapshot_buildings
        .iter()
        .find(|b| b.tile_idx == menu.tile_idx && b.kind == kind)
        .or_else(|| snapshot_buildings.iter().find(|b| b.tile_idx == menu.tile_idx));
    let level = live_building
        .map(|b| b.active_level().max(1))
        .unwrap_or_else(|| menu.building_level.max(1));
    let under_construction = live_building
        .map(|b| b.under_construction)
        .unwrap_or(menu.building_under_construction);

    let lod = BuildingLod::for_zoom(zoom_scaled, &dev.shader_params);
    let metrics = lod.metrics(kind, level, zoom_scaled, under_construction);
    let building_size = [metrics.size_css[0] * sf, metrics.size_css[1] * sf];
    let (ring_center, base_radius) =
        building_ground_ring_geometry(bx, by, building_size, input, sf);
    let elapsed = now.saturating_duration_since(menu.opened_at).as_secs_f32();
    let (tap_scale, tap_glow) =
        crate::render::motion::building_selection_tap_motion(elapsed, reduced_motion);
    let motion = crate::render::motion::building_selection_ring_motion(elapsed, reduced_motion);
    let (ring_color, core_glow) = if under_construction {
        (
            [
                0.28 - 0.22 * tap_glow,
                0.92 + 0.08 * tap_glow,
                0.92 - 0.54 * tap_glow,
                motion.alpha,
            ],
            [
                0.90 - 0.42 * tap_glow,
                0.99 + 0.01 * tap_glow,
                1.0 - 0.36 * tap_glow,
                0.88 + 0.12 * tap_glow,
            ],
        )
    } else {
        (
            [
                0.22 - 0.16 * tap_glow,
                0.98 + 0.02 * tap_glow,
                0.56 - 0.20 * tap_glow,
                motion.alpha,
            ],
            [
                0.88 - 0.36 * tap_glow,
                1.0,
                0.94 - 0.32 * tap_glow,
                0.92 + 0.08 * tap_glow,
            ],
        )
    };
    text.push_selection_ring(
        ring_center,
        [base_radius[0] * tap_scale, base_radius[1] * tap_scale],
        1.0,
        motion.phase,
        motion.pulse,
        motion.acquire,
        ring_color,
        core_glow,
    );

    if kind == BuildingKind::City {
        let city_radius =
            sow_core::building::city_construction_radius(level.max(1), &sim.config) as f32 * zoom;
        text.push_ring(
            anchor_center,
            city_radius,
            [0.32, 0.88, 0.68, 0.34],
            (1.6 * sf).max(1.0),
        );
    }

    let tower_level = snapshot_buildings
        .iter()
        .find(|b| b.tile_idx == menu.tile_idx && b.kind == BuildingKind::Tower)
        .map(|b| b.active_level())
        .unwrap_or_else(|| {
            if kind == BuildingKind::Tower && !menu.building_under_construction {
                menu.building_level
            } else {
                0
            }
        });
    if tower_level > 0 {
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
        let range =
            sow_core::building::tower_range_for_level(tower_level, &sim.config, range_multiplier);
        text.push_ring(
            anchor_center,
            range as f32 * zoom,
            [0.91, 0.71, 0.34, 0.30],
            (1.4 * sf).max(1.0),
        );
        if tower_level >= 5 {
            text.push_ring(
                anchor_center,
                range as f32 * zoom,
                [0.35, 0.85, 0.98, 0.35],
                (1.6 * sf).max(1.0),
            );
        }
    }
    let sam_level = snapshot_buildings
        .iter()
        .find(|building| {
            building.tile_idx == menu.tile_idx && building.kind == BuildingKind::SAM
        })
        .map_or(0, |building| building.active_level());
    if sam_level > 0 {
        let sam_radius = sow_core::execution::sam::sam_range(sam_level) * zoom;
        text.push_ring(
            anchor_center,
            sam_radius,
            [0.35, 0.85, 0.98, 0.35],
            (1.6 * sf).max(1.0),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PlacementPreviewColors {
    fill: Option<[f32; 4]>,
    line: [f32; 4],
    building: [f32; 4],
}

const PLACEMENT_CONFLICT_RADIUS: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
struct PlacementFootprintGeometry {
    top_left: [f32; 2],
    center: [f32; 2],
    size: [f32; 2],
}

fn placement_footprint_geometry(
    footprint: sow_core::building::BuildingFootprint,
    input: &InputState,
) -> PlacementFootprintGeometry {
    let left = footprint.left as f32;
    let top = footprint.top as f32;
    let center_world = [left + footprint.width as f32 * 0.5, top + footprint.height as f32 * 0.5];
    let center = input.world_to_screen(center_world);
    let top_y = input.world_to_screen([center_world[0], top])[1];
    let bottom_y = input.world_to_screen([center_world[0], top + footprint.height as f32])[1];
    PlacementFootprintGeometry {
        top_left: [input.camera_x + left * input.camera_zoom, top_y],
        center,
        size: [footprint.width as f32 * input.camera_zoom, bottom_y - top_y],
    }
}

fn placement_preview_colors(site_valid: bool, can_confirm: bool) -> PlacementPreviewColors {
    if !site_valid {
        PlacementPreviewColors {
            fill: None,
            line: [1.0, 0.55, 0.48, 1.0],
            building: [1.0, 0.28, 0.28, 0.94],
        }
    } else if can_confirm {
        PlacementPreviewColors {
            fill: Some([0.035, 0.26, 0.06, 1.0]),
            line: [0.66, 0.97, 0.45, 1.0],
            building: [1.0, 1.0, 1.0, 0.94],
        }
    } else {
        PlacementPreviewColors {
            fill: Some([0.32, 0.36, 0.38, 1.0]),
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
    fill: Option<[f32; 4]>,
) {
    if let Some(fill) = fill {
        text.push_rect(top_left, size, fill);
    }
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

fn render_placement_conflicts(
    text: &mut TextRenderer,
    map: &sow_core::map::GameMap,
    building_grid: &sow_core::building::BuildingGrid,
    kind: BuildingKind,
    owner_id: u16,
    anchor: [u32; 2],
    input: &InputState,
    sf: f32,
) {
    let [tile_x, tile_y] = anchor;
    let radius = PLACEMENT_CONFLICT_RADIUS;
    let zoom = input.camera_zoom;
    // The widest existing footprint is a City. Its anchor can be outside a blocked cell.
    let occupancy_radius = BuildingKind::City.footprint_dimensions().0 / 2;
    for y in tile_y.saturating_sub(radius)..=tile_y.saturating_add(radius).min(map.height - 1) {
        for x in tile_x.saturating_sub(radius)..=tile_x.saturating_add(radius).min(map.width - 1) {
            let terrain = map.terrain[map.ref_id(x, y)].as_byte();
            if sow_core::building::validate_building_cell(kind, owner_id, map.owner_id(x, y), terrain)
                .is_ok()
                && !building_grid
                    .iter_all_in_range(x, y, occupancy_radius)
                    .any(|building| {
                        sow_core::building::BuildingFootprint::at(
                            building.kind,
                            building.x,
                            building.y,
                        )
                        .contains(x as i32, y as i32)
                    })
            {
                continue;
            }

            let distance = tile_x.abs_diff(x).max(tile_y.abs_diff(y));
            let strength = 1.0 - 0.20 * distance as f32;
            let fill = [0.58, 0.045, 0.06, 0.36 * strength];
            let top_left = input.world_to_screen([x as f32, y as f32]);
            if zoom >= 6.0 * sf {
                text.push_rounded_rect(
                    [top_left[0] + zoom * 0.5, top_left[1] + zoom * 0.5],
                    [zoom - 1.25 * sf; 2],
                    0.08,
                    fill,
                    [1.0, 0.29, 0.25, 0.55 * strength],
                    sf,
                );
            } else {
                text.push_rect(top_left, [zoom; 2], fill);
            }
        }
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
    let Some(engine) = sim.engine.as_ref() else {
        return;
    };
    let map = &engine.state.map;
    let zoom = input.camera_zoom;
    if !zoom.is_finite()
        || zoom <= 0.0
        || placement.tile_idx >= map.width.saturating_mul(map.height)
    {
        return;
    }
    let my_id = sim.my_player_id.unwrap_or(0);
    let cities = if engine.legacy_building_rules {
        Vec::new()
    } else {
        sow_core::building::city_anchors_from_buildings(&engine.buildings, map.width)
    };
    let site_legal = if engine.legacy_building_rules {
        sow_core::building::validate_structure_site(
            map,
            my_id,
            kind,
            placement.tile_idx,
            &engine.building_grid,
        )
        .is_ok()
    } else {
        sow_core::building::validate_match_structure_site(
            map,
            my_id,
            kind,
            placement.tile_idx,
            &engine.building_grid,
            &cities,
            placement.city_id,
            &engine.state.config,
        )
        .is_ok()
    };
    let (tile_x, tile_y) = (
        placement.tile_idx % map.width,
        placement.tile_idx / map.width,
    );
    let visible_bounds = crate::input::placement::visible_building_anchor_bounds(
        kind,
        map.width,
        map.height,
        input,
    );
    let target_visible = visible_bounds.is_some_and(|(min_x, min_y, max_x, max_y)| {
        (min_x..=max_x).contains(&(tile_x as i32)) && (min_y..=max_y).contains(&(tile_y as i32))
    });
    let site_valid = site_legal && target_visible;
    let footprint = sow_core::building::BuildingFootprint::at(kind, tile_x, tile_y);
    let geometry = placement_footprint_geometry(footprint, input);
    let center_px = geometry.center;
    let footprint_size_px = geometry.size;
    let footprint_top_left = geometry.top_left;
    let colors = placement_preview_colors(site_valid, can_confirm);
    let mut metrics = placement_grid_metrics(zoom, sf);
    metrics.draw_inner_grid &= site_valid;
    render_building_footprint_fill(text, footprint_top_left, footprint_size_px, colors.fill);
    render_placement_conflicts(
        text,
        map,
        &engine.building_grid,
        kind,
        my_id,
        [tile_x, tile_y],
        input,
        sf,
    );
    render_building_footprint_grid(
        text,
        footprint,
        footprint_top_left,
        footprint_size_px,
        zoom,
        metrics,
        colors.line,
    );
    let dragging = input.building_drag.is_some();
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
    if kind == BuildingKind::City {
        let initial_radius =
            sow_core::building::city_construction_radius(1, &sim.config) as f32 * input.camera_zoom;
        text.push_ring(
            center_px,
            initial_radius,
            [0.42, 0.92, 0.72, 0.32],
            (1.5 * sf).max(1.0),
        );
        for city in &cities {
            let excl = sow_core::building::city_exclusion_radius(
                city.active_level.max(city.level).max(1),
                &sim.config,
            );
            let dx = tile_x.abs_diff(city.x);
            let dy = tile_y.abs_diff(city.y);
            if dx * dx + dy * dy < excl * excl {
                let c_screen = input.world_to_screen([city.x as f32 + 0.5, city.y as f32 + 0.5]);
                text.push_ring(
                    c_screen,
                    excl as f32 * input.camera_zoom,
                    [0.95, 0.28, 0.28, 0.34],
                    (1.5 * sf).max(1.0),
                );
            }
        }
    } else if !engine.legacy_building_rules {
        let anchor_city = placement.city_id.and_then(|cid| {
            cities.iter().find(|c| c.id == cid && c.owner_id == my_id)
        }).or_else(|| {
            cities.iter().find(|c| {
                c.owner_id == my_id
                    && sow_core::building::city_covers_coord(**c, tile_x, tile_y, &sim.config)
            })
        });
        if let Some(city) = anchor_city {
            let c_screen = input.world_to_screen([city.x as f32 + 0.5, city.y as f32 + 0.5]);
            let range =
                sow_core::building::city_construction_radius(city.active_level, &sim.config) as f32
                    * input.camera_zoom;
            text.push_ring(
                c_screen,
                range,
                [0.32, 0.88, 0.68, 0.34],
                (1.6 * sf).max(1.0),
            );
        }
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
    } else if kind == BuildingKind::SAM {
        text.push_ring(
            center_px,
            sow_core::execution::sam::sam_range(1) * input.camera_zoom,
            [0.35, 0.85, 0.98, 0.35],
            (1.6 * sf).max(1.0),
        );
    }
}

fn cached_buildings<'a>(
    ui: &'a mut UiState,
    snapshot: &SimSnapshot,
    sim: &SimState,
    lod: BuildingLod,
    my_id: u16,
    fog_of_war: bool,
    selected_tile: Option<u32>,
) -> &'a [RenderedBuilding] {
    let map_w = sim.map_w.max(1);
    let cache = &mut ui.building_render_cache;
    if !cache.matches(snapshot.tick, map_w, lod, my_id, fog_of_war, selected_tile) {
        collect_buildings(
            snapshot,
            map_w,
            lod,
            my_id,
            fog_of_war,
            &sim.fog_visible,
            selected_tile,
            cache,
        );
        cache.tick = Some(snapshot.tick);
        cache.map_w = map_w;
        cache.lod = Some(lod);
        cache.player_id = my_id;
        cache.fog_of_war = fog_of_war;
        cache.selected_tile = selected_tile;
    }
    &cache.buildings
}

#[inline]
fn building_sort_key(
    b: &RenderedBuilding,
    my_id: u16,
    selected_tile: Option<u32>,
) -> (bool, bool, u8, u8, bool, std::cmp::Reverse<u32>) {
    (
        selected_tile == Some(b.tile_idx),
        b.keep_local_progress,
        building_priority_rank(b.kind),
        b.level,
        b.owner_id == my_id,
        std::cmp::Reverse(b.tile_idx),
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_buildings(
    snapshot: &SimSnapshot,
    map_w: u32,
    lod: BuildingLod,
    my_id: u16,
    fog_of_war: bool,
    fog_visible: &sow_core::bitset::DenseBitSet,
    selected_tile: Option<u32>,
    cache: &mut BuildingRenderCache,
) {
    const FALLBACK_OWNER_RGB: [f32; 3] = [0.62, 0.64, 0.68];

    let map_w = map_w.max(1);
    cache.buildings.clear();
    cache.scratch_candidates.clear();
    cache.city_tiles.clear();
    cache.under_construction_tiles.clear();
    cache.owner_colors.clear();

    for player in &snapshot.players {
        let c = super::nameplates::player_color(player);
        cache.owner_colors.insert(player.id, [c[0], c[1], c[2]]);
    }

    for building in &snapshot.buildings {
        if building.kind == BuildingKind::City {
            cache.city_tiles.insert(building.tile_idx);
        }
        if building.under_construction {
            cache.under_construction_tiles.insert(building.tile_idx);
        }
    }

    for building in &snapshot.buildings {
        if building.kind != BuildingKind::City && cache.city_tiles.contains(&building.tile_idx) {
            continue;
        }
        // Filter Fog of War BEFORE spatial thinning so hidden enemy structures never suppress
        // visible nearby structures.
        if fog_of_war
            && building.owner_id != my_id
            && !fog_visible.contains(building.tile_idx)
        {
            continue;
        }
        let active_level = building.active_level();
        let has_active_city_build = building.kind == BuildingKind::City
            && (building.under_construction
                || building.arsenal_build.is_some()
                || cache.under_construction_tiles.contains(&building.tile_idx));
        let keep_local_progress = building.owner_id == my_id
            && (building.under_construction || has_active_city_build);
        if !lod.show_districts
            && building.kind.is_non_city()
            && selected_tile != Some(building.tile_idx)
            && !keep_local_progress
        {
            continue;
        }
        let [bx, by] = building_world_center(building.kind, building.tile_idx, map_w);
        let owner_rgb = cache
            .owner_colors
            .get(&building.owner_id)
            .copied()
            .unwrap_or(FALLBACK_OWNER_RGB);
        cache.scratch_candidates.push(RenderedBuilding {
            bx,
            by,
            kind: building.kind,
            level: active_level,
            under_construction: building.under_construction,
            keep_local_progress,
            owner_id: building.owner_id,
            tile_idx: building.tile_idx,
            owner_rgb,
            stack_hint: false,
        });
    }

    // Sort descending by strategic priority: highest-priority buildings claim spatial slots first
    // and end up at the back of `cache.buildings` after reversal so they paint on top.
    cache.scratch_candidates.sort_unstable_by(|a, b| {
        building_sort_key(b, my_id, selected_tile).cmp(&building_sort_key(a, my_id, selected_tile))
    });

    if !lod.needs_spatial_thinning() {
        cache.buildings.extend(cache.scratch_candidates.drain(..));
        // Close zoom: sort back-to-front by Y for natural isometric overlap, keeping selected last.
        cache.buildings.sort_unstable_by(|a, b| {
            (selected_tile == Some(a.tile_idx))
                .cmp(&(selected_tile == Some(b.tile_idx)))
                .then_with(|| a.by.partial_cmp(&b.by).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.bx.partial_cmp(&b.bx).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.tile_idx.cmp(&b.tile_idx))
        });
        return;
    }

    // World-anchored spatial-hash thinning (3×3 neighborhood Euclidean distance check).
    // Uses an intrusive linked list (`spatial_heads` + `spatial_next`) for zero per-bucket allocs.
    cache.spatial_heads.clear();
    cache.spatial_next.clear();

    let same_owner_dist = lod.min_spacing_px / lod.quantized_zoom;
    let same_owner_dist_sq = same_owner_dist * same_owner_dist;
    let diff_owner_dist = same_owner_dist * BUILDING_DIFF_OWNER_SPACING_RATIO;
    let diff_owner_dist_sq = diff_owner_dist * diff_owner_dist;
    let cell_size = same_owner_dist.max(1.0);

    for candidate in cache.scratch_candidates.drain(..) {
        let cell_x = (candidate.bx / cell_size).floor() as i32;
        let cell_y = (candidate.by / cell_size).floor() as i32;
        let must_keep = selected_tile == Some(candidate.tile_idx) || candidate.keep_local_progress;

        let mut suppressed = false;
        if !must_keep {
            'neighbors: for dy in -1..=1 {
                for dx in -1..=1 {
                    let mut idx_opt = cache.spatial_heads.get(&(cell_x + dx, cell_y + dy)).copied();
                    while let Some(kept_idx) = idx_opt {
                        let kept = &mut cache.buildings[kept_idx];
                        let ddx = candidate.bx - kept.bx;
                        let ddy = candidate.by - kept.by;
                        let dist_sq = ddx * ddx + ddy * ddy;
                        let same_owner = kept.owner_id == candidate.owner_id;
                        let limit_sq = if same_owner {
                            same_owner_dist_sq
                        } else {
                            diff_owner_dist_sq
                        };
                        if dist_sq < limit_sq {
                            if same_owner {
                                kept.stack_hint = true;
                            }
                            suppressed = true;
                            break 'neighbors;
                        }
                        let next = cache.spatial_next[kept_idx];
                        idx_opt = (next != usize::MAX).then_some(next);
                    }
                }
            }
        }

        if !suppressed {
            let kept_idx = cache.buildings.len();
            cache.buildings.push(candidate);
            let prev_head = cache
                .spatial_heads
                .insert((cell_x, cell_y), kept_idx)
                .unwrap_or(usize::MAX);
            cache.spatial_next.push(prev_head);
        }
    }

    // Reverse so lowest-priority markers paint first and highest-priority (Selected, Silo, City,
    // SAM) paint last on top.
    cache.buildings.reverse();
}

