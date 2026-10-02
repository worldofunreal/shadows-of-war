use crate::engine::SowEngine;

impl SowEngine {
    pub fn tick(&mut self) {
        if let crate::game::GamePhase::Spawning { end_tick } = self.state.phase {
            self.state.tick += 1;

            // ── Stagger ghost (is_ai_controlled) spawns across the deploy window ──
            // Each unspawned ghost has a deterministic spawn moment, uniformly and
            // independently placed in [1, end_tick-1] via WyRand(seed ^ pid). When
            // the current tick reaches that moment, the ghost is placed via the
            // same find_valid_spawn + place_spawn path used by the safety net
            // below. This avoids both a continuous scatter-storm and a mass-pop
            // at the end of the deploy window.
            // Each ghost fires AT MOST once; any that miss (no valid tile) are
            // caught by the safety net at the phase transition. Deterministic →
            // lockstep-safe across all clients (same seed, same pid → same moment).
            if end_tick >= 2 {
                use crate::rng::NextIntExt;
                use wyrand::WyRand;
                let now = self.state.tick;
                let seed = self.state.seed;
                // Collect due ghosts in ascending pid order for deterministic
                // tile ownership progression (find_valid_spawn sees prior spawns).
                let due: Vec<u16> = self
                    .state
                    .players
                    .iter()
                    .filter(|p| {
                        p.is_ai_controlled && !p.has_spawned && {
                            let mut m = WyRand::new(seed.wrapping_add(p.id as u64));
                            m.next_int(1, (end_tick as i32) - 1) as u64 == now
                        }
                    })
                    .map(|p| p.id)
                    .collect();
                for pid in due {
                    let mut rng = WyRand::new(seed.wrapping_add(pid as u64).wrapping_add(now));
                    // OF teamSpawnArea parity: team ghosts spawn inside their
                    // map half (Red left, Blue right) — zone is cohesion, the
                    // member floor is separation. Ring/anchor are fallbacks
                    // only when the zone can't place them. (The world-map
                    // path lands here: random_spawn=false, so spawn_human
                    // never places anyone.)
                    let team = self
                        .state
                        .players
                        .iter()
                        .find(|p| p.id == pid)
                        .and_then(|p| p.team);
                    let spawn_point = team
                        .as_ref()
                        .map(|t| self.team_spawn_area(t))
                        .and_then(|area| self.find_spawn_in_area(&mut rng, area))
                        .or_else(|| {
                            team.as_ref().and_then(|t| self.team_centroid(t)).and_then(
                                |(cx, cy)| self.find_valid_spawn_near(&mut rng, cx, cy, 12, 36),
                            )
                        })
                        .or_else(|| self.find_valid_spawn(&mut rng));
                    if let Some((sx, sy)) = spawn_point {
                        self.state.place_spawn(pid, sx, sy);
                    }
                }
            }

            if self.state.tick >= end_tick {
                self.state.phase = crate::game::GamePhase::Playing;
                // Auto-spawn players who missed the window
                let unspawned: Vec<u16> = self
                    .state
                    .players
                    .iter()
                    .filter(|p| !p.has_spawned)
                    .map(|p| p.id)
                    .collect();

                for pid in unspawned {
                    use wyrand::WyRand;
                    let mut rng = WyRand::new(self.state.seed.wrapping_add(pid as u64));
                    // Teamed stragglers (incl. real humans) land in their
                    // team's half too — the team must not split at spawn.
                    let spawn_point = self
                        .state
                        .players
                        .iter()
                        .find(|p| p.id == pid)
                        .and_then(|p| p.team)
                        .map(|t| self.team_spawn_area(&t))
                        .and_then(|area| self.find_spawn_in_area(&mut rng, area))
                        .or_else(|| self.find_valid_spawn(&mut rng));
                    if let Some((sx, sy)) = spawn_point {
                        self.state.place_spawn(pid, sx, sy);
                        log::info!("Auto-spawned missing player {} at {}, {}", pid, sx, sy);
                    }
                }
            }
            return;
        }

        self.state.events.clear(); // Prevent unbounded memory leak (was growing infinitely on tile capture)
        self.state.tick();

        if self.sea_lane_calc.is_some() {
            crate::sea_lane::update_sea_lanes(self);
        }

        self.execute_income();
        self.prune_alliance_diplomacy();
        self.execute_ai_think();
        self.execute_construction();
        self.execute_ship_production();
        self.execute_projectiles();
        self.execute_sam();
        self.execute_combat();
        self.apply_campaign_unlocks_and_support();

        // Sync building ownership with tile ownership
        for b in &mut self.buildings {
            let col = b.tile_idx % self.state.map.width;
            let row = b.tile_idx / self.state.map.width;
            let tile_owner = self.state.map.owner_id(col, row);

            // Only transfer if the tile has a new valid owner
            if tile_owner != 0 && tile_owner != b.owner_id {
                let old_owner = b.owner_id;
                let new_owner = tile_owner;
                let kind = b.kind;

                // Transfer ownership
                b.owner_id = new_owner;
                self.building_aggregates_dirty = true;
                if kind == crate::game::BuildingKind::Bunker {
                    self.defense_grid_dirty = true;
                }

                // Update player counts if necessary
                if kind == crate::game::BuildingKind::City {
                    if old_owner != 0
                        && let Some(p) = self.state.player_mut(old_owner)
                    {
                        p.cities = p.cities.saturating_sub(1);
                    }
                    if new_owner != 0
                        && let Some(p) = self.state.player_mut(new_owner)
                    {
                        p.cities += 1;
                    }
                }
            }
        }

        self.execute_fleets();
        self.check_winner();

        let mut expired_alliances = Vec::new();
        for player in &mut self.state.players {
            let pid = player.id;
            if player.emoji_timer > 0 && !player.emoji_pinned {
                player.emoji_timer -= 1;
                if player.emoji_timer == 0 {
                    player.active_emoji = None;
                }
            }

            // Decay alliance timers
            let mut expired_for_player = Vec::new();
            for (&ally_id, timer) in &mut player.alliance_timers {
                if *timer > 0 {
                    *timer -= 1;
                    if *timer == 0 {
                        expired_for_player.push(ally_id);
                    }
                }
            }
            for ally_id in expired_for_player {
                player.alliances.retain(|&id| id != ally_id);
                player.alliance_timers.remove(&ally_id);
                expired_alliances.push((pid, ally_id));
            }
        }

        // Mutual expiration enforcement
        for (a, b) in expired_alliances {
            if let Some(p_b) = self.state.player_mut(b) {
                p_b.alliances.retain(|&id| id != a);
                p_b.alliance_timers.remove(&a);
            }
        }
    }

    fn apply_campaign_unlocks_and_support(&mut self) {
        let unlock_target = self.state.config.buildings_unlock_after_defeated.clone();
        if !self.state.config.buildings_enabled
            && unlock_target.as_ref().is_some_and(|name| {
                self.state
                    .players
                    .iter()
                    .any(|player| player.name == *name && !player.alive)
            })
        {
            self.state.config.buildings_enabled = true;
        }

        let Some(support) = self.state.config.campaign_support.clone() else {
            return;
        };
        if !self
            .state
            .players
            .iter()
            .any(|player| player.name == support.after_defeated && !player.alive)
        {
            return;
        }
        let contact_check_ticks = (1000.0 / self.state.config.tick_rate_ms.max(1.0)).ceil() as u64;
        let immediate_support_due = self
            .campaign_support_next_tick
            .values()
            .any(|&next_tick| next_tick <= self.state.tick);
        if !immediate_support_due && self.state.tick % contact_check_ticks.max(1) != 0 {
            return;
        }
        let Some(receiver) =
            self.state.players.iter().find(|player| {
                player.player_type == crate::player::PlayerType::Human && player.alive
            })
        else {
            return;
        };
        let receiver_id = receiver.id;
        let allied_ids = receiver.alliances.clone();
        let map_w = self.state.map.width;
        let mut contacted = std::collections::HashSet::new();
        for tile in receiver.border_tiles.ones() {
            let x = tile % map_w;
            let y = tile / map_w;
            self.state.map.for_each_neighbor(x, y, |nx, ny| {
                let other_id = self.state.map.owner_id(nx, ny);
                if other_id != 0 && other_id != receiver_id {
                    contacted.insert(other_id);
                }
            });
        }
        let eligible: Vec<(u16, u32)> = self
            .state
            .players
            .iter()
            .filter(|player| {
                player.id != receiver_id
                    && player.player_type != crate::player::PlayerType::Human
                    && player.alive
                    && self.campaign_support_intervals.contains_key(&player.id)
                    && self.campaign_relations.get(&player.id)
                        == Some(&crate::protocol::CampaignRelation::Allied)
                    && contacted.contains(&player.id)
                    && player.alliances.contains(&receiver_id)
                    && allied_ids.contains(&player.id)
            })
            .map(|player| {
                (
                    player.id,
                    self.campaign_support_intervals
                        .get(&player.id)
                        .copied()
                        .unwrap_or(30),
                )
            })
            .collect();
        let eligible_ids: std::collections::HashSet<u16> =
            eligible.iter().map(|(id, _)| *id).collect();
        self.campaign_support_next_tick
            .retain(|id, _| eligible_ids.contains(id));
        let share = f64::from(support.share_percent) / 100.0;
        for (ally_id, interval_seconds) in eligible {
            let interval_ticks = (u64::from(interval_seconds) * 1000
                / u64::from(self.state.config.tick_rate_ms.max(1.0) as u32))
            .max(1);
            let Some(next_tick) = self.campaign_support_next_tick.get(&ally_id).copied() else {
                self.campaign_support_next_tick
                    .insert(ally_id, self.state.tick.saturating_add(interval_ticks));
                continue;
            };
            if self.state.tick < next_tick {
                continue;
            }
            self.campaign_support_next_tick
                .insert(ally_id, self.state.tick.saturating_add(interval_ticks));
            let Some(sender) = self.state.player(ally_id) else {
                continue;
            };
            let gold = sender.gold * share;
            let receiver_troop_capacity = self
                .state
                .player(receiver_id)
                .map(|player| (player.max_troops - player.troops).max(0.0))
                .unwrap_or(0.0);
            let troops = (sender.troops * share)
                .min((sender.troops - 1.0).max(0.0))
                .min(receiver_troop_capacity);
            if gold <= 0.0 && troops <= 0.0 {
                continue;
            }
            self.apply_intents(&[crate::protocol::StampedIntent {
                player_id: ally_id,
                intent: crate::protocol::GameplayIntent::SendResources {
                    target_player: receiver_id,
                    gold,
                    troops,
                },
            }]);
        }
    }

    fn check_winner(&mut self) {
        // Campaign objectives decide tutorial completion; the client still handles player death.
        if self.state.config.tutorial || self.state.winner.is_some() {
            return;
        }

        if self.state.total_land_tiles == 0 {
            self.state.total_land_tiles = self
                .state
                .map
                .terrain
                .iter()
                .filter(|t| t.is_land())
                .count() as u32;
            if self.state.total_land_tiles == 0 {
                self.state.total_land_tiles = 1; // Prevent division by zero
            }
        }

        let win_threshold = (self.state.total_land_tiles as f32
            * self.state.config.map_control_win_percentage) as u32;

        if self.state.config.game_mode == "Teams"
            || self.state.config.game_mode == "HumansVsNations"
        {
            self.check_team_winner(win_threshold);
        } else {
            self.check_ffa_winner(win_threshold);
        }
    }

    fn check_ffa_winner(&mut self, win_threshold: u32) {
        let mut alive_players = 0;
        let mut last_alive_id = None;
        let mut map_control_winner = None;

        for p in &self.state.players {
            if p.alive && p.tile_count > 0 {
                alive_players += 1;
                last_alive_id = Some(p.id);
                if p.tile_count >= win_threshold {
                    map_control_winner = Some(p.id);
                }
            }
        }

        if let Some(wid) = map_control_winner {
            self.end_game(wid, None);
        } else if alive_players == 1 {
            if let Some(wid) = last_alive_id {
                self.end_game(wid, None);
            }
        } else if alive_players == 0 && !self.state.players.is_empty() {
            self.state.phase = crate::game::GamePhase::GameOver;
        }
    }

    pub(crate) fn check_team_winner(&mut self, win_threshold: u32) {
        use crate::protocol::Team;
        use std::collections::HashMap;

        let mut team_tiles: HashMap<Team, u32> = HashMap::new();
        let mut best_player_on_team: HashMap<Team, (u16, u32)> = HashMap::new();
        let mut unaffiliated_with_land = 0;

        for p in &self.state.players {
            if !p.alive || p.tile_count == 0 {
                continue;
            }
            let Some(team) = p.team else {
                unaffiliated_with_land += 1;
                continue;
            };
            *team_tiles.entry(team).or_insert(0) += p.tile_count;
            best_player_on_team
                .entry(team)
                .and_modify(|(best_id, best_tiles)| {
                    if p.tile_count > *best_tiles {
                        *best_id = p.id;
                        *best_tiles = p.tile_count;
                    }
                })
                .or_insert((p.id, p.tile_count));
        }

        let mut map_control_team = None;
        let mut teams_with_land = 0;
        let mut last_team_with_land = None;

        for (&team, &tiles) in &team_tiles {
            if tiles == 0 {
                continue;
            }
            teams_with_land += 1;
            last_team_with_land = Some(team);
            if tiles >= win_threshold {
                map_control_team = Some(team);
            }
        }

        if let Some(team) = map_control_team {
            if let Some(&(wid, _)) = best_player_on_team.get(&team) {
                self.end_game(wid, Some(team));
            }
        } else if teams_with_land == 1 && unaffiliated_with_land == 0 {
            if let Some(team) = last_team_with_land
                && let Some(&(wid, _)) = best_player_on_team.get(&team)
            {
                self.end_game(wid, Some(team));
            }
        } else if teams_with_land == 0
            && unaffiliated_with_land == 0
            && !self.state.players.is_empty()
        {
            self.state.phase = crate::game::GamePhase::GameOver;
        }
    }

    fn end_game(&mut self, winner_id: u16, winning_team: Option<crate::protocol::Team>) {
        self.state.winner = Some(winner_id);
        self.state.winning_team = winning_team;
        self.state.phase = crate::game::GamePhase::GameOver;
        self.state.events.push(crate::game::GameEvent::GameOver {
            winner_id,
            winning_team,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::SowEngine;
    use crate::game::{GameEvent, GamePhase, GameState};
    use crate::game_config::{CampaignSupport, GameConfig};
    use crate::player::{Player, PlayerType};
    use crate::protocol::{GameplayIntent, StampedIntent, Team};
    use crate::water_components::WaterComponents;

    #[test]
    fn campaign_support_waits_for_defeat_and_contact_then_sends_half_each_interval() {
        let milestone = "The Iceni Despoilers";
        let config = GameConfig {
            tick_rate_ms: 1000.0,
            buildings_enabled: false,
            buildings_unlock_after_defeated: Some(milestone.to_string()),
            campaign_support: Some(CampaignSupport {
                after_defeated: milestone.to_string(),
                share_percent: 50,
            }),
            ..GameConfig::default()
        };
        let mut state = GameState::new(1, 5, 5, config.clone());
        state.phase = GamePhase::Playing;

        let mut human = Player::new_human(1, "Boudica".into(), [1.0; 3], &config);
        human.alliances.push(2);
        human.gold = 0.0;
        human.troops = 0.0;
        human.max_troops = 500.0;
        human.border_tiles.insert(6); // (1, 1), beside (0, 1)
        state.register_player(human);

        let mut ally = Player::new_human(2, "Snettisham".into(), [1.0; 3], &config);
        ally.player_type = PlayerType::Bot;
        ally.alliances.push(1);
        ally.gold = 500.0;
        ally.troops = 500.0;
        state.register_player(ally);

        let mut roman = Player::new_human(3, milestone.into(), [1.0; 3], &config);
        roman.team = Some(Team::Blue);
        state.register_player(roman);

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine
            .campaign_relations
            .insert(2, crate::protocol::CampaignRelation::Allied);
        engine.campaign_support_intervals.insert(2, 8);
        engine.state.tick = 1;
        engine.apply_campaign_unlocks_and_support();
        assert!(!engine.state.config.buildings_enabled);
        assert!(
            !engine
                .state
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ResourceTransferred { .. }))
        );

        engine.state.player_mut(3).unwrap().alive = false;
        engine.apply_campaign_unlocks_and_support();
        assert!(engine.state.config.buildings_enabled);
        assert!(
            !engine
                .state
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ResourceTransferred { .. }))
        );

        engine.state.map.set_owner_id(0, 1, 2);
        engine.apply_campaign_unlocks_and_support();
        assert_eq!(engine.state.player(1).unwrap().gold, 0.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 500.0);

        engine.state.events.clear();
        engine.state.tick = 8;
        engine.apply_campaign_unlocks_and_support();
        assert!(
            !engine
                .state
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ResourceTransferred { .. }))
        );

        engine.state.tick = 9;
        engine.apply_campaign_unlocks_and_support();
        assert_eq!(engine.state.player(1).unwrap().gold, 250.0);
        assert_eq!(engine.state.player(1).unwrap().troops, 250.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 250.0);
        assert_eq!(engine.state.player(2).unwrap().troops, 250.0);

        engine.state.events.clear();
        engine.state.tick = 16;
        engine.apply_campaign_unlocks_and_support();
        assert!(
            !engine
                .state
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ResourceTransferred { .. }))
        );

        engine.state.tick = 17;
        engine.apply_campaign_unlocks_and_support();
        assert_eq!(engine.state.player(1).unwrap().gold, 375.0);
        assert_eq!(engine.state.player(1).unwrap().troops, 375.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 125.0);
        assert_eq!(engine.state.player(2).unwrap().troops, 125.0);
    }

    #[test]
    fn newly_allied_faction_sends_first_support_on_the_next_tick_then_waits_for_interval() {
        let milestone = "The Iceni Despoilers";
        let config = GameConfig {
            tick_rate_ms: 100.0,
            campaign_support: Some(CampaignSupport {
                after_defeated: milestone.to_string(),
                share_percent: 50,
            }),
            ..GameConfig::default()
        };
        let mut state = GameState::new(9, 5, 5, config.clone());
        state.phase = GamePhase::Playing;

        let mut human = Player::new_human(1, "Boudica".into(), [1.0; 3], &config);
        human.gold = 100.0;
        human.troops = 0.0;
        human.max_troops = 1000.0;
        human.border_tiles.insert(6); // (1, 1), adjacent to (0, 1)
        state.register_player(human);

        let mut ally = Player::new_human(2, "Snettisham".into(), [0.5; 3], &config);
        ally.player_type = PlayerType::Bot;
        ally.gold = 500.0;
        ally.troops = 500.0;
        state.register_player(ally);

        let mut roman = Player::new_human(3, milestone.into(), [0.0; 3], &config);
        roman.alive = false;
        state.register_player(roman);
        state.map.set_owner_id(0, 1, 2);

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine
            .campaign_relations
            .insert(2, crate::protocol::CampaignRelation::Neutral);
        engine.campaign_support_intervals.insert(2, 8);
        engine.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: GameplayIntent::ResolveCampaignDiplomacy {
                target_player: 2,
                relation: crate::protocol::CampaignRelation::Allied,
                gold_cost: 0.0,
            },
        }]);

        engine.state.tick = 1;
        engine.apply_campaign_unlocks_and_support();
        assert_eq!(engine.state.player(1).unwrap().gold, 350.0);
        assert_eq!(engine.state.player(1).unwrap().troops, 250.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 250.0);
        assert_eq!(engine.state.player(2).unwrap().troops, 250.0);
        assert!(engine.state.events.iter().any(|event| matches!(
            event,
            GameEvent::ResourceTransferred {
                sender_id: 2,
                receiver_id: 1,
                gold: 250.0,
                troops: 250.0
            }
        )));

        engine.state.tick = 2;
        engine.apply_campaign_unlocks_and_support();
        assert_eq!(engine.state.player(1).unwrap().gold, 350.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 250.0);
    }

    #[test]
    fn campaign_support_requires_each_allied_faction_to_be_contacted_directly() {
        let config = GameConfig {
            tick_rate_ms: 1000.0,
            campaign_support: Some(CampaignSupport {
                after_defeated: "Rome".into(),
                share_percent: 50,
            }),
            ..GameConfig::default()
        };
        let mut state = GameState::new(2, 6, 6, config.clone());
        state.phase = GamePhase::Playing;

        let mut human = Player::new_human(1, "Boudica".into(), [1.0; 3], &config);
        human.gold = 0.0;
        human.troops = 0.0;
        human.max_troops = 1000.0;
        human.alliances = vec![2, 3];
        human.border_tiles.insert(7); // (1, 1), beside faction 2 at (0, 1)
        state.register_player(human);

        for id in [2, 3] {
            let mut ally = Player::new_human(id, format!("Tribe {id}"), [0.5; 3], &config);
            ally.player_type = PlayerType::Bot;
            ally.gold = 200.0;
            ally.troops = 200.0;
            ally.alliances.push(1);
            state.register_player(ally);
        }

        let mut rome = Player::new_human(4, "Rome".into(), [0.0; 3], &config);
        rome.alive = false;
        state.register_player(rome);

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.state.map.set_owner_id(0, 1, 2);
        engine
            .campaign_relations
            .insert(2, crate::protocol::CampaignRelation::Allied);
        engine
            .campaign_relations
            .insert(3, crate::protocol::CampaignRelation::Allied);
        engine
            .campaign_alliance_groups
            .insert(2, "trinovantes".into());
        engine
            .campaign_alliance_groups
            .insert(3, "trinovantes".into());
        engine.campaign_support_intervals.insert(2, 5);
        engine.campaign_support_intervals.insert(3, 5);
        engine.state.tick = 1;
        engine.apply_campaign_unlocks_and_support();
        engine.state.tick = 6;
        engine.apply_campaign_unlocks_and_support();

        assert_eq!(engine.state.player(1).unwrap().gold, 100.0);
        assert_eq!(engine.state.player(1).unwrap().troops, 100.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 100.0);
        assert_eq!(engine.state.player(3).unwrap().gold, 200.0);
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::ResourceTransferred { receiver_id: 1, .. }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn campaign_pact_forms_for_every_member_and_breaks_as_a_bloc() {
        let config = GameConfig::default();
        let mut state = GameState::new(3, 6, 6, config.clone());
        state.phase = GamePhase::Playing;
        state.register_player(Player::new_human(1, "Boudica".into(), [1.0; 3], &config));
        for id in [2, 3] {
            let mut ally = Player::new_human(id, format!("Tribe {id}"), [0.5; 3], &config);
            ally.player_type = PlayerType::Bot;
            state.register_player(ally);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine
            .campaign_alliance_groups
            .insert(2, "trinovantes".into());
        engine
            .campaign_alliance_groups
            .insert(3, "trinovantes".into());
        engine.push_alliance_proposal(1, 2);
        engine.apply_intents(&[StampedIntent {
            player_id: 2,
            intent: GameplayIntent::AcceptAlliance { target_player: 1 },
        }]);
        assert!(engine.state.player(1).unwrap().alliances.contains(&2));
        assert!(engine.state.player(1).unwrap().alliances.contains(&3));
        assert!(engine.state.player(2).unwrap().alliances.contains(&1));
        assert!(engine.state.player(3).unwrap().alliances.contains(&1));

        engine.campaign_support_next_tick.insert(2, 100);
        engine.campaign_support_next_tick.insert(3, 100);
        engine.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: GameplayIntent::BreakAlliance { target_player: 2 },
        }]);
        assert!(engine.state.player(1).unwrap().alliances.is_empty());
        assert!(engine.state.player(2).unwrap().alliances.is_empty());
        assert!(engine.state.player(3).unwrap().alliances.is_empty());
        assert!(!engine.campaign_support_next_tick.contains_key(&2));
        assert!(!engine.campaign_support_next_tick.contains_key(&3));
    }

    #[test]
    fn tutorial_skips_automatic_victory_without_changing_normal_games() {
        for game_mode in ["FFA", "Teams", "HumansVsNations"] {
            // Map-control victory with an opponent, then last player/team below the threshold.
            for (red_tiles, blue_tiles) in [(70, 10), (10, 0)] {
                for tutorial in [false, true] {
                    let config = GameConfig {
                        game_mode: game_mode.into(),
                        tutorial,
                        random_spawn: true,
                        map_control_win_percentage: 0.60,
                        ..Default::default()
                    };
                    let mut state = GameState::new(42, 10, 10, config);
                    state.total_land_tiles = 100;
                    for (id, team, tiles) in
                        [(1, Team::Red, red_tiles), (2, Team::Blue, blue_tiles)]
                    {
                        let mut player =
                            Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
                        player.team = Some(team);
                        player.tile_count = tiles;
                        player.alive = tiles > 0;
                        player.has_spawned = true;
                        state.register_player(player);
                    }
                    let mut engine = SowEngine::new(state, WaterComponents::default());
                    engine.check_winner();

                    assert_eq!(
                        engine.state.phase,
                        if tutorial {
                            GamePhase::Playing
                        } else {
                            GamePhase::GameOver
                        },
                        "{game_mode}, tutorial={tutorial}, tiles={red_tiles}/{blue_tiles}"
                    );
                    assert_eq!(engine.state.winner, (!tutorial).then_some(1));
                    assert_eq!(
                        engine.state.winning_team,
                        (!tutorial && game_mode != "FFA").then_some(Team::Red)
                    );
                    assert_eq!(
                        engine
                            .state
                            .events
                            .iter()
                            .any(|event| matches!(event, GameEvent::GameOver { .. })),
                        !tutorial
                    );
                }
            }
        }
    }
}
