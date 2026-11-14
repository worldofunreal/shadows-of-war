use sow_data::PREMIUM_COLORS;
use super::{Player, PlayerType};

/// Shared Red/Blue territory color for multiplayer teams.
#[inline]
pub fn team_territory_rgb(team: crate::protocol::Team) -> [f32; 3] {
    match team {
        crate::protocol::Team::Red => [1.0, 0.2, 0.2],
        crate::protocol::Team::Blue => [0.2, 0.5, 1.0],
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
        return [0.5; 3];
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

