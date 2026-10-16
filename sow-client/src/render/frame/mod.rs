use crate::app::SowApp;
use crate::render::gpu::MapGlobals;
use blade_graphics as gpu;

mod ui;

#[inline]
fn attack_threat_radius(troops: f32, troop_cap: f32, max_radius: f32) -> f32 {
    if troop_cap <= 0.0 || max_radius <= 0.0 {
        0.0
    } else {
        (troops / troop_cap).clamp(0.0, 1.0) * max_radius
    }
}

impl SowApp {
    pub fn render_frame(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        if let Some(win) = self.gfx.window.as_ref() {
            crate::viewport::sync_wasm_window(self, win.as_ref());
        }

        let wanted = self
            .gfx
            .window
            .as_ref()
            .map(|w| crate::viewport::Viewport::measure(w.as_ref()));

        if let Some(ref vp) = wanted
            && vp.wants_reconfigure(self)
        {
            self.apply_surface_resize(vp.physical, false);
        }

        let draw_world = self.should_draw_world();
        let spatial_audio = self.spatial_audio_ctx();

        if let Some(win) = self.gfx.window.as_ref() {
            win.pre_present_notify();
        }
        let Some(frame) = self.gfx.surface.as_mut().map(|s| s.acquire_frame()) else {
            return;
        };

        let configured = self.gfx.configured_physical;
        let window_ahead = wanted.as_ref().is_some_and(|vp| {
            vp.physical.width != configured.width || vp.physical.height != configured.height
        });
        if window_ahead {
            if let Some(vp) = wanted {
                self.apply_surface_resize(vp.physical, false);
            }
            if let Some(win) = self.gfx.window.as_ref() {
                win.request_redraw();
            }
            return;
        }

        let sf = wanted.as_ref().map(|v| v.scale_factor).unwrap_or(1.0);
        crate::viewport::Viewport::from_configured(self, sf).sync_to_app(self);

        if let Some(ref mut s) = self.gfx.surface {
            let mut render_ctx = match self.gfx.render_ctx.take() {
                Some(ctx) => ctx,
                None => return,
            };

            if let Some(sp) = self.gfx.prev_sync_point.take() {
                let _ = render_ctx.context.wait_for(&sp, !0);
            }

            render_ctx.command_encoder.start();
            render_ctx.command_encoder.init_texture(frame.texture());

            let mut map_drawn = false;
            if let Some(ref mut mr) = self.gfx.map_renderer {
                // Upload map state on first frame or after each tick (runs during splash for loader step 2).
                if self.gfx.needs_first_upload {
                    render_ctx.command_encoder.init_texture(mr.terrain_texture);
                    render_ctx.command_encoder.init_texture(mr.owner_texture);
                    render_ctx
                        .command_encoder
                        .init_texture(mr.occupancy_texture);
                    if let Some(ref tr) = self.gfx.text_renderer {
                        tr.init_textures(&mut render_ctx.command_encoder);
                        tr.upload_atlas(&mut render_ctx.command_encoder, &render_ctx.context);
                    }
                    self.gfx.needs_first_upload = false;
                    mr.upload_terrain(&mut render_ctx.command_encoder);
                }

                if draw_world {
                    // --- Layer 4: Track and Spawn Detonations ---
                    self.ui.detonation_scratch.clear();
                    if let Some(snap) = &self.sim.current_snapshot {
                        if self.ui.last_projectile_snapshot_tick != Some(snap.tick) {
                            for (id, prev_proj) in &self.ui.last_projectiles {
                                if !snap.projectiles.iter().any(|p| p.id == *id) {
                                    let at_end = prev_proj.path_cursor
                                        + (prev_proj.steps_per_tick as usize)
                                        >= prev_proj.path_len;
                                    if at_end {
                                        let dst_x = (prev_proj.dst_tile % self.sim.map_w) as f32;
                                        let dst_y = (prev_proj.dst_tile / self.sim.map_w) as f32;
                                        self.ui.detonation_scratch.push((
                                            dst_x,
                                            dst_y,
                                            prev_proj.kind,
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    // Spawns and tracks active fallout zones
                    let current_time = web_time::Instant::now();
                    for &(dx, dy, kind) in &self.ui.detonation_scratch {
                        if let sow_core::game::ProjectileKind::Nuke { level } = kind {
                            self.sfx.play_nuke_impact(
                                current_time,
                                level,
                                spatial_audio.params(dx + 0.5, dy + 0.5),
                            );

                            let blast_radius = sow_core::game::nuke_inner_radius(level) as f32;
                            self.ui.fallout_zones.push(crate::app::FalloutZone {
                                x: dx,
                                y: dy,
                                radius: blast_radius,
                                start_time: current_time,
                            });
                        }
                    }

                    // Detect new nuke launches and set client-side silo cooldowns
                    if let Some(snap) = &self.sim.current_snapshot {
                        let current_tick = snap.tick;
                        const SILO_COOLDOWN_TICKS: u64 = 90;

                        for proj in &snap.projectiles {
                            if matches!(proj.kind, sow_core::game::ProjectileKind::Nuke { .. })
                                && !self.ui.last_projectiles.contains_key(&proj.id)
                            {
                                let src_x = (proj.src_tile % self.sim.map_w) as f32 + 0.5;
                                let src_y = (proj.src_tile / self.sim.map_w) as f32 + 0.5;
                                self.sfx.play_nuke_launch(
                                    current_time,
                                    spatial_audio.params(src_x, src_y),
                                );

                                // New nuke — find source building by src_tile
                                if let Some(b) = snap.buildings.iter().find(|b| {
                                    b.kind == sow_core::game::BuildingKind::City
                                        && b.tile_idx == proj.src_tile
                                }) {
                                    self.ui
                                        .silo_cooldowns
                                        .insert(b.id, current_tick + SILO_COOLDOWN_TICKS);
                                }
                            }
                        }

                        // Prune expired cooldowns
                        self.ui
                            .silo_cooldowns
                            .retain(|_, expires| *expires > current_tick);
                    }

                    // Sync last_projectiles once per authoritative snapshot tick.
                    if let Some(snap) = &self.sim.current_snapshot {
                        if self.ui.last_projectile_snapshot_tick != Some(snap.tick) {
                            self.ui.last_projectiles.clear();
                            for proj in &snap.projectiles {
                                self.ui.last_projectiles.insert(
                                    proj.id,
                                    crate::app::TrackedProjectile::from_snapshot(proj),
                                );
                            }
                            self.ui.last_projectile_snapshot_tick = Some(snap.tick);
                        }
                    }

                    let dev = crate::theme::dev_config::DevConfig::get();
                    let border_thickness = dev.thickness;
                    let border_darkness = dev.darkness;
                    let shore_thickness = dev.shore_thickness;
                    let shore_darkness = dev.shore_darkness;
                    let territory_opacity = dev.territory_opacity;
                    let blend_mode = dev.blend_mode;
                    let conquest_duration = dev.conquest_duration;
                    let threat_troop_cap = dev.value("threat.troop_cap")[0];
                    let threat_max_radius = dev.value("threat.max_radius")[0];
                    let current_time = web_time::Instant::now();
                    let preview_duration = self.ui.dev_vfx_preview.duration(&dev);
                    let preview_phase = self
                        .ui
                        .dev_vfx_preview
                        .advance(current_time, preview_duration);
                    let preview_effect = if self.ui.dev_vfx_preview.active {
                        self.ui.dev_vfx_preview.target
                    } else {
                        0
                    };
                    let preview_time = preview_phase * preview_duration.max(0.0);
                    let preview_player_id = self.sim.my_player_id.unwrap_or(0);
                    let preview_player = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                        snapshot
                            .players
                            .iter()
                            .find(|player| {
                                player.id == preview_player_id
                                    && player.alive
                                    && player.tile_count > 0
                            })
                            .map(|player| (player.id, player.centroid_x, player.centroid_y))
                    });
                    let preview_partner = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                        snapshot
                            .players
                            .iter()
                            .find(|player| {
                                player.id != preview_player_id
                                    && player.alive
                                    && player.tile_count > 0
                            })
                            .map(|player| (player.id, player.centroid_x, player.centroid_y))
                    });
                    let preview_center = preview_player
                        .map(|(_, x, y)| [x, y])
                        .unwrap_or([self.sim.map_w as f32 * 0.5, self.sim.map_h as f32 * 0.5]);

                    let dirty = self
                        .sim
                        .current_snapshot
                        .as_ref()
                        .map(|s| s.dirty_tiles.as_slice())
                        .unwrap_or(&[]);
                    thread_local! {
                        static LAST_FOG_OF_WAR_TOGGLE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
                    }
                    let mut force_fog_upload = self.sim.force_fog_upload;
                    self.sim.force_fog_upload = false;
                    let toggle_changed = LAST_FOG_OF_WAR_TOGGLE.with(|cell| {
                        let prev = cell.get();
                        if prev != Some(dev.fog_of_war) {
                            cell.set(Some(dev.fog_of_war));
                            true
                        } else {
                            false
                        }
                    });
                    if toggle_changed {
                        force_fog_upload = true;
                    }

                    mr.update(
                        &mut render_ctx.command_encoder,
                        &render_ctx.context,
                        dirty,
                        conquest_duration,
                        &self.sim.fog_visible,
                        force_fog_upload,
                    );
                    if let Some(snap) = &mut self.sim.current_snapshot {
                        snap.dirty_tiles.clear();
                    }

                    let mut player_colors = [[0.5, 0.5, 0.5, 1.0]; 256];
                    let mut player_skin_styles = [[0.0f32; 4]; 256];
                    if let Some(snap) = &self.sim.current_snapshot {
                        for p in &snap.players {
                            if (p.id as usize) < 256 {
                                // Team players render with the planar team color;
                                // everyone else keeps their personal color.
                                let rgb =
                                    p.team.map_or(p.color, sow_core::player::team_territory_rgb);
                                player_colors[p.id as usize] = [rgb[0], rgb[1], rgb[2], 1.0];
                                player_skin_styles[p.id as usize][0] = p.skin_style as f32;
                            }
                        }
                    }

                    let mut threat_slots = [[0.0f32; 4]; 9];
                    if let Some(snap) = &self.sim.current_snapshot {
                        let my_id = self.sim.my_player_id.unwrap_or(0);
                        let mut slot = 0usize;
                        for attack in &snap.attacks {
                            if slot >= 8 {
                                break;
                            }
                            let is_mine = my_id != 0 && attack.owner_id == my_id;
                            let targets_me = my_id != 0 && attack.target_owner == my_id;
                            if !(is_mine || targets_me) || attack.troops <= 0.0 {
                                continue;
                            }
                            if attack.front_cx == 0.0 && attack.front_cy == 0.0 {
                                continue;
                            }
                            let radius = attack_threat_radius(
                                attack.troops as f32,
                                threat_troop_cap,
                                threat_max_radius,
                            );
                            threat_slots[slot] = [
                                attack.front_cx,
                                attack.front_cy,
                                radius,
                                attack.target_owner as f32 * 1024.0 + attack.owner_id as f32,
                            ];
                            slot += 1;
                        }
                    }
                    if preview_player_id > 0
                        && matches!(
                            preview_effect,
                            crate::app::DevVfxPreview::EXPANSION
                                | crate::app::DevVfxPreview::COMBAT_HAZE
                        )
                    {
                        let target_id = if preview_effect == crate::app::DevVfxPreview::EXPANSION {
                            0
                        } else {
                            preview_player_id
                        };
                        let attacker_id = if preview_effect == crate::app::DevVfxPreview::EXPANSION
                        {
                            preview_player_id
                        } else {
                            preview_partner
                                .map(|(id, _, _)| id)
                                .unwrap_or(preview_player_id)
                        };
                        threat_slots[8] = [
                            preview_center[0],
                            preview_center[1],
                            attack_threat_radius(100.0, threat_troop_cap, threat_max_radius),
                            target_id as f32 * 1024.0 + attacker_id as f32,
                        ];
                    }

                    let placement_active = if self.ui.app.hud_state.building_placement.is_some() {
                        1.0
                    } else {
                        0.0
                    };
                    if let Some(snapshot) = &self.sim.current_snapshot {
                        mr.update_building_occupancy(
                            &mut render_ctx.command_encoder,
                            &render_ctx.context,
                            &snapshot.buildings,
                        );
                    }

                    let mut fallout_slots = [[0.0f32; 4]; 9];
                    {
                        let mut slot = 0usize;
                        let duration = dev.value("nuke.zone.duration")[0];
                        self.ui.fallout_zones.retain(|fz| {
                            let elapsed = current_time.duration_since(fz.start_time).as_secs_f32();
                            if duration <= 0.0 || elapsed >= duration {
                                return false;
                            }
                            if slot < 8 {
                                let alpha_p = (1.0 - elapsed / duration).max(0.0);
                                fallout_slots[slot] = [fz.x, fz.y, fz.radius, alpha_p];
                                slot += 1;
                            }
                            true
                        });
                    }
                    if preview_effect == crate::app::DevVfxPreview::NUCLEAR {
                        fallout_slots[8] = [
                            preview_center[0],
                            preview_center[1],
                            sow_core::game::nuke_inner_radius(1) as f32,
                            1.0 - preview_phase,
                        ];
                    }

                    let mut alliance_celebration_slots = [[0.0f32; 4]; 18];
                    let snapshot = self.sim.current_snapshot.as_ref();
                    let mut slot = 0usize;
                    let alliance_duration = dev.value("alliance.duration")[0];
                    self.ui.alliance_celebrations.retain(|effect| {
                        let Some(age) = crate::app::alliance_celebration_age(
                            effect.start_time,
                            current_time,
                            alliance_duration,
                        ) else {
                            return false;
                        };
                        let Some(snapshot) = snapshot else {
                            return true;
                        };
                        if slot < crate::app::MAX_ALLIANCE_CELEBRATIONS {
                            for (participant, player_id) in effect.player_ids.iter().enumerate() {
                                if let Some(player) =
                                    snapshot.players.iter().find(|p| p.id == *player_id)
                                {
                                    alliance_celebration_slots[slot * 2 + participant] = [
                                        *player_id as f32,
                                        player.centroid_x,
                                        player.centroid_y,
                                        age,
                                    ];
                                }
                            }
                        }
                        slot += 1;
                        true
                    });
                    if preview_effect == crate::app::DevVfxPreview::ALLIANCE
                        && let Some((first_id, first_x, first_y)) = preview_player
                    {
                        alliance_celebration_slots[16] =
                            [first_id as f32, first_x, first_y, preview_time];
                        if let Some((second_id, second_x, second_y)) = preview_partner {
                            alliance_celebration_slots[17] =
                                [second_id as f32, second_x, second_y, preview_time];
                        }
                    }

                    // ── Dialog Terms Border Highlight ──
                    let mut dialog_border = match (&self.ui.dialog_border_highlight, snapshot) {
                        (Some(effect), Some(snapshot))
                            if snapshot
                                .players
                                .iter()
                                .any(|player| player.id == effect.player_id) =>
                        {
                            [
                                effect.player_id as f32,
                                current_time.duration_since(effect.start_time).as_secs_f32(),
                                0.0,
                                0.0,
                            ]
                        }
                        _ => [0.0f32; 4],
                    };
                    if preview_effect == crate::app::DevVfxPreview::DIALOG && preview_player_id > 0
                    {
                        dialog_border = [preview_player_id as f32, preview_time, 0.0, 0.0];
                    }

                    // ── Attack Border Flash ──
                    let (attack_flash_target, attack_flash_t) = {
                        self.ui.border_flash_intensities.clear();
                        let intensities = &mut self.ui.border_flash_intensities;
                        let duration = dev.value("attack.flash.duration")[0];
                        let base = dev.value("attack.flash.base")[0];
                        let per_attacker = dev.value("attack.flash.per_attacker")[0];
                        let max_intensity = dev.value("attack.flash.max")[0].max(0.0);
                        self.ui.border_flashes.retain(|flash| {
                            let elapsed =
                                current_time.duration_since(flash.start_time).as_secs_f32();
                            if let Some(t) =
                                crate::app::easeout_flash_with_duration(elapsed, duration)
                            {
                                let intensity = (base
                                    + flash.attacker_count.saturating_sub(1) as f32 * per_attacker)
                                    .max(0.0)
                                    .min(max_intensity);
                                let entry = intensities.entry(flash.player_id).or_insert(0.0);
                                *entry += t * intensity;
                                true
                            } else {
                                false
                            }
                        });

                        intensities
                            .iter()
                            .max_by(|a, b| {
                                a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(&id, &t)| (id as f32, t.min(max_intensity)))
                            .unwrap_or((0.0, 0.0))
                    };
                    let (attack_flash_target, attack_flash_t) = if preview_effect
                        == crate::app::DevVfxPreview::ATTACK_FLASH
                        && preview_player_id > 0
                    {
                        (preview_player_id as f32, (1.0 - preview_phase).powi(2))
                    } else {
                        (attack_flash_target, attack_flash_t)
                    };

                    // ── Viewport Alert Vignette ──
                    let currently_under_attack = if let Some(snap) = &self.sim.current_snapshot {
                        let my_id = self.sim.my_player_id.unwrap_or(0);
                        my_id != 0
                            && snap
                                .attacks
                                .iter()
                                .any(|a| a.target_owner == my_id && a.troops > 0.0)
                    } else {
                        false
                    };

                    if currently_under_attack {
                        self.ui
                            .trigger_viewport_alert(crate::app::ViewportAlertKind::UnderAttack);
                    } else if let Some(ref current) = self.ui.viewport_alert
                        && current.kind == crate::app::ViewportAlertKind::UnderAttack
                    {
                        self.ui.viewport_alert = None;
                    }

                    let (mut alert_color, mut alert_intensity) = if let Some(ref alert) =
                        self.ui.viewport_alert
                    {
                        let kind = alert.kind;
                        let elapsed = current_time.duration_since(alert.start_time).as_secs_f32();
                        let duration = dev.value("alert.duration")[0];
                        let persistent = kind == crate::app::ViewportAlertKind::UnderAttack
                            || kind == crate::app::ViewportAlertKind::Victory
                            || kind == crate::app::ViewportAlertKind::Defeat;

                        let intensity = if persistent {
                            if duration <= 0.0 {
                                1.0
                            } else {
                                (elapsed / duration).min(1.0)
                            }
                        } else {
                            crate::app::easeout_flash_with_duration(elapsed, duration)
                                .unwrap_or(0.0)
                        };

                        let color_id = match kind {
                            crate::app::ViewportAlertKind::UnderAttack => {
                                "alert.under_attack.color"
                            }
                            crate::app::ViewportAlertKind::Victory => "alert.victory.color",
                            crate::app::ViewportAlertKind::Defeat => "alert.defeat.color",
                            crate::app::ViewportAlertKind::ConquerPlayer => "alert.conquer.color",
                            crate::app::ViewportAlertKind::AllianceRequest => {
                                "alert.alliance.color"
                            }
                            crate::app::ViewportAlertKind::Betrayal => "alert.betrayal.color",
                        };
                        let mut color = dev.value(color_id);
                        let tint = dev.value("alert.color");
                        for channel in 0..4 {
                            color[channel] *= tint[channel];
                        }
                        (color, intensity)
                    } else {
                        ([0.0f32; 4], 0.0f32)
                    };
                    if preview_effect == crate::app::DevVfxPreview::ALERT {
                        alert_color = dev.value("alert.under_attack.color");
                        let tint = dev.value("alert.color");
                        for channel in 0..4 {
                            alert_color[channel] *= tint[channel];
                        }
                        alert_intensity = 1.0 - preview_phase;
                    }
                    // Clear expired one-shot alerts
                    if preview_effect != crate::app::DevVfxPreview::ALERT && alert_intensity <= 0.0
                    {
                        self.ui.viewport_alert = None;
                    }

                    let globals = MapGlobals {
                        camera_pos: [self.input.camera_x, self.input.camera_y],
                        zoom: self.input.camera_zoom,
                        time: self.time.start_time.elapsed().as_secs_f32() % 1000.0,
                        screen_size: [self.input.screen_w, self.input.screen_h],
                        map_size: [self.sim.map_w as f32, self.sim.map_h as f32],
                        border_thickness,
                        border_darkness,
                        shore_thickness,
                        shore_darkness,
                        threat_slots,
                        alliance_celebration_slots,
                        effect_shockwave: if dev.vfx_conquer { 1.0 } else { 0.0 },
                        effect_breathe: if dev.vfx_border_breathe { 1.0 } else { 0.0 },
                        effect_energy_flow: if dev.vfx_energy_flow { 1.0 } else { 0.0 },
                        my_player_id: self.sim.my_player_id.unwrap_or(0) as f32,
                        placement_active,
                        _placement_pad: [0.0; 2],
                        territory_opacity,
                        fallout_slots,
                        blend_mode,
                        effect_heartbeat: if dev.vfx_heartbeat { 1.0 } else { 0.0 },
                        effect_war_fog: if dev.vfx_war_fog { 1.0 } else { 0.0 },
                        effect_fallout: if dev.vfx_fallout { 1.0 } else { 0.0 },
                        effect_golden_hour: if dev.vfx_ambient_grade { 1.0 } else { 0.0 },
                        effect_holo_grid: if dev.vfx_holo_grid { 1.0 } else { 0.0 },
                        attack_flash_target,
                        attack_flash_t,
                        alert_intensity,
                        fog_of_war: if dev.fog_of_war { 1.0 } else { 0.0 },
                        tutorial_target_player: if self.ui.tutorial_active
                            && dev.vfx_tutorial_highlight
                        {
                            self.ui.tutorial_marker_player_id.unwrap_or(0) as f32
                        } else {
                            0.0
                        },
                        _pad2: 0.0,
                        alert_color,
                        dialog_border: if dev.vfx_dialog_border {
                            dialog_border
                        } else {
                            [0.0; 4]
                        },
                        effect_vignette: if dev.vfx_screen_vignette { 1.0 } else { 0.0 },
                        preview_effect: preview_effect as f32,
                        preview_phase,
                        preview_time,
                        shader_params: dev.shader_params,
                    };
                    let colors_struct = crate::render::gpu::PlayerColors {
                        colors: player_colors,
                    };
                    let skin_styles_struct = crate::render::gpu::PlayerSkinStyles {
                        styles: player_skin_styles,
                    };
                    mr.draw(
                        &mut render_ctx.command_encoder,
                        frame.texture_view(),
                        globals,
                        colors_struct,
                        skin_styles_struct,
                    );
                    map_drawn = true;

                    if self.gfx.text_renderer.is_none() {
                        let surface_format = s.info().format;
                        let tr = crate::render::gpu::TextRenderer::new(
                            &render_ctx.context,
                            surface_format,
                        );
                        tr.init_textures(&mut render_ctx.command_encoder);
                        tr.upload_atlas(&mut render_ctx.command_encoder, &render_ctx.context);
                        self.gfx.text_renderer = Some(tr);
                    }

                    // ── GPU-instanced movers (boats, nukes, SAM) ─────────────
                    if dev.vfx_movers || dev.vfx_mover_trails {
                        if self.gfx.mover_renderer.is_none() {
                            let surface_format = s.info().format;
                            self.gfx.mover_renderer = Some(crate::render::gpu::MoverRenderer::new(
                                &render_ctx.context,
                                surface_format,
                            ));
                            if let Some(ref mr_mover) = self.gfx.mover_renderer {
                                mr_mover.upload_atlas(
                                    &mut render_ctx.command_encoder,
                                    &render_ctx.context,
                                );
                            }
                        }
                        if let (Some(mover_r), Some(snap)) =
                            (&mut self.gfx.mover_renderer, &self.sim.current_snapshot)
                        {
                            let now = web_time::Instant::now();
                            let linear_alpha = self.time.interp.linear_alpha(now);
                            let pack = crate::render::world::movers::MoverPackParams {
                                camera_x: self.input.camera_x,
                                camera_y: self.input.camera_y,
                                camera_zoom: self.input.camera_zoom,
                                screen_w: self.input.screen_w,
                                screen_h: self.input.screen_h,
                                linear_alpha,
                                shader_params: &dev.shader_params,
                                sprites_enabled: dev.vfx_movers,
                                trails_enabled: dev.vfx_mover_trails,
                            };
                            self.ui.mover_scene.on_snapshot(
                                snap,
                                self.sim.map_w,
                                self.sim.map_h,
                                dev.fog_of_war,
                                self.sim.my_player_id.unwrap_or(0),
                                &self.sim.fog_visible,
                                &dev.shader_params,
                            );
                            self.ui.mover_scene.pack_gpu(&pack, mover_r);
                            let mover_globals = crate::render::gpu::MoverGlobals {
                                camera_pos: [self.input.camera_x, self.input.camera_y],
                                zoom: self.input.camera_zoom,
                                sprite_count: 0,
                                screen_size: [self.input.screen_w, self.input.screen_h],
                                trail_count: 0,
                                _pad: 0.0,
                                shader_params: dev.shader_params,
                            };
                            mover_r.draw(
                                &mut render_ctx.command_encoder,
                                frame.texture_view(),
                                mover_globals,
                                &render_ctx.context,
                            );
                        }
                    }
                }
            }

            if !map_drawn {
                let pass = render_ctx.command_encoder.render(
                    "clear_pass",
                    gpu::RenderTargetSet {
                        colors: &[gpu::RenderTarget {
                            view: frame.texture_view(),
                            init_op: gpu::InitOp::Clear(gpu::TextureColor::OpaqueBlack),
                            finish_op: gpu::FinishOp::Store,
                        }],
                        depth_stencil: None,
                    },
                );
                drop(pass);
            }

            // Restore the context so UI-action handlers (e.g. opening the map
            // editor) can take ownership of it during `run_ui` below. The
            // command encoder stays mid-frame on the same object across the
            // put/re-take, so UI painting continues recording into it.
            self.gfx.render_ctx = Some(render_ctx);

            self.render_frame_ui_and_present(sf, frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::attack_threat_radius;

    #[test]
    fn attack_threat_radius_reaches_full_radius_at_one_thousand_troops() {
        assert_eq!(attack_threat_radius(0.0, 1_000.0, 100.0), 0.0);
        assert_eq!(attack_threat_radius(100.0, 1_000.0, 100.0), 10.0);
        assert_eq!(attack_threat_radius(1_000.0, 1_000.0, 100.0), 100.0);
        assert_eq!(attack_threat_radius(10_000.0, 1_000.0, 100.0), 100.0);
        assert_eq!(attack_threat_radius(50_000.0, 1_000.0, 100.0), 100.0);
        assert_eq!(attack_threat_radius(100_000.0, 1_000.0, 100.0), 100.0);
        assert_eq!(attack_threat_radius(150_000.0, 1_000.0, 100.0), 100.0);
        assert_eq!(attack_threat_radius(100.0, 0.0, 100.0), 0.0);
        assert_eq!(attack_threat_radius(100.0, 1000.0, 0.0), 0.0);
    }
}
