use super::overlays::{avatar_slot, world_to_screen_values};
use crate::app::{InputState, SimState};
use crate::render::gpu::{AVATAR_CORNER_RADIUS_RATIO, TextRenderer};
use crate::theme::dev_config::DevConfig;
use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::{PlayerSnapshot, SimSnapshot};
use sow_render::nameplate::{
    BadgeKind, MapPoint, NAMEPLATE_SAMPLE_TICKS, NameplateCapacityPlan, NameplateLandCache,
    NameplateLayout, NameplateMetrics, NameplatePresentation, NameplateStatus, ScreenPoint,
    WorldRect, assign_nameplate_capacity, fit_bounds_to_land, fit_size_to_land, fitted_font_px,
    fog_allows_nameplate, font_size_scale, nameplate_visible_after_viewport_and_fog,
    sample_due as nameplate_sample_due,
    size_needs_interpolation as nameplate_size_needs_interpolation,
};
use sow_render::text::NameplateStatusSprite;
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

const NAMEPLATE_WORLD_SCALE: f32 = 0.05;
const NAMEPLATE_MIN_FONT: f32 = 8.0;
const NAMEPLATE_MAX_FONT: f32 = 32.0;
const NAMEPLATE_HIDE_ZOOM: f32 = 1.5;
const NAMEPLATE_MAX_CATCHUP_TICKS: u64 = NAMEPLATE_SAMPLE_TICKS * 2;
const LOD_DOT_RADIUS: f32 = 2.0;
const CATEGORY_EMOJI_DIAMETER_SCALE: f32 = 0.70;
pub(crate) const INLINE_EMOJI_SCALE: f32 = 1.4;

#[derive(Clone, Debug)]
pub(crate) struct NameplateVisualState {
    to_center: MapPoint,
    from_size: f32,
    to_size: f32,
    land_bounds: WorldRect,
    source_name: String,
    player_type: PlayerType,
    display_name: String,
    troops_bits: u64,
    troops_text: String,
    troops_scratch: String,
    prepared_name: sow_render::text::PreparedText,
    prepared_troops: sow_render::text::PreparedText,
    text_style_key: Option<u32>,
}

#[derive(Default)]
pub(crate) struct NameplateSystem {
    sample_tick: Option<u64>,
    sample_at: Option<Instant>,
    order_my_id: Option<u16>,
    order: Vec<usize>,
    active_ids: HashSet<u16>,
    visuals: HashMap<u16, NameplateVisualState>,
    land_cache: NameplateLandCache,
    frame_plans: Vec<PreparedNameplate>,
}

#[derive(Clone, Copy)]
struct PreparedNameplate {
    player_index: usize,
    is_human: bool,
    full: Option<FullNameplate>,
    dot: LodDot,
    presentation: NameplatePresentation,
}

impl NameplateCapacityPlan for PreparedNameplate {
    fn is_human(&self) -> bool {
        self.is_human
    }

    fn full_instance_count(&self) -> Option<usize> {
        self.full.map(|full| full.instance_count)
    }

    fn select_presentation(&mut self, presentation: NameplatePresentation) {
        self.presentation = presentation;
    }

    fn selected_presentation(&self) -> NameplatePresentation {
        self.presentation
    }
}

#[derive(Clone, Copy)]
struct LodDot {
    center: ScreenPoint,
    color: [f32; 4],
    scale: f32,
}

#[derive(Clone, Copy)]
struct FullNameplate {
    player_id: u16,
    avatar: Option<AvatarPlan>,
    badges: [Option<BadgePlan>; 7],
    badge_count: usize,
    name: Option<TextPlan>,
    troops: Option<TroopsPlan>,
    text_style: sow_render::text::TextPaintStyle,
    emoji_outline: sow_render::text::OutlineStyle,
    instance_count: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct AvatarPlan {
    center: [f32; 2],
    radius: f32,
    frame_radius: f32,
    border: f32,
    frame_color: [f32; 4],
    content: AvatarContent,
}

#[derive(Clone, Copy)]
enum AvatarContent {
    Sprite([f32; 4]),
    Emblem(&'static str),
    None,
}

#[derive(Clone, Copy)]
struct TextPlan {
    position: [f32; 2],
    final_font_size: f32,
    color: [f32; 4],
}

#[derive(Clone, Copy)]
struct TroopsPlan {
    icon_center: [f32; 2],
    icon_diameter: f32,
    text: TextPlan,
}

#[derive(Clone, Copy)]
struct BadgePlan {
    kind: BadgeKind,
    center: [f32; 2],
    diameter: f32,
    tint: [f32; 4],
}

impl NameplateSystem {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn anchor_for(&self, player_id: u16) -> Option<[f32; 2]> {
        self.visuals.get(&player_id).map(|state| state.to_center.0)
    }
}

pub(super) fn render_nameplates(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    system: &mut NameplateSystem,
    input: &InputState,
    dev: &DevConfig,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    sf: f32,
    zoom_scaled: f32,
    now: Instant,
    my_id: u16,
    leaderboard_top_three: [Option<u16>; 3],
    tutorial_target_player: Option<u16>,
) {
    let sf = if sf.is_finite() { sf.max(0.01) } else { 1.0 };
    sample_nameplates(text, snapshot, sim, system, dev, sf, my_id, now);

    let my_player = snapshot.players.iter().find(|player| player.id == my_id);
    let mut full_labels_drawn = 0usize;
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let fog_hidden = matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. });
    let alpha = nameplate_sample_alpha(system.sample_at, now, nameplate_sample_duration(sim));
    let inverse_alpha = 1.0 - alpha;

    let (order, visuals, frame_plans) = (&system.order, &system.visuals, &mut system.frame_plans);
    frame_plans.clear();
    for &player_index in order.iter() {
        let player = &snapshot.players[player_index];
        let is_me = player.id == my_id;
        let presentation_type = if tutorial_target_player == Some(player.id) {
            PlayerType::Human
        } else {
            player.player_type
        };
        let is_human = presentation_type == PlayerType::Human;
        let Some(state) = visuals.get(&player.id) else {
            continue;
        };
        let center_world = state.to_center;
        let center = ScreenPoint(world_to_screen_values(
            center_world.0[0],
            center_world.0[1],
            input.camera_x,
            input.camera_y,
            input.camera_zoom,
            sf,
        ));
        let land_min = ScreenPoint(world_to_screen_values(
            state.land_bounds.0[0],
            state.land_bounds.0[1],
            input.camera_x,
            input.camera_y,
            input.camera_zoom,
            sf,
        ));
        let land_max = ScreenPoint(world_to_screen_values(
            state.land_bounds.0[2],
            state.land_bounds.0[3],
            input.camera_x,
            input.camera_y,
            input.camera_zoom,
            sf,
        ));
        if !nameplate_visible_after_viewport_and_fog(land_min, land_max, screen_w, screen_h, || {
            fog_allows_nameplate(
                dev.fog_of_war,
                is_me,
                fog_hidden,
                player_at_explored_tile(center_world, sim),
            )
        }) {
            continue;
        }

        let dot = LodDot {
            center,
            color: player_color(player),
            scale: fit_size_to_land(
                2.0 * (LOD_DOT_RADIUS + 1.0),
                2.0 * (LOD_DOT_RADIUS + 1.0),
                state.land_bounds,
                zoom_scaled,
            ),
        };

        if zoom_scaled < NAMEPLATE_HIDE_ZOOM && !is_me && !is_human {
            frame_plans.push(PreparedNameplate {
                player_index,
                is_human,
                full: None,
                dot,
                presentation: NameplatePresentation::Hidden,
            });
            continue;
        }

        let world_size = if state.from_size == state.to_size {
            state.to_size
        } else {
            lerp(state.from_size, state.to_size, inverse_alpha)
        };

        let scaled_size = nameplate_font_px(world_size, zoom_scaled, is_human);

        let is_allied = my_player
            .filter(|me| me.id != player.id)
            .is_some_and(|me| me.alliances.contains(&player.id));
        let has_request = my_player
            .filter(|me| me.id != player.id)
            .is_some_and(|me| me.alliance_requests.contains(&player.id));
        let rank = leaderboard_top_three
            .iter()
            .position(|id| *id == Some(player.id))
            .map(|index| index + 1);
        let status = NameplateStatus {
            show_names: dev.vfx_nameplate_names,
            show_troops: dev.vfx_nameplate_troops,
            is_me,
            is_allied,
            has_request,
            rank,
            has_traitor: player.traitor,
            has_active_emoji: player
                .active_emoji
                .as_deref()
                .is_some_and(|emoji| emoji != "🗡️"),
            has_disconnected: player.disconnected,
        };
        let show_bot_avatars = dev.vfx_bot_avatars || player.campaign_avatar.is_some();
        let layout = compute_nameplate_layout(
            presentation_type,
            show_bot_avatars,
            center,
            scaled_size,
            &state.prepared_name,
            &state.prepared_troops,
            status,
            dev,
            sf,
        );
        let glyph_padding = (dev.font_face_dilate + dev.font_outline_thickness)
            .max(dev.font_face_dilate)
            .max(0.0);
        let shadow_padding = dev.font_underlay_softness.max(0.0) + dev.font_shadow_y.abs();
        let text_padding = glyph_padding
            .max(shadow_padding)
            .max(layout.badge_effect_padding());
        let layout_bounds = layout.visual_bounds(center, text_padding);
        let fit_scale = fit_bounds_to_land(layout_bounds, center, state.land_bounds, zoom_scaled);
        if fit_scale <= 0.0 || !fit_scale.is_finite() {
            continue;
        }
        let fitted_font_size = fitted_font_px(
            scaled_size,
            fit_scale,
            dev.font_size_scale,
            dev.vfx_nameplate_names,
            dev.vfx_nameplate_troops,
        );
        let show_full = fitted_font_size >= 7.0 && (is_human || full_labels_drawn < 80);
        if !show_full {
            frame_plans.push(PreparedNameplate {
                player_index,
                is_human,
                full: None,
                dot,
                presentation: NameplatePresentation::Hidden,
            });
            continue;
        }
        if !is_human {
            full_labels_drawn += 1;
        }

        let full = prepare_full_nameplate(
            text,
            player,
            state,
            center,
            scaled_size,
            layout,
            fit_scale,
            presentation_type,
            dev,
            campaign_avatar_slots,
            sf,
        );
        frame_plans.push(PreparedNameplate {
            player_index,
            is_human,
            full: Some(full),
            dot,
            presentation: NameplatePresentation::Hidden,
        });
    }

    assign_nameplate_capacity(frame_plans, text.remaining_instance_capacity());
    for plan in frame_plans.iter() {
        let player = &snapshot.players[plan.player_index];
        match plan.presentation {
            NameplatePresentation::Full => {
                let full = plan
                    .full
                    .expect("full presentation requires a complete prepared plate");
                let state = visuals
                    .get(&full.player_id)
                    .expect("prepared plate retains its sampled player state");
                paint_prepared_nameplate(text, player, state, full);
            }
            NameplatePresentation::Compact => {
                paint_lod_dot(text, plan.dot.center, plan.dot.color, sf, plan.dot.scale)
            }
            NameplatePresentation::Hidden => {}
        }
    }
}

fn sample_nameplates(
    text: &TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    system: &mut NameplateSystem,
    dev: &DevConfig,
    sf: f32,
    my_id: u16,
    now: Instant,
) {
    let my_id_changed = system.order_my_id != Some(my_id);
    if !nameplate_sample_due(system.sample_tick, snapshot.tick, my_id_changed) {
        return;
    }

    let Some(engine) = sim.engine.as_ref() else {
        system.order.clear();
        system.visuals.clear();
        system.land_cache = NameplateLandCache::default();
        return;
    };
    let new_player_needs_landmass = snapshot.players.iter().any(|player| {
        player.alive && player.tile_count > 0 && !system.land_cache.has_landmass(player.id)
    });
    if system
        .land_cache
        .needs_rebuild(&engine.state.map, snapshot.tick, new_player_needs_landmass)
        && !system.land_cache.rebuild(&engine.state.map, snapshot.tick)
    {
        system.order.clear();
        system.visuals.clear();
        return;
    }

    let tick_gap = system
        .sample_tick
        .map(|tick| snapshot.tick.saturating_sub(tick))
        .unwrap_or(NAMEPLATE_SAMPLE_TICKS);

    let force_snap = tick_gap > NAMEPLATE_MAX_CATCHUP_TICKS;
    let sample_alpha = if force_snap {
        1.0
    } else {
        nameplate_sample_alpha(system.sample_at, now, nameplate_sample_duration(sim))
    };
    let inverse_sample_alpha = 1.0 - sample_alpha;
    let style_key = nameplate_metrics_style_key(dev);
    system.order.clear();
    system.order.extend(
        snapshot
            .players
            .iter()
            .enumerate()
            .filter(|(_, player)| {
                player.alive
                    && player.tile_count > 0
                    && system.land_cache.rect_for(player.id).is_some()
            })
            .map(|(index, _)| index),
    );
    system.order.sort_unstable_by(|a, b| {
        let a = &snapshot.players[*a];
        let b = &snapshot.players[*b];
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

    system.active_ids.clear();
    system
        .active_ids
        .extend(system.order.iter().map(|index| snapshot.players[*index].id));
    system
        .visuals
        .retain(|player_id, _| system.active_ids.contains(player_id));

    for player in snapshot
        .players
        .iter()
        .filter(|player| player.alive && player.tile_count > 0)
    {
        let Some(land_rect) = system.land_cache.rect_for(player.id) else {
            continue;
        };
        let target_center = land_rect.center();
        let land_bounds = land_rect.world_bounds();
        let target_size = nameplate_world_size(player.tile_count);
        if let Some(state) = system.visuals.get_mut(&player.id) {
            let current_size = if state.from_size == state.to_size {
                state.to_size
            } else {
                lerp(state.from_size, state.to_size, inverse_sample_alpha)
            };
            state.from_size =
                if force_snap || !nameplate_size_needs_interpolation(current_size, target_size) {
                    target_size
                } else {
                    current_size
                };
            state.to_center = target_center;
            state.to_size = target_size;
            state.land_bounds = land_bounds;
            refresh_nameplate_text_cache(text, state, player, style_key, dev, sf);
        } else {
            let mut state =
                new_nameplate_visual_state(player, target_center, target_size, land_bounds);
            refresh_nameplate_text_cache(text, &mut state, player, style_key, dev, sf);
            system.visuals.insert(player.id, state);
        }
    }

    system.sample_tick = Some(snapshot.tick);
    system.sample_at = Some(now);
    system.order_my_id = Some(my_id);
}

fn new_nameplate_visual_state(
    player: &PlayerSnapshot,
    center: MapPoint,
    size: f32,
    land_bounds: WorldRect,
) -> NameplateVisualState {
    NameplateVisualState {
        to_center: center,
        from_size: size,
        to_size: size,
        land_bounds,
        source_name: String::new(),
        player_type: player.player_type,
        display_name: String::new(),
        troops_bits: u64::MAX,
        troops_text: String::new(),
        troops_scratch: String::with_capacity(12),
        prepared_name: sow_render::text::PreparedText::default(),
        prepared_troops: sow_render::text::PreparedText::default(),
        text_style_key: None,
    }
}

fn refresh_nameplate_text_cache(
    text: &TextRenderer,
    state: &mut NameplateVisualState,
    player: &PlayerSnapshot,
    style_key: u32,
    dev: &DevConfig,
    _sf: f32,
) {
    let identity_changed =
        state.source_name != player.name || state.player_type != player.player_type;
    if identity_changed {
        state.source_name = player.name.clone();
        state.player_type = player.player_type;
        let display_name =
            sow_core::player::display_name(player.id, &player.name, player.player_type);
        state.display_name = if player.player_type == PlayerType::Bot {
            display_name
                .strip_prefix(sow_core::player::tribe_animal(player.id, &player.name))
                .unwrap_or(&display_name)
                .trim_start()
                .to_owned()
        } else {
            display_name
        };
    }
    let troops_bits = player.troops.to_bits();
    let mut troops_text_changed = false;
    if troops_bits != state.troops_bits {
        state.troops_bits = troops_bits;
        crate::utils::format_number_into(&mut state.troops_scratch, player.troops);
        if state.troops_text != state.troops_scratch {
            std::mem::swap(&mut state.troops_text, &mut state.troops_scratch);
            troops_text_changed = true;
        }
    }
    if identity_changed || state.text_style_key != Some(style_key) {
        state.prepared_name = text.prepare_string(
            &state.display_name,
            nameplate_char_spacing(dev),
            INLINE_EMOJI_SCALE,
            0.5,
        );
    }
    if troops_text_changed || state.text_style_key != Some(style_key) {
        state.prepared_troops = text.prepare_string(
            &state.troops_text,
            nameplate_char_spacing(dev),
            INLINE_EMOJI_SCALE,
            0.0,
        );
    }
    state.text_style_key = Some(style_key);
}

fn nameplate_metrics_style_key(dev: &DevConfig) -> u32 {
    nameplate_char_spacing(dev).to_bits()
}

fn nameplate_char_spacing(dev: &DevConfig) -> f32 {
    if dev.font_char_spacing.is_finite() {
        dev.font_char_spacing.max(0.1)
    } else {
        1.0
    }
}

fn nameplate_sample_duration(sim: &SimState) -> Duration {
    Duration::from_secs_f32(
        (sim.config.tick_rate_ms.max(1.0) / 1000.0) * NAMEPLATE_SAMPLE_TICKS as f32,
    )
}

fn nameplate_sample_alpha(sample_at: Option<Instant>, now: Instant, duration: Duration) -> f32 {
    let Some(sample_at) = sample_at else {
        return 1.0;
    };
    let duration_secs = duration.as_secs_f32().max(0.001);
    let t = (now.duration_since(sample_at).as_secs_f32() / duration_secs).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(from: f32, to: f32, inverse_amount: f32) -> f32 {
    to - (to - from) * inverse_amount
}

fn player_at_explored_tile(center: MapPoint, sim: &SimState) -> bool {
    let col = center.0[0].floor() as i32;
    let row = center.0[1].floor() as i32;
    if col < 0 || row < 0 || col >= sim.map_w as i32 || row >= sim.map_h as i32 {
        return false;
    }
    sim.fog_explored
        .contains((row * sim.map_w as i32 + col) as u32)
}

fn nameplate_world_size(tile_count: u32) -> f32 {
    (tile_count as f32).sqrt().clamp(0.2, 150.0)
}

fn nameplate_font_px(world_size: f32, zoom_scaled: f32, is_human: bool) -> f32 {
    let world_px = world_size * NAMEPLATE_WORLD_SCALE * zoom_scaled;
    if is_human {
        world_px.clamp(NAMEPLATE_MIN_FONT, NAMEPLATE_MAX_FONT)
    } else {
        world_px
    }
}

fn compute_nameplate_layout(
    player_type: PlayerType,
    show_bot_avatars: bool,
    center: ScreenPoint,
    scaled_size: f32,
    prepared_name: &sow_render::text::PreparedText,
    prepared_troops: &sow_render::text::PreparedText,
    status: NameplateStatus,
    dev: &DevConfig,
    sf: f32,
) -> NameplateLayout {
    let metrics = NameplateMetrics::compute(scaled_size, player_type, show_bot_avatars);
    let outline = crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let badge_padding = outline
        .scaled_for_emoji(metrics.badge_size() * sf)
        .effect_padding()
        / sf;
    NameplateLayout::compute(
        center,
        metrics,
        prepared_name,
        prepared_troops,
        font_size_scale(dev.font_size_scale),
        status,
        badge_padding,
    )
}

fn prepare_full_nameplate(
    text: &TextRenderer,
    player: &PlayerSnapshot,
    state: &NameplateVisualState,
    center: ScreenPoint,
    scaled_size: f32,
    layout: NameplateLayout,
    fit_scale: f32,
    presentation_type: PlayerType,
    dev: &DevConfig,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    sf: f32,
) -> FullNameplate {
    let color = player_color(player);
    let layout = layout.scaled_about(center, fit_scale);
    let font_scale = font_size_scale(dev.font_size_scale);
    let metrics = NameplateMetrics::compute(
        scaled_size,
        presentation_type,
        dev.vfx_bot_avatars || player.campaign_avatar.is_some(),
    );
    let name_font_size = metrics.render_size() * font_scale;
    let troops_font_size = metrics.troops_render_size() * font_scale;
    let emoji_outline = scale_outline_style(
        crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]),
        fit_scale,
    );
    let text_style = crate::render::dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]).scaled(fit_scale);
    let avatar = (layout.avatar_radius() > 0.0)
        .then(|| prepare_avatar(text, player, campaign_avatar_slots, layout, color, sf));
    let (badges, badge_count) = prepare_badges(layout, sf);
    let name = dev.vfx_nameplate_names.then_some(TextPlan {
        position: {
            let anchor = layout.name_anchor(center);
            [anchor.0[0] * sf, anchor.0[1] * sf]
        },
        final_font_size: name_font_size * sf * fit_scale,
        color,
    });
    let troops = dev.vfx_nameplate_troops.then(|| {
        let (icon_center, icon_diameter) = layout.troops_icon(center);
        let text_anchor = layout.troops_text_anchor(center);
        TroopsPlan {
            icon_center: [icon_center.0[0] * sf, icon_center.0[1] * sf],
            icon_diameter: icon_diameter * sf,
            text: TextPlan {
                position: [text_anchor.0[0] * sf, text_anchor.0[1] * sf],
                final_font_size: troops_font_size * sf * fit_scale,
                color,
            },
        }
    });
    let instance_count = usize::from(avatar.is_some())
        .saturating_add(avatar.map_or(0, |avatar| {
            usize::from(!matches!(avatar.content, AvatarContent::None))
        }))
        .saturating_add(badge_count)
        .saturating_add(name.map_or(0, |_| state.prepared_name.instance_count()))
        .saturating_add(troops.map_or(0, |_| {
            1usize.saturating_add(state.prepared_troops.instance_count())
        }));

    FullNameplate {
        player_id: player.id,
        avatar,
        badges,
        badge_count,
        name,
        troops,
        text_style,
        emoji_outline,
        instance_count,
    }
}

pub(crate) fn prepare_death_avatar(
    text: &TextRenderer,
    identity: &sow_core::player::AvatarIdentity,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    center: [f32; 2],
    radius: f32,
    color: [f32; 4],
) -> AvatarPlan {
    let identity_ref = match identity {
        sow_core::player::AvatarIdentity::Portrait { slug, leader } => {
            sow_core::player::AvatarIdentityRef::Portrait {
                slug,
                leader: *leader,
            }
        }
        sow_core::player::AvatarIdentity::Emblem { symbol } => {
            sow_core::player::AvatarIdentityRef::Emblem { symbol }
        }
        sow_core::player::AvatarIdentity::Fallback => sow_core::player::AvatarIdentityRef::Fallback,
    };
    let (frame_color, content) =
        resolve_avatar_content(text, identity_ref, campaign_avatar_slots, color);
    let border = radius * 0.12;
    AvatarPlan {
        center,
        radius,
        frame_radius: radius + border * 0.3,
        border,
        frame_color,
        content,
    }
}

fn resolve_avatar_content(
    text: &TextRenderer,
    identity: sow_core::player::AvatarIdentityRef<'_>,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    color: [f32; 4],
) -> ([f32; 4], AvatarContent) {
    match identity {
        sow_core::player::AvatarIdentityRef::Portrait { slug, leader: None } => {
            let slot = campaign_avatar_slots
                .get(slug)
                .copied()
                .unwrap_or(Leader::ALL.len());
            let content = text
                .avatar_uv(slot)
                .or_else(|| text.avatar_uv(Leader::ALL.len()))
                .map_or(AvatarContent::None, AvatarContent::Sprite);
            (color, content)
        }
        sow_core::player::AvatarIdentityRef::Portrait {
            leader: Some(leader),
            ..
        } => {
            let rgb = leader.filler_rgb();
            let frame = [rgb[0], rgb[1], rgb[2], 1.0];
            let content = text
                .avatar_uv(avatar_slot(Some(leader)))
                .or_else(|| text.avatar_uv(avatar_slot(None)))
                .map_or(AvatarContent::None, AvatarContent::Sprite);
            (frame, content)
        }
        sow_core::player::AvatarIdentityRef::Emblem { symbol } => {
            (color, AvatarContent::Emblem(symbol))
        }
        sow_core::player::AvatarIdentityRef::Fallback => (
            color,
            text.avatar_uv(Leader::ALL.len())
                .map_or(AvatarContent::None, AvatarContent::Sprite),
        ),
    }
}

fn prepare_avatar(
    text: &TextRenderer,
    player: &PlayerSnapshot,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    layout: NameplateLayout,
    color: [f32; 4],
    sf: f32,
) -> AvatarPlan {
    let (frame_color, content) = resolve_avatar_content(
        text,
        sow_core::player::avatar_identity_ref(player),
        campaign_avatar_slots,
        color,
    );
    let center = [
        layout.avatar_center().0[0] * sf,
        layout.avatar_center().0[1] * sf,
    ];
    let radius = layout.avatar_radius() * sf;
    let border = radius * 0.12;
    AvatarPlan {
        center,
        radius,
        frame_radius: radius + border * 0.3,
        border,
        frame_color,
        content,
    }
}

fn prepare_badges(layout: NameplateLayout, sf: f32) -> ([Option<BadgePlan>; 7], usize) {
    (
        (*layout.badges()).map(|badge| {
            badge.map(|badge| BadgePlan {
                kind: badge.kind(),
                center: [badge.center().0[0] * sf, badge.center().0[1] * sf],
                diameter: badge.diameter() * sf,
                tint: badge.tint(),
            })
        }),
        layout.badge_count(),
    )
}

fn paint_prepared_nameplate(
    text: &mut TextRenderer,
    player: &PlayerSnapshot,
    state: &NameplateVisualState,
    plan: FullNameplate,
) {
    let before = text.instance_count();
    if let Some(avatar) = plan.avatar {
        paint_prepared_avatar(text, avatar, plan.emoji_outline, 1.0);
    }
    for badge in plan.badges[..plan.badge_count].iter().flatten() {
        let status_sprite = match badge.kind {
            BadgeKind::Request => Some(NameplateStatusSprite::Request),
            BadgeKind::Allied => Some(NameplateStatusSprite::Allied),
            BadgeKind::Traitor => Some(NameplateStatusSprite::Traitor),
            _ => None,
        };
        if let Some(sprite) = status_sprite {
            text.push_status_sprite(sprite, badge.center, badge.diameter, badge.tint);
            continue;
        }
        let icon = match badge.kind {
            BadgeKind::Rank1 => Some("👑"),
            BadgeKind::Rank2 => Some("🥈"),
            BadgeKind::Rank3 => Some("🥉"),
            BadgeKind::Star => Some("⭐"),
            BadgeKind::Request | BadgeKind::Allied | BadgeKind::Traitor => None,
            BadgeKind::Active => player.active_emoji.as_deref(),
            BadgeKind::Disconnected => Some("🔌"),
        };
        if let Some(icon) = icon {
            draw_emoji(
                text,
                icon,
                badge.center,
                badge.diameter,
                badge.tint,
                plan.emoji_outline,
                1.0,
            );
        }
    }
    if let Some(name) = plan.name {
        let _ = text.push_prepared_text(
            &state.prepared_name,
            name.position,
            name.final_font_size,
            name.color,
            plan.text_style,
        );
    }
    if let Some(troops) = plan.troops {
        draw_emoji(
            text,
            "⚔",
            troops.icon_center,
            troops.icon_diameter,
            troops.text.color,
            plan.emoji_outline,
            1.0,
        );
        let _ = text.push_prepared_text(
            &state.prepared_troops,
            troops.text.position,
            troops.text.final_font_size,
            troops.text.color,
            plan.text_style,
        );
    }
    debug_assert!(text.instance_count().saturating_sub(before) <= plan.instance_count);
}

pub(crate) fn paint_prepared_avatar(
    text: &mut TextRenderer,
    avatar: AvatarPlan,
    outline: sow_render::text::OutlineStyle,
    alpha: f32,
) {
    let mut frame_color = avatar.frame_color;
    frame_color[3] *= alpha;
    text.push_rounded_rect(
        avatar.center,
        [avatar.frame_radius * 2.0; 2],
        AVATAR_CORNER_RADIUS_RATIO,
        frame_color,
        [0.0, 0.0, 0.0, 160.0 / 255.0 * alpha],
        avatar.border * 0.5,
    );
    let tint = [1.0, 1.0, 1.0, alpha];
    match avatar.content {
        AvatarContent::Sprite(uv) => {
            text.push_sprite(avatar.center, avatar.radius, uv, tint);
        }
        AvatarContent::Emblem(glyph) => {
            let _ = text.push_emoji(
                glyph,
                avatar.center,
                avatar.radius * CATEGORY_EMOJI_DIAMETER_SCALE,
                tint,
                outline,
            );
        }
        AvatarContent::None => {}
    }
}

fn scale_outline_style(
    mut outline: crate::render::gpu::OutlineStyle,
    scale: f32,
) -> crate::render::gpu::OutlineStyle {
    outline.thickness *= scale;
    outline.shadow_y *= scale;
    outline.reference_diameter *= scale;
    outline
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

fn paint_lod_dot(
    text: &mut TextRenderer,
    center: ScreenPoint,
    color: [f32; 4],
    sf: f32,
    fit_scale: f32,
) {
    let center = [center.0[0] * sf, center.0[1] * sf];
    let radius = LOD_DOT_RADIUS * fit_scale * sf;
    text.push_disc(center, radius, color);
    text.push_ring(
        center,
        radius,
        [0.0, 0.0, 0.0, 180.0 / 255.0],
        fit_scale * sf,
    );
}

pub(crate) fn player_color(player: &PlayerSnapshot) -> [f32; 4] {
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
