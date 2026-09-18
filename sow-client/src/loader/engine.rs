use super::helpers::*;
use crate::EngineInitEvent;
use crate::app::SowApp;
use sow_core::protocol::SimCommand;
use crate::ClientPhase;

#[cfg(target_arch = "wasm32")]
async fn yield_to_browser() {
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};

    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let resolve_for_callback = resolve.clone();
        let callback = Closure::once_into_js(move || {
            let _ = resolve_for_callback.call0(&JsValue::UNDEFINED);
        });
        if let Some(window) = web_sys::window() {
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 0);
        } else {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

impl SowApp {
    pub(crate) fn abort_engine_init(&mut self) {
        while self.tasks.engine_init_rx.try_recv().is_ok() {}
        self.tasks.engine_init_queued_msg = None;
        self.tasks.pending_engine_init_data = None;
    }

    fn release_client_game_gpu(&mut self) {
        if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
            if let Some(sp) = self.gfx.prev_sync_point.take() {
                let _ = render_ctx.context.wait_for(&sp, !0);
            }
            if let Some(mut mr) = self.gfx.map_renderer.take() {
                mr.destroy(render_ctx);
            }
            if let Some(mut mover) = self.gfx.mover_renderer.take() {
                mover.destroy(render_ctx);
            }
            if let Some(mut text) = self.gfx.text_renderer.take() {
                text.destroy(render_ctx);
            }
            render_ctx.reset_command_encoder();
        }
    }

    pub(crate) fn reset_game_session(&mut self) {
        self.abort_engine_init();
        self.sim.turn_queue.clear();
        self.time.turn_queue_peak = 0;
        self.sim.offline_intents.clear();
        self.sim.last_synced_cost_tick = None;
        self.sim.my_lobby_id = None;
        self.sim.my_player_id = None;
        self.sim.relay_ticket = None;
        self.sim.relay_reconnect_ticket = None;
        self.ui.app.hud_state.spawn_timer_secs = None;
        self.ui.app.hud_state.sync_state = None;
        self.ui.app.hud_state.clear_notifications();
        self.ui.app.main_menu_state.wait_timer_secs = 0.0;
        self.ui.app.main_menu_state.cached_map = None;
        self.ui.app.main_menu_state.cached_map_key = None;
        self.net.pending_lobby_rejoin = false;
        self.ui.last_projectiles.clear();
        self.ui.last_projectile_snapshot_tick = None;
        self.ui.detonation_scratch.clear();
        self.ui.border_flash_intensities.clear();
        self.ui.placement_scratch.clear();
        self.ui.click_markers.clear();
        self.ui.floating_notices.clear();
        self.ui.attack_badge_labels.clear();
        self.ui.attack_badge_style_key = None;
        self.ui.attack_badge_cache_tick = None;
        self.ui.attack_badge_active_ids.clear();
        self.ui.nameplate_order_tick = None;
        self.ui.nameplate_order_my_id = None;
        self.ui.nameplate_order.clear();
        self.ui.building_render_cache = Default::default();
        self.ui.leaderboard_top_three = [None; 3];
        self.ui.leaderboard_refresh_at = None;
        self.ui.leaderboard_publish_pending = false;
        self.ui.last_resource_notice_tick = None;
        self.ui.tutorial_active = false;

        self.dispatch_sim_command(SimCommand::Shutdown);
        self.gfx.needs_first_upload = true;
        self.release_client_game_gpu();
    }

    fn map_bytes_for_start(&mut self, map_name: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.ui.app.asset_loader.take_map(map_name) {
            return Some(bytes);
        }
        if self.ui.app.main_menu_state.downloading_map_name.as_deref() == Some(map_name) {
            return self.ui.app.main_menu_state.cached_map.take();
        }
        None
    }

    pub fn update_loader(&mut self) {
        if let Some(start_msg) = self.tasks.engine_init_queued_msg.take() {
            let map_name = start_msg.config.map_name.clone();
            let has_map = self.ui.app.asset_loader.has_map(&map_name)
                || self.ui.app.main_menu_state.cached_map.is_some();

            if !has_map {
                self.tasks.engine_init_queued_msg = Some(start_msg);

                let pct = self.ui.app.main_menu_state.map_download_progress;
                log::debug!("Map download progress: {pct}%");
                splash_show_loading_progress(&mut self.ui.app.splash_state, pct as f32 / 100.0);
            } else {
                let cached_map = self.map_bytes_for_start(&map_name);
                log::info!(
                    "Map '{}' ready, computing heavy init in background",
                    map_name
                );

                splash_show_loading_progress(&mut self.ui.app.splash_state, 0.05);

                let mut start_msg_clone = start_msg.clone();

                let tx = self.tasks.engine_init_tx.clone();

                let init_logic = move || {
                    let parsed_map = cached_map.as_ref().and_then(|bytes| {
                        sow_core::maps::load_map_from_payload(bytes)
                            .map_err(|e| {
                                log::error!("Failed to parse map.bin: {e}");
                                e
                            })
                            .ok()
                    });

                    if cached_map.is_none() {
                        log::error!("Cached map data not found! Terrain will be empty.");
                    }

                    let (w, h) = if let Some(ref m) = parsed_map {
                        (m.width, m.height)
                    } else {
                        (
                            start_msg_clone.config.map_width,
                            start_msg_clone.config.map_height,
                        )
                    };
                    start_msg_clone.config.map_width = w;
                    start_msg_clone.config.map_height = h;

                    let mut state = sow_core::game::GameState::new(
                        start_msg_clone.seed,
                        w,
                        h,
                        start_msg_clone.config.clone(),
                    );

                    if let Some(map_file) = parsed_map {
                        state.total_land_tiles = map_file.num_land_tiles;
                        state.map_spawns = map_file.spawns;
                        state.geo_bounds = map_file.geo_bounds;
                        if map_file.terrain.len() == state.map.terrain.len() {
                            let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    map_file.terrain.as_ptr(),
                                    dest_ptr,
                                    map_file.terrain.len(),
                                );
                            }
                        } else {
                            log::error!(
                                "Terrain length mismatch: expected {}, got {}",
                                state.map.terrain.len(),
                                map_file.terrain.len()
                            );
                        }
                    }

                    #[cfg(target_arch = "wasm32")]
                    {
                        let tx_complete = tx.clone();
                        let tx_progress = tx.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let mut builder =
                                sow_core::water_components::WaterComponentsBuilder::new(&state.map);
                            loop {
                                let (progress, result) = builder.step(16_384);
                                let _ = tx_progress.send(EngineInitEvent::Progress(progress));
                                if let Some(water) = result {
                                    let _ = tx_complete.send(EngineInitEvent::Complete(
                                        Box::new(state),
                                        water,
                                        Box::new(start_msg_clone),
                                    ));
                                    break;
                                }
                                yield_to_browser().await;
                            }
                        });
                    }

                };

                init_logic();

                self.sim.turn_queue.clear();

                self.sim.current_snapshot = None;
                self.gfx.needs_first_upload = true;
            }
        }
        // Poll engine init channel
        if self.ui.app.phase == crate::ClientPhase::Splash {
            match self.ui.app.splash_state.job {
                crate::ui::loading_screen::SplashJob::Boot => {
                    #[cfg(target_arch = "wasm32")]
                    {
                        // loader.js owns the boot presentation. Rust only publishes the phase
                        // and waits for identity/bootstrap work before exposing the menu.
                        splash_show_loading_progress(&mut self.ui.app.splash_state, 0.95);
                        if self.should_portal_auto_intro() {
                            if self.boot_db_settled {
                                self.finish_boot_route();
                            }
                        } else {
                            self.finish_boot_to_main_menu();
                        }
                    }

                }
                crate::ui::loading_screen::SplashJob::ExitGame => {
                    let step = self.ui.app.splash_state.gpu_load_step;
                    if step == 0 {
                        log::info!("Exit game splash: reconnecting to orchestrator");
                        self.ui.app.splash_state.progress = 0.0;
                        self.ui.app.splash_state.gpu_load_step = 1;
                        self.ui.app.splash_state.frames_drawn = 0;
                    } else if step == 1 {
                        let p = self.ui.app.splash_state.progress;
                        if p < 0.99 {
                            let inc = ((0.99 - p) * 0.03).clamp(0.001, 0.04);
                            splash_show_loading_progress(
                                &mut self.ui.app.splash_state,
                                (p + inc).min(0.99),
                            );
                        }

                        let hero_ready = true;
                        let lobby_snapshot_ready = self.web_exit_lobbies_ready;
                        if self.net.client.is_some() && hero_ready && lobby_snapshot_ready {
                            log::info!(
                                "Exit game splash: menu dependencies ready, transitioning to main menu"
                            );
                            self.ui.app.splash_state.progress = 1.0;
                            self.ui.app.splash_state.gpu_load_step = 2;
                            self.ui.app.splash_state.frames_drawn = 0;
                        }
                    } else if step == 2 && self.ui.app.splash_state.frames_drawn > 1 {
                        self.reset_game_session();
                        self.ui.app.splash_state.done = true;
                        if self.ui.app.splash_state.target_phase.is_none() {
                            self.ui.app.splash_state.target_phase = Some(ClientPhase::MainMenu);
                        }
                    }
                }
                crate::ui::loading_screen::SplashJob::EnterGame => {
                    while let Ok(event) = self.tasks.engine_init_rx.try_recv() {
                        match event {
                            EngineInitEvent::Progress(prog) => {
                                self.ui.app.splash_state.progress = 0.05 + (prog * 0.45);
                            }
                            EngineInitEvent::Complete(state, water, start_msg) => {
                                if self.ui.app.splash_state.job
                                    != crate::ui::loading_screen::SplashJob::EnterGame
                                {
                                    continue;
                                }
                                log::info!("Engine initialization complete; allocating GPU memory");
                                self.net.load_telemetry.mark_engine_init_complete();
                                splash_show_loading_progress(&mut self.ui.app.splash_state, 0.55);
                                self.ui.app.splash_state.frames_drawn = 0;
                                self.ui.app.splash_state.gpu_load_step = 1;
                                self.tasks.pending_engine_init_data =
                                    Some((*state, water, *start_msg));
                            }
                        }
                    }
                }
            }

            if self.ui.app.splash_state.job == crate::ui::loading_screen::SplashJob::EnterGame
                && self.tasks.pending_engine_init_data.is_some()
            {
                let step = self.ui.app.splash_state.gpu_load_step;
                if step == 1 && self.ui.app.splash_state.frames_drawn > 1 {
                    // Step 1: Allocate GPU Memory & Send Init Command
                    if let Some((state, water, start_msg)) =
                        self.tasks.pending_engine_init_data.take()
                    {
                        let map_bytes: Vec<u8> =
                            state.map.terrain.iter().map(|t| t.as_byte()).collect();

                        self.sim.current_snapshot = None; // MANDATORY: Clear old snapshot so Step 3 waits for the new one!

                        self.dispatch_sim_command(SimCommand::Init {
                            config: Box::new(start_msg.config.clone()),
                            seed: start_msg.seed,
                            map_bytes: map_bytes.clone(),
                            players: start_msg.players.clone(),
                            // map_bytes above is raw terrain; anchors/bounds
                            // from the parsed map file must ride explicitly.
                            map_spawns: state.map_spawns.clone(),
                            geo_bounds: state.geo_bounds,
                            num_land_tiles: state.total_land_tiles,
                        });

                        // POKA-YOKE — the single match-init chokepoint that EVERY match (offline,
                        // tutorial, multiplayer) funnels through. The tutorial UI is *derived* here
                        // from THIS match's config, never retained across matches: a normal skirmish
                        // and any server-sent multiplayer config have `tutorial == false`, so the
                        // tutorial can't leak onto them. This is the ONLY writer of `tutorial_active`
                        // after startup; the render gate additionally requires `is_offline`.
                        self.ui.tutorial_active = start_msg.config.tutorial;

                        for turn in &start_msg.missed_turns {
                            self.dispatch_sim_command(SimCommand::Turn(turn.clone()));
                        }

                        self.sim.map_w = start_msg.config.map_width;
                        self.sim.map_h = start_msg.config.map_height;
                        self.sim.fog_explored = sow_core::bitset::DenseBitSet::new();
                        self.sim.fog_visible = sow_core::bitset::DenseBitSet::new();
                        self.sim.force_fog_upload = true;
                        self.release_client_game_gpu();
                        if let Some(render_ctx) = self.gfx.render_ctx.as_mut()
                            && let Some(ref s) = self.gfx.surface
                        {
                            let format = s.info().format;
                            self.gfx.map_renderer =
                                Some(crate::render::gpu::map_renderer::MapRenderer::new(
                                    &render_ctx.context,
                                    self.sim.map_w,
                                    self.sim.map_h,
                                    format,
                                    &map_bytes,
                                ));
                            self.gfx.mover_renderer = Some(crate::render::gpu::MoverRenderer::new(
                                &render_ctx.context,
                                format,
                            ));
                            self.gfx.text_renderer = Some(crate::render::gpu::TextRenderer::new(
                                &render_ctx.context,
                                format,
                            ));
                            self.gfx.needs_first_upload = true;
                        }

                        // Move to step 2: Texture uploading happens automatically next frame
                        self.ui.app.splash_state.gpu_load_step = 2;
                        self.ui.app.splash_state.frames_drawn = 0;
                        log::info!("Enter game splash: uploading map texture");
                        splash_show_loading_progress(&mut self.ui.app.splash_state, 0.70);

                        // Re-insert pending data so we stay in this block until Step 4
                        self.tasks.pending_engine_init_data = Some((state, water, start_msg));
                    }
                } else if step == 2 && !self.gfx.needs_first_upload {
                    // Step 2 Finished: GPU Texture is uploaded!
                    self.ui.app.splash_state.gpu_load_step = 3;
                    splash_show_loading_progress(&mut self.ui.app.splash_state, 0.80);
                } else if step == 3 && self.sim.current_snapshot.is_some() {
                    self.net.load_telemetry.mark_snapshot_available();
                    let ready_to_release = if self.net.is_offline {
                        true
                    } else {
                        self.ui.app.main_menu_state.is_connected
                            && self.net.client.is_some()
                            && self.ws_on_relay()
                    };

                    if ready_to_release {
                        self.ui.app.splash_state.gpu_load_step = 4;
                        self.ui.app.splash_state.progress = 1.0;
                        self.ui.app.splash_state.done = true;
                        if self.ui.app.splash_state.target_phase.is_none() {
                            self.ui.app.splash_state.target_phase =
                                Some(crate::ClientPhase::Playing);
                            crate::store_portals::gameplay_start();
                            if !self.net.is_offline {
                                crate::analytics::track_with(
                                    "match_started_client",
                                    serde_json::json!({
                                        "tutorial": self.sim.config.tutorial,
                                    }),
                                );
                            }
                        }

                        // Clear pending init data to completely finish EnterGame phase
                        self.tasks.pending_engine_init_data = None;
                        log::info!("EnterGame load complete; fading out loader");

                        if !self.net.is_offline
                            && let Some(c) = self.net.client.as_ref()
                            && let (Some(lid), Some(pid)) =
                                (self.sim.my_lobby_id, self.sim.my_player_id)
                        {
                            if let Some(ready_msg) = self.make_initial_relay_ready_message(lid, pid)
                                && let Ok(json) = bincode::serialize(&ready_msg)
                            {
                                c.send(json);
                                self.net.load_telemetry.mark_ready_sent();
                            } else {
                                log::warn!(
                                    "EnterGame waiting for relay ticket before sending ready"
                                );
                            }
                        }
                    } else {
                        let p = self.ui.app.splash_state.progress;
                        if p < 0.95 {
                            let inc = ((0.95 - p) * 0.03).clamp(0.001, 0.02);
                            splash_show_loading_progress(&mut self.ui.app.splash_state, p + inc);
                        }
                        if self.ui.app.splash_state.frames_drawn.is_multiple_of(120) {
                            log::warn!(
                                "[LOADER] Waiting for relay connection before releasing loader: is_connected={}, has_client={}, on_relay={}, my_lobby_id={:?}, my_player_id={:?}, phase={:?}",
                                self.ui.app.main_menu_state.is_connected,
                                self.net.client.is_some(),
                                self.ws_on_relay(),
                                self.sim.my_lobby_id,
                                self.sim.my_player_id,
                                self.ui.app.phase
                            );
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            if let Some(target_phase) = self.ui.app.splash_state.target_phase.take() {
                self.ui.app.phase = target_phase;
            }
        }
    }
}
