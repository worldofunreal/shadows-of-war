use crate::diplomacy::maybe_betray_for_attack;
use crate::engine::SowEngine;
use crate::game::{BuildingKind, NukeKind};
use crate::protocol::{AttackIntent, GameplayIntent};
use crate::rng::NextIntExt;
use std::collections::{BTreeSet, HashMap};
use wyrand::WyRand;

use super::profile::{AiSlot, AiTier, BotDecision, BotDecisionKind, GhostRetaliation};

const GHOST_RETALIATION_DELAY_SALT: u64 = 0x4752_4459_0000_0001;
const GHOST_RETALIATION_FORCE_SALT: u64 = 0x4752_4643_0000_0002;
const BOT_NAVAL_CANDIDATE_BUDGET: usize = 256;
const BOT_NAVAL_LOCAL_BUDGET: usize = 192;
const BOT_NAVAL_LOCAL_RADIUS: u32 = 150;
const BOT_NAVAL_CHANCE_SALT: u64 = 0x4E41_5641_4C00_0003;
const BOT_NAVAL_SOURCE_SALT: u64 = 0x4E41_5653_5243_0001;
const VANILLA_TRIBE_TARGET_SALT: u64 = 0x5452_4942_455F_0001;
const HUMAN_ATTACK_GRACE_MS: f64 = 120_000.0;

struct NavalDecisionContext {
    attacks_players: bool,
    campaign_target: Option<u16>,
    defender_target: Option<u16>,
    has_land_move: bool,
    force_naval: bool,
    has_hostile_neighbors: bool,
    ghost_tribe_bypass: bool,
    vanilla_tribe: bool,
}

struct BotSurroundings {
    borders_actor: bool,
    open: bool,
    bots: BTreeSet<u16>,
}

#[inline]
fn ghost_attack_rng(seed: u64, ghost_id: u16, attack_id: u64, salt: u64) -> WyRand {
    let ghost_key = u64::from(ghost_id).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let attack_key = attack_id.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    WyRand::new(seed ^ ghost_key ^ attack_key ^ salt)
}

pub(super) fn retaliation_delay_ticks_for_tier(
    ai: &crate::game_config::AiTuning,
    tier: AiTier,
    seed: u64,
    actor_id: u16,
    attack_id: u64,
) -> u64 {
    let (min, max) = super::structures::think_window_ticks(ai, tier, true);
    let mut rng = ghost_attack_rng(seed, actor_id, attack_id, GHOST_RETALIATION_DELAY_SALT);
    rng.next_int(min as i32, max as i32 + 1) as u64
}

#[cfg(test)]
pub(super) fn ghost_retaliation_delay_ticks(
    ai: &crate::game_config::AiTuning,
    seed: u64,
    ghost_id: u16,
    attack_id: u64,
) -> u64 {
    retaliation_delay_ticks_for_tier(ai, AiTier::Ghost, seed, ghost_id, attack_id)
}

pub(super) fn ranked_ghost_retaliation_delay_ticks(
    seed: u64,
    ghost_id: u16,
    attack_id: u64,
    iq: u32,
) -> u64 {
    let skill_gap = 180 - iq.clamp(60, 180);
    let min_delay = 1 + u64::from(skill_gap * 2 / 120);
    let max_delay = 3 + u64::from(skill_gap * 9 / 120);
    let mut rng = ghost_attack_rng(seed, ghost_id, attack_id, GHOST_RETALIATION_DELAY_SALT);
    let roll = rng.next_int(0, 100) as u64;
    min_delay + roll * (max_delay - min_delay + 1) / 100
}

pub(super) fn ghost_retaliation_send_troops(
    seed: u64,
    ghost_id: u16,
    retaliation: GhostRetaliation,
    troops: f64,
    max_troops: f64,
    reserve_ratio: f64,
) -> f64 {
    let mut rng = ghost_attack_rng(
        seed,
        ghost_id,
        retaliation.attack_id,
        GHOST_RETALIATION_FORCE_SALT,
    );
    let force_ratio = rng.next_int(75, 126) as f64 / 100.0;
    let defensive_reserve = (max_troops * reserve_ratio * 0.5).max(0.0);
    let available = (troops - defensive_reserve).max(0.0);
    (retaliation.incoming_troops.max(0.0) * force_ratio).min(available)
}

#[inline]
fn is_real_human(player: &crate::player::Player) -> bool {
    player.is_human() && !player.is_ai_controlled
}

#[inline]
fn vanilla_tribe_reluctant_target(player: &crate::player::Player) -> bool {
    is_real_human(player)
        || player.player_type == crate::player::PlayerType::Nation
        || (player.player_type == crate::player::PlayerType::Human && player.is_ai_controlled)
}

#[inline]
fn vanilla_tribe_skips_target(
    seed: u64,
    tribe_id: u16,
    target_id: u16,
    tick: u64,
    chance: i32,
) -> bool {
    let mut rng = WyRand::new(
        seed ^ u64::from(tribe_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(target_id).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ tick.wrapping_mul(0xA076_1D64_78BD_642F)
            ^ VANILLA_TRIBE_TARGET_SALT,
    );
    rng.next_int(0, 100) < chance
}

#[inline]
fn human_betrayal_allowed(
    allows_human_betrayal: bool,
    relation: Option<crate::protocol::CampaignRelation>,
) -> bool {
    match relation {
        Some(_) => false,
        None => allows_human_betrayal,
    }
}

#[inline]
fn campaign_attacks_players(
    relation: Option<crate::protocol::CampaignRelation>,
    campaign_override: Option<bool>,
    profile_default: bool,
) -> bool {
    match relation {
        Some(crate::protocol::CampaignRelation::Enemy) => campaign_override.unwrap_or(true),
        Some(
            crate::protocol::CampaignRelation::Allied | crate::protocol::CampaignRelation::Neutral,
        ) => false,
        None => profile_default,
    }
}

impl SowEngine {
    /// O(1) zero-allocation prize & defense estimate from pre-aggregated buildings.
    #[inline(always)]
    fn ai_capturable_structure_prize(&self, target_id: u16) -> (f64, u32) {
        let Some(agg) = self.building_aggregates.get(usize::from(target_id)) else {
            return (0.0, 0);
        };
        let prize = 45.0 * f64::from(agg.count_city)
            + 25.0 * f64::from(agg.city_levels)
            + 35.0 * f64::from(agg.count_trading_post)
            + 20.0 * f64::from(agg.trading_post_levels)
            + 30.0 * f64::from(agg.count_port)
            + 15.0 * f64::from(agg.port_levels)
            + 20.0 * f64::from(agg.count_military_camp)
            + 10.0 * f64::from(agg.military_camp_levels);
        (prize, agg.tower_levels)
    }

    /// O(1) zero-allocation prey score: prioritizes high tile+building yield per
    /// effective defending troop, tribes as map food, and depleted/disconnected targets.
    #[inline(always)]
    fn ai_target_prey_score(&self, attacker_troops: f64, target_id: u16) -> f64 {
        let Some(target) = self.state.player(target_id) else {
            return -1.0;
        };
        let (prize_tiles, tower_levels) = self.ai_capturable_structure_prize(target_id);
        let target_troops = target.troops.max(0.0);
        let effective_troops =
            target_troops.max(25.0) * (1.0 + 0.15 * f64::from(tower_levels));
        let base_yield = (f64::from(target.tile_count.max(1)) + prize_tiles) / effective_troops;
        let depletion = (1.0 - (target_troops / target.max_troops.max(1.0))).clamp(0.0, 1.0);
        let mut mult = 1.0 + depletion;
        if target.player_type == crate::player::PlayerType::Bot {
            mult *= 2.25;
        } else if prize_tiles == 0.0 || target_troops > attacker_troops * 0.65 {
            mult *= 0.25;
        }
        if target.disconnected {
            mult *= 1.35;
        }
        base_yield * mult
    }

    pub(crate) fn naval_target_allowed(&self, attacker_id: u16, target_id: u16) -> bool {
        if attacker_id == target_id {
            return false;
        }
        let respect_human_attack_grace = !self.state.player(attacker_id).is_some_and(is_real_human);
        self.ai_target_allowed_with_grace(attacker_id, target_id, None, respect_human_attack_grace)
            && !self.players_friendly(attacker_id, target_id)
            && self
                .state
                .player(target_id)
                .is_some_and(|player| player.alive)
    }

    fn human_attack_grace_elapsed(&self) -> bool {
        let tick_rate_ms =
            if self.state.config.tick_rate_ms.is_finite() && self.state.config.tick_rate_ms > 0.0 {
                self.state.config.tick_rate_ms
            } else {
                100.0
            };
        let playing_start_tick = if self.state.config.random_spawn {
            0
        } else {
            crate::game::deployment_phase_end_tick(tick_rate_ms)
        };
        let elapsed_ticks = self.state.tick.saturating_sub(playing_start_tick);
        (elapsed_ticks as f64 * f64::from(tick_rate_ms)) >= HUMAN_ATTACK_GRACE_MS
    }

    fn is_attacked_by(&self, defender_id: u16, attacker_id: u16) -> bool {
        self.ai_attack_index
            .get(defender_id as usize)
            .is_some_and(|indices| {
                indices.iter().any(|&attack_index| {
                    self.attacks.get(attack_index).is_some_and(|attack| {
                        attack.owner_id == attacker_id
                            && attack.target_owner == defender_id
                            && !attack.retreating
                    })
                })
            })
    }

    /// Real humans are protected during the opening grace period, except in
    /// Co-op and while disconnected. Campaign relations remain the
    /// authority for scripted factions, and retaliation is always eligible.
    fn ai_target_allowed(
        &self,
        attacker_id: u16,
        target_id: u16,
        defender_target: Option<u16>,
    ) -> bool {
        self.ai_target_allowed_with_grace(attacker_id, target_id, defender_target, true)
    }

    fn ai_target_allowed_with_grace(
        &self,
        attacker_id: u16,
        target_id: u16,
        defender_target: Option<u16>,
        respect_human_attack_grace: bool,
    ) -> bool {
        if defender_target == Some(target_id) || self.is_attacked_by(attacker_id, target_id) {
            return true;
        }
        let Some(target) = self.state.player(target_id) else {
            return false;
        };
        if !is_real_human(target) {
            return true;
        }

        if self.campaign_relations.contains_key(&attacker_id) {
            let relation = self.campaign_relations.get(&attacker_id).copied();
            let override_attack = self.campaign_can_attack_player.get(&attacker_id).copied();
            return campaign_attacks_players(relation, override_attack, true);
        }

        (crate::game_config::is_coop_game_mode(&self.state.config.game_mode)
            && self
                .state
                .player(attacker_id)
                .is_some_and(|attacker| attacker.player_type == crate::player::PlayerType::Nation))
            || target.disconnected
            || !respect_human_attack_grace
            || self.human_attack_grace_elapsed()
    }

    pub(super) fn ghost_retaliation_for(
        &self,
        ghost_id: u16,
        tick: u64,
    ) -> (Option<GhostRetaliation>, u64) {
        let Some(defender) = self.state.player(ghost_id) else {
            return (None, 0);
        };
        let tier = super::profile::ai_tier(defender.player_type, defender.is_ai_controlled)
            .unwrap_or(AiTier::Ghost);
        let Some(inbound_attacks) = self.ai_attack_index.get(ghost_id as usize) else {
            return (None, 0);
        };
        let mut largest_ready: Option<GhostRetaliation> = None;
        let mut largest_waiting: Option<GhostRetaliation> = None;

        for &attack_index in inbound_attacks {
            let Some(attack) = self.attacks.get(attack_index) else {
                continue;
            };
            if attack.target_owner != ghost_id || attack.owner_id == ghost_id || attack.retreating {
                continue;
            }
            let Some(attacker) = self.state.player(attack.owner_id) else {
                continue;
            };
            let is_friendly = defender.alliances.contains(&attack.owner_id)
                || (defender.team.is_some() && defender.team == attacker.team);
            if !attacker.alive || is_friendly {
                continue;
            }
            let already_counterattacking = self
                .ai_attack_index
                .get(attack.owner_id as usize)
                .is_some_and(|outgoing_attacks| {
                    outgoing_attacks.iter().any(|&outgoing_index| {
                        self.attacks.get(outgoing_index).is_some_and(|outgoing| {
                            outgoing.owner_id == ghost_id
                                && outgoing.target_owner == attack.owner_id
                                && !outgoing.retreating
                        })
                    })
                });
            if already_counterattacking {
                continue;
            }

            let delay = if self.state.config.ranked_1v1 {
                ranked_ghost_retaliation_delay_ticks(
                    self.state.seed,
                    ghost_id,
                    attack.id,
                    defender.iq,
                )
            } else {
                retaliation_delay_ticks_for_tier(
                    &self.state.config.ai,
                    tier,
                    self.state.seed,
                    ghost_id,
                    attack.id,
                )
            };
            let retaliation = GhostRetaliation {
                attack_id: attack.id,
                attacker_id: attack.owner_id,
                incoming_troops: attack.troops,
                ready_tick: attack.created_tick.saturating_add(delay),
            };
            let chosen = if retaliation.is_ready_at(tick) {
                &mut largest_ready
            } else {
                &mut largest_waiting
            };
            if chosen.is_none_or(|current| retaliation.incoming_troops > current.incoming_troops) {
                *chosen = Some(retaliation);
            }
        }

        let retaliation = largest_ready.or(largest_waiting);
        (retaliation, 0)
    }

    fn nation_launch_campaign_assault_fleet(
        &mut self,
        bot_id: u16,
        target_id: u16,
        troops: f64,
        decisions: &mut Vec<BotDecision>,
    ) -> bool {
        if !self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet) {
            return false;
        }
        use crate::warp_fleet::resolve_fleet_route;

        if troops < self.state.config.attack_cost_neutral {
            return false;
        }
        let routed_target = {
            let Some(target) = self.state.player(target_id) else {
                return false;
            };
            if !target.alive || target.border_tiles.is_empty() {
                return false;
            }
            let start = self.state.tick.wrapping_add(bot_id as u64) as u32;
            let Some(target_tile) = target.border_tiles.first_one_from(start) else {
                return false;
            };
            let border_tiles = &self.state.player(bot_id).unwrap().border_tiles;
            #[cfg(test)]
            {
                self.bot_work.naval_routes_calculated += 1;
            }
            let route = resolve_fleet_route(
                &self.state.map,
                &self.water,
                &mut self.path_scratch,
                bot_id,
                (target_id, target_tile),
                border_tiles,
                Some(&target.border_tiles),
            );
            route.ok().map(|route| (target_tile, route))
        };
        let Some((target_tile, route)) = routed_target else {
            return false;
        };
        self.cache_bot_route(bot_id, target_tile, route);
        decisions.push(BotDecision {
            bot_id,
            kind: BotDecisionKind::Attack,
            intent: GameplayIntent::LaunchFleet {
                target_tile,
                troops: Some(troops),
            },
        });
        true
    }

    fn scan_bot_surroundings(
        &self,
        actor_id: u16,
        bot_id: u16,
        closed_only: bool,
        within: Option<&BTreeSet<u16>>,
    ) -> Option<BotSurroundings> {
        let map = &self.state.map;
        let bot = self.state.player(bot_id)?;
        if map.width == 0 {
            return None;
        }
        let mut result = BotSurroundings {
            borders_actor: false,
            open: false,
            bots: BTreeSet::new(),
        };
        let mut gave_up = false;
        for (x, y) in bot.border_coords(map.width) {
            if gave_up {
                break;
            }
            if x == 0
                || y == 0
                || x + 1 == map.width
                || y + 1 == map.height
                || map.terrain[map.ref_id(x, y)].is_shoreline()
            {
                result.open = true;
                gave_up = closed_only;
                continue;
            }
            map.for_each_neighbor4(x, y, |nx, ny| {
                if gave_up {
                    return;
                }
                let owner_id = map.owner_id(nx, ny);
                if owner_id == bot_id {
                    return;
                }
                if owner_id == actor_id {
                    result.borders_actor = true;
                    return;
                }
                let attackable_bot = self.state.player(owner_id).is_some_and(|owner| {
                    owner.player_type == crate::player::PlayerType::Bot
                        && !self.players_friendly(actor_id, owner_id)
                });
                if !attackable_bot {
                    result.open = true;
                    gave_up = closed_only;
                } else if within.is_some_and(|within| !within.contains(&owner_id)) {
                    gave_up = true;
                } else {
                    result.bots.insert(owner_id);
                }
            });
        }
        (!gave_up).then_some(result)
    }

    pub(crate) fn players_friendly(&self, first_id: u16, second_id: u16) -> bool {
        let (Some(first), Some(second)) =
            (self.state.player(first_id), self.state.player(second_id))
        else {
            return false;
        };
        first.alliances.contains(&second_id)
            || second.alliances.contains(&first_id)
            || (first.team.is_some() && first.team == second.team)
    }

    fn bot_is_attacked_by(&self, actor_id: u16, target_id: u16) -> bool {
        self.attacks
            .iter()
            .any(|attack| attack.owner_id == actor_id && attack.target_owner == target_id)
    }

    fn bot_has_structures(&self, bot_id: u16) -> bool {
        self.buildings
            .iter()
            .any(|building| building.owner_id == bot_id)
    }

    fn walled_in_bot_targets(&self, actor_id: u16, bots: &[u16]) -> Vec<u16> {
        let no_other_bots = BTreeSet::new();
        bots.iter()
            .copied()
            .filter(|&bot_id| !self.bot_is_attacked_by(actor_id, bot_id))
            .filter(|&bot_id| {
                self.scan_bot_surroundings(actor_id, bot_id, true, Some(&no_other_bots))
                    .is_some_and(|surroundings| surroundings.borders_actor)
            })
            .collect()
    }

    fn plan_bot_attacks(&self, actor_id: u16, bots: &[u16]) -> Vec<u16> {
        let mut region = HashMap::new();
        for &bot_id in bots {
            if let Some(surroundings) = self.scan_bot_surroundings(actor_id, bot_id, false, None) {
                region.insert(bot_id, surroundings);
            }
        }
        let front: Vec<u16> = bots
            .iter()
            .copied()
            .filter(|bot_id| {
                region
                    .get(bot_id)
                    .is_some_and(|surroundings: &BotSurroundings| surroundings.borders_actor)
            })
            .collect();
        let mut behind = BTreeSet::new();
        for &bot_id in &front {
            let others = region
                .get(&bot_id)
                .map(|surroundings| surroundings.bots.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            for other_id in others {
                if region.contains_key(&other_id) || !behind.insert(other_id) {
                    continue;
                }
                if let Some(surroundings) =
                    self.scan_bot_surroundings(actor_id, other_id, true, None)
                {
                    region.insert(other_id, surroundings);
                }
            }
        }

        let mut candidates: Vec<u16> = region
            .iter()
            .filter_map(|(&bot_id, surroundings)| {
                (!surroundings.open
                    && !surroundings.bots.is_empty()
                    && !self.bot_is_attacked_by(actor_id, bot_id)
                    && !self.bot_has_structures(bot_id))
                .then_some(bot_id)
            })
            .collect();
        candidates.sort_by(|a, b| {
            let a_tiles = self.state.player(*a).map_or(0, |player| player.tile_count);
            let b_tiles = self.state.player(*b).map_or(0, |player| player.tile_count);
            b_tiles.cmp(&a_tiles).then_with(|| a.cmp(b))
        });

        let mut kept = BTreeSet::new();
        let mut next_to_kept = BTreeSet::new();
        let mut walled_in = HashMap::<u16, f64>::new();
        for bot_id in candidates {
            if next_to_kept.contains(&bot_id) {
                continue;
            }
            kept.insert(bot_id);
            let surroundings = &region[&bot_id];
            let tile_count = self
                .state
                .player(bot_id)
                .map_or(0, |player| player.tile_count);
            let share = tile_count as f64 / surroundings.bots.len() as f64;
            for &other_id in &surroundings.bots {
                next_to_kept.insert(other_id);
                *walled_in.entry(other_id).or_default() += share;
            }
        }

        let value = |bot_id: u16| {
            let player = self.state.player(bot_id);
            let tiles = player.map_or(0, |player| player.tile_count) as f64;
            let troops = player.map_or(0.0, |player| player.troops).max(1.0);
            (tiles + walled_in.get(&bot_id).copied().unwrap_or(0.0)) / troops
        };
        let mut targets: Vec<u16> = bots
            .iter()
            .copied()
            .filter(|bot_id| {
                let Some(surroundings) = region.get(bot_id) else {
                    return false;
                };
                let walled = surroundings.borders_actor
                    && !surroundings.open
                    && surroundings.bots.is_empty();
                !kept.contains(bot_id) && !walled
            })
            .collect();
        targets.sort_by(|a, b| value(*b).total_cmp(&value(*a)).then_with(|| a.cmp(b)));
        if !targets.iter().any(|bot_id| {
            region
                .get(bot_id)
                .is_some_and(|surroundings| surroundings.borders_actor)
        }) {
            let mut fallback: Vec<u16> = front
                .into_iter()
                .filter(|bot_id| kept.contains(bot_id))
                .collect();
            fallback.sort_by(|a, b| value(*b).total_cmp(&value(*a)).then_with(|| a.cmp(b)));
            targets.extend(fallback);
        }
        targets
    }

    pub(super) fn nation_bot_attack_order(&self, actor_id: u16, targets: &[u16]) -> Vec<u16> {
        let mut bots: Vec<u16> = targets
            .iter()
            .copied()
            .filter(|target_id| {
                self.state
                    .player(*target_id)
                    .is_some_and(|player| player.player_type == crate::player::PlayerType::Bot)
            })
            .collect();
        if bots.is_empty() {
            return bots;
        }
        if self.state.config.bot_difficulty == crate::game_config::BotDifficulty::Terminator {
            bots = self.plan_bot_attacks(actor_id, &bots);
        } else {
            bots.sort_by(|a, b| {
                let density = |id| {
                    self.state.player(id).map_or(f64::INFINITY, |player| {
                        player.troops.max(0.0) / f64::from(player.tile_count.max(1))
                    })
                };
                density(*a).total_cmp(&density(*b)).then_with(|| a.cmp(b))
            });
        }
        bots.sort_by(|a, b| {
            self.bot_has_structures(*b)
                .cmp(&self.bot_has_structures(*a))
        });
        bots
    }

    pub(super) fn nation_run_combat_for_slot(
        &mut self,
        slot: &AiSlot,
        bot: (u16, u32),
        costs: (f64, f64),
        neighbor_players: &[u16],
        has_neutral: bool,
        decisions: &mut Vec<BotDecision>,
    ) {
        let (bot_id, bot_iq) = bot;
        let (attack_cost, alliance_cost) = costs;
        let policy = slot.policy();
        let campaign_relation = self.campaign_relations.get(&bot_id).copied();
        let vanilla_tribe = slot.tier == AiTier::Tribe
            && self.state.config.bot_difficulty
                == crate::game_config::BotDifficulty::Vanilla
            && campaign_relation.is_none();
        let attacks_players = campaign_attacks_players(
            campaign_relation,
            self.campaign_can_attack_player.get(&bot_id).copied(),
            slot.profile.attacks_players,
        );
        let assault_controls_attacks = self
            .campaign_assault
            .as_ref()
            .is_some_and(|assault| assault.attacker_ids.contains(&bot_id));
        // ── Attack logic (both Bots and Nations) ────────────────────
        if slot.do_attack {
            let campaign_target = if assault_controls_attacks {
                None
            } else if let Some(assault) = &self.campaign_assault {
                assault.focus_targets.get(&bot_id).copied()
            } else {
                self.campaign_assault_targets
                    .get(&bot_id)
                    .and_then(|targets| {
                        (!targets.is_empty()).then(|| {
                            let index = (self.state.tick.wrapping_add(u64::from(bot_id))
                                % targets.len() as u64)
                                as usize;
                            targets[index]
                        })
                    })
            };
            let ready_retaliation = (!assault_controls_attacks)
                .then_some(slot.ghost_retaliation)
                .flatten()
                .filter(|retaliation| retaliation.is_ready_at(self.state.tick));
            let mut campaign_route_attempted = false;
            if let Some(target_id) = campaign_target
                && ready_retaliation.is_none()
            {
                let can_pursue = self.state.player(target_id).is_some_and(|target| {
                    target.alive
                        && !self.state.player(bot_id).is_some_and(|attacker| {
                            attacker.alliances.contains(&target_id)
                                || (attacker.team.is_some() && attacker.team == target.team)
                        })
                });
                if can_pursue {
                    if neighbor_players.contains(&target_id) {
                        let available = self
                            .state
                            .player(bot_id)
                            .map_or(0.0, |player| player.troops);
                        if available >= self.state.config.attack_cost_neutral {
                            if let Some(attacker) = self.state.player_mut(bot_id) {
                                attacker.iq_points = (attacker.iq_points - attack_cost).max(0.0);
                            }
                            decisions.push(BotDecision {
                                bot_id,
                                kind: BotDecisionKind::Attack,
                                intent: GameplayIntent::Attack(AttackIntent {
                                    target_owner: target_id,
                                    troops: Some(available),
                                    target_tile: None,
                                }),
                            });
                        }
                        return;
                    }
                    let available = self
                        .state
                        .player(bot_id)
                        .map_or(0.0, |player| player.troops);
                    campaign_route_attempted = available >= self.state.config.attack_cost_neutral;
                    if self.nation_launch_campaign_assault_fleet(
                        bot_id, target_id, available, decisions,
                    ) {
                        if let Some(attacker) = self.state.player_mut(bot_id) {
                            attacker.iq_points = (attacker.iq_points - attack_cost).max(0.0);
                        }
                        return;
                    }
                }
            }
            // War still spends iq_points (clamped at zero below); growth and
            // defense never freeze for lack of budget.
            {
                let mut betray_then_attack: Option<u16> = None;
                if !assault_controls_attacks {
                    let tick = self.current_tick_u32();
                    let betray_cd = self.alliance_betray_cooldown_until.get(&bot_id).copied();
                    let bordering_count = neighbor_players.len();
                    let allied_on_border: Vec<u16> = {
                        let p_me = self.state.player(bot_id).unwrap();
                        p_me.alliances
                            .iter()
                            .copied()
                            .filter(|id| neighbor_players.contains(id))
                            .collect()
                    };
                    for ally_id in allied_on_border {
                        let should_betray = {
                            let p_me = self.state.player(bot_id).unwrap();
                            let Some(p_ally) = self.state.player(ally_id) else {
                                continue;
                            };
                            if is_real_human(p_ally)
                                && !self.ai_target_allowed(bot_id, ally_id, None)
                            {
                                continue;
                            }
                            if p_ally.is_human()
                                && !human_betrayal_allowed(policy.betrays_human_allies, campaign_relation)
                            {
                                continue;
                            }
                            let mut rng = WyRand::new(
                                self.state
                                    .seed
                                    .wrapping_add(bot_id as u64)
                                    .wrapping_add(ally_id as u64)
                                    .wrapping_add(tick as u64),
                            );
                            maybe_betray_for_attack(
                                p_me,
                                p_ally,
                                bordering_count,
                                tick,
                                betray_cd,
                                &mut rng,
                            )
                        };
                        if should_betray {
                            betray_then_attack = Some(ally_id);
                            break;
                        }
                    }
                    if let Some(ally_id) = betray_then_attack {
                        if let Some(p_me) = self.state.player_mut(bot_id)
                            && p_me.iq_points >= alliance_cost
                        {
                            p_me.iq_points -= alliance_cost;
                        }
                        decisions.push(BotDecision {
                            bot_id,
                            kind: BotDecisionKind::Build,
                            intent: GameplayIntent::BreakAlliance {
                                target_player: ally_id,
                            },
                        });
                    }
                }

                let vanilla_tribe_defense = vanilla_tribe;
                let defensive_player_attackers = if campaign_relation
                    == Some(crate::protocol::CampaignRelation::Neutral)
                    || (vanilla_tribe_defense && slot.is_under_attack)
                {
                    let mut attackers = std::collections::HashSet::new();
                    if let (Some(defender), Some(inbound_attacks)) = (
                        self.state.player(bot_id),
                        self.ai_attack_index.get(bot_id as usize),
                    ) {
                        for &attack_index in inbound_attacks {
                            let Some(attack) = self.attacks.get(attack_index) else {
                                continue;
                            };
                            if attack.target_owner != bot_id || attack.retreating {
                                continue;
                            }
                            let Some(attacker) = self.state.player(attack.owner_id) else {
                                continue;
                            };
                            if !attacker.alive
                                || (campaign_relation
                                    == Some(crate::protocol::CampaignRelation::Neutral)
                                    && !neighbor_players.contains(&attack.owner_id))
                            {
                                continue;
                            }
                            let is_friendly = defender.alliances.contains(&attack.owner_id)
                                || (defender.team.is_some() && defender.team == attacker.team);
                            if !is_friendly && (vanilla_tribe_defense || is_real_human(attacker)) {
                                attackers.insert(attack.owner_id);
                            }
                        }
                    }
                    attackers
                } else {
                    std::collections::HashSet::new()
                };
                let trigger_ratio = slot.profile.trigger_ratio;
                let reserve_ratio = slot.profile.reserve_ratio;
                let expand_ratio = slot.profile.expand_ratio;
                let refuse_human_chance = slot.profile.refuse_human_chance;

                let (troops, max_troops) = {
                    let player = self.state.player(bot_id).unwrap();
                    (player.troops, player.max_troops)
                };

                // Build candidate targets. Exclude allies AND teammates so a
                // bot never wastes its (rare) action deciding to hit a friend —
                // `apply_attack_intent` would silently block it anyway.
                // Neutral campaign factions target a human only to answer an
                // attack already launched against them.
                let mut targets: Vec<u16> = if assault_controls_attacks {
                    Vec::new()
                } else {
                    neighbor_players
                        .iter()
                        .copied()
                        .filter(|&id| {
                            if policy.uses_retaliation_ai
                                && slot.ghost_retaliation.is_some_and(|retaliation| {
                                    id == retaliation.attacker_id
                                        && !retaliation.is_ready_at(self.state.tick)
                                })
                            {
                                return false;
                            }
                            if ready_retaliation
                                .is_none_or(|retaliation| id != retaliation.attacker_id)
                                && let Some(target_id) = campaign_target
                                && (!neighbor_players.contains(&target_id) || id != target_id)
                            {
                                return false;
                            }
                            let human_target_allowed =
                                self.state.player(id).is_some_and(is_real_human)
                                    && (defensive_player_attackers.contains(&id)
                                        || ready_retaliation.is_some_and(|retaliation| {
                                            retaliation.attacker_id == id
                                                && retaliation.is_ready_at(self.state.tick)
                                        })
                                        || self.ai_target_allowed(bot_id, id, None));
                            if self.state.player(id).is_some_and(is_real_human)
                                && !human_target_allowed
                            {
                                return false;
                            }
                            if betray_then_attack == Some(id) {
                                return true;
                            }
                            if let Some(p_me) = self.state.player(bot_id) {
                                let is_ally = p_me.alliances.contains(&id);
                                let is_teammate = p_me.team.is_some()
                                    && p_me.team == self.state.player(id).and_then(|t| t.team);
                                if is_ally || is_teammate {
                                    return false;
                                }
                                if !attacks_players
                                    && let Some(t) = self.state.player(id)
                                    && t.player_type != crate::player::PlayerType::Bot
                                    && !defensive_player_attackers.contains(&id)
                                    && !human_target_allowed
                                {
                                    return false;
                                }
                                true
                            } else {
                                true
                            }
                        })
                        .collect()
                };

                if let Some(retaliation) = ready_retaliation
                    && !targets.contains(&retaliation.attacker_id)
                    && self
                        .state
                        .player(retaliation.attacker_id)
                        .is_some_and(|attacker| {
                            attacker.alive
                                && !self.state.player(bot_id).is_some_and(|defender| {
                                    defender.alliances.contains(&retaliation.attacker_id)
                                        || (defender.team.is_some()
                                            && defender.team == attacker.team)
                                })
                        })
                {
                    targets.push(retaliation.attacker_id);
                }

                let can_fleet = !assault_controls_attacks
                    && self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet);
                let has_tribe_target = policy.uses_regional_nation_planning
                    && targets.iter().any(|&target_id| {
                        self.state
                            .player(target_id)
                            .is_some_and(|p| p.player_type == crate::player::PlayerType::Bot)
                    });

                let mut defender_target = ready_retaliation
                    .filter(|retaliation| targets.contains(&retaliation.attacker_id))
                    .map(|retaliation| retaliation.attacker_id);
                if !policy.uses_retaliation_ai
                    && (bot_iq >= 100 || !defensive_player_attackers.is_empty())
                {
                    let mut largest_attack = 0.0;
                    let inbound_attacks = self
                        .ai_attack_index
                        .get(bot_id as usize)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    for &attack_index in inbound_attacks {
                        let Some(att) = self.attacks.get(attack_index) else {
                            continue;
                        };
                        if att.target_owner == bot_id
                            && !att.retreating
                            && (targets.contains(&att.owner_id)
                                || defensive_player_attackers.contains(&att.owner_id))
                            && self
                                .state
                                .player(att.owner_id)
                                .is_some_and(|attacker| attacker.alive)
                            && att.troops > largest_attack
                        {
                            largest_attack = att.troops;
                            defender_target = Some(att.owner_id);
                        }
                    }
                }
                if let Some(defender_id) = defender_target
                    && defensive_player_attackers.contains(&defender_id)
                    && !targets.contains(&defender_id)
                {
                    targets.push(defender_id);
                }

                // Real humans still pass the grace rule; Vanilla tribes can
                // also target other AI tiers after neutral land is exhausted.
                let attackable_targets: Vec<u16> = targets
                    .iter()
                    .copied()
                    .filter(|&target_id| {
                        self.state.player(target_id).is_some_and(|target| {
                            if is_real_human(target) {
                                defensive_player_attackers.contains(&target_id)
                                    || self.ai_target_allowed(bot_id, target_id, defender_target)
                            } else {
                                attacks_players
                            }
                        })
                    })
                    .collect();

                let is_pro = slot.is_pro(
                    bot_iq,
                    self.state.config.bot_difficulty,
                    self.state.config.ranked_1v1,
                ) && !self.legacy_building_rules;
                let pro_weights = is_pro.then(|| self.pro_weights_for(bot_id));
                let pro_punish_ready = pro_weights.is_some_and(|w| {
                    attackable_targets.iter().any(|&tid| {
                        self.state.player(tid).is_some_and(|t| {
                            t.troops < troops * 0.45
                                && t.troops < t.max_troops * w.punish_depletion_threshold
                        })
                    })
                });
                let trigger_cap = pro_weights.map_or(trigger_ratio, |w| trigger_ratio.min(w.overcap_expand_ratio));
                let trigger_met = troops >= max_troops * trigger_cap || pro_punish_ready;
                let ghost_tribe_bypass = policy.uses_tribe_bypass
                    && !trigger_met
                    && defender_target.is_none()
                    && slot.ghost_retaliation.is_none()
                    && campaign_target.is_none()
                    && !assault_controls_attacks
                    && (can_fleet
                        || attackable_targets.iter().any(|target_id| {
                            self.state.player(*target_id).is_some_and(|target| {
                                target.player_type == crate::player::PlayerType::Bot
                            })
                        }))
                    && self
                        .state
                        .player_mut(bot_id)
                        .unwrap()
                        .bot_rng
                        .next_int(0, 3)
                        == 0;
                let ghost_tribe_target = if ghost_tribe_bypass {
                    let mut best: Option<(u16, f64)> = None;
                    for &target_id in &attackable_targets {
                        if self.state.player(target_id).is_some_and(|target| {
                            target.player_type == crate::player::PlayerType::Bot
                        }) {
                            let score = self.ai_target_prey_score(troops, target_id);
                            if best.is_none_or(|(bid, bscore)| {
                                score > bscore || (score == bscore && target_id < bid)
                            }) {
                                best = Some((target_id, score));
                            }
                        }
                    }
                    best.map(|(id, _)| id)
                } else if policy.uses_tribe_bypass
                    && !trigger_met
                    && defender_target.is_none()
                    && slot.ghost_retaliation.is_none()
                    && campaign_target.is_none()
                    && !assault_controls_attacks
                {
                    let mut best: Option<(u16, f64)> = None;
                    for &target_id in &attackable_targets {
                        let (prize_tiles, _) = self.ai_capturable_structure_prize(target_id);
                        if prize_tiles > 0.0
                            && self.state.player(target_id).is_some_and(|target| {
                                target.troops.max(0.0) <= troops * 0.25
                            })
                        {
                            let score = self.ai_target_prey_score(troops, target_id);
                            if best.is_none_or(|(bid, bscore)| {
                                score > bscore || (score == bscore && target_id < bid)
                            }) {
                                best = Some((target_id, score));
                            }
                        }
                    }
                    best.map(|(id, _)| id)
                } else {
                    None
                };
                let vanilla_tribe_target = if vanilla_tribe
                    && !has_neutral
                    && trigger_met
                    && defender_target.is_none()
                {
                    let mut chosen = None;
                    if !attackable_targets.is_empty() {
                        let start = self
                            .state
                            .player_mut(bot_id)
                            .unwrap()
                            .bot_rng
                            .next_int(0, attackable_targets.len() as i32)
                            as usize;
                        for offset in 0..attackable_targets.len() {
                            let target_id = attackable_targets
                                [(start + offset) % attackable_targets.len()];
                            let defensive_target = defender_target == Some(target_id)
                                || defensive_player_attackers.contains(&target_id)
                                || self.is_attacked_by(bot_id, target_id);
                            let reluctant = self
                                .state
                                .player(target_id)
                                .is_some_and(vanilla_tribe_reluctant_target);
                            if reluctant
                                && !defensive_target
                                && vanilla_tribe_skips_target(
                                    self.state.seed,
                                    bot_id,
                                    target_id,
                                    self.state.tick,
                                    refuse_human_chance,
                                )
                            {
                                continue;
                            }
                            chosen = Some(target_id);
                            break;
                        }
                    }
                    chosen
                } else {
                    None
                };

                let nation_bot_order = if policy.uses_regional_nation_planning {
                    self.nation_bot_attack_order(bot_id, &targets)
                } else {
                    Vec::new()
                };
                if policy.uses_regional_nation_planning
                    && self.state.config.bot_difficulty
                        == crate::game_config::BotDifficulty::Terminator
                    && !assault_controls_attacks
                    && campaign_target.is_none()
                    && defender_target.is_none()
                {
                    let neighboring_bots: Vec<u16> = targets
                        .iter()
                        .copied()
                        .filter(|target_id| {
                            self.state.player(*target_id).is_some_and(|player| {
                                player.player_type == crate::player::PlayerType::Bot
                            })
                        })
                        .collect();
                    let probe_troops = (troops / 100.0).floor();
                    if probe_troops >= 1.0 {
                        for target_owner in self.walled_in_bot_targets(bot_id, &neighboring_bots) {
                            decisions.push(BotDecision {
                                bot_id,
                                kind: BotDecisionKind::Attack,
                                intent: GameplayIntent::Attack(AttackIntent {
                                    target_owner,
                                    troops: Some(probe_troops),
                                    target_tile: None,
                                }),
                            });
                        }
                    }
                }

                let has_land_attack = !attackable_targets.is_empty()
                    && (trigger_met || ghost_tribe_target.is_some());
                let local_defense =
                    defender_target.is_some_and(|target| neighbor_players.contains(&target));
                let has_land_move = has_neutral || has_land_attack || local_defense;
                let force_naval = !has_land_move
                    || defender_target.is_some_and(|target| !neighbor_players.contains(&target));
                if can_fleet
                    && !(vanilla_tribe && has_neutral)
                    && !local_defense
                    && !campaign_route_attempted
                    && self.try_naval_move(
                        slot,
                        bot_id,
                        attack_cost,
                        NavalDecisionContext {
                            attacks_players,
                            campaign_target,
                            defender_target,
                            has_land_move,
                            force_naval,
                            has_hostile_neighbors: !attackable_targets.is_empty(),
                            ghost_tribe_bypass,
                            vanilla_tribe,
                        },
                        decisions,
                    )
                {
                    return;
                }

                // Target precedence: Vanilla tribes expand into free land;
                // other decisions defend first, then apply their tier policy.
                let land_defender_target =
                    defender_target.filter(|target| neighbor_players.contains(target));
                if slot.tier == AiTier::Tribe
                    && let Some(remote_defender) =
                        defender_target.filter(|target| !neighbor_players.contains(target))
                {
                    targets.retain(|target| *target != remote_defender);
                }

                let (target_owner, is_neutral) = if vanilla_tribe && has_neutral {
                    (0, true)
                } else if let Some(attacker_id) = land_defender_target {
                    (attacker_id, false)
                } else if let Some(tribe_id) = ghost_tribe_target {
                    (tribe_id, false)
                } else if vanilla_tribe {
                    let Some(target_id) = vanilla_tribe_target else {
                        return;
                    };
                    (target_id, false)
                } else if policy.uses_regional_nation_planning
                    && trigger_met
                    && !targets.is_empty()
                    && (!has_neutral || has_tribe_target)
                {
                    let mut chosen_target = has_tribe_target
                        .then(|| {
                            nation_bot_order
                                .iter()
                                .copied()
                                .find(|target| attackable_targets.contains(target))
                        })
                        .flatten();
                    if chosen_target.is_none()
                        && (!has_tribe_target
                            || self.state.config.bot_difficulty
                                == crate::game_config::BotDifficulty::Terminator)
                    {
                        chosen_target = attackable_targets
                            .iter()
                            .copied()
                            .filter(|target_id| {
                                (!has_tribe_target
                                    || self.state.player(*target_id).is_some_and(|target| {
                                        target.player_type != crate::player::PlayerType::Bot
                                    }))
                                    && self.ai_target_allowed(bot_id, *target_id, defender_target)
                            })
                            .min_by(|a, b| {
                                let a_troops = self.state.player(*a).map_or(f64::MAX, |p| p.troops);
                                let b_troops = self.state.player(*b).map_or(f64::MAX, |p| p.troops);
                                a_troops.total_cmp(&b_troops).then_with(|| a.cmp(b))
                            });
                    }
                    if let Some(chosen_target) = chosen_target {
                        let is_defensive_target = defender_target == Some(chosen_target)
                            || self.is_attacked_by(bot_id, chosen_target);
                        let is_target_human =
                            self.state.player(chosen_target).is_some_and(is_real_human);
                        if is_target_human && !is_defensive_target {
                            let refuse_roll = self
                                .state
                                .player_mut(bot_id)
                                .unwrap()
                                .bot_rng
                                .next_int(0, 100);
                            if refuse_roll < refuse_human_chance {
                                return;
                            }
                        }
                        (chosen_target, false)
                    } else if has_neutral {
                        (0, true)
                    } else {
                        return;
                    }
                } else if policy.uses_regional_nation_planning && has_neutral {
                    (0, true)
                } else if !attackable_targets.is_empty() && troops >= max_troops * trigger_ratio {
                    let target_owner = if bot_iq >= 130 {
                        let pro_score = |t_id: u16| {
                            let mut s = self.ai_target_prey_score(troops, t_id);
                            if let Some(w) = pro_weights
                                && let Some(target) = self.state.player(t_id)
                            {
                                if target.troops < target.max_troops * w.punish_depletion_threshold {
                                    s *= w.punish_depletion_mult;
                                }
                                let distracted = self.attacks.iter().any(|a| {
                                    a.owner_id == t_id && a.target_owner != bot_id && !a.retreating
                                });
                                if distracted {
                                    s *= 1.35;
                                }
                            }
                            s
                        };
                        let mut best_target = attackable_targets[0];
                        let mut best_score = pro_score(best_target);
                        for &t_id in &attackable_targets[1..] {
                            let score = pro_score(t_id);
                            if score > best_score {
                                best_score = score;
                                best_target = t_id;
                            }
                        }
                        best_target
                    } else if bot_iq >= 100 {
                        let mut weakest = attackable_targets[0];
                        for &t_id in &attackable_targets {
                            if let (Some(p_t), Some(p_w)) =
                                (self.state.player(t_id), self.state.player(weakest))
                                && p_t.troops < p_w.troops
                            {
                                weakest = t_id;
                            }
                        }
                        weakest
                    } else {
                        let p_mut = self.state.player_mut(bot_id).unwrap();
                        let roll =
                            p_mut.bot_rng.next_int(0, attackable_targets.len() as i32) as usize;
                        attackable_targets[roll]
                    };

                    // Keep the historical random stream for active non-Nation
                    // target choices, including AI opponents. Only apply the
                    // refusal decision to eligible proactive human targets.
                    let refuse_roll = self
                        .state
                        .player_mut(bot_id)
                        .unwrap()
                        .bot_rng
                        .next_int(0, 100);
                    let is_defensive_target = defender_target == Some(target_owner)
                        || self.is_attacked_by(bot_id, target_owner);
                    let is_target_human =
                        self.state.player(target_owner).is_some_and(is_real_human);
                    if is_target_human && !is_defensive_target && refuse_roll < refuse_human_chance
                    {
                        return;
                    }

                    (target_owner, false)
                } else if has_neutral {
                    (0, true)
                } else if targets.is_empty() {
                    if policy.launches_nukes {
                        self.maybe_launch_nuke(
                            bot_id,
                            decisions,
                            bot_iq,
                            &targets,
                            defender_target,
                        );
                    }
                    return;
                } else {
                    // Below trigger, enemy neighbors only, no neutral left:
                    // bank this tick and keep accumulating for the war push.
                    return;
                };

                let is_defending = land_defender_target.is_some()
                    && (slot.tier != AiTier::Tribe || trigger_met);
                let target_is_real_human =
                    self.state.player(target_owner).is_some_and(is_real_human);
                let can_initiate = !assault_controls_attacks
                    && (attacks_players
                        || (target_is_real_human
                            && self.ai_target_allowed(bot_id, target_owner, defender_target)));
                let (mut target_owner, mut is_neutral) = (target_owner, is_neutral);

                // OF odds discipline (AiAttackBehavior parity) — initiation
                // only; defense and retaliation stay exempt:
                //   · vs tribes (`calculateBotAttackTroops`): strike with 4×
                //     the tribe's troops; if we can't afford that, our
                //     affordable wave must still be ≥2× or we bank —
                //     half-hearted pokes at food are how tribes balloon.
                //   · vs real players (`isAttackTooWeak`): never initiate with
                //     less than 20% of the target's troops; bleeding into a
                //     prepared defense (tile cost scales with the defender's
                //     TOTAL troops) only feeds the pile-on.
                //   · blocked with free land adjacent: grow instead (OF
                //     expansions precede wars) — never frozen.
                // Affordability keys on the STANDING ARMY, not max_troops:
                // max grows with territory while troops trail it for most of
                // the match, so a max-based reserve zeroes out every war
                // decision mid-expansion (OF can key on max — its armies sit
                // near cap).
                let mut odds_send: Option<f64> = None;
                // Team games are exempt from the PLAYER-target odds gates —
                // OF parity: troopSendCap/isAttackTooWeak return
                // Infinity/false when teammates back the attack. The tribe
                // window applies everywhere (tribes are the map's food).
                let is_team_game = self.state.config.game_mode != "FFA";
                let ghost_tribe_subtrigger_attack = policy.uses_tribe_bypass
                    && ghost_tribe_target.is_some_and(|id| id == target_owner);
                if !is_neutral
                    && !is_defending
                    && bot_iq >= 130
                    && (trigger_met || ghost_tribe_subtrigger_attack)
                {
                    let (target_troops, target_is_tribe) = self
                        .state
                        .player(target_owner)
                        .map(|p| {
                            (
                                p.troops.max(0.0),
                                p.player_type == crate::player::PlayerType::Bot,
                            )
                        })
                        .unwrap_or((0.0, false));
                    // Tribe targets: NO affordability floor. Tribes sit AT
                    // their troop cap (they never spend) while nations sit
                    // far below theirs (sweeps drain troops and every
                    // conquest raises max), so any troops-ratio window — OF's
                    // 2× included — structurally favors the tribe and
                    // re-freezes the map mid-game. The 4× sizing caps the
                    // send; the conquest math (5:1 → max power at 1×) makes
                    // even parity waves grind territory. FFA discipline stays
                    // for PLAYER targets only, and team games are exempt
                    // (OF: troopSendCap/isAttackTooWeak are FFA-only).
                    let affordable = troops * (1.0 - slot.profile.reserve_ratio);
                    let committed = if target_is_tribe {
                        Some((target_troops * 4.0).min(affordable))
                    } else if is_team_game
                        || (affordable >= target_troops * 0.20 && target_troops < troops)
                    {
                        Some(affordable)
                    } else if let Some(tribe_alt) = {
                        let mut best: Option<(u16, f64)> = None;
                        for &id in &targets {
                            if self
                                .state
                                .player(id)
                                .is_some_and(|p| p.player_type == crate::player::PlayerType::Bot)
                            {
                                let score = self.ai_target_prey_score(troops, id);
                                if best.is_none_or(|(_, bscore)| score > bscore) {
                                    best = Some((id, score));
                                }
                            }
                        }
                        best.map(|(id, _)| id)
                    } {
                        // Odds-locked player target (FFA): swing at a tribe
                        // neighbor instead of banking — the attrition war on
                        // tribes is always open, and picking a locked target
                        // used to stall the whole AI while an attackable
                        // tribe sat on the same border.
                        let tt = self
                            .state
                            .player(tribe_alt)
                            .map(|p| p.troops.max(0.0))
                            .unwrap_or(0.0);
                        target_owner = tribe_alt;
                        Some((tt * 4.0).min(affordable))
                    } else {
                        None
                    };
                    match committed {
                        Some(s) => odds_send = Some(s),
                        None if has_neutral => {
                            target_owner = 0;
                            is_neutral = true;
                        }
                        None => return, // bank and accumulate for the real push
                    }
                }

                // A committed odds decision IS the war trigger — the classic
                // trigger ratio keys on max_troops, which explodes with
                // territory while troops trail it, so mid-expansion nations
                // could never pass it (they only wared true-late).
                let odds_committed = odds_send.is_some();
                if is_neutral
                    || is_defending
                    || odds_committed
                    || (can_initiate && troops >= max_troops * trigger_ratio)
                {
                    let reserve = max_troops
                        * if is_neutral {
                            expand_ratio
                        } else if is_defending && policy.desperate_defense_half_reserve {
                            // Desperate defense: keep only half of standard reserve ratio
                            reserve_ratio * 0.5
                        } else {
                            reserve_ratio
                        };
                    let is_standard_bot = slot.tier == AiTier::Tribe;
                    let mut p_send = if is_standard_bot && !is_neutral && !is_defending {
                        (troops / 4.0).max(0.0)
                    } else {
                        (troops - reserve).max(0.0)
                    };
                    if let Some(s) = odds_send {
                        p_send = s;
                    }
                    if let Some(retaliation) = ready_retaliation.filter(|retaliation| {
                        !is_neutral && target_owner == retaliation.attacker_id
                    }) {
                        p_send = ghost_retaliation_send_troops(
                            self.state.seed,
                            bot_id,
                            retaliation,
                            troops,
                            max_troops,
                            reserve_ratio,
                        );
                    }
                    if p_send >= self.state.config.attack_cost_neutral {
                        // Neutral expansion is GROWTH, not war: it must stay
                        // free of the iq budget, or a high-cadence bot drains
                        // its points on contested frontiers and permanently
                        // freezes mid-game (bankruptcy = zero actions, troops
                        // piling at cap while free land sits next door).
                        if !is_neutral && let Some(p_me) = self.state.player_mut(bot_id) {
                            p_me.iq_points = (p_me.iq_points - attack_cost).max(0.0);
                        }
                        let target_tile = if is_pro {
                            self.pro_choose_attack_target_tile(bot_id, target_owner, is_defending)
                        } else {
                            None
                        };
                        decisions.push(BotDecision {
                            bot_id,
                            kind: BotDecisionKind::Attack,
                            intent: GameplayIntent::Attack(AttackIntent {
                                target_owner,
                                troops: Some(p_send),
                                target_tile,
                            }),
                        });
                    }
                }
                if policy.launches_nukes && !assault_controls_attacks {
                    self.maybe_launch_nuke(bot_id, decisions, bot_iq, &targets, defender_target);
                }
            }
        }
    }

    /// Shared, bounded naval search for every AI tier. It samples cached shores,
    /// then runs the normal water pathfinder once for the selected landing.
    fn try_naval_move(
        &mut self,
        slot: &AiSlot,
        bot_id: u16,
        attack_cost: f64,
        context: NavalDecisionContext,
        decisions: &mut Vec<BotDecision>,
    ) -> bool {
        if !self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet) {
            return false;
        }
        let player_index = bot_id as usize;
        let Some(player) = self.state.player(bot_id) else {
            return false;
        };
        if !player.alive {
            return false;
        }
        let Some(used) = self.bot_naval_slots_used.get(player_index).copied() else {
            return false;
        };
        let port_slots = self
            .building_aggregates
            .get(player_index)
            .map_or(0, |aggregate| aggregate.port_fleet_slots);
        if used >= crate::building::cost::player_fleet_capacity_from_port_slots(port_slots) {
            return false;
        }
        if self.state.map.width == 0 || self.state.map.height == 0 {
            return false;
        }

        let (troops, max_troops) = (player.troops, player.max_troops);
        if troops < self.state.config.attack_cost_neutral {
            return false;
        }

        if context.has_land_move && !context.force_naval {
            // Ranked 1v1 only: boats are a real option (1 in 3 on neutral land, 1 in 5 vs a rival).
            // Other modes keep the old odds so nation and tribe balance does not move.
            let odds = match (self.state.config.ranked_1v1, context.has_hostile_neighbors) {
                (true, false) => 3,
                (true, true) => 5,
                (false, false) => 5,
                (false, true) => 10,
            };
            let mut rng = WyRand::new(
                self.state.seed
                    ^ u64::from(bot_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ self.state.tick.wrapping_mul(0xD6E8_FEB8_6659_FD93)
                    ^ BOT_NAVAL_CHANCE_SALT,
            );
            if rng.next_int(0, odds) != 0 {
                return false;
            }
        }

        let map_area = self.state.map.width.saturating_mul(self.state.map.height);
        let Some(cursor) = self.bot_naval_cursors.get_mut(player_index) else {
            return false;
        };
        let Some(player) = self.state.player(bot_id) else {
            return false;
        };
        player.border_tiles.sample_ones(
            cursor.source_cursor,
            BOT_NAVAL_CANDIDATE_BUDGET,
            &mut self.bot_naval_shore_sample,
        );
        if self.bot_naval_shore_sample.is_empty() {
            return false;
        }
        if map_area == 0 {
            return false;
        }

        #[cfg(test)]
        {
            self.bot_work.naval_border_candidates_examined +=
                self.bot_naval_shore_sample.len() as u64;
        }
        cursor.source_cursor = self
            .bot_naval_shore_sample
            .last()
            .copied()
            .unwrap_or_default()
            .wrapping_add(1)
            % map_area;

        for &source_tile in &self.bot_naval_shore_sample {
            let source_index = source_tile as usize;
            if self.state.map.owner_id(
                source_tile % self.state.map.width,
                source_tile / self.state.map.width,
            ) != bot_id
                || !self.state.map.terrain[source_index].is_land()
                || !self.state.map.terrain[source_index].is_shoreline()
            {
                continue;
            }
            let water_component = self.water.component_of(source_tile);
            if water_component == 0 {
                continue;
            }
            let Some(component_cursor) = cursor
                .components
                .iter_mut()
                .find(|candidate| candidate.water_component == water_component)
            else {
                cursor
                    .components
                    .push(crate::engine::BotNavalComponentCursor {
                        water_component,
                        source_tile,
                        shoreline_cursor: 0,
                    });
                continue;
            };
            let source_still_owned = self.state.map.owner_id(
                component_cursor.source_tile % self.state.map.width,
                component_cursor.source_tile / self.state.map.width,
            ) == bot_id;
            if !source_still_owned {
                component_cursor.source_tile = source_tile;
            }
        }

        if cursor.components.is_empty() {
            return false;
        }
        cursor.next_component %= cursor.components.len();
        let component_index = cursor.next_component;
        cursor.next_component = (component_index + 1) % cursor.components.len();
        let component_cursor = &mut cursor.components[component_index];
        let component = component_cursor.water_component;
        let mut sources: Vec<u32> = self
            .bot_naval_shore_sample
            .iter()
            .copied()
            .filter(|&tile| {
                self.state
                    .map
                    .owner_id(tile % self.state.map.width, tile / self.state.map.width)
                    == bot_id
                    && self.state.map.terrain[tile as usize].is_land()
                    && self.state.map.terrain[tile as usize].is_shoreline()
                    && self.water.component_of(tile) == component
            })
            .collect();
        if !sources.is_empty() {
            // Keep source selection independent of the bot's combat/diplomacy RNG.
            let mut rng = WyRand::new(
                self.state.seed
                    ^ u64::from(bot_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ self.state.tick.wrapping_mul(0xD6E8_FEB8_6659_FD93)
                    ^ BOT_NAVAL_SOURCE_SALT,
            );
            component_cursor.source_tile = sources[rng.next_int(0, sources.len() as i32) as usize];
        }
        let source_tile = component_cursor.source_tile;
        let source_is_valid = self.state.map.owner_id(
            source_tile % self.state.map.width,
            source_tile / self.state.map.width,
        ) == bot_id
            && self
                .state
                .map
                .terrain
                .get(source_tile as usize)
                .is_some_and(|tile| tile.is_land() && tile.is_shoreline())
            && self.water.component_of(source_tile) == component;
        if !source_is_valid {
            cursor.components.remove(component_index);
            if !cursor.components.is_empty() {
                cursor.next_component %= cursor.components.len();
            } else {
                cursor.next_component = 0;
            }
            return false;
        }
        if sources.is_empty() {
            sources.push(source_tile);
        }

        let Some(shoreline) = self.water.shoreline_tiles.get(component as usize) else {
            return false;
        };
        if shoreline.is_empty() {
            return false;
        }
        let start = component_cursor.shoreline_cursor % shoreline.len();
        let nearby = self.water.nearby_shores(
            &self.state.map,
            component,
            source_tile,
            BOT_NAVAL_LOCAL_RADIUS,
            BOT_NAVAL_LOCAL_BUDGET,
        );

        let policy = slot.policy();
        let reserve_ratio = slot.profile.reserve_ratio;
        let expand_ratio = slot.profile.expand_ratio;
        let trigger_met = troops >= max_troops * slot.profile.trigger_ratio;
        let team_game = self.state.config.game_mode != "FFA";
        let mut chosen: Option<(u8, u32, f64, u32, u16, f64)> = None;
        for local_search in [true, false] {
            let examined = if local_search {
                nearby.len()
            } else {
                let remaining = BOT_NAVAL_CANDIDATE_BUDGET - nearby.len();
                let examined = shoreline.len().min(remaining);
                self.bot_naval_cursors[player_index].components[component_index].shoreline_cursor =
                    (start + examined) % shoreline.len();
                examined
            };
            #[cfg(test)]
            {
                self.bot_work.shoreline_candidates_examined += examined as u64;
            }
            for offset in 0..examined {
                let target_tile = if local_search {
                    nearby[offset]
                } else {
                    shoreline[(start + offset) % shoreline.len()]
                };
                let target_owner = self.state.map.owner_id(
                    target_tile % self.state.map.width,
                    target_tile / self.state.map.width,
                );
                if target_owner == bot_id {
                    continue;
                }
                if target_owner == 0
                    && (context.defender_target.is_some() || context.campaign_target.is_some())
                {
                    continue;
                }

                let is_defending = context.defender_target == Some(target_owner);
                let send = if target_owner == 0 {
                    (troops * 0.20).min((troops - max_troops * expand_ratio).max(0.0))
                } else {
                    let Some(target) = self.state.player(target_owner) else {
                        continue;
                    };
                    if !target.alive
                        || target.id == bot_id
                        || context.defender_target.is_some_and(|id| id != target_owner)
                        || context.campaign_target.is_some_and(|id| {
                            id != target_owner && context.defender_target.is_none()
                        })
                    {
                        continue;
                    }
                    let Some(attacker) = self.state.player(bot_id) else {
                        continue;
                    };
                    if attacker.alliances.contains(&target_owner)
                        || (attacker.team.is_some() && attacker.team == target.team)
                    {
                        continue;
                    }
                    if is_real_human(target)
                        && !self.ai_target_allowed(bot_id, target_owner, context.defender_target)
                    {
                        continue;
                    }
                    if context.vanilla_tribe
                        && !is_defending
                        && vanilla_tribe_reluctant_target(target)
                        && vanilla_tribe_skips_target(
                            self.state.seed,
                            bot_id,
                            target_owner,
                            self.state.tick,
                            slot.profile.refuse_human_chance,
                        )
                    {
                        continue;
                    }
                    if policy.uses_retaliation_ai
                        && slot.ghost_retaliation.is_some_and(|retaliation| {
                            target_owner == retaliation.attacker_id
                                && !retaliation.is_ready_at(self.state.tick)
                        })
                    {
                        continue;
                    }
                    let can_attack_player = context.attacks_players || is_real_human(target);
                    let ghost_tribe_subtrigger_attack = policy.uses_tribe_bypass
                        && context.ghost_tribe_bypass
                        && target.player_type == crate::player::PlayerType::Bot;
                    if slot.tier == AiTier::Tribe && is_defending && !trigger_met {
                        continue;
                    }
                    if !is_defending
                        && (!can_attack_player
                            || (!trigger_met && !ghost_tribe_subtrigger_attack))
                    {
                        continue;
                    }

                    if let Some(retaliation) = slot.ghost_retaliation.filter(|retaliation| {
                        is_defending
                            && target_owner == retaliation.attacker_id
                            && retaliation.is_ready_at(self.state.tick)
                    }) {
                        ghost_retaliation_send_troops(
                            self.state.seed,
                            bot_id,
                            retaliation,
                            troops,
                            max_troops,
                            reserve_ratio,
                        )
                    } else if is_defending {
                        let reserve = if policy.desperate_defense_half_reserve {
                            reserve_ratio * 0.5
                        } else {
                            reserve_ratio
                        };
                        (troops - max_troops * reserve)
                            .max(0.0)
                            .min(troops * 0.20)
                    } else {
                        (troops - max_troops * reserve_ratio)
                            .max(0.0)
                            .min(troops * 0.20)
                    }
                };
                if send < self.state.config.attack_cost_neutral {
                    continue;
                }

                let (category, neg_prey_score) = if target_owner == 0 {
                    (0, 0.0)
                } else {
                    let target = self.state.player(target_owner).unwrap();
                    let odds_ok = is_defending
                        || team_game
                        || (target.troops < troops && send >= target.troops.max(0.0) * 0.20);
                    if !odds_ok {
                        continue;
                    }
                    (
                        if target.player_type == crate::player::PlayerType::Bot {
                            0
                        } else {
                            1
                        },
                        -self.ai_target_prey_score(troops, target_owner),
                    )
                };
                let distance = (source_tile % self.state.map.width)
                    .abs_diff(target_tile % self.state.map.width)
                    + (source_tile / self.state.map.width)
                        .abs_diff(target_tile / self.state.map.width);
                let distance_band = distance / 8;
                let is_better = chosen.is_none_or(
                    |(best_category, best_band, best_neg_score, best_tile, _, _)| {
                        category < best_category
                            || (category == best_category
                                && (distance_band < best_band
                                    || (distance_band == best_band
                                        && (neg_prey_score.total_cmp(&best_neg_score).is_lt()
                                            || (neg_prey_score == best_neg_score
                                                && target_tile < best_tile)))))
                    },
                );
                if is_better {
                    chosen = Some((
                        category,
                        distance_band,
                        neg_prey_score,
                        target_tile,
                        target_owner,
                        send,
                    ));
                }
            }
            if chosen.is_some() {
                break;
            }
        }

        let Some((_, _, _, target_tile, target_owner, send)) = chosen else {
            return false;
        };
        #[cfg(test)]
        {
            self.bot_work.naval_routes_calculated += 1;
        }
        let Ok(route) = crate::warp_fleet::resolve_fleet_route_between_shores(
            &self.state.map,
            &mut self.path_scratch,
            &sources,
            target_tile,
        ) else {
            return false;
        };

        if target_owner != 0
            && let Some(attacker) = self.state.player_mut(bot_id)
        {
            attacker.iq_points = (attacker.iq_points - attack_cost).max(0.0);
        }
        if let Some(slots) = self.bot_naval_slots_used.get_mut(player_index) {
            *slots = slots.saturating_add(1);
        }
        self.cache_bot_route(bot_id, target_tile, route);
        decisions.push(BotDecision {
            bot_id,
            kind: BotDecisionKind::Attack,
            intent: GameplayIntent::LaunchFleet {
                target_tile,
                troops: Some(send),
            },
        });
        true
    }

    pub(super) fn maybe_launch_nuke(
        &mut self,
        bot_id: u16,
        decisions: &mut Vec<BotDecision>,
        bot_iq: u32,
        targets: &[u16],
        defender_target: Option<u16>,
    ) {
        if !self.campaign_allows_action(crate::campaign::CampaignAction::Nuke) {
            return;
        }
        if bot_iq < 100 {
            return;
        }
        if !crate::building::player_has_nuclear_launch_city(&self.buildings, bot_id, self.campaign_unlocks.is_some()) {
            return;
        }

        let legal_targets: Vec<u16> = targets
            .iter()
            .copied()
            .filter(|&target_id| {
                self.state.player(target_id).is_some()
                    && self.ai_target_allowed(bot_id, target_id, defender_target)
            })
            .collect();
        let has_legal_target = defender_target
            .is_some_and(|target_id| targets.contains(&target_id))
            || !legal_targets.is_empty();
        if !has_legal_target {
            return;
        }

        if self.building_aggregates_dirty {
            self.bot_sam_tiles_cache = None;
            self.refresh_building_counts_only();
        }

        let Some(player) = self.state.player(bot_id) else {
            return;
        };
        let gold = player.gold;
        let player_home = player.home;
        let nuke_cost = self.nuclear_price(NukeKind::AtomBomb);
        self.refresh_building_grid();
        let silos: Vec<_> = self
            .buildings
            .iter()
            .filter(|b| {
                b.owner_id == bot_id
                    && ((b.kind == BuildingKind::MilitaryCamp && b.active_level() >= 5)
                        || (self.legacy_building_rules
                            && b.kind == BuildingKind::MissileSilo
                            && b.active_level() > 0))
            })
            .collect();
        if silos.is_empty() {
            if self.legacy_building_rules {
                let silo_cost = self
                    .foundation_quote(bot_id, BuildingKind::MissileSilo)
                    .map_or(f64::INFINITY, |(_, cost, _)| cost);
                if gold >= silo_cost + nuke_cost
                    && self.campaign_allows_building(BuildingKind::MissileSilo, 1)
                    && !super::structures::has_pending_build(decisions, bot_id)
                    && let Some((x, y)) = player_home
                {
                    if let Some(target_tile) = crate::building::find_structure_site_near(
                        &self.state.map,
                        bot_id,
                        y * self.state.map.width + x,
                        BuildingKind::MissileSilo,
                        &self.building_grid,
                        &mut self.placement_scratch,
                        true,
                    ) {
                        decisions.push(BotDecision {
                            bot_id,
                            kind: BotDecisionKind::Build,
                            intent: GameplayIntent::BuildStructure {
                                kind: BuildingKind::MissileSilo,
                                target_tile,
                            },
                        });
                    }
                }
            }
            return;
        }
        let silo_charges = |b: &crate::building::Building| -> u8 {
            if b.kind == BuildingKind::MilitaryCamp {
                b.active_level().saturating_sub(4)
            } else {
                b.active_level()
            }
        };
        let max_camp_level = silos.iter().map(|b| b.active_level()).max().unwrap_or(0);
        let charges: usize = silos
            .iter()
            .map(|b| usize::from(self.nuclear_charges(b.id, silo_charges(b))))
            .sum();
        if charges == 0 || gold < nuke_cost {
            return;
        }

        let leader = if let Some(leader) = self.bot_crown_leader {
            leader
        } else {
            let mut leader = 0;
            let mut leader_tiles = 0;
            for p in &self.state.players {
                if p.alive && p.tile_count > leader_tiles {
                    leader = p.id;
                    leader_tiles = p.tile_count;
                }
            }
            self.bot_crown_leader = Some(leader);
            leader
        };

        let primary_target = defender_target
            .filter(|target_id| targets.contains(target_id))
            .or_else(|| {
                if legal_targets.contains(&leader) {
                    Some(leader)
                } else {
                    legal_targets.first().copied()
                }
            });
        let Some(primary_target) = primary_target else {
            return;
        };

        let bot_alliances = self
            .state
            .player(bot_id)
            .map(|p| p.alliances.clone())
            .unwrap_or_default();
        if self.bot_sam_tiles_cache.is_none() {
            self.bot_sam_tiles_cache = Some(
                self.buildings
                    .iter()
                    .filter_map(|b| {
                        let lvl = b.active_level();
                        if b.kind == BuildingKind::Tower && lvl >= 5 {
                            let mult = self
                                .state
                                .player(b.owner_id)
                                .map_or(1.0, |p| p.leader.tower_range_multiplier());
                            let range = crate::building::tower_range_for_level(
                                lvl,
                                &self.state.config,
                                mult,
                            )
                            .max(0) as u32;
                            Some((b.tile_idx, b.owner_id, range, b.id, lvl.saturating_sub(4)))
                        } else if b.kind == BuildingKind::SAM && lvl > 0 {
                            let range = crate::execution::sam::sam_range(lvl) as u32;
                            Some((b.tile_idx, b.owner_id, range, b.id, lvl))
                        } else {
                            None
                        }
                    })
                    .collect(),
            );
        }
        let shielded_cities = self.bot_sam_tiles_cache.as_ref().unwrap();
        let silo_capacity: usize = silos.iter().map(|silo| usize::from(silo_charges(silo))).sum();

        // Find best structure to nuke
        let mut best_score = -1.0;
        let mut best_tile = 0;
        let mut best_kind = NukeKind::AtomBomb;
        let mut best_shots = 1;
        let mut needs_more_charges = false;

        let mirv_in_flight = self.projectiles.iter().any(|p| {
            p.active
                && p.owner_id == bot_id
                && matches!(p.kind, crate::game::ProjectileKind::Nuke { level: 3 | 4 })
        });
        let mirv_price = self.nuclear_price(NukeKind::MIRV);

        for b in &self.buildings {
            if b.owner_id != primary_target || b.under_construction {
                continue;
            }
            let mut score = match b.kind {
                BuildingKind::City => 25000.0 + 10000.0 * (b.active_level() as f64),
                BuildingKind::Tower => 5000.0 * (b.level as f64),
                BuildingKind::TradingPost => 15000.0 * (b.level as f64),
                BuildingKind::Port => 10000.0 * (b.level as f64),
                BuildingKind::MilitaryCamp => 7000.0 * (b.level as f64) + if b.level >= 5 { 15000.0 } else { 0.0 },
                BuildingKind::Farm => 9000.0 * (b.level as f64),
                BuildingKind::SAM | BuildingKind::MissileSilo => 4500.0 * (b.level as f64),
            };

            let bx = b.tile_idx % self.state.map.width;
            let by = b.tile_idx / self.state.map.width;

            // SAM avoidance
            let covering: usize = shielded_cities.iter().filter(|&&(sam_tile, sam_owner, range, _, _)| {
                sam_owner != bot_id && !bot_alliances.contains(&sam_owner)
                    && crate::execution::sam::sam_tile_distance(b.tile_idx, sam_tile, self.state.map.width) <= range
            }).map(|&(_, _, _, id, level)| usize::from(self.nuclear_charges(id, level))).sum();
            let nearby = self.building_grid.iter_all_in_range(bx, by, 80).filter(|other| {
                self.state.map.owner_id(other.x, other.y) == primary_target
                    && other.x.abs_diff(bx).pow(2) + other.y.abs_diff(by).pow(2) <= 80 * 80
            }).take(8).count();
            let mut kind = NukeKind::AtomBomb;
            let mut shots = covering + 1;
            let can_mirv = self.legacy_building_rules || max_camp_level >= NukeKind::MIRV.required_military_camp_level();
            let can_hydro = self.legacy_building_rules || max_camp_level >= NukeKind::HydrogenBomb.required_military_camp_level();
            if can_mirv
                && !mirv_in_flight
                && covering >= 2
                && nearby >= 7
                && gold >= mirv_price * 1.4
            {
                kind = NukeKind::MIRV;
                shots = 1;
            } else if can_hydro && nearby >= 3 && gold >= self.nuclear_price(NukeKind::HydrogenBomb) * shots as f64 {
                kind = NukeKind::HydrogenBomb;
            }
            if gold < self.nuclear_price(kind) * shots as f64 {
                continue;
            }
            let min_lvl = kind.required_military_camp_level();
            let eligible_charges: usize = silos
                .iter()
                .filter(|s| self.legacy_building_rules || s.active_level() >= min_lvl)
                .map(|s| usize::from(self.nuclear_charges(s.id, silo_charges(s))))
                .sum();
            if eligible_charges < shots {
                needs_more_charges |= shots > silo_capacity;
                continue;
            }
            if shots > 1 {
                let width = self.state.map.width;
                let mut launchers: Vec<_> = silos.iter().filter_map(|silo| {
                    if !self.legacy_building_rules && silo.active_level() < min_lvl {
                        return None;
                    }
                    let ready = usize::from(self.nuclear_charges(silo.id, silo_charges(silo)));
                    (ready > 0).then_some((
                        (silo.tile_idx % width).abs_diff(bx) + (silo.tile_idx / width).abs_diff(by),
                        silo.id,
                        crate::execution::sam::sam_tile_distance(silo.tile_idx, b.tile_idx, width).div_ceil(2),
                        ready,
                    ))
                }).collect();
                launchers.sort_unstable();
                let mut available = 0;
                let mut earliest = u32::MAX;
                let mut latest = 0;
                for (_, _, eta, ready) in launchers {
                    available += ready;
                    earliest = earliest.min(eta);
                    latest = latest.max(eta);
                    if available >= shots {
                        break;
                    }
                }
                if latest.saturating_sub(earliest) > crate::execution::sam::SAM_COOLDOWN_TICKS / 2 {
                    needs_more_charges = true;
                    continue;
                }
            }
            score += 3000.0 * nearby as f64;

            // Target dedup
            if let Some(&count) = self.recent_nuke_targets.get(&(primary_target, b.tile_idx)) {
                score -= 50_000.0 * count as f64;
            }

            if score > best_score {
                best_score = score;
                best_tile = b.tile_idx;
                best_kind = kind;
                best_shots = shots;
            }
        }

        if best_score > 0.0 {
            *self
                .recent_nuke_targets
                .entry((primary_target, best_tile))
                .or_default() += 1;
            for _ in 0..best_shots {
                decisions.push(BotDecision {
                    bot_id,
                    kind: BotDecisionKind::Attack,
                    intent: GameplayIntent::LaunchNuke { kind: best_kind, target_tile: best_tile },
                });
            }
            let p_me = self.state.player_mut(bot_id).unwrap();
            p_me.iq_points -= 15.0; // Assume 15 points
        } else if needs_more_charges
            && !super::structures::has_pending_build(decisions, bot_id)
            && let Some(upgrade) = super::structures::stack_build_decision(
                self,
                bot_id,
                if self.legacy_building_rules {
                    BuildingKind::MissileSilo
                } else {
                    BuildingKind::MilitaryCamp
                },
                gold - nuke_cost,
                if self.legacy_building_rules {
                    BuildingKind::MissileSilo.max_level()
                } else {
                    BuildingKind::MilitaryCamp.max_level()
                },
            )
        {
            decisions.push(upgrade);
        }
    }
}

#[cfg(test)]
mod campaign_relationship_tests {
    use super::{campaign_attacks_players, human_betrayal_allowed};
    use crate::protocol::CampaignRelation;

    #[test]
    fn campaign_relation_alone_controls_player_attack_permission() {
        assert!(campaign_attacks_players(
            Some(CampaignRelation::Enemy),
            None,
            false
        ));
        assert!(!campaign_attacks_players(
            Some(CampaignRelation::Enemy),
            Some(false),
            true
        ));
        assert!(campaign_attacks_players(
            Some(CampaignRelation::Enemy),
            Some(true),
            false
        ));
        assert!(!campaign_attacks_players(
            Some(CampaignRelation::Neutral),
            Some(true),
            true
        ));
        assert!(!campaign_attacks_players(
            Some(CampaignRelation::Allied),
            Some(true),
            true
        ));
        assert!(campaign_attacks_players(None, None, true));
        assert!(!campaign_attacks_players(None, None, false));
    }

    #[test]
    fn campaign_allies_never_betray_humans_but_other_bot_rules_stay_intact() {
        assert!(!human_betrayal_allowed(
            true,
            Some(CampaignRelation::Allied)
        ));
        assert!(!human_betrayal_allowed(
            true,
            Some(CampaignRelation::Neutral)
        ));
        assert!(!human_betrayal_allowed(
            true,
            Some(CampaignRelation::Enemy)
        ));
        assert!(!human_betrayal_allowed(false, None));
        assert!(human_betrayal_allowed(true, None));
    }
}
