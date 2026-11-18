use crate::engine::SowEngine;
use crate::game::{BuildingKind, GameEvent, GamePhase, UnitType};
use crate::geo_entities::{Ability, District, Source};
use crate::player::Leader;

impl SowEngine {
    fn credit_city_upgrade_reinforcements(&mut self, building: crate::building::Building) {
        let Some(player) = self.state.player(building.owner_id) else {
            return;
        };
        if !player.alive || player.tile_count == 0 {
            return;
        }
        let agg = self
            .building_aggregates
            .get(usize::from(building.owner_id))
            .copied()
            .unwrap_or_default();
        let capacity = self.troop_capacity(player, agg);
        let mut base =
            self.state.config.city_max_troops * player.leader.city_troop_capacity_multiplier();
        if player.player_type == crate::player::PlayerType::Bot {
            base /= 1.5;
        }
        let troops = crate::building::city_upgrade_reinforcements(building.level, base);
        if let Some(player) = self.state.player_mut(building.owner_id) {
            player.max_troops = capacity;
            player.credit_troops_capped(troops, capacity);
        }
    }

    /// Advance construction timers and emit [`GameEvent::StructureReady`].
    pub fn execute_construction(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // buildings are maintained sorted by id on insertion.

        let mut completed = Vec::new();
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
                if b.kind == BuildingKind::SAM {
                    self.bot_sam_tiles_cache = None;
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
                    let width = self.state.map.width;
                    self.state.announce_leader_ability(
                        b.owner_id,
                        Some((b.tile_idx % width, b.tile_idx / width)),
                    );
                }
                self.state.events.push(GameEvent::StructureReady {
                    id: b.id,
                    tile_idx: b.tile_idx,
                    kind: b.kind,
                });
                completed.push(*b);
            }
        }
        if completed.iter().any(|b| b.kind == BuildingKind::City && b.level > 1) {
            self.refresh_building_aggregates();
        }
        for building in completed {
            if building.kind == BuildingKind::City && building.level > 1 {
                self.credit_city_upgrade_reinforcements(building);
            }
            self.hero_structure_ready(building);
        }

        let mut finished_production = Vec::new();
        for (&building_id, state) in &mut self.building_productions {
            let Some(build) = &mut state.arsenal_build else {
                continue;
            };
            if build.ticks_until_complete > 0 {
                build.ticks_until_complete -= 1;
            }
            if build.ticks_until_complete == 0 {
                let build = *build;
                state.arsenal_build = None;
                match build.kind {
                    crate::game::CityArsenalKind::Warship => {
                        finished_production.push((building_id, build));
                    }
                    crate::game::CityArsenalKind::Nuke => {
                        state.nuke_ready = true;
                    }
                    crate::game::CityArsenalKind::AntiNuke => {
                        state.has_anti_nuke = true;
                    }
                }
            }
        }
        for (building_id, build) in finished_production {
            let Some(port) = self.buildings.iter().find(|b| b.id == building_id).copied() else {
                continue;
            };
            if port.kind != BuildingKind::Port || port.under_construction || port.active_level() < 1 {
                continue;
            }
            let (source, landing, patrol, path) = if let (Some(source), Some(target)) =
                (build.warship_source_tile, build.warship_target_tile)
            {
                let Some(path) = self
                    .path_scratch
                    .astar
                    .find_path(&self.state.map, &[source], target)
                else {
                    continue;
                };
                (source, target, target, path)
            } else if let Some(spawn_tile) = crate::warp_fleet::coastal_warship_spawn_tile(
                &self.state.map,
                &self.water,
                port.tile_idx,
            ) {
                (spawn_tile, spawn_tile, spawn_tile, Vec::new())
            } else {
                continue;
            };
            self.spawn_warship_fleet(
                port.owner_id,
                port.id,
                source,
                landing,
                patrol,
                path,
            );
        }
    }
}

const MAX_REGIONAL_TRADE_PORTS: usize = 12;
const SHORT_RANGE_MIN_DIST_SQ: u64 = 28 * 28;
const TRADE_SHIP_SPAWN_SPACING_TICKS: u64 = 15;

impl SowEngine {
    pub(crate) fn refresh_port_topology_cache(&mut self) {
        let map_width = self.state.map.width.max(1);
        let fp = crate::engine::compute_port_fingerprint(
            &self.buildings,
            map_width,
            self.water.components.len(),
        );
        if self.port_topology_cache.valid && self.port_topology_cache.fingerprint == fp {
            return;
        }
        let cache = &mut self.port_topology_cache;
        cache.ready_ports.clear();
        cache.by_tile.clear();
        cache.by_component.clear();
        cache.component_unique_owner.clear();
        cache.port_levels_by_owner.clear();

        for b in &self.buildings {
            if b.kind != BuildingKind::Port {
                continue;
            }
            let active_level = b.active_level();
            if active_level > 0 {
                *cache.port_levels_by_owner.entry(b.owner_id).or_default() +=
                    u32::from(active_level);
            }
            if b.under_construction || active_level < 1 {
                continue;
            }
            let (water_tile, water_component) = crate::sea_lane::adjacent_water_tile_with_component(
                &self.state.map,
                &self.water,
                b.tile_idx,
            )
            .unwrap_or((b.tile_idx, 0));
            let idx = cache.ready_ports.len();
            cache.ready_ports.push(crate::engine::IndexedTradePort {
                id: b.id,
                owner_id: b.owner_id,
                tile_idx: b.tile_idx,
                active_level,
                water_tile,
                water_component,
                x: b.tile_idx % map_width,
                y: b.tile_idx / map_width,
            });
            cache.by_tile.insert(b.tile_idx, idx);
            if water_component > 0 {
                cache.by_component.entry(water_component).or_default().push(idx);
                cache
                    .component_unique_owner
                    .entry(water_component)
                    .and_modify(|owner| {
                        if *owner != 0 && *owner != b.owner_id {
                            *owner = 0;
                        }
                    })
                    .or_insert(b.owner_id);
            }
        }
        cache.fingerprint = fp;
        cache.valid = true;
    }

    pub fn execute_trade_ships(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        if !self.campaign_allows_action(crate::campaign::CampaignAction::TradeShip) {
            return;
        }

        self.refresh_port_topology_cache();

        if !self.state.sea_lanes.is_empty() {
            self.execute_trade_ships_via_sea_lanes();
            return;
        }

        if self.port_topology_cache.ready_ports.is_empty() {
            let mut removed = false;
            for i in (0..self.fleets.len()).rev() {
                let fleet = &self.fleets[i];
                if fleet.unit_type == UnitType::TradeShip
                    && fleet.troops > 0.0
                    && fleet.captured_trade_return.is_none()
                {
                    self.fleets.swap_remove(i);
                    removed = true;
                }
            }
            if removed {
                self.fleets.sort_unstable_by_key(|fleet| fleet.id);
            }
            return;
        }

        let mut ships_per_port_tile = std::collections::HashMap::<u32, u32>::new();
        let mut in_flight_pairs = std::collections::HashMap::<(u32, u32), u32>::new();
        let mut to_remove = Vec::new();
        let mut needs_reroute: Vec<(usize, u16, u32, Option<u32>, Option<u32>)> = Vec::new();
        let mut punt_arrivals = Vec::new();

        for (index, fleet) in self.fleets.iter().enumerate() {
            if fleet.unit_type != UnitType::TradeShip || fleet.troops <= 0.0 {
                continue;
            }
            if fleet.captured_trade_return.is_some() {
                continue;
            }

            let origin = self
                .port_topology_cache
                .by_tile
                .get(&fleet.src_tile)
                .copied()
                .map(|idx| self.port_topology_cache.ready_ports[idx])
                .filter(|port| port.owner_id == fleet.owner_id);
            let dst_valid = self
                .port_topology_cache
                .by_tile
                .get(&fleet.dst_tile)
                .copied()
                .is_some_and(|idx| {
                    self.port_topology_cache.ready_ports[idx].owner_id != fleet.owner_id
                });
            let arrived = (fleet.path.is_empty() && fleet.path_cursor > 0)
                || (!fleet.path.is_empty() && fleet.path_cursor >= fleet.path.len());
            if arrived
                && dst_valid
                && let Some(origin_port) = origin
                && self
                    .state
                    .player(fleet.owner_id)
                    .is_some_and(|p| p.leader.ability() == Ability::PuntGardens)
            {
                punt_arrivals.push(origin_port.id);
            }

            let origin_has_capacity = origin.is_some_and(|port| {
                port.water_component > 0
                    && self
                        .port_topology_cache
                        .component_unique_owner
                        .get(&port.water_component)
                        .is_some_and(|&only| only != port.owner_id)
                    && ships_per_port_tile.get(&port.tile_idx).copied().unwrap_or(0)
                        < u32::from(port.active_level)
            });

            if origin_has_capacity {
                let origin_port = origin.unwrap();
                *ships_per_port_tile.entry(origin_port.tile_idx).or_default() += 1;
                if !fleet.path.is_empty() && !arrived && dst_valid {
                    *in_flight_pairs
                        .entry((fleet.src_tile, fleet.dst_tile))
                        .or_default() += 1;
                } else {
                    needs_reroute.push((
                        index,
                        fleet.owner_id,
                        fleet.current_tile,
                        Some(origin_port.tile_idx),
                        arrived.then_some(fleet.dst_tile),
                    ));
                }
            } else {
                // Origin port was captured, destroyed, upgrading, or over quota — try re-homing.
                needs_reroute.push((
                    index,
                    fleet.owner_id,
                    fleet.current_tile,
                    None,
                    arrived.then_some(fleet.dst_tile),
                ));
            }
        }

        let map_width = self.state.map.width.max(1);
        for (roll_seq, (index, owner_id, current_tile, fixed_origin_tile, avoid_dst)) in
            needs_reroute.into_iter().enumerate()
        {
            let source_port = if let Some(origin_tile) = fixed_origin_tile {
                self.port_topology_cache
                    .by_tile
                    .get(&origin_tile)
                    .copied()
                    .map(|idx| self.port_topology_cache.ready_ports[idx])
            } else {
                // Pick the closest owned ready port with spare slot capacity and foreign partners.
                let cx = current_tile % map_width;
                let cy = current_tile / map_width;
                let chosen = self
                    .port_topology_cache
                    .ready_ports
                    .iter()
                    .copied()
                    .filter(|port| {
                        port.owner_id == owner_id
                            && port.water_component > 0
                            && self
                                .port_topology_cache
                                .component_unique_owner
                                .get(&port.water_component)
                                .is_some_and(|&only| only != owner_id)
                            && ships_per_port_tile.get(&port.tile_idx).copied().unwrap_or(0)
                                < u32::from(port.active_level)
                    })
                    .min_by_key(|port| {
                        let dx = u64::from(port.x.abs_diff(cx));
                        let dy = u64::from(port.y.abs_diff(cy));
                        (dx * dx + dy * dy, port.id)
                    });
                if let Some(port) = chosen {
                    *ships_per_port_tile.entry(port.tile_idx).or_default() += 1;
                }
                chosen
            };

            let Some(src_port) = source_port else {
                to_remove.push(index);
                continue;
            };
            let Some(route) = self.pick_organic_trade_route(
                src_port,
                avoid_dst,
                &in_flight_pairs,
                roll_seq as u64,
            ) else {
                if let Some(count) = ships_per_port_tile.get_mut(&src_port.tile_idx) {
                    *count = count.saturating_sub(1);
                }
                to_remove.push(index);
                continue;
            };
            let fleet = &mut self.fleets[index];
            fleet.src_tile = route.src_tile;
            fleet.dst_tile = route.dst_tile;
            fleet.target_owner = route.dst_owner;
            fleet.replace_path_arc(route.path);
            *in_flight_pairs
                .entry((route.src_tile, route.dst_tile))
                .or_default() += 1;
        }

        if !to_remove.is_empty() {
            to_remove.sort_unstable();
            for index in to_remove.into_iter().rev() {
                self.fleets.swap_remove(index);
            }
            self.fleets.sort_unstable_by_key(|fleet| fleet.id);
        }
        for origin_id in punt_arrivals {
            self.hero_trade_arrived(origin_id);
        }

        // Per-port organic staggered spawn: each ready port manages its own `active_level` slots.
        let mut spawn_candidates = Vec::new();
        for &port in &self.port_topology_cache.ready_ports {
            if port.water_component == 0 {
                continue;
            }
            if !self
                .state
                .player(port.owner_id)
                .is_some_and(|player| player.alive)
            {
                continue;
            }
            if self
                .port_topology_cache
                .component_unique_owner
                .get(&port.water_component)
                .is_none_or(|&only| only == port.owner_id)
            {
                continue;
            }
            let existing = ships_per_port_tile.get(&port.tile_idx).copied().unwrap_or(0);
            if existing >= u32::from(port.active_level) {
                continue;
            }
            // First ship of any port departs immediately; Lv2+ extra ships stagger every 15 ticks (~1.5s)
            // so ships form spaced convoys instead of overlapping on the same pixel.
            if existing > 0
                && !self
                    .state
                    .tick
                    .wrapping_add(port.id)
                    .is_multiple_of(TRADE_SHIP_SPAWN_SPACING_TICKS)
            {
                continue;
            }
            spawn_candidates.push(port);
        }

        let mut new_fleets = Vec::new();
        for (spawn_seq, port) in spawn_candidates.into_iter().enumerate() {
            let Some(route) = self.pick_organic_trade_route(
                port,
                None,
                &in_flight_pairs,
                (spawn_seq as u64).wrapping_add(0x517c_c1b7_2722_0a95),
            ) else {
                continue;
            };
            let id = self.state.next_fleet_id;
            self.state.next_fleet_id = self.state.next_fleet_id.wrapping_add(1).max(1);
            let mut fleet = crate::warp_fleet::WarpFleet::new_with_shared_path(
                id,
                port.owner_id,
                route.dst_owner,
                UnitType::TradeShip,
                UnitType::TradeShip.max_health(),
                (route.src_tile, route.dst_tile),
                route.path,
            );
            let port_levels = self
                .port_topology_cache
                .port_levels_by_owner
                .get(&port.owner_id)
                .copied()
                .unwrap_or_default();
            fleet.set_speed_bonus_percent(
                crate::building::cost::boat_speed_bonus_from_port_levels(port_levels),
            );
            if self
                .state
                .player(port.owner_id)
                .is_some_and(|player| player.leader.has_resource_bonus_from(Source::Trade))
            {
                self.state.announce_leader_ability(port.owner_id, None);
            }
            *in_flight_pairs
                .entry((route.src_tile, route.dst_tile))
                .or_default() += 1;
            *ships_per_port_tile.entry(port.tile_idx).or_default() += 1;
            new_fleets.push(fleet);
        }
        for fleet in new_fleets {
            self.add_fleet(fleet);
        }
    }

    fn pick_organic_trade_route(
        &mut self,
        src: crate::engine::IndexedTradePort,
        avoid_destination_tile: Option<u32>,
        in_flight_pairs: &std::collections::HashMap<(u32, u32), u32>,
        roll_salt: u64,
    ) -> Option<TradeRoute> {
        let comp_indices = self
            .port_topology_cache
            .by_component
            .get(&src.water_component)?;
        let mut candidates: Vec<(u64, crate::engine::IndexedTradePort)> = Vec::new();
        let mut fallback_avoided: Option<(u64, crate::engine::IndexedTradePort)> = None;

        for &idx in comp_indices {
            let dst = self.port_topology_cache.ready_ports[idx];
            if dst.owner_id == src.owner_id {
                continue;
            }
            let dx = u64::from(src.x.abs_diff(dst.x));
            let dy = u64::from(src.y.abs_diff(dst.y));
            let dist_sq = dx * dx + dy * dy;
            if Some(dst.tile_idx) == avoid_destination_tile {
                fallback_avoided = Some((dist_sq, dst));
                continue;
            }
            candidates.push((dist_sq, dst));
        }

        if candidates.is_empty() {
            if let Some(fallback) = fallback_avoided {
                candidates.push(fallback);
            } else {
                return None;
            }
        }

        // Regional band: select the closest MAX_REGIONAL_TRADE_PORTS ports in O(P).
        if candidates.len() > MAX_REGIONAL_TRADE_PORTS {
            candidates.select_nth_unstable_by_key(MAX_REGIONAL_TRADE_PORTS, |&(d, p)| (d, p.id));
            candidates.truncate(MAX_REGIONAL_TRADE_PORTS);
        }
        candidates.sort_unstable_by_key(|&(d, p)| (d, p.id));

        let n = candidates.len();
        let proximity_count = (n / 3).max(4).min(n);
        let owner_player = self.state.player(src.owner_id);
        let owner_team = owner_player.and_then(|p| p.team);

        let mut weights = [0u64; MAX_REGIONAL_TRADE_PORTS];
        let mut total_weight = 0u64;
        for (i, &(dist_sq, dst)) in candidates.iter().enumerate() {
            let mut mult = 1u64;
            if i < proximity_count && dist_sq >= SHORT_RANGE_MIN_DIST_SQ {
                mult += 1;
            }
            let friendly = owner_player.is_some_and(|p| {
                p.alliances.contains(&dst.owner_id)
                    || (owner_team.is_some()
                        && self
                            .state
                            .player(dst.owner_id)
                            .is_some_and(|other| other.team == owner_team))
            });
            if friendly {
                mult += 1;
            }
            let base = u64::from(dst.active_level.max(1)) * mult;
            let in_flight = u64::from(
                in_flight_pairs
                    .get(&(src.tile_idx, dst.tile_idx))
                    .copied()
                    .unwrap_or(0),
            );
            let w = ((base * 4) / (1 + in_flight * 3)).max(1);
            weights[i] = w;
            total_weight += w;
        }

        let seed = self
            .state
            .seed
            .wrapping_add(self.state.tick.wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .wrapping_add(src.id.wrapping_mul(0xbf58_476d_1ce4_e5b9))
            .wrapping_add(roll_salt.wrapping_mul(0x94d0_49bb_1331_11eb));
        let mut rng = wyrand::WyRand::new(seed);
        let mut roll = rng.rand() % total_weight.max(1);
        let mut start_idx = 0usize;
        for (i, &w) in weights[..n].iter().enumerate() {
            if roll < w {
                start_idx = i;
                break;
            }
            roll -= w;
        }

        // Try the weighted pick first; fall back through remaining regional candidates only if A* fails.
        for offset in 0..n {
            let (_, dst) = candidates[(start_idx + offset) % n];
            if let Some(path) = self.path_scratch.find_path_cached(
                &self.state.map,
                src.water_tile,
                dst.water_tile,
            ) && !path.is_empty()
            {
                return Some(TradeRoute {
                    src_tile: src.tile_idx,
                    dst_tile: dst.tile_idx,
                    dst_owner: dst.owner_id,
                    path,
                });
            }
        }
        None
    }

    /// Compatibility path for tests or scenarios supplying explicit synthetic `state.sea_lanes`.
    fn execute_trade_ships_via_sea_lanes(&mut self) {
        let sea_lanes = self.state.sea_lanes.clone();
        let adjacency = crate::sea_lane::SeaLaneAdjacency::build(&sea_lanes);
        let map_width = self.state.map.width.max(1);
        let ports: Vec<TradePort> = self
            .port_topology_cache
            .ready_ports
            .iter()
            .map(|p| TradePort {
                id: p.id,
                owner_id: p.owner_id,
                tile_idx: p.tile_idx,
            })
            .collect();
        let mut route_cache =
            std::collections::HashMap::<(u64, u64), Option<std::sync::Arc<Vec<u32>>>>::new();
        let mut retained = std::collections::HashMap::<u16, u32>::new();
        let mut to_remove = Vec::new();
        let mut punt_arrivals = Vec::new();

        for (index, fleet) in self.fleets.iter_mut().enumerate() {
            if fleet.unit_type != UnitType::TradeShip || fleet.troops <= 0.0 {
                continue;
            }
            if fleet.captured_trade_return.is_some() {
                continue;
            }

            let capacity = self
                .port_topology_cache
                .port_levels_by_owner
                .get(&fleet.owner_id)
                .copied()
                .unwrap_or_default();
            let count = retained.entry(fleet.owner_id).or_default();
            if *count >= capacity {
                to_remove.push(index);
                continue;
            }

            let origin = self
                .port_topology_cache
                .by_tile
                .get(&fleet.src_tile)
                .copied()
                .map(|idx| self.port_topology_cache.ready_ports[idx])
                .filter(|port| port.owner_id == fleet.owner_id);
            let route_is_current = origin.is_some()
                && self
                    .port_topology_cache
                    .by_tile
                    .get(&fleet.dst_tile)
                    .copied()
                    .is_some_and(|idx| {
                        self.port_topology_cache.ready_ports[idx].owner_id != fleet.owner_id
                    });
            let arrived = (fleet.path.is_empty() && fleet.path_cursor > 0)
                || (!fleet.path.is_empty() && fleet.path_cursor >= fleet.path.len());
            if arrived
                && route_is_current
                && let Some(origin) = origin
                && self
                    .state
                    .player(fleet.owner_id)
                    .is_some_and(|p| p.leader.ability() == Ability::PuntGardens)
            {
                punt_arrivals.push(origin.id);
            }
            if fleet.path.is_empty() || arrived || !route_is_current {
                let Some(route) = find_trade_route_via_lanes(
                    &adjacency,
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
                fleet.replace_path_arc(route.path);
            }
            *count += 1;
        }

        if !to_remove.is_empty() {
            for index in to_remove.into_iter().rev() {
                self.fleets.swap_remove(index);
            }
            self.fleets.sort_unstable_by_key(|fleet| fleet.id);
        }
        for origin_id in punt_arrivals {
            self.hero_trade_arrived(origin_id);
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
            let capacity = self
                .port_topology_cache
                .port_levels_by_owner
                .get(&player_id)
                .copied()
                .unwrap_or_default();
            if capacity == 0 {
                continue;
            }
            let count = retained.entry(player_id).or_default();
            while *count < capacity {
                let Some(route) = find_trade_route_via_lanes(
                    &adjacency,
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
                let mut fleet = crate::warp_fleet::WarpFleet::new_with_shared_path(
                    id,
                    player_id,
                    route.dst_owner,
                    UnitType::TradeShip,
                    UnitType::TradeShip.max_health(),
                    (route.src_tile, route.dst_tile),
                    route.path,
                );
                fleet.set_speed_bonus_percent(
                    crate::building::cost::boat_speed_bonus_from_port_levels(capacity),
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
    path: std::sync::Arc<Vec<u32>>,
}

/// Whether finishing a `kind` building pays `leader` its ability: building income,
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

fn find_trade_route_via_lanes(
    adjacency: &crate::sea_lane::SeaLaneAdjacency,
    sea_lanes: &[crate::sea_lane::SeaLane],
    map_width: u32,
    ports: &[TradePort],
    owner_id: u16,
    preferred_tile: Option<u32>,
    avoid_destination_tile: Option<u32>,
    route_cache: &mut std::collections::HashMap<(u64, u64), Option<std::sync::Arc<Vec<u32>>>>,
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
                    crate::sea_lane::route_through_adjacency(
                        adjacency,
                        sea_lanes,
                        source.id,
                        destination.id,
                    )
                    .map(std::sync::Arc::new)
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
            find_trade_route_via_lanes(
                adjacency,
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
