use crate::render::world::utils::{get_building_icon_size, get_level_str};
use sow_ui_kit::theme::dev_config::DevConfig;

pub(super) const BUILDING_CULL_FLOOR: f32 = 0.25;

const LOD_SCALE_START_ZOOM: f32 = 1.0;
const LOD_SCALE_ZOOM_RANGE: f32 = 9.0;
const MIN_MARKER_SIZE: f32 = 14.0;
const CLUSTER_TARGET_SCREEN_SIZE: f32 = 40.0;
const LEVEL_FONT_RATIO: f32 = 0.58;

#[derive(Clone, Copy, Debug)]
pub(super) struct BuildingLod {
    pub final_scale: f32,
    pub cluster_cell_size: f32,
    pub compact: bool,
}

impl BuildingLod {
    pub(super) fn for_zoom(zoom_scaled: f32, building_scale: f32, clamp_emoji_zoom: bool) -> Self {
        let zoom_factor =
            ((zoom_scaled - LOD_SCALE_START_ZOOM) / LOD_SCALE_ZOOM_RANGE).clamp(0.0, 1.0);
        let lod_scale = 0.5 + 0.5 * zoom_factor;
        let final_scale = building_scale * lod_scale;
        let natural_marker_size = icon_size(zoom_scaled, clamp_emoji_zoom) * final_scale;
        let compact = natural_marker_size < MIN_MARKER_SIZE;
        let cluster_cell_size = if compact {
            (CLUSTER_TARGET_SCREEN_SIZE / zoom_scaled.max(BUILDING_CULL_FLOOR)).max(1.0)
        } else {
            1.0
        };

        Self {
            final_scale,
            cluster_cell_size,
            compact,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BuildingVisualMetrics {
    pub marker_size: f32,
    pub level_font_size: f32,
    pub level_offset: egui::Vec2,
    pub final_scale: f32,
}

impl BuildingVisualMetrics {
    pub(super) fn for_building(
        lod: BuildingLod,
        zoom_scaled: f32,
        count: usize,
        dev: &DevConfig,
    ) -> Self {
        let icon_size = icon_size(zoom_scaled, dev.clamp_emoji_zoom);
        let natural_size = if count > 1 {
            icon_size * 1.2
        } else {
            icon_size
        } * lod.final_scale;
        let marker_size = if lod.compact {
            natural_size.max(MIN_MARKER_SIZE)
        } else {
            natural_size
        };
        let level_font_size = (marker_size * LEVEL_FONT_RATIO).clamp(8.0, 18.0).round();

        Self {
            marker_size,
            level_font_size,
            level_offset: egui::vec2(marker_size * 0.45, -marker_size * 0.45),
            final_scale: lod.final_scale,
        }
    }
}

pub(super) fn building_level_label(level: u8, count: usize) -> String {
    if count > 1 {
        format!("{} × {}", get_level_str(level), count)
    } else {
        get_level_str(level).to_owned()
    }
}

fn icon_size(zoom_scaled: f32, clamp_emoji_zoom: bool) -> f32 {
    let raw = get_building_icon_size(zoom_scaled);
    if clamp_emoji_zoom {
        raw.clamp(8.0, 50.0)
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_lod_uses_a_screen_sized_cluster() {
        let lod = BuildingLod::for_zoom(1.0, 0.5, false);

        assert!(lod.compact);
        assert!((lod.cluster_cell_size - CLUSTER_TARGET_SCREEN_SIZE).abs() < f32::EPSILON);

        let metrics = BuildingVisualMetrics::for_building(lod, 1.0, 1, &DevConfig::default());
        assert_eq!(metrics.marker_size, MIN_MARKER_SIZE);
        assert_eq!(metrics.level_font_size, 8.0);
        assert_eq!(metrics.level_offset.x, metrics.marker_size * 0.45);
        assert_eq!(metrics.level_offset.y, -metrics.marker_size * 0.45);
    }

    #[test]
    fn close_lod_keeps_natural_size_and_stops_clustering() {
        let lod = BuildingLod::for_zoom(20.0, 0.5, false);

        assert!(!lod.compact);
        assert_eq!(lod.cluster_cell_size, 1.0);
        assert_eq!(lod.final_scale, 0.5);
    }

    #[test]
    fn cluster_label_keeps_level_and_count_together() {
        assert_eq!(building_level_label(3, 1), "3");
        assert_eq!(building_level_label(3, 7), "3 × 7");
    }
}
