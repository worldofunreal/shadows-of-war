use sow_data::PREMIUM_COLORS;
use super::{Player, PlayerType};

/// Red/Blue tint used to keep teams recognizable while preserving each
/// player's own territory color.
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
) -> [f32; 3] {
    if player_type == PlayerType::Bot {
        return [0.5; 3];
    }
    const TEAM_TINT: f32 = 0.08;
    let color = team.map_or(color, |team| {
        let team_color = team_territory_rgb(team);
        std::array::from_fn(|i| color[i] * (1.0 - TEAM_TINT) + team_color[i] * TEAM_TINT)
    });
    if player_type == crate::player::PlayerType::Nation {
        muted_nation_rgb(color)
    } else {
        color
    }
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

/// Pick the free Nation color that stays farthest from the colors in this match.
/// Used for breakaway Nations, whose team can differ from the catalog Nations.
pub fn best_available_nation_palette_color(
    players: &[Player],
    team: Option<crate::protocol::Team>,
) -> Option<[f32; 3]> {
    const MIN_COLOR_GAP: f32 = 0.03;
    let used = used_nation_palette_slots(players);
    let existing: Vec<[f32; 3]> = players
        .iter()
        .filter(|player| {
            player.alive
                && matches!(player.player_type, PlayerType::Human | PlayerType::Nation)
        })
        .map(|player| {
            player_territory_rgb(player.player_type, player.team, player.color)
        })
        .map(oklab2)
        .collect();

    PREMIUM_COLORS
        .iter()
        .enumerate()
        .filter(|(index, _)| !used[*index])
        .filter_map(|(_, color)| {
            let rendered = oklab2(player_territory_rgb(
                PlayerType::Nation,
                team,
                color.rgb,
            ));
            let gap = existing
                .iter()
                .map(|other| distance_ok2(rendered, *other))
                .fold(f32::INFINITY, f32::min);
            (gap >= MIN_COLOR_GAP).then_some((gap, color.rgb))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, color)| color)
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

fn oklab2(rgb: [f32; 3]) -> [f32; 3] {
    let linear = rgb.map(|channel| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    let [r, g, b] = linear;
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0519036911 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
        2.0 * (1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s),
        2.0 * (0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s),
    ]
}

fn distance_ok2(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn muted_nation_rgb(color: [f32; 3]) -> [f32; 3] {
    const MAX_CHANNEL: f32 = 0.72;
    let gray = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
    let muted = color.map(|channel| channel * 0.5 + gray * 0.5);
    let peak = muted[0].max(muted[1]).max(muted[2]);
    let scale = (MAX_CHANNEL / peak.max(0.001)).min(1.0);
    muted.map(|channel| channel * scale)
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

