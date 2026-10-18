//! Authoritative profile and return handoff; see docs/player-journeys.md.
use super::state::{MenuReturnContext, SowApp};
use web_time::{Duration, Instant};

const REWARD_PROFILE_RETRY_INTERVAL: Duration = Duration::from_secs(30);
const EXIT_REWARD_PROFILE_RETRY_INTERVAL: Duration = Duration::from_secs(1);

fn account_hint(account_id: Option<&str>) -> String {
    account_id
        .map(|id| id.chars().take(8).collect::<String>())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn apply_menu_return_profile(
    context: &mut Option<MenuReturnContext>,
    account_id: &str,
    cloud: &crate::player_progress::PlayerProgress,
) -> bool {
    let Some(context) = context
        .as_mut()
        .filter(|context| matches!(context.status.as_str(), "waiting" | "error"))
    else {
        return false;
    };
    if context.account_id.is_none() {
        context.account_id = Some(account_id.to_string());
    }
    let requires_verification = context.source == "multiplayer";
    let ready = context.account_id.as_deref() == Some(account_id)
        && (!requires_verification
            || matches!(
                context.server_state,
                Some(
                    sow_data::profile::MatchExitState::Confirmed
                        | sow_data::profile::MatchExitState::Rejected
                )
            ))
        && context.match_id.as_deref().is_some_and(|match_id| {
            cloud.reward_receipts.get(match_id).is_some_and(|receipt| {
                receipt.id == match_id
                    && receipt.match_id == match_id
                    && matches!(
                        receipt.status,
                        sow_data::profile::RewardSettlementStatus::Applied
                            | sow_data::profile::RewardSettlementStatus::Presented
                    )
                    && (!requires_verification
                        || matches!(
                            receipt.verification_status,
                            Some(
                                sow_data::profile::ReplayVerificationStatus::Verified
                                    | sow_data::profile::ReplayVerificationStatus::Rejected
                            )
                        ))
            })
        });
    if ready {
        context.status = "confirmed".into();
        context.receipt_ids = context.match_id.iter().cloned().collect();
    }
    ready
}

fn fetch_anonymous_profile_request(
    url: String,
    account_id: Option<String>,
    requested_display_name: Option<String>,
    tx: crate::app::WakeSender<crate::player_progress::DbEvent>,
    reset_stale_id: bool,
    request_id: u64,
) {
    #[derive(serde::Serialize)]
    struct AnonymousProfileRequest {
        account_id: Option<String>,
        display_name: Option<String>,
        auth_secret: Option<String>,
    }

    let retry_display_name = requested_display_name.clone();
    let payload = AnonymousProfileRequest {
        account_id: account_id.clone(),
        display_name: requested_display_name,
        auth_secret: crate::anonymous_identity::load_account_secret(),
    };
    let Ok(body) = serde_json::to_vec(&payload) else {
        log::error!(
            "[identity] profile request id={request_id} serialize_failed account={}",
            account_hint(account_id.as_deref())
        );
        let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
            request_id,
            status: None,
        });
        return;
    };
    let mut request = ehttp::Request::post(&url, body);
    request.headers.insert("Content-Type", "application/json");
    request
        .headers
        .insert("X-SOW-Identity-Request", request_id.to_string());
    log::info!(
        "[identity] profile request id={request_id} start account={} reset_stale_id={reset_stale_id}",
        account_hint(account_id.as_deref())
    );
    ehttp::fetch(request, move |result| match result {
        Ok(res) if res.ok => {
            #[derive(serde::Deserialize)]
            struct DbAccount {
                account_id: String,
                #[serde(default)]
                display_name: String,
                profile: crate::player_progress::PlayerProgress,
                /// One-time ownership secret, present only when just minted.
                #[serde(default)]
                auth_secret: Option<String>,
            }
            match serde_json::from_slice::<DbAccount>(&res.bytes) {
                Ok(account) => {
                    crate::anonymous_identity::save_account_id(&account.account_id);
                    if let Some(secret) = account.auth_secret.as_deref()
                        && !secret.is_empty()
                    {
                        crate::anonymous_identity::save_account_secret(secret);
                        log::info!(
                            "[identity] account secret minted and stored account={}",
                            account_hint(Some(&account.account_id))
                        );
                    }
                    log::info!(
                        "[identity] profile request id={request_id} ack account={} name_len={}",
                        account_hint(Some(&account.account_id)),
                        account.display_name.chars().count()
                    );
                    let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                        progress: account.profile,
                        account_id: account.account_id,
                        display_name: account.display_name,
                        provider: "anonymous".to_string(),
                        request_id,
                    });
                }
                Err(error) => {
                    log::error!("[identity] profile request id={request_id} parse_failed: {error}");
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                        request_id,
                        status: Some(res.status),
                    });
                }
            }
        }
        Ok(res) => {
            if res.status == 404 && reset_stale_id && account_id.is_some() {
                log::warn!(
                    "[identity] profile request id={request_id} missing_account account={} action=create_replacement",
                    account_hint(account_id.as_deref())
                );
                crate::anonymous_identity::clear_account_id();
                // The current UI name is the only client-side presentation value. The
                // server persists it with the newly issued account ID.
                fetch_anonymous_profile_request(
                    url,
                    None,
                    retry_display_name,
                    tx,
                    false,
                    request_id,
                );
                return;
            }
            log::warn!(
                "[identity] profile request id={request_id} failed status={} account={}",
                res.status,
                account_hint(account_id.as_deref())
            );
            let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                request_id,
                status: Some(res.status),
            });
        }
        Err(error) => {
            log::error!("[identity] profile request id={request_id} network_failed: {error}");
            let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                request_id,
                status: None,
            });
        }
    });
}

impl SowApp {
    fn next_identity_request_id(&mut self) -> u64 {
        self.identity_request_seq = self.identity_request_seq.wrapping_add(1);
        if self.identity_request_seq == 0 {
            self.identity_request_seq = 1;
        }
        self.identity_request_seq
    }

    fn apply_platform_auth(request: &mut ehttp::Request) {
        let identity = crate::store_portals::load_identity("Player");
        if let Some(token) = identity.auth_token.filter(|t| !t.is_empty()) {
            request.headers.insert("X-Platform-Auth", token);
            if identity.provider != "self" {
                request
                    .headers
                    .insert("X-Platform-Provider", identity.provider);
            }
        }
    }

    /// Mirror in-game entitlements as NFTROPOLY collectibles in WOU-ID:
    /// every owned commander, skin, and unlocked achievement becomes a
    /// claimed card on the player's own account, using the player's WOU-ID
    /// bearer token (guests own nothing server-side by design). One-shot per
    /// account per session, re-run when the owned set grows (purchases).
    /// Best-effort: failures log and are retried on the next profile apply.
    pub(crate) fn sync_wou_collectibles(&mut self) {
        if self.progress_provider != "wou" {
            return;
        }
        let Some(account_id) = self.progress_account_id.clone() else {
            return;
        };
        let identity = crate::store_portals::load_identity("Player");
        if identity.provider != "wou" {
            return;
        }
        let Some(session_token) = identity.auth_token.filter(|token| !token.is_empty()) else {
            return;
        };
        // Assets in WOU-ID are keyed by the WOU-ID account id (the JWT
        // subject), which the portal surfaces as external_id. Prefer it over
        // the SOW profile account id so the owner lookup can never drift.
        let owner_id = identity
            .external_id
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| account_id.clone());
        let mut desired: Vec<String> = Vec::new();
        for leader_id in &self.progress.owned_leaders {
            if let Some(card) = sow_data::commerce::leader_from_id(leader_id)
                .and_then(sow_core::player::Leader::card_token_id)
            {
                desired.push(card.to_string());
            }
        }
        for skin_id in &self.progress.owned_skins {
            desired.push(sow_data::commerce::skin_card_token_id(skin_id));
        }
        for achievement in &self.progress.unlocked_achievements {
            desired.push(sow_data::rewards::achievement_card_token_id(achievement));
        }
        if desired.is_empty() {
            return;
        }
        if self
            .wou_collectibles_synced
            .as_ref()
            .is_some_and(|(synced_account, synced_count)| {
                synced_account == &account_id && *synced_count >= desired.len()
            })
        {
            return;
        }
        self.wou_collectibles_synced = Some((account_id.clone(), desired.len()));
        const ID_API: &str = "https://id.worldofunreal.com";
        let mut request = ehttp::Request::get(format!("{ID_API}/api/v1/assets/owner/{owner_id}"));
        request
            .headers
            .insert("Authorization", format!("Bearer {session_token}"));
        log::info!(
            "[collectibles] syncing {} owned cards for account={}",
            desired.len(),
            account_hint(Some(&account_id))
        );
        ehttp::fetch(request, move |result| {
            let owned: Vec<String> = match result {
                Ok(response) if response.ok => {
                    serde_json::from_slice::<Vec<serde_json::Value>>(&response.bytes)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    item.get("token")
                                        .and_then(|token| token.as_str())
                                        .map(str::to_string)
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                }
                Ok(response) => {
                    log::warn!(
                        "[collectibles] owner lookup failed HTTP {:?}",
                        response.status
                    );
                    return;
                }
                Err(error) => {
                    log::warn!("[collectibles] owner lookup failed: {error}");
                    return;
                }
            };
            for token in desired.into_iter().filter(|card| !owned.contains(card)) {
                let Ok(body) = serde_json::to_vec(&serde_json::json!({ "token": &token })) else {
                    continue;
                };
                let mut claim = ehttp::Request::post(format!("{ID_API}/api/v1/assets/claim"), body);
                claim
                    .headers
                    .insert("Authorization", format!("Bearer {session_token}"));
                claim.headers.insert("Content-Type", "application/json");
                ehttp::fetch(claim, move |result| match result {
                    Ok(response) if response.ok => {
                        log::info!("[collectibles] claimed {token}")
                    }
                    Ok(response) => log::warn!(
                        "[collectibles] claim {token} failed HTTP {:?}",
                        response.status
                    ),
                    Err(error) => {
                        log::warn!("[collectibles] claim {token} failed: {error}")
                    }
                });
            }
        });
    }

    fn track_reward_receipt_sync(&mut self, receipt_id: impl Into<String>) {
        self.pending_reward_receipt_ids.insert(receipt_id.into());
        if let Some(account_id) = self.progress_account_id.as_deref() {
            crate::anonymous_identity::save_pending_reward_receipt_ids(
                account_id,
                &self.pending_reward_receipt_ids,
            );
        }
    }

    fn reward_profile_retry_interval(&self) -> Duration {
        if self.waiting_menu_return() {
            EXIT_REWARD_PROFILE_RETRY_INTERVAL
        } else {
            REWARD_PROFILE_RETRY_INTERVAL
        }
    }

    pub(crate) fn begin_reward_profile_sync(
        &mut self,
        receipt_id: Option<String>,
        requires_verification: bool,
    ) {
        if requires_verification {
            // Old generic profile requests cannot overwrite the result observed below.
            self.profile_last_applied_request = self.identity_request_seq;
            self.profile_request_in_flight = false;
            self.profile_refresh_pending = false;
        }
        if let Some(receipt_id) = receipt_id {
            self.track_reward_receipt_sync(receipt_id);
        }
        self.reward_profile_retry_at = Some(Instant::now());
        self.fetch_cloud_progress();
    }

    pub(crate) fn prepare_menu_return(&mut self, source: &str, receipt_id: Option<String>) {
        self.menu_return = Some(MenuReturnContext {
            account_id: self.progress_account_id.clone(),
            cycle_id: self.ui.app.splash_state.cycle_id,
            source: source.to_string(),
            match_id: receipt_id.clone(),
            receipt_ids: receipt_id.iter().cloned().collect(),
            status: if receipt_id.is_some() {
                "waiting"
            } else {
                "not_applicable"
            }
            .to_string(),
            requested_screen: None,
            started_at: Some(Instant::now()),
            ..Default::default()
        });
        self.save_menu_return_context();
    }

    pub(crate) fn prepare_match_result(&mut self, through_turn: u64) {
        if self.net.is_offline {
            return;
        }
        let Some(id) = self.sim.my_lobby_id else {
            return;
        };
        let id = id.to_string();
        if !self.menu_return.as_ref().is_some_and(|c| {
            c.source == "multiplayer" && c.match_id.as_deref() == Some(id.as_str())
        }) {
            self.prepare_menu_return("multiplayer", Some(id.clone()));
        }
        let context = self.menu_return.as_mut().unwrap();
        context.preparation_only = true;
        context.voluntary_exit = Some(false);
        if context.preparation_started_at_ms.is_none() {
            context.preparation_started_at_ms = Some(
                web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );
        }
        if let Some(client) = &self.net.client {
            if let Ok(bytes) =
                bincode::serialize(&sow_core::protocol::ClientMessage::PrepareResult {
                    through_turn,
                })
            {
                client.send(bytes);
            }
        }
        self.save_menu_return_context();
        self.begin_reward_profile_sync(Some(id), true);
    }

    pub(crate) fn finished_relay_match(&self) -> bool {
        self.menu_return.as_ref().is_some_and(|c| {
            c.match_finished
                && c.match_id == self.sim.my_lobby_id.map(|id| id.to_string())
                && self.ui.app.phase == crate::ClientPhase::Playing
        })
    }

    pub(crate) fn save_menu_return_context(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(context) = &self.menu_return {
            crate::anonymous_identity::save_menu_return(context);
        }
    }

    pub(crate) fn waiting_menu_return(&self) -> bool {
        self.menu_return
            .as_ref()
            .is_some_and(|context| matches!(context.status.as_str(), "waiting" | "error"))
    }

    pub(crate) fn poll_reward_profile_sync(&mut self, now: Instant) {
        self.observe_relay_exit_recording();
        if self.pending_reward_receipt_ids.is_empty() && !self.waiting_menu_return() {
            return;
        }
        let observing_disconnect = self.waiting_menu_return()
            && (self.ui.app.phase == crate::ClientPhase::Playing
                || (self.ui.app.phase == crate::ClientPhase::Splash
                    && self.ui.app.splash_state.job
                        == crate::ui::loading_screen::SplashJob::EnterGame));
        let waiting_in_exit_loader = self.waiting_menu_return()
            && self.ui.app.phase == crate::ClientPhase::Splash
            && matches!(
                self.ui.app.splash_state.job,
                crate::ui::loading_screen::SplashJob::ExitGame
                    | crate::ui::loading_screen::SplashJob::Boot
            );
        if self.ui.app.phase != crate::ClientPhase::MainMenu
            && !waiting_in_exit_loader
            && !observing_disconnect
        {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if !self.wasm_doc_was_visible {
            return;
        }
        if self
            .reward_profile_retry_at
            .is_some_and(|retry_at| retry_at > now)
        {
            return;
        }
        self.reward_profile_retry_at = Some(now + self.reward_profile_retry_interval());
        self.fetch_cloud_progress();
    }

    pub(crate) fn mark_menu_return_error(&mut self) {
        if let Some(context) = &mut self.menu_return
            && context.account_id == self.progress_account_id
            && context.cycle_id == self.ui.app.splash_state.cycle_id
            && context.status == "waiting"
        {
            context.status = "error".to_string();
            self.save_menu_return_context();
        }
    }

    pub(crate) fn schedule_reward_profile_retry(&mut self) {
        if !self.pending_reward_receipt_ids.is_empty() || self.waiting_menu_return() {
            self.reward_profile_retry_at =
                Some(Instant::now() + self.reward_profile_retry_interval());
        }
    }

    pub(crate) fn fetch_cloud_progress(&mut self) {
        if self.fetch_match_result() {
            return;
        }
        if self.display_name_save_request_id.is_some() {
            self.profile_refresh_pending = true;
            log::debug!(
                "[identity] profile refresh queued behind rename request; account={}",
                account_hint(self.progress_account_id.as_deref())
            );
            return;
        }
        if self.profile_request_in_flight {
            self.profile_refresh_pending = true;
            log::debug!("[identity] profile refresh coalesced behind in-flight request");
            return;
        }
        let identity = crate::store_portals::load_identity("Player");
        let provider = identity.provider.to_string();
        let android_twa = crate::store_portals::is_android_twa();
        let Some(ext_id) = identity
            .external_id
            .clone()
            .filter(|id| !id.is_empty())
            .filter(|_| matches!(provider.as_str(), "crazygames" | "wou" | "playgames"))
        else {
            if android_twa {
                log::info!("[identity] Play Games unavailable; using anonymous Android profile");
            }
            self.fetch_anonymous_progress();
            return;
        };
        let request_id = self.next_identity_request_id();
        self.profile_request_in_flight = true;
        let db_url = self.asset_config.database_base.clone();

        let encoded_provider =
            url::form_urlencoded::byte_serialize(provider.as_bytes()).collect::<String>();
        let encoded_id =
            url::form_urlencoded::byte_serialize(ext_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profile?provider={}&external_id={}",
            db_url.trim_end_matches('/'),
            encoded_provider,
            encoded_id
        );

        log::info!(
            "[identity] profile request id={request_id} start provider={provider} external_id_len={}",
            ext_id.chars().count()
        );
        let tx = self.tasks.db_tx.clone();
        let profile_provider = provider.clone();
        let mut request = ehttp::Request::get(&url);
        Self::apply_platform_auth(&mut request);
        request
            .headers
            .insert("X-SOW-Identity-Request", request_id.to_string());

        ehttp::fetch(
            request,
            move |result: ehttp::Result<ehttp::Response>| match result {
                Ok(res) => {
                    if res.ok {
                        #[derive(serde::Deserialize)]
                        struct DbAccount {
                            account_id: String,
                            #[serde(default)]
                            display_name: String,
                            profile: crate::player_progress::PlayerProgress,
                        }
                        match serde_json::from_slice::<DbAccount>(&res.bytes) {
                            Ok(account) => {
                                let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                                    progress: account.profile,
                                    account_id: account.account_id,
                                    display_name: account.display_name,
                                    provider: profile_provider,
                                    request_id,
                                });
                            }
                            Err(e) => {
                                log::error!(
                                    "[identity] profile request id={request_id} parse_failed: {e}"
                                );
                                let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                                    request_id,
                                    status: Some(res.status),
                                });
                            }
                        }
                    } else {
                        log::warn!(
                            "[identity] profile request id={request_id} failed status={}",
                            res.status
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                            request_id,
                            status: Some(res.status),
                        });
                    }
                }
                Err(e) => {
                    log::error!("[identity] profile request id={request_id} network_failed: {e}");
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                        request_id,
                        status: None,
                    });
                }
            },
        );
    }

    fn fetch_match_result(&mut self) -> bool {
        let Some(context) = self.menu_return.as_ref().filter(|context| {
            context.source == "multiplayer"
                && matches!(context.status.as_str(), "waiting" | "error")
        }) else {
            return false;
        };
        if context.result_request_id.is_some() {
            return true;
        }
        let (Some(account_id), Some(match_id)) =
            (self.progress_account_id.clone(), context.match_id.clone())
        else {
            return false;
        };
        let cycle_id = context.cycle_id;
        // Voluntary returns wait straight for the reward. A playing client
        // observing a disconnect also needs the relay's closure notification.
        let closure_known = context.exit_recorded || context.voluntary_exit.unwrap_or(true);
        let provider = self.progress_provider.clone();
        let request_id = self.next_identity_request_id();
        let auth_secret = (provider == "anonymous")
            .then(crate::anonymous_identity::load_account_secret)
            .flatten();
        let body = serde_json::json!({"account_id":account_id,"match_id":match_id,"auth_secret":auth_secret,"closure_known":closure_known});
        let url = format!(
            "{}/profile/match-result",
            self.asset_config.database_base.trim_end_matches('/')
        );
        let mut request = ehttp::Request::post(
            &url,
            serde_json::to_vec(&body).expect("serializable result request"),
        );
        request.headers.insert("Content-Type", "application/json");
        Self::apply_platform_auth(&mut request);
        self.menu_return.as_mut().unwrap().result_request_id = Some(request_id);
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(request, move |result| {
            #[derive(serde::Deserialize)]
            struct MatchResult {
                account_id: String,
                match_id: String,
                #[serde(default)]
                display_name: String,
                #[serde(default)]
                profile: Option<crate::player_progress::PlayerProgress>,
                state: sow_data::profile::MatchExitState,
                #[serde(default)]
                closure_recorded: bool,
            }
            let status = result.as_ref().ok().map(|response| response.status);
            if let Ok(response) = result {
                if response.ok {
                    if let Ok(account) = serde_json::from_slice::<MatchResult>(&response.bytes) {
                        if account.account_id == account_id && account.match_id == match_id {
                            let _ = tx.send(crate::player_progress::DbEvent::MatchResultLoaded {
                                account_id,
                                match_id,
                                cycle_id,
                                request_id,
                                display_name: account.display_name,
                                provider,
                                progress: account.profile,
                                state: account.state,
                                closure_recorded: account.closure_recorded,
                            });
                            return;
                        }
                    }
                }
            }
            let _ = tx.send(crate::player_progress::DbEvent::MatchResultFailed {
                account_id,
                match_id,
                cycle_id,
                request_id,
                status,
            });
        });
        true
    }

    pub(crate) fn observe_relay_exit_recording(&mut self) {
        if !self.waiting_menu_return() {
            self.net.closing_game = None;
            return;
        }
        let mut recorded = false;
        if let Some(client) = &self.net.closing_game {
            while let Ok(bytes) = client.rx.try_recv() {
                if let Ok(sow_core::protocol::ServerMessage::ExitRecorded {
                    account_id,
                    match_id,
                    closure_turn,
                    reason,
                }) = bincode::deserialize(&bytes)
                {
                    if self.menu_return.as_ref().is_some_and(|context| {
                        context.account_id.as_deref() == Some(account_id.as_str())
                            && context.match_id.as_deref() == Some(match_id.to_string().as_str())
                    }) {
                        recorded = true;
                        log::info!(
                            "[exit-timing] relay recorded match={match_id} turn={closure_turn} reason={reason:?}"
                        );
                    }
                }
            }
        }
        if recorded {
            if let Some(context) = &mut self.menu_return {
                context.exit_recorded = true;
            }
            self.net.closing_game = None;
            self.save_menu_return_context();
        }
    }

    pub(crate) fn observe_disconnected_match(&mut self) {
        if self.waiting_menu_return()
            || self
                .menu_return
                .as_ref()
                .is_some_and(|c| c.preparation_only)
        {
            return;
        }
        let Some(match_id) = self.sim.my_lobby_id else {
            return;
        };
        self.prepare_menu_return("multiplayer", Some(match_id.to_string()));
        self.menu_return.as_mut().unwrap().voluntary_exit = Some(false);
        self.begin_reward_profile_sync(Some(match_id.to_string()), true);
        self.save_menu_return_context();
    }

    pub(crate) fn clear_disconnect_observation(&mut self) {
        if self.ui.app.phase == crate::ClientPhase::Playing
            && self.menu_return.as_ref().is_some_and(|context| {
                context.source == "multiplayer"
                    && context.voluntary_exit == Some(false)
                    && !context.preparation_only
            })
        {
            if let Some(context) = self.menu_return.take() {
                if let Some(id) = context.match_id {
                    self.pending_reward_receipt_ids.remove(&id);
                }
                #[cfg(target_arch = "wasm32")]
                if let Some(account_id) = context.account_id {
                    crate::anonymous_identity::clear_menu_return(&account_id);
                    crate::anonymous_identity::save_pending_reward_receipt_ids(
                        &account_id,
                        &self.pending_reward_receipt_ids,
                    );
                }
            }
            // Invalidate callbacks from the canceled observation by removing its context.
            self.reward_profile_retry_at = None;
        }
    }

    pub(crate) fn matches_exit_response(
        &self,
        account_id: &str,
        match_id: &str,
        cycle_id: u32,
    ) -> bool {
        self.progress_account_id.as_deref() == Some(account_id)
            && self.menu_return.as_ref().is_some_and(|context| {
                context.account_id.as_deref() == Some(account_id)
                    && context.match_id.as_deref() == Some(match_id)
                    && context.cycle_id == cycle_id
                    && matches!(context.status.as_str(), "waiting" | "error")
            })
    }

    fn fetch_anonymous_progress(&mut self) {
        let request_id = self.next_identity_request_id();
        self.profile_request_in_flight = true;
        let url = format!(
            "{}/profile/anonymous",
            self.asset_config.database_base.trim_end_matches('/')
        );
        let tx = self.tasks.db_tx.clone();
        let requested_display_name = Some(
            self.pending_display_name
                .clone()
                .unwrap_or_else(|| self.ui.app.main_menu_state.player_name.clone()),
        );
        fetch_anonymous_profile_request(
            url,
            crate::anonymous_identity::load_account_id(),
            requested_display_name,
            tx,
            true,
            request_id,
        );
    }

    pub(crate) fn save_display_name(&mut self, display_name: String) {
        let display_name = display_name.trim().to_string();
        if display_name.is_empty() {
            log::warn!("Refusing to save an empty display name");
            return;
        }
        self.pending_display_name = Some(display_name.clone());
        self.ui.app.main_menu_state.player_name = display_name.clone();
        self.ui.app.main_menu_state.error_message = None;
        crate::anonymous_identity::save_pending_display_name(
            self.progress_account_id.as_deref(),
            &display_name,
        );
        if self.profile_request_in_flight {
            log::debug!(
                "[identity] rename queued behind profile request account={}",
                account_hint(self.progress_account_id.as_deref())
            );
            return;
        }
        if self.display_name_save_request_id.is_some() {
            log::debug!("[identity] newer rename queued behind in-flight rename");
            return;
        }
        let Some(account_id) = self.progress_account_id.clone() else {
            log::debug!("Queued display-name update until the account is loaded");
            return;
        };
        let identity = crate::store_portals::load_identity("Player");
        let external_id = identity.external_id.clone().filter(|id| !id.is_empty());
        let use_platform_identity = identity.provider != "self"
            && external_id.is_some()
            && identity
                .auth_token
                .as_ref()
                .is_some_and(|token| !token.is_empty());
        if !use_platform_identity && self.progress_provider != "anonymous" {
            log::warn!(
                "Cannot save display name without the current identity provider={}",
                self.progress_provider
            );
            return;
        }
        let request_id = self.next_identity_request_id();
        let account_hint_value = account_hint(Some(&account_id));
        let requested_name_len = display_name.chars().count();
        self.display_name_save_request_id = Some(request_id);
        let url = format!(
            "{}/profile/name",
            self.asset_config.database_base.trim_end_matches('/')
        );
        #[derive(serde::Serialize)]
        struct RenameRequest {
            account_id: Option<String>,
            display_name: String,
            auth_secret: Option<String>,
            provider: Option<String>,
            external_id: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct DbAccount {
            account_id: String,
            #[serde(default)]
            display_name: String,
        }
        let auth_secret = if use_platform_identity {
            None
        } else {
            let Some(secret) = crate::anonymous_identity::load_account_secret() else {
                log::error!(
                    "[identity] rename request id={request_id} missing account secret account={account_hint_value}"
                );
                let _ =
                    self.tasks
                        .db_tx
                        .send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                            request_id,
                            status: Some(401),
                        });
                return;
            };
            Some(secret)
        };
        let body = match serde_json::to_vec(&RenameRequest {
            account_id: Some(account_id),
            display_name,
            auth_secret,
            provider: use_platform_identity.then(|| identity.provider.to_string()),
            external_id,
        }) {
            Ok(body) => body,
            Err(error) => {
                log::error!(
                    "[identity] rename request id={request_id} serialize_failed account={account_hint_value}: {error}"
                );
                let _ =
                    self.tasks
                        .db_tx
                        .send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                            request_id,
                            status: None,
                        });
                return;
            }
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        if use_platform_identity {
            Self::apply_platform_auth(&mut request);
        }
        request
            .headers
            .insert("X-SOW-Identity-Request", request_id.to_string());
        log::info!(
            "[identity] rename request id={request_id} start account={account_hint_value} name_len={requested_name_len}"
        );
        ehttp::fetch(request, move |result| match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<DbAccount>(&response.bytes) {
                    Ok(account) => {
                        if !use_platform_identity {
                            crate::anonymous_identity::save_account_id(&account.account_id);
                        }
                        log::info!(
                            "[identity] rename request id={request_id} ack account={} name_len={}",
                            account_hint(Some(&account.account_id)),
                            account.display_name.chars().count()
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaved {
                            account_id: account.account_id,
                            display_name: account.display_name,
                            request_id,
                        });
                    }
                    Err(error) => {
                        log::error!(
                            "[identity] rename request id={request_id} parse_failed: {error}"
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                            request_id,
                            status: Some(response.status),
                        });
                    }
                }
            }
            Ok(response) => {
                log::warn!(
                    "[identity] rename request id={request_id} failed status={} account={account_hint_value}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                    request_id,
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::error!("[identity] rename request id={request_id} network_failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                    request_id,
                    status: None,
                });
            }
        });
    }

    pub(crate) fn apply_progress_preferences(&mut self) {
        if self.ui.app.main_menu_state.has_explicit_leader_selection() {
            return;
        }
        if let Some(leader) = self.progress.preferred_leader {
            self.ui
                .app
                .main_menu_state
                .set_selected_leader(leader, false);
        }
        self.ui
            .app
            .main_menu_state
            .set_random_leader(self.progress.prefer_random_leader, false);
    }

    pub(crate) fn apply_cloud_profile(
        &mut self,
        cloud: crate::player_progress::PlayerProgress,
        account_id: String,
        display_name: String,
        provider: String,
    ) {
        let cloud_preferred_leader = cloud.preferred_leader.clone();
        let cloud_completed_episodes = cloud.completed_episodes.clone();
        let account_changed = self.progress_account_id.as_deref() != Some(account_id.as_str());
        let account_switch = account_changed && self.progress_account_id.is_some();
        let cloud_intro_completed = cloud.intro_completed.unwrap_or(false);
        let retry_tutorial = !account_switch
            && self.progress.intro_completed.unwrap_or(false)
            && !cloud_intro_completed;
        if account_changed {
            self.progress.reward_receipts.clear();
            self.progress.menu_guides.clear();
            self.progress.unlocked_achievements.clear();
        }
        if account_switch {
            self.pending_display_name = None;
            crate::anonymous_identity::clear_pending_display_name();
            self.pending_reward_receipt_ids.clear();
            self.reward_profile_retry_at = None;
            self.profile_action_retry_at = None;
            self.pending_campaign_exit = None;
            self.menu_return = None;
            self.name_save_revision = 0;
            self.leader_selection_revision = 0;
            self.purchase_revision = 0;
            self.quick_match_revision = 0;
            self.menu_guide_save_revision = 0;
            self.menu_guide_save_pending = false;
        }
        #[cfg(target_arch = "wasm32")]
        if self.menu_return.is_none() {
            self.menu_return =
                crate::anonymous_identity::load_menu_return(&account_id).map(|mut context| {
                    context.cycle_id = self.ui.app.splash_state.cycle_id;
                    context
                });
        }
        if retry_tutorial && self.menu_return.is_none() {
            self.prepare_menu_return("tutorial", Some("tutorial".to_string()));
        }
        if let Some(context) = &mut self.menu_return {
            if context.account_id.is_none() {
                context.account_id = Some(account_id.clone());
            }
        }
        #[cfg(target_arch = "wasm32")]
        crate::anonymous_identity::clear_legacy_exit_relay_proof(&account_id);
        if apply_menu_return_profile(&mut self.menu_return, &account_id, &cloud) {
            self.net.closing_game = None;
            if let Some((Some(match_id), prepared_at_ms)) =
                self.menu_return.as_ref().and_then(|context| {
                    Some((context.match_id.clone(), context.preparation_started_at_ms?))
                })
            {
                let confirmed_at_ms = web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                log::info!(
                    "[rewards-timing] match={match_id} stage=prepare_to_receipt elapsed_ms={}",
                    confirmed_at_ms.saturating_sub(prepared_at_ms)
                );
            }
            log::info!(
                "[exit-timing] server confirmed account={} cycle={} return_ms={}",
                account_hint(Some(&account_id)),
                self.ui.app.splash_state.cycle_id,
                self.menu_return
                    .as_ref()
                    .and_then(|context| context.started_at)
                    .map_or(0, |started| started.elapsed().as_millis())
            );
        }
        self.save_menu_return_context();
        self.pending_reward_receipt_ids.extend(
            crate::anonymous_identity::load_pending_reward_receipt_ids(&account_id),
        );
        let ready_receipts = self
            .pending_reward_receipt_ids
            .iter()
            .filter(|id| {
                cloud.reward_receipts.get(*id).is_some_and(|receipt| {
                    receipt.verification_status
                        != Some(sow_data::profile::ReplayVerificationStatus::Pending)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        let portal = self.progress.clone();
        if account_switch {
            self.progress.replace_account_profile(cloud);
        } else if retry_tutorial {
            self.progress = portal.clone();
        } else {
            self.progress.merge_boot_profile(cloud);
        }
        if account_switch {
            self.ui
                .app
                .main_menu_state
                .reset_leader_selection_override();
        }
        self.progress_account_id = Some(account_id.clone());
        self.progress_provider = provider;
        if self.pending_display_name.is_none() {
            if let Some((pending_account_id, pending_name)) =
                crate::anonymous_identity::load_pending_display_name()
            {
                if pending_account_id.is_none()
                    || pending_account_id.as_deref() == Some(account_id.as_str())
                {
                    self.pending_display_name = Some(pending_name);
                } else {
                    crate::anonymous_identity::clear_pending_display_name();
                }
            }
        }
        if self.pending_display_name.as_deref() == Some(display_name.as_str()) {
            self.pending_display_name = None;
            crate::anonymous_identity::clear_pending_display_name();
        }
        self.ui.app.main_menu_state.player_name =
            self.pending_display_name.clone().unwrap_or(display_name);
        if let Some(pending_display_name) = self.pending_display_name.clone()
            && self.display_name_save_request_id.is_none()
        {
            self.save_display_name(pending_display_name);
        }
        if !account_switch && !self.progress.has_history() && portal.has_history() {
            self.progress = portal;
        }
        self.progress.preferred_leader = Some(cloud_preferred_leader.unwrap_or_else(|| {
            sow_data::commerce::assigned_leader_for_account(
                &account_id,
                sow_data::commerce::current_rotation_period(),
            )
        }));
        if !account_switch {
            self.progress
                .pending_campaign_completions
                .retain(|episode| !cloud_completed_episodes.contains(episode));
            self.progress.pending_campaign_completions.extend(
                self.progress
                    .completed_episodes
                    .iter()
                    .filter(|episode| !cloud_completed_episodes.contains(*episode))
                    .cloned(),
            );
        }
        self.apply_progress_preferences();
        if retry_tutorial {
            log::info!(
                "[tutorial] cloud profile is missing completion; retrying one-time reward sync"
            );
            self.persist_tutorial_completion();
        }
        for receipt_id in &ready_receipts {
            self.pending_reward_receipt_ids.remove(receipt_id);
        }
        if self.waiting_menu_return() {
            if let Some(receipt_id) = self
                .menu_return
                .as_ref()
                .and_then(|context| context.match_id.clone())
            {
                self.pending_reward_receipt_ids.insert(receipt_id);
            }
        }
        for receipt in self.progress.reward_receipts.values() {
            if receipt.verification_status
                == Some(sow_data::profile::ReplayVerificationStatus::Pending)
            {
                self.pending_reward_receipt_ids.insert(receipt.id.clone());
            }
        }
        crate::anonymous_identity::save_pending_reward_receipt_ids(
            &account_id,
            &self.pending_reward_receipt_ids,
        );
        if !ready_receipts.is_empty() {
            log::info!("[rewards] receipt is visible in the main-menu profile");
        }
        if self.pending_reward_receipt_ids.is_empty() && !self.waiting_menu_return() {
            self.reward_profile_retry_at = None;
        } else {
            self.reward_profile_retry_at =
                Some(Instant::now() + self.reward_profile_retry_interval());
        }
        self.sync_wou_collectibles();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn should_portal_auto_intro(&self) -> bool {
        let mm = &self.ui.app.main_menu_state;
        if self.progress.is_first_game() {
            // A real invite link should still bypass the intro, but instant-MP host intent should not.
            mm.pending_join_lobby_id.is_none()
        } else {
            mm.pending_join_lobby_id.is_none() && !mm.host_private_pending
        }
    }

    pub(crate) fn save_local_progress(&self) {
        crate::store_portals::save_portal_progress(&self.progress);
    }

    pub(crate) fn save_menu_guide_progress(
        &mut self,
        guide_id: String,
        completed_steps: Vec<String>,
        current_step: Option<String>,
        status: sow_data::profile::MenuGuideStatus,
        selected_leader: Option<String>,
    ) {
        let fields = serde_json::json!({
            "guide_id": guide_id, "completed_steps": completed_steps, "current_step": current_step,
            "status": status, "selected_leader": selected_leader,
        });
        if let Some(fields) = fields.as_object() {
            self.post_profile_action(
                "menu guide progress",
                "/profile/menu-guide-progress",
                fields.clone(),
            );
        }
    }

    pub(crate) fn persist_leader_selection(&mut self) {
        let fields = serde_json::json!({
            "preferred_leader": self.progress.preferred_leader.map(sow_data::commerce::leader_wire_id),
            "prefer_random_leader": self.progress.prefer_random_leader,
        });
        if let Some(fields) = fields.as_object() {
            self.profile_action_retry_at = Some(Instant::now() + Duration::from_secs(2));
            self.post_profile_action(
                "leader selection",
                "/profile/leader-selection",
                fields.clone(),
            );
        }
    }

    pub(crate) fn poll_profile_actions(&mut self, now: Instant) {
        if self.ui.app.main_menu_state.store_busy
            || self
                .profile_action_retry_at
                .is_some_and(|retry_at| retry_at > now)
        {
            return;
        }
        if let Some(episode_id) = self
            .progress
            .pending_campaign_completions
            .iter()
            .next()
            .cloned()
        {
            if sow_data::campaign::episode(&episode_id).is_none() {
                return;
            }
            self.profile_action_retry_at = Some(now + Duration::from_secs(2));
            let fields = serde_json::json!({ "episode_id": episode_id });
            if let Some(fields) = fields.as_object() {
                self.post_profile_action(
                    &format!("campaign completion:{episode_id}"),
                    "/profile/campaign-complete",
                    fields.clone(),
                );
            }
            return;
        }
        if self.progress.leader_selection_pending {
            self.persist_leader_selection();
        } else {
            self.profile_action_retry_at = None;
        }
    }

    fn post_profile_action(
        &mut self,
        operation: &str,
        path: &str,
        mut fields: serde_json::Map<String, serde_json::Value>,
    ) {
        let guide_operation = operation == "menu guide progress";
        if (guide_operation && self.menu_guide_save_pending)
            || (!guide_operation && self.ui.app.main_menu_state.store_busy)
        {
            return;
        }
        let Some(account_id) = self.progress_account_id.clone() else {
            self.ui.app.main_menu_state.error_message =
                Some(crate::ui::UiText::new("profile.loading_profile"));
            return;
        };
        fields.insert(
            "account_id".into(),
            serde_json::Value::String(account_id.clone()),
        );
        if self.progress_provider == "anonymous" {
            let Some(auth_secret) = crate::anonymous_identity::load_account_secret() else {
                self.ui.app.main_menu_state.error_message =
                    Some(crate::ui::UiText::new("menu.account_setup_required"));
                return;
            };
            fields.insert("auth_secret".into(), serde_json::Value::String(auth_secret));
        }
        let Ok(body) = serde_json::to_vec(&serde_json::Value::Object(fields)) else {
            self.ui.app.main_menu_state.error_message =
                Some(crate::ui::UiText::new("store.action_unavailable"));
            return;
        };
        if guide_operation {
            self.menu_guide_save_pending = true;
        } else {
            self.ui.app.main_menu_state.store_busy = true;
            self.ui.app.main_menu_state.error_message = None;
        }
        let url = format!(
            "{}{}",
            self.asset_config.database_base.trim_end_matches('/'),
            path
        );
        let operation = operation.to_string();
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        Self::apply_platform_auth(&mut request);
        ehttp::fetch(request, move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct StoreAccount {
                    account_id: String,
                    profile: crate::player_progress::PlayerProgress,
                }
                match serde_json::from_slice::<StoreAccount>(&response.bytes) {
                    Ok(account) => {
                        let _ = tx.send(crate::player_progress::DbEvent::StoreProfileLoaded {
                            account_id: account.account_id,
                            progress: account.profile,
                            operation: operation.clone(),
                        });
                    }
                    Err(error) => {
                        log::error!("[store] {operation} response parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
                            account_id: account_id.clone(),
                            operation: operation.clone(),
                            status: Some(response.status),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!("[store] {operation} failed status={}", response.status);
                let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
                    account_id: account_id.clone(),
                    operation: operation.clone(),
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::error!("[store] {operation} network failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
                    account_id: account_id.clone(),
                    operation: operation.clone(),
                    status: None,
                });
            }
        });
    }

    pub(crate) fn acknowledge_reward_receipts(&mut self, receipt_ids: Vec<String>) {
        let Some(account_id) = self.progress_account_id.clone() else {
            return;
        };
        let auth_secret = if self.progress_provider == "anonymous" {
            let Some(secret) = crate::anonymous_identity::load_account_secret() else {
                return;
            };
            Some(secret)
        } else {
            None
        };
        let url = format!(
            "{}/profile/reward-receipts/ack",
            self.asset_config.database_base.trim_end_matches('/')
        );
        for batch in receipt_ids.chunks(32) {
            let batch = batch.to_vec();
            let mut fields = serde_json::Map::new();
            fields.insert(
                "account_id".into(),
                serde_json::Value::String(account_id.clone()),
            );
            fields.insert(
                "receipt_ids".into(),
                serde_json::Value::Array(
                    batch
                        .iter()
                        .map(|id| serde_json::Value::String(id.clone()))
                        .collect(),
                ),
            );
            if let Some(secret) = &auth_secret {
                fields.insert(
                    "auth_secret".into(),
                    serde_json::Value::String(secret.clone()),
                );
            }
            let Ok(body) = serde_json::to_vec(&serde_json::Value::Object(fields)) else {
                return;
            };
            let tx = self.tasks.db_tx.clone();
            let account_id = account_id.clone();
            let mut request = ehttp::Request::post(&url, body);
            request.headers.insert("Content-Type", "application/json");
            Self::apply_platform_auth(&mut request);
            ehttp::fetch(request, move |result| match result {
                Ok(response) if response.ok => {
                    let _ = tx.send(crate::player_progress::DbEvent::RewardReceiptsAcked {
                        account_id,
                        receipt_ids: batch,
                    });
                }
                Ok(response) => {
                    log::warn!(
                        "[rewards] receipt acknowledgement failed status={}",
                        response.status
                    );
                }
                Err(error) => log::warn!("[rewards] receipt acknowledgement failed: {error}"),
            });
        }
    }

    pub(crate) fn unlock_leader(&mut self, leader_id: String, currency: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("leader_id".into(), serde_json::Value::String(leader_id));
        fields.insert("currency".into(), serde_json::Value::String(currency));
        self.post_profile_action("leader unlock", "/store/leaders/unlock", fields);
    }

    pub(crate) fn unlock_skin(&mut self, skin_id: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("skin_id".into(), serde_json::Value::String(skin_id));
        self.post_profile_action("skin unlock", "/store/skins/unlock", fields);
    }

    pub(crate) fn equip_skin(&mut self, skin_id: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("skin_id".into(), serde_json::Value::String(skin_id));
        self.post_profile_action("skin equip", "/store/skins/equip", fields);
    }

    pub(crate) fn load_profile(&mut self) {
        let Some(account_id) = self
            .ui
            .app
            .main_menu_state
            .profile
            .account_id
            .clone()
            .or_else(|| self.profile_account_id.clone())
        else {
            self.ui.app.main_menu_state.profile.loading = false;
            self.ui.app.main_menu_state.profile.error =
                Some(crate::ui::UiText::new("profile.loading_profile"));
            return;
        };
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        self.ui.app.main_menu_state.profile.account_id = Some(account_id.clone());
        let url = format!(
            "{}/profiles/{}",
            self.asset_config.database_base.trim_end_matches('/'),
            url::form_urlencoded::byte_serialize(account_id.as_bytes()).collect::<String>()
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<sow_data::profile::PublicProfileView>(
                    &response.bytes,
                ) {
                    Ok(view) => {
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileViewLoaded {
                            account_id,
                            view,
                        });
                    }
                    Err(error) => {
                        log::error!("[profile] profile parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileLoadFailed {
                            account_id,
                            status: Some(response.status),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] profile request failed status={}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::ProfileLoadFailed {
                    account_id,
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::error!("[profile] profile request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::ProfileLoadFailed {
                    account_id,
                    status: None,
                });
            }
        });
    }

    pub(crate) fn load_profile_history(&mut self) {
        if self.ui.app.main_menu_state.profile.loading {
            return;
        }
        let Some(account_id) = self
            .ui
            .app
            .main_menu_state
            .profile
            .account_id
            .clone()
            .or_else(|| self.profile_account_id.clone())
        else {
            self.ui.app.main_menu_state.profile.error =
                Some(crate::ui::UiText::new("profile.loading_profile"));
            return;
        };
        let cursor = self.ui.app.main_menu_state.profile.history_cursor;
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        let encoded_id =
            url::form_urlencoded::byte_serialize(account_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profiles/{}/matches?cursor={cursor}&limit=20",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_id
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct MatchHistoryPage {
                    items: Vec<sow_data::profile::PublicMatchSummary>,
                    next_cursor: Option<usize>,
                }
                match serde_json::from_slice::<MatchHistoryPage>(&response.bytes) {
                    Ok(page) => {
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileHistoryLoaded {
                            account_id,
                            items: page.items,
                            next_cursor: page.next_cursor,
                        });
                    }
                    Err(error) => {
                        log::error!("[profile] history response parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                            account_id: Some(account_id),
                            operation: "match history".into(),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] history request failed status={}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: Some(account_id),
                    operation: "match history".into(),
                });
            }
            Err(error) => {
                log::error!("[profile] history request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: Some(account_id),
                    operation: "match history".into(),
                });
            }
        });
    }

    pub(crate) fn search_profiles(&mut self, query: String) {
        let query = query.trim().to_string();
        if query.is_empty() || self.ui.app.main_menu_state.profile.loading {
            return;
        }
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        let encoded_query =
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profiles/search?q={}&limit=20",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_query
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct SearchResponse {
                    items: Vec<sow_data::profile::PublicProfileSummary>,
                }
                match serde_json::from_slice::<SearchResponse>(&response.bytes) {
                    Ok(payload) => {
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileSearchLoaded {
                            query,
                            items: payload.items,
                        });
                    }
                    Err(error) => {
                        log::error!("[profile] search response parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                            account_id: None,
                            operation: "profile search".into(),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!("[profile] search request failed status={}", response.status);
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: None,
                    operation: "profile search".into(),
                });
            }
            Err(error) => {
                log::error!("[profile] search request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: None,
                    operation: "profile search".into(),
                });
            }
        });
    }

    pub(crate) fn load_match_detail(&mut self, match_id: String) {
        let match_id = match_id.trim().to_string();
        if match_id.is_empty() {
            return;
        }
        let encoded_id =
            url::form_urlencoded::byte_serialize(match_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/matches/{}",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_id
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<sow_data::profile::PublicMatchDetail>(
                    &response.bytes,
                ) {
                    Ok(detail) => {
                        let _ = tx.send(crate::player_progress::DbEvent::MatchDetailLoaded {
                            match_id,
                            detail,
                        });
                    }
                    Err(error) => {
                        log::error!("[profile] match detail response parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                            account_id: None,
                            operation: "match detail".into(),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] match detail request failed status={}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: None,
                    operation: "match detail".into(),
                });
            }
            Err(error) => {
                log::error!("[profile] match detail request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: None,
                    operation: "match detail".into(),
                });
            }
        });
    }

    /// Persist tutorial completion through the current platform proof. The
    /// server owns the one-time reward; local storage is only the retry signal.
    pub(crate) fn persist_tutorial_completion(&mut self) {
        // Remember the obligation before identity exists. A missing proof is
        // retryable work, never permission to release the return loader.
        self.track_reward_receipt_sync("tutorial");
        let Some(account_id) = self.progress_account_id.clone() else {
            return;
        };
        let identity = crate::store_portals::load_identity("Player");
        let use_platform_identity = matches!(identity.provider, "wou" | "crazygames" | "playgames")
            && identity
                .external_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
            && identity
                .auth_token
                .as_ref()
                .is_some_and(|token| !token.is_empty());
        let auth_secret = if use_platform_identity {
            None
        } else {
            crate::anonymous_identity::load_account_secret()
        };
        if !use_platform_identity && auth_secret.is_none() {
            return;
        }
        if self.profile_request_in_flight {
            return;
        }
        let request_id = self.next_identity_request_id();
        self.profile_request_in_flight = true;
        let url = format!(
            "{}/profile/tutorial-complete",
            self.asset_config.database_base.trim_end_matches('/')
        );
        #[derive(serde::Serialize)]
        struct TutorialCompleteRequest {
            account_id: String,
            auth_secret: Option<String>,
        }
        let body = match serde_json::to_vec(&TutorialCompleteRequest {
            account_id,
            auth_secret,
        }) {
            Ok(body) => body,
            Err(error) => {
                log::error!("[tutorial] completion serialize failed: {error}");
                self.profile_request_in_flight = false;
                return;
            }
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        if use_platform_identity {
            Self::apply_platform_auth(&mut request);
        }
        request
            .headers
            .insert("X-SOW-Identity-Request", request_id.to_string());
        let response_provider = if use_platform_identity {
            identity.provider.to_string()
        } else {
            "anonymous".to_string()
        };
        ehttp::fetch(request, move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct DbAccount {
                    account_id: String,
                    #[serde(default)]
                    display_name: String,
                    profile: crate::player_progress::PlayerProgress,
                }
                match serde_json::from_slice::<DbAccount>(&response.bytes) {
                    Ok(account) => {
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                            progress: account.profile,
                            account_id: account.account_id,
                            display_name: account.display_name,
                            provider: response_provider,
                            request_id,
                        });
                    }
                    Err(error) => {
                        log::error!("[tutorial] completion response parse failed: {error}");
                        let _ =
                            tx.send(crate::player_progress::DbEvent::TutorialCompletionFailed {
                                request_id,
                                status: Some(response.status),
                            });
                    }
                }
            }
            Ok(response) => {
                log::warn!(
                    "[tutorial] completion request failed status={}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::TutorialCompletionFailed {
                    request_id,
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::warn!("[tutorial] completion request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::TutorialCompletionFailed {
                    request_id,
                    status: None,
                });
            }
        });
    }
}

#[cfg(test)]
pub(crate) mod reward_exit_tests {
    use super::apply_menu_return_profile;
    use crate::app::state::MenuReturnContext;
    use crate::player_progress::PlayerProgress;
    use sow_data::profile::{ReplayVerificationStatus, RewardReceipt, RewardSettlementStatus};
    use wasm_bindgen_test::wasm_bindgen_test;

    // Run the real HUD command, Rust app and HTTP callbacks in Node. Only the
    // browser APIs and delayed database response are replaced; no preconfirmed
    // menu state is injected into the return path.
    #[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
        let saved, currentProfile, completedProfile, releaseReward, tutorialRequests, sockets, removedExitRouteRequests, resultRequests, resultReady, gameLeaveWrites, prepareResultWriteCount, gameCloseCode, gameErrors;
        export function setupTutorialExit(initial, completed) {
            const keys = ['Window', 'window', 'devicePixelRatio', 'localStorage', 'WebSocket', 'fetch',
                'matchMedia', 'SOW_MAPS_URL', 'SOW_ASSETS_URL', 'SOW_DATABASE_URL', 'SOW_WS_URL', 'SOW_BUILD_VERSION'];
            saved = keys.map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]);
            globalThis.Window = class { static [Symbol.hasInstance](value) { return value === globalThis; } };
            globalThis.window = globalThis;
            const storage = new Map();
            globalThis.localStorage = { getItem: key => storage.get(key) ?? null,
                setItem: (key, value) => storage.set(key, String(value)), removeItem: key => storage.delete(key) };
            globalThis.matchMedia = () => ({ matches: false });
            globalThis.WebSocket = class {
                constructor(url) { this.url = url; sockets.push(this); this.readyState = 0; queueMicrotask(() => { this.readyState = 1; this.onopen?.(); }); }
                send(data) {
                    const bytes = data instanceof ArrayBuffer ? new Uint8Array(data) : data;
                    if (bytes?.byteLength >= 4) {
                        const kind = new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength).getUint32(0,true);
                        if (kind === 2) gameLeaveWrites++;
                        if (kind === 13) prepareResultWriteCount++;
                    }
                }
                close() { this.readyState = 3; this.onclose?.(); }
            };
            Object.assign(globalThis, { devicePixelRatio: 1,
                SOW_MAPS_URL: 'https://journey.invalid/maps',
                SOW_ASSETS_URL: 'https://journey.invalid/assets', SOW_DATABASE_URL: 'https://journey.invalid/api',
                SOW_WS_URL: 'wss://journey.invalid/ws/' });
            currentProfile = initial; completedProfile = completed; tutorialRequests = 0;
            sockets = []; removedExitRouteRequests = 0; resultRequests = 0; resultReady = false; gameLeaveWrites = 0; prepareResultWriteCount = 0; gameCloseCode = 0; gameErrors = 0;
            globalThis.fetch = request => {
                if (request.url.endsWith('/profile/tutorial-complete')) {
                    tutorialRequests++;
                    return new Promise(resolve => { releaseReward = () => {
                        currentProfile = completedProfile;
                        resolve(new Response(currentProfile, { status: 200 }));
                    }; });
                }
                return Promise.resolve(new Response(currentProfile, { status: 200 }));
            };
            return JSON.parse(initial).account_id;
        }
        export function clickTutorialExit(source, command) {
            const handler = source.slice(source.indexOf('function handleHudClick(event) {'),
                source.indexOf('hudRoot.addEventListener("click", handleHudClick)'));
            const click = new Function('send', 'renderHud',
                'let surrenderModalOpen = true, surrenderMessage = null; ' + handler + '; return handleHudClick;')(
                type => command(JSON.stringify({ type })), () => {});
            const button = { dataset: { command: 'confirm_surrender' } };
            click({ target: { closest: selector => selector === '[data-command]' ? button : null } });
        }
        export function rewardRequestCount() { return tutorialRequests; }
        export function setupMultiplayerExit(initial, completed) {
            const accountId = setupTutorialExit(initial, completed);
            globalThis.fetch = async request => {
                if (request.url.endsWith('/profile/match-exit')) {
                    removedExitRouteRequests++;
                    return Promise.resolve(new Response('{}', { status: 404 }));
                }
                if (request.url.endsWith('/profile/match-result')) {
                    resultRequests++;
                    const command = await request.json();
                    if (resultRequests === 1) throw new Error('simulated lost result observation');
                    return Promise.resolve(new Response(JSON.stringify({ ...JSON.parse(resultReady ? completedProfile : currentProfile),
                        match_id: command.match_id, state: resultReady ? 'confirmed' : 'awaiting_result' }), { status: resultReady ? 200 : 202 }));
                }
                return Promise.resolve(new Response(currentProfile, { status: 200 }));
            };
            return accountId;
        }
        export function removedExitRouteRequestCount() { return removedExitRouteRequests; }
        export function multiplayerResultRequests() { return resultRequests; }
        export function prepareResultWrites() { return prepareResultWriteCount; }
        export function relayConnections() { return sockets.filter(socket => socket.url.includes('relay.invalid')).length; }
        export function closeMultiplayerRelay() {
            for (const socket of sockets.filter(socket => socket.url.includes('relay.invalid'))) socket.close();
        }
        export function releaseMultiplayerReward() { resultReady = true; currentProfile = completedProfile; }
        export function liveExitFixture() {
            const file = globalThis.process?.env?.SOW_LIVE_MATCH_EXIT_FILE;
            return file ? globalThis.process.getBuiltinModule('fs').readFileSync(file, 'utf8') : '';
        }
        export function setupLiveExit(fixture) {
            const config = JSON.parse(fixture);
            const transport = { fetch: globalThis.fetch, WebSocket: globalThis.WebSocket };
            if (typeof transport.WebSocket !== 'function') throw new Error('real WebSocket transport is required');
            setupTutorialExit(JSON.stringify(config.account), JSON.stringify(config.account));
            globalThis.WebSocket = class extends transport.WebSocket {
                constructor(url) {
                    super(url); sockets.push(this);
                    if (this.url === config.relay_url) {
                        this.addEventListener('error', () => gameErrors++);
                        this.addEventListener('close', event => { gameCloseCode = event.code; });
                    }
                }
                send(data) {
                    const bytes = data instanceof ArrayBuffer ? new Uint8Array(data) : data;
                    if (this.url === config.relay_url && bytes?.byteLength >= 4) {
                        const kind = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(0, true);
                        if (kind === 2) gameLeaveWrites++;
                        if (kind === 13) prepareResultWriteCount++;
                    }
                    return super.send(data);
                }
            };
            globalThis.fetch = (request, ...args) => {
                if ((request.url ?? String(request)).endsWith('/profile/match-exit')) removedExitRouteRequests++;
                return transport.fetch(request, ...args);
            };
            Object.assign(globalThis, { SOW_MAPS_URL: config.maps_base, SOW_ASSETS_URL: config.assets_base,
                SOW_DATABASE_URL: config.database_base, SOW_WS_URL: config.orchestrator_url, SOW_BUILD_VERSION: config.public_version });
            localStorage.setItem('sow_account_id', config.account.account_id);
            localStorage.setItem('sow_account_secret', config.account.auth_secret);
        }
        export function liveGameLeaveWrites() { return gameLeaveWrites; }
        export function liveGameCloseCode() { return gameCloseCode; }
        export function liveGameErrors() { return gameErrors; }
        export function liveRelayConnections(url) { return sockets.filter(socket => socket.url === url).length; }
        export function saveLiveReturn(path, state) { globalThis.process.getBuiltinModule('fs').writeFileSync(path, state, { mode: 0o600 }); }
        export function nextLiveTurn() { return new Promise(resolve => setTimeout(resolve, 10)); }
        export function releaseTutorialReward() { releaseReward(); }
        export function nextTutorialTurn() { return new Promise(resolve => setTimeout(resolve, 0)); }
        export async function restoreTutorialExit() {
            // Let queued wasm futures acquire the browser APIs before removing them.
            await nextTutorialTurn();
            for (const [key, descriptor] of saved) {
                if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key];
            }
        }
    "#)]
    extern "C" {
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = setupTutorialExit)]
        pub(crate) fn setup_tutorial_exit(initial: &str, completed: &str) -> String;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = clickTutorialExit)]
        fn click_tutorial_exit(source: &str, command: &js_sys::Function);
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = rewardRequestCount)]
        fn reward_request_count() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = setupMultiplayerExit)]
        fn setup_multiplayer_exit(initial: &str, completed: &str) -> String;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = removedExitRouteRequestCount)]
        fn removed_exit_route_request_count() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = multiplayerResultRequests)]
        fn multiplayer_result_requests() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = prepareResultWrites)]
        fn prepare_result_writes() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = relayConnections)]
        fn relay_connections() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = closeMultiplayerRelay)]
        fn close_multiplayer_relay();
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = releaseMultiplayerReward)]
        fn release_multiplayer_reward();
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = liveExitFixture)]
        fn live_exit_fixture() -> String;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = setupLiveExit)]
        fn setup_live_exit(fixture: &str);
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = liveGameLeaveWrites)]
        fn live_game_leave_writes() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = liveGameCloseCode)]
        fn live_game_close_code() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = liveGameErrors)]
        fn live_game_errors() -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = liveRelayConnections)]
        fn live_relay_connections(url: &str) -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = saveLiveReturn)]
        fn save_live_return(path: &str, state: &str);
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = nextLiveTurn)]
        fn next_live_turn() -> js_sys::Promise;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = releaseTutorialReward)]
        fn release_tutorial_reward();
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = nextTutorialTurn)]
        pub(crate) fn next_tutorial_turn() -> js_sys::Promise;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = restoreTutorialExit)]
        pub(crate) fn restore_tutorial_exit() -> js_sys::Promise;
    }

    fn defeated_match_snapshot(tick: u64) -> sow_core::protocol::SimSnapshot {
        use sow_core::{
            game::GamePhase,
            player::{Leader, PlayerType},
            protocol::{PlayerSnapshot, SimSnapshot, Team},
        };

        let player = |id, alive| PlayerSnapshot {
            id,
            name: format!("Player {id}"),
            troops: 0.0,
            max_troops: 100.0,
            gold: 100.0,
            tile_count: 0,
            centroid_x: 0.0,
            centroid_y: 0.0,
            player_type: PlayerType::Human,
            color: [0.5; 3],
            team: Some(Team::Red),
            has_spawned: true,
            alive,
            iq: 100,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            alliance_requests: Vec::new(),
            resource_requests: Vec::new(),
            disconnected: false,
            active_emote: None,
            can_express_emote: false,
            can_retaliate: false,
            traitor: false,
            leader: Leader::of("caesar"),
            campaign_avatar: None,
            is_campaign_faction: false,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            boats_in_use: 0,
            boat_capacity: 3,
            nuke_available: false,
            nuke_cooldown_ticks: 0,
        };
        SimSnapshot {
            tick,
            phase: GamePhase::Playing,
            spawn_timer_secs: None,
            players: vec![player(1, false), player(2, true)],
            dirty_tiles: Vec::new(),
            fleets: Vec::new(),
            attacks: Vec::new(),
            buildings: Vec::new(),
            projectiles: Vec::new(),
            nuke_alerts: Vec::new(),
            resource_transfers: Vec::new(),
            resource_rejections: Vec::new(),
            resource_transfer_rejections: Vec::new(),
            winner: None,
            winning_team: None,
            defense_posts: Vec::new(),
            defense_dirty: false,
            total_land_tiles: 10_000,
            sea_lanes: std::sync::Arc::new(Vec::new()),
            debug_mem_info: String::new(),
        }
    }

    #[wasm_bindgen_test]
    async fn hud_leave_tutorial_requests_reward_before_releasing_return() {
        use wasm_bindgen::{JsCast, closure::Closure};
        let account_id = "0123456789abcdef0123456789abcdef";
        let initial = serde_json::json!({ "account_id": account_id, "auth_secret": "audit-proof",
            "display_name": "Audit", "profile": PlayerProgress::default() })
        .to_string();
        let mut completed = profile_with("tutorial", None, RewardSettlementStatus::Applied);
        completed.intro_completed = Some(true);
        completed.xp = 100;
        completed.level = 2;
        completed.crowns = 100;
        completed
            .reward_receipts
            .get_mut("tutorial")
            .unwrap()
            .level_before = Some(1);
        completed
            .reward_receipts
            .get_mut("tutorial")
            .unwrap()
            .level_after = Some(2);
        completed
            .menu_guides
            .insert("welcome".to_string(), Default::default());
        let response = serde_json::json!({ "account_id": account_id,
            "display_name": "Audit", "profile": completed })
        .to_string();
        let account_id = setup_tutorial_exit(&initial, &response);
        let mut app = crate::app::SowApp::new();
        for _ in 0..200 {
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if app.profile_last_applied_request > 0 {
                break;
            }
        }
        assert!(
            app.profile_last_applied_request > 0,
            "boot must finish its actual profile request"
        );
        assert_eq!(
            app.progress_account_id.as_deref(),
            Some(account_id.as_str())
        );
        assert_eq!(app.progress.xp, 0);
        assert!(app.progress.menu_guides.is_empty());
        app.ui.app.phase = crate::ClientPhase::Playing;
        app.net.is_offline = true;
        app.ui.tutorial_active = true;
        app.ui.tutorial_campaign = crate::campaign::CampaignId::Boudica;
        let command = Closure::<dyn FnMut(String)>::new(crate::web_menu::sow_menu_command);
        click_tutorial_exit(
            include_str!("../../../sow-web/shell/main_menu.hud.js"),
            command.as_ref().unchecked_ref(),
        );
        app.process_web_menu_commands();
        assert_eq!(app.ui.app.phase, crate::ClientPhase::Splash);
        assert_eq!(
            app.ui.app.splash_state.job,
            crate::ui::loading_screen::SplashJob::ExitGame
        );
        assert!(app.waiting_menu_return());
        assert_eq!(app.menu_return.as_ref().unwrap().source, "tutorial");
        assert_eq!(app.menu_return.as_ref().unwrap().status, "waiting");
        assert_eq!(app.progress.xp, 0);
        wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
            .await
            .unwrap();
        assert_eq!(
            reward_request_count(),
            1,
            "the actual HUD exit must request its reward"
        );
        app.update_assets();
        assert!(
            app.waiting_menu_return(),
            "a delayed server cannot release the return"
        );
        release_tutorial_reward();
        // The exit's queued refresh must also finish through the real HTTP callbacks.
        for _ in 0..200 {
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if !app.waiting_menu_return() {
                break;
            }
        }
        assert!(!app.waiting_menu_return());
        assert_eq!(app.menu_return.as_ref().unwrap().status, "confirmed");
        assert_eq!(app.menu_return.as_ref().unwrap().receipt_ids, ["tutorial"]);
        assert_eq!(
            (app.progress.xp, app.progress.level, app.progress.crowns),
            (100, 2, 100)
        );
        assert!(app.progress.menu_guides.contains_key("welcome"));
        app.ui.app.phase = crate::ClientPhase::MainMenu;
        crate::web_menu::publish_state(&mut app);
        let state: serde_json::Value = serde_json::from_str(
            &js_sys::Reflect::get(&js_sys::global(), &"SOW_MENU_STATE".into())
                .unwrap()
                .as_string()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(state["menu_return"]["status"], "confirmed");
        assert_eq!(state["reward_receipts"][0]["id"], "tutorial");
        assert_eq!(state["menu_guides"]["welcome"]["status"], "active");
        drop(app);
        wasm_bindgen_futures::JsFuture::from(restore_tutorial_exit())
            .await
            .unwrap();
    }

    #[wasm_bindgen_test]
    async fn multiplayer_exit_survives_closed_port_delay_and_reload() {
        let live = live_exit_fixture();
        if !live.is_empty() {
            live_multiplayer_exit(&live).await;
            return;
        }
        let account_id = "0123456789abcdef0123456789abcdef";
        let mut initial_progress = PlayerProgress::default();
        initial_progress.xp = 120;
        initial_progress.level = 2;
        initial_progress.intro_completed = Some(true);
        let initial = serde_json::json!({"account_id":account_id,"auth_secret":"audit-proof",
            "display_name":"Audit","profile":initial_progress})
        .to_string();
        let mut completed = profile_with(
            "42",
            Some(ReplayVerificationStatus::Verified),
            RewardSettlementStatus::Applied,
        );
        completed.xp = 176;
        completed.level = 2;
        completed.intro_completed = Some(true);
        let final_profile =
            serde_json::json!({"account_id":account_id,"display_name":"Audit","profile":completed})
                .to_string();
        setup_multiplayer_exit(&initial, &final_profile);
        let mut app = crate::app::SowApp::new();
        for _ in 0..200 {
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if app.profile_last_applied_request > 0 {
                break;
            }
        }
        app.ui.app.phase = crate::ClientPhase::Playing;
        app.ui.tutorial_active = false;
        app.net.is_offline = false;
        app.sim.my_lobby_id = Some(42);
        app.sim.my_player_id = Some(1);
        app.sim.relay_reconnect_ticket = Some("private-test-ticket".into());
        app.net.ws_url = "wss://relay.invalid:25594/ws/".into();
        app.net.client = Some(
            sow_net::client::SowClient::connect(&app.net.ws_url)
                .await
                .unwrap(),
        );
        let original_connections = relay_connections();
        app.process_ui_actions(Some(crate::UiAction::ReturnToMenu));
        close_multiplayer_relay();
        app.ui.app.main_menu_state.is_connecting = true;
        app.net
            .connect_tx
            .send((
                "wss://relay.invalid:25594/ws/".into(),
                Err("late game connection failure".into()),
            ))
            .unwrap();
        app.update_net(web_time::Instant::now());
        assert!(
            app.ui.app.main_menu_state.is_connecting,
            "an obsolete game connection must not reset the pending menu connection"
        );
        app.ui.app.main_menu_state.is_connecting = false;
        for step in 1..9 {
            let now = web_time::Instant::now() + web_time::Duration::from_secs(step * 3);
            app.update_net(now);
            app.poll_reward_profile_sync(now);
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
        }
        assert!(
            multiplayer_result_requests() > 0,
            "return waits through read-only server observation"
        );
        assert_eq!(
            removed_exit_route_request_count(),
            0,
            "the server has no exit command endpoint"
        );
        assert_eq!(
            relay_connections(),
            original_connections,
            "waiting for rewards must never reconnect to the closed game port"
        );
        assert!(app.waiting_menu_return());
        assert_eq!(app.progress.xp, 120);
        let cycle = app.menu_return.as_ref().unwrap().cycle_id;
        assert!(!app.matches_exit_response("another-account", "42", cycle));
        assert!(!app.matches_exit_response(account_id, "43", cycle));
        assert!(!app.matches_exit_response(account_id, "42", cycle.wrapping_add(1)));
        drop(app);
        let mut app = crate::app::SowApp::new();
        for _ in 0..200 {
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if app.profile_last_applied_request > 0 {
                break;
            }
        }
        assert!(
            app.waiting_menu_return(),
            "reload must recover the pending server-owned return"
        );
        release_multiplayer_reward();
        for _ in 0..200 {
            app.poll_reward_profile_sync(
                web_time::Instant::now() + web_time::Duration::from_secs(60),
            );
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if !app.waiting_menu_return() {
                break;
            }
        }
        assert!(!app.waiting_menu_return());
        assert_eq!(app.progress.xp, 176);
        assert_eq!(app.menu_return.as_ref().unwrap().status, "confirmed");
        assert_eq!(relay_connections(), original_connections);
        drop(app);
        wasm_bindgen_futures::JsFuture::from(restore_tutorial_exit())
            .await
            .unwrap();
    }

    #[wasm_bindgen_test]
    async fn preparation_keeps_playing_and_observing_without_leave_or_exit_command() {
        let account = "0123456789abcdef0123456789abcdef";
        let mut progress = PlayerProgress::default();
        progress.intro_completed = Some(true);
        let initial = serde_json::json!({"account_id":account,"auth_secret":"audit-proof","display_name":"Audit","profile":progress}).to_string();
        let completed = serde_json::json!({"account_id":account,"display_name":"Audit","profile":profile_with("42",Some(ReplayVerificationStatus::Verified),RewardSettlementStatus::Applied)}).to_string();
        setup_multiplayer_exit(&initial, &completed);
        let mut app = crate::app::SowApp::new();
        for _ in 0..200 {
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if app.profile_last_applied_request > 0 {
                break;
            }
        }
        app.ui.app.phase = crate::ClientPhase::Playing;
        app.net.is_offline = false;
        app.ui.tutorial_active = false;
        app.sim.my_lobby_id = Some(42);
        app.sim.my_player_id = Some(1);
        app.net.ws_url = "wss://relay.invalid:25594/ws/".into();
        app.net.client = Some(
            sow_net::client::SowClient::connect(&app.net.ws_url)
                .await
                .unwrap(),
        );
        let connections = relay_connections();
        let defeated = defeated_match_snapshot(701);
        app.maybe_record_match_progress(
            &defeated,
            defeated.winner,
            defeated.winning_team,
            Some(sow_core::protocol::Team::Red),
        );
        assert!(app.progress_match_recorded);
        assert_eq!(
            prepare_result_writes(),
            1,
            "defeat sends one preparation hint"
        );
        let preparation_started_at_ms = app
            .menu_return
            .as_ref()
            .unwrap()
            .preparation_started_at_ms
            .expect("result preparation must start its timing record");
        for _ in 0..8 {
            app.poll_reward_profile_sync(
                web_time::Instant::now() + web_time::Duration::from_secs(120),
            );
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            assert_eq!(app.ui.app.phase, crate::ClientPhase::Playing);
            assert!(app.net.closing_game.is_none());
            assert_eq!(removed_exit_route_request_count(), 0);
            assert_eq!(live_game_leave_writes(), 0);
            assert_eq!(
                prepare_result_writes(),
                1,
                "preparation is not resent while observing"
            );
        }
        app.ui.is_spectating = true;
        release_multiplayer_reward();
        for _ in 0..200 {
            app.poll_reward_profile_sync(
                web_time::Instant::now() + web_time::Duration::from_secs(240),
            );
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if !app.waiting_menu_return() {
                break;
            }
        }
        assert_eq!(app.menu_return.as_ref().unwrap().status, "confirmed");
        assert_eq!(app.ui.app.phase, crate::ClientPhase::Playing);
        assert!(app.ui.is_spectating);
        assert_eq!(removed_exit_route_request_count(), 0);
        assert_eq!(live_game_leave_writes(), 0);
        app.menu_return.as_mut().unwrap().exit_recorded = true;
        app.process_ui_actions(Some(crate::UiAction::ReturnToMenu));
        assert!(!app.menu_return.as_ref().unwrap().preparation_only);
        assert_eq!(
            app.menu_return.as_ref().unwrap().preparation_started_at_ms,
            Some(preparation_started_at_ms),
            "returning to the menu must preserve the time when result preparation began"
        );
        assert_eq!(app.menu_return.as_ref().unwrap().status, "confirmed");
        assert_eq!(relay_connections(), connections);
        assert_eq!(
            live_game_leave_writes(),
            0,
            "prepared closure must not send a new surrender"
        );
        drop(app);
        wasm_bindgen_futures::JsFuture::from(restore_tutorial_exit())
            .await
            .unwrap();
    }

    #[wasm_bindgen_test]
    async fn natural_finish_keeps_endgame_open_until_the_player_returns() {
        let account = "0123456789abcdef0123456789abcdef";
        let initial = serde_json::json!({"account_id":account,"display_name":"Audit","profile":PlayerProgress::default()}).to_string();
        let completed = serde_json::json!({"account_id":account,"display_name":"Audit",
            "profile":profile_with("42",Some(ReplayVerificationStatus::Verified),RewardSettlementStatus::Applied)}).to_string();
        setup_multiplayer_exit(&initial, &completed);
        let mut app = crate::app::SowApp::new();
        app.ui.app.phase = crate::ClientPhase::Playing;
        app.net.is_offline = false;
        app.sim.my_lobby_id = Some(42);
        app.progress_account_id = Some(account.into());
        app.net.ws_url = "wss://relay.invalid:25594/ws/".into();
        app.net.client = Some(
            sow_net::client::SowClient::connect(&app.net.ws_url)
                .await
                .unwrap(),
        );
        app.menu_return = None;
        app.handle_result_notifications(None, Some(701));
        for _ in 0..8 {
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
        }
        assert_eq!(app.ui.app.phase, crate::ClientPhase::Playing);
        assert!(app.finished_relay_match());
        assert!(app.menu_return.as_ref().unwrap().match_finished);
        assert!(app.menu_return.as_ref().unwrap().preparation_only);
        assert_eq!(
            prepare_result_writes(),
            1,
            "natural finish sends one preparation hint"
        );
        assert_eq!(
            live_game_leave_writes(),
            0,
            "natural finish does not send Leave"
        );
        assert_eq!(
            removed_exit_route_request_count(),
            0,
            "natural completion must not request a new exit"
        );
        drop(app);
        wasm_bindgen_futures::JsFuture::from(restore_tutorial_exit())
            .await
            .unwrap();
    }

    #[wasm_bindgen_test]
    async fn connection_loss_observes_relay_closure_without_ordering_exit() {
        let account_id = "0123456789abcdef0123456789abcdef";
        let mut initial_progress = PlayerProgress::default();
        initial_progress.intro_completed = Some(true);
        let initial = serde_json::json!({"account_id":account_id,"auth_secret":"audit-proof",
            "display_name":"Audit","profile":initial_progress})
        .to_string();
        let mut completed = profile_with(
            "42",
            Some(ReplayVerificationStatus::Verified),
            RewardSettlementStatus::Applied,
        );
        completed.intro_completed = Some(true);
        let completed =
            serde_json::json!({"account_id":account_id,"display_name":"Audit","profile":completed})
                .to_string();
        setup_multiplayer_exit(&initial, &completed);
        let mut app = crate::app::SowApp::new();
        for _ in 0..200 {
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if app.profile_last_applied_request > 0 {
                break;
            }
        }
        app.ui.app.phase = crate::ClientPhase::Playing;
        app.ui.tutorial_active = false;
        app.net.is_offline = false;
        app.net.relay_handoff_done = true;
        app.sim.my_lobby_id = Some(42);
        app.sim.my_player_id = Some(1);
        app.net.ws_url = "wss://relay.invalid:25594/ws/".into();
        app.ui.app.main_menu_state.server_address = app.net.ws_url.clone();
        app.net.client = Some(
            sow_net::client::SowClient::connect(&app.net.ws_url)
                .await
                .unwrap(),
        );
        close_multiplayer_relay();
        for second in [1, 30, 90] {
            let now = web_time::Instant::now() + web_time::Duration::from_secs(second);
            app.update_net(now);
            app.poll_reward_profile_sync(now);
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            assert_eq!(
                app.ui.app.phase,
                crate::ClientPhase::Playing,
                "local elapsed time cannot abandon the player"
            );
            assert_eq!(removed_exit_route_request_count(), 0);
        }
        assert_eq!(
            app.menu_return.as_ref().unwrap().voluntary_exit,
            Some(false)
        );
        release_multiplayer_reward();
        for _ in 0..200 {
            app.poll_reward_profile_sync(
                web_time::Instant::now() + web_time::Duration::from_secs(120),
            );
            wasm_bindgen_futures::JsFuture::from(next_tutorial_turn())
                .await
                .unwrap();
            app.update_assets();
            if !app.waiting_menu_return() {
                break;
            }
        }
        assert_eq!(app.ui.app.phase, crate::ClientPhase::Splash);
        assert_eq!(app.menu_return.as_ref().unwrap().status, "confirmed");
        assert_eq!(
            removed_exit_route_request_count(),
            0,
            "a relay-decided exit never creates a voluntary command"
        );
        drop(app);
        wasm_bindgen_futures::JsFuture::from(restore_tutorial_exit())
            .await
            .unwrap();
    }

    async fn live_multiplayer_exit(fixture: &str) {
        console_log::init_with_level(log::Level::Info).expect("live client logger");
        #[derive(serde::Deserialize)]
        struct LiveMatch {
            match_id: u64,
            player_id: u16,
            relay_url: String,
            relay_ticket: String,
            state_path: String,
        }
        let config: LiveMatch = serde_json::from_str(fixture).unwrap();
        setup_live_exit(fixture);
        let mut app = crate::app::SowApp::new();
        let deadline = web_time::Instant::now() + web_time::Duration::from_secs(110);
        while app.profile_last_applied_request == 0 {
            assert!(
                web_time::Instant::now() < deadline,
                "live profile bootstrap timed out"
            );
            wasm_bindgen_futures::JsFuture::from(next_live_turn())
                .await
                .unwrap();
            app.update_assets();
        }
        app.net.ws_url = config.relay_url.clone();
        let client = sow_net::client::SowClient::connect(&config.relay_url)
            .await
            .unwrap();
        client.send(
            bincode::serialize(&sow_core::protocol::ClientMessage::ReadyWithTicket {
                lobby_id: config.match_id,
                player_id: config.player_id,
                ticket: config.relay_ticket,
            })
            .unwrap(),
        );
        let mut received_turn = false;
        while !received_turn {
            assert!(
                web_time::Instant::now() < deadline,
                "live relay never delivered a turn"
            );
            while let Ok(bytes) = client.rx.try_recv() {
                if matches!(
                    bincode::deserialize::<sow_core::protocol::ServerMessage>(&bytes),
                    Ok(sow_core::protocol::ServerMessage::Turn(_))
                ) {
                    received_turn = true;
                }
            }
            wasm_bindgen_futures::JsFuture::from(next_live_turn())
                .await
                .unwrap();
        }
        app.net.client = Some(client);
        app.sim.my_lobby_id = Some(config.match_id);
        app.sim.my_player_id = Some(config.player_id);
        app.net.is_offline = false;
        app.ui.tutorial_active = false;
        app.ui.app.phase = crate::ClientPhase::Playing;
        let connections = live_relay_connections(&config.relay_url);
        assert_eq!(connections, 1);
        use wasm_bindgen::{JsCast, closure::Closure};
        let command = Closure::<dyn FnMut(String)>::new(crate::web_menu::sow_menu_command);
        click_tutorial_exit(
            include_str!("../../../sow-web/shell/main_menu.hud.js"),
            command.as_ref().unchecked_ref(),
        );
        app.process_web_menu_commands();
        assert!(app.waiting_menu_return());
        while app.waiting_menu_return() || app.ui.app.splash_state.gpu_load_step < 2 {
            assert!(
                web_time::Instant::now() < deadline,
                "live exit never received its final server receipt"
            );
            let now = web_time::Instant::now();
            app.update_net(now);
            app.update_assets();
            app.poll_reward_profile_sync(now);
            app.update_loader();
            if app.waiting_menu_return() {
                assert!(
                    app.ui.app.splash_state.gpu_load_step < 2,
                    "loader advanced before the real receipt"
                );
            }
            assert_eq!(
                live_relay_connections(&config.relay_url),
                connections,
                "exit opened a game connection while waiting for the server"
            );
            wasm_bindgen_futures::JsFuture::from(next_live_turn())
                .await
                .unwrap();
        }
        assert_eq!(app.menu_return.as_ref().unwrap().status, "confirmed");
        let return_elapsed = app
            .menu_return
            .as_ref()
            .and_then(|context| context.started_at)
            .expect("live return timing must start when the player exits")
            .elapsed();
        assert!(
            return_elapsed <= web_time::Duration::from_secs(2),
            "live WASM exit receipt exceeded 2 seconds: {} ms",
            return_elapsed.as_millis()
        );
        assert_eq!(
            live_game_leave_writes(),
            1,
            "the actual HUD exit sends one relay command"
        );
        assert_eq!(
            removed_exit_route_request_count(),
            0,
            "normal exit must never order exit through the server"
        );
        assert!(app.net.closing_game.is_none());
        while live_game_close_code() == 0 {
            assert!(
                web_time::Instant::now() < deadline,
                "game connection did not close"
            );
            wasm_bindgen_futures::JsFuture::from(next_live_turn())
                .await
                .unwrap();
        }
        assert_eq!(
            live_game_close_code(),
            1000,
            "normal exit closed the game connection abruptly"
        );
        assert_eq!(
            live_game_errors(),
            0,
            "normal exit raised a game connection error"
        );
        assert!(
            app.progress
                .reward_receipts
                .contains_key(&config.match_id.to_string())
        );
        // Rendering is outside this headless check; deliver its completed-frame
        // count to the real loader and assert the destination it authorizes.
        app.ui.app.splash_state.frames_drawn = 2;
        app.update_loader();
        assert_eq!(app.ui.app.phase, crate::ClientPhase::MainMenu);
        crate::web_menu::publish_state(&mut app);
        let state = js_sys::Reflect::get(&js_sys::global(), &"SOW_MENU_STATE".into())
            .unwrap()
            .as_string()
            .unwrap();
        save_live_return(&config.state_path, &state);
        drop(app);
        wasm_bindgen_futures::JsFuture::from(restore_tutorial_exit())
            .await
            .unwrap();
    }

    fn wait(receipt_id: &str, requires_verification: bool) -> Option<MenuReturnContext> {
        Some(MenuReturnContext {
            account_id: Some("account-a".to_string()),
            match_id: Some(receipt_id.to_string()),
            source: if requires_verification {
                "multiplayer"
            } else {
                "tutorial"
            }
            .into(),
            status: "waiting".into(),
            server_state: Some(sow_data::profile::MatchExitState::Confirmed),
            ..Default::default()
        })
    }

    #[wasm_bindgen_test]
    fn a_visible_balance_cannot_replace_the_server_return_confirmation() {
        let mut expected = wait("42", true);
        expected.as_mut().unwrap().server_state =
            Some(sow_data::profile::MatchExitState::Verifying);
        let profile = profile_with(
            "42",
            Some(ReplayVerificationStatus::Verified),
            RewardSettlementStatus::Applied,
        );
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile
        ));
        expected.as_mut().unwrap().server_state =
            Some(sow_data::profile::MatchExitState::Confirmed);
        assert!(apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile
        ));
    }

    fn profile_with(
        receipt_id: &str,
        verification_status: Option<ReplayVerificationStatus>,
        status: RewardSettlementStatus,
    ) -> PlayerProgress {
        let mut profile = PlayerProgress::default();
        profile.reward_receipts.insert(
            receipt_id.to_string(),
            RewardReceipt {
                id: receipt_id.to_string(),
                match_id: receipt_id.to_string(),
                verification_status,
                status,
                ..Default::default()
            },
        );
        profile
    }

    #[wasm_bindgen_test]
    fn exit_wait_stays_until_the_exact_account_match_is_final() {
        let mut expected = wait("match-a", true);
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &PlayerProgress::default()
        ));
        assert!(expected.is_some());
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-b",
            &profile_with(
                "match-a",
                Some(ReplayVerificationStatus::Verified),
                RewardSettlementStatus::Applied,
            )
        ));
        assert!(expected.is_some());
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile_with(
                "match-a",
                Some(ReplayVerificationStatus::Pending),
                RewardSettlementStatus::Applied,
            )
        ));
        assert!(expected.is_some());
        assert!(apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile_with(
                "match-a",
                Some(ReplayVerificationStatus::Verified),
                RewardSettlementStatus::Applied,
            )
        ));
        assert_eq!(expected.as_ref().unwrap().status, "confirmed");

        let mut rejected = wait("match-a", true);
        assert!(apply_menu_return_profile(
            &mut rejected,
            "account-a",
            &profile_with(
                "match-a",
                Some(ReplayVerificationStatus::Rejected),
                RewardSettlementStatus::Applied,
            )
        ));
        assert_eq!(rejected.as_ref().unwrap().status, "confirmed");
    }

    #[wasm_bindgen_test]
    fn another_match_receipt_cannot_release_the_exit_loader() {
        let profile = profile_with(
            "match-a",
            Some(ReplayVerificationStatus::Verified),
            RewardSettlementStatus::Applied,
        );
        let mut expected = wait("match-b", true);
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile
        ));
        assert!(expected.is_some());
    }

    #[wasm_bindgen_test]
    fn verified_but_unapplied_receipt_cannot_release_loader() {
        let mut expected = wait("match-a", true);
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile_with(
                "match-a",
                Some(ReplayVerificationStatus::Verified),
                RewardSettlementStatus::Pending
            )
        ));
        assert!(expected.is_some());
    }

    #[wasm_bindgen_test]
    fn delayed_profile_cannot_rewind_completed_menu_guide() {
        use sow_data::profile::{MenuGuideProgress, MenuGuideStatus};
        let mut progress = PlayerProgress::default();
        progress.menu_guides.insert(
            "welcome".to_string(),
            MenuGuideProgress {
                completed_steps: ["welcome_name".to_string(), "welcome_complete".to_string()]
                    .into_iter()
                    .collect(),
                status: MenuGuideStatus::Completed,
                selected_leader: None,
                ..Default::default()
            },
        );
        let mut cloud = PlayerProgress::default();
        cloud
            .menu_guides
            .insert("welcome".to_string(), MenuGuideProgress::default());
        progress.merge_boot_profile(cloud);
        assert_eq!(
            progress.menu_guides["welcome"].status,
            MenuGuideStatus::Completed
        );
        assert!(
            progress.menu_guides["welcome"]
                .completed_steps
                .contains("welcome_name")
        );
    }

    #[wasm_bindgen_test]
    fn confirmed_first_match_context_survives_delayed_snapshots() {
        use sow_data::profile::MenuGuideProgress;
        let mut progress = PlayerProgress::default();
        let confirmed = MenuGuideProgress {
            source_match_id: Some("match-a".to_string()),
            ..Default::default()
        };
        progress.merge_menu_guides(std::collections::BTreeMap::from([(
            "first_multiplayer".to_string(),
            confirmed,
        )]));
        assert_eq!(
            progress.menu_guides["first_multiplayer"]
                .source_match_id
                .as_deref(),
            Some("match-a")
        );
        progress.merge_menu_guides(std::collections::BTreeMap::from([(
            "first_multiplayer".to_string(),
            MenuGuideProgress::default(),
        )]));
        assert_eq!(
            progress.menu_guides["first_multiplayer"]
                .source_match_id
                .as_deref(),
            Some("match-a")
        );
    }

    #[wasm_bindgen_test]
    fn tutorial_obligation_survives_identity_arriving_after_exit() {
        let mut expected = wait("tutorial", false);
        expected.as_mut().unwrap().account_id = None;
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &PlayerProgress::default()
        ));
        assert_eq!(
            expected
                .as_ref()
                .and_then(|wait| wait.account_id.as_deref()),
            Some("account-a")
        );
        let receipt = profile_with("tutorial", None, RewardSettlementStatus::Applied);
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-b",
            &receipt
        ));
        assert!(apply_menu_return_profile(
            &mut expected,
            "account-a",
            &receipt
        ));
        assert_eq!(expected.as_ref().unwrap().status, "confirmed");
    }

    #[wasm_bindgen_test]
    fn tutorial_exit_waits_for_its_applied_server_receipt() {
        let mut expected = wait("tutorial", false);
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &PlayerProgress::default()
        ));
        assert!(expected.is_some());
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-b",
            &profile_with("tutorial", None, RewardSettlementStatus::Applied,)
        ));
        assert!(expected.is_some());
        assert!(!apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile_with("tutorial", None, RewardSettlementStatus::Pending,)
        ));
        assert!(expected.is_some());
        assert!(apply_menu_return_profile(
            &mut expected,
            "account-a",
            &profile_with("tutorial", None, RewardSettlementStatus::Applied,)
        ));
        assert_eq!(expected.as_ref().unwrap().status, "confirmed");
    }
}
