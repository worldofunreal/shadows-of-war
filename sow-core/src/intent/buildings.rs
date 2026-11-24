use crate::building::{
    Building, CityArsenalBuild, city_arsenal_cost_gold,
    structure_max_affordable_upgrade, structure_multi_upgrade_cost_and_ticks,
    structure_upgrade_cost_gold, validate_structure_site,
};
use crate::engine::SowEngine;
use crate::game::{BuildingKind, CityArsenalKind, GameEvent, GamePhase, NavalActionFailure, UnitType};

impl SowEngine {
    pub(crate) fn foundation_quote(
        &self,
        owner_id: u16,
        kind: BuildingKind,
    ) -> Option<(u8, f64, u32)> {
        let city_level = self.building_aggregates.get(usize::from(owner_id))
            .map_or(0, |aggregate| aggregate.highest_city_level);
        let level = if self.campaign_unlocks.is_some() {
            1
        } else {
            kind.foundation_level(city_level)
        };
        if !self.campaign_allows_building(kind, level) {
            return None;
        }
        let (cost, ticks) = crate::building::cost::structure_foundation_quote(
            kind, level, self.paid_kind_levels(owner_id, kind), &self.state.config,
        )?;
        Some((level, cost, ticks))
    }

    /// Highest level currently owned or already paid/queued for `building`.
    #[inline]
    pub fn building_reserved_level(&self, building: Building) -> u8 {
        self.upgrade_targets
            .get(&building.id)
            .copied()
            .map_or(building.level, |target| target.max(building.level))
    }

    /// Lifetime levels of this kind that the player constructed or paid to upgrade.
    /// Captured structures are deliberately excluded.
    pub(crate) fn paid_kind_levels(&self, owner_id: u16, kind: BuildingKind) -> u32 {
        self.state
            .player(owner_id)
            .map_or(0, |player| player.structure_levels_built[kind as usize])
    }

    fn tutorial_city_upgrade_free_available(&self, player_id: u16) -> bool {
        if !self.state.config.tutorial
            || !self.state.config.buildings_enabled
            || self.state.config.player_leader.id() != "boudica"
        {
            return false;
        }
        !self.buildings.iter().any(|candidate| {
            candidate.owner_id == player_id
                && candidate.kind == BuildingKind::City
                && self.building_reserved_level(*candidate) >= 2
        })
    }

    /// Boudica's tutorial grants one free City level-2 upgrade.
    pub fn tutorial_city_upgrade_is_free(&self, player_id: u16, building_id: u64) -> bool {
        if !self.tutorial_city_upgrade_free_available(player_id) {
            return false;
        }
        let Some(building) = self
            .buildings
            .iter()
            .find(|building| building.id == building_id && building.owner_id == player_id)
        else {
            return false;
        };
        building.kind == BuildingKind::City
            && building.level == 1
            && !building.under_construction
    }

    /// Shared price for the confirmation UI and authoritative upgrade intent.
    pub fn structure_upgrade_quote(&self, building_id: u64) -> Option<(f64, bool)> {
        let index = self
            .buildings
            .binary_search_by_key(&building_id, |building| building.id)
            .ok()?;
        let building = self.buildings[index];
        Some(
            if self.tutorial_city_upgrade_is_free(building.owner_id, building_id) {
                (0.0, false)
            } else {
                self.paid_structure_upgrade_quote(building)
            },
        )
    }

    /// Cumulative `(cost, total_ticks, any_hero_discount)` to upgrade `building_id` from its
    /// current reserved level through `target_level`.
    pub fn structure_upgrade_to_quote(
        &self,
        building_id: u64,
        target_level: u8,
    ) -> Option<(f64, u32, bool)> {
        let index = self
            .buildings
            .binary_search_by_key(&building_id, |building| building.id)
            .ok()?;
        let building = self.buildings[index];
        if building.active_level() < 1 && building.under_construction {
            return None;
        }
        let from_level = self.building_reserved_level(building);
        if target_level <= from_level
            || target_level > building.kind.max_level()
            || !self.campaign_allows_building(building.kind, target_level)
        {
            return None;
        }
        let built_levels = self.paid_kind_levels(building.owner_id, building.kind);
        let free_lv2 = building.kind == BuildingKind::City
            && from_level < 2
            && self.tutorial_city_upgrade_free_available(building.owner_id);
        let discount = self.hero_city_upgrade_discount(building);
        structure_multi_upgrade_cost_and_ticks(
            building.kind,
            from_level,
            target_level,
            built_levels,
            free_lv2,
            discount,
            self.state.config.tutorial,
            &self.state.config,
        )
    }

    /// Highest level in `(reserved + 1)..=cap` affordable with `gold`: `(target_level, cost, ticks)`.
    pub fn max_affordable_upgrade_level(
        &self,
        building_id: u64,
        gold: f64,
        campaign_max_level: u8,
    ) -> Option<(u8, f64, u32)> {
        let index = self
            .buildings
            .binary_search_by_key(&building_id, |building| building.id)
            .ok()?;
        let building = self.buildings[index];
        if building.active_level() < 1 && building.under_construction {
            return None;
        }
        let from_level = self.building_reserved_level(building);
        let mut max_allowed = campaign_max_level.min(building.kind.max_level());
        while max_allowed > from_level && !self.campaign_allows_building(building.kind, max_allowed)
        {
            max_allowed -= 1;
        }
        let built_levels = self.paid_kind_levels(building.owner_id, building.kind);
        let free_lv2 = building.kind == BuildingKind::City
            && from_level < 2
            && self.tutorial_city_upgrade_free_available(building.owner_id);
        let discount = self.hero_city_upgrade_discount(building);
        structure_max_affordable_upgrade(
            building.kind,
            from_level,
            max_allowed,
            built_levels,
            free_lv2,
            discount,
            self.state.config.tutorial,
            gold,
            &self.state.config,
        )
    }

    pub(super) fn paid_structure_upgrade_quote(&self, building: Building) -> (f64, bool) {
        let built_levels = self.paid_kind_levels(building.owner_id, building.kind);
        let cost = structure_upgrade_cost_gold(
            building.kind,
            building.level.saturating_add(1),
            built_levels,
            &self.state.config,
        );
        let discount = self.hero_city_upgrade_discount(building);
        if discount > 0 && cost.is_finite() && cost > 0.0 {
            (cost * f64::from(100 - discount) / 100.0, true)
        } else {
            (cost, false)
        }
    }

    pub(super) fn apply_build_structure_intent(
        &mut self,
        player_id: u16,
        kind: BuildingKind,
        target_tile: u32,
    ) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        if !self.state.config.buildings_enabled || !kind.is_enabled() {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }
        self.refresh_building_aggregates();
        let Some((level, cost, dur)) = self.foundation_quote(player_id, kind) else {
            return;
        };
        let w = self.state.map.width;
        let area = w.saturating_mul(self.state.map.height);
        if area == 0 || target_tile >= area {
            return;
        }
        self.refresh_building_grid();

        if self.legacy_building_rules && kind == BuildingKind::MilitaryCamp {
            let map = &self.state.map;
            let x = target_tile % w;
            let y = target_tile / w;
            let footprint = crate::building::BuildingFootprint::legacy_at(x, y);
            if map.owner_id(x, y) != player_id
                || map.terrain_type(x, y) != crate::map::TerrainType::Land
                || !crate::building::footprint_fits(map, player_id, footprint)
                || self
                    .building_grid
                    .iter_all_in_range(x, y, 3)
                    .any(|building| {
                        footprint.intersects(crate::building::BuildingFootprint::legacy_at(
                            building.x, building.y,
                        ))
                    })
            {
                return;
            }
        }

        // Placement is intentionally separate from upgrades. A nearby building
        // never absorbs a new foundation order.
        let spawn_idx = if !self.legacy_building_rules {
            if validate_structure_site(
                &self.state.map,
                player_id,
                kind,
                target_tile,
                &self.building_grid,
            )
            .is_err()
            {
                return;
            }
            target_tile
        } else if kind == BuildingKind::MilitaryCamp {
            target_tile
        } else {
            let Some(spawn_idx) = crate::building::find_structure_site_near(
                &self.state.map,
                player_id,
                target_tile,
                kind,
                &self.building_grid,
                &mut self.placement_scratch,
                true,
            ) else {
                return;
            };
            spawn_idx
        };

        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);
        player_mut.structures_built = player_mut.structures_built.saturating_add(1);
        let built_levels = &mut player_mut.structure_levels_built[kind as usize];
        *built_levels = built_levels.saturating_add(u32::from(level));
        let id = self.state.next_building_id;
        self.state.next_building_id = self.state.next_building_id.wrapping_add(1).max(1);
        if level > 1 {
            self.upgrade_targets.insert(id, level);
        }
        self.add_building(Building {
            id,
            owner_id: player_id,
            tile_idx: spawn_idx,
            kind,
            level: 1,
            under_construction: true,
            ticks_until_complete: if level > 1 {
                let build_ticks = crate::building::structure_build_duration_ticks(kind);
                if self.state.config.tutorial {
                    (build_ticks / 2).max(1)
                } else {
                    build_ticks
                }
            } else {
                dur
            },
        });
        self.state.events.push(GameEvent::StructureSpawned {
            id,
            owner_id: player_id,
            tile_idx: spawn_idx,
            kind,
            level: 1,
        });
    }

    pub(super) fn apply_upgrade_structure_intent(&mut self, player_id: u16, building_id: u64) {
        self.apply_upgrade_structure_to_intent(player_id, building_id, None);
    }

    pub(super) fn apply_upgrade_structure_to_intent(
        &mut self,
        player_id: u16,
        building_id: u64,
        requested_target_level: Option<u8>,
    ) {
        if self.state.phase != GamePhase::Playing || !self.state.config.buildings_enabled {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }

        let Some(idx) = self
            .buildings
            .binary_search_by_key(&building_id, |b| b.id)
            .ok()
        else {
            return;
        };
        let building = self.buildings[idx];
        let extending_active_upgrade = requested_target_level.is_some()
            && building.under_construction
            && building.active_level() >= 1;
        if building.owner_id != player_id
            || !building.kind.is_enabled()
            || (building.under_construction && !extending_active_upgrade)
            || self
                .building_productions
                .get(&building.id)
                .is_some_and(|state| state.arsenal_build.is_some())
        {
            return;
        }
        let current_reserved = self.building_reserved_level(building);
        let target_level = requested_target_level.unwrap_or_else(|| current_reserved.saturating_add(1));
        if target_level <= current_reserved
            || target_level > building.kind.max_level()
            || !self.campaign_allows_building(building.kind, target_level)
        {
            return;
        }

        let levels_bought = u32::from(target_level.saturating_sub(current_reserved));
        let Some((cost, _total_ticks, discounted)) =
            self.structure_upgrade_to_quote(building_id, target_level)
        else {
            return;
        };
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);
        let built_levels = &mut player_mut.structure_levels_built[building.kind as usize];
        *built_levels = built_levels.saturating_add(levels_bought);

        if extending_active_upgrade {
            if target_level > self.buildings[idx].level {
                self.upgrade_targets.insert(building.id, target_level);
            }
            if discounted {
                let width = self.state.map.width;
                self.state.announce_leader_ability(
                    player_id,
                    Some((building.tile_idx % width, building.tile_idx / width)),
                );
            }
            return;
        }

        let step_level = building.level.saturating_add(1);
        let b = &mut self.buildings[idx];
        b.level = step_level;
        b.under_construction = true;
        let upgrade_ticks = crate::building::structure_upgrade_duration_ticks(b.kind, step_level);
        b.ticks_until_complete = if self.state.config.tutorial {
            (upgrade_ticks / 2).max(1)
        } else {
            upgrade_ticks
        };
        let b_id = b.id;
        let b_tile = b.tile_idx;
        let b_kind = b.kind;
        let b_level = b.level;
        if target_level > b_level {
            self.upgrade_targets.insert(b_id, target_level);
        } else {
            self.upgrade_targets.remove(&b_id);
        }
        if b_kind == BuildingKind::Tower {
            self.defense_grid_dirty = true;
        }
        self.building_aggregates_dirty = true;
        self.bot_sam_tiles_cache = None;
        if b_kind == BuildingKind::Port {
            self.sea_lanes_dirty = true;
        }
        self.state.events.push(GameEvent::StructureUpgraded {
            id: b_id,
            tile_idx: b_tile,
            kind: b_kind,
            level: b_level,
        });
        if discounted {
            let width = self.state.map.width;
            self.state.announce_leader_ability(
                player_id,
                Some((building.tile_idx % width, building.tile_idx / width)),
            );
        }
    }

    pub(super) fn apply_building_production_intent(
        &mut self,
        player_id: u16,
        building_id: u64,
        kind: CityArsenalKind,
    ) {
        self.queue_building_production(player_id, building_id, kind, None);
    }

    pub(super) fn apply_building_production_max_intent(
        &mut self,
        player_id: u16,
        building_id: u64,
        kind: CityArsenalKind,
    ) {
        if kind != CityArsenalKind::Warship {
            self.queue_building_production(player_id, building_id, kind, None);
            return;
        }
        let Some(gold) = self.state.player(player_id).map(|p| p.gold) else {
            return;
        };
        let Some((count, _total_cost)) = self.max_affordable_warships(player_id, building_id, gold)
        else {
            // Trigger the canonical rejection event (CapacityFull, QueueFull, InsufficientGold, etc.)
            self.queue_building_production(player_id, building_id, kind, None);
            return;
        };
        for _ in 0..count {
            if !self.queue_building_production(player_id, building_id, kind, None) {
                break;
            }
        }
    }

    pub(super) fn apply_cancel_warship_queue_intent(
        &mut self,
        player_id: u16,
        building_id: u64,
        index: u8,
    ) {
        if self.state.phase != GamePhase::Playing || !self.state.config.buildings_enabled {
            return;
        }
        let Some(building) = self.buildings.iter().find(|b| b.id == building_id) else {
            return;
        };
        if building.owner_id != player_id || building.kind != BuildingKind::Port {
            return;
        }
        let Some(state) = self.building_productions.get_mut(&building_id) else {
            return;
        };
        state.normalize_legacy_warship();
        let idx = usize::from(index);
        if idx >= state.warship_queue.len() {
            return;
        }
        let removed = state.warship_queue.remove(idx);
        let refund = removed.paid_gold();
        if refund.is_finite()
            && refund > 0.0
            && let Some(player_mut) = self.state.player_mut(player_id)
        {
            player_mut.gold += refund;
        }
    }

    /// Computes how many warships can be queued at `building_id` given `gold`, capacity, and
    /// per-port queue limits, returning `Some((count, total_cost))` if at least 1 fits.
    pub fn max_affordable_warships(
        &self,
        player_id: u16,
        building_id: u64,
        gold: f64,
    ) -> Option<(u8, f64)> {
        if self.state.phase != GamePhase::Playing
            || !self.state.config.buildings_enabled
            || !self.campaign_allows_action(crate::campaign::CampaignAction::Warship)
        {
            return None;
        }
        let player = self.state.player(player_id)?;
        if !player.alive {
            return None;
        }
        let building = self.buildings.iter().find(|b| b.id == building_id)?;
        if building.owner_id != player_id
            || building.under_construction
            || building.kind != BuildingKind::Port
            || building.active_level() < 1
        {
            return None;
        }
        crate::warp_fleet::coastal_warship_spawn_tile(
            &self.state.map,
            &self.water,
            building.tile_idx,
        )?;
        let queue_len = self
            .building_productions
            .get(&building_id)
            .map_or(0, crate::building::CityArsenalState::warship_queue_len);
        let queue_room = crate::building::MAX_WARSHIP_QUEUE.saturating_sub(queue_len);
        if queue_room == 0 {
            return None;
        }
        let capacity = crate::building::player_fleet_capacity(&self.buildings, player_id);
        let queued_total = self.queued_warships_for_player(player_id);
        let cap_room = capacity.saturating_sub(self.boat_slots_used(player_id).saturating_add(queued_total));
        let max_count = queue_room.min(cap_room as usize);
        if max_count == 0 {
            return None;
        }
        let active_warships = self
            .fleets
            .iter()
            .filter(|f| f.owner_id == player_id && f.unit_type == UnitType::Warship)
            .count() as u32;
        let mut owned_so_far = active_warships.saturating_add(queued_total);
        let mut remaining_gold = gold;
        let mut total_cost = 0.0;
        let mut count = 0u8;
        for _ in 0..max_count {
            let cost = city_arsenal_cost_gold(
                CityArsenalKind::Warship,
                owned_so_far,
                player.leader,
                &self.state.config,
            );
            if !cost.is_finite() || remaining_gold < cost {
                break;
            }
            remaining_gold -= cost;
            total_cost += cost;
            owned_so_far = owned_so_far.saturating_add(1);
            count = count.saturating_add(1);
        }
        (count > 0).then_some((count, total_cost))
    }

    fn queued_warships_for_player(&self, player_id: u16) -> u32 {
        self.building_productions
            .iter()
            .filter(|(building_id, _)| {
                self.buildings
                    .iter()
                    .any(|b| b.id == **building_id && b.owner_id == player_id)
            })
            .map(|(_, state)| state.warship_queue_len() as u32)
            .sum()
    }

    /// Old replays address a City tile; resolve their former co-located producer if present.
    pub(super) fn apply_city_arsenal_intent(
        &mut self,
        player_id: u16,
        tile_idx: u32,
        kind: CityArsenalKind,
    ) {
        let producer_kind = match kind {
            CityArsenalKind::Warship => BuildingKind::Port,
            CityArsenalKind::Nuke => return,
            CityArsenalKind::AntiNuke => return,
        };
        let Some(building_id) = self
            .buildings
            .iter()
            .find(|b| b.tile_idx == tile_idx && b.owner_id == player_id && b.kind == producer_kind)
            .map(|b| b.id)
        else {
            return;
        };
        self.queue_building_production(player_id, building_id, kind, None);
    }

    pub(super) fn apply_build_ship_intent(
        &mut self,
        player_id: u16,
        target_tile: u32,
        unit_type: UnitType,
    ) {
        if unit_type != UnitType::Warship {
            return;
        }
        if !self.campaign_allows_action(crate::campaign::CampaignAction::Warship) {
            self.state.events.push(GameEvent::NavalActionRejected {
                player_id,
                reason: NavalActionFailure::CampaignLocked,
            });
            return;
        }
        let route = match crate::warp_fleet::resolve_warship_route(
            &self.state.map,
            &self.water,
            &mut self.path_scratch,
            &self.buildings,
            player_id,
            target_tile,
        ) {
            Ok(route) => route,
            Err(error) => {
                let reason = match error {
                    crate::warp_fleet::WarshipLaunchError::InvalidTile => NavalActionFailure::InvalidTarget,
                    crate::warp_fleet::WarshipLaunchError::TargetNotWater => NavalActionFailure::TargetNotWater,
                    crate::warp_fleet::WarshipLaunchError::NoPort => NavalActionFailure::NoPort,
                    crate::warp_fleet::WarshipLaunchError::NoConnectedPort => NavalActionFailure::NoConnectedPort,
                    crate::warp_fleet::WarshipLaunchError::NoWaterPath => NavalActionFailure::NoWaterPath,
                };
                self.state.events.push(GameEvent::NavalActionRejected { player_id, reason });
                return;
            }
        };
        self.queue_building_production(
            player_id,
            route.port_id,
            CityArsenalKind::Warship,
            Some(route),
        );
    }

    fn queue_building_production(
        &mut self,
        player_id: u16,
        building_id: u64,
        kind: CityArsenalKind,
        warship_route: Option<crate::warp_fleet::WarshipRoute>,
    ) -> bool {
        if self.state.phase != GamePhase::Playing || !self.state.config.buildings_enabled {
            return false;
        }
        let Some(player) = self.state.player(player_id) else {
            return false;
        };
        if !player.alive {
            return false;
        }
        let leader = player.leader;
        let allowed = match kind {
            CityArsenalKind::Warship => {
                self.campaign_allows_action(crate::campaign::CampaignAction::Warship)
            }
            CityArsenalKind::Nuke => {
                self.campaign_allows_action(crate::campaign::CampaignAction::Nuke)
            }
            CityArsenalKind::AntiNuke => false,
        };
        if !allowed {
            if kind == CityArsenalKind::Warship {
                self.state.events.push(GameEvent::NavalActionRejected {
                    player_id,
                    reason: NavalActionFailure::CampaignLocked,
                });
            }
            return false;
        }

        let Some(building) = self.buildings.iter().find(|b| b.id == building_id).copied() else {
            return false;
        };
        if building.owner_id != player_id || building.under_construction {
            return false;
        }

        match kind {
            CityArsenalKind::Warship => {
                if building.kind != BuildingKind::Port || building.active_level() < 1 {
                    self.state.events.push(GameEvent::NavalActionRejected {
                        player_id,
                        reason: NavalActionFailure::NoPort,
                    });
                    return false;
                }
                if warship_route.is_none()
                    && crate::warp_fleet::coastal_warship_spawn_tile(
                        &self.state.map,
                        &self.water,
                        building.tile_idx,
                    )
                    .is_none()
                {
                    self.state.events.push(GameEvent::NavalActionRejected {
                        player_id,
                        reason: NavalActionFailure::NoConnectedPort,
                    });
                    return false;
                }
                let queue_len = self
                    .building_productions
                    .get(&building_id)
                    .map_or(0, crate::building::CityArsenalState::warship_queue_len);
                if queue_len >= crate::building::MAX_WARSHIP_QUEUE {
                    self.state.events.push(GameEvent::NavalActionRejected {
                        player_id,
                        reason: NavalActionFailure::QueueFull,
                    });
                    return false;
                }
                let capacity = crate::building::player_fleet_capacity(&self.buildings, player_id);
                let queued = self.queued_warships_for_player(player_id);
                if self.boat_slots_used(player_id).saturating_add(queued) >= capacity {
                    self.state.events.push(GameEvent::NavalActionRejected {
                        player_id,
                        reason: NavalActionFailure::CapacityFull,
                    });
                    return false;
                }
            }
            CityArsenalKind::Nuke | CityArsenalKind::AntiNuke => return false,
        }

        let queued_warships = self.queued_warships_for_player(player_id);
        let owned_warships = (self
            .fleets
            .iter()
            .filter(|f| f.owner_id == player_id && f.unit_type == UnitType::Warship)
            .count() as u32)
            .saturating_add(queued_warships);
        let cost = city_arsenal_cost_gold(kind, owned_warships, leader, &self.state.config);
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return false;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            if kind == CityArsenalKind::Warship {
                self.state.events.push(GameEvent::NavalActionRejected {
                    player_id,
                    reason: NavalActionFailure::InsufficientGold,
                });
            }
            return false;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        let duration = kind.duration_ticks();
        let mut build = CityArsenalBuild {
            kind,
            ticks_until_complete: duration,
            total_ticks: duration,
            warship_source_tile: warship_route.as_ref().map(|route| route.src_tile),
            warship_target_tile: warship_route.as_ref().map(|route| route.landing_tile),
            paid_gold_bits: 0,
        };
        build.set_paid_gold(cost);
        let entry = self.building_productions.entry(building_id).or_default();
        entry.normalize_legacy_warship();
        entry.warship_queue.push(build);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    fn engine(tutorial: bool) -> SowEngine {
        let config = GameConfig {
            tutorial,
            buildings_enabled: true,
            player_leader: if tutorial {
                crate::player::Leader::of("boudica")
            } else {
                crate::player::Leader::of("caesar")
            },
            ..GameConfig::default()
        };
        let mut game = GameState::new(1, 5, 5, config.clone());
        game.phase = GamePhase::Playing;
        let mut player = Player::new_human(1, "Boudica".into(), [1.0, 0.0, 0.0], &config);
        player.gold = 1_000_000.0;
        game.register_player(player);
        SowEngine::new(game, WaterComponents::default())
    }

    #[test]
    fn confirmed_foundations_touch_without_upgrading_or_relocating_and_rejection_costs_nothing() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 32, 32, config.clone());
        state.phase = GamePhase::Playing;
        let mut player = Player::new_human(1, "Builder".into(), [1.0, 0.0, 0.0], &config);
        player.gold = 1_000_000.0;
        state.register_player(player);
        for y in 0..32 {
            for x in 0..32 {
                state.map.set_owner_id(x, y, 1);
            }
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        for (kind, x, y) in [
            (BuildingKind::City, 10, 10),
            (BuildingKind::City, 14, 10),
        ] {
            engine.apply_build_structure_intent(1, kind, y * 32 + x);
            assert!(
                engine
                    .buildings
                    .iter()
                    .any(|b| b.kind == kind && b.tile_idx == y * 32 + x && b.level == 1)
            );
        }
        let max_ticks = engine
            .buildings
            .iter()
            .map(|b| b.ticks_until_complete)
            .max()
            .unwrap_or(0);
        for _ in 0..max_ticks {
            engine.execute_construction();
        }
        engine.apply_build_structure_intent(1, BuildingKind::TradingPost, 10 * 32 + 17);
        assert!(engine.buildings.iter().any(|b| {
            b.kind == BuildingKind::TradingPost && b.tile_idx == 10 * 32 + 17 && b.level == 1
        }));
        let gold = engine.state.player(1).unwrap().gold;
        engine.apply_build_structure_intent(1, BuildingKind::City, 10 * 32 + 13);
        assert_eq!(engine.buildings.len(), 3);
        assert_eq!(engine.state.player(1).unwrap().gold, gold);
        assert!(engine.buildings.iter().all(|b| b.level == 1));
    }

    fn pacing_match() -> SowEngine {
        let config = GameConfig::default();
        let mut game = GameState::new(7, 20, 12, config.clone());
        game.phase = GamePhase::Playing;
        game.config.map_control_win_percentage = 2.0;
        for terrain in &mut game.map.terrain {
            *terrain = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let mut players = Vec::new();
        for (id, y0) in [(1u16, 0u32), (2, 8)] {
            let mut player =
                Player::new_human(id, format!("Player {id}"), [0.4, 0.6, 0.8], &config);
            player.has_spawned = true;
            player.tile_count = 32;
            player.gold = config.starting_gold;
            player.troops = config.starting_troops;
            let x_ranges: &[(u32, u32)] = if id == 1 {
                &[(0, 4), (6, 10)]
            } else {
                &[(12, 20)]
            };
            for y in y0..y0 + 4 {
                for &(x_start, x_end) in x_ranges {
                    for x in x_start..x_end {
                        let tile = y * game.map.width + x;
                        game.map.set_owner_id(x, y, id);
                        player.border_insert(tile);
                        player.sum_x += u64::from(x);
                        player.sum_y += u64::from(y);
                    }
                }
            }
            players.push(player);
        }
        game.players = players;
        game.player_lookup = vec![None, Some(0), Some(1)];
        game.total_land_tiles = 20 * 12;
        SowEngine::new(game, WaterComponents::default())
    }

    fn building(id: u64, kind: BuildingKind, level: u8) -> Building {
        Building {
            id,
            owner_id: 1,
            tile_idx: id as u32,
            kind,
            level,
            under_construction: false,
            ticks_until_complete: 0,
        }
    }

    #[test]
    fn structure_upgrade_charges_the_reduced_price_and_activates_when_finished() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::City, 1));
        game.buildings.push(Building {
            tile_idx: 1,
            ..building(2, BuildingKind::TradingPost, 1)
        });
        let before = game.state.player(1).unwrap().gold;
        let expected = structure_upgrade_cost_gold(BuildingKind::TradingPost, 2, 1, &game.state.config);

        game.apply_upgrade_structure_intent(1, 2);

        let trading_post = game.buildings[1];
        assert!((game.state.player(1).unwrap().gold - (before - expected)).abs() < 1e-9);
        assert!(trading_post.under_construction);
        assert_eq!(trading_post.active_level(), 1);

        for _ in 0..trading_post.ticks_until_complete {
            game.execute_construction();
        }
        assert_eq!(game.buildings[1].active_level(), 2);
    }

    #[test]
    fn first_boudica_tutorial_city_upgrade_is_free_once() {
        let mut game = engine(true);
        assert!(game.set_campaign_unlocks(crate::campaign::CampaignUnlocks {
            buildings: std::collections::HashMap::from([
                (BuildingKind::City, 2),
                (BuildingKind::TradingPost, 2),
            ]),
            actions: vec![],
        }));
        game.buildings.push(building(1, BuildingKind::City, 1));
        game.buildings.push(Building {
            tile_idx: 1,
            ..building(2, BuildingKind::TradingPost, 1)
        });
        game.state.player_mut(1).unwrap().gold = 0.0;

        assert!(!game.tutorial_city_upgrade_is_free(1, 2));
        assert!(game.tutorial_city_upgrade_is_free(1, 1));
        game.apply_upgrade_structure_intent(1, 1);

        assert_eq!(game.buildings[0].level, 2);
        assert_eq!(game.state.player(1).unwrap().gold, 0.0);
        assert!(!game.tutorial_city_upgrade_is_free(1, 1));

        game.buildings.push(Building {
            tile_idx: 100,
            ..building(3, BuildingKind::City, 1)
        });
        assert!(!game.tutorial_city_upgrade_is_free(1, 3));
        game.state.player_mut(1).unwrap().gold = 1_000_000.0;
        let cost = structure_upgrade_cost_gold(BuildingKind::City, 2, 3, &game.state.config);
        game.apply_upgrade_structure_intent(1, 3);
        assert!((game.state.player(1).unwrap().gold - (1_000_000.0 - cost)).abs() < 1e-9);
    }

    #[test]
    fn first_city_upgrade_is_not_free_outside_boudica_tutorial() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::City, 1));
        assert!(!game.tutorial_city_upgrade_is_free(1, 1));
    }

    #[test]
    fn trading_post_upgrades_without_a_city_level_requirement() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::TradingPost, 1));

        game.apply_upgrade_structure_intent(1, 1);
        assert_eq!(game.buildings[0].level, 2);
        assert!(game.buildings[0].under_construction);
    }

    #[test]
    fn trading_post_build_order_has_no_city_level_requirement() {
        let mut game = engine(true);
        assert!(game.set_campaign_unlocks(crate::campaign::CampaignUnlocks {
            buildings: std::collections::HashMap::from([(BuildingKind::TradingPost, 1)]),
            actions: vec![],
        }));
        game.state.map = crate::map::GameMap::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                game.state.map.set_owner_id(x, y, 1);
            }
        }
        let trading_post_tile = 16 * 32 + 16;
        game.state.player_mut(1).unwrap().tile_count = 32 * 32;
        let gold = game.state.player(1).unwrap().gold;

        game.apply_build_structure_intent(1, BuildingKind::TradingPost, trading_post_tile);

        assert_eq!(game.buildings.len(), 1);
        assert_eq!(game.buildings[0].kind, BuildingKind::TradingPost);
        assert!(game.state.player(1).unwrap().gold < gold);
    }

    #[test]
    fn negative_gold_cannot_build_or_upgrade() {
        let mut game = engine(false);
        game.state.map = crate::map::GameMap::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                game.state.map.set_owner_id(x, y, 1);
            }
        }
        game.state.player_mut(1).unwrap().gold = -1.0;
        game.buildings.push(building(1, BuildingKind::City, 1));

        game.apply_build_structure_intent(1, BuildingKind::TradingPost, 16 * 32 + 16);
        game.apply_upgrade_structure_intent(1, 1);

        assert_eq!(game.buildings.len(), 1);
        assert_eq!(game.buildings[0].level, 1);
        assert!(!game.buildings[0].under_construction);
        assert_eq!(game.state.player(1).unwrap().gold, -1.0);
    }

    #[test]
    fn military_camps_require_owned_lowland_and_allow_multiple_owned_plots() {
        let mut game = engine(false);
        game.state.map = crate::map::GameMap::new(20, 20);
        game.state.next_building_id = 3;
        let city_a = game.state.map.ref_id(4, 4) as u32;
        let city_b = game.state.map.ref_id(12, 12) as u32;
        game.buildings.push(Building {
            tile_idx: city_a,
            ..building(1, BuildingKind::City, 1)
        });
        game.buildings.push(Building {
            tile_idx: city_b,
            ..building(2, BuildingKind::City, 1)
        });
        for y in 0..20 {
            for x in 0..20 {
                let idx = game.state.map.ref_id(x, y);
                game.state.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
                game.state.map.set_owner_id(x, y, 1);
            }
        }
        game.state.map.set_owner_id(2, 2, 0);

        let unowned_tile = game.state.map.ref_id(2, 2) as u32;
        let empty_land_tile = game.state.map.ref_id(4, 12) as u32;
        let highland = game.state.map.ref_id(12, 4);
        game.state.map.terrain[highland] = crate::map::MapTile::from_byte(0x80 | 10);
        game.apply_build_structure_intent(1, BuildingKind::MilitaryCamp, unowned_tile);
        assert_eq!(game.buildings.len(), 2);

        game.apply_build_structure_intent(1, BuildingKind::MilitaryCamp, highland as u32);
        assert_eq!(game.buildings.len(), 3);
        assert_eq!(game.buildings[2].tile_idx, highland as u32);

        game.apply_build_structure_intent(1, BuildingKind::MilitaryCamp, empty_land_tile);
        assert_eq!(game.buildings.len(), 4);
        assert_eq!(game.buildings[3].tile_idx, empty_land_tile);
        assert!(game.buildings[3].under_construction);

        // Occupied foundations reject both another MilitaryCamp and overlap with a City.
        game.apply_build_structure_intent(1, BuildingKind::MilitaryCamp, empty_land_tile);
        game.apply_build_structure_intent(1, BuildingKind::MilitaryCamp, city_a);
        assert_eq!(game.buildings.len(), 4);
    }

    #[test]
    fn nuclear_launches_require_ready_silo_charge_and_gold() {
        use crate::game::NukeKind;
        use crate::player::Player;
        let mut game = engine(false);
        let mut defender = Player::new_human(2, "Defender".into(), [0.0, 0.0, 1.0], &game.state.config);
        defender.tile_count = 1;
        game.state.register_player(defender);
        let mut ally = Player::new_human(3, "Ally".into(), [0.0, 1.0, 0.0], &game.state.config);
        ally.tile_count = 1;
        game.state.register_player(ally);
        game.state.player_mut(1).unwrap().alliances.push(3);
        game.state.player_mut(3).unwrap().alliances.push(1);
        game.state.map.set_owner_id(4, 0, 2);
        game.state.player_mut(1).unwrap().gold = 1000.0;
        game.add_building(building(1, BuildingKind::TradingPost, 4));
        game.add_building(building(2, BuildingKind::Tower, 4));
        game.apply_building_production_intent(1, 1, CityArsenalKind::Nuke);
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        assert!(game.projectiles.is_empty());
        assert_eq!(game.state.player(1).unwrap().gold, 1000.0);
        assert!(game.state.events.iter().any(|event| matches!(event,
            crate::game::GameEvent::NuclearActionRejected {
                player_id: 1,
                reason: crate::game::NuclearActionFailure::CityRequirement,
                ..
            }
        )));

        game.add_building(building(7, BuildingKind::City, crate::building::NUCLEAR_CITY_LEVEL));
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        assert!(game.projectiles.is_empty());
        assert!(game.state.events.iter().any(|event| matches!(event,
            crate::game::GameEvent::NuclearActionRejected {
                player_id: 1,
                reason: crate::game::NuclearActionFailure::NoReadySilo,
                ..
            }
        )));

        let mut silo = building(3, BuildingKind::MissileSilo, 1);
        silo.under_construction = true;
        game.add_building(silo);
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        assert!(game.projectiles.is_empty());
        game.buildings[2].under_construction = false;
        game.state.player_mut(1).unwrap().gold = 499.0;
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        assert!(game.projectiles.is_empty());
        assert!(game.state.events.iter().any(|event| matches!(event,
            crate::game::GameEvent::NuclearActionRejected {
                player_id: 1,
                reason: crate::game::NuclearActionFailure::InsufficientGold,
                ..
            }
        )));
        game.state.player_mut(1).unwrap().gold = 1000.0;
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        let city_idx = game.buildings.iter().position(|building| building.id == 7).unwrap();
        game.buildings[city_idx].under_construction = true;
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        assert_eq!(game.projectiles.len(), 1);
        assert_eq!(game.state.player(1).unwrap().gold, 500.0);
        assert_eq!(game.nuclear_charges(3, 1), 0);
        game.buildings[city_idx].under_construction = false;
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        assert_eq!(game.projectiles.len(), 1);
        assert!(game.state.events.iter().any(|event| matches!(event,
            crate::game::GameEvent::NuclearActionRejected {
                player_id: 1,
                reason: crate::game::NuclearActionFailure::NoReadySilo,
                ..
            }
        )));
        assert_eq!(game.state.player(1).unwrap().gold, 500.0);
        game.state.tick += 90;
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, 4);
        assert_eq!(game.projectiles.len(), 2);
        assert_eq!(game.state.player(1).unwrap().gold, 0.0);

        let mut closer = building(4, BuildingKind::MissileSilo, 2);
        closer.tile_idx = 3;
        game.add_building(closer);
        game.projectiles.clear();
        game.state.player_mut(1).unwrap().gold = 20_000.0;
        game.launch_nuclear_weapon(1, NukeKind::HydrogenBomb, 4);
        assert_eq!(game.projectiles[0].src_tile, 3);
        assert_eq!(game.projectiles[0].kind, crate::game::ProjectileKind::Nuke { level: 2 });
        game.projectiles.clear();
        game.state.tick += 5;
        game.launch_nuclear_weapon(1, NukeKind::MIRV, 4);
        assert_eq!(game.state.player(1).unwrap().gold, 0.0);
        assert_eq!(game.nuclear_price(NukeKind::MIRV), 26_667.0);
        assert_eq!(game.nuclear_charges(4, 2), 0);
        game.execute_projectiles();
        assert_eq!(game.projectiles[0].kind, crate::game::ProjectileKind::Nuke { level: 4 });
        game.state.tick += 85;
        assert_eq!(game.nuclear_charges(4, 2), 1);
        game.state.tick += 5;
        assert_eq!(game.nuclear_charges(4, 2), 2);

        game.projectiles.clear();
        game.state.player_mut(1).unwrap().gold = 1_000_000.0;
        game.state.map.set_owner_id(1, 1, 1);
        game.state.map.set_owner_id(2, 1, 3);
        game.state.map.terrain[12] = crate::map::MapTile::from_byte(0);
        game.add_building(building(5, BuildingKind::MissileSilo, 4));
        game.add_building(building(6, BuildingKind::MissileSilo, 4));
        for (kind, target) in [
            (NukeKind::AtomBomb, 6),       // Own land
            (NukeKind::AtomBomb, 4),       // Enemy land
            (NukeKind::HydrogenBomb, 7),   // Allied land
            (NukeKind::HydrogenBomb, 8),   // Neutral land
            (NukeKind::HydrogenBomb, 12),  // Water
            (NukeKind::MIRV, 12),          // Water
        ] {
            game.launch_nuclear_weapon(1, kind, target);
        }
        for target in [4, 6, 7, 8, 12] {
            assert!(game.projectiles.iter().any(|projectile| projectile.dst_tile == target));
        }
        assert_eq!(
            game.projectiles.iter().filter(|projectile| projectile.dst_tile == 12).count(),
            2
        );

        let mut spread_game = engine(false);
        spread_game.state.map = crate::map::GameMap::new(256, 256);
        let mut spread_enemy = Player::new_human(
            2,
            "Spread target".into(),
            [0.0, 0.0, 1.0],
            &spread_game.state.config,
        );
        spread_enemy.tile_count = 1;
        spread_game.state.register_player(spread_enemy);
        let spread_target = 128 * 256 + 128;
        spread_game.state.map.set_owner_id(128, 128, 2);
        let mut spread_silo = building(1, BuildingKind::MissileSilo, 1);
        spread_silo.tile_idx = spread_target;
        spread_game.add_building(spread_silo);
        let mut spread_city = building(2, BuildingKind::City, 4);
        spread_city.tile_idx = spread_target - 1;
        spread_game.add_building(spread_city);
        spread_game.state.player_mut(1).unwrap().gold = 100_000.0;
        spread_game.launch_nuclear_weapon(1, NukeKind::MIRV, spread_target);
        spread_game.execute_projectiles();
        let warheads: Vec<_> = spread_game
            .projectiles
            .iter()
            .filter(|projectile| projectile.kind == crate::game::ProjectileKind::Nuke { level: 4 })
            .collect();
        assert!(warheads.len() > 1);
        assert!(warheads.iter().any(|projectile| {
            projectile.dst_tile != spread_target
                && spread_game.state.map.owner_id(
                    projectile.dst_tile % spread_game.state.map.width,
                    projectile.dst_tile / spread_game.state.map.width,
                ) != 2
        }));
        assert!(warheads
            .iter()
            .all(|projectile| projectile.dst_tile < 256 * 256));
    }

    #[test]
    fn campaign_nuke_succeeds_with_level_one_silo_and_level_four_city() {
        use crate::campaign::{CampaignAction, CampaignUnlocks};
        use crate::game::NukeKind;
        use crate::map::MapTile;
        use crate::player::Player;
        use std::collections::HashMap;

        let config = GameConfig {
            tutorial: true,
            buildings_enabled: true,
            ..GameConfig::default()
        };
        let mut state = GameState::new(1, 32, 32, config.clone());
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x80));
        let mut player = Player::new_human(1, "Six Sky".into(), [1.0, 0.0, 0.0], &config);
        player.gold = 5_000.0;
        state.register_player(player);
        let mut enemy = Player::new_human(2, "Tikal".into(), [0.5, 0.2, 0.8], &config);
        enemy.tile_count = 1;
        state.register_player(enemy);
        for y in 0..16 {
            for x in 0..16 {
                state.map.set_owner_id(x, y, 1);
            }
        }
        state.map.set_owner_id(24, 24, 2);

        let mut game = SowEngine::new(state, WaterComponents::default());
        assert!(game.set_campaign_unlocks(CampaignUnlocks {
            buildings: HashMap::from([
                (BuildingKind::MissileSilo, 1),
                (BuildingKind::City, 4),
            ]),
            actions: vec![CampaignAction::Nuke],
        }));
        let silo_tile = 5 * 32 + 5;
        let city_tile = 10 * 32 + 10;

        game.apply_build_structure_intent(1, BuildingKind::MissileSilo, silo_tile);
        game.apply_build_structure_intent(1, BuildingKind::City, city_tile);
        assert_eq!(game.buildings.len(), 2);
        for b in &mut game.buildings {
            b.under_construction = false;
            b.ticks_until_complete = 0;
            if b.kind == BuildingKind::City {
                b.level = 4;
            }
        }
        let snapshot = game.build_snapshot();
        assert!(snapshot.players.iter().any(|p| p.id == 1 && p.nuke_available));
        assert!(crate::building::snapshot_has_nuclear_launch_city(
            &snapshot.buildings,
            1,
            true,
        ));

        let target_tile = 24 * 32 + 24;
        game.launch_nuclear_weapon(1, NukeKind::AtomBomb, target_tile);
        assert_eq!(game.projectiles.len(), 1);
        assert_eq!(game.projectiles[0].dst_tile, target_tile);
    }

    #[test]
    fn first_city_becomes_affordable_and_is_building_within_game_time_budget() {
        let mut game = pacing_match();
        let config = game.state.config.clone();
        assert_eq!(game.state.player(1).unwrap().gold, config.starting_gold);
        let city_tile = 2 * game.state.map.width + 8;
        let mut city_ordered = false;
        // Preserve the original game-time budget when the default speed or tick rate changes.
        let ticks = (52.8 / config.per_tick(1.0)).ceil() as u64;
        for _ in 0..ticks {
            game.execute_income();
            game.state.tick += 1;
            if !city_ordered && game.state.player(1).unwrap().gold >= config.cost_city {
                game.apply_build_structure_intent(1, BuildingKind::City, city_tile);
                city_ordered = true;
            }
        }
        assert!(
            city_ordered,
            "starting gold and ordinary income should afford a City within 52.8 game seconds"
        );
        assert!(game.buildings.iter().any(|building| {
            building.owner_id == 1
                && building.kind == BuildingKind::City
                && building.under_construction
        }));
        let player = game.state.player(1).unwrap();
        assert!(player.gold.is_finite() && player.gold >= 0.0);
        assert!(player.troops.is_finite() && player.max_troops.is_finite());
    }

    #[test]
    fn upgrades_charge_the_quoted_price_and_apply_hero_discounts() {
        let mut game = pacing_match();
        game.add_building(building(1, BuildingKind::City, 1));
        game.add_building(Building {
            tile_idx: 1,
            ..building(2, BuildingKind::TradingPost, 1)
        });
        game.state.player_mut(1).unwrap().gold = 1_000.0;
        let config = game.state.config.clone();
        let upgrade_cost = structure_upgrade_cost_gold(BuildingKind::TradingPost, 2, 1, &config);
        assert_eq!(upgrade_cost, 105.0);

        let gold = game.state.player(1).unwrap().gold;
        game.apply_upgrade_structure_intent(1, 2);
        assert!(game.buildings[1].under_construction);
        let after_upgrade = game.state.player(1).unwrap().gold;
        assert!((after_upgrade - (gold - upgrade_cost)).abs() < 1e-9);

        game.state.player_mut(1).unwrap().leader = crate::player::Leader::of("queen_nefertiti");
        let city_cost = structure_upgrade_cost_gold(BuildingKind::City, 2, 1, &config) * 0.65;
        let (quoted, discounted) = game.structure_upgrade_quote(1).unwrap();
        assert!(discounted && (quoted - city_cost).abs() < 1e-9);
        game.apply_upgrade_structure_intent(1, 1);
        assert!((game.state.player(1).unwrap().gold - (after_upgrade - city_cost)).abs() < 1e-9);
    }

    #[test]
    fn gold_income_does_not_charge_troop_upkeep_and_trade_ships_add_income() {
        fn trace(
            troops: f64,
            tiles: u32,
            trading_post_level: u8,
            trade_ships: u32,
            include_assisted_defeat: bool,
        ) -> [f64; 4] {
            let mut game = pacing_match();
            let player = game.state.player_mut(1).unwrap();
            player.troops = troops;
            player.tile_count = tiles;
            if trading_post_level > 0 {
                game.add_building(building(1, BuildingKind::TradingPost, trading_post_level));
            }
            if trade_ships > 0 {
                game.add_building(building(2, BuildingKind::Port, 4));
                for id in 1..=u64::from(trade_ships) {
                    game.add_fleet(crate::warp_fleet::WarpFleet::new(
                        id,
                        1,
                        0,
                        crate::game::UnitType::TradeShip,
                        0.0,
                        (0, 1),
                        vec![0, 1],
                    ));
                }
            }
            if include_assisted_defeat {
                game.state.player_mut(2).unwrap().player_type = crate::player::PlayerType::Nation;
                let config = game.state.config.clone();
                let mut assistant = crate::player::Player::new_human(
                    3,
                    "Assistant".into(),
                    [0.2, 0.7, 0.4],
                    &config,
                );
                assistant.tile_count = 1;
                assistant.gold = 0.0;
                game.state.register_player(assistant);
                game.state
                    .player_mut(1)
                    .unwrap()
                    .tile_conquests
                    .insert(2, 5);
                game.state
                    .player_mut(3)
                    .unwrap()
                    .tile_conquests
                    .insert(2, 1);
            }
            let mut checkpoints = [0.0; 4];
            for tick in 1..=6_000 {
                game.state.tick = tick;
                game.execute_income();
                if include_assisted_defeat && tick == 1_200 {
                    game.eliminate_player(2, 1, 0, 0, false);
                    assert!(matches!(
                        game.state.events.last(),
                        Some(crate::game::GameEvent::PlayerEliminated {
                            gold_bounty: 75,
                            assists,
                            ..
                        }) if assists == &vec![(3, 75)]
                    ));
                }
                if matches!(tick, 300 | 1_200 | 3_600 | 6_000) {
                    let player = game.state.player(1).unwrap();
                    eprintln!(
                        "economy tick={tick} tiles={} troops={:.0} gold={:.2} trade={trade_ships}",
                        player.tile_count, player.troops, player.gold,
                    );
                }
                match tick {
                    300 => checkpoints[0] = game.state.player(1).unwrap().gold,
                    1_200 => checkpoints[1] = game.state.player(1).unwrap().gold,
                    3_600 => checkpoints[2] = game.state.player(1).unwrap().gold,
                    6_000 => checkpoints[3] = game.state.player(1).unwrap().gold,
                    _ => {}
                }
            }
            checkpoints
        }

        let early = trace(1_000.0, 32, 0, 0, false);
        let large_army_same_territory = trace(100_000.0, 32, 0, 0, false);
        let growing_army = trace(25_000.0, 1_024, 0, 0, true);
        let industrial_trade = trace(100_000.0, 10_000, 4, 3, false);
        eprintln!(
            "gold at 30s/2m/6m/10m: early={early:?}, growing_army={growing_army:?}, industrial_trade={industrial_trade:?}"
        );

        assert!(early.iter().all(|gold| gold.is_finite()));
        assert!(growing_army.iter().all(|gold| gold.is_finite()));
        assert!(industrial_trade.iter().all(|gold| gold.is_finite()));
        assert_eq!(early, large_army_same_territory);
        assert!(early.windows(2).all(|pair| pair[1] > pair[0]));
        assert!(growing_army.windows(2).all(|pair| pair[1] > pair[0]));
        assert!(industrial_trade.windows(2).all(|pair| pair[1] > pair[0]));
        assert!(industrial_trade[1] > industrial_trade[0]);
    }

    #[test]
    fn multi_upgrade_quote_matches_sequential_step_costs_and_ownership_scaling() {
        let mut sequential = engine(false);
        sequential.buildings.push(building(1, BuildingKind::TradingPost, 1));
        sequential.state.player_mut(1).unwrap().gold = 10_000.0;
        let start_gold = sequential.state.player(1).unwrap().gold;
        for _ in 0..3 {
            sequential.apply_upgrade_structure_intent(1, 1);
            let ticks = sequential.buildings[0].ticks_until_complete;
            for _ in 0..ticks {
                sequential.execute_construction();
            }
        }
        let sequential_spent = start_gold - sequential.state.player(1).unwrap().gold;

        let mut bulk = engine(false);
        bulk.buildings.push(building(1, BuildingKind::TradingPost, 1));
        bulk.state.player_mut(1).unwrap().gold = 10_000.0;
        let (bulk_quote, total_ticks, _) = bulk.structure_upgrade_to_quote(1, 4).unwrap();
        assert!((bulk_quote - sequential_spent).abs() < 1e-9);
        assert_eq!(
            total_ticks,
            (2..=4)
                .map(|lvl| crate::building::structure_upgrade_duration_ticks(BuildingKind::TradingPost, lvl))
                .sum::<u32>()
        );
    }

    #[test]
    fn upgrade_structure_to_steps_through_levels_and_fires_milestones() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::City, 1));
        game.state.player_mut(1).unwrap().gold = 50_000.0;
        game.state.player_mut(1).unwrap().troops = 0.0;
        let initial_gold = game.state.player(1).unwrap().gold;
        let (expected_cost, total_ticks, _) = game.structure_upgrade_to_quote(1, 4).unwrap();

        game.apply_upgrade_structure_to_intent(1, 1, Some(4));
        assert!((game.state.player(1).unwrap().gold - (initial_gold - expected_cost)).abs() < 1e-9);
        assert_eq!(game.buildings[0].level, 2);
        assert_eq!(game.buildings[0].active_level(), 1);
        assert_eq!(game.upgrade_targets.get(&1).copied(), Some(4));

        // Extending from an idle or finished state works step-by-step and awards City troops at Lv.2, 3, 4.
        let mut ready_levels = Vec::new();
        for _ in 0..total_ticks {
            game.state.events.clear();
            game.execute_construction();
            for ev in &game.state.events {
                if let GameEvent::StructureReady { .. } = ev {
                    ready_levels.push(game.buildings[0].active_level());
                }
            }
        }
        assert_eq!(ready_levels, vec![2, 3, 4]);
        assert!(!game.buildings[0].under_construction);
        assert_eq!(game.buildings[0].active_level(), 4);
        assert!(!game.upgrade_targets.contains_key(&1));
    }

    #[test]
    fn capture_clears_queued_upgrade_target() {
        let mut game = engine(false);
        game.buildings.push(Building {
            tile_idx: 22,
            ..building(1, BuildingKind::City, 1)
        });
        game.state.player_mut(1).unwrap().gold = 50_000.0;
        game.apply_upgrade_structure_to_intent(1, 1, Some(4));
        assert_eq!(game.upgrade_targets.get(&1).copied(), Some(4));

        let x = 22 % game.state.map.width;
        let y = 22 / game.state.map.width;
        game.state.set_tile_owner(x, y, 2);
        game.sync_building_ownership();

        assert_eq!(game.buildings[0].owner_id, 2);
        assert!(!game.upgrade_targets.contains_key(&1));
        for _ in 0..game.buildings[0].ticks_until_complete {
            game.execute_construction();
        }
        assert_eq!(game.buildings[0].active_level(), 2);
        assert!(!game.buildings[0].under_construction);
    }

    #[test]
    fn warship_queue_stacks_up_to_five_refunds_on_cancel_and_supports_max() {
        let mut game = engine(false);
        // Make tile 0 and 2 ocean water adjacent to port at tile 1 (5x5 map).
        game.state.map.terrain[0] = crate::map::MapTile(0b0100_0000);
        game.state.map.terrain[2] = crate::map::MapTile(0b0100_0000);
        game.water = WaterComponents::compute(&game.state.map, |_| {});
        game.buildings.push(Building {
            tile_idx: 1,
            ..building(1, BuildingKind::Port, 4)
        });
        // Add extra ports so fleet capacity >= 6 (base 0 + 6 port levels).
        game.buildings.push(Building {
            tile_idx: 6,
            ..building(2, BuildingKind::Port, 4)
        });
        let start_gold = game.state.player(1).unwrap().gold;

        // Queue 5 warships at port 1.
        for _ in 0..5 {
            game.apply_building_production_intent(1, 1, CityArsenalKind::Warship);
        }
        assert_eq!(game.building_productions.get(&1).unwrap().warship_queue_len(), 5);
        let gold_after_5 = game.state.player(1).unwrap().gold;
        assert!(gold_after_5 < start_gold);

        // 6th fails with QueueFull.
        game.state.events.clear();
        game.apply_building_production_intent(1, 1, CityArsenalKind::Warship);
        assert!(game.state.events.iter().any(|ev| matches!(
            ev,
            GameEvent::NavalActionRejected {
                player_id: 1,
                reason: NavalActionFailure::QueueFull
            }
        )));

        // Cancelling index 2 refunds its exact paid gold.
        let paid_idx2 = game.building_productions.get(&1).unwrap().warship_queue[2].paid_gold();
        game.apply_cancel_warship_queue_intent(1, 1, 2);
        assert_eq!(game.building_productions.get(&1).unwrap().warship_queue_len(), 4);
        assert!((game.state.player(1).unwrap().gold - (gold_after_5 + paid_idx2)).abs() < 1e-9);

        // Max fills remaining slot back to 5.
        game.apply_building_production_max_intent(1, 1, CityArsenalKind::Warship);
        assert_eq!(game.building_productions.get(&1).unwrap().warship_queue_len(), 5);

        // Complete the first warship in queue, then test MoveWarships -> Holding & PatrolWarships -> Patrolling.
        let dur = CityArsenalKind::Warship.duration_ticks();
        for _ in 0..dur {
            game.execute_construction();
        }
        assert_eq!(game.building_productions.get(&1).unwrap().warship_queue_len(), 4);
        assert_eq!(game.fleets.len(), 1);
        let fleet_id = game.fleets[0].id;
        game.apply_move_warships_intent(1, &[fleet_id], 2);
        assert_eq!(
            game.fleets[0].warship.as_ref().unwrap().mode,
            crate::warp_fleet::WarshipMode::Holding
        );
        game.apply_patrol_warships_intent(1, &[fleet_id]);
        assert_eq!(
            game.fleets[0].warship.as_ref().unwrap().mode,
            crate::warp_fleet::WarshipMode::Patrolling
        );

        // Legacy single-slot CityArsenalState JSON (without warship_queue / paid_gold_bits) deserializes & migrates cleanly.
        let legacy_json = r#"{"has_anti_nuke":false,"nuke_ready":false,"arsenal_build":{"kind":"Warship","ticks_until_complete":10,"total_ticks":50}}"#;
        let mut legacy: crate::building::CityArsenalState = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(legacy.warship_queue_len(), 1);
        legacy.normalize_legacy_warship();
        assert!(legacy.arsenal_build.is_none());
        assert_eq!(legacy.warship_queue.len(), 1);
    }
}
