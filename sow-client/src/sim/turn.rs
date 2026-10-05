use crate::app::{SowApp, TutorialObservation};
use sow_core::engine::SowEngine;
use sow_core::game::GameEvent;
use sow_core::protocol::Turn;

impl TutorialObservation {
    pub(crate) fn observe_sim(&mut self, engine: &SowEngine, my_id: u16) {
        if my_id == 0 {
            return;
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
                    sow_core::game::BuildingKind::Bunker => "bunkers",
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
            (sow_core::game::BuildingKind::Bunker, "bunker"),
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
        self.foundry_level = self.foundry_level.max(
            engine
                .buildings
                .iter()
                .filter(|building| {
                    building.owner_id == my_id
                        && building.kind == sow_core::game::BuildingKind::City
                        && !building.under_construction
                })
                .map(|building| u64::from(building.modules.foundry))
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
                        sow_core::game::BuildingKind::Bunker => "bunkers",
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
            if let GameEvent::TileUpgraded { tile_idx, .. } = event {
                let width = engine.state.map.width;
                if width > 0
                    && engine
                        .state
                        .map
                        .owner_id(tile_idx % width, tile_idx / width)
                        == my_id
                {
                    self.tile_upgrades = self.tile_upgrades.saturating_add(1);
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
                if let Some(faction_id) = engine.campaign_faction_ids.get(&player.id) {
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
    pub(crate) fn handle_sim_turn(&mut self, turn: Turn) {
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let observe_tutorial = self.net.is_offline && self.sim.config.tutorial && my_id != 0;
        let (mut snap, events) = {
            let Some(e) = self.sim.engine.as_mut() else {
                return;
            };
            e.apply_intents(&turn.intents);
            if observe_tutorial {
                // Capture accepted actions before tick() clears their events.
                self.sim.tutorial_observation.observe_events(e, my_id);
                // Accepted launches can finish and disappear in their first tick.
                self.sim.tutorial_observation.observe_sim(e, my_id);
            }
            e.tick();
            self.sim.config.buildings_enabled = e.state.config.buildings_enabled;
            if observe_tutorial {
                self.sim.tutorial_observation.observe_events(e, my_id);
                self.sim.tutorial_observation.observe_sim(e, my_id);
            }
            let snap = e.build_snapshot();
            let events: Vec<_> = std::mem::take(&mut e.state.events);
            (snap, events)
        };

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

        self.sim.current_snapshot = Some(snap);

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
            crate::sim::visibility::compute_visibility(
                (self.sim.map_w, self.sim.map_h),
                my_id,
                owners,
                snap_ref,
                &mut self.sim.fog_explored,
                &mut self.sim.fog_visible,
                dev.fog_of_war,
            );
            self.sim.force_fog_upload = true;
        }

        self.time.interp.stamp_applied(web_time::Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::building::{Building, CityModules};
    use sow_core::game::{BuildingKind, GamePhase, GameState, ProjectileKind, UnitType};
    use sow_core::game_config::GameConfig;
    use sow_core::map::MapTile;
    use sow_core::player::Player;
    use sow_core::protocol::{AttackIntent, GameplayIntent, StampedIntent};
    use sow_core::warp_fleet::WarpFleet;
    use sow_core::water_components::WaterComponents;

    fn engine() -> SowEngine {
        let config = GameConfig {
            tutorial: true,
            ..Default::default()
        };
        let mut state = GameState::new(42, 8, 3, config.clone());
        state.phase = GamePhase::Playing;
        for (id, name, x) in [(1, "Player", 1), (2, "Neighbor", 2), (3, "Distant", 6)] {
            let mut player = Player::new_human(id, name.into(), [1.0; 3], &config);
            player.has_spawned = true;
            player.troops = 100_000.0;
            player.gold = 1_000_000_000.0;
            state.register_player(player);
            state.set_tile_owner(x, 1, id);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine
            .campaign_faction_ids
            .extend([(2, "neighbor".to_string()), (3, "distant".to_string())]);
        engine
    }

    #[test]
    fn observations_count_owned_entities_once_and_keep_completions_after_destruction() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        engine.apply_intents(&[
            StampedIntent {
                player_id: 1,
                intent: GameplayIntent::Attack(AttackIntent {
                    target_owner: 2,
                    troops: Some(100.0),
                }),
            },
            StampedIntent {
                player_id: 2,
                intent: GameplayIntent::Attack(AttackIntent {
                    target_owner: 0,
                    troops: Some(100.0),
                }),
            },
        ]);
        for owner_id in [1, 2] {
            engine.add_fleet(WarpFleet::new(
                u64::from(owner_id),
                owner_id,
                0,
                UnitType::TransportShip,
                100.0,
                (9, 10),
                vec![9, 10],
            ));
            for under_construction in [false, true] {
                engine.add_building(Building {
                    id: u64::from(owner_id) * 10 + u64::from(under_construction),
                    owner_id,
                    tile_idx: 8 + u32::from(owner_id),
                    kind: BuildingKind::City,
                    level: 1,
                    under_construction,
                    ticks_until_complete: u32::from(under_construction),
                    modules: CityModules::default(),
                });
            }
            engine.apply_launch_nuke_intent(owner_id, 9);
        }
        // A second launch at the same tile is a distinct nuke; SAMs are not nukes.
        engine.silo_cooldowns.clear();
        engine.apply_launch_nuke_intent(1, 9);
        let mut sam = engine.projectiles[0].clone();
        sam.id = 100;
        sam.kind = ProjectileKind::SAMMissile;
        engine.projectiles.push(sam);

        observation.observe_sim(&engine, 0);
        assert!(observation.seen_attacks.is_empty());
        observation.observe_sim(&engine, 1);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.seen_attacks.len(), 1);
        assert_eq!(observation.attacks_by_faction_id.get("neighbor"), Some(&1));
        assert_eq!(observation.seen_fleets, [1].into_iter().collect());
        assert_eq!(observation.seen_nukes.len(), 2);
        assert_eq!(observation.seen_structures, [10, 11].into_iter().collect());
        assert_eq!(observation.seen_cities, [10, 11].into_iter().collect());

        engine.execute_construction();
        engine.buildings.clear();
        observation.observe_events(&engine, 1);
        engine.attacks.clear();
        engine.fleets.clear();
        engine.projectiles.clear();
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.seen_structures, [10, 11].into_iter().collect());
        assert_eq!(observation.seen_attacks.len(), 1);
        assert_eq!(observation.seen_fleets.len(), 1);
        assert_eq!(observation.seen_nukes.len(), 2);
    }

    #[test]
    fn campaign_action_events_survive_the_tick_event_clear() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        for (id, tile_idx, kind) in [(20, 9, BuildingKind::City), (21, 10, BuildingKind::Port)] {
            engine.add_building(Building {
                id,
                owner_id: 1,
                tile_idx,
                kind,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
                modules: CityModules::default(),
            });
        }
        observation.observe_sim(&engine, 1);
        engine.state.events.extend([
            GameEvent::StructureUpgraded {
                id: 20,
                tile_idx: 9,
                kind: BuildingKind::City,
                level: 2,
            },
            GameEvent::StructureUpgraded {
                id: 21,
                tile_idx: 10,
                kind: BuildingKind::Port,
                level: 2,
            },
            GameEvent::TileUpgraded {
                tile_idx: 9,
                level: 1,
            },
            GameEvent::ResourceTransferred {
                sender_id: 1,
                receiver_id: 2,
                gold: 10.0,
                troops: 10.0,
            },
        ]);

        observation.observe_events(&engine, 1);
        engine.tick(); // This clears intent events at the start of every simulation tick.
        assert_eq!(observation.structure_upgrades, 2);
        assert_eq!(observation.city_upgrades, 1);
        assert_eq!(observation.port_upgrades, 1);
        assert_eq!(observation.tile_upgrades, 1);
        assert_eq!(observation.resource_transfers, 1);
    }

    #[test]
    fn campaign_alliance_metric_ignores_starting_allies() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        engine.state.player_mut(1).unwrap().alliances.push(2);

        observation.observe_sim(&engine, 1);
        assert_eq!(observation.alliances_formed, 0);
        assert!(observation.seen_alliance_faction_ids.contains("neighbor"));

        engine.state.player_mut(1).unwrap().alliances.push(3);
        observation.observe_sim(&engine, 1);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.alliances_formed, 1);
        assert!(observation.seen_alliance_faction_ids.contains("distant"));
    }

    #[test]
    fn campaign_structure_level_facts_use_highest_building_not_sum() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        for (id, kind) in [
            (30, BuildingKind::City),
            (31, BuildingKind::Port),
            (32, BuildingKind::Port),
        ] {
            engine.add_building(Building {
                id,
                owner_id: 1,
                tile_idx: 9,
                kind,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
                modules: CityModules::default(),
            });
        }

        observation.observe_sim(&engine, 1);
        assert_eq!(observation.city_levels, 1);
        assert_eq!(observation.port_levels, 1);

        engine
            .buildings
            .iter_mut()
            .find(|building| building.id == 30)
            .unwrap()
            .level = 3;
        engine
            .buildings
            .iter_mut()
            .find(|building| building.id == 31)
            .unwrap()
            .level = 2;
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.city_levels, 3);
        assert_eq!(observation.port_levels, 2);
    }

    #[test]
    fn campaign_fleet_facts_distinguish_unit_and_transport_destination_once() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        for (id, kind, target) in [
            (40, UnitType::TransportShip, 2),
            (41, UnitType::TradeShip, 2),
            (42, UnitType::Warship, 3),
        ] {
            engine.add_fleet(WarpFleet::new(
                id,
                1,
                target,
                kind,
                100.0,
                (9, 10),
                vec![9, 10],
            ));
        }

        observation.observe_sim(&engine, 1);
        observation.observe_sim(&engine, 1);
        assert_eq!(
            observation.seen_fleets_by_type.get("TransportShip"),
            Some(&1)
        );
        assert_eq!(observation.seen_fleets_by_type.get("TradeShip"), Some(&1));
        assert_eq!(observation.seen_fleets_by_type.get("Warship"), Some(&1));
        assert_eq!(
            observation
                .seen_transport_fleets_by_faction_id
                .get("neighbor"),
            Some(&1)
        );
        assert_eq!(observation.seen_transport_fleets_by_faction_id.len(), 1);
    }

    #[test]
    fn campaign_foundry_fact_tracks_its_own_highest_completed_module_level() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        for (id, level, under_construction) in [(50, 2, false), (51, 3, true)] {
            let mut modules = CityModules::default();
            modules.foundry = level;
            engine.add_building(Building {
                id,
                owner_id: 1,
                tile_idx: 9,
                kind: BuildingKind::City,
                level: 1,
                under_construction,
                ticks_until_complete: u32::from(under_construction),
                modules,
            });
        }

        observation.observe_sim(&engine, 1);
        assert_eq!(observation.foundry_level, 2);
        engine.buildings[0].modules.foundry = 1;
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.foundry_level, 2);
    }

    #[test]
    fn campaign_support_receipts_are_attributed_to_the_allied_sender_and_resources() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine.state.events.extend([
            GameEvent::ResourceTransferred {
                sender_id: 2,
                receiver_id: 1,
                gold: 10.0,
                troops: 5.0,
            },
            GameEvent::ResourceTransferred {
                sender_id: 2,
                receiver_id: 1,
                gold: 10.0,
                troops: 0.0,
            },
            GameEvent::ResourceTransferred {
                sender_id: 3,
                receiver_id: 1,
                gold: 100.0,
                troops: 100.0,
            },
        ]);

        observation.observe_events(&engine, 1);
        let receipt = observation
            .support_deliveries_by_faction_id
            .get("neighbor")
            .unwrap();
        assert_eq!(receipt.deliveries, 2);
        assert_eq!(receipt.gold, 20.0);
        assert_eq!(receipt.troops, 5.0);
        assert_eq!(observation.ally_support_deliveries, 2);
        assert_eq!(observation.resource_transfers, 0);
    }

    #[test]
    fn rejected_commands_do_not_count_and_first_tick_nukes_are_retained() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        engine.apply_intents(&[
            StampedIntent {
                player_id: 1,
                intent: GameplayIntent::Attack(AttackIntent {
                    target_owner: 1,
                    troops: Some(100.0),
                }),
            },
            StampedIntent {
                player_id: 1,
                intent: GameplayIntent::LaunchFleet {
                    target_tile: u32::MAX,
                    troops: Some(100.0),
                },
            },
        ]);
        engine.apply_launch_nuke_intent(1, 9); // No ready city.
        observation.observe_sim(&engine, 1);
        assert!(observation.seen_attacks.is_empty());
        assert!(observation.seen_fleets.is_empty());
        assert!(observation.seen_nukes.is_empty());

        engine.add_building(Building {
            id: 1,
            owner_id: 1,
            tile_idx: 9,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: CityModules::default(),
        });
        engine.apply_launch_nuke_intent(1, 9);
        observation.observe_sim(&engine, 1);
        engine.tick();
        assert!(engine.projectiles.is_empty());
        observation.observe_events(&engine, 1);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.seen_nukes.len(), 1);
    }

    #[test]
    fn contacts_require_owned_land_neighbors_and_defeats_require_player_contribution() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        // A stale border entry near Distant must not create contact.
        engine.state.player_mut(1).unwrap().border_insert(13);
        engine.state.map.terrain[10] = MapTile::from_byte(0);
        observation.observe_sim(&engine, 1);
        assert!(observation.seen_contacts.is_empty());
        engine.state.map.terrain[10] = MapTile::from_byte(0x80);
        engine.state.map.terrain[9] = MapTile::from_byte(0);
        observation.observe_sim(&engine, 1);
        assert!(observation.seen_contacts.is_empty());
        engine.state.map.terrain[9] = MapTile::from_byte(0x80);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.seen_contacts, [2].into_iter().collect());
        assert!(observation.seen_contact_faction_ids.contains("neighbor"));

        // Diagonal adjacency is also a land border in the simulation.
        engine.state.set_tile_owner(6, 1, 0);
        engine.state.set_tile_owner(2, 2, 3);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.seen_contacts, [2, 3].into_iter().collect());
        assert!(observation.seen_contact_faction_ids.contains("distant"));

        engine.state.player_mut(2).unwrap().alive = false;
        observation.observe_sim(&engine, 1);
        assert!(!observation.seen_defeated_faction_ids.contains("neighbor"));
        engine.state.events.push(GameEvent::PlayerEliminated {
            player_id: 2,
            conqueror_id: 3,
            gold_bounty: 0,
            elimination_x: 2,
            elimination_y: 1,
            assists: vec![],
            by_nuke: false,
        });
        observation.observe_events(&engine, 1);
        assert!(!observation.seen_defeated_faction_ids.contains("neighbor"));
        if let Some(GameEvent::PlayerEliminated { assists, .. }) = engine.state.events.last_mut() {
            assists.push((1, 1));
        }
        observation.observe_events(&engine, 1);
        assert!(observation.seen_defeated_faction_ids.contains("neighbor"));
        assert_eq!(engine.state.player(1).unwrap().kills, 0);
        assert!(observation.seen_contact_faction_ids.contains("neighbor"));
    }

    #[test]
    fn territory_gains_exclude_spawn_never_decrease_and_reset_between_matches() {
        let mut engine = engine();
        let mut observation = TutorialObservation::default();
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.tiles_gained, 0);
        engine.state.set_tile_owner(0, 1, 1);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.tiles_gained, 1);
        engine.state.set_tile_owner(0, 1, 0);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.tiles_gained, 1);
        engine.state.set_tile_owner(0, 1, 1);
        observation.observe_sim(&engine, 1);
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.tiles_gained, 2);

        observation.reset();
        observation.observe_sim(&engine, 1);
        assert_eq!(observation.tiles_gained, 0);
        assert!(observation.seen_attacks.is_empty());
        assert!(observation.seen_nukes.is_empty());
    }
}
