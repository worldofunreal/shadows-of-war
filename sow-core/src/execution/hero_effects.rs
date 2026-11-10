//! Situational effects shared by construction, conquest, income and naval combat.
//! Rewards use the authoritative owners and normal placement/cap rules.
use crate::building::{Building, BuildingGrid};
use crate::diplomacy::ALLIANCE_DURATION_TICKS;
use crate::engine::SowEngine;
use crate::execution::{AttackExecution, RETREAT_PENALTY_VS_PLAYER};
use crate::game::{BuildingKind, GameEvent, GameState, UnitType};
use crate::geo_entities::Ability;
use crate::map::TerrainType;
use crate::player::Player;
use std::borrow::Cow;
use std::collections::BTreeSet;

const RELOCATION_RADIUS: u32 = 24;
const FEAST_MIN_GIFT: f64 = 500.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbilityState {
    Ready,
    Conditional,
    Active,
    Triggered,
    Spent,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbilityRestriction {
    CampaignOnly,
    BuildingsDisabled,
    Defeated,
}

/// Local HUD feedback, derived from the same state that executes the effect.
/// `Ready` means armed for its automatic trigger, never a manual cast button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AbilityFeedback {
    pub state: AbilityState,
    pub remaining_seconds: Option<u32>,
    pub restriction: Option<AbilityRestriction>,
}

pub(crate) fn allied(state: &crate::game::GameState, a: u16, b: u16) -> bool {
    a != b
        && matches!((state.player(a), state.player(b)), (Some(a), Some(b))
        if a.alive && b.alive && (a.team.is_some() && a.team == b.team
            || a.alliances.contains(&b.id) && b.alliances.contains(&a.id)))
}

/// The land army and the HUD use the same test for a neutral plain on a rival's border.
pub(crate) fn contested_steppe(state: &GameState, owner: u16, x: u32, y: u32) -> bool {
    if state.map.owner_id(x, y) != 0 || state.map.terrain_type(x, y) != TerrainType::Land {
        return false;
    }
    let mut contested = false;
    state.map.for_each_neighbor(x, y, |nx, ny| {
        let rival = state.map.owner_id(nx, ny);
        contested |= rival != 0
            && rival != owner
            && state.player(rival).is_some_and(|p| p.alive)
            && !allied(state, owner, rival);
    });
    contested
}

/// The target and approach must both still be valid when capture progress is spent.
pub(crate) fn zagros_ascent_target(state: &GameState, owner: u16, x: u32, y: u32) -> bool {
    let map = &state.map;
    let rival = map.owner_id(x, y);
    if map.terrain_type(x, y) != TerrainType::Mountain
        || rival == 0
        || rival == owner
        || !state.player(rival).is_some_and(|p| p.alive)
        || allied(state, owner, rival)
    {
        return false;
    }
    let mut highland = false;
    map.for_each_neighbor(x, y, |nx, ny| {
        highland |=
            map.owner_id(nx, ny) == owner && map.terrain_type(nx, ny) == TerrainType::Highland;
    });
    highland
}

pub(crate) fn regional_commands_multiplier(
    state: &GameState,
    attacks: &[AttackExecution],
    defender: &Player,
) -> f64 {
    let Ability::RegionalCommands { percent } = defender.leader.ability() else {
        return 1.0;
    };
    if !defender.alive || percent == 0 {
        return 1.0;
    }
    let mut invaders = attacks.iter().filter(|a| {
        a.target_owner == defender.id
            && a.owner_id != defender.id
            && !a.retreating
            && a.troops.is_finite()
            && a.troops > 0.0
            && a.troops >= state.config.attack_cost_neutral
            && state.player(a.owner_id).is_some_and(|p| p.alive)
            && !allied(state, defender.id, a.owner_id)
            && a.to_conquer.iter().any(|t| {
                state.map.owner_id(t.x, t.y) == defender.id
                    && state.map.is_adjacent_to_player(t.x, t.y, a.owner_id)
            })
    });
    if let Some(first) = invaders.next()
        && invaders.any(|a| a.owner_id != first.owner_id)
    {
        return 1.0 + f64::from(percent) / 100.0;
    }
    1.0
}

/// An allied pair of neighboring highlands protects both sides. One hero
/// provides the bonus even if several qualifying allies touch the target.
pub(crate) fn nyabingi_support(
    state: &GameState,
    (x, y): (u32, u32),
    invader: Option<u16>,
) -> Option<(u16, u32)> {
    let map = &state.map;
    if map.terrain_type(x, y) != TerrainType::Highland {
        return None;
    }
    let defender = state.player(map.owner_id(x, y)).filter(|p| p.alive)?;
    if invader.is_some_and(|id| id == defender.id || allied(state, id, defender.id)) {
        return None;
    }
    let mut best = None;
    let strength = |(hero, percent): (u16, u32)| (percent, std::cmp::Reverse(hero));
    map.for_each_neighbor(x, y, |nx, ny| {
        let neighbor = map.owner_id(nx, ny);
        if neighbor == 0
            || neighbor == defender.id
            || map.terrain_type(nx, ny) != TerrainType::Highland
            || !allied(state, neighbor, defender.id)
        {
            return;
        }
        for id in [defender.id, neighbor] {
            if let Some(Ability::NyabingiNetwork { percent }) =
                state.player(id).map(|p| p.leader.ability())
                && percent > 0
                && best.is_none_or(|old| strength((id, percent)) > strength(old))
            {
                best = Some((id, percent));
            }
        }
    });
    best
}

pub(crate) fn mounted_capture_rate(state: &GameState, owner: u16, rival: u16) -> f64 {
    let Some(Ability::MountedCapture { percent }) = state.player(owner).map(|p| p.leader.ability())
    else {
        return 0.0;
    };
    if !state.player(rival).is_some_and(|p| p.alive) || allied(state, owner, rival) {
        return 0.0;
    }
    f64::from(percent.min(100)) / 100.0
}

pub(crate) fn mounted_defense_capture_rate(
    state: &GameState,
    defender: u16,
    attacker: u16,
) -> f64 {
    let Some(Ability::MountedDefenseCapture { percent }) =
        state.player(defender).map(|p| p.leader.ability())
    else {
        return 0.0;
    };
    if !state.player(defender).is_some_and(|p| p.alive)
        || !state.player(attacker).is_some_and(|p| p.alive)
        || allied(state, defender, attacker)
    {
        return 0.0;
    }
    f64::from(percent.min(100)) / 100.0
}

fn reunification_troops(state: &GameState, winner: &Player, rival: &Player, reserves: f64) -> f64 {
    let Ability::Reunification { percent } = winner.leader.ability() else {
        return 0.0;
    };
    if !winner.alive || !rival.alive || winner.id == rival.id || allied(state, winner.id, rival.id)
    {
        return 0.0;
    }
    let recruits = reserves * (f64::from(percent.min(100)) / 100.0);
    recruits.min(winner.max_troops - winner.troops).max(0.0)
}

pub(crate) fn recruit_conquered_reserves(
    state: &mut GameState,
    winner: u16,
    victim: u16,
    tile: (u32, u32),
    last_defenders: Option<f64>,
) {
    let (Some(conqueror), Some(rival)) = (state.player(winner), state.player(victim)) else {
        return;
    };
    let reserves = last_defenders.unwrap_or(rival.troops);
    let recruits = reunification_troops(state, conqueror, rival, reserves);
    if recruits > 0.0 {
        let conqueror = state.player_mut(winner).unwrap();
        conqueror.credit_troops_capped(recruits, conqueror.max_troops);
        if last_defenders.is_none() {
            state.player_mut(victim).unwrap().troops -= recruits;
        }
        state.announce_leader_ability_to(winner, &[victim], Some(tile));
    }
}

fn factory_raid_reward(
    state: &GameState,
    claims: &BTreeSet<(u16, u64, u8)>,
    raider: &Player,
    factory: Building,
) -> f64 {
    let Ability::FactoryRaid { troops_per_level } = raider.leader.ability() else {
        return 0.0;
    };
    if !raider.alive
        || factory.kind != BuildingKind::Factory
        || factory.owner_id == 0
        || factory.owner_id == raider.id
        || allied(state, raider.id, factory.owner_id)
        || claims.contains(&(raider.id, factory.id, 0))
    {
        return 0.0;
    }
    f64::from(troops_per_level) * f64::from(factory.active_level())
}

pub(crate) fn raid_captured_factory(
    state: &mut GameState,
    claims: &mut BTreeSet<(u16, u64, u8)>,
    attack: &mut AttackExecution,
    factory: Building,
) {
    let Some(raider) = state.player(attack.owner_id) else {
        return;
    };
    let reward = factory_raid_reward(state, claims, raider, factory);
    if reward > 0.0 {
        claims.insert((attack.owner_id, factory.id, 0));
        attack.troops += reward;
        let width = state.map.width;
        state.announce_leader_ability_to(
            attack.owner_id,
            &[factory.owner_id],
            Some((factory.tile_idx % width, factory.tile_idx / width)),
        );
    }
}

pub(crate) fn joint_front_allies(
    state: &GameState,
    attacks: &[AttackExecution],
    attack: &AttackExecution,
) -> Vec<u16> {
    if attack.retreating
        || attack.target_owner == 0
        || allied(state, attack.owner_id, attack.target_owner)
    {
        return Vec::new();
    }
    attacks
        .iter()
        .filter(|ally| {
            ally.target_owner == attack.target_owner
                && !ally.retreating
                && ally.troops >= state.config.attack_cost_neutral
                && !ally.to_conquer.is_empty()
                && allied(state, attack.owner_id, ally.owner_id)
                && !allied(state, ally.owner_id, attack.target_owner)
        })
        .map(|ally| ally.owner_id)
        .collect()
}

pub(crate) fn joint_front_supporter(
    state: &GameState,
    allies: &[u16],
    tile: (u32, u32),
) -> Option<u16> {
    allies
        .iter()
        .copied()
        .find(|&ally| state.map.is_adjacent_to_player(tile.0, tile.1, ally))
}

pub(crate) fn armistice(state: &crate::game::GameState, a: u16, b: u16) -> Option<(u16, u32)> {
    if !allied(state, a, b) {
        return None;
    }
    [a, b]
        .into_iter()
        .filter_map(|id| match state.player(id)?.leader.ability() {
            Ability::Armistice { percent } if percent > 0 => Some((id, percent.min(100))),
            _ => None,
        })
        .max_by_key(|&(id, percent)| (percent, std::cmp::Reverse(id)))
}

pub(crate) fn retreat_beneficiary(
    state: &GameState,
    attacks: &[AttackExecution],
    invader: u16,
    defender: u16,
) -> Option<(u16, u32)> {
    let ruler = state
        .player(defender)
        .filter(|p| p.alive && p.tile_count > 0)?;
    let Ability::RetreatHandover { percent } = ruler.leader.ability() else {
        return None;
    };
    if percent == 0 || invader == defender || allied(state, invader, defender) {
        return None;
    }
    attacks
        .iter()
        .filter(|attack| {
            attack.target_owner == invader
                && !attack.retreating
                && attack.troops > 0.0
                && allied(state, defender, attack.owner_id)
                && !allied(state, invader, attack.owner_id)
                && state
                    .player(attack.owner_id)
                    .is_some_and(|ally| ally.tile_count > 0 && ally.troops < ally.max_troops)
        })
        .max_by(|a, b| {
            a.troops
                .total_cmp(&b.troops)
                .then_with(|| b.owner_id.cmp(&a.owner_id))
        })
        .map(|attack| (attack.owner_id, percent.min(100)))
}

pub(crate) fn retreat_handover_amount(recipient: &Player, returned: f64, percent: u32) -> f64 {
    (returned * f64::from(percent) / 100.0).min((recipient.max_troops - recipient.troops).max(0.0))
}

pub(crate) fn retreat_loss_multipliers(state: &GameState, attack: &AttackExecution) -> (f64, f64) {
    let penalty = state.player(attack.target_owner).map_or(1.0, |p| {
        super::combat::targeted_multiplier(
            state,
            attack.target_owner,
            attack.owner_id,
            attack.troops,
            |target| p.leader.enemy_retreat_penalty_multiplier(target),
        )
    });
    let softening = state.player(attack.owner_id).map_or(1.0, |p| {
        p.leader
            .attacker_loss_multiplier(crate::geo_entities::LossWhen::Retreat)
    });
    (penalty, softening)
}

fn city_guard_cost(
    state: &GameState,
    claims: &BTreeSet<(u16, u64, u8)>,
    guard: &Player,
    city: Building,
) -> Option<f64> {
    let Ability::CityIntercession { troops } = guard.leader.ability() else {
        return None;
    };
    let cost = f64::from(troops);
    (cost > 0.0
        && guard.troops >= cost
        && guard.tile_count > 0
        && city.kind == BuildingKind::City
        && city.active_level() > 0
        && allied(state, guard.id, city.owner_id)
        && !claims.contains(&(guard.id, city.id, 0)))
    .then_some(cost)
}

pub(crate) fn intercede_for_city(
    state: &mut GameState,
    claims: &mut BTreeSet<(u16, u64, u8)>,
    city: Building,
    attacker: u16,
) -> Option<f64> {
    let (guard, cost) = state
        .players
        .iter()
        .filter(|guard| guard.id != attacker && !allied(state, guard.id, attacker))
        .filter_map(|guard| {
            city_guard_cost(state, claims, guard, city).map(|cost| (guard.id, cost))
        })
        .min_by_key(|&(guard, _)| guard)?;
    claims.insert((guard, city.id, 0));
    state.player_mut(guard).unwrap().troops -= cost;
    let width = state.map.width;
    state.announce_leader_ability_to(
        guard,
        &[city.owner_id, attacker],
        Some((city.tile_idx % width, city.tile_idx / width)),
    );
    Some(cost)
}

fn tile_distance_squared(a: u32, b: u32, width: u32) -> u64 {
    let dx = (a % width).abs_diff(b % width);
    let dy = (a / width).abs_diff(b / width);
    u64::from(dx).pow(2) + u64::from(dy).pow(2)
}

fn nearby(a: u32, b: u32, width: u32, radius: u32) -> bool {
    tile_distance_squared(a, b, width) <= u64::from(radius).pow(2)
}

impl SowEngine {
    pub fn hero_ability_feedback(&self, owner: u16) -> Option<AbilityFeedback> {
        use crate::geo_entities::{District, LossWhen, Resource, Side, Source};
        use AbilityState::*;
        let p = self.state.player(owner)?;
        let ability = p.leader.ability();
        let mut feedback = AbilityFeedback {
            state: Conditional,
            remaining_seconds: None,
            restriction: None,
        };
        let has = |kind| {
            self.buildings
                .iter()
                .any(|b| b.owner_id == owner && b.kind == kind && b.active_level() > 0)
        };
        let ready = |condition| if condition { Ready } else { Conditional };
        let enemy_has = |kind| {
            self.buildings.iter().any(|b| {
                b.owner_id != 0
                    && b.owner_id != owner
                    && b.kind == kind
                    && b.active_level() > 0
                    && !allied(&self.state, owner, b.owner_id)
            })
        };
        if !p.alive {
            feedback.restriction = Some(AbilityRestriction::Defeated);
        } else if p.tile_count == 0 {
            return Some(feedback);
        } else if matches!(ability, Ability::AllianceCost { .. }) && !self.state.config.tutorial {
            feedback.restriction = Some(AbilityRestriction::CampaignOnly);
        } else if !self.state.config.buildings_enabled
            && matches!(
                ability,
                Ability::CoastalFounding
                    | Ability::AlliedCityGrant { .. }
                    | Ability::CityTowerUpgrade
                    | Ability::CityFarmFounding
                    | Ability::CityDenial
                    | Ability::PuntGardens
                    | Ability::LastCityRelocation
                    | Ability::WarRegalia { .. }
                    | Ability::FactoryCityUpgradeDiscount { .. }
                    | Ability::FactoryRaid { .. }
                    | Ability::ImperialStandards { .. }
                    | Ability::SiegeBreach
                    | Ability::CookhamExchange
                    | Ability::SecondCrown
                    | Ability::KitchenPhilosophy
                    | Ability::AksumRout
                    | Ability::TigranesAid { .. }
                    | Ability::AkkadGarrison
                    | Ability::SharedFeast
                    | Ability::CounterLevy { .. }
                    | Ability::SharedCitySpoils { .. }
                    | Ability::StreltsyMuster { .. }
                    | Ability::CityEvacuation { .. }
                    | Ability::CityIntercession { .. }
                    | Ability::GuildNetwork { .. }
                    | Ability::CityCapture { .. }
                    | Ability::CityDefense { .. }
                    | Ability::CityTroopCapacity { .. }
                    | Ability::TowerRange { .. }
                    | Ability::TowerLoss { .. }
                    | Ability::WarshipCost { .. }
                    | Ability::WarshipPrize { .. }
                    | Ability::ResourceBonus {
                        source: Source::District(_),
                        ..
                    }
                    | Ability::AttackerLoss {
                        when: LossWhen::City,
                        ..
                    }
            )
        {
            feedback.restriction = Some(AbilityRestriction::BuildingsDisabled);
        }
        if feedback.restriction.is_some() {
            feedback.state = Unavailable;
            return Some(feedback);
        }
        feedback.state = match ability {
            Ability::CoastalFounding => {
                if self.hero_claims.contains(&(owner, 0, 0)) {
                    Spent
                } else {
                    Ready
                }
            }
            Ability::LastCityRelocation => {
                if self.hero_claims.contains(&(owner, 0, 0)) {
                    Spent
                } else {
                    let mut cities = self.active_cities(owner);
                    let sole_city = cities.next().filter(|_| cities.next().is_none());
                    ready(
                        self.campaign_allows_building(BuildingKind::City, 1)
                            && sole_city.is_some_and(|city| {
                                self.hero_foundation_near(
                                    owner,
                                    city.tile_idx,
                                    BuildingKind::City,
                                    RELOCATION_RADIUS,
                                    &self.placement_grid(),
                                )
                                .is_some()
                            }),
                    )
                }
            }
            Ability::SecondCrown => {
                if self.hero_claims.contains(&(owner, 0, 0)) {
                    Spent
                } else {
                    ready(self.alliances_proposed.iter().any(|offer| {
                        let partner = if offer.proposer == owner {
                            offer.target
                        } else if offer.target == owner {
                            offer.proposer
                        } else {
                            return false;
                        };
                        !allied(&self.state, owner, partner)
                            && self.second_crown_city(owner, partner).is_some()
                    }))
                }
            }
            Ability::RetreatHandover { .. } => ready(self.attacks.iter().any(|attack| {
                attack.target_owner == owner
                    && !attack.retreating
                    && attack.troops > 0.0
                    && retreat_beneficiary(&self.state, &self.attacks, attack.owner_id, owner)
                        .is_some()
            })),
            Ability::TigranesAid { .. } => {
                if self.hero_claims.contains(&(owner, 0, 0)) {
                    Spent
                } else {
                    ready(
                        p.troops < p.max_troops
                            && self.attacks.iter().any(|attack| {
                                attack.owner_id == owner
                                    && !attack.retreating
                                    && self.tigranes_aid_city(attack).is_some()
                                    && self.retreat_has_room_for_aid(p, attack)
                            }),
                    )
                }
            }
            Ability::SharedFeast => ready(self.state.players.iter().any(|ally| {
                let max_gift = self.troops_transferable(owner, ally.id, f64::INFINITY);
                !(self.state.config.ranked_1v1 && p.is_human() && ally.is_human())
                    && max_gift >= FEAST_MIN_GIFT
                    && (ally.max_troops - ally.troops > FEAST_MIN_GIFT
                        || p.troops - max_gift < p.max_troops)
                    && self.shared_feast_farm(owner, ally.id).is_some()
            })),
            Ability::ContestedSteppe { percent } => ready(
                percent > 0
                    && self.state.config.attack_cost_neutral > 0.0
                    && p.border_tiles.ones().any(|tile| {
                        let width = self.state.map.width;
                        let mut eligible = false;
                        self.state
                            .map
                            .for_each_neighbor(tile % width, tile / width, |x, y| {
                                eligible |= contested_steppe(&self.state, owner, x, y);
                            });
                        eligible
                    }),
            ),
            Ability::NyabingiNetwork { .. } => ready(
                (p.team.is_some() || !p.alliances.is_empty())
                    && p.border_coords(self.state.map.width)
                        .any(|tile| nyabingi_support(&self.state, tile, None).is_some()),
            ),
            Ability::RegionalCommands { .. } => ready(
                self.state.config.attack_cost_enemy > 0.0
                    && regional_commands_multiplier(&self.state, &self.attacks, p) > 1.0,
            ),
            Ability::ZagrosAscent { percent } => ready(
                percent > 0
                    && p.border_coords(self.state.map.width).any(|(x, y)| {
                        if self.state.map.terrain_type(x, y) != TerrainType::Highland {
                            return false;
                        }
                        let mut eligible = false;
                        self.state.map.for_each_neighbor(x, y, |nx, ny| {
                            eligible |= zagros_ascent_target(&self.state, owner, nx, ny);
                        });
                        eligible
                    }),
            ),
            Ability::GreatLawOfPeace => ready(self.alliances_proposed.iter().any(|offer| {
                use crate::protocol::CampaignRelation::Enemy;
                let (a, b) = (offer.proposer, offer.target);
                !allied(&self.state, a, b)
                    && self.campaign_relations.get(&a) != Some(&Enemy)
                    && self.campaign_relations.get(&b) != Some(&Enemy)
                    && self.great_law_renewal(p, a, b).is_some()
            })),
            Ability::ReturnCampaign { .. } => {
                if let Some((target, until)) = p
                    .return_campaign
                    .filter(|(_, until)| self.state.tick < *until)
                {
                    feedback.remaining_seconds = Some(
                        ((until - self.state.tick) as f64
                            * f64::from(self.state.config.tick_rate_ms)
                            / 1_000.0)
                            .ceil() as u32,
                    );
                    if self.attacks.iter().any(|a| {
                        a.owner_id == owner
                            && a.target_owner == target
                            && !a.retreating
                            && a.troops > 0.0
                    }) {
                        Active
                    } else {
                        Ready
                    }
                } else {
                    Conditional
                }
            }
            Ability::ImperialStandards { .. } => {
                let agg = self
                    .building_aggregates
                    .get(owner as usize)
                    .copied()
                    .unwrap_or_default();
                let (gold, troops) = super::income_rates::imperial_standard_income(
                    agg,
                    p.leader,
                    &self.state.config,
                );
                if gold > 0.0 || troops > 0.0 {
                    Active
                } else {
                    Conditional
                }
            }
            Ability::AllyPatronage { .. }
            | Ability::SurplusTreasury { .. }
            | Ability::GuildNetwork { .. } => {
                if self.hero_gold_per_second(owner) > 0.0 {
                    Active
                } else {
                    Conditional
                }
            }
            Ability::Reunification { .. } => ready(self.state.players.iter().any(|rival| {
                rival.tile_count > 0
                    && reunification_troops(&self.state, p, rival, rival.troops) > 0.0
            })),
            Ability::WarRegalia { .. } => ready(
                self.state
                    .players
                    .iter()
                    .any(|ally| self.war_regalia_cost(p, ally.id).is_some()),
            ),
            Ability::AlliedCityGrant { .. } => ready(
                self.state
                    .players
                    .iter()
                    .any(|ally| allied(&self.state, owner, ally.id)),
            ),
            Ability::CityTowerUpgrade => ready(self.buildings.iter().any(|city| {
                city.owner_id == owner
                    && city.kind == BuildingKind::City
                    && city.active_level() > 0
                    && (city.under_construction || city.level < city.kind.max_level())
                    && self
                        .nearby_structure_upgrade(*city, BuildingKind::Tower, 20)
                        .is_some()
            })),
            Ability::KitchenPhilosophy => ready(self.buildings.iter().any(|&farm| {
                farm.owner_id == owner
                    && farm.under_construction
                    && self
                        .kitchen_study_city(farm, farm.ticks_until_complete.max(1))
                        .is_some()
            })),
            Ability::SiegeBreach => ready(self.buildings.iter().any(|&tower| {
                let map = &self.state.map;
                tower.kind == BuildingKind::Tower
                    && map.owner_id(tower.tile_idx % map.width, tower.tile_idx / map.width)
                        == tower.owner_id
                    && self
                        .siege_breach_target(owner, tower.owner_id, tower)
                        .is_some()
            })),
            Ability::FactoryRaid { .. } => ready(self.buildings.iter().any(|&factory| {
                let map = &self.state.map;
                factory_raid_reward(&self.state, &self.hero_claims, p, factory) > 0.0
                    && map.owner_id(factory.tile_idx % map.width, factory.tile_idx / map.width)
                        == factory.owner_id
            })),
            Ability::FactoryCityUpgradeDiscount { .. } => {
                ready(self.active_cities(owner).any(|city| {
                    !city.under_construction
                        && city.level < city.kind.max_level()
                        && self.campaign_allows_building(city.kind, city.level + 1)
                        && self
                            .structure_upgrade_quote(city.id)
                            .is_some_and(|(cost, discounted)| discounted && p.gold >= cost)
                }))
            }
            Ability::CityFarmFounding => {
                if !self.campaign_allows_building(BuildingKind::City, 2)
                    || !self.campaign_allows_building(BuildingKind::Farm, 1)
                {
                    Conditional
                } else {
                    let grid = self.placement_grid();
                    ready(self.buildings.iter().any(|city| {
                        city.owner_id == owner
                            && city.kind == BuildingKind::City
                            && city.active_level() == 1
                            && self
                                .hero_foundation_near(
                                    owner,
                                    city.tile_idx,
                                    BuildingKind::Farm,
                                    12,
                                    &grid,
                                )
                                .is_some()
                    }))
                }
            }
            Ability::CounterLevy { .. } => ready(
                self.buildings
                    .iter()
                    .any(|tower| self.counter_levy(owner, tower.owner_id, *tower) > 0.0),
            ),
            Ability::SharedCitySpoils { .. } => ready(self.buildings.iter().any(|b| {
                b.kind == BuildingKind::City
                    && !b.under_construction
                    && b.owner_id != 0
                    && b.owner_id != owner
                    && !allied(&self.state, owner, b.owner_id)
                    && !self.hero_claims.contains(&(owner, b.id, 0))
            })),
            Ability::StreltsyMuster { .. } => {
                if self.hero_claims.contains(&(owner, 0, 0)) {
                    Spent
                } else {
                    ready(
                        p.troops < p.max_troops
                            && self.buildings.iter().any(|city| {
                                city.kind == BuildingKind::City
                                    && !city.under_construction
                                    && city.active_level() > 0
                                    && city.owner_id != 0
                                    && city.owner_id != owner
                                    && !allied(&self.state, owner, city.owner_id)
                            }),
                    )
                }
            }
            Ability::AksumRout => ready(self.buildings.iter().any(|&city| {
                let map = &self.state.map;
                map.owner_id(city.tile_idx % map.width, city.tile_idx / map.width) == city.owner_id
                    && self.aksum_rout_target(owner, city.owner_id, city).is_some()
            })),
            Ability::AkkadGarrison => {
                let grid = self.placement_grid();
                ready(self.buildings.iter().any(|&city| {
                    let map = &self.state.map;
                    map.owner_id(city.tile_idx % map.width, city.tile_idx / map.width)
                        == city.owner_id
                        && self
                            .akkad_garrison_site(owner, city.owner_id, city, &grid)
                            .is_some()
                }))
            }
            Ability::PuntGardens => ready(
                self.campaign_allows_action(crate::campaign::CampaignAction::TradeShip)
                    && self.fleets.iter().any(|fleet| {
                        fleet.owner_id == owner
                            && fleet.unit_type == UnitType::TradeShip
                            && fleet.troops > 0.0
                            && fleet.captured_trade_return.is_none()
                            && self.buildings.iter().any(|port| {
                                port.owner_id != owner
                                    && port.kind == BuildingKind::Port
                                    && !port.under_construction
                                    && port.tile_idx == fleet.dst_tile
                            })
                            && self.buildings.iter().any(|port| {
                                port.owner_id == owner
                                    && port.tile_idx == fleet.src_tile
                                    && self.punt_garden_farm(*port).is_some()
                            })
                    }),
            ),
            Ability::CityDenial => ready(self.buildings.iter().any(|city| {
                city.owner_id == owner
                    && city.kind == BuildingKind::City
                    && city.active_level() > 1
                    && !self.hero_claims.contains(&(owner, city.id, 0))
            })),
            Ability::CookhamExchange => {
                ready(self.buildings.iter().any(|&farm| {
                    farm.owner_id == owner && self.cookham_exchange_city(farm).is_some()
                }))
            }
            Ability::CityIntercession { .. } => {
                ready(self.buildings.iter().any(|&city| {
                    city_guard_cost(&self.state, &self.hero_claims, p, city).is_some()
                }))
            }
            Ability::CityEvacuation { .. } => ready(
                p.troops < p.max_troops
                    && self.buildings.iter().any(|b| {
                        b.owner_id == owner
                            && b.kind == BuildingKind::City
                            && !self.hero_claims.contains(&(owner, b.id, 0))
                    }),
            ),
            Ability::WarshipPrize { .. } => ready(self.fleets.iter().any(|f| {
                f.owner_id == owner && f.unit_type == UnitType::Warship && f.troops > 0.0
            })),
            Ability::Armistice { .. } => ready(self.attacks.iter().any(|attack| {
                let rival = if attack.owner_id == owner {
                    attack.target_owner
                } else if attack.target_owner == owner {
                    attack.owner_id
                } else {
                    return false;
                };
                attack.troops > 0.0
                    && !attack.retreating
                    && self.state.player(rival).is_some_and(|other| {
                        other.alive
                            && !(self.state.config.ranked_1v1 && p.is_human() && other.is_human())
                    })
            })),
            Ability::ResourceBonus {
                source, resource, ..
            } => {
                let produces = match source {
                    Source::District(district) => has(match district {
                        District::City => BuildingKind::City,
                        District::Factory => BuildingKind::Factory,
                        District::Port => BuildingKind::Port,
                        District::Farm => BuildingKind::Farm,
                    }),
                    Source::Territory => true,
                    Source::Alliance => !p.alliances.is_empty(),
                    Source::Trade => self.fleets.iter().any(|f| {
                        f.owner_id == owner
                            && f.unit_type == UnitType::TradeShip
                            && f.captured_trade_return.is_none()
                            && f.troops > 0.0
                    }),
                    Source::Elimination => {
                        return Some(AbilityFeedback {
                            state: Ready,
                            ..feedback
                        });
                    }
                };
                if produces && (resource == Resource::Gold || p.troops < p.max_troops) {
                    Active
                } else {
                    Conditional
                }
            }
            Ability::AttackStrength { against, .. } | Ability::ConquestSpeed { against, .. } => {
                if self.attacks.iter().any(|a| {
                    a.owner_id == owner
                        && !a.retreating
                        && a.troops > 0.0
                        && super::combat::target_applies(
                            &self.state,
                            owner,
                            a.target_owner,
                            a.troops,
                            against,
                        )
                }) {
                    Active
                } else {
                    Conditional
                }
            }
            Ability::DefenseStrength { against, .. } => {
                if self.attacks.iter().any(|a| {
                    a.target_owner == owner
                        && !a.retreating
                        && a.troops > 0.0
                        && super::combat::target_applies(
                            &self.state,
                            owner,
                            a.owner_id,
                            a.troops,
                            against,
                        )
                }) {
                    Active
                } else {
                    Conditional
                }
            }
            Ability::TowerRange { .. } => {
                if has(BuildingKind::Tower) {
                    Active
                } else {
                    Conditional
                }
            }
            Ability::CityTroopCapacity { .. } => {
                if has(BuildingKind::City) {
                    Active
                } else {
                    Conditional
                }
            }
            Ability::TowerLoss { side, .. } => ready(match side {
                Side::Defender => has(BuildingKind::Tower),
                Side::Attacker => enemy_has(BuildingKind::Tower),
            }),
            Ability::CityCapture { side, .. } => ready(match side {
                Side::Defender => has(BuildingKind::City),
                Side::Attacker => enemy_has(BuildingKind::City),
            }),
            Ability::CityDefense { .. } => ready(has(BuildingKind::City)),
            Ability::AttackerLoss {
                when: LossWhen::City,
                ..
            } => ready(enemy_has(BuildingKind::City)),
            Ability::AttackerLoss {
                when: LossWhen::RecaptureWindow,
                ..
            } => {
                if p.recapture_ticks > 0 {
                    feedback.remaining_seconds = Some(
                        (f64::from(p.recapture_ticks) * f64::from(self.state.config.tick_rate_ms)
                            / 1_000.0)
                            .ceil() as u32,
                    );
                    Ready
                } else {
                    Conditional
                }
            }
            Ability::AttackerLoss {
                when: LossWhen::Retreat,
                ..
            } => ready(
                self.attacks
                    .iter()
                    .any(|a| a.owner_id == owner && a.target_owner != 0 && a.troops > 0.0),
            ),
            Ability::AttackerLoss {
                when: LossWhen::SeaRetreat,
                ..
            } => ready(self.fleets.iter().any(|f| {
                f.owner_id == owner
                    && f.unit_type == UnitType::TransportShip
                    && f.troops > 0.0
                    && p.max_troops - p.troops
                        > f.troops * (1.0 - crate::warp_fleet::FLEET_RETREAT_SHORE_MALUS)
            })),
            Ability::WarshipCost { .. } => ready(
                has(BuildingKind::Port)
                    && p.gold
                        >= UnitType::Warship.gold_cost_with_owned_count_for_leader(
                            self.fleets
                                .iter()
                                .filter(|f| f.owner_id == owner && f.unit_type == UnitType::Warship)
                                .count() as u32,
                            p.leader,
                        ),
            ),
            Ability::EnemyRetreatPenalty { .. } => Ready,
            Ability::MountedCapture { .. } => ready(
                p.troops >= self.state.config.attack_cost_neutral
                    && p.max_troops > 0.0
                    && self.attacks.iter().any(|attack| {
                        attack.target_owner == owner
                            && !attack.retreating
                            && attack.troops >= self.state.config.attack_cost_enemy
                            && mounted_capture_rate(&self.state, owner, attack.owner_id) > 0.0
                    }),
            ),
            Ability::MountedDefenseCapture { .. } => ready(
                p.troops < p.max_troops
                    && self.attacks.iter().any(|attack| {
                        attack.target_owner == owner
                            && !attack.retreating
                            && attack.troops >= self.state.config.attack_cost_enemy
                            && mounted_defense_capture_rate(&self.state, owner, attack.owner_id)
                                > 0.0
                    }),
            ),
            Ability::AttackerLoss {
                when: LossWhen::JointFront,
                ..
            } => ready(self.attacks.iter().any(|attack| {
                if attack.owner_id != owner || attack.troops < self.state.config.attack_cost_neutral
                {
                    return false;
                }
                let allies = joint_front_allies(&self.state, &self.attacks, attack);
                !allies.is_empty()
                    && attack.to_conquer.iter().any(|tile| {
                        self.state.map.owner_id(tile.x, tile.y) == attack.target_owner
                            && self.state.map.is_adjacent_to_player(tile.x, tile.y, owner)
                            && joint_front_supporter(&self.state, &allies, (tile.x, tile.y))
                                .is_some()
                    })
            })),
            Ability::AttackerLoss { .. } | Ability::AllianceCost { .. } => Conditional,
        };
        Some(feedback)
    }

    fn war_regalia_cost(&self, donor: &Player, beneficiary: u16) -> Option<f64> {
        let Ability::WarRegalia { troops } = donor.leader.ability() else {
            return None;
        };
        let cost = f64::from(troops);
        (cost > 0.0
            && donor.tile_count > 0
            && donor.troops >= cost
            && allied(&self.state, donor.id, beneficiary)
            && !self
                .hero_claims
                .contains(&(donor.id, u64::from(beneficiary), 2))
            && self.active_cities(donor.id).next().is_some())
        .then_some(cost)
    }

    pub(crate) fn hero_equip_allied_attack(
        &mut self,
        beneficiary: u16,
        target: u16,
        frontier: (u32, u32),
    ) -> f64 {
        if !self.state.config.buildings_enabled
            || target == 0
            || allied(&self.state, beneficiary, target)
        {
            return 0.0;
        }
        let mut reinforcements = 0.0;
        for index in 0..self.state.players.len() {
            let donor = &self.state.players[index];
            let Some(cost) = self.war_regalia_cost(donor, beneficiary) else {
                continue;
            };
            let id = donor.id;
            if id == target || allied(&self.state, id, target) {
                continue;
            }
            self.hero_claims.insert((id, u64::from(beneficiary), 2));
            self.state.players[index].troops -= cost;
            self.state
                .announce_leader_ability_to(id, &[beneficiary, target], Some(frontier));
            reinforcements += cost;
        }
        reinforcements
    }

    pub(crate) fn hero_city_upgrade_discount(&self, city: Building) -> u32 {
        if city.kind != BuildingKind::City {
            return 0;
        }
        let Some(Ability::FactoryCityUpgradeDiscount { percent }) = self
            .state
            .player(city.owner_id)
            .map(|owner| owner.leader.ability())
        else {
            return 0;
        };
        if self.buildings.iter().any(|factory| {
            factory.owner_id == city.owner_id
                && factory.kind == BuildingKind::Factory
                && factory.active_level() > 0
                && nearby(city.tile_idx, factory.tile_idx, self.state.map.width, 12)
        }) {
            percent.min(100)
        } else {
            0
        }
    }

    fn nearby_structure_upgrade(
        &self,
        anchor: Building,
        kind: BuildingKind,
        radius: u32,
    ) -> Option<usize> {
        self.buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.owner_id == anchor.owner_id
                    && b.kind == kind
                    && !b.under_construction
                    && b.level < b.kind.max_level()
                    && self.campaign_allows_building(b.kind, b.level + 1)
                    && nearby(b.tile_idx, anchor.tile_idx, self.state.map.width, radius)
            })
            .min_by_key(|(_, b)| (b.level, b.id))
            .map(|(i, _)| i)
    }

    fn placement_grid(&self) -> Cow<'_, BuildingGrid> {
        if self.building_grid.dirty {
            let mut grid = BuildingGrid::default();
            let map = &self.state.map;
            grid.rebuild(self.buildings.iter(), map.width, map.height);
            Cow::Owned(grid)
        } else {
            Cow::Borrowed(&self.building_grid)
        }
    }

    fn active_cities(&self, owner: u16) -> impl Iterator<Item = &Building> {
        self.buildings.iter().filter(move |city| {
            city.owner_id == owner && city.kind == BuildingKind::City && city.active_level() > 0
        })
    }

    fn shared_feast_farm(&self, owner: u16, ally: u16) -> Option<Building> {
        if !self.state.config.buildings_enabled
            || !self.resources_can_move(owner, ally)
            || self.hero_claims.contains(&(owner, u64::from(ally), 2))
        {
            return None;
        }
        let map = &self.state.map;
        let farm = |player| {
            self.buildings.iter().copied().find(|b| {
                b.owner_id == player
                    && b.kind == BuildingKind::Farm
                    && b.active_level() > 0
                    && map.owner_id(b.tile_idx % map.width, b.tile_idx / map.width) == player
            })
        };
        farm(ally)?;
        farm(owner)
    }

    pub(crate) fn hero_shared_feast(&mut self, owner: u16, ally: u16, gift: f64) {
        if gift < FEAST_MIN_GIFT
            || !self
                .state
                .player(owner)
                .is_some_and(|p| p.leader.ability() == Ability::SharedFeast)
        {
            return;
        }
        let Some(farm) = self.shared_feast_farm(owner, ally) else {
            return;
        };
        let bonus = (gift / 4.0).min(500.0);
        let mut paid = false;
        for id in [owner, ally] {
            if let Some(p) = self.state.player_mut(id) {
                let received = bonus.min(p.max_troops - p.troops);
                if received > 0.0 {
                    p.credit_troops(received);
                    paid = true;
                }
            }
        }
        if paid {
            self.hero_claims.insert((owner, u64::from(ally), 2));
            let width = self.state.map.width;
            self.state.announce_leader_ability_to(
                owner,
                &[ally],
                Some((farm.tile_idx % width, farm.tile_idx / width)),
            );
        }
    }

    pub(crate) fn tigranes_aid_city(&self, attack: &AttackExecution) -> Option<(Building, u32)> {
        let owner = attack.owner_id;
        let player = self.state.player(owner)?;
        let Ability::TigranesAid { max_troops } = player.leader.ability() else {
            return None;
        };
        let min_ticks = (3_000.0 / self.state.config.tick_rate_ms.max(1.0)).ceil() as u64;
        if !self.state.config.buildings_enabled
            || !player.alive
            || player.tile_count == 0
            || attack.troops <= 0.0
            || self.state.tick.saturating_sub(attack.created_tick) < min_ticks
            || attack.target_owner == 0
            || !self
                .state
                .player(attack.target_owner)
                .is_some_and(|p| p.alive)
            || allied(&self.state, owner, attack.target_owner)
            || self.hero_claims.contains(&(owner, 0, 0))
        {
            return None;
        }
        let map = &self.state.map;
        self.buildings
            .iter()
            .copied()
            .find(|city| {
                city.kind == BuildingKind::City
                    && city.active_level() > 0
                    && allied(&self.state, owner, city.owner_id)
                    && map.owner_id(city.tile_idx % map.width, city.tile_idx / map.width)
                        == city.owner_id
            })
            .map(|city| (city, max_troops))
    }

    fn retreat_has_room_for_aid(&self, player: &Player, attack: &AttackExecution) -> bool {
        let (penalty, softening) = retreat_loss_multipliers(&self.state, attack);
        let back = attack.troops * (1.0 - RETREAT_PENALTY_VS_PLAYER * penalty * softening);
        let diverted = retreat_beneficiary(
            &self.state,
            &self.attacks,
            attack.owner_id,
            attack.target_owner,
        )
        .and_then(|(id, percent)| {
            self.state
                .player(id)
                .map(|p| retreat_handover_amount(p, back, percent))
        })
        .unwrap_or(0.0);
        let returned = back - diverted;
        returned > 0.0 && returned < player.max_troops - player.troops
    }

    fn second_crown_city(&self, owner: u16, partner: u16) -> Option<usize> {
        use crate::protocol::CampaignRelation::Enemy;
        if !self.state.config.buildings_enabled
            || self.campaign_relations.get(&owner) == Some(&Enemy)
            || self.campaign_relations.get(&partner) == Some(&Enemy)
            || self.hero_claims.contains(&(owner, 0, 0))
            || !self.state.player(owner).is_some_and(|p| {
                p.alive && p.tile_count > 0 && p.leader.ability() == Ability::SecondCrown
            })
            || !self
                .state
                .player(partner)
                .is_some_and(|p| p.alive && p.tile_count > 0)
        {
            return None;
        }
        let map = &self.state.map;
        let owned = |b: &Building| {
            map.owner_id(b.tile_idx % map.width, b.tile_idx / map.width) == b.owner_id
        };
        if !self.active_cities(owner).any(owned) || self.active_cities(partner).any(owned) {
            return None;
        }
        self.buildings
            .iter()
            .position(|b| {
                b.owner_id == partner
                    && b.kind == BuildingKind::City
                    && b.level == 1
                    && b.under_construction
                    && owned(b)
            })
            .filter(|&index| self.buildings[index].ticks_until_complete > 0)
    }

    fn great_law_renewal(&self, owner: &Player, a: u16, b: u16) -> Option<(u16, u64, u8)> {
        let claim = (
            owner.id,
            (u64::from(a.min(b)) << 16) | u64::from(a.max(b)),
            3,
        );
        if !owner.alive
            || owner.tile_count == 0
            || owner.leader.ability() != Ability::GreatLawOfPeace
            || self.hero_claims.contains(&claim)
        {
            return None;
        }
        let mut needs_renewal = false;
        for id in [a, b] {
            if !allied(&self.state, owner.id, id) {
                return None;
            }
            let ally = self.state.player(id)?;
            let timers = [
                *owner.alliance_timers.get(&id)?,
                *ally.alliance_timers.get(&owner.id)?,
            ];
            if timers.contains(&0) {
                return None;
            }
            needs_renewal |= timers.iter().any(|&ticks| ticks < ALLIANCE_DURATION_TICKS);
        }
        needs_renewal.then_some(claim)
    }

    pub(crate) fn hero_alliance_formed(&mut self, a: u16, b: u16) {
        if !allied(&self.state, a, b) {
            return;
        }
        for (owner, partner) in [(a, b), (b, a)] {
            if let Some(index) = self.second_crown_city(owner, partner) {
                self.hero_claims.insert((owner, 0, 0));
                // Let the normal construction phase publish the completed city.
                let city = &mut self.buildings[index];
                city.ticks_until_complete = 0;
                let width = self.state.map.width;
                self.state.announce_leader_ability_to(
                    owner,
                    &[partner],
                    Some((city.tile_idx % width, city.tile_idx / width)),
                );
            }
        }
        let renewals: Vec<_> = self
            .state
            .players
            .iter()
            .filter_map(|p| {
                let claim = self.great_law_renewal(p, a, b)?;
                let map = &self.state.map;
                let tile = p
                    .home
                    .filter(|&(x, y)| map.owner_id(x, y) == p.id)
                    .or_else(|| {
                        let index = p.border_tiles.ones().next()?;
                        Some((index % map.width, index / map.width))
                    });
                Some((claim, tile))
            })
            .collect();
        for (claim, tile) in renewals {
            let owner = claim.0;
            for ally in [a, b] {
                self.state
                    .player_mut(owner)
                    .unwrap()
                    .alliance_timers
                    .insert(ally, ALLIANCE_DURATION_TICKS);
                self.state
                    .player_mut(ally)
                    .unwrap()
                    .alliance_timers
                    .insert(owner, ALLIANCE_DURATION_TICKS);
            }
            self.hero_claims.insert(claim);
            self.state.announce_leader_ability_to(owner, &[a, b], tile);
        }
    }

    fn hero_foundation_near(
        &self,
        owner: u16,
        center: u32,
        kind: BuildingKind,
        radius: u32,
        grid: &BuildingGrid,
    ) -> Option<u32> {
        let map = &self.state.map;
        crate::building::placement::nearest_structure_site(
            map,
            owner,
            kind,
            (center % map.width) as i32,
            (center / map.width) as i32,
            radius as i32,
            grid,
        )
        .filter(|&site| nearby(site, center, map.width, radius))
    }

    fn kitchen_study_city(&self, farm: Building, elapsed_ticks: u32) -> Option<usize> {
        let map = &self.state.map;
        let owned = |b: Building| {
            map.owner_id(b.tile_idx % map.width, b.tile_idx / map.width) == b.owner_id
        };
        if farm.kind != BuildingKind::Farm || farm.level < 2 || !owned(farm) {
            return None;
        }
        self.buildings
            .iter()
            .enumerate()
            .filter_map(|(index, &city)| {
                if city.kind != BuildingKind::City
                    || city.owner_id != farm.owner_id
                    || !city.under_construction
                    || city.level < 2
                    || city.ticks_until_complete <= elapsed_ticks
                    || !owned(city)
                {
                    return None;
                }
                let distance = tile_distance_squared(farm.tile_idx, city.tile_idx, map.width);
                (distance <= 12 * 12).then_some((distance, city.id, index))
            })
            .min()
            .map(|(_, _, index)| index)
            .filter(|&index| self.buildings[index].ticks_until_complete - elapsed_ticks > 1)
    }

    pub(crate) fn hero_structure_ready(&mut self, building: Building) {
        if building.kind == BuildingKind::Farm
            && self.state.config.buildings_enabled
            && self
                .state
                .player(building.owner_id)
                .is_some_and(|p| p.alive && p.leader.ability() == Ability::KitchenPhilosophy)
            && let Some(index) = self.kitchen_study_city(building, 0)
        {
            let city = &mut self.buildings[index];
            city.ticks_until_complete = city.ticks_until_complete.div_ceil(2);
            let width = self.state.map.width;
            self.state.announce_leader_ability(
                city.owner_id,
                Some((city.tile_idx % width, city.tile_idx / width)),
            );
        }
        if building.kind != BuildingKind::City {
            return;
        }
        let tile = Some((
            building.tile_idx % self.state.map.width,
            building.tile_idx / self.state.map.width,
        ));
        if building.level == 1 {
            let patrons: Vec<_> = self
                .state
                .players
                .iter()
                .filter_map(|p| match p.leader.ability() {
                    Ability::AlliedCityGrant { gold }
                        if allied(&self.state, p.id, building.owner_id) =>
                    {
                        Some((p.id, gold))
                    }
                    _ => None,
                })
                .collect();
            for (patron, gold) in patrons {
                if self.hero_claims.insert((patron, building.id, 1)) {
                    self.state
                        .player_mut(patron)
                        .unwrap()
                        .credit_gold(f64::from(gold));
                    self.state
                        .player_mut(building.owner_id)
                        .unwrap()
                        .credit_gold(f64::from(gold));
                    self.state
                        .announce_leader_ability_to(patron, &[building.owner_id], tile);
                }
            }
        } else if building.level == 2
            && self.state.config.buildings_enabled
            && self.campaign_allows_building(BuildingKind::Farm, 1)
            && self
                .state
                .player(building.owner_id)
                .is_some_and(|p| p.alive && p.leader.ability() == Ability::CityFarmFounding)
        {
            self.refresh_building_grid();
            if let Some(site) = self.hero_foundation_near(
                building.owner_id,
                building.tile_idx,
                BuildingKind::Farm,
                12,
                &self.building_grid,
            ) {
                self.found_hero_structure(building.owner_id, BuildingKind::Farm, site);
            }
        } else if self
            .state
            .player(building.owner_id)
            .is_some_and(|p| p.alive && p.leader.ability() == Ability::CityTowerUpgrade)
        {
            if let Some(index) = self.nearby_structure_upgrade(building, BuildingKind::Tower, 20) {
                self.hero_upgrade_structure(index, &[]);
            }
        }
    }

    fn hero_upgrade_structure(&mut self, index: usize, affected: &[u16]) {
        self.buildings[index].level += 1;
        let building = self.buildings[index];
        self.building_aggregates_dirty = true;
        if building.kind == BuildingKind::Tower {
            self.defense_grid_dirty = true;
            self.render_defense_dirty = true;
        }
        self.state.events.push(GameEvent::StructureUpgraded {
            id: building.id,
            tile_idx: building.tile_idx,
            kind: building.kind,
            level: building.level,
        });
        self.state.announce_leader_ability_to(
            building.owner_id,
            affected,
            Some((
                building.tile_idx % self.state.map.width,
                building.tile_idx / self.state.map.width,
            )),
        );
    }

    fn punt_garden_farm(&self, home: Building) -> Option<usize> {
        if home.kind != BuildingKind::Port
            || home.under_construction
            || self.hero_claims.contains(&(home.owner_id, home.id, 0))
        {
            return None;
        }
        self.nearby_structure_upgrade(home, BuildingKind::Farm, 12)
    }

    pub(crate) fn hero_trade_arrived(&mut self, origin_id: u64) {
        if !self.state.config.buildings_enabled {
            return;
        }
        let Ok(index) = self.buildings.binary_search_by_key(&origin_id, |b| b.id) else {
            return;
        };
        let home = self.buildings[index];
        if self
            .state
            .player(home.owner_id)
            .is_some_and(|p| p.alive && p.leader.ability() == Ability::PuntGardens)
            && let Some(farm) = self.punt_garden_farm(home)
        {
            self.hero_claims.insert((home.owner_id, origin_id, 0));
            self.hero_upgrade_structure(farm, &[]);
        }
    }

    fn counter_levy(&self, owner: u16, rival: u16, tower: Building) -> f64 {
        if !self.state.config.buildings_enabled
            || tower.kind != BuildingKind::Tower
            || tower.under_construction
            || rival == owner
            || self.hero_claims.contains(&(owner, tower.id, 0))
        {
            return 0.0;
        }
        let Some(captor) = self
            .state
            .player(owner)
            .filter(|p| p.alive && p.tile_count > 0)
        else {
            return 0.0;
        };
        let Ability::CounterLevy { percent, max_gold } = captor.leader.ability() else {
            return 0.0;
        };
        let Some(payer) = self
            .state
            .player(rival)
            .filter(|p| p.alive && p.tile_count > 0)
        else {
            return 0.0;
        };
        if allied(&self.state, owner, rival)
            || !self.attacks.iter().any(|attack| {
                attack.owner_id == rival
                    && attack.target_owner == owner
                    && !attack.retreating
                    && attack.troops > 0.0
            })
        {
            return 0.0;
        }
        (payer.gold.max(0.0) * f64::from(percent.min(100)) / 100.0).min(f64::from(max_gold))
    }

    fn relocate_last_city(&mut self, lost: Building, owner: u16) {
        if !self.state.config.buildings_enabled
            || lost.active_level() == 0
            || !self.state.player(owner).is_some_and(|p| {
                p.alive && p.tile_count > 0 && p.leader.ability() == Ability::LastCityRelocation
            })
            || self.hero_claims.contains(&(owner, 0, 0))
            || !self.campaign_allows_building(BuildingKind::City, 1)
            || self.active_cities(owner).next().is_some()
        {
            return;
        }
        self.refresh_building_grid();
        if let Some(site) = self.hero_foundation_near(
            owner,
            lost.tile_idx,
            BuildingKind::City,
            RELOCATION_RADIUS,
            &self.building_grid,
        ) {
            self.hero_claims.insert((owner, 0, 0));
            self.found_hero_structure(owner, BuildingKind::City, site);
        }
    }

    fn siege_breach_target(&self, owner: u16, rival: u16, captured: Building) -> Option<usize> {
        let level = captured.active_level();
        if !self.state.config.buildings_enabled
            || captured.kind != BuildingKind::Tower
            || level <= 1
            || rival == 0
            || rival == owner
            || allied(&self.state, owner, rival)
            || self.hero_claims.contains(&(owner, captured.id, 0))
        {
            return None;
        }

        let map = &self.state.map;
        self.buildings
            .iter()
            .enumerate()
            .filter(|(_, tower)| {
                tower.owner_id == rival
                    && tower.kind == BuildingKind::Tower
                    && (1..level).contains(&tower.active_level())
                    && map.owner_id(tower.tile_idx % map.width, tower.tile_idx / map.width) == rival
            })
            .filter_map(|(index, tower)| {
                let distance = tile_distance_squared(captured.tile_idx, tower.tile_idx, map.width);
                (distance <= 12 * 12).then_some((distance, tower.id, index))
            })
            .min()
            .map(|(_, _, index)| index)
    }

    fn cookham_exchange_city(&self, farm: Building) -> Option<usize> {
        if farm.kind != BuildingKind::Farm
            || farm.active_level() == 0
            || self.hero_claims.contains(&(farm.owner_id, farm.id, 0))
        {
            return None;
        }
        self.nearby_structure_upgrade(farm, BuildingKind::City, 12)
    }

    fn aksum_rout_target(&self, owner: u16, rival: u16, city: Building) -> Option<usize> {
        if city.kind != BuildingKind::City
            || city.active_level() == 0
            || owner == rival
            || !self.state.player(rival).is_some_and(|p| p.alive)
            || allied(&self.state, owner, rival)
            || self.hero_claims.contains(&(owner, city.id, 0))
        {
            return None;
        }
        self.attacks
            .iter()
            .enumerate()
            .filter(|(_, attack)| {
                attack.owner_id == rival
                    && attack.target_owner != 0
                    && !attack.retreating
                    && !attack.to_conquer.is_empty()
                    && attack.troops >= self.state.config.attack_cost_neutral
                    && self
                        .state
                        .player(attack.target_owner)
                        .is_some_and(|p| p.alive)
                    && !allied(&self.state, rival, attack.target_owner)
            })
            .max_by(|(_, a), (_, b)| a.troops.total_cmp(&b.troops).then_with(|| b.id.cmp(&a.id)))
            .map(|(index, _)| index)
    }

    fn akkad_garrison_site(
        &self,
        owner: u16,
        rival: u16,
        city: Building,
        grid: &BuildingGrid,
    ) -> Option<u32> {
        const RADIUS: u32 = 12;
        let map = &self.state.map;
        if city.kind != BuildingKind::City
            || city.active_level() == 0
            || rival == 0
            || rival == owner
            || allied(&self.state, owner, rival)
            || self.hero_claims.contains(&(owner, city.id, 0))
            || !self.campaign_allows_building(BuildingKind::Tower, 1)
            || self.buildings.iter().any(|tower| {
                tower.kind == BuildingKind::Tower
                    && tower.owner_id == owner
                    && map.owner_id(tower.tile_idx % map.width, tower.tile_idx / map.width) == owner
                    && nearby(tower.tile_idx, city.tile_idx, map.width, RADIUS)
            })
        {
            return None;
        }
        self.hero_foundation_near(owner, city.tile_idx, BuildingKind::Tower, RADIUS, grid)
    }

    pub(crate) fn hero_structure_transferred(&mut self, building: Building, previous_owner: u16) {
        if !matches!(
            building.kind,
            BuildingKind::City | BuildingKind::Tower | BuildingKind::Farm
        ) || previous_owner == 0
            || allied(&self.state, building.owner_id, previous_owner)
        {
            return;
        }
        if building.kind == BuildingKind::Farm {
            if self.state.config.buildings_enabled
                && self.state.player(previous_owner).is_some_and(|p| {
                    p.alive && p.tile_count > 0 && p.leader.ability() == Ability::CookhamExchange
                })
                && let Some(city) = self.cookham_exchange_city(Building {
                    owner_id: previous_owner,
                    ..building
                })
            {
                self.hero_claims.insert((previous_owner, building.id, 0));
                self.hero_upgrade_structure(city, &[building.owner_id]);
            }
            return;
        }
        let tile = Some((
            building.tile_idx % self.state.map.width,
            building.tile_idx / self.state.map.width,
        ));
        if building.kind == BuildingKind::Tower {
            let owner = building.owner_id;
            if self.state.player(owner).is_some_and(|p| {
                p.alive && p.tile_count > 0 && p.leader.ability() == Ability::SiegeBreach
            }) && let Some(index) = self.siege_breach_target(owner, previous_owner, building)
            {
                let demolished = self.buildings.remove(index);
                self.hero_claims.insert((owner, building.id, 0));
                self.building_grid.mark_dirty();
                self.building_aggregates_dirty = true;
                self.bot_sam_tiles_cache = None;
                self.defense_grid_dirty = true;
                self.render_defense_dirty = true;
                let width = self.state.map.width;
                self.state.announce_leader_ability_to(
                    owner,
                    &[previous_owner],
                    Some((demolished.tile_idx % width, demolished.tile_idx / width)),
                );
            }
            let levy = self.counter_levy(owner, previous_owner, building);
            if levy > 0.0 {
                self.hero_claims.insert((owner, building.id, 0));
                self.state
                    .player_mut(previous_owner)
                    .unwrap()
                    .credit_gold(-levy);
                self.state.player_mut(owner).unwrap().credit_gold(levy);
                self.state
                    .announce_leader_ability_to(owner, &[previous_owner], tile);
            }
            return;
        }
        self.relocate_last_city(building, previous_owner);
        if self.state.config.buildings_enabled
            && self.state.player(building.owner_id).is_some_and(|p| {
                p.alive && p.tile_count > 0 && p.leader.ability() == Ability::AkkadGarrison
            })
        {
            self.refresh_building_grid();
            if let Some(site) = self.akkad_garrison_site(
                building.owner_id,
                previous_owner,
                building,
                &self.building_grid,
            ) {
                self.hero_claims.insert((building.owner_id, building.id, 0));
                self.found_hero_structure(building.owner_id, BuildingKind::Tower, site);
            }
        }
        if self.state.config.buildings_enabled
            && self.state.player(building.owner_id).is_some_and(|p| {
                p.alive && p.tile_count > 0 && p.leader.ability() == Ability::AksumRout
            })
            && let Some(attack) =
                self.aksum_rout_target(building.owner_id, previous_owner, building)
        {
            self.hero_claims.insert((building.owner_id, building.id, 0));
            self.attacks[attack].retreating = true;
            self.state
                .announce_leader_ability_to(building.owner_id, &[previous_owner], tile);
        }
        if self.state.config.buildings_enabled
            && building.active_level() > 1
            && self
                .state
                .player(previous_owner)
                .is_some_and(|victim| victim.leader.ability() == Ability::CityDenial)
            && let Ok(index) = self.buildings.binary_search_by_key(&building.id, |b| b.id)
            && self.hero_claims.insert((previous_owner, building.id, 0))
        {
            let city = &mut self.buildings[index];
            city.level = building.active_level() - 1;
            city.under_construction = false;
            city.ticks_until_complete = 0;
            self.building_aggregates_dirty = true;
            self.state
                .announce_leader_ability_to(previous_owner, &[building.owner_id], tile);
        }
        if let Some(victim) = self.state.player(previous_owner)
            && victim.alive
            && victim.tile_count > 0
            && let Ability::CityEvacuation { troops } = victim.leader.ability()
            && self.hero_claims.insert((previous_owner, building.id, 0))
        {
            let victim = self.state.player_mut(previous_owner).unwrap();
            let before = victim.troops;
            victim.credit_troops_capped(f64::from(troops), victim.max_troops);
            if victim.troops > before {
                self.state
                    .announce_leader_ability_to(previous_owner, &[building.owner_id], tile);
            }
        }
        if building.under_construction {
            return;
        }
        if self.state.config.buildings_enabled
            && let Some(captor) = self.state.player(building.owner_id)
            && captor.alive
            && captor.tile_count > 0
            && building.kind == BuildingKind::City
            && building.active_level() > 0
            && let Ability::StreltsyMuster { troops } = captor.leader.ability()
            && !self.hero_claims.contains(&(building.owner_id, 0, 0))
        {
            let gained = {
                let captor = self.state.player_mut(building.owner_id).unwrap();
                let before = captor.troops;
                captor.credit_troops_capped(f64::from(troops), captor.max_troops);
                captor.troops - before
            };
            if gained > 0.0 && self.hero_claims.insert((building.owner_id, 0, 0)) {
                self.state
                    .announce_leader_ability_to(building.owner_id, &[previous_owner], tile);
            }
        }
        if let Some(captor) = self.state.player(building.owner_id)
            && captor.alive
            && let Ability::SharedCitySpoils { gold } = captor.leader.ability()
            && self.hero_claims.insert((building.owner_id, building.id, 0))
        {
            let recipients: Vec<_> = self
                .state
                .players
                .iter()
                .filter(|p| {
                    p.id == building.owner_id || allied(&self.state, building.owner_id, p.id)
                })
                .map(|p| p.id)
                .collect();
            let share = f64::from(gold) / recipients.len() as f64;
            for &id in &recipients {
                self.state.player_mut(id).unwrap().credit_gold(share);
            }
            self.state
                .announce_leader_ability_to(building.owner_id, &recipients, tile);
        }
    }

    pub(crate) fn hero_coastal_foundings(&mut self) {
        if !self.state.config.buildings_enabled
            || !self.campaign_allows_building(BuildingKind::Port, 1)
        {
            return;
        }
        let coasts: Vec<_> =
            self.state
                .events
                .iter()
                .filter_map(|event| {
                    if let GameEvent::TileCaptured {
                        x,
                        y,
                        new_owner,
                        previous_owner,
                        ..
                    } = *event
                        && previous_owner != 0
                        && previous_owner != new_owner
                        && self.state.player(new_owner).is_some_and(|p| {
                            p.alive && p.leader.ability() == Ability::CoastalFounding
                        })
                        && !self.hero_claims.contains(&(new_owner, 0, 0))
                        && crate::building::placement::is_shore_land_tile(&self.state.map, x, y)
                    {
                        Some((new_owner, x, y))
                    } else {
                        None
                    }
                })
                .collect();
        for (owner, x, y) in coasts {
            if self.hero_claims.contains(&(owner, 0, 0)) {
                continue;
            }
            self.refresh_building_grid();
            let Some(tile_idx) = crate::building::placement::nearest_structure_site(
                &self.state.map,
                owner,
                BuildingKind::Port,
                x as i32,
                y as i32,
                12,
                &self.building_grid,
            ) else {
                continue;
            };
            self.hero_claims.insert((owner, 0, 0));
            self.found_hero_structure(owner, BuildingKind::Port, tile_idx);
        }
    }

    fn found_hero_structure(&mut self, owner: u16, kind: BuildingKind, tile_idx: u32) {
        let id = self.state.next_building_id;
        self.state.next_building_id = id.wrapping_add(1).max(1);
        let building = Building {
            id,
            owner_id: owner,
            tile_idx,
            kind,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        };
        self.add_building(building);
        if kind == BuildingKind::City {
            self.state.player_mut(owner).unwrap().cities += 1;
        }
        self.state.events.push(GameEvent::StructureSpawned {
            id,
            owner_id: owner,
            tile_idx,
            kind,
            level: 1,
        });
        self.state
            .events
            .push(GameEvent::StructureReady { id, tile_idx, kind });
        if kind == BuildingKind::City {
            self.hero_structure_ready(building);
        }
        self.state.announce_leader_ability(
            owner,
            Some((
                tile_idx % self.state.map.width,
                tile_idx / self.state.map.width,
            )),
        );
    }

    /// Extra gold in game seconds. Both the income tick and the HUD call this.
    /// Patronage excludes other hero income, so two patrons cannot compound it.
    pub fn hero_gold_per_second(&self, owner: u16) -> f64 {
        use super::income_rates::{gold_income_per_second, troop_income_per_second};
        let Some(player) = self
            .state
            .player(owner)
            .filter(|p| p.alive && p.tile_count > 0)
        else {
            return 0.0;
        };
        let cfg = &self.state.config;
        match player.leader.ability() {
            Ability::AllyPatronage { percent } => {
                self.state
                    .players
                    .iter()
                    .filter(|p| {
                        p.tile_count > player.tile_count && allied(&self.state, owner, p.id)
                    })
                    .map(|p| {
                        gold_income_per_second(
                            p.tile_count,
                            self.building_aggregates
                                .get(p.id as usize)
                                .copied()
                                .unwrap_or_default(),
                            p.leader,
                            cfg,
                        ) * if p.player_type == crate::player::PlayerType::Bot {
                            0.75
                        } else {
                            1.0
                        }
                    })
                    .fold(0.0_f64, f64::max)
                    * f64::from(percent)
                    / 100.0
            }
            Ability::SurplusTreasury { troops_per_gold }
                if troops_per_gold > 0
                    && !self.attacks.iter().any(|a| {
                        !a.retreating
                            && a.troops > 0.0
                            && a.target_owner != 0
                            && (a.owner_id == owner || a.target_owner == owner)
                    }) =>
            {
                let agg = self
                    .building_aggregates
                    .get(owner as usize)
                    .copied()
                    .unwrap_or_default();
                let generated = cfg.per_tick(troop_income_per_second(
                    player.tile_count,
                    agg,
                    player.leader,
                    cfg,
                ));
                let wasted = (generated - (player.max_troops - player.troops).max(0.0)).max(0.0);
                wasted / cfg.per_tick(1.0).max(f64::EPSILON) / f64::from(troops_per_gold)
            }
            Ability::ImperialStandards { .. } => {
                let agg = self
                    .building_aggregates
                    .get(owner as usize)
                    .copied()
                    .unwrap_or_default();
                super::income_rates::imperial_standard_income(agg, player.leader, cfg).0
            }
            Ability::GuildNetwork { gold_per_second } => {
                // ponytail: scan this ruler's factories and cities; index active
                // city owners in BuildingGrid if dense guild networks need it.
                let cities: Vec<_> = self
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.owner_id == owner && b.kind == BuildingKind::City && b.active_level() > 0
                    })
                    .map(|b| b.tile_idx)
                    .collect();
                let levels: u32 = self
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.owner_id == owner
                            && b.kind == BuildingKind::Factory
                            && b.active_level() > 0
                    })
                    .filter(|factory| {
                        cities
                            .iter()
                            .any(|&city| nearby(factory.tile_idx, city, self.state.map.width, 20))
                    })
                    .map(|b| u32::from(b.active_level()))
                    .sum();
                f64::from(levels) * f64::from(gold_per_second)
            }
            _ => 0.0,
        }
    }

    pub(crate) fn hero_warship_prize(&mut self, owner: u16, index: usize) -> bool {
        let Some(player) = self.state.player(owner).filter(|p| p.alive) else {
            return false;
        };
        let Ability::WarshipPrize { percent } = player.leader.ability() else {
            return false;
        };
        let fleet = &mut self.fleets[index];
        if fleet.unit_type != UnitType::Warship || percent == 0 {
            return false;
        }
        let previous = fleet.owner_id;
        fleet.owner_id = owner;
        fleet.target_owner = 0;
        fleet.troops = UnitType::Warship.max_health() * f64::from(percent.min(100)) / 100.0;
        fleet.retreating = false;
        fleet.warship = Some(crate::warp_fleet::WarshipState::new(fleet.current_tile));
        fleet.src_tile = fleet.current_tile;
        fleet.dst_tile = fleet.current_tile;
        fleet.replace_path(Vec::new());
        let tile = fleet.current_tile;
        self.state.announce_leader_ability_to(
            owner,
            &[previous],
            Some((tile % self.state.map.width, tile / self.state.map.width)),
        );
        true
    }
}
