use crate::engine::SowEngine;
use crate::game::{BuildingKind, GameEvent, GamePhase, UnitType};
use crate::geo_entities::{Ability, District, Source};
use crate::player::Leader;

fn port_ship_spawn_tile(map: &crate::map::GameMap, port_tile: u32) -> Option<u32> {
    if map.width == 0 || port_tile as usize >= map.terrain.len() {
        return None;
    }

    let anchor_x = port_tile % map.width;
    let anchor_y = port_tile / map.width;
    let footprint = crate::building::BuildingFootprint::at(BuildingKind::Port, anchor_x, anchor_y);
    let mut candidates = Vec::new();

    for y in footprint.top..footprint.top + footprint.height as i32 {
        for x in footprint.left..footprint.left + footprint.width as i32 {
            if !map.is_valid_coord(x, y) {
                continue;
            }
            map.for_each_neighbor(x as u32, y as u32, |nx, ny| {
                if footprint.contains(nx as i32, ny as i32)
                    || !map.terrain[map.ref_id(nx, ny)].is_water()
                {
                    return;
                }
                let dx = nx.abs_diff(anchor_x);
                let dy = ny.abs_diff(anchor_y);
                candidates.push((dx + dy, ny * map.width + nx));
            });
        }
    }

    candidates.into_iter().min().map(|(_, tile)| tile)
}

/// Advance construction timers; emits [`GameEvent::StructureReady`].
/// Split out for unit tests (`advance_building_construction_tick`).
impl SowEngine {
    pub fn execute_construction(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // buildings are maintained sorted by id on insertion.

        for b in &mut self.buildings {
            if !b.under_construction {
                continue;
            }
            if b.ticks_until_complete > 0 {
                b.ticks_until_complete -= 1;
            }
            if b.ticks_until_complete == 0 {
                b.under_construction = false;
                self.building_aggregates_dirty = true;
                if b.kind == BuildingKind::Port {
                    self.sea_lanes_dirty = true;
                }
                if b.kind == BuildingKind::Tower {
                    self.defense_grid_dirty = true;
                    self.render_defense_dirty = true;
                }
                if self
                    .state
                    .player(b.owner_id)
                    .is_some_and(|player| structure_marks_leader(player.leader, b.kind))
                {
                    self.state.announce_leader_ability(b.owner_id, None);
                }
                self.state.events.push(GameEvent::StructureReady {
                    id: b.id,
                    tile_idx: b.tile_idx,
                    kind: b.kind,
                });
            }
        }
    }

    pub fn execute_ship_production(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        if !self.campaign_allows_action(crate::campaign::CampaignAction::Warship) {
            return;
        }

        let mut new_fleets = Vec::new();

        for (port_id, queue) in self.port_queues.iter_mut() {
            let Some(prod) = queue.front_mut() else {
                continue;
            };
            prod.ticks_until_complete = prod.ticks_until_complete.saturating_sub(1);
            if prod.ticks_until_complete > 0 {
                continue;
            }

            let Some(port) = self
                .buildings
                .iter()
                .find(|building| building.id == *port_id)
            else {
                queue.pop_front();
                continue;
            };
            let owner_id = port.owner_id;
            let Some(spawn_tile) = port_ship_spawn_tile(&self.state.map, port.tile_idx) else {
                // A legal port always touches water; leave the order queued if a
                // malformed map or legacy save temporarily violates that rule.
                continue;
            };
            let prod = queue.pop_front().expect("completed ship production exists");
            let fid = self.state.next_fleet_id;
            self.state.next_fleet_id = self.state.next_fleet_id.wrapping_add(1).max(1);
            let mut fleet = crate::warp_fleet::WarpFleet::new(
                fid,
                owner_id,
                0,
                prod.kind,
                prod.kind.max_health(), // Treat troops as health for ships
                (spawn_tile, spawn_tile),
                vec![],
            );
            fleet.set_speed_bonus_percent(crate::building::player_boat_speed_bonus(
                &self.buildings,
                owner_id,
            ));
            new_fleets.push(fleet);
        }

        for f in new_fleets {
            self.add_fleet(f);
        }
    }

    pub fn execute_trade_ships(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        if !self.campaign_allows_action(crate::campaign::CampaignAction::TradeShip) {
            return;
        }

        let ports = ready_trade_ports(&self.buildings);
        let sea_lanes = self.state.sea_lanes.clone();
        let map_width = self.state.map.width.max(1);
        let capacities: std::collections::HashMap<_, _> = self
            .state
            .players
            .iter()
            .map(|player| {
                (
                    player.id,
                    crate::building::player_trade_ship_capacity(&self.buildings, player.id),
                )
            })
            .collect();
        let port_levels: std::collections::HashMap<_, _> = self
            .state
            .players
            .iter()
            .map(|player| {
                (
                    player.id,
                    crate::building::player_port_levels(&self.buildings, player.id),
                )
            })
            .collect();
        let mut route_cache = std::collections::HashMap::new();
        let mut retained = std::collections::HashMap::<u16, u32>::new();
        let mut to_remove = Vec::new();

        for (index, fleet) in self.fleets.iter_mut().enumerate() {
            if fleet.unit_type != UnitType::TradeShip || fleet.troops <= 0.0 {
                continue;
            }

            let capacity = capacities.get(&fleet.owner_id).copied().unwrap_or_default();
            let count = retained.entry(fleet.owner_id).or_default();
            if *count >= capacity {
                to_remove.push(index);
                continue;
            }

            let route_is_current = ports
                .iter()
                .any(|port| port.owner_id == fleet.owner_id && port.tile_idx == fleet.src_tile)
                && ports
                    .iter()
                    .any(|port| port.tile_idx == fleet.dst_tile && port.owner_id != fleet.owner_id);
            let arrived = (fleet.path.is_empty() && fleet.path_cursor > 0)
                || (!fleet.path.is_empty() && fleet.path_cursor >= fleet.path.len());
            if fleet.path.is_empty() || arrived || !route_is_current {
                let Some(route) = find_trade_route(
                    &sea_lanes,
                    map_width,
                    &ports,
                    fleet.owner_id,
                    Some(fleet.current_tile),
                    arrived.then_some(fleet.dst_tile),
                    &mut route_cache,
                ) else {
                    to_remove.push(index);
                    continue;
                };
                fleet.src_tile = route.src_tile;
                fleet.dst_tile = route.dst_tile;
                fleet.replace_path(route.path);
            }
            *count += 1;
        }

        for index in to_remove.into_iter().rev() {
            self.fleets.swap_remove(index);
        }
        if !retained.is_empty() {
            self.fleets.sort_unstable_by_key(|fleet| fleet.id);
        }

        let mut new_fleets = Vec::new();
        let player_ids: Vec<_> = self
            .state
            .players
            .iter()
            .filter(|player| player.alive)
            .map(|player| player.id)
            .collect();
        for player_id in player_ids {
            let capacity = capacities.get(&player_id).copied().unwrap_or_default();
            if capacity == 0 {
                continue;
            }
            let count = retained.entry(player_id).or_default();
            while *count < capacity {
                let Some(route) = find_trade_route(
                    &sea_lanes,
                    map_width,
                    &ports,
                    player_id,
                    None,
                    None,
                    &mut route_cache,
                ) else {
                    break;
                };
                let id = self.state.next_fleet_id;
                self.state.next_fleet_id = self.state.next_fleet_id.wrapping_add(1).max(1);
                let mut fleet = crate::warp_fleet::WarpFleet::new(
                    id,
                    player_id,
                    route.dst_owner,
                    UnitType::TradeShip,
                    UnitType::TradeShip.max_health(),
                    (route.src_tile, route.dst_tile),
                    route.path,
                );
                let port_levels = port_levels.get(&player_id).copied().unwrap_or_default();
                fleet.set_speed_bonus_percent(
                    crate::building::cost::boat_speed_bonus_from_port_levels(port_levels),
                );
                if self
                    .state
                    .player(player_id)
                    .is_some_and(|player| player.leader.has_resource_bonus_from(Source::Trade))
                {
                    self.state.announce_leader_ability(player_id, None);
                }
                new_fleets.push(fleet);
                *count += 1;
            }
        }
        for fleet in new_fleets {
            self.add_fleet(fleet);
        }
    }
}

#[derive(Clone, Copy)]
struct TradePort {
    id: u64,
    owner_id: u16,
    tile_idx: u32,
}

struct TradeRoute {
    src_tile: u32,
    dst_tile: u32,
    dst_owner: u16,
    path: Vec<u32>,
}

/// Whether finishing a `kind` building pays `leader` its ability: a district's income,
/// a city's troop cap or a tower's reach.
fn structure_marks_leader(leader: Leader, kind: BuildingKind) -> bool {
    match (leader.ability(), kind) {
        (Ability::TowerRange { .. }, BuildingKind::Tower) => true,
        (Ability::CityTroopCapacity { .. }, BuildingKind::City) => true,
        (
            Ability::ResourceBonus {
                source: Source::District(district),
                ..
            },
            _,
        ) => matches!(
            (district, kind),
            (District::City, BuildingKind::City)
                | (District::Factory, BuildingKind::Factory)
                | (District::Port, BuildingKind::Port)
                | (District::Farm, BuildingKind::Farm)
        ),
        _ => false,
    }
}

fn ready_trade_ports(buildings: &[crate::building::Building]) -> Vec<TradePort> {
    buildings
        .iter()
        .filter(|building| {
            building.kind == BuildingKind::Port
                && !building.under_construction
                && building.active_level() >= 1
        })
        .map(|building| TradePort {
            id: building.id,
            owner_id: building.owner_id,
            tile_idx: building.tile_idx,
        })
        .collect()
}

fn find_trade_route(
    sea_lanes: &[crate::sea_lane::SeaLane],
    map_width: u32,
    ports: &[TradePort],
    owner_id: u16,
    preferred_tile: Option<u32>,
    avoid_destination_tile: Option<u32>,
    route_cache: &mut std::collections::HashMap<(u64, u64), Option<Vec<u32>>>,
) -> Option<TradeRoute> {
    let mut sources: Vec<_> = ports
        .iter()
        .copied()
        .filter(|port| port.owner_id == owner_id)
        .collect();
    if let Some(tile) = preferred_tile {
        let width = map_width.max(1);
        sources.sort_unstable_by_key(|port| {
            let ax = port.tile_idx % width;
            let ay = port.tile_idx / width;
            let bx = tile % width;
            let by = tile / width;
            ax.abs_diff(bx) + ay.abs_diff(by)
        });
    }

    let mut best: Option<(usize, TradeRoute)> = None;
    for source in sources {
        for destination in ports
            .iter()
            .copied()
            .filter(|port| port.owner_id != owner_id)
            .filter(|port| Some(port.tile_idx) != avoid_destination_tile)
        {
            let path = route_cache
                .entry((source.id, destination.id))
                .or_insert_with(|| {
                    crate::sea_lane::route_through_lanes(sea_lanes, source.id, destination.id)
                })
                .clone();
            let Some(path) = path else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            let distance = path.len();
            if best
                .as_ref()
                .is_some_and(|(best_distance, _)| *best_distance <= distance)
            {
                continue;
            }
            best = Some((
                distance,
                TradeRoute {
                    src_tile: source.tile_idx,
                    dst_tile: destination.tile_idx,
                    dst_owner: destination.owner_id,
                    path,
                },
            ));
        }
    }
    best.map(|(_, route)| route).or_else(|| {
        avoid_destination_tile.and_then(|_| {
            find_trade_route(
                sea_lanes,
                map_width,
                ports,
                owner_id,
                preferred_tile,
                None,
                route_cache,
            )
        })
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::building::Building;
    use crate::game::{GameEvent, GamePhase, GameState, UnitType};
    use crate::game_config::GameConfig;
    use crate::player::{Leader, Player};
    use crate::sea_lane::SeaLane;
    use crate::water_components::WaterComponents;

    pub(crate) fn port(id: u64, owner_id: u16, tile_idx: u32) -> Building {
        Building {
            id,
            owner_id,
            tile_idx,
            kind: BuildingKind::Port,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        }
    }

    pub(crate) fn engine_with_ports(ports: &[Building], lanes: Vec<SeaLane>) -> SowEngine {
        let mut state = GameState::new(1, 64, 64, GameConfig::default());
        state.phase = GamePhase::Playing;
        let config = state.config.clone();
        state.register_player(Player::new_human(1, "One".into(), [1.0, 0.0, 0.0], &config));
        state.register_player(Player::new_human(2, "Two".into(), [0.0, 1.0, 0.0], &config));
        state.player_mut(2).unwrap().alive = false;
        state.sea_lanes = std::sync::Arc::new(lanes);
        let mut engine = SowEngine::new(state, WaterComponents::default());
        for building in ports {
            engine.add_building(*building);
        }
        engine.sea_lanes_dirty = false;
        engine
    }

    pub(crate) fn lane(id: u64, a: u64, b: u64, path: &[u32]) -> SeaLane {
        SeaLane {
            id,
            port_a_id: a,
            port_b_id: b,
            path: path.to_vec(),
        }
    }

    #[test]
    fn warship_spawns_on_water_outside_the_complete_port_footprint() {
        let width = 64;
        let anchor_x = 20;
        let anchor_y = 20;
        let port_tile = anchor_y * width + anchor_x;
        let water_tile = anchor_y * width + (anchor_x - 2);
        let mut engine = engine_with_ports(&[port(1, 1, port_tile)], vec![]);
        engine
            .state
            .map
            .terrain
            .fill(crate::map::MapTile::from_byte(0x80));
        engine.state.map.terrain[water_tile as usize] = crate::map::MapTile::from_byte(0x20);
        engine.port_queues.insert(
            1,
            std::collections::VecDeque::from([crate::game::ShipProduction {
                kind: UnitType::Warship,
                ticks_until_complete: 1,
            }]),
        );

        engine.execute_ship_production();

        assert_eq!(engine.fleets.len(), 1);
        assert_eq!(engine.fleets[0].current_tile, water_tile);
        assert_eq!(engine.fleets[0].unit_type, UnitType::Warship);
        assert!(engine.state.map.terrain[engine.fleets[0].current_tile as usize].is_water());
    }

    #[test]
    fn trade_ship_departure_shows_the_mark_only_for_trade_heroes() {
        for (hero, marks) in [("solomon", 1), ("hongi_hika", 1), ("cleopatra", 0)] {
            let mut engine = engine_with_ports(
                &[port(1, 1, 10), port(2, 2, 20)],
                vec![lane(1, 1, 2, &[100, 101])],
            );
            engine.state.player_mut(1).unwrap().leader = Leader::of(hero);
            engine.execute_trade_ships();
            let shown = engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::AbilityTriggered { owner_id: 1, .. }))
                .count();
            assert_eq!(shown, marks, "{hero} marks its trade ship departure");
        }
    }

    #[test]
    fn finished_structures_show_the_mark_only_for_their_hero() {
        for (hero, kind, marks) in [
            ("nezahualcoyotl", BuildingKind::Factory, 1),
            ("nezahualcoyotl", BuildingKind::Port, 0),
            ("cleopatra", BuildingKind::Port, 1),
            ("cleopatra", BuildingKind::Factory, 0),
            ("lobengula", BuildingKind::City, 1),
            ("lobengula", BuildingKind::Tower, 0),
        ] {
            let mut engine = engine_with_ports(&[], vec![]);
            engine.state.player_mut(1).unwrap().leader = Leader::of(hero);
            engine.add_building(Building {
                id: 1,
                owner_id: 1,
                tile_idx: 10,
                kind,
                level: 1,
                under_construction: true,
                ticks_until_complete: 1,
            });
            engine.execute_construction();
            let shown = engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::AbilityTriggered { owner_id: 1, .. }))
                .count();
            assert_eq!(shown, marks, "{hero} when a {kind:?} is finished");
        }
    }

    #[test]
    fn passive_trade_ships_spawn_without_manual_purchase_and_choose_another_route() {
        let mut engine = engine_with_ports(
            &[port(1, 1, 10), port(2, 2, 20), port(3, 2, 30)],
            vec![lane(1, 1, 2, &[100, 101]), lane(2, 1, 3, &[110, 111, 112])],
        );

        engine.execute_trade_ships();
        assert_eq!(engine.fleets.len(), 1);
        assert_eq!(engine.fleets[0].unit_type, UnitType::TradeShip);
        assert_eq!(engine.fleets[0].dst_tile, 20);

        engine.fleets[0].path_cursor = engine.fleets[0].path.len();
        engine.execute_fleets();
        assert!(engine.fleets[0].path.is_empty());
        engine.execute_trade_ships();
        assert_eq!(engine.fleets.len(), 1);
        assert_eq!(engine.fleets[0].dst_tile, 30);
    }

    #[test]
    fn passive_trade_ships_rehome_after_capture_and_disappear_when_no_route_remains() {
        let mut engine = engine_with_ports(
            &[port(1, 1, 10), port(2, 2, 20), port(3, 2, 30)],
            vec![lane(1, 1, 2, &[100, 101]), lane(2, 1, 3, &[110, 111])],
        );
        engine.execute_trade_ships();
        assert_eq!(engine.fleets.len(), 1);

        engine
            .buildings
            .iter_mut()
            .find(|building| building.id == 2)
            .unwrap()
            .owner_id = 1;
        engine.execute_trade_ships();
        assert_eq!(engine.fleets[0].dst_tile, 30);

        engine.buildings.retain(|building| building.id != 3);
        engine.execute_trade_ships();
        assert!(engine.fleets.is_empty());
    }
}
