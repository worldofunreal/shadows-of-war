use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, ProjectileKind};

impl SowEngine {
    pub fn apply_launch_nuke_intent(&mut self, player_id: u16, target_tile: u32) {
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

        let w = self.state.map.width;
        let area = w.saturating_mul(self.state.map.height);
        if target_tile >= area || w == 0 {
            return;
        }

        // Find the closest armed level-four Factory or Tower producer.
        let tx = (target_tile % w) as i32;
        let ty = (target_tile / w) as i32;

        let best_silo = self
            .buildings
            .iter()
            .filter(|b| {
                matches!(b.kind, BuildingKind::Factory | BuildingKind::Tower)
                    && b.owner_id == player_id
                    && !b.under_construction
                    && b.active_level() >= crate::game::NukeKind::AtomBomb.required_district_level()
                    && self
                        .building_productions
                        .get(&b.id)
                        .is_some_and(|state| state.nuke_ready)
            })
            .min_by_key(|b| {
                let bx = (b.tile_idx % w) as i32;
                let by = (b.tile_idx / w) as i32;
                (bx - tx).abs() + (by - ty).abs()
            });

        let Some(silo) = best_silo else {
            return;
        };
        let silo_id = silo.id;
        let silo_tile = silo.tile_idx;
        let level = 1;

        // Consume the armed nuke on launch (gold and build time were paid in production).
        if let Some(state) = self.building_productions.get_mut(&silo_id) {
            state.nuke_ready = false;
        }

        // Spawn projectile
        let id = self.state.next_projectile_id;
        self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);

        let path = crate::pathfinding::bresenham_line(silo_tile, target_tile, w);

        self.projectiles.push(crate::game::Projectile {
            id,
            kind: ProjectileKind::Nuke { level },
            owner_id: player_id,
            src_tile: silo_tile,
            dst_tile: target_tile,
            path,
            path_cursor: 0,
            steps_per_tick: 1 + level,
            active: true,
            source_fleet_id: None,
            target_fleet_id: None,
            damage: 0.0,
            expires_at_tick: u64::MAX,
        });
    }
}
