use crate::engine::SowEngine;
use crate::protocol::StampedIntent;
use crate::rng::NextIntExt;
use wyrand::WyRand;

mod build;
mod combat;
mod diplomacy;
mod pro_ai;
pub(crate) mod profile;
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
        let under_attack = self
            .ai_attack_index
            .get(bot_id as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .any(|&attack_index| {
                self.attacks
                    .get(attack_index)
                    .is_some_and(|attack| !player.alliances.contains(&attack.owner_id))
            });
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
            let policy = profile::AiBehaviorPolicy::for_tier(tier);
            let (is_under_attack, ghost_retaliation, _attack_entries_examined) =
                if policy.uses_retaliation_ai {
                    let (retaliation, examined) = self.ghost_retaliation_for(bot_id, tick);
                    (retaliation.is_some(), retaliation, examined)
                } else {
                    let (under_attack, examined) = self.ai_is_under_attack(bot_id);
                    (under_attack, None, examined)
                };
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
        let num_lookup = self.state.player_lookup.len();
        let mut alive_warships_by_player: Vec<bool> = Vec::new();
        let mut owned_warships_by_player: Vec<u32> = Vec::new();
        let mut queued_warships_by_player: Vec<u32> = Vec::new();
        if any_attacks {
            self.refresh_bot_naval_slots_used();
            self.bot_naval_cursors
                .resize(num_lookup, Default::default());

            alive_warships_by_player.resize(num_lookup, false);
            owned_warships_by_player.resize(num_lookup, 0);
            queued_warships_by_player.resize(num_lookup, 0);
            for fleet in &self.fleets {
                if fleet.unit_type == crate::game::UnitType::Warship {
                    let pid = usize::from(fleet.owner_id);
                    if pid < num_lookup {
                        owned_warships_by_player[pid] =
                            owned_warships_by_player[pid].saturating_add(1);
                        if fleet.troops > 0.0 {
                            alive_warships_by_player[pid] = true;
                        }
                    }
                }
            }
            if !self.building_productions.is_empty() {
                for b in &self.buildings {
                    let pid = usize::from(b.owner_id);
                    if pid < num_lookup
                        && let Some(state) = self.building_productions.get(&b.id)
                    {
                        let queued = state.warship_queue_len() as u32;
                        if queued > 0 {
                            queued_warships_by_player[pid] =
                                queued_warships_by_player[pid].saturating_add(queued);
                        }
                    }
                }
            }
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

            let is_pro = slot.is_pro(
                bot_iq,
                self.state.config.bot_difficulty,
                self.state.config.ranked_1v1,
            );
            if slot.do_attack && is_pro {
                let pid = usize::from(bot_id);
                let owned_warships = owned_warships_by_player.get(pid).copied().unwrap_or(0);
                let queued_warships = queued_warships_by_player.get(pid).copied().unwrap_or(0);
                let used_slots = self.bot_naval_slots_used.get(pid).copied().unwrap_or(0);
                self.pro_fleet_decisions(
                    bot_id,
                    owned_warships,
                    queued_warships,
                    used_slots,
                    &mut decisions,
                );
            } else if slot.do_attack
                && (self.legacy_building_rules || !structures::has_pending_build(&decisions, bot_id))
                && !alive_warships_by_player
                    .get(usize::from(bot_id))
                    .copied()
                    .unwrap_or(false)
                && self.state.player(bot_id).is_some_and(|player| {
                    let pid = usize::from(bot_id);
                    let owned_warships = owned_warships_by_player.get(pid).copied().unwrap_or(0);
                    let queued_warships = queued_warships_by_player.get(pid).copied().unwrap_or(0);
                    let cost = if self.legacy_building_rules {
                        crate::game::UnitType::Warship
                            .gold_cost_with_owned_count_for_leader(owned_warships, player.leader)
                    } else {
                        crate::building::city_arsenal_cost_gold(
                            crate::game::CityArsenalKind::Warship,
                            owned_warships.saturating_add(queued_warships),
                            player.leader,
                            &self.state.config,
                        )
                    };
                    let used_slots = self.bot_naval_slots_used.get(pid).copied().unwrap_or(0);
                    let cap = crate::building::cost::player_fleet_capacity_from_port_slots(
                        self.building_aggregates
                            .get(pid)
                            .map_or(0, |agg| agg.port_fleet_slots),
                    );
                    player.gold >= cost && used_slots.saturating_add(queued_warships) < cap
                })
                && self.campaign_allows_action(crate::campaign::CampaignAction::Warship)
            {
                if !self.legacy_building_rules {
                    if let Some(port_id) = self.ai_warship_port_id(bot_id) {
                        decisions.push(BotDecision {
                            bot_id,
                            kind: profile::BotDecisionKind::Build,
                            intent: crate::protocol::GameplayIntent::BuildingProduction {
                                building_id: port_id,
                                kind: crate::game::CityArsenalKind::Warship,
                            },
                        });
                    }
                } else if let Some(target_tile) = self.ai_warship_patrol_target(bot_id) {
                    decisions.push(BotDecision {
                        bot_id,
                        kind: profile::BotDecisionKind::Build,
                        intent: crate::protocol::GameplayIntent::BuildShip {
                            target_tile,
                            kind: crate::game::UnitType::Warship,
                        },
                    });
                }
            }

            self.placement_scratch.neighbor_scratch = neighbor_players;
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

    fn ai_warship_port_id(&self, owner_id: u16) -> Option<u64> {
        let mut fallback: Option<u64> = None;
        for port in self.buildings.iter().filter(|port| {
            port.owner_id == owner_id
                && port.kind == crate::game::BuildingKind::Port
                && !port.under_construction
                && port.active_level() >= 1
                && self
                    .building_productions
                    .get(&port.id)
                    .is_none_or(|state| !state.is_building())
        }) {
            let Some(spawn_tile) = crate::warp_fleet::coastal_warship_spawn_tile(
                &self.state.map,
                &self.water,
                port.tile_idx,
            ) else {
                continue;
            };
            let spawn_component = self.water.component_of(spawn_tile);
            let touches_enemy_fleet = self.fleets.iter().any(|fleet| {
                fleet.troops > 0.0
                    && fleet.owner_id != owner_id
                    && self.naval_target_allowed(owner_id, fleet.owner_id)
                    && self.water.component_of(fleet.current_tile) == spawn_component
            });
            if touches_enemy_fleet {
                return Some(port.id);
            }
            if fallback.is_none() {
                fallback = Some(port.id);
            }
        }
        fallback
    }

    fn ai_warship_patrol_target(&self, owner_id: u16) -> Option<u32> {
        if self.state.map.width == 0 {
            return None;
        }
        let mut connected_components = std::collections::HashSet::new();
        let mut first_shore: Option<u32> = None;
        for port in self.buildings.iter().filter(|port| {
            port.owner_id == owner_id
                && port.kind == crate::game::BuildingKind::Port
                && !port.under_construction
                && port.active_level() >= 1
        }) {
            crate::building::for_each_structure_coastal_water_tile(
                &self.state.map,
                crate::game::BuildingKind::Port,
                port.tile_idx,
                |tile| {
                    let component = self.water.component_of(tile);
                    if component != 0 {
                        connected_components.insert(component);
                        first_shore = Some(first_shore.map_or(tile, |cur| cur.min(tile)));
                    }
                },
            );
        }
        self.fleets
            .iter()
            .filter(|fleet| {
                fleet.troops > 0.0
                    && fleet.owner_id != owner_id
                    && self.naval_target_allowed(owner_id, fleet.owner_id)
                    && connected_components.contains(&self.water.component_of(fleet.current_tile))
            })
            .min_by_key(|fleet| {
                let priority = match fleet.unit_type {
                    crate::game::UnitType::TransportShip => 0,
                    crate::game::UnitType::Warship => 1,
                    crate::game::UnitType::TradeShip => 2,
                };
                (priority, fleet.id)
            })
            .map(|fleet| fleet.current_tile)
            .or(first_shore)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "bot_lab.rs"]
mod bot_lab;
