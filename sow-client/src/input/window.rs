use crate::app::{HoverPointer, MapContextMenuView, MapPointerStart, SowApp};
use crate::input::map_click::{TOUCH_HOLD_MS, is_quick_tap};
use crate::{ClientPhase, camera_zoom_upper_bound};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, PointerKind, WindowEvent};

/// A right press this soon after an accepted one is treated as the same click.
const RIGHT_PRESS_LOCKOUT_MS: u128 = 300;

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
        self.finish_pointer_gesture();
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
        #[cfg(target_arch = "wasm32")]
        if crate::web_canvas::ui_has_keyboard_focus() {
            self.input.key_pan_up = false;
            self.input.key_pan_down = false;
            self.input.key_pan_left = false;
            self.input.key_pan_right = false;
            return;
        }
        if self.ui.app.hud_state.building_placement.is_some() && !self.ui.tutorial_camera_only {
            use super::placement::PlacementDirection;
            let direction = match code {
                winit::keyboard::KeyCode::ArrowUp => Some(PlacementDirection::Up),
                winit::keyboard::KeyCode::ArrowRight => Some(PlacementDirection::Right),
                winit::keyboard::KeyCode::ArrowDown => Some(PlacementDirection::Down),
                winit::keyboard::KeyCode::ArrowLeft => Some(PlacementDirection::Left),
                _ => None,
            };
            if let Some(direction) = direction {
                if pressed {
                    self.nudge_building_placement(direction);
                }
                return;
            }
        }
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
                Some(sow_core::game::BuildingKind::Tower)
            }
            winit::keyboard::KeyCode::Digit5 | winit::keyboard::KeyCode::Numpad5 => {
                Some(sow_core::game::BuildingKind::Farm)
            }
            _ => None,
        };
        if let Some(kind) = building {
            self.select_building_kind_from_pointer(kind);
        }
        if code == winit::keyboard::KeyCode::Enter || code == winit::keyboard::KeyCode::NumpadEnter
        {
            self.confirm_building_placement();
        }
        if code == winit::keyboard::KeyCode::Digit0 || code == winit::keyboard::KeyCode::Numpad0 {
            self.select_nuke_kind(sow_core::game::NukeKind::AtomBomb);
        }
        if code == winit::keyboard::KeyCode::Escape || code == winit::keyboard::KeyCode::KeyQ {
            self.clear_placement();
            self.close_map_context_menu();
        }
    }

    /// A press that only closes the radial menu is consumed by that close: it
    /// must not attack, and it must not reopen a menu on the tile it lands on.
    fn press_dismisses_radial(&self, x: f64, y: f64) -> bool {
        self.input
            .map_context_menu
            .is_some_and(|menu| menu.view == MapContextMenuView::Radial)
            && !self.building_panel_button_at(x as f32, y as f32)
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
        if pressed
            && pointer_was_down
            && matches!(button, winit::event::ButtonSource::Mouse(MouseButton::Left))
        {
            self.handle_pointer_move(winit::event::PointerSource::Mouse, x, y, primary);
            return;
        }

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
                        action_sent: self.press_dismisses_radial(x, y),
                    });
                } else {
                    self.finish_pointer_gesture();
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

        let build_tool_selected = self.ui.app.hud_state.building_placement.is_some();
        let warship_tool_selected = self.ui.app.hud_state.selected_warship_build;

        if left {
            if pressed {
                // winit-web reports pointermove with a held button through this
                // handler. Keep the original press so release still resolves
                // the tile that was clicked instead of turning the gesture
                // into a new click at every move.
                // A press on the building panel's button keeps the panel open.
                let panel_button = self.building_panel_button_at(x as f32, y as f32);
                let dismisses_radial = self.press_dismisses_radial(x, y);
                if !panel_button {
                    self.close_map_context_menu();
                }
                if !is_touch && !pointer_was_down {
                    self.input.tutorial_camera_drag_recorded = false;
                }
                self.input.dragging = !panel_button
                    && !self.machinima_camera_active()
                    && (self.ui.tutorial_camera_only
                        || (!warship_tool_selected
                            && self.ui.app.hud_state.selected_nuke_kind.is_none()));
                if !is_touch {
                    let action_sent = dismisses_radial
                        || (!self.ui.tutorial_camera_only
                            && !build_tool_selected
                            && !panel_button
                            && self.try_attack_at(x, y));
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch,
                        action_sent,
                    });
                } else if self.input.map_pointer_start.is_none()
                    && self.input.active_touches.len() == 1
                {
                    self.input.map_pointer_start = Some(MapPointerStart {
                        started_at: web_time::Instant::now(),
                        x,
                        y,
                        is_touch,
                        action_sent: dismisses_radial,
                    });
                }
                if build_tool_selected && (!is_touch || self.input.active_touches.len() == 1) {
                    if let Some(controls) = self.placement_controls() {
                        if controls.button_at(x as f32, y as f32).is_some() {
                            // Release on the button decides; the press never pans the map.
                            self.input.dragging = false;
                        } else if controls.grabs(x as f32, y as f32) {
                            self.input.building_drag_offset = Some((
                                (x as f32 - controls.center[0]) / self.input.camera_zoom,
                                (y as f32 - controls.center[1]) / self.input.camera_zoom,
                            ));
                            self.input.dragging = false;
                        }
                    }
                }
            } else {
                let was_building_drag = self.input.building_drag_offset.is_some();
                let Some(start) = self.finish_pointer_gesture() else {
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
                if start.action_sent || was_building_drag {
                    return;
                }
                if build_tool_selected {
                    self.handle_map_click(x, y);
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
            let since_ms = self.input.last_right_press.map(|at| at.elapsed().as_millis());
            let repeated = since_ms.is_some_and(|ms| ms < RIGHT_PRESS_LOCKOUT_MS);
            log::info!(
                "[RIGHT TRACE] press x={x:.0} y={y:.0} since_last_ms={since_ms:?} repeated={repeated} radial_open={}",
                self.input.map_context_menu.is_some()
            );
            if repeated {
                return;
            }
            self.input.last_right_press = Some(web_time::Instant::now());
            if self.ui.app.hud_state.building_placement.is_some()
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
        } else if right && !pressed {
            log::info!(
                "[RIGHT TRACE] release x={x:.0} y={y:.0} radial_open={}",
                self.input.map_context_menu.is_some()
            );
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
        // A code-directed camera move owns the view: drags and pinches start nothing until it lands.
        let machinima = self.machinima_camera_active();
        if machinima {
            self.input.dragging = false;
            self.input.last_pinch_state = None;
        }
        if self.input.active_touches.len() >= 2 && !machinima {
            self.finish_pointer_gesture();
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
        } else if let Some((offset_x, offset_y)) = self.input.building_drag_offset {
            let col = ((x as f32 - self.input.camera_x) / self.input.camera_zoom - offset_x).round()
                as i32;
            let row = ((y as f32 - self.input.camera_y) / self.input.camera_zoom - offset_y).round()
                as i32;
            self.move_building_preview(col, row);
        } else if is_touch {
            let mut crossed_drag_threshold = false;
            if let Some(start) = self.input.map_pointer_start.as_ref() {
                let distance_sq = (x - start.x).powi(2) + (y - start.y).powi(2);
                if distance_sq > 400.0 {
                    self.record_tutorial_camera_drag_if_moved(x, y);
                    self.input.map_pointer_start = None;
                    crossed_drag_threshold = true;
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
        if !self.input.active_touches.is_empty()
            || self.ui.app.phase != ClientPhase::Playing
            || self.machinima_camera_active()
        {
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
        self.finish_pointer_gesture();
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
            || self.ui.app.hud_state.building_placement.is_some()
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

    fn finish_pointer_gesture(&mut self) -> Option<MapPointerStart> {
        self.input.building_drag_offset = None;
        self.input.map_pointer_start.take()
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
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn touch_hover_requires_exactly_one_finger() {
        let mut touches = HashMap::new();
        assert_eq!(single_touch_position(&touches), None);
        touches.insert(7, (120.0, 240.0));
        assert_eq!(single_touch_position(&touches), Some((120.0, 240.0)));
        touches.insert(8, (200.0, 240.0));
        assert_eq!(single_touch_position(&touches), None);
    }
    #[wasm_bindgen_test]
    #[cfg(target_arch = "wasm32")]
    async fn building_drag_pan_cancel_and_pending_confirmation_never_spend_gold() {
        use crate::app::SowApp;
        use crate::ui::hud::state::BuildingPlacement;
        use winit::event::{ButtonSource, FingerId, MouseButton, PointerSource};
        let profile = serde_json::json!({ "account_id": "0123456789abcdef0123456789abcdef", "auth_secret": "placement-test", "display_name": "Builder", "profile": crate::player_progress::PlayerProgress::default() }).to_string();
        crate::app::setup_tutorial_exit(&profile, &profile);
        let mut app = SowApp::new();
        app.ui.app.phase = crate::ClientPhase::Playing;
        app.sim.map_w = 64;
        app.sim.map_h = 64;
        app.input.camera_zoom = 12.0;
        app.input.target_zoom = 12.0;
        app.input.camera_x = 0.0;
        app.input.camera_y = 0.0;
        app.input.screen_w = 800.0;
        app.input.screen_h = 600.0;
        let config = sow_core::game_config::GameConfig::default();
        let mut state = sow_core::game::GameState::new(42, 64, 64, config.clone());
        state.phase = sow_core::game::GamePhase::Playing;
        let mut player =
            sow_core::player::Player::new_human(1, "Builder".into(), [1.0; 3], &config);
        player.has_spawned = true;
        player.gold = 1_000_000.0;
        state.register_player(player);
        for y in 0..64 {
            for x in 0..64 {
                state.map.set_owner_id(x, y, 1);
                state.map.terrain[(y * 64 + x) as usize] = sow_core::map::MapTile::from_byte(0x80);
            }
        }
        // A water channel the arrows must jump over.
        for y in 0..64 {
            for x in 50..52 {
                state.map.terrain[(y * 64 + x) as usize] = sow_core::map::MapTile::from_byte(0);
            }
        }
        let mut engine = sow_core::engine::SowEngine::new(
            state,
            sow_core::water_components::WaterComponents::default(),
        );
        app.sim.current_snapshot = Some(engine.build_snapshot());
        app.sim.engine = Some(engine);
        app.sim.my_player_id = Some(1);
        app.sim.config = config;
        app.net.is_offline = true;

        use crate::app::HoverPointer;
        use winit::keyboard::{KeyCode, PhysicalKey};
        let pointer_position = (240.0, 240.0);
        let (pointer_col, pointer_row) = app
            .mouse_to_tile(pointer_position.0, pointer_position.1)
            .unwrap();
        app.input.last_mouse_x = pointer_position.0;
        app.input.last_mouse_y = pointer_position.1;
        app.input.hover_pointer = HoverPointer::Mouse;
        app.handle_key_event(true, PhysicalKey::Code(KeyCode::Digit2));
        let placement = app.ui.app.hud_state.building_placement.unwrap();
        assert_eq!(placement.kind, sow_core::game::BuildingKind::Factory);
        assert_eq!(
            placement.tile_idx,
            (pointer_row * 64 + pointer_col) as u32,
            "a building hotkey starts on the legal map tile under the mouse"
        );

        let water_position = (600.0, 240.0);
        let (water_col, water_row) = app
            .mouse_to_tile(water_position.0, water_position.1)
            .unwrap();
        let zoom = app.input.camera_zoom.max(0.01);
        let radius = (app.input.screen_w.max(app.input.screen_h) * 0.5 / zoom).ceil() as i32;
        let nearest_legal = {
            let engine = app.sim.engine.as_mut().unwrap();
            engine.refresh_building_grid();
            sow_core::building::nearest_structure_site(
                &engine.state.map,
                1,
                sow_core::game::BuildingKind::Farm,
                water_col,
                water_row,
                radius,
                &engine.building_grid,
            )
            .unwrap()
        };
        app.input.last_mouse_x = water_position.0;
        app.input.last_mouse_y = water_position.1;
        app.handle_key_event(true, PhysicalKey::Code(KeyCode::Digit5));
        let placement = app.ui.app.hud_state.building_placement.unwrap();
        assert_eq!(placement.kind, sow_core::game::BuildingKind::Farm);
        assert_eq!(placement.tile_idx, nearest_legal);
        assert_ne!(placement.tile_idx, (water_row * 64 + water_col) as u32);

        let camera_center = (
            ((app.input.screen_w * 0.5 - app.input.camera_x) / zoom) as i32,
            ((app.input.screen_h * 0.45 - app.input.camera_y) / zoom) as i32,
        );
        let center_site = {
            let engine = app.sim.engine.as_mut().unwrap();
            engine.refresh_building_grid();
            sow_core::building::nearest_structure_site(
                &engine.state.map,
                1,
                sow_core::game::BuildingKind::City,
                camera_center.0,
                camera_center.1,
                radius,
                &engine.building_grid,
            )
            .unwrap()
        };
        app.select_building_kind(sow_core::game::BuildingKind::City);
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            center_site,
            "the HUD selector keeps its camera-centered placement origin"
        );

        app.input.hover_pointer = HoverPointer::None;
        app.input.last_mouse_x = 120.0;
        app.input.last_mouse_y = 120.0;
        let tower_center_site = {
            let engine = app.sim.engine.as_mut().unwrap();
            engine.refresh_building_grid();
            sow_core::building::nearest_structure_site(
                &engine.state.map,
                1,
                sow_core::game::BuildingKind::Tower,
                camera_center.0,
                camera_center.1,
                radius,
                &engine.building_grid,
            )
            .unwrap()
        };
        app.handle_key_event(true, PhysicalKey::Code(KeyCode::Digit4));
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            tower_center_site,
            "a hotkey without a current map pointer preserves the old camera-center behavior"
        );

        app.clear_placement();
        {
            let engine = app.sim.engine.as_mut().unwrap();
            for terrain in &mut engine.state.map.terrain {
                *terrain = sow_core::map::MapTile::from_byte(0);
            }
            engine.refresh_building_grid();
        }
        app.input.hover_pointer = HoverPointer::Mouse;
        app.input.last_mouse_x = pointer_position.0;
        app.input.last_mouse_y = pointer_position.1;
        app.handle_key_event(true, PhysicalKey::Code(KeyCode::Digit5));
        assert!(app.ui.app.hud_state.building_placement.is_none());
        assert_eq!(
            app.ui.app.hud_state.map_feedback.as_ref().unwrap().text.key,
            "hud.build_no_space"
        );

        {
            let engine = app.sim.engine.as_mut().unwrap();
            for (index, terrain) in engine.state.map.terrain.iter_mut().enumerate() {
                let col = index as u32 % 64;
                *terrain = if (50..52).contains(&col) {
                    sow_core::map::MapTile::from_byte(0)
                } else {
                    sow_core::map::MapTile::from_byte(0x80)
                };
            }
            engine.refresh_building_grid();
        }
        app.input.camera_focus_target = None;
        app.input.camera_focus_waiting_for_input_release = false;
        app.ui.app.hud_state.building_placement = Some(BuildingPlacement {
            kind: sow_core::game::BuildingKind::City,
            tile_idx: 20 * 64 + 20,
            pending: false,
        });
        use crate::input::placement::PlacementDirection;
        for key in [
            KeyCode::ArrowUp,
            KeyCode::ArrowRight,
            KeyCode::ArrowDown,
            KeyCode::ArrowLeft,
        ] {
            let before = app.ui.app.hud_state.building_placement.unwrap().tile_idx;
            app.handle_key_event(true, PhysicalKey::Code(key));
            assert_ne!(
                app.ui.app.hud_state.building_placement.unwrap().tile_idx,
                before
            );
            assert!(
                !app.input.key_pan_up
                    && !app.input.key_pan_left
                    && !app.input.key_pan_right
                    && !app.input.key_pan_down
            );
        }
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            20 * 64 + 20
        );
        app.handle_key_event(true, PhysicalKey::Code(KeyCode::KeyW));
        assert!(app.input.key_pan_up);
        app.handle_key_event(false, PhysicalKey::Code(KeyCode::KeyW));
        app.ui
            .app
            .hud_state
            .building_placement
            .as_mut()
            .unwrap()
            .tile_idx = 0;
        app.nudge_building_placement(PlacementDirection::Left);
        app.nudge_building_placement(PlacementDirection::Up);
        assert_eq!(app.ui.app.hud_state.building_placement.unwrap().tile_idx, 0);
        app.nudge_building_placement(PlacementDirection::Right);
        app.nudge_building_placement(PlacementDirection::Down);
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            0,
            "a line without legal foundations never moves the preview"
        );
        app.ui
            .app
            .hud_state
            .building_placement
            .as_mut()
            .unwrap()
            .tile_idx = 20 * 64 + 48;
        app.nudge_building_placement(PlacementDirection::Right);
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            20 * 64 + 54,
            "arrows jump over water to the next legal foundation"
        );
        app.move_building_preview(50, 20);
        let snapped = app.ui.app.hud_state.building_placement.unwrap().tile_idx;
        assert!(
            sow_core::building::validate_structure_site(
                &app.sim.engine.as_ref().unwrap().state.map,
                1,
                sow_core::game::BuildingKind::City,
                snapped,
                &app.sim.engine.as_ref().unwrap().building_grid,
            )
            .is_ok(),
            "drag over water snaps to a legal foundation"
        );
        app.ui
            .app
            .hud_state
            .building_placement
            .as_mut()
            .unwrap()
            .tile_idx = 20 * 64 + 20;
        assert!(app.sim.offline_intents.is_empty());
        let left = ButtonSource::Mouse(MouseButton::Left);
        app.handle_pointer_button(true, left.clone(), 240.0, 240.0, true);
        app.handle_pointer_move(PointerSource::Mouse, 300.0, 240.0, true);
        app.handle_pointer_button(false, left.clone(), 300.0, 240.0, true);
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            20 * 64 + 25
        );
        assert!(app.sim.offline_intents.is_empty());
        let tile = app.ui.app.hud_state.building_placement.unwrap().tile_idx;
        app.handle_pointer_button(true, left.clone(), 40.0, 40.0, true);
        app.handle_pointer_move(PointerSource::Mouse, 80.0, 60.0, true);
        app.handle_pointer_button(false, left, 80.0, 60.0, true);
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            tile
        );
        assert_eq!(app.input.camera_x, 40.0);
        assert_eq!(app.input.camera_y, 20.0);
        let touch = |id| ButtonSource::Touch {
            finger_id: FingerId::from_raw(id),
            force: None,
        };
        let movement = |id| PointerSource::Touch {
            finger_id: FingerId::from_raw(id),
            force: None,
        };
        app.handle_pointer_button(true, touch(1), 340.0, 260.0, true);
        app.handle_pointer_move(movement(1), 352.0, 260.0, true);
        app.handle_pointer_button(false, touch(1), 352.0, 260.0, true);
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            20 * 64 + 26
        );
        app.handle_pointer_button(true, touch(1), 160.0, 140.0, true);
        app.handle_pointer_button(false, touch(1), 160.0, 140.0, true);
        let tile = 10 * 64 + 10;
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            tile
        );
        app.handle_pointer_button(true, touch(1), 160.0, 140.0, true);
        app.handle_pointer_button(true, touch(2), 260.0, 140.0, false);
        app.handle_pointer_move(movement(2), 280.0, 140.0, false);
        app.handle_pointer_move(movement(2), 300.0, 140.0, false);
        assert!(app.input.target_zoom > 12.0);
        app.handle_pointer_button(false, touch(2), 300.0, 140.0, false);
        app.handle_pointer_button(false, touch(1), 160.0, 140.0, true);
        app.cancel_pointer_gesture();
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            tile
        );
        app.ui
            .app
            .hud_state
            .building_placement
            .as_mut()
            .unwrap()
            .pending = true;
        app.nudge_building_placement(PlacementDirection::Right);
        assert_eq!(
            app.ui.app.hud_state.building_placement.unwrap().tile_idx,
            tile
        );
        app.confirm_building_placement();
        app.confirm_building_placement();
        assert!(app.sim.offline_intents.is_empty());
        app.ui
            .app
            .hud_state
            .building_placement
            .as_mut()
            .unwrap()
            .pending = false;
        app.clear_placement();
        assert!(app.ui.app.hud_state.building_placement.is_none());
        assert!(app.sim.offline_intents.is_empty());
        drop(app);
        wasm_bindgen_futures::JsFuture::from(crate::app::restore_tutorial_exit())
            .await
            .unwrap();
    }
}
