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

type CameraKey = ((f32, f32), f32);

/// Camera center and zoom at `progress` (0..1) through a shot. One ease in and out spans every
/// waypoint at an even pace, so the move never stops between legs.
fn camera_shot_frame(keys: &[CameraKey], progress: f32, screen: (f32, f32)) -> CameraKey {
    let view = screen.0.min(screen.1).max(1.0);
    // Leg length in screens panned plus zoom doublings, so pans and zooms share one pace.
    let legs: Vec<f32> = keys
        .windows(2)
        .map(|leg| {
            let ((a, za), (b, zb)) = (leg[0], leg[1]);
            (b.0 - a.0).hypot(b.1 - a.1) * za.min(zb) / view + (zb / za).ln().abs()
        })
        .collect();
    let total: f32 = legs.iter().sum();
    let t = progress.clamp(0.0, 1.0);
    let mut distance = t * t * t * (t * (t * 6.0 - 15.0) + 10.0) * total;
    for (leg, length) in keys.windows(2).zip(&legs) {
        if distance <= *length && *length > 0.0 {
            return camera_shot_leg(leg[0], leg[1], distance / length);
        }
        distance -= length;
    }
    keys[keys.len() - 1]
}

/// One leg of a shot. Zooming legs scale around their fixed point, so the frame grows or
/// shrinks as one piece instead of swinging the subject off screen mid-move.
fn camera_shot_leg((a, za): CameraKey, (b, zb): CameraKey, u: f32) -> CameraKey {
    let zoom = za * (zb / za).powf(u);
    if (zb / za).ln().abs() < 0.05 {
        return ((a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u), zoom);
    }
    let anchor = (
        (b.0 * zb - a.0 * za) / (zb - za),
        (b.1 * zb - a.1 * za) / (zb - za),
    );
    let scale = za / zoom;
    (
        (
            anchor.0 - (anchor.0 - a.0) * scale,
            anchor.1 - (anchor.1 - a.1) * scale,
        ),
        zoom,
    )
}

/// Player camera input ends a narration shot. Machinima moves are code-directed and never end that way.
fn machinima_move_taken_over(machinima: bool, player_moved_camera: bool) -> bool {
    !machinima && player_moved_camera
}

impl SowApp {
    /// Film `keys` after the current view over `seconds`; any player camera input ends the shot.
    pub(crate) fn start_camera_shot(&mut self, keys: Vec<CameraKey>, seconds: f32, machinima: bool) {
        let zoom = self.input.camera_zoom;
        let current = (
            (
                (self.input.screen_w * 0.5 - self.input.camera_x) / zoom,
                (self.input.screen_h * 0.5 - self.input.camera_y) / zoom,
            ),
            zoom,
        );
        self.input.has_snapped_camera_to_spawn = true;
        self.input.camera_focus_target = None;
        self.input.camera_focus_waiting_for_input_release = false;
        self.input.tutorial_camera_focus = false;
        self.input.target_zoom = zoom;
        self.input.camera_shot = Some(crate::app::CameraShot {
            keys: std::iter::once(current).chain(keys).collect(),
            seconds,
            elapsed: 0.0,
            left_at: (self.input.camera_x, self.input.camera_y, zoom),
            machinima,
        });
    }

    /// True while a code-directed camera move owns the view and player camera input is ignored.
    pub(crate) fn machinima_camera_active(&self) -> bool {
        self.input.camera_shot.as_ref().is_some_and(|shot| shot.machinima)
    }

    pub(super) fn advance_camera(&mut self, dt: f32) {
        if self.ui.app.phase == ClientPhase::Playing && !self.input.input_focused {
            let dx = self.input.key_pan_left as i32 as f32 - self.input.key_pan_right as i32 as f32;
            let dy = self.input.key_pan_up as i32 as f32 - self.input.key_pan_down as i32 as f32;
            if (dx != 0.0 || dy != 0.0) && !self.machinima_camera_active() {
                let pan = 1000.0 * dt;
                self.input.camera_x += dx * pan;
                self.input.camera_y += dy * pan;
                self.clamp_camera_to_map();
            }
        }
        if let Some(mut shot) = self.input.camera_shot.take() {
            let (x, y, zoom) = shot.left_at;
            let player_moved_camera = self.input.camera_focus_target.is_some()
                || self.input.is_pointer_gesture_active()
                || self.input.camera_x != x
                || self.input.camera_y != y
                || self.input.camera_zoom != zoom
                || self.input.target_zoom != zoom;
            let taken_over = machinima_move_taken_over(shot.machinima, player_moved_camera);
            if !taken_over {
                shot.elapsed += dt;
                let progress = shot.elapsed / shot.seconds;
                let screen = (self.input.screen_w, self.input.screen_h);
                let (center, zoom) = camera_shot_frame(&shot.keys, progress, screen);
                self.input.camera_zoom = zoom;
                self.input.target_zoom = zoom;
                self.input.camera_x = screen.0 * 0.5 - center.0 * zoom;
                self.input.camera_y = screen.1 * 0.5 - center.1 * zoom;
                self.clamp_camera_to_map();
                if progress < 1.0 {
                    shot.left_at = (
                        self.input.camera_x,
                        self.input.camera_y,
                        self.input.camera_zoom,
                    );
                    self.input.camera_shot = Some(shot);
                }
                return;
            }
        }
        let pointer_gesture_active = self.input.is_pointer_gesture_active();
        let is_building_drag = self.input.building_drag_offset.is_some();
        if pointer_gesture_active
            && !self.input.camera_focus_waiting_for_input_release
            && !is_building_drag
        {
            self.input.camera_focus_target = None;
            self.input.tutorial_camera_focus = false;
        }
        if (!pointer_gesture_active || is_building_drag)
            && let Some((world_x, world_y)) = self.input.camera_focus_target
        {
            self.input.camera_focus_waiting_for_input_release = false;
            let rate = if self.input.tutorial_camera_focus {
                2.0
            } else if self.ui.app.hud_state.building_placement.is_some() {
                5.5
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
                let (camera, zoom, _) = focus_camera_step(
                    (self.input.camera_x, self.input.camera_y),
                    self.input.camera_zoom,
                    self.input.target_zoom,
                    (world_x, world_y),
                    (self.input.screen_w, self.input.screen_h),
                    (self.sim.map_w, self.sim.map_h),
                    1.0,
                );
                self.input.camera_zoom = zoom;
                self.input.camera_x = camera.0;
                self.input.camera_y = camera.1;
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
    }

    pub(super) fn render_frame_ui_and_present(
        &mut self,
        sf: f32,
        frame: blade_graphics::Frame,
        dt: f32,
    ) {
        self.ui.app.splash_state.frames_drawn =
            self.ui.app.splash_state.frames_drawn.saturating_add(1);
        let now = Instant::now();
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

        let placement_can_confirm = self.building_placement_can_confirm();
        if let Some(text) = &mut self.gfx.text_renderer {
            text.begin_frame();
            let dirty_avatar_slots = std::mem::take(&mut self.ui.app.asset_loader.gpu_avatar_dirty);
            for &(slot, ref cell) in &self.ui.app.asset_loader.gpu_avatar_cells {
                if text.avatar_uv(slot).is_none() || dirty_avatar_slots.contains(&slot) {
                    text.upload_avatar(slot, cell);
                }
            }
            if self.ui.app.phase == ClientPhase::Playing {
                let campaign_avatar_slots =
                    std::mem::take(&mut self.ui.app.asset_loader.campaign_avatar_slots);
                let overlays_started_at = crate::diag::startup_perf_timer();
                crate::render::world::render_overlays(
                    text,
                    &self.sim,
                    &mut self.ui,
                    &self.input,
                    &campaign_avatar_slots,
                    sf,
                    now,
                    placement_can_confirm,
                );
                if let Some(started_at) = overlays_started_at {
                    crate::diag::record_startup_perf(
                        crate::diag::StartupMetric::WorldOverlays,
                        started_at,
                    );
                }
                self.ui.app.asset_loader.campaign_avatar_slots = campaign_avatar_slots;
            }
        }
        let menu_publish_started_at = crate::diag::startup_perf_timer();
        crate::web_menu::publish_tutorial_camera_anchor_frame(self);
        if self.ui.app.phase == ClientPhase::Playing {
            crate::web_menu::publish_state(self);
            crate::web_menu::publish_hover_card(self);
            crate::web_menu::publish_tutorial_pointer(self);
        }
        if let Some(started_at) = menu_publish_started_at {
            crate::diag::record_startup_perf(
                crate::diag::StartupMetric::MenuPublish,
                started_at,
            );
        }

        let Some(mut render_ctx) = self.gfx.render_ctx.take() else {
            return;
        };
        if let Some(text) = &mut self.gfx.text_renderer {
            let text_draw_started_at = crate::diag::startup_perf_timer();
            text.draw(
                &mut render_ctx.command_encoder,
                frame.texture_view(),
                [self.input.screen_w, self.input.screen_h],
                &crate::theme::dev_config::DevConfig::get().shader_params,
                &render_ctx.context,
            );
            if let Some(started_at) = text_draw_started_at {
                crate::diag::record_startup_perf(
                    crate::diag::StartupMetric::TextDraw,
                    started_at,
                );
            }
        }
        render_ctx.command_encoder.present(frame);
        let submit_started_at = crate::diag::startup_perf_timer();
        let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
        if let Some(started_at) = submit_started_at {
            crate::diag::record_startup_perf(crate::diag::StartupMetric::GpuSubmit, started_at);
        }
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
    use super::{machinima_move_taken_over, camera_shot_frame, focus_camera_step};
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn machinima_moves_ignore_player_camera_input_until_they_land() {
        assert!(machinima_move_taken_over(false, true), "narration shots still hand control back to the player");
        assert!(!machinima_move_taken_over(false, false));
        assert!(!machinima_move_taken_over(true, true), "machinima ignores drag, zoom and key pan");
        assert!(!machinima_move_taken_over(true, false));
    }

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

    #[wasm_bindgen_test]
    fn camera_shot_keeps_its_subject_on_screen_and_lands_on_each_end() {
        let screen = (1440.0, 900.0);
        let current = ((700.0, 80.0), 40.0);
        let wide = ((760.0, 140.0), 5.0);
        let close = ((820.0, 200.0), 100.0);
        let keys = [current, wide, close];
        let near = |(a, za): ((f32, f32), f32), (b, zb): ((f32, f32), f32)| {
            (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01 && (za - zb).abs() < 0.01
        };
        assert!(near(camera_shot_frame(&keys, 0.0, screen), current));
        assert!(near(camera_shot_frame(&keys, 1.0, screen), close));
        let mut last_zoom = f32::INFINITY;
        let mut pushing_in = false;
        for step in 1..200 {
            let ((cx, cy), zoom) = camera_shot_frame(&keys, step as f32 / 200.0, screen);
            pushing_in |= zoom > last_zoom;
            if pushing_in {
                // After the wide frame the zoom only grows and the subject never leaves the screen.
                assert!(zoom >= last_zoom, "push-in reversed");
                assert!(((close.0.0 - cx) * zoom).abs() <= screen.0 * 0.5);
                assert!(((close.0.1 - cy) * zoom).abs() <= screen.1 * 0.5);
            }
            last_zoom = zoom;
        }
        assert!(pushing_in, "the shot never pushed in from its wide frame");
    }
}
