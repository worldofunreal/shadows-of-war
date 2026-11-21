use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, NuclearActionFailure, NukeKind, ProjectileKind};

impl SowEngine {
    pub fn nuclear_price(&self, kind: NukeKind) -> f64 {
        let launched = self.mirv_launches.values().copied().fold(0, u32::saturating_add);
        kind.gold_cost(launched) * (self.state.config.nuke_cost / 500.0)
    }

    pub fn nuclear_charges(&self, building_id: u64, level: u8) -> u8 {
        let used = self.nuclear_reload.get(&building_id).map_or(0, |ticks| {
            ticks.iter().filter(|&&ready| ready > self.state.tick).count()
        });
        level.saturating_sub(used as u8)
    }

    pub fn nuclear_wait_ticks(&self, building_id: u64) -> u32 {
        self.nuclear_reload.get(&building_id).into_iter().flatten()
            .filter_map(|&ready| ready.checked_sub(self.state.tick).filter(|&left| left > 0))
            .min().unwrap_or(0) as u32
    }

    pub fn spend_nuclear_charge(&mut self, building_id: u64) {
        let tick = self.state.tick;
        let reload = self.nuclear_reload.entry(building_id).or_default();
        reload.retain(|&ready| ready > tick);
        reload.push(tick.saturating_add(u64::from(crate::execution::nukes::SILO_COOLDOWN_TICKS)));
    }

    pub fn ready_nuclear_silo(&self, player_id: u16, target_tile: u32) -> Option<crate::building::Building> {
        let w = self.state.map.width;
        if w == 0 || target_tile >= w.saturating_mul(self.state.map.height) {
            return None;
        }
        let tx = target_tile % w;
        let ty = target_tile / w;
        self
            .buildings
            .iter()
            .filter(|b| {
                b.kind == BuildingKind::MissileSilo
                    && b.owner_id == player_id
                    && self.nuclear_charges(b.id, b.active_level()) > 0
            })
            .min_by_key(|b| {
                let bx = b.tile_idx % w;
                let by = b.tile_idx / w;
                (bx.abs_diff(tx) + by.abs_diff(ty), b.id)
            }).copied()
    }

    pub fn launch_nuclear_weapon(&mut self, player_id: u16, kind: NukeKind, target_tile: u32) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        if !self.campaign_allows_action(crate::campaign::CampaignAction::Nuke) {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }
        let player_gold = player.gold;
        let w = self.state.map.width;
        let cost = self.nuclear_price(kind);
        let reported_cost = if cost.is_finite() && cost >= 0.0 {
            cost
        } else {
            0.0
        };
        if !crate::building::player_has_nuclear_launch_city(&self.buildings, player_id) {
            self.state.events.push(crate::game::GameEvent::NuclearActionRejected {
                player_id,
                reason: NuclearActionFailure::CityRequirement,
                cost: reported_cost,
            });
            return;
        }
        let Some(silo) = self.ready_nuclear_silo(player_id, target_tile) else {
            self.state.events.push(crate::game::GameEvent::NuclearActionRejected {
                player_id,
                reason: NuclearActionFailure::NoReadySilo,
                cost: reported_cost,
            });
            return;
        };
        if !cost.is_finite() || cost < 0.0 || player_gold < cost {
            self.state.events.push(crate::game::GameEvent::NuclearActionRejected {
                player_id,
                reason: NuclearActionFailure::InsufficientGold,
                cost: reported_cost,
            });
            return;
        }

        let silo_id = silo.id;
        let silo_tile = silo.tile_idx;
        let level = match kind {
            NukeKind::AtomBomb => 1,
            NukeKind::HydrogenBomb => 2,
            NukeKind::MIRV => 3,
        };
        self.state.player_mut(player_id).unwrap().gold -= cost;
        self.spend_nuclear_charge(silo_id);
        if kind == NukeKind::MIRV {
            let count = self.mirv_launches.entry(player_id).or_default();
            *count = count.saturating_add(1);
        }

        // Spawn projectile
        let id = self.state.next_projectile_id;
        self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);

        let path = std::sync::Arc::new(crate::pathfinding::bresenham_line(
            silo_tile,
            target_tile,
            w,
        ));

        self.projectiles.push(crate::game::Projectile {
            id,
            kind: ProjectileKind::Nuke { level },
            owner_id: player_id,
            src_tile: silo_tile,
            dst_tile: target_tile,
            path,
            path_cursor: 0,
            steps_per_tick: 2,
            active: true,
            source_fleet_id: None,
            target_fleet_id: None,
            damage: 0.0,
            expires_at_tick: u64::MAX,
        });
    }
}
