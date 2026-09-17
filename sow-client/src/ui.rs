pub mod theme;
pub mod utils;
pub mod asset_loader;
pub mod hud;
pub mod loading_screen;
pub mod main_menu;
pub mod settings;
pub mod app;

pub use crate::ClientPhase;
pub use app::ClientApp;

#[derive(Clone, Debug)]
pub struct UiText {
    pub key: &'static str,
    pub values: Vec<(&'static str, String)>,
}

impl UiText {
    pub fn new(key: &'static str) -> Self { Self { key, values: Vec::new() } }

    pub fn with(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.values.push((key, value.into()));
        self
    }

    pub fn profile_operation(operation: &str) -> Self {
        let key = match operation {
            "match history" => "profile.match_history_unavailable",
            "ranked records" => "profile.ranked_records_unavailable",
            "profile search" => "profile.player_search_unavailable",
            "match detail" => "profile.match_detail_unavailable",
            _ => "profile.profile_unavailable",
        };
        Self::new(key)
    }
}
