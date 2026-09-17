use crate::app::{InputState, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::theme::dev_config::DevConfig;
use sow_core::game::BuildingKind;
use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::{PlayerSnapshot, SimSnapshot};
use std::collections::HashMap;

const NAMEPLATE_WORLD_SCALE: f32 = 0.05;
const NAMEPLATE_MAX_FONT: f32 = 32.0;
const NAMEPLATE_HIDE_ZOOM: f32 = 1.5;
const LOD_DOT_RADIUS: f32 = 2.0;

const HUMAN_AVATAR_SCALE: f32 = 4.0;
const BOT_AVATAR_SCALE: f32 = 3.0;
const NATION_AVATAR_SCALE: f32 = 3.6;
const BADGE_SCALE: f32 = 1.8;
const TROOPS_SCALE: f32 = 1.30;
const AVATAR_TEXT_GAP_SCALE: f32 = 0.16;
const BADGE_GAP: f32 = 3.0;
const BADGE_STACK_GAP: f32 = 2.0;
const CATEGORY_EMOJI_DIAMETER_SCALE: f32 = 0.70;
const INLINE_EMOJI_SCALE: f32 = 1.4;

const BUILDING_SCALE: f32 = 0.5;
const BUILDING_CULL_FLOOR: f32 = 0.25;
const BUILDING_LOD_START_ZOOM: f32 = 1.0;
const BUILDING_LOD_ZOOM_RANGE: f32 = 9.0;
const BUILDING_MIN_MARKER_SIZE: f32 = 14.0;
const BUILDING_CLUSTER_TARGET_SIZE: f32 = 40.0;
const BUILDING_LEVEL_FONT_RATIO: f32 = 0.58;

#[derive(Clone, Copy, Debug, PartialEq)]
struct TutorialAvatarGeometry {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

pub(crate) fn nameplate_screen_center(
    player: &PlayerSnapshot,
    input: &InputState,
    sf: f32,
) -> [f32; 2] {
    nameplate_screen_center_from_world(
        player.centroid_x,
        player.centroid_y,
        input.camera_x,
        input.camera_y,
        input.camera_zoom,
        sf,
    )
}

fn nameplate_screen_center_from_world(
    centroid_x: f32,
    centroid_y: f32,
    camera_x: f32,
    camera_y: f32,
    camera_zoom: f32,
    sf: f32,
) -> [f32; 2] {
    let sf = sf.max(0.01);
    [
        (camera_x + (centroid_x + 0.5) * camera_zoom) / sf,
        (camera_y + (centroid_y + 0.5) * camera_zoom) / sf,
    ]
}

fn tutorial_avatar_geometry(
    text: &TextRenderer,
    player: &PlayerSnapshot,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
) -> Option<TutorialAvatarGeometry> {
    if !player.alive || player.tile_count == 0 || !player.has_spawned {
        return None;
    }
    let sf = sf.max(0.01);
    let zoom_scaled = input.camera_zoom / sf;
    let is_human = player.player_type == PlayerType::Human;
    let scaled_size = nameplate_font_px(player.tile_count, zoom_scaled, is_human);
    if !is_human && (zoom_scaled < NAMEPLATE_HIDE_ZOOM || scaled_size < 7.0) {
        return None;
    }
    let center = nameplate_screen_center(player, input, sf);
    let layout = compute_nameplate_layout(text, player, center, scaled_size, dev, sf);
    (layout.avatar_radius > 0.0).then_some(TutorialAvatarGeometry {
        x: layout.avatar_center[0],
        y: layout.avatar_center[1],
        radius: layout.avatar_radius,
    })
}

pub(crate) fn render_overlays(
    text: &mut TextRenderer,
    sim: &SimState,
    ui: &UiState,
    input: &InputState,
    sf: f32,
    time_secs: f32,
) {
    let Some(snapshot) = sim.current_snapshot.as_ref() else {
        return;
    };
    let sf = sf.max(0.01);
    let dev = DevConfig::get();
    let zoom_scaled = input.camera_zoom / sf;

    if dev.vfx_world_buildings {
        render_buildings(text, snapshot, sim, input, &dev, sf, zoom_scaled);
    }
    render_nameplates(text, snapshot, sim, ui, input, &dev, sf, zoom_scaled);

    if !ui.tutorial_active {
        return;
    }
    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    if let Some(geometry) = snapshot
        .players
        .iter()
        .find(|player| player.id == my_id)
        .and_then(|player| tutorial_avatar_geometry(text, player, input, &dev, sf))
    {
        render_tutorial_pointer(text, geometry, time_secs, sf);
    }
}

fn render_tutorial_pointer(
    text: &mut TextRenderer,
    geometry: TutorialAvatarGeometry,
    time_secs: f32,
    sf: f32,
) {
    let sf = sf.max(0.01);
    let base_radius = geometry.radius.max(0.0) + 6.0;
    let center = [geometry.x * sf, geometry.y * sf];
    let gold = [1.0, 200.0 / 255.0, 90.0 / 255.0, 1.0];

    for k in 0..2 {
        let phase = (time_secs * 1.1 + k as f32 * 0.5).rem_euclid(1.0);
        let radius = (base_radius + phase * base_radius * 2.2) * sf;
        text.push_ring(
            center,
            radius,
            [gold[0], gold[1], gold[2], 1.0 - phase],
            2.0 * sf,
        );
    }
    text.push_ring(center, base_radius * sf, gold, 2.5 * sf);

    let bob = (time_secs * 3.0).sin() * 5.0;
    let tip_y = geometry.y - base_radius - 10.0 + bob;
    text.push_triangle(
        [geometry.x * sf, (tip_y - 5.5) * sf],
        [18.0 * sf, 11.0 * sf],
        gold,
    );
}

fn render_nameplates(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &UiState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    zoom_scaled: f32,
) {
    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    let mut players: Vec<&PlayerSnapshot> = snapshot
        .players
        .iter()
        .filter(|player| player.alive && player.tile_count > 0)
        .collect();

    players.sort_unstable_by(|a, b| {
        let precedence = |player: &PlayerSnapshot| match player.player_type {
            PlayerType::Human if player.id == my_id => 1,
            PlayerType::Human => 2,
            _ => 0,
        };
        precedence(a)
            .cmp(&precedence(b))
            .then_with(|| b.tile_count.cmp(&a.tile_count))
            .then_with(|| a.id.cmp(&b.id))
    });

    let my_player = snapshot.players.iter().find(|player| player.id == my_id);
    let mut full_labels_drawn = 0usize;
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let fog_hidden = matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. });

    for player in players {
        let is_me = player.id == my_id;
        let is_human = player.player_type == PlayerType::Human;
        if dev.fog_of_war && !is_me && !fog_hidden && !player_at_explored_tile(player, sim) {
            continue;
        }

        let center = nameplate_screen_center(player, input, sf);
        if center[0] < -160.0
            || center[0] > screen_w + 160.0
            || center[1] < -160.0
            || center[1] > screen_h + 160.0
        {
            continue;
        }

        let scaled_size = nameplate_font_px(player.tile_count, zoom_scaled, is_human);
        if zoom_scaled < NAMEPLATE_HIDE_ZOOM && !is_me && !is_human {
            paint_lod_dot(text, center, player_color(player), sf);
            continue;
        }

        let show_full = is_human || (scaled_size >= 7.0 && full_labels_drawn < 80);
        if !show_full {
            paint_lod_dot(text, center, player_color(player), sf);
            continue;
        }
        if !is_human {
            full_labels_drawn += 1;
        }

        let display_name =
            sow_core::player::display_name(player.id, &player.name, player.player_type);
        let troops = crate::utils::format_number(player.troops);
        let is_allied = my_player
            .filter(|me| me.id != player.id)
            .is_some_and(|me| me.alliances.contains(&player.id));
        let has_request = my_player
            .filter(|me| me.id != player.id)
            .is_some_and(|me| me.alliance_requests.contains(&player.id));
        let rank = ui
            .leaderboard_rankings
            .iter()
            .position(|ranking| ranking.id == player.id)
            .map(|index| index + 1)
            .filter(|rank| *rank <= 3);

        paint_nameplate(
            text,
            player,
            center,
            scaled_size,
            &display_name,
            &troops,
            is_me,
            is_allied,
            has_request,
            rank,
            dev,
            sf,
        );
    }
}

fn player_at_explored_tile(player: &PlayerSnapshot, sim: &SimState) -> bool {
    let col = player.centroid_x.floor() as i32;
    let row = player.centroid_y.floor() as i32;
    if col < 0 || row < 0 || col >= sim.map_w as i32 || row >= sim.map_h as i32 {
        return false;
    }
    sim.fog_explored
        .contains((row * sim.map_w as i32 + col) as u32)
}

fn nameplate_font_px(tile_count: u32, zoom_scaled: f32, is_human: bool) -> f32 {
    let territory_side = (tile_count as f32).sqrt().clamp(0.2, 150.0);
    let world_px = territory_side * NAMEPLATE_WORLD_SCALE * zoom_scaled;
    if is_human {
        world_px.clamp(8.0, NAMEPLATE_MAX_FONT)
    } else {
        world_px
    }
}

#[derive(Clone, Copy)]
struct NameplateMetrics {
    render_size: f32,
    avatar_diameter: f32,
    avatar_radius: f32,
    badge_size: f32,
    troops_render_size: f32,
}

impl NameplateMetrics {
    fn compute(scaled_size: f32, player_type: PlayerType, show_bot_avatars: bool) -> Self {
        let render_size = scaled_size.max(7.0);
        let avatar_scale = match player_type {
            PlayerType::Human => HUMAN_AVATAR_SCALE,
            PlayerType::Bot if show_bot_avatars => BOT_AVATAR_SCALE,
            PlayerType::Nation => NATION_AVATAR_SCALE,
            PlayerType::Bot => 0.0,
        };
        let avatar_diameter = if avatar_scale > 0.0 {
            (render_size * avatar_scale).max(4.0)
        } else {
            0.0
        };
        Self {
            render_size,
            avatar_diameter,
            avatar_radius: avatar_diameter * 0.5,
            badge_size: render_size * BADGE_SCALE,
            troops_render_size: render_size * TROOPS_SCALE,
        }
    }
}

#[derive(Clone, Copy)]
struct NameplateLayout {
    avatar_center: [f32; 2],
    avatar_radius: f32,
    badge_size: f32,
    left_x: f32,
    right_x: f32,
    rank_center: [f32; 2],
    star_center: [f32; 2],
    text_top: f32,
    item_spacing_y: f32,
    name_size: [f32; 2],
    troops_size: [f32; 2],
    stack_step: f32,
}

impl NameplateLayout {
    fn compute(
        center: [f32; 2],
        metrics: NameplateMetrics,
        name_size: [f32; 2],
        troops_size: [f32; 2],
        show_names: bool,
        show_troops: bool,
        badge_effect_padding: f32,
    ) -> Self {
        let name_size = if show_names { name_size } else { [0.0; 2] };
        let troops_size = if show_troops { troops_size } else { [0.0; 2] };
        let item_spacing_y = if show_names && show_troops {
            metrics.render_size * 0.111
        } else {
            0.0
        };
        let text_height = name_size[1] + item_spacing_y + troops_size[1];
        let avatar_text_gap = if metrics.avatar_diameter > 0.0 && text_height > 0.0 {
            metrics.render_size * AVATAR_TEXT_GAP_SCALE
        } else {
            0.0
        };
        let total_height = metrics.avatar_diameter + avatar_text_gap + text_height;
        let content_top = center[1] - total_height * 0.5;
        let avatar_center = [center[0], content_top + metrics.avatar_diameter * 0.5];
        let avatar_visual_radius = avatar_visual_radius(metrics.avatar_radius);
        let badge_clearance = BADGE_GAP + badge_effect_padding.max(0.0);
        let badge_half = metrics.badge_size * 0.5;
        let stack_step = metrics.badge_size + BADGE_STACK_GAP + badge_effect_padding.max(0.0) * 2.0;
        let left_x = center[0] - avatar_visual_radius - badge_half - badge_clearance;
        let right_x = center[0] + avatar_visual_radius + badge_half + badge_clearance;

        Self {
            avatar_center,
            avatar_radius: metrics.avatar_radius,
            badge_size: metrics.badge_size,
            left_x,
            right_x,
            rank_center: [
                center[0],
                avatar_center[1] - avatar_visual_radius - badge_half - badge_clearance,
            ],
            star_center: [left_x, avatar_center[1]],
            text_top: content_top + metrics.avatar_diameter + avatar_text_gap,
            item_spacing_y,
            name_size,
            troops_size,
            stack_step,
        }
    }

    fn side_badge_center(&self, left: bool, stack_slot: usize, is_me: bool) -> [f32; 2] {
        let x = if left { self.left_x } else { self.right_x };
        let y = if left && is_me {
            self.avatar_center[1] + self.stack_step
        } else {
            self.avatar_center[1] - stack_slot as f32 * self.stack_step
        };
        [x, y]
    }

    fn express_center(&self, right_stack_slots: usize) -> [f32; 2] {
        [
            self.right_x + 2.0,
            self.side_badge_center(false, right_stack_slots, false)[1] - self.badge_size * 0.5,
        ]
    }
}

fn compute_nameplate_layout(
    text: &TextRenderer,
    player: &PlayerSnapshot,
    center: [f32; 2],
    scaled_size: f32,
    dev: &DevConfig,
    sf: f32,
) -> NameplateLayout {
    let metrics = NameplateMetrics::compute(scaled_size, player.player_type, dev.vfx_bot_avatars);
    let font_scale = dev.font_size_scale.max(0.1);
    let char_spacing = dev.font_char_spacing.max(0.1);
    let name_font_size = metrics.render_size * font_scale;
    let troops_font_size = metrics.troops_render_size * font_scale;
    let troops_measure = text_measure(
        text,
        &crate::utils::format_number(player.troops),
        troops_font_size,
        char_spacing,
        sf,
    );
    let troops_icon_size = troops_font_size;
    let troops_size = [
        troops_icon_size + 3.0 + troops_measure[0],
        troops_icon_size.max(troops_measure[1]),
    ];
    let display_name = sow_core::player::display_name(player.id, &player.name, player.player_type);
    let name_measure = text_measure(text, &display_name, name_font_size, char_spacing, sf);
    let outline = crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let badge_padding = outline
        .scaled_for_emoji(metrics.badge_size * sf)
        .effect_padding()
        / sf;
    NameplateLayout::compute(
        center,
        metrics,
        name_measure,
        troops_size,
        dev.vfx_nameplate_names,
        dev.vfx_nameplate_troops,
        badge_padding,
    )
}

fn paint_nameplate(
    text: &mut TextRenderer,
    player: &PlayerSnapshot,
    center: [f32; 2],
    scaled_size: f32,
    display_name: &str,
    troops: &str,
    is_me: bool,
    is_allied: bool,
    has_request: bool,
    rank: Option<usize>,
    dev: &DevConfig,
    sf: f32,
) {
    let metrics = NameplateMetrics::compute(scaled_size, player.player_type, dev.vfx_bot_avatars);
    let font_scale = dev.font_size_scale.max(0.1);
    let char_spacing = dev.font_char_spacing.max(0.1);
    let name_font_size = metrics.render_size * font_scale;
    let troops_font_size = metrics.troops_render_size * font_scale;
    let troops_icon_size = troops_font_size;
    let outline = crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let layout = compute_nameplate_layout(text, player, center, scaled_size, dev, sf);
    let color = player_color(player);
    let text_style = crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]);

    if layout.avatar_radius > 0.0 {
        draw_avatar(
            text,
            player,
            layout.avatar_center,
            layout.avatar_radius,
            color,
            outline,
            sf,
        );

        if let Some(rank) = rank {
            let icon = match rank {
                1 => "👑",
                2 => "🥈",
                _ => "🥉",
            };
            let tint = match rank {
                1 => [250.0 / 255.0, 204.0 / 255.0, 21.0 / 255.0, 1.0],
                2 => [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
                _ => [217.0 / 255.0, 119.0 / 255.0, 6.0 / 255.0, 1.0],
            };
            draw_emoji(
                text,
                icon,
                layout.rank_center,
                layout.badge_size,
                tint,
                outline,
                sf,
            );
        }
        if is_me {
            draw_emoji(
                text,
                "⭐",
                layout.star_center,
                layout.badge_size,
                [1.0; 4],
                outline,
                sf,
            );
        }

        let mut right_slots = 0usize;
        if has_request {
            draw_emoji(
                text,
                "📨",
                layout.side_badge_center(true, 0, is_me),
                layout.badge_size,
                [1.0; 4],
                outline,
                sf,
            );
        }
        if is_allied {
            draw_emoji(
                text,
                "🤝",
                layout.side_badge_center(false, right_slots, is_me),
                layout.badge_size,
                [1.0; 4],
                outline,
                sf,
            );
            right_slots += 1;
        }
        if player.traitor {
            draw_emoji(
                text,
                "🗡️",
                layout.side_badge_center(false, right_slots, is_me),
                layout.badge_size,
                [1.0; 4],
                outline,
                sf,
            );
            right_slots += 1;
        }
        if let Some(active_emoji) = player
            .active_emoji
            .as_deref()
            .filter(|emoji| *emoji != "🗡️")
        {
            draw_emoji(
                text,
                active_emoji,
                layout.express_center(right_slots),
                layout.badge_size,
                [1.0; 4],
                outline,
                sf,
            );
        }
        if player.disconnected {
            draw_emoji(
                text,
                "🔌",
                [
                    layout.avatar_center[0] + layout.avatar_radius * 0.6,
                    layout.avatar_center[1] + layout.avatar_radius * 0.6,
                ],
                layout.avatar_radius * 0.8,
                [1.0; 4],
                outline,
                sf,
            );
        }
    }

    if dev.vfx_nameplate_names {
        text.push_string(
            display_name,
            [
                center[0] * sf,
                (layout.text_top + layout.name_size[1] * 0.85) * sf,
            ],
            name_font_size * sf,
            color,
            text_style,
            (0.5, char_spacing, INLINE_EMOJI_SCALE),
        );
    }
    if dev.vfx_nameplate_troops {
        let row_y = if dev.vfx_nameplate_names {
            layout.text_top + layout.name_size[1] + layout.item_spacing_y
        } else {
            layout.text_top
        };
        let left_x = center[0] - layout.troops_size[0] * 0.5;
        draw_emoji(
            text,
            "⚔",
            [
                left_x + troops_icon_size * 0.5,
                row_y + troops_icon_size * 0.5,
            ],
            troops_icon_size,
            color,
            outline,
            sf,
        );
        text.push_string(
            troops,
            [
                (left_x + troops_icon_size + 3.0) * sf,
                (row_y + layout.troops_size[1] * 0.85) * sf,
            ],
            troops_font_size * sf,
            color,
            text_style,
            (0.0, char_spacing, INLINE_EMOJI_SCALE),
        );
    }
}

fn text_measure(
    text: &TextRenderer,
    value: &str,
    font_size: f32,
    char_spacing: f32,
    sf: f32,
) -> [f32; 2] {
    let measure = text.measure_string(value, font_size * sf, char_spacing, INLINE_EMOJI_SCALE);
    [measure.width / sf, measure.height / sf]
}

fn draw_avatar(
    text: &mut TextRenderer,
    player: &PlayerSnapshot,
    center: [f32; 2],
    radius: f32,
    color: [f32; 4],
    outline: crate::render::gpu::OutlineStyle,
    sf: f32,
) {
    let center = [center[0] * sf, center[1] * sf];
    let radius = radius * sf;
    let frame = if player.player_type == PlayerType::Human {
        let rgb = player.leader.filler_rgb();
        let frame = [rgb[0], rgb[1], rgb[2], 1.0];
        if let Some(uv) = text
            .avatar_uv(avatar_slot(Some(player.leader)))
            .or_else(|| text.avatar_uv(avatar_slot(None)))
        {
            text.push_sprite(center, radius, uv, [1.0; 4]);
        } else {
            text.push_disc(center, radius, frame);
        }
        frame
    } else {
        text.push_disc(center, radius, color);
        color
    };

    let border = (radius * 0.12).max(1.0);
    text.push_ring(
        center,
        radius + border * 0.3,
        [0.0, 0.0, 0.0, 160.0 / 255.0],
        border,
    );
    text.push_ring(center, radius, frame, border * 0.8);
    text.push_ring(
        center,
        radius - border * 0.15,
        [1.0, 1.0, 1.0, 80.0 / 255.0],
        border * 0.35,
    );

    let glyph = match player.player_type {
        PlayerType::Bot => Some(sow_core::player::tribe_animal(player.id, &player.name)),
        PlayerType::Nation => Some(sow_core::player::empire_emoji(player.id, &player.name)),
        PlayerType::Human => None,
    };
    if let Some(glyph) = glyph {
        let _ = text.push_emoji(
            glyph,
            center,
            radius * CATEGORY_EMOJI_DIAMETER_SCALE,
            [1.0; 4],
            outline,
        );
    }
}

fn draw_emoji(
    text: &mut TextRenderer,
    emoji: &str,
    center: [f32; 2],
    diameter: f32,
    tint: [f32; 4],
    outline: crate::render::gpu::OutlineStyle,
    sf: f32,
) {
    let _ = text.push_emoji(
        emoji,
        [center[0] * sf, center[1] * sf],
        diameter * sf * 0.5,
        tint,
        outline,
    );
}

fn paint_lod_dot(text: &mut TextRenderer, center: [f32; 2], color: [f32; 4], sf: f32) {
    let center = [center[0] * sf, center[1] * sf];
    let radius = LOD_DOT_RADIUS * sf;
    text.push_disc(center, radius, color);
    text.push_ring(center, radius, [0.0, 0.0, 0.0, 180.0 / 255.0], sf);
}

fn player_color(player: &PlayerSnapshot) -> [f32; 4] {
    let rgb = player
        .team
        .map_or(player.color, sow_core::player::team_territory_rgb);
    let luminance = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let factor = if luminance < 0.60 {
        (0.60 - luminance) / (1.0 - luminance).max(0.001)
    } else {
        0.0
    };
    [
        rgb[0] + (1.0 - rgb[0]) * factor,
        rgb[1] + (1.0 - rgb[1]) * factor,
        rgb[2] + (1.0 - rgb[2]) * factor,
        1.0,
    ]
}

fn avatar_slot(leader: Option<Leader>) -> usize {
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
    final_scale: f32,
    cluster_cell_size: f32,
    compact: bool,
}

impl BuildingLod {
    fn for_zoom(zoom_scaled: f32) -> Self {
        let zoom_factor =
            ((zoom_scaled - BUILDING_LOD_START_ZOOM) / BUILDING_LOD_ZOOM_RANGE).clamp(0.0, 1.0);
        let final_scale = BUILDING_SCALE * (0.5 + 0.5 * zoom_factor);
        let natural_size = building_icon_size(zoom_scaled) * final_scale;
        let compact = natural_size < BUILDING_MIN_MARKER_SIZE;
        let cluster_cell_size = if compact {
            (BUILDING_CLUSTER_TARGET_SIZE / zoom_scaled.max(BUILDING_CULL_FLOOR)).max(1.0)
        } else {
            1.0
        };
        Self {
            final_scale,
            cluster_cell_size,
            compact,
        }
    }
}

#[derive(Clone, Copy)]
struct RenderedBuilding {
    bx: f32,
    by: f32,
    kind: BuildingKind,
    level: u8,
    under_construction: bool,
    count: usize,
    owner_id: u16,
    tile_idx: Option<u32>,
}

fn render_buildings(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    zoom_scaled: f32,
) {
    if zoom_scaled < BUILDING_CULL_FLOOR {
        return;
    }
    let lod = BuildingLod::for_zoom(zoom_scaled);
    let mut buildings = collect_buildings(snapshot, sim.map_w, lod);
    buildings.sort_unstable_by(|a, b| {
        a.by.partial_cmp(&b.by)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.bx.partial_cmp(&b.bx).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.count.cmp(&b.count))
    });

    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let my_id = sim.my_player_id.unwrap_or(0);
    let text_style = crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]);

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

        let center = [
            (input.camera_x + building.bx * input.camera_zoom) / sf,
            (input.camera_y + building.by * input.camera_zoom) / sf,
        ];
        let margin = zoom_scaled * 2.0;
        if center[0] < -margin
            || center[0] > screen_w + margin
            || center[1] < -margin
            || center[1] > screen_h + margin
        {
            continue;
        }

        let icon_size = building_icon_size(zoom_scaled);
        let natural_size = if building.count > 1 {
            icon_size * 1.2
        } else {
            icon_size
        } * lod.final_scale;
        let marker_size = if lod.compact {
            natural_size.max(BUILDING_MIN_MARKER_SIZE)
        } else {
            natural_size
        };
        let alpha = if building.under_construction {
            0.5
        } else {
            1.0
        };
        let _ = text.push_emoji(
            building_kind_emoji(building.kind),
            [center[0] * sf, center[1] * sf],
            marker_size * sf * 0.5,
            [1.0, 1.0, 1.0, alpha],
            crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha]),
        );

        if building.level != 1 || building.count > 1 {
            let label = if building.count > 1 {
                format!("{} × {}", building.level, building.count)
            } else {
                building.level.to_string()
            };
            let level_font_size = (marker_size * BUILDING_LEVEL_FONT_RATIO)
                .clamp(8.0, 18.0)
                .round()
                * dev.font_size_scale.max(0.1);
            let label_center = [
                center[0] + marker_size * 0.45,
                center[1] - marker_size * 0.45,
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
    }
}

fn collect_buildings(
    snapshot: &SimSnapshot,
    map_w: u32,
    lod: BuildingLod,
) -> Vec<RenderedBuilding> {
    let map_w = map_w.max(1);
    if lod.cluster_cell_size <= 1.0 {
        return snapshot
            .buildings
            .iter()
            .map(|building| {
                let (bx, by) =
                    crate::render::world::movers::tile_to_world(building.tile_idx, map_w);
                RenderedBuilding {
                    bx,
                    by,
                    kind: building.kind,
                    level: building.active_level(),
                    under_construction: building.under_construction,
                    count: 1,
                    owner_id: building.owner_id,
                    tile_idx: Some(building.tile_idx),
                }
            })
            .collect();
    }

    #[derive(Hash, PartialEq, Eq)]
    struct ClusterKey {
        grid_x: i32,
        grid_y: i32,
        owner_id: u16,
        kind: BuildingKind,
        level: u8,
    }

    let mut clusters: HashMap<ClusterKey, (f32, f32, usize)> = HashMap::new();
    for building in &snapshot.buildings {
        let (bx, by) = crate::render::world::movers::tile_to_world(building.tile_idx, map_w);
        let tile_x = (building.tile_idx % map_w) as f32;
        let tile_y = (building.tile_idx / map_w) as f32;
        let key = ClusterKey {
            grid_x: (tile_x / lod.cluster_cell_size) as i32,
            grid_y: (tile_y / lod.cluster_cell_size) as i32,
            owner_id: building.owner_id,
            kind: building.kind,
            level: building.active_level(),
        };
        let entry = clusters.entry(key).or_insert((0.0, 0.0, 0));
        entry.0 += bx;
        entry.1 += by;
        entry.2 += 1;
    }

    clusters
        .into_iter()
        .map(|(key, (sum_x, sum_y, count))| RenderedBuilding {
            bx: sum_x / count as f32,
            by: sum_y / count as f32,
            kind: key.kind,
            level: key.level,
            under_construction: false,
            count,
            owner_id: key.owner_id,
            tile_idx: None,
        })
        .collect()
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

fn building_kind_emoji(kind: BuildingKind) -> &'static str {
    match kind {
        BuildingKind::City => "🏛️",
        BuildingKind::Factory => "🏭",
        BuildingKind::Port => "⚓",
        BuildingKind::Bunker => "🛡️",
    }
}

fn avatar_visual_radius(radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let border = (radius * 0.12).max(1.0);
    radius + border * 0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nameplate_scales_match_the_last_gpu_tuning() {
        let human = NameplateMetrics::compute(14.0, PlayerType::Human, true);
        let bot = NameplateMetrics::compute(14.0, PlayerType::Bot, true);
        let nation = NameplateMetrics::compute(14.0, PlayerType::Nation, true);
        assert_eq!(human.avatar_diameter, 14.0 * HUMAN_AVATAR_SCALE);
        assert_eq!(bot.avatar_diameter, 14.0 * BOT_AVATAR_SCALE);
        assert_eq!(nation.avatar_diameter, 14.0 * NATION_AVATAR_SCALE);
    }

    #[test]
    fn building_lod_clusters_when_the_marker_is_not_readable() {
        let lod = BuildingLod::for_zoom(1.0);
        assert!(lod.compact);
        assert_eq!(lod.cluster_cell_size, BUILDING_CLUSTER_TARGET_SIZE);
    }

    #[test]
    fn tutorial_pointer_and_avatar_share_the_same_dpr_projection() {
        let world = (18.25, 7.75, 96.0, 48.0, 3.0);
        let metrics = NameplateMetrics::compute(14.0, PlayerType::Human, true);
        for sf in [1.0, 2.0] {
            let center = nameplate_screen_center_from_world(
                world.0, world.1, world.2, world.3, world.4, sf,
            );
            let layout = NameplateLayout::compute(
                center,
                metrics,
                [100.0, 20.0],
                [100.0, 20.0],
                true,
                true,
                0.0,
            );
            assert_eq!(layout.avatar_center[0], center[0]);
            let total_height = metrics.avatar_diameter
                + metrics.render_size * AVATAR_TEXT_GAP_SCALE
                + 20.0
                + metrics.render_size * 0.111
                + 20.0;
            let expected_y = center[1] - total_height * 0.5 + metrics.avatar_radius;
            assert!((layout.avatar_center[1] - expected_y).abs() < 0.001);
        }
    }
}
