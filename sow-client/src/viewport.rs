//! Unified browser viewport sizing.
//!
//! Physical pixels come from the GPU swapchain after reconfigure (`configured_physical`).
//! Window `surface_size()` is the resize *request*; layout and drawing use swapchain extent.

use winit::dpi::PhysicalSize;

use crate::app::SowApp;

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub physical: PhysicalSize<u32>,
    pub scale_factor: f32,
}

impl Viewport {
    pub fn measure(_win: &dyn winit::window::Window) -> Self {
        let physical = {
            let (w, h) = crate::web_canvas::physical_viewport_size();
            PhysicalSize::new(w, h)
        };

        // winit's initial scale factor can be 1.0 on web; query the browser directly.
        let scale_factor = crate::web_canvas::device_pixel_ratio() as f32;
        Self::from_physical(physical, scale_factor.max(0.01))
    }

    pub fn from_physical(physical: PhysicalSize<u32>, scale_factor: f32) -> Self {
        let sf = scale_factor.max(0.01);
        Self {
            physical,
            scale_factor: sf,
        }
    }

    pub fn from_configured(app: &SowApp, scale_factor: f32) -> Self {
        Self::from_physical(app.gfx.configured_physical, scale_factor)
    }

    /// Window size differs from the last successful swapchain reconfigure.
    pub fn wants_reconfigure(&self, app: &SowApp) -> bool {
        if self.physical.width == 0 || self.physical.height == 0 {
            return false;
        }
        self.physical.width != app.gfx.configured_physical.width
            || self.physical.height != app.gfx.configured_physical.height
    }

    pub fn sync_to_app(&self, app: &mut SowApp) {
        app.input.screen_w = self.physical.width as f32;
        app.input.screen_h = self.physical.height as f32;
    }
}

pub fn sync_wasm_window(app: &SowApp, win: &dyn winit::window::Window) {
    let (w, h) = crate::web_canvas::canvas_logical_size();
    // ponytail: query device_pixel_ratio directly as winit scale_factor is 1.0 initially
    let sf = crate::web_canvas::device_pixel_ratio();
    let expected_w = (w * sf) as u32;
    let expected_h = (h * sf) as u32;

    // ±1px tolerance: physical = logical × dpr can land on a fractional value, so an exact
    // compare flip-flops every frame on fractional-DPI displays and churns
    // request_surface_size. Restores dde7d6f's known-good `abs_diff > 1` guard.
    if app.gfx.configured_physical.width.abs_diff(expected_w) > 1
        || app.gfx.configured_physical.height.abs_diff(expected_h) > 1
    {
        let _ = win.request_surface_size(winit::dpi::LogicalSize::new(w, h).into());
    }
}
