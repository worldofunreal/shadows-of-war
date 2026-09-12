use crate::config::ClientVisualConfig;

/// Scaling factors relative to base territory font render size.
const HUMAN_AVATAR_SCALE: f32 = 3.6;
const BOT_AVATAR_SCALE: f32 = 2.2;
const NATION_AVATAR_SCALE: f32 = 2.2;
const BADGE_SCALE: f32 = 1.8;
const TROOPS_SCALE: f32 = 1.30;
const AVATAR_TEXT_GAP_SCALE: f32 = 0.16;

/// Computed dimensions for a single nameplate instance (Poka-Yoke architecture).
/// Decouples font size calculations from font layout and avatar/badge geometry so future
/// modifiers cannot confuse text bounds with avatar diameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NameplateMetrics {
    /// Continuous render size derived from territory and zoom.
    pub render_size: f32,
    /// Quantized font size for egui glyph layout.
    pub font_size: f32,
    /// Scale factor reconciling continuous GPU rendering with quantized egui font layout.
    pub text_scale: f32,
    /// On-screen diameter of the avatar circle in screen px (0.0 if disabled/hidden).
    pub avatar_diameter: f32,
    /// On-screen radius of the avatar circle (avatar_diameter / 2.0).
    pub avatar_radius: f32,
    /// Size of flanking status badges (📨, 🤝, 🗡️).
    pub badge_size: f32,
    /// Font size for troops text.
    pub troops_font_size: f32,
    /// Continuous render size for troops text.
    pub troops_render_size: f32,
}

impl NameplateMetrics {
    #[inline]
    pub fn compute(
        scaled_size: f32,
        player_type: sow_core::player::PlayerType,
        show_bot_avatars: bool,
    ) -> Self {
        let render_size = scaled_size.max(7.0);
        let font_size = if render_size > 20.0 {
            (render_size / 2.0).round() * 2.0
        } else {
            render_size.round()
        }
        .max(7.0);
        let text_scale = render_size / font_size;

        let avatar_scale = match player_type {
            sow_core::player::PlayerType::Human => HUMAN_AVATAR_SCALE,
            sow_core::player::PlayerType::Bot => {
                if show_bot_avatars {
                    BOT_AVATAR_SCALE
                } else {
                    0.0
                }
            }
            sow_core::player::PlayerType::Nation => NATION_AVATAR_SCALE,
        };

        let avatar_diameter = if avatar_scale > 0.0 {
            (render_size * avatar_scale).max(4.0)
        } else {
            0.0
        };

        let badge_size = render_size * BADGE_SCALE;
        let troops_render_size = render_size * TROOPS_SCALE;
        let troops_font_size = (font_size * TROOPS_SCALE).round().max(2.0);

        Self {
            render_size,
            font_size,
            text_scale,
            avatar_diameter,
            avatar_radius: avatar_diameter / 2.0,
            badge_size,
            troops_font_size,
            troops_render_size,
        }
    }
}

/// World-anchored nameplate font size in screen px. `nameplate_size * zoom_scaled` is the
/// territory's on-screen side length, so the plate shrinks as you zoom out and grows with
/// territory — it always occupies the same fraction of the land it labels. Humans keep a
/// small readability floor; bots/nations have none, so far-away ones fall through to the
/// <7px dot LOD. Caps only catch extremes (huge empire fully zoomed in), not normal play.
pub(crate) fn nameplate_font_px(
    nameplate_size: f32,
    zoom_scaled: f32,
    is_human: bool,
    cfg: &ClientVisualConfig,
) -> f32 {
    let world_px = nameplate_size * cfg.nameplate_world_scale * zoom_scaled;
    if is_human {
        world_px.clamp(8.0, cfg.nameplate_max_font)
    } else {
        world_px.min(cfg.nameplate_max_font)
    }
}

pub(crate) const BADGE_GAP: f32 = 3.0;
const STACK_GAP: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NameplateLayout {
    pub avatar_center: egui::Pos2,
    pub avatar_radius: f32,
    pub avatar_visual_radius: f32,
    pub badge_size: f32,
    pub left_x: f32,
    pub right_x: f32,
    pub rank_center: egui::Pos2,
    pub star_center: egui::Pos2,
    pub text_top: f32,
    pub item_spacing_y: f32,
    pub name_size: egui::Vec2,
    pub troops_size: egui::Vec2,
    stack_step: f32,
}

impl NameplateLayout {
    pub fn compute(
        center: egui::Pos2,
        metrics: NameplateMetrics,
        name_size: egui::Vec2,
        troops_size: egui::Vec2,
        show_names: bool,
        show_troops: bool,
        badge_effect_padding: f32,
    ) -> Self {
        let name_size = if show_names {
            name_size
        } else {
            egui::Vec2::ZERO
        };
        let troops_size = if show_troops {
            troops_size
        } else {
            egui::Vec2::ZERO
        };
        let item_spacing_y = if show_names && show_troops {
            metrics.render_size * 0.111
        } else {
            0.0
        };
        let right_h = name_size.y + item_spacing_y + troops_size.y;
        let spacing_y = if metrics.avatar_diameter > 0.0 && right_h > 0.0 {
            metrics.render_size * AVATAR_TEXT_GAP_SCALE
        } else {
            0.0
        };
        let total_h = metrics.avatar_diameter + spacing_y + right_h;
        let content_top = center.y - total_h / 2.0;
        let avatar_center = egui::pos2(center.x, content_top + metrics.avatar_diameter / 2.0);
        let avatar_visual_radius = avatar_visual_radius(metrics.avatar_radius);
        let badge_clearance = BADGE_GAP + badge_effect_padding.max(0.0);
        let badge_half = metrics.badge_size / 2.0;
        let stack_step = metrics.badge_size + STACK_GAP + badge_effect_padding.max(0.0) * 2.0;
        let left_x = center.x - avatar_visual_radius - badge_half - badge_clearance;
        let right_x = center.x + avatar_visual_radius + badge_half + badge_clearance;

        Self {
            avatar_center,
            avatar_radius: metrics.avatar_radius,
            avatar_visual_radius,
            badge_size: metrics.badge_size,
            left_x,
            right_x,
            rank_center: egui::pos2(
                center.x,
                avatar_center.y - avatar_visual_radius - badge_half - badge_clearance,
            ),
            star_center: egui::pos2(left_x, avatar_center.y),
            text_top: content_top + metrics.avatar_diameter + spacing_y,
            item_spacing_y,
            name_size,
            troops_size,
            stack_step,
        }
    }

    pub fn side_badge_center(&self, left: bool, stack_slot: usize, is_me: bool) -> egui::Pos2 {
        let x = if left { self.left_x } else { self.right_x };
        let y = if left && is_me {
            self.avatar_center.y + self.stack_step
        } else {
            self.avatar_center.y - stack_slot as f32 * self.stack_step
        };
        egui::pos2(x, y)
    }

    pub fn express_center(&self, right_stack_slots: usize) -> egui::Pos2 {
        egui::pos2(
            self.right_x + 2.0,
            self.side_badge_center(false, right_stack_slots, false).y - self.badge_size / 2.0,
        )
    }

    pub fn disconnect_center(&self) -> egui::Pos2 {
        egui::pos2(
            self.avatar_center.x + self.avatar_radius * 0.6,
            self.avatar_center.y + self.avatar_radius * 0.6,
        )
    }
}

fn avatar_visual_radius(radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let border = (radius * 0.12).max(1.0);
    // The outer ring is centered at `radius + border * 0.3` and its stroke extends half its
    // width outward. Keep badge clearance measured against that actual visual bound.
    radius + border * 0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    // Guards the two bugs previous fixes shipped: sizes pinned at a clamp across the whole
    // playable zoom range (constant on the viewport), and territory differences erased.
    // Playable zoom_scaled range: camera min 0.75 to max 100; plates are drawn at every zoom.
    #[test]
    fn nameplate_size_tracks_zoom_and_territory() {
        let cfg = ClientVisualConfig::default();
        let mid_territory = 2000.0_f32.sqrt(); // ~45 world units side

        // Zooming out must shrink the plate through the responsive band (above the readability
        // floor, below the cap), not sit pinned on a clamp.
        let far = nameplate_font_px(mid_territory, 8.0, true, &cfg);
        let mid = nameplate_font_px(mid_territory, 10.0, true, &cfg);
        let near = nameplate_font_px(mid_territory, 12.0, true, &cfg);
        assert!(
            far < mid && mid < near,
            "not zoom-responsive: {far} {mid} {near}"
        );

        // Bigger territory must render bigger at the same zoom (both inside the band).
        let small = nameplate_font_px(400.0_f32.sqrt(), 10.0, true, &cfg);
        let large = nameplate_font_px(2500.0_f32.sqrt(), 10.0, true, &cfg);
        assert!(small < large, "not territory-responsive: {small} {large}");

        // Caps and floors only catch extremes.
        assert!(nameplate_font_px(150.0, 100.0, true, &cfg) <= cfg.nameplate_max_font);
        assert!(nameplate_font_px(0.2, 0.75, true, &cfg) >= 8.0);

        // Hiding is gated on camera zoom, not per-plate pixel size: a non-human plate is only
        // dotted below nameplate_hide_zoom. Just ABOVE that threshold it must still render (the
        // render path floors it to a readable size), so nothing pops while zoomed in to play.
        assert!(
            cfg.nameplate_hide_zoom <= 1.5,
            "hide threshold crept too close/in: {}",
            cfg.nameplate_hide_zoom
        );
        let just_inside = cfg.nameplate_hide_zoom + 0.5;
        let tiny_bot_font = nameplate_font_px(10.0, just_inside, false, &cfg).max(7.0); // render_size floor
        assert!(
            tiny_bot_font >= 7.0,
            "should stay readable, not hide: {tiny_bot_font}"
        );
    }

    #[test]
    fn pokayoke_human_avatar_is_significantly_larger_than_bots() {
        let scaled_size = 14.0;
        let human_metrics =
            NameplateMetrics::compute(scaled_size, sow_core::player::PlayerType::Human, true);
        let bot_metrics =
            NameplateMetrics::compute(scaled_size, sow_core::player::PlayerType::Bot, true);
        let nation_metrics =
            NameplateMetrics::compute(scaled_size, sow_core::player::PlayerType::Nation, true);

        // Human avatar must be >= 1.5x larger than bot and nation icons
        assert!(
            human_metrics.avatar_diameter >= bot_metrics.avatar_diameter * 1.5,
            "Human avatar ({}) must be significantly larger than bot avatar ({})",
            human_metrics.avatar_diameter,
            bot_metrics.avatar_diameter
        );
        assert_eq!(bot_metrics.avatar_diameter, nation_metrics.avatar_diameter);
        assert!((human_metrics.avatar_diameter / scaled_size - HUMAN_AVATAR_SCALE).abs() < 1e-5);
        assert!((bot_metrics.avatar_diameter / scaled_size - BOT_AVATAR_SCALE).abs() < 1e-5);

        // Bot avatars can be disabled via dev settings, human avatars stay visible
        let bot_hidden_metrics =
            NameplateMetrics::compute(scaled_size, sow_core::player::PlayerType::Bot, false);
        assert_eq!(bot_hidden_metrics.avatar_diameter, 0.0);

        let human_unaffected_metrics =
            NameplateMetrics::compute(scaled_size, sow_core::player::PlayerType::Human, false);
        assert!(human_unaffected_metrics.avatar_diameter > 0.0);
    }

    fn metrics(size: f32) -> NameplateMetrics {
        NameplateMetrics::compute(size, sow_core::player::PlayerType::Human, true)
    }

    #[test]
    fn badges_keep_a_real_gap_from_avatar() {
        let layout = NameplateLayout::compute(
            egui::pos2(100.0, 100.0),
            metrics(14.0),
            egui::vec2(60.0, 14.0),
            egui::vec2(50.0, 12.0),
            true,
            true,
            2.0,
        );
        let badge_half = layout.badge_size / 2.0;
        let avatar_left = layout.avatar_center.x - layout.avatar_visual_radius;
        let avatar_top = layout.avatar_center.y - layout.avatar_visual_radius;

        assert!(
            layout.star_center.x + badge_half + 2.0 <= avatar_left - BADGE_GAP,
            "star effect may touch avatar"
        );
        assert!(
            layout.rank_center.y + badge_half + 2.0 <= avatar_top - BADGE_GAP,
            "rank effect may touch avatar"
        );
        let right_first = layout.side_badge_center(false, 0, false);
        let right_second = layout.side_badge_center(false, 1, false);
        assert!(
            (right_first.y - right_second.y) - layout.badge_size - 4.0 >= STACK_GAP - 1e-5,
            "stacked badge effects may touch each other"
        );
        let border = (layout.avatar_radius * 0.12).max(1.0);
        assert_eq!(
            layout.avatar_visual_radius,
            layout.avatar_radius + border * 0.8,
            "clearance must include the full outer avatar stroke"
        );
    }

    #[test]
    fn avatar_text_gap_uses_the_shared_compact_scale() {
        let metrics = metrics(14.0);
        let layout = NameplateLayout::compute(
            egui::pos2(100.0, 100.0),
            metrics,
            egui::vec2(60.0, 14.0),
            egui::vec2(50.0, 12.0),
            true,
            true,
            0.0,
        );
        let gap = layout.text_top - (layout.avatar_center.y + layout.avatar_radius);
        assert!((gap - metrics.render_size * AVATAR_TEXT_GAP_SCALE).abs() < 1e-5);
    }

    #[test]
    fn crown_and_star_clearance_holds_across_sizes_and_effect_padding() {
        for (size, effect_padding) in [(7.0, 0.4), (11.0, 0.8), (18.0, 1.5), (32.0, 2.5)] {
            let layout = NameplateLayout::compute(
                egui::pos2(320.0, 240.0),
                metrics(size),
                egui::vec2(size * 4.0, size),
                egui::vec2(size * 3.0, size),
                true,
                true,
                effect_padding,
            );
            let badge_half = layout.badge_size / 2.0;
            let avatar_left = layout.avatar_center.x - layout.avatar_visual_radius;
            let avatar_top = layout.avatar_center.y - layout.avatar_visual_radius;
            let required_gap = BADGE_GAP + effect_padding;

            assert!(
                avatar_left - (layout.star_center.x + badge_half) >= required_gap - 1e-4,
                "star intersects avatar at size {size} / padding {effect_padding}"
            );
            assert!(
                avatar_top - (layout.rank_center.y + badge_half) >= required_gap - 1e-4,
                "crown intersects avatar at size {size} / padding {effect_padding}"
            );
        }
    }

    #[test]
    fn layout_scales_without_changing_the_clearance_rule() {
        let small = NameplateLayout::compute(
            egui::pos2(0.0, 0.0),
            metrics(8.0),
            egui::vec2(20.0, 8.0),
            egui::vec2(18.0, 7.0),
            true,
            true,
            1.5,
        );
        let large = NameplateLayout::compute(
            egui::pos2(0.0, 0.0),
            metrics(24.0),
            egui::vec2(80.0, 24.0),
            egui::vec2(70.0, 20.0),
            true,
            true,
            1.5,
        );

        assert!(small.badge_size < large.badge_size);
        assert!(
            (small.left_x
                - (small.avatar_center.x
                    - small.avatar_visual_radius
                    - small.badge_size / 2.0
                    - BADGE_GAP
                    - 1.5))
                .abs()
                < f32::EPSILON
        );
        assert!(
            (large.left_x
                - (large.avatar_center.x
                    - large.avatar_visual_radius
                    - large.badge_size / 2.0
                    - BADGE_GAP
                    - 1.5))
                .abs()
                < f32::EPSILON
        );
    }
}
