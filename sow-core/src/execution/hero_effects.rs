//! Situational effects shared by construction, conquest, income and naval combat.
//! Rewards use the authoritative owners and normal placement/cap rules.
use crate::building::Building;
use crate::engine::SowEngine;
use crate::game::{BuildingKind, GameEvent, UnitType};
use crate::geo_entities::Ability;

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

fn nearby(a: u32, b: u32, width: u32) -> bool {
    let dx = (a % width).abs_diff(b % width);
    let dy = (a / width).abs_diff(b / width);
    u64::from(dx).pow(2) + u64::from(dy).pow(2) <= 20 * 20
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
                    | Ability::SharedCitySpoils { .. }
                    | Ability::CityEvacuation { .. }
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
            Ability::AllyPatronage { .. }
            | Ability::SurplusTreasury { .. }
            | Ability::GuildNetwork { .. } => {
                if self.hero_gold_per_second(owner) > 0.0 {
                    Active
                } else {
                    Conditional
                }
            }
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
                    && self.city_upgrade_tower(city.tile_idx, owner).is_some()
            })),
            Ability::SharedCitySpoils { .. } => ready(self.buildings.iter().any(|b| {
                b.kind == BuildingKind::City
                    && !b.under_construction
                    && b.owner_id != 0
                    && b.owner_id != owner
                    && !allied(&self.state, owner, b.owner_id)
                    && !self.hero_claims.contains(&(owner, b.id, 0))
            })),
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
            Ability::AttackerLoss { .. } | Ability::AllianceCost { .. } => Conditional,
        };
        Some(feedback)
    }

    fn city_upgrade_tower(&self, city_tile: u32, owner: u16) -> Option<usize> {
        self.buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.owner_id == owner
                    && b.kind == BuildingKind::Tower
                    && !b.under_construction
                    && b.level < b.kind.max_level()
                    && self.campaign_allows_building(b.kind, b.level + 1)
                    && nearby(b.tile_idx, city_tile, self.state.map.width)
            })
            .min_by_key(|(_, b)| (b.level, b.id))
            .map(|(i, _)| i)
    }

    pub(crate) fn hero_structure_ready(&mut self, building: Building) {
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
        } else if self
            .state
            .player(building.owner_id)
            .is_some_and(|p| p.alive && p.leader.ability() == Ability::CityTowerUpgrade)
        {
            let tower = self.city_upgrade_tower(building.tile_idx, building.owner_id);
            if let Some(index) = tower {
                self.buildings[index].level += 1;
                let tower = self.buildings[index];
                self.building_aggregates_dirty = true;
                self.defense_grid_dirty = true;
                self.render_defense_dirty = true;
                self.state.events.push(GameEvent::StructureUpgraded {
                    id: tower.id,
                    tile_idx: tower.tile_idx,
                    kind: tower.kind,
                    level: tower.level,
                });
                self.state.announce_leader_ability(
                    building.owner_id,
                    Some((
                        tower.tile_idx % self.state.map.width,
                        tower.tile_idx / self.state.map.width,
                    )),
                );
            }
        }
    }

    pub(crate) fn hero_city_transferred(&mut self, building: Building, previous_owner: u16) {
        if building.kind != BuildingKind::City
            || previous_owner == 0
            || allied(&self.state, building.owner_id, previous_owner)
        {
            return;
        }
        let tile = Some((
            building.tile_idx % self.state.map.width,
            building.tile_idx / self.state.map.width,
        ));
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
            let id = self.state.next_building_id;
            self.state.next_building_id = id.wrapping_add(1).max(1);
            self.add_building(Building {
                id,
                owner_id: owner,
                tile_idx,
                kind: BuildingKind::Port,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            });
            self.state.events.push(GameEvent::StructureSpawned {
                id,
                owner_id: owner,
                tile_idx,
                kind: BuildingKind::Port,
                level: 1,
            });
            self.state.events.push(GameEvent::StructureReady {
                id,
                tile_idx,
                kind: BuildingKind::Port,
            });
            self.state.announce_leader_ability(
                owner,
                Some((
                    tile_idx % self.state.map.width,
                    tile_idx / self.state.map.width,
                )),
            );
        }
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
                            .any(|&city| nearby(factory.tile_idx, city, self.state.map.width))
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
