pub struct SettingsState {
    pub music_volume: f32,
    pub mute_all: bool,
    pub reduced_motion: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            music_volume: 0.8,
            mute_all: false,
            reduced_motion: false,
        }
    }
}
