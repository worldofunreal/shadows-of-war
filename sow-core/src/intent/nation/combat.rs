use crate::diplomacy::maybe_betray_for_attack;
use crate::engine::SowEngine;
use crate::game::{BuildingKind, NukeKind};
use crate::protocol::{AttackIntent, GameplayIntent};
use crate::rng::NextIntExt;
use wyrand::WyRand;

use super::profile::{AiSlot, AiTier, BotDecision, BotDecisionKind, GhostRetaliation};

const GHOST_RETALIATION_DELAY_SALT: u64 = 0x4752_4459_0000_0001;
const GHOST_RETALIATION_FORCE_SALT: u64 = 0x4752_4643_0000_0002;
const BOT_NAVAL_CANDIDATE_BUDGET: usize = 256;
const BOT_NAVAL_CHANCE_SALT: u64 = 0x4E41_5641_4C00_0003;

struct NavalDecisionContext {
    attacks_players: bool,
    campaign_target: Option<u16>,
    defender_target: Option<u16>,
    has_land_move: bool,
    force_naval: bool,
    has_hostile_neighbors: bool,
}

#[inline]
fn ghost_attack_rng(seed: u64, ghost_id: u16, attack_id: u64, salt: u64) -> WyRand {
    let ghost_key = u64::from(ghost_id).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let attack_key = attack_id.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    WyRand::new(seed ^ ghost_key ^ attack_key ^ salt)
}

pub(super) fn ghost_retaliation_delay_ticks(seed: u64, ghost_id: u16, attack_id: u64) -> u64 {
    let mut rng = ghost_attack_rng(seed, ghost_id, attack_id, GHOST_RETALIATION_DELAY_SALT);
    rng.next_int(3, 13) as u64
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
pub(super) fn nation_target_allowed(
    target_id: u16,
    target_is_human: bool,
    defender_target: Option<u16>,
) -> bool {
    if let Some(defender_id) = defender_target {
        target_id == defender_id
    } else {
        !target_is_human
    }
}

#[inline]
fn human_betrayal_allowed(
    is_nation: bool,
    relation: Option<crate::protocol::CampaignRelation>,
) -> bool {
    match relation {
        Some(_) => false,
        None => !is_nation,
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
    pub(super) fn ghost_retaliation_for(
        &self,
        ghost_id: u16,
        tick: u64,
    ) -> (Option<GhostRetaliation>, u64) {
        let Some(defender) = self.state.player(ghost_id) else {
            return (None, 0);
        };
        let Some(inbound_attacks) = self.ai_attack_index.get(ghost_id as usize) else {
            return (None, 0);
        };
        let mut largest_ready: Option<GhostRetaliation> = None;
        let mut largest_waiting: Option<GhostRetaliation> = None;
        #[cfg(feature = "ai-metrics")]
        let mut entries_examined = 0;

        for &attack_index in inbound_attacks {
            #[cfg(feature = "ai-metrics")]
            {
                entries_examined += 1;
            }
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

            let retaliation = GhostRetaliation {
                attack_id: attack.id,
                attacker_id: attack.owner_id,
                incoming_troops: attack.troops,
                ready_tick: attack
                    .created_tick
                    .saturating_add(ghost_retaliation_delay_ticks(
                        self.state.seed,
                        ghost_id,
                        attack.id,
                    )),
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
        #[cfg(feature = "ai-metrics")]
        return (retaliation, entries_examined);
        #[cfg(not(feature = "ai-metrics"))]
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
        #[cfg(not(feature = "ai-metrics"))]
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
            #[cfg(any(test, feature = "ai-metrics"))]
            {
                self.bot_work.naval_routes_calculated += 1;
            }
            #[cfg(feature = "ai-metrics")]
            let route = crate::warp_fleet::resolve_fleet_route_with_metrics(
                &self.state.map,
                &self.water,
                &mut self.path_scratch,
                bot_id,
                (target_id, target_tile),
                border_tiles,
                Some(&target.border_tiles),
                &mut self.bot_work.shoreline_candidates_examined,
            );
            #[cfg(not(feature = "ai-metrics"))]
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
        let is_mfo = slot.tier == AiTier::Nation;
        let campaign_relation = self.campaign_relations.get(&bot_id).copied();
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
                            if p_ally.is_human()
                                && !human_betrayal_allowed(is_mfo, campaign_relation)
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

                let vanilla_tribe_defense =
                    slot.tier == AiTier::Tribe && campaign_relation.is_none() && !attacks_players;
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
                            if slot.tier == AiTier::Ghost
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
                let has_tribe_target = is_mfo
                    && targets.iter().any(|&target_id| {
                        self.state
                            .player(target_id)
                            .is_some_and(|p| p.player_type == crate::player::PlayerType::Bot)
                    });

                let mut defender_target = ready_retaliation
                    .filter(|retaliation| targets.contains(&retaliation.attacker_id))
                    .map(|retaliation| retaliation.attacker_id);
                if slot.tier != AiTier::Ghost
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

                let has_land_attack = if is_mfo {
                    targets.iter().any(|&target_id| {
                        self.state.player(target_id).is_some_and(|target| {
                            nation_target_allowed(target_id, is_real_human(target), defender_target)
                        })
                    }) && (!has_neutral || has_tribe_target)
                } else {
                    !targets.is_empty()
                        && attacks_players
                        && troops >= max_troops * slot.profile.trigger_ratio
                };
                let local_defense =
                    defender_target.is_some_and(|target| neighbor_players.contains(&target));
                let has_land_move = has_neutral || has_land_attack || local_defense;
                let force_naval = !has_land_move
                    || defender_target.is_some_and(|target| !neighbor_players.contains(&target));
                if can_fleet
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
                            has_hostile_neighbors: attacks_players && !targets.is_empty(),
                        },
                        decisions,
                    )
                {
                    return;
                }

                // Target precedence:
                //   1. Defend the biggest inbound attack.
                //   2. Nations: eat tribes or expand into wilderness.
                //      Tribes remain valid food even while free land exists.
                //   3. Nations: never initiate against Humans; other AI
                //      players remain valid fallback.
                //   4. Other tiers keep their existing player/neutral behavior.
                let land_defender_target =
                    defender_target.filter(|target| neighbor_players.contains(target));
                if slot.tier == AiTier::Tribe
                    && let Some(remote_defender) =
                        defender_target.filter(|target| !neighbor_players.contains(target))
                {
                    targets.retain(|target| *target != remote_defender);
                }

                let (target_owner, is_neutral) = if let Some(attacker_id) = land_defender_target {
                    (attacker_id, false)
                } else if is_mfo && !targets.is_empty() && (!has_neutral || has_tribe_target) {
                    let mut chosen_target = None;
                    let mut min_troops = f64::MAX;
                    for &t_id in &targets {
                        let Some(p_t) = self.state.player(t_id) else {
                            continue;
                        };
                        if has_tribe_target && p_t.player_type != crate::player::PlayerType::Bot {
                            continue;
                        }
                        if !nation_target_allowed(t_id, is_real_human(p_t), defender_target) {
                            continue;
                        }
                        if p_t.troops < min_troops {
                            min_troops = p_t.troops;
                            chosen_target = Some(t_id);
                        }
                    }
                    if let Some(chosen_target) = chosen_target {
                        (chosen_target, false)
                    } else if has_neutral {
                        (0, true)
                    } else {
                        return;
                    }
                } else if is_mfo && has_neutral {
                    (0, true)
                } else if attacks_players
                    && !targets.is_empty()
                    && troops >= max_troops * trigger_ratio
                {
                    let target_owner = if bot_iq >= 130 {
                        let mut best_target = targets[0];
                        for &t_id in &targets {
                            if let (Some(p_t), Some(p_b)) =
                                (self.state.player(t_id), self.state.player(best_target))
                            {
                                let t_is_tribe = p_t.player_type == crate::player::PlayerType::Bot;
                                let b_is_tribe = p_b.player_type == crate::player::PlayerType::Bot;

                                if (t_is_tribe && !b_is_tribe)
                                    || (t_is_tribe == b_is_tribe && p_t.troops < p_b.troops)
                                {
                                    best_target = t_id;
                                }
                            }
                        }
                        best_target
                    } else if bot_iq >= 100 {
                        let mut weakest = targets[0];
                        for &t_id in &targets {
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
                        let roll = p_mut.bot_rng.next_int(0, targets.len() as i32) as usize;
                        targets[roll]
                    };

                    let is_target_human = self
                        .state
                        .player(target_owner)
                        .map(is_real_human)
                        .unwrap_or(false);
                    let refuse_roll = self
                        .state
                        .player_mut(bot_id)
                        .unwrap()
                        .bot_rng
                        .next_int(0, 100);
                    if is_target_human && refuse_roll < refuse_human_chance {
                        return;
                    }

                    (target_owner, false)
                } else if has_neutral {
                    (0, true)
                } else if targets.is_empty() {
                    if slot.tier == AiTier::Nation {
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

                let is_defending = land_defender_target.is_some();
                // Initiation against players is gated on `attacks_players`
                // (Vanilla tribes expand into neutral land but never initiate
                // against another player). Active tiers may
                // initiate once their trigger threshold is reached.
                let can_initiate = attacks_players && !assault_controls_attacks;
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
                if !is_neutral && !is_defending && bot_iq >= 130 {
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
                    } else if let Some(tribe_alt) = targets.iter().copied().find(|id| {
                        self.state
                            .player(*id)
                            .map(|p| p.player_type == crate::player::PlayerType::Bot)
                            .unwrap_or(false)
                    }) {
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
                        } else if is_defending {
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
                        decisions.push(BotDecision {
                            bot_id,
                            kind: BotDecisionKind::Attack,
                            intent: GameplayIntent::Attack(AttackIntent {
                                target_owner,
                                troops: Some(p_send),
                            }),
                        });
                    }
                }
                if slot.tier == AiTier::Nation && !assault_controls_attacks {
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
        let port_levels = self
            .building_aggregates
            .get(player_index)
            .map_or(0, |aggregate| aggregate.port_levels);
        if used >= crate::building::cost::player_fleet_capacity_from_port_levels(port_levels) {
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
            let odds = if context.has_hostile_neighbors { 10 } else { 5 };
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

        #[cfg(any(test, feature = "ai-metrics"))]
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
        let source_tile = component_cursor.source_tile;
        let component = component_cursor.water_component;
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

        let Some(shoreline) = self.water.shoreline_tiles.get(component as usize) else {
            return false;
        };
        if shoreline.is_empty() {
            return false;
        }
        let start = component_cursor.shoreline_cursor % shoreline.len();
        let examined = shoreline.len().min(BOT_NAVAL_CANDIDATE_BUDGET);
        component_cursor.shoreline_cursor = (start + examined) % shoreline.len();
        #[cfg(any(test, feature = "ai-metrics"))]
        {
            self.bot_work.shoreline_candidates_examined += examined as u64;
        }

        let reserve_ratio = slot.profile.reserve_ratio;
        let expand_ratio = slot.profile.expand_ratio;
        let trigger_met = troops >= max_troops * slot.profile.trigger_ratio;
        let team_game = self.state.config.game_mode != "FFA";
        let mut chosen: Option<(u8, f64, u32, u16, f64)> = None;
        for offset in 0..examined {
            let target_tile = shoreline[(start + offset) % shoreline.len()];
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
                    || context
                        .campaign_target
                        .is_some_and(|id| id != target_owner && context.defender_target.is_none())
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
                if slot.tier == AiTier::Nation
                    && !nation_target_allowed(
                        target_owner,
                        is_real_human(target),
                        context.defender_target,
                    )
                {
                    continue;
                }
                if slot.tier == AiTier::Ghost
                    && slot.ghost_retaliation.is_some_and(|retaliation| {
                        target_owner == retaliation.attacker_id
                            && !retaliation.is_ready_at(self.state.tick)
                    })
                {
                    continue;
                }
                if !is_defending
                    && (!context.attacks_players
                        || (!trigger_met
                            && !(slot.tier == AiTier::Nation
                                && target.player_type == crate::player::PlayerType::Bot)))
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
                    (troops - max_troops * reserve_ratio * 0.5)
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

            let (category, strength) = if target_owner == 0 {
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
                        1
                    } else {
                        2
                    },
                    target.troops.max(0.0),
                )
            };
            let is_better = chosen.is_none_or(|(best_category, best_strength, best_tile, _, _)| {
                category < best_category
                    || (category == best_category
                        && (strength.total_cmp(&best_strength).is_lt()
                            || (strength == best_strength && target_tile < best_tile)))
            });
            if is_better {
                chosen = Some((category, strength, target_tile, target_owner, send));
            }
        }

        let Some((_, _, target_tile, target_owner, send)) = chosen else {
            return false;
        };
        #[cfg(any(test, feature = "ai-metrics"))]
        {
            self.bot_work.naval_routes_calculated += 1;
        }
        let Ok(route) = crate::warp_fleet::resolve_fleet_route_between_shores(
            &self.state.map,
            &mut self.path_scratch,
            source_tile,
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

        let target_allowed = |target_id: u16| {
            self.state.player(target_id).is_some_and(|p| {
                nation_target_allowed(target_id, is_real_human(p), defender_target)
            })
        };
        let has_legal_target = defender_target
            .is_some_and(|target_id| targets.contains(&target_id))
            || targets.iter().copied().any(target_allowed);
        if !has_legal_target {
            return;
        }

        if self.building_aggregates_dirty {
            self.bot_sam_tiles_cache = None;
            self.building_aggregates = crate::building::core::aggregate_buildings_per_player(
                self.buildings.iter().copied(),
                self.state.players.len(),
            );
            self.building_aggregates_dirty = false;
        }

        if !self.buildings.iter().any(|b| {
            b.owner_id == bot_id
                && b.kind == BuildingKind::City
                && b.active_level() >= BuildingKind::City.max_level()
        }) {
            return;
        }

        let mut has_silo = false;
        for b in &self.buildings {
            #[cfg(feature = "ai-metrics")]
            {
                self.bot_work.nuke_buildings_examined += 1;
            }
            if b.owner_id == bot_id
                && b.kind == BuildingKind::City
                && b.active_level() >= BuildingKind::City.max_level()
                && !b.under_construction
                && self.silo_cooldowns.get(&b.id).copied().unwrap_or(0) == 0
            {
                has_silo = true;
                break;
            }
        }
        if !has_silo {
            return;
        }

        let Some(player) = self.state.player(bot_id) else {
            return;
        };
        let cost = self.state.config.nuke_cost;
        let perceived_cost = cost;

        if player.gold < perceived_cost {
            return;
        }
        let kind = NukeKind::AtomBomb;

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
                if targets.contains(&leader) && target_allowed(leader) {
                    Some(leader)
                } else {
                    targets
                        .iter()
                        .copied()
                        .find(|&target_id| target_allowed(target_id))
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
                    .filter(|b| {
                        b.kind == BuildingKind::Bunker
                            && b.active_level() >= BuildingKind::Bunker.max_level()
                            && !b.under_construction
                    })
                    .map(|b| (b.tile_idx, b.owner_id))
                    .collect(),
            );
        }
        let shielded_cities = self.bot_sam_tiles_cache.as_ref().unwrap();

        // Find best structure to nuke
        let mut best_score = -1.0;
        let mut best_tile = 0;
        #[cfg(feature = "ai-metrics")]
        let mut sam_checks = 0u64;

        for b in &self.buildings {
            #[cfg(feature = "ai-metrics")]
            {
                self.bot_work.nuke_buildings_examined += 1;
            }
            if b.owner_id != primary_target || b.under_construction {
                continue;
            }
            let mut score = match b.kind {
                BuildingKind::City => 25000.0 + 10000.0 * (b.active_level() as f64),
                BuildingKind::Bunker => 5000.0 * (b.level as f64),
                BuildingKind::Factory => 15000.0 * (b.level as f64),
                BuildingKind::Port => 10000.0 * (b.level as f64),
                BuildingKind::Farm => 7000.0 * (b.level as f64),
            };

            let bx = b.tile_idx % self.state.map.width;
            let by = b.tile_idx / self.state.map.width;

            // SAM avoidance
            let sam_covered = shielded_cities.iter().any(|&(sam_tile, sam_owner)| {
                #[cfg(feature = "ai-metrics")]
                {
                    sam_checks += 1;
                }
                if sam_owner == bot_id || bot_alliances.contains(&sam_owner) {
                    return false;
                }
                let (sx, sy) = (
                    sam_tile % self.state.map.width,
                    sam_tile / self.state.map.width,
                );
                (bx as i32 - sx as i32).abs() + (by as i32 - sy as i32).abs() <= 48
            });
            if sam_covered {
                score -= 100000.0;
            }

            // Target dedup
            #[cfg(feature = "ai-metrics")]
            {
                self.bot_work.nuke_history_lookups += 1;
            }
            if let Some(&count) = self.recent_nuke_targets.get(&(primary_target, b.tile_idx)) {
                score -= 50_000.0 * count as f64;
            }

            if score > best_score {
                best_score = score;
                best_tile = b.tile_idx;
            }
        }
        #[cfg(feature = "ai-metrics")]
        {
            self.bot_work.nuke_sam_checks += sam_checks;
        }

        if best_score > 0.0 {
            *self
                .recent_nuke_targets
                .entry((primary_target, best_tile))
                .or_default() += 1;
            decisions.push(BotDecision {
                bot_id,
                kind: BotDecisionKind::Attack,
                intent: GameplayIntent::LaunchNuke {
                    kind,
                    target_tile: best_tile,
                },
            });
            let p_me = self.state.player_mut(bot_id).unwrap();
            p_me.iq_points -= 15.0; // Assume 15 points
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
            false,
            Some(CampaignRelation::Allied)
        ));
        assert!(!human_betrayal_allowed(
            false,
            Some(CampaignRelation::Neutral)
        ));
        assert!(!human_betrayal_allowed(
            false,
            Some(CampaignRelation::Enemy)
        ));
        assert!(!human_betrayal_allowed(true, None));
        assert!(human_betrayal_allowed(false, None));
    }
}
