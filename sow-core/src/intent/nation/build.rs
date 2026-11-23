use crate::building::find_structure_site_near;
use crate::engine::SowEngine;
use crate::game::BuildingKind;
use crate::protocol::GameplayIntent;
use crate::rng::NextIntExt;

use super::profile::{AiSlot, AiTier, BotDecision, BotDecisionKind};
use super::structures::{
    PLACEMENT_ATTEMPTS, StructureCandidates, bot_structure_target_count, cheapest_gold_cost,
    resolve_farm_from_candidates, resolve_structure_from_candidates, stack_build_decision,
};

impl SowEngine {
    pub(super) fn nation_run_structure_build_for_slot(
        &mut self,
        slot: &AiSlot,
        bot_id: u16,
        bot_iq: u32,
        build_cost: f64,
        decisions: &mut Vec<BotDecision>,
    ) {
        // ── Structure building (IQ-scaled, all bots) ─────────────────
        if slot.do_structures {
            let current_points = self.state.player(bot_id).unwrap().iq_points;
            if current_points >= build_cost {
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

                    let has_enemy_nuclear_capability = self.buildings.iter().any(|building| {
                        building.owner_id != bot_id
                            && !crate::execution::hero_effects::allied(
                                &self.state,
                                bot_id,
                                building.owner_id,
                            )
                            && !building.under_construction
                            && building.kind == BuildingKind::MissileSilo
                            && building.active_level() > 0
                    });
                    let sam_defense_enabled = bot_iq >= 100
                        && has_enemy_nuclear_capability
                        && self.foundation_quote(bot_id, BuildingKind::SAM).is_some();
                    let mut all_owned_buildings_covered = false;
                    if sam_defense_enabled {
                        let sams: Vec<_> = self.buildings.iter()
                            .filter(|sam| {
                                sam.owner_id == bot_id
                                    && sam.kind == BuildingKind::SAM
                                    && (sam.active_level() > 0 || sam.under_construction)
                            })
                            .map(|sam| (sam.tile_idx, sam.active_level().max(1)))
                            .collect();
                        all_owned_buildings_covered = true;
                        let mut sam_site = None;
                        const MAX_SAM_SITE_SEARCHES_PER_THINK: usize = 8;
                        let mut sam_searches = 0usize;
                        for building in self.buildings.iter().filter(|b| b.owner_id == bot_id) {
                            let covered = sams.iter().any(|&(tile, level)| {
                                crate::execution::sam::sam_covers_tile(
                                    tile,
                                    building.tile_idx,
                                    self.state.map.width,
                                    level,
                                )
                            });
                            if covered {
                                continue;
                            }
                            all_owned_buildings_covered = false;
                            if sam_site.is_none() && sam_searches < MAX_SAM_SITE_SEARCHES_PER_THINK {
                                sam_searches += 1;
                                sam_site = find_structure_site_near(
                                    &self.state.map,
                                    bot_id,
                                    building.tile_idx,
                                    BuildingKind::SAM,
                                    &self.building_grid,
                                    &mut self.placement_scratch,
                                    self.legacy_building_rules,
                                );
                                if sam_site.is_some() {
                                    break;
                                }
                            } else if sam_searches >= MAX_SAM_SITE_SEARCHES_PER_THINK {
                                break;
                            }
                        }
                        if let Some(sam_site) = sam_site {
                            if self.foundation_quote(bot_id, BuildingKind::SAM)
                                .is_some_and(|(_, cost, _)| player_gold >= cost)
                            {
                                if let Some(player) = self.state.player_mut(bot_id) {
                                    player.iq_points -= build_cost;
                                }
                                decisions.push(BotDecision {
                                    bot_id,
                                    kind: BotDecisionKind::Build,
                                    intent: GameplayIntent::BuildStructure {
                                        kind: BuildingKind::SAM,
                                        target_tile: sam_site,
                                    },
                                });
                                return;
                            }
                        }
                    }
                    let nuclear_city_level = crate::building::nuclear_city_level(self.campaign_unlocks.is_some());
                    let wants_nuclear_progression = matches!(slot.tier, AiTier::Ghost | AiTier::Nation)
                        && bot_iq >= 100
                        && agg.highest_city_level >= 4
                        && self.campaign_allows_building(BuildingKind::MissileSilo, 1)
                        && self.campaign_allows_building(BuildingKind::City, nuclear_city_level)
                        && self.buildings.iter().any(|building| {
                            building.kind == BuildingKind::City
                                && self.naval_target_allowed(bot_id, building.owner_id)
                        });
                    if wants_nuclear_progression && agg.highest_city_level < nuclear_city_level
                        && !super::structures::has_pending_build(decisions, bot_id)
                        && let Some(upgrade) = stack_build_decision(
                            self, bot_id, BuildingKind::City, player_gold, nuclear_city_level,
                        )
                    {
                        if let Some(player) = self.state.player_mut(bot_id) {
                            player.iq_points -= build_cost;
                        }
                        decisions.push(upgrade);
                        return;
                    }
                    if wants_nuclear_progression && agg.count_silo == 0
                        && self.foundation_quote(bot_id, BuildingKind::MissileSilo)
                            .is_some_and(|(_, cost, _)| player_gold >= cost + self.nuclear_price(crate::game::NukeKind::AtomBomb))
                    {
                        return;
                    }
                    let economy_priority_gap = slot.policy().prioritizes_economy_buildings
                        && [BuildingKind::City, BuildingKind::Factory, BuildingKind::Farm]
                            .into_iter()
                            .any(|kind| {
                                agg.total_structures_of_kind(kind)
                                    < bot_structure_target_count(kind, city_equivalent, bot_iq)
                            });
                    let mut build_order = [
                        BuildingKind::Tower,
                        BuildingKind::City,
                        BuildingKind::Factory,
                        BuildingKind::Port,
                        BuildingKind::Farm,
                    ];
                    let port_target_count =
                        bot_structure_target_count(BuildingKind::Port, city_equivalent, bot_iq);
                    let prioritize_ports =
                        city_equivalent >= 3 && agg.count_port < port_target_count;
                    build_order.sort_by(|&a, &b| {
                        if economy_priority_gap {
                            let a_unmet_economy = matches!(
                                a,
                                BuildingKind::City | BuildingKind::Factory | BuildingKind::Farm
                            ) && agg.total_structures_of_kind(a)
                                < bot_structure_target_count(a, city_equivalent, bot_iq);
                            let b_unmet_economy = matches!(
                                b,
                                BuildingKind::City | BuildingKind::Factory | BuildingKind::Farm
                            ) && agg.total_structures_of_kind(b)
                                < bot_structure_target_count(b, city_equivalent, bot_iq);
                            if a_unmet_economy != b_unmet_economy {
                                return b_unmet_economy.cmp(&a_unmet_economy);
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
                            crate::building::cost::structure_foundation_quote(kind, level, agg.levels_of_kind(kind), &self.state.config)
                                .map_or(f64::INFINITY, |(value, _)| value)
                        };
                        let (cost_a, cost_b) = (quote(a), quote(b));
                        cost_a
                            .partial_cmp(&cost_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let decisions_before_building = decisions.len();
                    for kind in build_order {
                        let Some((_, cost, _)) = self.foundation_quote(bot_id, kind) else { continue; };
                        if kind == BuildingKind::Factory && agg.highest_city_level == 0 && !agg.has_completed_port {
                            continue;
                        }
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
                            + agg.count_factory
                            + agg.count_port
                            + agg.count_farm
                            + agg.count_sam;
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
                                    p_me.iq_points -= build_cost;
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
                            continue;
                        }
                        let target_tile = {
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
                                if p.tile_count > 500 {
                                    let cx = (p.sum_x / p.tile_count as u64) as i32;
                                    let cy = (p.sum_y / p.tile_count as u64) as i32;
                                    for _ in 0..PLACEMENT_ATTEMPTS {
                                        let dx = p.bot_rng.next_int(-40, 41);
                                        let dy = p.bot_rng.next_int(-40, 41);
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
                            let target_tile = if kind == BuildingKind::Farm && self.legacy_building_rules {
                                resolve_farm_from_candidates(
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
                            target_tile
                        };
                        if let Some(target_tile) = target_tile {
                            if let Some(p_me) = self.state.player_mut(bot_id) {
                                p_me.iq_points -= build_cost;
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
                                p_me.iq_points -= build_cost;
                            }
                            decisions.push(d);
                            break;
                        }
                    }
                    if decisions.len() == decisions_before_building
                        && sam_defense_enabled
                        && all_owned_buildings_covered
                        && self.campaign_allows_building(BuildingKind::SAM, 2)
                    {
                        if let Some(decision) = stack_build_decision(
                            self,
                            bot_id,
                            BuildingKind::SAM,
                            player_gold,
                            self.campaign_unlocks
                                .as_ref()
                                .map_or(BuildingKind::SAM.max_level(), |unlocks| {
                                    unlocks.building_level(BuildingKind::SAM)
                                }),
                        ) {
                            if let Some(player) = self.state.player_mut(bot_id) {
                                player.iq_points -= build_cost;
                            }
                            decisions.push(decision);
                        }
                    }
                }
            }
        }
    }
}
