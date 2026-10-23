use crate::ClientPhase;
use crate::MapDownloadEvent;
use crate::app::SowApp;

impl SowApp {
    pub fn update_assets(&mut self) {
        if self.ui.app.phase == ClientPhase::Playing {
            // The browser shell owns the main menu, so the normal WASM path never reaches
            // ClientApp::draw(MainMenu), where avatars are otherwise queued.
            self.ui
                .app
                .asset_loader
                .queue_leader_avatar(self.ui.app.main_menu_state.selected_leader, true);
        }
        self.poll_avatar_fetches();
        self.poll_portal_avatar_fetch();
        self.poll_database_events();

        // Poll map download channel
        while let Ok(res) = self.tasks.map_rx.try_recv() {
            match res {
                MapDownloadEvent::CatalogReady(entries) => {
                    self.ui.app.asset_loader.catalog_in_flight = false;
                    self.ui
                        .app
                        .main_menu_state
                        .apply_map_catalog_custom(&entries);
                    self.ui.app.asset_loader.map_catalog = Some(entries);
                }
                MapDownloadEvent::Progress(downloaded_map_name, progress) => {
                    if Some(downloaded_map_name.clone())
                        == self.ui.app.main_menu_state.downloading_map_name
                    {
                        self.ui.app.main_menu_state.map_download_progress = progress;
                        if let (Some(lid), Some(pid)) =
                            (self.sim.my_lobby_id, self.sim.my_player_id)
                            && let Some(c) = self.net.client.as_ref()
                        {
                            c.send(
                                bincode::serialize(
                                    &sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: lid,
                                        player_id: pid,
                                        progress,
                                    },
                                )
                                .unwrap(),
                            );
                        }
                    }
                }
                MapDownloadEvent::MapReady(map_name, bytes) => {
                    self.ui.app.asset_loader.maps_in_flight.remove(&map_name);
                    crate::map_cache::persist(&map_name, &bytes);
                    self.ui
                        .app
                        .asset_loader
                        .maps
                        .insert(map_name.clone(), bytes.clone());

                    if Some(map_name.clone()) == self.ui.app.main_menu_state.downloading_map_name {
                        log::info!("Map download completed successfully.");
                        self.ui.app.main_menu_state.cached_map = Some(bytes);
                        self.ui.app.main_menu_state.is_downloading_map = false;
                        self.ui.app.main_menu_state.map_download_progress = 100;

                        if let (Some(lid), Some(pid)) =
                            (self.sim.my_lobby_id, self.sim.my_player_id)
                            && let Some(c) = self.net.client.as_ref()
                        {
                            c.send(
                                bincode::serialize(
                                    &sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: lid,
                                        player_id: pid,
                                        progress: 100,
                                    },
                                )
                                .unwrap(),
                            );
                            c.send(
                                bincode::serialize(
                                    &sow_core::protocol::ClientMessage::LobbyReady {
                                        lobby_id: lid,
                                        player_id: pid,
                                    },
                                )
                                .unwrap(),
                            );
                        }
                    }
                }
                MapDownloadEvent::AvatarReady { key, bytes } => {
                    match self
                        .ui
                        .app
                        .asset_loader
                        .ingest_avatar_webp_bytes(key.clone(), &bytes)
                    {
                        Ok(()) => log::debug!("Loaded avatar {:?}", key),
                        Err(e) => {
                            log::warn!("Failed to ingest avatar {:?}: {e}", key);
                            self.ui.app.asset_loader.note_avatar_fetch_failed(key, e);
                        }
                    }
                    // GPU atlas upload is fed inside `ingest_avatar_webp_bytes`.
                }
                MapDownloadEvent::AvatarFailed { key, reason } => {
                    log::warn!("Avatar fetch failed for {:?}: {reason}", key);
                    self.ui
                        .app
                        .asset_loader
                        .note_avatar_fetch_failed(key, reason);
                }
                MapDownloadEvent::PortalAvatarReady { bytes } => {
                    match self.ui.app.asset_loader.ingest_portal_avatar_bytes(&bytes) {
                        Ok(()) => log::info!("Loaded portal identity avatar"),
                        Err(e) => log::warn!("Failed to ingest portal avatar: {e}"),
                    }
                }
                MapDownloadEvent::PortalAvatarFailed { reason } => {
                    self.ui.app.asset_loader.note_portal_avatar_failed(reason);
                }
                MapDownloadEvent::Error(e) => {
                    log::error!("Map download aborted: {}", e);
                    self.ui.app.main_menu_state.is_downloading_map = false;
                    self.tasks.engine_init_queued_msg = None;
                    if self.ui.app.phase == crate::ClientPhase::MainMenu {
                        self.leave_lobby_to_main_menu();
                    } else {
                        self.begin_exit_to_main_menu();
                    }
                    break;
                }
            }
        }
    }

    fn fetch_avatar(
        url: String,
        tx: crate::app::WakeSender<MapDownloadEvent>,
        key: crate::ui::asset_loader::AvatarFetchKey,
    ) {
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            let send = match result {
                Ok(res) if res.ok => MapDownloadEvent::AvatarReady {
                    key: key.clone(),
                    bytes: res.bytes,
                },
                Ok(res) => MapDownloadEvent::AvatarFailed {
                    key: key.clone(),
                    reason: format!("HTTP {}", res.status),
                },
                Err(e) => MapDownloadEvent::AvatarFailed {
                    key,
                    reason: e.to_string(),
                },
            };
            let _ = tx.send(send);
        });
    }

    fn poll_avatar_fetches(&mut self) {
        use crate::ui::asset_loader::{AssetLoader, AvatarFetchKey, MAX_AVATAR_FETCHES_IN_FLIGHT};

        let priority_leader = self.ui.app.main_menu_state.selected_leader;
        let priority = AvatarFetchKey::Leader(priority_leader);

        if let Some(snapshot) = self.sim.current_snapshot.as_ref() {
            for player in &snapshot.players {
                if player.player_type == sow_core::player::PlayerType::Human {
                    self.ui.app.asset_loader.queue_leader_avatar(
                        player.leader,
                        player.id == self.sim.my_player_id.unwrap_or_default(),
                    );
                }
            }
        }
        let drawn = self.ui.nameplates.take_avatar_wants();
        self.ui.app.asset_loader.request_drawn_portraits(&drawn);

        while self.ui.app.asset_loader.avatars_in_flight.len() < MAX_AVATAR_FETCHES_IN_FLIGHT {
            let Some(key) = self
                .ui
                .app
                .asset_loader
                .take_next_avatar_fetch_pending(&priority)
            else {
                break;
            };

            let filename = AssetLoader::avatar_filename(&key);
            let url = self.asset_config.avatar_url(&filename);
            log::debug!("Fetching avatar {:?} url={}", key, url);
            let tx = self.tasks.map_tx.clone();
            Self::fetch_avatar(url, tx, key);
        }
    }

    fn poll_portal_avatar_fetch(&mut self) {
        if self.ui.app.asset_loader.portal_avatar.is_some()
            || self.ui.app.asset_loader.portal_avatar_in_flight
        {
            return;
        }
        let Some(url) = self.ui.app.asset_loader.portal_avatar_request.clone() else {
            return;
        };
        self.ui.app.asset_loader.portal_avatar_in_flight = true;
        self.ui.app.asset_loader.portal_avatar_request = None;
        log::debug!("Fetching portal avatar url={url}");
        let tx = self.tasks.map_tx.clone();
        Self::fetch_portal_avatar(url, tx);
    }

    fn fetch_portal_avatar(url: String, tx: crate::app::WakeSender<MapDownloadEvent>) {
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            let send = match result {
                Ok(res) if res.ok => MapDownloadEvent::PortalAvatarReady { bytes: res.bytes },
                Ok(res) => MapDownloadEvent::PortalAvatarFailed {
                    reason: format!("HTTP {}", res.status),
                },
                Err(e) => MapDownloadEvent::PortalAvatarFailed {
                    reason: e.to_string(),
                },
            };
            let _ = tx.send(send);
        });
    }

    fn poll_database_events(&mut self) {
        while let Ok(event) = self.tasks.db_rx.try_recv() {
            match event {
                crate::player_progress::DbEvent::MatchResultLoaded {
                    account_id,
                    match_id,
                    cycle_id,
                    request_id,
                    display_name,
                    provider,
                    progress,
                    state,
                    closure_recorded,
                } => {
                    if !self.matches_exit_response(&account_id, &match_id, cycle_id)
                        || self
                            .menu_return
                            .as_ref()
                            .and_then(|context| context.result_request_id)
                            != Some(request_id)
                    {
                        continue;
                    }
                    let context = self.menu_return.as_mut().unwrap();
                    context.result_request_id = None;
                    context.server_state = Some(state);
                    context.exit_recorded |= closure_recorded;
                    let preparing = context.preparation_only;
                    if closure_recorded
                        && !preparing
                        && self.ui.app.phase == crate::ClientPhase::Playing
                    {
                        self.begin_relay_closed_return();
                    }
                    if let Some(progress) = progress {
                        if matches!(
                            state,
                            sow_data::profile::MatchExitState::Confirmed
                                | sow_data::profile::MatchExitState::Rejected
                        ) {
                            if !preparing
                                && (self.ui.app.phase == crate::ClientPhase::Playing
                                    || (self.ui.app.phase == crate::ClientPhase::Splash
                                        && self.ui.app.splash_state.job
                                            == crate::ui::loading_screen::SplashJob::EnterGame))
                            {
                                self.begin_relay_closed_return();
                                if let Some(context) = &mut self.menu_return {
                                    context.server_state = Some(state);
                                }
                            }
                            self.profile_last_applied_request =
                                self.profile_last_applied_request.max(request_id);
                            self.apply_cloud_profile(progress, account_id, display_name, provider);
                            #[cfg(target_arch = "wasm32")]
                            {
                                self.boot_db_settled = true;
                            }
                        }
                    }
                    self.schedule_reward_profile_retry();
                }
                crate::player_progress::DbEvent::MatchResultFailed {
                    account_id,
                    match_id,
                    cycle_id,
                    request_id,
                    status,
                } => {
                    if !self.matches_exit_response(&account_id, &match_id, cycle_id)
                        || self
                            .menu_return
                            .as_ref()
                            .and_then(|context| context.result_request_id)
                            != Some(request_id)
                    {
                        continue;
                    }
                    self.menu_return.as_mut().unwrap().result_request_id = None;
                    self.mark_menu_return_error();
                    log::warn!(
                        "[menu-flow] result observation failed match={match_id} cycle={cycle_id} status={status:?}"
                    );
                    self.schedule_reward_profile_retry();
                }
                crate::player_progress::DbEvent::ProfileLoaded {
                    progress,
                    account_id,
                    display_name,
                    provider,
                    request_id,
                } => {
                    if request_id <= self.profile_last_applied_request {
                        log::warn!(
                            "[identity] ignoring stale profile response id={request_id} last_applied={}",
                            self.profile_last_applied_request
                        );
                        continue;
                    }
                    self.profile_request_in_flight = false;
                    if self.profile_refresh_pending && self.display_name_save_request_id.is_none() {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                        continue;
                    }
                    self.profile_last_applied_request = request_id;
                    let old_level = self.progress.level;
                    log::info!(
                        "[identity] applying profile request id={request_id} provider={provider} account_len={} name_len={}",
                        account_id.chars().count(),
                        display_name.chars().count()
                    );
                    self.apply_cloud_profile(progress, account_id, display_name, provider);
                    log::info!(
                        "Successfully synced profile from cloud database: level {} ({} XP)",
                        self.progress.level,
                        self.progress.xp
                    );
                    if self.progress.level > old_level {
                        crate::store_portals::happytime();
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.boot_db_settled = true;
                    }
                    if self.join_waiting_for_identity
                        && self.ui.app.main_menu_state.is_waiting
                        && self.ui.app.main_menu_state.joined_lobby_id.is_none()
                        && self.net.client.is_some()
                    {
                        let join_msg = self.make_join_message(
                            self.ui.app.main_menu_state.pending_join_lobby_id,
                            self.ui.app.main_menu_state.custom_game_is_private,
                            (!self.join_matchmaking)
                                .then(|| self.ui.app.main_menu_state.custom_game_config.clone()),
                            Some(self.ui.app.main_menu_state.custom_game_password.clone())
                                .filter(|password| !password.is_empty()),
                        );
                        if let Some(join_msg) = join_msg
                            && let Ok(json) = bincode::serialize(&join_msg)
                            && let Some(client) = self.net.client.as_ref()
                        {
                            client.send(json);
                            self.join_waiting_for_identity = false;
                        }
                    }
                    if self.profile_refresh_pending && self.display_name_save_request_id.is_none() {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                    }
                }
                crate::player_progress::DbEvent::DisplayNameSaved {
                    account_id,
                    display_name,
                    request_id,
                } => {
                    if self.display_name_save_request_id != Some(request_id) {
                        log::warn!(
                            "[identity] ignoring stale rename ACK id={request_id} current={:?}",
                            self.display_name_save_request_id
                        );
                        continue;
                    }
                    self.display_name_save_request_id = None;
                    if self.progress_account_id.as_deref() != Some(account_id.as_str()) {
                        log::error!(
                            "[identity] ignoring rename ACK id={request_id}: account changed while request was in flight"
                        );
                        if self.profile_refresh_pending {
                            self.profile_refresh_pending = false;
                            self.fetch_cloud_progress();
                        }
                        continue;
                    }
                    if self.pending_display_name.as_deref() != Some(display_name.as_str()) {
                        if let Some(next_name) = self.pending_display_name.clone() {
                            self.ui.app.main_menu_state.player_name = next_name.clone();
                            self.save_display_name(next_name);
                        }
                        continue;
                    }
                    crate::anonymous_identity::clear_pending_display_name();
                    self.pending_display_name = None;
                    self.ui.app.main_menu_state.player_name = display_name;
                    self.ui.app.main_menu_state.error_message = None;
                    if self.profile_refresh_pending {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                    }
                }
                crate::player_progress::DbEvent::DisplayNameSaveFailed { request_id, status } => {
                    if self.display_name_save_request_id != Some(request_id) {
                        log::warn!(
                            "[identity] ignoring stale rename failure id={request_id} current={:?}",
                            self.display_name_save_request_id
                        );
                        continue;
                    }
                    self.display_name_save_request_id = None;
                    log::warn!(
                        "[identity] rename request id={request_id} failed status={status:?}; keeping pending name"
                    );
                    self.ui.app.main_menu_state.error_message =
                        Some(crate::ui::UiText::new("profile.profile_unavailable"));
                    if self.profile_refresh_pending {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                    }
                }
                crate::player_progress::DbEvent::LoadFailed { request_id, status } => {
                    if request_id <= self.profile_last_applied_request {
                        continue;
                    }
                    self.profile_request_in_flight = false;
                    if self.profile_refresh_pending && self.display_name_save_request_id.is_none() {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                        continue;
                    }
                    if !self.pending_reward_receipt_ids.is_empty() || self.waiting_menu_return() {
                        self.mark_menu_return_error();
                        log::warn!(
                            "[rewards] profile refresh id={request_id} failed status={status:?}; keeping retry window"
                        );
                        self.schedule_reward_profile_retry();
                        continue;
                    }
                    self.profile_refresh_pending = false;
                    // No fallback: continuing without an identity once masked a
                    // 403 from a misrouted endpoint and booted the wrong mode.
                    // A failed identity load is a hard failure — crash loudly.
                    panic!(
                        "[identity] profile request id={request_id} failed status={status:?} — no fallback, refusing to continue without identity"
                    );
                }
                crate::player_progress::DbEvent::TutorialCompletionFailed {
                    request_id,
                    status,
                } => {
                    if request_id <= self.profile_last_applied_request {
                        continue;
                    }
                    self.profile_request_in_flight = false;
                    self.mark_menu_return_error();
                    log::warn!(
                        "[tutorial] completion request id={request_id} failed status={status:?}; local completion marker retained"
                    );
                    self.schedule_reward_profile_retry();
                    if self.profile_refresh_pending {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                    }
                }
                crate::player_progress::DbEvent::ProfileViewLoaded { account_id, view } => {
                    if self.ui.app.main_menu_state.profile.account_id.as_deref()
                        != Some(account_id.as_str())
                    {
                        continue;
                    }
                    self.ui.app.main_menu_state.profile.loading = false;
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.history = view.recent_matches.clone();
                    self.ui.app.main_menu_state.profile.history_cursor = view.recent_matches.len();
                    self.ui.app.main_menu_state.profile.history_has_next =
                        view.matches_played > view.recent_matches.len() as u32;
                    self.ui.app.main_menu_state.profile.view = Some(view);
                }
                crate::player_progress::DbEvent::ProfileLoadFailed { account_id, status } => {
                    if self.ui.app.main_menu_state.profile.account_id.as_deref()
                        != Some(account_id.as_str())
                    {
                        continue;
                    }
                    self.ui.app.main_menu_state.profile.loading = false;
                    log::error!(
                        "[profile] profile unavailable account={account_id} status={status:?}"
                    );
                    self.ui.app.main_menu_state.profile.error =
                        Some(crate::ui::UiText::new("profile.profile_unavailable"));
                }
                crate::player_progress::DbEvent::ProfileHistoryLoaded {
                    account_id,
                    items,
                    next_cursor,
                } => {
                    if self.ui.app.main_menu_state.profile.account_id.as_deref()
                        != Some(account_id.as_str())
                    {
                        continue;
                    }
                    self.ui.app.main_menu_state.profile.loading = false;
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.history.extend(items);
                    self.ui.app.main_menu_state.profile.history_cursor =
                        next_cursor.unwrap_or(self.ui.app.main_menu_state.profile.history_cursor);
                    self.ui.app.main_menu_state.profile.history_has_next = next_cursor.is_some();
                }
                crate::player_progress::DbEvent::ProfileSearchLoaded { query, items } => {
                    self.ui.app.main_menu_state.profile.loading = false;
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.search_query = query;
                    self.ui.app.main_menu_state.profile.search_results = items;
                }
                crate::player_progress::DbEvent::MatchDetailLoaded {
                    match_id: _,
                    detail,
                } => {
                    self.ui.app.main_menu_state.profile.loading = false;
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.match_detail = Some(detail);
                }
                crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id,
                    operation,
                } => {
                    if let Some(account_id) = account_id.as_deref()
                        && self.ui.app.main_menu_state.profile.account_id.as_deref()
                            != Some(account_id)
                    {
                        continue;
                    }
                    self.ui.app.main_menu_state.profile.loading = false;
                    self.ui.app.main_menu_state.profile.error =
                        Some(crate::ui::UiText::profile_operation(&operation));
                    log::error!("[profile] {operation} failed");
                }
                crate::player_progress::DbEvent::RewardReceiptsAcked {
                    account_id,
                    receipt_ids,
                } => {
                    if self.progress_account_id.as_deref() != Some(account_id.as_str()) {
                        continue;
                    }
                    let presented_at = web_time::SystemTime::now()
                        .duration_since(web_time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    for receipt_id in receipt_ids {
                        if let Some(receipt) = self.progress.reward_receipts.get_mut(&receipt_id) {
                            receipt.presented_at = Some(presented_at);
                            receipt.status = sow_data::profile::RewardSettlementStatus::Presented;
                        }
                    }
                    self.save_local_progress();
                }
                crate::player_progress::DbEvent::StoreProfileLoaded {
                    account_id,
                    mut progress,
                    operation,
                } => {
                    if self.progress_account_id.as_deref() != Some(account_id.as_str()) {
                        log::error!(
                            "[store] ignoring {operation} response for a different account"
                        );
                        continue;
                    }
                    if operation == "menu guide progress" {
                        self.progress.merge_menu_guides(progress.menu_guides);
                        self.menu_guide_save_pending = false;
                        self.save_local_progress();
                        continue;
                    }
                    let campaign_episode = operation
                        .strip_prefix("campaign completion:")
                        .map(str::to_string);
                    let finish_campaign_exit = campaign_episode.as_deref().is_some_and(|episode| {
                        self.pending_campaign_exit.as_deref() == Some(episode)
                    });
                    let saved_guides = std::mem::take(&mut self.progress.menu_guides);
                    progress.merge_menu_guides(saved_guides);
                    let local_episodes = self.progress.completed_episodes.clone();
                    let local_pending_episodes = self.progress.pending_campaign_completions.clone();
                    let local_selection = (
                        self.progress.preferred_leader,
                        self.progress.prefer_random_leader,
                        self.progress.leader_selection_pending,
                    );
                    let server_episodes = progress.completed_episodes.clone();
                    progress.completed_episodes.extend(local_episodes);
                    progress
                        .pending_campaign_completions
                        .extend(local_pending_episodes);
                    progress.pending_campaign_completions.extend(
                        progress
                            .completed_episodes
                            .iter()
                            .filter(|episode| !server_episodes.contains(*episode))
                            .cloned(),
                    );
                    progress
                        .pending_campaign_completions
                        .retain(|episode| !server_episodes.contains(episode));
                    if let Some(episode) = campaign_episode.as_deref() {
                        progress.completed_episodes.insert(episode.to_string());
                        progress.pending_campaign_completions.remove(episode);
                    }
                    if operation == "leader selection" {
                        progress.leader_selection_pending = false;
                    } else if local_selection.2 {
                        progress.preferred_leader = local_selection.0;
                        progress.prefer_random_leader = local_selection.1;
                        progress.leader_selection_pending = true;
                    }
                    self.progress = progress;
                    self.save_local_progress();
                    self.sync_wou_collectibles();
                    self.ui.app.main_menu_state.store_busy = false;
                    self.ui.app.main_menu_state.error_message = None;
                    self.profile_action_retry_at = None;
                    log::info!("[store] {operation} acknowledged by server");
                    if finish_campaign_exit {
                        self.begin_exit_to_main_menu();
                    }
                }
                crate::player_progress::DbEvent::StoreActionFailed {
                    account_id,
                    operation,
                    status,
                } => {
                    if self.progress_account_id.as_deref() != Some(account_id.as_str()) {
                        continue;
                    }
                    if operation == "menu guide progress" {
                        self.menu_guide_save_pending = false;
                        log::warn!(
                            "[menu-guide] progress save failed status={status:?}; retry remains pending"
                        );
                        continue;
                    }
                    if operation == "leader selection"
                        || operation.starts_with("campaign completion:")
                    {
                        self.profile_action_retry_at = Some(
                            web_time::Instant::now()
                                + web_time::Duration::from_secs(if status == Some(409) {
                                    30
                                } else {
                                    2
                                }),
                        );
                        log::warn!(
                            "[profile] {operation} failed status={status:?}; saved request will retry"
                        );
                    }
                    self.ui.app.main_menu_state.store_busy = false;
                    self.ui.app.main_menu_state.error_message =
                        Some(crate::ui::UiText::new("store.action_unavailable"));
                    log::error!("[store] {operation} failed status={status:?}");
                }
            }
        }
    }
}
