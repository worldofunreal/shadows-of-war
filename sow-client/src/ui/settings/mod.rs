pub struct SettingsState {
    pub music_volume: f32,
    pub mute_all: bool,
    pub reduced_motion: bool,
    pub free_zoom_out: bool,
    pub sticky_building_mode: bool,
    pub show_dev_tools: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            music_volume: 0.8,
            mute_all: false,
            reduced_motion: false,
            free_zoom_out: true,
            sticky_building_mode: false,
            show_dev_tools: false,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sticky_building_mode_defaults_to_one_shot_placement() {
        assert!(!super::SettingsState::default().sticky_building_mode);
    }
}
