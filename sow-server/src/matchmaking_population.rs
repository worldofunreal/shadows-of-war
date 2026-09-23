use rand::Rng;
use sow_core::game_config::GameConfig;
use sow_core::map_file::MapCatalogEntry;

pub const MIN_MATCHMAKING_PLAYERS: u32 = 12;
pub const MAX_MATCHMAKING_PLAYERS: u32 = 256;

const MIN_NEUTRAL_AI: u32 = 64;
const MAX_NEUTRAL_AI: u32 = 640;
const DEFAULT_NEUTRAL_AI: f64 = 256.0;
const AI_LAND_TILES_PER_ENTITY: f64 = 1_200.0;
const AI_VARIATION_MIN: f64 = 0.85;
const AI_VARIATION_MAX: f64 = 1.15;
const NATION_SHARE_MIN: f64 = 0.12;
const NATION_SHARE_MAX: f64 = 0.20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchmakingPopulation {
    pub max_players: u32,
    pub bot_count: u32,
    pub nation_count: u32,
}

pub fn roll(
    map: Option<&MapCatalogEntry>,
    game_mode: &str,
    rng: &mut impl Rng,
) -> MatchmakingPopulation {
    let max_players = roll_human_capacity(game_mode, rng);
    let neutral_ai = roll_neutral_ai(map.map(|entry| entry.num_land_tiles).unwrap_or(0), rng);
    let (bot_count, nation_count) = if game_mode == "HumansVsNations" {
        // HvN resolves its opposing nation side when the final roster exists.
        (neutral_ai, 0)
    } else {
        let nation_share = rng.gen_range(NATION_SHARE_MIN..=NATION_SHARE_MAX);
        let nation_count = ((neutral_ai as f64) * nation_share).round() as u32;
        (neutral_ai - nation_count, nation_count)
    };

    MatchmakingPopulation {
        max_players,
        bot_count,
        nation_count,
    }
}

pub fn finalize_for_start(config: &mut GameConfig, human_side_players: u32) -> bool {
    if config.game_mode != "HumansVsNations" {
        return false;
    }
    config.nation_count = human_side_players;
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

fn roll_neutral_ai(land_tiles: u32, rng: &mut impl Rng) -> u32 {
    let base = if land_tiles == 0 {
        DEFAULT_NEUTRAL_AI
    } else {
        land_tiles as f64 / AI_LAND_TILES_PER_ENTITY
    };
    let variation = rng.gen_range(AI_VARIATION_MIN..=AI_VARIATION_MAX);
    (base * variation)
        .round()
        .clamp(MIN_NEUTRAL_AI as f64, MAX_NEUTRAL_AI as f64) as u32
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
    fn larger_maps_get_more_neutral_ai_budget() {
        assert!(
            roll_neutral_ai(93_119, &mut StdRng::seed_from_u64(1))
                < roll_neutral_ai(702_702, &mut StdRng::seed_from_u64(1))
        );
    }

    #[test]
    fn ffa_population_splits_a_varied_map_budget() {
        let mut rng = StdRng::seed_from_u64(29);
        let map = map(702_702);
        let mut nation_counts = std::collections::HashSet::new();

        for _ in 0..100 {
            let population = roll(Some(&map), "FFA", &mut rng);
            let neutral_total = population.bot_count + population.nation_count;
            assert!((MIN_NEUTRAL_AI..=MAX_NEUTRAL_AI).contains(&neutral_total));
            assert!(population.bot_count > 0);
            assert!(population.nation_count > 0);
            nation_counts.insert(population.nation_count);
        }

        assert!(nation_counts.len() > 1);
    }

    #[test]
    fn hvn_starts_without_extra_nations_and_finalizes_to_human_roster() {
        let mut rng = StdRng::seed_from_u64(13);
        let population = roll(Some(&map(420_244)), "HumansVsNations", &mut rng);
        assert_eq!(population.nation_count, 0);

        let mut config = GameConfig {
            game_mode: "HumansVsNations".to_string(),
            nation_count: 128,
            ..Default::default()
        };
        assert!(finalize_for_start(&mut config, 37));
        assert_eq!(config.nation_count, 37);
    }
}
