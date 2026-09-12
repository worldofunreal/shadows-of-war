pub mod interact;
pub mod world;

mod frame;
mod surface;
pub use sow_render as gpu;

pub(crate) fn dev_text_style(
    dev: &sow_ui_kit::theme::dev_config::DevConfig,
    sf: f32,
    outline_color: [f32; 4],
) -> gpu::TextPaintStyle {
    gpu::TextPaintStyle {
        face_dilate: dev.font_face_dilate * sf,
        outline: gpu::OutlineStyle {
            color: outline_color,
            thickness: dev.font_outline_thickness * sf,
            shadow_y: dev.font_shadow_y * sf,
        },
        underlay_softness: dev.font_underlay_softness * sf,
    }
}

pub(crate) fn dev_emoji_outline(
    dev: &sow_ui_kit::theme::dev_config::DevConfig,
    sf: f32,
    color: [f32; 4],
) -> gpu::OutlineStyle {
    gpu::OutlineStyle {
        color,
        thickness: dev.font_outline_thickness * sf,
        shadow_y: dev.font_shadow_y * sf,
    }
}
