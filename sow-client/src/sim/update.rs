use crate::app::SowApp;
use web_time::Instant;

impl SowApp {
    pub fn update_sim(&mut self, now: Instant) {
        self.time.turn_queue_peak = self.time.turn_queue_peak.max(self.sim.turn_queue.len());
        if let Some(snap) = &self.sim.current_snapshot {
            if let Some(target_secs) = snap.spawn_timer_secs {
                if let Some(ref mut current) = self.ui.app.hud_state.spawn_timer_secs {
                    if (*current - target_secs).abs() >= 0.1 {
                        *current = target_secs;
                    }
                } else {
                    self.ui.app.hud_state.spawn_timer_secs = Some(target_secs);
                }
            } else {
                self.ui.app.hud_state.spawn_timer_secs = None;
            }
        } else {
            self.ui.app.hud_state.spawn_timer_secs = None;
        }
        let is_playing_or_loading = self.ui.app.phase == crate::ClientPhase::Playing
            || (self.ui.app.phase == crate::ClientPhase::Splash
                && self.ui.app.splash_state.job == crate::ui::loading_screen::SplashJob::EnterGame);

        if is_playing_or_loading {
            if self.net.client.is_some() {
                // Multiplayer: lockstep execution dictated by server
                let mut ticks_processed = 0;
                while let Some(turn) = self.sim.turn_queue.pop_front() {
                    ticks_processed += 1;
                    self.dispatch_sim_command(sow_core::protocol::SimCommand::Turn(turn));

                    // NOTE(2026-08-22): sim-tick perf instrumentation added to
                    // diagnose the F-Stack relay lag report (sow-dist/prod.rs
                    // era). Forensics exonerated BOTH the relay and the sim —
                    // p50=9ms @ 558 players, 3 anomalous ticks in ~16k — the
                    // only real cost was Chrome stack-capturing these very
                    // console.warn lines when DevTools was open. Disabled so
                    // the console stays free. Re-enable via `crate::diag` if a
                    // regression ever needs measuring:
                    //   let tick_start = Instant::now();
                    //   ... dispatch ...
                    //   crate::diag::record_tick((Instant::now() - tick_start).as_micros() as u64);

                    // Update UI HUD State from my player id
                    self.sync_hud_player_state();
                    self.process_resource_notifications();
                    self.sync_building_costs();

                    if ticks_processed >= 10 {
                        break;
                    }
                }
                if ticks_processed > 0 {
                    // NOTE(2026-08-22): [SIM PERF]/[DIAG SIM TICK] reporting
                    // disabled — added to diagnose the F-Stack relay lag
                    // report; forensics exonerated relay and sim (p50=9ms @
                    // 558 players), and each console.warn line made DevTools
                    // stack-capture a 12-frame wasm trace, which was the only
                    // measurable lag. See `crate::diag` to re-enable.
                }
            } else {
                // Singleplayer: pace ticks from wall clock + lobby tick_rate_ms (same as relay).
                const MAX_OFFLINE_TICKS_PER_FRAME: u32 = 10;
                const MAX_OFFLINE_CATCHUP_SECS: f32 = 0.25;

                let tick_secs = (self.sim.config.tick_rate_ms / 1000.0).max(0.001);
                let dt = now
                    .duration_since(self.sim.offline_last_update)
                    .as_secs_f32()
                    .min(MAX_OFFLINE_CATCHUP_SECS);
                self.sim.offline_last_update = now;
                self.sim.offline_tick_timer += dt;

                // Pause: stop advancing the sim (nobody moves) but keep rendering.
                if self.sim.paused {
                    self.sim.offline_tick_timer = 0.0;
                }

                let mut ticks_this_frame = 0u32;
                while self.sim.offline_tick_timer >= tick_secs
                    && ticks_this_frame < MAX_OFFLINE_TICKS_PER_FRAME
                {
                    self.sim.offline_tick_timer -= tick_secs;
                    ticks_this_frame += 1;

                    let raw_intents = std::mem::take(&mut self.sim.offline_intents);
                    let mut stamped_intents = Vec::with_capacity(raw_intents.len());
                    for intent in raw_intents {
                        stamped_intents.push(sow_core::protocol::StampedIntent {
                            player_id: self.sim.my_player_id.unwrap_or(1),
                            intent,
                        });
                    }

                    let turn = sow_core::protocol::Turn {
                        turn_number: 0, // Ignored by client simulation
                        intents: stamped_intents,
                    };
                    self.dispatch_sim_command(sow_core::protocol::SimCommand::Turn(turn));
                }

                self.sync_hud_player_state();
                self.process_resource_notifications();
                self.sync_building_costs();
            }
        }
        if self.ui.app.phase == crate::ClientPhase::Playing
            && !self.input.has_snapped_camera_to_spawn
            && let Some(pid) = self.sim.my_player_id
            && let Some(snap) = &self.sim.current_snapshot
            && let Some(player) = snap.players.iter().find(|p| p.id == pid)
        {
            let is_playing = matches!(snap.phase, sow_core::game::GamePhase::Playing);
            if player.tile_count > 0 && player.alive && is_playing {
                // If user is panning/zooming during the animation, abort the animation
                if self.input.dragging
                    || self.input.last_pinch_state.is_some()
                    || !self.input.active_touches.is_empty()
                {
                    self.input.has_snapped_camera_to_spawn = true;
                } else {
                    let cx = player.centroid_x;
                    let cy = player.centroid_y;
                    let target_world_cx = cx + 0.5;
                    let target_world_cy = cy + 0.5;
                    let target_zoom = 20.0;

                    let current_world_cx =
                        (self.input.screen_w * 0.5 - self.input.camera_x) / self.input.camera_zoom;
                    let current_world_cy =
                        (self.input.screen_h * 0.5 - self.input.camera_y) / self.input.camera_zoom;

                    let speed = 0.01;
                    let next_world_cx =
                        current_world_cx + (target_world_cx - current_world_cx) * speed;
                    let next_world_cy =
                        current_world_cy + (target_world_cy - current_world_cy) * speed;
                    let next_zoom =
                        self.input.camera_zoom + (target_zoom - self.input.camera_zoom) * speed;

                    self.input.camera_zoom = next_zoom;
                    self.input.target_zoom = next_zoom;
                    self.input.camera_x = self.input.screen_w * 0.5 - next_world_cx * next_zoom;
                    self.input.camera_y = self.input.screen_h * 0.5 - next_world_cy * next_zoom;
                    self.clamp_camera_to_map();

                    if (target_zoom - next_zoom).abs() < 0.2
                        && (target_world_cx - next_world_cx).abs() < 0.1
                        && (target_world_cy - next_world_cy).abs() < 0.1
                    {
                        self.input.camera_zoom = target_zoom;
                        self.input.target_zoom = target_zoom;
                        self.input.camera_x =
                            self.input.screen_w * 0.5 - target_world_cx * target_zoom;
                        self.input.camera_y =
                            self.input.screen_h * 0.5 - target_world_cy * target_zoom;
                        self.clamp_camera_to_map();
                        self.input.has_snapped_camera_to_spawn = true;
                        log::info!(
                            "Game started! Camera smoothly arrived at player spawn at ({}, {}), zoom={}",
                            target_world_cx,
                            target_world_cy,
                            self.input.camera_zoom
                        );
                    }
                }
            }
        }

        // Periodic memory profiler print
        let now = web_time::Instant::now();
        if self
            .time
            .last_debug_print
            .is_none_or(|t| now.duration_since(t).as_secs() >= 5)
        {
            self.time.last_debug_print = Some(now);
            if let Some(snap) = &self.sim.current_snapshot
                && !snap.debug_mem_info.is_empty()
            {
                log::info!(
                    "[MEM_PROFILER] Turn Queue: {} (peak {}) | Dirty Tiles: {} | {}",
                    self.sim.turn_queue.len(),
                    self.time.turn_queue_peak,
                    snap.dirty_tiles.len(),
                    snap.debug_mem_info
                );
                self.time.turn_queue_peak = self.sim.turn_queue.len();
            }
        }
    }

    fn sync_building_costs(&mut self) {
        let snap_tick = self.sim.current_snapshot.as_ref().map(|s| s.tick);
        if snap_tick.is_some() && snap_tick == self.sim.last_synced_cost_tick {
            return;
        }
        self.sim.last_synced_cost_tick = snap_tick;

        let my_player_id = self.sim.my_player_id.unwrap_or(1);
        let buildings = self.sim.current_snapshot.as_ref().map(|s| &s.buildings);

        for i in 0..self.ui.app.hud_state.building_costs.len() {
            if let Some(&kind) = sow_core::game::BuildingKind::ALL.get(i) {
                let count = if let Some(b_list) = buildings {
                    b_list
                        .iter()
                        .filter(|b| b.owner_id == my_player_id && b.kind == kind)
                        .map(|b| b.level as u32)
                        .sum()
                } else {
                    0
                };
                self.ui.app.hud_state.building_costs[i] =
                    sow_core::building::structure_build_cost_gold(kind, count, &self.sim.config);
            } else {
                self.ui.app.hud_state.building_costs[i] = self.sim.config.cost_city;
            }
        }
    }

    fn process_resource_notifications(&mut self) {
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return;
        };
        if self.ui.last_resource_notice_tick == Some(snapshot.tick) {
            return;
        }
        self.ui.last_resource_notice_tick = Some(snapshot.tick);

        let my_id = self.sim.my_player_id.unwrap_or(1);
        let mut notifications = Vec::new();
        let (mut notice_x, mut notice_y) = (0.5, 0.5);
        if let Some(player) = snapshot.players.iter().find(|player| player.id == my_id)
            && (player.centroid_x > 0.001 || player.centroid_y > 0.001)
        {
            notice_x = player.centroid_x + 0.5;
            notice_y = player.centroid_y + 0.5;
        }
        let notice_time = Instant::now();

        for transfer in &snapshot.resource_transfers {
            let (prefix, other_id, color) = if transfer.receiver_id == my_id {
                ("received", transfer.sender_id, crate::rgb(74, 222, 128))
            } else if transfer.sender_id == my_id {
                ("sent", transfer.receiver_id, crate::rgb(220, 220, 220))
            } else {
                continue;
            };
            let name = snapshot
                .players
                .iter()
                .find(|player| player.id == other_id)
                .map(|player| player.name.as_str())
                .unwrap_or("Ally");
            let text = match (prefix, transfer.gold > 0.0, transfer.troops > 0.0) {
                ("received", true, true) => crate::ui::UiText::new("hud.resource_received_both")
                    .with("gold", crate::utils::format_number(transfer.gold))
                    .with("troops", crate::utils::format_number(transfer.troops))
                    .with("name", name),
                ("received", true, false) => crate::ui::UiText::new("hud.resource_received_gold")
                    .with("gold", crate::utils::format_number(transfer.gold))
                    .with("name", name),
                ("received", false, true) => crate::ui::UiText::new("hud.resource_received_troops")
                    .with("troops", crate::utils::format_number(transfer.troops))
                    .with("name", name),
                ("sent", true, true) => crate::ui::UiText::new("hud.resource_sent_both")
                    .with("gold", crate::utils::format_number(transfer.gold))
                    .with("troops", crate::utils::format_number(transfer.troops))
                    .with("name", name),
                ("sent", true, false) => crate::ui::UiText::new("hud.resource_sent_gold")
                    .with("gold", crate::utils::format_number(transfer.gold))
                    .with("name", name),
                ("sent", false, true) => crate::ui::UiText::new("hud.resource_sent_troops")
                    .with("troops", crate::utils::format_number(transfer.troops))
                    .with("name", name),
                _ => continue,
            };
            notifications.push((text, color));

            if transfer.receiver_id == my_id && transfer.gold > 0.0 {
                self.ui.floating_notices.push(crate::app::FloatingNotice {
                    text: format!("+{} Gold", crate::utils::format_number(transfer.gold)),
                    world_x: notice_x,
                    world_y: notice_y,
                    start_time: notice_time,
                    duration: web_time::Duration::from_millis(3000),
                    color: crate::rgb(250, 204, 21),
                });
            }
            if transfer.receiver_id == my_id && transfer.troops > 0.0 {
                self.ui.floating_notices.push(crate::app::FloatingNotice {
                    text: format!("+{} Troops", crate::utils::format_number(transfer.troops)),
                    world_x: notice_x,
                    world_y: notice_y + 0.5,
                    start_time: notice_time,
                    duration: web_time::Duration::from_millis(3000),
                    color: crate::rgb(6, 182, 212),
                });
            }
        }

        for rejection in &snapshot.resource_rejections {
            if rejection.requester_id != my_id {
                continue;
            }
            let name = snapshot
                .players
                .iter()
                .find(|player| player.id == rejection.rejector_id)
                .map(|player| player.name.as_str())
                .unwrap_or("Ally");
            notifications.push((
                crate::ui::UiText::new("hud.resource_request_declined").with("name", name),
                crate::rgb(239, 68, 68),
            ));
        }

        for (text, color) in notifications {
            self.ui.app.hud_state.push_notification(text, color);
        }
    }

    pub(crate) fn sync_hud_player_state(&mut self) {
        if let Some(player) = self.sim.current_snapshot.as_ref().and_then(|s| {
            s.players
                .iter()
                .find(|p| p.id == self.sim.my_player_id.unwrap_or(1))
        }) {
            self.ui.app.hud_state.gold = player.gold;
            self.ui.app.hud_state.troops = player.troops;
            self.ui.app.hud_state.max_troops = player.max_troops;
            self.ui.observing = !player.alive && player.has_spawned;

            // Compute correct actual troop rate
            if let Some(e) = &self.sim.engine {
                let my_pid = self.sim.my_player_id.unwrap_or(1);
                let agg = e
                    .building_aggregates
                    .get(my_pid as usize)
                    .copied()
                    .unwrap_or_default();
                self.ui.app.hud_state.troop_rate =
                    sow_core::execution::income_rates::troop_income_per_second(
                        player.tile_count,
                        agg,
                        player.leader,
                        &self.sim.config,
                    ) * self.sim.config.global_speed_multiplier;
            }
        }
    }
}
