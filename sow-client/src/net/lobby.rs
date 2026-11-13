use crate::MapDownloadEvent;
use crate::app::SowApp;
use crate::ui::main_menu::MainMenuState;
use sow_core::protocol::{LobbyInfo, LobbyKind, ServerSyncStateMessage};
use std::collections::HashSet;

pub(crate) fn clear_lobby_snapshot(state: &mut MainMenuState) {
    state.lobbies.clear();
    state.ranked_matchup = None;
}

fn normalize_lobby_list(lobbies: &mut Vec<LobbyInfo>) {
    let mut seen = HashSet::with_capacity(lobbies.len());
    lobbies.retain(|lobby| seen.insert(lobby.id));
}

/// Private lobbies are excluded from the global LobbiesBroadcast; seed local state on join.
pub(crate) fn seed_joined_lobby_entry(
    state: &mut MainMenuState,
    ack: &sow_core::protocol::ServerJoinAckMessage,
) {
    state.ranked_matchup = (ack.lobby_info.kind == LobbyKind::Ranked1v1)
        .then(|| ack.lobby_info.clone());
    normalize_lobby_list(&mut state.lobbies);
    if let Some(existing) = state.lobbies.iter_mut().find(|l| l.id == ack.lobby_id) {
        *existing = ack.lobby_info.clone();
    } else {
        state.lobbies.push(ack.lobby_info.clone());
    }
}

pub(crate) fn apply_lobby_sync_state(
    state: &mut MainMenuState,
    id: u64,
    kind: LobbyKind,
    sync: &ServerSyncStateMessage,
) {
    normalize_lobby_list(&mut state.lobbies);
    if let Some(lobby) = state.lobbies.iter_mut().find(|l| l.id == id) {
        lobby.timer_secs = sync.time_remaining;
        lobby.is_counting_down = sync.time_remaining > 0.0 && sync.time_remaining < 30.0;
        lobby.num_players = sync.players.len() as u32;
        lobby.players = sync.players.clone();
        return;
    }

    state.lobbies.push(LobbyInfo {
        id,
        kind,
        num_players: sync.players.len() as u32,
        max_players: 0,
        is_counting_down: sync.time_remaining > 0.0 && sync.time_remaining < 30.0,
        timer_secs: sync.time_remaining,
        map_name: "Loading...".to_string(),
        game_mode: "FFA".to_string(),
        players: sync.players.clone(),
        has_password: false,
        host_name: String::new(),
        bot_count: 0,
        nation_count: 0,
        bot_difficulty: Default::default(),
    });
}

pub(crate) fn apply_lobbies_broadcast(
    state: &mut MainMenuState,
    broadcast: &sow_core::protocol::ServerLobbiesBroadcastMessage,
) {
    let joined_id = if state.is_waiting {
        state.joined_lobby_id
    } else {
        None
    };
    let joined_snapshot =
        joined_id.and_then(|id| state.lobbies.iter().find(|l| l.id == id).cloned());
    state.lobbies = broadcast.lobbies.clone();
    normalize_lobby_list(&mut state.lobbies);
    if let Some(id) = joined_id {
        if let Some(broadcast_lobby) = state.lobbies.iter_mut().find(|l| l.id == id) {
            // If the joined lobby is in the broadcast, preserve live timer if countdown is active
            if let Some(ref snap) = joined_snapshot
                && snap.is_counting_down
                && snap.timer_secs < broadcast_lobby.timer_secs
            {
                broadcast_lobby.timer_secs = snap.timer_secs;
                broadcast_lobby.is_counting_down = snap.is_counting_down;
            }
        } else if let Some(snap) = joined_snapshot {
            // If the joined lobby is in Loading/ReadyForRelay phase and omitted from broadcast,
            // push the existing live snapshot so the UI retains live timer_secs and players.
            state.lobbies.push(snap);
        }
    }
}

impl SowApp {
    pub(crate) fn fetch_map_catalog_if_needed(&mut self) {
        if self.ui.app.asset_loader.map_catalog.is_some()
            || self.ui.app.asset_loader.catalog_in_flight
        {
            return;
        }
        self.ui.app.asset_loader.catalog_in_flight = true;
        // Seconds-resolution cache buster: a catalog re-fetched after the owner
        // publishes a map must not come back from the browser or CDN cache.
        let cache_bust = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        let url = format!(
            "{}/catalog.bin?v={cache_bust}",
            self.asset_config.maps_base.trim_end_matches('/')
        );
        let tx = self.tasks.map_tx.clone();
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            if let Ok(res) = result
                && res.ok
                && let Ok(catalog) = sow_core::map_file::parse_catalog(&res.bytes)
            {
                let _ = tx.send(MapDownloadEvent::CatalogReady(catalog.entries));
                return;
            }
            log::warn!("Failed to fetch map catalog.bin");
            let cached = crate::map_cache::catalog_from_cache();
            if cached.is_empty() {
                let _ = tx.send(MapDownloadEvent::CatalogReady(Vec::new()));
            } else {
                log::info!(
                    "Using {} map(s) from offline cache for catalog",
                    cached.len()
                );
                let _ = tx.send(MapDownloadEvent::CatalogReady(cached));
            }
        });
    }

    pub(crate) fn send_join_if_connected(
        &mut self,
        target_lobby_id: Option<u64>,
        host_private: bool,
    ) {
        let join_msg = self.make_join_message(target_lobby_id, host_private, None, None);
        if let Some(join_msg) = join_msg
            && let Ok(json) = bincode::serialize(&join_msg)
            && let Some(c) = self.net.client.as_ref()
        {
            c.send(json);
            self.join_waiting_for_identity = false;
        } else {
            self.join_waiting_for_identity = true;
        }
        self.ui.app.main_menu_state.is_waiting = true;
    }
}
