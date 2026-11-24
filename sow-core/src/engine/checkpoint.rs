//! Private, complete verifier storage. GameState's public wire representation
//! deliberately omits RNG and map metadata, so it cannot resume simulation.
pub(super) mod state {
    use crate::{
        game::GameState,
        map_file::{GeoBounds, MapRosterPreset, MapSpawn},
    };
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize)]
    struct Saved<'a> {
        state: &'a GameState,
        rng: Vec<&'a wyrand::WyRand>,
        spawns: &'a [MapSpawn],
        rosters: &'a [MapRosterPreset],
        bounds: &'a Option<GeoBounds>,
        ownership_revision: u64,
    }
    #[derive(Deserialize)]
    struct Restored {
        state: GameState,
        rng: Vec<wyrand::WyRand>,
        spawns: Vec<MapSpawn>,
        rosters: Vec<MapRosterPreset>,
        bounds: Option<GeoBounds>,
        ownership_revision: u64,
    }
    pub fn serialize<S: Serializer>(state: &GameState, serializer: S) -> Result<S::Ok, S::Error> {
        Saved {
            state,
            rng: state.players.iter().map(|p| &p.bot_rng).collect(),
            spawns: &state.map_spawns,
            rosters: &state.map_rosters,
            bounds: &state.geo_bounds,
            ownership_revision: state.map.ownership_revision(),
        }
        .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<GameState, D::Error> {
        let mut saved = Restored::deserialize(deserializer)?;
        if saved.rng.len() != saved.state.players.len() {
            return Err(serde::de::Error::custom(
                "checkpoint player RNG count mismatch",
            ));
        }
        let max_id = saved
            .state
            .players
            .iter()
            .map(|p| p.id as usize)
            .max()
            .unwrap_or(0);
        saved.state.player_lookup = vec![None; max_id + 1];
        for (index, (player, rng)) in saved.state.players.iter_mut().zip(saved.rng).enumerate() {
            player.bot_rng = rng;
            saved.state.player_lookup[player.id as usize] = Some(index);
        }
        saved.state.map_spawns = saved.spawns;
        saved.state.map_rosters = saved.rosters;
        saved.state.geo_bounds = saved.bounds;
        saved
            .state
            .map
            .restore_ownership_revision(saved.ownership_revision);
        Ok(saved.state)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        engine::{SowEngine, initialize_match_engine},
        game_config::GameConfig,
        protocol::{AttackIntent, GameplayIntent, PlayerInfo, StampedIntent},
    };

    #[test]
    fn complete_checkpoint_resumes_attacks_rng_fleets_buildings_and_timers() {
        let config = GameConfig {
            map_width: 80,
            map_height: 40,
            random_spawn: true,
            nation_count: 3,
            bot_count: 5,
            map_control_win_percentage: 0.99,
            ..Default::default()
        };
        let players = (1..=2)
            .map(|id| PlayerInfo {
                id,
                name: format!("Human {id}"),
                player_type: crate::player::PlayerType::Human,
                color: [0.5; 3],
                team: None,
                spawn_x: 0,
                spawn_y: 0,
                leader: crate::player::Leader::default(),
                skin_style: 0,
                is_ai_controlled: false,
            })
            .collect();
        let terrain = (0..3200)
            .map(|i| if i % 80 < 60 { 128 } else { 0 })
            .collect::<Vec<_>>();
        let mut original = initialize_match_engine(
            config,
            419,
            &terrain,
            players,
            Vec::new(),
            None,
            Vec::new(),
            2400,
        );
        for _ in 0..20 {
            original.tick();
        }
        let rail_row = 3;
        for x in 2..=14 {
            let tile = original.state.map.ref_id(x, rail_row);
            original.state.map.terrain[tile] = crate::map::MapTile::from_byte(0x80);
            original.state.map.set_owner_id(x, rail_row, 1);
        }
        original.buildings.extend([
            crate::building::Building {
                id: 100_001,
                owner_id: 1,
                tile_idx: rail_row * 80 + 2,
                kind: crate::game::BuildingKind::TradingPost,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
            crate::building::Building {
                id: 100_002,
                owner_id: 2,
                tile_idx: rail_row * 80 + 14,
                kind: crate::game::BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
        ]);
        original.building_aggregates_dirty = true;
        original.refresh_building_aggregates();
        let active_route_index = original
            .railway_links
            .iter()
            .position(|link| link.from_building_id == 100_001)
            .expect("test Trading Post should have a route");
        let active_path = original.railway_links[active_route_index]
            .station_path
            .clone();
        let active_progress = original.trade_route_progress[active_route_index];
        let route_tiles = crate::building::expand_path(
            &original.railway_links[active_route_index].route_path,
            80,
        );
        let added_station_tile = route_tiles
            .iter()
            .skip(1)
            .take(route_tiles.len().saturating_sub(2))
            .copied()
            .find(|tile| {
                !original
                    .buildings
                    .iter()
                    .any(|building| building.tile_idx == *tile)
            })
            .expect("test route should have an unoccupied interior tile");
        let x = added_station_tile % 80;
        let y = added_station_tile / 80;
        let tile = original.state.map.ref_id(x, y);
        original.state.map.terrain[tile] = crate::map::MapTile::from_byte(0x80);
        original.state.map.set_owner_id(x, y, 2);
        original.buildings.push(crate::building::Building {
            id: 100_003,
            owner_id: 2,
            tile_idx: added_station_tile,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        original.building_aggregates_dirty = true;
        original.refresh_building_aggregates();
        assert_eq!(
            original.railway_links[active_route_index].station_path, active_path,
            "an in-flight itinerary must not change when a station opens"
        );
        assert_eq!(
            original.trade_route_progress[active_route_index],
            active_progress
        );
        assert!(original.trade_route_pending[active_route_index].changed);
        assert!(
            original.trade_route_pending[active_route_index]
                .route
                .as_ref()
                .is_some_and(|route| route.station_path != active_path)
        );
        assert!(!original.trade_delivery_queue.is_empty());
        original.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: GameplayIntent::Attack(AttackIntent {
                target_owner: 0,
                troops: Some(20.0),
                target_tile: None,
            }),
        }]);
        assert!(!original.attacks.is_empty());
        original.fleets.push(crate::warp_fleet::WarpFleet::new(
            777,
            1,
            0,
            crate::game::UnitType::TransportShip,
            5.0,
            (60, 62),
            vec![60, 61, 62],
        ));
        original.silo_cooldowns.insert(1001, 53);
        original.mirv_launches.insert(1, 3);
        original
            .recent_nuke_targets
            .insert((2, 31), original.state.tick);
        original.alliance_request_cooldown_until.insert((1, 2), 91);
        for player in &mut original.state.players {
            for _ in 0..7 {
                player.bot_rng.rand();
            }
        }
        let bytes = bincode::serialize(&original).unwrap();
        let mut restored: SowEngine = bincode::deserialize(&bytes).unwrap();
        assert_eq!(restored.railway_links, original.railway_links);
        assert_eq!(restored.railway_tracks, original.railway_tracks);
        assert_eq!(restored.trade_route_progress, original.trade_route_progress);
        assert_eq!(restored.trade_route_pending, original.trade_route_pending);
        assert_eq!(
            restored.trade_delivery_queue.clone().into_sorted_vec(),
            original.trade_delivery_queue.clone().into_sorted_vec()
        );
        assert_eq!(
            restored.state.map.ownership_revision(),
            original.state.map.ownership_revision()
        );
        assert_eq!(restored.silo_cooldowns, original.silo_cooldowns);
        assert_eq!(restored.fleets.len(), original.fleets.len());
        for (a, b) in original.state.players.iter().zip(&restored.state.players) {
            assert!(a.bot_rng == b.bot_rng, "player RNG was reset");
        }
        for (a, b) in original.attacks.iter().zip(&restored.attacks) {
            assert!(a.rng == b.rng, "attack RNG was reset");
        }
        for tick in 0..1000 {
            original.tick();
            restored.tick();
            assert_eq!(
                serde_json::to_value(&original.state).unwrap(),
                serde_json::to_value(&restored.state).unwrap(),
                "simulation diverged at resumed tick {tick}"
            );
            assert_eq!(original.attacks.len(), restored.attacks.len());
            assert_eq!(original.fleets.len(), restored.fleets.len());
            assert_eq!(original.buildings, restored.buildings);
        }
    }
}
