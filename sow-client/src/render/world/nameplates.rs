//! Owns live/death avatar presentation; see docs/nameplates-and-emotes.md.
use super::overlays::world_to_screen_values;
use crate::app::{InputState, SimState};
use crate::render::gpu::TextRenderer;
use crate::render::motion::{EmoteMotion, emote_motion, smoothstep};
use crate::theme::dev_config::DevConfig;
use sow_core::emote::{ActiveEmote, AvatarExpression, EMOTE_DURATION_SECS, EMOTES};
use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::{PlayerSnapshot, SimSnapshot};
use sow_render::nameplate::{
    BadgeKind, MapPoint, NAMEPLATE_MAX_BADGES, NAMEPLATE_SAMPLE_TICKS, NameplateCapacityPlan,
    NameplateLandCache, NameplateLayout, NameplateMetrics, NameplatePresentation, NameplateStatus,
    NameplateStyle, ScreenPoint, WorldRect, assign_nameplate_capacity, fit_bounds_to_land,
    fit_size_to_land, fitted_font_px, fog_allows_nameplate, font_size_scale,
    nameplate_visible_after_viewport_and_fog, sample_due as nameplate_sample_due,
    size_needs_interpolation as nameplate_size_needs_interpolation,
};
use sow_render::text::NameplateStatusSprite;
use sow_render::text::{AVATAR_FALLBACK_SLOT, leader_avatar_slot};
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

const NAMEPLATE_WORLD_SCALE: f32 = 0.05;
const NAMEPLATE_TEXT_COLOR: [f32; 4] = [1.0; 4];
const NAMEPLATE_MIN_FONT: f32 = 8.0;
const NAMEPLATE_MAX_FONT: f32 = 32.0;
const NAMEPLATE_HIDE_ZOOM: f32 = 1.5;
const NAMEPLATE_MAX_CATCHUP_TICKS: u64 = NAMEPLATE_SAMPLE_TICKS * 2;
const SPAWN_STICKY_HOLD: Duration = Duration::from_millis(120);
const SPAWN_SETTLE_DURATION: Duration = Duration::from_millis(180);
const LOD_DOT_RADIUS: f32 = 2.0;
const CATEGORY_EMOJI_DIAMETER_SCALE: f32 = 0.70;
const AVATAR_PLACEHOLDER_COLOR: [f32; 4] = [0.16, 0.18, 0.20, 1.0];
pub(crate) const INLINE_EMOJI_SCALE: f32 = 1.4;

#[derive(Clone, Debug)]
pub(crate) struct NameplateVisualState {
    from_center: MapPoint,
    to_center: MapPoint,
    from_land_bounds: WorldRect,
    from_size: f32,
    to_size: f32,
    land_bounds: WorldRect,
    center_changed_at: Option<Instant>,
    slide_center: bool,
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
    local_spawn_tick: Option<u64>,
    frame_plans: Vec<PreparedNameplate>,
    tutorial_target_hit_bounds: Option<(u16, [f32; 4])>,
    emote_visuals: HashMap<u16, AvatarEmote>,
    emote_tick: Option<u64>,
    emote_tick_at: Option<Instant>,
    emote_texts: [String; 4],
    prepared_emotes: [sow_render::text::PreparedText; 4],
    emote_text_style_key: Option<u32>,
    safe_area: [f32; 4],
}

#[derive(Clone, Copy)]
struct PreparedNameplate {
    player_index: usize,
    has_human_priority: bool,
    full: Option<FullNameplate>,
    dot: LodDot,
    presentation: NameplatePresentation,
    tutorial_hit_bounds: Option<[f32; 4]>,
}

impl NameplateCapacityPlan for PreparedNameplate {
    fn has_human_priority(&self) -> bool {
        self.has_human_priority
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
    badges: [Option<BadgePlan>; NAMEPLATE_MAX_BADGES],
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
    content: AvatarContent,
    scale: [f32; 2],
    blend: Option<([f32; 4], f32)>,
}

impl AvatarPlan {
    fn base_instance_count(self) -> usize {
        1
    }

    fn transition_instance_count(self) -> usize {
        usize::from(self.blend.is_some())
    }
}

#[derive(Clone, Copy)]
enum AvatarContent {
    Sprite([f32; 4]),
    Emblem(&'static str),
    None,
}

#[derive(Clone, Copy)]
struct AvatarEmote {
    event: ActiveEmote,
    previous: AvatarExpression,
    from_motion: EmoteMotion,
    age_bias: f32,
    release_at: Option<Instant>,
    suppress_bubble: bool,
}

impl AvatarEmote {
    fn age(self, tick: u64, tick_secs: f32, fraction: f32) -> f32 {
        (self.event.age_seconds(tick, tick_secs * 1000.0) + fraction - self.age_bias).max(0.0)
    }

    fn motion(self, age: f32, now: Instant, reduced: bool) -> EmoteMotion {
        if let Some(released) = self.release_at {
            let fade = 1.0 - smoothstep(now.duration_since(released).as_secs_f32() / 0.15);
            return EmoteMotion {
                offset: self
                    .from_motion
                    .offset
                    .map(|v| if reduced { 0.0 } else { v * fade }),
                scale: self
                    .from_motion
                    .scale
                    .map(|v| if reduced { 1.0 } else { 1.0 + (v - 1.0) * fade }),
                bubble_alpha: self.from_motion.bubble_alpha * fade,
                bubble_scale: self.from_motion.bubble_scale,
            };
        }
        let mut motion = emote_motion(self.event.kind, age, reduced);
        if !reduced {
            let continuity = 1.0 - smoothstep(age / 0.07);
            for axis in 0..2 {
                motion.offset[axis] += self.from_motion.offset[axis] * continuity;
                motion.scale[axis] += (self.from_motion.scale[axis] - 1.0) * continuity;
            }
        }
        if self.suppress_bubble {
            motion.bubble_alpha = 0.0;
        }
        motion
    }

    fn expressions(self, age: f32, now: Instant) -> (AvatarExpression, AvatarExpression, f32) {
        let expression = self.event.kind.definition().expression;
        if let Some(released) = self.release_at {
            return (
                self.previous,
                AvatarExpression::Calm,
                smoothstep(now.duration_since(released).as_secs_f32() / 0.15),
            );
        }
        if !self.event.pinned && age >= EMOTE_DURATION_SECS - 0.15 {
            return (
                expression,
                AvatarExpression::Calm,
                smoothstep((age - (EMOTE_DURATION_SECS - 0.15)) / 0.15),
            );
        }
        (self.previous, expression, smoothstep((age - 0.07) / 0.12))
    }
}

#[derive(Clone, Copy)]
struct TextPlan {
    position: [f32; 2],
    final_font_size: f32,
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
        let texts = std::mem::take(&mut self.emote_texts);
        let safe_area = self.safe_area;
        *self = Self::default();
        self.emote_texts = texts;
        self.safe_area = safe_area;
    }

    pub(crate) fn set_emote_texts(&mut self, texts: [String; 4], safe_area: [f32; 4]) {
        for (index, value) in texts.into_iter().enumerate() {
            let value = value.trim();
            self.emote_texts[index] = if value.is_empty()
                || value.chars().count() > 160
                || value.chars().any(char::is_control)
            {
                EMOTES[index].fallback.to_owned()
            } else {
                value.to_owned()
            };
        }
        self.safe_area = safe_area.map(|v| {
            if v.is_finite() {
                v.clamp(0.0, 300.0)
            } else {
                0.0
            }
        });
        self.emote_text_style_key = None;
    }

    fn observe_emotes(
        &mut self,
        snapshot: &SimSnapshot,
        tick_secs: f32,
        now: Instant,
        reduced: bool,
    ) {
        if self.emote_tick.is_some_and(|tick| snapshot.tick < tick) {
            self.emote_visuals.clear();
        }
        if self.emote_tick != Some(snapshot.tick) {
            self.emote_tick = Some(snapshot.tick);
            self.emote_tick_at = Some(now);
        }
        let fraction = self.emote_tick_at.map_or(0.0, |at| {
            now.duration_since(at).as_secs_f32().min(tick_secs)
        });
        for player in &snapshot.players {
            if !player.alive {
                self.emote_visuals.remove(&player.id);
                continue;
            }
            let old = self.emote_visuals.get(&player.id).copied();
            if let Some(event) = player.active_emote {
                if old.is_some_and(|old| {
                    old.event.event_id == event.event_id && old.release_at.is_none()
                }) {
                    continue;
                }
                let raw_age = event.age_seconds(snapshot.tick, tick_secs * 1000.0);
                let (from_motion, previous) = old
                    .map(|old| {
                        let age = old.age(snapshot.tick, tick_secs, fraction);
                        let (from, to, blend) = old.expressions(age, now);
                        (
                            old.motion(age, now, reduced),
                            if blend < 0.5 { from } else { to },
                        )
                    })
                    .unwrap_or((EmoteMotion::default(), AvatarExpression::Calm));
                self.emote_visuals.insert(
                    player.id,
                    AvatarEmote {
                        event,
                        previous,
                        from_motion,
                        age_bias: if raw_age <= tick_secs * 2.0 {
                            raw_age
                        } else {
                            0.0
                        },
                        release_at: None,
                        suppress_bubble: event.pinned && old.is_none() && raw_age > tick_secs * 2.0,
                    },
                );
            } else if let Some(mut old) = old {
                if let Some(released) = old.release_at {
                    if now.duration_since(released).as_secs_f32() >= 0.15 {
                        self.emote_visuals.remove(&player.id);
                    }
                } else if old.event.expired(snapshot.tick, tick_secs * 1000.0) {
                    // Finish the existing return fade; expiry must not restart it.
                    if old.age(snapshot.tick, tick_secs, fraction) >= EMOTE_DURATION_SECS {
                        self.emote_visuals.remove(&player.id);
                    }
                } else {
                    let age = old.age(snapshot.tick, tick_secs, fraction);
                    let (from, to, blend) = old.expressions(age, now);
                    old.previous = if blend < 0.5 { from } else { to };
                    old.from_motion = old.motion(age, now, reduced);
                    old.release_at = Some(now);
                    self.emote_visuals.insert(player.id, old);
                }
            }
        }
    }

    fn prepare_emote_texts(&mut self, text: &TextRenderer, dev: &DevConfig) {
        let style_key = nameplate_metrics_style_key(dev);
        if self.emote_text_style_key != Some(style_key) {
            for (index, definition) in EMOTES.iter().enumerate() {
                let source = if self.emote_texts[index].is_empty() {
                    definition.fallback
                } else {
                    &self.emote_texts[index]
                };
                self.prepared_emotes[index] =
                    text.prepare_string(source, nameplate_char_spacing(dev), 1.0, 0.5);
            }
            self.emote_text_style_key = Some(style_key);
        }
    }

    pub(crate) fn anchor_for(&self, player_id: u16) -> Option<[f32; 2]> {
        self.visuals.get(&player_id).map(|state| state.to_center.0)
    }

    pub(crate) fn tutorial_target_contains(&self, player_id: u16, x: f64, y: f64) -> bool {
        let Some((target_id, [left, top, right, bottom])) = self.tutorial_target_hit_bounds else {
            return false;
        };
        target_id == player_id
            && x.is_finite()
            && y.is_finite()
            && x >= f64::from(left)
            && x <= f64::from(right)
            && y >= f64::from(top)
            && y <= f64::from(bottom)
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
    reduced_motion: bool,
) {
    let sf = if sf.is_finite() { sf.max(0.01) } else { 1.0 };
    system.observe_emotes(
        snapshot,
        (sim.config.tick_rate_ms / 1000.0).max(0.001),
        now,
        reduced_motion,
    );
    system.prepare_emote_texts(text, dev);
    system.tutorial_target_hit_bounds = None;
    sample_nameplates(text, snapshot, sim, system, dev, sf, my_id, now);

    let my_player = snapshot.players.iter().find(|player| player.id == my_id);
    let mut full_labels_drawn = 0usize;
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let fog_hidden = matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. });
    let alpha = nameplate_sample_alpha(system.sample_at, now, nameplate_sample_duration(sim));
    let inverse_alpha = 1.0 - alpha;

    let tick_secs = (sim.config.tick_rate_ms / 1000.0).max(0.001);
    let emote_fraction = system.emote_tick_at.map_or(0.0, |at| {
        now.duration_since(at).as_secs_f32().min(tick_secs)
    });
    let (order, visuals, frame_plans) = (&system.order, &system.visuals, &mut system.frame_plans);
    frame_plans.clear();
    for &player_index in order.iter() {
        let player = &snapshot.players[player_index];
        if !player.alive {
            continue;
        }
        let is_me = player.id == my_id;
        let has_human_priority = player.player_type == PlayerType::Human;
        let style = NameplateStyle::for_player(player.player_type, player.is_campaign_faction);
        let Some(state) = visuals.get(&player.id) else {
            continue;
        };
        let (center_world, land_bounds, spawn_settle_scale) = spawn_center_visual(state, now);
        let center = ScreenPoint(world_to_screen_values(
            center_world.0[0],
            center_world.0[1],
            input.camera_x,
            input.camera_y,
            input.camera_zoom,
            sf,
        ));
        let land_min = ScreenPoint(world_to_screen_values(
            land_bounds.0[0],
            land_bounds.0[1],
            input.camera_x,
            input.camera_y,
            input.camera_zoom,
            sf,
        ));
        let land_max = ScreenPoint(world_to_screen_values(
            land_bounds.0[2],
            land_bounds.0[3],
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
                land_bounds,
                zoom_scaled,
            ),
        };

        if zoom_scaled < NAMEPLATE_HIDE_ZOOM && !is_me && !has_human_priority {
            frame_plans.push(PreparedNameplate {
                player_index,
                has_human_priority,
                full: None,
                dot,
                presentation: NameplatePresentation::Hidden,
                tutorial_hit_bounds: None,
            });
            continue;
        }

        let world_size = (if state.from_size == state.to_size {
            state.to_size
        } else {
            lerp(state.from_size, state.to_size, inverse_alpha)
        }) * spawn_settle_scale;

        let scaled_size = nameplate_font_px(world_size, zoom_scaled, style);

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
            has_disconnected: player.disconnected,
        };
        let show_bot_avatars = dev.vfx_bot_avatars || player.campaign_avatar.is_some();
        let layout = compute_nameplate_layout(
            style,
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
        let fit_scale = fit_bounds_to_land(layout_bounds, center, land_bounds, zoom_scaled);
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
        let show_full = fitted_font_size >= 7.0 && (has_human_priority || full_labels_drawn < 80);
        if !show_full {
            frame_plans.push(PreparedNameplate {
                player_index,
                has_human_priority,
                full: None,
                dot,
                presentation: NameplatePresentation::Hidden,
                tutorial_hit_bounds: None,
            });
            continue;
        }
        if !has_human_priority {
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
            style,
            dev,
            campaign_avatar_slots,
            sf,
        );
        let tutorial_hit_bounds = (tutorial_target_player == Some(player.id)).then(|| {
            let [half_width, half_height] = layout_bounds.extents_about(center);
            let half_width = half_width * fit_scale * sf;
            let half_height = half_height * fit_scale * sf;
            [
                center.0[0] * sf - half_width,
                center.0[1] * sf - half_height,
                center.0[0] * sf + half_width,
                center.0[1] * sf + half_height,
            ]
        });
        frame_plans.push(PreparedNameplate {
            player_index,
            has_human_priority,
            full: Some(full),
            dot,
            presentation: NameplatePresentation::Hidden,
            tutorial_hit_bounds,
        });
    }

    assign_nameplate_capacity(frame_plans, text.remaining_instance_capacity());
    let base_reserved = frame_plans
        .iter()
        .map(|plan| match plan.presentation {
            NameplatePresentation::Full => plan.full.map_or(0, |full| full.instance_count),
            NameplatePresentation::Compact => 2,
            NameplatePresentation::Hidden => 0,
        })
        .sum::<usize>();
    let mut decoration_budget = text
        .remaining_instance_capacity()
        .saturating_sub(base_reserved);
    let mut tutorial_target_hit_bounds = None;
    for plan in frame_plans.iter() {
        let player = &snapshot.players[plan.player_index];
        if plan.presentation == NameplatePresentation::Full
            && tutorial_target_player == Some(player.id)
        {
            tutorial_target_hit_bounds = plan.tutorial_hit_bounds.map(|bounds| (player.id, bounds));
        }
        match plan.presentation {
            NameplatePresentation::Full => {
                let mut full = plan
                    .full
                    .expect("full presentation requires a complete prepared plate");
                let state = visuals
                    .get(&full.player_id)
                    .expect("prepared plate retains its sampled player state");
                if let Some(avatar) = &mut full.avatar
                    && let Some(emote) = system.emote_visuals.get(&player.id).copied()
                {
                    let age = emote.age(snapshot.tick, tick_secs, emote_fraction);
                    let motion = emote.motion(age, now, reduced_motion);
                    let diameter = avatar.radius * 2.0;
                    for axis in 0..2 {
                        avatar.center[axis] += motion.offset[axis] * diameter;
                    }
                    avatar.scale = motion.scale;
                    let (from, to, mix) = emote.expressions(age, now);
                    avatar.content = resolve_avatar_content(
                        text,
                        sow_core::player::avatar_identity_ref(player),
                        campaign_avatar_slots,
                        to,
                    );
                    if from != to && mix < 1.0 && decoration_budget > 0 {
                        if let AvatarContent::Sprite(from_uv) = resolve_avatar_content(
                            text,
                            sow_core::player::avatar_identity_ref(player),
                            campaign_avatar_slots,
                            from,
                        ) {
                            avatar.blend = Some((from_uv, mix));
                            decoration_budget -= 1;
                        }
                    }
                }
                paint_prepared_nameplate(text, state, full);
            }
            NameplatePresentation::Compact => {
                paint_lod_dot(text, plan.dot.center, plan.dot.color, sf, plan.dot.scale)
            }
            NameplatePresentation::Hidden => {}
        }
    }
    // Decorations are a second pass so glyph pressure cannot hide another player's base plate.
    for plan in frame_plans.iter() {
        if plan.presentation != NameplatePresentation::Full {
            continue;
        }
        let player = &snapshot.players[plan.player_index];
        if !sow_core::emote::is_recipient(&snapshot.players, player.id, my_id) {
            continue;
        }
        let Some(avatar) = plan.full.and_then(|full| full.avatar) else {
            continue;
        };
        let Some(emote) = system.emote_visuals.get(&player.id).copied() else {
            continue;
        };
        let age = emote.age(snapshot.tick, tick_secs, emote_fraction);
        let motion = emote.motion(age, now, reduced_motion);
        let prepared = &system.prepared_emotes[emote.event.kind as usize];
        let needed = prepared.instance_count().saturating_add(5);
        if motion.bubble_alpha <= 0.0 || needed > text.remaining_instance_capacity() {
            continue;
        }
        paint_emote_bubble(
            text,
            prepared,
            avatar,
            motion,
            player_color(player),
            input,
            system.safe_area,
            sf,
        );
    }
    system.tutorial_target_hit_bounds = tutorial_target_hit_bounds;
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
    let local_spawn = sim
        .latest_local_spawn
        .filter(|(tick, _, _)| system.local_spawn_tick != Some(*tick));
    if !nameplate_sample_due(system.sample_tick, snapshot.tick, my_id_changed)
        && local_spawn.is_none()
    {
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
    let animate_local_spawn = if let Some((tick, x, y)) = local_spawn {
        let updated = system
            .land_cache
            .update_spawn_anchor(&engine.state.map, my_id, x, y)
            .is_some();
        system.local_spawn_tick = Some(tick);
        updated
    } else {
        false
    };

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
            if animate_local_spawn && player.id == my_id && state.to_center != target_center {
                let (visible_center, visible_bounds, _) = spawn_center_visual(state, now);
                state.from_center = visible_center;
                state.from_land_bounds = visible_bounds;
                state.center_changed_at = Some(now);
                state.slide_center = center_inside_land(visible_center, land_bounds);
            }
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
        from_center: center,
        to_center: center,
        from_land_bounds: land_bounds,
        from_size: size,
        to_size: size,
        land_bounds,
        center_changed_at: None,
        slide_center: false,
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

fn spawn_center_visual(state: &NameplateVisualState, now: Instant) -> (MapPoint, WorldRect, f32) {
    let Some(changed_at) = state.center_changed_at else {
        return (state.to_center, state.land_bounds, 1.0);
    };
    let elapsed = now.duration_since(changed_at);
    if elapsed < SPAWN_STICKY_HOLD {
        return (state.from_center, state.from_land_bounds, 1.0);
    }
    let t = ((elapsed - SPAWN_STICKY_HOLD).as_secs_f32() / SPAWN_SETTLE_DURATION.as_secs_f32())
        .clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    let center = if state.slide_center {
        MapPoint([
            lerp(state.from_center.0[0], state.to_center.0[0], eased),
            lerp(state.from_center.0[1], state.to_center.0[1], eased),
        ])
    } else {
        state.to_center
    };
    (center, state.land_bounds, 0.94 + 0.06 * eased)
}

fn center_inside_land(center: MapPoint, bounds: WorldRect) -> bool {
    center.0[0] >= bounds.0[0]
        && center.0[0] <= bounds.0[2]
        && center.0[1] >= bounds.0[1]
        && center.0[1] <= bounds.0[3]
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
        state.display_name =
            sow_core::player::nameplate_display_name(player.id, &player.name, player.player_type);
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

#[cfg(test)]
mod emote_tests {
    use super::*;
    use sow_core::emote::EmoteKind;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn snapshot(event: Option<ActiveEmote>) -> SimSnapshot {
        let config = sow_core::game_config::GameConfig::default();
        let mut state = sow_core::game::GameState::new(7, 4, 4, config.clone());
        state.phase = sow_core::game::GamePhase::Playing;
        state.tick = 1;
        let mut player = sow_core::player::Player::new_human(1, "Hero".into(), [1.0; 3], &config);
        player.has_spawned = true;
        player.active_emote = event;
        state.register_player(player);
        let mut engine = sow_core::engine::SowEngine::new(
            state,
            sow_core::water_components::WaterComponents::default(),
        );
        engine.build_snapshot()
    }

    fn event(id: u64, pinned: bool) -> ActiveEmote {
        ActiveEmote {
            kind: EmoteKind::Threat,
            pinned,
            started_tick: 0,
            event_id: id,
        }
    }

    #[wasm_bindgen_test]
    fn portrait_uses_one_instance_and_crossfade_adds_only_the_second_image() {
        let portrait = avatar_plan([20.0, 30.0], 16.0, AvatarContent::Sprite([0.0; 4]));
        assert_eq!(portrait.base_instance_count(), 1);
        assert_eq!(portrait.transition_instance_count(), 0);

        let transition = AvatarPlan {
            blend: Some(([0.5, 0.5, 0.75, 0.75], 0.5)),
            ..portrait
        };
        assert_eq!(transition.base_instance_count(), 1);
        assert_eq!(transition.transition_instance_count(), 1);
    }

    #[wasm_bindgen_test]
    fn reduced_motion_keeps_active_and_releasing_avatars_still() {
        let now = Instant::now();
        let from_motion = EmoteMotion {
            offset: [0.08, -0.08],
            scale: [1.04, 0.96],
            bubble_alpha: 0.8,
            ..Default::default()
        };
        let active = AvatarEmote {
            event: event(1, false),
            previous: AvatarExpression::Calm,
            from_motion,
            age_bias: 0.0,
            release_at: None,
            suppress_bubble: false,
        };
        let active_motion = active.motion(0.18, now, true);
        assert_eq!(active_motion.offset, [0.0; 2]);
        assert_eq!(active_motion.scale, [1.0; 2]);
        assert!(active_motion.bubble_alpha > 0.0);

        let releasing = AvatarEmote {
            release_at: Some(now),
            ..active
        };
        let release_motion = releasing.motion(0.18, now + Duration::from_millis(30), true);
        assert_eq!(release_motion.offset, [0.0; 2]);
        assert_eq!(release_motion.scale, [1.0; 2]);
        assert!(release_motion.bubble_alpha > 0.0);
    }

    #[wasm_bindgen_test]
    fn events_are_immediate_duplicates_do_not_replay_and_death_clears_them() {
        let now = Instant::now();
        let mut system = NameplateSystem::default();
        let mut snap = snapshot(Some(event(1, false)));
        system.observe_emotes(&snap, 0.1, now, false);
        assert!(system.sample_tick.is_none());
        let original = system.emote_visuals[&1];
        assert!(!original.suppress_bubble);
        system.observe_emotes(&snap, 0.1, now + Duration::from_millis(50), false);
        assert_eq!(
            system.emote_visuals[&1].event.event_id,
            original.event.event_id
        );
        assert_eq!(system.emote_tick_at, Some(now));
        snap.players[0].alive = false;
        system.observe_emotes(&snap, 0.1, now, false);
        assert!(system.emote_visuals.is_empty());
    }

    #[wasm_bindgen_test]
    fn repeated_emotes_keep_current_pose_and_clear_returns_to_calm() {
        let now = Instant::now();
        let mut system = NameplateSystem::default();
        let mut snap = snapshot(Some(event(1, true)));
        system.observe_emotes(&snap, 0.1, now, false);
        snap.tick = 2;
        let previous = system.emote_visuals[&1];
        let previous_motion = previous.motion(previous.age(2, 0.1, 0.0), now, false);
        snap.players[0].active_emote = Some(ActiveEmote {
            event_id: 2,
            started_tick: 1,
            ..event(1, true)
        });
        system.observe_emotes(&snap, 0.1, now, false);
        let repeated = system.emote_visuals[&1];
        let first = repeated.motion(0.0, now, false);
        assert_eq!(first.offset, previous_motion.offset);
        assert_eq!(first.scale, previous_motion.scale);
        snap.players[0].active_emote = None;
        system.observe_emotes(&snap, 0.1, now, false);
        let released = system.emote_visuals[&1];
        assert!(released.release_at.is_some());
        assert_eq!(
            released
                .expressions(0.0, now + Duration::from_millis(150))
                .1,
            AvatarExpression::Calm
        );
        system.observe_emotes(&snap, 0.1, now + Duration::from_millis(160), false);
        assert!(system.emote_visuals.is_empty());
    }

    #[wasm_bindgen_test]
    fn old_pin_does_not_replay_bubble_and_reset_keeps_local_language() {
        let now = Instant::now();
        let mut system = NameplateSystem::default();
        let mut snap = snapshot(Some(event(1, true)));
        snap.tick = 500;
        system.observe_emotes(&snap, 0.1, now, false);
        let old = system.emote_visuals[&1];
        assert!(old.suppress_bubble);
        assert_eq!(
            old.motion(old.age(500, 0.1, 0.0), now, false).bubble_alpha,
            0.0
        );
        assert_eq!(
            old.expressions(50.0, now),
            (AvatarExpression::Calm, AvatarExpression::Angry, 1.0)
        );
        system.set_emote_texts(
            [
                "¡Saludos!".into(),
                "¡Ayuda!".into(),
                "¡Prepárate!".into(),
                "¡Victoria!".into(),
            ],
            [12.0, 0.0, 24.0, 0.0],
        );
        system.reset();
        assert!(system.emote_visuals.is_empty());
        assert_eq!(system.emote_texts[1], "¡Ayuda!");
        assert_eq!(system.safe_area[2], 24.0);
    }

    #[wasm_bindgen_test]
    fn expiry_finishes_the_return_without_restarting_it_at_different_frame_rates() {
        let now = Instant::now();
        for frame_ms in [8, 16, 33] {
            let mut system = NameplateSystem::default();
            let mut snap = snapshot(Some(event(1, false)));
            system.observe_emotes(&snap, 0.1, now, false);
            for elapsed_ms in (frame_ms..=3200).step_by(frame_ms as usize) {
                snap.tick = 1 + elapsed_ms / 100;
                if snap.tick >= 30 {
                    snap.players[0].active_emote = None;
                }
                system.observe_emotes(&snap, 0.1, now + Duration::from_millis(elapsed_ms), false);
                if let Some(visual) = system.emote_visuals.get(&1) {
                    assert!(visual.release_at.is_none(), "expiry restarted the return");
                }
            }
            assert!(system.emote_visuals.is_empty());
        }
    }
}

fn nameplate_world_size(tile_count: u32) -> f32 {
    (tile_count as f32).sqrt().clamp(0.2, 150.0)
}

fn nameplate_font_px(world_size: f32, zoom_scaled: f32, style: NameplateStyle) -> f32 {
    let world_px = world_size * NAMEPLATE_WORLD_SCALE * zoom_scaled;
    if style == NameplateStyle::HumanGhost {
        world_px.clamp(NAMEPLATE_MIN_FONT, NAMEPLATE_MAX_FONT)
    } else {
        world_px
    }
}

fn compute_nameplate_layout(
    style: NameplateStyle,
    show_bot_avatars: bool,
    center: ScreenPoint,
    scaled_size: f32,
    prepared_name: &sow_render::text::PreparedText,
    prepared_troops: &sow_render::text::PreparedText,
    status: NameplateStatus,
    dev: &DevConfig,
    sf: f32,
) -> NameplateLayout {
    let metrics = NameplateMetrics::compute(scaled_size, style, show_bot_avatars);
    let outline = crate::render::dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let badge_padding = outline.effect_padding() / sf;
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
    style: NameplateStyle,
    dev: &DevConfig,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    sf: f32,
) -> FullNameplate {
    let layout = layout.scaled_about(center, fit_scale);
    let font_scale = font_size_scale(dev.font_size_scale);
    let metrics = NameplateMetrics::compute(
        scaled_size,
        style,
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
        .then(|| prepare_avatar(text, player, campaign_avatar_slots, layout, sf));
    let (badges, badge_count) = prepare_badges(layout, sf);
    let name = dev.vfx_nameplate_names.then_some(TextPlan {
        position: {
            let anchor = layout.name_anchor(center);
            [anchor.0[0] * sf, anchor.0[1] * sf]
        },
        final_font_size: name_font_size * sf * fit_scale,
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
            },
        }
    });
    let instance_count = avatar
        .map_or(0, AvatarPlan::base_instance_count)
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
    let content = resolve_avatar_content(
        text,
        identity_ref,
        campaign_avatar_slots,
        AvatarExpression::Calm,
    );
    avatar_plan(center, radius, content)
}

fn resolve_avatar_content(
    text: &TextRenderer,
    identity: sow_core::player::AvatarIdentityRef<'_>,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    expression: AvatarExpression,
) -> AvatarContent {
    match identity {
        sow_core::player::AvatarIdentityRef::Portrait { slug, leader: None } => {
            let slot = Leader::ALL
                .iter()
                .find(|leader| sow_data::commerce::leader_id(**leader) == slug)
                .map(|leader| leader_avatar_slot(*leader, expression))
                .or_else(|| campaign_avatar_slots.get(slug).copied())
                .unwrap_or(AVATAR_FALLBACK_SLOT);
            text.avatar_uv(slot)
                .or_else(|| text.avatar_uv(AVATAR_FALLBACK_SLOT))
                .map_or(AvatarContent::None, AvatarContent::Sprite)
        }
        sow_core::player::AvatarIdentityRef::Portrait {
            leader: Some(leader),
            ..
        } => text
            .avatar_uv(leader_avatar_slot(leader, expression))
            .or_else(|| text.avatar_uv(AVATAR_FALLBACK_SLOT))
            .map_or(AvatarContent::None, AvatarContent::Sprite),
        sow_core::player::AvatarIdentityRef::Emblem { symbol } => AvatarContent::Emblem(symbol),
        sow_core::player::AvatarIdentityRef::Fallback => text
            .avatar_uv(AVATAR_FALLBACK_SLOT)
            .map_or(AvatarContent::None, AvatarContent::Sprite),
    }
}

fn prepare_avatar(
    text: &TextRenderer,
    player: &PlayerSnapshot,
    campaign_avatar_slots: &std::collections::HashMap<String, usize>,
    layout: NameplateLayout,
    sf: f32,
) -> AvatarPlan {
    let content = resolve_avatar_content(
        text,
        sow_core::player::avatar_identity_ref(player),
        campaign_avatar_slots,
        AvatarExpression::Calm,
    );
    let center = [
        layout.avatar_center().0[0] * sf,
        layout.avatar_center().0[1] * sf,
    ];
    let radius = layout.avatar_radius() * sf;
    avatar_plan(center, radius, content)
}

fn avatar_plan(center: [f32; 2], radius: f32, content: AvatarContent) -> AvatarPlan {
    AvatarPlan {
        center,
        radius,
        content,
        scale: [1.0; 2],
        blend: None,
    }
}

fn prepare_badges(
    layout: NameplateLayout,
    sf: f32,
) -> ([Option<BadgePlan>; NAMEPLATE_MAX_BADGES], usize) {
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
    state: &NameplateVisualState,
    plan: FullNameplate,
) {
    let before = text.instance_count();
    if let Some(avatar) = plan.avatar {
        paint_prepared_avatar(text, avatar, plan.emoji_outline, 1.0);
    }
    for badge in plan.badges[..plan.badge_count].iter().flatten() {
        // Every badge is a cell of the art/badges strip; none is a system emoji.
        let sprite = match badge.kind {
            BadgeKind::Request => NameplateStatusSprite::Request,
            BadgeKind::Allied => NameplateStatusSprite::Allied,
            BadgeKind::Traitor => NameplateStatusSprite::Traitor,
            BadgeKind::Rank1 => NameplateStatusSprite::Rank1,
            BadgeKind::Rank2 => NameplateStatusSprite::Rank2,
            BadgeKind::Rank3 => NameplateStatusSprite::Rank3,
            BadgeKind::Star => NameplateStatusSprite::You,
            BadgeKind::Disconnected => NameplateStatusSprite::Disconnected,
        };
        text.push_status_sprite(sprite, badge.center, badge.diameter, badge.tint);
    }
    if let Some(name) = plan.name {
        let _ = text.push_prepared_text(
            &state.prepared_name,
            name.position,
            name.final_font_size,
            NAMEPLATE_TEXT_COLOR,
            plan.text_style,
        );
    }
    if let Some(troops) = plan.troops {
        text.push_troop_icon(
            troops.icon_center,
            troops.icon_diameter * 0.5,
            1.0,
            plan.emoji_outline,
        );
        let _ = text.push_prepared_text(
            &state.prepared_troops,
            troops.text.position,
            troops.text.final_font_size,
            NAMEPLATE_TEXT_COLOR,
            plan.text_style,
        );
    }
    debug_assert!(
        text.instance_count().saturating_sub(before)
            <= plan.instance_count + plan.avatar.map_or(0, AvatarPlan::transition_instance_count)
    );
}

pub(crate) fn paint_prepared_avatar(
    text: &mut TextRenderer,
    avatar: AvatarPlan,
    outline: sow_render::text::OutlineStyle,
    alpha: f32,
) {
    let tint = [1.0, 1.0, 1.0, alpha];
    match avatar.content {
        AvatarContent::Sprite(uv) => {
            let size = avatar.scale.map(|scale| avatar.radius * 2.0 * scale);
            if let Some((previous, mix)) = avatar.blend {
                text.push_sprite(avatar.center, size, previous, [1.0, 1.0, 1.0, alpha]);
                text.push_sprite(avatar.center, size, uv, [1.0, 1.0, 1.0, alpha * mix]);
            } else {
                text.push_sprite(avatar.center, size, uv, tint);
            }
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
        AvatarContent::None => text.push_disc(
            avatar.center,
            avatar.radius * avatar.scale[0],
            [
                AVATAR_PLACEHOLDER_COLOR[0],
                AVATAR_PLACEHOLDER_COLOR[1],
                AVATAR_PLACEHOLDER_COLOR[2],
                alpha,
            ],
        ),
    }
}

fn paint_emote_bubble(
    text: &mut TextRenderer,
    prepared: &sow_render::text::PreparedText,
    avatar: AvatarPlan,
    motion: EmoteMotion,
    accent: [f32; 4],
    input: &InputState,
    insets: [f32; 4],
    sf: f32,
) {
    let safe = insets.map(|v| v * sf + 8.0 * sf);
    let max_width = (240.0 * sf).min((input.screen_w - safe[1] - safe[3]).max(1.0));
    let mut font = 14.0 * sf;
    let measured = prepared.measure_at(font);
    font *= ((max_width - 24.0 * sf).max(1.0) / measured.width.max(1.0)).min(1.0);
    let measured = prepared.measure_at(font);
    let Some(bounds) = prepared.visual_bounds_at(font) else {
        return;
    };
    let width = (measured.width + 24.0 * sf).min(max_width);
    let height = (bounds[3] - bounds[1] + 18.0 * sf).max(32.0 * sf);
    let above = avatar.center[1] - avatar.radius - height - 10.0 * sf >= safe[0];
    let direction = if above { -1.0 } else { 1.0 };
    let center = [
        avatar.center[0].clamp(
            safe[3] + width * 0.5,
            (input.screen_w - safe[1] - width * 0.5).max(safe[3] + width * 0.5),
        ),
        (avatar.center[1] + direction * (avatar.radius + height * 0.5 + 10.0 * sf)).clamp(
            safe[0] + height * 0.5,
            (input.screen_h - safe[2] - height * 0.5).max(safe[0] + height * 0.5),
        ),
    ];
    let alpha = motion.bubble_alpha;
    let size = [width * motion.bubble_scale, height * motion.bubble_scale];
    let tip_inset = (12.0 * sf).min(size[0] * 0.45);
    let tip_x = avatar.center[0].clamp(
        center[0] - size[0] * 0.5 + tip_inset,
        center[0] + size[0] * 0.5 - tip_inset,
    );
    let tip_y = center[1] - direction * (size[1] * 0.5 + 3.0 * sf);
    text.push_rounded_rect(
        [center[0], center[1] + 3.0 * sf],
        [size[0] + 4.0 * sf, size[1] + 4.0 * sf],
        0.2,
        [0.0, 0.0, 0.0, 0.18 * alpha],
        [0.0; 4],
        0.0,
    );
    text.push_rounded_rect(
        center,
        size,
        0.2,
        [0.97, 0.95, 0.90, alpha],
        [accent[0], accent[1], accent[2], 0.9 * alpha],
        1.2 * sf,
    );
    text.push_triangle(
        [tip_x, tip_y],
        [12.0 * sf, -direction * 8.0 * sf],
        [accent[0], accent[1], accent[2], alpha],
    );
    text.push_triangle(
        [tip_x, tip_y - direction * sf],
        [8.0 * sf, -direction * 6.0 * sf],
        [0.97, 0.95, 0.90, alpha],
    );
    let _ = text.push_prepared_text(
        prepared,
        [
            center[0],
            center[1] - (bounds[1] + bounds[3]) * 0.5 * motion.bubble_scale,
        ],
        font * motion.bubble_scale,
        [0.07, 0.09, 0.12, alpha],
        sow_render::text::TextPaintStyle {
            face_dilate: 0.0,
            outline: sow_render::text::OutlineStyle::NONE,
            underlay_softness: 0.0,
        },
    );
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

#[cfg(test)]
mod spawn_center_tests {
    use super::*;

    fn state(slide_center: bool) -> NameplateVisualState {
        NameplateVisualState {
            from_center: MapPoint([4.0, 5.0]),
            to_center: MapPoint([6.0, 5.0]),
            from_land_bounds: WorldRect([3.0, 4.0, 5.0, 6.0]),
            from_size: 10.0,
            to_size: 10.0,
            land_bounds: WorldRect([3.0, 4.0, 7.0, 6.0]),
            center_changed_at: None,
            slide_center,
            source_name: String::new(),
            player_type: PlayerType::Human,
            display_name: String::new(),
            troops_bits: u64::MAX,
            troops_text: String::new(),
            troops_scratch: String::new(),
            prepared_name: sow_render::text::PreparedText::default(),
            prepared_troops: sow_render::text::PreparedText::default(),
            text_style_key: None,
        }
    }

    #[test]
    fn local_spawn_sticks_then_eases_inside_the_new_land_anchor() {
        let now = Instant::now();
        let mut visual = state(true);
        visual.center_changed_at = Some(now);

        let (held_center, held_bounds, _) =
            spawn_center_visual(&visual, now + Duration::from_millis(119));
        assert_eq!(held_center, visual.from_center);
        assert_eq!(held_bounds, visual.from_land_bounds);

        let (moving_center, moving_bounds, scale) =
            spawn_center_visual(&visual, now + Duration::from_millis(210));
        assert_eq!(moving_center, MapPoint([5.0, 5.0]));
        assert_eq!(moving_bounds, visual.land_bounds);
        assert!((scale - 0.97).abs() < 0.001);

        let (arrived_center, _, _) = spawn_center_visual(&visual, now + Duration::from_millis(300));
        assert_eq!(arrived_center, visual.to_center);
    }

    #[test]
    fn local_spawn_snaps_after_sticky_hold_when_the_route_leaves_land() {
        let now = Instant::now();
        let mut visual = state(false);
        visual.to_center = MapPoint([12.0, 5.0]);
        visual.land_bounds = WorldRect([11.0, 4.0, 13.0, 6.0]);
        visual.center_changed_at = Some(now);

        let (held_center, held_bounds, _) =
            spawn_center_visual(&visual, now + Duration::from_millis(119));
        assert_eq!(held_center, visual.from_center);
        assert_eq!(held_bounds, visual.from_land_bounds);

        let (arrived_center, arrived_bounds, _) =
            spawn_center_visual(&visual, now + Duration::from_millis(120));
        assert_eq!(arrived_center, visual.to_center);
        assert_eq!(arrived_bounds, visual.land_bounds);
        assert!(!center_inside_land(visual.from_center, visual.land_bounds));
    }
}
