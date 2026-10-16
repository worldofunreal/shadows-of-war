pub mod dev_config {
    use sow_render::shader_params::{self, PARAM_CAPACITY, ParamKind};
    use std::sync::{LazyLock, RwLock};

    #[inline]
    fn bool_value(value: bool) -> f32 {
        if value { 1.0 } else { 0.0 }
    }

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
        pub vfx_dialog_border: bool,
        pub vfx_energy_flow: bool,
        pub vfx_heartbeat: bool,
        pub fog_of_war: bool,
        pub vfx_ambient_grade: bool,
        pub vfx_screen_vignette: bool,
        pub vfx_holo_grid: bool,
        pub vfx_movers: bool,
        pub vfx_tutorial_highlight: bool,
        pub vfx_mover_trails: bool,
        pub vfx_click_markers: bool,
        pub vfx_railways: bool,
        pub vfx_fleet_blink: bool,
        pub vfx_tower: bool,
        pub vfx_tower_range: bool,
        pub vfx_attack_badges: bool,
        pub vfx_nuke_preview: bool,
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
        pub shader_params: [[f32; 4]; PARAM_CAPACITY],
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
                vfx_dialog_border: true,
                vfx_energy_flow: true,
                vfx_heartbeat: true,
                fog_of_war: false,
                vfx_ambient_grade: true,
                vfx_screen_vignette: true,
                vfx_holo_grid: true,
                vfx_movers: true,
                vfx_tutorial_highlight: true,
                vfx_mover_trails: true,
                vfx_click_markers: true,
                vfx_railways: true,
                vfx_fleet_blink: true,
                vfx_tower: true,
                vfx_tower_range: true,
                vfx_attack_badges: true,
                vfx_nuke_preview: true,
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
                shader_params: shader_params::shader_defaults(),
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

        pub fn value(&self, id: &str) -> [f32; 4] {
            if let Some(slot) = shader_params::shader_slot(id) {
                return self.shader_params[slot];
            }
            match id {
                "thickness" => [self.thickness, 0.0, 0.0, 0.0],
                "darkness" => [self.darkness, 0.0, 0.0, 0.0],
                "shore_thickness" => [self.shore_thickness, 0.0, 0.0, 0.0],
                "shore_darkness" => [self.shore_darkness, 0.0, 0.0, 0.0],
                "conquest_duration" => [self.conquest_duration, 0.0, 0.0, 0.0],
                "territory_opacity" => [self.territory_opacity, 0.0, 0.0, 0.0],
                "blend_mode" => [self.blend_mode, 0.0, 0.0, 0.0],
                "font_size_scale" => [self.font_size_scale, 0.0, 0.0, 0.0],
                "font_face_dilate" => [self.font_face_dilate, 0.0, 0.0, 0.0],
                "font_outline_thickness" => [self.font_outline_thickness, 0.0, 0.0, 0.0],
                "font_shadow_y" => [self.font_shadow_y, 0.0, 0.0, 0.0],
                "font_underlay_softness" => [self.font_underlay_softness, 0.0, 0.0, 0.0],
                "font_char_spacing" => [self.font_char_spacing, 0.0, 0.0, 0.0],
                "vfx_conquer" => [bool_value(self.vfx_conquer), 0.0, 0.0, 0.0],
                "vfx_border_breathe" => [bool_value(self.vfx_border_breathe), 0.0, 0.0, 0.0],
                "vfx_dialog_border" => [bool_value(self.vfx_dialog_border), 0.0, 0.0, 0.0],
                "vfx_energy_flow" => [bool_value(self.vfx_energy_flow), 0.0, 0.0, 0.0],
                "vfx_heartbeat" => [bool_value(self.vfx_heartbeat), 0.0, 0.0, 0.0],
                "fog_of_war" => [bool_value(self.fog_of_war), 0.0, 0.0, 0.0],
                "vfx_ambient_grade" => [bool_value(self.vfx_ambient_grade), 0.0, 0.0, 0.0],
                "vfx_screen_vignette" => [bool_value(self.vfx_screen_vignette), 0.0, 0.0, 0.0],
                "vfx_holo_grid" => [bool_value(self.vfx_holo_grid), 0.0, 0.0, 0.0],
                "vfx_movers" => [bool_value(self.vfx_movers), 0.0, 0.0, 0.0],
                "vfx_tutorial_highlight" => {
                    [bool_value(self.vfx_tutorial_highlight), 0.0, 0.0, 0.0]
                }
                "vfx_mover_trails" => [bool_value(self.vfx_mover_trails), 0.0, 0.0, 0.0],
                "vfx_click_markers" => [bool_value(self.vfx_click_markers), 0.0, 0.0, 0.0],
                "vfx_attack_badges" => [bool_value(self.vfx_attack_badges), 0.0, 0.0, 0.0],
                "vfx_world_buildings" => [bool_value(self.vfx_world_buildings), 0.0, 0.0, 0.0],
                "vfx_bot_avatars" => [bool_value(self.vfx_bot_avatars), 0.0, 0.0, 0.0],
                "vfx_nameplate_names" => [bool_value(self.vfx_nameplate_names), 0.0, 0.0, 0.0],
                "vfx_nameplate_troops" => [bool_value(self.vfx_nameplate_troops), 0.0, 0.0, 0.0],
                _ => [0.0; 4],
            }
        }

        pub fn assign(&mut self, id: &str, value: [f32; 4]) -> bool {
            if let Some(slot) = shader_params::shader_slot(id) {
                self.shader_params[slot] = value;
                return true;
            }
            match id {
                "thickness" => self.thickness = value[0],
                "darkness" => self.darkness = value[0],
                "shore_thickness" => self.shore_thickness = value[0],
                "shore_darkness" => self.shore_darkness = value[0],
                "conquest_duration" => self.conquest_duration = value[0],
                "territory_opacity" => self.territory_opacity = value[0],
                "blend_mode" => self.blend_mode = value[0],
                "font_size_scale" => self.font_size_scale = value[0],
                "font_face_dilate" => self.font_face_dilate = value[0],
                "font_outline_thickness" => self.font_outline_thickness = value[0],
                "font_shadow_y" => self.font_shadow_y = value[0],
                "font_underlay_softness" => self.font_underlay_softness = value[0],
                "font_char_spacing" => self.font_char_spacing = value[0],
                "vfx_conquer" => self.vfx_conquer = value[0] != 0.0,
                "vfx_border_breathe" => self.vfx_border_breathe = value[0] != 0.0,
                "vfx_dialog_border" => self.vfx_dialog_border = value[0] != 0.0,
                "vfx_energy_flow" => self.vfx_energy_flow = value[0] != 0.0,
                "vfx_heartbeat" => self.vfx_heartbeat = value[0] != 0.0,
                "fog_of_war" => self.fog_of_war = value[0] != 0.0,
                "vfx_ambient_grade" => self.vfx_ambient_grade = value[0] != 0.0,
                "vfx_screen_vignette" => self.vfx_screen_vignette = value[0] != 0.0,
                "vfx_holo_grid" => self.vfx_holo_grid = value[0] != 0.0,
                "vfx_movers" => self.vfx_movers = value[0] != 0.0,
                "vfx_tutorial_highlight" => self.vfx_tutorial_highlight = value[0] != 0.0,
                "vfx_mover_trails" => self.vfx_mover_trails = value[0] != 0.0,
                "vfx_click_markers" => self.vfx_click_markers = value[0] != 0.0,
                "vfx_attack_badges" => self.vfx_attack_badges = value[0] != 0.0,
                "vfx_world_buildings" => self.vfx_world_buildings = value[0] != 0.0,
                "vfx_bot_avatars" => self.vfx_bot_avatars = value[0] != 0.0,
                "vfx_nameplate_names" => self.vfx_nameplate_names = value[0] != 0.0,
                "vfx_nameplate_troops" => self.vfx_nameplate_troops = value[0] != 0.0,
                _ => return false,
            }
            true
        }

        pub fn reset_section(&mut self, section: &str) {
            let defaults = Self::default();
            for param in shader_params::PARAMS
                .iter()
                .filter(|param| param.section == section)
            {
                self.assign(param.id, defaults.value(param.id));
            }
        }

        pub fn reset_field(&mut self, field: &str) -> bool {
            let Some(param) = shader_params::definition(field) else {
                return false;
            };
            self.assign(param.id, Self::default().value(param.id))
        }

        fn switch_ids(group: &str) -> &'static [&'static str] {
            shader_params::PERF_SWITCHES
                .iter()
                .find(|(name, _)| *name == group)
                .map_or(&[], |(_, ids)| ids)
        }

        pub fn set_switch_group(&mut self, group: &str, on: bool) {
            let defaults = Self::default();
            for id in Self::switch_ids(group) {
                let value = if on { defaults.value(id) } else { [0.0; 4] };
                self.assign(id, value);
            }
        }

        /// "on" = every id at its default, "off" = every id zero, else "mixed".
        pub fn switch_group_state(&self, group: &str) -> &'static str {
            let defaults = Self::default();
            let ids = Self::switch_ids(group);
            if ids.iter().all(|id| self.value(id)[0] == defaults.value(id)[0]) {
                "on"
            } else if ids.iter().all(|id| self.value(id)[0] == 0.0) {
                "off"
            } else {
                "mixed"
            }
        }

        pub fn fingerprint(&self) -> Vec<u32> {
            let mut fingerprint = Vec::with_capacity(shader_params::PARAMS.len() * 4);
            for param in shader_params::PARAMS {
                let value = self.value(param.id);
                let channels = if matches!(param.kind, ParamKind::Color | ParamKind::Vector) {
                    4
                } else {
                    1
                };
                fingerprint.extend(value[..channels].iter().map(|v| v.to_bits()));
            }
            fingerprint
        }
    }
}
