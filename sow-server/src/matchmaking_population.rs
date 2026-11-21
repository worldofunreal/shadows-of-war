use rand::Rng;
use sow_core::game_config::GameConfig;
use sow_core::map_file::MapCatalogEntry;

pub const MIN_MATCHMAKING_PLAYERS: u32 = 12;
pub const MAX_MATCHMAKING_PLAYERS: u32 = sow_core::protocol::MAX_MATCH_PARTICIPANTS as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchmakingPopulation {
    pub max_players: u32,
    pub bot_count: u32,
    pub nation_count: u32,
}

pub fn roll(
    _map: Option<&MapCatalogEntry>,
    game_mode: &str,
    rng: &mut impl Rng,
) -> MatchmakingPopulation {
    let max_players = roll_human_capacity(game_mode, rng);
    let bot_count = GameConfig::max_tribe_bots();
    // Public FFA/Teams stay Tribes-only; Co-op Bot Heroes are resolved at start from human_side_players.
    let nation_count = 0;

    MatchmakingPopulation {
        max_players,
        bot_count,
        nation_count,
    }
}

pub fn finalize_for_start(config: &mut GameConfig, human_side_players: u32) -> bool {
    if !sow_core::game_config::is_coop_game_mode(&config.game_mode) {
        return false;
    }
    config.nation_count = human_side_players;
    config.clamp_bot_counts();
    true
}

fn roll_human_capacity(game_mode: &str, rng: &mut impl Rng) -> u32 {
    // Square the random position so small rooms are common and large rooms are
    // possible without a fixed repeating bucket sequence.
    let position = rng.gen_range(0.0..=1.0);
    let raw = MIN_MATCHMAKING_PLAYERS as f64
        + (MAX_MATCHMAKING_PLAYERS - MIN_MATCHMAKING_PLAYERS) as f64 * position * position;
    let mut players = raw.round() as u32;
    if game_mode == "Teams" {
        players -= players % 2;
    }
    players.clamp(MIN_MATCHMAKING_PLAYERS, MAX_MATCHMAKING_PLAYERS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn map(land_tiles: u32) -> MapCatalogEntry {
        MapCatalogEntry {
            key: "test".to_string(),
            display_name: "Test".to_string(),
            width: 100,
            height: 100,
            num_land_tiles: land_tiles,
            multiplayer_frequency: 1,
            default_roster: None,
            roster_presets: Vec::new(),
        }
    }

    #[test]
    fn matchmaking_capacity_is_bounded_and_varied() {
        let mut rng = StdRng::seed_from_u64(7);
        let map = map(702_702);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1_000 {
            let population = roll(Some(&map), "FFA", &mut rng);
            assert!(
                (MIN_MATCHMAKING_PLAYERS..=MAX_MATCHMAKING_PLAYERS)
                    .contains(&population.max_players)
            );
            seen.insert(population.max_players);
        }
        assert!(seen.len() > 20, "capacity roll is not varied: {seen:?}");
        assert!(seen.iter().any(|capacity| *capacity >= 128));
    }

    #[test]
    fn teams_capacity_is_even() {
        let mut rng = StdRng::seed_from_u64(11);
        let map = map(420_244);
        for _ in 0..1_000 {
            let population = roll(Some(&map), "Teams", &mut rng);
            assert_eq!(population.max_players % 2, 0);
            assert!(
                (MIN_MATCHMAKING_PLAYERS..=MAX_MATCHMAKING_PLAYERS)
                    .contains(&population.max_players)
            );
        }
    }

    #[test]
    fn matchmaking_modes_keep_full_tribe_pool_and_drop_nations() {
        let full_tribe_pool = GameConfig::max_tribe_bots();
        // Tribes promoted to playable heroes leave the bot pool (Wiradjuri: 401 -> 400).
        assert_eq!(full_tribe_pool, 400, "matchmaking requires 400 tribes");
        let map = map(702_702);

        for (seed, mode) in [(29, "FFA"), (31, "Teams")] {
            let mut rng = StdRng::seed_from_u64(seed);

            for _ in 0..100 {
                let population = roll(Some(&map), mode, &mut rng);
                assert_eq!(population.bot_count, full_tribe_pool, "mode={mode}");
                assert_eq!(population.nation_count, 0, "mode={mode}");
            }
        }
    }

    #[test]
    fn coop_starts_without_extra_nations_and_finalizes_to_human_roster() {
        let mut rng = StdRng::seed_from_u64(13);
        let population = roll(Some(&map(420_244)), "Coop", &mut rng);
        assert_eq!(population.bot_count, GameConfig::max_tribe_bots());
        assert_eq!(population.nation_count, 0);

        let mut config = GameConfig {
            game_mode: "Coop".to_string(),
            bot_count: population.bot_count,
            nation_count: population.nation_count,
            ..Default::default()
        };
        assert!(finalize_for_start(&mut config, 37));
        assert_eq!(config.bot_count, GameConfig::max_tribe_bots());
        assert_eq!(config.nation_count, 37);
    }
}
