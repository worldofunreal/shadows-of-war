use crate::app::SowApp;
use web_time::Instant;

impl SowApp {
    pub(crate) fn current_player_gold(&self) -> f64 {
        let player_id = self
            .sim
            .my_player_id
            .unwrap_or(self.ui.app.hud_state.my_player_id);
        self.sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .players
                    .iter()
                    .find(|player| player.id == player_id)
            })
            .map(|player| player.gold)
            .unwrap_or(self.ui.app.hud_state.gold)
    }

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
                if self.input.is_pointer_gesture_active() {
                    self.input.has_snapped_camera_to_spawn = true;
                } else {
                    let player_focus = (player.centroid_x + 0.5, player.centroid_y + 0.5);
                    let (target_world_cx, target_world_cy, target_zoom) =
                        if let Some(((x, y), zoom)) = crate::campaign::tutorial_camera_frame(
                            self.ui.tutorial_campaign,
                            &self.sim.config,
                            self.input.screen_w,
                            self.input.screen_h,
                        ) {
                            (x, y, zoom)
                        } else {
                            (
                                player_focus.0,
                                player_focus.1,
                                if self.input.tutorial_camera_focus {
                                    45.0
                                } else {
                                    20.0
                                },
                            )
                        };

                    let current_world_cx =
                        (self.input.screen_w * 0.5 - self.input.camera_x) / self.input.camera_zoom;
                    let current_world_cy =
                        (self.input.screen_h * 0.5 - self.input.camera_y) / self.input.camera_zoom;

                    let speed = if self.input.tutorial_camera_focus {
                        0.04
                    } else {
                        0.01
                    };
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
                            "Game started! Camera smoothly arrived at opening frame at ({}, {}), zoom={}",
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

    pub(in crate::sim) fn sync_building_costs(&mut self) {
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

        for transfer in &snapshot.resource_transfers {
            if transfer.sender_id == my_id {
                let other_id = transfer.receiver_id;
                let name = snapshot
                    .players
                    .iter()
                    .find(|player| player.id == other_id)
                    .map(|player| player.name.as_str())
                    .unwrap_or("Ally");
                let text = match (transfer.gold > 0.0, transfer.troops > 0.0) {
                    (true, true) => crate::ui::UiText::new("hud.resource_sent_both")
                        .with("gold", crate::utils::format_number(transfer.gold))
                        .with("troops", crate::utils::format_number(transfer.troops))
                        .with("name", name),
                    (true, false) => crate::ui::UiText::new("hud.resource_sent_gold")
                        .with("gold", crate::utils::format_number(transfer.gold))
                        .with("name", name),
                    (false, true) => crate::ui::UiText::new("hud.resource_sent_troops")
                        .with("troops", crate::utils::format_number(transfer.troops))
                        .with("name", name),
                    _ => continue,
                };
                notifications.push((
                    text,
                    [Some(my_id), Some(other_id)],
                    3,
                    format!("resource:sent:{other_id}"),
                ));
            }
            if transfer.receiver_id != my_id {
                continue;
            }
            let other_id = transfer.sender_id;
            let players = [Some(transfer.sender_id), Some(my_id)];
            let name = snapshot
                .players
                .iter()
                .find(|player| player.id == other_id)
                .map(|player| player.name.as_str())
                .unwrap_or("Ally");
            let text = match (transfer.gold > 0.0, transfer.troops > 0.0) {
                (true, true) => crate::ui::UiText::new("hud.resource_received_both")
                    .with("gold", crate::utils::format_number(transfer.gold))
                    .with("troops", crate::utils::format_number(transfer.troops))
                    .with("name", name),
                (true, false) => crate::ui::UiText::new("hud.resource_received_gold")
                    .with("gold", crate::utils::format_number(transfer.gold))
                    .with("name", name),
                (false, true) => crate::ui::UiText::new("hud.resource_received_troops")
                    .with("troops", crate::utils::format_number(transfer.troops))
                    .with("name", name),
                _ => continue,
            };
            notifications.push((text, players, 3, format!("resource:received:{other_id}")));
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
                [Some(rejection.rejector_id), Some(my_id)],
                2,
                format!("resource-rejected:{}", rejection.rejector_id),
            ));
        }

        for rejection in &snapshot.resource_transfer_rejections {
            if rejection.sender_id != my_id {
                continue;
            }
            let name = snapshot
                .players
                .iter()
                .find(|player| player.id == rejection.receiver_id)
                .map(|player| player.name.as_str())
                .unwrap_or("Ally");
            notifications.push((
                crate::ui::UiText::new("hud.resource_send_failed").with("name", name),
                [Some(my_id), Some(rejection.receiver_id)],
                2,
                format!("resource:send-failed:{}", rejection.receiver_id),
            ));
        }

        for (text, players, priority, group) in notifications {
            self.ui.app.hud_state.push_notification_for_players(
                text,
                players,
                priority,
                Some(group),
                true,
            );
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
                let trade_ships = e
                    .fleets
                    .iter()
                    .filter(|fleet| {
                        fleet.owner_id == my_pid
                            && fleet.unit_type == sow_core::game::UnitType::TradeShip
                    })
                    .count() as u32;
                self.ui.app.hud_state.gold_rate =
                    (sow_core::execution::income_rates::gold_income_per_second(
                        player.tile_count,
                        agg,
                        player.leader,
                        &self.sim.config,
                    ) + sow_core::execution::income_rates::trade_income_per_second(
                        trade_ships,
                        &self.sim.config,
                    )) * self.sim.config.global_speed_multiplier;
            }
        }
    }
}
