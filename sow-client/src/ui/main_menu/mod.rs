use sow_core::protocol::LobbyInfo;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameModeFilter { #[default] All, Ffa, Teams, HumansVsNations }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyNotice { HostLeft, Kicked, Banned, ConnectionLost }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainMenuRoute { Home, Browser, Create, Queue, Heroes, Store, Profile }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainMenuSection { Battle, Heroes, Store, Profile }

pub mod profile {
    #[derive(Default)]
    pub struct ProfileState {
        pub account_id: Option<String>, pub view: Option<sow_data::profile::PublicProfileView>,
        pub history: Vec<sow_data::profile::PublicMatchSummary>, pub ratings: Vec<sow_data::profile::PublicRatingView>,
        pub search_results: Vec<sow_data::profile::PublicProfileSummary>, pub search_query: String,
        pub history_cursor: usize, pub history_has_next: bool,
        pub match_detail: Option<sow_data::profile::PublicMatchDetail>, pub ratings_loaded: bool,
        pub loading: bool, pub error: Option<crate::ui::UiText>, pub active_tab: ProfileTab,
    }
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ProfileTab { #[default] Overview, Leaders, History, Ranked }
}

pub struct MainMenuState {
    pub is_connected: bool, pub is_connecting: bool, pub is_waiting: bool, pub wait_timer_secs: f32,
    pub server_address: String, pub lobbies: Vec<LobbyInfo>, pub last_matchmaking_lobby: Option<LobbyInfo>,
    pub matchmaking_countdown_anchor: Option<(u64, f32, f64)>, pub player_name: String,
    pub host_private_pending: bool, pub in_private_match: bool, pub is_lobby_host: bool, pub custom_game_is_private: bool,
    pub custom_game_is_sp: bool, pub custom_game_config: Box<sow_core::game_config::GameConfig>, pub custom_game_password: String,
    pub join_mode_filter: GameModeFilter, pub join_lobby_code: String, pub join_password_input: String,
    pub join_password_for_lobby: Option<u64>, pub pending_join_lobby_id: Option<u64>, pub joined_lobby_id: Option<u64>,
    pub downloading_map_name: Option<String>, pub is_downloading_map: bool, pub cached_map: Option<Vec<u8>>, pub cached_map_key: Option<String>,
    pub map_download_progress: u8, pub show_leader_picker: bool, pub clan_tag: String,
    pub selected_leader: sow_core::player::Leader, pub selected_civilization: sow_core::player::Civilization,
    pub error_message: Option<crate::ui::UiText>, pub my_player_id: Option<u16>, pub notice: Option<LobbyNotice>, pub notice_at: Option<f64>,
    pub safe_area_bottom: f32, pub account_level: u32, pub account_xp: u32,
    pub store_catalog: sow_data::commerce::StoreCatalog, pub selected_skin: Option<String>, pub store_busy: bool,
    pub profile: profile::ProfileState, pub route: MainMenuRoute,
}

impl Default for MainMenuState {
    fn default() -> Self {
        let ms = web_time::SystemTime::now().duration_since(web_time::SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis();
        let leader = match ms % 12 {
            0 => sow_core::player::Leader::Caesar, 1 => sow_core::player::Leader::Cleopatra,
            2 => sow_core::player::Leader::Ragnar, 3 => sow_core::player::Leader::SunTzu,
            4 => sow_core::player::Leader::Alexander, 5 => sow_core::player::Leader::GenghisKhan,
            6 => sow_core::player::Leader::RichardTheLionheart, 7 => sow_core::player::Leader::Vercingetorix,
            8 => sow_core::player::Leader::Boudica, 9 => sow_core::player::Leader::LadySixSky,
            10 => sow_core::player::Leader::Leonidas, _ => sow_core::player::Leader::Napoleon,
        };
        let empty_leaders = BTreeSet::new(); let empty_skins = BTreeSet::new();
        Self {
            is_connected: false, is_connecting: false, is_waiting: false, wait_timer_secs: 0.0,
            server_address: std::env::var("SOW_WS_URL").unwrap_or_else(|_| "wss://ws.shadowsofwar.io/ws/".into()),
            lobbies: Vec::new(), last_matchmaking_lobby: None, matchmaking_countdown_anchor: None,
            player_name: format!("ANON{:03}", ms % 1000), host_private_pending: false,
            in_private_match: false, is_lobby_host: false, custom_game_is_private: false, custom_game_is_sp: true,
            custom_game_config: Box::new(sow_core::game_config::GameConfig { seed: ms as u64, ..Default::default() }),
            custom_game_password: String::new(), join_mode_filter: GameModeFilter::All, join_lobby_code: String::new(),
            join_password_input: String::new(), join_password_for_lobby: None, pending_join_lobby_id: None, joined_lobby_id: None,
            downloading_map_name: None, is_downloading_map: false, cached_map: None, cached_map_key: None, map_download_progress: 0,
            show_leader_picker: false, clan_tag: String::new(), selected_leader: leader, selected_civilization: leader.civilization(),
            error_message: None, my_player_id: None, notice: None, notice_at: None, safe_area_bottom: 0.0,
            account_level: 1, account_xp: 0,
            store_catalog: sow_data::commerce::catalog_for_profile(&empty_leaders, &empty_skins, 0, 0, 0),
            selected_skin: None, store_busy: false, profile: profile::ProfileState::default(), route: MainMenuRoute::Home,
        }
    }
}

impl MainMenuState {
    pub fn visible_route(&self) -> MainMenuRoute { if self.is_waiting && matches!(self.route, MainMenuRoute::Home | MainMenuRoute::Browser | MainMenuRoute::Create | MainMenuRoute::Queue) { MainMenuRoute::Queue } else { self.route } }
    pub fn open_route(&mut self, route: MainMenuRoute) { if route != MainMenuRoute::Queue { self.route = route; } }
    pub fn go_home(&mut self) { self.route = MainMenuRoute::Home; }
    pub fn active_section(&self) -> MainMenuSection { match self.visible_route() { MainMenuRoute::Heroes => MainMenuSection::Heroes, MainMenuRoute::Store => MainMenuSection::Store, MainMenuRoute::Profile => MainMenuSection::Profile, _ => MainMenuSection::Battle } }
    pub fn open_section(&mut self, section: MainMenuSection) { self.route = match section { MainMenuSection::Battle => MainMenuRoute::Home, MainMenuSection::Heroes => MainMenuRoute::Heroes, MainMenuSection::Store => MainMenuRoute::Store, MainMenuSection::Profile => MainMenuRoute::Profile }; }
    pub fn apply_map_catalog_custom(&mut self, catalog: &[sow_core::maps::MapCatalogEntry]) { let cfg = &mut self.custom_game_config; cfg.map_name = sow_core::maps::resolve_map_name(catalog, &cfg.map_name); sow_core::maps::apply_catalog_dimensions(catalog, &mut cfg.map_name, &mut cfg.map_width, &mut cfg.map_height); }
}

pub fn primary_lobby_for_browser(lobbies: &[LobbyInfo]) -> Option<LobbyInfo> { let mut choices: Vec<&LobbyInfo> = lobbies.iter().filter(|l| l.is_counting_down).collect(); if choices.is_empty() { choices = lobbies.iter().collect(); } choices.sort_by_key(|l| l.id); choices.first().cloned().cloned() }
