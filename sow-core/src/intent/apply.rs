use crate::diplomacy::{
    ALLIANCE_DURATION_TICKS, ALLIANCE_RENEWAL_WINDOW_TICKS, BOT_BETRAYAL_EMOJI_TICKS,
    HUMAN_BETRAYAL_EMOJI_TICKS, TRAITOR_STATUS_TICKS,
};
use crate::engine::SowEngine;
use crate::player::PlayerType;
use crate::protocol::{GameplayIntent, StampedIntent};

impl SowEngine {
    fn campaign_alliance_members(&self, player_id: u16) -> Vec<u16> {
        let Some(group) = self.campaign_alliance_groups.get(&player_id) else {
            return vec![player_id];
        };
        self.campaign_alliance_groups
            .iter()
            .filter_map(|(&member_id, member_group)| (member_group == group).then_some(member_id))
            .collect()
    }

    fn form_campaign_alliance(&mut self, proposer: u16, target: u16) {
        if self.campaign_relations.get(&proposer)
            == Some(&crate::protocol::CampaignRelation::Enemy)
            || self.campaign_relations.get(&target)
                == Some(&crate::protocol::CampaignRelation::Enemy)
        {
            return;
        }
        let proposers = self.campaign_alliance_members(proposer);
        let targets = self.campaign_alliance_members(target);
        for proposer_id in proposers {
            for &target_id in &targets {
                if proposer_id == target_id
                    || !self
                        .state
                        .player(proposer_id)
                        .is_some_and(|player| player.alive)
                    || !self
                        .state
                        .player(target_id)
                        .is_some_and(|player| player.alive)
                {
                    continue;
                }
                let campaign_pair = self.campaign_relations.contains_key(&proposer_id)
                    || self.campaign_relations.contains_key(&target_id);
                if let Some(player) = self.state.player_mut(proposer_id) {
                    if !player.alliances.contains(&target_id) {
                        player.alliances.push(target_id);
                    }
                    if campaign_pair {
                        player.alliance_timers.remove(&target_id);
                    } else {
                        player.alliance_timers.insert(target_id, ALLIANCE_DURATION_TICKS);
                    }
                }
                if let Some(player) = self.state.player_mut(target_id) {
                    if !player.alliances.contains(&proposer_id) {
                        player.alliances.push(proposer_id);
                    }
                    if campaign_pair {
                        player.alliance_timers.remove(&proposer_id);
                    } else {
                        player.alliance_timers.insert(proposer_id, ALLIANCE_DURATION_TICKS);
                    }
                }
                if campaign_pair {
                    if self.state.player(proposer_id).is_some_and(|player| player.is_human())
                        && self.campaign_relations.contains_key(&target_id)
                    {
                        self.campaign_relations.insert(
                            target_id,
                            crate::protocol::CampaignRelation::Allied,
                        );
                    }
                    if self.state.player(target_id).is_some_and(|player| player.is_human())
                        && self.campaign_relations.contains_key(&proposer_id)
                    {
                        self.campaign_relations.insert(
                            proposer_id,
                            crate::protocol::CampaignRelation::Allied,
                        );
                    }
                    if self.campaign_relations.contains_key(&proposer_id) {
                        self.campaign_alliance_started_tick
                            .insert(proposer_id, self.current_tick_u32());
                    }
                    if self.campaign_relations.contains_key(&target_id) {
                        self.campaign_alliance_started_tick
                            .insert(target_id, self.current_tick_u32());
                    }
                }
                self.retreat_mutual_aggression(proposer_id, target_id);
            }
        }
    }

    fn break_campaign_alliance(&mut self, breaker: u16, target: u16) {
        let breakers = self.campaign_alliance_members(breaker);
        let targets = self.campaign_alliance_members(target);
        for &breaker_id in &breakers {
            for &target_id in &targets {
                if breaker_id == target_id {
                    continue;
                }
                if let Some(player) = self.state.player_mut(breaker_id) {
                    player.alliances.retain(|&id| id != target_id);
                    player.alliance_timers.remove(&target_id);
                }
                if let Some(player) = self.state.player_mut(target_id) {
                    player.alliances.retain(|&id| id != breaker_id);
                    player.alliance_timers.remove(&breaker_id);
                }
            }
        }
        for id in breakers.into_iter().chain(targets) {
            self.campaign_support_next_tick.remove(&id);
            self.campaign_alliance_started_tick.remove(&id);
        }
    }

    fn campaign_players_touch(&self, human_id: u16, target_id: u16) -> bool {
        let Some(human) = self.state.player(human_id) else {
            return false;
        };
        let mut touching = false;
        for tile in human.border_tiles.ones() {
            let x = tile % self.state.map.width;
            let y = tile / self.state.map.width;
            self.state.map.for_each_neighbor(x, y, |nx, ny| {
                if self.state.map.owner_id(nx, ny) == target_id
                    && self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land()
                {
                    touching = true;
                }
            });
            if touching {
                break;
            }
        }
        touching
    }

    fn resolve_campaign_diplomacy(
        &mut self,
        human_id: u16,
        target_id: u16,
        relation: crate::protocol::CampaignRelation,
        gold_cost: f64,
    ) {
        if !self.campaign_relations.contains_key(&target_id)
            || self.campaign_contact_resolved.contains(&target_id)
            || !gold_cost.is_finite()
            || gold_cost < 0.0
            || gold_cost > 1_000_000.0
            || (relation != crate::protocol::CampaignRelation::Allied && gold_cost != 0.0)
            || self.campaign_relations.get(&target_id)
                != Some(&crate::protocol::CampaignRelation::Neutral)
            || !self.campaign_players_touch(human_id, target_id)
        {
            return;
        }
        let Some(human) = self.state.player(human_id) else {
            return;
        };
        if human.player_type != PlayerType::Human || !human.alive || human.gold < gold_cost {
            return;
        }
        if !self.state.player(target_id).is_some_and(|target| target.alive) {
            return;
        }

        if gold_cost > 0.0 {
            if let Some(human) = self.state.player_mut(human_id) {
                human.gold -= gold_cost;
            }
            if let Some(target) = self.state.player_mut(target_id) {
                target.gold += gold_cost;
            }
            self.state
                .events
                .push(crate::game::GameEvent::ResourceTransferred {
                    sender_id: human_id,
                    receiver_id: target_id,
                    gold: gold_cost,
                    troops: 0.0,
                });
        }

        let allied = relation == crate::protocol::CampaignRelation::Allied;
        if let Some(human) = self.state.player_mut(human_id) {
            human.alliances.retain(|&id| id != target_id);
            human.alliance_timers.remove(&target_id);
            if allied {
                human.alliances.push(target_id);
            }
        }
        if let Some(target) = self.state.player_mut(target_id) {
            target.alliances.retain(|&id| id != human_id);
            target.alliance_timers.remove(&human_id);
            if allied {
                target.alliances.push(human_id);
            }
        }
        if allied {
            self.campaign_alliance_started_tick
                .insert(target_id, self.current_tick_u32());
            self.retreat_mutual_aggression(human_id, target_id);
        } else {
            self.campaign_support_next_tick.remove(&target_id);
            self.campaign_alliance_started_tick.remove(&target_id);
        }
        self.campaign_relations.insert(target_id, relation);
        self.campaign_contact_resolved.insert(target_id);
    }

    pub fn apply_intents(&mut self, intents: &[StampedIntent]) {
        for (i, stamped) in intents.iter().enumerate() {
            self.apply_stamped_intent(stamped, i as u32);
        }
    }

    pub(crate) fn apply_stamped_intent_with_fleet_route(
        &mut self,
        stamped: &StampedIntent,
        route: crate::warp_fleet::FleetRoute,
        intent_index: u32,
    ) {
        match &stamped.intent {
            GameplayIntent::LaunchFleet {
                target_tile,
                troops,
            } => self.apply_launch_fleet_stamped(
                stamped.player_id,
                *target_tile,
                *troops,
                Some(route),
            ),
            _ => self.apply_stamped_intent(stamped, intent_index),
        }
    }

    pub fn retreat_mutual_aggression(&mut self, p1: u16, p2: u16) {
        for ex in &mut self.attacks {
            if (ex.owner_id == p1 && ex.target_owner == p2)
                || (ex.owner_id == p2 && ex.target_owner == p1)
            {
                ex.retreating = true;
            }
        }
        for wf in &mut self.fleets {
            if (wf.owner_id == p1 && wf.target_owner == p2)
                || (wf.owner_id == p2 && wf.target_owner == p1)
            {
                wf.retreating = true;
                wf.retreat_dst = None;
                wf.replace_path(Vec::new());
            }
        }
    }

    pub fn apply_stamped_intent(&mut self, stamped: &StampedIntent, intent_index: u32) {
        match &stamped.intent {
            GameplayIntent::RecallFleet { fleet_id } => {
                let pid = stamped.player_id;
                for wf in &mut self.fleets {
                    if wf.id != *fleet_id {
                        continue;
                    }
                    if wf.owner_id != pid {
                        continue;
                    }
                    wf.retreating = true;
                    wf.retreat_dst = None;
                    wf.replace_path(Vec::new());
                    break;
                }
            }
            GameplayIntent::LaunchFleet {
                target_tile,
                troops,
            } => self.apply_launch_fleet_stamped(stamped.player_id, *target_tile, *troops, None),
            GameplayIntent::CancelAttack { attack_id } => {
                let pid = stamped.player_id;
                for ex in &mut self.attacks {
                    if ex.id == *attack_id && ex.owner_id == pid {
                        ex.retreating = true;
                        return;
                    }
                }
                log::debug!(
                    "apply_stamped_intent: cancel attack_id={} for player {} — not found or not owner",
                    attack_id,
                    pid
                );
            }
            GameplayIntent::Attack(attack) => {
                let owner = attack.target_owner;
                let is_betrayer = self
                    .state
                    .player(owner)
                    .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                    .unwrap_or(false);
                let is_allied_in_list = self
                    .state
                    .player(stamped.player_id)
                    .map(|p| p.alliances.contains(&owner))
                    .unwrap_or(false);

                if is_allied_in_list && is_betrayer {
                    // Silently break the alliance without any penalty for the attacker
                    let attacker = stamped.player_id;
                    if let Some(p1) = self.state.player_mut(attacker) {
                        p1.alliances.retain(|&id| id != owner);
                        p1.alliance_timers.remove(&owner);
                    }
                    if let Some(p2) = self.state.player_mut(owner) {
                        p2.alliances.retain(|&id| id != attacker);
                        p2.alliance_timers.remove(&attacker);
                    }
                }

                let is_allied = self
                    .state
                    .player(stamped.player_id)
                    .map(|p| p.alliances.contains(&owner))
                    .unwrap_or(false);
                if !is_allied {
                    self.apply_attack_intent(stamped.player_id, attack, intent_index);
                }
            }
            GameplayIntent::BuildStructure { kind, target_tile } => {
                self.apply_build_structure_intent(stamped.player_id, *kind, *target_tile);
            }
            GameplayIntent::UpgradeStructure { building_id } => {
                self.apply_upgrade_structure_intent(stamped.player_id, *building_id);
            }
            GameplayIntent::UpgradeCityModule {
                building_id,
                module,
            } => {
                self.apply_upgrade_city_module_intent(stamped.player_id, *building_id, *module);
            }
            GameplayIntent::UpgradeTile { tile_idx } => {
                self.apply_upgrade_tile_intent(stamped.player_id, *tile_idx);
            }
            GameplayIntent::BuildShip { port_tile, kind } => {
                let pid = stamped.player_id;
                let cost = kind.gold_cost();
                let required_city_level = match kind {
                    crate::game::UnitType::TradeShip => 4,
                    crate::game::UnitType::Warship => 5,
                    crate::game::UnitType::TransportShip => 1,
                };
                let required_port_level = match kind {
                    crate::game::UnitType::TradeShip => 3,
                    crate::game::UnitType::Warship => 4,
                    crate::game::UnitType::TransportShip => 1,
                };
                let has_city_requirement = self.buildings.iter().any(|b| {
                    b.owner_id == pid
                        && b.kind == crate::game::BuildingKind::City
                        && b.active_level() >= required_city_level
                });
                let port_id = self
                    .buildings
                    .iter()
                    .find(|b| {
                        b.tile_idx == *port_tile
                            && b.owner_id == pid
                            && !b.under_construction
                            && b.kind == crate::game::BuildingKind::Port
                            && b.active_level() >= required_port_level
                    })
                    .map(|b| b.id);

                let boat_capacity = crate::building::player_fleet_capacity(&self.buildings, pid);
                if has_city_requirement
                    && self.boat_slots_used(pid) < boat_capacity
                    && let Some(port_id) = port_id
                    && let Some(player) = self.state.player_mut(pid)
                    && player.gold >= cost
                {
                    player.gold -= cost;
                    let queue = self.port_queues.entry(port_id).or_default();
                    queue.push_back(crate::game::ShipProduction {
                        kind: *kind,
                        ticks_until_complete: kind.build_duration_ticks(),
                    });
                }
            }
            GameplayIntent::MoveWarships {
                unit_ids,
                target_tile,
            } => {
                let pid = stamped.player_id;
                let target = *target_tile;
                let w = self.state.map.width;
                let area = w.saturating_mul(self.state.map.height);
                if target >= area {
                    return;
                }
                for uid in unit_ids {
                    if let Some(fleet) = self.fleets.iter_mut().find(|f| {
                        f.id == *uid
                            && f.owner_id == pid
                            && f.unit_type == crate::game::UnitType::Warship
                    }) {
                        // Try sea lane routing first (Dijkstra on ~20 port nodes)
                        let lane_path = if !self.state.sea_lanes.is_empty() {
                            let src_comp = self.water.component_of(fleet.current_tile);
                            let dst_comp = self.water.component_of(target);
                            if src_comp > 0 && src_comp == dst_comp {
                                let src_port = crate::sea_lane::closest_port_on_component(
                                    &self.buildings,
                                    &self.state.map,
                                    &self.water,
                                    fleet.current_tile,
                                    src_comp,
                                );
                                let dst_port = crate::sea_lane::closest_port_on_component(
                                    &self.buildings,
                                    &self.state.map,
                                    &self.water,
                                    target,
                                    dst_comp,
                                );
                                match (src_port, dst_port) {
                                    (Some(sp), Some(dp)) => crate::sea_lane::route_through_lanes(
                                        &self.state.sea_lanes,
                                        sp,
                                        dp,
                                    ),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let path = lane_path.or_else(|| {
                            self.path_scratch.astar.find_path(
                                &self.state.map,
                                &[fleet.current_tile],
                                target,
                            )
                        });

                        if let Some(path) = path {
                            fleet.replace_path(path);
                            fleet.retreating = false;
                        }
                    }
                }
            }
            GameplayIntent::LaunchNuke { target_tile, .. } => {
                self.apply_launch_nuke_intent(stamped.player_id, *target_tile);
            }
            // INK TIDE lockstep input: never reaches the SoW engine — the
            // racer sim runs on the racer clients. Opaque to SoW.
            GameplayIntent::RacerControls(_) => {}
            GameplayIntent::ResolveCampaignDiplomacy {
                target_player,
                relation,
                gold_cost,
            } => self.resolve_campaign_diplomacy(
                stamped.player_id,
                *target_player,
                *relation,
                *gold_cost,
            ),
            GameplayIntent::Spawn { x, y } => {
                if let crate::game::GamePhase::Spawning { .. } = self.state.phase {
                    let x = *x;
                    let y = *y;
                    let pid = stamped.player_id;

                    if self.state.map.is_valid_coord(x as i32, y as i32)
                        && self.state.map.terrain[self.state.map.ref_id(x, y)].is_land()
                        && self.state.map.owner_id(x, y) == 0
                    {
                        // Clear old tiles and buildings for this player
                        let w = self.state.map.width;
                        let mut to_clear = Vec::new();
                        for (i, &owner) in self.state.map.owner_states().iter().enumerate() {
                            if owner == pid {
                                to_clear.push(i as u32);
                            }
                        }
                        for i in to_clear {
                            self.state.set_tile_owner(i % w, i / w, 0);
                        }
                        let had_defense_post = self.buildings.iter().any(|b| {
                            b.owner_id == pid && b.kind == crate::game::BuildingKind::Bunker
                        });
                        self.buildings.retain(|b| b.owner_id != pid);
                        self.building_grid.mark_dirty();
                        if had_defense_post {
                            self.defense_grid_dirty = true;
                            self.render_defense_dirty = true;
                        }

                        // Set new spawn
                        self.state.place_spawn(pid, x, y);
                    }
                }
            }
            GameplayIntent::Resign => {
                self.kill_player(stamped.player_id);
            }
            GameplayIntent::MarkDisconnected { is_disconnected } => {
                if let Some(player) = self.state.player_mut(stamped.player_id) {
                    player.disconnected = *is_disconnected;
                }
            }
            GameplayIntent::ExpressEmoji { emoji, pinned } => {
                let max_emoji_ticks =
                    (3.0 * 1000.0 / self.state.config.tick_rate_ms).round() as u32;
                if let Some(player) = self.state.player_mut(stamped.player_id) {
                    player.active_emoji = Some(emoji.clone());
                    player.emoji_pinned = *pinned;
                    player.emoji_timer = if *pinned { 0 } else { max_emoji_ticks };
                }
            }
            GameplayIntent::ProposeAlliance { target_player } => {
                let proposer = stamped.player_id;
                let target = *target_player;
                if proposer != target
                    && self.campaign_relations.get(&proposer)
                        != Some(&crate::protocol::CampaignRelation::Enemy)
                    && self.campaign_relations.get(&target)
                        != Some(&crate::protocol::CampaignRelation::Enemy)
                {
                    let proposer_alive = self
                        .state
                        .player(proposer)
                        .map(|p| p.alive)
                        .unwrap_or(false);
                    let target_alive = self.state.player(target).map(|p| p.alive).unwrap_or(false);
                    if proposer_alive && target_alive {
                        let is_teammate = {
                            let p_prop = self.state.player(proposer).unwrap();
                            let p_target = self.state.player(target).unwrap();
                            p_prop.team.is_some() && p_prop.team == p_target.team
                        };
                        let (is_allied, can_renew) = self
                            .state
                            .player(proposer)
                            .map(|p| {
                                let allied = p.alliances.contains(&target);
                                let timer = p.alliance_timers.get(&target).copied().unwrap_or(0);
                                (allied, allied && timer <= ALLIANCE_RENEWAL_WINDOW_TICKS)
                            })
                            .unwrap_or((false, false));

                        if is_teammate {
                            log::warn!(
                                "ABERRATION: Player {} tried to propose alliance to teammate/ally {} in team game",
                                proposer,
                                target
                            );
                            return;
                        }

                        if is_allied && !can_renew {
                            // Benign race: another bot's AcceptAlliance in the same
                            // tick batch already created this alliance before our
                            // ProposeAlliance was applied. Silently skip.
                            return;
                        }

                        if !is_allied || can_renew {
                            if self.has_alliance_proposal(target, proposer) {
                                // Mutual request! Accept/Renew it immediately.
                                let idx = self
                                    .alliances_proposed
                                    .iter()
                                    .position(|p| p.proposer == target && p.target == proposer)
                                    .unwrap();
                                self.alliances_proposed.remove(idx);
                                self.form_campaign_alliance(proposer, target);
                            } else if self.can_send_alliance_request(proposer, target) {
                                self.push_alliance_proposal(proposer, target);
                            }
                        }
                    }
                }
            }
            GameplayIntent::AcceptAlliance { target_player } => {
                let acceptor = stamped.player_id;
                let target = *target_player;
                if self.campaign_relations.get(&acceptor)
                    == Some(&crate::protocol::CampaignRelation::Enemy)
                    || self.campaign_relations.get(&target)
                        == Some(&crate::protocol::CampaignRelation::Enemy)
                {
                    return;
                }
                let prop_idx = self
                    .alliances_proposed
                    .iter()
                    .position(|p| p.proposer == target && p.target == acceptor);
                if let Some(idx) = prop_idx {
                    self.alliances_proposed.remove(idx);
                    if let Some(rev_idx) = self
                        .alliances_proposed
                        .iter()
                        .position(|p| p.proposer == acceptor && p.target == target)
                    {
                        self.alliances_proposed.remove(rev_idx);
                    }
                    self.form_campaign_alliance(acceptor, target);
                }
            }
            GameplayIntent::RejectAlliance { target_player } => {
                let rejector = stamped.player_id;
                let target = *target_player;
                let prop_idx = self
                    .alliances_proposed
                    .iter()
                    .position(|p| p.proposer == target && p.target == rejector);
                if let Some(idx) = prop_idx {
                    self.alliances_proposed.remove(idx);
                    self.mark_alliance_request_cooldown(target, rejector);
                }
            }
            GameplayIntent::BreakAlliance { target_player } => {
                let breaker = stamped.player_id;
                let target = *target_player;
                let emoji_ticks = self
                    .state
                    .player(breaker)
                    .map(|p| {
                        if p.player_type == PlayerType::Human {
                            HUMAN_BETRAYAL_EMOJI_TICKS
                        } else {
                            BOT_BETRAYAL_EMOJI_TICKS
                        }
                    })
                    .unwrap_or(BOT_BETRAYAL_EMOJI_TICKS);
                let traitor_until = self.current_tick_u32().saturating_add(TRAITOR_STATUS_TICKS);
                if let Some(p1) = self.state.player_mut(breaker) {
                    p1.traitor = true;
                    p1.traitor_tick = traitor_until;
                    p1.active_emoji = Some("🗡️".to_string());
                    p1.emoji_timer = emoji_ticks;
                }
                self.mark_betrayal_cooldown(breaker);
                self.break_campaign_alliance(breaker, target);
                let human_id = self
                    .state
                    .players
                    .iter()
                    .find(|player| player.player_type == PlayerType::Human)
                    .map(|player| player.id);
                if human_id == Some(target) && self.campaign_relations.contains_key(&breaker) {
                    self.campaign_relations
                        .insert(breaker, crate::protocol::CampaignRelation::Enemy);
                    if self.campaign_betrayal.get(&breaker)
                        == Some(&crate::game_config::CampaignBetrayal::Opportunistic)
                    {
                        self.campaign_hostilities
                            .insert(breaker, crate::game_config::CampaignHostility::Aggressive);
                    }
                } else if human_id == Some(breaker)
                    && self.campaign_relations.contains_key(&target)
                {
                    self.campaign_relations
                        .insert(target, crate::protocol::CampaignRelation::Enemy);
                }
            }
            GameplayIntent::SendResources {
                target_player,
                gold,
                troops,
            } => {
                let sender = stamped.player_id;
                let target = *target_player;
                let g = *gold;
                let t = *troops;
                let is_allied = self
                    .state
                    .player(sender)
                    .map(|p| {
                        p.alliances.contains(&target)
                            || (p.team.is_some()
                                && p.team == self.state.player(target).and_then(|t| t.team))
                    })
                    .unwrap_or(false);
                if sender != target
                    && is_allied
                    && (g > 0.0 || t > 0.0)
                    && !g.is_nan()
                    && !t.is_nan()
                {
                    let mut actual_g = 0.0;
                    let mut actual_t = 0.0;
                    let mut sender_ok = false;
                    if let Some(s_player) = self.state.player_mut(sender)
                        && s_player.alive
                    {
                        actual_g = if g > 0.0 { g.min(s_player.gold) } else { 0.0 };
                        let max_t_to_send = (s_player.troops - 1.0).max(0.0);
                        actual_t = if t > 0.0 { t.min(max_t_to_send) } else { 0.0 };
                        s_player.gold -= actual_g;
                        s_player.troops -= actual_t;
                        sender_ok = true;
                    }
                    if sender_ok && (actual_g > 0.0 || actual_t > 0.0) {
                        if let Some(t_player) = self.state.player_mut(target)
                            && t_player.alive
                        {
                            t_player.gold += actual_g;
                            t_player.troops = (t_player.troops + actual_t).min(t_player.max_troops);
                        }
                        self.state
                            .events
                            .push(crate::game::GameEvent::ResourceTransferred {
                                sender_id: sender,
                                receiver_id: target,
                                gold: actual_g,
                                troops: actual_t,
                            });
                    }
                }
            }
            GameplayIntent::RequestResources {
                target_player,
                gold,
                troops,
            } => {
                let proposer = stamped.player_id;
                let target = *target_player;
                let g = *gold;
                let t = *troops;
                if proposer != target && (g > 0.0 || t > 0.0) && !g.is_nan() && !t.is_nan() {
                    let proposer_alive = self
                        .state
                        .player(proposer)
                        .map(|p| p.alive)
                        .unwrap_or(false);
                    let target_alive = self.state.player(target).map(|p| p.alive).unwrap_or(false);
                    if proposer_alive && target_alive {
                        // Clear any existing request between these two
                        self.resource_requests_proposed
                            .retain(|r| !(r.proposer == proposer && r.target == target));
                        self.resource_requests_proposed.push(
                            crate::engine::ResourceRequestProposed {
                                proposer,
                                target,
                                gold: g,
                                troops: t,
                            },
                        );
                    }
                }
            }
            GameplayIntent::AcceptResourceRequest { target_player } => {
                let acceptor = stamped.player_id;
                let target = *target_player; // target here is the proposer
                if let Some(pos) = self
                    .resource_requests_proposed
                    .iter()
                    .position(|r| r.proposer == target && r.target == acceptor)
                {
                    let req = self.resource_requests_proposed.remove(pos);
                    let mut actual_g = 0.0;
                    let mut actual_t = 0.0;
                    let mut acceptor_ok = false;
                    // Acceptor pays the resources
                    if let Some(acc_player) = self.state.player_mut(acceptor)
                        && acc_player.alive
                    {
                        actual_g = if req.gold > 0.0 {
                            req.gold.min(acc_player.gold)
                        } else {
                            0.0
                        };
                        let max_t_to_send = (acc_player.troops - 1.0).max(0.0);
                        actual_t = if req.troops > 0.0 {
                            req.troops.min(max_t_to_send)
                        } else {
                            0.0
                        };
                        acc_player.gold -= actual_g;
                        acc_player.troops -= actual_t;
                        acceptor_ok = true;
                    }
                    // Proposer receives the resources
                    if acceptor_ok
                        && (actual_g > 0.0 || actual_t > 0.0)
                        && let Some(prop_player) = self.state.player_mut(target)
                        && prop_player.alive
                    {
                        prop_player.gold += actual_g;
                        prop_player.troops =
                            (prop_player.troops + actual_t).min(prop_player.max_troops);
                    }
                }
            }
            GameplayIntent::RejectResourceRequest { target_player } => {
                let rejector = stamped.player_id;
                let target = *target_player; // target here is the proposer
                if let Some(pos) = self
                    .resource_requests_proposed
                    .iter()
                    .position(|r| r.proposer == target && r.target == rejector)
                {
                    self.resource_requests_proposed.remove(pos);
                    self.state
                        .events
                        .push(crate::game::GameEvent::ResourceRequestRejected {
                            rejector_id: rejector,
                            requester_id: target,
                        });
                }
            }
        }
    }

    fn apply_launch_fleet_stamped(
        &mut self,
        player_id: u16,
        target_tile: u32,
        troops: Option<f64>,
        route: Option<crate::warp_fleet::FleetRoute>,
    ) {
        let owner = self.state.map.owner_states()[target_tile as usize];
        let is_betrayer = self
            .state
            .player(owner)
            .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
            .unwrap_or(false);
        let is_allied_in_list = self
            .state
            .player(player_id)
            .map(|p| p.alliances.contains(&owner))
            .unwrap_or(false);

        if is_allied_in_list && is_betrayer {
            // Silently break the alliance without any penalty for the attacker
            if let Some(p1) = self.state.player_mut(player_id) {
                p1.alliances.retain(|&id| id != owner);
                p1.alliance_timers.remove(&owner);
            }
            if let Some(p2) = self.state.player_mut(owner) {
                p2.alliances.retain(|&id| id != player_id);
                p2.alliance_timers.remove(&player_id);
            }
        }

        let is_allied = self
            .state
            .player(player_id)
            .map(|p| p.alliances.contains(&owner))
            .unwrap_or(false);
        if is_allied {
            return;
        }
        if let Some(route) = route {
            self.apply_launch_fleet_intent_with_route(player_id, target_tile, troops, route);
        } else {
            self.apply_launch_fleet_intent(player_id, target_tile, troops);
        }
    }
}
