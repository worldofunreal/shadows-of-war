use crate::building::{city_anchors_from_buildings, find_structure_site_near};
use crate::engine::SowEngine;
use crate::game::BuildingKind;
use crate::protocol::GameplayIntent;
use crate::rng::NextIntExt;

use super::profile::{AiSlot, AiTier, BotDecision, BotDecisionKind};
use super::structures::{
    PLACEMENT_ATTEMPTS, StructureCandidates, bot_structure_target_count, cheapest_gold_cost,
    find_city_site_from_candidates, find_district_site_in_owned_cities,
    resolve_military_camp_from_candidates, resolve_structure_from_candidates, stack_build_decision,
};

impl SowEngine {
    pub(super) fn nation_run_structure_build_for_slot(
        &mut self,
        slot: &AiSlot,
        bot_id: u16,
        bot_iq: u32,
        build_cost: f64,
        shared: &super::AiTickShared,
        decisions: &mut Vec<BotDecision>,
    ) {
        // ── Structure building (IQ-scaled, all bots) ─────────────────
        if slot.do_structures {
            {
                let (player_gold, player_tile_count) = {
                    if let Some(player) = self.state.player(bot_id) {
                        (player.gold, player.tile_count)
                    } else {
                        return;
                    }
                };
                if player_gold >= cheapest_gold_cost(&self.state.config) {
                    let agg = self
                        .building_aggregates
                        .get(bot_id as usize)
                        .copied()
                        .unwrap_or_default();
                    let city_equivalent = (player_tile_count / 1000).max(1);

                    let has_enemy_nuclear_capability =
                        shared.silo_owners.iter().any(|&owner_id| {
                            owner_id != bot_id
                                && !crate::execution::hero_effects::allied(
                                    &self.state,
                                    bot_id,
                                    owner_id,
                                )
                        });
                    let sam_defense_enabled = bot_iq >= 100
                        && has_enemy_nuclear_capability
                        && self.campaign_allows_building(BuildingKind::Tower, 5);
                    let mut all_owned_buildings_covered = false;
                    if sam_defense_enabled && !self.legacy_building_rules {
                        let width = self.state.map.width.max(1);
                        let sams: Vec<_> = self
                            .buildings
                            .iter()
                            .filter(|t| {
                                t.owner_id == bot_id
                                    && t.kind == BuildingKind::Tower
                                    && t.active_level() >= 5
                            })
                            .map(|t| (t.tile_idx, t.active_level()))
                            .collect();
                        all_owned_buildings_covered = true;
                        let mut uncovered_tile = None;
                        for building in self.buildings.iter().filter(|b| b.owner_id == bot_id) {
                            let covered = sams.iter().any(|&(tile, level)| {
                                let range = crate::building::tower_range_for_level(
                                    level,
                                    &self.state.config,
                                    1.0,
                                )
                                .max(0) as u32;
                                crate::execution::sam::sam_tile_distance(tile, building.tile_idx, width)
                                    <= range
                            });
                            if !covered {
                                all_owned_buildings_covered = false;
                                uncovered_tile.get_or_insert(building.tile_idx);
                            }
                        }
                        if let Some(target) = uncovered_tile {
                            if let Some(upgrade_tower) = self
                                .buildings
                                .iter()
                                .filter(|b| {
                                    b.owner_id == bot_id
                                        && b.kind == BuildingKind::Tower
                                        && !b.under_construction
                                        && b.level < 5
                                })
                                .min_by_key(|b| {
                                    crate::execution::sam::sam_tile_distance(b.tile_idx, target, width)
                                })
                                .copied()
                            {
                                let cap = self.building_upgrade_level_cap(upgrade_tower);
                                if upgrade_tower.level < cap {
                                    let (cost, _) = self.paid_structure_upgrade_quote(upgrade_tower);
                                    if player_gold >= cost {
                                        if let Some(player) = self.state.player_mut(bot_id) {
                                            player.iq_points = (player.iq_points - build_cost).max(0.0);
                                        }
                                        decisions.push(BotDecision {
                                            bot_id,
                                            kind: BotDecisionKind::Build,
                                            intent: GameplayIntent::UpgradeStructure {
                                                building_id: upgrade_tower.id,
                                            },
                                        });
                                        return;
                                    }
                                } else if let Some(city) = self
                                    .buildings
                                    .iter()
                                    .filter(|c| {
                                        c.owner_id == bot_id
                                            && c.kind == BuildingKind::City
                                            && !c.under_construction
                                            && c.level < 5
                                    })
                                    .min_by_key(|c| {
                                        crate::execution::sam::sam_tile_distance(
                                            c.tile_idx,
                                            upgrade_tower.tile_idx,
                                            width,
                                        )
                                    })
                                    .copied()
                                {
                                    let (cost, _) = self.paid_structure_upgrade_quote(city);
                                    if player_gold >= cost {
                                        if let Some(player) = self.state.player_mut(bot_id) {
                                            player.iq_points = (player.iq_points - build_cost).max(0.0);
                                        }
                                        decisions.push(BotDecision {
                                            bot_id,
                                            kind: BotDecisionKind::Build,
                                            intent: GameplayIntent::UpgradeStructure {
                                                building_id: city.id,
                                            },
                                        });
                                        return;
                                    }
                                }
                            } else if let Some((_, cost, _)) =
                                self.foundation_quote(bot_id, BuildingKind::Tower)
                                && player_gold >= cost
                                && let Some(site) = find_district_site_in_owned_cities(
                                    self,
                                    bot_id,
                                    BuildingKind::Tower,
                                    Some(target),
                                )
                            {
                                if let Some(player) = self.state.player_mut(bot_id) {
                                    player.iq_points = (player.iq_points - build_cost).max(0.0);
                                }
                                decisions.push(BotDecision {
                                    bot_id,
                                    kind: BotDecisionKind::Build,
                                    intent: GameplayIntent::BuildStructure {
                                        kind: BuildingKind::Tower,
                                        target_tile: site,
                                    },
                                });
                                return;
                            }
                        }
                    }
                    let wants_nuclear_progression = matches!(slot.tier, AiTier::Ghost | AiTier::Nation)
                        && bot_iq >= 100
                        && agg.highest_city_level >= 4
                        && self.campaign_allows_building(BuildingKind::MilitaryCamp, 5)
                        && self.campaign_allows_building(BuildingKind::City, 5)
                        && shared
                            .city_owners
                            .iter()
                            .any(|&owner_id| self.naval_target_allowed(bot_id, owner_id));
                    if wants_nuclear_progression
                        && agg.highest_city_level < 5
                        && !super::structures::has_pending_build(decisions, bot_id)
                        && let Some(upgrade) = stack_build_decision(
                            self,
                            bot_id,
                            BuildingKind::City,
                            player_gold,
                            5,
                        )
                    {
                        if let Some(player) = self.state.player_mut(bot_id) {
                            player.iq_points = (player.iq_points - build_cost).max(0.0);
                        }
                        decisions.push(upgrade);
                        return;
                    }
                    if wants_nuclear_progression
                        && agg.silo_levels == 0
                        && agg.highest_city_level >= 5
                        && !super::structures::has_pending_build(decisions, bot_id)
                    {
                        if let Some(upgrade) = stack_build_decision(
                            self,
                            bot_id,
                            BuildingKind::MilitaryCamp,
                            player_gold,
                            5,
                        ) {
                            if let Some(player) = self.state.player_mut(bot_id) {
                                player.iq_points = (player.iq_points - build_cost).max(0.0);
                            }
                            decisions.push(upgrade);
                            return;
                        }
                    }
                    let is_campaign_faction = self.campaign_unlocks.is_some()
                        || self.campaign_contact_resolved.contains(&bot_id)
                        || self.campaign_structure_hold_ids.contains(&bot_id);
                    if !is_campaign_faction
                        && !self.legacy_building_rules
                        && slot.is_pro(
                            bot_iq,
                            self.state.config.bot_difficulty,
                            self.state.config.ranked_1v1,
                        )
                    {
                        if let Some(decision) = self.pro_decide_structure_action(
                            bot_id,
                            bot_iq,
                            player_gold,
                            player_tile_count,
                            agg,
                            slot.is_under_attack,
                            shared,
                        ) {
                            if let Some(player) = self.state.player_mut(bot_id) {
                                player.iq_points = (player.iq_points - build_cost).max(0.0);
                            }
                            decisions.push(decision);
                        }
                        return;
                    }
                    let all_structures = agg.count_city
                        + agg.count_tower
                        + agg.count_trading_post
                        + agg.count_port
                        + agg.count_military_camp
                        + agg.count_farm;
                    let prefers_towers_first = slot.policy().prefers_towers_first
                        && !is_campaign_faction
                        && agg.count_tower == 0;
                    let economy_priority_gap = slot.policy().prioritizes_economy_buildings
                        && [
                            BuildingKind::City,
                            BuildingKind::Farm,
                            BuildingKind::TradingPost,
                            BuildingKind::MilitaryCamp,
                        ]
                        .into_iter()
                        .any(|kind| {
                            kind.is_enabled()
                                && (kind != BuildingKind::TradingPost || all_structures > 0)
                                && agg.total_structures_of_kind(kind)
                                    < bot_structure_target_count(kind, city_equivalent, bot_iq)
                        });
                    let mut build_order = [
                        BuildingKind::City,
                        BuildingKind::Farm,
                        BuildingKind::TradingPost,
                        BuildingKind::Port,
                        BuildingKind::MilitaryCamp,
                        BuildingKind::Tower,
                    ];
                    let port_target_count =
                        bot_structure_target_count(BuildingKind::Port, city_equivalent, bot_iq);
                    let prioritize_ports =
                        city_equivalent >= 3 && agg.count_port < port_target_count;
                    build_order.sort_by(|&a, &b| {
                        if prefers_towers_first && (a == BuildingKind::Tower) != (b == BuildingKind::Tower) {
                            return if a == BuildingKind::Tower {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if agg.count_city == 0 && (a == BuildingKind::City) != (b == BuildingKind::City) {
                            return if a == BuildingKind::City {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        if economy_priority_gap {
                            let is_unmet_econ = |k: BuildingKind| {
                                k.is_enabled()
                                    && matches!(
                                        k,
                                        BuildingKind::City
                                            | BuildingKind::TradingPost
                                            | BuildingKind::MilitaryCamp
                                            | BuildingKind::Port
                                    )
                                    && (k != BuildingKind::TradingPost || all_structures > 0)
                                    && (k != BuildingKind::Port || all_structures >= 2)
                                    && agg.total_structures_of_kind(k)
                                        < bot_structure_target_count(k, city_equivalent, bot_iq)
                            };
                            let a_unmet_economy = is_unmet_econ(a);
                            let b_unmet_economy = is_unmet_econ(b);
                            if a_unmet_economy != b_unmet_economy {
                                return b_unmet_economy.cmp(&a_unmet_economy);
                            }
                            if a_unmet_economy && b_unmet_economy {
                                let count_cmp = agg
                                    .total_structures_of_kind(a)
                                    .cmp(&agg.total_structures_of_kind(b));
                                if count_cmp != std::cmp::Ordering::Equal {
                                    return count_cmp;
                                }
                            }
                        }
                        if prioritize_ports
                            && (a == BuildingKind::Port) != (b == BuildingKind::Port)
                        {
                            return if a == BuildingKind::Port {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            };
                        }
                        let quote = |kind: BuildingKind| {
                            let level = if self.campaign_unlocks.is_some() { 1 } else { kind.foundation_level(agg.highest_city_level) };
                            crate::building::cost::structure_foundation_quote(kind, level, self.paid_kind_levels(bot_id, kind), &self.state.config)
                                .map_or(f64::INFINITY, |(value, _)| value)
                        };
                        let (cost_a, cost_b) = (quote(a), quote(b));
                        cost_a
                            .partial_cmp(&cost_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let decisions_before_building = decisions.len();
                    for kind in build_order {
                        if !kind.is_enabled() {
                            continue;
                        }
                        if kind == BuildingKind::Tower && economy_priority_gap && !prefers_towers_first {
                            continue;
                        }
                        let Some((_, cost, _)) = self.foundation_quote(bot_id, kind) else { continue; };
                        let owned = agg.total_structures_of_kind(kind);
                        let mut target_count =
                            bot_structure_target_count(kind, city_equivalent, bot_iq);
                        if kind == BuildingKind::Tower && bot_iq >= 110 {
                            let under_attack = self
                                .ai_attack_index
                                .get(bot_id as usize)
                                .is_some_and(|attacks| !attacks.is_empty());
                            if under_attack && !economy_priority_gap {
                                target_count += 3;
                            }
                        }
                        let all_structures = agg.count_city
                            + agg.count_tower
                            + agg.count_trading_post
                            + agg.count_port
                            + agg.count_military_camp
                            + agg.count_farm;
                        let density = all_structures as f32 / player_tile_count.max(1) as f32;
                        let is_density_high = bot_iq >= 110 && density > 1.0 / 600.0;
                        let structure_floor = player_tile_count / 800;
                        let under_structure_floor = all_structures < structure_floor;
                        let wants_new = owned < target_count || under_structure_floor;

                        if !wants_new || is_density_high {
                            if let Some(d) = stack_build_decision(
                                self,
                                bot_id,
                                kind,
                                player_gold,
                                self.campaign_unlocks
                                    .as_ref()
                                    .map_or(kind.max_level(), |unlocks| {
                                        unlocks.building_level(kind)
                                    }),
                            ) {
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points = (p_me.iq_points - build_cost).max(0.0);
                                }
                                decisions.push(d);
                                break;
                            }
                            if !wants_new {
                                continue;
                            }
                        }

                        if !wants_new {
                            continue;
                        }
                        if player_gold < cheapest_gold_cost(&self.state.config) {
                            continue;
                        }
                        if self.state.player(bot_id).is_none_or(|p| p.gold < cost) {
                            if let Some(d) = stack_build_decision(
                                self,
                                bot_id,
                                kind,
                                player_gold,
                                self.campaign_unlocks
                                    .as_ref()
                                    .map_or(kind.max_level(), |unlocks| unlocks.building_level(kind)),
                            ) {
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points = (p_me.iq_points - build_cost).max(0.0);
                                }
                                decisions.push(d);
                                break;
                            }
                            continue;
                        }
                        let mut target_tile = if !self.legacy_building_rules && kind.is_non_city() {
                            let preferred_probe = if kind == BuildingKind::Port {
                                let w = self.state.map.width.max(1);
                                let cities = city_anchors_from_buildings(&self.buildings, w);
                                cities
                                    .into_iter()
                                    .find(|c| {
                                        c.owner_id == bot_id
                                            && c.active_level >= 1
                                            && crate::building::evaluate_city_site_traits(
                                                &self.state.map,
                                                c.x,
                                                c.y,
                                                &[],
                                                &self.state.config,
                                            )
                                            .has_coastal_port_access
                                    })
                                    .map(|c| c.y * w + c.x)
                            } else {
                                None
                            };
                            find_district_site_in_owned_cities(self, bot_id, kind, preferred_probe)
                        } else if kind == BuildingKind::TradingPost {
                            let anchor = self
                                .buildings
                                .iter()
                                .filter(|b| b.owner_id == bot_id && b.kind == BuildingKind::City)
                                .chain(self.buildings.iter().filter(|b| b.owner_id == bot_id))
                                .map(|b| b.tile_idx)
                                .next();
                            anchor.and_then(|tile_idx| {
                                find_structure_site_near(
                                    &self.state.map,
                                    bot_id,
                                    tile_idx,
                                    BuildingKind::TradingPost,
                                    &self.building_grid,
                                    &mut self.placement_scratch,
                                    self.legacy_building_rules,
                                )
                            })
                        } else {
                            None
                        };
                        if target_tile.is_none() && (self.legacy_building_rules || kind == BuildingKind::City) {
                            target_tile = {
                                let (_blocks_examined, _directory_words_examined) = {
                                    let (state, scratch) = (&mut self.state, &mut self.placement_scratch);
                                    let p = state.player_mut(bot_id).unwrap();
                                    scratch.border_scratch.clear();
                                    scratch.interior_scratch.clear();
                                    let blocks_examined = {
                                        p.border_tiles.sample_ones(
                                            p.bot_rng.rand() as u32,
                                            PLACEMENT_ATTEMPTS as usize,
                                            &mut scratch.border_scratch,
                                        );
                                        (0, 0)
                                    };
                                    if p.tile_count > 0 {
                                        if let Some((hx, hy)) = p.home {
                                            scratch.interior_scratch.push((hx as i32, hy as i32));
                                        }
                                        let cx = (p.sum_x / p.tile_count as u64) as i32;
                                        let cy = (p.sum_y / p.tile_count as u64) as i32;
                                        let radius = if p.tile_count > 500 { 41 } else { 13 };
                                        for _ in 0..PLACEMENT_ATTEMPTS {
                                            let dx = p.bot_rng.next_int(-radius + 1, radius);
                                            let dy = p.bot_rng.next_int(-radius + 1, radius);
                                            scratch.interior_scratch.push((cx + dx, cy + dy));
                                        }
                                    }
                                    blocks_examined
                                };
                                let border_candidates =
                                    std::mem::take(&mut self.placement_scratch.border_scratch);
                                let interior_candidates =
                                    std::mem::take(&mut self.placement_scratch.interior_scratch);
                                let candidates = StructureCandidates {
                                    border: &border_candidates,
                                    interior: &interior_candidates,
                                };
                                let resolved = if !self.legacy_building_rules && kind == BuildingKind::City {
                                    find_city_site_from_candidates(self, bot_id, candidates)
                                } else if kind == BuildingKind::MilitaryCamp && self.legacy_building_rules {
                                    resolve_military_camp_from_candidates(
                                        &self.state.map,
                                        bot_id,
                                        candidates,
                                        &self.buildings,
                                        &self.building_grid,
                                        self.legacy_building_rules,
                                    )
                                } else {
                                    resolve_structure_from_candidates(
                                        &self.state.map,
                                        bot_id,
                                        kind,
                                        candidates,
                                        &self.building_grid,
                                        &mut self.placement_scratch,
                                        self.legacy_building_rules,
                                    )
                                };
                                self.placement_scratch.border_scratch = border_candidates;
                                self.placement_scratch.interior_scratch = interior_candidates;
                                resolved
                            };
                        }
                        if let Some(target_tile) = target_tile {
                            if let Some(p_me) = self.state.player_mut(bot_id) {
                                p_me.iq_points = (p_me.iq_points - build_cost).max(0.0);
                            }
                            decisions.push(BotDecision {
                                bot_id,
                                kind: BotDecisionKind::Build,
                                intent: GameplayIntent::BuildStructure { kind, target_tile },
                            });
                            break;
                        }

                        // Boxed in: stack on existing (player rules via apply_build_structure_intent)
                        if let Some(d) = stack_build_decision(
                            self,
                            bot_id,
                            kind,
                            player_gold,
                            self.campaign_unlocks
                                .as_ref()
                                .map_or(kind.max_level(), |unlocks| unlocks.building_level(kind)),
                        ) {
                            if let Some(p_me) = self.state.player_mut(bot_id) {
                                p_me.iq_points = (p_me.iq_points - build_cost).max(0.0);
                            }
                            decisions.push(d);
                            break;
                        }
                    }
                    if decisions.len() == decisions_before_building
                        && sam_defense_enabled
                        && all_owned_buildings_covered
                        && self.campaign_allows_building(BuildingKind::Tower, 6)
                    {
                        if let Some(decision) = stack_build_decision(
                            self,
                            bot_id,
                            BuildingKind::Tower,
                            player_gold,
                            self.campaign_unlocks
                                .as_ref()
                                .map_or(BuildingKind::Tower.max_level(), |unlocks| {
                                    unlocks.building_level(BuildingKind::Tower)
                                }),
                        ) {
                            if let Some(player) = self.state.player_mut(bot_id) {
                                player.iq_points = (player.iq_points - build_cost).max(0.0);
                            }
                            decisions.push(decision);
                        }
                    }
                }
            }
        }
    }
}
