use crate::engine::SowEngine;

impl SowEngine {
    pub fn build_snapshot(&mut self) -> crate::protocol::SimSnapshot {
        self.refresh_building_aggregates();
        let mut dirty_indices = std::mem::take(&mut self.state.map.dirty_tiles);
        let dirty_tiles: Vec<crate::protocol::DirtyTile> = {
            let map = &self.state.map;
            dirty_indices
                .drain(..)
                .map(|i| crate::protocol::DirtyTile {
                    index: i as u32,
                    new_owner: map.owner_states()[i],
                })
                .collect()
        };
        self.state.map.dirty_tiles = dirty_indices;
        self.refresh_building_aggregates();

        let tick = self.current_tick_u32();
        let proposed = &self.alliances_proposed;
        let proposed_resources = &self.resource_requests_proposed;
        let proposed_help = &self.help_calls_proposed;
        let emote_phase = self.state.phase.clone();
        let boats_in_use = self.boat_slots_used_by_player();
        let max_pid = self
            .state
            .players
            .iter()
            .map(|p| usize::from(p.id))
            .max()
            .unwrap_or(0);

        let mut alliance_reqs_by_target: std::collections::HashMap<
            u16,
            (Vec<u16>, std::collections::HashMap<u16, u32>),
        > = std::collections::HashMap::new();
        for prop in proposed {
            let entry = alliance_reqs_by_target
                .entry(prop.target)
                .or_insert_with(|| (Vec::new(), std::collections::HashMap::new()));
            entry.0.push(prop.proposer);
            let remaining = crate::diplomacy::ALLIANCE_REQUEST_TTL_TICKS
                .saturating_sub(tick.saturating_sub(prop.created_tick));
            entry.1.insert(prop.proposer, remaining);
        }

        let mut resource_reqs_by_target: std::collections::HashMap<
            u16,
            Vec<crate::protocol::ResourceRequest>,
        > = std::collections::HashMap::new();
        for r in proposed_resources {
            if self.resources_can_move(r.proposer, r.target) {
                resource_reqs_by_target
                    .entry(r.target)
                    .or_default()
                    .push(crate::protocol::ResourceRequest {
                        requester: r.proposer,
                        gold: r.gold,
                        troops: r.troops,
                        pay_gold: r.pay_gold,
                        pay_troops: r.pay_troops,
                        remaining_ticks: crate::diplomacy::RESOURCE_REQUEST_TTL_TICKS
                            .saturating_sub(tick.saturating_sub(r.created_tick)),
                    });
            }
        }

        let mut help_calls_by_target: std::collections::HashMap<
            u16,
            Vec<crate::protocol::HelpCall>,
        > = std::collections::HashMap::new();
        for h in proposed_help {
            if self.resources_can_move(h.target, h.caller)
                && let Some(target_player) = self.state.player(h.target)
                && let Some(caller) = self.state.player(h.caller)
            {
                let caller_under_attack = self.is_player_under_attack(h.caller);
                let (suggested_gold, suggested_troops) =
                    crate::diplomacy::compute_help_aid(target_player, caller, caller_under_attack);
                help_calls_by_target
                    .entry(h.target)
                    .or_default()
                    .push(crate::protocol::HelpCall {
                        caller: h.caller,
                        call_id: h.call_id,
                        suggested_gold,
                        suggested_troops,
                        remaining_ticks: crate::diplomacy::HELP_CALL_TTL_TICKS
                            .saturating_sub(tick.saturating_sub(h.created_tick)),
                    });
            }
        }
        let mut nuke_available_by_player = vec![false; max_pid + 1];
        let mut nuke_cooldown_by_player = vec![0u32; max_pid + 1];
        for building in &self.buildings {
            let pid = usize::from(building.owner_id);
            if pid > max_pid || building.kind != crate::game::BuildingKind::MissileSilo {
                continue;
            }
            nuke_available_by_player[pid] |= self.nuclear_charges(building.id, building.active_level()) > 0;
            let wait = self.nuclear_wait_ticks(building.id);
            let current = &mut nuke_cooldown_by_player[pid];
            if wait > 0 && (*current == 0 || wait < *current) {
                *current = wait;
            }
        }
        for player_id in 0..=max_pid {
            if !crate::building::player_has_nuclear_launch_city(
                &self.buildings,
                player_id as u16,
            ) {
                nuke_available_by_player[player_id] = false;
            }
        }

        let mut empowered_attack_by_player = vec![false; max_pid + 1];
        for attack in &self.attacks {
            let pid = usize::from(attack.owner_id);
            if pid <= max_pid
                && !empowered_attack_by_player[pid]
                && crate::execution::combat::empowered_attack_applies(
                    &self.state,
                    attack.owner_id,
                    attack.target_owner,
                    attack.troops,
                )
            {
                empowered_attack_by_player[pid] = true;
            }
        }
        let players = self
            .state
            .players
            .iter()
            .map(|p| {
                let (cx, cy) = if p.tile_count > 0 {
                    (
                        (p.sum_x / p.tile_count as u64) as f32,
                        (p.sum_y / p.tile_count as u64) as f32,
                    )
                } else {
                    (0.0, 0.0)
                };

                let name = p.name.clone();

                let (alliance_requests, alliance_request_timers) = alliance_reqs_by_target
                    .remove(&p.id)
                    .unwrap_or_default();
                let resource_requests = resource_reqs_by_target.remove(&p.id).unwrap_or_default();
                let help_calls = help_calls_by_target.remove(&p.id).unwrap_or_default();
                let help_cooldown_ticks = self.help_call_cooldown_remaining(p.id);

                crate::protocol::PlayerSnapshot {
                    id: p.id,
                    name,
                    troops: p.troops,
                    max_troops: p.max_troops,
                    gold: p.gold,
                    tile_count: p.tile_count,
                    centroid_x: cx,
                    centroid_y: cy,
                    player_type: p.player_type,
                    color: self
                        .campaign_relations
                        .get(&p.id)
                        .and_then(|relation| crate::player::campaign_relation_rgb(*relation))
                        .unwrap_or(p.color),
                    team: p.team,
                    has_spawned: p.has_spawned,
                    alive: p.alive,
                    iq: p.iq,
                    alliances: p.alliances.clone(),
                    alliance_timers: p.alliance_timers.clone(),
                    alliance_requests,
                    alliance_request_timers,
                    resource_requests,
                    disconnected: p.disconnected,
                    active_emote: p.active_emote,
                    can_express_emote: crate::emote::can_express(p, &emote_phase),
                    can_retaliate: crate::diplomacy::can_retaliate_against(p),
                    traitor: p.traitor,
                    leader: p.leader,
                    empowered_attack: empowered_attack_by_player
                        .get(p.id as usize)
                        .copied()
                        .unwrap_or(false),
                    campaign_avatar: self.campaign_avatars.get(&p.id).cloned(),
                    is_campaign_faction: self.campaign_faction_ids.contains_key(&p.id),
                    skin_style: p.skin_style,
                    kills: p.kills,
                    deaths: p.deaths,
                    assists: p.assists,
                    boats_in_use: boats_in_use.get(p.id as usize).copied().unwrap_or_default(),
                    boat_capacity: crate::building::cost::player_fleet_capacity_from_port_slots(
                        self.building_aggregates
                            .get(p.id as usize)
                            .map_or(0, |aggregate| aggregate.port_fleet_slots),
                    ),
                    nuke_available: p.alive
                        && nuke_available_by_player
                            .get(p.id as usize)
                            .copied()
                            .unwrap_or(false),
                    nuke_cooldown_ticks: nuke_cooldown_by_player
                        .get(p.id as usize)
                        .copied()
                        .unwrap_or(0),
                    help_calls,
                    help_cooldown_ticks,
                }
            })
            .collect();

        let fleets = self
            .fleets
            .iter()
            .map(|f| crate::protocol::FleetSnapshot {
                id: f.id,
                owner_id: f.owner_id,
                unit_type: f.unit_type,
                troops: f.troops,
                current_tile: f.current_tile,
                path: f.path.clone(),
                path_cursor: f.path_cursor,
                movement_progress: f.movement_progress as f32,
                eta_seconds: f.remaining_eta_seconds(),
                retreating: f.retreating,
                max_health: f.warship.as_ref().map_or_else(
                    || f.unit_type.max_health(),
                    crate::warp_fleet::WarshipState::max_health,
                ),
                warship: f.warship.clone(),
                captured_trade_return: f.captured_trade_return.is_some(),
            })
            .collect();

        let attacks = self
            .attacks
            .iter()
            .map(|a| {
                let (fcx, fcy) = if a.target_owner != 0 {
                    a.frontier_centroid()
                } else {
                    (0.0, 0.0)
                };
                crate::protocol::AttackSnapshot {
                    id: a.id,
                    owner_id: a.owner_id,
                    target_owner: a.target_owner,
                    troops: a.troops,
                    retreating: a.retreating,
                    front_cx: fcx,
                    front_cy: fcy,
                }
            })
            .collect();

        let mut defense_posts = Vec::new();
        if self.render_defense_dirty {
            for b in &self.buildings {
                if b.kind == crate::game::BuildingKind::Tower && !b.under_construction {
                    defense_posts.push(b.tile_idx);
                }
            }
        }
        let defense_dirty = self.render_defense_dirty;
        self.render_defense_dirty = false;

        let spawn_timer_secs =
            if let crate::game::GamePhase::Spawning { end_tick } = &self.state.phase {
                Some(
                    end_tick.saturating_sub(self.state.tick) as f32
                        * (self.state.config.tick_rate_ms / 1000.0),
                )
            } else {
                None
            };

        let buildings: Vec<crate::protocol::BuildingSnapshot> = self
            .buildings
            .iter()
            .map(|b| {
                let mut production = self
                    .building_productions
                    .get(&b.id)
                    .cloned()
                    .unwrap_or_default();
                production.normalize_legacy_warship();
                let has_anti_nuke = self.legacy_building_rules
                    && b.kind == crate::game::BuildingKind::City
                    && production.has_anti_nuke;
                let nuke_ready = b.kind == crate::game::BuildingKind::MissileSilo
                    && self.nuclear_charges(b.id, b.active_level()) > 0;
                let (arsenal_build, warship_queue) = if b.kind == crate::game::BuildingKind::Port {
                    (production.active_warship_build(), production.warship_queue)
                } else {
                    (None, Vec::new())
                };
                let sam_cooldown_ticks = self.nuclear_wait_ticks(b.id);
                let upgrade_target_level = self
                    .upgrade_targets
                    .get(&b.id)
                    .copied()
                    .filter(|&target| target > b.level)
                    .unwrap_or(0);
                crate::protocol::BuildingSnapshot {
                    id: b.id,
                    tile_idx: b.tile_idx,
                    owner_id: b.owner_id,
                    kind: b.kind,
                    level: b.level,
                    under_construction: b.under_construction,
                    ticks_until_complete: b.ticks_until_complete,
                    has_anti_nuke,
                    nuke_ready,
                    arsenal_build,
                    warship_queue,
                    sam_cooldown_ticks,
                    upgrade_target_level,
                }
            })
            .collect();

        crate::protocol::SimSnapshot {
            tick: self.state.tick,
            phase: self.state.phase.clone(),
            spawn_timer_secs,
            players,
            dirty_tiles,
            fleets,
            attacks,
            buildings,
            railway_links: self.railway_links.clone(),
            projectiles: self
                .projectiles
                .iter()
                .filter(|p| p.active)
                .map(|p| crate::protocol::ProjectileSnapshot {
                    id: p.id,
                    kind: p.kind,
                    owner_id: p.owner_id,
                    src_tile: p.src_tile,
                    dst_tile: p.dst_tile,
                    path: std::sync::Arc::clone(&p.path),
                    path_cursor: p.path_cursor,
                    steps_per_tick: p.steps_per_tick,
                    source_fleet_id: p.source_fleet_id,
                    target_fleet_id: p.target_fleet_id,
                })
                .collect(),
            nuke_alerts: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::NukeDetonated {
                        tile_x,
                        tile_y,
                        owner_id,
                        inner_radius: _,
                        outer_radius: _,
                    } = e
                    {
                        let kind = crate::game::NukeKind::AtomBomb;
                        Some(crate::protocol::NukeAlert {
                            owner_id: *owner_id,
                            kind,
                            tile_x: *tile_x,
                            tile_y: *tile_y,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            resource_transfers: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::ResourceTransferred {
                        sender_id,
                        receiver_id,
                        gold,
                        troops,
                    } = e
                    {
                        Some(crate::protocol::ResourceTransfer {
                            sender_id: *sender_id,
                            receiver_id: *receiver_id,
                            gold: *gold,
                            troops: *troops,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            resource_rejections: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::ResourceRequestRejected {
                        rejector_id,
                        requester_id,
                    } = e
                    {
                        Some(crate::protocol::ResourceRejection {
                            rejector_id: *rejector_id,
                            requester_id: *requester_id,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            help_call_rejections: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::HelpCallDeclined {
                        responder_id,
                        caller_id,
                    } = e
                    {
                        Some(crate::protocol::ResourceRejection {
                            rejector_id: *responder_id,
                            requester_id: *caller_id,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            resource_transfer_rejections: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::ResourceTransferRejected {
                        sender_id,
                        receiver_id,
                    } = e
                    {
                        Some(crate::protocol::ResourceTransferRejection {
                            sender_id: *sender_id,
                            receiver_id: *receiver_id,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            winner: self.state.winner,
            winning_team: self.state.winning_team,
            total_land_tiles: self.state.total_land_tiles,
            defense_posts,
            defense_dirty,
            sea_lanes: self.state.sea_lanes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::SowEngine;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    #[test]
    fn snapshot_marks_campaign_membership_not_avatar_presence() {
        let state = GameState::new(1, 4, 4, GameConfig::default());
        let mut engine = SowEngine::new(state, WaterComponents::default());
        let config = engine.state.config.clone();
        engine.state.players.push(Player::new_bot(
            1,
            "Roman Outpost".into(),
            [1.0, 0.0, 0.0],
            &config,
        ));
        engine.state.players.push(Player::new_bot(
            2,
            "Geographic faction".into(),
            [0.0, 1.0, 0.0],
            &config,
        ));
        engine
            .campaign_faction_ids
            .insert(1, "roman_outpost".into());
        engine.campaign_avatars.insert(2, "some_portrait".into());

        let snapshot = engine.build_snapshot();
        assert!(
            snapshot
                .players
                .iter()
                .find(|p| p.id == 1)
                .unwrap()
                .is_campaign_faction
        );
        assert!(
            !snapshot
                .players
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .is_campaign_faction
        );
    }
}
