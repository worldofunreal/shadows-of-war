//! Private, complete verifier storage. GameState's public wire representation
//! deliberately omits RNG and map metadata, so it cannot resume simulation.
pub(super) mod state {
    use crate::{game::GameState, map_file::{GeoBounds, MapRosterPreset, MapSpawn}};
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
        Saved { state, rng: state.players.iter().map(|p| &p.bot_rng).collect(),
            spawns: &state.map_spawns, rosters: &state.map_rosters, bounds: &state.geo_bounds,
            ownership_revision: state.map.ownership_revision() }.serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<GameState, D::Error> {
        let mut saved = Restored::deserialize(deserializer)?;
        if saved.rng.len() != saved.state.players.len() {
            return Err(serde::de::Error::custom("checkpoint player RNG count mismatch"));
        }
        let max_id = saved.state.players.iter().map(|p| p.id as usize).max().unwrap_or(0);
        saved.state.player_lookup = vec![None; max_id + 1];
        for (index, (player, rng)) in saved.state.players.iter_mut().zip(saved.rng).enumerate() {
            player.bot_rng = rng;
            saved.state.player_lookup[player.id as usize] = Some(index);
        }
        saved.state.map_spawns = saved.spawns;
        saved.state.map_rosters = saved.rosters;
        saved.state.geo_bounds = saved.bounds;
        saved.state.map.restore_ownership_revision(saved.ownership_revision);
        Ok(saved.state)
    }
}

#[cfg(test)]
mod tests {
    use crate::{engine::{SowEngine,initialize_match_engine}, game_config::GameConfig, protocol::{AttackIntent,GameplayIntent,PlayerInfo,StampedIntent}};

    #[test]
    fn complete_checkpoint_resumes_attacks_rng_fleets_buildings_and_timers() {
        let config = GameConfig { map_width:80,map_height:40,random_spawn:true,
            nation_count:3,bot_count:5,map_control_win_percentage:0.99,..Default::default() };
        let players = (1..=2).map(|id| PlayerInfo { id,name:format!("Human {id}"),
            player_type:crate::player::PlayerType::Human,color:[0.5;3],team:None,spawn_x:0,spawn_y:0,
            civilization:crate::player::Civilization::Rome,leader:crate::player::Leader::Caesar,
            skin_style:0,is_ai_controlled:false }).collect();
        let terrain = (0..3200).map(|i|if i%80 < 60 { 128 } else { 0 }).collect::<Vec<_>>();
        let mut original = initialize_match_engine(config,419,&terrain,players,Vec::new(),None,Vec::new(),2400);
        for _ in 0..20 { original.tick(); }
        original.apply_intents(&[StampedIntent { player_id:1,intent:GameplayIntent::Attack(AttackIntent { target_owner:0,troops:Some(20.0) }) }]);
        assert!(!original.attacks.is_empty());
        original.fleets.push(crate::warp_fleet::WarpFleet::new(777,1,0,crate::game::UnitType::TransportShip,5.0,(60,62),vec![60,61,62]));
        original.port_queues.insert(999,std::collections::VecDeque::from([crate::game::ShipProduction { kind:crate::game::UnitType::TradeShip,ticks_until_complete:30 }]));
        original.silo_cooldowns.insert(1001,53);
        original.mirv_launches.insert(1,3);
        original.recent_nuke_targets.insert((2,31),original.state.tick);
        original.alliance_request_cooldown_until.insert((1,2),91);
        for player in &mut original.state.players { for _ in 0..7 { player.bot_rng.rand(); } }
        let bytes = bincode::serialize(&original).unwrap();
        let mut restored: SowEngine = bincode::deserialize(&bytes).unwrap();
        assert_eq!(restored.state.map.ownership_revision(),original.state.map.ownership_revision());
        assert_eq!(restored.port_queues,original.port_queues);
        assert_eq!(restored.silo_cooldowns,original.silo_cooldowns);
        assert_eq!(restored.fleets.len(),original.fleets.len());
        for (a,b) in original.state.players.iter().zip(&restored.state.players) {
            assert!(a.bot_rng == b.bot_rng,"player RNG was reset");
        }
        for (a,b) in original.attacks.iter().zip(&restored.attacks) {
            assert!(a.rng == b.rng,"attack RNG was reset");
        }
        for tick in 0..1000 {
            original.tick(); restored.tick();
            assert_eq!(serde_json::to_value(&original.state).unwrap(),serde_json::to_value(&restored.state).unwrap(),"simulation diverged at resumed tick {tick}");
            assert_eq!(original.attacks.len(),restored.attacks.len());
            assert_eq!(original.fleets.len(),restored.fleets.len());
            assert_eq!(original.buildings,restored.buildings);
        }
    }
}
