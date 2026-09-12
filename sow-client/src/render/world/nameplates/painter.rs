//! Single render component for nameplates.
//!
//! `render.rs` decides which semantic mode is visible; this module owns every nameplate draw
//! operation, including the low-LOD dot and the tutorial avatar exception.

use super::emoji::{SideBadgeOpts, draw_side_express_emoji, draw_side_status_badge};
use super::layout::{NameplateLayout, NameplateMetrics};

#[derive(Clone, Copy)]
pub(crate) struct NameplateStyle {
    pub gpu_text: crate::render::gpu::TextPaintStyle,
    pub gpu_emoji: crate::render::gpu::OutlineStyle,
    pub egui_emoji: sow_ui_kit::theme::TextGlowStyle,
    pub font_size_scale: f32,
    pub char_spacing: f32,
    pub emoji_size_scale: f32,
    scale_factor: f32,
}

impl NameplateStyle {
    pub fn from_dev(dev: &sow_ui_kit::theme::dev_config::DevConfig, sf: f32) -> Self {
        let outline_color = [0.0, 0.0, 0.0, 0.9];
        Self {
            gpu_text: crate::render::dev_text_style(dev, sf, outline_color),
            gpu_emoji: crate::render::dev_emoji_outline(dev, sf, outline_color),
            egui_emoji: sow_ui_kit::theme::NAMEPLATE,
            font_size_scale: dev.font_size_scale,
            char_spacing: dev.font_char_spacing,
            emoji_size_scale: dev.emoji_size_scale,
            scale_factor: sf,
        }
    }

    pub fn emoji_effect_padding(&self, logical_badge_size: f32) -> f32 {
        let gpu_padding = self.gpu_emoji.effect_padding() / self.scale_factor.max(0.001);
        let egui_padding = self
            .egui_emoji
            .outline_width(logical_badge_size)
            .max(self.egui_emoji.shadow_dy(logical_badge_size));
        gpu_padding.max(egui_padding)
    }

    #[inline]
    fn name_render_size(&self, metrics: NameplateMetrics) -> f32 {
        metrics.render_size * self.font_size_scale
    }

    #[inline]
    fn troops_render_size(&self, metrics: NameplateMetrics) -> f32 {
        metrics.troops_render_size * self.font_size_scale
    }

    #[inline]
    fn troops_icon_size(&self, metrics: NameplateMetrics) -> f32 {
        crate::hud::nameplate::troops_icon_size_from_text(self.troops_render_size(metrics))
    }
}

pub(crate) struct NameplateInput<'a> {
    pub center: egui::Pos2,
    pub metrics: NameplateMetrics,
    pub player_id: u16,
    pub player_name: &'a str,
    pub player_type: sow_core::player::PlayerType,
    pub player_color: [f32; 3],
    pub leader: &'a sow_core::player::Leader,
    pub active_emoji: Option<&'a str>,
    pub display_name: &'a str,
    pub troops: &'a str,
    pub vibrant_color: egui::Color32,
    pub is_me: bool,
    pub is_allied: bool,
    pub is_heart_flashing: bool,
    pub has_req: bool,
    pub betrayal_flash: bool,
    pub is_disconnected: bool,
    pub heart_flash_alpha: f32,
    pub rank_1based: Option<usize>,
    pub show_names: bool,
    pub show_troops: bool,
    pub asset_loader: &'a sow_ui::ui::asset_loader::AssetLoader,
}

pub(crate) struct NameplatePainter<'a> {
    painter: &'a egui::Painter,
    text_renderer: Option<&'a mut crate::render::gpu::TextRenderer>,
    sf: f32,
    style: NameplateStyle,
}

impl<'a> NameplatePainter<'a> {
    pub fn new(
        painter: &'a egui::Painter,
        text_renderer: Option<&'a mut crate::render::gpu::TextRenderer>,
        sf: f32,
        style: NameplateStyle,
    ) -> Self {
        Self {
            painter,
            text_renderer,
            sf,
            style,
        }
    }

    pub fn paint_lod_dot(&mut self, center: egui::Pos2, radius: f32, color: egui::Color32) {
        if let Some(tr) = self.text_renderer.as_deref_mut() {
            let center = [center.x * self.sf, center.y * self.sf];
            let radius = radius * self.sf;
            tr.push_disc(center, radius, color.to_array().map(|v| v as f32 / 255.0));
            tr.push_ring(
                center,
                radius,
                [0.0, 0.0, 0.0, 180.0 / 255.0],
                1.0 * self.sf,
            );
        } else {
            self.painter.circle_filled(center, radius, color);
            self.painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)),
            );
        }
    }

    pub fn paint_tutorial_avatar(
        &mut self,
        center: egui::Pos2,
        radius: f32,
        player_id: u16,
        player_name: &str,
        player_type: sow_core::player::PlayerType,
        player_color: [f32; 3],
        leader: sow_core::player::Leader,
        asset_loader: &sow_ui::ui::asset_loader::AssetLoader,
    ) {
        if let Some(tr) = self.text_renderer.as_deref_mut() {
            crate::hud::avatar::draw_player_avatar_gpu(
                tr,
                &crate::hud::avatar::GpuAvatarOpts {
                    center: [center.x * self.sf, center.y * self.sf],
                    radius: radius * self.sf,
                    player_id,
                    player_name,
                    player_type,
                    player_color,
                    leader,
                    emoji_outline: crate::render::gpu::OutlineStyle::NONE,
                },
            );
        } else {
            crate::hud::avatar::draw_player_avatar(
                self.painter,
                &crate::hud::avatar::AvatarRenderOpts {
                    center,
                    radius,
                    player_id,
                    player_name,
                    player_type,
                    player_color,
                    leader: &leader,
                    emoji_style: self.style.egui_emoji,
                },
                asset_loader,
            );
        }
    }

    pub fn paint(&mut self, input: NameplateInput<'_>) {
        let troops_font_id = egui::FontId::proportional(input.metrics.troops_font_size);
        let (prepared_name, troops_galley, name_size, troops_size) =
            if let Some(tr) = self.text_renderer.as_deref() {
                let sf = self.sf.max(0.001);
                let name_font_size = self.style.name_render_size(input.metrics) * sf;
                let name_size = if input.show_names {
                    let measure = tr.measure_string(
                        input.display_name,
                        name_font_size,
                        self.style.char_spacing,
                        self.style.emoji_size_scale,
                    );
                    egui::vec2(measure.width / sf, measure.height / sf)
                } else {
                    egui::Vec2::ZERO
                };

                let troops_size = if input.show_troops {
                    let icon_size = self.style.troops_icon_size(input.metrics);
                    let troops_font_size = self.style.troops_render_size(input.metrics) * sf;
                    let measure = tr.measure_string(
                        input.troops,
                        troops_font_size,
                        self.style.char_spacing,
                        self.style.emoji_size_scale,
                    );
                    egui::vec2(
                        icon_size + 3.0 + measure.width / sf,
                        icon_size.max(measure.height / sf),
                    )
                } else {
                    egui::Vec2::ZERO
                };

                (None, None, name_size, troops_size)
            } else {
                let font_id = egui::FontId::proportional(input.metrics.font_size);
                let prepared_name = input.show_names.then(|| {
                    sow_ui_kit::widgets::prepare_name(self.painter, input.display_name, &font_id)
                });
                let troops_galley = input.show_troops.then(|| {
                    self.painter.layout_no_wrap(
                        input.troops.to_owned(),
                        troops_font_id.clone(),
                        input.vibrant_color,
                    )
                });
                let text_scale = input.metrics.text_scale;
                let name_size = prepared_name
                    .as_ref()
                    .map(|prepared| prepared.size * text_scale)
                    .unwrap_or(egui::Vec2::ZERO);
                let troops_size = troops_galley
                    .as_ref()
                    .map(|galley| {
                        egui::vec2(
                            crate::hud::nameplate::troops_row_width(galley, &troops_font_id)
                                * text_scale,
                            galley.rect.height() * text_scale,
                        )
                    })
                    .unwrap_or(egui::Vec2::ZERO);
                (prepared_name, troops_galley, name_size, troops_size)
            };
        let layout = NameplateLayout::compute(
            input.center,
            input.metrics,
            name_size,
            troops_size,
            input.show_names,
            input.show_troops,
            self.style.emoji_effect_padding(input.metrics.badge_size),
        );

        self.paint_status_badges(&layout, &input);
        self.paint_rank(&layout, input.rank_1based);
        if input.is_me {
            self.paint_emoji(
                "⭐",
                egui::Rect::from_center_size(
                    layout.star_center,
                    egui::Vec2::splat(layout.badge_size),
                ),
                [1.0; 4],
                egui::Color32::WHITE,
                layout.badge_size * 0.7,
            );
        }

        if layout.avatar_radius > 0.0 {
            let center = [
                layout.avatar_center.x * self.sf,
                layout.avatar_center.y * self.sf,
            ];
            if let Some(tr) = self.text_renderer.as_deref_mut() {
                crate::hud::avatar::draw_player_avatar_gpu(
                    tr,
                    &crate::hud::avatar::GpuAvatarOpts {
                        center,
                        radius: layout.avatar_radius * self.sf,
                        player_id: input.player_id,
                        player_name: input.player_name,
                        player_type: input.player_type,
                        player_color: input.player_color,
                        leader: *input.leader,
                        emoji_outline: self.style.gpu_emoji,
                    },
                );
            } else {
                crate::hud::avatar::draw_player_avatar(
                    self.painter,
                    &crate::hud::avatar::AvatarRenderOpts {
                        center: layout.avatar_center,
                        radius: layout.avatar_radius,
                        player_id: input.player_id,
                        player_name: input.player_name,
                        player_type: input.player_type,
                        player_color: input.player_color,
                        leader: input.leader,
                        emoji_style: self.style.egui_emoji,
                    },
                    input.asset_loader,
                );
            }
        }

        if input.is_disconnected && layout.avatar_radius > 0.0 {
            let disc_size = (layout.avatar_radius * 2.0 * 0.4).round().max(3.0);
            let disc_rect = egui::Rect::from_center_size(
                layout.disconnect_center(),
                egui::Vec2::splat(disc_size),
            );
            self.paint_emoji("🔌", disc_rect, [1.0; 4], egui::Color32::WHITE, disc_size);
        }

        self.paint_text(
            &layout,
            &input,
            prepared_name.as_ref(),
            troops_galley.as_ref(),
            &troops_font_id,
        );
    }

    fn paint_status_badges(&mut self, layout: &NameplateLayout, input: &NameplateInput<'_>) {
        draw_side_status_badge(
            self.painter,
            self.text_renderer.as_deref_mut(),
            self.sf,
            &SideBadgeOpts {
                pos: layout.side_badge_center(true, 0, input.is_me),
                size: layout.badge_size,
                player_id: input.player_id,
                is_me: input.is_me,
                active: input.has_req,
                anim_id_str: "request_anim_progress",
                emoji: "📨",
                color_glow: Some(egui::Color32::from_rgb(34, 211, 238)),
                flash_alpha: 1.0,
            },
            self.style.gpu_emoji,
            self.style.egui_emoji,
        );

        let flash_alpha = if input.is_heart_flashing {
            input.heart_flash_alpha
        } else {
            1.0
        };
        let mut right_slots = 0;
        if input.is_allied {
            draw_side_status_badge(
                self.painter,
                self.text_renderer.as_deref_mut(),
                self.sf,
                &SideBadgeOpts {
                    pos: layout.side_badge_center(false, right_slots, input.is_me),
                    size: layout.badge_size,
                    player_id: input.player_id,
                    is_me: input.is_me,
                    active: true,
                    anim_id_str: "allied_anim_progress",
                    emoji: "🤝",
                    color_glow: Some(egui::Color32::from_rgb(255, 200, 60)),
                    flash_alpha,
                },
                self.style.gpu_emoji,
                self.style.egui_emoji,
            );
            right_slots += 1;
        }
        if input.betrayal_flash {
            draw_side_status_badge(
                self.painter,
                self.text_renderer.as_deref_mut(),
                self.sf,
                &SideBadgeOpts {
                    pos: layout.side_badge_center(false, right_slots, input.is_me),
                    size: layout.badge_size,
                    player_id: input.player_id,
                    is_me: input.is_me,
                    active: true,
                    anim_id_str: "betrayal_anim_progress",
                    emoji: "🗡️",
                    color_glow: Some(egui::Color32::from_rgb(220, 38, 38)),
                    flash_alpha: 1.0,
                },
                self.style.gpu_emoji,
                self.style.egui_emoji,
            );
            right_slots += 1;
        }
        draw_side_express_emoji(
            self.painter,
            self.text_renderer.as_deref_mut(),
            self.sf,
            layout.express_center(right_slots),
            layout.badge_size,
            input.player_id,
            input.active_emoji,
            self.style.gpu_emoji,
            self.style.egui_emoji,
        );
    }

    fn paint_rank(&mut self, layout: &NameplateLayout, rank_1based: Option<usize>) {
        let Some(rank_1based) = rank_1based.filter(|rank| *rank <= 3) else {
            return;
        };
        let (icon, tint_arr, tint_col) = match rank_1based {
            1 => (
                "👑",
                [250.0 / 255.0, 204.0 / 255.0, 21.0 / 255.0, 1.0],
                egui::Color32::from_rgb(250, 204, 21),
            ),
            2 => (
                "🥈",
                [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
                egui::Color32::from_rgb(203, 213, 225),
            ),
            _ => (
                "🥉",
                [217.0 / 255.0, 119.0 / 255.0, 6.0 / 255.0, 1.0],
                egui::Color32::from_rgb(217, 119, 6),
            ),
        };
        self.paint_emoji(
            icon,
            egui::Rect::from_center_size(layout.rank_center, egui::Vec2::splat(layout.badge_size)),
            tint_arr,
            tint_col,
            layout.badge_size * 0.7,
        );
    }

    fn paint_emoji(
        &mut self,
        emoji: &str,
        rect: egui::Rect,
        tint: [f32; 4],
        fallback_tint: egui::Color32,
        fallback_font_size: f32,
    ) {
        let gpu_painted = self.text_renderer.as_deref_mut().is_some_and(|tr| {
            tr.push_emoji(
                emoji,
                [rect.center().x * self.sf, rect.center().y * self.sf],
                rect.width() * 0.5 * self.sf,
                tint,
                self.style.gpu_emoji,
            )
        });
        if gpu_painted
            || sow_ui_kit::widgets::try_paint_emoji_with_style(
                self.painter,
                emoji,
                rect,
                fallback_tint,
                self.style.egui_emoji,
            )
        {
            return;
        }
        self.painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            emoji,
            egui::FontId::proportional(fallback_font_size),
            fallback_tint,
        ); // emoji-ok: atlas miss is the documented last-resort fallback
    }

    fn paint_text(
        &mut self,
        layout: &NameplateLayout,
        input: &NameplateInput<'_>,
        prepared_name: Option<&sow_ui_kit::widgets::PreparedName>,
        troops_galley: Option<&std::sync::Arc<egui::Galley>>,
        troops_font_id: &egui::FontId,
    ) {
        let color_arr = input.vibrant_color.to_array().map(|v| v as f32 / 255.0);
        if let Some(tr) = self.text_renderer.as_deref_mut() {
            if input.show_names {
                tr.push_string(
                    input.display_name,
                    [
                        input.center.x * self.sf,
                        (layout.text_top + layout.name_size.y * 0.85) * self.sf,
                    ],
                    self.style.name_render_size(input.metrics) * self.sf,
                    color_arr,
                    self.style.gpu_text,
                    (0.5, self.style.char_spacing, self.style.emoji_size_scale),
                );
            }
            if input.show_troops {
                let troops_row_y = if input.show_names {
                    layout.text_top + layout.name_size.y + layout.item_spacing_y
                } else {
                    layout.text_top
                };
                let icon_size = self.style.troops_icon_size(input.metrics);
                let troops_left_x = input.center.x - layout.troops_size.x / 2.0;
                tr.push_emoji(
                    "⚔",
                    [
                        (troops_left_x + icon_size * 0.5) * self.sf,
                        (troops_row_y + icon_size * 0.5) * self.sf,
                    ],
                    icon_size * 0.5 * self.sf,
                    color_arr,
                    self.style.gpu_emoji,
                );
                tr.push_string(
                    input.troops,
                    [
                        (troops_left_x + icon_size + 3.0) * self.sf,
                        (troops_row_y + layout.troops_size.y * 0.85) * self.sf,
                    ],
                    self.style.troops_render_size(input.metrics) * self.sf,
                    color_arr,
                    self.style.gpu_text,
                    (0.0, self.style.char_spacing, self.style.emoji_size_scale),
                );
            }
            return;
        }

        if let Some(prepared) = prepared_name
            && input.show_names
        {
            sow_ui_kit::widgets::paint_prepared_name_with_glow(
                self.painter,
                egui::pos2(input.center.x - layout.name_size.x / 2.0, layout.text_top),
                egui::Align2::LEFT_TOP,
                prepared,
                input.vibrant_color,
                self.style.egui_emoji,
                Some(layout.name_size.y),
            );
        }
        if let Some(galley) = troops_galley
            && input.show_troops
        {
            let troops_row_y = if input.show_names {
                layout.text_top + layout.name_size.y + layout.item_spacing_y
            } else {
                layout.text_top
            };
            crate::hud::nameplate::paint_glow_troops_row_with_style(
                self.painter,
                egui::pos2(input.center.x - layout.troops_size.x / 2.0, troops_row_y),
                galley.clone(),
                troops_font_id,
                input.vibrant_color,
                self.style.egui_emoji,
                Some(layout.name_size.y),
            );
        }
    }
}
