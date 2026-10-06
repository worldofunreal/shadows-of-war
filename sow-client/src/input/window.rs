use crate::app::{HoverPointer, MapPointerStart, SowApp};
use crate::input::map_click::{TOUCH_HOLD_MS, is_quick_tap};
use crate::{ClientPhase, camera_zoom_upper_bound};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, PointerKind, WindowEvent};

const HOLD_BUILD_INTERVAL_SECS: f32 = 0.25;
const HOLD_BUILD_BURST_INTERVAL_SECS: f32 = 0.1;
pub(crate) const HOLD_BUILD_BURST_AFTER_SECS: f32 = 2.0;

pub(crate) fn hold_build_repeat_interval(held_secs: f32) -> f32 {
    if held_secs >= HOLD_BUILD_BURST_AFTER_SECS {
        HOLD_BUILD_BURST_INTERVAL_SECS
    } else {
        HOLD_BUILD_INTERVAL_SECS
    }
}

fn advance_hold_build_timer(remaining: &mut f32, dt: f32, held_secs: f32) -> bool {
    *remaining -= dt.max(0.0);
    if *remaining > 0.0 {
        return false;
    }
    *remaining = hold_build_repeat_interval(held_secs);
    true
}

impl SowApp {
    pub fn handle_window_event(
        &mut self,
        event_loop: &dyn winit::event_loop::ActiveEventLoop,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                    if let Some(sp) = self.gfx.prev_sync_point.take() {
                        let _ = render_ctx.context.wait_for(&sp, !0);
                    }
                    if let Some(mut surface) = self.gfx.surface.take() {
                        render_ctx.reset_command_encoder();
                        render_ctx.context.destroy_surface(&mut surface);
                    }
                }
                event_loop.exit();
            }
            WindowEvent::SurfaceResized(size) => self.apply_surface_resize(size, false),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = self.gfx.window.as_ref() {
                    let viewport = crate::viewport::Viewport::measure(window.as_ref());
                    if viewport.wants_reconfigure(self) {
                        self.apply_surface_resize(viewport.physical, false);
                    } else {
                        viewport.sync_to_app(self);
                    }
                }
            }
            WindowEvent::Focused(false) => {
                self.input.shift_pressed = false;
                self.cancel_pointer_gesture();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.input.shift_pressed = modifiers.state().shift_key();
            }
            WindowEvent::PointerEntered {
                position,
                primary,
                kind,
                ..
            } => self.handle_pointer_enter(kind, position.x, position.y, primary),
            WindowEvent::PointerLeft { kind, .. } => self.handle_pointer_left(kind),
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_event(event.state == ElementState::Pressed, event.physical_key)
            }
            WindowEvent::PointerButton {
                state,
                button,
                position,
                primary,
                ..
            } => self.handle_pointer_button(
                state == ElementState::Pressed,
                button,
                position.x,
                position.y,
                primary,
            ),
            WindowEvent::PointerMoved {
                source,
                position,
                primary,
                ..
            } => self.handle_pointer_move(source, position.x, position.y, primary),
            WindowEvent::MouseWheel { delta, .. } => self.handle_wheel(delta),
            _ => {}
        }
    }

    fn handle_pointer_enter(&mut self, kind: PointerKind, x: f64, y: f64, primary: bool) {
        match kind {
            PointerKind::Touch(_) => self.sync_hover_pointer(HoverPointer::Touch, None),
            _ if primary => self.sync_hover_pointer(HoverPointer::Mouse, Some((x, y))),
            _ => {}
        }
    }

    fn handle_pointer_left(&mut self, kind: PointerKind) {
        self.finish_build_pointer_gesture();
        match kind {
            PointerKind::Touch(finger_id) => {
                self.input
                    .active_touches
                    .remove(&(finger_id.into_raw() as u64));
                if self.input.active_touches.len() < 2 {
                    self.input.last_pinch_state = None;
                }
                if self.input.active_touches.is_empty() {
                    self.input.map_pointer_start = None;
                }
                self.rebase_single_touch_drag();
                self.sync_hover_pointer(HoverPointer::Touch, None);
            }
            _ => {
                self.input.dragging = false;
                self.sync_hover_pointer(HoverPointer::None, None);
            }
        }
    }

    fn rebase_single_touch_drag(&mut self) {
        if self.input.active_touches.len() == 1
            && self.ui.app.hud_state.selected_building_kind.is_none()
            && self.ui.app.hud_state.selected_nuke_kind.is_none()
            && !self.ui.app.hud_state.selected_warship_build
        {
            if let Some((x, y)) = single_touch_position(&self.input.active_touches) {
                self.input.last_mouse_x = x;
                self.input.last_mouse_y = y;
                self.input.dragging = true;
            }
        } else {
            self.input.dragging = false;
        }
    }

    fn sync_hover_pointer(&mut self, pointer: HoverPointer, position: Option<(f64, f64)>) {
        match pointer {
            HoverPointer::None => self.input.hover_pointer = HoverPointer::None,
            HoverPointer::Mouse => {
                let Some((x, y)) = position else {
                    return;
                };
                self.input.last_mouse_x = x;
                self.input.last_mouse_y = y;
                self.input.hover_pointer = HoverPointer::Mouse;
            }
            HoverPointer::Touch => {
                let Some((x, y)) = single_touch_position(&self.input.active_touches) else {
                    self.input.hover_pointer = HoverPointer::None;
                    return;
                };
                self.input.last_mouse_x = x;
                self.input.last_mouse_y = y;
                self.input.hover_pointer = HoverPointer::Touch;
            }
        }
    }

    fn record_tutorial_camera_drag_if_moved(&mut self, x: f64, y: f64) {
        if !self.ui.tutorial_camera_only || self.input.tutorial_camera_drag_recorded {
            return;
        }
        let moved_sq = self
            .input
            .map_pointer_start
            .as_ref()
            .map(|start| (x - start.x).powi(2) + (y - start.y).powi(2))
            .unwrap_or(0.0);
        if moved_sq > 400.0 {
            self.record_tutorial_camera_drag();
            self.input.tutorial_camera_drag_recorded = true;
        }
    }

    fn handle_key_event(&mut self, pressed: bool, key: winit::keyboard::PhysicalKey) {
        let in_game =
            self.ui.app.phase == ClientPhase::Playing && self.ui.app.hud_state.sync_state.is_none();
        if !in_game || self.input.input_focused {
            self.input.key_pan_up = false;
            self.input.key_pan_down = false;
            self.input.key_pan_left = false;
            self.input.key_pan_right = false;
            return;
        }
        let winit::keyboard::PhysicalKey::Code(code) = key else {
            return;
        };
        let pan_key_was_pressed = match code {
            winit::keyboard::KeyCode::KeyW | winit::keyboard::KeyCode::ArrowUp => {
                self.input.key_pan_up
            }
            winit::keyboard::KeyCode::KeyS | winit::keyboard::KeyCode::ArrowDown => {
                self.input.key_pan_down
            }
            winit::keyboard::KeyCode::KeyA | winit::keyboard::KeyCode::ArrowLeft => {
                self.input.key_pan_left
            }
            winit::keyboard::KeyCode::KeyD | winit::keyboard::KeyCode::ArrowRight => {
                self.input.key_pan_right
            }
            _ => false,
        };
        match code {
            winit::keyboard::KeyCode::KeyW | winit::keyboard::KeyCode::ArrowUp => {
                self.input.key_pan_up = pressed
            }
            winit::keyboard::KeyCode::KeyS | winit::keyboard::KeyCode::ArrowDown => {
                self.input.key_pan_down = pressed
            }
            winit::keyboard::KeyCode::KeyA | winit::keyboard::KeyCode::ArrowLeft => {
                self.input.key_pan_left = pressed
            }
            winit::keyboard::KeyCode::KeyD | winit::keyboard::KeyCode::ArrowRight => {
                self.input.key_pan_right = pressed
            }
            _ => {}
        }
        if pressed && self.ui.tutorial_camera_only && !pan_key_was_pressed {
            self.record_tutorial_camera_key_pan();
        }
        if !pressed || self.ui.tutorial_camera_only {
            return;
        }

        if code == winit::keyboard::KeyCode::KeyB {
            if let Some((col, row)) =
                self.mouse_to_tile(self.input.last_mouse_x, self.input.last_mouse_y)
            {
                let idx = (row * self.sim.map_w as i32 + col) as usize;
                self.launch_fleet_from_tile(idx as u32);
            }
        }

        let building = match code {
            winit::keyboard::KeyCode::Digit1 | winit::keyboard::KeyCode::Numpad1 => {
                Some(sow_core::game::BuildingKind::City)
            }
            winit::keyboard::KeyCode::Digit2 | winit::keyboard::KeyCode::Numpad2 => {
                Some(sow_core::game::BuildingKind::Factory)
            }
            winit::keyboard::KeyCode::Digit3 | winit::keyboard::KeyCode::Numpad3 => {
                Some(sow_core::game::BuildingKind::Port)
            }
            winit::keyboard::KeyCode::Digit4 | winit::keyboard::KeyCode::Numpad4 => {
                Some(sow_core::game::BuildingKind::Bunker)
            }
            winit::keyboard::KeyCode::Digit5 | winit::keyboard::KeyCode::Numpad5 => {
                Some(sow_core::game::BuildingKind::Farm)
            }
            _ => None,
        };
        if let Some(kind) = building {
            self.select_building_kind(kind);
        }
        if code == winit::keyboard::KeyCode::Digit0 || code == winit::keyboard::KeyCode::Numpad0 {
            self.select_nuke_kind(sow_core::game::NukeKind::AtomBomb);
        }
        if code == winit::keyboard::KeyCode::Escape || code == winit::keyboard::KeyCode::KeyQ {
            self.clear_placement();
            self.close_map_context_menu();
        }
    }

    fn handle_pointer_button(
        &mut self,
        pressed: bool,
        button: winit::event::ButtonSource,
        x: f64,
        y: f64,
        primary: bool,
    ) {
        let pointer_was_down = self.input.map_pointer_start.is_some();
        let previous_mouse_x = self.input.last_mouse_x;
        let previous_mouse_y = self.input.last_mouse_y;

        let is_touch = matches!(button, winit::event::ButtonSource::Touch { .. });
        let left =
            matches!(button, winit::event::ButtonSource::Mouse(MouseButton::Left)) || is_touch;
        let right = matches!(
            button,
            winit::event::ButtonSource::Mouse(MouseButton::Right)
        );

        if let winit::event::ButtonSource::Touch { finger_id, .. } = button {
            let id = finger_id.into_raw() as u64;
            if pressed {
                let first_touch = self.input.active_touches.is_empty();
                self.input.active_touches.insert(id, (x, y));
                if first_touch {
                    self.input.tutorial_camera_drag_recorded = false;
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch: true,
                        action_sent: false,
                    });
                } else {
                    self.finish_build_pointer_gesture();
                    self.cancel_hold_build();
                    self.input.dragging = false;
                    self.close_map_context_menu();
                }
            } else {
                self.input.active_touches.remove(&id);
                if self.input.active_touches.len() < 2 {
                    self.input.last_pinch_state = None;
                }
                self.rebase_single_touch_drag();
            }
        }

        if is_touch {
            self.sync_hover_pointer(HoverPointer::Touch, None);
        } else if primary {
            self.sync_hover_pointer(HoverPointer::Mouse, Some((x, y)));
        }

        let in_game =
            self.ui.app.phase == ClientPhase::Playing && self.ui.app.hud_state.sync_state.is_none();
        let build_tool_selected = self.ui.app.hud_state.selected_building_kind.is_some();
        let warship_tool_selected = self.ui.app.hud_state.selected_warship_build;

        if left {
            if pressed {
                // winit-web reports pointermove with a held button through this
                // handler. Keep the original press so release still resolves
                // the tile that was clicked instead of turning the gesture
                // into a new click at every move.
                if !is_touch && pointer_was_down {
                    if self.input.dragging {
                        self.record_tutorial_camera_drag_if_moved(x, y);
                        self.close_map_context_menu();
                        self.input.camera_x += (x - previous_mouse_x) as f32;
                        self.input.camera_y += (y - previous_mouse_y) as f32;
                        self.clamp_camera_to_map();
                    }
                    return;
                }
                self.close_map_context_menu();
                if !is_touch && !pointer_was_down {
                    self.input.tutorial_camera_drag_recorded = false;
                }
                self.input.dragging = self.ui.tutorial_camera_only
                    || (!build_tool_selected
                        && !warship_tool_selected
                        && self.ui.app.hud_state.selected_nuke_kind.is_none());
                if !is_touch {
                    let action_sent = !self.ui.tutorial_camera_only
                        && (build_tool_selected || self.try_attack_at(x, y));
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch,
                        action_sent,
                    });
                    if build_tool_selected {
                        if in_game && !self.ui.observing {
                            self.begin_hold_build(x, y);
                        }
                    }
                } else if self.input.map_pointer_start.is_none()
                    && self.input.active_touches.len() == 1
                {
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch,
                        action_sent: build_tool_selected,
                    });
                }
                if is_touch && build_tool_selected && self.input.active_touches.len() == 1 {
                    let has_start = if let Some(start) = self.input.map_pointer_start.as_mut() {
                        start.action_sent = true;
                        true
                    } else {
                        false
                    };
                    if has_start && in_game && !self.ui.observing {
                        self.begin_hold_build(x, y);
                    }
                }
            } else {
                let Some(start) = self.finish_build_pointer_gesture() else {
                    if self.input.active_touches.is_empty() {
                        self.input.dragging = false;
                    }
                    return;
                };
                if self.input.active_touches.is_empty() {
                    self.input.dragging = false;
                }
                let distance_sq = (x - start.x).powi(2) + (y - start.y).powi(2);
                if distance_sq > 400.0 || self.ui.app.phase != ClientPhase::Playing {
                    return;
                }
                if start.action_sent {
                    return;
                }
                if start.is_touch {
                    let elapsed_ms = start.started_at.elapsed().as_millis();
                    if is_quick_tap(elapsed_ms, distance_sq) {
                        self.handle_map_click(start.x, start.y);
                    } else if self.input.active_touches.is_empty() {
                        self.open_map_context_menu(start.x, start.y);
                    }
                } else {
                    self.handle_map_click(start.x, start.y);
                }
            }
        } else if right
            && pressed
            && self.ui.app.phase == ClientPhase::Playing
            && !self.ui.tutorial_camera_only
        {
            if self.ui.app.hud_state.selected_building_kind.is_some()
                || self.ui.app.hud_state.selected_nuke_kind.is_some()
                || self.ui.app.hud_state.selected_warship_build
            {
                self.clear_placement();
                self.close_map_context_menu();
            } else if !self.move_selected_warships(x, y) {
                self.open_map_context_menu(x, y);
            } else {
                self.close_map_context_menu();
            }
        }
    }

    fn handle_pointer_move(
        &mut self,
        source: winit::event::PointerSource,
        x: f64,
        y: f64,
        primary: bool,
    ) {
        if let winit::event::PointerSource::Touch { finger_id, .. } = source {
            self.input
                .active_touches
                .insert(finger_id.into_raw() as u64, (x, y));
        }
        let is_touch = matches!(source, winit::event::PointerSource::Touch { .. });
        if self.input.active_touches.len() >= 2 {
            self.finish_build_pointer_gesture();
            self.cancel_hold_build();
            self.input.dragging = false;
            self.input.hover_pointer = HoverPointer::None;
            self.close_map_context_menu();
            let mut points = self.input.active_touches.values();
            let (x1, y1) = *points.next().expect("two active touches");
            let (x2, y2) = *points.next().expect("two active touches");
            let dx = x1 - x2;
            let dy = y1 - y2;
            let distance = (dx * dx + dy * dy).sqrt();
            let cx = (x1 + x2) * 0.5;
            let cy = (y1 + y2) * 0.5;
            if let Some((last_distance, last_x, last_y)) = self.input.last_pinch_state {
                self.input.camera_x += (cx - last_x) as f32;
                self.input.camera_y += (cy - last_y) as f32;
                self.process_camera_zoom(
                    1.0 + ((distance - last_distance) as f32 * 0.005),
                    cx as f32,
                    cy as f32,
                );
            }
            self.input.last_pinch_state = Some((distance, cx, cy));
        } else if is_touch {
            let mut crossed_drag_threshold = false;
            if let Some(start) = self.input.map_pointer_start.as_ref() {
                let distance_sq = (x - start.x).powi(2) + (y - start.y).powi(2);
                if distance_sq > 400.0 {
                    if !self.input.hold_build_active {
                        self.record_tutorial_camera_drag_if_moved(x, y);
                        self.input.map_pointer_start = None;
                        crossed_drag_threshold = true;
                    }
                }
            }
            if (crossed_drag_threshold || self.input.map_pointer_start.is_none())
                && self.input.dragging
            {
                if self.ui.tutorial_camera_only && !self.input.tutorial_camera_drag_recorded {
                    self.record_tutorial_camera_drag();
                    self.input.tutorial_camera_drag_recorded = true;
                }
                self.close_map_context_menu();
                self.input.camera_x += (x - self.input.last_mouse_x) as f32;
                self.input.camera_y += (y - self.input.last_mouse_y) as f32;
                self.clamp_camera_to_map();
            }
        } else if matches!(source, winit::event::PointerSource::Mouse)
            && self.input.dragging
            && self
                .input
                .map_pointer_start
                .as_ref()
                .is_some_and(|start| !start.is_touch)
        {
            self.record_tutorial_camera_drag_if_moved(x, y);
            self.close_map_context_menu();
            self.input.camera_x += (x - self.input.last_mouse_x) as f32;
            self.input.camera_y += (y - self.input.last_mouse_y) as f32;
            self.clamp_camera_to_map();
        }
        if is_touch {
            self.sync_hover_pointer(HoverPointer::Touch, None);
        } else if primary {
            self.sync_hover_pointer(HoverPointer::Mouse, Some((x, y)));
        }
    }

    fn handle_wheel(&mut self, delta: MouseScrollDelta) {
        if !self.input.active_touches.is_empty() || self.ui.app.phase != ClientPhase::Playing {
            return;
        }
        self.close_map_context_menu();
        let scroll = match delta {
            MouseScrollDelta::LineDelta(x, y) => {
                if y.abs() >= x.abs() {
                    y
                } else {
                    x
                }
            }
            MouseScrollDelta::PixelDelta(position) => {
                let x = position.x as f32 / 50.0;
                let y = position.y as f32 / 50.0;
                if y.abs() >= x.abs() { y } else { x }
            }
        };
        let zmin = self.zoom_floor();
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h).max(zmin);
        let previous_zoom = self.input.target_zoom;
        self.input.target_zoom = (previous_zoom * (1.0 + scroll * 0.15)).clamp(zmin, zmax);
        self.record_tutorial_zoom(self.input.target_zoom - previous_zoom);
    }

    fn cancel_pointer_gesture(&mut self) {
        self.finish_build_pointer_gesture();
        self.input.dragging = false;
        self.input.active_touches.clear();
        self.input.last_pinch_state = None;
        self.sync_hover_pointer(HoverPointer::None, None);
    }

    pub(crate) fn poll_pointer_hold(&mut self) {
        let Some(start) = self.input.map_pointer_start.as_ref() else {
            return;
        };
        if !start.is_touch
            || self.input.active_touches.len() != 1
            || self.ui.tutorial_camera_only
            || self.ui.app.phase != ClientPhase::Playing
            || start.action_sent
            || self.ui.app.hud_state.selected_building_kind.is_some()
            || self.ui.app.hud_state.selected_nuke_kind.is_some()
            || self.ui.app.hud_state.selected_warship_build
            || start.started_at.elapsed().as_millis() < TOUCH_HOLD_MS
        {
            return;
        }
        let (x, y) = (start.x, start.y);
        self.input.map_pointer_start = None;
        self.input.dragging = false;
        self.open_map_context_menu(x, y);
    }

    pub(crate) fn begin_hold_build(&mut self, x: f64, y: f64) {
        if self.ui.tutorial_camera_only {
            return;
        }
        self.input.hold_build_action_succeeded = false;
        self.input.hold_build_shift_override = false;
        self.input.hold_build_active = true;
        self.input.hold_build_accum = HOLD_BUILD_INTERVAL_SECS;
        self.handle_map_click(x, y);
    }

    fn finish_build_pointer_gesture(&mut self) -> Option<MapPointerStart> {
        let start = self.input.map_pointer_start.take();
        let held_mobile = start.as_ref().is_some_and(|start| {
            start.is_touch
                && start.started_at.elapsed().as_secs_f32() >= HOLD_BUILD_INTERVAL_SECS
        });
        let should_exit = should_exit_build_mode_after_gesture(
            self.ui.app.settings_state.sticky_building_mode,
            self.input.hold_build_action_succeeded,
            self.input.hold_build_shift_override,
            held_mobile,
        );
        self.cancel_hold_build();
        self.input.hold_build_action_succeeded = false;
        self.input.hold_build_shift_override = false;
        if should_exit {
            self.clear_placement();
        }
        start
    }

    pub(crate) fn cancel_hold_build(&mut self) {
        self.input.hold_build_active = false;
        self.input.hold_build_accum = 0.0;
    }

    pub(crate) fn pump_hold_build(&mut self, dt: f32, now: web_time::Instant) {
        if !self.input.hold_build_active {
            return;
        }
        let Some(start) = self.input.map_pointer_start.as_ref() else {
            self.cancel_hold_build();
            return;
        };
        if self.ui.app.phase != ClientPhase::Playing
            || self.ui.app.hud_state.sync_state.is_some()
            || self.ui.observing
            || self.ui.app.hud_state.selected_building_kind.is_none()
            || self.input.active_touches.len() > 1
        {
            self.cancel_hold_build();
            return;
        }
        let held_secs = now.duration_since(start.started_at).as_secs_f32();
        if advance_hold_build_timer(&mut self.input.hold_build_accum, dt, held_secs) {
            self.handle_map_click(self.input.last_mouse_x, self.input.last_mouse_y);
        }
    }
}

fn should_exit_build_mode_after_gesture(
    sticky: bool,
    action_succeeded: bool,
    shift_override: bool,
    held_mobile: bool,
) -> bool {
    action_succeeded && !sticky && !shift_override && !held_mobile
}

fn single_touch_position(
    active_touches: &std::collections::HashMap<u64, (f64, f64)>,
) -> Option<(f64, f64)> {
    (active_touches.len() == 1)
        .then(|| active_touches.values().next().copied())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::{
        HOLD_BUILD_BURST_INTERVAL_SECS, HOLD_BUILD_INTERVAL_SECS, advance_hold_build_timer,
        hold_build_repeat_interval, should_exit_build_mode_after_gesture, single_touch_position,
    };
    use std::collections::HashMap;

    #[test]
    fn touch_hover_requires_exactly_one_finger() {
        let mut touches = HashMap::new();
        assert_eq!(single_touch_position(&touches), None);

        touches.insert(7, (120.0, 240.0));
        assert_eq!(single_touch_position(&touches), Some((120.0, 240.0)));

        touches.insert(8, (320.0, 480.0));
        assert_eq!(single_touch_position(&touches), None);
    }

    #[test]
    fn hold_build_changes_to_burst_after_two_seconds() {
        assert_eq!(hold_build_repeat_interval(1.999), HOLD_BUILD_INTERVAL_SECS);
        assert_eq!(
            hold_build_repeat_interval(2.0),
            HOLD_BUILD_BURST_INTERVAL_SECS
        );

        let mut remaining = HOLD_BUILD_INTERVAL_SECS;
        assert!(!advance_hold_build_timer(&mut remaining, 0.249, 0.0));
        assert!(advance_hold_build_timer(&mut remaining, 0.001, 0.25));
        assert_eq!(remaining, HOLD_BUILD_INTERVAL_SECS);

        remaining = 0.001;
        assert!(advance_hold_build_timer(&mut remaining, 0.001, 2.0));
        assert_eq!(remaining, HOLD_BUILD_BURST_INTERVAL_SECS);
    }

    #[test]
    fn building_mode_exits_only_after_success_without_a_keep_override() {
        assert!(should_exit_build_mode_after_gesture(false, true, false, false));
        assert!(!should_exit_build_mode_after_gesture(false, false, false, false));
        assert!(!should_exit_build_mode_after_gesture(true, true, false, false));
        assert!(!should_exit_build_mode_after_gesture(false, true, true, false));
        assert!(!should_exit_build_mode_after_gesture(false, true, false, true));
    }
}
