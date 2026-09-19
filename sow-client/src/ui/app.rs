use crate::{
    ClientPhase,
    ui::{asset_loader, hud, loading_screen, main_menu, settings},
};

pub struct ClientApp {
    pub phase: ClientPhase,
    pub main_menu_state: main_menu::MainMenuState,
    pub hud_state: hud::HudState,
    pub splash_state: loading_screen::SplashState,
    pub asset_loader: asset_loader::AssetLoader,
    pub is_settings_open: bool,
    pub is_credits_open: bool,
    pub is_privacy_open: bool,
    pub is_terms_open: bool,
    pub is_showcase_open: bool,
    pub settings_state: settings::SettingsState,
}

impl Default for ClientApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientApp {
    pub fn new() -> Self {
        Self {
            phase: ClientPhase::Splash,
            main_menu_state: main_menu::MainMenuState::default(),
            hud_state: hud::HudState::default(),
            splash_state: loading_screen::SplashState::default(),
            asset_loader: asset_loader::AssetLoader::new(),
            is_settings_open: false,
            is_credits_open: false,
            is_privacy_open: false,
            is_terms_open: false,
            is_showcase_open: false,
            settings_state: settings::SettingsState::default(),
        }
    }
}
