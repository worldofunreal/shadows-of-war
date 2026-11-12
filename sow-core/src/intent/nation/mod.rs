use crate::engine::SowEngine;
use crate::protocol::StampedIntent;
use crate::rng::NextIntExt;
use wyrand::WyRand;

mod build;
mod combat;
mod diplomacy;
mod profile;
mod structures;

use profile::{AiSlot, BotDecision, ai_profile_for, ai_tier};
use structures::{cheapest_gold_cost, think_window_ticks};

impl SowEngine {
    fn rebuild_ai_attack_index(&mut self) {
        if !self.ai_attack_index_dirty
            && self.ai_attack_index.len() == self.state.player_lookup.len()
        {
            return;
        }
        #[cfg(feature = "ai-metrics")]
        {
            self.bot_work.attack_entries_scanned_last_update += self.attacks.len() as u64;
        }
        self.ai_attack_index
            .resize_with(self.state.player_lookup.len(), Vec::new);
        for attacks in &mut self.ai_attack_index {
            attacks.clear();
        }
        for (attack_index, attack) in self.attacks.iter().enumerate() {
            if let Some(attacks) = self.ai_attack_index.get_mut(attack.target_owner as usize) {
                attacks.push(attack_index);
            }
        }
        self.ai_attack_index_dirty = false;
    }

    fn ai_is_under_attack(&self, bot_id: u16) -> (bool, u64) {
        let Some(player) = self.state.player(bot_id) else {
            return (false, 0);
        };
        #[cfg(feature = "ai-metrics")]
        let mut entries_examined = 0;
        let under_attack = self
            .ai_attack_index
            .get(bot_id as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .any(|&attack_index| {
                #[cfg(feature = "ai-metrics")]
                {
                    entries_examined += 1;
                }
                self.attacks
                    .get(attack_index)
                    .is_some_and(|attack| !player.alliances.contains(&attack.owner_id))
            });
        #[cfg(feature = "ai-metrics")]
        return (under_attack, entries_examined);
        #[cfg(not(feature = "ai-metrics"))]
        (under_attack, 0)
    }

    /// Unified AI pipeline for both Tribes (`Bot`) and Nations.
    ///
    /// - Builds one combined schedule of all AI entities.
    /// - Every scheduled bot acts each tick (no global budget cap).
    /// - Each bot self-throttles via `think_window_ticks` keyed on tier.
    /// - Uses `placement_scratch.border_scratch` for zero-allocation border scanning.
    pub fn execute_ai_think(&mut self) {
        if self.state.phase != crate::game::GamePhase::Playing {
            return;
        }

        #[cfg(test)]
        self.test_last_ai_intents.clear();

        let tick = self.state.tick;
        self.bot_work = Default::default();
        self.bot_route_cache.clear();
        self.bot_sam_tiles_cache = None;
        self.bot_crown_leader = None;
        #[cfg(feature = "ai-metrics")]
        {
            self.placement_scratch.candidates_examined = 0;
        }
        #[cfg(feature = "ai-metrics")]
        {
            self.placement_scratch.building_checks = 0;
        }
        self.rebuild_ai_attack_index();

        // ── Build unified schedule ──────────────────────────────────────────
        let mut schedule: Vec<AiSlot> = Vec::new();
        let mut any_structures = false;

        for p in self.state.players.iter() {
            let Some(tier) = ai_tier(p.player_type, p.is_ai_controlled) else {
                continue; // real human: no AI brain
            };
            if !p.alive || self.campaign_halted_ids.contains(&p.id) {
                continue;
            }
            let bot_id = p.id;
            let ranked_ghost = self.state.config.ranked_1v1 && tier == profile::AiTier::Ghost;
            let profile = if ranked_ghost {
                profile::ranked_ghost_profile(p.iq)
            } else {
                ai_profile_for(&self.state.config.ai, tier, self.state.config.bot_difficulty)
            };

            // Unified metronomic scheduler for Nations, Tribes, AND ghost
            // (is_ai_controlled) Humans. Tier is resolved once by
            // `ai_tier(player_type, is_ai_controlled)` — the single source of
            // truth. IQ (assigned per-tier at spawn) drives cadence; RNG is
            // WyRand(seed, bot_id, interval) → lockstep-safe across clients.
            let (is_under_attack, ghost_retaliation, _attack_entries_examined) =
                if tier == profile::AiTier::Ghost {
                    let (retaliation, examined) = self.ghost_retaliation_for(bot_id, tick);
                    (retaliation.is_some(), retaliation, examined)
                } else {
                    let (under_attack, examined) = self.ai_is_under_attack(bot_id);
                    (under_attack, None, examined)
                };
            #[cfg(feature = "ai-metrics")]
            {
                self.bot_work.attack_entries_scanned_last_update += _attack_entries_examined;
            }
            let reacts_to_attack = p.iq >= 100 && is_under_attack;

            let (min_interval, max_interval) = if ranked_ghost {
                let base = 5 + u64::from(180 - p.iq.clamp(60, 180)) / 3;
                (base, base * 2 - 1)
            } else {
                think_window_ticks(&self.state.config.ai, tier, reacts_to_attack)
            };

            let mut sched_rng = WyRand::new(
                self.state
                    .seed
                    .wrapping_add(bot_id as u64)
                    .wrapping_add(min_interval),
            );
            let interval = sched_rng
                .next_int(min_interval as i32, max_interval as i32 + 1)
                .max(1) as u64;
            let offset = sched_rng.next_int(0, interval as i32) as u64;

            let phase = tick % interval;
            let scheduled_attack = phase == offset;
            let retaliation_due =
                ghost_retaliation.is_some_and(|retaliation| retaliation.is_ready_at(tick));
            let do_attack = scheduled_attack || retaliation_due;

            let do_structures = !self.campaign_structures_held(bot_id) && if p.iq >= 100 {
                let one_third = (offset + interval / 3) % interval;
                let two_thirds = (offset + (interval * 2) / 3) % interval;
                scheduled_attack || phase == one_third || phase == two_thirds
            } else {
                scheduled_attack
            };

            #[cfg(feature = "ai-metrics")]
            if std::env::var("SOW_AI_DEBUG").is_ok() && bot_id == 1 && tick < 250 {
                eprintln!(
                    "SCHED id={bot_id} tick={tick} interval={interval} offset={offset} phase={phase} do_attack={do_attack}"
                );
            }
            if !do_attack && !do_structures {
                continue; // Nothing to do this tick for this entity
            }

            if do_structures && p.gold >= cheapest_gold_cost(&self.state.config) {
                any_structures = true;
            }

            schedule.push(AiSlot {
                bot_id,
                tier,
                do_attack,
                do_structures,
                is_under_attack,
                ghost_retaliation,
                profile,
            });
        }

        schedule.sort_unstable_by_key(|s| s.bot_id);
        #[cfg(feature = "ai-metrics")]
        if std::env::var("SOW_AI_DEBUG").is_ok() {
            let probes: Vec<(u16, usize)> = self
                .state
                .players
                .iter()
                .map(|p| (p.id, p.border_tiles.count_ones()))
                .collect();
            let mut neutral_probe: Vec<(u16, bool)> = Vec::new();
            for p in &self.state.players {
                let mut has = false;
                let mut border_sample = Vec::new();
                p.border_tiles.sample_ones(0, 256, &mut border_sample);
                for raw in border_sample {
                    let bx = raw % self.state.map.width;
                    let by = raw / self.state.map.width;
                    self.state.map.for_each_neighbor(bx, by, |nx, ny| {
                        if self.state.map.owner_id(nx, ny) == 0
                            && self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land()
                        {
                            has = true;
                        }
                    });
                }
                neutral_probe.push((p.id, has));
            }
            eprintln!(
                "AICALL tick={} sched={} ids={:?} borders={:?} neutral={:?}",
                self.state.tick,
                schedule.len(),
                schedule.iter().map(|s| s.bot_id).collect::<Vec<_>>(),
                probes,
                neutral_probe
            );
        }
        if schedule.is_empty() {
            return;
        }

        // ── Refresh shared building/boat counts once for this AI update ────
        let any_attacks = schedule.iter().any(|slot| slot.do_attack);
        if any_structures {
            self.refresh_building_grid();
        }
        if any_structures || any_attacks {
            self.refresh_building_aggregates();
        }
        if any_attacks {
            self.refresh_bot_naval_slots_used();
            self.bot_naval_cursors
                .resize(self.state.player_lookup.len(), Default::default());
        }

        // ── Process all scheduled bots (no global cap) ─────
        let mut decisions: Vec<BotDecision> = Vec::new();

        for slot in &schedule {
            let bot_id = slot.bot_id;

            let bot_iq = {
                let Some(player) = self.state.player(bot_id) else {
                    continue;
                };
                player.iq
            };

            // Smarter AIs are highly efficient and have much lower action costs!
            let (attack_cost, build_cost, alliance_cost, send_cost) = if bot_iq >= 130 {
                (5.0, 5.0, 5.0, 5.0)
            } else if bot_iq >= 100 {
                (5.0, 5.0, 5.0, 999.0)
            } else {
                (10.0, 10.0, 10.0, 999.0)
            };

            let (neighbor_players, has_neutral) = self.nation_scan_neighbors(bot_id);

            self.nation_run_diplomacy_for_slot(
                (bot_id, bot_iq),
                (alliance_cost, send_cost),
                &neighbor_players,
                has_neutral,
                slot.is_under_attack,
                &mut decisions,
            );

            self.nation_run_structure_build_for_slot(
                slot,
                bot_id,
                bot_iq,
                build_cost,
                &mut decisions,
            );

            self.nation_run_combat_for_slot(
                slot,
                (bot_id, bot_iq),
                (attack_cost, alliance_cost),
                &neighbor_players,
                has_neutral,
                &mut decisions,
            );

            if slot.do_attack
                && !self.fleets.iter().any(|fleet| {
                    fleet.owner_id == bot_id
                        && fleet.unit_type == crate::game::UnitType::Warship
                        && fleet.troops > 0.0
                })
                && self.state.player(bot_id).is_some_and(|player| {
                    let owned_warships = self
                        .fleets
                        .iter()
                        .filter(|fleet| {
                            fleet.owner_id == bot_id
                                && fleet.unit_type == crate::game::UnitType::Warship
                        })
                        .count() as u32;
                    player.gold
                        >= crate::game::UnitType::Warship
                            .gold_cost_with_owned_count_for_leader(owned_warships, player.leader)
                        && self.boat_slots_used(bot_id)
                            < crate::building::player_fleet_capacity(&self.buildings, bot_id)
                })
                && self.campaign_allows_action(crate::campaign::CampaignAction::Warship)
                && let Some(target_tile) = self.ai_warship_patrol_target(bot_id)
            {
                decisions.push(BotDecision {
                    bot_id,
                    kind: profile::BotDecisionKind::Build,
                    intent: crate::protocol::GameplayIntent::BuildShip {
                        target_tile,
                        kind: crate::game::UnitType::Warship,
                    },
                });
            }

            self.placement_scratch.neighbor_scratch = neighbor_players;
        }

        #[cfg(feature = "ai-metrics")]
        {
            self.bot_work.placement_candidates_examined +=
                self.placement_scratch.candidates_examined;
            self.bot_work.placement_building_checks += self.placement_scratch.building_checks;
        }
        #[cfg(feature = "ai-metrics")]
        {
            self.placement_scratch.candidates_examined = 0;
        }
        #[cfg(feature = "ai-metrics")]
        {
            self.placement_scratch.building_checks = 0;
        }
        #[cfg(feature = "ai-metrics")]
        if std::env::var("SOW_AI_DEBUG").is_ok() {
            eprintln!(
                "AIWORK tick={} attack_entries={} border_cells={} border_blocks={} border_directory_words={} neighbor_cells={} placement_candidates={} placement_building_checks={} naval_routes={} naval_border_candidates={} shoreline_candidates={} diplomacy_proposals={} diplomacy_resource_requests={} nuke_buildings={} nuke_sam_checks={} nuke_history_lookups={}",
                self.state.tick,
                self.bot_work.attack_entries_scanned_last_update,
                self.bot_work.border_cells_examined,
                self.bot_work.border_blocks_examined,
                self.bot_work.border_directory_words_examined,
                self.bot_work.neighbor_cells_examined,
                self.bot_work.placement_candidates_examined,
                self.bot_work.placement_building_checks,
                self.bot_work.naval_routes_calculated,
                self.bot_work.naval_border_candidates_examined,
                self.bot_work.shoreline_candidates_examined,
                self.bot_work.diplomacy_proposals_examined,
                self.bot_work.diplomacy_resource_requests_examined,
                self.bot_work.nuke_buildings_examined,
                self.bot_work.nuke_sam_checks,
                self.bot_work.nuke_history_lookups,
            );
        }

        // ── Apply decisions deterministically ───────────────────────────────
        decisions.sort_by_key(|d| (d.bot_id, d.kind));
        #[cfg(test)]
        {
            self.test_last_ai_intents = decisions
                .iter()
                .map(|d| StampedIntent {
                    player_id: d.bot_id,
                    intent: d.intent.clone(),
                })
                .collect();
        }
        for (intent_index, d) in decisions.into_iter().enumerate() {
            let fleet = match &d.intent {
                crate::protocol::GameplayIntent::LaunchFleet {
                    target_tile,
                    troops,
                } => Some((*target_tile, *troops)),
                _ => None,
            };
            let route =
                fleet.and_then(|(target_tile, _)| self.take_bot_route(d.bot_id, target_tile));
            let stamped = StampedIntent {
                player_id: d.bot_id,
                intent: d.intent,
            };
            if let Some(route) = route {
                self.apply_stamped_intent_with_fleet_route(&stamped, route, intent_index as u32);
            } else {
                self.apply_stamped_intent(&stamped, intent_index as u32);
            }
        }
    }

    fn ai_warship_patrol_target(&self, owner_id: u16) -> Option<u32> {
        let width = self.state.map.width;
        if width == 0 {
            return None;
        }
        let connected_components: std::collections::HashSet<u32> = self.buildings.iter()
            .filter(|port| port.owner_id == owner_id
                && port.kind == crate::game::BuildingKind::Port
                && !port.under_construction
                && port.active_level() >= 1)
            .flat_map(|port| {
                let footprint = crate::building::BuildingFootprint::at(
                    crate::game::BuildingKind::Port,
                    port.tile_idx % width,
                    port.tile_idx / width,
                );
                let mut components = Vec::new();
                for y in footprint.top..footprint.top + footprint.height as i32 {
                    for x in footprint.left..footprint.left + footprint.width as i32 {
                        if self.state.map.is_valid_coord(x, y) {
                            self.state.map.for_each_neighbor(x as u32, y as u32, |nx, ny| {
                                if !footprint.contains(nx as i32, ny as i32) {
                                    let tile = ny * width + nx;
                                    if self.state.map.terrain[tile as usize].is_water() {
                                        let component = self.water.component_of(tile);
                                        if component != 0 { components.push(component); }
                                    }
                                }
                            });
                        }
                    }
                }
                components
            })
            .collect();
        self.fleets.iter()
            .filter(|fleet| fleet.troops > 0.0
                && fleet.owner_id != owner_id
                && self.naval_target_allowed(owner_id, fleet.owner_id)
                && connected_components.contains(&self.water.component_of(fleet.current_tile)))
            .min_by_key(|fleet| {
                let priority = match fleet.unit_type {
                    crate::game::UnitType::TransportShip => 0,
                    crate::game::UnitType::Warship => 1,
                    crate::game::UnitType::TradeShip => 2,
                };
                (priority, fleet.id)
            })
            .map(|fleet| fleet.current_tile)
            .or_else(|| self.buildings.iter().filter(|port| {
                port.owner_id == owner_id
                    && port.kind == crate::game::BuildingKind::Port
                    && !port.under_construction
                    && port.active_level() >= 1
            }).flat_map(|port| {
                let footprint = crate::building::BuildingFootprint::at(
                    crate::game::BuildingKind::Port,
                    port.tile_idx % width,
                    port.tile_idx / width,
                );
                let mut shores = Vec::new();
                for y in footprint.top..footprint.top + footprint.height as i32 {
                    for x in footprint.left..footprint.left + footprint.width as i32 {
                        if self.state.map.is_valid_coord(x, y) {
                            self.state.map.for_each_neighbor(x as u32, y as u32, |nx, ny| {
                                if !footprint.contains(nx as i32, ny as i32) {
                                    let tile = ny * width + nx;
                                    if self.state.map.terrain[tile as usize].is_water()
                                        && connected_components.contains(&self.water.component_of(tile))
                                    {
                                        shores.push(tile);
                                    }
                                }
                            });
                        }
                    }
                }
                shores
            }).min())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "bot_lab.rs"]
mod bot_lab;
