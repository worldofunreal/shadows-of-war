mod combat;
mod elimination;

fn ability_is_visible_to_player(owner_id: u16, affected: &[u16], my_id: u16) -> bool {
    my_id != 0 && (owner_id == my_id || affected.contains(&my_id))
}

use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

fn suppress_submitted_notice_if_formed(
    status: sow_core::game::AllianceRequestStatus,
    was_allied: bool,
    is_allied: bool,
) -> bool {
    status == sow_core::game::AllianceRequestStatus::Submitted && !was_allied && is_allied
}

fn alliance_request_notification(
    proposer_id: u16,
    target_id: u16,
    status: sow_core::game::AllianceRequestStatus,
    my_id: u16,
    target_name: String,
) -> Option<(crate::ui::UiText, [Option<u16>; 2])> {
    if my_id == 0 || proposer_id != my_id {
        return None;
    }
    let text = match status {
        sow_core::game::AllianceRequestStatus::Submitted => {
            crate::ui::UiText::new("hud.alliance_request_sent")
        }
        sow_core::game::AllianceRequestStatus::Rejected => {
            crate::ui::UiText::new("hud.alliance_request_rejected")
        }
        sow_core::game::AllianceRequestStatus::Expired => {
            crate::ui::UiText::new("hud.alliance_request_expired")
        }
    };
    Some((
        text.with("name", target_name),
        [Some(proposer_id), Some(target_id)],
    ))
}

impl SowApp {
    pub(crate) fn process_tick_events(
        &mut self,
        events: Vec<sow_core::game::GameEvent>,
        snap: &SimSnapshot,
        my_id: u16,
    ) -> crate::player_progress::SessionDefeats {
        let mut turn_defeats = crate::player_progress::SessionDefeats::default();
        let mut played_combat_this_tick = false;
        let mut captured_enemy_tiles: u32 = 0;
        let mut last_enemy_capture_pos: Option<(f32, f32)> = None;
        let mut best_tutorial_capture: Option<(sow_core::game::BuildingKind, u8, u32)> = None;
        let now_instant = web_time::Instant::now();
        self.sfx.sync_server_phase(&snap.phase);
        self.sfx.begin_tick();
        for event in events {
            match event {
                sow_core::game::GameEvent::ResourceTransferred {
                    receiver_id,
                    troops,
                    ..
                } => {
                    self.ui
                        .nameplates
                        .animate_troop_change(receiver_id, troops, now_instant);
                }
                sow_core::game::GameEvent::TransportShipLanded { owner_id, x, y } => {
                    let color = snap
                        .players
                        .iter()
                        .find(|player| player.id == owner_id)
                        .map(|player| {
                            sow_core::player::player_territory_rgb(
                                player.player_type,
                                player.team,
                                player.color,
                                player.is_campaign_faction,
                            )
                        })
                        .unwrap_or([0.13, 0.83, 0.94]);
                    if self.ui.transport_impacts.len() == crate::app::MAX_TRANSPORT_IMPACTS {
                        self.ui.transport_impacts.pop_front();
                    }
                    self.ui
                        .transport_impacts
                        .push_back(crate::app::TransportImpact {
                            tile_x: x,
                            tile_y: y,
                            color,
                            cue_color: None,
                            start_time: now_instant,
                        });
                }
                sow_core::game::GameEvent::NavalActionRejected { player_id, reason } => {
                    if player_id != my_id || my_id == 0 {
                        continue;
                    }
                    // Literal keys: the publisher validates every UiText key against the catalog.
                    let text = match reason {
                        sow_core::game::NavalActionFailure::InvalidTarget
                        | sow_core::game::NavalActionFailure::TargetNotWater => {
                            crate::ui::UiText::new("hud.fleet_invalid_target")
                        }
                        sow_core::game::NavalActionFailure::NoPort => {
                            crate::ui::UiText::new("hud.fleet_no_water_access")
                        }
                        sow_core::game::NavalActionFailure::NoConnectedPort
                        | sow_core::game::NavalActionFailure::NoWaterPath => {
                            crate::ui::UiText::new("hud.fleet_no_water_path")
                        }
                        sow_core::game::NavalActionFailure::CapacityFull => {
                            crate::ui::UiText::new("hud.fleet_capacity_full")
                        }
                        sow_core::game::NavalActionFailure::QueueFull => {
                            crate::ui::UiText::new("hud.warship_queue_full")
                        }
                        sow_core::game::NavalActionFailure::InsufficientGold => {
                            let available = snap
                                .players
                                .iter()
                                .find(|player| player.id == my_id)
                                .map_or(0.0, |player| player.gold);
                            let owned_warships = snap
                                .fleets
                                .iter()
                                .filter(|fleet| {
                                    fleet.owner_id == my_id
                                        && fleet.unit_type == sow_core::game::UnitType::Warship
                                })
                                .count() as u32;
                            crate::ui::UiText::new("hud.not_enough_gold")
                                .with(
                                    "cost",
                                    sow_core::game::UnitType::Warship
                                        .gold_cost_with_owned_count_for_leader(
                                            owned_warships,
                                            snap.players
                                                .iter()
                                                .find(|player| player.id == my_id)
                                                .map_or(
                                                    sow_core::player::Leader::default(),
                                                    |player| player.leader,
                                                ),
                                        )
                                        .ceil()
                                        .to_string(),
                                )
                                .with("available", available.floor().to_string())
                        }
                        sow_core::game::NavalActionFailure::CampaignLocked => {
                            crate::ui::UiText::new("hud.campaign_unlock_required")
                        }
                    };
                    self.ui.app.hud_state.push_notification_for_players(
                        text,
                        [Some(my_id), None],
                        3,
                        None,
                        false,
                    );
                }
                sow_core::game::GameEvent::NuclearActionRejected {
                    player_id,
                    reason,
                    cost,
                } => {
                    if player_id != my_id || my_id == 0 {
                        continue;
                    }
                    let text = match reason {
                        sow_core::game::NuclearActionFailure::CityRequirement => {
                            crate::ui::UiText::new("hud.nuke_requires_city")
                        }
                        sow_core::game::NuclearActionFailure::NoReadySilo => {
                            crate::ui::UiText::new("hud.nuke_no_charges")
                        }
                        sow_core::game::NuclearActionFailure::InsufficientGold => {
                            let available = snap
                                .players
                                .iter()
                                .find(|player| player.id == my_id)
                                .map_or(0.0, |player| player.gold);
                            crate::ui::UiText::new("hud.not_enough_gold")
                                .with("cost", cost.ceil().to_string())
                                .with("available", available.floor().to_string())
                        }
                    };
                    self.ui.app.hud_state.push_notification_for_players(
                        text,
                        [Some(my_id), None],
                        3,
                        None,
                        false,
                    );
                }
                sow_core::game::GameEvent::WarshipSpawned { owner_id, fleet_id, tile }
                    if my_id != 0 && owner_id == my_id =>
                {
                    let (x, y) = crate::render::world::movers::tile_to_world(
                        tile, self.sim.map_w.max(1),
                    );
                    if !self.ui.app.settings_state.reduced_motion {
                        self.ui.mover_scene.warship_launch_pulse([x, y], owner_id, now_instant);
                    }
                    self.ui.app.hud_state.push_warship_launch(fleet_id);
                    self.sfx.play_completion(
                        now_instant,
                        sow_audio::BuildingSoundKind::Port,
                        self.spatial_sound_params(x, y),
                    );
                }
                sow_core::game::GameEvent::NavalShot { owner_id, tile, .. } => {
                    let x = tile % self.sim.map_w.max(1);
                    let y = tile / self.sim.map_w.max(1);
                    self.sfx.play_combat(
                        now_instant,
                        sow_audio::CombatSoundKind::NavalShot,
                        1.0,
                        (u32::from(owner_id) << 16) ^ tile,
                        self.spatial_sound_params(x as f32 + 0.5, y as f32 + 0.5),
                    );
                }
                sow_core::game::GameEvent::NavalImpact {
                    owner_id,
                    target_owner,
                    tile,
                    target_type,
                    troops_lost,
                    sunk,
                    ..
                } => {
                    let x = tile % self.sim.map_w.max(1);
                    let y = tile / self.sim.map_w.max(1);
                    let color = snap
                        .players
                        .iter()
                        .find(|player| player.id == owner_id)
                        .map(|player| {
                            sow_core::player::player_territory_rgb(
                                player.player_type,
                                player.team,
                                player.color,
                                player.is_campaign_faction,
                            )
                        })
                        .unwrap_or([0.13, 0.83, 0.94]);
                    if self.ui.transport_impacts.len() == crate::app::MAX_TRANSPORT_IMPACTS {
                        self.ui.transport_impacts.pop_front();
                    }
                    self.ui
                        .transport_impacts
                        .push_back(crate::app::TransportImpact {
                            tile_x: x,
                            tile_y: y,
                            color,
                            cue_color: None,
                            start_time: now_instant,
                        });
                    if sunk && crate::theme::dev_config::DevConfig::with(|dev| dev.vfx_blasts) {
                        let victim_rgb = snap
                            .players
                            .iter()
                            .find(|player| player.id == target_owner)
                            .map(|player| {
                                sow_core::player::player_territory_rgb(
                                    player.player_type,
                                    player.team,
                                    player.color,
                                    player.is_campaign_faction,
                                )
                            })
                            .unwrap_or(color);
                        self.ui.blast_scene.spawn_entity_death(
                            x as f32 + 0.5,
                            y as f32 + 0.5,
                            tile,
                            owner_id,
                            victim_rgb,
                            3.0,
                            now_instant,
                        );
                    }
                    if my_id != 0
                        && owner_id == my_id
                        && target_type == sow_core::game::UnitType::TransportShip
                        && sunk
                    {
                        let target_player =
                            snap.players.iter().find(|player| player.id == target_owner);
                        let target = target_player
                            .map(|player| {
                                sow_core::player::display_name(
                                    player.id,
                                    &player.name,
                                    player.player_type,
                                )
                            })
                            .unwrap_or_else(|| format!("Player {target_owner}"));
                        let color = target_player
                            .map(crate::render::world::nameplates::player_color)
                            .unwrap_or([0.85, 0.85, 0.85, 1.0]);
                        let avatar = target_player
                            .map(sow_core::player::avatar_identity)
                            .unwrap_or(sow_core::player::AvatarIdentity::Fallback);
                        crate::app::DeathNameplateAnimation::enqueue(
                            &mut self.ui.death_nameplates,
                            crate::app::DeathNameplateAnimation {
                                world_x: x as f32 + 0.5,
                                world_y: y as f32 + 0.5,
                                curve_direction: if target_owner % 2 == 0 { 1.0 } else { -1.0 },
                                start_time: now_instant,
                                by_nuke: false,
                                name: target,
                                color,
                                avatar,
                                casualties: Some(format!(
                                    "-{}",
                                    crate::utils::format_number(troops_lost)
                                )),
                                reward: Some(crate::app::DeathNameplateReward {
                                    amount: format!(
                                        "+{}",
                                        crate::utils::format_number(
                                            target_type.naval_bounty_gold()
                                        )
                                    ),
                                    is_assist: false,
                                }),
                                prepared_name: sow_render::text::PreparedText::default(),
                                prepared_casualties: None,
                                prepared_reward: None,
                                text_style_key: None,
                            },
                        );
                        let leader_gender = snap
                            .players
                            .iter()
                            .find(|player| player.id == my_id)
                            .and_then(|player| player.leader.character().gender);
                        self.record_combat_kill_feedback(
                            x as f32 + 0.5,
                            y as f32 + 0.5,
                            leader_gender,
                            now_instant,
                        );
                    }
                    self.sfx.play_combat(
                        now_instant,
                        if sunk {
                            sow_audio::CombatSoundKind::NavalSink
                        } else {
                            sow_audio::CombatSoundKind::NavalImpact
                        },
                        1.0,
                        (u32::from(owner_id) << 16) ^ tile,
                        self.spatial_sound_params(x as f32 + 0.5, y as f32 + 0.5),
                    );
                }
                sow_core::game::GameEvent::TradeShipCaptured { .. } => {}
                sow_core::game::GameEvent::AbilityTriggered {
                    owner_id,
                    tile,
                    affected,
                } => {
                    if !ability_is_visible_to_player(owner_id, &affected, my_id) {
                        continue;
                    }
                    let Some(owner) = snap.players.iter().find(|player| player.id == owner_id)
                    else {
                        continue;
                    };
                    if let Some((x, y)) = tile {
                        if let Some(renderer) = self.gfx.map_renderer.as_mut() {
                            renderer.flash_ability_tile(x, y);
                        }
                        let color = sow_core::player::player_territory_rgb(
                            owner.player_type,
                            owner.team,
                            owner.color,
                            owner.is_campaign_faction,
                        );
                        if self.ui.transport_impacts.len() == crate::app::MAX_TRANSPORT_IMPACTS {
                            self.ui.transport_impacts.pop_front();
                        }
                        self.ui
                            .transport_impacts
                            .push_back(crate::app::TransportImpact {
                                tile_x: x,
                                tile_y: y,
                                color,
                                cue_color: Some(crate::app::ability_cue_rgb(
                                    owner.leader.ability().cue(),
                                )),
                                start_time: now_instant,
                            });
                    }
                    // The notice shows the hero's icon and its own ability name.
                    let hero = sow_core::commerce::leader_id(owner.leader).to_string();
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.ability_notice")
                            .with("hero", hero.clone())
                            .with("hero_art", owner.leader.art_slug())
                            .with("ability_key", format!("site.leader_{hero}_ability"))
                            .with("description_key", format!("site.leader_{hero}_description")),
                        [Some(owner_id), (my_id != owner_id).then_some(my_id)],
                        3,
                        Some(format!("ability:{owner_id}")),
                        false,
                    );
                }
                sow_core::game::GameEvent::PlayerEliminated {
                    player_id,
                    conqueror_id,
                    gold_bounty,
                    elimination_x,
                    elimination_y,
                    assists,
                    by_nuke,
                } => {
                    self.handle_player_eliminated(
                        snap,
                        my_id,
                        now_instant,
                        &mut turn_defeats,
                        &elimination::EliminationEventInfo {
                            player_id,
                            conqueror_id,
                            pos: (elimination_x, elimination_y),
                            gold_bounty,
                            assists: &assists,
                            by_nuke,
                        },
                    );
                    if conqueror_id == my_id && my_id != 0 {
                        self.ui
                            .trigger_viewport_alert(crate::app::ViewportAlertKind::ConquerPlayer);
                    }
                }
                sow_core::game::GameEvent::TileCaptured {
                    x,
                    y,
                    new_owner,
                    previous_owner,
                    ..
                } => {
                    if my_id != 0 && new_owner == my_id && previous_owner != 0 {
                        captured_enemy_tiles = captured_enemy_tiles.saturating_add(1);
                        last_enemy_capture_pos = Some((x as f32 + 0.5, y as f32 + 0.5));
                    }
                    self.handle_tile_captured(
                        my_id,
                        &mut played_combat_this_tick,
                        &combat::CaptureEventInfo {
                            new_owner,
                            previous_owner,
                        },
                    );
                }
                sow_core::game::GameEvent::AllianceRequestLifecycle {
                    proposer_id,
                    target_id,
                    status,
                } => {
                    if proposer_id != my_id || my_id == 0 {
                        continue;
                    }
                    let was_allied = self
                        .sim
                        .current_snapshot
                        .as_ref()
                        .and_then(|previous| {
                            previous
                                .players
                                .iter()
                                .find(|player| player.id == proposer_id)
                        })
                        .is_some_and(|player| player.alliances.contains(&target_id));
                    let is_allied = snap
                        .players
                        .iter()
                        .find(|player| player.id == proposer_id)
                        .is_some_and(|player| player.alliances.contains(&target_id));
                    if suppress_submitted_notice_if_formed(status, was_allied, is_allied) {
                        continue;
                    }
                    let target_name = snap
                        .players
                        .iter()
                        .find(|player| player.id == target_id)
                        .map(|player| {
                            sow_core::player::display_name(
                                player.id,
                                &player.name,
                                player.player_type,
                            )
                        })
                        .unwrap_or_else(|| format!("Player {target_id}"));
                    if let Some((text, players)) = alliance_request_notification(
                        proposer_id,
                        target_id,
                        status,
                        my_id,
                        target_name,
                    ) {
                        self.ui
                            .app
                            .hud_state
                            .push_notification_for_players(text, players, 3, None, false);
                    }
                }
                sow_core::game::GameEvent::StructureCaptured {
                    kind,
                    new_owner,
                    tile_idx,
                    level,
                    ..
                } => {
                    if new_owner == my_id && my_id != 0 && self.sim.config.tutorial {
                        let candidate = (kind, level, tile_idx);
                        let replace = best_tutorial_capture.is_none_or(|(best_kind, _, _)| {
                            crate::app::structure_capture_priority(kind)
                                > crate::app::structure_capture_priority(best_kind)
                        });
                        if replace {
                            best_tutorial_capture = Some(candidate);
                        }
                    }
                }
                sow_core::game::GameEvent::NukeDetonated {
                    tile_x,
                    tile_y,
                    inner_radius,
                    outer_radius,
                    owner_id,
                } => {
                    let vfx_blasts =
                        crate::theme::dev_config::DevConfig::with(|dev| dev.vfx_blasts);
                    let reduced_motion = self.ui.app.settings_state.reduced_motion;
                    let expand_dur = sow_render::crater_expand_duration(
                        inner_radius,
                        reduced_motion || !vfx_blasts,
                    );
                    if expand_dur > 0.0
                        && let Some(ref mut mr) = self.gfx.map_renderer
                    {
                        mr.enqueue_crater_reveal(
                            tile_x,
                            tile_y,
                            inner_radius,
                            expand_dur,
                            now_instant,
                        );
                    }
                    if vfx_blasts {
                        let sf = (crate::web_canvas::device_pixel_ratio() as f32).max(1.0);
                        self.ui.blast_scene.spawn_nuke_detonation(
                            tile_x,
                            tile_y,
                            inner_radius,
                            outer_radius,
                            owner_id,
                            self.sim.map_w,
                            expand_dur,
                            now_instant,
                            reduced_motion,
                            (
                                self.input.camera_x,
                                self.input.camera_y,
                                self.input.camera_zoom,
                            ),
                            (self.input.screen_w, self.input.screen_h),
                            sf,
                        );
                    }
                }
                sow_core::game::GameEvent::TacticalSurge {
                    player_id,
                    tile_x,
                    tile_y,
                    bonus_troops,
                    chain_step,
                } => {
                    if player_id == my_id && my_id != 0 {
                        let wx = tile_x as f32 + 0.5;
                        let wy = tile_y as f32 + 0.5;
                        let amount = crate::utils::format_number(bonus_troops as f64);
                        let label = if chain_step >= 2 {
                            format!("+{amount} x{chain_step}")
                        } else {
                            format!("+{amount}")
                        };
                        let step = self.ui.push_cascade_callout(
                            wx,
                            wy,
                            label,
                            Some(chain_step),
                            crate::app::CascadeCalloutKind::Surge,
                            Some(crate::app::CascadeIcon::Status(
                                sow_render::text::NameplateStatusSprite::Troops,
                            )),
                            28.0,
                            now_instant,
                        );
                        self.sfx
                            .play_cascade(now_instant, step, self.spatial_sound_params(wx, wy));
                        self.ui
                            .trigger_viewport_alert(crate::app::ViewportAlertKind::ConquerPlayer);
                    }
                }
                _ => {}
            }
        }
        if let Some((kind, level, tile_idx)) = best_tutorial_capture {
            let map_w = self.sim.map_w.max(1);
            let wx = (tile_idx % map_w) as f32 + 0.5;
            let wy = (tile_idx / map_w) as f32 + 0.5;
            let bonus_troops = u32::from(level.max(1)) * 120;
            let label = format!("+{}", crate::utils::format_number(bonus_troops as f64));
            let step = self.ui.push_cascade_callout(
                wx,
                wy,
                label,
                None,
                crate::app::CascadeCalloutKind::Capture,
                Some(crate::app::CascadeIcon::Building(kind)),
                28.0,
                now_instant,
            );
            self.sfx
                .play_cascade(now_instant, step, self.spatial_sound_params(wx, wy));
        }
        if let Some((wx, wy)) = last_enemy_capture_pos
            && let Some(step) =
                self.ui
                    .record_territory_advance(captured_enemy_tiles, wx, wy, now_instant)
        {
            self.sfx
                .play_cascade(now_instant, step, self.spatial_sound_params(wx, wy));
        }
        self.sfx.flush_tick(now_instant);
        turn_defeats
    }
}
