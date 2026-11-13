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
        let now_instant = web_time::Instant::now();
        let alive_warships: std::collections::HashSet<u64> = snap
            .fleets
            .iter()
            .filter(|fleet| {
                fleet.owner_id == my_id && fleet.unit_type == sow_core::game::UnitType::Warship
            })
            .map(|fleet| fleet.id)
            .collect();
        self.input
            .selected_warships
            .retain(|id| alive_warships.contains(id));
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
                sow_core::game::GameEvent::WarshipSpawned {
                    owner_id, fleet_id, ..
                } => {
                    if owner_id == my_id {
                        self.ui.app.hud_state.selected_warship_build = false;
                        self.input.selected_warships = vec![fleet_id];
                    }
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
                sow_core::game::GameEvent::NavalShot { owner_id, tile } => {
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
                    tile,
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
                sow_core::game::GameEvent::PlayerRebelled {
                    player_id,
                    rebel_id,
                    ..
                } if player_id == my_id && my_id != 0 => {
                    let name = snap
                        .players
                        .iter()
                        .find(|player| player.id == rebel_id)
                        .map(|player| player.name.clone())
                        .unwrap_or_else(|| "Rebels".to_string());
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.debt_rebellion").with("name", name),
                        [Some(my_id), Some(rebel_id)],
                        3,
                        Some(format!("debt-rebellion:{rebel_id}")),
                        false,
                    );
                }
                sow_core::game::GameEvent::TileCaptured {
                    x,
                    y,
                    new_owner,
                    previous_owner,
                    troops,
                } => {
                    self.handle_tile_captured(
                        snap,
                        my_id,
                        &mut played_combat_this_tick,
                        &combat::CaptureEventInfo {
                            pos: (x, y),
                            new_owner,
                            previous_owner,
                            troops,
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
                _ => {}
            }
        }
        self.sfx.flush_tick(now_instant);
        turn_defeats
    }
}

