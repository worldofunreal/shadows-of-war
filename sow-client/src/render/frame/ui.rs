use crate::ClientPhase;
use crate::app::SowApp;
use crate::input::surface::clamp_camera_offset;
use web_time::{Duration, Instant};

fn focus_camera_step(
    camera: (f32, f32),
    camera_zoom: f32,
    target_zoom: f32,
    focus: (f32, f32),
    screen: (f32, f32),
    map: (u32, u32),
    amount: f32,
) -> ((f32, f32), f32, (f32, f32)) {
    let zoom = camera_zoom + (target_zoom - camera_zoom) * amount;
    let target = (
        clamp_camera_offset(
            screen.0,
            map.0 as f32 * zoom,
            screen.0 * 0.5 - focus.0 * zoom,
        ),
        clamp_camera_offset(
            screen.1,
            map.1 as f32 * zoom,
            screen.1 * 0.5 - focus.1 * zoom,
        ),
    );
    let current_center = (
        (screen.0 * 0.5 - camera.0) / camera_zoom,
        (screen.1 * 0.5 - camera.1) / camera_zoom,
    );
    let target_center = (
        (screen.0 * 0.5 - target.0) / zoom,
        (screen.1 * 0.5 - target.1) / zoom,
    );
    let center = (
        current_center.0 + (target_center.0 - current_center.0) * amount,
        current_center.1 + (target_center.1 - current_center.1) * amount,
    );
    (
        (
            screen.0 * 0.5 - center.0 * zoom,
            screen.1 * 0.5 - center.1 * zoom,
        ),
        zoom,
        target,
    )
}

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
        self.pump_hold_build(dt, now);

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
        let pointer_gesture_active = self.input.is_pointer_gesture_active();
        if pointer_gesture_active && !self.input.camera_focus_waiting_for_input_release {
            self.input.camera_focus_target = None;
            self.input.tutorial_camera_focus = false;
        }
        if !pointer_gesture_active && let Some((world_x, world_y)) = self.input.camera_focus_target
        {
            self.input.camera_focus_waiting_for_input_release = false;
            let rate = if self.input.tutorial_camera_focus {
                2.0
            } else {
                12.0
            };
            let lerp = (1.0 - f32::exp(-rate * dt)).clamp(0.0, 1.0);
            let (camera, zoom, target) = focus_camera_step(
                (self.input.camera_x, self.input.camera_y),
                self.input.camera_zoom,
                self.input.target_zoom,
                (world_x, world_y),
                (self.input.screen_w, self.input.screen_h),
                (self.sim.map_w, self.sim.map_h),
                lerp,
            );
            self.input.camera_x = camera.0;
            self.input.camera_y = camera.1;
            self.input.camera_zoom = zoom;
            self.clamp_camera_to_map();
            let target_x = target.0;
            let target_y = target.1;
            if (target_x - self.input.camera_x).abs() < 0.5
                && (target_y - self.input.camera_y).abs() < 0.5
                && (self.input.target_zoom - self.input.camera_zoom).abs() < 0.001
            {
                self.input.camera_zoom = self.input.target_zoom;
                self.input.camera_x = clamp_camera_offset(
                    self.input.screen_w,
                    self.sim.map_w as f32 * self.input.camera_zoom,
                    self.input.screen_w * 0.5 - world_x * self.input.camera_zoom,
                );
                self.input.camera_y = clamp_camera_offset(
                    self.input.screen_h,
                    self.sim.map_h as f32 * self.input.camera_zoom,
                    self.input.screen_h * 0.5 - world_y * self.input.camera_zoom,
                );
                self.clamp_camera_to_map();
                self.input.camera_focus_target = None;
                self.input.camera_focus_waiting_for_input_release = false;
                if self.input.tutorial_camera_focus {
                    self.input.tutorial_camera_focus = false;
                    self.input.has_snapped_camera_to_spawn = true;
                }
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
                let slot = match key {
                    crate::ui::asset_loader::AvatarFetchKey::Leader(leader) => {
                        sow_core::player::Leader::ALL
                            .iter()
                            .position(|value| value == leader)
                            .unwrap_or(0)
                    }
                    crate::ui::asset_loader::AvatarFetchKey::Fallback => {
                        sow_core::player::Leader::ALL.len()
                    }
                    crate::ui::asset_loader::AvatarFetchKey::Campaign { slot, .. } => *slot,
                };
                if text.avatar_uv(slot).is_none() {
                    text.upload_avatar(slot, cell);
                }
            }
            if self.ui.app.phase == ClientPhase::Playing {
                let campaign_avatar_slots =
                    std::mem::take(&mut self.ui.app.asset_loader.campaign_avatar_slots);
                crate::render::world::render_overlays(
                    text,
                    &self.sim,
                    &mut self.ui,
                    &self.input,
                    self.gfx.map_renderer.as_ref(),
                    &campaign_avatar_slots,
                    sf,
                    self.time.start_time.elapsed().as_secs_f32() % 1000.0,
                    now,
                );
                self.ui.app.asset_loader.campaign_avatar_slots = campaign_avatar_slots;
            }
        }
        crate::web_menu::publish_tutorial_camera_anchor_frame(self);
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

#[cfg(test)]
mod tests {
    use super::focus_camera_step;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn tutorial_camera_focus_stays_straight_when_zoom_changes() {
        for screen in [(1440.0, 900.0), (390.0, 844.0)] {
            let focus = (715.0, 200.0);
            let mut zoom = 10.0;
            let mut center = (700.0, 80.0);
            let mut camera = (
                screen.0 * 0.5 - center.0 * zoom,
                screen.1 * 0.5 - center.1 * zoom,
            );
            for frame in 0..240 {
                let target_zoom = if frame < 30 { 16.0 } else { 8.0 };
                let amount = 1.0 - f32::exp(-2.0 / 60.0);
                let (next_camera, next_zoom, _) = focus_camera_step(
                    camera,
                    zoom,
                    target_zoom,
                    focus,
                    screen,
                    (2000, 1000),
                    amount,
                );
                let next_center = (
                    (screen.0 * 0.5 - next_camera.0) / next_zoom,
                    (screen.1 * 0.5 - next_camera.1) / next_zoom,
                );
                assert!(next_center.0 >= center.0, "camera reversed on the x axis");
                assert!(next_center.1 >= center.1, "camera reversed on the y axis");
                center = next_center;
                camera = next_camera;
                zoom = next_zoom;
            }
            assert!((center.0 - focus.0).abs() * zoom < 0.5);
            assert!((center.1 - focus.1).abs() * zoom < 0.5);
            assert!((zoom - 8.0).abs() < 0.01);
        }
    }

    #[wasm_bindgen_test]
    fn tutorial_camera_focus_moves_without_changing_zoom() {
        let screen = (1440.0, 900.0);
        let map = (2000, 1000);
        let zoom = 10.0;
        let center = (700.0, 80.0);
        let focus = (715.0, 200.0);
        let camera = (
            screen.0 * 0.5 - center.0 * zoom,
            screen.1 * 0.5 - center.1 * zoom,
        );
        let (next_camera, next_zoom, _) =
            focus_camera_step(camera, zoom, zoom, focus, screen, map, 0.5);

        assert_eq!(next_zoom, zoom);
        assert_ne!(next_camera, camera);
        let next_center = (
            (screen.0 * 0.5 - next_camera.0) / next_zoom,
            (screen.1 * 0.5 - next_camera.1) / next_zoom,
        );
        assert!(next_center.0 > center.0 && next_center.0 < focus.0);
        assert!(next_center.1 > center.1 && next_center.1 < focus.1);
    }
}
