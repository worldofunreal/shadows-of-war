pub struct SettingsState {
    pub music_volume: f32,
    pub mute_all: bool,
    pub reduced_motion: bool,
    reduced_motion_is_explicit: bool,
    pub free_zoom_out: bool,
    pub show_fps_ping: bool,
    pub show_dev_tools: bool,
}

impl SettingsState {
    #[cfg(target_arch = "wasm32")]
    const MUSIC_VOLUME_KEY: &'static str = "sow_settings_v1_music_volume";
    #[cfg(target_arch = "wasm32")]
    const MUTE_ALL_KEY: &'static str = "sow_settings_v1_mute_all";
    #[cfg(target_arch = "wasm32")]
    const REDUCED_MOTION_KEY: &'static str = "sow_settings_v1_reduced_motion";
    #[cfg(target_arch = "wasm32")]
    const FREE_ZOOM_OUT_KEY: &'static str = "sow_settings_v1_free_zoom_out";
    #[cfg(target_arch = "wasm32")]
    const SHOW_FPS_PING_KEY: &'static str = "sow_settings_v1_show_fps_ping";
    #[cfg(target_arch = "wasm32")]
    const SHOW_DEV_TOOLS_KEY: &'static str = "sow_settings_v1_show_dev_tools";

    pub fn load_local() -> Self {
        let mut settings = Self::default();
        #[cfg(target_arch = "wasm32")]
        if let Some(window) = web_sys::window() {
            let storage = window.local_storage().ok().flatten();
            if let Some(storage) = storage.as_ref() {
                settings.music_volume = storage
                    .get_item(Self::MUSIC_VOLUME_KEY)
                    .ok()
                    .flatten()
                    .and_then(|value| value.parse::<f32>().ok())
                    .filter(|value| value.is_finite())
                    .map(|value| value.clamp(0.0, 1.0))
                    .unwrap_or(settings.music_volume);
                settings.mute_all = storage
                    .get_item(Self::MUTE_ALL_KEY)
                    .ok()
                    .flatten()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(settings.mute_all);
                settings.free_zoom_out = storage
                    .get_item(Self::FREE_ZOOM_OUT_KEY)
                    .ok()
                    .flatten()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(settings.free_zoom_out);
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
            let stored_reduced_motion = storage
                .as_ref()
                .and_then(|storage| storage.get_item(Self::REDUCED_MOTION_KEY).ok().flatten())
                .and_then(|value| value.parse::<bool>().ok());
            if let Some(value) = stored_reduced_motion {
                settings.reduced_motion = value;
                settings.reduced_motion_is_explicit = true;
            } else {
                settings.reduced_motion = window
                    .match_media("(prefers-reduced-motion: reduce)")
                    .ok()
                    .flatten()
                    .map(|query| query.matches())
                    .unwrap_or(false);
            }
        }
        settings
    }

    pub(crate) fn set_reduced_motion(&mut self, value: bool) {
        self.reduced_motion = value;
        self.reduced_motion_is_explicit = true;
    }

    pub fn persist_local(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            for (key, value) in [
                (Self::MUSIC_VOLUME_KEY, self.music_volume.to_string()),
                (Self::MUTE_ALL_KEY, self.mute_all.to_string()),
                (Self::FREE_ZOOM_OUT_KEY, self.free_zoom_out.to_string()),
                (Self::SHOW_FPS_PING_KEY, self.show_fps_ping.to_string()),
                (Self::SHOW_DEV_TOOLS_KEY, self.show_dev_tools.to_string()),
            ] {
                if let Err(error) = storage.set_item(key, &value) {
                    log::warn!("settings: localStorage save failed for {key}: {error:?}");
                }
            }
            if self.reduced_motion_is_explicit {
                let value = self.reduced_motion.to_string();
                if let Err(error) = storage.set_item(Self::REDUCED_MOTION_KEY, &value) {
                    log::warn!(
                        "settings: localStorage save failed for {}: {error:?}",
                        Self::REDUCED_MOTION_KEY
                    );
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
            reduced_motion_is_explicit: false,
            free_zoom_out: true,
            show_fps_ping: false,
            show_dev_tools: false,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fps_ping_and_dev_tools_are_off_by_default() {
        let settings = super::SettingsState::default();
        assert!(!settings.show_fps_ping);
        assert!(!settings.show_dev_tools);
    }
}
