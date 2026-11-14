//! Owns live/death avatar presentation.
use super::overlays::world_to_screen_values;
use crate::app::{InputState, SimState};
use crate::render::avatars::AvatarSlots;
use crate::render::gpu::TextRenderer;
use crate::render::motion::{
    EmoteMotion, emote_motion, smoothstep, spring_overshoot, troop_transfer_motion,
};
use crate::theme::dev_config::DevConfig;
use sow_core::emote::{ActiveEmote, AvatarExpression, EMOTE_DURATION_SECS, EMOTES, EmoteKind};
use sow_core::player::PlayerType;
use sow_core::protocol::{PlayerSnapshot, SimSnapshot};
use sow_render::nameplate::{
    BadgeKind, MapPoint, NAMEPLATE_MAX_BADGES, NAMEPLATE_SAMPLE_TICKS, NameplateCapacityPlan,
    NameplateLandCache, NameplateLayout, NameplateMetrics, NameplatePresentation, NameplateStatus,
    NameplateStyle, ScreenPoint, WorldRect, assign_nameplate_capacity, fit_size_to_land,
    fitted_font_px, fog_allows_nameplate, font_size_scale,
    nameplate_visible_after_viewport_and_fog, sample_due as nameplate_sample_due,
    size_needs_interpolation as nameplate_size_needs_interpolation,
};
use sow_render::text::NameplateStatusSprite;
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

const NAMEPLATE_WORLD_SCALE: f32 = 0.05;
const NAMEPLATE_TEXT_COLOR: [f32; 4] = [1.0; 4];
const NAMEPLATE_MIN_FONT: f32 = 8.0;
const NAMEPLATE_MAX_FONT: f32 = 32.0;
const NAMEPLATE_MAX_CATCHUP_TICKS: u64 = NAMEPLATE_SAMPLE_TICKS * 2;
const SPAWN_STICKY_HOLD: Duration = Duration::from_millis(120);
const SPAWN_SETTLE_DURATION: Duration = Duration::from_millis(180);
const LAND_CACHE_ENTRY_BUDGET: Duration = Duration::from_millis(8);
const LOD_DOT_RADIUS: f32 = 2.0;
const LOD_STAR_DIAMETER: f32 = 12.0;
const EMOTE_BUBBLE_FONT_RATIO: f32 = 0.22;
const EMOTE_BUBBLE_MIN_FONT_SIZE: f32 = 9.0;
const EMOTE_BUBBLE_HORIZONTAL_PADDING_RATIO: f32 = 0.16;
const EMOTE_BUBBLE_VERTICAL_PADDING_RATIO: f32 = 0.12;
const EMOTE_BUBBLE_MIN_HEIGHT_RATIO: f32 = 0.46;
const EMOTE_BUBBLE_MAX_WIDTH_RATIO: f32 = 2.5;
const EMOTE_BUBBLE_TAIL_WIDTH_RATIO: f32 = 0.30;
const EMOTE_BUBBLE_TAIL_LENGTH_RATIO: f32 = 0.22;
const EMOTE_BUBBLE_INNER_TAIL_WIDTH_RATIO: f32 = 0.23;
const EMOTE_BUBBLE_INNER_TAIL_LENGTH_RATIO: f32 = 0.16;
const EMOTE_BUBBLE_BODY_OFFSET_RATIO: f32 = 0.35;
const STATUS_BADGE_COUNT: usize = 7;
const STATUS_BADGE_ENTER_SECS: f32 = 0.52;
const STATUS_BADGE_EXIT_SECS: f32 = 0.65;
const STATUS_BADGE_EXIT_SCALE: [f32; 2] = [1.50, 1.42];
const STATUS_BADGE_MAX_SCALE: f32 = 1.60;
const STATUS_BADGE_ENTER_PREP_END: f32 = 0.15;
const STATUS_BADGE_ENTER_POP_END: f32 = 0.58;
const STATUS_BADGE_EXIT_PREP_END: f32 = 0.12;
const STATUS_BADGE_EXIT_FADE_START: f32 = 0.34;
const NAMEPLATE_RENDER_BADGE_CAPACITY: usize = NAMEPLATE_MAX_BADGES + STATUS_BADGE_COUNT;
const TROOP_TRANSFER_TEXT_DELAY: f32 = 0.08;
const TROOP_TRANSFER_GLYPH_DELAY: f32 = 0.035;
const AVATAR_TRANSITION_EXIT_SECS: f32 = 0.15;
const AVATAR_TRANSITION_ENTER_SECS: f32 = 0.5;
const AVATAR_TRANSITION_CELEBRATE_SECS: f32 = 0.65;
const INCOMING_FORCE_LOSS_ENTER_RATIO: f64 = 0.50;
const INCOMING_FORCE_LOSS_EXIT_RATIO: f64 = 0.35;

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
    troop_change_at: Option<Instant>,
    troop_change_positive: bool,
}

impl NameplateVisualState {
    fn troop_change_age(&self, now: Instant, reduced_motion: bool) -> Option<f32> {
        if reduced_motion {
            return None;
        }
        let age = now.duration_since(self.troop_change_at?).as_secs_f32();
        let duration = TROOP_TRANSFER_TEXT_DELAY
            + TROOP_TRANSFER_GLYPH_DELAY
                * self.prepared_troops.instance_count().saturating_sub(1) as f32
            + 0.5;
        (age < duration).then_some(age)
    }
}

struct AvatarEntry {
    identity: sow_core::player::AvatarIdentity,
    loaded: bool,
    started_at: Option<Instant>,
}

fn observe_avatar_entry(
    entries: &mut HashMap<u16, AvatarEntry>,
    player_id: u16,
    identity: sow_core::player::AvatarIdentity,
    loaded: bool,
    now: Instant,
    reduced_motion: bool,
) -> Option<f32> {
    let entry = entries.entry(player_id).or_insert_with(|| AvatarEntry {
        identity: identity.clone(),
        loaded: false,
        started_at: None,
    });
    if entry.identity != identity {
        *entry = AvatarEntry {
            identity,
            loaded: false,
            started_at: None,
        };
    }
    if !loaded {
        entry.loaded = false;
        entry.started_at = None;
        return None;
    }
    if !entry.loaded {
        entry.loaded = true;
        entry.started_at = (!reduced_motion).then_some(now);
    } else if reduced_motion {
        entry.started_at = None;
    }
    let age = now
        .saturating_duration_since(entry.started_at?)
        .as_secs_f32();
    (age < AVATAR_TRANSITION_CELEBRATE_SECS).then_some(age)
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
    land_cache_build_started_at: Option<Instant>,
    land_cache_build_cpu: Duration,
    land_cache_build_max_step: Duration,
    last_land_cache_build: Option<(u64, u64, u64)>,
    local_spawn_tick: Option<u64>,
    frame_plans: Vec<PreparedNameplate>,
    tutorial_target_hit_bounds: Option<(u16, [f32; 4])>,
    emote_visuals: HashMap<u16, AvatarEmote>,
    avatar_entries: HashMap<u16, AvatarEntry>,
    badge_animations: HashMap<u16, PlayerStatusBadgeAnimations>,
    emote_tick: Option<u64>,
    campaign_base_expressions: HashMap<u16, AvatarExpression>,
    campaign_base_expressions_dirty: bool,
    avatar_base_tick: Option<u64>,
    incoming_force_at_start: HashMap<u16, f64>,
    fear_under_attack_players: HashSet<u16>,
    avatar_bases: HashMap<u16, AvatarExpression>,
    avatar_transitions: HashMap<u16, AvatarTransition>,
    next_avatar_transition_id: u64,
    avatar_presentations: HashMap<u16, AvatarPresentation>,
    pub(crate) avatar_revision: u64,
    scratch_combatants: HashSet<u16>,
    scratch_incoming_targets: HashSet<u16>,
    scratch_committed_troops: HashMap<u16, f64>,
    scratch_incoming_force_at_start: HashMap<u16, f64>,
    scratch_fear_under_attack_players: HashSet<u16>,
    scratch_avatar_bases: HashMap<u16, AvatarExpression>,
    scratch_alive_ids: HashSet<u16>,
    scratch_presentations: HashMap<u16, AvatarPresentation>,
    emote_texts: [String; 4],
    prepared_emotes: [sow_render::text::PreparedText; 4],
    emote_text_style_key: Option<u32>,
    campaign_emotes: HashMap<u16, CampaignEmoteCue>,
    safe_area: [f32; 4],
}

struct CampaignEmoteCue {
    player_id: u16,
    event_id: u64,
    token: String,
    message: String,
    prepared: Option<(u32, sow_render::text::PreparedText)>,
    visible: bool,
}

#[derive(Clone, Copy)]
struct PreparedNameplate {
    player_index: usize,
    has_human_priority: bool,
    full: Option<FullNameplate>,
    marker: LodMarker,
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
enum LodMarker {
    Dot {
        center: ScreenPoint,
        color: [f32; 4],
        scale: f32,
    },
    Star {
        center: ScreenPoint,
    },
}

#[derive(Clone, Copy)]
struct FullNameplate {
    player_id: u16,
    avatar: Option<AvatarPlan>,
    avatar_entry_age: Option<f32>,
    badges: [Option<BadgePlan>; NAMEPLATE_RENDER_BADGE_CAPACITY],
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
    alpha: f32,
    blend: Option<([f32; 4], f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AvatarTransitionStage {
    Exit,
    Enter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AvatarTransitionPresentation {
    pub id: u64,
    pub from: AvatarExpression,
    pub to: AvatarExpression,
    pub stage: AvatarTransitionStage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AvatarPresentation {
    pub expression: AvatarExpression,
    pub manual_emote_active: bool,
    pub transition: Option<AvatarTransitionPresentation>,
}

#[derive(Clone, Copy)]
struct AvatarTransition {
    id: u64,
    from: AvatarExpression,
    to: AvatarExpression,
    stage: AvatarTransitionStage,
    stage_started_at: Instant,
}

impl AvatarTransition {
    fn duration(self) -> Duration {
        Duration::from_secs_f32(match self.stage {
            AvatarTransitionStage::Exit => AVATAR_TRANSITION_EXIT_SECS,
            AvatarTransitionStage::Enter => avatar_transition_enter_secs(self.to),
        })
    }

    fn progress(self, now: Instant) -> f32 {
        (now.saturating_duration_since(self.stage_started_at).as_secs_f32()
            / self.duration().as_secs_f32().max(0.001))
        .clamp(0.0, 1.0)
    }

    fn presentation(self) -> AvatarTransitionPresentation {
        AvatarTransitionPresentation {
            id: self.id,
            from: self.from,
            to: self.to,
            stage: self.stage,
        }
    }
}

fn avatar_transition_enter_secs(expression: AvatarExpression) -> f32 {
    if expression == AvatarExpression::Happy {
        AVATAR_TRANSITION_CELEBRATE_SECS
    } else {
        AVATAR_TRANSITION_ENTER_SECS
    }
}

fn avatar_transition_crossfade(progress: f32) -> f32 {
    0.5 + 0.5 * smoothstep(progress)
}

fn avatar_transition_emote(expression: AvatarExpression) -> EmoteKind {
    match expression {
        AvatarExpression::Happy => EmoteKind::Celebrate,
        AvatarExpression::Fear => EmoteKind::Help,
        AvatarExpression::Angry => EmoteKind::Threat,
        AvatarExpression::Calm => EmoteKind::Greet,
    }
}

impl AvatarPlan {
    fn base_instance_count(self) -> usize {
        usize::from(matches!(self.content, AvatarContent::Sprite(_)))
    }

    fn transition_instance_count(self) -> usize {
        usize::from(matches!(self.content, AvatarContent::Sprite(_)) && self.blend.is_some())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AvatarContent {
    Sprite([f32; 4]),
    None,
}

#[derive(Clone, Copy)]
struct AvatarEmote {
    event: ActiveEmote,
    previous: AvatarExpression,
    from_motion: EmoteMotion,
    observed_at: Instant,
    observed_age: f32,
    release_at: Option<Instant>,
    suppress_bubble: bool,
}

impl AvatarEmote {
    fn age(self, now: Instant) -> f32 {
        self.observed_age
            + now
                .saturating_duration_since(self.observed_at)
                .as_secs_f32()
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
        let mut motion = emote_motion(self.event.kind, age, reduced, self.event.pinned);
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

    fn expressions(
        self,
        age: f32,
        now: Instant,
        idle: AvatarExpression,
    ) -> (AvatarExpression, AvatarExpression, f32) {
        let expression = self.event.kind.definition().expression;
        if let Some(released) = self.release_at {
            return (
                self.previous,
                idle,
                smoothstep(now.duration_since(released).as_secs_f32() / 0.15),
            );
        }
        if !self.event.pinned && age >= EMOTE_DURATION_SECS - 0.15 {
            return (
                expression,
                idle,
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
    transfer_age: Option<f32>,
}

#[derive(Clone, Copy)]
struct BadgePlan {
    kind: BadgeKind,
    center: [f32; 2],
    size: [f32; 2],
    tint: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BadgeMotion {
    scale: [f32; 2],
    alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BadgeTransition {
    Enter,
    Exit,
}

#[derive(Clone, Copy)]
struct BadgeAnimation {
    started_at: Instant,
    duration_secs: f32,
    transition: BadgeTransition,
    from_scale: [f32; 2],
    from_alpha: f32,
}

impl BadgeAnimation {
    fn values(self, now: Instant, reduced_motion: bool) -> BadgeMotion {
        let progress = (now.duration_since(self.started_at).as_secs_f32()
            / self.duration_secs.max(f32::EPSILON))
        .clamp(0.0, 1.0);
        if progress >= 1.0 {
            return match self.transition {
                BadgeTransition::Enter => BadgeMotion {
                    scale: [1.0; 2],
                    alpha: 1.0,
                },
                BadgeTransition::Exit => BadgeMotion {
                    scale: STATUS_BADGE_EXIT_SCALE,
                    alpha: 0.0,
                },
            };
        }

        let mix = |from: f32, to: f32, t: f32| from + (to - from) * t;
        if reduced_motion {
            let alpha = match self.transition {
                BadgeTransition::Enter => mix(self.from_alpha, 1.0, smoothstep(progress)),
                BadgeTransition::Exit => {
                    self.from_alpha
                        * (1.0
                            - smoothstep(
                                (progress - STATUS_BADGE_EXIT_FADE_START)
                                    / (1.0 - STATUS_BADGE_EXIT_FADE_START),
                            ))
                }
            };
            return BadgeMotion {
                scale: [1.0; 2],
                alpha: alpha.clamp(0.0, 1.0),
            };
        }

        let (scale, alpha) = match self.transition {
            BadgeTransition::Enter => {
                let alpha = if progress < STATUS_BADGE_ENTER_PREP_END {
                    mix(
                        self.from_alpha,
                        0.42,
                        smoothstep(progress / STATUS_BADGE_ENTER_PREP_END),
                    )
                } else if progress < STATUS_BADGE_ENTER_POP_END {
                    mix(
                        0.42,
                        1.0,
                        smoothstep(
                            (progress - STATUS_BADGE_ENTER_PREP_END)
                                / (STATUS_BADGE_ENTER_POP_END - STATUS_BADGE_ENTER_PREP_END),
                        ),
                    )
                } else {
                    1.0
                };
                let scale = if progress < STATUS_BADGE_ENTER_PREP_END {
                    let t = smoothstep(progress / STATUS_BADGE_ENTER_PREP_END);
                    [
                        mix(self.from_scale[0], 0.76, t),
                        mix(self.from_scale[1], 0.68, t),
                    ]
                } else if progress < STATUS_BADGE_ENTER_POP_END {
                    let t = smoothstep(
                        (progress - STATUS_BADGE_ENTER_PREP_END)
                            / (STATUS_BADGE_ENTER_POP_END - STATUS_BADGE_ENTER_PREP_END),
                    );
                    [mix(0.76, 1.20, t), mix(0.68, 1.25, t)]
                } else {
                    let t = (progress - STATUS_BADGE_ENTER_POP_END)
                        / (1.0 - STATUS_BADGE_ENTER_POP_END);
                    let settle = 1.0 - spring_overshoot(t);
                    [1.0 + 0.20 * settle, 1.0 + 0.25 * settle]
                };
                (scale, alpha)
            }
            BadgeTransition::Exit => {
                let scale = if progress < STATUS_BADGE_EXIT_PREP_END {
                    let t = smoothstep(progress / STATUS_BADGE_EXIT_PREP_END);
                    [
                        mix(self.from_scale[0], 0.86, t),
                        mix(self.from_scale[1], 0.78, t),
                    ]
                } else {
                    let t = (progress - STATUS_BADGE_EXIT_PREP_END)
                        / (1.0 - STATUS_BADGE_EXIT_PREP_END);
                    let spring = spring_overshoot(t);
                    [
                        0.86 + (STATUS_BADGE_EXIT_SCALE[0] - 0.86) * spring,
                        0.78 + (STATUS_BADGE_EXIT_SCALE[1] - 0.78) * spring,
                    ]
                };
                let alpha = self.from_alpha
                    * (1.0
                        - smoothstep(
                            (progress - STATUS_BADGE_EXIT_FADE_START)
                                / (1.0 - STATUS_BADGE_EXIT_FADE_START),
                        ));
                (scale, alpha)
            }
        };

        BadgeMotion {
            scale: scale.map(|axis| axis.clamp(0.0, STATUS_BADGE_MAX_SCALE)),
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    fn is_finished(self, now: Instant) -> bool {
        now.duration_since(self.started_at).as_secs_f32() >= self.duration_secs
    }
}

#[derive(Clone, Copy, Default)]
struct BadgeGeometry {
    center_from_avatar: [f32; 2],
    diameter: f32,
    tint: [f32; 4],
}

#[derive(Clone, Copy, Default)]
struct StatusBadgeAnimation {
    present: bool,
    animation: Option<BadgeAnimation>,
    geometry: Option<BadgeGeometry>,
}

#[derive(Clone, Copy, Default)]
struct PlayerStatusBadgeAnimations {
    observed: bool,
    badges: [StatusBadgeAnimation; STATUS_BADGE_COUNT],
}

impl PlayerStatusBadgeAnimations {
    fn observe(&mut self, present: [bool; STATUS_BADGE_COUNT], now: Instant, reduced_motion: bool) {
        if !self.observed {
            self.observed = true;
            for (badge, is_present) in self.badges.iter_mut().zip(present) {
                badge.present = is_present;
            }
            return;
        }

        for (badge, is_present) in self.badges.iter_mut().zip(present) {
            if badge.present != is_present {
                let from = badge.animation.map_or(
                    if badge.present {
                        BadgeMotion {
                            scale: [1.0; 2],
                            alpha: 1.0,
                        }
                    } else {
                        BadgeMotion {
                            scale: [0.72; 2],
                            alpha: 0.0,
                        }
                    },
                    |animation| animation.values(now, reduced_motion),
                );
                let entering = is_present;
                badge.animation = Some(BadgeAnimation {
                    started_at: now,
                    duration_secs: if entering {
                        STATUS_BADGE_ENTER_SECS
                    } else {
                        STATUS_BADGE_EXIT_SECS
                    },
                    transition: if entering {
                        BadgeTransition::Enter
                    } else {
                        BadgeTransition::Exit
                    },
                    from_scale: from.scale,
                    from_alpha: from.alpha,
                });
                badge.present = is_present;
            } else if badge
                .animation
                .is_some_and(|animation| animation.is_finished(now))
            {
                badge.animation = None;
                if !badge.present {
                    badge.geometry = None;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmoteBubbleQuadrant {
    UpperRight,
    UpperLeft,
    LowerRight,
    LowerLeft,
}

impl EmoteBubbleQuadrant {
    fn signs(self) -> (f32, f32) {
        match self {
            Self::UpperRight => (1.0, -1.0),
            Self::UpperLeft => (-1.0, -1.0),
            Self::LowerRight => (1.0, 1.0),
            Self::LowerLeft => (-1.0, 1.0),
        }
    }
}

#[derive(Clone, Copy)]
struct EmoteBubbleLayout {
    center: [f32; 2],
    size: [f32; 2],
    font_size: f32,
    text_bounds: [f32; 4],
    outer_tail_center: [f32; 2],
    outer_tail_size: [f32; 2],
    inner_tail_center: [f32; 2],
    inner_tail_size: [f32; 2],
}

impl NameplateSystem {
    pub(crate) fn prepare_land_cache(
        &mut self,
        map: &sow_core::map::GameMap,
        tick: u64,
        players: &[sow_core::protocol::PlayerSnapshot],
    ) -> bool {
        let cache_covers_current_players = self.land_cache_covers_players(players);
        if self.land_cache.has_result_for(map)
            && (self.land_cache.is_ready_for(map) || cache_covers_current_players)
        {
            return true;
        }
        if self.land_cache_build_started_at.is_none() {
            self.land_cache_build_started_at = Some(Instant::now());
            self.land_cache_build_cpu = Duration::ZERO;
            self.land_cache_build_max_step = Duration::ZERO;
        }

        let step_started_at = Instant::now();
        let result = self
            .land_cache
            .step_rebuild(map, tick, LAND_CACHE_ENTRY_BUDGET);
        let step_duration = step_started_at.elapsed();
        self.land_cache_build_cpu += step_duration;
        self.land_cache_build_max_step = self.land_cache_build_max_step.max(step_duration);

        match result {
            Ok(true) => {
                let wall = self
                    .land_cache_build_started_at
                    .take()
                    .map_or(Duration::ZERO, |started_at| started_at.elapsed());
                self.last_land_cache_build = Some((
                    self.land_cache_build_cpu.as_micros() as u64,
                    wall.as_micros() as u64,
                    self.land_cache_build_max_step.as_micros() as u64,
                ));
                self.land_cache.is_ready_for(map) || self.land_cache_covers_players(players)
            }
            Ok(false) => false,
            Err(()) => {
                self.land_cache_build_started_at = None;
                self.land_cache_build_cpu = Duration::ZERO;
                self.land_cache_build_max_step = Duration::ZERO;
                false
            }
        }
    }

    fn land_cache_covers_players(&self, players: &[sow_core::protocol::PlayerSnapshot]) -> bool {
        players.iter().all(|player| {
            !player.alive || player.tile_count == 0 || self.land_cache.has_landmass(player.id)
        })
    }

    pub(crate) fn take_land_cache_build_stats(&mut self) -> Option<(u64, u64, u64)> {
        self.last_land_cache_build.take()
    }

    fn observe_avatar_bases(
        &mut self,
        snapshot: &SimSnapshot,
        now: Instant,
        reduced_motion: bool,
    ) -> bool {
        if self.avatar_base_tick == Some(snapshot.tick) && !self.campaign_base_expressions_dirty {
            return false;
        }
        let mut combatants = std::mem::take(&mut self.scratch_combatants);
        let mut incoming_attack_targets = std::mem::take(&mut self.scratch_incoming_targets);
        let mut committed_troops = std::mem::take(&mut self.scratch_committed_troops);
        combatants.clear();
        incoming_attack_targets.clear();
        committed_troops.clear();
        for attack in &snapshot.attacks {
            if attack.owner_id != 0 && attack.troops.is_finite() && attack.troops > 0.0 {
                *committed_troops.entry(attack.owner_id).or_default() += attack.troops;
            }
            if attack.owner_id != 0
                && attack.target_owner != 0
                && attack.owner_id != attack.target_owner
                && attack.troops.is_finite()
                && attack.troops > 0.0
                && !attack.retreating
            {
                combatants.insert(attack.owner_id);
                combatants.insert(attack.target_owner);
                incoming_attack_targets.insert(attack.target_owner);
            }
        }
        let mut incoming_force_at_start = std::mem::take(&mut self.scratch_incoming_force_at_start);
        let mut fear_under_attack_players =
            std::mem::take(&mut self.scratch_fear_under_attack_players);
        incoming_force_at_start.clear();
        fear_under_attack_players.clear();
        for player in snapshot.players.iter().filter(|player| player.alive) {
            if !incoming_attack_targets.contains(&player.id) {
                continue;
            }
            // La fuerza enviada sigue siendo tuya; no debe parecer una baja al contraatacarte.
            let reserve = if player.troops.is_finite() {
                player.troops.max(0.0)
            } else {
                0.0
            };
            let current_force = reserve + committed_troops.get(&player.id).copied().unwrap_or(0.0);
            let start_force = self
                .incoming_force_at_start
                .get(&player.id)
                .copied()
                .unwrap_or(current_force);
            incoming_force_at_start.insert(player.id, start_force);
            if !start_force.is_finite() || start_force <= 0.0 || !current_force.is_finite() {
                continue;
            }
            let loss_ratio = ((start_force - current_force).max(0.0)) / start_force;
            let threshold = if self.fear_under_attack_players.contains(&player.id) {
                INCOMING_FORCE_LOSS_EXIT_RATIO
            } else {
                INCOMING_FORCE_LOSS_ENTER_RATIO
            };
            if loss_ratio >= threshold {
                fear_under_attack_players.insert(player.id);
            }
        }

        let mut avatar_bases = std::mem::take(&mut self.scratch_avatar_bases);
        avatar_bases.clear();
        for player in &snapshot.players {
            let expression = self
                .campaign_base_expressions
                .get(&player.id)
                .copied()
                .unwrap_or(if fear_under_attack_players.contains(&player.id) {
                    AvatarExpression::Fear
                } else if combatants.contains(&player.id) {
                    AvatarExpression::Angry
                } else {
                    AvatarExpression::Calm
                });
            if let Some(previous) = self.avatar_bases.get(&player.id).copied()
                && previous != expression
            {
                self.request_avatar_transition(player.id, previous, expression, now, reduced_motion);
            }
            avatar_bases.insert(player.id, expression);
        }
        std::mem::swap(&mut self.avatar_bases, &mut avatar_bases);
        std::mem::swap(&mut self.incoming_force_at_start, &mut incoming_force_at_start);
        std::mem::swap(
            &mut self.fear_under_attack_players,
            &mut fear_under_attack_players,
        );
        self.scratch_combatants = combatants;
        self.scratch_incoming_targets = incoming_attack_targets;
        self.scratch_committed_troops = committed_troops;
        self.scratch_incoming_force_at_start = incoming_force_at_start;
        self.scratch_fear_under_attack_players = fear_under_attack_players;
        self.scratch_avatar_bases = avatar_bases;
        self.avatar_base_tick = Some(snapshot.tick);
        self.campaign_base_expressions_dirty = false;
        true
    }

    pub(crate) fn set_campaign_base_expressions(
        &mut self,
        expressions: HashMap<u16, AvatarExpression>,
    ) {
        if self.campaign_base_expressions != expressions {
            self.campaign_base_expressions = expressions;
            self.campaign_base_expressions_dirty = true;
        }
    }

    fn request_avatar_transition(
        &mut self,
        player_id: u16,
        from: AvatarExpression,
        to: AvatarExpression,
        now: Instant,
        reduced_motion: bool,
    ) {
        if reduced_motion || from == to {
            self.avatar_transitions.remove(&player_id);
            return;
        }
        if self.avatar_transitions.contains_key(&player_id) {
            let cancel = {
                let transition = self.avatar_transitions.get_mut(&player_id).unwrap();
                match transition.stage {
                    AvatarTransitionStage::Exit if to == transition.from => true,
                    AvatarTransitionStage::Exit => {
                        transition.to = to;
                        false
                    }
                    AvatarTransitionStage::Enter if to != transition.to => {
                        let from = transition.to;
                        *transition = AvatarTransition {
                            id: self.next_avatar_transition_id,
                            from,
                            to,
                            stage: AvatarTransitionStage::Exit,
                            stage_started_at: now,
                        };
                        self.next_avatar_transition_id = self.next_avatar_transition_id.wrapping_add(1);
                        false
                    }
                    AvatarTransitionStage::Enter => false,
                }
            };
            if cancel {
                self.avatar_transitions.remove(&player_id);
            }
            return;
        }
        self.avatar_transitions.insert(
            player_id,
            AvatarTransition {
                id: self.next_avatar_transition_id,
                from,
                to,
                stage: AvatarTransitionStage::Exit,
                stage_started_at: now,
            },
        );
        self.next_avatar_transition_id = self.next_avatar_transition_id.wrapping_add(1);
    }

    fn advance_avatar_transitions(
        &mut self,
        snapshot: &SimSnapshot,
        now: Instant,
        reduced_motion: bool,
    ) -> bool {
        if self.avatar_transitions.is_empty() {
            return false;
        }
        if reduced_motion {
            self.avatar_transitions.clear();
            return true;
        }
        let mut alive = std::mem::take(&mut self.scratch_alive_ids);
        alive.clear();
        alive.extend(
            snapshot
                .players
                .iter()
                .filter(|player| player.alive)
                .map(|player| player.id),
        );
        self.avatar_transitions.retain(|player_id, transition| {
            if !alive.contains(player_id) {
                return false;
            }
            if now.saturating_duration_since(transition.stage_started_at) < transition.duration() {
                return true;
            }
            match transition.stage {
                AvatarTransitionStage::Exit => {
                    transition.stage = AvatarTransitionStage::Enter;
                    transition.stage_started_at =
                        transition.stage_started_at + Duration::from_secs_f32(AVATAR_TRANSITION_EXIT_SECS);
                    now.saturating_duration_since(transition.stage_started_at) < transition.duration()
                }
                AvatarTransitionStage::Enter => false,
            }
        });
        self.scratch_alive_ids = alive;
        true
    }

    pub(crate) fn manual_emote_active(
        &self,
        player: &sow_core::player::Player,
        tick: u64,
        tick_rate_ms: f32,
    ) -> bool {
        let Some(event) = player
            .active_emote
            .filter(|event| !event.scripted && !event.expired(tick, tick_rate_ms))
        else {
            return false;
        };
        if self
            .emote_visuals
            .get(&player.id)
            .is_some_and(|visual| visual.event.event_id == event.event_id)
        {
            self.avatar_presentation(player.id).manual_emote_active
        } else {
            true
        }
    }

    pub(crate) fn avatar_presentation(&self, player_id: u16) -> AvatarPresentation {
        self.avatar_presentations
            .get(&player_id)
            .copied()
            .unwrap_or(AvatarPresentation {
                expression: AvatarExpression::Calm,
                manual_emote_active: false,
                transition: None,
            })
    }

    pub(crate) fn set_campaign_emote(
        &mut self,
        player_id: u16,
        event_id: u64,
        token: String,
        message: String,
    ) -> bool {
        let value = message.trim();
        if value.is_empty() || value.chars().count() > 160 || value.chars().any(char::is_control) {
            return false;
        }
        self.campaign_emotes.insert(
            player_id,
            CampaignEmoteCue {
                player_id,
                event_id,
                token,
                message: value.to_owned(),
                prepared: None,
                visible: false,
            },
        );
        true
    }

    pub(crate) fn campaign_emote_matches(&self, token: &str, player_id: u16) -> bool {
        self.campaign_emotes
            .get(&player_id)
            .is_some_and(|cue| cue.token == token && cue.player_id == player_id)
    }

    pub(crate) fn clear_campaign_emote(&mut self, token: Option<&str>, player_id: u16) -> bool {
        if !self.campaign_emotes.get(&player_id).is_some_and(|cue| {
            cue.player_id == player_id && token.is_none_or(|token| cue.token == token)
        }) {
            return false;
        }
        self.campaign_emotes.remove(&player_id);
        true
    }

    pub(crate) fn clear_campaign_emote_token(&mut self, token: &str) -> Option<(u16, u64)> {
        let player_id = self
            .campaign_emotes
            .iter()
            .find_map(|(player_id, cue)| (cue.token == token).then_some(*player_id))?;
        let cue = self.campaign_emotes.remove(&player_id)?;
        Some((cue.player_id, cue.event_id))
    }

    pub(crate) fn campaign_emote_ready_tokens(&self) -> Vec<String> {
        self.campaign_emotes
            .values()
            .filter(|cue| cue.visible)
            .map(|cue| cue.token.clone())
            .collect()
    }

    pub(crate) fn animate_troop_change(&mut self, player_id: u16, amount: f64, now: Instant) {
        if amount.is_finite()
            && amount != 0.0
            && let Some(state) = self.visuals.get_mut(&player_id)
        {
            state.troop_change_positive = amount > 0.0;
            if state.troop_change_age(now, false).is_some() {
                return;
            }
            // Merge simultaneous gains or expenses into the current wave.
            state.troop_change_at = Some(now);
        }
    }

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
        bases_updated: bool,
        transitions_touched: bool,
    ) {
        if self.emote_tick == Some(snapshot.tick)
            && !bases_updated
            && !transitions_touched
            && self.emote_visuals.is_empty()
            && self.campaign_emotes.is_empty()
            && !self.avatar_presentations.is_empty()
        {
            return;
        }
        if self.emote_tick.is_some_and(|tick| snapshot.tick < tick) {
            self.emote_visuals.clear();
        }
        let same_tick = self.emote_tick == Some(snapshot.tick);
        self.emote_tick = Some(snapshot.tick);
        if !same_tick || !self.emote_visuals.is_empty() {
            for player in &snapshot.players {
                if !player.alive {
                    self.emote_visuals.remove(&player.id);
                    continue;
                }
                let old = self.emote_visuals.get(&player.id).copied();
                let idle = self
                    .avatar_bases
                    .get(&player.id)
                    .copied()
                    .unwrap_or(AvatarExpression::Calm);
                if let Some(event) = player.active_emote {
                    if old.is_some_and(|old| {
                        old.event.event_id == event.event_id
                            && old.event.kind == event.kind
                            && old.event.pinned == event.pinned
                            && old.event.scripted == event.scripted
                            && old.event.started_tick == event.started_tick
                            && old.release_at.is_none()
                    }) {
                        continue;
                    }
                    let raw_age = event.age_seconds(snapshot.tick, tick_secs * 1000.0);
                    let (from_motion, previous) = old
                        .map(|old| {
                            let age = old.age(now);
                            let (from, to, blend) = old.expressions(age, now, idle);
                            (
                                old.motion(age, now, reduced),
                                if blend < 0.5 { from } else { to },
                            )
                        })
                        .unwrap_or((EmoteMotion::default(), idle));
                    self.emote_visuals.insert(
                        player.id,
                        AvatarEmote {
                            event,
                            previous,
                            from_motion,
                            observed_at: now,
                            observed_age: if event.scripted || raw_age <= tick_secs * 2.0 {
                                0.0
                            } else {
                                raw_age
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
                        if old.age(now) >= EMOTE_DURATION_SECS {
                            self.emote_visuals.remove(&player.id);
                        }
                    } else {
                        let age = old.age(now);
                        let (from, to, blend) = old.expressions(age, now, idle);
                        old.previous = if blend < 0.5 { from } else { to };
                        old.from_motion = old.motion(age, now, reduced);
                        old.release_at = Some(now);
                        self.emote_visuals.insert(player.id, old);
                    }
                }
            }
        }
        let mut presentations = std::mem::take(&mut self.scratch_presentations);
        presentations.clear();
        for player in &snapshot.players {
            let idle = self
                .avatar_bases
                .get(&player.id)
                .copied()
                .unwrap_or(AvatarExpression::Calm);
            let emote = self.emote_visuals.get(&player.id).copied();
            let (expression, manual) = emote.map_or((idle, false), |visual| {
                let age = visual.age(now);
                let (from, to, mix) = visual.expressions(age, now, idle);
                (
                    if mix < 0.5 { from } else { to },
                    !visual.event.scripted
                        && visual.release_at.is_none()
                        && (visual.event.pinned || age < EMOTE_DURATION_SECS),
                )
            });
            let transition = if emote.is_none() {
                self.avatar_transitions.get(&player.id).map(|transition| {
                    transition.presentation()
                })
            } else {
                None
            };
            let expression = transition.map_or(expression, |transition| match transition.stage {
                AvatarTransitionStage::Exit => transition.from,
                AvatarTransitionStage::Enter => transition.to,
            });
            presentations.insert(
                player.id,
                AvatarPresentation {
                    expression,
                    manual_emote_active: manual,
                    transition,
                },
            );
        }
        if self.avatar_presentations != presentations {
            std::mem::swap(&mut self.avatar_presentations, &mut presentations);
            self.avatar_revision = self.avatar_revision.wrapping_add(1);
        }
        self.scratch_presentations = presentations;
        if !self.campaign_emotes.is_empty() {
            let emote_visuals = &self.emote_visuals;
            self.campaign_emotes.retain(|player_id, cue| {
                emote_visuals.get(player_id).is_some_and(|visual| {
                    visual.event.event_id == cue.event_id
                        && visual.event.scripted
                        && (visual.event.pinned || visual.age(now) < EMOTE_DURATION_SECS)
                })
            });
        }
    }

    fn observe_status_badges(
        &mut self,
        snapshot: &SimSnapshot,
        my_id: u16,
        leaderboard_top_three: [Option<u16>; 3],
        now: Instant,
        reduced_motion: bool,
    ) {
        let active_ids = &self.active_ids;
        self.badge_animations
            .retain(|player_id, _| active_ids.contains(player_id));
        let my_player = snapshot.players.iter().find(|player| player.id == my_id);
        for &player_index in &self.order {
            let player = &snapshot.players[player_index];
            let other_me = my_player.filter(|me| me.id != player.id);
            let rank = leaderboard_top_three
                .iter()
                .position(|id| *id == Some(player.id))
                .map(|index| index + 1);
            let present = [
                other_me.is_some_and(|me| me.alliance_requests.contains(&player.id)),
                other_me.is_some_and(|me| me.alliances.contains(&player.id)),
                player.traitor,
                player.disconnected,
                rank == Some(1),
                rank == Some(2),
                rank == Some(3),
            ];
            self.badge_animations.entry(player.id).or_default().observe(
                present,
                now,
                reduced_motion,
            );
        }
    }

    fn prepare_campaign_emote_message(&mut self, text: &TextRenderer, dev: &DevConfig) {
        let style_key = nameplate_metrics_style_key(dev);
        for cue in self.campaign_emotes.values_mut() {
            if cue
                .prepared
                .as_ref()
                .is_some_and(|(key, _)| *key == style_key)
            {
                continue;
            }
            let prepared = text.prepare_string(&cue.message, nameplate_char_spacing(dev), 0.5);
            cue.prepared = Some((style_key, prepared));
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
                    text.prepare_string(source, nameplate_char_spacing(dev), 0.5);
            }
            self.emote_text_style_key = Some(style_key);
        }
    }

    pub(crate) fn anchor_for(&self, player_id: u16) -> Option<[f32; 2]> {
        self.visuals
            .get(&player_id)
            .map(|state| state.to_center.0)
            .or_else(|| {
                self.land_cache
                    .rect_for(player_id)
                    .map(|rect| rect.center().0)
            })
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
    avatars: &AvatarSlots,
    sf: f32,
    zoom_scaled: f32,
    now: Instant,
    my_id: u16,
    leaderboard_top_three: [Option<u16>; 3],
    tutorial_target_player: Option<u16>,
    tutorial_active: bool,
    reduced_motion: bool,
) {
    let sf = if sf.is_finite() { sf.max(0.01) } else { 1.0 };
    if system.order_my_id.is_some_and(|previous| previous != my_id)
        || system
            .sample_tick
            .is_some_and(|previous| snapshot.tick < previous)
    {
        system.badge_animations.clear();
    }
    if !tutorial_active && !system.campaign_base_expressions.is_empty() {
        system.set_campaign_base_expressions(HashMap::new());
    }
    let bases_updated = system.observe_avatar_bases(snapshot, now, reduced_motion);
    let transitions_touched = system.advance_avatar_transitions(snapshot, now, reduced_motion);
    system.observe_emotes(
        snapshot,
        (sim.config.tick_rate_ms / 1000.0).max(0.001),
        now,
        reduced_motion,
        bases_updated,
        transitions_touched,
    );
    system.prepare_emote_texts(text, dev);
    system.prepare_campaign_emote_message(text, dev);
    system.tutorial_target_hit_bounds = None;
    let sample_started_at = crate::diag::startup_perf_timer();
    sample_nameplates(text, snapshot, sim, system, dev, sf, my_id, now);
    if let Some(started_at) = sample_started_at {
        crate::diag::record_startup_perf(crate::diag::StartupMetric::NameplateSample, started_at);
    }
    system.observe_status_badges(snapshot, my_id, leaderboard_top_three, now, reduced_motion);
    let active_ids = &system.active_ids;
    system
        .avatar_entries
        .retain(|player_id, _| active_ids.contains(player_id));

    let my_player = snapshot.players.iter().find(|player| player.id == my_id);
    let mut full_labels_drawn = 0usize;
    let bot_hide_zoom =
        sow_render::shader_params::number_value(&dev.shader_params, "lod.nameplate.bot_hide_zoom");
    let dot_text_px =
        sow_render::shader_params::number_value(&dev.shader_params, "lod.nameplate.dot_text_px");
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let fog_hidden = matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. });
    let alpha = nameplate_sample_alpha(system.sample_at, now, nameplate_sample_duration(sim));
    let inverse_alpha = 1.0 - alpha;

    let (order, visuals, frame_plans, badge_animations, avatar_entries) = (
        &system.order,
        &mut system.visuals,
        &mut system.frame_plans,
        &mut system.badge_animations,
        &mut system.avatar_entries,
    );
    frame_plans.clear();
    for &player_index in order.iter() {
        let player = &snapshot.players[player_index];
        if !player.alive {
            continue;
        }
        let is_me = player.id == my_id;
        let has_human_priority =
            player.player_type == PlayerType::Human || system.campaign_emotes.contains_key(&player.id);
        let style = NameplateStyle::for_player(player.player_type, player.is_campaign_faction);
        let Some(state) = visuals.get_mut(&player.id) else {
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

        if state.troop_change_at.is_some() {
            if state.troop_change_age(now, false).is_some() {
                // A troop change must update the count ahead of slower spatial sampling.
                refresh_nameplate_text_cache(
                    text,
                    state,
                    player,
                    nameplate_metrics_style_key(dev),
                    dev,
                    sf,
                );
            } else {
                state.troop_change_at = None;
            }
        }

        let marker = if is_me {
            LodMarker::Star { center }
        } else {
            LodMarker::Dot {
                center,
                color: player_color(player),
                scale: fit_size_to_land(
                    2.0 * (LOD_DOT_RADIUS + 1.0),
                    2.0 * (LOD_DOT_RADIUS + 1.0),
                    land_bounds,
                    zoom_scaled,
                ),
            }
        };

        if zoom_scaled < bot_hide_zoom && !is_me && !has_human_priority {
            frame_plans.push(PreparedNameplate {
                player_index,
                has_human_priority,
                full: None,
                marker,
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
        let base_extents = layout_bounds.extents_about(center);
        let fit_scale = fit_nameplate_bounds_to_land(
            layout,
            center,
            land_bounds,
            zoom_scaled,
            base_extents,
            badge_animations.get(&player.id),
        );
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
        let show_full = (fitted_font_size >= dot_text_px
            || system.campaign_emotes.contains_key(&player.id))
            && (has_human_priority || full_labels_drawn < 80);
        if !show_full {
            frame_plans.push(PreparedNameplate {
                player_index,
                has_human_priority,
                full: None,
                marker,
                presentation: NameplatePresentation::Hidden,
                tutorial_hit_bounds: None,
            });
            continue;
        }
        if !has_human_priority {
            full_labels_drawn += 1;
        }

        let base_expression = system
            .avatar_bases
            .get(&player.id)
            .copied()
            .unwrap_or(AvatarExpression::Calm);
        let identity_ref = sow_core::player::avatar_identity_ref(player);
        let portrait_loaded = layout.avatar_radius() > 0.0
            && avatars.has_loaded_portrait(text, identity_ref, base_expression);
        let avatar_entry_age = observe_avatar_entry(
            avatar_entries,
            player.id,
            sow_core::player::avatar_identity(player),
            portrait_loaded,
            now,
            reduced_motion,
        );
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
            avatars,
            badge_animations.entry(player.id).or_default(),
            sf,
            now,
            reduced_motion,
            base_expression,
            avatar_entry_age,
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
            marker,
            presentation: NameplatePresentation::Hidden,
            tutorial_hit_bounds,
        });
    }

    let campaign_message_instances = system
        .campaign_emotes
        .values()
        .filter_map(|cue| cue.prepared.as_ref())
        .map(|(_, prepared)| prepared.instance_count().saturating_add(5))
        .sum();
    assign_nameplate_capacity(
        frame_plans,
        text.remaining_instance_capacity()
            .saturating_sub(campaign_message_instances),
    );
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
        .saturating_sub(base_reserved)
        .saturating_sub(campaign_message_instances);
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
                    let age = emote.age(now);
                    let motion = emote.motion(age, now, reduced_motion);
                    *avatar = apply_emote_avatar_motion(*avatar, motion);
                    let (from, to, mix) = emote.expressions(
                        age,
                        now,
                        system
                            .avatar_bases
                            .get(&player.id)
                            .copied()
                            .unwrap_or(AvatarExpression::Calm),
                    );
                    let identity = sow_core::player::avatar_identity_ref(player);
                    avatar.content = resolve_avatar_content(text, avatars, identity, to);
                    if from != to && mix < 1.0 && decoration_budget > 0 {
                        if let AvatarContent::Sprite(from_uv) =
                            resolve_avatar_content(text, avatars, identity, from)
                        {
                            avatar.blend = Some((from_uv, mix));
                            decoration_budget -= 1;
                        }
                    }
                } else if let Some(avatar) = &mut full.avatar
                    && let Some(transition) = system.avatar_transitions.get(&player.id).copied()
                {
                    let allow_crossfade = decoration_budget > 0;
                    apply_base_avatar_transition(
                        text,
                        avatars,
                        avatar,
                        player,
                        transition,
                        now,
                        allow_crossfade,
                    );
                    if allow_crossfade && avatar.blend.is_some() {
                        decoration_budget -= 1;
                    }
                }
                if let Some(avatar) = &mut full.avatar {
                    *avatar = apply_avatar_entry(*avatar, full.avatar_entry_age, reduced_motion);
                }
                paint_prepared_nameplate(text, state, full);
            }
            NameplatePresentation::Compact => paint_lod_marker(text, plan.marker, sf),
            NameplatePresentation::Hidden => {}
        }
    }
    system.tutorial_target_hit_bounds = tutorial_target_hit_bounds;
}

pub(super) fn render_emote_bubbles(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    system: &mut NameplateSystem,
    input: &InputState,
    sf: f32,
    now: Instant,
    my_id: u16,
    reduced_motion: bool,
) {
    let campaign_speakers = system.campaign_emotes.keys().copied().collect::<HashSet<_>>();
    for plan in system
        .frame_plans
        .iter()
        .filter(|plan| campaign_speakers.contains(&snapshot.players[plan.player_index].id))
        .chain(
            system
                .frame_plans
                .iter()
                .filter(|plan| !campaign_speakers.contains(&snapshot.players[plan.player_index].id)),
        )
    {
        if plan.presentation != NameplatePresentation::Full {
            continue;
        }
        let player = &snapshot.players[plan.player_index];
        if !sow_core::emote::is_visible(&snapshot.players, player, my_id) {
            continue;
        }
        let Some(full) = plan.full else {
            continue;
        };
        let Some(mut avatar) = full.avatar else {
            continue;
        };
        let Some(emote) = system.emote_visuals.get(&player.id).copied() else {
            continue;
        };
        let age = emote.age(now);
        let motion = emote.motion(age, now, reduced_motion);
        avatar = apply_emote_avatar_motion(avatar, motion);
        avatar = apply_avatar_entry(avatar, full.avatar_entry_age, reduced_motion);
        let campaign_cue = system.campaign_emotes.get(&player.id).filter(|cue| {
            cue.event_id == emote.event.event_id && emote.event.scripted
        });
        let prepared = campaign_cue
            .and_then(|cue| cue.prepared.as_ref().map(|(_, prepared)| prepared))
            .unwrap_or(&system.prepared_emotes[emote.event.kind as usize]);
        let needed = prepared.instance_count().saturating_add(5);
        if motion.bubble_alpha <= 0.0 || needed > text.remaining_instance_capacity() {
            continue;
        }
        let drawn = paint_emote_bubble(
            text,
            prepared,
            avatar,
            motion,
            player_color(player),
            input,
            system.safe_area,
            sf,
        );
        if let Some(cue) = system.campaign_emotes.get_mut(&player.id)
            && drawn
            && cue.event_id == emote.event.event_id
        {
            cue.visible = true;
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
        system.active_ids.clear();
        system.visuals.clear();
        system.badge_animations.clear();
        system.land_cache = NameplateLandCache::default();
        return;
    };
    // Deployment spawns owners one by one; anchor each fresh spawn locally instead of forcing a
    // full-map rebuild per spawn. Only owners the local flood cannot cover force the rebuild.
    let mut new_player_needs_landmass = false;
    for player in &snapshot.players {
        if !player.alive || player.tile_count == 0 || system.land_cache.has_landmass(player.id) {
            continue;
        }
        let anchored = system.land_cache.anchor_fresh_owner(
            &engine.state.map,
            player.id,
            player.centroid_x.round() as u32,
            player.centroid_y.round() as u32,
            player.tile_count,
        );
        new_player_needs_landmass |= !anchored;
    }
    let needs_rebuild = system.land_cache.needs_rebuild(
        &engine.state.map,
        snapshot.tick,
        new_player_needs_landmass,
    );
    if needs_rebuild {
        let rebuild_started_at = crate::diag::startup_perf_timer();
        let rebuilt = system.land_cache.rebuild(&engine.state.map, snapshot.tick);
        if let Some(started_at) = rebuild_started_at {
            crate::diag::record_startup_perf(
                crate::diag::StartupMetric::LandCacheRebuild,
                started_at,
            );
        }
        if !rebuilt {
            system.order.clear();
            system.active_ids.clear();
            system.visuals.clear();
            system.badge_animations.clear();
            return;
        }
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
        troop_change_at: None,
        troop_change_positive: false,
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
            sow_core::player::display_name(player.id, &player.name, player.player_type);
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
        state.prepared_name =
            text.prepare_string(&state.display_name, nameplate_char_spacing(dev), 0.5);
    }
    if troops_text_changed || state.text_style_key != Some(style_key) {
        state.prepared_troops =
            text.prepare_string(&state.troops_text, nameplate_char_spacing(dev), 0.0);
    }
    state.text_style_key = Some(style_key);
}

fn nameplate_metrics_style_key(dev: &DevConfig) -> u32 {
    // The Arabic font arrives on demand; its arrival re-prepares names shaped without it.
    nameplate_char_spacing(dev).to_bits() ^ sow_render::arabic_font_generation()
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

fn fit_nameplate_bounds_to_land(
    layout: NameplateLayout,
    center: ScreenPoint,
    land_bounds: WorldRect,
    zoom_scaled: f32,
    mut extents: [f32; 2],
    animations: Option<&PlayerStatusBadgeAnimations>,
) -> f32 {
    let badge_padding = layout.badge_effect_padding();

    if let Some(animations) = animations {
        for badge in layout.badges().iter().flatten() {
            let Some(index) = status_badge_index(badge.kind()) else {
                continue;
            };
            if animations.badges[index].animation.is_some() {
                include_badge_animation_extent(
                    &mut extents,
                    center.0,
                    badge.center().0,
                    badge.diameter(),
                    badge_padding,
                );
            }
        }

        let avatar_center = layout.avatar_center().0;
        for badge in &animations.badges {
            if badge.present || badge.animation.is_none() {
                continue;
            }
            let Some(geometry) = badge.geometry else {
                continue;
            };
            include_badge_animation_extent(
                &mut extents,
                center.0,
                [
                    avatar_center[0] + geometry.center_from_avatar[0],
                    avatar_center[1] + geometry.center_from_avatar[1],
                ],
                geometry.diameter,
                badge_padding,
            );
        }
    }

    fit_size_to_land(extents[0] * 2.0, extents[1] * 2.0, land_bounds, zoom_scaled)
}

fn include_badge_animation_extent(
    extents: &mut [f32; 2],
    reference: [f32; 2],
    badge_center: [f32; 2],
    diameter: f32,
    padding: f32,
) {
    let radius = diameter * STATUS_BADGE_MAX_SCALE * 0.5 + padding;
    for axis in 0..2 {
        extents[axis] = extents[axis].max((badge_center[axis] - reference[axis]).abs() + radius);
    }
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
    avatars: &AvatarSlots,
    badge_animations: &mut PlayerStatusBadgeAnimations,
    sf: f32,
    now: Instant,
    reduced_motion: bool,
    base_expression: AvatarExpression,
    avatar_entry_age: Option<f32>,
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
    let avatar = (layout.avatar_radius() > 0.0).then(|| {
        prepare_avatar(text, avatars, player, layout, sf, base_expression)
    });
    let (badges, badge_count) = prepare_badges(layout, sf, badge_animations, now, reduced_motion);
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
            transfer_age: state.troop_change_age(now, reduced_motion),
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
        avatar_entry_age,
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
    avatars: &AvatarSlots,
    identity: &sow_core::player::AvatarIdentity,
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
        sow_core::player::AvatarIdentity::Wilderness
        | sow_core::player::AvatarIdentity::Fallback => {
            sow_core::player::AvatarIdentityRef::Fallback
        }
    };
    let content =
        resolve_avatar_content(text, avatars, identity_ref, death_avatar_expression(identity));
    avatar_plan(center, radius, content)
}

fn death_avatar_expression(identity: &sow_core::player::AvatarIdentity) -> AvatarExpression {
    match identity {
        sow_core::player::AvatarIdentity::Portrait {
            leader: Some(_), ..
        } => AvatarExpression::Fear,
        sow_core::player::AvatarIdentity::Portrait { leader: None, .. }
        | sow_core::player::AvatarIdentity::Wilderness
        | sow_core::player::AvatarIdentity::Fallback => AvatarExpression::Calm,
    }
}

fn resolve_avatar_content(
    text: &TextRenderer,
    avatars: &AvatarSlots,
    identity: sow_core::player::AvatarIdentityRef<'_>,
    expression: AvatarExpression,
) -> AvatarContent {
    avatars
        .uv(text, identity, expression)
        .map_or(AvatarContent::None, AvatarContent::Sprite)
}

pub(crate) fn paint_attack_badge_avatar(
    text: &mut TextRenderer,
    avatars: &AvatarSlots,
    player: Option<&PlayerSnapshot>,
    center: [f32; 2],
    diameter: f32,
) {
    let identity = player.map_or(
        sow_core::player::AvatarIdentityRef::Fallback,
        sow_core::player::avatar_identity_ref,
    );
    let content = resolve_avatar_content(text, avatars, identity, AvatarExpression::Calm);
    paint_prepared_avatar(text, avatar_plan(center, diameter * 0.5, content), 1.0);
}

fn prepare_avatar(
    text: &TextRenderer,
    avatars: &AvatarSlots,
    player: &PlayerSnapshot,
    layout: NameplateLayout,
    sf: f32,
    expression: AvatarExpression,
) -> AvatarPlan {
    let content = resolve_avatar_content(
        text,
        avatars,
        sow_core::player::avatar_identity_ref(player),
        expression,
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
        alpha: 1.0,
        blend: None,
    }
}

fn apply_emote_avatar_motion(mut avatar: AvatarPlan, motion: EmoteMotion) -> AvatarPlan {
    let diameter = avatar.radius * 2.0;
    for axis in 0..2 {
        avatar.center[axis] += motion.offset[axis] * diameter;
        avatar.scale[axis] *= motion.scale[axis];
    }
    avatar
}

fn avatar_entry_motion(age: f32, reduced_motion: bool) -> EmoteMotion {
    emote_motion(EmoteKind::Celebrate, age, reduced_motion, true)
}

fn apply_avatar_entry(avatar: AvatarPlan, age: Option<f32>, reduced_motion: bool) -> AvatarPlan {
    if !matches!(avatar.content, AvatarContent::Sprite(_)) {
        return avatar;
    }
    age.map_or(avatar, |age| {
        apply_emote_avatar_motion(avatar, avatar_entry_motion(age, reduced_motion))
    })
}

fn apply_base_avatar_transition(
    text: &TextRenderer,
    avatars: &AvatarSlots,
    avatar: &mut AvatarPlan,
    player: &PlayerSnapshot,
    transition: AvatarTransition,
    now: Instant,
    allow_crossfade: bool,
) {
    let progress = transition.progress(now);
    let identity = sow_core::player::avatar_identity_ref(player);
    let from_content = resolve_avatar_content(text, avatars, identity, transition.from);
    let to_content = resolve_avatar_content(text, avatars, identity, transition.to);
    if from_content == to_content {
        avatar.content = to_content;
        return;
    }
    match transition.stage {
        AvatarTransitionStage::Exit => {
            avatar.content = from_content;
            let scale = 1.0 - 0.18 * progress;
            avatar.scale = [scale; 2];
            avatar.center[1] += avatar.radius * 2.0 * 0.08 * progress;
            avatar.alpha = 1.0;
        }
        AvatarTransitionStage::Enter => {
            avatar.content = to_content;
            let duration = transition.duration().as_secs_f32();
            let motion = emote_motion(
                avatar_transition_emote(transition.to),
                progress * duration,
                false,
                true,
            );
            *avatar = apply_emote_avatar_motion(*avatar, motion);
            avatar.alpha = 1.0;
            if allow_crossfade
                && let AvatarContent::Sprite(from_uv) = from_content
            {
                avatar.blend = Some((from_uv, avatar_transition_crossfade(progress)));
            }
        }
    }
}

fn prepare_badges(
    layout: NameplateLayout,
    sf: f32,
    animations: &mut PlayerStatusBadgeAnimations,
    now: Instant,
    reduced_motion: bool,
) -> ([Option<BadgePlan>; NAMEPLATE_RENDER_BADGE_CAPACITY], usize) {
    let mut plans = [None; NAMEPLATE_RENDER_BADGE_CAPACITY];
    if layout.avatar_radius() <= 0.0 {
        return (plans, 0);
    }
    let avatar_center = layout.avatar_center().0;
    let mut count = 0;
    for badge in layout.badges().iter().flatten() {
        let kind = badge.kind();
        let mut size = [badge.diameter() * sf; 2];
        let mut tint = badge.tint();
        if let Some(index) = status_badge_index(kind) {
            let visual = &mut animations.badges[index];
            visual.geometry = Some(BadgeGeometry {
                center_from_avatar: [
                    badge.center().0[0] - avatar_center[0],
                    badge.center().0[1] - avatar_center[1],
                ],
                diameter: badge.diameter(),
                tint,
            });
            if let Some(animation) = visual.animation {
                let motion = animation.values(now, reduced_motion);
                size[0] *= motion.scale[0];
                size[1] *= motion.scale[1];
                tint[3] *= motion.alpha;
            }
        }
        plans[count] = Some(BadgePlan {
            kind,
            center: [badge.center().0[0] * sf, badge.center().0[1] * sf],
            size,
            tint,
        });
        count += 1;
    }

    for index in 0..STATUS_BADGE_COUNT {
        let visual = animations.badges[index];
        if visual.present {
            continue;
        }
        let (Some(animation), Some(geometry)) = (visual.animation, visual.geometry) else {
            continue;
        };
        let motion = animation.values(now, reduced_motion);
        if motion.alpha <= 0.0 {
            continue;
        }
        let kind = status_badge_kind(index);
        plans[count] = Some(BadgePlan {
            kind,
            center: [
                (avatar_center[0] + geometry.center_from_avatar[0]) * sf,
                (avatar_center[1] + geometry.center_from_avatar[1]) * sf,
            ],
            size: [
                geometry.diameter * sf * motion.scale[0],
                geometry.diameter * sf * motion.scale[1],
            ],
            tint: [
                geometry.tint[0],
                geometry.tint[1],
                geometry.tint[2],
                geometry.tint[3] * motion.alpha,
            ],
        });
        count += 1;
    }
    (plans, count)
}

fn status_badge_index(kind: BadgeKind) -> Option<usize> {
    match kind {
        BadgeKind::Request => Some(0),
        BadgeKind::Allied => Some(1),
        BadgeKind::Traitor => Some(2),
        BadgeKind::Disconnected => Some(3),
        BadgeKind::Rank1 => Some(4),
        BadgeKind::Rank2 => Some(5),
        BadgeKind::Rank3 => Some(6),
        BadgeKind::Star => None,
    }
}

fn status_badge_kind(index: usize) -> BadgeKind {
    match index {
        0 => BadgeKind::Request,
        1 => BadgeKind::Allied,
        2 => BadgeKind::Traitor,
        3 => BadgeKind::Disconnected,
        4 => BadgeKind::Rank1,
        5 => BadgeKind::Rank2,
        6 => BadgeKind::Rank3,
        _ => unreachable!("status badge index is bounded by STATUS_BADGE_COUNT"),
    }
}

fn paint_prepared_nameplate(
    text: &mut TextRenderer,
    state: &NameplateVisualState,
    plan: FullNameplate,
) {
    let before = text.instance_count();
    if let Some(avatar) = plan.avatar {
        paint_prepared_avatar(text, avatar, 1.0);
    }
    for badge in plan.badges[..plan.badge_count].iter().flatten() {
        // Every badge is a cell of the art/icons strip; none is a system emoji.
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
        text.push_status_sprite_scaled(sprite, badge.center, badge.size, badge.tint);
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
        let (icon_scale, icon_offset) = troops
            .transfer_age
            .map(troop_transfer_motion)
            .unwrap_or(([1.0; 2], 0.0));
        text.push_troop_icon(
            [
                troops.icon_center[0],
                troops.icon_center[1] + icon_offset * troops.icon_diameter,
            ],
            troops.icon_diameter * 0.5 * icon_scale[1],
            1.0,
            plan.emoji_outline,
        );
        if let Some(age) = troops.transfer_age {
            let _ = text.push_prepared_text_transformed(
                &state.prepared_troops,
                troops.text.position,
                troops.text.final_font_size,
                NAMEPLATE_TEXT_COLOR,
                plan.text_style,
                |index, glyph| {
                    let glyph_age =
                        age - TROOP_TRANSFER_TEXT_DELAY - index as f32 * TROOP_TRANSFER_GLYPH_DELAY;
                    let (scale, offset) = troop_transfer_motion(glyph_age);
                    let tint = 1.0 - smoothstep((glyph_age / 0.45).clamp(0.0, 1.0));
                    let target = if state.troop_change_positive {
                        [0.35, 1.0, 0.45, 1.0]
                    } else {
                        [1.0, 0.35, 0.35, 1.0]
                    };
                    for (channel, target) in target.into_iter().enumerate() {
                        glyph.color[channel] =
                            NAMEPLATE_TEXT_COLOR[channel] * (1.0 - tint) + target * tint;
                    }
                    for (axis, scale) in scale.into_iter().enumerate() {
                        glyph.screen_pos[axis] += glyph.size[axis] * (1.0 - scale) * 0.5;
                        glyph.size[axis] *= scale;
                    }
                    glyph.screen_pos[1] += offset * troops.text.final_font_size;
                },
            );
        } else {
            let _ = text.push_prepared_text(
                &state.prepared_troops,
                troops.text.position,
                troops.text.final_font_size,
                NAMEPLATE_TEXT_COLOR,
                plan.text_style,
            );
        }
    }
    debug_assert!(
        text.instance_count().saturating_sub(before)
            <= plan.instance_count + plan.avatar.map_or(0, AvatarPlan::transition_instance_count)
    );
}

pub(crate) fn paint_prepared_avatar(text: &mut TextRenderer, avatar: AvatarPlan, alpha: f32) {
    let alpha = alpha * avatar.alpha;
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
        AvatarContent::None => {}
    }
}

#[derive(Clone, Copy)]
struct EmoteBubbleContent {
    font_size: f32,
    width: f32,
    height: f32,
    text_bounds: [f32; 4],
}

fn emote_bubble_base_font_size(avatar_diameter: f32, sf: f32) -> f32 {
    (avatar_diameter * EMOTE_BUBBLE_FONT_RATIO).max(EMOTE_BUBBLE_MIN_FONT_SIZE * sf)
}

fn size_emote_bubble_content(
    avatar_diameter: f32,
    base_font_size: f32,
    measured_width: f32,
    visual_bounds: [f32; 4],
    max_width: f32,
) -> Option<EmoteBubbleContent> {
    if !avatar_diameter.is_finite()
        || avatar_diameter <= 0.0
        || !base_font_size.is_finite()
        || base_font_size <= 0.0
        || !measured_width.is_finite()
        || measured_width < 0.0
        || !max_width.is_finite()
        || max_width <= 0.0
        || visual_bounds.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let max_width = max_width.min(avatar_diameter * EMOTE_BUBBLE_MAX_WIDTH_RATIO);
    let padding_x = (avatar_diameter * EMOTE_BUBBLE_HORIZONTAL_PADDING_RATIO)
        .min(((max_width - 1.0) * 0.5).max(0.0));
    let padding_y = avatar_diameter * EMOTE_BUBBLE_VERTICAL_PADDING_RATIO;
    let text_scale =
        (((max_width - 2.0 * padding_x).max(1.0)) / measured_width.max(1.0)).clamp(0.0, 1.0);
    let width = (measured_width * text_scale + 2.0 * padding_x).min(max_width);
    let visual_height = (visual_bounds[3] - visual_bounds[1]).max(0.0) * text_scale;
    let height =
        (visual_height + 2.0 * padding_y).max(avatar_diameter * EMOTE_BUBBLE_MIN_HEIGHT_RATIO);

    Some(EmoteBubbleContent {
        font_size: base_font_size * text_scale,
        width,
        height,
        text_bounds: visual_bounds.map(|value| value * text_scale),
    })
}

fn place_emote_bubble(
    avatar: AvatarPlan,
    size: [f32; 2],
    tail_width: f32,
    tail_length: f32,
    inner_tail_width: f32,
    _inner_tail_length: f32,
    screen: [f32; 2],
    safe: [f32; 4],
    sf: f32,
) -> Option<EmoteBubbleLayout> {
    let mut fallback = None;
    for quadrant in [
        EmoteBubbleQuadrant::UpperRight,
        EmoteBubbleQuadrant::UpperLeft,
        EmoteBubbleQuadrant::LowerRight,
        EmoteBubbleQuadrant::LowerLeft,
    ] {
        let Some((layout, fits_without_clamping)) = emote_bubble_candidate(
            avatar,
            size,
            tail_width,
            tail_length,
            inner_tail_width,
            _inner_tail_length,
            screen,
            safe,
            sf,
            quadrant,
        ) else {
            continue;
        };
        if fits_without_clamping {
            return Some(layout);
        }
        if fallback.is_none() {
            fallback = Some(layout);
        }
    }
    fallback
}

#[allow(clippy::too_many_arguments)]
fn emote_bubble_candidate(
    avatar: AvatarPlan,
    size: [f32; 2],
    tail_width: f32,
    tail_length: f32,
    inner_tail_width: f32,
    _inner_tail_length: f32,
    screen: [f32; 2],
    safe: [f32; 4],
    sf: f32,
    quadrant: EmoteBubbleQuadrant,
) -> Option<(EmoteBubbleLayout, bool)> {
    let (x_sign, y_sign) = quadrant.signs();
    let radius_x = avatar.radius * avatar.scale[0];
    let radius_y = avatar.radius * avatar.scale[1];
    let tip = [
        avatar.center[0] + x_sign * radius_x * 0.6,
        avatar.center[1] + y_sign * radius_y * 0.8,
    ];
    if !tip[0].is_finite()
        || !tip[1].is_finite()
        || tip[1] < safe[0]
        || tip[1] > screen[1] - safe[2]
        || tail_width <= 0.0
        || tail_length <= 0.0
        || size[0] < tail_width
        || size[1] <= 0.0
    {
        return None;
    }

    let half_width = size[0] * 0.5;
    let half_height = size[1] * 0.5;
    let half_tail_width = tail_width * 0.5;
    let shadow_x = 2.0 * sf;
    let shadow_top = sf;
    let shadow_bottom = 5.0 * sf;
    let min_center_x = (safe[3] + half_width + shadow_x).max(tip[0] - half_width + half_tail_width);
    let max_center_x =
        (screen[0] - safe[1] - half_width - shadow_x).min(tip[0] + half_width - half_tail_width);
    if min_center_x > max_center_x
        || tip[0] - half_tail_width < safe[3]
        || tip[0] + half_tail_width > screen[0] - safe[1]
    {
        return None;
    }

    let min_center_y_screen = safe[0] + half_height + shadow_top;
    let max_center_y_screen = screen[1] - safe[2] - half_height - shadow_bottom;
    let min_tail_gap = (0.5 * sf).max(0.25);
    let (min_center_y_tail, max_center_y_tail) = if y_sign < 0.0 {
        (
            tip[1] - tail_length - half_height,
            tip[1] - min_tail_gap - half_height,
        )
    } else {
        (
            tip[1] + min_tail_gap + half_height,
            tip[1] + tail_length + half_height,
        )
    };
    let min_center_y = min_center_y_screen.max(min_center_y_tail);
    let max_center_y = max_center_y_screen.min(max_center_y_tail);
    if min_center_y > max_center_y {
        return None;
    }

    let ideal_center = [
        tip[0] + x_sign * half_width * EMOTE_BUBBLE_BODY_OFFSET_RATIO,
        tip[1] + y_sign * (tail_length + half_height),
    ];
    let center = [
        ideal_center[0].clamp(min_center_x, max_center_x),
        ideal_center[1].clamp(min_center_y, max_center_y),
    ];
    let exact = (center[0] - ideal_center[0]).abs() <= 0.001
        && (center[1] - ideal_center[1]).abs() <= 0.001;
    let tail_gap = if y_sign < 0.0 {
        tip[1] - (center[1] + half_height)
    } else {
        (center[1] - half_height) - tip[1]
    };
    if !tail_gap.is_finite() || tail_gap < min_tail_gap {
        return None;
    }

    let outer_signed_height = -y_sign * tail_gap;
    let body_edge_y = center[1] - y_sign * half_height;
    let overlap = (1.6 * sf).min(tail_gap * 0.35);
    let tip_inset = (1.8 * sf).min(tail_gap * 0.25);
    let inner_base_y = body_edge_y + y_sign * overlap;
    let inner_tip_y = tip[1] + y_sign * tip_inset;
    let inner_signed_height = inner_tip_y - inner_base_y;
    let inner_tail_center = [tip[0], (inner_base_y + inner_tip_y) * 0.5];
    let inner_w = (tail_width - 2.6 * sf)
        .min(inner_tail_width)
        .max(tail_width * 0.65);
    let layout = EmoteBubbleLayout {
        center,
        size,
        font_size: 0.0,
        text_bounds: [0.0; 4],
        outer_tail_center: [tip[0], tip[1] - outer_signed_height * 0.5],
        outer_tail_size: [tail_width, outer_signed_height],
        inner_tail_center,
        inner_tail_size: [inner_w, inner_signed_height],
    };
    Some((layout, exact))
}

fn paint_emote_bubble(
    text: &mut TextRenderer,
    prepared: &sow_render::text::PreparedText,
    avatar: AvatarPlan,
    motion: EmoteMotion,
    _accent: [f32; 4],
    input: &InputState,
    insets: [f32; 4],
    sf: f32,
) -> bool {
    let avatar_diameter = avatar.radius * 2.0;
    let safe = insets.map(|value| value * sf + 8.0 * sf);
    let max_width = (avatar_diameter * EMOTE_BUBBLE_MAX_WIDTH_RATIO)
        .min((input.screen_w - safe[1] - safe[3] - 4.0 * sf).max(1.0));
    let base_font_size = emote_bubble_base_font_size(avatar_diameter, sf);
    let measured = prepared.measure_at(base_font_size);
    let Some(base_bounds) = prepared.visual_bounds_at(base_font_size) else {
        return false;
    };
    let Some(content) = size_emote_bubble_content(
        avatar_diameter,
        base_font_size,
        measured.width,
        base_bounds,
        max_width,
    ) else {
        return false;
    };

    let motion_scale = motion.bubble_scale.clamp(0.0, 1.15);
    let diameter_scale = avatar_diameter * motion_scale;
    let size = [content.width * motion_scale, content.height * motion_scale];
    let Some(mut layout) = place_emote_bubble(
        avatar,
        size,
        diameter_scale * EMOTE_BUBBLE_TAIL_WIDTH_RATIO,
        diameter_scale * EMOTE_BUBBLE_TAIL_LENGTH_RATIO,
        diameter_scale * EMOTE_BUBBLE_INNER_TAIL_WIDTH_RATIO,
        diameter_scale * EMOTE_BUBBLE_INNER_TAIL_LENGTH_RATIO,
        [input.screen_w, input.screen_h],
        safe,
        sf,
    ) else {
        return false;
    };
    layout.font_size = content.font_size * motion_scale;
    layout.text_bounds = content.text_bounds.map(|value| value * motion_scale);

    let alpha = motion.bubble_alpha;
    text.push_rounded_rect(
        [layout.center[0], layout.center[1] + 3.0 * sf],
        [layout.size[0] + 4.0 * sf, layout.size[1] + 4.0 * sf],
        0.2,
        [0.0, 0.0, 0.0, 0.18 * alpha],
        [0.0; 4],
        0.0,
    );
    let border_color = [0.15, 0.18, 0.22, 0.35 * alpha];
    text.push_rounded_rect(
        layout.center,
        layout.size,
        0.2,
        [0.97, 0.95, 0.90, alpha],
        border_color,
        1.2 * sf,
    );
    text.push_triangle(
        layout.outer_tail_center,
        layout.outer_tail_size,
        border_color,
    );
    text.push_triangle(
        layout.inner_tail_center,
        layout.inner_tail_size,
        [0.97, 0.95, 0.90, alpha],
    );
    text.push_prepared_text(
        prepared,
        [
            layout.center[0],
            layout.center[1] - (layout.text_bounds[1] + layout.text_bounds[3]) * 0.5,
        ],
        layout.font_size,
        [0.07, 0.09, 0.12, alpha],
        sow_render::text::TextPaintStyle {
            face_dilate: 0.0,
            outline: sow_render::text::OutlineStyle::NONE,
            underlay_softness: 0.0,
        },
    )
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

fn paint_lod_marker(text: &mut TextRenderer, marker: LodMarker, sf: f32) {
    match marker {
        LodMarker::Dot {
            center,
            color,
            scale,
        } => paint_lod_dot(text, center, color, sf, scale),
        LodMarker::Star { center } => text.push_status_sprite(
            NameplateStatusSprite::You,
            [center.0[0] * sf, center.0[1] * sf],
            LOD_STAR_DIAMETER * sf,
            [1.0; 4],
        ),
    }
}

pub(crate) fn player_color(player: &PlayerSnapshot) -> [f32; 4] {
    let rgb = sow_core::player::player_territory_rgb(
        player.player_type,
        player.team,
        player.color,
        player.is_campaign_faction,
    );
    [rgb[0], rgb[1], rgb[2], 1.0]
}

