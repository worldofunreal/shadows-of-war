use crate::ClientPhase;
use crate::app::SowApp;
use web_time::{Duration, Instant};

impl SowApp {
    pub(super) fn render_frame_ui_and_present(&mut self, sf: f32, frame: blade_graphics::Frame) {
        self.ui.app.splash_state.frames_drawn =
            self.ui.app.splash_state.frames_drawn.saturating_add(1);
        let now = Instant::now();
        let dt = now
            .duration_since(self.time.last_frame_time)
            .as_secs_f32()
            .min(0.1);
        self.time.last_frame_time = now;

        if self.ui.app.phase == ClientPhase::Playing && !self.input.input_focused {
            let dx = self.input.key_pan_left as i32 as f32 - self.input.key_pan_right as i32 as f32;
            let dy = self.input.key_pan_up as i32 as f32 - self.input.key_pan_down as i32 as f32;
            if dx != 0.0 || dy != 0.0 {
                let pan = 1000.0 * dt;
                self.input.camera_x += dx * pan;
                self.input.camera_y += dy * pan;
                self.clamp_camera_to_map();
            }
        }
        if self.input.dragging {
            self.input.camera_focus_target = None;
        }
        if let Some((world_x, world_y)) = self.input.camera_focus_target {
            let lerp = (1.0 - f32::exp(-12.0 * dt)).clamp(0.0, 1.0);
            self.input.camera_zoom += (self.input.target_zoom - self.input.camera_zoom) * lerp;
            let target_x = self.input.screen_w * 0.5 - world_x * self.input.camera_zoom;
            let target_y = self.input.screen_h * 0.5 - world_y * self.input.camera_zoom;
            self.input.camera_x += (target_x - self.input.camera_x) * lerp;
            self.input.camera_y += (target_y - self.input.camera_y) * lerp;
            self.clamp_camera_to_map();
            if (target_x - self.input.camera_x).abs() < 0.5
                && (target_y - self.input.camera_y).abs() < 0.5
            {
                self.input.camera_focus_target = None;
            }
        } else {
            let diff = self.input.target_zoom - self.input.camera_zoom;
            if diff.abs() > 0.0001 {
                let lerp = (1.0 - f32::exp(-12.0 * dt)).clamp(0.0, 1.0);
                let cx = self.input.last_mouse_x as f32;
                let cy = self.input.last_mouse_y as f32;
                let old_zoom = self.input.camera_zoom;
                self.input.camera_zoom += diff * lerp;
                let map_x = (cx - self.input.camera_x) / old_zoom;
                let map_y = (cy - self.input.camera_y) / old_zoom;
                self.input.camera_x = cx - map_x * self.input.camera_zoom;
                self.input.camera_y = cy - map_y * self.input.camera_zoom;
                self.clamp_camera_to_map();
            }
        }

        self.ui.app.main_menu_state.wait_timer_secs =
            (self.ui.app.main_menu_state.wait_timer_secs - dt).max(0.0);
        if let Some(timer) = &mut self.ui.app.hud_state.spawn_timer_secs {
            *timer = (*timer - dt).max(0.0);
        }
        if let Some(sync) = &mut self.ui.app.hud_state.sync_state {
            sync.time_remaining = (sync.time_remaining - dt).max(0.0);
        }
        if self.ui.app.phase == ClientPhase::Playing {
            self.sync_hud_player_state();
        }

        if let Some(text) = &mut self.gfx.text_renderer {
            text.begin_frame();
            for (key, cell) in &self.ui.app.asset_loader.gpu_avatar_cells {
                let leader = match key {
                    crate::ui::asset_loader::AvatarFetchKey::Leader(l) => Some(*l),
                    crate::ui::asset_loader::AvatarFetchKey::Fallback => None,
                };
                let slot = match leader {
                    Some(leader) => sow_core::player::Leader::ALL
                        .iter()
                        .position(|value| *value == leader)
                        .unwrap_or(0),
                    None => sow_core::player::Leader::ALL.len(),
                };
                if text.avatar_uv(slot).is_none() {
                    text.upload_avatar(slot, cell);
                }
            }
            if self.ui.app.phase == ClientPhase::Playing {
                crate::render::world::render_overlays(
                    text,
                    &self.sim,
                    &mut self.ui,
                    &self.input,
                    sf,
                    self.time.start_time.elapsed().as_secs_f32() % 1000.0,
                    now,
                );
            }
        }
        crate::web_menu::publish_state(self);

        let Some(mut render_ctx) = self.gfx.render_ctx.take() else {
            return;
        };
        if let Some(text) = &mut self.gfx.text_renderer {
            text.draw(
                &mut render_ctx.command_encoder,
                frame.texture_view(),
                [self.input.screen_w, self.input.screen_h],
                &render_ctx.context,
            );
        }
        #[cfg(target_arch = "wasm32")]
        if !self.web_loader_hidden && self.ui.app.phase != ClientPhase::Splash {
            crate::loader::hide_web_loader();
            self.web_loader_hidden = true;
        }
        render_ctx.command_encoder.present(frame);
        let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
        self.net.load_telemetry.mark_gpu_upload_complete();
        self.gfx.prev_sync_point = Some(sync_point);
        self.gfx.render_ctx = Some(render_ctx);

        self.time.frame_count = self.time.frame_count.saturating_add(1);
        let metrics_now = Instant::now();
        if metrics_now.duration_since(self.time.last_fps_time) >= Duration::from_secs(1) {
            self.time.current_fps = self.time.frame_count;
            self.time.frame_count = 0;
            self.time.last_fps_time = metrics_now;
        }
    }
}
