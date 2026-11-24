use super::profile::{AiStrategyWeights, BotDecision, BotDecisionKind};
use super::structures::{
    PLACEMENT_ATTEMPTS, StructureCandidates, bot_structure_target_count,
    resolve_military_camp_from_candidates, resolve_structure_from_candidates,
};
use crate::building::{
    Building, BuildingAggregate, MAX_WARSHIP_QUEUE, RAILWAY_LINK_DISTANCE,
    city_capacity_level_weight, city_gold_level_weight, city_income_level_weight,
    find_structure_site_near, military_camp_gold_level_weight, military_camp_troop_level_weight,
    port_income_level_weight, structure_max_affordable_upgrade, trading_post_gold_level_weight,
};
use crate::engine::SowEngine;
use crate::execution::income_rates::gold_income_per_second;
use crate::game::{BuildingKind, CityArsenalKind, UnitType};
use crate::geo_entities::{District, Resource, Source};
use crate::protocol::GameplayIntent;
use crate::rng::NextIntExt;
use crate::warp_fleet::WarshipMode;

#[derive(Clone, Debug)]
enum CandidateAction {
    Build {
        kind: BuildingKind,
        site: u32,
    },
    UpgradeSingle {
        building_id: u64,
    },
    UpgradeTo {
        building_id: u64,
        target_level: u8,
    },
}

struct EvaluatedCandidate {
    action: CandidateAction,
    cost: f64,
    roi_score: f64,
}

impl SowEngine {
    /// Returns the effective Pro strategy weights for `bot_id` (allowing test tournaments to
    /// compare weight sets headlessly while keeping live builds deterministic on `PRO`).
    #[inline]
    pub(super) fn pro_weights_for(&self, _bot_id: u16) -> AiStrategyWeights {
        #[cfg(test)]
        if let Some(weights) = self.test_pro_weights_by_player.get(&_bot_id) {
            return *weights;
        }
        AiStrategyWeights::PRO
    }

    /// Collects nearby hostile reference coordinates (`home` / buildings) once for site safety scoring.
    fn pro_hostile_anchors(&self, bot_id: u16) -> Vec<(u32, u32)> {
        let w = self.state.map.width.max(1);
        let mut anchors = Vec::new();
        for p in &self.state.players {
            if !p.alive || p.id == bot_id || crate::execution::hero_effects::allied(&self.state, bot_id, p.id) {
                continue;
            }
            if let Some((hx, hy)) = p.home {
                anchors.push((hx, hy));
            }
        }
        for b in &self.buildings {
            if b.owner_id != bot_id && !crate::execution::hero_effects::allied(&self.state, bot_id, b.owner_id) {
                anchors.push((b.tile_idx % w, b.tile_idx / w));
            }
        }
        anchors
    }

    /// Scores how safe an interior candidate tile is (higher = further from hostile anchors).
    fn pro_site_safety_score(&self, tile_idx: u32, anchors: &[(u32, u32)]) -> u64 {
        if anchors.is_empty() || self.state.map.width == 0 {
            return 1;
        }
        let w = self.state.map.width;
        let x = tile_idx % w;
        let y = tile_idx / w;
        let mut min_dist_sq = u64::MAX;
        for &(ax, ay) in anchors {
            let dx = u64::from(x.abs_diff(ax));
            let dy = u64::from(y.abs_diff(ay));
            min_dist_sq = min_dist_sq.min(dx * dx + dy * dy);
        }
        min_dist_sq
    }

    /// Finds the best valid site for `kind` using strategic map geometry rather than the first
    /// arbitrary border sample.
    fn pro_find_site_for_kind(
        &mut self,
        bot_id: u16,
        kind: BuildingKind,
        anchors: &[(u32, u32)],
    ) -> Option<u32> {
        let w = self.state.map.width;
        let h = self.state.map.height;
        if w == 0 || h == 0 {
            return None;
        }

        if kind == BuildingKind::TradingPost {
            // Optimal TradingPost site: within RAILWAY_LINK_DISTANCE of an owned ready/built
            // station, preferring longer safe rail segments (for railway_distance_bonus up to 2x).
            let stations: Vec<u32> = self
                .buildings
                .iter()
                .filter(|b| {
                    b.owner_id == bot_id
                        && matches!(
                            b.kind,
                            BuildingKind::City | BuildingKind::Port | BuildingKind::TradingPost
                        )
                })
                .map(|b| b.tile_idx)
                .collect();
            let mut best: Option<(i64, u32)> = None;
            let ideal_r: i32 = 44;
            for &station_tile in stations.iter().take(6) {
                let sx = (station_tile % w) as i32;
                let sy = (station_tile / w) as i32;
                let offsets = [
                    (ideal_r, 0),
                    (-ideal_r, 0),
                    (0, ideal_r),
                    (0, -ideal_r),
                    (30, 30),
                    (-30, 30),
                    (30, -30),
                    (-30, -30),
                    (20, 0),
                    (-20, 0),
                    (0, 20),
                    (0, -20),
                ];
                for (dx, dy) in offsets {
                    let nx = sx + dx;
                    let ny = sy + dy;
                    if !self.state.map.is_valid_coord(nx, ny) {
                        continue;
                    }
                    let probe = ny as u32 * w + nx as u32;
                    let Some(site) = find_structure_site_near(
                        &self.state.map,
                        bot_id,
                        probe,
                        BuildingKind::TradingPost,
                        &self.building_grid,
                        &mut self.placement_scratch,
                        self.legacy_building_rules,
                    ) else {
                        continue;
                    };
                    let rx = site % w;
                    let ry = site / w;
                    let dist_sq = rx.abs_diff(sx as u32).pow(2) + ry.abs_diff(sy as u32).pow(2);
                    if dist_sq > RAILWAY_LINK_DISTANCE.saturating_mul(RAILWAY_LINK_DISTANCE) {
                        continue;
                    }
                    let safety = self.pro_site_safety_score(site, anchors).min(10_000) as i64;
                    let rail_score = (dist_sq as f64).sqrt() as i64 * 40 + safety;
                    if best.is_none_or(|(best_score, _)| rail_score > best_score) {
                        best = Some((rail_score, site));
                    }
                }
                if let Some(site) = find_structure_site_near(
                    &self.state.map,
                    bot_id,
                    station_tile,
                    BuildingKind::TradingPost,
                    &self.building_grid,
                    &mut self.placement_scratch,
                    self.legacy_building_rules,
                ) {
                    let rx = site % w;
                    let ry = site / w;
                    let dist = ((rx.abs_diff(sx as u32).pow(2) + ry.abs_diff(sy as u32).pow(2)) as f64)
                        .sqrt() as i64;
                    let safety = self.pro_site_safety_score(site, anchors).min(10_000) as i64;
                    let rail_score = dist * 40 + safety;
                    if best.is_none_or(|(best_score, _)| rail_score > best_score) {
                        best = Some((rail_score, site));
                    }
                }
            }
            if let Some((_, site)) = best {
                return Some(site);
            }
        }

        if kind == BuildingKind::Tower && !anchors.is_empty() {
            // Place Towers near our highest-value building closest to an enemy front.
            let mut best_anchor_tile: Option<(u64, u32)> = None;
            for b in &self.buildings {
                if b.owner_id != bot_id
                    || !matches!(
                        b.kind,
                        BuildingKind::City | BuildingKind::TradingPost | BuildingKind::Port
                    )
                {
                    continue;
                }
                let dist_sq = self.pro_site_safety_score(b.tile_idx, anchors);
                if best_anchor_tile.is_none_or(|(best_d, _)| dist_sq < best_d) {
                    best_anchor_tile = Some((dist_sq, b.tile_idx));
                }
            }
            if let Some((_, protect_tile)) = best_anchor_tile
                && let Some(site) = find_structure_site_near(
                    &self.state.map,
                    bot_id,
                    protect_tile,
                    BuildingKind::Tower,
                    &self.building_grid,
                    &mut self.placement_scratch,
                    self.legacy_building_rules,
                )
            {
                return Some(site);
            }
        }

        // General candidate pool (borders + home + interior samples).
        {
            let (state, scratch) = (&mut self.state, &mut self.placement_scratch);
            let p = state.player_mut(bot_id)?;
            scratch.border_scratch.clear();
            scratch.interior_scratch.clear();
            p.border_tiles.sample_ones(
                p.bot_rng.rand() as u32,
                (PLACEMENT_ATTEMPTS * 2) as usize,
                &mut scratch.border_scratch,
            );
            if p.tile_count > 0 {
                if let Some((hx, hy)) = p.home {
                    scratch.interior_scratch.push((hx as i32, hy as i32));
                }
                let cx = (p.sum_x / p.tile_count as u64) as i32;
                let cy = (p.sum_y / p.tile_count as u64) as i32;
                scratch.interior_scratch.push((cx, cy));
                let radius = if p.tile_count > 500 { 41 } else { 13 };
                for _ in 0..PLACEMENT_ATTEMPTS {
                    let dx = p.bot_rng.next_int(-radius + 1, radius);
                    let dy = p.bot_rng.next_int(-radius + 1, radius);
                    scratch.interior_scratch.push((cx + dx, cy + dy));
                }
            }
        }

        if kind == BuildingKind::Port {
            let border_candidates = std::mem::take(&mut self.placement_scratch.border_scratch);
            let mut best_port: Option<(usize, u32)> = None;
            for &tile in &border_candidates {
                let comp = self.water.component_of(tile);
                if comp == 0 {
                    continue;
                }
                let Some(site) = find_structure_site_near(
                    &self.state.map,
                    bot_id,
                    tile,
                    BuildingKind::Port,
                    &self.building_grid,
                    &mut self.placement_scratch,
                    self.legacy_building_rules,
                ) else {
                    continue;
                };
                let sea_size = self
                    .water
                    .shoreline_tiles
                    .get(comp as usize)
                    .map_or(0, Vec::len);
                if best_port.is_none_or(|(best_size, _)| sea_size > best_size) {
                    best_port = Some((sea_size, site));
                }
            }
            self.placement_scratch.border_scratch = border_candidates;
            self.placement_scratch.interior_scratch.clear();
            return best_port.map(|(_, s)| s);
        }

        let border_candidates = std::mem::take(&mut self.placement_scratch.border_scratch);
        let interior_candidates = std::mem::take(&mut self.placement_scratch.interior_scratch);
        let candidates = StructureCandidates {
            border: &border_candidates,
            interior: &interior_candidates,
        };
        let resolved = if kind == BuildingKind::MilitaryCamp && self.legacy_building_rules {
            resolve_military_camp_from_candidates(
                &self.state.map,
                bot_id,
                candidates,
                &self.buildings,
                &self.building_grid,
                self.legacy_building_rules,
            )
        } else {
            // Try safest interior points first for economic core structures.
            let mut best_site: Option<(u64, u32)> = None;
            if matches!(kind, BuildingKind::City | BuildingKind::MilitaryCamp) {
                for &(ix, iy) in &interior_candidates {
                    if !self.state.map.is_valid_coord(ix, iy) {
                        continue;
                    }
                    let idx = iy as u32 * w + ix as u32;
                    if let Some(site) = find_structure_site_near(
                        &self.state.map,
                        bot_id,
                        idx,
                        kind,
                        &self.building_grid,
                        &mut self.placement_scratch,
                        self.legacy_building_rules,
                    ) {
                        let safety = self.pro_site_safety_score(site, anchors);
                        if best_site.is_none_or(|(best_s, _)| safety > best_s) {
                            best_site = Some((safety, site));
                        }
                    }
                }
            }
            best_site.map(|(_, site)| site).or_else(|| {
                resolve_structure_from_candidates(
                    &self.state.map,
                    bot_id,
                    kind,
                    candidates,
                    &self.building_grid,
                    &mut self.placement_scratch,
                    self.legacy_building_rules,
                )
            })
        };
        self.placement_scratch.border_scratch = border_candidates;
        self.placement_scratch.interior_scratch = interior_candidates;
        resolved
    }

    fn marginal_value_for_upgrade(
        &self,
        kind: BuildingKind,
        from_lvl: u8,
        to_lvl: u8,
        leader: crate::player::Leader,
        has_rail_network: bool,
        is_under_attack: bool,
        weights: AiStrategyWeights,
    ) -> f64 {
        let cfg = &self.state.config;
        let hero_gold = |d| leader.resource_bonus_multiplier(Source::District(d), Resource::Gold);
        let hero_troop = |d| leader.resource_bonus_multiplier(Source::District(d), Resource::Troops);
        let hero_city_bonus = leader
            .resource_bonus_hero_bonus(Source::District(District::City), Resource::Gold)
            .unwrap_or(0.0);

        match kind {
            BuildingKind::City => {
                let d_gold = (city_gold_level_weight(to_lvl) - city_gold_level_weight(from_lvl))
                    * (cfg.city_gold_income * hero_gold(District::City)
                        + cfg.hero_city_gold_income * hero_city_bonus);
                let d_troop = (city_income_level_weight(to_lvl) - city_income_level_weight(from_lvl))
                    * cfg.city_troop_income
                    * hero_troop(District::City);
                let d_cap = (city_capacity_level_weight(to_lvl) - city_capacity_level_weight(from_lvl))
                    * cfg.city_max_troops
                    * leader.city_troop_capacity_multiplier();
                let nuke_bonus = if from_lvl >= 4 && to_lvl <= 6 {
                    weights.city_reserve_bonus * weights.nuclear_city_push_bonus
                } else {
                    weights.city_reserve_bonus
                };
                d_gold * weights.gold_value_weight
                    + d_troop * weights.troop_value_weight
                    + d_cap * weights.troop_cap_bonus_weight
                    + nuke_bonus
            }
            BuildingKind::TradingPost => {
                let rail_mult = if has_rail_network { 1.45 } else { 0.15 };
                let d_gold = (trading_post_gold_level_weight(to_lvl)
                    - trading_post_gold_level_weight(from_lvl))
                    * cfg.trading_post_gold_income
                    * hero_gold(District::TradingPost)
                    * rail_mult;
                d_gold * weights.gold_value_weight
            }
            BuildingKind::MilitaryCamp => {
                let d_gold = (military_camp_gold_level_weight(to_lvl)
                    - military_camp_gold_level_weight(from_lvl))
                    * cfg.military_camp_gold_income
                    * hero_gold(District::MilitaryCamp);
                let d_troop = (military_camp_troop_level_weight(to_lvl)
                    - military_camp_troop_level_weight(from_lvl))
                    * cfg.military_camp_troop_income
                    * hero_troop(District::MilitaryCamp);
                d_gold * weights.gold_value_weight + d_troop * weights.troop_value_weight + 12.0
            }
            BuildingKind::Port => {
                let d_weight = port_income_level_weight(to_lvl) - port_income_level_weight(from_lvl);
                let d_gold = d_weight * cfg.port_gold_income * hero_gold(District::Port);
                let d_troop = d_weight * cfg.port_troop_income * hero_troop(District::Port);
                d_gold * weights.gold_value_weight
                    + d_troop * weights.troop_value_weight
                    + weights.port_naval_slot_bonus
            }
            BuildingKind::Tower => {
                if is_under_attack {
                    18.0 * weights.tower_defense_priority * f64::from(to_lvl - from_lvl)
                } else {
                    2.5 * weights.tower_defense_priority * f64::from(to_lvl - from_lvl)
                }
            }
            _ => 0.0,
        }
    }

    /// Evaluates build orders, ROI of new structures vs single/multi-level upgrades, and strategic
    /// site placement for Pro-tier bots (`Ghost` / `Terminator`).
    pub(super) fn pro_decide_structure_action(
        &mut self,
        bot_id: u16,
        bot_iq: u32,
        player_gold: f64,
        player_tile_count: u32,
        agg: BuildingAggregate,
        is_under_attack: bool,
    ) -> Option<BotDecision> {
        let weights = self.pro_weights_for(bot_id);
        let player = self.state.player(bot_id)?;
        let leader = player.leader;
        let anchors = self.pro_hostile_anchors(bot_id);
        let all_structures = agg.count_city
            + agg.count_tower
            + agg.count_trading_post
            + agg.count_port
            + agg.count_military_camp
            + agg.count_sam;

        // 1. Always establish an opening City first.
        if agg.count_city == 0
            && let Some((_, cost, _)) = self.foundation_quote(bot_id, BuildingKind::City)
        {
            if player_gold >= cost
                && let Some(site) = self.pro_find_site_for_kind(bot_id, BuildingKind::City, &anchors)
            {
                return Some(BotDecision {
                    bot_id,
                    kind: BotDecisionKind::Build,
                    intent: GameplayIntent::BuildStructure {
                        kind: BuildingKind::City,
                        target_tile: site,
                    },
                });
            }
            return None;
        }

        let current_gold_ps =
            gold_income_per_second(player_tile_count, agg, leader, &self.state.config)
                * self.state.config.global_speed_multiplier
                / crate::game_config::NORMAL_GAME_SPEED;
        let lookahead_budget = player_gold + current_gold_ps.max(0.5) * weights.lookahead_seconds;
        let city_equivalent = (player_tile_count / 1000).max(1);
        let has_rail_partner = all_structures > 0;
        let built_tp_exists = agg.count_trading_post > 0;

        let mut best_affordable: Option<EvaluatedCandidate> = None;
        let mut best_overall: Option<EvaluatedCandidate> = None;

        // 2. Evaluate founding a new structure for each economic/military/defensive kind.
        for kind in [
            BuildingKind::TradingPost,
            BuildingKind::City,
            BuildingKind::MilitaryCamp,
            BuildingKind::Port,
            BuildingKind::Tower,
        ] {
            if kind == BuildingKind::TradingPost && !has_rail_partner {
                continue;
            }
            let Some((level, cost, _)) = self.foundation_quote(bot_id, kind) else {
                continue;
            };
            if cost > lookahead_budget || !cost.is_finite() {
                continue;
            }
            let target_count = bot_structure_target_count(kind, city_equivalent, bot_iq).max(1);
            let owned = agg.total_structures_of_kind(kind);
            if kind == BuildingKind::Tower && !is_under_attack && owned >= target_count {
                continue;
            }
            let Some(site) = self.pro_find_site_for_kind(bot_id, kind, &anchors) else {
                continue;
            };

            let mut value = self.marginal_value_for_upgrade(
                kind,
                0,
                level,
                leader,
                has_rail_partner,
                is_under_attack,
                weights,
            );
            // Encourage early diversity (first City -> first TradingPost -> first Camp -> first Port).
            if owned == 0 && kind != BuildingKind::Tower {
                value *= 1.45;
            } else if owned >= target_count {
                value *= 0.55;
            }
            let roi_score = value / cost.max(1.0);

            if best_overall
                .as_ref()
                .is_none_or(|best| roi_score > best.roi_score)
            {
                best_overall = Some(EvaluatedCandidate {
                    action: CandidateAction::Build { kind, site },
                    cost,
                    roi_score,
                });
            }
            if cost <= player_gold
                && best_affordable
                    .as_ref()
                    .is_none_or(|best| roi_score > best.roi_score)
            {
                best_affordable = Some(EvaluatedCandidate {
                    action: CandidateAction::Build { kind, site },
                    cost,
                    roi_score,
                });
            }
        }

        // 3. Evaluate upgrading existing ready structures (single-step + multi-level UpgradeStructureTo).
        let buildings_snapshot: Vec<Building> = self
            .buildings
            .iter()
            .filter(|b| {
                b.owner_id == bot_id
                    && !b.under_construction
                    && b.level < b.kind.max_level()
                    && !self
                        .building_productions
                        .get(&b.id)
                        .is_some_and(|state| state.is_building())
            })
            .copied()
            .collect();

        for b in buildings_snapshot {
            let max_allowed = self
                .campaign_unlocks
                .as_ref()
                .map_or(b.kind.max_level(), |unlocks| unlocks.building_level(b.kind));
            if b.level >= max_allowed {
                continue;
            }
            let has_rail = built_tp_exists && all_structures >= 2;
            let (single_cost, _) = self.paid_structure_upgrade_quote(b);
            if single_cost.is_finite() && single_cost <= lookahead_budget {
                let value = self.marginal_value_for_upgrade(
                    b.kind,
                    b.level,
                    b.level + 1,
                    leader,
                    has_rail,
                    is_under_attack,
                    weights,
                );
                let roi_score = value / single_cost.max(1.0);
                if best_overall
                    .as_ref()
                    .is_none_or(|best| roi_score > best.roi_score)
                {
                    best_overall = Some(EvaluatedCandidate {
                        action: CandidateAction::UpgradeSingle { building_id: b.id },
                        cost: single_cost,
                        roi_score,
                    });
                }
                if single_cost <= player_gold
                    && best_affordable
                        .as_ref()
                        .is_none_or(|best| roi_score > best.roi_score)
                {
                    best_affordable = Some(EvaluatedCandidate {
                        action: CandidateAction::UpgradeSingle { building_id: b.id },
                        cost: single_cost,
                        roi_score,
                    });
                }
            }

            // Evaluate atomic multi-level jump via UpgradeStructureTo when gold covers 2+ levels.
            let built_levels = self.paid_kind_levels(bot_id, b.kind);
            let discount = self.hero_city_upgrade_discount(b);
            if let Some((target_lvl, multi_cost, _)) = structure_max_affordable_upgrade(
                b.kind,
                b.level,
                max_allowed,
                built_levels,
                false,
                discount,
                self.state.config.tutorial,
                player_gold,
                &self.state.config,
            ) && target_lvl >= b.level + 2
            {
                let value = self.marginal_value_for_upgrade(
                    b.kind,
                    b.level,
                    target_lvl,
                    leader,
                    has_rail,
                    is_under_attack,
                    weights,
                ) * weights.upgrade_multi_level_bias;
                let roi_score = value / multi_cost.max(1.0);
                if best_affordable
                    .as_ref()
                    .is_none_or(|best| roi_score > best.roi_score)
                {
                    best_affordable = Some(EvaluatedCandidate {
                        action: CandidateAction::UpgradeTo {
                            building_id: b.id,
                            target_level: target_lvl,
                        },
                        cost: multi_cost,
                        roi_score,
                    });
                }
            }
        }

        // Lookahead discipline: if saving ~8s unlocks a >25% higher-ROI play, wait instead of wasting gold.
        if let (Some(overall), Some(aff)) = (&best_overall, &best_affordable)
            && overall.cost > player_gold
            && overall.roi_score > aff.roi_score * 1.25
            && !is_under_attack
        {
            return None;
        }

        let chosen = best_affordable?;
        let intent = match chosen.action {
            CandidateAction::Build { kind, site } => GameplayIntent::BuildStructure {
                kind,
                target_tile: site,
            },
            CandidateAction::UpgradeSingle { building_id } => {
                GameplayIntent::UpgradeStructure { building_id }
            }
            CandidateAction::UpgradeTo {
                building_id,
                target_level,
            } => GameplayIntent::UpgradeStructureTo {
                building_id,
                target_level,
            },
        };
        Some(BotDecision {
            bot_id,
            kind: BotDecisionKind::Build,
            intent,
        })
    }

    /// Selects a tactical `target_tile` for `AttackIntent` (undefended high-value buildings on
    /// offense; spearhead front on defense).
    pub(super) fn pro_choose_attack_target_tile(
        &self,
        bot_id: u16,
        target_owner: u16,
        is_defending: bool,
    ) -> Option<u32> {
        if target_owner == 0 || self.state.map.width == 0 {
            return None;
        }
        let w = self.state.map.width;
        if is_defending
            && let Some(spearhead) = self
                .attacks
                .iter()
                .filter(|a| a.owner_id == target_owner && a.target_owner == bot_id && !a.retreating)
                .max_by(|a, b| a.troops.total_cmp(&b.troops))
                .and_then(|a| a.to_conquer.peek())
        {
            return Some(spearhead.y * w + spearhead.x);
        }

        let (ox, oy) = self
            .state
            .player(bot_id)
            .and_then(|p| {
                p.home.or_else(|| {
                    (p.tile_count > 0).then(|| {
                        ((p.sum_x / p.tile_count as u64) as u32, (p.sum_y / p.tile_count as u64) as u32)
                    })
                })
            })
            .unwrap_or((w / 2, self.state.map.height / 2));

        let mut best: Option<(f64, u32)> = None;
        for b in &self.buildings {
            if b.owner_id != target_owner {
                continue;
            }
            let base_val = match b.kind {
                BuildingKind::MissileSilo => 140.0,
                BuildingKind::City => 110.0 + 25.0 * f64::from(b.level),
                BuildingKind::TradingPost => 95.0 + 20.0 * f64::from(b.level),
                BuildingKind::Port => 80.0 + 18.0 * f64::from(b.level),
                BuildingKind::MilitaryCamp => 70.0 + 15.0 * f64::from(b.level),
                BuildingKind::SAM => 60.0 + 12.0 * f64::from(b.level),
                BuildingKind::Tower => 12.0 * f64::from(b.level),
            };
            let bx = b.tile_idx % w;
            let by = b.tile_idx / w;
            let dist = ((bx.abs_diff(ox).pow(2) + by.abs_diff(oy).pow(2)) as f64).sqrt();
            let defense_penalty = 1.0
                + 0.25
                    * f64::from(
                        self.building_grid
                            .iter_all_in_range(bx, by, 10)
                            .filter(|other| {
                                other.kind == BuildingKind::Tower
                                    && self.state.map.owner_id(other.x, other.y) == target_owner
                            })
                            .count() as u32,
                    );
            let score = base_val / ((1.0 + dist * 0.035) * defense_penalty);
            if best.is_none_or(|(best_score, _)| score > best_score) {
                best = Some((score, b.tile_idx));
            }
        }
        best.map(|(_, tile)| tile)
    }

    /// Active Warship production scaling and naval interception/patrol micro for Pro bots.
    pub(super) fn pro_fleet_decisions(
        &self,
        bot_id: u16,
        owned_warships: u32,
        queued_warships: u32,
        used_slots: u32,
        decisions: &mut Vec<BotDecision>,
    ) {
        if !self.campaign_allows_action(crate::campaign::CampaignAction::Warship) {
            return;
        }
        let weights = self.pro_weights_for(bot_id);
        let Some(player) = self.state.player(bot_id) else {
            return;
        };
        let pid = usize::from(bot_id);
        let agg = self
            .building_aggregates
            .get(pid)
            .copied()
            .unwrap_or_default();
        let cap = crate::building::cost::player_fleet_capacity_from_port_slots(agg.port_fleet_slots);
        let desired_warships = agg
            .count_port
            .min(3)
            .saturating_mul(weights.warship_target_per_port)
            .min(cap);

        // 1. Scale Warships up to desired target (using BuildingProductionMax when gold covers 2+).
        if !self.legacy_building_rules
            && !super::structures::has_pending_build(decisions, bot_id)
            && owned_warships.saturating_add(queued_warships) < desired_warships
            && used_slots.saturating_add(queued_warships) < cap
        {
            let mut chosen_port: Option<u64> = None;
            let mut port_queued = 0u32;
            for port in self.buildings.iter().filter(|p| {
                p.owner_id == bot_id
                    && p.kind == BuildingKind::Port
                    && !p.under_construction
                    && p.active_level() >= 1
            }) {
                let q_len = self
                    .building_productions
                    .get(&port.id)
                    .map_or(0, |s| s.warship_queue_len());
                if q_len < MAX_WARSHIP_QUEUE
                    && crate::warp_fleet::coastal_warship_spawn_tile(
                        &self.state.map,
                        &self.water,
                        port.tile_idx,
                    )
                    .is_some()
                {
                    chosen_port = Some(port.id);
                    port_queued = q_len as u32;
                    break;
                }
            }
            if let Some(port_id) = chosen_port {
                let first_cost = crate::building::city_arsenal_cost_gold(
                    CityArsenalKind::Warship,
                    owned_warships.saturating_add(queued_warships),
                    player.leader,
                    &self.state.config,
                );
                let second_cost = first_cost
                    + crate::building::city_arsenal_cost_gold(
                        CityArsenalKind::Warship,
                        owned_warships.saturating_add(queued_warships).saturating_add(1),
                        player.leader,
                        &self.state.config,
                    );
                if player.gold >= second_cost
                    && port_queued + 1 < MAX_WARSHIP_QUEUE as u32
                    && owned_warships + queued_warships + 1 < desired_warships
                    && used_slots + queued_warships + 1 < cap
                {
                    decisions.push(BotDecision {
                        bot_id,
                        kind: BotDecisionKind::Build,
                        intent: GameplayIntent::BuildingProductionMax {
                            building_id: port_id,
                            kind: CityArsenalKind::Warship,
                        },
                    });
                } else if player.gold >= first_cost {
                    decisions.push(BotDecision {
                        bot_id,
                        kind: BotDecisionKind::Build,
                        intent: GameplayIntent::BuildingProduction {
                            building_id: port_id,
                            kind: CityArsenalKind::Warship,
                        },
                    });
                }
            }
        }

        // 2. Micro active Warships: intercept nearby enemy TradeShips / TransportShips, otherwise patrol.
        let tick = self.state.tick;
        let w = self.state.map.width.max(1);
        for fleet in &self.fleets {
            if fleet.owner_id != bot_id
                || fleet.unit_type != UnitType::Warship
                || fleet.troops <= 0.0
                || fleet.retreating
            {
                continue;
            }
            let Some(ref ws) = fleet.warship else {
                continue;
            };
            if tick.saturating_sub(ws.manual_order_tick) < 20 {
                continue;
            }
            let comp = self.water.component_of(fleet.current_tile);
            if comp == 0 {
                continue;
            }
            let fx = fleet.current_tile % w;
            let fy = fleet.current_tile / w;

            // Find closest high-priority enemy merchant or transport in same sea.
            let target_ship = self
                .fleets
                .iter()
                .filter(|other| {
                    other.troops > 0.0
                        && other.owner_id != bot_id
                        && matches!(other.unit_type, UnitType::TradeShip | UnitType::TransportShip)
                        && self.naval_target_allowed(bot_id, other.owner_id)
                        && self.water.component_of(other.current_tile) == comp
                })
                .min_by_key(|other| {
                    let priority = u32::from(other.unit_type != UnitType::TransportShip);
                    let dist = (other.current_tile % w).abs_diff(fx)
                        + (other.current_tile / w).abs_diff(fy);
                    (priority, dist, other.id)
                });

            if let Some(prey) = target_ship {
                let dst_dist = (prey.current_tile % w).abs_diff(fleet.dst_tile % w)
                    + (prey.current_tile / w).abs_diff(fleet.dst_tile / w);
                if dst_dist > 12 {
                    decisions.push(BotDecision {
                        bot_id,
                        kind: BotDecisionKind::Attack,
                        intent: GameplayIntent::MoveWarships {
                            unit_ids: vec![fleet.id],
                            target_tile: prey.current_tile,
                        },
                    });
                }
            } else if ws.mode == WarshipMode::Holding && fleet.path_cursor >= fleet.path.len() {
                decisions.push(BotDecision {
                    bot_id,
                    kind: BotDecisionKind::Attack,
                    intent: GameplayIntent::PatrolWarships {
                        unit_ids: vec![fleet.id],
                    },
                });
            }
        }
    }
}
