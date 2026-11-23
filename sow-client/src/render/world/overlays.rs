use super::feedback;
use crate::app::{InputState, MapContextMenuView, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::game::BuildingKind;
use sow_core::protocol::SimSnapshot;
use sow_render::text::NameplateStatusSprite;
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

const BUILDING_CULL_FLOOR: f32 = 0.25;
const BUILDING_MIN_MARKER_SIZE: f32 = 14.0;
/// Short side, in tiles, of the standard building the Dev Tools zoom cutoffs describe.
const BUILDING_LOD_REFERENCE_SIDE: f32 = 2.0;
const BUILDING_ZOOM_STEPS_PER_OCTAVE: f32 = 8.0;
const BUILDING_FOOTPRINT_FILL: f32 = 0.9;
const BUILDING_UPGRADE_SCALES: [f32; 4] = [1.0, 1.15, 1.3, 1.5];
const BUILDING_LEVEL_FONT_RATIO: f32 = 0.58;
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
) {
    let Some(snapshot) = sim.current_snapshot.as_ref() else {
        return;
    };
    let sf = sf.max(0.01);
    let zoom_scaled = input.camera_zoom / sf;
    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    let leaderboard_top_three = ui.leaderboard_top_three;

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
            sf,
            ui.app.settings_state.reduced_motion,
            now,
        );
        if dev.vfx_world_buildings {
            render_buildings(text, snapshot, sim, ui, input, dev, sf, zoom_scaled, now);
        } else {
            ui.hovered_building = None;
            ui.prev_hovered_building = None;
        }
        render_building_placement_preview(text, sim, ui, input, sf, placement_can_confirm);
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
}

#[derive(Clone, Copy, PartialEq)]
struct BuildingLod {
    band: i16,
    quantized_zoom: f32,
    cluster_cell_size: f32,
    compact_zoom: f32,
    cluster_zoom: f32,
}

impl BuildingLod {
    /// Cutoffs come from the Dev Tools "Zoom LOD" tab and describe a standard 2×2 building;
    /// larger footprints keep their detail proportionally longer.
    fn for_zoom(
        zoom_scaled: f32,
        params: &[[f32; 4]; sow_render::shader_params::PARAM_CAPACITY],
    ) -> Self {
        let setting = |id| sow_render::shader_params::number_value(params, id);
        let zoom = zoom_scaled.max(BUILDING_CULL_FLOOR);
        let band = (zoom.log2() * BUILDING_ZOOM_STEPS_PER_OCTAVE).floor() as i16;
        let quantized_zoom = 2.0_f32.powf(band as f32 / BUILDING_ZOOM_STEPS_PER_OCTAVE);
        Self {
            band,
            quantized_zoom,
            cluster_cell_size: (setting("lod.building.group_px") / quantized_zoom).max(1.0),
            compact_zoom: setting("lod.building.compact_zoom"),
            cluster_zoom: setting("lod.building.cluster_zoom"),
        }
    }

    fn detail(self, kind: BuildingKind) -> BuildingDetail {
        let (width, height) = kind.footprint_dimensions();
        let reference_zoom =
            width.min(height) as f32 / BUILDING_LOD_REFERENCE_SIDE * self.quantized_zoom;
        if reference_zoom < self.cluster_zoom {
            BuildingDetail::Cluster
        } else if reference_zoom < self.compact_zoom {
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
    #[allow(dead_code)]
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
    lod: Option<BuildingLod>,
    player_id: u16,
    tick_rate_ms_bits: u32,
    buildings: Vec<RenderedBuilding>,
    clusters: HashMap<BuildingClusterKey, (f32, f32, usize)>,
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
        tick_rate_ms_bits: u32,
    ) -> bool {
        self.tick == Some(tick)
            && self.map_w == map_w
            && self.lod == Some(lod)
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

pub(crate) fn construction_progress(
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

fn construction_status_label(ticks_remaining: u32, tick_rate_ms: f32) -> String {
    format_construction_time(ticks_remaining, tick_rate_ms)
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
        label: show_label
            .then(|| construction_status_label(building.ticks_until_complete, tick_rate_ms)),
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
    let desktop_hover_enabled = crate::web_canvas::supports_desktop_hover()
        && matches!(input.hover_pointer, crate::app::HoverPointer::Mouse)
        && !input.is_pointer_gesture_active()
        && input.camera_zoom > 0.0
        && input.last_mouse_x >= 0.0
        && input.last_mouse_y >= 0.0;
    let reduced_motion = ui.app.settings_state.reduced_motion;
    let buildings = cached_buildings(ui, snapshot, sim.map_w, lod, my_id, sim.config.tick_rate_ms);
    let (next_hovered_building, next_exiting_hover) = if desktop_hover_enabled {
        let [wx, wy] = input.screen_to_world([input.last_mouse_x as f32, input.last_mouse_y as f32]);
        let pointer_x = input.last_mouse_x as f32 / sf;
        let pointer_y = input.last_mouse_y as f32 / sf;
        let mut closest: Option<(f32, u32)> = None;
        for building in buildings {
            if building.owner_id != my_id {
                continue;
            }
            let Some(tile_idx) = building.tile_idx else {
                continue;
            };
            let (fw, fh) = building.kind.footprint_dimensions();
            let hw = fw as f32 * 0.5;
            let hh = fh as f32 * 0.5;
            if wx < building.bx - hw
                || wx >= building.bx + hw
                || wy < building.by - hh
                || wy >= building.by + hh
            {
                continue;
            }
            let center = world_to_screen(building.bx, building.by, input, sf);
            let dx = pointer_x - center[0];
            let dy = pointer_y - center[1];
            let dist_sq = dx * dx + dy * dy;
            if closest.is_none_or(|(best_dist, best_tile)| {
                dist_sq < best_dist || (dist_sq == best_dist && tile_idx < best_tile)
            }) {
                closest = Some((dist_sq, tile_idx));
            }
        }
        let hovered_tile = closest.map(|(_, tile_idx)| tile_idx);
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
                || selected_menu.is_some_and(|menu| menu.tile_idx == exit_tile)
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
        let selected_tile = selected_menu.map(|menu| menu.tile_idx);
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
                .find(|b| b.owner_id == my_id && b.tile_idx == Some(target_tile))
            else {
                continue;
            };
            let elapsed = now.saturating_duration_since(started_at).as_secs_f32();
            let Some(motion) =
                crate::render::motion::building_hover_ring_motion(elapsed, exiting, reduced_motion)
            else {
                continue;
            };
            let marker_size = building_marker_size(building, zoom_scaled);
            let building_size = building_sprite_size(marker_size, sf, building.level.max(1));
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

        let alpha = if building.under_construction {
            0.5
        } else {
            1.0
        };
        let building_center = [center[0] * sf, center[1] * sf];
        let mut sx = 1.0f32;
        let mut sy = 1.0f32;
        let mut lift_y = 0.0f32;
        let mut sprite_tint = [1.0, 1.0, 1.0, alpha];
        if let Some(tile_idx) = building.tile_idx {
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
                    let t = (elapsed / 0.38).clamp(0.0, 1.0);
                    // Anticipation squash (t < 0.14) -> stretch & damped wobble.
                    let env = (-6.5 * t).exp();
                    let osc = (t * 22.0 - 0.6).sin();
                    let stretch = env * osc * 0.16;
                    sx *= 1.0 - stretch * 0.85;
                    sy *= 1.0 + stretch;
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
                    text.push_status_sprite(
                        NameplateStatusSprite::UpgradeSparkle,
                        building_center,
                        1.7 * marker_extent * sf * (0.45 + 0.45 * t),
                        [0.45, 0.92, 1.0, (1.0 - t) * 0.85],
                    );
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
        }
        let animated_center = [building_center[0], building_center[1] + lift_y];
        let animated_size = [building_size[0] * sx, building_size[1] * sy];
        // Every kind and level has art in assets/art/buildings (checked by tests).
        if let Some(uv_rect) =
            crate::render::gpu::building_sprite_uv(building.kind, building.level.max(1))
        {
            text.push_building_sprite(
                animated_center,
                animated_size,
                uv_rect,
                sprite_tint,
            );
        }

        if building.kind == BuildingKind::MissileSilo {
            text.push_status_sprite(
                NameplateStatusSprite::Radiation,
                building_center,
                (marker_extent * sf).max(9.0 * sf),
                [1.0, 0.78, 0.32, alpha],
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
                            [1.0, 0.84, 0.36, alpha * 0.9],
                        );
                    }
                    celebration_budget -= count;
                }
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
                    if building.kind == BuildingKind::MissileSilo {
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

    let lod = DevConfig::with(|dev| BuildingLod::for_zoom(zoom_scaled, &dev.shader_params));
    let buildings = cached_buildings(ui, snapshot, sim.map_w, lod, my_id, sim.config.tick_rate_ms);
    let pointer_x = x as f32 / sf;
    let pointer_y = y as f32 / sf;
    let [pointer_world_x, pointer_world_y] = input.screen_to_world([x as f32, y as f32]);
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
    let marker_size = building_footprint_marker_size(kind, zoom_scaled, BUILDING_MIN_MARKER_SIZE);
    let building_size = building_sprite_size(marker_size, sf, level.max(1));
    let bottom_y = center[1] + (building_size[1] * 0.5) / sf + 6.0;
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
        .map(|b| b.level.max(1))
        .unwrap_or_else(|| menu.building_level.max(1));
    let under_construction = live_building
        .map(|b| b.under_construction)
        .unwrap_or(menu.building_under_construction);

    let marker_size = building_footprint_marker_size(kind, zoom_scaled, 0.0);
    let building_size = building_sprite_size(marker_size, sf, level);
    let (ring_center, base_radius) =
        building_ground_ring_geometry(bx, by, building_size, input, sf);
    let elapsed = now.saturating_duration_since(menu.opened_at).as_secs_f32();
    let motion = crate::render::motion::building_selection_ring_motion(elapsed, reduced_motion);
    let (ring_color, core_glow) = if under_construction {
        (
            [0.28, 0.92, 0.92, motion.alpha],
            [0.90, 0.99, 1.0, 0.88],
        )
    } else {
        (
            [0.22, 0.98, 0.56, motion.alpha],
            [0.88, 1.0, 0.94, 0.92],
        )
    };
    text.push_selection_ring(
        ring_center,
        [base_radius[0] * motion.scale, base_radius[1] * motion.scale],
        1.0,
        motion.phase,
        motion.pulse,
        motion.acquire,
        ring_color,
        core_glow,
    );

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
    let site_legal = sow_core::building::validate_structure_site(
        map,
        my_id,
        kind,
        placement.tile_idx,
        &engine.building_grid,
    )
    .is_ok();
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

/// Letter size at which a row (`widths` measured at `font_size`, plus `gaps`) fits `max_width`.
// ponytail: letters stop at 40% of their size; a longer row needs a wider button.

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
    if !cache.matches(snapshot.tick, map_w, lod, my_id, tick_rate_ms_bits) {
        collect_buildings(
            snapshot,
            map_w,
            lod,
            my_id,
            tick_rate_ms,
            &mut cache.buildings,
            &mut cache.clusters,
            &mut cache.city_tiles,
            &mut cache.under_construction_tiles,
        );
        cache.buildings.sort_unstable_by(|a, b| {
            a.by.partial_cmp(&b.by)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.bx.partial_cmp(&b.bx).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.count.cmp(&b.count))
        });
        cache.tick = Some(snapshot.tick);
        cache.map_w = map_w;
        cache.lod = Some(lod);
        cache.player_id = my_id;
        cache.tick_rate_ms_bits = tick_rate_ms_bits;
    }
    &cache.buildings
}

#[allow(clippy::too_many_arguments)]
fn collect_buildings(
    snapshot: &SimSnapshot,
    map_w: u32,
    lod: BuildingLod,
    my_id: u16,
    tick_rate_ms: f32,
    out: &mut Vec<RenderedBuilding>,
    clusters: &mut HashMap<BuildingClusterKey, (f32, f32, usize)>,
    city_tiles: &mut HashSet<u32>,
    under_construction_tiles: &mut HashSet<u32>,
) {
    let map_w = map_w.max(1);
    out.clear();
    clusters.clear();
    city_tiles.clear();
    under_construction_tiles.clear();
    for building in &snapshot.buildings {
        if building.kind == BuildingKind::City {
            city_tiles.insert(building.tile_idx);
        }
        if building.under_construction {
            under_construction_tiles.insert(building.tile_idx);
        }
    }
    for building in &snapshot.buildings {
        if building.kind != BuildingKind::City && city_tiles.contains(&building.tile_idx) {
            continue;
        }
        let tile_x = building.tile_idx % map_w;
        let tile_y = building.tile_idx / map_w;
        let active_level = building.active_level();
        let display_level = active_level;
        let detail = lod.detail(building.kind);
        let has_active_city_build = building.kind == BuildingKind::City
            && (building.under_construction
                || building.arsenal_build.is_some()
                || under_construction_tiles.contains(&building.tile_idx));
        let keep_local_progress = building.owner_id == my_id
            && (building.under_construction || has_active_city_build);
        if detail == BuildingDetail::Cluster && !keep_local_progress {
            let (bx, by) = crate::render::world::movers::tile_to_world(building.tile_idx, map_w);
            let key = BuildingClusterKey {
                grid_x: (tile_x as f32 / lod.cluster_cell_size) as i32,
                grid_y: (tile_y as f32 / lod.cluster_cell_size) as i32,
                owner_id: building.owner_id,
                kind: building.kind,
                level: display_level,
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
            level: display_level,
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

