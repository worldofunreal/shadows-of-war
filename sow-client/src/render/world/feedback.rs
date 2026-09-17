use crate::app::{InputState, SimState, UiState};
use crate::render::gpu::TextRenderer;
use crate::render::world::overlays::{INLINE_EMOJI_SCALE, world_to_screen};
use crate::render::{dev_emoji_outline, dev_text_style};
use crate::theme::dev_config::DevConfig;
use sow_core::protocol::{AttackSnapshot, SimSnapshot};
use web_time::Instant;

const CLICK_MARKER_DURATION: f32 = 0.16;
const NOTICE_FONT_SIZE: f32 = 14.0;
const NOTICE_RISE: f32 = 6.5;
const ATTACK_BADGE_FONT_SIZE: f32 = 13.0;
const ATTACK_BADGE_UPDATE_SECS: f32 = 0.09;

pub(crate) fn render(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    dev: &DevConfig,
    sf: f32,
    now: Instant,
) {
    render_click_markers(text, ui, input, dev, sf, now);
    render_attack_badges(text, snapshot, sim, ui, input, dev, sf, now);
    render_floating_notices(text, ui, input, dev, sf, now);
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
        if elapsed >= CLICK_MARKER_DURATION {
            return false;
        }

        let t = (elapsed / CLICK_MARKER_DURATION).clamp(0.0, 1.0);
        let center = world_to_screen(marker.world_x, marker.world_y, input, sf);
        if center[0] < -80.0
            || center[0] > screen_w + 80.0
            || center[1] < -80.0
            || center[1] > screen_h + 80.0
        {
            return true;
        }

        let color = [1.0, 1.0, 1.0, (1.0 - t) * 200.0 / 255.0];
        let center = [center[0] * sf, center[1] * sf];
        text.push_ring(center, 24.0 * t * zoom_scaled * sf, color, 1.5 * sf);
        text.push_cross(center, 4.0 * zoom_scaled * (1.0 - t) * sf, color, 1.0 * sf);
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
    sf: f32,
    now: Instant,
) {
    if !dev.vfx_attack_badges {
        ui.attack_badge_labels.clear();
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
        ui.attack_badge_labels
            .retain(|id, _| snapshot.attacks.iter().any(|attack| attack.id == *id));
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

    for attack in &snapshot.attacks {
        let Some(color) = attack_badge_color(attack, my_id) else {
            continue;
        };
        if !attack.front_cx.is_finite()
            || !attack.front_cy.is_finite()
            || (attack.front_cx == 0.0 && attack.front_cy == 0.0)
        {
            continue;
        }

        let screen = world_to_screen(attack.front_cx + 0.5, attack.front_cy + 0.5, input, sf);
        if screen[0] < -80.0
            || screen[0] > screen_w + 80.0
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
            let measure =
                text.measure_string(&label, font_size * sf, char_spacing, INLINE_EMOJI_SCALE);
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
        let row_width = icon_size + 3.0 + entry.width;
        let row_left = screen[0] - row_width * 0.5;
        let row_top = screen[1] - entry.height * 0.5;

        text.push_emoji(
            "⚔",
            [
                (row_left + icon_size * 0.5) * sf,
                (row_top + icon_size * 0.5) * sf,
            ],
            icon_size * 0.5 * sf,
            color,
            emoji_outline,
        );
        text.push_string(
            &entry.text,
            [
                (row_left + icon_size + 3.0) * sf,
                (row_top + entry.height * 0.85) * sf,
            ],
            font_size * sf,
            color,
            text_style,
            (0.0, char_spacing, INLINE_EMOJI_SCALE),
        );
    }
}

fn attack_badge_color(attack: &AttackSnapshot, my_id: u16) -> Option<[f32; 4]> {
    if my_id == 0 || !attack.troops.is_finite() || attack.troops <= 0.0 {
        return None;
    }
    if attack.target_owner == my_id {
        Some(crate::rgb(255, 90, 90))
    } else if attack.owner_id == my_id {
        Some(crate::rgb(6, 182, 212))
    } else {
        None
    }
}

fn render_floating_notices(
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

    ui.floating_notices.retain(|notice| {
        let elapsed = now.duration_since(notice.start_time).as_secs_f32();
        let Some((t, rise, scale)) = notice_animation(elapsed, notice.duration.as_secs_f32())
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

        let alpha = (1.0 - t).clamp(0.0, 1.0) * notice.color[3];
        let color = [notice.color[0], notice.color[1], notice.color[2], alpha];
        let outline = dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]);
        let emoji_scale = if notice.text.contains('⚔') {
            INLINE_EMOJI_SCALE * 0.65
        } else {
            INLINE_EMOJI_SCALE
        };
        text.push_string(
            &notice.text,
            [screen[0] * sf, screen[1] * sf],
            NOTICE_FONT_SIZE * scale * font_scale * sf,
            color,
            outline,
            (0.5, char_spacing, emoji_scale),
        );
        true
    });
}

fn notice_animation(elapsed: f32, duration: f32) -> Option<(f32, f32, f32)> {
    if !elapsed.is_finite() || !duration.is_finite() || duration <= 0.0 || elapsed >= duration {
        return None;
    }
    let t = (elapsed / duration).clamp(0.0, 1.0);
    let scale = if elapsed < 0.5 {
        spring_overshoot(elapsed / 0.5)
    } else if elapsed > duration - 0.5 {
        spring_overshoot((duration - elapsed) / 0.5).clamp(0.0, 1.2)
    } else {
        1.0
    };
    Some((t, t * NOTICE_RISE, scale))
}

#[inline]
fn spring_overshoot(t: f32) -> f32 {
    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_badges_only_include_local_incoming_or_outgoing_attacks() {
        let base = AttackSnapshot {
            id: 1,
            owner_id: 2,
            target_owner: 3,
            troops: 100.0,
            retreating: false,
            front_cx: 1.0,
            front_cy: 1.0,
        };
        assert_eq!(attack_badge_color(&base, 2), Some(crate::rgb(6, 182, 212)));
        assert_eq!(attack_badge_color(&base, 3), Some(crate::rgb(255, 90, 90)));
        assert_eq!(attack_badge_color(&base, 4), None);
    }

    #[test]
    fn notice_animation_rises_and_expires() {
        let start = notice_animation(0.0, 1.5).unwrap();
        let middle = notice_animation(0.75, 1.5).unwrap();
        assert_eq!(start.0, 0.0);
        assert_eq!(start.1, 0.0);
        assert_eq!(middle.0, 0.5);
        assert_eq!(middle.1, NOTICE_RISE * 0.5);
        assert_eq!(middle.2, 1.0);
        assert!(notice_animation(1.5, 1.5).is_none());
    }

    #[test]
    fn spring_starts_at_zero() {
        assert_eq!(spring_overshoot(0.0), 0.0);
    }
}
