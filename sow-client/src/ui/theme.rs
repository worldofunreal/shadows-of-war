pub mod dev_config {
    use std::sync::{LazyLock, RwLock};

    #[derive(Clone, Debug)]
    pub struct DevConfig {
        pub thickness: f32,
        pub darkness: f32,
        pub shore_thickness: f32,
        pub shore_darkness: f32,
        pub conquest_duration: f32,
        pub territory_opacity: f32,
        pub blend_mode: f32,
        pub vfx_conquer: bool,
        pub vfx_border_breathe: bool,
        pub vfx_energy_flow: bool,
        pub vfx_heartbeat: bool,
        pub vfx_war_fog: bool,
        pub fog_of_war: bool,
        pub vfx_fallout: bool,
        pub vfx_ambient_grade: bool,
        pub vfx_holo_grid: bool,
        pub vfx_mover_trails: bool,
        pub vfx_click_markers: bool,
        pub vfx_railways: bool,
        pub vfx_fleet_blink: bool,
        pub vfx_tower: bool,
        pub vfx_tower_range: bool,
        pub vfx_attack_badges: bool,
        pub vfx_nuke_preview: bool,
        pub vfx_status_emojis: bool,
        pub vfx_world_buildings: bool,
        pub vfx_bot_avatars: bool,
        pub vfx_nameplate_names: bool,
        pub vfx_nameplate_troops: bool,
        pub font_face_dilate: f32,
        pub font_outline_thickness: f32,
        pub font_shadow_y: f32,
        pub font_underlay_softness: f32,
        pub font_char_spacing: f32,
        pub font_size_scale: f32,
        pub bunker_laser_target: bool,
        pub bunker_laser_arc: bool,
        pub bunker_laser_scatter: bool,
    }

    impl Default for DevConfig {
        fn default() -> Self {
            Self {
                thickness: 0.5,
                darkness: 0.35,
                shore_thickness: 0.35,
                shore_darkness: 1.0,
                conquest_duration: 1.5,
                territory_opacity: 1.0,
                blend_mode: 0.0,
                vfx_conquer: true,
                vfx_border_breathe: true,
                vfx_energy_flow: true,
                vfx_heartbeat: true,
                vfx_war_fog: true,
                fog_of_war: false,
                vfx_fallout: true,
                vfx_ambient_grade: true,
                vfx_holo_grid: true,
                vfx_mover_trails: true,
                vfx_click_markers: true,
                vfx_railways: true,
                vfx_fleet_blink: true,
                vfx_tower: true,
                vfx_tower_range: true,
                vfx_attack_badges: true,
                vfx_nuke_preview: true,
                vfx_status_emojis: true,
                vfx_world_buildings: true,
                vfx_bot_avatars: true,
                vfx_nameplate_names: true,
                vfx_nameplate_troops: true,
                font_face_dilate: -0.2,
                font_outline_thickness: 1.4,
                font_shadow_y: 2.0,
                font_underlay_softness: 0.1,
                font_char_spacing: 0.95,
                font_size_scale: 2.0,
                bunker_laser_target: true,
                bunker_laser_arc: true,
                bunker_laser_scatter: false,
            }
        }
    }

    static GLOBAL: LazyLock<RwLock<DevConfig>> =
        LazyLock::new(|| RwLock::new(DevConfig::default()));
    impl DevConfig {
        pub fn get() -> Self {
            GLOBAL.read().expect("dev config lock").clone()
        }
        pub fn set(value: Self) {
            *GLOBAL.write().expect("dev config lock") = value;
        }
        pub fn update(f: impl FnOnce(&mut Self)) {
            f(&mut GLOBAL.write().expect("dev config lock"));
        }
    }
}
