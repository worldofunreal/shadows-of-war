use crate::diplomacy::maybe_betray_for_attack;
use crate::engine::SowEngine;
use crate::game::{BuildingKind, NukeKind};
use crate::protocol::{AttackIntent, GameplayIntent};
use crate::rng::NextIntExt;
use wyrand::WyRand;

use super::profile::{AiSlot, AiTier, BotDecision, BotDecisionKind, GhostRetaliation};

const GHOST_RETALIATION_DELAY_SALT: u64 = 0x4752_4459_0000_0001;
const GHOST_RETALIATION_FORCE_SALT: u64 = 0x4752_4643_0000_0002;

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
    profile_default: bool,
) -> bool {
    match relation {
        Some(crate::protocol::CampaignRelation::Enemy) => true,
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
            #[cfg(feature = "ai-metrics")]
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
        let attacks_players =
            campaign_attacks_players(campaign_relation, slot.profile.attacks_players);
        let assault_controls_attacks = self
            .campaign_assault
            .as_ref()
            .is_some_and(|assault| assault.attacker_ids.contains(&bot_id));
        // ── Attack logic (both Bots and Nations) ────────────────────
        if slot.do_attack && !assault_controls_attacks {
            let campaign_target = if let Some(assault) = &self.campaign_assault {
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
            let ready_retaliation = slot
                .ghost_retaliation
                .filter(|retaliation| retaliation.is_ready_at(self.state.tick));
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
                let mut betray_then_attack: Option<u16> = None;
                for ally_id in allied_on_border {
                    let should_betray = {
                        let p_me = self.state.player(bot_id).unwrap();
                        let Some(p_ally) = self.state.player(ally_id) else {
                            continue;
                        };
                        if p_ally.is_human() && !human_betrayal_allowed(is_mfo, campaign_relation) {
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

                // D1 — OpenFront `sendBoatAttackToNearbyTerraNullius` parity:
                // with no free land on our own frontier, cross water to the
                // nearest neutral shores. Free like land expansion (growth,
                // not war); every tier — neutral
                // never means combat.
                let defensive_player_attackers =
                    if campaign_relation == Some(crate::protocol::CampaignRelation::Neutral) {
                        let mut attackers = std::collections::HashSet::new();
                        if let (Some(defender), Some(inbound_attacks)) = (
                            self.state.player(bot_id),
                            self.ai_attack_index.get(bot_id as usize),
                        ) {
                            for &attack_index in inbound_attacks {
                                let Some(attack) = self.attacks.get(attack_index) else {
                                    continue;
                                };
                                if attack.target_owner != bot_id
                                    || !neighbor_players.contains(&attack.owner_id)
                                {
                                    continue;
                                }
                                let Some(attacker) = self.state.player(attack.owner_id) else {
                                    continue;
                                };
                                let is_friendly = defender.alliances.contains(&attack.owner_id)
                                    || (defender.team.is_some() && defender.team == attacker.team);
                                if is_real_human(attacker) && !is_friendly {
                                    attackers.insert(attack.owner_id);
                                }
                            }
                        }
                        attackers
                    } else {
                        std::collections::HashSet::new()
                    };
                if ready_retaliation.is_none()
                    && !has_neutral
                    && defensive_player_attackers.is_empty()
                    && self.try_expansion_boat(bot_id, decisions)
                {
                    return;
                }

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
                let mut targets: Vec<u16> = neighbor_players
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
                        if ready_retaliation.is_none_or(|retaliation| id != retaliation.attacker_id)
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
                    .collect();

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

                // Every Nation shares the same mid-tier capability set. The
                // action phase below remains seed/id-jittered, but the ID no
                // longer decides which Nation gets fleet behavior. Ghosts
                // (is_ai_controlled humans) get the same naval breakout so a
                // teammate fully enclosed by allies keeps advancing instead
                // of idling when its border has no enemy contact.
                let can_fleet = (is_mfo || slot.tier == AiTier::Ghost)
                    && self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet);
                let has_port =
                    crate::building::cost::player_has_completed_port(&self.buildings, bot_id);
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
                            && targets.contains(&att.owner_id)
                            && att.troops > largest_attack
                        {
                            largest_attack = att.troops;
                            defender_target = Some(att.owner_id);
                        }
                    }
                }

                let mut launched_fleet = false;
                // The engine's fleet rule needs no port — the AI self-restricts to
                // ports. An enclosed ghost (allies/teammates on every border tile,
                // no neutral left) has no land move at all, so it may launch
                // portless: idle "comfortable" ghosts must never happen.
                let enclosed = !has_neutral && targets.is_empty();
                // Portless breakout is tier-blind: an enclosed Nation starves
                // exactly like an enclosed Ghost (islands = zero land actions).
                // Vanilla tribes stay excluded (passive by design).
                if can_fleet
                    && ready_retaliation.is_none()
                    && (has_port || (enclosed && slot.tier != AiTier::Tribe))
                    && troops >= max_troops * 0.20
                    && (self.state.tick + bot_id as u64).is_multiple_of(24)
                {
                    let boat_send = (troops - (max_troops * 0.05)).max(0.0);
                    let mut best_target_p_id = None;
                    let mut best_overall_p_id = None;
                    let mut min_troops = f64::MAX;
                    let mut min_overall = f64::MAX;
                    for p in &self.state.players {
                        if p.alive && p.id != bot_id {
                            let is_friendly = {
                                let p_me = self.state.player(bot_id).unwrap();
                                p_me.alliances.contains(&p.id)
                                    || (p_me.team.is_some() && p_me.team == p.team)
                            };
                            let target_allowed = if slot.tier == AiTier::Ghost {
                                slot.ghost_retaliation.is_none_or(|retaliation| {
                                    retaliation.is_ready_at(self.state.tick)
                                        || p.id != retaliation.attacker_id
                                })
                            } else {
                                !is_mfo
                                    || nation_target_allowed(
                                        p.id,
                                        is_real_human(p),
                                        defender_target,
                                    )
                            };
                            if !is_friendly && target_allowed && !p.border_tiles.is_empty() {
                                if p.troops < min_overall {
                                    min_overall = p.troops;
                                    best_overall_p_id = Some(p.id);
                                }
                                // Same odds discipline as land initiation: no
                                // boat suicide into a dwarfing target.
                                let p_troops = p.troops.max(0.0);
                                let p_is_tribe = p.player_type == crate::player::PlayerType::Bot;
                                let odds_ok = if p_is_tribe {
                                    boat_send >= p_troops * 2.0
                                } else {
                                    boat_send >= p_troops * 0.20
                                };
                                if odds_ok && p.troops < min_troops {
                                    min_troops = p.troops;
                                    best_target_p_id = Some(p.id);
                                }
                            }
                        }
                    }
                    if best_target_p_id.is_none() {
                        // Enclosed with no odds-passing target: a desperate
                        // breakout beats idling — idle enclosed bots must
                        // never happen (the D2 contract).
                        best_target_p_id = best_overall_p_id.filter(|_| enclosed);
                    }
                    if let Some(target_p_id) = best_target_p_id {
                        let mut resolved_route = None;
                        {
                            let target_p = self.state.player(target_p_id).unwrap();
                            let start = self.state.tick.wrapping_add(bot_id as u64) as u32;
                            if let Some(t_tile) = target_p.border_tiles.first_one_from(start) {
                                let border_tiles = &self.state.player(bot_id).unwrap().border_tiles;
                                #[cfg(feature = "ai-metrics")]
                                {
                                    self.bot_work.naval_routes_calculated += 1;
                                }
                                #[cfg(feature = "ai-metrics")]
                                let route = crate::warp_fleet::resolve_fleet_route_with_metrics(
                                    &self.state.map,
                                    &self.water,
                                    &mut self.path_scratch,
                                    bot_id,
                                    (target_p_id, t_tile),
                                    border_tiles,
                                    Some(&target_p.border_tiles),
                                    &mut self.bot_work.shoreline_candidates_examined,
                                );
                                #[cfg(not(feature = "ai-metrics"))]
                                let route = crate::warp_fleet::resolve_fleet_route(
                                    &self.state.map,
                                    &self.water,
                                    &mut self.path_scratch,
                                    bot_id,
                                    (target_p_id, t_tile),
                                    border_tiles,
                                    Some(&target_p.border_tiles),
                                );
                                if let Ok(route) = route {
                                    resolved_route = Some((t_tile, route));
                                }
                            }
                        }
                        if let Some((target_tile, route)) = resolved_route {
                            let p_send = (troops - (max_troops * 0.05)).max(0.0);
                            if p_send >= self.state.config.attack_cost_neutral {
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points = (p_me.iq_points - attack_cost).max(0.0);
                                }
                                self.cache_bot_route(bot_id, target_tile, route);
                                decisions.push(BotDecision {
                                    bot_id,
                                    kind: BotDecisionKind::Attack,
                                    intent: GameplayIntent::LaunchFleet {
                                        target_tile,
                                        troops: Some(p_send),
                                    },
                                });
                                launched_fleet = true;
                            }
                        }
                    }
                }

                if launched_fleet {
                    return;
                }

                // Target precedence:
                //   1. Defend the biggest inbound attack.
                //   2. Nations: eat tribes or expand into wilderness.
                //      Tribes remain valid food even while free land exists.
                //   3. Nations: never initiate against Humans; other AI
                //      players remain valid fallback.
                //   4. Other tiers keep their existing player/neutral behavior.
                let (target_owner, is_neutral) = if let Some(attacker_id) = defender_target {
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

                let is_defending = defender_target.is_some();
                // Initiation against players is gated on `attacks_players`
                // (Vanilla tribes are passive food: they expand into neutral
                // land but never target another player). Active tiers may
                // initiate once their trigger threshold is reached.
                let can_initiate = attacks_players;
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
                    if let Some(retaliation) = ready_retaliation {
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
                if slot.tier == AiTier::Nation {
                    self.maybe_launch_nuke(bot_id, decisions, bot_iq, &targets, defender_target);
                }
            }
        }
    }

    /// OpenFront `sendBoatAttackToNearbyTerraNullius` parity: probe one
    /// deterministic neutral land tile and sail one expansion wave. No port,
    /// no player target, no iq cost — pure growth.
    pub(super) fn try_expansion_boat(
        &mut self,
        bot_id: u16,
        decisions: &mut Vec<BotDecision>,
    ) -> bool {
        if !self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet) {
            return false;
        }
        use crate::rng::NextIntExt;
        #[cfg(not(feature = "ai-metrics"))]
        use crate::warp_fleet::resolve_fleet_route;
        use wyrand::WyRand;

        let Some(p0) = self.state.player(bot_id) else {
            return false;
        };
        let (troops, max_troops) = (p0.troops, p0.max_troops);
        let (width, height) = (self.state.map.width, self.state.map.height);
        let reserve = max_troops * 0.10; // OF expandRatio band (10–20%)
        let send = (troops - reserve).max(0.0);
        if send < self.state.config.attack_cost_neutral {
            return false;
        }
        #[cfg(feature = "ai-metrics")]
        if std::env::var("SOW_AI_DEBUG").is_ok() {
            eprintln!("TNBOAT enter id={bot_id} troops={troops:.0}");
        }
        let mut rng = WyRand::new(
            self.state
                .seed
                .wrapping_add(bot_id as u64)
                .wrapping_mul(0x9E3779B97F4A7C15)
                .wrapping_add(self.state.tick),
        );
        let tx = rng.next_int(0, width as i32).max(0) as u32;
        let ty = rng.next_int(0, height as i32).max(0) as u32;
        let owner = self.state.map.owner_id(tx, ty);
        let is_land = self.state.map.terrain[self.state.map.ref_id(tx, ty)].is_land();
        #[cfg(feature = "ai-metrics")]
        if std::env::var("SOW_AI_DEBUG").is_ok() {
            eprintln!(
                "SMP id={bot_id} tick={} t=({tx},{ty}) owner={owner} land={is_land}",
                self.state.tick
            );
        }
        if owner != 0 || !is_land {
            return false;
        }
        #[cfg(feature = "ai-metrics")]
        {
            self.bot_work.naval_routes_calculated += 1;
        }
        #[cfg(feature = "ai-metrics")]
        let route = crate::warp_fleet::resolve_fleet_route_with_metrics(
            &self.state.map,
            &self.water,
            &mut self.path_scratch,
            bot_id,
            (0, ty * width + tx),
            &p0.border_tiles,
            None,
            &mut self.bot_work.shoreline_candidates_examined,
        );
        #[cfg(not(feature = "ai-metrics"))]
        let route = resolve_fleet_route(
            &self.state.map,
            &self.water,
            &mut self.path_scratch,
            bot_id,
            (0, ty * width + tx),
            &p0.border_tiles,
            None,
        );
        #[cfg(feature = "ai-metrics")]
        if std::env::var("SOW_AI_DEBUG").is_ok() {
            eprintln!(
                "ROUTE t={} target=({tx},{ty}) ok={} err={:?}",
                self.state.tick,
                route.is_ok(),
                route.as_ref().err()
            );
        }
        if let Ok(route) = route {
            let target_tile = ty * width + tx;
            self.cache_bot_route(bot_id, target_tile, route);
            decisions.push(BotDecision {
                bot_id,
                kind: BotDecisionKind::Attack,
                intent: GameplayIntent::LaunchFleet {
                    target_tile,
                    troops: Some(send),
                },
            });
            return true;
        }
        false
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
            false
        ));
        assert!(!campaign_attacks_players(
            Some(CampaignRelation::Neutral),
            true
        ));
        assert!(!campaign_attacks_players(
            Some(CampaignRelation::Allied),
            true
        ));
        assert!(campaign_attacks_players(None, true));
        assert!(!campaign_attacks_players(None, false));
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
