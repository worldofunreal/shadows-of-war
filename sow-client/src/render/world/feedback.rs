use crate::app::{InputState, SimState, TransportTargetMarker, UiState};
use crate::death_nameplate::{death_animation, death_emoji};
use crate::render::gpu::TextRenderer;
use crate::render::world::overlays::{INLINE_EMOJI_SCALE, world_to_screen};
use crate::render::{dev_emoji_outline, dev_text_style};
use crate::theme::dev_config::DevConfig;
use sow_core::protocol::{AttackSnapshot, SimSnapshot};
use web_time::Instant;

const CLICK_MARKER_DURATION: f32 = 0.16;
const TRANSPORT_TARGET_FADE_IN_SECS: f32 = 0.2;
const TRANSPORT_TARGET_FADE_OUT_SECS: f32 = 0.3;
const TRANSPORT_IMPACT_DURATION_SECS: f32 = 0.62;
const NOTICE_FONT_SIZE: f32 = 14.0;
const NOTICE_RISE: f32 = 6.5;
const DEATH_NAMEPLATE_FONT_SIZE: f32 = 18.0;
const DEATH_NAMEPLATE_RISE: f32 = 5.0;
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
    render_transport_targets(text, snapshot, sim, ui, input, sf, now);
    render_transport_impacts(text, ui, input, sf, now);
    render_death_nameplates(text, ui, input, dev, sf, now);
    render_attack_badges(text, snapshot, sim, ui, input, dev, sf, now);
    render_floating_notices(text, ui, input, dev, sf, now);
}

fn render_transport_targets(
    text: &mut TextRenderer,
    snapshot: &SimSnapshot,
    sim: &SimState,
    ui: &mut UiState,
    input: &InputState,
    sf: f32,
    now: Instant,
) {
    if ui.transport_target_snapshot_tick != Some(snapshot.tick) {
        if ui
            .transport_target_snapshot_tick
            .is_some_and(|tick| snapshot.tick < tick)
        {
            ui.transport_target_markers.clear();
            ui.transport_impacts.clear();
        }

        ui.transport_target_seen.clear();
        let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
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
                            start_time: now,
                            fade_out_at: None,
                        });
                if marker.tile_idx != tile_idx || marker.fade_out_at.is_some() {
                    marker.tile_idx = tile_idx;
                    marker.start_time = now;
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

    let my_id = sim.my_player_id.unwrap_or(ui.app.hud_state.my_player_id);
    let player_color = snapshot
        .players
        .iter()
        .find(|player| player.id == my_id)
        .map(|player| {
            player
                .team
                .map_or(player.color, sow_core::player::team_territory_rgb)
        })
        .unwrap_or([0.13, 0.83, 0.94]);
    let color = [
        player_color[0] * 0.68 + 0.32,
        player_color[1] * 0.68 + 0.32,
        player_color[2] * 0.68 + 0.32,
    ];
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

        let pulse = (age * 2.7).sin() * 0.5 + 0.5;
        let outer_radius = (19.0 + 2.0 * pulse) * sf;
        let inner_radius = (12.0 + pulse) * sf;
        let center_px = [center[0] * sf, center[1] * sf];
        text.push_ring(
            center_px,
            outer_radius,
            [color[0], color[1], color[2], alpha * 0.42],
            (1.25 * sf).max(1.0),
        );
        text.push_arc(
            center_px,
            inner_radius,
            0.38 + pulse * 0.26,
            [color[0], color[1], color[2], alpha * 0.92],
            (1.8 * sf).max(1.0),
        );
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
    sf: f32,
    now: Instant,
) {
    let sf = sf.max(0.01);
    let screen_w = input.screen_w / sf;
    let screen_h = input.screen_h / sf;
    let font_scale = dev.font_size_scale.max(0.1);
    let font_size = DEATH_NAMEPLATE_FONT_SIZE * font_scale;
    let char_spacing = dev.font_char_spacing.max(0.1);

    ui.death_nameplates.retain(|animation| {
        let elapsed = now.duration_since(animation.start_time).as_secs_f32();
        let Some((_, eased, alpha)) = death_animation(elapsed) else {
            return false;
        };
        if alpha <= 0.0 {
            return true;
        }

        let center = world_to_screen(
            animation.world_x,
            animation.world_y - DEATH_NAMEPLATE_RISE * eased,
            input,
            sf,
        );
        let measure = text.measure_string(
            &animation.name,
            font_size * sf,
            char_spacing,
            INLINE_EMOJI_SCALE,
        );
        let name_width = measure.width / sf;
        let name_height = measure.height / sf;
        let icon_size = font_size * 0.8;
        let icon_center = [
            center[0],
            center[1] - name_height * 0.5 - icon_size * 0.5 - 4.0,
        ];
        let margin_x = (name_width * 0.5).max(icon_size * 0.5) + 8.0;
        let margin_y = name_height + icon_size + 8.0;
        if center[0] < -margin_x
            || center[0] > screen_w + margin_x
            || center[1] < -margin_y
            || center[1] > screen_h + margin_y
        {
            return true;
        }

        let color = [animation.color[0], animation.color[1], animation.color[2], alpha];
        let text_style = dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]);
        let emoji_outline = dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha]);
        text.push_string(
            &animation.name,
            [center[0] * sf, (center[1] + name_height * 0.35) * sf],
            font_size * sf,
            color,
            text_style,
            (0.5, char_spacing, INLINE_EMOJI_SCALE),
        );
        text.push_emoji(
            death_emoji(animation.by_nuke),
            [icon_center[0] * sf, icon_center[1] * sf],
            icon_size * sf * 0.5,
            [1.0, 1.0, 1.0, alpha],
            emoji_outline,
        );
        true
    });
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
