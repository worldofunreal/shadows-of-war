use crate::app::SowApp;
use crate::render::world::movers::world_to_tile;
use crate::{camera_zoom_lower_bound, camera_zoom_upper_bound};
use blade_graphics as gpu;

pub(crate) fn clamp_camera_offset(screen_size: f32, map_size: f32, offset: f32) -> f32 {
    if map_size <= screen_size {
        (screen_size - map_size) * 0.5
    } else {
        offset.clamp(screen_size - map_size, 0.0)
    }
}

impl SowApp {
    pub(crate) fn record_tutorial_zoom(&mut self, zoom_delta: f32) {
        if !self.ui.tutorial_active || !self.net.is_offline || zoom_delta.abs() < f32::EPSILON {
            return;
        }
        let count = if zoom_delta > 0.0 {
            &mut self.input.tutorial_zoom_in_events
        } else {
            &mut self.input.tutorial_zoom_out_events
        };
        *count = count.saturating_add(1);
    }

    pub(crate) fn record_tutorial_camera_drag(&mut self) {
        if self.ui.tutorial_active && self.ui.tutorial_camera_only && self.net.is_offline {
            self.input.tutorial_camera_drag_events =
                self.input.tutorial_camera_drag_events.saturating_add(1);
        }
    }

    pub(crate) fn record_tutorial_camera_key_pan(&mut self) {
        if self.ui.tutorial_active && self.ui.tutorial_camera_only && self.net.is_offline {
            self.input.tutorial_camera_key_pan_events =
                self.input.tutorial_camera_key_pan_events.saturating_add(1);
        }
    }

    pub(crate) fn mouse_to_tile(&self, x: f64, y: f64) -> Option<(i32, i32)> {
        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;

        let (col, row) = world_to_tile(world_x, world_y);

        if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
            Some((col, row))
        } else {
            None
        }
    }

    pub(crate) fn apply_surface_resize(
        &mut self,
        physical_size: winit::dpi::PhysicalSize<u32>,
        force_reconfigure: bool,
    ) {
        if physical_size.width == 0 || physical_size.height == 0 {
            return;
        }
        // Bypass winit's initial 1.0 scale factor to avoid a zoom mismatch.
        let sf = web_sys::window()
            .map(|window| window.device_pixel_ratio() as f32)
            .unwrap_or(1.0)
            .max(0.01);
        let vp = crate::viewport::Viewport {
            physical: physical_size,
            scale_factor: sf,
        };
        let needs_reconfigure = vp.wants_reconfigure(self) || force_reconfigure;

        if needs_reconfigure && let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
            if let Some(sp) = self.gfx.prev_sync_point.take() {
                let _ = render_ctx.context.wait_for(&sp, !0);
            }
            if let Some(ref mut s) = self.gfx.surface {
                crate::web_canvas::set_canvas_backing_store_size(
                    physical_size.width,
                    physical_size.height,
                );
                render_ctx.context.reconfigure_surface(
                    s,
                    gpu::SurfaceConfig {
                        size: gpu::Extent {
                            width: physical_size.width,
                            height: physical_size.height,
                            depth: 1,
                        },
                        usage: gpu::TextureUsage::TARGET,
                        display_sync: gpu::DisplaySync::Tear,
                        color_space: gpu::ColorSpace::Linear,
                        ..Default::default()
                    },
                );
            }
        }

        if needs_reconfigure {
            self.gfx.configured_physical = physical_size;
        }

        crate::viewport::Viewport::from_configured(self, sf).sync_to_app(self);
        let zmin = self.zoom_floor();
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h).max(zmin);
        self.input.camera_zoom = self.input.camera_zoom.clamp(zmin, zmax);
        self.input.target_zoom = self.input.target_zoom.clamp(zmin, zmax);
        self.clamp_camera_to_map();

        if needs_reconfigure && let Some(win) = self.gfx.window.as_ref() {
            win.request_redraw();
        }
    }
    /// Zoom floor: fit-to-screen normally, or the old fixed `CAMERA_MIN_ZOOM`
    /// when the free zoom-out setting lets the map shrink below the viewport.
    pub(crate) fn zoom_floor(&self) -> f32 {
        if self.ui.app.settings_state.free_zoom_out {
            return crate::CAMERA_MIN_ZOOM;
        }
        camera_zoom_lower_bound(
            self.input.screen_w,
            self.input.screen_h,
            self.sim.map_w,
            self.sim.map_h,
        )
    }

    pub(crate) fn process_camera_zoom(&mut self, zoom_factor: f32, cx: f32, cy: f32) {
        let old_zoom = self.input.camera_zoom;
        let zmin = self.zoom_floor();
        self.input.camera_zoom *= zoom_factor;
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h).max(zmin);
        self.input.camera_zoom = self.input.camera_zoom.clamp(zmin, zmax);
        self.input.target_zoom = self.input.camera_zoom;
        let map_x = (cx - self.input.camera_x) / old_zoom;
        let map_y = (cy - self.input.camera_y) / old_zoom;
        self.input.camera_x = cx - map_x * self.input.camera_zoom;
        self.input.camera_y = cy - map_y * self.input.camera_zoom;
        self.clamp_camera_to_map();
        self.record_tutorial_zoom(self.input.camera_zoom - old_zoom);
    }

    /// Keep the visible rect inside `[0,map_w]x[0,map_h]` so void is never shown,
    /// unless the free zoom-out setting is on (then only the zoom bounds apply).
    /// If the map is smaller than the viewport at current zoom, center it.
    /// All coords are physical px: `screen = world*zoom + camera`.
    /// Also enforces zoom in `[lower,upper]` so the map always covers the viewport.
    pub(crate) fn clamp_camera_to_map(&mut self) {
        if self.sim.map_w == 0 || self.sim.map_h == 0 {
            return;
        }
        let zmin = self.zoom_floor();
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h).max(zmin);
        self.input.camera_zoom = self.input.camera_zoom.clamp(zmin, zmax);
        self.input.target_zoom = self.input.target_zoom.clamp(zmin, zmax);
        if self.ui.app.settings_state.free_zoom_out {
            return;
        }
        let z = self.input.camera_zoom;
        let mw = self.sim.map_w as f32 * z;
        let mh = self.sim.map_h as f32 * z;
        let sw = self.input.screen_w;
        let sh = self.input.screen_h;
        self.input.camera_x = clamp_camera_offset(sw, mw, self.input.camera_x);
        self.input.camera_y = clamp_camera_offset(sh, mh, self.input.camera_y);
    }
}
