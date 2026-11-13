use crate::engine::SowEngine;
use crate::execution::refund_fleet_troops_to_player;
use crate::game::{GameEvent, GamePhase};
use crate::geo_entities::LossWhen;
use crate::warp_fleet::best_shore_spawn_for_transport;

impl SowEngine {
    /// Fleet movement, retreat, landing, and naval combat.
    pub fn execute_fleets(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        let w = self.state.map.width;

        if self.building_aggregates_dirty {
            let max_pid = self
                .state
                .players
                .iter()
                .map(|p| p.id as usize)
                .max()
                .unwrap_or(0);
            self.building_aggregates = crate::building::aggregate_buildings_per_player(
                self.buildings.iter().copied(),
                max_pid,
            );
            self.building_aggregates_dirty = false;
        }

        let held_target = self
            .campaign_assault
            .as_ref()
            .filter(|assault| assault.hold_last_tile)
            .map(|assault| assault.root_target);
        let pre_defeat_targets = self.campaign_pre_defeat_targets.clone();
        let pre_defeat_starting_tile_counts = self.campaign_starting_tile_counts.clone();
        let mut pre_defeat_pending = self.campaign_pre_defeat_pending;
        // fleets are sorted on insertion
        let mut to_remove = Vec::new();
        let mut elimination_candidates = Vec::new();
        let mut territory_changes = Vec::new();
        let tick_rate_ms = self.state.config.tick_rate_ms;
        let transport_base_steps_per_tick =
            crate::warp_fleet::transport_base_steps_per_tick(tick_rate_ms);

        for i in 0..self.fleets.len() {
            let fleet = &mut self.fleets[i];

            if fleet.troops <= 0.0 {
                to_remove.push(i);
                continue;
            }

            let port_levels = self
                .building_aggregates
                .get(fleet.owner_id as usize)
                .map(|aggregate| aggregate.port_levels)
                .unwrap_or_default();
            fleet.set_speed_bonus_percent(
                crate::building::cost::boat_speed_bonus_from_port_levels(port_levels),
            );

            if fleet.retreating && fleet.retreat_dst.is_none() {
                if let Some(player) = self.state.player(fleet.owner_id) {
                    let here_comp = self.water.component_of(fleet.current_tile);
                    let comp_filter = if here_comp == 0 {
                        None
                    } else {
                        Some((&self.water, here_comp))
                    };
                    if let Some(r_dst) = best_shore_spawn_for_transport(
                        &self.state.map,
                        fleet.owner_id,
                        &player.border_tiles,
                        fleet.current_tile,
                        comp_filter,
                    ) {
                        fleet.retreat_dst = Some(r_dst);
                        if let Some(path) = self.path_scratch.astar.find_path(
                            &self.state.map,
                            &[fleet.current_tile],
                            r_dst,
                        ) {
                            fleet.replace_path(path);
                        } else {
                            refund_fleet_troops_to_player(
                                &mut self.state,
                                fleet.owner_id,
                                fleet.troops,
                            );
                            to_remove.push(i);
                            continue;
                        }
                    } else {
                        refund_fleet_troops_to_player(
                            &mut self.state,
                            fleet.owner_id,
                            fleet.troops,
                        );
                        to_remove.push(i);
                        continue;
                    }
                } else {
                    to_remove.push(i);
                    continue;
                }
            }

            if fleet.unit_type == crate::game::UnitType::TransportShip
                && fleet.flow_target.is_none()
                && fleet.path.is_empty()
            {
                refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                to_remove.push(i);
                continue;
            }

            if let Some(target) = fleet.flow_target {
                let map = &self.state.map;
                let flow_field = self.flow_field_cache.get_or_compute(target, map);
                let dir = flow_field.directions[fleet.current_tile as usize];
                if dir < 8 {
                    let w = map.width;
                    let cx = fleet.current_tile % w;
                    let cy = fleet.current_tile / w;
                    let dx = [0, 1, 1, 1, 0, -1, -1, -1];
                    let dy = [-1, -1, 0, 1, 1, 1, 0, -1];
                    let nx = cx as i32 + dx[dir as usize];
                    let ny = cy as i32 + dy[dir as usize];
                    fleet.current_tile = ny as u32 * w + nx as u32;
                } else if dir == 8 {
                    // Reached destination!
                    fleet.replace_path(Vec::new());
                } else {
                    // Unreachable via FlowField
                    refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                    to_remove.push(i);
                    continue;
                }
            } else if fleet.path_cursor < fleet.path.len() {
                let base_speed = if fleet.unit_type == crate::game::UnitType::TransportShip {
                    transport_base_steps_per_tick
                } else {
                    1.0
                };
                let speed = fleet.movement_steps_per_tick(base_speed);
                let progress = fleet.movement_progress + speed;
                let remaining = fleet.path.len() - fleet.path_cursor;
                if !progress.is_finite() {
                    fleet.movement_progress = 0.0;
                } else {
                    let advance = progress.floor().min(remaining as f64) as usize;
                    if advance > 0 {
                        fleet.path_cursor += advance;
                        fleet.current_tile = fleet.path[fleet.path_cursor - 1];
                        fleet.movement_progress = if fleet.path_cursor >= fleet.path.len() {
                            0.0
                        } else {
                            (progress - advance as f64).clamp(0.0, 1.0)
                        };
                    } else {
                        fleet.movement_progress = progress.max(0.0);
                    }
                }
            }

            fleet.update_eta_after_tick(transport_base_steps_per_tick, tick_rate_ms);

            if fleet.unit_type == crate::game::UnitType::TradeShip {
                if let Some(port_id) = fleet.captured_trade_return
                    && fleet.path_cursor >= fleet.path.len()
                    && let Some(port) = self.buildings.iter().find(|port| {
                        port.id == port_id
                            && port.owner_id == fleet.owner_id
                            && port.kind == crate::game::BuildingKind::Port
                            && !port.under_construction
                            && port.active_level() >= 1
                    })
                    && (port.tile_idx % w)
                        .abs_diff(fleet.current_tile % w)
                        .max((port.tile_idx / w).abs_diff(fleet.current_tile / w))
                        <= WARSHIP_DOCKING_RANGE
                {
                    let owner_id = fleet.owner_id;
                    let fleet_id = fleet.id;
                    let tile = fleet.current_tile;
                    let payout = self.state.player(owner_id).map(|player| {
                        let seconds = 21.0;
                        (
                            crate::execution::income_rates::trade_income_per_second(
                                1,
                                player.leader,
                                &self.state.config,
                            ) * seconds,
                            crate::execution::income_rates::trade_troops_per_second(
                                1,
                                player.leader,
                                &self.state.config,
                            ) * seconds,
                        )
                    });
                    let mut gold_paid = 0.0;
                    if let Some((gold, troops)) = payout
                        && let Some(player) = self.state.player_mut(owner_id)
                    {
                        gold_paid = gold;
                        player.credit_gold(gold);
                        player.credit_troops_capped(troops, player.max_troops);
                    }
                    self.state.events.push(GameEvent::TradeShipDelivered {
                        owner_id,
                        fleet_id,
                        tile,
                        gold: gold_paid,
                    });
                    fleet.captured_trade_return = None;
                    fleet.piracy_safe_until = self.state.tick.saturating_add(50);
                    fleet.path = std::sync::Arc::new(Vec::new());
                    fleet.path_cursor = 1;
                    continue;
                }
                if fleet.path_cursor >= fleet.path.len() && !fleet.path.is_empty() {
                    // Let the passive trade scheduler choose the next destination
                    // on the following tick instead of reversing the same lane.
                    fleet.path = std::sync::Arc::new(Vec::new());
                }
                continue;
            }

            if fleet.unit_type == crate::game::UnitType::Warship {
                // Stop at destination
                if fleet.path_cursor >= fleet.path.len() {
                    fleet.replace_path(Vec::new());
                }
                continue;
            }

            if fleet.path_cursor < fleet.path.len() {
                continue;
            }

            let w = self.state.map.width;
            let lx = fleet.current_tile % w;
            let ly = fleet.current_tile / w;
            let owner_here = self.state.map.owner_id(lx, ly);

            if fleet.retreating {
                if owner_here == fleet.owner_id {
                    let deaths = fleet.troops * crate::warp_fleet::FLEET_RETREAT_SHORE_MALUS;
                    if let Some(p) = self.state.player_mut(fleet.owner_id) {
                        let loss_multiplier =
                            p.leader.attacker_loss_multiplier(LossWhen::SeaRetreat);
                        let ordinary_return =
                            (p.troops + (fleet.troops - deaths)).min(p.max_troops);
                        p.credit_troops_capped(
                            fleet.troops - deaths * loss_multiplier,
                            p.max_troops,
                        );
                        if loss_multiplier < 1.0 && p.troops > ordinary_return {
                            self.state
                                .announce_leader_ability(fleet.owner_id, Some((lx, ly)));
                        }
                    }
                } else {
                    refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                }
                to_remove.push(i);
                continue;
            }

            if owner_here != fleet.owner_id
                && Self::pre_defeat_watches(&pre_defeat_targets, pre_defeat_pending, owner_here)
                && self.state.player(owner_here).is_some_and(|player| {
                    Self::pre_defeat_reached(&pre_defeat_starting_tile_counts, owner_here, player)
                })
            {
                if pre_defeat_pending.is_none() {
                    pre_defeat_pending = Some(owner_here);
                    self.campaign_pre_defeat_since = self.state.tick;
                }
                continue;
            }

            if fleet.unit_type == crate::game::UnitType::TransportShip {
                self.state.events.push(GameEvent::TransportShipLanded {
                    owner_id: fleet.owner_id,
                    x: lx,
                    y: ly,
                });
            }

            if owner_here == fleet.owner_id {
                let deaths = fleet.troops * crate::warp_fleet::FLEET_RETREAT_SHORE_MALUS;
                let survivors = fleet.troops - deaths;
                if let Some(p) = self.state.player_mut(fleet.owner_id) {
                    p.credit_troops_capped(survivors, p.max_troops);
                }
                to_remove.push(i);
                continue;
            }

            if held_target == Some(owner_here)
                && !Self::pre_defeat_watches(&pre_defeat_targets, pre_defeat_pending, owner_here)
                && self
                    .state
                    .player(owner_here)
                    .is_some_and(|target| target.tile_count <= 1)
            {
                refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                to_remove.push(i);
                continue;
            }

            let prev_owner = owner_here;
            let f_owner = fleet.owner_id;
            let f_troops = fleet.troops;
            let f_id = fleet.id;
            let f_target = fleet.target_owner;
            // Transports ashore on another player's land: their troops land from ships.
            let f_landed =
                fleet.unit_type == crate::game::UnitType::TransportShip && owner_here != 0;

            let (eliminated, candidates) = self.state.set_tile_owner_for_execution(lx, ly, f_owner);
            elimination_candidates.extend(
                eliminated
                    .into_iter()
                    .map(|(victim, x, y)| (victim, f_owner, x, y)),
            );
            if let Some(change) = candidates {
                territory_changes.push(change);
            }
            self.state.events.push(GameEvent::TileCaptured {
                x: lx,
                y: ly,
                new_owner: f_owner,
                previous_owner: prev_owner,
                troops: f_troops,
            });

            // Drop mutable borrow of self.fleets before calling method on self
            crate::intent::spawn_attack_for_fleet_arrival(
                self,
                f_owner,
                f_target,
                f_troops,
                f_id,
                f_landed,
                ly * w + lx,
            );
            to_remove.push(i);
        }

        // Remove finished fleets in O(1) and re-sort to preserve deterministic order
        let has_removals = !to_remove.is_empty();
        for i in to_remove.into_iter().rev() {
            self.fleets.swap_remove(i);
        }
        self.campaign_pre_defeat_pending = pre_defeat_pending;
        if has_removals {
            self.fleets.sort_unstable_by_key(|f| f.id);
        }

        elimination_candidates.extend(self.annex_enclosed_components(territory_changes));
        for (victim, conqueror, x, y) in elimination_candidates {
            crate::execution::hero_effects::recruit_conquered_reserves(
                &mut self.state,
                conqueror,
                victim,
                (x, y),
                None,
            );
            self.eliminate_player(victim, conqueror, x, y, false);
        }
        self.execute_warship_ai();
        self.hero_coastal_foundings();
    }
}

const WARSHIP_TARGET_RANGE: u32 = 130;
const WARSHIP_PATROL_RANGE: u32 = 100;
const WARSHIP_DOCKING_RANGE: u32 = 5;
const WARSHIP_RETREAT_HEALTH_PERCENT: f64 = 75.0;
const WARSHIP_RELOAD_TICKS: u32 = 20;
const FLEET_SPATIAL_CELL: u32 = 16;

impl SowEngine {
    fn execute_warship_ai(&mut self) {
        if !self.campaign_allows_action(crate::campaign::CampaignAction::Warship) {
            return;
        }
        let width = self.state.map.width;
        if width == 0 {
            return;
        }

        let mut spatial = std::collections::HashMap::<(u32, u32), Vec<usize>>::new();
        for (index, fleet) in self.fleets.iter().enumerate() {
            if fleet.troops > 0.0 {
                spatial
                    .entry((
                        fleet.current_tile % width / FLEET_SPATIAL_CELL,
                        fleet.current_tile / width / FLEET_SPATIAL_CELL,
                    ))
                    .or_default()
                    .push(index);
            }
        }
        for indices in spatial.values_mut() {
            indices.sort_unstable_by_key(|&index| self.fleets[index].id);
        }
        let mut docked_per_port = std::collections::HashMap::<u64, usize>::new();
        let connected_port_components: std::collections::HashSet<(u16, u32)> = self
            .buildings
            .iter()
            .filter(|port| {
                port.kind == crate::game::BuildingKind::Port
                    && !port.under_construction
                    && port.active_level() >= 1
            })
            .flat_map(|port| {
                let mut components = Vec::new();
                let footprint = crate::building::BuildingFootprint::at(
                    crate::game::BuildingKind::Port,
                    port.tile_idx % width,
                    port.tile_idx / width,
                );
                for y in footprint.top..footprint.top + footprint.height as i32 {
                    for x in footprint.left..footprint.left + footprint.width as i32 {
                        if self.state.map.is_valid_coord(x, y) {
                            self.state
                                .map
                                .for_each_neighbor(x as u32, y as u32, |nx, ny| {
                                    if !footprint.contains(nx as i32, ny as i32) {
                                        let tile = ny * width + nx;
                                        if self.state.map.terrain[tile as usize].is_water() {
                                            let component = self.water.component_of(tile);
                                            if component != 0 {
                                                components.push((port.owner_id, component));
                                            }
                                        }
                                    }
                                });
                        }
                    }
                }
                components
            })
            .collect();
        for fleet in &self.fleets {
            if let Some(state) = fleet.warship.as_ref()
                && state.mode == crate::warp_fleet::WarshipMode::Docked
                && let Some(port_id) = state.retreat_port_id
            {
                *docked_per_port.entry(port_id).or_default() += 1;
            }
        }

        let mut candidate_indices = Vec::with_capacity(64);
        for index in 0..self.fleets.len() {
            if self.fleets[index].unit_type != crate::game::UnitType::Warship
                || self.fleets[index].troops <= 0.0
            {
                continue;
            }
            let current = self.fleets[index].current_tile;
            let owner_id = self.fleets[index].owner_id;
            let component = self.water.component_of(current);
            if component == 0 {
                continue;
            }

            let (cx, cy) = (current % width, current / width);
            let patrol_center = self.fleets[index]
                .warship
                .as_ref()
                .map_or(current, |state| state.patrol_tile);
            let has_connected_port = connected_port_components.contains(&(owner_id, component));
            let min_cell_x = cx.saturating_sub(WARSHIP_TARGET_RANGE) / FLEET_SPATIAL_CELL;
            let max_cell_x =
                cx.saturating_add(WARSHIP_TARGET_RANGE).min(width - 1) / FLEET_SPATIAL_CELL;
            let map_height = self.state.map.height;
            let min_cell_y = cy.saturating_sub(WARSHIP_TARGET_RANGE) / FLEET_SPATIAL_CELL;
            let max_cell_y =
                cy.saturating_add(WARSHIP_TARGET_RANGE).min(map_height - 1) / FLEET_SPATIAL_CELL;
            candidate_indices.clear();
            for by in min_cell_y..=max_cell_y {
                for bx in min_cell_x..=max_cell_x {
                    if let Some(bucket) = spatial.get(&(bx, by)) {
                        candidate_indices.extend(bucket.iter().copied());
                    }
                }
            }
            candidate_indices.sort_unstable_by_key(|&candidate| self.fleets[candidate].id);
            candidate_indices.dedup();

            let target = candidate_indices
                .iter()
                .copied()
                .filter(|&candidate| candidate != index)
                .filter_map(|candidate| {
                    let fleet = &self.fleets[candidate];
                    if fleet.troops <= 0.0
                        || self.water.component_of(fleet.current_tile) != component
                        || !self.naval_target_allowed(owner_id, fleet.owner_id)
                    {
                        return None;
                    }
                    let priority = match fleet.unit_type {
                        crate::game::UnitType::TransportShip => 0,
                        crate::game::UnitType::Warship => {
                            if fleet.warship.as_ref().is_some_and(|state| {
                                state.mode == crate::warp_fleet::WarshipMode::Docked
                            }) {
                                return None;
                            }
                            1
                        }
                        crate::game::UnitType::TradeShip => {
                            let patrol_distance_sq = {
                                let (px, py) = (patrol_center % width, patrol_center / width);
                                let tx = fleet.current_tile % width;
                                let ty = fleet.current_tile / width;
                                let dx = px.abs_diff(tx);
                                let dy = py.abs_diff(ty);
                                dx * dx + dy * dy
                            };
                            if fleet.piracy_safe_until > self.state.tick
                                || !has_connected_port
                                || patrol_distance_sq > WARSHIP_PATROL_RANGE * WARSHIP_PATROL_RANGE
                                || self.trade_ship_destination_is_friendly(
                                    owner_id,
                                    fleet.target_owner,
                                )
                                || self.trade_ship_is_protected_by_port(candidate)
                            {
                                return None;
                            }
                            2
                        }
                    };
                    let (tx, ty) = (fleet.current_tile % width, fleet.current_tile / width);
                    let dx = cx.abs_diff(tx);
                    let dy = cy.abs_diff(ty);
                    let distance = dx * dx + dy * dy;
                    (distance <= WARSHIP_TARGET_RANGE * WARSHIP_TARGET_RANGE)
                        .then_some((priority, distance, fleet.id, candidate))
                })
                .min();

            let target_index = target.map(|(_, _, _, target_index)| target_index);
            let mode = self.fleets[index]
                .warship
                .as_ref()
                .map(|state| state.mode)
                .unwrap_or(crate::warp_fleet::WarshipMode::Patrolling);

            self.apply_warship_repairs(index, &mut docked_per_port);
            if self.fleets[index].troops <= 0.0 {
                continue;
            }

            let max_health = self.fleets[index]
                .warship
                .as_ref()
                .map_or(1_000.0, crate::warp_fleet::WarshipState::max_health);
            if mode == crate::warp_fleet::WarshipMode::Patrolling
                && self.state.tick.saturating_sub(
                    self.fleets[index]
                        .warship
                        .as_ref()
                        .map_or(0, |state| state.manual_order_tick),
                ) >= 50
                && self.fleets[index].troops * 100.0 < max_health * WARSHIP_RETREAT_HEALTH_PERCENT
                && let Some((port_id, port_tile, path)) =
                    self.nearest_port_route(owner_id, current, Some(&docked_per_port))
            {
                let fleet = &mut self.fleets[index];
                let state = fleet.warship.get_or_insert_with(|| {
                    crate::warp_fleet::WarshipState::new(fleet.current_tile)
                });
                state.mode = crate::warp_fleet::WarshipMode::Retreating;
                state.retreat_port_id = Some(port_id);
                state.target_fleet = None;
                fleet.retreating = true;
                fleet.dst_tile = port_tile;
                fleet.replace_path(path);
            }

            let current_mode = self.fleets[index]
                .warship
                .as_ref()
                .map(|state| state.mode)
                .unwrap_or(crate::warp_fleet::WarshipMode::Patrolling);
            if current_mode == crate::warp_fleet::WarshipMode::Docked {
                self.handle_docked_warship(index, &docked_per_port);
                continue;
            }

            let is_retreating = current_mode == crate::warp_fleet::WarshipMode::Retreating;
            let selected_target = if is_retreating {
                target_index.filter(|&target| {
                    matches!(
                        self.fleets[target].unit_type,
                        crate::game::UnitType::TransportShip | crate::game::UnitType::Warship
                    )
                })
            } else {
                target_index
            };

            if let Some(target_index) = selected_target {
                let target_id = self.fleets[target_index].id;
                let target_type = self.fleets[target_index].unit_type;
                if let Some(state) = self.fleets[index].warship.as_mut() {
                    state.target_fleet = Some(target_id);
                }
                if target_type == crate::game::UnitType::TradeShip {
                    self.pursue_or_capture_trade_ship(index, target_index);
                } else {
                    self.fire_warship_shell(index, target_index);
                }
            } else {
                if let Some(state) = self.fleets[index].warship.as_mut() {
                    state.target_fleet = None;
                }
                if !is_retreating && self.fleets[index].path_cursor >= self.fleets[index].path.len()
                {
                    self.set_next_patrol_route(index);
                }
            }

            if is_retreating {
                self.ensure_retreat_route(index, &docked_per_port);
                self.try_dock_warship(index, &mut docked_per_port);
            }

            if let Some(state) = self.fleets[index].warship.as_mut() {
                state.reload_ticks = state.reload_ticks.saturating_sub(1);
            }
        }
    }

    fn trade_ship_destination_is_friendly(&self, owner_id: u16, destination: u16) -> bool {
        destination == owner_id
            || (destination != 0 && self.players_friendly(owner_id, destination))
    }

    fn trade_ship_is_protected_by_port(&self, fleet_index: usize) -> bool {
        let fleet = &self.fleets[fleet_index];
        let width = self.state.map.width;
        let (x, y) = (fleet.current_tile % width, fleet.current_tile / width);
        self.buildings.iter().any(|port| {
            port.kind == crate::game::BuildingKind::Port
                && !port.under_construction
                && port.active_level() >= 1
                && port.owner_id == fleet.owner_id
                && (port.tile_idx % width)
                    .abs_diff(x)
                    .max((port.tile_idx / width).abs_diff(y))
                    <= WARSHIP_DOCKING_RANGE
        })
    }

    fn apply_warship_repairs(
        &mut self,
        index: usize,
        docked_per_port: &mut std::collections::HashMap<u64, usize>,
    ) {
        let fleet = &self.fleets[index];
        let owner_id = fleet.owner_id;
        let tile = fleet.current_tile;
        let width = self.state.map.width;
        let max_health = fleet
            .warship
            .as_ref()
            .map_or(1_000.0, crate::warp_fleet::WarshipState::max_health);
        let passive_port = self.buildings.iter().any(|port| {
            if port.owner_id != owner_id
                || port.kind != crate::game::BuildingKind::Port
                || port.under_construction
                || port.active_level() == 0
            {
                return false;
            }
            let dx = (port.tile_idx % width).abs_diff(tile % width);
            let dy = (port.tile_idx / width).abs_diff(tile / width);
            dx * dx + dy * dy <= 150 * 150
                && self.port_has_water_component(port.tile_idx, self.water.component_of(tile))
        });
        if passive_port {
            self.fleets[index].troops =
                (self.fleets[index].troops + self.state.config.per_tick(10.0)).min(max_health);
        }
        let Some(state) = self.fleets[index].warship.as_ref() else {
            return;
        };
        if state.mode != crate::warp_fleet::WarshipMode::Docked {
            return;
        }
        let Some(port_id) = state.retreat_port_id else {
            return;
        };
        let Some(port) = self.buildings.iter().find(|port| {
            port.id == port_id
                && port.owner_id == owner_id
                && port.kind == crate::game::BuildingKind::Port
                && !port.under_construction
        }) else {
            return;
        };
        let docked = docked_per_port.get(&port_id).copied().unwrap_or(1).max(1) as f64;
        let repair_per_second = f64::from(port.active_level()) * 50.0 / docked;
        self.fleets[index].troops = (self.fleets[index].troops
            + self.state.config.per_tick(repair_per_second))
        .min(max_health);
        if let Some(state) = self.fleets[index].warship.as_mut() {
            state.docked_ticks = state.docked_ticks.saturating_add(1);
        }
    }

    fn handle_docked_warship(
        &mut self,
        index: usize,
        docked_per_port: &std::collections::HashMap<u64, usize>,
    ) {
        let owner_id = self.fleets[index].owner_id;
        let current_tile = self.fleets[index].current_tile;
        let retreat_port_id = self.fleets[index]
            .warship
            .as_ref()
            .and_then(|state| state.retreat_port_id);
        let valid_port = retreat_port_id.is_some_and(|port_id| {
            self.buildings.iter().any(|port| {
                port.id == port_id
                    && port.owner_id == owner_id
                    && port.kind == crate::game::BuildingKind::Port
                    && !port.under_construction
                    && port.active_level() >= 1
            })
        });
        if !valid_port {
            if let Some((port_id, tile, path)) =
                self.nearest_port_route(owner_id, current_tile, Some(docked_per_port))
            {
                let fleet = &mut self.fleets[index];
                if let Some(state) = fleet.warship.as_mut() {
                    state.mode = crate::warp_fleet::WarshipMode::Retreating;
                    state.retreat_port_id = Some(port_id);
                }
                fleet.retreating = true;
                fleet.dst_tile = tile;
                fleet.replace_path(path);
            } else {
                if let Some(state) = self.fleets[index].warship.as_mut() {
                    state.mode = crate::warp_fleet::WarshipMode::Patrolling;
                    state.retreat_port_id = None;
                }
                self.fleets[index].retreating = false;
            }
            return;
        }
        let max_health = self.fleets[index]
            .warship
            .as_ref()
            .map_or(1_000.0, crate::warp_fleet::WarshipState::max_health);
        if self.fleets[index].troops + 0.01 < max_health {
            return;
        }
        let current = current_tile;
        let port_tile = retreat_port_id
            .and_then(|id| self.buildings.iter().find(|port| port.id == id))
            .map(|port| port.tile_idx);
        let next = port_tile
            .and_then(|tile| self.port_water_tiles(tile).first().copied())
            .unwrap_or(current);
        let path = self
            .path_scratch
            .astar
            .find_path(&self.state.map, &[current], next)
            .unwrap_or_else(|| vec![current]);
        let fleet = &mut self.fleets[index];
        if let Some(state) = fleet.warship.as_mut() {
            state.mode = crate::warp_fleet::WarshipMode::Patrolling;
            state.retreat_port_id = None;
            state.docked_ticks = 0;
            state.patrol_target = None;
        }
        fleet.retreating = false;
        fleet.replace_path(path);
    }

    fn try_dock_warship(
        &mut self,
        index: usize,
        docked_per_port: &mut std::collections::HashMap<u64, usize>,
    ) {
        let Some(port_id) = self.fleets[index]
            .warship
            .as_ref()
            .and_then(|state| state.retreat_port_id)
        else {
            return;
        };
        let Some(port) = self.buildings.iter().find(|port| {
            port.id == port_id
                && port.owner_id == self.fleets[index].owner_id
                && !port.under_construction
                && port.active_level() >= 1
        }) else {
            if let Some((new_port, tile, path)) = self.nearest_port_route(
                self.fleets[index].owner_id,
                self.fleets[index].current_tile,
                Some(docked_per_port),
            ) {
                let fleet = &mut self.fleets[index];
                if let Some(state) = fleet.warship.as_mut() {
                    state.retreat_port_id = Some(new_port);
                }
                fleet.dst_tile = tile;
                fleet.replace_path(path);
            }
            return;
        };
        let width = self.state.map.width;
        let tile = self.fleets[index].current_tile;
        let distance = (port.tile_idx % width)
            .abs_diff(tile % width)
            .max((port.tile_idx / width).abs_diff(tile / width));
        if distance > WARSHIP_DOCKING_RANGE {
            return;
        }
        let occupancy = docked_per_port.get(&port_id).copied().unwrap_or(0);
        if occupancy >= usize::from(port.active_level()) {
            if let Some((new_port, tile, path)) = self.nearest_port_route(
                self.fleets[index].owner_id,
                self.fleets[index].current_tile,
                Some(docked_per_port),
            ) && new_port != port_id
            {
                if let Some(state) = self.fleets[index].warship.as_mut() {
                    state.retreat_port_id = Some(new_port);
                }
                self.fleets[index].dst_tile = tile;
                self.fleets[index].replace_path(path);
            }
            return;
        }
        *docked_per_port.entry(port_id).or_default() += 1;
        let fleet = &mut self.fleets[index];
        fleet.replace_path(Vec::new());
        fleet.retreating = false;
        if let Some(state) = fleet.warship.as_mut() {
            state.mode = crate::warp_fleet::WarshipMode::Docked;
            state.target_fleet = None;
        }
    }

    fn ensure_retreat_route(
        &mut self,
        index: usize,
        docked_per_port: &std::collections::HashMap<u64, usize>,
    ) {
        if self.fleets[index].path_cursor < self.fleets[index].path.len() {
            return;
        }
        if let Some((port_id, tile, path)) = self.nearest_port_route(
            self.fleets[index].owner_id,
            self.fleets[index].current_tile,
            Some(docked_per_port),
        ) {
            let fleet = &mut self.fleets[index];
            if let Some(state) = fleet.warship.as_mut() {
                state.retreat_port_id = Some(port_id);
            }
            fleet.dst_tile = tile;
            fleet.replace_path(path);
        } else {
            self.fleets[index].retreating = false;
            if let Some(state) = self.fleets[index].warship.as_mut() {
                state.mode = crate::warp_fleet::WarshipMode::Patrolling;
                state.retreat_port_id = None;
            }
        }
    }

    fn nearest_port_route(
        &mut self,
        owner_id: u16,
        from: u32,
        docked_per_port: Option<&std::collections::HashMap<u64, usize>>,
    ) -> Option<(u64, u32, Vec<u32>)> {
        let width = self.state.map.width;
        let component = self.water.component_of(from);
        let (x, y) = (from % width, from / width);
        let mut candidates = Vec::new();
        for port in self.buildings.iter().filter(|port| {
            port.owner_id == owner_id
                && port.kind == crate::game::BuildingKind::Port
                && !port.under_construction
                && port.active_level() >= 1
                && docked_per_port.is_none_or(|docked| {
                    docked.get(&port.id).copied().unwrap_or(0) < usize::from(port.active_level())
                })
        }) {
            for shore in self.port_water_tiles(port.tile_idx) {
                if self.water.component_of(shore) == component {
                    let distance = (shore % width).abs_diff(x) + (shore / width).abs_diff(y);
                    candidates.push((distance, port.id, shore));
                }
            }
        }
        candidates.sort_unstable();
        for (_, port_id, shore) in candidates {
            if let Some(path) = self
                .path_scratch
                .astar
                .find_path(&self.state.map, &[from], shore)
            {
                return Some((port_id, shore, path));
            }
        }
        None
    }

    fn port_water_tiles(&self, port_tile: u32) -> Vec<u32> {
        let width = self.state.map.width;
        if width == 0 || port_tile >= width.saturating_mul(self.state.map.height) {
            return Vec::new();
        }
        let footprint = crate::building::BuildingFootprint::at(
            crate::game::BuildingKind::Port,
            port_tile % width,
            port_tile / width,
        );
        let mut tiles = Vec::new();
        for y in footprint.top..footprint.top + footprint.height as i32 {
            for x in footprint.left..footprint.left + footprint.width as i32 {
                if self.state.map.is_valid_coord(x, y) {
                    self.state
                        .map
                        .for_each_neighbor(x as u32, y as u32, |nx, ny| {
                            if !footprint.contains(nx as i32, ny as i32) {
                                let tile = ny * width + nx;
                                if self.state.map.terrain[tile as usize].is_water() {
                                    tiles.push(tile);
                                }
                            }
                        });
                }
            }
        }
        tiles.sort_unstable();
        tiles.dedup();
        tiles
    }

    fn port_has_water_component(&self, port_tile: u32, component: u32) -> bool {
        if component == 0 || self.state.map.width == 0 {
            return false;
        }
        let width = self.state.map.width;
        if port_tile >= width.saturating_mul(self.state.map.height) {
            return false;
        }
        let footprint = crate::building::BuildingFootprint::at(
            crate::game::BuildingKind::Port,
            port_tile % width,
            port_tile / width,
        );
        for y in footprint.top..footprint.top + footprint.height as i32 {
            for x in footprint.left..footprint.left + footprint.width as i32 {
                if !self.state.map.is_valid_coord(x, y) {
                    continue;
                }
                let mut found = false;
                self.state
                    .map
                    .for_each_neighbor(x as u32, y as u32, |nx, ny| {
                        if found || footprint.contains(nx as i32, ny as i32) {
                            return;
                        }
                        let tile = ny * width + nx;
                        found = self.state.map.terrain[tile as usize].is_water()
                            && self.water.component_of(tile) == component;
                    });
                if found {
                    return true;
                }
            }
        }
        false
    }

    fn set_next_patrol_route(&mut self, index: usize) {
        let current = self.fleets[index].current_tile;
        let fleet_id = self.fleets[index].id;
        let center = self.fleets[index]
            .warship
            .as_ref()
            .map_or(current, |state| state.patrol_tile);
        let cycle = self.fleets[index]
            .warship
            .as_ref()
            .map_or(0, |state| state.patrol_cycle);
        let target =
            deterministic_patrol_tile(&self.state.map, &self.water, center, fleet_id, cycle)
                .unwrap_or(center);
        if let Some(path) = self
            .path_scratch
            .astar
            .find_path(&self.state.map, &[current], target)
        {
            let fleet = &mut self.fleets[index];
            if let Some(state) = fleet.warship.as_mut() {
                state.patrol_target = Some(target);
                state.patrol_cycle = state.patrol_cycle.wrapping_add(1);
            }
            fleet.replace_path(path);
        }
    }

    fn pursue_or_capture_trade_ship(&mut self, warship_index: usize, target_index: usize) {
        let width = self.state.map.width;
        let from = self.fleets[warship_index].current_tile;
        let target_tile = self.fleets[target_index].current_tile;
        let distance = (from % width).abs_diff(target_tile % width)
            + (from / width).abs_diff(target_tile / width);
        if distance <= WARSHIP_DOCKING_RANGE {
            let owner_id = self.fleets[warship_index].owner_id;
            let Some((port_id, port_water, path)) =
                self.nearest_port_route(owner_id, target_tile, None)
            else {
                return;
            };
            let fleet_id = self.fleets[target_index].id;
            let captured = &mut self.fleets[target_index];
            captured.owner_id = owner_id;
            captured.target_owner = owner_id;
            captured.captured_trade_return = Some(port_id);
            captured.piracy_safe_until = u64::MAX;
            captured.src_tile = target_tile;
            captured.dst_tile = port_water;
            captured.replace_path(path);
            if let Some(state) = self.fleets[warship_index].warship.as_mut() {
                state.record_progress(10);
                state.target_fleet = None;
            }
            self.state
                .events
                .push(crate::game::GameEvent::TradeShipCaptured {
                    owner_id,
                    fleet_id,
                    tile: target_tile,
                });
            return;
        }
        let target_id = self.fleets[target_index].id;
        let target_moved = {
            let last = self.fleets[warship_index].dst_tile;
            let (lx, ly) = (last % width, last / width);
            let (tx, ty) = (target_tile % width, target_tile / width);
            lx.abs_diff(tx) + ly.abs_diff(ty) >= 8
        };
        let should_repath = self.fleets[warship_index]
            .warship
            .as_ref()
            .is_some_and(|state| {
                state.target_fleet != Some(target_id)
                    || target_moved
                    || self.fleets[warship_index].path_cursor
                        >= self.fleets[warship_index].path.len()
            });
        if should_repath
            && let Some(path) =
                self.path_scratch
                    .astar
                    .find_path(&self.state.map, &[from], target_tile)
        {
            self.fleets[warship_index].dst_tile = target_tile;
            self.fleets[warship_index].replace_path(path);
        }
    }

    fn fire_warship_shell(&mut self, source_index: usize, target_index: usize) {
        let source_id = self.fleets[source_index].id;
        let target_id = self.fleets[target_index].id;
        let target_type = self.fleets[target_index].unit_type;
        if target_type == crate::game::UnitType::TransportShip
            && self.projectiles.iter().any(|projectile| {
                projectile.kind == crate::game::ProjectileKind::Shell
                    && projectile.target_fleet_id == Some(target_id)
            })
        {
            return;
        }
        let Some(state) = self.fleets[source_index].warship.as_ref() else {
            return;
        };
        if state.reload_ticks > 0 {
            return;
        }
        let veterancy = state.veterancy;
        let src_tile = self.fleets[source_index].current_tile;
        let dst_tile = self.fleets[target_index].current_tile;
        let owner_id = self.fleets[source_index].owner_id;
        let seed = self.state.seed
            ^ source_id.rotate_left(17)
            ^ target_id.rotate_left(31)
            ^ self.state.tick.rotate_left(7);
        let roll = deterministic_mix(seed) % 6;
        let mut damage = 500.0 + roll as f64 * 62.5;
        damage *= 1.0 + f64::from(veterancy) * 0.2;
        if let Some(state) = self.fleets[source_index].warship.as_mut() {
            state.reload_ticks = WARSHIP_RELOAD_TICKS;
        }
        let id = self.state.next_projectile_id;
        self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);
        self.projectiles.push(crate::game::Projectile {
            id,
            kind: crate::game::ProjectileKind::Shell,
            owner_id,
            src_tile,
            dst_tile,
            path: crate::pathfinding::bresenham_line(src_tile, dst_tile, self.state.map.width),
            path_cursor: 0,
            steps_per_tick: 3,
            active: true,
            source_fleet_id: Some(source_id),
            target_fleet_id: Some(target_id),
            damage,
            expires_at_tick: self.state.tick.saturating_add(50),
        });
        self.state.events.push(crate::game::GameEvent::NavalShot {
            owner_id,
            tile: src_tile,
        });
    }
}

fn deterministic_mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn deterministic_patrol_tile(
    map: &crate::map::GameMap,
    water: &crate::water_components::WaterComponents,
    center: u32,
    fleet_id: u64,
    cycle: u32,
) -> Option<u32> {
    let width = map.width;
    if width == 0 || center >= width.saturating_mul(map.height) {
        return None;
    }
    let (cx, cy) = (center % width, center / width);
    let component = water.component_of(center);
    for attempt in 0..24u64 {
        let random = deterministic_mix(fleet_id ^ u64::from(cycle).rotate_left(23) ^ attempt);
        let span = u64::from(WARSHIP_PATROL_RANGE) + 1;
        let dx = (random % span) as i32 - (WARSHIP_PATROL_RANGE / 2) as i32;
        let dy = ((random >> 32) % span) as i32 - (WARSHIP_PATROL_RANGE / 2) as i32;
        let x = cx as i32 + dx;
        let y = cy as i32 + dy;
        if !map.is_valid_coord(x, y) {
            continue;
        }
        let tile = y as u32 * width + x as u32;
        if map.terrain[tile as usize].is_water() && water.component_of(tile) == component {
            return Some(tile);
        }
    }
    Some(center)
}

#[cfg(test)]
mod tests {
    use super::SowEngine;
    use crate::execution::{AttackExecution, PrioritizedTile, fractional_extra_tiles_milli};
    use crate::game::{GameEvent, GameState, UnitType};
    use crate::game_config::GameConfig;
    use crate::map::MapTile;
    use crate::player::{Player, PlayerType};
    use crate::protocol::Team;
    use crate::warp_fleet::WarpFleet;
    use crate::water_components::WaterComponents;
    use std::collections::BinaryHeap;

    fn invasion_in_transit(original_target: u16) -> (SowEngine, u32) {
        let mut state = GameState::new(9, 20, 8, GameConfig::default());
        state.phase = crate::game::GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x20));
        for id in 1..=4 {
            let mut player = Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
            player.team = Some(if id <= 2 { Team::Blue } else { Team::Red });
            player.troops = 10_000.0;
            player.max_troops = 20_000.0;
            state.register_player(player);
        }
        for (x, y, owner) in [
            (1, 2, 1),
            (2, 2, original_target), // An unrelated, existing land front.
            (12, 2, original_target),
            (13, 2, original_target),
            (14, 2, original_target),
            (12, 3, 2),
            (13, 3, 2),
            (1, 5, 4),
        ] {
            state.map.terrain[(y * 20 + x) as usize] = MapTile::from_byte(0x80);
            state.set_tile_owner(x, y, owner);
        }
        state.map.compute_shorelines();
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = SowEngine::new(state, water);
        let landing = 2 * 20 + 12;
        engine.apply_intents(&[crate::protocol::StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::LaunchFleet {
                target_tile: landing,
                troops: Some(1_000.0),
            },
        }]);
        assert_eq!(
            engine.fleets.len(),
            1,
            "launch must target a legal enemy/neutral shore"
        );
        assert_eq!(engine.fleets[0].dst_tile, landing);
        (engine, landing)
    }

    fn finish_invasion(engine: &mut SowEngine) {
        for _ in 0..200 {
            engine.state.tick += 1;
            engine.execute_fleets();
            if engine.fleets.is_empty() {
                return;
            }
        }
        panic!("transport never arrived");
    }

    #[test]
    fn changed_landing_owner_never_retargets_the_invasion() {
        for original_target in [0, 3] {
            for new_owner in [2, 4] {
                let (mut engine, landing) = invasion_in_transit(original_target);
                engine.state.set_tile_owner(12, 2, new_owner);
                finish_invasion(&mut engine);
                assert_eq!(engine.state.map.owner_id(12, 2), 1);
                assert!(engine.state.events.iter().any(|event| matches!(event,
                    GameEvent::TileCaptured { x: 12, y: 2, previous_owner, .. }
                    if *previous_owner == new_owner
                )));
                assert_eq!(engine.attacks.len(), 1);
                let attack = &engine.attacks[0];
                assert_eq!(attack.target_owner, original_target);
                assert_eq!(attack.troops, 1_000.0);
                assert_eq!(attack.to_conquer.len(), 1);
                assert_eq!(
                    attack.to_conquer.peek().map(|tile| (tile.x, tile.y)),
                    Some((13, 2))
                );
                assert_eq!(engine.state.map.owner_id(landing % 20, landing / 20), 1);
                for _ in 0..200 {
                    engine.state.tick += 1;
                    engine.execute_combat();
                }
                assert_eq!(engine.state.map.owner_id(12, 3), 2);
                assert_eq!(engine.state.map.owner_id(13, 3), 2);
                assert_eq!(engine.state.map.owner_id(2, 2), original_target);
            }
        }
    }

    #[test]
    fn landing_without_original_target_neighbors_refunds_instead_of_using_a_distant_front() {
        for original_target in [0, 3] {
            let (mut engine, _) = invasion_in_transit(original_target);
            for x in 12..=14 {
                engine.state.set_tile_owner(x, 2, 2);
            }
            finish_invasion(&mut engine);
            assert_eq!(engine.state.map.owner_id(12, 2), 1);
            assert_eq!(engine.state.map.owner_id(2, 2), original_target);
            assert!(engine.attacks.is_empty());
            assert_eq!(engine.state.player(1).unwrap().troops, 10_000.0);
        }
    }

    #[test]
    fn allied_dead_or_missing_original_target_refunds_the_landed_troops() {
        for status in ["allied", "same_team", "dead", "missing"] {
            let (mut engine, _) = invasion_in_transit(3);
            match status {
                "allied" => engine.state.player_mut(3).unwrap().alliances.push(1),
                "same_team" => engine.state.player_mut(3).unwrap().team = Some(Team::Blue),
                "dead" => engine.state.player_mut(3).unwrap().alive = false,
                "missing" => engine.state.player_lookup[3] = None,
                _ => unreachable!(),
            }
            finish_invasion(&mut engine);
            assert_eq!(engine.state.map.owner_id(12, 2), 1, "{status}");
            assert_eq!(engine.state.map.owner_id(13, 2), 3, "{status}");
            assert!(engine.attacks.is_empty(), "{status}");
            assert_eq!(engine.state.player(1).unwrap().troops, 10_000.0, "{status}");
        }
    }

    #[test]
    fn new_landing_stays_separate_from_an_existing_land_attack() {
        let (mut engine, _) = invasion_in_transit(3);
        engine.apply_intents(&[crate::protocol::StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::Attack(crate::protocol::AttackIntent {
                target_owner: 3,
                troops: Some(500.0),
            }),
        }]);
        assert_eq!(engine.attacks.len(), 1);
        finish_invasion(&mut engine);
        assert_eq!(engine.attacks.len(), 2);
        assert_eq!(engine.attacks[0].troops, 500.0);
        assert_eq!(engine.attacks[1].troops, 1_000.0);
        assert!(
            engine.attacks[1]
                .to_conquer
                .iter()
                .all(|tile| (tile.x, tile.y) == (13, 2))
        );
    }

    #[test]
    fn destination_captured_by_the_sender_keeps_the_existing_return_penalty() {
        let (mut engine, _) = invasion_in_transit(3);
        engine.state.set_tile_owner(12, 2, 1);
        finish_invasion(&mut engine);
        assert!(engine.attacks.is_empty());
        assert_eq!(engine.state.player(1).unwrap().troops, 9_750.0);
    }

    #[test]
    fn naval_landing_can_finish_and_share_the_force_only_once() {
        let width = 64;
        let landing = 10 * width + 10;
        let mut state = GameState::new(
            5,
            width,
            32,
            GameConfig {
                random_spawn: true,
                tick_rate_ms: 100.0,
                global_speed_multiplier: 0.0,
                ..Default::default()
            },
        );
        state.phase = crate::game::GamePhase::Playing;
        state.tick = 1_800;
        state.map.terrain.fill(MapTile::from_byte(0x80));
        for id in 1..=4 {
            let mut player = Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
            player.max_troops = 20_000.0;
            state.register_player(player);
        }
        state.player_mut(4).unwrap().troops = 0.0;
        for (x, y, owner) in [
            (9, 10, 1),
            (19, 10, 3),
            (29, 10, 4),
            (10, 10, 2),
            (11, 10, 2),
            (18, 10, 2),
        ] {
            state.set_tile_owner(x, y, owner);
        }
        for index in 0..10 {
            state.set_tile_owner(24 + index * 3, 20, 2);
        }

        let mut engine = SowEngine::new(state, WaterComponents::default());
        for (id, owner_id, troops, target_tile) in
            [(10, 1, 300.0, (11, 10)), (11, 3, 300.0, (18, 10))]
        {
            engine.add_attack(AttackExecution {
                id,
                owner_id,
                target_owner: 2,
                created_tick: 0,
                troops,
                to_conquer: BinaryHeap::from([PrioritizedTile {
                    priority: 0,
                    insert_seq: 0,
                    x: target_tile.0,
                    y: target_tile.1,
                }]),
                insert_seq_counter: 1,
                rng: wyrand::WyRand::new(id),
                retreating: false,
                landed: false,
            });
        }
        engine.add_fleet(WarpFleet::new(
            1,
            1,
            2,
            UnitType::TransportShip,
            600.0,
            (3, landing),
            vec![landing],
        ));
        engine.add_fleet(WarpFleet::new(
            2,
            4,
            2,
            UnitType::TransportShip,
            50.0,
            (4, landing),
            vec![4, landing],
        ));

        engine.execute_fleets();

        assert!(!engine.state.player(2).unwrap().alive);
        assert_eq!(engine.state.player(1).unwrap().tile_count, 11);
        assert_eq!(engine.state.player(3).unwrap().tile_count, 4);
        assert_eq!(engine.state.player(1).unwrap().kills, 1);
        assert_eq!(engine.state.player(3).unwrap().assists, 1);
        assert_eq!(engine.state.player(4).unwrap().troops, 50.0);
        assert!(engine.attacks.is_empty());
        assert!(engine.fleets.is_empty());
        let captures = engine
            .state
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::TileCaptured {
                    new_owner,
                    previous_owner: 2,
                    ..
                } => Some(*new_owner),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(captures.iter().filter(|&&owner| owner == 1).count(), 10);
        assert_eq!(captures.iter().filter(|&&owner| owner == 3).count(), 3);
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::PlayerEliminated { player_id: 2, .. }))
                .count(),
            1
        );
    }

    #[test]
    fn explicit_campaign_assault_preserves_its_friendly_target_exception() {
        let (mut engine, _) = invasion_in_transit(3);
        engine.state.player_mut(3).unwrap().alliances.push(1);
        engine.campaign_assault = Some(crate::engine::CampaignAssaultState {
            root_target: 3,
            team: Team::Blue,
            include_allies: false,
            preserve_relation: true,
            hold_last_tile: false,
            capacity_ratio: 1.0,
            interval_ticks: 6_000,
            next_tick: 6_000,
            phase_index: 0,
            attacker_ids: vec![1],
            target_ids: vec![3],
            wave_index: 0,
            cycle_released_troops: Default::default(),
            target_last_dispatched_wave: Default::default(),
            focus_targets: Default::default(),
            reserve_spawned: false,
        });
        finish_invasion(&mut engine);
        assert_eq!(engine.attacks.len(), 1);
        assert_eq!(engine.attacks[0].target_owner, 3);
        assert_eq!(engine.attacks[0].troops, 1_000.0);
    }

    #[test]
    fn landing_on_last_tile_records_one_defeat_for_the_fleet_owner() {
        for has_other_tile in [false, true] {
            let mut state = GameState::new(
                1,
                5,
                3,
                GameConfig {
                    random_spawn: true,
                    ..Default::default()
                },
            );
            state.map.terrain.fill(MapTile::from_byte(0x20));
            for id in [1, 2] {
                let mut player = Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
                player.gold = 0.0;
                player.has_spawned = true;
                state.register_player(player);
            }
            for (x, y, owner) in [(0, 1, 1), (4, 1, 2)] {
                state.map.terrain[(y * 5 + x) as usize] = MapTile::from_byte(0xC0);
                state.set_tile_owner(x, y, owner);
            }
            if has_other_tile {
                state.map.terrain[14] = MapTile::from_byte(0xC0);
                state.set_tile_owner(4, 2, 2);
            }
            let mut engine = SowEngine::new(state, WaterComponents::default());
            // The route is already at its final shore tile; exercise the real landing path.
            engine.add_fleet(WarpFleet::new(
                1,
                1,
                2,
                UnitType::TransportShip,
                50.0,
                (5, 9),
                vec![9],
            ));
            engine.execute_fleets();
            let gold_after_landing = engine.state.player(1).unwrap().gold;
            engine.execute_fleets();

            let killer = engine.state.player(1).unwrap();
            let victim = engine.state.player(2).unwrap();
            assert_eq!(engine.state.map.owner_id(4, 1), 1);
            assert_eq!(victim.alive, has_other_tile);
            assert_eq!(victim.deaths, u32::from(!has_other_tile));
            assert_eq!(killer.kills, u32::from(!has_other_tile));
            assert_eq!(killer.gold, gold_after_landing);
            assert_eq!(killer.gold > 0.0, !has_other_tile);
            let defeats = engine
                .state
                .events
                .iter()
                .filter_map(|event| match event {
                    GameEvent::PlayerEliminated {
                        player_id,
                        conqueror_id,
                        elimination_x,
                        elimination_y,
                        ..
                    } => Some((*player_id, *conqueror_id, *elimination_x, *elimination_y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                defeats,
                if has_other_tile {
                    vec![]
                } else {
                    vec![(2, 1, 4, 1)]
                }
            );
            assert!(engine.fleets.is_empty());
        }
    }

    #[test]
    fn enclosure_eliminations_are_credited_once_and_never_capture_teammates() {
        for by_fleet in [false, true] {
            for same_team in [false, true] {
                let mut state = GameState::new(
                    1,
                    5,
                    5,
                    GameConfig {
                        random_spawn: true,
                        global_speed_multiplier: 100.0,
                        ..Default::default()
                    },
                );
                state.map.terrain.fill(MapTile::from_byte(0x20));

                for (id, team) in [
                    (1, Team::Blue),
                    (2, if same_team { Team::Blue } else { Team::Red }),
                ] {
                    let mut player =
                        Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
                    player.gold = 0.0;
                    player.has_spawned = true;
                    player.team = Some(team);
                    state.register_player(player);
                }

                // P1 surrounds P2 except for one P2 land tile to be captured.
                // Capturing the victim's tile triggers OpenFront's check.
                for y in 1..=3 {
                    for x in 1..=3 {
                        if (x, y) != (2, 1) && (x, y) != (2, 2) {
                            state.map.terrain[(y * 5 + x) as usize] = MapTile::from_byte(0x80);
                            state.set_tile_owner(x, y, 1);
                        }
                    }
                }
                state.map.terrain[2 * 5 + 2] = MapTile::from_byte(0x80);
                state.set_tile_owner(2, 2, 2);
                state.map.terrain[1 * 5 + 2] = MapTile::from_byte(0x80); // (x=2, y=1): row-major
                state.set_tile_owner(2, 1, 2);
                state.map.compute_shorelines();

                let mut engine = SowEngine::new(state, WaterComponents::default());
                if same_team {
                    // A direct defender change triggers the enclosure check,
                    // which must leave the still-owned core with its teammate.
                    let (_, change) = engine.state.set_tile_owner_for_execution(2, 1, 1);
                    assert!(
                        engine
                            .annex_enclosed_components(change.into_iter().collect())
                            .is_empty()
                    );
                } else if by_fleet {
                    // The fleet is already at the victim-owned landing tile.
                    engine.add_fleet(WarpFleet::new(
                        1,
                        1,
                        2,
                        UnitType::TransportShip,
                        100.0,
                        (5, 7),
                        vec![7],
                    ));
                    engine.execute_fleets();
                    engine.execute_fleets();
                } else {
                    engine.add_attack(AttackExecution {
                        landed: false,
                        id: 1,
                        owner_id: 1,
                        target_owner: 2,
                        created_tick: 0,
                        troops: 100_000.0,
                        to_conquer: BinaryHeap::from([PrioritizedTile {
                            priority: 0,
                            insert_seq: 0,
                            x: 2,
                            y: 1,
                        }]),
                        insert_seq_counter: 1,
                        rng: wyrand::WyRand::new(1),
                        retreating: false,
                    });
                    engine.execute_combat();
                    engine.execute_combat();
                }

                let eliminated = !same_team;
                assert_eq!(engine.state.map.owner_id(2, 1), 1);
                assert_eq!(
                    engine.state.map.owner_id(2, 2),
                    if eliminated { 1 } else { 2 }
                );
                let attacker = engine.state.player(1).unwrap();
                let victim = engine.state.player(2).unwrap();
                assert_eq!(victim.alive, !eliminated);
                assert_eq!(victim.deaths, u32::from(eliminated));
                assert_eq!(attacker.kills, u32::from(eliminated));
                assert_eq!(attacker.gold > 0.0, eliminated);

                let defeats = engine
                    .state
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        GameEvent::PlayerEliminated {
                            player_id,
                            conqueror_id,
                            elimination_x,
                            elimination_y,
                            ..
                        } => Some((*player_id, *conqueror_id, *elimination_x, *elimination_y)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    defeats,
                    if eliminated {
                        vec![(2, 1, 2, 2)]
                    } else {
                        vec![]
                    },
                    "by_fleet={by_fleet}, same_team={same_team}"
                );
            }
        }
    }

    #[test]
    fn land_and_fleet_conquests_annex_a_sealed_multi_tile_region() {
        const W: u32 = 7;
        for by_fleet in [false, true] {
            let mut state = GameState::new(
                2,
                W,
                W,
                GameConfig {
                    random_spawn: true,
                    global_speed_multiplier: 100.0,
                    ..Default::default()
                },
            );
            state.map.terrain.fill(MapTile::from_byte(0x20));
            for (id, team) in [(1, Team::Blue), (2, Team::Red)] {
                let mut player = Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
                player.gold = 0.0;
                player.has_spawned = true;
                player.team = Some(team);
                state.register_player(player);
            }

            // P1 owns the ring except for one coastal landing tile. P2 owns
            // that tile and the connected inland 2x2 enclave; taking the
            // victim's tile triggers the same owner check as OpenFront.
            for y in 1..=4 {
                for x in 1..=4 {
                    let enclave = (2..=3).contains(&x) && (2..=3).contains(&y);
                    if !enclave && (x, y) != (2, 1) {
                        state.map.terrain[(y * W + x) as usize] = MapTile::from_byte(0x80);
                        state.set_tile_owner(x, y, 1);
                    }
                }
            }
            for y in 2..=3 {
                for x in 2..=3 {
                    state.map.terrain[(y * W + x) as usize] = MapTile::from_byte(0x80);
                    state.set_tile_owner(x, y, 2);
                }
            }
            state.map.terrain[W as usize + 2] = MapTile::from_byte(0x80);
            state.set_tile_owner(2, 1, 2);
            state.map.compute_shorelines();

            let mut engine = SowEngine::new(state, WaterComponents::default());
            if by_fleet {
                let gap = W + 2;
                engine.add_fleet(WarpFleet::new(
                    1,
                    1,
                    2,
                    UnitType::TransportShip,
                    100.0,
                    (5, gap),
                    vec![gap],
                ));
                engine.execute_fleets();
            } else {
                engine.add_attack(AttackExecution {
                    landed: false,
                    id: 1,
                    owner_id: 1,
                    target_owner: 2,
                    created_tick: 0,
                    troops: 100_000.0,
                    to_conquer: BinaryHeap::from([PrioritizedTile {
                        priority: 0,
                        insert_seq: 0,
                        x: 2,
                        y: 1,
                    }]),
                    insert_seq_counter: 1,
                    rng: wyrand::WyRand::new(2),
                    retreating: false,
                });
                engine.execute_combat();
            }

            for y in 2..=3 {
                for x in 2..=3 {
                    assert_eq!(engine.state.map.owner_id(x, y), 1, "by_fleet={by_fleet}");
                }
            }
            assert_eq!(engine.state.player(2).unwrap().tile_count, 0);
            assert_eq!(engine.state.player(2).unwrap().deaths, 1);
            assert_eq!(engine.state.player(1).unwrap().kills, 1);
            assert_eq!(engine.state.player(1).unwrap().gold, 300.0);
            assert_eq!(
                engine
                    .state
                    .events
                    .iter()
                    .filter(|event| matches!(event, GameEvent::TileCaptured { .. }))
                    .count(),
                5,
                "the gap and four enclave tiles must each be credited once"
            );
            assert_eq!(
                engine
                    .state
                    .events
                    .iter()
                    .filter(|event| matches!(
                        event,
                        GameEvent::PlayerEliminated { player_id: 2, .. }
                    ))
                    .count(),
                1,
                "enclosed transfers must eliminate the defender once"
            );
        }
    }

    fn enclosure_fixture(case: &str) -> (SowEngine, (u32, u32), (u32, u32)) {
        const W: u32 = 7;
        let mut state = GameState::new(
            3,
            W,
            W,
            GameConfig {
                random_spawn: true,
                ..Default::default()
            },
        );
        let mut captor = if case == "vanilla" {
            Player::new_bot(1, "Tribe".into(), [1.0; 3], &state.config)
        } else {
            Player::new_human(1, "Captor".into(), [1.0; 3], &state.config)
        };
        let mut victim = Player::new_human(2, "Victim".into(), [1.0; 3], &state.config);
        if case == "same-team" {
            captor.team = Some(Team::Blue);
            victim.team = Some(Team::Blue);
        } else if case == "allied" {
            captor.alliances.push(2);
        }
        state.register_player(captor);
        state.register_player(victim);
        if case == "multiple-captors" {
            state.register_player(Player::new_human(
                3,
                "Other captor".into(),
                [1.0; 3],
                &state.config,
            ));
        }

        let (gap, core) = if case == "map-edge" {
            state.map.terrain.fill(MapTile::from_byte(0x80));
            state.set_tile_owner(0, 3, 2);
            for (x, y) in state.map.neighbors(0, 3) {
                if (x, y) != (1, 3) {
                    state.set_tile_owner(x, y, 1);
                }
            }
            ((1, 3), (0, 3))
        } else {
            state.map.terrain.fill(MapTile::from_byte(0x20));
            let gap = if case == "water-boundary" {
                (1, 2)
            } else {
                (2, 1)
            };
            for y in 1..=4 {
                for x in 1..=4 {
                    let core_tile = (2..=3).contains(&x) && (2..=3).contains(&y);
                    if core_tile || (x, y) == gap {
                        continue;
                    }
                    if case == "open-neutral" && (x, y) == (3, 1) {
                        state.map.terrain[(y * W + x) as usize] = MapTile::from_byte(0x80);
                        continue;
                    }
                    let water_boundary = case == "water-boundary" && (x, y) == (2, 1);
                    state.map.terrain[(y * W + x) as usize] =
                        MapTile::from_byte(if water_boundary { 0x20 } else { 0x80 });
                    if water_boundary {
                        continue;
                    }
                    state.set_tile_owner(
                        x,
                        y,
                        if case == "multiple-captors" && (x, y) == (1, 2) {
                            3
                        } else {
                            1
                        },
                    );
                }
            }
            for y in 2..=3 {
                for x in 2..=3 {
                    state.map.terrain[(y * W + x) as usize] = MapTile::from_byte(0x80);
                    state.set_tile_owner(x, y, 2);
                }
            }
            state.map.terrain[(gap.1 * W + gap.0) as usize] = MapTile::from_byte(0x80);
            (gap, (2, 2))
        };
        state.map.compute_shorelines();
        (SowEngine::new(state, WaterComponents::default()), gap, core)
    }

    #[test]
    fn open_coastal_edge_mixed_captor_and_allied_regions_are_not_annexed() {
        for case in [
            "open-neutral",
            "water-boundary",
            "map-edge",
            "multiple-captors",
            "same-team",
            "allied",
        ] {
            for direct_loss in [false, true] {
                let (mut engine, gap, core) = enclosure_fixture(case);
                let trigger = if direct_loss {
                    engine.state.map.terrain[5 * 7 + 5] = MapTile::from_byte(0x80);
                    engine.state.set_tile_owner(5, 5, 2);
                    engine.state.set_tile_owner_for_execution(5, 5, 1).1
                } else {
                    None
                };
                let (_, close_gap) = engine.state.set_tile_owner_for_execution(gap.0, gap.1, 1);
                engine.annex_enclosed_components(trigger.into_iter().chain(close_gap).collect());
                assert_eq!(
                    engine.state.map.owner_id(core.0, core.1),
                    2,
                    "invalid enclosure was annexed: {case}, direct_loss={direct_loss}"
                );
            }
        }
    }

    #[test]
    fn neutral_land_and_fleet_closures_annex_all_player_types_once() {
        use crate::game_config::BotDifficulty::{Terminator, Vanilla};

        for difficulty in [Vanilla, Terminator] {
            for victim_type in [PlayerType::Human, PlayerType::Nation, PlayerType::Bot] {
                for by_fleet in [false, true] {
                    let (mut engine, gap, _) = enclosure_fixture("vanilla");
                    engine.state.phase = crate::game::GamePhase::Playing;
                    engine.state.config.bot_difficulty = difficulty;
                    engine.state.config.global_speed_multiplier = 100.0;
                    engine.state.player_mut(1).unwrap().gold = 0.0;
                    engine.state.player_mut(2).unwrap().player_type = victim_type;
                    if by_fleet {
                        let tile = gap.1 * engine.state.map.width + gap.0;
                        engine.add_fleet(WarpFleet::new(
                            1,
                            1,
                            0,
                            UnitType::TransportShip,
                            100.0,
                            (5, tile),
                            vec![tile],
                        ));
                        engine.execute_fleets();
                    } else {
                        engine.add_attack(AttackExecution {
                            landed: false,
                            id: 1,
                            owner_id: 1,
                            target_owner: 0,
                            created_tick: 0,
                            troops: 100_000.0,
                            to_conquer: BinaryHeap::from([PrioritizedTile {
                                priority: 0,
                                insert_seq: 0,
                                x: gap.0,
                                y: gap.1,
                            }]),
                            insert_seq_counter: 1,
                            rng: wyrand::WyRand::new(2),
                            retreating: false,
                        });
                        engine.execute_combat();
                    }
                    for y in 2..=3 {
                        for x in 2..=3 {
                            assert_eq!(engine.state.map.owner_id(x, y), 1);
                        }
                    }
                    let victim = engine.state.player(2).unwrap();
                    assert!(
                        !victim.alive,
                        "{difficulty:?}/{victim_type:?}/fleet={by_fleet}"
                    );
                    assert_eq!(victim.tile_count, 0);
                    assert_eq!(victim.deaths, 1);
                    let captor = engine.state.player(1).unwrap();
                    assert_eq!(captor.kills, 1);
                    assert_eq!(
                        captor.gold,
                        match victim_type {
                            PlayerType::Human => 300.0,
                            PlayerType::Nation => 150.0,
                            PlayerType::Bot => 50.0,
                        }
                    );
                    assert_eq!(
                        engine
                            .state
                            .events
                            .iter()
                            .filter(|event| matches!(event, GameEvent::TileCaptured { .. }))
                            .count(),
                        5,
                    );
                    assert_eq!(
                        engine
                            .state
                            .events
                            .iter()
                            .filter(|event| matches!(
                                event,
                                GameEvent::PlayerEliminated { player_id: 2, .. }
                            ))
                            .count(),
                        1,
                    );
                }
            }
        }
    }

    #[test]
    fn neutral_closure_only_transfers_the_enclosed_component() {
        let (mut engine, gap, core) = enclosure_fixture("vanilla");
        engine.state.map.terrain[5 * 7 + 5] = MapTile::from_byte(0x80);
        engine.state.set_tile_owner(5, 5, 2);
        engine.state.map.compute_shorelines();
        let (_, change) = engine.state.set_tile_owner_for_execution(gap.0, gap.1, 1);

        assert!(
            engine
                .annex_enclosed_components(change.into_iter().collect())
                .is_empty()
        );
        assert_eq!(engine.state.map.owner_id(core.0, core.1), 1);
        assert_eq!(engine.state.map.owner_id(5, 5), 2);
        assert_eq!(engine.state.player(2).unwrap().tile_count, 1);
        assert!(engine.state.player(2).unwrap().alive);
        assert_eq!(engine.state.player(1).unwrap().kills, 0);
    }

    #[test]
    fn neutral_batches_are_deterministic_and_leave_unrelated_owners_alone() {
        let mut state = GameState::new(2, 15, 15, GameConfig::default());
        state.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2, 65, 4095] {
            state.register_player(Player::new_human(
                id,
                format!("P{id}"),
                [1.0; 3],
                &state.config,
            ));
        }
        for (cx, cy, victim) in [(3, 3, 2), (7, 7, 4095), (11, 11, 65)] {
            for y in cy - 1..=cy + 1 {
                for x in cx - 1..=cx + 1 {
                    if victim != 65 && [(cx, cy - 1), (cx - 1, cy)].contains(&(x, y)) {
                        continue;
                    }
                    state.set_tile_owner(x, y, if (x, y) == (cx, cy) { victim } else { 1 });
                }
            }
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        let mut changes = Vec::new();
        // Each victim is touched twice. The already sealed, unrelated owner must not be scanned.
        for (x, y) in [(7, 6), (3, 2), (6, 7), (2, 3)] {
            changes.extend(engine.state.set_tile_owner_for_execution(x, y, 1).1);
        }
        let mut reversed = engine.clone();
        let mut reverse_changes = changes.clone();
        reverse_changes.reverse();
        let eliminated = engine.annex_enclosed_components(changes.clone());
        assert_eq!(
            eliminated.iter().map(|entry| entry.0).collect::<Vec<_>>(),
            vec![2, 4095]
        );
        assert_eq!(
            reversed.annex_enclosed_components(reverse_changes),
            eliminated
        );
        assert_eq!(engine.state.events, reversed.state.events);
        assert_eq!(
            engine.state.map.owner_states(),
            reversed.state.map.owner_states()
        );
        assert_eq!(engine.state.map.owner_id(11, 11), 65);
        assert!(engine.annex_enclosed_components(changes).is_empty());
        assert_eq!(engine.state.events.len(), 2);
    }

    #[test]
    fn neutral_closure_respects_campaign_holds_and_relations() {
        for protection in ["assault", "watched", "pending", "neutral", "allied"] {
            let (mut engine, gap, core) = enclosure_fixture("vanilla");
            match protection {
                "assault" => {
                    engine.campaign_assault = Some(crate::engine::CampaignAssaultState {
                        root_target: 2,
                        team: Team::Red,
                        include_allies: true,
                        preserve_relation: false,
                        hold_last_tile: true,
                        capacity_ratio: 2.0,
                        interval_ticks: 100,
                        next_tick: 100,
                        phase_index: 1,
                        attacker_ids: vec![1],
                        target_ids: vec![2],
                        wave_index: 0,
                        cycle_released_troops: Default::default(),
                        target_last_dispatched_wave: Default::default(),
                        focus_targets: Default::default(),
                        reserve_spawned: false,
                    });
                }
                "watched" => {
                    engine.campaign_pre_defeat_targets.insert(2);
                }
                "pending" => engine.campaign_pre_defeat_pending = Some(2),
                "neutral" | "allied" => {
                    engine.campaign_relations.insert(
                        1,
                        if protection == "neutral" {
                            crate::protocol::CampaignRelation::Neutral
                        } else {
                            crate::protocol::CampaignRelation::Allied
                        },
                    );
                }
                _ => unreachable!(),
            }
            let (_, change) = engine.state.set_tile_owner_for_execution(gap.0, gap.1, 1);
            assert!(
                engine
                    .annex_enclosed_components(change.into_iter().collect())
                    .is_empty()
            );
            assert_eq!(engine.state.map.owner_id(core.0, core.1), 2, "{protection}");
            assert_eq!(
                engine.state.player(2).unwrap().tile_count,
                4,
                "{protection}"
            );

            engine.campaign_assault = None;
            engine.campaign_pre_defeat_targets.clear();
            engine.campaign_pre_defeat_pending = None;
            engine
                .campaign_relations
                .insert(1, crate::protocol::CampaignRelation::Enemy);
            let (_, change) = engine.state.set_tile_owner_for_execution(core.0, core.1, 1);
            assert_eq!(
                engine
                    .annex_enclosed_components(change.into_iter().collect())
                    .len(),
                1
            );
            assert_eq!(engine.state.player(2).unwrap().tile_count, 0);
        }
    }

    #[test]
    fn losing_land_rechecks_the_victims_other_enclosed_regions() {
        const W: u32 = 11;
        let mut state = GameState::new(2, W, W, GameConfig::default());
        state.map.terrain.fill(MapTile::from_byte(0x80));
        state.register_player(Player::new_human(
            1,
            "Captor".into(),
            [1.0; 3],
            &state.config,
        ));
        state.register_player(Player::new_human(
            2,
            "Victim".into(),
            [1.0; 3],
            &state.config,
        ));

        // A one-tile enclave is already sealed far from the region where the
        // victim is about to lose land.
        for y in 2..=4 {
            for x in 2..=4 {
                if (x, y) != (3, 3) {
                    state.set_tile_owner(x, y, 1);
                }
            }
        }
        state.set_tile_owner(3, 3, 2);
        state.set_tile_owner(7, 5, 2);
        state.set_tile_owner(8, 5, 2);
        state.map.compute_shorelines();

        let mut engine = SowEngine::new(state, WaterComponents::default());
        let (_, change) = engine.state.set_tile_owner_for_execution(7, 5, 1);
        engine.annex_enclosed_components(change.into_iter().collect());

        assert_eq!(engine.state.map.owner_id(3, 3), 1);
        assert_eq!(engine.state.map.owner_id(8, 5), 2);
    }

    #[test]
    fn enclosed_unowned_hole_does_not_keep_the_defenders_land_open() {
        const W: u32 = 13;
        let mut state = GameState::new(2, W, W, GameConfig::default());
        state.map.terrain.fill(MapTile::from_byte(0x80));
        state.register_player(Player::new_human(
            1,
            "Captor".into(),
            [1.0; 3],
            &state.config,
        ));
        state.register_player(Player::new_human(
            2,
            "Victim".into(),
            [1.0; 3],
            &state.config,
        ));

        for y in 2..=10 {
            for x in 2..=10 {
                if x == 2 || y == 2 || x == 10 || y == 10 {
                    state.set_tile_owner(x, y, 1);
                } else if (x, y) != (6, 6) {
                    state.set_tile_owner(x, y, 2);
                }
            }
        }
        state.set_tile_owner(11, 5, 2);
        state.map.compute_shorelines();

        let mut engine = SowEngine::new(state, WaterComponents::default());
        let (_, change) = engine.state.set_tile_owner_for_execution(11, 5, 1);
        let eliminated = engine.annex_enclosed_components(change.into_iter().collect());

        assert_eq!(engine.state.map.owner_id(6, 5), 1);
        assert_eq!(engine.state.map.owner_id(6, 6), 0);
        assert_eq!(eliminated.len(), 1);
        assert_eq!(eliminated[0].0, 2);
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::TileCaptured { .. }))
                .count(),
            48
        );
    }

    #[test]
    fn fractional_extra_tile_milli_threshold_is_stable() {
        // 12.499 -> threshold 499 (roll 498 adds one; 499 does not)
        assert_eq!(fractional_extra_tiles_milli(12.499, 498), 1);
        assert_eq!(fractional_extra_tiles_milli(12.499, 499), 0);
        // Very small fraction below one milli should never add.
        assert_eq!(fractional_extra_tiles_milli(7.0009, 0), 0);
        // Near-one fraction clamps to 999 max threshold.
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 998), 1);
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 999), 0);
    }
}
