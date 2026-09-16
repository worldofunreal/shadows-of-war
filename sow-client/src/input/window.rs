use crate::app::SowApp;
use crate::{camera_zoom_lower_bound, camera_zoom_upper_bound};
use sow_ui_kit::ClientPhase;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

impl SowApp {
    pub fn handle_window_event(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                    if let Some(sp) = self.gfx.prev_sync_point.take() { let _ = render_ctx.context.wait_for(&sp, !0); }
                    if let Some(mut surface) = self.gfx.surface.take() { render_ctx.reset_command_encoder(); render_ctx.context.destroy_surface(&mut surface); }
                }
                event_loop.exit();
            }
            WindowEvent::SurfaceResized(size) => { self.apply_surface_resize(size, false); }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = self.gfx.window.as_ref() {
                    let vp = crate::viewport::Viewport::measure(window.as_ref());
                    if vp.wants_reconfigure(self) { self.apply_surface_resize(vp.physical, false); } else { vp.sync_to_app(self); }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => self.handle_key_event(event.state == ElementState::Pressed, event.physical_key),
            WindowEvent::PointerButton { state, button, position, primary, .. } => {
                self.handle_pointer_button(state == ElementState::Pressed, button, position.x, position.y, primary);
            }
            WindowEvent::PointerMoved { source, position, primary, .. } => {
                self.handle_pointer_move(source, position.x, position.y, primary);
            }
            WindowEvent::MouseWheel { delta, .. } => self.handle_wheel(delta),
            _ => {}
        }
    }

    fn handle_key_event(&mut self, pressed: bool, key: winit::keyboard::PhysicalKey) {
        if pressed && key == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F11) {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(window) = self.gfx.window.as_ref() {
                let fullscreen = window.fullscreen().is_none();
                self.ui.app.settings_state.is_fullscreen = fullscreen;
                window.set_fullscreen(fullscreen.then(|| winit::monitor::Fullscreen::Borderless(None)));
            }
        }
        let in_game = self.ui.app.phase == ClientPhase::Playing && self.ui.app.hud_state.sync_state.is_none();
        if !in_game || self.input.input_focused { self.input.key_pan_up = false; self.input.key_pan_down = false; self.input.key_pan_left = false; self.input.key_pan_right = false; return; }
        let winit::keyboard::PhysicalKey::Code(code) = key else { return; };
        match code {
            winit::keyboard::KeyCode::KeyW | winit::keyboard::KeyCode::ArrowUp => self.input.key_pan_up = pressed,
            winit::keyboard::KeyCode::KeyS | winit::keyboard::KeyCode::ArrowDown => self.input.key_pan_down = pressed,
            winit::keyboard::KeyCode::KeyA | winit::keyboard::KeyCode::ArrowLeft => self.input.key_pan_left = pressed,
            winit::keyboard::KeyCode::KeyD | winit::keyboard::KeyCode::ArrowRight => self.input.key_pan_right = pressed,
            _ => {}
        }
        if !pressed { return; }
        if code == winit::keyboard::KeyCode::KeyB {
            if let Some((col, row)) = self.mouse_to_tile(self.input.last_mouse_x, self.input.last_mouse_y) {
                let idx = (row * self.sim.map_w as i32 + col) as usize;
                let owner = self.gfx.map_renderer.as_ref().and_then(|mr| mr.owners.get(idx)).copied().unwrap_or(0);
                if owner != 0 && owner != self.sim.my_player_id.unwrap_or(0) {
                    self.send_intent(sow_core::protocol::GameplayIntent::LaunchFleet { target_tile: idx as u32, troops: Some(self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64) });
                }
            }
        }
        let building = match code {
            winit::keyboard::KeyCode::Digit1 | winit::keyboard::KeyCode::Numpad1 => Some(sow_core::game::BuildingKind::City),
            winit::keyboard::KeyCode::Digit2 | winit::keyboard::KeyCode::Numpad2 => Some(sow_core::game::BuildingKind::Factory),
            winit::keyboard::KeyCode::Digit3 | winit::keyboard::KeyCode::Numpad3 => Some(sow_core::game::BuildingKind::Port),
            winit::keyboard::KeyCode::Digit4 | winit::keyboard::KeyCode::Numpad4 => Some(sow_core::game::BuildingKind::Bunker),
            _ => None,
        };
        if let Some(kind) = building {
            self.ui.app.hud_state.selected_building_kind = (self.ui.app.hud_state.selected_building_kind != Some(kind)).then_some(kind);
            self.ui.app.hud_state.selected_nuke_kind = None;
        }
        if code == winit::keyboard::KeyCode::Digit0 || code == winit::keyboard::KeyCode::Numpad0 {
            let kind = sow_core::game::NukeKind::AtomBomb;
            self.ui.app.hud_state.selected_nuke_kind = (self.ui.app.hud_state.selected_nuke_kind != Some(kind)).then_some(kind);
            self.ui.app.hud_state.selected_building_kind = None;
        }
        if code == winit::keyboard::KeyCode::Escape || code == winit::keyboard::KeyCode::KeyQ { self.ui.app.hud_state.selected_building_kind = None; self.ui.app.hud_state.selected_nuke_kind = None; }
    }

    fn handle_pointer_button(&mut self, pressed: bool, button: winit::event::ButtonSource, x: f64, y: f64, primary: bool) {
        if primary { self.input.last_mouse_x = x; self.input.last_mouse_y = y; }
        let left = matches!(button, winit::event::ButtonSource::Mouse(MouseButton::Left)) || matches!(button, winit::event::ButtonSource::Touch { .. });
        let right = matches!(button, winit::event::ButtonSource::Mouse(MouseButton::Right));
        if let winit::event::ButtonSource::Touch { finger_id, .. } = button {
            let id = finger_id.into_raw() as u64;
            if pressed { self.input.active_touches.insert(id, (x, y)); } else { self.input.active_touches.remove(&id); self.input.last_pinch_state = None; }
        }
        if left {
            if pressed {
                self.input.dragging = self.ui.app.hud_state.selected_building_kind.is_none() && self.ui.app.hud_state.selected_nuke_kind.is_none();
                self.input.map_touch_start = Some((web_time::Instant::now(), x, y));
            } else {
                self.input.dragging = false;
                if let Some((started, sx, sy)) = self.input.map_touch_start.take() {
                    let quick = started.elapsed().as_millis() < 300;
                    let still = (x - sx).powi(2) + (y - sy).powi(2) <= 400.0;
                    if quick && still && self.ui.app.phase == ClientPhase::Playing { self.handle_map_click(sx, sy); }
                }
            }
        } else if right && !pressed && self.ui.app.phase == ClientPhase::Playing {
            self.ui.app.hud_state.selected_building_kind = None;
            self.ui.app.hud_state.selected_nuke_kind = None;
        }
    }

    fn handle_pointer_move(&mut self, source: winit::event::PointerSource, x: f64, y: f64, primary: bool) {
        if let winit::event::PointerSource::Touch { finger_id, .. } = source { self.input.active_touches.insert(finger_id.into_raw() as u64, (x, y)); }
        let touch = matches!(source, winit::event::PointerSource::Touch { .. });
        if self.input.active_touches.len() >= 2 {
            self.input.dragging = false;
            let mut points = self.input.active_touches.values(); let (x1, y1) = *points.next().unwrap(); let (x2, y2) = *points.next().unwrap();
            let dx = x1 - x2; let dy = y1 - y2; let distance = (dx * dx + dy * dy).sqrt(); let cx = (x1 + x2) * 0.5; let cy = (y1 + y2) * 0.5;
            if let Some((last_distance, last_x, last_y)) = self.input.last_pinch_state { self.input.camera_x += (cx - last_x) as f32; self.input.camera_y += (cy - last_y) as f32; self.process_camera_zoom(1.0 + ((distance - last_distance) as f32 * 0.005), cx as f32, cy as f32); }
            self.input.last_pinch_state = Some((distance, cx, cy));
        } else if primary && self.input.dragging && !touch {
            self.input.camera_x += (x - self.input.last_mouse_x) as f32; self.input.camera_y += (y - self.input.last_mouse_y) as f32; self.clamp_camera_to_map();
        }
        if primary { self.input.last_mouse_x = x; self.input.last_mouse_y = y; }
    }

    fn handle_wheel(&mut self, delta: MouseScrollDelta) {
        if !self.input.active_touches.is_empty() || self.ui.app.phase != ClientPhase::Playing { return; }
        let scroll = match delta { MouseScrollDelta::LineDelta(x, y) => if y.abs() >= x.abs() { y } else { x }, MouseScrollDelta::PixelDelta(pos) => { let x = pos.x as f32 / 50.0; let y = pos.y as f32 / 50.0; if y.abs() >= x.abs() { y } else { x } } };
        let zmin = camera_zoom_lower_bound(self.input.screen_w, self.input.screen_h, self.sim.map_w, self.sim.map_h);
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h).max(zmin);
        self.input.target_zoom = (self.input.target_zoom * (1.0 + scroll * 0.15)).clamp(zmin, zmax);
    }
}
