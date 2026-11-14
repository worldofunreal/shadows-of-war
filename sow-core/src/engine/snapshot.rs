use crate::engine::SowEngine;

impl SowEngine {
    pub fn build_snapshot(&mut self) -> crate::protocol::SimSnapshot {
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

                let mut alliance_requests = Vec::new();
                let mut alliance_request_timers = std::collections::HashMap::new();
                for prop in proposed.iter().filter(|prop| prop.target == p.id) {
                    alliance_requests.push(prop.proposer);
                    let remaining = crate::diplomacy::ALLIANCE_REQUEST_TTL_TICKS
                        .saturating_sub(tick.saturating_sub(prop.created_tick));
                    alliance_request_timers.insert(prop.proposer, remaining);
                }

                let resource_requests = proposed_resources
                    .iter()
                    .filter(|r| r.target == p.id && self.resources_can_move(r.proposer, r.target))
                    .map(|r| crate::protocol::ResourceRequest {
                        requester: r.proposer,
                        gold: r.gold,
                        troops: r.troops,
                        pay_gold: r.pay_gold,
                        pay_troops: r.pay_troops,
                        remaining_ticks: crate::diplomacy::RESOURCE_REQUEST_TTL_TICKS
                            .saturating_sub(tick.saturating_sub(r.created_tick)),
                    })
                    .collect();

                let help_calls = proposed_help
                    .iter()
                    .filter(|h| h.target == p.id && self.resources_can_move(h.target, h.caller))
                    .filter_map(|h| {
                        let caller = self.state.player(h.caller)?;
                        let caller_under_attack = self.is_player_under_attack(h.caller);
                        let (suggested_gold, suggested_troops) =
                            crate::diplomacy::compute_help_aid(p, caller, caller_under_attack);
                        Some(crate::protocol::HelpCall {
                            caller: h.caller,
                            suggested_gold,
                            suggested_troops,
                            remaining_ticks: crate::diplomacy::HELP_CALL_TTL_TICKS
                                .saturating_sub(tick.saturating_sub(h.created_tick)),
                        })
                    })
                    .collect();
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
                    empowered_attack: self.attacks.iter().any(|a| {
                        a.owner_id == p.id
                            && crate::execution::combat::empowered_attack_applies(
                                &self.state,
                                p.id,
                                a.target_owner,
                                a.troops,
                            )
                    }),
                    campaign_avatar: self.campaign_avatars.get(&p.id).cloned(),
                    is_campaign_faction: self.campaign_faction_ids.contains_key(&p.id),
                    skin_style: p.skin_style,
                    kills: p.kills,
                    deaths: p.deaths,
                    assists: p.assists,
                    boats_in_use: boats_in_use.get(p.id as usize).copied().unwrap_or_default(),
                    boat_capacity: crate::building::cost::player_fleet_capacity_from_port_levels(
                        self.building_aggregates
                            .get(p.id as usize)
                            .map_or(0, |aggregate| aggregate.port_levels),
                    ),
                    nuke_available: p.alive
                        && self.buildings.iter().any(|building| {
                            building.owner_id == p.id
                                && building.kind == crate::game::BuildingKind::City
                                && !building.under_construction
                                && self
                                    .city_arsenals
                                    .get(&building.id)
                                    .is_some_and(|state| state.nuke_ready)
                        }),
                    nuke_cooldown_ticks: self
                        .buildings
                        .iter()
                        .filter(|building| {
                            building.owner_id == p.id
                                && building.kind == crate::game::BuildingKind::City
                                && !building.under_construction
                        })
                        .filter_map(|building| {
                            self.city_arsenals.get(&building.id).and_then(|state| {
                                state.arsenal_build.and_then(|build| {
                                    (build.kind == crate::game::CityArsenalKind::Nuke)
                                        .then_some(build.ticks_until_complete)
                                })
                            })
                        })
                        .min()
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
                let (has_anti_nuke, nuke_ready, arsenal_build, sam_cooldown_ticks) =
                    if b.kind == crate::game::BuildingKind::City {
                        let arsenal = self.city_arsenals.get(&b.id).copied().unwrap_or_default();
                        let sam_cd = if arsenal.has_anti_nuke {
                            self.silo_cooldowns.get(&b.id).copied().unwrap_or(0)
                        } else {
                            0
                        };
                        (
                            arsenal.has_anti_nuke,
                            arsenal.nuke_ready,
                            arsenal.arsenal_build,
                            sam_cd,
                        )
                    } else {
                        (false, false, None, 0)
                    };
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
                    sam_cooldown_ticks,
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
                    path: p.path.clone(),
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
