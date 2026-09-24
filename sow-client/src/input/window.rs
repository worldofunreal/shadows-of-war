use crate::app::{HoverPointer, MapPointerStart, SowApp};
use crate::input::map_click::{TOUCH_HOLD_MS, is_quick_tap};
use crate::{ClientPhase, camera_zoom_lower_bound, camera_zoom_upper_bound};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, PointerKind, WindowEvent};

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
                self.cancel_pointer_gesture();
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
                self.input.map_pointer_start = None;
                self.input.dragging = false;
                self.sync_hover_pointer(HoverPointer::None, None);
            }
        }
    }

    fn rebase_single_touch_drag(&mut self) {
        if self.input.active_touches.len() == 1
            && self.ui.app.hud_state.selected_building_kind.is_none()
            && self.ui.app.hud_state.selected_nuke_kind.is_none()
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
        if !pressed {
            return;
        }

        if code == winit::keyboard::KeyCode::KeyB {
            if let Some((col, row)) =
                self.mouse_to_tile(self.input.last_mouse_x, self.input.last_mouse_y)
            {
                let idx = (row * self.sim.map_w as i32 + col) as usize;
                self.launch_fleet_from_tile(
                    idx as u32,
                    (self.input.last_mouse_x, self.input.last_mouse_y),
                );
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
            _ => None,
        };
        if let Some(kind) = building {
            self.select_building_kind(kind);
        }
        if code == winit::keyboard::KeyCode::Digit0 || code == winit::keyboard::KeyCode::Numpad0 {
            let kind = sow_core::game::NukeKind::AtomBomb;
            let selected = &mut self.ui.app.hud_state.selected_nuke_kind;
            *selected = (*selected != Some(kind)).then_some(kind);
            self.ui.app.hud_state.selected_building_kind = None;
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

        if let winit::event::ButtonSource::Touch { finger_id, .. } = button {
            let id = finger_id.into_raw() as u64;
            if pressed {
                let first_touch = self.input.active_touches.is_empty();
                self.input.active_touches.insert(id, (x, y));
                if first_touch {
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch: true,
                        attack_sent: false,
                    });
                } else {
                    self.input.map_pointer_start = None;
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

        if left {
            if pressed {
                // winit-web reports pointermove with a held button through this
                // handler. Keep the original press so release still resolves
                // the tile that was clicked instead of turning the gesture
                // into a new click at every move.
                if !is_touch && pointer_was_down {
                    if self.input.dragging {
                        self.close_map_context_menu();
                        self.input.camera_x += (x - previous_mouse_x) as f32;
                        self.input.camera_y += (y - previous_mouse_y) as f32;
                        self.clamp_camera_to_map();
                    }
                    return;
                }
                self.close_map_context_menu();
                self.input.dragging = self.ui.app.hud_state.selected_building_kind.is_none()
                    && self.ui.app.hud_state.selected_nuke_kind.is_none();
                if !is_touch {
                    let attack_sent = self.try_attack_at(x, y);
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch,
                        attack_sent,
                    });
                } else if self.input.map_pointer_start.is_none()
                    && self.input.active_touches.len() == 1
                {
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch,
                        attack_sent: false,
                    });
                }
            } else {
                if self.input.active_touches.is_empty() {
                    self.input.dragging = false;
                }
                let Some(start) = self.input.map_pointer_start.take() else {
                    return;
                };
                let distance_sq = (x - start.x).powi(2) + (y - start.y).powi(2);
                if distance_sq > 400.0 || self.ui.app.phase != ClientPhase::Playing {
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
                    if !start.attack_sent {
                        self.handle_map_click(start.x, start.y);
                    }
                }
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
            self.input.map_pointer_start = None;
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
                    self.input.map_pointer_start = None;
                    crossed_drag_threshold = true;
                }
            }
            if (crossed_drag_threshold || self.input.map_pointer_start.is_none())
                && self.input.dragging
            {
                self.close_map_context_menu();
                self.input.camera_x += (x - self.input.last_mouse_x) as f32;
                self.input.camera_y += (y - self.input.last_mouse_y) as f32;
                self.clamp_camera_to_map();
            }
        } else if primary && self.input.dragging {
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
        let zmin = camera_zoom_lower_bound(
            self.input.screen_w,
            self.input.screen_h,
            self.sim.map_w,
            self.sim.map_h,
        );
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h).max(zmin);
        self.input.target_zoom = (self.input.target_zoom * (1.0 + scroll * 0.15)).clamp(zmin, zmax);
    }

    fn cancel_pointer_gesture(&mut self) {
        self.input.dragging = false;
        self.input.map_pointer_start = None;
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
            || self.ui.app.phase != ClientPhase::Playing
            || self.ui.app.hud_state.selected_building_kind.is_some()
            || self.ui.app.hud_state.selected_nuke_kind.is_some()
            || start.started_at.elapsed().as_millis() < TOUCH_HOLD_MS
        {
            return;
        }
        let (x, y) = (start.x, start.y);
        self.input.map_pointer_start = None;
        self.input.dragging = false;
        self.open_map_context_menu(x, y);
    }
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
    use super::single_touch_position;
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
}
