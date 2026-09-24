use super::state::SowApp;
use web_time::{Duration, Instant};

const REWARD_PROFILE_RETRY_INTERVAL: Duration = Duration::from_secs(30);

fn account_hint(account_id: Option<&str>) -> String {
    account_id
        .map(|id| id.chars().take(8).collect::<String>())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "none".to_string())
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

    fn track_reward_receipt_sync(&mut self, receipt_id: impl Into<String>) {
        self.pending_reward_receipt_ids.insert(receipt_id.into());
        if let Some(account_id) = self.progress_account_id.as_deref() {
            crate::anonymous_identity::save_pending_reward_receipt_ids(
                account_id,
                &self.pending_reward_receipt_ids,
            );
        }
    }

    pub(crate) fn begin_reward_profile_sync(&mut self, receipt_id: impl ToString) {
        self.track_reward_receipt_sync(receipt_id.to_string());
        self.reward_profile_retry_at = Some(Instant::now());
        self.fetch_cloud_progress();
    }

    pub(crate) fn poll_reward_profile_sync(&mut self, now: Instant) {
        if self.pending_reward_receipt_ids.is_empty() {
            return;
        }
        if self.ui.app.phase != crate::ClientPhase::MainMenu {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if !self.wasm_doc_was_visible {
            return;
        }
        if self.profile_request_in_flight
            || self.display_name_save_request_id.is_some()
            || self
                .reward_profile_retry_at
                .is_none_or(|retry_at| retry_at > now)
        {
            return;
        }
        self.reward_profile_retry_at = Some(now + REWARD_PROFILE_RETRY_INTERVAL);
        self.fetch_cloud_progress();
    }

    pub(crate) fn schedule_reward_profile_retry(&mut self) {
        if !self.pending_reward_receipt_ids.is_empty() {
            self.reward_profile_retry_at = Some(Instant::now() + REWARD_PROFILE_RETRY_INTERVAL);
        }
    }

    pub(crate) fn fetch_cloud_progress(&mut self) {
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
    }

    pub(crate) fn apply_cloud_profile(
        &mut self,
        cloud: crate::player_progress::PlayerProgress,
        account_id: String,
        display_name: String,
        provider: String,
    ) {
        let account_changed = self.progress_account_id.as_deref() != Some(account_id.as_str());
        let account_switch = account_changed && self.progress_account_id.is_some();
        let cloud_intro_completed = cloud.intro_completed.unwrap_or(false);
        let retry_tutorial = !account_changed
            && self.progress.intro_completed.unwrap_or(false)
            && !cloud_intro_completed;
        if account_changed {
            self.exit_reward_preview = None;
            self.progress.reward_receipts.clear();
            self.progress.unlocked_achievements.clear();
        }
        if account_switch {
            self.pending_display_name = None;
            crate::anonymous_identity::clear_pending_display_name();
            self.pending_reward_receipt_ids.clear();
            self.reward_profile_retry_at = None;
        }
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
        if account_changed {
            self.apply_progress_preferences();
        }
        if retry_tutorial {
            log::info!(
                "[tutorial] cloud profile is missing completion; retrying one-time reward sync"
            );
            self.persist_tutorial_completion();
        }
        for receipt_id in &ready_receipts {
            self.pending_reward_receipt_ids.remove(receipt_id);
        }
        for receipt in self.progress.reward_receipts.values() {
            if receipt.verification_status
                == Some(sow_data::profile::ReplayVerificationStatus::Pending)
            {
                self.pending_reward_receipt_ids
                    .insert(receipt.id.clone());
            }
        }
        crate::anonymous_identity::save_pending_reward_receipt_ids(
            &account_id,
            &self.pending_reward_receipt_ids,
        );
        if !ready_receipts.is_empty() {
            log::info!("[rewards] receipt is visible in the main-menu profile");
        }
        if self.pending_reward_receipt_ids.is_empty() {
            self.reward_profile_retry_at = None;
        } else {
            self.reward_profile_retry_at = Some(Instant::now() + REWARD_PROFILE_RETRY_INTERVAL);
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

    fn post_store_action(
        &mut self,
        operation: &str,
        path: &str,
        mut fields: serde_json::Map<String, serde_json::Value>,
    ) {
        if self.ui.app.main_menu_state.store_busy {
            return;
        }
        let Some(account_id) = self.progress_account_id.clone() else {
            self.ui.app.main_menu_state.error_message =
                Some(crate::ui::UiText::new("profile.loading_profile"));
            return;
        };
        fields.insert("account_id".into(), serde_json::Value::String(account_id));
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
        self.ui.app.main_menu_state.store_busy = true;
        self.ui.app.main_menu_state.error_message = None;
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
                            operation: operation.clone(),
                            status: Some(response.status),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!("[store] {operation} failed status={}", response.status);
                let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
                    operation: operation.clone(),
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::error!("[store] {operation} network failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
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
        self.post_store_action("leader unlock", "/store/leaders/unlock", fields);
    }

    pub(crate) fn unlock_skin(&mut self, skin_id: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("skin_id".into(), serde_json::Value::String(skin_id));
        self.post_store_action("skin unlock", "/store/skins/unlock", fields);
    }

    pub(crate) fn equip_skin(&mut self, skin_id: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("skin_id".into(), serde_json::Value::String(skin_id));
        self.post_store_action("skin equip", "/store/skins/equip", fields);
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

    pub(crate) fn load_profile_ratings(&mut self) {
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
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        let encoded_id =
            url::form_urlencoded::byte_serialize(account_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profiles/{}/seasons",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_id
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct RatingsResponse {
                    items: Vec<sow_data::profile::PublicRatingView>,
                }
                match serde_json::from_slice::<RatingsResponse>(&response.bytes) {
                    Ok(payload) => {
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileRatingsLoaded {
                            account_id,
                            items: payload.items,
                        });
                    }
                    Err(error) => {
                        log::error!("[profile] ratings response parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                            account_id: Some(account_id),
                            operation: "ranked records".into(),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] ratings request failed status={}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: Some(account_id),
                    operation: "ranked records".into(),
                });
            }
            Err(error) => {
                log::error!("[profile] ratings request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::ProfileOperationFailed {
                    account_id: Some(account_id),
                    operation: "ranked records".into(),
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
        self.track_reward_receipt_sync("tutorial");
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
