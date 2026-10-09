pub struct SettingsState {
    pub music_volume: f32,
    pub mute_all: bool,
    pub reduced_motion: bool,
    pub free_zoom_out: bool,
    pub sticky_building_mode: bool,
    pub show_fps_ping: bool,
    pub show_dev_tools: bool,
}

impl SettingsState {
    #[cfg(target_arch = "wasm32")]
    const SHOW_FPS_PING_KEY: &'static str = "sow_settings_v1_show_fps_ping";
    #[cfg(target_arch = "wasm32")]
    const SHOW_DEV_TOOLS_KEY: &'static str = "sow_settings_v1_show_dev_tools";

    pub fn load_local() -> Self {
        let mut settings = Self::default();
        #[cfg(target_arch = "wasm32")]
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            settings.show_fps_ping = storage
                .get_item(Self::SHOW_FPS_PING_KEY)
                .ok()
                .flatten()
                .and_then(|value| value.parse().ok())
                .unwrap_or(settings.show_fps_ping);
            settings.show_dev_tools = storage
                .get_item(Self::SHOW_DEV_TOOLS_KEY)
                .ok()
                .flatten()
                .and_then(|value| value.parse().ok())
                .unwrap_or(settings.show_dev_tools);
        }
        settings
    }

    pub fn persist_local(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            for (key, value) in [
                (Self::SHOW_FPS_PING_KEY, self.show_fps_ping),
                (Self::SHOW_DEV_TOOLS_KEY, self.show_dev_tools),
            ] {
                if let Err(error) = storage.set_item(key, if value { "true" } else { "false" }) {
                    log::warn!("settings: localStorage save failed for {key}: {error:?}");
                }
            }
        }
    }
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            music_volume: 0.8,
            mute_all: false,
            reduced_motion: false,
            free_zoom_out: true,
            sticky_building_mode: false,
            show_fps_ping: true,
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

    #[test]
    fn fps_ping_is_visible_by_default_and_dev_tools_are_opt_in() {
        let settings = super::SettingsState::default();
        assert!(settings.show_fps_ping);
        assert!(!settings.show_dev_tools);
    }
}
