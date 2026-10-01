use crate::app::{InputState, SimState, TransportTargetMarker, UiState};
use crate::death_nameplate::{death_animation, death_emoji};
use crate::render::gpu::TextRenderer;
use crate::render::world::overlays::{INLINE_EMOJI_SCALE, world_to_screen};
use crate::render::{dev_emoji_outline, dev_text_style};
use crate::theme::dev_config::DevConfig;
use sow_core::protocol::{AttackSnapshot, SimSnapshot};
use sow_render::FontAtlas;
use web_time::Instant;

const CLICK_MARKER_DURATION: f32 = 0.16;
const TRANSPORT_TARGET_FADE_IN_SECS: f32 = 0.2;
const TRANSPORT_TARGET_FADE_OUT_SECS: f32 = 0.3;
const TRANSPORT_TARGET_ROTATIONS_PER_SEC: f32 = 1.0;
const TRANSPORT_TARGET_ETA_FONT_SIZE: f32 = 11.0;
const TRANSPORT_IMPACT_DURATION_SECS: f32 = 0.62;
const DEATH_NAMEPLATE_FONT_SIZE: f32 = 18.0;
const DEATH_NAMEPLATE_RISE: f32 = 5.0;
const ATTACK_BADGE_FONT_SIZE: f32 = 13.0;
const ATTACK_BADGE_UPDATE_SECS: f32 = 0.09;

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
    sf: f32,
    now: Instant,
) {
    render_click_markers(text, ui, input, dev, sf, now);
    render_transport_targets(text, snapshot, sim, ui, input, dev, sf, now);
    render_transport_impacts(text, ui, input, sf, now);
    render_death_nameplates(text, ui, input, dev, sf, now);
    render_attack_badges(text, snapshot, sim, ui, input, dev, sf, now);
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
            player
                .team
                .map_or(player.color, sow_core::player::team_territory_rgb)
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

        let pulse = (age * 2.7).sin() * 0.5 + 0.5;
        let rotation = (age * TRANSPORT_TARGET_ROTATIONS_PER_SEC).fract();
        let outer_radius = (24.0 + 3.0 * pulse) * sf;
        let inner_radius = (16.0 + 2.0 * pulse) * sf;
        let center_px = [center[0] * sf, center[1] * sf];
        text.push_ring(
            center_px,
            outer_radius,
            [player_rgb[0], player_rgb[1], player_rgb[2], alpha * 0.48],
            (2.5 * sf).max(1.0),
        );
        text.push_rotating_arc(
            center_px,
            inner_radius,
            0.76,
            rotation,
            [player_rgb[0], player_rgb[1], player_rgb[2], alpha],
            (3.5 * sf).max(1.0),
        );
        if marker.eta_seconds.is_some() {
            let font_size = TRANSPORT_TARGET_ETA_FONT_SIZE * sf;
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
                dev_text_style(dev, sf, [0.0, 0.0, 0.0, alpha]),
                (0.5, dev.font_char_spacing.max(0.1), INLINE_EMOJI_SCALE),
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
        let icon_size = font_size * 0.8;
        let margin = icon_size * 0.5 + 8.0;
        if center[0] < -margin
            || center[0] > screen_w + margin
            || center[1] < -margin
            || center[1] > screen_h + margin
        {
            return true;
        }

        let emoji_outline = dev_emoji_outline(dev, sf, [0.0, 0.0, 0.0, alpha]);
        text.push_emoji(
            death_emoji(animation.by_nuke),
            [center[0] * sf, center[1] * sf],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_eta_baseline_centers_short_and_wide_numbers() {
        let atlas = FontAtlas::load_static();
        let center_y = 100.0;
        let font_size = TRANSPORT_TARGET_ETA_FONT_SIZE;
        let base = atlas.atlas.common.base as f32;
        let font_scale = font_size / 48.0;

        for eta in ["1", "8888"] {
            let baseline = transport_eta_baseline_y(&atlas, eta, center_y, font_size);
            let (min_y, max_y) = eta.chars().filter_map(|ch| atlas.char_map.get(&ch)).fold(
                (f32::INFINITY, f32::NEG_INFINITY),
                |(min_y, max_y), glyph| {
                    (
                        min_y.min(glyph.yoffset as f32),
                        max_y.max(glyph.yoffset as f32 + glyph.height as f32),
                    )
                },
            );
            let visible_center_y =
                baseline - base * font_scale + (min_y + max_y) * 0.5 * font_scale;

            assert!((visible_center_y - center_y).abs() < 1e-5, "{eta}");
        }
    }

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
}
