use crate::app::{SowApp, TutorialObservation};
use sow_core::engine::SowEngine;
use sow_core::game::GameEvent;
use sow_core::protocol::Turn;

fn building_confirmation_result(
    placement: Option<crate::ui::hud::state::BuildingPlacement>,
    my_id: u16,
    intents: &[sow_core::protocol::StampedIntent],
    events: &[GameEvent],
) -> Option<bool> {
    let placement = placement.filter(|placement| placement.pending)?;
    intents.iter().any(|stamped| stamped.player_id == my_id && matches!(
        stamped.intent, sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile }
            if kind == placement.kind && target_tile == placement.tile_idx
    )).then(|| events.iter().any(|event| matches!(event,
        GameEvent::StructureSpawned { owner_id, kind, tile_idx, .. }
            if *owner_id == my_id && *kind == placement.kind && *tile_idx == placement.tile_idx
    )))
}

fn take_alliance_request_lifecycle_events(events: &mut Vec<GameEvent>) -> Vec<GameEvent> {
    let mut lifecycle_events = Vec::new();
    events.retain(|event| {
        if matches!(event, GameEvent::AllianceRequestLifecycle { .. }) {
            lifecycle_events.push(event.clone());
            false
        } else {
            true
        }
    });
    lifecycle_events
}

fn take_ability_events(events: &mut Vec<GameEvent>) -> Vec<GameEvent> {
    let mut ability_events = Vec::new();
    events.retain(|event| {
        if matches!(event, GameEvent::AbilityTriggered { .. }) {
            ability_events.push(event.clone());
            false
        } else {
            true
        }
    });
    ability_events
}

fn take_resource_transfer_events(events: &mut Vec<GameEvent>) -> Vec<GameEvent> {
    let mut transfer_events = Vec::new();
    events.retain(|event| {
        if matches!(
            event,
            GameEvent::ResourceRequestSubmitted { .. }
            | GameEvent::ResourceTransferred { .. }
            | GameEvent::ResourceTransferRejected { .. }
        ) {
            transfer_events.push(event.clone());
            false
        } else {
            true
        }
    });
    transfer_events
}

impl TutorialObservation {
    pub(crate) fn observe_sim(&mut self, engine: &SowEngine, my_id: u16) {
        if my_id == 0 {
            return;
        }
        self.pre_defeat_faction_ids.clear();
        if let Some(faction_id) = engine.campaign_pre_defeat_faction_id() {
            self.pre_defeat_faction_ids.insert(faction_id.to_owned());
        }
        for attack in &engine.attacks {
            if attack.owner_id == my_id
                && self.seen_attacks.insert(attack.id)
                && let Some(target) = engine.state.player(attack.target_owner)
                && let Some(faction_id) = engine.campaign_faction_ids.get(&target.id)
            {
                let count = self
                    .attacks_by_faction_id
                    .entry(faction_id.clone())
                    .or_default();
                *count = count.saturating_add(1);
            }
        }
        for fleet in engine.fleets.iter().filter(|fleet| fleet.owner_id == my_id) {
            if !self.seen_fleets.insert(fleet.id) {
                continue;
            }
            let kind = match fleet.unit_type {
                sow_core::game::UnitType::TransportShip => "TransportShip",
                sow_core::game::UnitType::TradeShip => "TradeShip",
                sow_core::game::UnitType::Warship => "Warship",
            };
            let count = self
                .seen_fleets_by_type
                .entry(kind.to_string())
                .or_default();
            *count = count.saturating_add(1);
            if fleet.unit_type == sow_core::game::UnitType::TransportShip
                && let Some(target) = engine.state.player(fleet.target_owner)
                && let Some(faction_id) = engine.campaign_faction_ids.get(&target.id)
            {
                let count = self
                    .seen_transport_fleets_by_faction_id
                    .entry(faction_id.clone())
                    .or_default();
                *count = count.saturating_add(1);
            }
        }
        self.seen_nukes.extend(
            engine
                .projectiles
                .iter()
                .filter(|projectile| {
                    projectile.owner_id == my_id
                        && projectile.active
                        && matches!(projectile.kind, sow_core::game::ProjectileKind::Nuke { .. })
                })
                .map(|projectile| projectile.id),
        );
        self.owned_structures.clear();
        self.owned_structure_kinds.clear();
        for building in &engine.buildings {
            if building.owner_id == my_id {
                self.owned_structures.insert(building.id);
                self.owned_structure_kinds
                    .insert(building.id, building.kind);
                // Campaign build objectives mean "placement accepted", not
                // "construction completed". The authoritative snapshot contains
                // the new building (including level-0 construction) as soon as the
                // server accepts the intent.
                self.seen_structures.insert(building.id);
                let kind = match building.kind {
                    sow_core::game::BuildingKind::City => "cities",
                    sow_core::game::BuildingKind::Tower => "towers",
                    sow_core::game::BuildingKind::Factory => "factories",
                    sow_core::game::BuildingKind::Port => "ports",
                    sow_core::game::BuildingKind::Farm => "farms",
                };
                self.seen_buildings_by_kind
                    .entry(kind.to_string())
                    .or_default()
                    .insert(building.id);
                if building.kind == sow_core::game::BuildingKind::City {
                    self.seen_cities.insert(building.id);
                }
            }
        }
        for (kind, key) in [
            (sow_core::game::BuildingKind::City, "city"),
            (sow_core::game::BuildingKind::Farm, "farm"),
            (sow_core::game::BuildingKind::Factory, "factory"),
            (sow_core::game::BuildingKind::Tower, "tower"),
            (sow_core::game::BuildingKind::Port, "port"),
        ] {
            let level = engine
                .buildings
                .iter()
                .filter(|building| building.owner_id == my_id && building.kind == kind)
                .map(|building| u64::from(building.active_level()))
                .max()
                .unwrap_or_default();
            self.structure_levels
                .entry(key.to_string())
                .and_modify(|current| *current = (*current).max(level))
                .or_insert(level);
        }
        let Some(me) = engine.state.player(my_id) else {
            return;
        };
        for ally_id in &me.alliances {
            if self.seen_alliances.insert(*ally_id) && self.alliances_initialized {
                self.alliances_formed = self.alliances_formed.saturating_add(1);
            }
            if let Some(ally) = engine.state.player(*ally_id) {
                if let Some(faction_id) = engine.campaign_faction_ids.get(&ally.id) {
                    self.seen_alliance_faction_ids.insert(faction_id.clone());
                }
            }
        }
        self.alliances_initialized = true;
        self.city_levels = self.city_levels.max(
            engine
                .buildings
                .iter()
                .filter(|building| {
                    building.owner_id == my_id
                        && building.kind == sow_core::game::BuildingKind::City
                })
                .map(|building| u64::from(building.active_level()))
                .max()
                .unwrap_or_default(),
        );
        self.port_levels = self.port_levels.max(
            engine
                .buildings
                .iter()
                .filter(|building| {
                    building.owner_id == my_id
                        && building.kind == sow_core::game::BuildingKind::Port
                })
                .map(|building| u64::from(building.active_level()))
                .max()
                .unwrap_or_default(),
        );
        for building in &engine.buildings {
            let Some(faction_id) = engine.campaign_faction_ids.get(&building.owner_id) else {
                continue;
            };
            let key = match building.kind {
                sow_core::game::BuildingKind::City => "city",
                sow_core::game::BuildingKind::Farm => "farm",
                sow_core::game::BuildingKind::Factory => "factory",
                sow_core::game::BuildingKind::Tower => "tower",
                sow_core::game::BuildingKind::Port => "port",
            };
            let level = u64::from(building.active_level());
            self.faction_structure_levels
                .entry(faction_id.clone())
                .or_default()
                .entry(key.to_string())
                .and_modify(|current| *current = (*current).max(level))
                .or_insert(level);
        }
        if me.has_spawned {
            if let Some(previous) = self.previous_tiles.replace(me.tile_count) {
                self.tiles_gained = self
                    .tiles_gained
                    .saturating_add(u64::from(me.tile_count.saturating_sub(previous)));
            }
        }
        let map = &engine.state.map;
        if !me.alive || map.width == 0 {
            return;
        }
        for tile in me.border_tiles.ones() {
            let (x, y) = (tile % map.width, tile / map.width);
            if y >= map.height
                || map.owner_id(x, y) != my_id
                || !map.terrain[tile as usize].is_land()
            {
                continue;
            }
            // Match the simulation's land adjacency, including diagonal neighbors.
            map.for_each_neighbor(x, y, |nx, ny| {
                let owner = map.owner_id(nx, ny);
                if owner != 0
                    && owner != my_id
                    && map.terrain[map.ref_id(nx, ny)].is_land()
                    && let Some(other) = engine.state.player(owner)
                    && other.alive
                    && self.seen_contacts.insert(owner)
                {
                    if let Some(faction_id) = engine.campaign_faction_ids.get(&other.id) {
                        self.seen_contact_faction_ids.insert(faction_id.clone());
                    }
                }
            });
        }
    }

    pub(crate) fn observe_events(&mut self, engine: &SowEngine, my_id: u16) {
        for event in &engine.state.events {
            if matches!(event, GameEvent::AllianceRequestLifecycle { proposer_id, status: sow_core::game::AllianceRequestStatus::Submitted, .. } if *proposer_id == my_id) {
                self.alliance_requests_submitted = self.alliance_requests_submitted.saturating_add(1);
            }
            if let GameEvent::ResourceRequestSubmitted {
                proposer_id,
                target_id,
                is_trade: false,
            } = event
                && *proposer_id == my_id
            {
                self.resource_requests = self.resource_requests.saturating_add(1);
                if let Some(target) = engine.state.player(*target_id)
                    && let Some(faction_id) = engine.campaign_faction_ids.get(&target.id)
                {
                    let count = self
                        .resource_requests_by_target_faction_id
                        .entry(faction_id.clone())
                        .or_insert(0);
                    *count = count.saturating_add(1);
                }
            }
            if let GameEvent::ResourceTransferred {
                sender_id,
                receiver_id,
                gold,
                troops,
            } = event
            {
                if *sender_id == my_id {
                    self.resource_transfers = self.resource_transfers.saturating_add(1);
                    if let Some(receiver) = engine.state.player(*receiver_id) {
                        if let Some(faction_id) = engine.campaign_faction_ids.get(&receiver.id) {
                            let counts = self
                                .resource_transfers_by_recipient_faction_id
                                .entry(faction_id.clone())
                                .or_default();
                            counts.total = counts.total.saturating_add(1);
                            if *gold > 0.0 {
                                counts.gold = counts.gold.saturating_add(1);
                            }
                            if *troops > 0.0 {
                                counts.troops = counts.troops.saturating_add(1);
                            }
                            if *gold > 0.0 && *troops > 0.0 {
                                counts.gold_troops = counts.gold_troops.saturating_add(1);
                            }
                        }
                    }
                }
                if *receiver_id == my_id
                    && let (Some(sender), Some(receiver)) = (
                        engine.state.player(*sender_id),
                        engine.state.player(*receiver_id),
                    )
                    && ((sender.team.is_some() && sender.team == receiver.team)
                        || sender.alliances.contains(receiver_id))
                {
                    self.ally_support_deliveries = self.ally_support_deliveries.saturating_add(1);
                    if let Some(faction_id) = engine.campaign_faction_ids.get(&sender.id) {
                        let receipt = self
                            .support_deliveries_by_faction_id
                            .entry(faction_id.clone())
                            .or_default();
                        if receipt.deliveries == 0 {
                            receipt.first_tick = engine.state.tick;
                        }
                        receipt.deliveries = receipt.deliveries.saturating_add(1);
                        receipt.gold += (*gold).max(0.0);
                        receipt.troops += (*troops).max(0.0);
                    }
                }
            }
            // Construction runs before combat. Keep a completion even if the building
            // is destroyed or captured later in this tick, using its pre-tick owner.
            if let GameEvent::StructureReady { id, .. } = event
                && self.owned_structures.contains(id)
            {
                self.seen_structures.insert(*id);
                if let Some(kind) = self.owned_structure_kinds.get(id).copied() {
                    let key = match kind {
                        sow_core::game::BuildingKind::City => "cities",
                        sow_core::game::BuildingKind::Tower => "towers",
                        sow_core::game::BuildingKind::Factory => "factories",
                        sow_core::game::BuildingKind::Port => "ports",
                        sow_core::game::BuildingKind::Farm => "farms",
                    };
                    self.seen_buildings_by_kind
                        .entry(key.to_string())
                        .or_default()
                        .insert(*id);
                }
            }
            if let GameEvent::StructureUpgraded { id, kind, .. } = event
                && self.owned_structures.contains(id)
            {
                self.structure_upgrades = self.structure_upgrades.saturating_add(1);
                match kind {
                    sow_core::game::BuildingKind::City => {
                        self.city_upgrades = self.city_upgrades.saturating_add(1);
                    }
                    sow_core::game::BuildingKind::Port => {
                        self.port_upgrades = self.port_upgrades.saturating_add(1);
                    }
                    _ => {}
                }
            }
            if let GameEvent::PlayerEliminated {
                player_id,
                conqueror_id,
                assists,
                ..
            } = event
                && let Some(player) = engine.state.player(*player_id)
            {
                if player.id == my_id {
                    self.eliminated_faction_ids.insert("player".to_string());
                } else if let Some(faction_id) = engine.campaign_faction_ids.get(&player.id) {
                    self.eliminated_faction_ids.insert(faction_id.clone());
                }
                if *conqueror_id == my_id
                    || assists.iter().any(|(player_id, _)| *player_id == my_id)
                {
                    self.seen_defeated.insert(*player_id);
                    if let Some(faction_id) = engine.campaign_faction_ids.get(&player.id) {
                        self.seen_defeated_faction_ids.insert(faction_id.clone());
                    }
                }
            }
        }
    }
}

impl SowApp {
    pub(crate) fn apply_paused_tutorial_intent(
        &mut self,
        intent: sow_core::protocol::GameplayIntent,
    ) {
        self.apply_paused_tutorial_intent_as(self.sim.my_player_id.unwrap_or(0), intent);
    }

    pub(crate) fn apply_paused_tutorial_intent_as(
        &mut self,
        actor_id: u16,
        intent: sow_core::protocol::GameplayIntent,
    ) {
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let observe_tutorial = self.net.is_offline
            && self.ui.tutorial_active
            && self.sim.config.tutorial
            && my_id != 0;
        let pending = self.ui.app.hud_state.building_placement;
        let (mut snap, events, placement_result, troops_spent) = {
            let Some(engine) = self.sim.engine.as_mut() else {
                return;
            };
            let neutral_attack_troops_before = match &intent {
                sow_core::protocol::GameplayIntent::Attack(attack) if attack.target_owner == 0 => {
                    Some(
                        engine
                            .attacks
                            .iter()
                            .filter(|execution| {
                                execution.owner_id == my_id && execution.target_owner == 0
                            })
                            .map(|execution| execution.troops)
                            .sum::<f64>(),
                    )
                }
                _ => None,
            };
            let intents = [sow_core::protocol::StampedIntent {
                player_id: actor_id,
                intent,
            }];
            let troops_spent =
                engine.apply_intents_tracking_troop_spend(&intents, Some(my_id));
            let placement_result =
                building_confirmation_result(pending, my_id, &intents, &engine.state.events);
            if let Some(before) = neutral_attack_troops_before {
                let after = engine
                    .attacks
                    .iter()
                    .filter(|execution| execution.owner_id == my_id && execution.target_owner == 0)
                    .map(|execution| execution.troops)
                    .sum::<f64>();
                if after > before {
                    self.sim.tutorial_observation.wilderness_orders_accepted = self
                        .sim
                        .tutorial_observation
                        .wilderness_orders_accepted
                        .saturating_add(1);
                }
            }
            if observe_tutorial {
                self.sim.tutorial_observation.observe_events(engine, my_id);
                self.sim.tutorial_observation.observe_sim(engine, my_id);
            }
            let snap = engine.build_snapshot();
            let events = std::mem::take(&mut engine.state.events);
            (snap, events, placement_result, troops_spent)
        };
        self.ui.nameplates.animate_troop_change(
            my_id,
            -troops_spent,
            web_time::Instant::now(),
        );
        self.process_tick_events(events, &snap, my_id);
        self.apply_snapshot_fx(&mut snap, my_id);
        self.sim.current_snapshot = Some(snap);
        // Paused actions replace the snapshot without advancing its tick.
        self.ui.building_render_cache = Default::default();
        self.ui.railway_render_state.invalidate_snapshot();
        self.sim.last_synced_cost_tick = None;
        self.sync_hud_player_state();
        self.sync_building_costs();
        if let Some(accepted) = placement_result {
            self.finish_building_confirmation(accepted);
        }
    }

    pub(crate) fn handle_sim_turn(&mut self, turn: Turn) {
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let observe_tutorial = self.net.is_offline && self.sim.config.tutorial && my_id != 0;
        let pending = self.ui.app.hud_state.building_placement;
        let (mut snap, events, local_spawn_target, placement_result, troops_spent) = {
            let Some(e) = self.sim.engine.as_mut() else {
                return;
            };
            let local_spawn_targets =
                if matches!(e.state.phase, sow_core::game::GamePhase::Spawning { .. }) {
                    turn.intents
                        .iter()
                        .rev()
                        .filter_map(|stamped| {
                            if stamped.player_id != my_id {
                                return None;
                            }
                            let sow_core::protocol::GameplayIntent::Spawn { x, y } =
                                &stamped.intent
                            else {
                                return None;
                            };
                            let map = &e.state.map;
                            (*x < map.width
                                && *y < map.height
                                && map.owner_id(*x, *y) == 0
                                && map.terrain[map.ref_id(*x, *y)].is_land())
                            .then_some((*x, *y))
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
            let troops_spent =
                e.apply_intents_tracking_troop_spend(&turn.intents, Some(my_id));
            let placement_result =
                building_confirmation_result(pending, my_id, &turn.intents, &e.state.events);
            let local_spawn_target = local_spawn_targets
                .into_iter()
                .find(|(x, y)| e.state.map.owner_id(*x, *y) == my_id);
            if observe_tutorial {
                // Capture accepted actions before tick() clears their events.
                self.sim.tutorial_observation.observe_events(e, my_id);
                // Accepted launches can finish and disappear in their first tick.
                self.sim.tutorial_observation.observe_sim(e, my_id);
            }
            let mut lifecycle_events = take_alliance_request_lifecycle_events(&mut e.state.events);
            // Hero cards of launched attacks are announced while intents apply.
            let mut ability_events = take_ability_events(&mut e.state.events);
            let transfer_events = take_resource_transfer_events(&mut e.state.events);
            e.tick();
            self.sim.config.buildings_enabled = e.state.config.buildings_enabled;
            if observe_tutorial {
                self.sim.tutorial_observation.observe_events(e, my_id);
                self.sim.tutorial_observation.observe_sim(e, my_id);
            }
            e.state.events.extend(transfer_events);
            let snap = e.build_snapshot();
            let mut events = std::mem::take(&mut e.state.events);
            events.append(&mut lifecycle_events);
            events.append(&mut ability_events);
            (snap, events, local_spawn_target, placement_result, troops_spent)
        };

        self.ui.nameplates.animate_troop_change(
            my_id,
            -troops_spent,
            web_time::Instant::now(),
        );

        let turn_defeats = self.process_tick_events(events, &snap, my_id);

        self.progress_session_defeats.players = self
            .progress_session_defeats
            .players
            .saturating_add(turn_defeats.players);
        self.progress_session_defeats.empires = self
            .progress_session_defeats
            .empires
            .saturating_add(turn_defeats.empires);
        self.progress_session_defeats.tribes = self
            .progress_session_defeats
            .tribes
            .saturating_add(turn_defeats.tribes);

        self.apply_snapshot_fx(&mut snap, my_id);
        self.process_nuke_alerts(&snap);

        let my_team = snap
            .players
            .iter()
            .find(|p| p.id == my_id)
            .and_then(|p| p.team);
        self.maybe_record_match_progress(&snap, snap.winner, snap.winning_team, my_team);

        // Viewport Alerts and one-shot result sound: Victory / Defeat.
        let match_won = snap
            .winner
            .map(|winner| winner == my_id)
            .or_else(|| snap.winning_team.map(|team| Some(team) == my_team));
        if let Some(won) = match_won {
            self.sfx.play_result(won);
            self.ui.trigger_viewport_alert(if won {
                crate::app::ViewportAlertKind::Victory
            } else {
                crate::app::ViewportAlertKind::Defeat
            });
        }

        if let Some((x, y)) = local_spawn_target {
            self.sim.latest_local_spawn = Some((snap.tick, x, y));
        }
        self.sim.current_snapshot = Some(snap);
        if let Some(accepted) = placement_result {
            self.finish_building_confirmation(accepted);
        }

        // Recompute Fog of War visibility
        if let Some(ref snap_ref) = self.sim.current_snapshot {
            let owners = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners.as_slice())
                .unwrap_or(&[]);
            self.sim
                .fog_explored
                .blocks
                .resize((self.sim.map_w * self.sim.map_h + 63) as usize / 64, 0);
            self.sim
                .fog_visible
                .blocks
                .resize((self.sim.map_w * self.sim.map_h + 63) as usize / 64, 0);
            let dev = crate::theme::dev_config::DevConfig::get();
            let visibility_changed = crate::sim::visibility::compute_visibility(
                (self.sim.map_w, self.sim.map_h),
                my_id,
                owners,
                snap_ref,
                &mut self.sim.fog_explored,
                &mut self.sim.fog_visible,
                dev.fog_of_war,
            );
            if visibility_changed {
                self.sim.force_fog_upload = true;
            }
        }

        self.time.interp.stamp_applied(web_time::Instant::now());
    }
}

