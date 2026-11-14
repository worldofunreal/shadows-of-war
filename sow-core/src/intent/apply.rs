use crate::diplomacy::{
    ALLIANCE_DURATION_TICKS, BOT_BETRAYAL_RETALIATION_TICKS, HUMAN_BETRAYAL_RETALIATION_TICKS,
    TRAITOR_STATUS_TICKS, alliance_can_renew,
};
use crate::engine::SowEngine;
use crate::execution::hero_effects::allied;
use crate::geo_entities::Source;
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
        if self.campaign_relations.get(&proposer) == Some(&crate::protocol::CampaignRelation::Enemy)
            || self.campaign_relations.get(&target)
                == Some(&crate::protocol::CampaignRelation::Enemy)
        {
            return;
        }
        let proposers = self.campaign_alliance_members(proposer);
        let targets = self.campaign_alliance_members(target);
        let new_alliance = !allied(&self.state, proposer, target);
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
                for ally in [proposer_id, target_id] {
                    if self.state.player(ally).is_some_and(|player| {
                        player.leader.has_resource_bonus_from(Source::Alliance)
                    }) {
                        self.state.announce_leader_ability(ally, None);
                    }
                }
                if let Some(player) = self.state.player_mut(proposer_id) {
                    if !player.alliances.contains(&target_id) {
                        player.alliances.push(target_id);
                    }
                    if campaign_pair {
                        player.alliance_timers.remove(&target_id);
                    } else {
                        player
                            .alliance_timers
                            .insert(target_id, ALLIANCE_DURATION_TICKS);
                    }
                }
                if let Some(player) = self.state.player_mut(target_id) {
                    if !player.alliances.contains(&proposer_id) {
                        player.alliances.push(proposer_id);
                    }
                    if campaign_pair {
                        player.alliance_timers.remove(&proposer_id);
                    } else {
                        player
                            .alliance_timers
                            .insert(proposer_id, ALLIANCE_DURATION_TICKS);
                    }
                }
                if campaign_pair {
                    if self
                        .state
                        .player(proposer_id)
                        .is_some_and(|player| player.is_human())
                        && self.campaign_relations.contains_key(&target_id)
                    {
                        self.campaign_relations
                            .insert(target_id, crate::protocol::CampaignRelation::Allied);
                    }
                    if self
                        .state
                        .player(target_id)
                        .is_some_and(|player| player.is_human())
                        && self.campaign_relations.contains_key(&proposer_id)
                    {
                        self.campaign_relations
                            .insert(proposer_id, crate::protocol::CampaignRelation::Allied);
                    }
                }
                self.retreat_mutual_aggression(proposer_id, target_id);
            }
        }
        if new_alliance {
            self.hero_alliance_formed(proposer, target);
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
        }

        let human_id = self
            .state
            .players
            .iter()
            .find(|player| player.player_type == PlayerType::Human)
            .map(|player| player.id);
        if human_id == Some(target) && self.campaign_relations.contains_key(&breaker) {
            self.campaign_relations
                .insert(breaker, crate::protocol::CampaignRelation::Enemy);
        } else if human_id == Some(breaker) && self.campaign_relations.contains_key(&target) {
            self.campaign_relations
                .insert(target, crate::protocol::CampaignRelation::Enemy);
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
        // Hero ability: this human's hero can make campaign alliances with some targets cheaper.
        let alliance_discount = crate::execution::combat::targeted_multiplier(
            &self.state,
            human_id,
            target_id,
            0.0,
            |target| human.leader.alliance_cost_multiplier(target),
        );
        let gold_cost = gold_cost * alliance_discount;
        if human.player_type != PlayerType::Human || !human.alive || human.gold < gold_cost {
            return;
        }
        if !self
            .state
            .player(target_id)
            .is_some_and(|target| target.alive)
        {
            return;
        }

        if gold_cost > 0.0 {
            if alliance_discount != 1.0 {
                self.state.announce_leader_ability(human_id, None);
            }
            if let Some(human) = self.state.player_mut(human_id) {
                human.gold -= gold_cost;
            }
            if let Some(target) = self.state.player_mut(target_id) {
                target.credit_gold(gold_cost);
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

        let is_allied = relation == crate::protocol::CampaignRelation::Allied;
        let new_alliance = is_allied && !allied(&self.state, human_id, target_id);
        if let Some(human) = self.state.player_mut(human_id) {
            human.alliances.retain(|&id| id != target_id);
            human.alliance_timers.remove(&target_id);
            if is_allied {
                human.alliances.push(target_id);
            }
        }
        if let Some(target) = self.state.player_mut(target_id) {
            target.alliances.retain(|&id| id != human_id);
            target.alliance_timers.remove(&human_id);
            if is_allied {
                target.alliances.push(human_id);
            }
        }
        if is_allied {
            if self.state.config.campaign_support.is_some()
                && self.campaign_support_intervals.contains_key(&target_id)
            {
                self.campaign_support_next_tick
                    .insert(target_id, self.state.tick);
            }
            self.retreat_mutual_aggression(human_id, target_id);
        } else {
            self.campaign_support_next_tick.remove(&target_id);
        }
        self.campaign_relations.insert(target_id, relation);
        self.campaign_contact_resolved.insert(target_id);
        if new_alliance {
            self.hero_alliance_formed(human_id, target_id);
        }
    }

    pub fn apply_intents(&mut self, intents: &[StampedIntent]) {
        self.apply_intents_tracking_troop_spend(intents, None);
    }

    pub fn apply_intents_tracking_troop_spend(
        &mut self,
        intents: &[StampedIntent],
        player_id: Option<u16>,
    ) -> f64 {
        let mut troops_spent = 0.0;
        for (i, stamped) in intents.iter().enumerate() {
            let troops_before = player_id
                .filter(|id| *id == stamped.player_id)
                .and_then(|id| self.state.player(id).map(|player| player.troops));
            self.apply_stamped_intent(stamped, i as u32);
            if let Some(before) = troops_before
                && let Some(after) = self.state.player(stamped.player_id).map(|player| player.troops)
            {
                troops_spent += (before - after).max(0.0);
            }
        }
        troops_spent
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

    /// Allies and teammates may move gold and troops to each other. Send, Request and
    /// Accept all ask this one question, so a broken alliance also voids a pending request.
    pub(crate) fn resources_can_move(&self, payer: u16, receiver: u16) -> bool {
        let (Some(payer_player), Some(receiver_player)) =
            (self.state.player(payer), self.state.player(receiver))
        else {
            return false;
        };
        payer != receiver
            && payer_player.alive
            && receiver_player.alive
            && (payer_player.alliances.contains(&receiver)
                || (payer_player.team.is_some() && payer_player.team == receiver_player.team))
    }

    /// Gold the payer can give now: the requested amount, capped at what it holds.
    pub(crate) fn gold_transferable(&self, payer: u16, requested: f64) -> f64 {
        let held = self.state.player(payer).map_or(0.0, |p| p.gold.max(0.0));
        requested.max(0.0).min(held)
    }

    /// Troops the payer can give the receiver now. The payer keeps one troop and the
    /// receiver never goes past its max, so whatever does not fit stays with the payer.
    pub(crate) fn troops_transferable(&self, payer: u16, receiver: u16, requested: f64) -> f64 {
        requested.max(0.0).min(self.troops_spare(payer)).min(self.troop_room(receiver))
    }

    pub(crate) fn troops_spare(&self, payer: u16) -> f64 {
        self.state
            .player(payer)
            .map_or(0.0, |p| (p.troops - 1.0).max(0.0))
    }

    pub(crate) fn troop_room(&self, receiver: u16) -> f64 {
        self.state
            .player(receiver)
            .map_or(0.0, |p| (p.max_troops - p.troops).max(0.0))
    }

    /// Records a request, or a trade when it carries a price. A plain request is cut to
    /// what will actually move; a trade keeps its agreed amounts and settles on accept.
    fn propose_resources(
        &mut self,
        proposer: u16,
        target: u16,
        gold: f64,
        troops: f64,
        pay_gold: f64,
        pay_troops: f64,
    ) {
        let amounts = [gold, troops, pay_gold, pay_troops];
        if amounts.iter().any(|v| !v.is_finite() || *v < 0.0)
            || !self.resources_can_move(proposer, target)
            || !self.can_send_resource_request(proposer, target)
        {
            return;
        }
        let request = if pay_gold > 0.0 || pay_troops > 0.0 {
            // Each resource flows one way, and the proposer must hold its price now.
            let one_way = !(gold > 0.0 && pay_gold > 0.0) && !(troops > 0.0 && pay_troops > 0.0);
            let can_pay = self.gold_transferable(proposer, pay_gold) >= pay_gold
                && self.troops_transferable(proposer, target, pay_troops) >= pay_troops;
            if !one_way || !can_pay || (gold <= 0.0 && troops <= 0.0) {
                return;
            }
            crate::engine::ResourceRequestProposed {
                proposer,
                target,
                gold,
                troops,
                pay_gold,
                pay_troops,
                created_tick: self.current_tick_u32(),
            }
        } else {
            // Only what the target can pay now and the proposer can hold, so the
            // request shows the amount that will actually move.
            let g = self.gold_transferable(target, gold);
            let t = self.troops_transferable(target, proposer, troops);
            if g <= 0.0 && t <= 0.0 {
                return;
            }
            crate::engine::ResourceRequestProposed {
                proposer,
                target,
                gold: g,
                troops: t,
                pay_gold: 0.0,
                pay_troops: 0.0,
                created_tick: self.current_tick_u32(),
            }
        };
        // One pending request per pair; a new one replaces it.
        self.resource_requests_proposed
            .retain(|r| !(r.proposer == proposer && r.target == target));
        let is_trade = request.is_trade();
        self.resource_requests_proposed.push(request);
        self.mark_resource_request_cooldown(proposer, target);
        self.state
            .events
            .push(crate::game::GameEvent::ResourceRequestSubmitted {
                proposer_id: proposer,
                target_id: target,
                is_trade,
            });
    }

    /// Whether both sides of a trade can pay in full right now.
    pub(crate) fn trade_settles(&self, trade: &crate::engine::ResourceRequestProposed) -> bool {
        let (proposer, acceptor) = (trade.proposer, trade.target);
        self.resources_can_move(acceptor, proposer)
            && self.gold_transferable(acceptor, trade.gold) >= trade.gold
            && self.troops_transferable(acceptor, proposer, trade.troops) >= trade.troops
            && self.gold_transferable(proposer, trade.pay_gold) >= trade.pay_gold
            && self.troops_transferable(proposer, acceptor, trade.pay_troops) >= trade.pay_troops
    }

    /// Moves amounts already checked by the caller.
    pub(crate) fn move_resources(&mut self, payer: u16, receiver: u16, gold: f64, troops: f64) {
        if let Some(p) = self.state.player_mut(payer) {
            p.gold -= gold;
            p.troops -= troops;
        }
        if let Some(r) = self.state.player_mut(receiver) {
            r.credit_gold(gold);
            r.credit_troops(troops);
        }
    }

    pub fn apply_stamped_intent(&mut self, stamped: &StampedIntent, intent_index: u32) {
        if self.state.config.ranked_1v1 {
            let target = match &stamped.intent {
                GameplayIntent::ProposeAlliance { target_player }
                | GameplayIntent::AcceptAlliance { target_player }
                | GameplayIntent::SendResources { target_player, .. }
                | GameplayIntent::RequestResources { target_player, .. }
                | GameplayIntent::OfferTrade { target_player, .. }
                | GameplayIntent::AcceptResourceRequest { target_player } => Some(*target_player),
                _ => None,
            };
            if let Some(target) = target
                && self.state.player(stamped.player_id).is_some_and(|p| p.is_human())
                && self.state.player(target).is_some_and(|p| p.is_human())
            {
                return;
            }
        }
        match &stamped.intent {
            GameplayIntent::RecallFleet { fleet_id } => {
                if !self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet) {
                    return;
                }
                let pid = stamped.player_id;
                for wf in &mut self.fleets {
                    if wf.id != *fleet_id {
                        continue;
                    }
                    if wf.owner_id != pid
                        || wf.unit_type != crate::game::UnitType::TransportShip
                        || wf.retreating
                    {
                        continue;
                    }
                    wf.retreating = true;
                    wf.retreat_dst = None;
                    wf.replace_path(Vec::new());
                    break;
                }
            }
            GameplayIntent::ScuttleFleet { fleet_id } => {
                if !self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet) {
                    return;
                }
                let pid = stamped.player_id;
                if let Some(index) = self.fleets.iter().position(|fleet| fleet.id == *fleet_id) {
                    let can_scuttle = self.fleets.get(index).is_some_and(|fleet| {
                        fleet.owner_id == pid
                            && fleet.unit_type == crate::game::UnitType::TransportShip
                    });
                    if can_scuttle {
                        self.fleets.remove(index);
                    }
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
                    .map(crate::diplomacy::can_retaliate_against)
                    .unwrap_or(false);
                let is_allied_in_list = self
                    .state
                    .player(stamped.player_id)
                    .map(|p| p.alliances.contains(&owner))
                    .unwrap_or(false);

                if is_allied_in_list && is_betrayer {
                    // Silently break the alliance without any penalty for the attacker
                    let attacker = stamped.player_id;
                    self.break_campaign_alliance(attacker, owner);
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
            GameplayIntent::BuildShip { target_tile, kind } => {
                let pid = stamped.player_id;
                if *kind != crate::game::UnitType::Warship {
                    return;
                }
                if *kind == crate::game::UnitType::Warship
                    && !self.campaign_allows_action(crate::campaign::CampaignAction::Warship)
                {
                    self.state.events.push(crate::game::GameEvent::NavalActionRejected {
                        player_id: pid,
                        reason: crate::game::NavalActionFailure::CampaignLocked,
                    });
                    return;
                }
                let Some(player) = self.state.player(pid) else {
                    return;
                };
                if !player.alive {
                    return;
                }
                let route = match crate::warp_fleet::resolve_warship_route(
                    &self.state.map,
                    &self.water,
                    &mut self.path_scratch,
                    &self.buildings,
                    pid,
                    *target_tile,
                ) {
                    Ok(route) => route,
                    Err(error) => {
                        let reason = match error {
                            crate::warp_fleet::WarshipLaunchError::InvalidTile => crate::game::NavalActionFailure::InvalidTarget,
                            crate::warp_fleet::WarshipLaunchError::TargetNotWater => crate::game::NavalActionFailure::TargetNotWater,
                            crate::warp_fleet::WarshipLaunchError::NoPort => crate::game::NavalActionFailure::NoPort,
                            crate::warp_fleet::WarshipLaunchError::NoConnectedPort => crate::game::NavalActionFailure::NoConnectedPort,
                            crate::warp_fleet::WarshipLaunchError::NoWaterPath => crate::game::NavalActionFailure::NoWaterPath,
                        };
                        self.state.events.push(crate::game::GameEvent::NavalActionRejected {
                            player_id: pid,
                            reason,
                        });
                        return;
                    }
                };
                let capacity = crate::building::player_fleet_capacity(&self.buildings, pid);
                if self.boat_slots_used(pid) >= capacity {
                    self.state.events.push(crate::game::GameEvent::NavalActionRejected {
                        player_id: pid,
                        reason: crate::game::NavalActionFailure::CapacityFull,
                    });
                    return;
                }
                let owned_warships = self
                    .fleets
                    .iter()
                    .filter(|fleet| fleet.owner_id == pid && fleet.unit_type == *kind)
                    .count() as u32;
                let cost = kind.gold_cost_with_owned_count_for_leader(owned_warships, player.leader);
                if player.gold < cost {
                    self.state.events.push(crate::game::GameEvent::NavalActionRejected {
                        player_id: pid,
                        reason: crate::game::NavalActionFailure::InsufficientGold,
                    });
                    return;
                }
                if let Some(player) = self.state.player_mut(pid) {
                    player.gold -= cost;
                }
                let fleet_id = self.state.next_fleet_id;
                self.state.next_fleet_id = self.state.next_fleet_id.wrapping_add(1).max(1);
                let mut fleet = crate::warp_fleet::WarpFleet::new(
                    fleet_id,
                    pid,
                    0,
                    crate::game::UnitType::Warship,
                    crate::game::UnitType::Warship.max_health(),
                    (route.src_tile, route.landing_tile),
                    route.path,
                );
                if let Some(warship) = fleet.warship.as_mut() {
                    warship.patrol_tile = *target_tile;
                }
                fleet.set_speed_bonus_percent(crate::building::player_boat_speed_bonus(
                    &self.buildings,
                    pid,
                ));
                self.add_fleet(fleet);
                self.state.events.push(crate::game::GameEvent::WarshipSpawned {
                    owner_id: pid,
                    fleet_id,
                    tile: route.src_tile,
                });
                if matches!(
                    self.state.player(pid).map(|player| player.leader.ability()),
                    Some(crate::geo_entities::Ability::WarshipCost { .. })
                ) {
                    let width = self.state.map.width.max(1);
                    self.state.announce_leader_ability(
                        pid,
                        Some((route.src_tile % width, route.src_tile / width)),
                    );
                }
            }
            GameplayIntent::MoveWarships {
                unit_ids,
                target_tile,
            } => {
                if !self.campaign_allows_action(crate::campaign::CampaignAction::Warship) {
                    return;
                }
                let pid = stamped.player_id;
                let target = *target_tile;
                let w = self.state.map.width;
                let order_tick = self.state.tick;
                let area = w.saturating_mul(self.state.map.height);
                if target >= area
                    || !self.state.map.terrain[target as usize].is_water()
                    || self.water.component_of(target) == 0
                {
                    return;
                }
                for uid in unit_ids {
                    if let Some(fleet) = self.fleets.iter_mut().find(|f| {
                        f.id == *uid
                            && f.owner_id == pid
                            && f.unit_type == crate::game::UnitType::Warship
                    }) {
                        if self.water.component_of(fleet.current_tile)
                            == self.water.component_of(target)
                            && let Some(path) = self.path_scratch.astar.find_path(
                                &self.state.map,
                                &[fleet.current_tile],
                                target,
                            )
                        {
                            fleet.replace_path(path);
                            fleet.retreating = false;
                            if let Some(state) = fleet.warship.as_mut() {
                                state.mode = crate::warp_fleet::WarshipMode::Patrolling;
                                state.patrol_tile = target;
                                state.patrol_target = Some(target);
                                state.target_fleet = None;
                                state.retreat_port_id = None;
                                state.manual_order_tick = order_tick;
                            }
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
                            b.owner_id == pid && b.kind == crate::game::BuildingKind::Tower
                        });
                        self.buildings.retain(|b| b.owner_id != pid);
                        self.building_grid.mark_dirty();
                        self.building_aggregates_dirty = true;
                        self.sea_lanes_dirty = true;
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
            GameplayIntent::ExpressEmote { kind, pinned } => {
                let tick = self.state.tick;
                let phase = self.state.phase.clone();
                if let Some(player) = self.state.player_mut(stamped.player_id)
                    && crate::emote::can_express(player, &phase)
                {
                    crate::emote::express(player, Some(*kind), *pinned, false, tick);
                }
            }
            GameplayIntent::ClearEmote => {
                if let Some(player) = self.state.player_mut(stamped.player_id) {
                    player.active_emote = None;
                }
            }
            GameplayIntent::SetEmotePinned { pinned } => {
                if let Some(player) = self.state.player_mut(stamped.player_id)
                    && let Some(event) = player.active_emote.as_mut()
                {
                    event.pinned = *pinned;
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
                                let remaining_ticks = p.alliance_timers.get(&target).copied();
                                (allied, alliance_can_renew(allied, remaining_ticks))
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
                    self.record_alliance_request_lifecycle(
                        target,
                        rejector,
                        crate::game::AllianceRequestStatus::Rejected,
                    );
                    self.mark_alliance_request_cooldown(target, rejector);
                }
            }
            GameplayIntent::BreakAlliance { target_player } => {
                let breaker = stamped.player_id;
                let target = *target_player;
                let retaliation_ticks = self
                    .state
                    .player(breaker)
                    .map(|p| {
                        if p.player_type == PlayerType::Human {
                            HUMAN_BETRAYAL_RETALIATION_TICKS
                        } else {
                            BOT_BETRAYAL_RETALIATION_TICKS
                        }
                    })
                    .unwrap_or(BOT_BETRAYAL_RETALIATION_TICKS);
                let traitor_until = self.current_tick_u32().saturating_add(TRAITOR_STATUS_TICKS);
                if let Some(p1) = self.state.player_mut(breaker) {
                    p1.traitor = true;
                    p1.traitor_tick = traitor_until;
                    p1.betrayal_retaliation_ticks = retaliation_ticks;
                }
                self.mark_betrayal_cooldown(breaker);
                self.break_campaign_alliance(breaker, target);
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
                let valid_amounts = (g > 0.0 || t > 0.0) && g.is_finite() && t.is_finite();
                if valid_amounts && self.resources_can_move(sender, target) {
                    let actual_g = self.gold_transferable(sender, g);
                    let actual_t = self.troops_transferable(sender, target, t);
                    if let Some(s_player) = self.state.player_mut(sender) {
                        s_player.gold -= actual_g;
                        s_player.troops -= actual_t;
                    }
                    if actual_g > 0.0 || actual_t > 0.0 {
                        if let Some(t_player) = self.state.player_mut(target) {
                            t_player.credit_gold(actual_g);
                            t_player.credit_troops(actual_t);
                        }
                        self.state
                            .events
                            .push(crate::game::GameEvent::ResourceTransferred {
                                sender_id: sender,
                                receiver_id: target,
                                gold: actual_g,
                                troops: actual_t,
                            });
                        self.hero_shared_feast(sender, target, actual_t);
                    } else if self
                        .state
                        .player(sender)
                        .is_some_and(|player| player.is_human())
                    {
                        self.state
                            .events
                            .push(crate::game::GameEvent::ResourceTransferRejected {
                                sender_id: sender,
                                receiver_id: target,
                            });
                    }
                } else if self
                    .state
                    .player(sender)
                    .is_some_and(|player| player.is_human())
                {
                    self.state
                        .events
                        .push(crate::game::GameEvent::ResourceTransferRejected {
                            sender_id: sender,
                            receiver_id: target,
                        });
                }
            }
            GameplayIntent::RequestResources {
                target_player,
                gold,
                troops,
            } => self.propose_resources(stamped.player_id, *target_player, *gold, *troops, 0.0, 0.0),
            GameplayIntent::OfferTrade {
                target_player,
                gold,
                troops,
                pay_gold,
                pay_troops,
            } => self.propose_resources(
                stamped.player_id,
                *target_player,
                *gold,
                *troops,
                *pay_gold,
                *pay_troops,
            ),
            GameplayIntent::AcceptResourceRequest { target_player } => {
                let acceptor = stamped.player_id;
                let target = *target_player; // target here is the proposer
                if let Some(pos) = self
                    .resource_requests_proposed
                    .iter()
                    .position(|r| r.proposer == target && r.target == acceptor)
                {
                    if self.resource_requests_proposed[pos].is_trade() {
                        // A trade settles in full or not at all; an unpayable one stays pending.
                        let req = self.resource_requests_proposed[pos].clone();
                        if self.trade_settles(&req) {
                            // An offer sent to every ally sells once: the first accept takes it
                            // and withdraws its copies from the other allies.
                            self.resource_requests_proposed.retain(|r| {
                                !(r.proposer == req.proposer
                                    && (r.gold, r.troops, r.pay_gold, r.pay_troops)
                                        == (req.gold, req.troops, req.pay_gold, req.pay_troops))
                            });
                            self.move_resources(acceptor, target, req.gold, req.troops);
                            self.move_resources(target, acceptor, req.pay_gold, req.pay_troops);
                        }
                        return;
                    }
                    let req = self.resource_requests_proposed.remove(pos);
                    if !self.resources_can_move(acceptor, target) {
                        return;
                    }
                    // The acceptor pays only what fits; the rest stays with the acceptor.
                    let actual_g = self.gold_transferable(acceptor, req.gold);
                    let actual_t = self.troops_transferable(acceptor, target, req.troops);
                    if let Some(acc_player) = self.state.player_mut(acceptor) {
                        acc_player.gold -= actual_g;
                        acc_player.troops -= actual_t;
                    }
                    if (actual_g > 0.0 || actual_t > 0.0)
                        && let Some(prop_player) = self.state.player_mut(target)
                    {
                        prop_player.credit_gold(actual_g);
                        prop_player.credit_troops(actual_t);
                        self.state
                            .events
                            .push(crate::game::GameEvent::ResourceTransferred {
                                sender_id: acceptor,
                                receiver_id: target,
                                gold: actual_g,
                                troops: actual_t,
                            });
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
        if !self.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet) {
            return;
        }
        let owner = self.state.map.owner_states()[target_tile as usize];
        let is_betrayer = self
            .state
            .player(owner)
            .map(crate::diplomacy::can_retaliate_against)
            .unwrap_or(false);
        let is_allied_in_list = self
            .state
            .player(player_id)
            .map(|p| p.alliances.contains(&owner))
            .unwrap_or(false);

        if is_allied_in_list && is_betrayer {
            // Silently break the alliance without any penalty for the attacker
            self.break_campaign_alliance(player_id, owner);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameState, UnitType};
    use crate::game_config::GameConfig;
    use crate::player::Player;
    use crate::protocol::StampedIntent;
    use crate::warp_fleet::WarpFleet;
    use crate::water_components::WaterComponents;

    #[test]
    fn recall_fleet_only_retires_owned_troop_transports_once() {
        let state = GameState::new(1, 4, 4, GameConfig::default());
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = crate::engine::SowEngine::new(state, water);
        for (id, owner_id, unit_type) in [
            (1, 7, UnitType::TransportShip),
            (2, 7, UnitType::TradeShip),
            (3, 7, UnitType::Warship),
            (4, 8, UnitType::TransportShip),
        ] {
            engine.add_fleet(WarpFleet::new(
                id,
                owner_id,
                9,
                unit_type,
                100.0,
                (0, 0),
                vec![],
            ));
        }
        let recall = StampedIntent {
            player_id: 7,
            intent: GameplayIntent::RecallFleet { fleet_id: 1 },
        };
        engine.apply_stamped_intent(&recall, 0);
        engine.apply_stamped_intent(&recall, 1);
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 7,
                intent: GameplayIntent::RecallFleet { fleet_id: 2 },
            },
            2,
        );
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 7,
                intent: GameplayIntent::RecallFleet { fleet_id: 3 },
            },
            3,
        );
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 7,
                intent: GameplayIntent::RecallFleet { fleet_id: 4 },
            },
            4,
        );

        assert!(engine.fleets[0].retreating);
        assert!(!engine.fleets[1].retreating);
        assert!(!engine.fleets[2].retreating);
        assert!(!engine.fleets[3].retreating);
    }

    #[test]
    fn scuttle_only_removes_owned_transports_and_frees_their_slots_without_refund() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        state.register_player(Player::new_human(7, "Seven".into(), [1.0, 0.0, 0.0], &config));
        state.register_player(Player::new_human(8, "Eight".into(), [0.0, 1.0, 0.0], &config));
        let initial_troops = state.player(7).unwrap().troops;
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = crate::engine::SowEngine::new(state, water);
        for (id, owner_id, unit_type) in [
            (1, 7, UnitType::TransportShip),
            (2, 7, UnitType::TradeShip),
            (3, 8, UnitType::TransportShip),
        ] {
            engine.add_fleet(WarpFleet::new(
                id,
                owner_id,
                9,
                unit_type,
                100.0,
                (0, 0),
                vec![0, 1],
            ));
        }

        assert_eq!(engine.boat_slots_used(7), 1);
        for intent in [
            StampedIntent {
                player_id: 8,
                intent: GameplayIntent::ScuttleFleet { fleet_id: 1 },
            },
            StampedIntent {
                player_id: 7,
                intent: GameplayIntent::ScuttleFleet { fleet_id: 2 },
            },
            StampedIntent {
                player_id: 7,
                intent: GameplayIntent::ScuttleFleet { fleet_id: 3 },
            },
        ] {
            engine.apply_stamped_intent(&intent, 0);
        }
        assert_eq!(engine.fleets.len(), 3);

        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 7,
                intent: GameplayIntent::ScuttleFleet { fleet_id: 1 },
            },
            1,
        );

        assert_eq!(engine.fleets.iter().map(|fleet| fleet.id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(engine.boat_slots_used(7), 0);
        assert_eq!(engine.state.player(7).unwrap().troops, initial_troops);
    }

    #[test]
    fn resource_requests_only_pay_once_between_live_participants() {
        for (payer_alive, requester_alive) in [(true, true), (true, false), (false, true)] {
            let config = GameConfig::default();
            let mut state = GameState::new(1, 4, 4, config.clone());
            for id in 1..=2 {
                let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
                player.gold = 100.0;
                player.troops = 100.0;
                player.max_troops = 1_000.0;
                player.alliances.push(if id == 1 { 2 } else { 1 });
                state.register_player(player);
            }
            let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
            let payer_gold_received = engine.state.player(1).unwrap().gold_received;
            let requester_gold_received = engine.state.player(2).unwrap().gold_received;
            let payer_troops_received = engine.state.player(1).unwrap().troops_received;
            let requester_troops_received = engine.state.player(2).unwrap().troops_received;
            engine.apply_stamped_intent(
                &StampedIntent {
                    player_id: 2,
                    intent: GameplayIntent::RequestResources {
                        target_player: 1,
                        gold: 20.0,
                        troops: 10.0,
                    },
                },
                0,
            );
            engine.apply_stamped_intent(
                &StampedIntent {
                    player_id: 3,
                    intent: GameplayIntent::AcceptResourceRequest { target_player: 2 },
                },
                1,
            );
            assert_eq!(engine.resource_requests_proposed.len(), 1);
            engine.state.player_mut(1).unwrap().alive = payer_alive;
            engine.state.player_mut(2).unwrap().alive = requester_alive;
            let accept = StampedIntent {
                player_id: 1,
                intent: GameplayIntent::AcceptResourceRequest { target_player: 2 },
            };
            engine.apply_stamped_intent(&accept, 2);
            engine.apply_stamped_intent(&accept, 3);
            let paid = payer_alive && requester_alive;
            let payer = engine.state.player(1).unwrap();
            let requester = engine.state.player(2).unwrap();
            assert_eq!(payer.gold, if paid { 80.0 } else { 100.0 });
            assert_eq!(payer.troops, if paid { 90.0 } else { 100.0 });
            assert_eq!(requester.gold, if paid { 120.0 } else { 100.0 });
            assert_eq!(requester.troops, if paid { 110.0 } else { 100.0 });
            assert_eq!(payer.gold_received, payer_gold_received);
            assert_eq!(requester.gold_received, requester_gold_received + if paid { 20.0 } else { 0.0 });
            assert_eq!(payer.troops_received, payer_troops_received);
            assert_eq!(requester.troops_received, requester_troops_received + if paid { 10.0 } else { 0.0 });
            assert!(engine.resource_requests_proposed.is_empty());
        }
    }

    #[test]
    fn ranked_duel_blocks_resource_requests_between_competitors() {
        for ranked_1v1 in [false, true] {
            let config = GameConfig { ranked_1v1, ..Default::default() };
            let mut state = GameState::new(1, 4, 4, config.clone());
            for id in 1..=2 {
                state.register_player(Player::new_human(id, format!("P{id}"), [1.0; 3], &config));
            }
            let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
            engine.state.player_mut(1).unwrap().alliances.push(2);
            engine.apply_stamped_intent(&StampedIntent {
                player_id: 1,
                intent: GameplayIntent::RequestResources { target_player: 2, gold: 20.0, troops: 0.0 },
            }, 0);
            assert_eq!(engine.resource_requests_proposed.len(), usize::from(!ranked_1v1));
        }
    }

    #[test]
    fn ranked_duel_blocks_direct_transfers_even_if_an_alliance_exists() {
        for ranked_1v1 in [false, true] {
            let config = GameConfig { ranked_1v1, ..Default::default() };
            let mut state = GameState::new(1, 4, 4, config.clone());
            for id in 1..=2 {
                state.register_player(Player::new_human(id, format!("P{id}"), [1.0; 3], &config));
            }
            state.player_mut(1).unwrap().alliances.push(2);
            let starting_gold = state.player(1).unwrap().gold;
            let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
            engine.apply_stamped_intent(&StampedIntent {
                player_id: 1,
                intent: GameplayIntent::SendResources { target_player: 2, gold: 20.0, troops: 0.0 },
            }, 0);
            assert_eq!(engine.state.player(1).unwrap().gold, starting_gold - if ranked_1v1 { 0.0 } else { 20.0 });
        }
    }

    #[test]
    fn direct_resource_transfers_count_only_the_amount_the_recipient_can_hold() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        for id in 1..=2 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.gold = 100.0;
            player.troops = 100.0;
            player.max_troops = if id == 2 { 105.0 } else { 1_000.0 };
            player.alliances.push(if id == 1 { 2 } else { 1 });
            state.register_player(player);
        }
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        let gold_before = engine.state.player(2).unwrap().gold_received;
        let troops_before = engine.state.player(2).unwrap().troops_received;

        let transfer = StampedIntent {
            player_id: 1,
            intent: GameplayIntent::SendResources {
                target_player: 2,
                gold: 20.0,
                troops: 10.0,
            },
        };
        assert_eq!(
            engine.apply_intents_tracking_troop_spend(std::slice::from_ref(&transfer), Some(1)),
            5.0
        );

        let recipient = engine.state.player(2).unwrap();
        assert_eq!(recipient.gold, 120.0);
        assert_eq!(recipient.troops, 105.0);
        assert_eq!(recipient.gold_received, gold_before + 20.0);
        assert_eq!(recipient.troops_received, troops_before + 5.0);
        assert_eq!(
            engine.apply_intents_tracking_troop_spend(&[transfer], Some(1)),
            0.0
        );
    }

    #[test]
    fn debt_cannot_send_or_accept_gold_that_does_not_exist() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        let mut debtor = Player::new_human(1, "Debtor".into(), [1.0; 3], &config);
        debtor.gold = -25.0;
        debtor.alliances.push(2);
        let mut ally = Player::new_human(2, "Ally".into(), [0.5; 3], &config);
        ally.gold = 80.0;
        ally.alliances.push(1);
        state.register_player(debtor);
        state.register_player(ally);
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());

        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::SendResources {
                    target_player: 2,
                    gold: 50.0,
                    troops: 0.0,
                },
            },
            0,
        );
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 2,
                intent: GameplayIntent::RequestResources {
                    target_player: 1,
                    gold: 50.0,
                    troops: 0.0,
                },
            },
            1,
        );
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::AcceptResourceRequest { target_player: 2 },
            },
            2,
        );

        assert_eq!(engine.state.player(1).unwrap().gold, -25.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 80.0);
    }

    #[test]
    fn requests_keep_only_what_the_payer_can_give_and_the_proposer_can_hold() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        let mut proposer = Player::new_human(1, "Proposer".into(), [1.0; 3], &config);
        proposer.troops = 90.0;
        proposer.max_troops = 100.0;
        proposer.alliances.push(2);
        let mut payer = Player::new_human(2, "Payer".into(), [0.5; 3], &config);
        payer.gold = 30.0;
        payer.troops = 50.0;
        payer.max_troops = 1_000.0;
        payer.alliances.push(1);
        state.register_player(proposer);
        state.register_player(payer);
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());

        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::RequestResources {
                    target_player: 2,
                    gold: 100.0,
                    troops: 100.0,
                },
            },
            0,
        );

        let request = &engine.resource_requests_proposed[0];
        assert_eq!((request.gold, request.troops), (30.0, 10.0));
    }

    #[test]
    fn accepted_request_moves_only_what_fits_and_the_payer_keeps_the_rest() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        let mut proposer = Player::new_human(1, "Proposer".into(), [1.0; 3], &config);
        proposer.troops = 90.0;
        proposer.max_troops = 100.0;
        proposer.alliances.push(2);
        let mut payer = Player::new_human(2, "Payer".into(), [0.5; 3], &config);
        payer.troops = 100.0;
        payer.max_troops = 1_000.0;
        payer.alliances.push(1);
        state.register_player(proposer);
        state.register_player(payer);
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        engine
            .resource_requests_proposed
            .push(crate::engine::ResourceRequestProposed {
                proposer: 1,
                target: 2,
                gold: 0.0,
                troops: 50.0,
                pay_gold: 0.0,
                pay_troops: 0.0,
                created_tick: 0,
            });

        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 2,
                intent: GameplayIntent::AcceptResourceRequest { target_player: 1 },
            },
            0,
        );

        assert_eq!(engine.state.player(1).unwrap().troops, 100.0);
        assert_eq!(engine.state.player(2).unwrap().troops, 90.0);
        assert!(engine.resource_requests_proposed.is_empty());
    }

    #[test]
    fn resource_request_cooldown_limits_repeat_requests_between_the_same_pair() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        for id in 1..=2 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.alliances.push(if id == 1 { 2 } else { 1 });
            state.register_player(player);
        }
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        let request = StampedIntent {
            player_id: 1,
            intent: GameplayIntent::RequestResources {
                target_player: 2,
                gold: 20.0,
                troops: 0.0,
            },
        };

        engine.apply_stamped_intent(&request, 0);
        engine.resource_requests_proposed.clear();
        engine.apply_stamped_intent(&request, 1);
        assert!(engine.resource_requests_proposed.is_empty());

        engine.state.tick += u64::from(crate::diplomacy::RESOURCE_REQUEST_COOLDOWN_TICKS);
        engine.apply_stamped_intent(&request, 2);
        assert_eq!(engine.resource_requests_proposed.len(), 1);
    }

    #[test]
    fn trade_settles_both_sides_at_once_or_waits() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        for id in 1..=2 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.gold = 100.0;
            player.troops = 500.0;
            player.max_troops = 1_000.0;
            player.alliances.push(if id == 1 { 2 } else { 1 });
            state.register_player(player);
        }
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        // Player 1 sells 300 troops to player 2 for 200 gold.
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::OfferTrade {
                    target_player: 2,
                    gold: 200.0,
                    troops: 0.0,
                    pay_gold: 0.0,
                    pay_troops: 300.0,
                },
            },
            0,
        );
        let trade = engine.resource_requests_proposed[0].clone();
        assert_eq!((trade.gold, trade.pay_troops), (200.0, 300.0), "a trade keeps its price");

        let accept = StampedIntent {
            player_id: 2,
            intent: GameplayIntent::AcceptResourceRequest { target_player: 1 },
        };
        // The buyer holds only 100 gold: nothing moves and the offer waits.
        engine.apply_stamped_intent(&accept, 1);
        assert_eq!(engine.state.player(1).unwrap().troops, 500.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 100.0);
        assert_eq!(engine.resource_requests_proposed.len(), 1);

        engine.state.player_mut(2).unwrap().gold = 250.0;
        engine.apply_stamped_intent(&accept, 2);
        let (seller, buyer) = (engine.state.player(1).unwrap(), engine.state.player(2).unwrap());
        assert_eq!((seller.troops, seller.gold), (200.0, 300.0));
        assert_eq!((buyer.troops, buyer.gold), (800.0, 50.0));
        assert!(engine.resource_requests_proposed.is_empty());
    }

    #[test]
    fn an_offer_to_every_ally_sells_once_and_lapses_unanswered() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        for id in 1..=3 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.gold = 500.0;
            player.troops = 500.0;
            player.max_troops = 1_000.0;
            player.alliances = (1..=3).filter(|other| *other != id).collect();
            state.register_player(player);
        }
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        // Player 1 offers 300 troops for 200 gold to both allies.
        for (index, ally) in [2, 3].into_iter().enumerate() {
            engine.apply_stamped_intent(
                &StampedIntent {
                    player_id: 1,
                    intent: GameplayIntent::OfferTrade {
                        target_player: ally,
                        gold: 200.0,
                        troops: 0.0,
                        pay_gold: 0.0,
                        pay_troops: 300.0,
                    },
                },
                index as u32,
            );
        }
        assert_eq!(engine.resource_requests_proposed.len(), 2);
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 3,
                intent: GameplayIntent::AcceptResourceRequest { target_player: 1 },
            },
            2,
        );
        assert!(engine.resource_requests_proposed.is_empty(), "the copy for player 2 is withdrawn");
        assert_eq!(engine.state.player(1).unwrap().troops, 200.0, "the seller pays once");
        assert_eq!(engine.state.player(3).unwrap().troops, 800.0);

        engine.state.tick += u64::from(crate::diplomacy::RESOURCE_REQUEST_COOLDOWN_TICKS);
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 2,
                intent: GameplayIntent::RequestResources { target_player: 1, gold: 50.0, troops: 0.0 },
            },
            3,
        );
        engine.state.tick += u64::from(crate::diplomacy::RESOURCE_REQUEST_TTL_TICKS);
        engine.prune_alliance_diplomacy();
        assert_eq!(engine.resource_requests_proposed.len(), 1, "still answerable at the deadline");
        engine.state.tick += 1;
        engine.prune_alliance_diplomacy();
        assert!(engine.resource_requests_proposed.is_empty(), "an unanswered request lapses");
    }

    #[test]
    fn trade_offers_need_the_price_in_hand_and_one_way_resources() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        for id in 1..=2 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.gold = 100.0;
            player.troops = 100.0;
            player.max_troops = 1_000.0;
            player.alliances.push(if id == 1 { 2 } else { 1 });
            state.register_player(player);
        }
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        let offer = |gold, pay_gold| StampedIntent {
            player_id: 1,
            intent: GameplayIntent::OfferTrade {
                target_player: 2,
                gold,
                troops: 10.0,
                pay_gold,
                pay_troops: 0.0,
            },
        };
        engine.apply_stamped_intent(&offer(0.0, 500.0), 0);
        engine.apply_stamped_intent(&offer(5.0, 50.0), 1);
        assert!(engine.resource_requests_proposed.is_empty());
    }

    #[test]
    fn broken_alliance_hides_and_voids_a_pending_request() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        for id in 1..=2 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.troops = 100.0;
            player.max_troops = 1_000.0;
            player.alliances.push(if id == 1 { 2 } else { 1 });
            state.register_player(player);
        }
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 1,
                intent: GameplayIntent::RequestResources {
                    target_player: 2,
                    gold: 20.0,
                    troops: 10.0,
                },
            },
            0,
        );
        assert_eq!(engine.resource_requests_proposed.len(), 1);

        for id in 1..=2 {
            engine.state.player_mut(id).unwrap().alliances.clear();
        }
        let snapshot = engine.build_snapshot();
        assert!(
            snapshot
                .players
                .iter()
                .all(|p| p.resource_requests.is_empty())
        );

        engine.apply_stamped_intent(
            &StampedIntent {
                player_id: 2,
                intent: GameplayIntent::AcceptResourceRequest { target_player: 1 },
            },
            1,
        );
        assert!(engine.resource_requests_proposed.is_empty());
        assert_eq!(engine.state.player(1).unwrap().troops, 100.0);
        assert_eq!(engine.state.player(2).unwrap().troops, 100.0);
    }
}
