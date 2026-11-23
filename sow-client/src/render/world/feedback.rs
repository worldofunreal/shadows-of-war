use crate::app::{InputState, SimState, TransportTargetMarker, UiState};
use crate::death_nameplate::{
    FLOATING_GOLD_RISE, GOLD_REWARD_DURATION, death_animation, death_icon, gold_reward_elapsed,
};
use crate::render::avatars::AvatarSlots;
use crate::render::gpu::TextRenderer;
use crate::render::motion::{cascade_callout_motion, spring_overshoot};
use crate::render::world::overlays::world_to_screen;
use crate::render::{dev_emoji_outline, dev_text_style};
use crate::theme::dev_config::DevConfig;
use sow_core::protocol::{AttackSnapshot, SimSnapshot};
use sow_render::FontAtlas;
use sow_render::text::{AVATAR_GOLD_SLOT, NameplateStatusSprite};
use web_time::Instant;

/// Troop icon diameter relative to the notice font size.
const TROOP_NOTICE_ICON_SCALE: f32 = 0.91;

const CLICK_MARKER_DURATION: f32 = 0.16;
const NAVAL_ORDER_MARKER_DURATION: f32 = 0.58;
const ATTACK_LAUNCH_NOTICE_DURATION: f32 = 1.5;
const ATTACK_LAUNCH_NOTICE_FONT_SIZE: f32 = 14.0;
const FLOATING_NOTICE_RISE: f32 = FLOATING_GOLD_RISE;
const TRANSPORT_TARGET_FADE_IN_SECS: f32 = 0.2;
const TRANSPORT_TARGET_FADE_OUT_SECS: f32 = 0.3;
const TRANSPORT_TARGET_ROTATIONS_PER_SEC: f32 = 1.0;
const TRANSPORT_TARGET_ETA_FONT_SIZE: f32 = 16.0;
/// Extra SDF face dilation over the label style so the ETA digits read bold.
const TRANSPORT_TARGET_ETA_BOLD_DILATE: f32 = 0.6;
const TRANSPORT_IMPACT_DURATION_SECS: f32 = 0.62;
const DEATH_NAMEPLATE_FONT_SIZE: f32 = 17.5;
const DEATH_NAMEPLATE_RISE: f32 = 5.0;
const DEATH_NAMEPLATE_ARC_OFFSET: f32 = 0.75;
const DEATH_NAMEPLATE_ICON_SIZE: f32 = 18.0;
const DEATH_NAMEPLATE_SCALE: f32 = 0.92;
const DEATH_CASUALTY_ICON_SIZE: f32 = 13.5;
const GOLD_REWARD_FONT_SIZE: f32 = 15.5;
const GOLD_REWARD_COIN_SIZE: f32 = 18.0;
const GOLD_REWARD_ASSIST_SIZE: f32 = 19.5;
const GOLD_REWARD_SCALE: f32 = 0.9;
const GOLD_REWARD_MAX_TEXT_SCALE: f32 = 1.15;
const DEATH_GOLD_ROW_GAP: f32 = 6.0;
const ATTACK_BADGE_FONT_SIZE: f32 = 13.0;
const ATTACK_BADGE_UPDATE_SECS: f32 = 0.09;
const OUTGOING_ATTACK_TEXT_COLOR: [f32; 4] = [1.0; 4];
const INCOMING_ATTACK_TEXT_COLOR: [f32; 4] = [1.0, 90.0 / 255.0, 90.0 / 255.0, 1.0];

fn death_gold_row_offset(death_bottom: f32, gold_top: f32, sf: f32) -> f32 {
    death_bottom - gold_top + DEATH_GOLD_ROW_GAP * sf.max(0.01)
}

fn troop_label_instance_requirement(amount_text: &str) -> usize {
    amount_text.chars().count().saturating_add(1)
}

fn attack_badge_instance_requirement(amount_text: &str) -> usize {
    troop_label_instance_requirement(amount_text).saturating_add(1)
}

fn transport_eta_baseline_y(
    font_atlas: &FontAtlas,
    eta_text: &str,
    center_y: f32,
    font_size: f32,
) -> f32 {
    let font_scale = font_size / 48.0;
    let base = font_atlas.atlas.common.base as f32;
    let (min_y, max_y) = eta_text
        .chars()
        .filter_map(|ch| font_atlas.char_map.get(&ch))
        .fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min_y, max_y), glyph| {
                (
                    min_y.min(glyph.yoffset as f32),
                    max_y.max(glyph.yoffset as f32 + glyph.height as f32),
                )
            },
        );

    if min_y.is_finite() && max_y.is_finite() {
        center_y + (base - (min_y + max_y) * 0.5) * font_scale
    } else {
        center_y
    }
}

pub(crate) fn render(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    avatars: &AvatarSlots,
    sf: f32,
    now: Instant,
) {
    render_click_markers(text, ui, input, dev, sf, now);
    render_transport_targets(text, snapshot, sim, ui, input, dev, sf, now);
    render_transport_impacts(text, ui, input, sf, now);
    render_death_nameplates(text, ui, input, dev, avatars, sf, now);
    render_cascade_callouts(text, ui, input, dev, sf, now);
    render_attack_badges(text, snapshot, sim, ui, input, dev, avatars, sf, now);
    render_attack_launch_notices(text, ui, input, dev, sf, now);
}

const CASCADE_CALLOUT_FONT_SIZE: f32 = 14.2;

fn cascade_callout_color(kind: crate::app::CascadeCalloutKind, chain_step: u8) -> [f32; 4] {
    use crate::app::CascadeCalloutKind;
    match kind {
        CascadeCalloutKind::Advance => {
            if chain_step >= 3 {
                crate::rgb(56, 239, 125)
            } else {
                crate::rgb(103, 232, 249)
            }
        }
        CascadeCalloutKind::Capture => crate::rgb(255, 209, 102),
        CascadeCalloutKind::Surge => crate::rgb(56, 239, 125),
        CascadeCalloutKind::KillStreak => match chain_step {
            0..=3 => crate::rgb(255, 195, 0),
            4..=6 => crate::rgb(255, 110, 64),
            _ => crate::rgb(255, 59, 92),
        },
        CascadeCalloutKind::MultiKill => match chain_step {
            0..=3 => crate::rgb(255, 145, 0),
            4..=6 => crate::rgb(255, 72, 48),
            _ => crate::rgb(255, 42, 85),
        },
    }
}

fn cascade_callout_kind_scale(kind: crate::app::CascadeCalloutKind) -> f32 {
    use crate::app::CascadeCalloutKind;
    match kind {
        CascadeCalloutKind::Advance => 0.90,
        CascadeCalloutKind::Capture | CascadeCalloutKind::Surge => 1.00,
        CascadeCalloutKind::KillStreak | CascadeCalloutKind::MultiKill => 1.14,
    }
}

fn render_cascade_callouts(
    text: &mut TextRenderer,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    now: Instant,
) {
    if ui.cascade_callouts.is_empty() {
        return;
    }
    let sf = sf.max(0.01);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let font_scale = dev.font_size_scale.max(0.1);
    let char_spacing = dev.font_char_spacing.max(0.1);
    let style_key = char_spacing.to_bits();
    let reduced_motion = ui.app.settings_state.reduced_motion;
    let mut ring_budget = if reduced_motion || ui.death_nameplates.len() > 8 {
        0usize
    } else {
        2usize
    };
    ui.cascade_callouts.retain_mut(|callout| {
        let raw_elapsed = now.duration_since(callout.start_time).as_secs_f32();
        if raw_elapsed < callout.delay_secs {
            return true;
        }
        let elapsed = raw_elapsed - callout.delay_secs;
        let emphatic = matches!(
            callout.kind,
            crate::app::CascadeCalloutKind::KillStreak | crate::app::CascadeCalloutKind::MultiKill
        );
        let Some(motion) = cascade_callout_motion(
            elapsed,
            crate::app::CASCADE_CALLOUT_DURATION,
            emphatic,
            reduced_motion,
        ) else {
            return false;
        };

        let world_y =
            callout.world_y - callout.initial_world_rise - motion.rise_progress * callout.rise_world;
        let mut screen = world_to_screen(callout.world_x, world_y, input, sf);
        screen[1] += callout.screen_offset_y * font_scale + motion.offset_y_px;
        if screen[0] < -140.0
            || screen[0] > screen_w + 140.0
            || screen[1] < -120.0
            || screen[1] > screen_h + 120.0
        {
            return true;
        }
        let step_boost = 1.0 + 0.05 * f32::from(callout.chain_step.saturating_sub(1).min(5));
        let kind_scale = cascade_callout_kind_scale(callout.kind);
        let base_rgb = cascade_callout_color(callout.kind, callout.chain_step);
        let flash = motion.flash;
        let color = [
            base_rgb[0] + (1.0 - base_rgb[0]) * flash,
            base_rgb[1] + (0.96 - base_rgb[1]) * flash,
            base_rgb[2] + (0.78 - base_rgb[2]) * flash,
            motion.alpha,
        ];

        let center_x = screen[0] * sf;
        let center_y = screen[1] * sf;
        if let Some(ring_t) = motion.ring_t
            && callout.ring_radius > 0.0
            && ring_budget > 0
            && text.remaining_instance_capacity() >= 1
        {
            let eased_ring = 1.0 - (1.0 - ring_t) * (1.0 - ring_t);
            let ring_fade = (1.0 - ring_t) * (1.0 - ring_t);
            text.push_ring(
                [center_x, center_y],
                callout.ring_radius * (0.35 + 0.65 * eased_ring) * sf,
                [base_rgb[0], base_rgb[1], base_rgb[2], ring_fade * 0.78],
                (1.8 * (1.0 - 0.45 * ring_t) * sf).max(1.0),
            );
            ring_budget -= 1;
        }

        let font_size = CASCADE_CALLOUT_FONT_SIZE
            * kind_scale
            * step_boost
            * motion.text_scale
            * font_scale
            * sf;
        let prepared = match &mut callout.prepared_label {
            Some((cached_key, existing)) if *cached_key == style_key => existing,
            slot => {
                *slot = Some((
                    style_key,
                    text.prepare_string(&callout.label, char_spacing, 0.0),
                ));
                &slot.as_ref().expect("just populated").1
            }
        };
        let measure = prepared.measure_at(font_size);
        let icon_extra = usize::from(callout.icon.is_some());
        let required = prepared.instance_count().saturating_add(1 + icon_extra);
        if text.remaining_instance_capacity() < required {
            return true;
        }

        if let Some(icon) = callout.icon {
            let icon_base = font_size * 0.95;
            let icon_w = icon_base * motion.icon_scale[0];
            let icon_h = icon_base * motion.icon_scale[1];
            let gap = 2.5 * sf;
            let total_w = icon_w + gap + measure.width;
            let row_left = center_x - total_w * 0.5;
            let icon_center = [
                row_left + icon_w * 0.5,
                center_y - measure.height * 0.05,
            ];
            let outline = dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, motion.alpha]);
            let icon_tint = [
                1.0,
                1.0 - 0.04 * flash,
                1.0 - 0.16 * flash,
                motion.alpha,
            ];
            match icon {
                crate::app::CascadeIcon::Status(NameplateStatusSprite::Troops) => {
                    text.push_troop_icon(icon_center, icon_h * 0.5, motion.alpha, outline);
                }
                crate::app::CascadeIcon::Status(sprite) => {
                    text.push_icon_scaled(
                        sprite,
                        icon_center,
                        [icon_w * 0.5, icon_h * 0.5],
                        icon_tint,
                        outline,
                    );
                }
                crate::app::CascadeIcon::Building(kind) => {
                    if let Some(uv_rect) = crate::render::gpu::building_sprite_uv(kind, 1) {
                        text.push_building_sprite(
                            icon_center,
                            [icon_w, icon_h],
                            uv_rect,
                            [1.0, 1.0, 1.0, motion.alpha],
                        );
                    }
                }
            }
            let _ = text.push_prepared_text(
                prepared,
                [
                    row_left + icon_w + gap,
                    center_y + measure.height * 0.35,
                ],
                font_size,
                color,
                dev_text_style(dev, sf, [0.0, 0.0, 0.0, motion.alpha]),
            );
        } else {
            let _ = text.push_prepared_text(
                prepared,
                [
                    center_x - measure.width * 0.5,
                    center_y + measure.height * 0.35,
                ],
                font_size,
                color,
                dev_text_style(dev, sf, [0.0, 0.0, 0.0, motion.alpha]),
            );
        }
        true
    });
}

fn render_attack_launch_notices(
    text: &mut TextRenderer,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    now: Instant,
) {
    let sf = sf.max(0.01);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let font_scale = dev.font_size_scale.max(0.1);
    let char_spacing = dev.font_char_spacing.max(0.1);
    ui.attack_launch_notices.retain(|notice| {
        let elapsed = now.duration_since(notice.start_time).as_secs_f32();
        let Some((progress, rise, scale)) =
            floating_notice_animation(elapsed, ATTACK_LAUNCH_NOTICE_DURATION)
        else {
            return false;
        };

        let screen = world_to_screen(notice.world_x, notice.world_y - rise, input, sf);
        if screen[0] < -150.0
            || screen[0] > screen_w + 150.0
            || screen[1] < -150.0
            || screen[1] > screen_h + 150.0
        {
            return true;
        }

        let alpha = 1.0 - progress;
        let color = [
            OUTGOING_ATTACK_TEXT_COLOR[0],
            OUTGOING_ATTACK_TEXT_COLOR[1],
            OUTGOING_ATTACK_TEXT_COLOR[2],
            alpha,
        ];
        let outline = dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]);
        let font_size = ATTACK_LAUNCH_NOTICE_FONT_SIZE * scale * font_scale * sf;
        let icon_diameter = font_size * TROOP_NOTICE_ICON_SCALE;
        let gap = 3.0 * sf;
        let measure = text.measure_string(&notice.amount_text, font_size, char_spacing);
        let required = troop_label_instance_requirement(&notice.amount_text);
        if text.remaining_instance_capacity() < required {
            return true;
        }
        let row_left = screen[0] * sf - (icon_diameter + gap + measure.width) * 0.5;
        text.push_troop_icon(
            [
                row_left + icon_diameter * 0.5,
                screen[1] * sf - icon_diameter * 0.5,
            ],
            icon_diameter * 0.5,
            alpha,
            dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha]),
        );
        text.push_string(
            &notice.amount_text,
            [row_left + icon_diameter + gap, screen[1] * sf],
            font_size,
            color,
            outline,
            (0.0, char_spacing),
        );
        true
    });
}

fn floating_notice_animation(elapsed: f32, duration: f32) -> Option<(f32, f32, f32)> {
    if !elapsed.is_finite() || !duration.is_finite() || duration <= 0.0 || elapsed >= duration {
        return None;
    }
    let progress = (elapsed / duration).clamp(0.0, 1.0);
    let scale = if elapsed < 0.5 {
        spring_overshoot(elapsed / 0.5)
    } else if elapsed > duration - 0.5 {
        spring_overshoot((duration - elapsed) / 0.5).clamp(0.0, 1.2)
    } else {
        1.0
    };
    Some((progress, progress * FLOATING_NOTICE_RISE, scale))
}

fn render_transport_targets(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    now: Instant,
) {
    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    if ui.transport_target_snapshot_tick != Some(snapshot.tick) {
        if ui
            .transport_target_snapshot_tick
            .is_some_and(|tick| snapshot.tick < tick)
        {
            ui.transport_target_markers.clear();
            ui.transport_impacts.clear();
        }

        ui.transport_target_seen.clear();
        if my_id != 0 {
            for fleet in &snapshot.fleets {
                if fleet.owner_id != my_id
                    || fleet.unit_type != sow_core::game::UnitType::TransportShip
                    || fleet.retreating
                {
                    continue;
                }
                let Some(tile_idx) = fleet.path.last().copied() else {
                    continue;
                };

                ui.transport_target_seen.insert(fleet.id);
                let marker =
                    ui.transport_target_markers
                        .entry(fleet.id)
                        .or_insert(TransportTargetMarker {
                            tile_idx,
                            eta_seconds: None,
                            eta_text: String::new(),
                            start_time: now,
                            fade_out_at: None,
                        });
                if marker.tile_idx != tile_idx || marker.fade_out_at.is_some() {
                    marker.tile_idx = tile_idx;
                    marker.start_time = now;
                }
                let eta_seconds = fleet
                    .eta_seconds
                    .filter(|seconds| seconds.is_finite())
                    .map(|seconds| seconds.ceil().max(1.0) as u32);
                if marker.eta_seconds != eta_seconds {
                    marker.eta_seconds = eta_seconds;
                    marker.eta_text = eta_seconds.map_or_else(String::new, |s| s.to_string());
                }
                marker.fade_out_at = None;
            }
        }

        let seen = &ui.transport_target_seen;
        for (id, marker) in &mut ui.transport_target_markers {
            if !seen.contains(id) && marker.fade_out_at.is_none() {
                marker.fade_out_at = Some(now);
            }
        }
        ui.transport_target_snapshot_tick = Some(snapshot.tick);
    }

    let player_rgb = snapshot
        .players
        .iter()
        .find(|player| player.id == my_id)
        .map(|player| {
            sow_core::player::player_territory_rgb(
                player.player_type,
                player.team,
                player.color,
                player.is_campaign_faction,
            )
        })
        .unwrap_or([0.5, 0.5, 0.5]);

    let sf = sf.max(0.01);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let map_w = sim.map_w.max(1);

    ui.transport_target_markers.retain(|_, marker| {
        let age = now.duration_since(marker.start_time).as_secs_f32();
        let fade_in = (age / TRANSPORT_TARGET_FADE_IN_SECS).clamp(0.0, 1.0);
        let fade_out = marker.fade_out_at.map_or(1.0, |start| {
            1.0 - (now.duration_since(start).as_secs_f32() / TRANSPORT_TARGET_FADE_OUT_SECS)
                .clamp(0.0, 1.0)
        });
        let alpha = fade_in.min(fade_out);
        if fade_out <= 0.0 {
            return false;
        }
        if alpha <= 0.0 {
            return true;
        }

        let x = (marker.tile_idx % map_w) as f32 + 0.5;
        let y = (marker.tile_idx / map_w) as f32 + 0.5;
        let center = world_to_screen(x, y, input, sf);
        if center[0] < -64.0
            || center[0] > screen_w + 64.0
            || center[1] < -64.0
            || center[1] > screen_h + 64.0
        {
            return true;
        }

        let rotation = if ui.app.settings_state.reduced_motion {
            0.125
        } else {
            (age * TRANSPORT_TARGET_ROTATIONS_PER_SEC).fract()
        };
        let center_px = [center[0] * sf, center[1] * sf];
        text.push_landing_beacon(
            center_px,
            25.0 * sf,
            rotation,
            [player_rgb[0], player_rgb[1], player_rgb[2], alpha],
        );
        if marker.eta_seconds.is_some() {
            let font_size = TRANSPORT_TARGET_ETA_FONT_SIZE * sf;
            let mut style = dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]);
            style.face_dilate += TRANSPORT_TARGET_ETA_BOLD_DILATE * sf;
            text.push_string(
                &marker.eta_text,
                [
                    center_px[0],
                    transport_eta_baseline_y(
                        &text.font_atlas_desc,
                        &marker.eta_text,
                        center_px[1],
                        font_size,
                    ),
                ],
                font_size,
                [1.0, 1.0, 1.0, alpha],
                style,
                (0.5, dev.font_char_spacing.max(0.1)),
            );
        }
        true
    });
}

fn render_transport_impacts(
    text: &mut TextRenderer,
    ui: &mut UiState,
    input: &InputState,
    sf: f32,
    now: Instant,
) {
    let sf = sf.max(0.01);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;

    ui.transport_impacts.retain(|impact| {
        let age = now.duration_since(impact.start_time).as_secs_f32();
        if age >= TRANSPORT_IMPACT_DURATION_SECS {
            return false;
        }

        let progress = (age / TRANSPORT_IMPACT_DURATION_SECS).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - progress).powi(3);
        let fade = (1.0 - progress).powi(2);
        let center = world_to_screen(
            impact.tile_x as f32 + 0.5,
            impact.tile_y as f32 + 0.5,
            input,
            sf,
        );
        if center[0] < -72.0
            || center[0] > screen_w + 72.0
            || center[1] < -72.0
            || center[1] > screen_h + 72.0
        {
            return true;
        }

        let center_px = [center[0] * sf, center[1] * sf];
        let radius = (5.0 + 25.0 * eased) * sf;
        text.push_ring(
            center_px,
            radius,
            [
                impact.color[0],
                impact.color[1],
                impact.color[2],
                0.9 * fade,
            ],
            (2.2 * sf).max(1.0),
        );
        if let Some(cue) = impact.cue_color {
            text.push_ring(
                center_px,
                radius * 1.35,
                [cue[0], cue[1], cue[2], 0.9 * fade],
                (2.6 * sf).max(1.0),
            );
        }
        text.push_ring(
            center_px,
            (radius * 0.62).max(2.0 * sf),
            [1.0, 0.92, 0.72, 0.58 * fade],
            (1.25 * sf).max(1.0),
        );
        text.push_disc(
            center_px,
            (4.5 * (1.0 - progress) * sf).max(0.5),
            [1.0, 1.0, 1.0, 0.82 * fade],
        );
        true
    });
}

fn render_death_nameplates(
    text: &mut TextRenderer,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    avatars: &AvatarSlots,
    sf: f32,
    now: Instant,
) {
    if ui.death_nameplates.is_empty() {
        return;
    }
    let sf = sf.max(0.01);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let font_scale = dev.font_size_scale.max(0.1);
    let char_spacing = dev.font_char_spacing.max(0.1);
    ui.death_nameplates.retain(|animation| {
        let elapsed = now.duration_since(animation.start_time).as_secs_f32();
        death_animation(elapsed).is_some()
            || (animation.reward.is_some() && gold_reward_elapsed(elapsed).is_some())
    });

    // Draw local gold first so rewards keep their full row if the shared text buffer is tight.
    for animation in &mut ui.death_nameplates {
        let Some((is_assist, amount_len)) = animation
            .reward
            .as_ref()
            .map(|reward| (reward.is_assist, reward.amount.chars().count()))
        else {
            continue;
        };
        let casualty_len = animation
            .casualties
            .as_ref()
            .map_or(0, |casualties| casualties.chars().count());
        let elapsed = now.duration_since(animation.start_time).as_secs_f32();
        let Some(reward_elapsed) = gold_reward_elapsed(elapsed) else {
            continue;
        };
        let Some((progress, rise, scale)) =
            floating_notice_animation(reward_elapsed, GOLD_REWARD_DURATION)
        else {
            continue;
        };
        let scale = scale.min(GOLD_REWARD_MAX_TEXT_SCALE);
        let mut screen = world_to_screen(animation.world_x, animation.world_y - rise, input, sf);
        let reward_scale = font_scale * GOLD_REWARD_SCALE;
        let rough_width = amount_len as f32 * GOLD_REWARD_FONT_SIZE * reward_scale
            + GOLD_REWARD_COIN_SIZE * reward_scale
            + if is_assist {
                (GOLD_REWARD_ASSIST_SIZE + 2.0 * DEATH_GOLD_ROW_GAP) * reward_scale
            } else {
                DEATH_GOLD_ROW_GAP * reward_scale
            }
            + if casualty_len > 0 {
                (DEATH_CASUALTY_ICON_SIZE
                    + casualty_len as f32 * GOLD_REWARD_FONT_SIZE
                    + 3.0 * DEATH_GOLD_ROW_GAP)
                    * reward_scale
            } else {
                0.0
            };
        if screen[0] < -rough_width
            || screen[0] > screen_w + rough_width
            || screen[1] < -256.0
            || screen[1] > screen_h + 256.0
        {
            continue;
        }

        prepare_death_nameplate_text(text, animation, char_spacing);
        let Some(prepared) = animation.prepared_reward.as_ref() else {
            continue;
        };
        let prepared_casualties = animation.prepared_casualties.as_ref();
        let Some(coin_uv) = text.avatar_uv(AVATAR_GOLD_SLOT) else {
            continue;
        };
        let alpha = 1.0 - progress;
        let streak_scale = if ui.cascade_streak > 1 {
            1.0 + 0.04 * f32::from(ui.cascade_streak.saturating_sub(1).min(4))
        } else {
            1.0
        };
        let font_size = GOLD_REWARD_FONT_SIZE * scale * streak_scale * reward_scale * sf;
        let measure = prepared.measure_at(font_size);
        let casualty_measure = prepared_casualties.map(|prepared| prepared.measure_at(font_size));
        let badge_size = GOLD_REWARD_ASSIST_SIZE * reward_scale * sf;
        let coin_size = GOLD_REWARD_COIN_SIZE * reward_scale * sf;
        let casualty_icon_size = DEATH_CASUALTY_ICON_SIZE * reward_scale * sf;
        let gap = DEATH_GOLD_ROW_GAP * 0.5 * reward_scale * sf;
        let death_scale = font_scale * DEATH_NAMEPLATE_SCALE;
        let death_font_size = DEATH_NAMEPLATE_FONT_SIZE * death_scale * sf;
        let death_measure = animation.prepared_name.measure_at(death_font_size);
        let death_bounds = animation
            .prepared_name
            .visual_bounds_at(death_font_size)
            .unwrap_or([
                0.0,
                -death_measure.height * 0.65,
                0.0,
                death_measure.height * 0.35,
            ]);
        let max_reward_font_size =
            GOLD_REWARD_FONT_SIZE * GOLD_REWARD_MAX_TEXT_SCALE * reward_scale * sf;
        let max_reward_measure = prepared.measure_at(max_reward_font_size);
        let max_reward_bounds = prepared.visual_bounds_at(max_reward_font_size).unwrap_or([
            0.0,
            -max_reward_measure.height * 0.65,
            0.0,
            max_reward_measure.height * 0.35,
        ]);
        let outline_clearance = (dev.font_outline_thickness.max(0.0)
            + dev.font_shadow_y.abs()
            + dev.font_underlay_softness.max(0.0))
            * sf;
        let death_icon_size = DEATH_NAMEPLATE_ICON_SIZE * death_scale * sf;
        let death_gap = 2.0 * death_scale * sf;
        let death_stack_height = death_icon_size + death_gap + death_measure.height;
        let death_stack_top = -death_stack_height * 0.5;
        let death_name_baseline =
            death_stack_top + death_icon_size + death_gap + death_measure.height * 0.85;
        let death_bottom = (death_icon_size * 0.5 + outline_clearance)
            .max(death_name_baseline + death_bounds[3] + outline_clearance);
        let gold_half = badge_size.max(coin_size) * 0.5;
        let reward_baseline = max_reward_measure.height * 0.35;
        let gold_top = (-gold_half).min(reward_baseline + max_reward_bounds[1] - outline_clearance);
        let row_separation = death_gold_row_offset(death_bottom, gold_top, sf);
        screen[1] += row_separation / sf;
        let left_count = if is_assist { badge_size + gap } else { 0.0 };
        let casualty_separator = if casualty_measure.is_some() {
            gap * 2.0
        } else {
            0.0
        };
        let casualty_width = casualty_measure
            .as_ref()
            .map(|measure| casualty_icon_size + gap + measure.width)
            .unwrap_or(0.0);
        let row_width =
            left_count + coin_size + gap + measure.width + casualty_separator + casualty_width;
        let center_px = [screen[0] * sf, screen[1] * sf];
        let row_left = center_px[0] - row_width * 0.5;
        if screen[1] < -64.0 || screen[1] > screen_h + 64.0 {
            continue;
        }
        let required = prepared
            .instance_count()
            .saturating_add(1)
            .saturating_add(usize::from(is_assist))
            .saturating_add(
                prepared_casualties
                    .map_or(0, |prepared| prepared.instance_count().saturating_add(1)),
            );
        if text.remaining_instance_capacity() < required {
            continue;
        }

        let base_color = crate::rgb(250, 204, 21);
        let tint = [base_color[0], base_color[1], base_color[2], alpha];
        let mut content_left = row_left;
        if is_assist {
            text.push_status_sprite(
                NameplateStatusSprite::Allied,
                [content_left + badge_size * 0.5, center_px[1]],
                badge_size,
                [1.0, 1.0, 1.0, alpha],
            );
            content_left += badge_size + gap;
        }
        text.push_sprite(
            [content_left + coin_size * 0.5, center_px[1]],
            [coin_size; 2],
            coin_uv,
            tint,
        );
        let _ = text.push_prepared_text(
            prepared,
            [
                content_left + coin_size + gap,
                center_px[1] + measure.height * 0.35,
            ],
            font_size,
            tint,
            dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]),
        );
        if let (Some(prepared), Some(measure)) = (prepared_casualties, casualty_measure) {
            let casualty_left = content_left + coin_size + gap + measure.width + casualty_separator;
            text.push_status_sprite(
                NameplateStatusSprite::Troops,
                [casualty_left + casualty_icon_size * 0.5, center_px[1]],
                casualty_icon_size,
                [1.0, 1.0, 1.0, alpha],
            );
            let casualty_color = crate::rgb(255, 120, 120);
            let casualty_tint = [
                casualty_color[0],
                casualty_color[1],
                casualty_color[2],
                alpha,
            ];
            let _ = text.push_prepared_text(
                prepared,
                [
                    casualty_left + casualty_icon_size + gap,
                    center_px[1] + measure.height * 0.35,
                ],
                font_size,
                casualty_tint,
                dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]),
            );
        }
    }

    let reduced_motion = ui.app.settings_state.reduced_motion;
    let mut death_vfx_budget = if ui.death_nameplates.len() > 10 {
        12usize
    } else {
        24usize
    };
    for animation in &mut ui.death_nameplates {
        let elapsed = now.duration_since(animation.start_time).as_secs_f32();
        let Some((_, eased, alpha)) = death_animation(elapsed) else {
            continue;
        };
        if alpha <= 0.0 {
            continue;
        }
        let entry_punch = if !reduced_motion && elapsed < 0.36 {
            let pt = (elapsed / 0.36).clamp(0.0, 1.0);
            1.0 + 0.24 * (-7.5 * pt).exp() * (pt * 20.0).cos()
        } else {
            1.0
        };
        let death_scale = font_scale * DEATH_NAMEPLATE_SCALE * entry_punch;
        let font_size = DEATH_NAMEPLATE_FONT_SIZE * death_scale;
        let arc = 4.0 * eased * (1.0 - eased);
        let center = world_to_screen(
            animation.world_x + animation.curve_direction * DEATH_NAMEPLATE_ARC_OFFSET * arc,
            animation.world_y - DEATH_NAMEPLATE_RISE * eased,
            input,
            sf,
        );
        let icon_size = DEATH_NAMEPLATE_ICON_SIZE * death_scale;
        let gap = 2.0 * death_scale;
        let rough_name_width = animation.name.chars().count() as f32 * font_size;
        let rough_width = icon_size + gap + rough_name_width.max(icon_size);
        if center[0] < -(rough_width * 0.5 + 8.0)
            || center[0] > screen_w + rough_width * 0.5 + 8.0
            || center[1] < -64.0
            || center[1] > screen_h + 64.0
        {
            continue;
        }

        prepare_death_nameplate_text(text, animation, char_spacing);
        let name_measure = animation.prepared_name.measure_at(font_size * sf);
        let icon_block = (icon_size + gap) * sf;
        let avatar_size = icon_size * sf;
        let name_stack_width = name_measure.width.max(avatar_size);
        let stack_height = avatar_size + gap * sf + name_measure.height;
        let death_icon = death_icon(animation.by_nuke);
        let row_width = icon_block + name_stack_width;
        let margin = row_width.max(stack_height) * 0.5 + 8.0;
        if center[0] < -margin
            || center[0] > screen_w + margin
            || center[1] < -margin
            || center[1] > screen_h + margin
        {
            continue;
        }

        let required = animation.prepared_name.instance_count().saturating_add(2);
        if text.remaining_instance_capacity() < required {
            continue;
        }

        // World-anchored sovereign elimination burst & portrait shatter halo.
        if !reduced_motion
            && elapsed < 0.68
            && death_vfx_budget >= 4
            && text.remaining_instance_capacity() >= required.saturating_add(6)
        {
            let bt = (elapsed / 0.68).clamp(0.0, 1.0);
            let b_ease = 1.0 - (1.0 - bt).powi(3);
            let b_fade = (1.0 - bt).powi(2);
            let origin = world_to_screen(animation.world_x, animation.world_y, input, sf);
            let origin_px = [origin[0] * sf, origin[1] * sf];
            if animation.casualties.is_none() {
                let base_r = (5.0 + 22.0 * b_ease) * sf;
                text.push_ring(
                    origin_px,
                    base_r,
                    [
                        animation.color[0].max(0.25),
                        animation.color[1].max(0.25),
                        animation.color[2].max(0.25),
                        0.88 * b_fade,
                    ],
                    (1.8 * (1.0 - 0.35 * bt) * sf).max(1.0),
                );
                let accent = if animation.by_nuke {
                    [1.0, 0.80, 0.22, 0.90 * b_fade]
                } else {
                    [1.0, 0.34, 0.22, 0.85 * b_fade]
                };
                text.push_ring(
                    origin_px,
                    (base_r * 0.62).max(2.5 * sf),
                    accent,
                    (1.25 * sf).max(1.0),
                );
            }
            if elapsed < 0.42 {
                let ct = (elapsed / 0.42).clamp(0.0, 1.0);
                text.push_cross(
                    origin_px,
                    (5.0 + 10.0 * ct) * sf,
                    [1.0, 0.95, 0.78, (1.0 - ct) * 0.88],
                    (1.5 * sf).max(1.0),
                );
                if animation.casualties.is_none() {
                    text.push_disc(
                        origin_px,
                        (4.0 * (1.0 - ct) * sf).max(0.5),
                        [1.0, 1.0, 1.0, (1.0 - ct) * 0.85],
                    );
                }
            }
            death_vfx_budget -= if animation.casualties.is_none() { 4 } else { 1 };
        }

        let center_px = [center[0] * sf, center[1] * sf];
        let row_left = center_px[0] - row_width * 0.5;
        let stack_top = center_px[1] - stack_height * 0.5;
        let name_left = row_left + icon_block + (name_stack_width - name_measure.width) * 0.5;
        let emoji_outline = dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha]);
        let icon_center = [row_left + icon_size * sf * 0.5, center_px[1]];
        let avatar_center = [
            row_left + icon_block + name_stack_width * 0.5,
            stack_top + avatar_size * 0.5,
        ];
        if !reduced_motion
            && elapsed < 0.52
            && death_vfx_budget >= 1
            && text.remaining_instance_capacity() >= required.saturating_add(2)
        {
            let ht = (elapsed / 0.52).clamp(0.0, 1.0);
            let h_ease = 1.0 - (1.0 - ht) * (1.0 - ht);
            text.push_ring(
                avatar_center,
                avatar_size * (0.52 + 0.56 * h_ease),
                [
                    animation.color[0].max(0.35),
                    animation.color[1].max(0.35),
                    animation.color[2].max(0.35),
                    (1.0 - ht) * 0.78 * alpha,
                ],
                (1.6 * sf).max(1.0),
            );
            death_vfx_budget -= 1;
        }
        let icon_flash = if !reduced_motion && elapsed < 0.22 {
            (1.0 - elapsed / 0.22).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let icon_scale = 1.0 + 0.18 * icon_flash;
        text.push_icon(
            death_icon,
            icon_center,
            icon_size * sf * 0.5 * icon_scale,
            [1.0, 1.0 - 0.15 * icon_flash, 1.0 - 0.35 * icon_flash, alpha],
            emoji_outline,
        );
        let avatar = crate::render::world::nameplates::prepare_death_avatar(
            text,
            avatars,
            &animation.avatar,
            avatar_center,
            avatar_size * 0.5,
        );
        crate::render::world::nameplates::paint_prepared_avatar(text, avatar, alpha);
        let text_flash = icon_flash * 0.35;
        let _ = text.push_prepared_text(
            &animation.prepared_name,
            [
                name_left,
                stack_top + avatar_size + gap * sf + name_measure.height * 0.85,
            ],
            font_size * sf,
            [
                (animation.color[0] + text_flash).min(1.0),
                (animation.color[1] + text_flash).min(1.0),
                (animation.color[2] + text_flash).min(1.0),
                alpha,
            ],
            dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]),
        );
    }
}

fn prepare_death_nameplate_text(
    text: &TextRenderer,
    animation: &mut crate::app::DeathNameplateAnimation,
    char_spacing: f32,
) {
    let style_key = char_spacing.to_bits();
    if animation.text_style_key == Some(style_key) {
        return;
    }
    animation.prepared_name = text.prepare_string(&animation.name, char_spacing, 0.0);
    animation.prepared_casualties = animation
        .casualties
        .as_ref()
        .map(|casualties| text.prepare_string(casualties, char_spacing, 0.0));
    animation.prepared_reward = animation
        .reward
        .as_ref()
        .map(|reward| text.prepare_string(&reward.amount, char_spacing, 0.0));
    animation.text_style_key = Some(style_key);
}

fn render_click_markers(
    text: &mut TextRenderer,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    now: Instant,
) {
    if !dev.vfx_click_markers {
        ui.click_markers.clear();
        return;
    }

    let sf = sf.max(0.01);
    let zoom_scaled = (input.camera_zoom / sf).max(0.0).min(1.0);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;

    ui.click_markers.retain(|marker| {
        let elapsed = now.duration_since(marker.start_time).as_secs_f32();
        let duration = match marker.kind {
            crate::app::ClickMarkerKind::Generic => CLICK_MARKER_DURATION,
            crate::app::ClickMarkerKind::TransportOrder => NAVAL_ORDER_MARKER_DURATION,
        };
        if elapsed >= duration {
            return false;
        }

        let t = (elapsed / duration).clamp(0.0, 1.0);
        let center = world_to_screen(marker.world_x, marker.world_y, input, sf);
        if center[0] < -80.0
            || center[0] > screen_w + 80.0
            || center[1] < -80.0
            || center[1] > screen_h + 80.0
        {
            return true;
        }

        let center = [center[0] * sf, center[1] * sf];
        match marker.kind {
            crate::app::ClickMarkerKind::Generic => {
                let color = [1.0, 1.0, 1.0, (1.0 - t) * 200.0 / 255.0];
                text.push_ring(center, 24.0 * t * zoom_scaled * sf, color, 1.5 * sf);
                text.push_cross(center, 4.0 * zoom_scaled * (1.0 - t) * sf, color, 1.0 * sf);
            }
            crate::app::ClickMarkerKind::TransportOrder => {
                let color = [0.28, 0.88, 1.0, (1.0 - t) * 0.95];
                text.push_ring(center, 34.0 * t * zoom_scaled * sf, color, 2.2 * sf);
                text.push_cross(center, 5.0 * zoom_scaled * (1.0 - t) * sf, color, 1.5 * sf);
            }
        }
        true
    });
}

fn render_attack_badges(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    avatars: &AvatarSlots,
    sf: f32,
    now: Instant,
) {
    if !dev.vfx_attack_badges {
        ui.attack_badge_labels.clear();
        ui.attack_badge_active_ids.clear();
        ui.attack_badge_cache_tick = None;
        return;
    }

    let style_key = [
        dev.font_size_scale.to_bits(),
        dev.font_char_spacing.to_bits(),
    ];
    if ui.attack_badge_style_key != Some(style_key) {
        ui.attack_badge_labels.clear();
        ui.attack_badge_style_key = Some(style_key);
    }
    if ui.attack_badge_cache_tick != Some(snapshot.tick) {
        ui.attack_badge_active_ids.clear();
        ui.attack_badge_active_ids
            .extend(snapshot.attacks.iter().map(|attack| attack.id));
        let active_ids = &ui.attack_badge_active_ids;
        ui.attack_badge_labels
            .retain(|id, _| active_ids.contains(id));
        ui.attack_badge_cache_tick = Some(snapshot.tick);
    }

    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    let sf = sf.max(0.01);
    let font_size = ATTACK_BADGE_FONT_SIZE * dev.font_size_scale.max(0.1);
    let char_spacing = dev.font_char_spacing.max(0.1);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let text_style = dev_text_style(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let emoji_outline = dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, 0.9]);
    let horizontal_cull_margin = 80.0 + font_size + 3.0;

    for attack in &snapshot.attacks {
        let Some((attacker_id, text_color)) = attack_badge_owner_and_text_color(attack, my_id)
        else {
            continue;
        };
        if !attack.front_cx.is_finite()
            || !attack.front_cy.is_finite()
            || (attack.front_cx == 0.0 && attack.front_cy == 0.0)
        {
            continue;
        }

        let screen = world_to_screen(attack.front_cx + 0.5, attack.front_cy + 0.5, input, sf);
        if screen[0] < -horizontal_cull_margin
            || screen[0] > screen_w + horizontal_cull_margin
            || screen[1] < -40.0
            || screen[1] > screen_h + 40.0
        {
            continue;
        }

        let needs_update = ui.attack_badge_labels.get(&attack.id).is_none_or(|entry| {
            (entry.troops - attack.troops).abs() > 0.0001
                && now.duration_since(entry.last_update).as_secs_f32() >= ATTACK_BADGE_UPDATE_SECS
        });
        if needs_update {
            let label = crate::utils::format_number(attack.troops);
            let measure = text.measure_string(&label, font_size * sf, char_spacing);
            ui.attack_badge_labels.insert(
                attack.id,
                crate::app::AttackBadgeLabel {
                    troops: attack.troops,
                    text: label,
                    width: measure.width / sf,
                    height: measure.height / sf,
                    last_update: now,
                },
            );
        }

        let Some(entry) = ui.attack_badge_labels.get(&attack.id) else {
            continue;
        };
        let icon_size = font_size;
        let gap = 3.0;
        let troop_row_width = icon_size + gap + entry.width;
        let row_height = icon_size.max(entry.height);
        let total_height = icon_size + gap + row_height;
        let row_left = screen[0] - troop_row_width * 0.5;
        let avatar_top = screen[1] - total_height * 0.5;
        let row_top = avatar_top + icon_size + gap;
        if row_left < 0.0
            || row_left + troop_row_width > screen_w
            || avatar_top < 0.0
            || row_top + row_height > screen_h
        {
            continue;
        }

        let required = attack_badge_instance_requirement(&entry.text);
        if text.remaining_instance_capacity() < required {
            continue;
        }

        let attacker = snapshot
            .players
            .iter()
            .find(|player| player.id == attacker_id);
        crate::render::world::nameplates::paint_attack_badge_avatar(
            text,
            avatars,
            attacker,
            [screen[0] * sf, (avatar_top + icon_size * 0.5) * sf],
            icon_size * sf,
        );

        text.push_troop_icon(
            [
                (row_left + icon_size * 0.5) * sf,
                (row_top + row_height * 0.5) * sf,
            ],
            icon_size * 0.5 * sf,
            1.0,
            emoji_outline,
        );
        text.push_string(
            &entry.text,
            [
                (row_left + icon_size + gap) * sf,
                (row_top + (row_height - entry.height) * 0.5 + entry.height * 0.85) * sf,
            ],
            font_size * sf,
            text_color,
            text_style,
            (0.0, char_spacing),
        );
    }
}

fn attack_badge_owner_and_text_color(
    attack: &AttackSnapshot,
    my_id: u16,
) -> Option<(u16, [f32; 4])> {
    if my_id == 0 || !attack.troops.is_finite() || attack.troops <= 0.0 {
        return None;
    }
    let text_color = if attack.target_owner == my_id {
        INCOMING_ATTACK_TEXT_COLOR
    } else if attack.owner_id == my_id {
        OUTGOING_ATTACK_TEXT_COLOR
    } else {
        return None;
    };
    Some((attack.owner_id, text_color))
}
