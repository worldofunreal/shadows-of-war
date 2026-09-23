use crate::MapDownloadEvent;
use crate::app::SowApp;
use crate::ui::main_menu::MainMenuState;
use sow_core::protocol::{LobbyInfo, LobbyKind, ServerSyncStateMessage};
use std::collections::HashSet;

pub(crate) fn clear_lobby_snapshot(state: &mut MainMenuState) {
    state.lobbies.clear();
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

#[cfg(test)]
mod tests {
    use super::{apply_lobbies_broadcast, apply_lobby_sync_state, clear_lobby_snapshot};
    use crate::ui::main_menu::MainMenuState;
    use sow_core::protocol::{
        LobbyInfo, LobbyKind, ServerLobbiesBroadcastMessage, ServerSyncStateMessage,
    };

    fn lobby(id: u64, timer_secs: f32) -> LobbyInfo {
        LobbyInfo {
            id,
            num_players: 1,
            max_players: 8,
            is_counting_down: timer_secs > 0.0,
            timer_secs,
            map_name: "world".to_string(),
            game_mode: "FFA".to_string(),
            players: Vec::new(),
            has_password: false,
            host_name: String::new(),
            bot_count: 0,
            nation_count: 0,
            bot_difficulty: Default::default(),
            kind: LobbyKind::Matchmaking,
        }
    }

    fn broadcast(lobbies: Vec<LobbyInfo>) -> ServerLobbiesBroadcastMessage {
        ServerLobbiesBroadcastMessage { lobbies }
    }

    #[test]
    fn preserves_joined_snapshot_only_while_waiting() {
        let mut state = MainMenuState::default();
        state.is_waiting = true;
        state.joined_lobby_id = Some(1);
        state.lobbies = vec![lobby(1, 4.0)];
        apply_lobbies_broadcast(&mut state, &broadcast(vec![lobby(2, 0.0)]));
        assert_eq!(
            state.lobbies.iter().map(|l| l.id).collect::<Vec<_>>(),
            vec![2, 1]
        );

        state.is_waiting = false;
        state.joined_lobby_id = None;
        apply_lobbies_broadcast(&mut state, &broadcast(vec![lobby(3, 0.0)]));
        assert_eq!(
            state.lobbies.iter().map(|l| l.id).collect::<Vec<_>>(),
            vec![3]
        );
    }

    #[test]
    fn clear_snapshot_removes_the_lobby_that_could_be_carried_forward() {
        let mut state = MainMenuState::default();
        state.lobbies = vec![lobby(1, 4.0)];
        clear_lobby_snapshot(&mut state);
        assert!(state.lobbies.is_empty());
    }

    #[test]
    fn local_sync_updates_one_entry_without_duplicates() {
        let mut state = MainMenuState::default();
        state.lobbies = vec![lobby(1, 4.0), lobby(1, 3.0)];
        let sync = ServerSyncStateMessage {
            time_remaining: 2.0,
            players: Vec::new(),
            is_starting: false,
        };
        apply_lobby_sync_state(&mut state, 1, LobbyKind::Matchmaking, &sync);
        assert_eq!(state.lobbies.len(), 1);
        assert_eq!(state.lobbies[0].timer_secs, 2.0);
    }

    #[test]
    fn broadcast_deduplicates_lobby_ids() {
        let mut state = MainMenuState::default();
        apply_lobbies_broadcast(&mut state, &broadcast(vec![lobby(1, 4.0), lobby(1, 3.0)]));
        assert_eq!(state.lobbies.len(), 1);
        assert_eq!(state.lobbies[0].timer_secs, 4.0);
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
        let url = format!(
            "{}/catalog.bin",
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
