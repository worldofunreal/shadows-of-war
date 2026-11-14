//! Authoritative profile and return handoff.
use super::state::{MenuReturnContext, SowApp};
use web_time::{Duration, Instant};

const REWARD_PROFILE_RETRY_INTERVAL: Duration = Duration::from_secs(30);
const EXIT_REWARD_PROFILE_RETRY_INTERVAL: Duration = Duration::from_millis(250);

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
            provider: "anonymous".to_string(),
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
                        provider: "anonymous".to_string(),
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
                provider: "anonymous".to_string(),
                status: Some(res.status),
            });
        }
        Err(error) => {
            log::error!("[identity] profile request id={request_id} network_failed: {error}");
            let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                request_id,
                provider: "anonymous".to_string(),
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

    pub(crate) fn schedule_identity_profile_retry(&mut self) {
        let delay = self.identity_profile_retry_delay;
        self.identity_profile_retry_at = Some(Instant::now() + delay);
        self.identity_profile_retry_delay =
            Duration::from_secs(delay.as_secs().saturating_mul(2).min(30));
    }

    pub(crate) fn poll_identity_profile_retry(&mut self, now: Instant) {
        if self.ui.app.phase != crate::ClientPhase::MainMenu
            || !self
                .identity_profile_retry_at
                .is_some_and(|retry_at| retry_at <= now)
        {
            return;
        }
        self.identity_profile_retry_at = None;
        self.fetch_cloud_progress();
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
            .filter(|_| matches!(provider.as_str(), "crazygames" | "sow" | "playgames"))
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
                                    provider: profile_provider.clone(),
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
                            provider: profile_provider.clone(),
                            status: Some(res.status),
                        });
                    }
                }
                Err(e) => {
                    log::error!("[identity] profile request id={request_id} network_failed: {e}");
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                        request_id,
                        provider: profile_provider.clone(),
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
                #[serde(default)]
                ranked_result: Option<sow_data::profile::RankedResultPresentation>,
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
                                ranked_result: account.ranked_result,
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
        if let Some(leader) = self
            .progress
            .preferred_leader
            .filter(|leader| leader.is_enabled())
        {
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
            self.quick_match_revision = 0;
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
            sow_core::player::Leader::all()
                .find(|leader| leader.always_free())
                .unwrap_or_default()
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
        menu_tips_disabled: Option<bool>,
    ) {
        if let Some(disabled) = menu_tips_disabled {
            self.progress.menu_tips_disabled = disabled;
        }
        let mut fields = serde_json::json!({
            "guide_id": guide_id, "completed_steps": completed_steps, "current_step": current_step,
            "status": status, "selected_leader": selected_leader,
        });
        if let Some(disabled) = menu_tips_disabled {
            fields["menu_tips_disabled"] = serde_json::Value::Bool(disabled);
        }
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

    pub(crate) fn unlock_leader(
        &mut self,
        leader_id: String,
        currency: String,
        offer_id: Option<String>,
    ) {
        let mut fields = serde_json::Map::new();
        fields.insert("leader_id".into(), serde_json::Value::String(leader_id));
        fields.insert("currency".into(), serde_json::Value::String(currency));
        if let Some(offer_id) = offer_id {
            fields.insert("offer_id".into(), serde_json::Value::String(offer_id));
        }
        self.post_profile_action("leader unlock", "/store/leaders/unlock", fields);
    }

    pub(crate) fn double_match_reward(&mut self, match_id: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("match_id".into(), serde_json::Value::String(match_id));
        self.post_profile_action(
            "reward double",
            "/profile/reward-receipts/double",
            fields,
        );
    }

    pub(crate) fn claim_mission_reward(&mut self, mission_id: &str, doubled_by_ad: bool) {
        // The server owns crowns, gems and the claimed flag. Nothing is credited here:
        // the balance changes only when the server's profile arrives, so a rejected
        // claim leaves no reward behind.
        let known = sow_data::missions::ONBOARDING_MISSIONS
            .iter()
            .chain(sow_data::missions::DAILY_MISSION_POOL.iter())
            .chain(sow_data::missions::WEEKLY_MISSION_POOL.iter())
            .any(|m| m.id == mission_id);
        let claimed = self
            .progress
            .missions_progress
            .get(mission_id)
            .is_some_and(|record| record.claimed);
        if !known || claimed {
            return;
        }
        let mut fields = serde_json::Map::new();
        fields.insert(
            "mission_id".into(),
            serde_json::Value::String(mission_id.to_string()),
        );
        fields.insert(
            "doubled_by_ad".into(),
            serde_json::Value::Bool(doubled_by_ad),
        );
        self.post_profile_action("claim mission", "/profile/missions/claim", fields);
    }

    pub(crate) fn swap_mission(&mut self, mission_id: &str, ad_funded: bool) {
        let known = sow_data::missions::DAILY_MISSION_POOL
            .iter()
            .chain(sow_data::missions::WEEKLY_MISSION_POOL.iter())
            .any(|m| m.id == mission_id);
        if !known {
            return;
        }
        let mut fields = serde_json::Map::new();
        fields.insert(
            "mission_id".into(),
            serde_json::Value::String(mission_id.to_string()),
        );
        fields.insert("used_ad".into(), serde_json::Value::Bool(ad_funded));
        self.post_profile_action("swap mission", "/profile/missions/swap", fields);
    }

    pub(crate) fn unlock_skin(
        &mut self,
        skin_id: String,
        currency: String,
        offer_id: Option<String>,
    ) {
        let mut fields = serde_json::Map::new();
        fields.insert("skin_id".into(), serde_json::Value::String(skin_id));
        fields.insert("currency".into(), serde_json::Value::String(currency));
        if let Some(offer_id) = offer_id {
            fields.insert("offer_id".into(), serde_json::Value::String(offer_id));
        }
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
        let use_platform_identity = matches!(identity.provider, "sow" | "crazygames" | "playgames")
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

