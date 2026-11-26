use sow_data::PREMIUM_COLORS;
use std::sync::OnceLock;
use super::{Player, PlayerType};

/// Neutral stone gray (`#D1CDC7`) used for standard tribes (`PlayerType::Bot`),
/// matching OpenFront's `default-theme.json` `"Bot"` color.
pub const TRIBE_TERRITORY_RGB: [f32; 3] = [209.0 / 255.0, 205.0 / 255.0, 199.0 / 255.0];

/// Territory color of a multiplayer team. Team 0 and team 1 keep the original
/// Red and Blue; each later team takes the candidate color farthest from all
/// earlier ones, so the colors stay spread out for any team count.
#[inline]
pub fn team_territory_rgb(team: crate::protocol::Team) -> [f32; 3] {
    static PALETTE: OnceLock<Vec<[f32; 3]>> = OnceLock::new();
    PALETTE.get_or_init(build_team_palette)[team.index()]
}

fn build_team_palette() -> Vec<[f32; 3]> {
    let total = usize::from(crate::protocol::Team::MAX_TEAMS);
    let mut palette = vec![[1.0, 0.2, 0.2], [0.2, 0.5, 1.0]];
    while palette.len() < total {
        let mut best = ([0.0; 3], f32::MIN);
        for hue in 0..360u32 {
            for lightness in [0.42, 0.62] {
                let candidate = hsl_to_rgb(hue as f32, 0.78, lightness);
                let spacing = palette
                    .iter()
                    .map(|taken| distance_sq(*taken, candidate))
                    .fold(f32::MAX, f32::min);
                if spacing > best.1 {
                    best = (candidate, spacing);
                }
            }
        }
        palette.push(best.0);
    }
    palette
}

fn distance_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3)
        .map(|i| {
            let d = a[i] - b[i];
            d * d
        })
        .sum()
}

/// HSL to RGB, with hue in degrees.
fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    [r + m, g + m, b + m]
}

#[cfg(test)]
mod team_palette_tests {
    use super::*;
    use crate::protocol::Team;

    fn closest_pair(count: u8) -> f32 {
        let colors: Vec<[f32; 3]> = (0..count)
            .map(|i| team_territory_rgb(Team::from_index(i).unwrap()))
            .collect();
        let mut closest = f32::MAX;
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                closest = closest.min(distance_sq(*a, *b).sqrt());
            }
        }
        closest
    }

    #[test]
    fn team_colors_keep_red_and_blue_and_stay_apart() {
        assert_eq!(team_territory_rgb(Team::RED), [1.0, 0.2, 0.2]);
        assert_eq!(team_territory_rgb(Team::BLUE), [0.2, 0.5, 1.0]);
        for count in 2..=Team::MAX_TEAMS {
            let closest = closest_pair(count);
            assert!(closest > 0.15, "{count} teams: closest pair {closest}");
        }
    }
}

/// Final territory color shared by the map, nameplates, movers and feedback.
pub fn player_territory_rgb(
    player_type: crate::player::PlayerType,
    team: Option<crate::protocol::Team>,
    color: [f32; 3],
    is_campaign_faction: bool,
) -> [f32; 3] {
    // Campaign factions use Bot AI but keep their authored/diplomatic colors.
    if player_type == PlayerType::Bot && !is_campaign_faction {
        return TRIBE_TERRITORY_RGB;
    }
    let color = team.map_or(color, |team| {
        let team_color = team_territory_rgb(team);
        if is_campaign_faction {
            const CAMPAIGN_TEAM_TINT: f32 = 0.08;
            std::array::from_fn(|i| {
                color[i] * (1.0 - CAMPAIGN_TEAM_TINT) + team_color[i] * CAMPAIGN_TEAM_TINT
            })
        } else {
            team_color
        }
    });
    color
}

/// First palette color not currently used by a living Nation.
pub fn next_nation_palette_color(players: &[Player]) -> Option<[f32; 3]> {
    let used = used_nation_palette_slots(players);
    PREMIUM_COLORS
        .iter()
        .enumerate()
        .find(|(index, _)| !used[*index])
        .map(|(_, color)| color.rgb)
}

fn used_nation_palette_slots(players: &[Player]) -> Vec<bool> {
    let mut used = vec![false; PREMIUM_COLORS.len()];
    for player in players
        .iter()
        .filter(|player| player.alive && player.player_type == PlayerType::Nation)
    {
        if let Some((index, _)) = PREMIUM_COLORS
            .iter()
            .enumerate()
            .find(|(_, color)| color.rgb == player.color)
        {
            used[index] = true;
        }
    }
    used
}

/// Campaign diplomacy colors are visual state only; they do not assign gameplay teams.
#[inline]
pub fn campaign_relation_rgb(relation: crate::protocol::CampaignRelation) -> Option<[f32; 3]> {
    match relation {
        crate::protocol::CampaignRelation::Neutral => None,
        crate::protocol::CampaignRelation::Allied => Some([0.2, 0.5, 1.0]),
        crate::protocol::CampaignRelation::Enemy => Some([1.0, 0.2, 0.2]),
    }
}

