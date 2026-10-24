use crate::ClientPhase;
use crate::app::SowApp;
use crate::spawn_sow_client_connect;
use web_time::{Duration, Instant};

mod messages;

impl SowApp {
    pub fn update_net(&mut self, now: Instant) {
        self.poll_portal_intents();

        #[cfg(target_arch = "wasm32")]
        {
            let doc_visible = web_sys::window()
                .and_then(|w| w.document())
                .map(|d| d.visibility_state() == web_sys::VisibilityState::Visible)
                .unwrap_or(true);
            if doc_visible && !self.wasm_doc_was_visible {
                self.net.ws_reconnect_after_resume = true;
                if self.ui.app.phase == crate::ClientPhase::MainMenu
                    && !self.pending_reward_receipt_ids.is_empty()
                {
                    self.reward_profile_retry_at = Some(now);
                    self.fetch_cloud_progress();
                }
            }
            self.wasm_doc_was_visible = doc_visible;
        }

        if self.net.ws_reconnect_after_resume {
            self.net.ws_reconnect_after_resume = false;
            self.net.ws_connect_not_before = self.net.ws_connect_not_before.min(now);
        }

        if matches!(
            self.ui.app.phase,
            crate::ClientPhase::MainMenu | crate::ClientPhase::Splash
        ) {
            self.fetch_map_catalog_if_needed();
        }

        // No fake map download simulation! Progress is real!

        while let Ok((url, res)) = self.net.connect_rx.try_recv() {
            // A connection started before leaving may finish after the target
            // changed. Its result must never replace the menu connection.
            if url != self.ui.app.main_menu_state.server_address {
                continue;
            }
            match res {
                Ok(client) => {
                    log::info!(
                        "[CLIENT NET] Received successfully connected WebSocket client from channel!"
                    );
                    self.ui.app.main_menu_state.is_connected = true;
                    self.ui.app.main_menu_state.is_connecting = false;
                    self.net.ws_connect_fail_backoff_ms = 400;
                    if self.ws_on_relay() {
                        self.net.load_telemetry.mark_relay_connected();
                        if let (Some(lid), Some(pid)) =
                            (self.sim.my_lobby_id, self.sim.my_player_id)
                        {
                            let msg = if self.sim.relay_reconnect_ticket.is_some() {
                                log::info!("Sending reconnect ticket to relay");
                                self.make_reconnect_message(lid, pid)
                            } else {
                                log::info!("Sending initial relay ticket");
                                self.make_relay_ready_message(lid, pid)
                            };
                            if let Some(msg) = msg
                                && let Ok(data) = bincode::serialize(&msg)
                            {
                                client.send(data);
                                self.net.load_telemetry.mark_ready_sent();
                            } else {
                                log::info!("Relay connection waiting for its ticket");
                            }
                        }
                    } else if self.ui.app.phase == crate::ClientPhase::Playing {
                        log::info!("Orchestrator reconnected while the relay session is active");
                    } else if self.ui.app.main_menu_state.host_private_pending {
                        log::info!("Hosting private lobby from portal instant");
                        let join_msg = self.make_join_message(None, true, None, None);
                        if let Some(join_msg) = join_msg
                            && let Ok(json) = bincode::serialize(&join_msg)
                        {
                            client.send(json);
                            self.ui.app.main_menu_state.host_private_pending = false;
                            self.ui.app.main_menu_state.is_waiting = true;
                        } else {
                            self.join_waiting_for_identity = true;
                        }
                    } else if let Some(id) = self.ui.app.main_menu_state.pending_join_lobby_id {
                        if self.ui.app.main_menu_state.is_waiting
                            && self.ui.app.main_menu_state.joined_lobby_id.is_none()
                            && (self.progress_account_id.is_some() || self.net.is_offline)
                        {
                            log::info!("Joining lobby {} from portal invite", id);
                            let join_msg = self.make_join_message(Some(id), false, None, None);
                            if let Some(join_msg) = join_msg
                                && let Ok(json) = bincode::serialize(&join_msg)
                            {
                                client.send(json);
                                self.join_waiting_for_identity = false;
                            } else {
                                self.join_waiting_for_identity = true;
                            }
                        }
                    } else if self.join_waiting_for_identity
                        && self.ui.app.main_menu_state.is_waiting
                        && self.ui.app.main_menu_state.joined_lobby_id.is_none()
                        && (self.progress_account_id.is_some() || self.net.is_offline)
                    {
                        let join_msg = self.make_join_message(
                            None,
                            self.ui.app.main_menu_state.custom_game_is_private,
                            (!self.join_matchmaking)
                                .then(|| self.ui.app.main_menu_state.custom_game_config.clone()),
                            Some(self.ui.app.main_menu_state.custom_game_password.clone())
                                .filter(|password| !password.is_empty()),
                        );
                        if let Some(join_msg) = join_msg
                            && let Ok(json) = bincode::serialize(&join_msg)
                        {
                            client.send(json);
                            self.join_waiting_for_identity = false;
                        } else {
                            self.join_waiting_for_identity = true;
                        }
                    }
                    self.net.client = Some(client);
                }
                Err(e) => {
                    log::warn!("[CLIENT NET] Failed to connect: {}", e);
                    self.ui.app.main_menu_state.is_connected = false;
                    self.ui.app.main_menu_state.is_connecting = false;
                    if self.ws_on_relay() && !self.net.is_offline {
                        // Transport retries cannot decide that a player abandoned.
                        self.observe_disconnected_match();
                        self.net.ws_connect_fail_backoff_ms = 500;
                        self.net.ws_connect_not_before = now + Duration::from_millis(500);
                        crate::web_menu::wake_event_loop_after(500);
                    } else {
                        self.net.ws_connect_fail_backoff_ms =
                            (self.net.ws_connect_fail_backoff_ms.saturating_mul(2)).min(30_000);
                        self.net.ws_connect_not_before =
                            now + Duration::from_millis(self.net.ws_connect_fail_backoff_ms);
                        crate::web_menu::wake_event_loop_after(self.net.ws_connect_fail_backoff_ms);
                    }
                }
            }
        }

        let messages::ProcessWsResult {
            mut ws_disconnected,
            switch_to_relay,
            exit_to_menu_after_net,
            lobby_exit_to_menu,
            relay_session_resumed,
        } = self.process_ws_messages();

        if self.finished_relay_match() {
            ws_disconnected = false;
            self.net.client = None;
            self.ui.app.main_menu_state.is_connecting = false;
        }

        if relay_session_resumed {
            self.clear_disconnect_observation();
        }

        if lobby_exit_to_menu {
            self.finish_lobby_exit_to_main_menu();
        }

        if exit_to_menu_after_net {
            self.begin_relay_closed_return();
        }

        if let Some((relay_port, relay_host)) = switch_to_relay {
            self.net.relay_handoff_done = true;
            self.net.load_telemetry.reset_for_relay_handoff();
            log::info!(
                "[CLIENT NET] Handoff from Master Orchestrator -> Game Relay on port {} (host {:?})",
                relay_port,
                relay_host
            );

            // Connect directly to the relay host (F-Stack endpoint with TLS).
            // Browser clients use wss:// — the relay terminates TLS.
            if let Some(host) = relay_host {
                self.net.ws_url = format!("wss://{}:{}/ws/", host, relay_port);
            } else if let Ok(mut url) = url::Url::parse(&self.net.ws_url) {
                let _ = url.set_port(Some(relay_port));
                self.net.ws_url = url.to_string();
            }

            log::info!("[CLIENT NET] Handoff URL resolved to: {}", self.net.ws_url);
            self.net.client = None; // Drop orchestrator connection
            self.net.current_ping_ms = None;
            self.ui.app.main_menu_state.is_connected = false; // Reset connection status during handoff
            self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
            ws_disconnected = false;

            // Clear stale connections
            while self.net.connect_rx.try_recv().is_ok() {
                log::info!("Purged stale connection from channel during handoff to relay");
            }

            self.net.ws_connect_not_before = now; // Ensure no backoff delays are active for retries
        }

        if ws_disconnected {
            log::warn!(
                "[CLIENT NET] WS disconnect observed: phase={:?}, waiting={}, splash_job={:?}, has_engine_init_queued={}, has_pending_init_data={}, on_relay={}, ws_url={}",
                self.ui.app.phase,
                self.ui.app.main_menu_state.is_waiting,
                self.ui.app.splash_state.job,
                self.tasks.engine_init_queued_msg.is_some(),
                self.tasks.pending_engine_init_data.is_some(),
                self.ws_on_relay(),
                self.net.ws_url
            );
            self.net.client = None;
            self.net.current_ping_ms = None;
            self.ui.app.main_menu_state.is_connected = false;
            self.ui.app.main_menu_state.is_connecting = false;
            if self.ws_on_relay() {
                self.net.ws_connect_not_before = now;
            } else {
                self.net.ws_connect_not_before = now + Duration::from_millis(2000);
                crate::web_menu::wake_event_loop_after(2000);
            }

            if self.net.is_offline {
                log::debug!("[CLIENT NET] Offline match; ignoring disconnect recovery");
            } else if self.ui.app.phase == ClientPhase::Playing {
                self.observe_disconnected_match();
                log::info!(
                    "[CLIENT NET] Temporary game disconnect; awaiting relay decision while reconnecting"
                );
            } else if self.ui.app.phase != ClientPhase::Splash {
                log::info!("[CLIENT NET] Disconnected outside match; reconnecting to orchestrator");
                self.net.ws_url = self.net.orchestrator_url.clone();
                self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
            }
        }

        if self.ui.app.phase == crate::ClientPhase::Playing
            && now.duration_since(self.net.last_ping_time) >= Duration::from_secs(1)
        {
            if let Some(client) = self.net.client.as_ref() {
                let ping = sow_core::protocol::ClientMessage::Ping {
                    client_time: self.time.start_time.elapsed().as_secs_f64(),
                };
                if let Ok(data) = bincode::serialize(&ping) {
                    client.send(data);
                    self.net.last_ping_time = now;
                }
            }
        }

        let allow_ws_spawn = self.wasm_doc_was_visible;

        if allow_ws_spawn
            && !self.finished_relay_match()
            && self.net.client.is_none()
            && !self.ui.app.main_menu_state.is_connecting
            && now >= self.net.ws_connect_not_before
            && !self.net.is_offline
        {
            self.ui.app.main_menu_state.is_connecting = true;
            let url = self.ui.app.main_menu_state.server_address.clone();
            log::info!(
                "[CLIENT NET] Auto-reconnect triggered: Spawning WS connection task to {}",
                url
            );
            spawn_sow_client_connect(url, &self.net.connect_tx);
        }
    }
}
