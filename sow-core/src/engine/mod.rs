use crate::building::{Building, BuildingAggregate, BuildingGrid, DefenseGrid};
use crate::diplomacy::{
    ALLIANCE_REQUEST_COOLDOWN_TICKS, ALLIANCE_REQUEST_TTL_TICKS, AllianceProposal,
    BETRAYAL_COOLDOWN_TICKS,
};
use crate::execution::AttackExecution;
use crate::game::GameState;
use crate::pathfinding::WaterPathfinderScratch;
use crate::player::PlayerId;
use crate::warp_fleet::WarpFleet;
use crate::water_components::WaterComponents;
use serde::{Deserialize, Serialize};

const CAMPAIGN_ASSAULT_PHASES: u8 = 4;

#[derive(Clone, Debug)]
pub struct CampaignAssaultState {
    pub root_target: PlayerId,
    pub team: crate::protocol::Team,
    pub include_allies: bool,
    pub preserve_relation: bool,
    pub hold_last_tile: bool,
    pub capacity_ratio: f64,
    pub interval_ticks: u64,
    pub next_tick: u64,
    /// Current phase in the four-phase troop release window (0 before first launch).
    pub phase_index: u8,
    pub attacker_ids: Vec<PlayerId>,
    pub target_ids: Vec<PlayerId>,
    /// Scheduled wave number used to rotate equal-pressure targets deterministically.
    pub wave_index: u64,
    /// Troops already launched by each force during the current 12-second window.
    pub cycle_released_troops: std::collections::HashMap<PlayerId, f64>,
    /// Last wave that successfully dispatched troops to each live target.
    pub target_last_dispatched_wave: std::collections::HashMap<PlayerId, u64>,
    /// Primary target selected by the latest coordinated wave for each Roman force.
    pub focus_targets: std::collections::HashMap<PlayerId, PlayerId>,
    pub reserve_spawned: bool,
}

struct CampaignAssaultRouteCandidate {
    target_id: PlayerId,
    target_tile: u32,
    route: Option<crate::warp_fleet::FleetRoute>,
    committed_troops: f64,
    defense_capacity: f64,
    tie_rank: usize,
}

fn allocate_campaign_assault_shares(
    available: f64,
    min_attack: f64,
    targets: &[(f64, f64)],
) -> Vec<f64> {
    let count = targets.len();
    if count == 0 || !available.is_finite() || !min_attack.is_finite() || min_attack <= 0.0 {
        return Vec::new();
    }
    let minimum_total = min_attack * count as f64;
    if available < minimum_total {
        return Vec::new();
    }

    // Water-fill each legal minimum attack until projected pressure is balanced.
    let mut breakpoints: Vec<(f64, usize)> = targets
        .iter()
        .enumerate()
        .map(|(index, (committed, capacity))| {
            let committed = if committed.is_nan() {
                0.0
            } else {
                committed.max(0.0).min(f64::MAX / 4.0)
            };
            let capacity = if capacity.is_finite() {
                capacity.max(min_attack)
            } else {
                min_attack
            };
            ((committed + min_attack) / capacity, index)
        })
        .collect();
    breakpoints.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut active_capacity = 0.0;
    let mut next = 0;
    let mut level = breakpoints[0].0;
    while next < count && breakpoints[next].0 == level {
        let index = breakpoints[next].1;
        active_capacity += targets[index].1.max(min_attack);
        next += 1;
    }

    let mut remaining = (available - minimum_total).max(0.0);
    while remaining > 0.0 && next < count {
        let next_level = breakpoints[next].0;
        let cost = (next_level - level).max(0.0) * active_capacity;
        if remaining <= cost {
            level += remaining / active_capacity;
            remaining = 0.0;
            break;
        }
        remaining -= cost;
        level = next_level;
        while next < count && breakpoints[next].0 == level {
            let index = breakpoints[next].1;
            active_capacity += targets[index].1.max(min_attack);
            next += 1;
        }
    }
    if remaining > 0.0 {
        level += remaining / active_capacity;
    }

    targets
        .iter()
        .map(|(committed, capacity)| {
            let committed = if committed.is_nan() {
                0.0
            } else {
                committed.max(0.0).min(f64::MAX / 4.0)
            };
            let capacity = if capacity.is_finite() {
                capacity.max(min_attack)
            } else {
                min_attack
            };
            (level * capacity - committed).max(min_attack)
        })
        .collect()
}

fn prioritize_campaign_assault_targets(
    mut reachable: Vec<CampaignAssaultRouteCandidate>,
    limit: usize,
    wave_index: u64,
    last_dispatched_wave: &std::collections::HashMap<PlayerId, u64>,
) -> Vec<CampaignAssaultRouteCandidate> {
    if reachable.is_empty() || limit == 0 {
        return Vec::new();
    }
    let count = reachable.len();
    let tie_start = (wave_index % count as u64) as usize;
    for (index, candidate) in reachable.iter_mut().enumerate() {
        candidate.tie_rank = (index + count - tie_start) % count;
    }
    reachable.sort_by(|a, b| {
        match (
            last_dispatched_wave.get(&a.target_id),
            last_dispatched_wave.get(&b.target_id),
        ) {
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| {
            (a.committed_troops / a.defense_capacity)
                .total_cmp(&(b.committed_troops / b.defense_capacity))
        })
        .then_with(|| a.tie_rank.cmp(&b.tie_rank))
        .then_with(|| a.target_id.cmp(&b.target_id))
    });
    reachable.truncate(limit.min(count));
    reachable
}

fn aggregate_campaign_assault_incoming(
    attacker_ids: &std::collections::HashSet<PlayerId>,
    target_ids: &std::collections::HashSet<PlayerId>,
    commitments: impl IntoIterator<Item = (PlayerId, PlayerId, f64, bool)>,
) -> std::collections::HashMap<PlayerId, f64> {
    let mut incoming: std::collections::HashMap<PlayerId, f64> = target_ids
        .iter()
        .copied()
        .map(|target_id| (target_id, 0.0))
        .collect();
    for (attacker_id, target_id, troops, retreating) in commitments {
        if !retreating
            && attacker_ids.contains(&attacker_id)
            && troops.is_finite()
            && let Some(projected) = incoming.get_mut(&target_id)
        {
            let total = *projected + troops.max(0.0);
            *projected = if total.is_finite() {
                total
            } else {
                f64::MAX / 4.0
            };
        }
    }
    incoming
}

/// Build the deterministic match state used by lockstep clients and replay verification.
pub fn initialize_match_engine(
    config: crate::game_config::GameConfig,
    seed: u64,
    map_bytes: &[u8],
    players: Vec<crate::protocol::PlayerInfo>,
    fallback_spawns: Vec<crate::map_file::MapSpawn>,
    fallback_geo_bounds: Option<crate::map_file::GeoBounds>,
    fallback_rosters: Vec<crate::map_file::MapRosterPreset>,
    fallback_land_tiles: u32,
) -> SowEngine {
    let mut state = GameState::new(seed, config.map_width, config.map_height, config);
    state.map_spawns = fallback_spawns;
    state.geo_bounds = fallback_geo_bounds;
    let selected_roster = state.config.map_roster_preset.clone();
    state.map_rosters = fallback_rosters
        .into_iter()
        .filter(|preset| selected_roster.as_deref() == Some(preset.id.as_str()))
        .collect();
    state.total_land_tiles = fallback_land_tiles;

    if let Ok(map_file) = crate::maps::load_map_from_payload(map_bytes) {
        state.total_land_tiles = map_file.num_land_tiles;
        state.map_spawns = map_file.spawns;
        state.geo_bounds = map_file.geo_bounds;
        state.map_rosters = map_file
            .rosters
            .into_iter()
            .filter(|preset| selected_roster.as_deref() == Some(preset.id.as_str()))
            .collect();
        if map_file.terrain.len() == state.map.terrain.len() {
            for (tile, byte) in state
                .map
                .terrain
                .iter_mut()
                .zip(map_file.terrain.iter().copied())
            {
                *tile = crate::map::MapTile::from_byte(byte);
            }
        }
    } else if map_bytes.len() == state.map.terrain.len() {
        for (tile, byte) in state.map.terrain.iter_mut().zip(map_bytes.iter().copied()) {
            *tile = crate::map::MapTile::from_byte(byte);
        }
    }

    let water = WaterComponents::compute(&state.map, |_| {});
    let mut engine = SowEngine::new(state, water);
    for player in players {
        if player.player_type == crate::player::PlayerType::Human {
            engine.spawn_human(crate::engine::HumanSpawn {
                player_id: player.id,
                name: player.name,
                color: player.color,
                team: player.team,
                civilization: player.civilization,
                leader: player.leader,
                skin_style: player.skin_style,
                is_ai_controlled: player.is_ai_controlled,
            });
        }
    }
    engine.spawn_scripted();
    engine.spawn_ai(
        engine.state.config.nation_count,
        engine.state.config.bot_count,
    );
    engine
}

#[derive(Clone)]
pub struct PlacementScratch {
    pub visited_stamp: [u32; 1024],
    pub stamp: u32,
    pub queue: Vec<u32>,
    pub border_scratch: Vec<u32>,
    pub interior_scratch: Vec<(i32, i32)>,
    pub neighbor_scratch: Vec<u16>,
    #[cfg(feature = "ai-metrics")]
    pub candidates_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub building_checks: u64,
}

impl Default for PlacementScratch {
    fn default() -> Self {
        Self {
            visited_stamp: [0; 1024],
            stamp: 0,
            queue: Vec::new(),
            border_scratch: Vec::new(),
            interior_scratch: Vec::new(),
            neighbor_scratch: Vec::new(),
            #[cfg(feature = "ai-metrics")]
            candidates_examined: 0,
            #[cfg(feature = "ai-metrics")]
            building_checks: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResourceRequestProposed {
    pub proposer: crate::player::PlayerId,
    pub target: crate::player::PlayerId,
    pub gold: f64,
    pub troops: f64,
}

pub type SeaLaneCalcState = (usize, Vec<crate::sea_lane::SeaLane>, Vec<(u64, u32, u32)>);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BotWorkCounters {
    #[cfg(feature = "ai-metrics")]
    pub border_cells_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub border_blocks_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub border_directory_words_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub neighbor_cells_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub placement_candidates_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub placement_building_checks: u64,
    #[cfg(feature = "ai-metrics")]
    pub naval_routes_calculated: u64,
    #[cfg(feature = "ai-metrics")]
    pub shoreline_candidates_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub attack_entries_scanned_last_update: u64,
    #[cfg(feature = "ai-metrics")]
    pub diplomacy_proposals_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub diplomacy_resource_requests_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub nuke_buildings_examined: u64,
    #[cfg(feature = "ai-metrics")]
    pub nuke_sam_checks: u64,
    #[cfg(feature = "ai-metrics")]
    pub nuke_history_lookups: u64,
}

#[derive(Clone)]
pub struct SowEngine {
    pub state: GameState,
    pub attacks: Vec<AttackExecution>,
    pub fleets: Vec<WarpFleet>,
    pub buildings: Vec<Building>,
    pub water: WaterComponents,
    pub path_scratch: WaterPathfinderScratch,
    pub flow_field_cache: crate::pathfinding::FlowFieldCache,
    pub placement_scratch: PlacementScratch,
    pub defense_grid: DefenseGrid,
    pub defense_grid_dirty: bool,
    pub render_defense_dirty: bool,
    pub building_grid: BuildingGrid,
    pub building_aggregates: Vec<BuildingAggregate>,
    pub building_aggregates_dirty: bool,
    pub sea_lanes_dirty: bool,
    pub sea_lane_calc: Option<SeaLaneCalcState>,

    pub alliances_proposed: Vec<AllianceProposal>,
    /// `(proposer, target)` → tick when another outgoing request is allowed.
    pub alliance_request_cooldown_until: std::collections::HashMap<(PlayerId, PlayerId), u32>,
    /// Bot id → tick when another betrayal is allowed.
    pub alliance_betray_cooldown_until: std::collections::HashMap<PlayerId, u32>,
    pub resource_requests_proposed: Vec<ResourceRequestProposed>,
    /// Campaign progression is episode-local and absent from every other match.
    pub campaign_unlocks: Option<crate::campaign::CampaignUnlocks>,
    pub campaign_unlock_revision: u64,
    /// Scripted roster portrait slugs, kept out of Player so ordinary matches stay unchanged.
    pub campaign_avatars: std::collections::HashMap<PlayerId, String>,
    /// Scripted campaign membership, kept out of Player and ordinary matches.
    pub campaign_support_intervals: std::collections::HashMap<PlayerId, u32>,
    /// Optional fixed bonus loot per scripted campaign faction.
    pub campaign_gold_loot_bonus: std::collections::HashMap<PlayerId, u32>,
    /// Optional fixed final bounty per scripted campaign faction.
    pub campaign_gold_loot_override: std::collections::HashMap<PlayerId, u32>,
    pub campaign_alliance_groups: std::collections::HashMap<PlayerId, String>,
    pub campaign_relations: std::collections::HashMap<PlayerId, crate::protocol::CampaignRelation>,
    pub campaign_faction_ids: std::collections::HashMap<PlayerId, String>,
    /// Scripted campaign attackers and the live factions they are ordered to pursue.
    pub campaign_assault_targets: std::collections::HashMap<PlayerId, Vec<PlayerId>>,
    /// Factions marked as eligible for a configured campaign assault in their roster.
    pub campaign_assault_force_ids: std::collections::HashSet<PlayerId>,
    /// Final-assault standing-army caps, excluding troops already deployed in combat.
    pub campaign_assault_troop_caps: std::collections::HashMap<PlayerId, f64>,
    pub campaign_assault: Option<CampaignAssaultState>,
    pub campaign_can_request_alliance: std::collections::HashMap<PlayerId, bool>,
    pub campaign_contact_resolved: std::collections::HashSet<PlayerId>,
    /// Next campaign-support send tick for each eligible ally.
    pub campaign_support_next_tick: std::collections::HashMap<PlayerId, u64>,
    /// Player id -> (first debt tick, rebellion already started this debt episode).
    pub(crate) debt_episodes: std::collections::HashMap<PlayerId, (u64, bool)>,
    pub port_queues:
        std::collections::HashMap<u64, std::collections::VecDeque<crate::game::ShipProduction>>,
    pub projectiles: Vec<crate::game::Projectile>,
    pub silo_cooldowns: std::collections::HashMap<u64, u32>,
    pub mirv_launches: std::collections::HashMap<u16, u32>,
    /// Launch count by `(target player, tile)`; lookup-only during scoring.
    pub recent_nuke_targets: std::collections::HashMap<(u16, u32), u64>,
    pub mirv_cooldown_targets: std::collections::HashMap<u16, u64>,
    pub(crate) bot_route_cache: Vec<(PlayerId, u32, crate::warp_fleet::FleetRoute)>,
    pub(crate) bot_sam_tiles_cache: Option<Vec<(u32, u16)>>,
    pub(crate) bot_crown_leader: Option<u16>,
    pub(crate) ai_attack_index: Vec<Vec<usize>>,
    pub(crate) ai_attack_index_dirty: bool,
    pub(crate) bot_work: BotWorkCounters,
    #[cfg(test)]
    pub(crate) test_last_ai_intents: Vec<crate::protocol::StampedIntent>,
}

impl SowEngine {
    pub fn new(mut state: GameState, water: WaterComponents) -> Self {
        let campaign_unlocks = state
            .config
            .tutorial
            .then(crate::campaign::CampaignUnlocks::default);
        state.map.compute_shorelines();
        let w = state.map.width;
        let h = state.map.height;

        let mut path_scratch = WaterPathfinderScratch::default();
        if w > 0 && h > 0 {
            path_scratch.astar.ensure_capacity(&state.map);
        }

        let mut placement_scratch = PlacementScratch::default();
        placement_scratch.queue.reserve(1024);
        placement_scratch.border_scratch.reserve(256);
        placement_scratch.interior_scratch.reserve(8);
        placement_scratch.neighbor_scratch.reserve(64);

        Self {
            state,
            attacks: Vec::with_capacity(1024),
            fleets: Vec::with_capacity(256),
            buildings: Vec::with_capacity(4096),
            water,
            path_scratch,
            flow_field_cache: crate::pathfinding::FlowFieldCache::default(),
            placement_scratch,
            defense_grid: DefenseGrid::default(),
            defense_grid_dirty: true,
            render_defense_dirty: true,
            building_grid: BuildingGrid::default(),
            building_aggregates: Vec::with_capacity(256),
            building_aggregates_dirty: true,
            sea_lanes_dirty: true,
            sea_lane_calc: None,

            alliances_proposed: Vec::new(),
            alliance_request_cooldown_until: std::collections::HashMap::new(),
            alliance_betray_cooldown_until: std::collections::HashMap::new(),
            resource_requests_proposed: Vec::new(),
            campaign_unlocks,
            campaign_unlock_revision: 0,
            campaign_avatars: std::collections::HashMap::new(),
            campaign_support_intervals: std::collections::HashMap::new(),
            campaign_gold_loot_bonus: std::collections::HashMap::new(),
            campaign_gold_loot_override: std::collections::HashMap::new(),
            campaign_alliance_groups: std::collections::HashMap::new(),
            campaign_relations: std::collections::HashMap::new(),
            campaign_faction_ids: std::collections::HashMap::new(),
            campaign_assault_targets: std::collections::HashMap::new(),
            campaign_assault_force_ids: std::collections::HashSet::new(),
            campaign_assault_troop_caps: std::collections::HashMap::new(),
            campaign_assault: None,
            campaign_can_request_alliance: std::collections::HashMap::new(),
            campaign_contact_resolved: std::collections::HashSet::new(),
            campaign_support_next_tick: std::collections::HashMap::new(),
            debt_episodes: std::collections::HashMap::new(),
            port_queues: std::collections::HashMap::new(),
            projectiles: Vec::new(),
            silo_cooldowns: std::collections::HashMap::new(),
            mirv_launches: std::collections::HashMap::new(),
            recent_nuke_targets: std::collections::HashMap::new(),
            mirv_cooldown_targets: std::collections::HashMap::new(),
            bot_route_cache: Vec::with_capacity(16),
            bot_sam_tiles_cache: None,
            bot_crown_leader: None,
            ai_attack_index: Vec::new(),
            ai_attack_index_dirty: true,
            bot_work: BotWorkCounters::default(),
            #[cfg(test)]
            test_last_ai_intents: Vec::new(),
        }
    }

    #[inline]
    pub fn campaign_allows_building(&self, kind: crate::game::BuildingKind, level: u8) -> bool {
        self.campaign_unlocks
            .as_ref()
            .is_none_or(|unlocks| unlocks.allows_building(kind, level))
    }

    #[inline]
    pub fn campaign_allows_action(&self, action: crate::campaign::CampaignAction) -> bool {
        self.campaign_unlocks
            .as_ref()
            .is_none_or(|unlocks| unlocks.allows_action(action))
    }

    /// Accept only valid, cumulative updates from the active local campaign runtime.
    pub fn set_campaign_unlocks(&mut self, unlocks: crate::campaign::CampaignUnlocks) -> bool {
        let Some(current) = self.campaign_unlocks.as_ref() else {
            return false;
        };
        if !unlocks.extends(current) {
            return false;
        }
        if unlocks != *current {
            self.campaign_unlocks = Some(unlocks);
            self.campaign_unlock_revision = self.campaign_unlock_revision.wrapping_add(1);
        }
        true
    }

    #[inline]
    pub fn current_tick_u32(&self) -> u32 {
        self.state.tick as u32
    }

    /// Expire stale proposals, cooldown entries, traitor flags, and emoji timers run elsewhere.
    pub fn prune_alliance_diplomacy(&mut self) {
        let tick = self.current_tick_u32();
        let mut expired = Vec::new();
        self.alliances_proposed.retain(|p| {
            let alive = tick.saturating_sub(p.created_tick) <= ALLIANCE_REQUEST_TTL_TICKS;
            if !alive {
                expired.push(*p);
            }
            alive
        });
        for p in expired {
            self.record_alliance_request_lifecycle(
                p.proposer,
                p.target,
                crate::game::AllianceRequestStatus::Expired,
            );
            self.mark_alliance_request_cooldown(p.proposer, p.target);
        }
        self.alliance_request_cooldown_until
            .retain(|_, until| *until > tick);
        self.alliance_betray_cooldown_until
            .retain(|_, until| *until > tick);
        for player in &mut self.state.players {
            if player.traitor && player.traitor_tick > 0 && tick >= player.traitor_tick {
                player.traitor = false;
                player.traitor_tick = 0;
            }
        }
    }

    #[inline]
    pub fn has_alliance_proposal(&self, proposer: PlayerId, target: PlayerId) -> bool {
        self.alliances_proposed
            .iter()
            .any(|p| p.proposer == proposer && p.target == target)
    }

    #[inline]
    pub fn can_send_alliance_request(&self, proposer: PlayerId, target: PlayerId) -> bool {
        if self.has_alliance_proposal(proposer, target) {
            return false;
        }
        let tick = self.current_tick_u32();
        self.alliance_request_cooldown_until
            .get(&(proposer, target))
            .is_none_or(|until| *until <= tick)
    }

    pub fn mark_alliance_request_cooldown(&mut self, proposer: PlayerId, target: PlayerId) {
        let until = self
            .current_tick_u32()
            .saturating_add(ALLIANCE_REQUEST_COOLDOWN_TICKS);
        self.alliance_request_cooldown_until
            .insert((proposer, target), until);
    }

    pub fn mark_betrayal_cooldown(&mut self, bot_id: PlayerId) {
        let until = self
            .current_tick_u32()
            .saturating_add(BETRAYAL_COOLDOWN_TICKS);
        self.alliance_betray_cooldown_until.insert(bot_id, until);
    }

    pub(crate) fn record_alliance_request_lifecycle(
        &mut self,
        proposer_id: PlayerId,
        target_id: PlayerId,
        status: crate::game::AllianceRequestStatus,
    ) {
        if self
            .state
            .player(proposer_id)
            .is_some_and(|player| player.player_type == crate::player::PlayerType::Human)
        {
            self.state
                .events
                .push(crate::game::GameEvent::AllianceRequestLifecycle {
                    proposer_id,
                    target_id,
                    status,
                });
        }
    }

    pub fn push_alliance_proposal(&mut self, proposer: PlayerId, target: PlayerId) {
        if self.has_alliance_proposal(proposer, target) {
            return;
        }
        self.alliances_proposed.push(AllianceProposal {
            proposer,
            target,
            created_tick: self.current_tick_u32(),
        });
        self.record_alliance_request_lifecycle(
            proposer,
            target,
            crate::game::AllianceRequestStatus::Submitted,
        );
    }

    pub fn refresh_building_grid(&mut self) {
        if !self.building_grid.dirty && self.building_grid.grid_w > 0 {
            return;
        }
        let w = self.state.map.width;
        let h = self.state.map.height;
        self.building_grid.rebuild(self.buildings.iter(), w, h);
    }

    pub(crate) fn refresh_defense_grid(&mut self) {
        if !self.defense_grid_dirty && self.defense_grid.grid_w > 0 {
            return;
        }
        self.defense_grid.rebuild(
            &self.buildings,
            self.state.map.width,
            self.state.map.height,
            crate::building::DEFENSE_GRID_CELL_SIZE,
            &self.state.config,
        );
        self.defense_grid_dirty = false;
    }

    pub fn kill_player(&mut self, player_id: u16) {
        if let Some(player) = self.state.player_mut(player_id) {
            player.alive = false;
        }
        let mut to_clear = Vec::new();
        for (i, &owner) in self.state.map.owner_states().iter().enumerate() {
            if owner == player_id {
                let x = (i % self.state.map.width as usize) as u32;
                let y = (i / self.state.map.width as usize) as u32;
                to_clear.push((x, y));
            }
        }
        for (x, y) in to_clear {
            self.state.set_tile_owner(x, y, 0);
        }
        self.attacks.retain(|a| a.owner_id != player_id);
        self.ai_attack_index_dirty = true;
        self.fleets.retain(|f| f.owner_id != player_id);
        self.clear_campaign_assaults_for(player_id);
    }

    pub fn eliminate_player(
        &mut self,
        victim_id: u16,
        conqueror_id: u16,
        ex: u32,
        ey: u32,
        by_nuke: bool,
    ) {
        let mut base_reward: u32 = 0;
        let mut is_alive = false;
        if let Some(target_player) = self.state.player(victim_id) {
            is_alive = target_player.alive;
            base_reward = match target_player.player_type {
                crate::player::PlayerType::Bot => 25,
                crate::player::PlayerType::Nation => 75,
                crate::player::PlayerType::Human => 150,
            };
        }

        if !is_alive {
            return;
        }

        let total_reward = if let Some(gold) = self.campaign_gold_loot_override.get(&victim_id) {
            *gold
        } else {
            base_reward.saturating_add(
                self.campaign_gold_loot_bonus
                    .get(&victim_id)
                    .copied()
                    .unwrap_or(0),
            )
        };

        // Gather tile conquest contributions (deterministic by player id)
        let mut contributors: Vec<(u16, u32)> = self
            .state
            .players
            .iter()
            .filter_map(|p| {
                if !p.alive {
                    return None;
                }
                p.tile_conquests
                    .get(&victim_id)
                    .copied()
                    .filter(|&c| c > 0)
                    .map(|c| (p.id, c))
            })
            .collect();
        contributors.sort_by_key(|(id, _)| *id);

        let others: Vec<(u16, u32)> = contributors
            .iter()
            .filter(|(id, _)| *id != conqueror_id && *id != victim_id)
            .copied()
            .collect();
        let assist_tiles: u64 = others.iter().map(|(_, count)| u64::from(*count)).sum();

        let (killer_gold, assist_rewards) = if assist_tiles == 0 {
            (total_reward, Vec::new())
        } else {
            let killer_share = total_reward / 2;
            let assist_pool = total_reward - killer_share;
            let mut rewards: Vec<(u16, u32, u64)> = Vec::with_capacity(others.len());
            let mut distributed = 0u32;
            for (id, count) in &others {
                let numerator = u64::from(assist_pool) * u64::from(*count);
                let share = (numerator / assist_tiles) as u32;
                distributed += share;
                rewards.push((*id, share, numerator % assist_tiles));
            }
            rewards.sort_by_key(|(id, _, remainder)| (std::cmp::Reverse(*remainder), *id));
            for (_, share, _) in rewards
                .iter_mut()
                .take((assist_pool - distributed) as usize)
            {
                *share += 1;
            }
            rewards.sort_by_key(|(id, _, _)| *id);
            let rewards = rewards
                .into_iter()
                .map(|(id, share, _)| (id, share))
                .collect();
            (killer_share, rewards)
        };

        // 1. Zero out defeated player and award death
        if let Some(target_player) = self.state.player_mut(victim_id) {
            target_player.gold = 0.0;
            target_player.alive = false;
            target_player.deaths += 1;
        }
        self.clear_campaign_assaults_for(victim_id);

        // 2. Transfer gold and award kill/assists
        let killer_final_gold = killer_gold;
        if let Some(attacker) = self.state.player_mut(conqueror_id) {
            attacker.gold += f64::from(killer_final_gold);
            attacker.kills += 1;
        }

        let mut assist_event_rewards = Vec::new();
        for (assist_id, share) in &assist_rewards {
            if let Some(p) = self.state.player_mut(*assist_id) {
                p.gold += f64::from(*share);
                p.assists += 1;
                assist_event_rewards.push((*assist_id, *share));
            }
        }

        // Clear conquest tallies for this victim
        for p in &mut self.state.players {
            p.tile_conquests.remove(&victim_id);
        }

        // 3. Emit elimination event
        self.state
            .events
            .push(crate::game::GameEvent::PlayerEliminated {
                player_id: victim_id,
                conqueror_id,
                gold_bounty: killer_final_gold,
                elimination_x: ex,
                elimination_y: ey,
                assists: assist_event_rewards,
                by_nuke,
            });
    }

    #[inline]
    pub fn add_building(&mut self, b: Building) {
        let is_ready_defense = b.kind == crate::game::BuildingKind::Bunker && !b.under_construction;
        let pos = self.buildings.partition_point(|x| x.id < b.id);
        self.buildings.insert(pos, b);
        self.building_grid.insert(
            b.tile_idx,
            b.kind,
            self.state.map.width,
            self.state.map.height,
        );
        self.building_aggregates_dirty = true;
        self.bot_sam_tiles_cache = None;
        if !b.under_construction && b.kind == crate::game::BuildingKind::Port {
            self.sea_lanes_dirty = true;
        }
        if is_ready_defense {
            self.defense_grid_dirty = true;
            self.render_defense_dirty = true;
        }
    }

    /// Set a campaign target, optionally beginning a live-alliance assault with scheduled reinforcements.
    pub fn activate_campaign_assault(
        &mut self,
        team: crate::protocol::Team,
        target_id: PlayerId,
        preserve_relation: bool,
        include_allies: bool,
        reinforcement: Option<(f64, u32)>,
        hold_last_tile: bool,
    ) -> usize {
        let Some(target) = self.state.player(target_id) else {
            return 0;
        };
        if !target.alive || target.tile_count == 0 || target.team == Some(team) {
            return 0;
        }
        if hold_last_tile && reinforcement.is_none() {
            log::warn!("[CAMPAIGN] holding a final tile requires scheduled reinforcement settings");
            return 0;
        }

        if let Some((ratio, seconds)) = reinforcement {
            if !ratio.is_finite() || !(1.0..=10.0).contains(&ratio) || !(1..=600).contains(&seconds)
            {
                log::warn!("[CAMPAIGN] rejected invalid assault reinforcement settings");
                return 0;
            }
            if let Some(assault) = self
                .campaign_assault
                .as_mut()
                .filter(|assault| assault.root_target == target_id && assault.team == team)
            {
                assault.hold_last_tile = hold_last_tile;
                return assault.attacker_ids.len();
            }
        } else if include_allies {
            log::warn!("[CAMPAIGN] ally-wide assault requires reinforcement settings");
            return 0;
        }

        let mut attackers: Vec<PlayerId> = if reinforcement.is_some() {
            self.campaign_assault_force_ids
                .iter()
                .copied()
                .filter(|id| {
                    self.state.player(*id).is_some_and(|player| {
                        player.alive && player.tile_count > 0 && player.team == Some(team)
                    })
                })
                .collect()
        } else {
            self.state
                .players
                .iter()
                .filter(|player| {
                    player.alive
                        && player.tile_count > 0
                        && player.id != target_id
                        && player.team == Some(team)
                        && self.campaign_faction_ids.contains_key(&player.id)
                })
                .map(|player| player.id)
                .collect()
        };
        attackers.sort_unstable();
        let mut reserve_spawned = false;
        if attackers.is_empty() && reinforcement.is_some() {
            if let Some(reserve_id) = self.spawn_campaign_assault_reserve(team, target_id) {
                attackers.push(reserve_id);
                reserve_spawned = true;
            }
        }
        if attackers.is_empty() {
            return 0;
        }

        let targets = self.resolve_campaign_assault_targets(target_id, include_allies);
        if targets.is_empty() {
            return 0;
        }
        self.break_campaign_assault_alliances(&attackers, &targets, preserve_relation);
        for attacker_id in &attackers {
            self.campaign_assault_targets
                .insert(*attacker_id, targets.clone());
        }

        if let Some((capacity_ratio, interval_seconds)) = reinforcement {
            let interval_ticks = (f64::from(interval_seconds) * 1000.0
                / f64::from(self.state.config.tick_rate_ms.max(1.0)))
            .ceil()
            .max(1.0) as u64;
            self.campaign_assault = Some(CampaignAssaultState {
                root_target: target_id,
                team,
                include_allies,
                preserve_relation,
                hold_last_tile,
                capacity_ratio,
                interval_ticks,
                next_tick: self.state.tick.saturating_add(interval_ticks),
                phase_index: 0,
                attacker_ids: attackers.clone(),
                target_ids: targets,
                wave_index: 0,
                cycle_released_troops: std::collections::HashMap::new(),
                target_last_dispatched_wave: std::collections::HashMap::new(),
                focus_targets: std::collections::HashMap::new(),
                reserve_spawned,
            });
            self.advance_campaign_assault_phase();
            self.run_campaign_assault_cycle(true);
        }
        attackers.len()
    }

    fn resolve_campaign_assault_targets(
        &self,
        root_id: PlayerId,
        include_allies: bool,
    ) -> Vec<PlayerId> {
        let Some(root) = self.state.player(root_id) else {
            return Vec::new();
        };
        if !root.alive || root.tile_count == 0 {
            return Vec::new();
        }
        let mut targets = vec![root_id];
        if include_allies {
            for ally in self
                .state
                .players
                .iter()
                .filter(|ally| ally.id != root_id && ally.alive && ally.tile_count > 0)
            {
                let root_lists_ally = root.alliances.contains(&ally.id);
                let ally_lists_root = ally.alliances.contains(&root_id);
                let relation = self.campaign_relations.get(&ally.id);
                let mutual_alliance = root_lists_ally && ally_lists_root;
                let is_ally = match relation {
                    Some(relation) => *relation == crate::protocol::CampaignRelation::Allied,
                    None => mutual_alliance,
                };
                if relation.is_some() && mutual_alliance != is_ally {
                    log::debug!(
                        "[CAMPAIGN_ASSAULT] faction {} campaign relation/alliance mismatch with {} (relation={:?}, mutual={}); using campaign relation",
                        ally.id,
                        root_id,
                        relation,
                        mutual_alliance
                    );
                } else if relation.is_none() && root_lists_ally != ally_lists_root {
                    log::debug!(
                        "[CAMPAIGN_ASSAULT] non-campaign faction {} has a one-sided alliance with {}; excluding",
                        ally.id,
                        root_id
                    );
                }
                if is_ally {
                    targets.push(ally.id);
                }
            }
        }
        targets.sort_unstable();
        targets.dedup();
        targets
    }

    fn break_campaign_assault_alliances(
        &mut self,
        attackers: &[PlayerId],
        targets: &[PlayerId],
        preserve_relation: bool,
    ) {
        self.alliances_proposed.retain(|proposal| {
            !((attackers.contains(&proposal.proposer) && targets.contains(&proposal.target))
                || (targets.contains(&proposal.proposer) && attackers.contains(&proposal.target)))
        });
        for attacker_id in attackers {
            if !preserve_relation {
                self.campaign_relations
                    .insert(*attacker_id, crate::protocol::CampaignRelation::Enemy);
                self.campaign_support_next_tick.remove(attacker_id);
            }
            for target_id in targets {
                if let Some(attacker) = self.state.player_mut(*attacker_id) {
                    attacker.alliances.retain(|id| id != target_id);
                    attacker.alliance_timers.remove(target_id);
                }
                if let Some(target) = self.state.player_mut(*target_id) {
                    target.alliances.retain(|id| id != attacker_id);
                    target.alliance_timers.remove(attacker_id);
                }
            }
        }
    }

    fn spawn_campaign_assault_reserve(
        &mut self,
        team: crate::protocol::Team,
        target_id: PlayerId,
    ) -> Option<PlayerId> {
        use crate::player::Player;
        use wyrand::WyRand;

        let id = self.state.next_free_player_id()?;
        let (cx, cy) = self.state.player(target_id).map_or(
            (self.state.map.width / 2, self.state.map.height / 2),
            |target| {
                if target.tile_count == 0 {
                    (self.state.map.width / 2, self.state.map.height / 2)
                } else {
                    (
                        (target.sum_x / u64::from(target.tile_count)) as u32,
                        (target.sum_y / u64::from(target.tile_count)) as u32,
                    )
                }
            },
        );
        let spawn = self
            .nearest_free_land(cx, cy)
            .or_else(|| self.find_valid_spawn(&mut WyRand::new(self.state.seed ^ u64::from(id))))?;
        let (name, color) = if team == crate::protocol::Team::Red {
            ("Roman Reserve", [0.85, 0.12, 0.12])
        } else {
            ("Campaign Reserve", [0.18, 0.42, 0.82])
        };
        let mut reserve = Player::new_bot(id, name.to_string(), color, &self.state.config);
        reserve.team = Some(team);
        self.state.spawn_player(reserve, spawn.0, spawn.1);

        let mut faction_id = "roman_reserve".to_string();
        let mut suffix = 2;
        while self
            .campaign_faction_ids
            .values()
            .any(|value| value == &faction_id)
        {
            faction_id = format!("roman_reserve_{suffix}");
            suffix += 1;
        }
        self.campaign_faction_ids.insert(id, faction_id);
        self.campaign_assault_force_ids.insert(id);
        self.campaign_relations
            .insert(id, crate::protocol::CampaignRelation::Enemy);
        self.campaign_avatars
            .insert(id, "roman_legionary".to_string());
        log::info!("[CAMPAIGN] summoned {name} for the final assault");
        Some(id)
    }

    fn update_campaign_assault_targets(&mut self) -> bool {
        let Some(assault) = self.campaign_assault.clone() else {
            return false;
        };
        let targets =
            self.resolve_campaign_assault_targets(assault.root_target, assault.include_allies);
        if targets.is_empty() {
            self.clear_campaign_assault();
            return false;
        }

        let mut attackers: Vec<PlayerId> = assault
            .attacker_ids
            .iter()
            .copied()
            .filter(|id| {
                self.state.player(*id).is_some_and(|player| {
                    player.alive && player.tile_count > 0 && player.team == Some(assault.team)
                })
            })
            .collect();
        attackers.sort_unstable();
        let mut reserve_spawned = assault.reserve_spawned;
        if attackers.is_empty() && !reserve_spawned {
            if let Some(id) = self.spawn_campaign_assault_reserve(assault.team, assault.root_target)
            {
                attackers.push(id);
                reserve_spawned = true;
            }
        }
        if attackers.is_empty() {
            return false;
        }

        self.break_campaign_assault_alliances(&attackers, &targets, assault.preserve_relation);
        for old_id in &assault.attacker_ids {
            self.campaign_assault_targets.remove(old_id);
        }
        for attacker_id in &attackers {
            self.campaign_assault_targets
                .insert(*attacker_id, targets.clone());
        }
        let live_attacker_ids: std::collections::HashSet<_> = attackers.iter().copied().collect();
        let live_target_ids: std::collections::HashSet<_> = targets.iter().copied().collect();
        if let Some(active) = &mut self.campaign_assault {
            active.attacker_ids = attackers;
            active.target_ids = targets;
            active.reserve_spawned = reserve_spawned;
            active
                .cycle_released_troops
                .retain(|id, _| live_attacker_ids.contains(id));
            active
                .target_last_dispatched_wave
                .retain(|id, _| live_target_ids.contains(id));
        }
        true
    }

    fn advance_campaign_assault_phase(&mut self) {
        if let Some(assault) = &mut self.campaign_assault {
            if assault.phase_index >= CAMPAIGN_ASSAULT_PHASES {
                assault.phase_index = 1;
                assault.cycle_released_troops.clear();
            } else {
                assault.phase_index = assault.phase_index.saturating_add(1);
            }
        }
    }

    fn update_campaign_assault_force_caps(&mut self, refill: bool) {
        let Some(assault) = self.campaign_assault.clone() else {
            return;
        };
        let enemy_capacity: f64 = assault
            .target_ids
            .iter()
            .filter_map(|id| self.state.player(*id))
            .filter(|player| player.alive && player.tile_count > 0)
            .map(|player| player.max_troops.max(0.0))
            .sum();
        let forces: Vec<PlayerId> = assault
            .attacker_ids
            .iter()
            .copied()
            .filter(|id| {
                self.state.player(*id).is_some_and(|player| {
                    player.alive && player.tile_count > 0 && player.team == Some(assault.team)
                })
            })
            .collect();
        if forces.is_empty() {
            return;
        }
        let per_force = enemy_capacity * assault.capacity_ratio / forces.len() as f64;
        for id in &forces {
            let deployed = self
                .attacks
                .iter()
                .filter(|attack| attack.owner_id == *id && attack.troops.is_finite())
                .map(|attack| attack.troops.max(0.0))
                .sum::<f64>()
                + self
                    .fleets
                    .iter()
                    .filter(|fleet| fleet.owner_id == *id && fleet.troops.is_finite())
                    .map(|fleet| fleet.troops.max(0.0))
                    .sum::<f64>();
            // Launched troops are committed; a smaller live target set only reduces reserves.
            let standing_cap = (per_force - deployed).max(0.0);
            self.campaign_assault_troop_caps.insert(*id, standing_cap);
            if let Some(player) = self.state.player_mut(*id) {
                player.max_troops = standing_cap;
                player.troops = if refill {
                    standing_cap
                } else {
                    player.troops.min(standing_cap)
                };
            }
        }
    }

    fn has_campaign_land_front(&self, attacker_id: PlayerId, target_id: PlayerId) -> bool {
        let Some(attacker) = self.state.player(attacker_id) else {
            return false;
        };
        let map = &self.state.map;
        let width = map.width;
        attacker.border_tiles.ones().any(|tile| {
            let (x, y) = (tile % width, tile / width);
            let mut found = false;
            map.for_each_neighbor(x, y, |nx, ny| {
                if map.owner_id(nx, ny) == target_id && map.terrain[map.ref_id(nx, ny)].is_land() {
                    found = true;
                }
            });
            found
        })
    }

    fn dispatch_campaign_assault_wave(&mut self) {
        let Some(assault) = self.campaign_assault.clone() else {
            return;
        };
        let min_attack = self.state.config.attack_cost_neutral;
        if !min_attack.is_finite() || min_attack <= 0.0 {
            return;
        }

        let target_snapshots: Vec<_> = assault
            .target_ids
            .iter()
            .filter_map(|target_id| {
                let target = self.state.player(*target_id)?;
                (target.alive && target.tile_count > 0).then(|| {
                    (
                        *target_id,
                        target.tile_count,
                        if target.max_troops.is_finite() {
                            target.max_troops.max(min_attack)
                        } else {
                            min_attack
                        },
                        target.border_tiles.clone(),
                    )
                })
            })
            .collect();
        let target_ids: std::collections::HashSet<_> =
            target_snapshots.iter().map(|(id, _, _, _)| *id).collect();
        let attacker_ids: std::collections::HashSet<_> =
            assault.attacker_ids.iter().copied().collect();
        let commitments = self
            .attacks
            .iter()
            .map(|attack| {
                (
                    attack.owner_id,
                    attack.target_owner,
                    attack.troops,
                    attack.retreating,
                )
            })
            .chain(self.fleets.iter().map(|fleet| {
                (
                    fleet.owner_id,
                    fleet.target_owner,
                    fleet.troops,
                    fleet.retreating,
                )
            }));
        let mut projected_incoming =
            aggregate_campaign_assault_incoming(&attacker_ids, &target_ids, commitments);

        let force_count = assault
            .attacker_ids
            .iter()
            .filter(|id| {
                self.state.player(**id).is_some_and(|player| {
                    player.alive && player.tile_count > 0 && player.team == Some(assault.team)
                })
            })
            .count();
        if force_count == 0 {
            return;
        }
        let enemy_capacity: f64 = target_snapshots
            .iter()
            .filter_map(|(id, _, _, _)| self.state.player(*id))
            .map(|player| player.max_troops.max(0.0))
            .sum();
        let per_force = enemy_capacity * assault.capacity_ratio / force_count as f64;
        if !per_force.is_finite() || per_force <= 0.0 {
            return;
        }
        let phase = assault.phase_index.clamp(1, CAMPAIGN_ASSAULT_PHASES);
        let phase_release_limit =
            per_force * (f64::from(phase) / f64::from(CAMPAIGN_ASSAULT_PHASES));
        let mut cycle_released_troops = assault.cycle_released_troops.clone();
        cycle_released_troops.retain(|id, _| attacker_ids.contains(id));
        let mut target_last_dispatched_wave = assault.target_last_dispatched_wave.clone();
        target_last_dispatched_wave.retain(|id, _| target_ids.contains(id));

        let mut routeable_targets = std::collections::HashSet::new();
        let mut eligible_targets = std::collections::HashSet::new();
        let mut attempted_targets = std::collections::HashSet::new();
        let mut dispatched_targets = std::collections::HashSet::new();
        let mut held_targets = std::collections::HashSet::new();

        let mut intent_index = 0u32;
        let mut focus_targets =
            std::collections::HashMap::with_capacity(assault.attacker_ids.len());
        for attacker_id in &assault.attacker_ids {
            let Some((standing_troops, attacker_border)) = self
                .state
                .player(*attacker_id)
                .filter(|player| player.alive && player.tile_count > 0)
                .map(|player| (player.troops.max(0.0), player.border_tiles.clone()))
            else {
                continue;
            };
            let remaining_phase_budget = (phase_release_limit
                - cycle_released_troops
                    .get(attacker_id)
                    .copied()
                    .unwrap_or_default())
            .max(0.0);
            let available = standing_troops.min(remaining_phase_budget);
            let mut reachable = Vec::new();
            for (target_id, tile_count, defense_capacity, target_border) in &target_snapshots {
                if assault.hold_last_tile && *target_id == assault.root_target && *tile_count <= 1 {
                    held_targets.insert(*target_id);
                    continue;
                }
                if *target_id == *attacker_id {
                    continue;
                }
                let Some(target_tile) = target_border
                    .first_one_from(self.state.tick.wrapping_add(u64::from(*attacker_id)) as u32)
                else {
                    continue;
                };
                let route = if self.has_campaign_land_front(*attacker_id, *target_id) {
                    None
                } else {
                    match crate::warp_fleet::resolve_fleet_route(
                        &self.state.map,
                        &self.water,
                        &mut self.path_scratch,
                        *attacker_id,
                        (*target_id, target_tile),
                        &attacker_border,
                        Some(target_border),
                    ) {
                        Ok(route) => Some(route),
                        Err(_) => continue,
                    }
                };
                routeable_targets.insert(*target_id);
                reachable.push(CampaignAssaultRouteCandidate {
                    target_id: *target_id,
                    target_tile,
                    route,
                    committed_troops: projected_incoming
                        .get(target_id)
                        .copied()
                        .unwrap_or_default(),
                    defense_capacity: *defense_capacity,
                    tie_rank: 0,
                });
            }
            if !available.is_finite() || available < min_attack {
                continue;
            }
            for candidate in &reachable {
                eligible_targets.insert(candidate.target_id);
            }
            if reachable.is_empty() {
                continue;
            }

            let affordable = (available / min_attack).floor() as usize;
            let reachable = prioritize_campaign_assault_targets(
                reachable,
                affordable,
                assault.wave_index,
                &target_last_dispatched_wave,
            );
            if reachable.is_empty() {
                continue;
            }
            let allocation_inputs: Vec<_> = reachable
                .iter()
                .map(|candidate| (candidate.committed_troops, candidate.defense_capacity))
                .collect();
            let shares =
                allocate_campaign_assault_shares(available, min_attack, &allocation_inputs);
            if shares.len() != reachable.len() {
                continue;
            }

            let mut remaining = available;
            let mut assigned_targets = Vec::with_capacity(reachable.len());
            let last_index = reachable.len() - 1;
            for (index, (candidate, planned_share)) in reachable.into_iter().zip(shares).enumerate()
            {
                let share = if index == last_index {
                    remaining
                } else {
                    planned_share.min(remaining)
                };
                remaining = (remaining - share).max(0.0);
                if share < min_attack {
                    continue;
                }

                let troops_before = self
                    .state
                    .player(*attacker_id)
                    .map_or(0.0, |player| player.troops);
                attempted_targets.insert(candidate.target_id);
                if let Some(route) = candidate.route {
                    self.apply_campaign_assault_fleet_with_route(
                        *attacker_id,
                        candidate.target_tile,
                        Some(share),
                        route,
                    );
                } else {
                    self.apply_campaign_assault_attack(
                        *attacker_id,
                        &crate::protocol::AttackIntent {
                            target_owner: candidate.target_id,
                            troops: Some(share),
                        },
                        intent_index,
                    );
                }
                intent_index = intent_index.wrapping_add(1);
                let troops_after = self
                    .state
                    .player(*attacker_id)
                    .map_or(troops_before, |player| player.troops);
                let launched = (troops_before - troops_after).max(0.0);
                if launched <= 0.0 {
                    continue;
                }
                *cycle_released_troops.entry(*attacker_id).or_default() += launched;
                target_last_dispatched_wave.insert(candidate.target_id, assault.wave_index);
                dispatched_targets.insert(candidate.target_id);
                let projected = projected_incoming.entry(candidate.target_id).or_default();
                *projected = (*projected + launched).min(f64::MAX / 4.0);
                assigned_targets.push((
                    candidate.target_id,
                    *projected / candidate.defense_capacity,
                    candidate.tie_rank,
                ));
            }
            let focus = assigned_targets.into_iter().min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.0.cmp(&b.0))
            });
            if let Some((target_id, _, _)) = focus {
                focus_targets.insert(*attacker_id, target_id);
            }
        }

        for (target_id, _, _, _) in &target_snapshots {
            if dispatched_targets.contains(target_id) {
                continue;
            }
            let reason = if held_targets.contains(target_id) {
                "the final tile is held until the player chooses resistance"
            } else if !routeable_targets.contains(target_id) {
                "no legal land or sea route"
            } else if !eligible_targets.contains(target_id) {
                "every force is below its current phase minimum"
            } else if attempted_targets.contains(target_id) {
                "launch was rejected before troops left the force"
            } else {
                "deferred by the phase budget or fair target rotation"
            };
            log::debug!(
                "[CAMPAIGN_ASSAULT] wave={} phase={}/{} target={} omitted: {}; it remains eligible next phase",
                assault.wave_index,
                phase,
                CAMPAIGN_ASSAULT_PHASES,
                target_id,
                reason
            );
        }

        if let Some(active) = &mut self.campaign_assault {
            active.focus_targets = focus_targets;
            active.cycle_released_troops = cycle_released_troops;
            active.target_last_dispatched_wave = target_last_dispatched_wave;
            active.wave_index = active.wave_index.wrapping_add(1);
        }
    }

    fn run_campaign_assault_cycle(&mut self, launch_wave: bool) {
        if !self.update_campaign_assault_targets() {
            return;
        }
        self.update_campaign_assault_force_caps(true);
        if launch_wave {
            self.dispatch_campaign_assault_wave();
            self.update_campaign_assault_force_caps(false);
        }
    }

    pub(crate) fn update_campaign_assault(&mut self) {
        let Some(assault) = self.campaign_assault.as_ref() else {
            return;
        };
        if self.state.tick < assault.next_tick {
            return;
        }
        let interval = assault.interval_ticks.max(1);
        if let Some(active) = &mut self.campaign_assault {
            active.next_tick = self.state.tick.saturating_add(interval);
        }
        self.advance_campaign_assault_phase();
        self.run_campaign_assault_cycle(true);
    }

    fn clear_campaign_assault(&mut self) {
        if let Some(assault) = self.campaign_assault.take() {
            for attacker_id in assault.attacker_ids {
                self.campaign_assault_targets.remove(&attacker_id);
                self.campaign_assault_troop_caps.remove(&attacker_id);
            }
        }
    }

    fn clear_campaign_assaults_for(&mut self, player_id: PlayerId) {
        if self
            .campaign_assault
            .as_ref()
            .is_some_and(|assault| assault.root_target == player_id)
        {
            self.clear_campaign_assault();
        }
        self.campaign_assault_troop_caps.remove(&player_id);
        self.campaign_assault_targets
            .retain(|attacker_id, target_ids| {
                if *attacker_id == player_id {
                    return false;
                }
                target_ids.retain(|target_id| *target_id != player_id);
                !target_ids.is_empty()
            });
    }

    #[inline]
    pub fn add_attack(&mut self, a: AttackExecution) {
        let pos = self.attacks.partition_point(|x| x.id < a.id);
        self.attacks.insert(pos, a);
        self.ai_attack_index_dirty = true;
    }

    pub(crate) fn cache_bot_route(
        &mut self,
        player_id: PlayerId,
        target_tile: u32,
        route: crate::warp_fleet::FleetRoute,
    ) {
        self.bot_route_cache.push((player_id, target_tile, route));
    }

    pub(crate) fn take_bot_route(
        &mut self,
        player_id: PlayerId,
        target_tile: u32,
    ) -> Option<crate::warp_fleet::FleetRoute> {
        let index = self
            .bot_route_cache
            .iter()
            .position(|(pid, tile, _)| *pid == player_id && *tile == target_tile)?;
        Some(self.bot_route_cache.swap_remove(index).2)
    }

    #[inline]
    pub fn add_fleet(&mut self, f: WarpFleet) {
        let pos = self.fleets.partition_point(|x| x.id < f.id);
        self.fleets.insert(pos, f);
    }
}

mod snapshot;
mod spawn;
pub use spawn::HumanSpawn;
mod tick;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameEvent, GameState};
    use crate::game_config::GameConfig;
    use crate::player::{Player, PlayerType};
    use crate::water_components::WaterComponents;

    #[test]
    fn campaign_building_policy_is_closed_only_for_tutorial_matches() {
        let mut custom_state = GameState::new(1, 4, 4, GameConfig::default());
        custom_state.config.tutorial = false;
        let mut custom = SowEngine::new(custom_state, WaterComponents::default());
        assert!(custom.campaign_unlocks.is_none());
        assert!(
            crate::campaign::CampaignAction::ALL
                .into_iter()
                .all(|action| custom.campaign_allows_action(action))
        );
        assert!(
            crate::game::BuildingKind::ALL
                .into_iter()
                .all(|kind| custom.campaign_allows_building(kind, 1))
        );
        assert!(!custom.set_campaign_unlocks(crate::campaign::CampaignUnlocks::default()));

        let mut tutorial_state = GameState::new(1, 4, 4, GameConfig::default());
        tutorial_state.config.tutorial = true;
        let mut tutorial = SowEngine::new(tutorial_state, WaterComponents::default());
        assert!(!tutorial.campaign_allows_building(crate::game::BuildingKind::City, 1));
        assert!(!tutorial.campaign_allows_action(crate::campaign::CampaignAction::TransportFleet));

        let first_grant = crate::campaign::CampaignUnlocks {
            buildings: std::collections::HashMap::from([(crate::game::BuildingKind::City, 1)]),
            actions: vec![],
        };
        assert!(tutorial.set_campaign_unlocks(first_grant));
        assert!(tutorial.campaign_allows_building(crate::game::BuildingKind::City, 1));
        assert!(!tutorial.campaign_allows_building(crate::game::BuildingKind::Factory, 1));

        let attempted_revoke = crate::campaign::CampaignUnlocks::default();
        assert!(!tutorial.set_campaign_unlocks(attempted_revoke));
        assert!(tutorial.campaign_allows_building(crate::game::BuildingKind::City, 1));
    }

    fn assault_candidate(
        target_id: PlayerId,
        committed_troops: f64,
    ) -> CampaignAssaultRouteCandidate {
        CampaignAssaultRouteCandidate {
            target_id,
            target_tile: target_id as u32,
            route: None,
            committed_troops,
            defense_capacity: 1_000.0,
            tie_rank: 0,
        }
    }

    #[test]
    fn campaign_assault_prioritizes_low_pressure_and_rotates_equal_targets() {
        let selected = prioritize_campaign_assault_targets(
            vec![
                assault_candidate(1, 0.0),
                assault_candidate(2, 500.0),
                assault_candidate(3, 250.0),
            ],
            2,
            0,
            &std::collections::HashMap::new(),
        );
        assert_eq!(
            selected
                .iter()
                .map(|target| target.target_id)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );

        let equal_targets = || {
            vec![
                assault_candidate(1, 0.0),
                assault_candidate(2, 0.0),
                assault_candidate(3, 0.0),
            ]
        };
        let rotated: Vec<_> = (0..3)
            .map(|wave| {
                prioritize_campaign_assault_targets(
                    equal_targets(),
                    1,
                    wave,
                    &std::collections::HashMap::new(),
                )
                .first()
                .unwrap()
                .target_id
            })
            .collect();
        assert_eq!(rotated, vec![1, 2, 3]);

        let mut served = std::collections::HashMap::from([(1, 7), (2, 4)]);
        let waiting = prioritize_campaign_assault_targets(equal_targets(), 1, 8, &served);
        assert_eq!(
            waiting[0].target_id, 3,
            "never-served allies get first turn"
        );
        served.insert(3, 8);
        let waiting = prioritize_campaign_assault_targets(equal_targets(), 1, 9, &served);
        assert_eq!(
            waiting[0].target_id, 2,
            "the least-recently-served ally rotates next"
        );
    }

    #[test]
    fn boudica_assault_uses_the_authored_alliance_and_east_anglia_map() {
        use crate::game::{GamePhase, GameState};
        use crate::game_config::{GameConfig, ScriptedSpawn};
        use crate::map::MapTile;
        use crate::player::{Civilization, Leader, Player};
        use crate::protocol::{CampaignRelation, Team};
        use crate::water_components::WaterComponents;
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct BoudicaRoster {
            map: String,
            player_spawn: [u32; 2],
            factions: Vec<RosterFaction>,
        }

        #[derive(Deserialize)]
        struct RosterFaction {
            id: String,
            name: String,
            x: u32,
            y: u32,
            starting_troops: f64,
            relation: String,
            #[serde(default)]
            team: Option<String>,
            #[serde(default)]
            can_request_alliance: bool,
            #[serde(default)]
            assault_force: bool,
            #[serde(default)]
            iq: Option<u32>,
            #[serde(default)]
            leader: Option<String>,
            #[serde(default)]
            civ: Option<String>,
        }

        let roster: BoudicaRoster =
            serde_json::from_slice(include_bytes!("../../../assets/campaign/boudica.json"))
                .expect("authored Boudica campaign roster");
        assert_eq!(roster.map, "eastanglia");
        let map = crate::map_file::parse(include_bytes!("../../../assets/maps/eastanglia/map.bin"))
            .expect("packaged East Anglia terrain");

        let selected = [
            "trinovantes",
            "legio_xiv_gemina",
            "legio_xx_valeria",
            "suetonius_paulinus",
            "suetonius_auxilia",
        ];
        let mut config = GameConfig::default();
        config.map_width = map.width;
        config.map_height = map.height;
        config.tick_rate_ms = 100.0;
        config.attack_cost_neutral = 1.0;
        config.scripted_spawns = roster
            .factions
            .into_iter()
            .filter(|faction| selected.contains(&faction.id.as_str()))
            .map(|faction| ScriptedSpawn {
                name: faction.name,
                x: faction.x,
                y: faction.y,
                color: [0.8, 0.2, 0.2],
                team: match faction.team.as_deref() {
                    Some("Red") => Some(Team::Red),
                    Some("Blue") => Some(Team::Blue),
                    _ => None,
                },
                leader: match faction.leader.as_deref() {
                    Some("Boudica") => Leader::Boudica,
                    _ => Leader::Caesar,
                },
                civilization: match faction.civ.as_deref() {
                    Some("Gallic Tribes") => Civilization::Gallic,
                    Some("Iceni Kingdom") => Civilization::Iceni,
                    _ => Civilization::Rome,
                },
                troops: Some(faction.starting_troops),
                iq: faction.iq,
                campaign_avatar: None,
                campaign_support_interval_seconds: None,
                campaign_gold_loot_bonus: None,
                campaign_gold_loot_override: None,
                campaign_alliance_group: None,
                campaign_relation: Some(match faction.relation.as_str() {
                    "allied" => CampaignRelation::Allied,
                    "enemy" => CampaignRelation::Enemy,
                    _ => CampaignRelation::Neutral,
                }),
                campaign_faction_id: Some(faction.id),
                campaign_can_request_alliance: Some(faction.can_request_alliance),
                campaign_assault_force: faction.assault_force,
            })
            .collect();
        assert_eq!(config.scripted_spawns.len(), selected.len());

        let mut state = GameState::new(20261009, map.width, map.height, config.clone());
        state.phase = GamePhase::Playing;
        state.map.terrain = map.terrain.into_iter().map(MapTile::from_byte).collect();
        let boudica = Player::new_human(1, "Boudica".into(), [0.2, 0.5, 1.0], &config);
        state.spawn_player(boudica, roster.player_spawn[0], roster.player_spawn[1]);
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = SowEngine::new(state, water);
        engine.spawn_scripted();

        let faction_id = |name: &str| {
            *engine
                .campaign_faction_ids
                .iter()
                .find_map(|(id, faction)| (faction == name).then_some(id))
                .expect("campaign faction spawned")
        };
        let trinovantes = faction_id("trinovantes");
        let roman_forces: Vec<_> = selected
            .iter()
            .filter(|id| **id != "trinovantes")
            .map(|id| faction_id(id))
            .collect();
        assert_eq!(roman_forces.len(), 4);
        assert!(
            engine
                .state
                .player(1)
                .unwrap()
                .alliances
                .contains(&trinovantes)
        );
        assert!(
            engine
                .state
                .player(trinovantes)
                .unwrap()
                .alliances
                .contains(&1)
        );

        assert_eq!(
            engine.campaign_assault_force_ids,
            roman_forces.iter().copied().collect()
        );
        assert_eq!(
            engine.activate_campaign_assault(Team::Red, 1, false, true, Some((2.0, 3)), false,),
            4
        );
        let targets = &engine.campaign_assault.as_ref().unwrap().target_ids;
        assert_eq!(targets, &vec![1, trinovantes]);
        let routes: Vec<_> = roman_forces
            .iter()
            .map(|attacker_id| {
                let attacker = engine.state.player(*attacker_id).unwrap();
                if engine.has_campaign_land_front(*attacker_id, trinovantes) {
                    return (*attacker_id, "land front".to_string());
                }
                let target = engine.state.player(trinovantes).unwrap();
                let tile = target.border_tiles.first_one_from(0).unwrap();
                let route = crate::warp_fleet::resolve_fleet_route(
                    &engine.state.map,
                    &engine.water,
                    &mut engine.path_scratch,
                    *attacker_id,
                    (trinovantes, tile),
                    &attacker.border_tiles,
                    Some(&target.border_tiles),
                );
                (
                    *attacker_id,
                    route.map_or_else(|error| error.to_string(), |_| "sea route".into()),
                )
            })
            .collect();
        let has_route = routes
            .iter()
            .any(|(_, route)| route == "land front" || route == "sea route");
        let dispatched = engine
            .attacks
            .iter()
            .any(|attack| attack.target_owner == trinovantes)
            || engine
                .fleets
                .iter()
                .any(|fleet| fleet.target_owner == trinovantes);
        assert_eq!(
            dispatched, has_route,
            "the authored ally must be hit whenever a launch route exists; routes={routes:?}"
        );
    }

    #[test]
    fn campaign_assault_pressure_counts_attacks_and_fleets_but_not_retreats() {
        let attackers = std::collections::HashSet::from([1]);
        let targets = std::collections::HashSet::from([8, 9]);
        let incoming = aggregate_campaign_assault_incoming(
            &attackers,
            &targets,
            [
                (1, 8, 250.0, false),
                (1, 8, 500.0, true),
                (1, 9, 300.0, false),
                (2, 8, 900.0, false),
            ],
        );

        assert_eq!(incoming.get(&8), Some(&250.0));
        assert_eq!(incoming.get(&9), Some(&300.0));
    }

    #[test]
    fn campaign_assault_shares_equalize_projected_defense_pressure() {
        let shares =
            allocate_campaign_assault_shares(500.0, 100.0, &[(100.0, 1_000.0), (300.0, 1_000.0)]);

        assert_eq!(shares.len(), 2);
        assert!((shares[0] - 350.0).abs() < 0.01);
        assert!((shares[1] - 150.0).abs() < 0.01);
        assert!(((100.0 + shares[0]) / 1_000.0 - (300.0 + shares[1]) / 1_000.0).abs() < 0.0001);
    }

    #[test]
    fn campaign_assault_shares_weight_targets_by_defense_capacity() {
        let shares =
            allocate_campaign_assault_shares(1_000.0, 100.0, &[(0.0, 1_000.0), (0.0, 500.0)]);

        assert_eq!(shares.len(), 2);
        assert!((shares.iter().sum::<f64>() - 1_000.0).abs() < 0.01);
        assert!((shares[0] / 1_000.0 - shares[1] / 500.0).abs() < 0.0001);
    }

    #[test]
    fn elimination_bounty_is_fixed_by_victim_type_and_keeps_campaign_bonus() {
        let reward = |victim_type: PlayerType, tick: u64, loot_bonus: Option<u32>| {
            let config = GameConfig::default();
            let mut state = GameState::new(1, 4, 4, config.clone());
            state.tick = tick;
            let mut human = Player::new_human(1, "Boudica".into(), [1.0; 3], &config);
            human.gold = 100.0;
            state.register_player(human);
            let mut victim = Player::new_human(2, "Outpost".into(), [0.5; 3], &config);
            victim.player_type = victim_type;
            state.register_player(victim);
            let mut engine = SowEngine::new(state, WaterComponents::default());
            if let Some(bonus) = loot_bonus {
                engine.campaign_gold_loot_bonus.insert(2, bonus);
            }
            engine.eliminate_player(2, 1, 0, 0, false);
            let bounty = engine
                .state
                .events
                .iter()
                .find_map(|event| match event {
                    GameEvent::PlayerEliminated { gold_bounty, .. } => Some(*gold_bounty),
                    _ => None,
                })
                .unwrap();
            (engine.state.player(1).unwrap().gold, bounty)
        };

        assert_eq!(reward(PlayerType::Bot, 100, None), (125.0, 25));
        assert_eq!(reward(PlayerType::Bot, 10_000, None), (125.0, 25));
        assert_eq!(reward(PlayerType::Nation, 1_000, None), (175.0, 75));
        assert_eq!(reward(PlayerType::Human, 10_000, None), (250.0, 150));
        assert_eq!(reward(PlayerType::Bot, 100, Some(125)), (250.0, 150));
    }

    #[test]
    fn elimination_assist_payments_are_integer_exact_and_conserve_the_bounty() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 4, 4, config.clone());
        for id in 1..=5 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.gold = 0.0;
            state.register_player(player);
        }
        state.player_mut(1).unwrap().tile_conquests.insert(4, 5);
        state.player_mut(2).unwrap().tile_conquests.insert(4, 1);
        state.player_mut(3).unwrap().tile_conquests.insert(4, 1);
        state.player_mut(4).unwrap().tile_conquests.insert(4, 100);
        state.player_mut(4).unwrap().player_type = PlayerType::Bot;
        state.player_mut(5).unwrap().alive = false;
        state
            .player_mut(5)
            .unwrap()
            .tile_conquests
            .insert(4, 10_000);
        let mut engine = SowEngine::new(state, WaterComponents::default());

        engine.eliminate_player(4, 1, 0, 0, false);

        assert_eq!(engine.state.player(1).unwrap().gold, 12.0);
        assert_eq!(engine.state.player(2).unwrap().gold, 7.0);
        assert_eq!(engine.state.player(3).unwrap().gold, 6.0);
        assert_eq!(engine.state.player(4).unwrap().gold, 0.0);
        assert_eq!(engine.state.player(5).unwrap().gold, 0.0);
        assert!(matches!(
            engine.state.events.last(),
            Some(GameEvent::PlayerEliminated {
                gold_bounty: 12,
                assists,
                ..
            }) if assists == &vec![(2, 7), (3, 6)]
        ));
        assert_eq!(
            engine.state.player(1).unwrap().gold
                + engine.state.player(2).unwrap().gold
                + engine.state.player(3).unwrap().gold,
            25.0
        );
    }

    #[test]
    fn test_spawn_ai_nations() {
        let config = GameConfig {
            map_name: crate::maps::DEFAULT_MAP_KEY.to_string(),
            map_width: 1000,
            map_height: 800,
            ..Default::default()
        };
        let mut state = GameState::new(42, 1000, 800, config.clone());
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(0, 0);
        assert_eq!(engine.state.players.len(), 0);

        let mut state = GameState::new(42, 1000, 800, config.clone());
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        state.map_spawns = vec![
            crate::map_file::MapSpawn {
                name: "Testland".to_string(),
                flag: "xx".to_string(),
                x: 10,
                y: 10,
            },
            crate::map_file::MapSpawn {
                name: "Testland".to_string(),
                flag: "xx".to_string(),
                x: 20,
                y: 20,
            },
        ];

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(2, 0);
        assert_eq!(engine.state.players.len(), 2);
        assert!(
            engine.state.players.iter().all(|p| p.name == "Testland"),
            "anchored spawns use map.bin spawn names"
        );
        assert_eq!(
            engine.state.map.owner_id(10, 10),
            engine.state.players[0].id
        );
        assert_eq!(
            engine.state.map.owner_id(20, 20),
            engine.state.players[1].id
        );

        let mut state = GameState::new(42, 1000, 800, config);
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(3, 0);
        assert_eq!(engine.state.players.len(), 3);
    }

    fn exercise_roster_on_map(bytes: &[u8], map_name: &str) -> Vec<(String, (u32, u32))> {
        use crate::map_file::{MapRosterEntry, MapRosterPreset, MapRosterRole};

        let mut map = crate::map_file::parse(bytes).unwrap();
        let mut entries = Vec::new();
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        let mut positions = Vec::new();
        let far_enough = |x: u32, y: u32, used: &[(u32, u32)]| {
            used.iter().all(|&(other_x, other_y)| {
                let dx = x as i64 - other_x as i64;
                let dy = y as i64 - other_y as i64;
                dx * dx + dy * dy > 121
            })
        };

        for (index, anchor) in map.spawns.iter().enumerate() {
            let Some(entity) = crate::geo_entities::all().find(|entity| entity.name == anchor.name)
            else {
                continue;
            };
            if anchor.x >= map.width || anchor.y >= map.height {
                continue;
            }
            let tile = (anchor.y * map.width + anchor.x) as usize;
            if !crate::map::MapTile::from_byte(map.terrain[tile]).is_land()
                || !far_enough(anchor.x, anchor.y, &positions)
                || !ids.insert(entity.id.clone())
                || !names.insert(entity.name.clone())
            {
                continue;
            }
            entries.push(MapRosterEntry {
                entity_id: entity.id.clone(),
                role: MapRosterRole::Nation,
                x: anchor.x,
                y: anchor.y,
                legacy_anchor: Some(index as u16),
            });
            positions.push((anchor.x, anchor.y));
            if entries
                .iter()
                .filter(|entry| entry.role == MapRosterRole::Nation)
                .count()
                == 2
            {
                break;
            }
        }

        let tribe_entities: Vec<_> = crate::geo_entities::all()
            .filter(|entity| {
                entity.kind == crate::geo_entities::EntityKind::Tribe
                    && !ids.contains(&entity.id)
                    && !names.contains(&entity.name)
            })
            .take(2)
            .collect();
        let mut tribe_positions = Vec::with_capacity(2);
        for y in 0..map.height {
            for x in 0..map.width {
                let tile = (y * map.width + x) as usize;
                if crate::map::MapTile::from_byte(map.terrain[tile]).is_land()
                    && far_enough(x, y, &positions)
                {
                    positions.push((x, y));
                    tribe_positions.push((x, y));
                    if tribe_positions.len() == tribe_entities.len() {
                        break;
                    }
                }
            }
            if tribe_positions.len() == tribe_entities.len() {
                break;
            }
        }
        assert_eq!(tribe_entities.len(), 2, "{map_name} needs two Atlas tribes");
        assert_eq!(
            tribe_positions.len(),
            2,
            "{map_name} needs two spaced land tiles"
        );
        for (entity, (x, y)) in tribe_entities.into_iter().zip(tribe_positions) {
            assert!(ids.insert(entity.id.clone()));
            assert!(names.insert(entity.name.clone()));
            entries.push(MapRosterEntry {
                entity_id: entity.id.clone(),
                role: MapRosterRole::Tribe,
                x,
                y,
                legacy_anchor: None,
            });
        }
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.role == MapRosterRole::Nation)
                .count(),
            2,
            "{map_name} needs two exactly linked Nation anchors"
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.role == MapRosterRole::Tribe)
                .count(),
            2,
            "{map_name} needs two on-land Atlas tribes"
        );

        let original_anchors = map.spawns.clone();
        map.default_roster = Some("historical".into());
        map.rosters = vec![MapRosterPreset {
            id: "historical".into(),
            name: "Historical".into(),
            entries: entries.clone(),
        }];
        crate::map_file::validate_rosters(&map, true).unwrap();
        let map_bytes = crate::map_file::encode(&map);
        assert_eq!(
            crate::map_file::parse(&map_bytes).unwrap().spawns,
            original_anchors
        );

        let config = GameConfig {
            map_name: map_name.into(),
            map_width: map.width,
            map_height: map.height,
            map_roster_preset: Some("historical".into()),
            nation_count: 2,
            bot_count: 2,
            ..Default::default()
        };
        let engine = initialize_match_engine(
            config,
            0x51_16_2026,
            &map_bytes,
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            map.num_land_tiles,
        );
        assert_eq!(engine.state.players.len(), 4, "{map_name}");
        let mut spawned = Vec::new();
        for player in &engine.state.players {
            let entity =
                crate::geo_entities::by_name(&player.name).expect("roster uses Atlas identities");
            let placed = entries
                .iter()
                .find(|entry| entry.entity_id == entity.id)
                .unwrap();
            assert_eq!(
                engine.state.map.owner_id(placed.x, placed.y),
                player.id,
                "{map_name}: {} must start at its authored tile",
                player.name
            );
            assert_eq!(
                player.player_type == crate::player::PlayerType::Bot,
                placed.role == MapRosterRole::Tribe
            );
            spawned.push((player.name.clone(), (placed.x, placed.y)));
        }
        spawned
    }

    #[test]
    fn world_and_regional_map_bins_spawn_map_roster_entries() {
        let world = exercise_roster_on_map(
            include_bytes!("../../../assets/maps/world/map.bin"),
            "world",
        );
        let europe = exercise_roster_on_map(
            include_bytes!("../../../assets/maps/europe/map.bin"),
            "europe",
        );
        assert_eq!(world.len(), 4);
        assert_eq!(europe.len(), 4);
    }

    #[test]
    fn map_roster_selection_is_seeded_unique_and_uses_saved_tiles() {
        fn run(seed: u64) -> Vec<(String, (u32, u32))> {
            let preset = crate::map_file::MapRosterPreset {
                id: "historical".into(),
                name: "Historical".into(),
                entries: vec![
                    crate::map_file::MapRosterEntry {
                        entity_id: "iceland".into(),
                        role: crate::map_file::MapRosterRole::Nation,
                        x: 15,
                        y: 15,
                        legacy_anchor: Some(1),
                    },
                    crate::map_file::MapRosterEntry {
                        entity_id: "ireland".into(),
                        role: crate::map_file::MapRosterRole::Nation,
                        x: 45,
                        y: 15,
                        legacy_anchor: None,
                    },
                    crate::map_file::MapRosterEntry {
                        entity_id: "united_kingdom".into(),
                        role: crate::map_file::MapRosterRole::Nation,
                        x: 75,
                        y: 15,
                        legacy_anchor: None,
                    },
                    crate::map_file::MapRosterEntry {
                        entity_id: "maya".into(),
                        role: crate::map_file::MapRosterRole::Tribe,
                        x: 115,
                        y: 15,
                        legacy_anchor: None,
                    },
                    crate::map_file::MapRosterEntry {
                        entity_id: "mapuche".into(),
                        role: crate::map_file::MapRosterRole::Tribe,
                        x: 145,
                        y: 15,
                        legacy_anchor: None,
                    },
                    crate::map_file::MapRosterEntry {
                        entity_id: "moche".into(),
                        role: crate::map_file::MapRosterRole::Tribe,
                        x: 175,
                        y: 15,
                        legacy_anchor: None,
                    },
                ],
            };
            let width = 220;
            let height = 100;
            let config = GameConfig {
                map_name: "world".into(),
                map_width: width,
                map_height: height,
                map_roster_preset: Some("historical".into()),
                nation_count: 4,
                bot_count: 2,
                ..Default::default()
            };
            let map = crate::map_file::MapFile {
                display_name: "World test".into(),
                width,
                height,
                num_land_tiles: width * height,
                spawns: vec![
                    crate::map_file::MapSpawn {
                        name: "Legacy spare".into(),
                        flag: String::new(),
                        x: 15,
                        y: 75,
                    },
                    crate::map_file::MapSpawn {
                        name: "Iceland".into(),
                        flag: "is".into(),
                        x: 45,
                        y: 75,
                    },
                ],
                geo_bounds: None,
                default_roster: Some("historical".into()),
                rosters: vec![preset.clone()],
                terrain: vec![0x80; (width * height) as usize],
            };
            let map_bytes = crate::map_file::encode(&map);
            let engine = initialize_match_engine(
                config,
                seed,
                &map_bytes,
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
                width * height,
            );
            assert_eq!(engine.state.players.len(), 6);
            let names: std::collections::HashSet<_> = engine
                .state
                .players
                .iter()
                .map(|player| player.name.as_str())
                .collect();
            assert_eq!(
                names.len(),
                engine.state.players.len(),
                "roster entries must not repeat"
            );

            engine
                .state
                .players
                .iter()
                .map(|player| {
                    let position = if let Some(entity) = crate::geo_entities::by_name(&player.name)
                    {
                        let placed = preset
                            .entries
                            .iter()
                            .find(|entry| entry.entity_id == entity.id)
                            .expect("spawned Atlas entity must come from the selected roster");
                        assert_eq!(engine.state.map.owner_id(placed.x, placed.y), player.id);
                        assert_eq!(
                            player.player_type == crate::player::PlayerType::Bot,
                            placed.role == crate::map_file::MapRosterRole::Tribe
                        );
                        (placed.x, placed.y)
                    } else {
                        assert_eq!(
                            engine.state.map.owner_id(15, 75),
                            player.id,
                            "only the missing nation slot uses the spare anchor"
                        );
                        (15, 75)
                    };
                    (player.name.clone(), position)
                })
                .collect()
        }

        assert_eq!(run(1234), run(1234));
    }

    #[test]
    fn test_ai_ids_follow_a_256_player_human_roster() {
        use crate::player::{Player, PlayerType};

        let config = GameConfig {
            map_name: crate::maps::DEFAULT_MAP_KEY.to_string(),
            map_width: 1000,
            map_height: 800,
            ..Default::default()
        };
        let mut state = GameState::new(42, 1000, 800, config.clone());
        for tile in &mut state.map.terrain {
            *tile = crate::map::MapTile::from_byte(0b1000_0000);
        }
        for id in 1..=256u16 {
            state.register_player(Player::new_human(
                id,
                format!("Human{id}"),
                [1.0, 1.0, 1.0],
                &config,
            ));
        }

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(2, 2);

        let ids: std::collections::HashSet<u16> = engine
            .state
            .players
            .iter()
            .map(|player| player.id)
            .collect();
        assert_eq!(ids.len(), 260);
        assert!(
            engine.state.players[256..]
                .iter()
                .all(|player| player.id > 256)
        );
        assert!(
            engine.state.players[256..]
                .iter()
                .any(|player| player.player_type == PlayerType::Nation)
        );
        assert_eq!(engine.state.player(256).map(|player| player.id), Some(256));
    }

    /// Regression: nations must NEVER carry a team in "Teams" mode (teams are
    /// human-only), and must be Blue in "HumansVsNations". The old code force-
    /// assigned Red/Blue to nations alternating by index — that bug must not
    /// return.
    #[test]
    fn test_nation_team_assignment_per_mode() {
        use crate::player::PlayerType;

        fn spawn_nations(mode: &str) -> Vec<crate::player::Player> {
            let config = GameConfig {
                map_name: crate::maps::DEFAULT_MAP_KEY.to_string(),
                map_width: 1000,
                map_height: 800,
                game_mode: mode.to_string(),
                ..Default::default()
            };
            let mut state = GameState::new(7, 1000, 800, config);
            for t in &mut state.map.terrain {
                *t = crate::map::MapTile::from_byte(0b1000_0000);
            }
            let mut engine = SowEngine::new(state, WaterComponents::default());
            engine.spawn_ai(4, 0);
            engine
                .state
                .players
                .iter()
                .filter(|p| p.player_type == PlayerType::Nation)
                .cloned()
                .collect()
        }

        // FFA: no teams anywhere.
        for p in spawn_nations("FFA") {
            assert!(
                p.team.is_none(),
                "FFA nation {} has a team {:?}",
                p.id,
                p.team
            );
        }
        // Teams: nations are wild (human-only teams).
        for p in spawn_nations("Teams") {
            assert!(
                p.team.is_none(),
                "Teams nation {} has a team {:?}",
                p.id,
                p.team
            );
        }
        // HumansVsNations: every nation is Blue.
        for p in spawn_nations("HumansVsNations") {
            assert_eq!(
                p.team,
                Some(crate::protocol::Team::Blue),
                "HvN nation {} wrong team {:?}",
                p.id,
                p.team
            );
        }
    }

    /// All-land 1000x800 state with the (approx) europe bbox stamped.
    fn geo_test_state(seed: u64) -> GameState {
        let config = GameConfig {
            map_name: "europe_test".to_string(),
            map_width: 1000,
            map_height: 800,
            ..Default::default()
        };
        let mut state = GameState::new(seed, 1000, 800, config);
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        state.geo_bounds = Some(crate::map_file::GeoBounds::from_degrees(
            -25.47, 29.00, 47.75, 72.56,
        ));
        state
    }

    fn owned_tile_near(state: &GameState, pid: u16, x: u32, y: u32, radius: i32) -> bool {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if state.map.is_valid_coord(nx, ny)
                    && state.map.owner_id(nx as u32, ny as u32) == pid
                {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn test_spawn_ai_geo_names_and_positions() {
        let state = geo_test_state(7);
        let bounds = state.geo_bounds.unwrap();
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(20, 30);
        assert_eq!(engine.state.players.len(), 50);

        for player in &engine.state.players {
            let entity = crate::geo_entities::all()
                .find(|e| e.name == player.name)
                .unwrap_or_else(|| {
                    panic!(
                        "'{}' not in geo database (geo pools should cover 20+30 on europe bounds)",
                        player.name
                    )
                });
            let is_tribe_kind = entity.kind == crate::geo_entities::EntityKind::Tribe;
            assert_eq!(
                player.player_type == crate::player::PlayerType::Bot,
                is_tribe_kind,
                "kind mismatch for {}",
                player.name
            );

            let (lat, lon) = entity
                .lat
                .zip(entity.lon)
                .expect("spawned geo entity has coordinates");
            let (x, y) = bounds
                .project(lat as f64, lon as f64, 1000, 800)
                .expect("spawned geo entity must project inside bounds");
            assert!(
                owned_tile_near(&engine.state, player.id, x, y, 120),
                "{} spawned far from its homeland tile ({x}, {y})",
                player.name
            );
            if let Some(avatar) = &entity.avatar {
                assert_eq!(engine.campaign_avatars.get(&player.id), Some(avatar));
            }
        }
    }

    #[test]
    fn first_geographic_tribe_uses_its_projected_tile_when_land_is_free() {
        let state = geo_test_state(29);
        let bounds = state.geo_bounds.unwrap();
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(0, 1);
        let player = &engine.state.players[0];
        let entity = crate::geo_entities::by_name(&player.name).unwrap();
        let (lat, lon) = entity.lat.zip(entity.lon).unwrap();
        let (x, y) = bounds.project(lat as f64, lon as f64, 1000, 800).unwrap();
        assert_eq!(engine.state.map.owner_id(x, y), player.id);
    }

    #[test]
    fn non_geographic_map_uses_its_curated_entity_pool() {
        let config = GameConfig {
            map_name: "pangaea".to_string(),
            map_width: 1000,
            map_height: 800,
            ..Default::default()
        };
        let mut state = GameState::new(31, 1000, 800, config);
        for tile in &mut state.map.terrain {
            *tile = crate::map::MapTile::from_byte(0b1000_0000);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(5, 5);
        assert_eq!(engine.state.players.len(), 10);
        for player in &engine.state.players {
            let entity = crate::geo_entities::by_name(&player.name).unwrap();
            assert!(entity.maps.iter().any(|map| map == "pangaea"));
        }
    }

    #[test]
    fn test_spawn_ai_geo_deterministic() {
        let spawns = |seed: u64| {
            let mut engine = SowEngine::new(geo_test_state(seed), WaterComponents::default());
            engine.spawn_ai(15, 25);
            engine
                .state
                .players
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        p.sum_x,
                        p.sum_y,
                        engine.campaign_avatars.get(&p.id).cloned(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            spawns(1234),
            spawns(1234),
            "same seed must give same spawns"
        );
        assert_ne!(spawns(1234), spawns(5678), "different seed should differ");
    }

    #[test]
    fn test_spawn_ai_geo_overflow_to_fallback() {
        // Europe bounds hold well under 500 geo tribes; the rest must come
        // from the fallback pools without panicking or duplicating names.
        let mut engine = SowEngine::new(geo_test_state(9), WaterComponents::default());
        engine.spawn_ai(0, 500);
        assert_eq!(engine.state.players.len(), 500);
        let mut seen = std::collections::HashSet::new();
        for p in &engine.state.players {
            assert!(seen.insert(p.name.clone()), "duplicate name {}", p.name);
        }
    }

    #[test]
    fn test_team_map_control_winner() {
        use crate::protocol::Team;

        let config = GameConfig {
            game_mode: "Teams".to_string(),
            map_control_win_percentage: 0.50,
            ..Default::default()
        };
        let mut state = GameState::new(42, 10, 10, config);
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        state.total_land_tiles = 100;

        let mut red =
            crate::player::Player::new_human(1, "Red".into(), [1.0, 0.2, 0.2], &state.config);
        red.team = Some(Team::Red);
        red.tile_count = 55;
        red.alive = true;

        let mut blue =
            crate::player::Player::new_human(2, "Blue".into(), [0.2, 0.5, 1.0], &state.config);
        blue.team = Some(Team::Blue);
        blue.tile_count = 10;
        blue.alive = true;

        state.register_player(red);
        state.register_player(blue);

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.check_team_winner(50);

        assert_eq!(engine.state.winning_team, Some(Team::Red));
        assert_eq!(engine.state.winner, Some(1));
    }
}
