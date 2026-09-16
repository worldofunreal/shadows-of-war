pub use sow_i18n::Language;

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicsQuality { Low, Medium, High }

pub struct SettingsState {
    pub graphics_quality: GraphicsQuality,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub mute_all: bool,
    pub language: Language,
    pub applied_hint_until: Option<web_time::Instant>,
    pub reduced_motion: bool,
    pub show_fps_ping: bool,
    pub custom_theme: bool,
    pub is_fullscreen: bool,
}

impl Default for SettingsState {
    fn default() -> Self { Self { graphics_quality: GraphicsQuality::High, music_volume: 0.8, sfx_volume: 0.5,
        mute_all: false, language: Language::English, applied_hint_until: None,
        reduced_motion: false, show_fps_ping: false, custom_theme: true, is_fullscreen: false } }
}
