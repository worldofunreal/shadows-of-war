//! Campaign data validation and conversion. The browser owns episode JSON and sends a validated
//! roster to the engine as `GameConfig.scripted_spawns`.

use sow_core::game_config::ScriptedSpawn;
use sow_core::player::{Civilization, Leader};
use sow_core::protocol::{CampaignRelation, Team};

/// Which scripted campaign the running tutorial match belongs to. Boudica is
/// the first-run teaching intro; the Six Sky episodes are the retention chain
/// that follows it.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum CampaignId {
    #[default]
    Boudica,
    SixSkyEp1,
    SixSkyEp2,
    SixSkyEp3,
}

impl CampaignId {
    pub const ALL: [CampaignId; 4] = [
        CampaignId::Boudica,
        CampaignId::SixSkyEp1,
        CampaignId::SixSkyEp2,
        CampaignId::SixSkyEp3,
    ];

    /// Stable id for campaign progress and analytics events.
    pub fn episode_id(self) -> &'static str {
        match self {
            CampaignId::Boudica => "boudica",
            CampaignId::SixSkyEp1 => "six_sky_ep1",
            CampaignId::SixSkyEp2 => "six_sky_ep2",
            CampaignId::SixSkyEp3 => "six_sky_ep3",
        }
    }

    pub fn from_episode_id(id: &str) -> Option<Self> {
        match id {
            "boudica" => Some(CampaignId::Boudica),
            "six_sky_ep1" => Some(CampaignId::SixSkyEp1),
            "six_sky_ep2" => Some(CampaignId::SixSkyEp2),
            "six_sky_ep3" => Some(CampaignId::SixSkyEp3),
            _ => None,
        }
    }

    pub fn menu_title(self) -> &'static str {
        match self {
            CampaignId::Boudica => "Rise of the Iceni",
            CampaignId::SixSkyEp1 => "Arrival",
            CampaignId::SixSkyEp2 => "Regent's Fire",
            CampaignId::SixSkyEp3 => "Moon Goddess",
        }
    }

    pub fn menu_subtitle(self) -> &'static str {
        match self {
            CampaignId::Boudica => "Tutorial · Boudica",
            CampaignId::SixSkyEp1 => "Lady Six Sky · 682 CE",
            CampaignId::SixSkyEp2 => "Lady Six Sky · 693 CE",
            CampaignId::SixSkyEp3 => "Lady Six Sky · 726–741 CE",
        }
    }

    pub fn menu_title_text(self) -> crate::ui::UiText {
        match self {
            CampaignId::Boudica => crate::ui::UiText::new("lobbies.campaign_boudica_title"),
            CampaignId::SixSkyEp1 => crate::ui::UiText::new("lobbies.campaign_six_sky_ep1_title"),
            CampaignId::SixSkyEp2 => crate::ui::UiText::new("lobbies.campaign_six_sky_ep2_title"),
            CampaignId::SixSkyEp3 => crate::ui::UiText::new("lobbies.campaign_six_sky_ep3_title"),
        }
    }

    pub fn menu_subtitle_text(self) -> crate::ui::UiText {
        match self {
            CampaignId::Boudica => crate::ui::UiText::new("lobbies.campaign_boudica_subtitle"),
            CampaignId::SixSkyEp1 => {
                crate::ui::UiText::new("lobbies.campaign_six_sky_ep1_subtitle")
            }
            CampaignId::SixSkyEp2 => {
                crate::ui::UiText::new("lobbies.campaign_six_sky_ep2_subtitle")
            }
            CampaignId::SixSkyEp3 => {
                crate::ui::UiText::new("lobbies.campaign_six_sky_ep3_subtitle")
            }
        }
    }

    pub fn is_completed(self, progress: &crate::player_progress::PlayerProgress) -> bool {
        match self {
            CampaignId::Boudica => progress.intro_completed.unwrap_or(false),
            _ => progress.completed_episodes.contains(self.episode_id()),
        }
    }

    pub fn is_unlocked(self, progress: &crate::player_progress::PlayerProgress) -> bool {
        match self {
            CampaignId::Boudica => true,
            // Owner decision: episode 1 is not finished yet, so the whole Six Sky
            // chain stays locked. Only the Boudica tutorial can be replayed.
            CampaignId::SixSkyEp1 => false,
            CampaignId::SixSkyEp2 => CampaignId::SixSkyEp1.is_completed(progress),
            CampaignId::SixSkyEp3 => CampaignId::SixSkyEp2.is_completed(progress),
        }
    }

    pub fn advisor(self) -> Leader {
        match self {
            CampaignId::Boudica => Leader::Boudica,
            CampaignId::SixSkyEp1 | CampaignId::SixSkyEp2 | CampaignId::SixSkyEp3 => {
                Leader::LadySixSky
            }
        }
    }
}

/// One placed episode faction. Identity is stable; the display name can change independently.
pub struct Faction {
    pub id: String,
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub starting_troops: u32,
    pub relation: CampaignRelation,
    pub team: Option<Team>,
    pub can_request_alliance: bool,
    pub color: [f32; 3],
    pub civ: Civilization,
    /// Bot intelligence override; `None` = engine default. Only the JSON loader sets it.
    pub iq: Option<u32>,
    pub leader: Leader,
    /// Shared by the map nameplate and story dialogue; absent uses the generic portrait.
    pub avatar: Option<String>,
    pub support_interval_seconds: Option<u32>,
    pub gold_loot_bonus: Option<u32>,
    pub gold_loot_override: Option<u32>,
    pub alliance_group: Option<String>,
    pub assault_force: bool,
}

// Rosters are authored only as JSON (assets/campaign/*.json) and parsed through this module.

/// Distinct own-colors for the neutral bystander factions (Welsh tribes, Gaul).
const NEUTRAL_PALETTE: [[f32; 3]; 6] = [
    [0.30, 0.65, 0.45], // green
    [0.66, 0.55, 0.25], // ochre
    [0.55, 0.35, 0.66], // purple
    [0.25, 0.60, 0.62], // teal
    [0.72, 0.45, 0.40], // clay
    [0.45, 0.50, 0.72], // slate
];

pub const PLAYER_COLOR: [f32; 3] = [0.94, 0.56, 0.16];

/// Log the episode roster summary before the engine places it.
pub fn log_plan(episode: &str, player_spawn: (u32, u32), factions: &[Faction]) {
    log_plan_for(episode, "Boudica/Iceni, 1000", player_spawn, factions);
}

/// Same as [`log_plan`] with an explicit player label for non-Boudica episodes.
pub fn log_plan_for(
    episode: &str,
    player_desc: &str,
    player_spawn: (u32, u32),
    factions: &[Faction],
) {
    log::info!(
        "campaign: {} — player ({}) spawns at ({},{}); {} bots ({} allied, {} neutral, {} enemy)",
        episode,
        player_desc,
        player_spawn.0,
        player_spawn.1,
        factions.len(),
        factions
            .iter()
            .filter(|f| f.relation == CampaignRelation::Allied)
            .count(),
        factions
            .iter()
            .filter(|f| f.relation == CampaignRelation::Neutral)
            .count(),
        factions
            .iter()
            .filter(|f| f.relation == CampaignRelation::Enemy)
            .count(),
    );
}

/// Turn an episode's faction list into engine-ready normal bot spawns.
pub fn to_scripted(factions: &[Faction]) -> Vec<ScriptedSpawn> {
    factions
        .iter()
        .map(|f| ScriptedSpawn {
            name: f.name.clone(),
            x: f.x,
            y: f.y,
            color: f.color,
            team: f.team,
            leader: f.leader,
            civilization: f.civ,
            troops: Some(f.starting_troops as f64),
            iq: f.iq,
            campaign_avatar: Some(f.avatar.clone().unwrap_or_else(|| "null".into())),
            campaign_support_interval_seconds: f.support_interval_seconds,
            campaign_gold_loot_bonus: f.gold_loot_bonus,
            campaign_gold_loot_override: f.gold_loot_override,
            campaign_alliance_group: f.alliance_group.clone(),
            campaign_relation: Some(f.relation),
            campaign_faction_id: Some(f.id.clone()),
            campaign_can_request_alliance: Some(f.can_request_alliance),
            campaign_assault_force: f.assault_force,
        })
        .collect()
}

// ---- Data-driven rosters (authored visually by tools/campaign-editor) ----

/// Faction identity, diplomacy, and presentation come from the same editor-authored roster.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RosterEntry {
    id: String,
    name: String,
    x: u32,
    y: u32,
    starting_troops: u32,
    relation: CampaignRelation,
    #[serde(default)]
    team: Option<Team>,
    #[serde(default)]
    can_request_alliance: Option<bool>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    iq: Option<u32>,
    civ: String,
    leader: String,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    support_interval_seconds: Option<u32>,
    #[serde(default)]
    gold_loot_bonus: Option<u32>,
    #[serde(default)]
    gold_loot_override: Option<u32>,
    #[serde(default)]
    alliance_group: Option<String>,
    #[serde(default)]
    assault_force: bool,
}

#[derive(serde::Deserialize)]
struct RosterFile {
    #[serde(default)]
    player_spawn: Option<(u32, u32)>,
    #[serde(default)]
    player_color: Option<String>,
    #[serde(default)]
    factions: Vec<RosterEntry>,
}

pub(crate) fn tutorial_camera_frame(
    campaign: CampaignId,
    config: &sow_core::game_config::GameConfig,
    screen_w: f32,
    screen_h: f32,
) -> Option<((f32, f32), f32)> {
    if !config.tutorial || campaign != CampaignId::Boudica {
        return None;
    }
    let (player_x, player_y) = config.player_spawn?;
    let target = config
        .scripted_spawns
        .iter()
        .find(|spawn| spawn.campaign_faction_id.as_deref() == Some("roman_outpost"))?;
    let dx = player_x.abs_diff(target.x) as f32;
    let dy = player_y.abs_diff(target.y) as f32;
    let center = (
        (player_x as f32 + target.x as f32 + 1.0) * 0.5,
        (player_y as f32 + target.y as f32 + 1.0) * 0.5,
    );
    let fit_zoom = (screen_w.max(1.0) / (dx + 7.0))
        .min(screen_h.max(1.0) / (dy + 7.0))
        .min(crate::camera_zoom_upper_bound(screen_w, screen_h));
    let min_zoom =
        crate::camera_zoom_lower_bound(screen_w, screen_h, config.map_width, config.map_height);
    Some((center, fit_zoom.max(min_zoom)))
}

/// Resolve a civilization from its roster value (`"maya"`, `"Maya"`,
/// `"Maya Civilization"`, …).
fn civ_from_id(value: &str) -> Option<Civilization> {
    let normalized: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    Civilization::ALL.into_iter().find(|civ| {
        [civ.name()]
            .into_iter()
            .map(|candidate| {
                candidate
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .map(|c| c.to_ascii_lowercase())
                    .collect::<String>()
            })
            .any(|candidate| candidate == normalized)
    })
}

/// Parse an episode roster from JSON text. `None` on any problem (invalid identity, empty
/// list). This is the **single** roster code path — shared by the runtime file override and the
/// embedded committed default — so there is exactly one format and no second way to define a roster.
pub fn parse_roster(text: &str) -> Option<(Vec<Faction>, (u32, u32), [f32; 3])> {
    let rf: RosterFile = serde_json::from_str(text).ok()?;
    let mut factions = Vec::with_capacity(rf.factions.len());
    let mut names = std::collections::HashSet::new();
    let mut ids = std::collections::HashSet::new();
    for (index, e) in rf.factions.iter().enumerate() {
        if !valid_campaign_entity_id(&e.id)
            || e.name.trim().is_empty()
            || e.starting_troops > 1_000_000
            || !ids.insert(e.id.as_str())
            || !names.insert(e.name.as_str())
        {
            return None;
        }
        if e.support_interval_seconds
            .is_some_and(|seconds| !(5..=600).contains(&seconds))
            || e.gold_loot_bonus.is_some_and(|gold| gold > 1_000_000)
            || e.gold_loot_override.is_some_and(|gold| gold > 1_000_000)
            || e.alliance_group
                .as_deref()
                .is_some_and(|group| !valid_campaign_group_id(group))
        {
            return None;
        }
        let color = match e.color.as_deref() {
            Some(color) => parse_campaign_color(color)?,
            None => NEUTRAL_PALETTE[index % NEUTRAL_PALETTE.len()],
        };
        let civ = civ_from_id(&e.civ)?;
        let leader = sow_data::commerce::leader_from_id(&e.leader)?;
        let relation = e.relation;
        if e.avatar
            .as_deref()
            .is_some_and(|avatar| !valid_avatar_id(avatar))
        {
            return None;
        }
        factions.push(Faction {
            id: e.id.clone(),
            name: e.name.clone(),
            x: e.x,
            y: e.y,
            starting_troops: e.starting_troops,
            relation,
            team: e.team,
            can_request_alliance: e.can_request_alliance.unwrap_or(true),
            color,
            civ,
            iq: e.iq,
            leader,
            avatar: e.avatar.clone(),
            support_interval_seconds: e.support_interval_seconds,
            gold_loot_bonus: e.gold_loot_bonus,
            gold_loot_override: e.gold_loot_override,
            alliance_group: e.alliance_group.clone(),
            assault_force: e.assault_force,
        });
    }
    let player_color = match rf.player_color.as_deref() {
        Some(color) => parse_campaign_color(color)?,
        None => PLAYER_COLOR,
    };
    Some((factions, rf.player_spawn.unwrap_or((696, 45)), player_color))
}

pub fn parse_campaign_color(value: &str) -> Option<[f32; 3]> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |start| {
        u8::from_str_radix(&hex[start..start + 2], 16)
            .ok()
            .map(|value| value as f32 / 255.0)
    };
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn valid_avatar_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn valid_campaign_entity_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.count() < 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_campaign_group_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::{CampaignId, parse_roster, to_scripted, tutorial_camera_frame};
    use sow_core::game_config::GameConfig;
    use sow_core::protocol::{CampaignRelation, Team};
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn boudica_tutorial_camera_frame_stays_on_the_first_target_on_desktop_and_mobile() {
        let (factions, player_spawn, _) =
            parse_roster(include_str!("../../../assets/campaign/boudica.json")).unwrap();
        let config = GameConfig {
            tutorial: true,
            player_spawn: Some(player_spawn),
            scripted_spawns: to_scripted(&factions),
            ..GameConfig::default()
        };

        for (screen, expected_zoom) in [((1440.0, 900.0), 100.0), ((390.0, 844.0), 390.0 / 14.0)] {
            let (center, zoom) =
                tutorial_camera_frame(CampaignId::Boudica, &config, screen.0, screen.1).unwrap();
            assert_eq!(center, (719.0, 79.5));
            assert_eq!(zoom, expected_zoom);
        }
    }

    #[test]
    fn gold_loot_bonus_survives_roster_parse_and_scripted_spawn_conversion() {
        let roster = r##"{"player_spawn":[1,1],"factions":[{"id":"rome","name":"Rome","x":2,"y":1,"starting_troops":1000,"relation":"enemy","civ":"Roman Empire","leader":"Caesar","gold_loot_bonus":275}]}"##;
        let (factions, _, _) = parse_roster(roster).unwrap();
        assert_eq!(factions[0].gold_loot_bonus, Some(275));
        assert_eq!(
            to_scripted(&factions)[0].campaign_gold_loot_bonus,
            Some(275)
        );
        assert_eq!(
            to_scripted(&factions)[0].campaign_faction_id.as_deref(),
            Some("rome")
        );
        assert_eq!(to_scripted(&factions)[0].troops, Some(1000.0));

        let invalid = r##"{"factions":[{"id":"rome","name":"Rome","x":2,"y":1,"starting_troops":1000,"relation":"enemy","civ":"Roman Empire","leader":"Caesar","gold_loot_bonus":1000001}]}"##;
        assert!(parse_roster(invalid).is_none());
    }

    #[test]
    fn roster_identity_and_starting_troops_are_required_and_unique() {
        let entry = r##"{"id":"rome","name":"Rome","x":2,"y":1,"starting_troops":0,"relation":"neutral","civ":"Roman Empire","leader":"Caesar"}"##;
        let roster = format!(r##"{{"factions":[{entry}]}}"##);
        let (factions, _, _) = parse_roster(&roster).unwrap();
        let scripted = to_scripted(&factions);
        assert_eq!(scripted[0].troops, Some(0.0));
        assert_eq!(scripted[0].campaign_faction_id.as_deref(), Some("rome"));

        let duplicate = format!(r##"{{"factions":[{entry},{entry}]}}"##);
        assert!(parse_roster(&duplicate).is_none());
        assert!(
            parse_roster(r##"{"factions":[{"name":"Rome","x":2,"y":1,"role":"vassal"}]}"##)
                .is_none()
        );

        let empty = parse_roster(r##"{"player_spawn":[1,1],"factions":[]}"##).unwrap();
        assert!(empty.0.is_empty());
        assert!(to_scripted(&empty.0).is_empty());
    }

    #[test]
    fn campaign_relationship_and_alliance_offer_setting_reach_scripted_spawns() {
        let roster = r##"{"factions":[{"id":"outpost","name":"Roman Outpost","x":2,"y":1,"starting_troops":0,"relation":"enemy","can_request_alliance":false,"civ":"Roman Empire","leader":"Caesar","gold_loot_override":200}]}"##;
        let (factions, _, _) = parse_roster(roster).unwrap();
        let spawn = &to_scripted(&factions)[0];
        assert!(!factions[0].can_request_alliance);
        assert_eq!(factions[0].starting_troops, 0);
        assert_eq!(spawn.campaign_can_request_alliance, Some(false));
        assert_eq!(spawn.campaign_relation, Some(CampaignRelation::Enemy));
        assert_eq!(spawn.campaign_gold_loot_override, Some(200));

        let legacy = r##"{"factions":[{"id":"rome","name":"Rome","x":2,"y":1,"starting_troops":1000,"relation":"enemy","civ":"Roman Empire","leader":"Caesar"}]}"##;
        let (legacy, _, _) = parse_roster(legacy).unwrap();
        assert!(legacy[0].can_request_alliance);
        assert_eq!(
            to_scripted(&legacy)[0].campaign_relation,
            Some(CampaignRelation::Enemy)
        );
        let removed_behavior = r##"{"factions":[{"id":"outpost","name":"Roman Outpost","x":2,"y":1,"starting_troops":0,"relation":"enemy","hostility":"aggressive","civ":"Roman Empire","leader":"Caesar"}]}"##;
        assert!(parse_roster(removed_behavior).is_none());
    }

    #[test]
    fn campaign_team_enum_reaches_spawns_and_validates_assignments() {
        let roster = r##"{"factions":[{"id":"rome","name":"Rome","x":2,"y":1,"starting_troops":1000,"relation":"enemy","team":"Red","civ":"Roman Empire","leader":"Caesar"}]}"##;
        let (factions, _, _) = parse_roster(roster).unwrap();
        let spawn = &to_scripted(&factions)[0];
        assert_eq!(factions[0].team, Some(Team::Red));
        assert_eq!(spawn.team, Some(Team::Red));
        assert_eq!(spawn.campaign_relation, Some(CampaignRelation::Enemy));
        let blue_roster = roster.replace("Red", "Blue");
        assert_eq!(
            to_scripted(&parse_roster(&blue_roster).unwrap().0)[0].team,
            Some(Team::Blue)
        );

        let without_team = r##"{"factions":[{"id":"clan","name":"Clan","x":2,"y":1,"starting_troops":500,"relation":"neutral","civ":"Iceni Kingdom","leader":"Boudica"}]}"##;
        assert_eq!(
            to_scripted(&parse_roster(without_team).unwrap().0)[0].team,
            None
        );
        let invalid_team = roster.replace("Red", "Romans");
        assert!(parse_roster(&invalid_team).is_none());
    }
}
