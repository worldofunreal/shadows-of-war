//! Campaign data validation and conversion. The browser owns episode JSON and sends a validated
//! roster to the engine as `GameConfig.scripted_spawns`; Rust keeps only the generic faction rules
//! that turn a role into team, color, AI, and troop tier.

use sow_core::game_config::ScriptedSpawn;
use sow_core::game_config::{CampaignBetrayal, CampaignHostility};
use sow_core::player::{Civilization, Leader};
use sow_core::protocol::CampaignRelation;

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

    /// Stable id for progress tracking + Poki `measure()` events.
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
            CampaignId::SixSkyEp1 => CampaignId::Boudica.is_completed(progress),
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

/// A faction's role fixes its AI tier, civilization default, and starting troop tier. The ladder runs
/// Independent (500) → Vassal (1 000) → Boss (2 500) → BigBoss (5 000); the player (Boudica)
/// starts at 1 000 and grows by conquest, so the ladder climbs as the campaign progresses.
#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    /// Iceni identity; diplomacy is configured per faction, not by this role.
    Kin,
    /// Lone clan — **500** starting troops.
    Independent,
    /// Client tribe — **1 000** starting troops.
    Vassal,
    /// A city — expanding nation, **2 500**.
    Boss,
    /// A major power — expanding nation, **5 000** (the apex).
    BigBoss,
    /// Unaligned bystander — **500** starting troops.
    Neutral,
}

impl Role {
    /// **Starting** troop count for this tier — the unit then grows normally with territory
    /// (so an expanding tribe's nameplate climbs, not freezes). The ladder is the head start.
    fn troops(self) -> Option<f64> {
        match self {
            Role::Kin | Role::Independent | Role::Neutral => Some(500.0),
            Role::Vassal => Some(1000.0),
            Role::Boss => Some(2500.0),
            Role::BigBoss => Some(5000.0),
        }
    }
    /// **Hard** max-troop ceiling. Currently unused (`None` for all) — every faction grows
    /// naturally with territory, so nameplates track reality (no frozen-at-500 look). Kept as a
    /// knob in case a future flavor unit must stay pinned; allies stay small by being passive +
    /// starting at 500, not by a hard cap.
    fn troop_cap(self) -> Option<f64> {
        None
    }
    /// Only the city and major-power tiers use the expanding nation AI.
    fn is_nation(self) -> bool {
        matches!(self, Role::Boss | Role::BigBoss)
    }
    /// Civilization implied by the role (kin = Iceni, Rome's cities/empire = Rome, rest Gallic).
    fn civ(self) -> Civilization {
        match self {
            Role::Kin => Civilization::Iceni,
            Role::Boss | Role::BigBoss => Civilization::Rome,
            _ => Civilization::Gallic,
        }
    }
    /// Parse a role name from a data file (the JSON roster authored by tools/campaign-editor).
    fn from_name(s: &str) -> Option<Role> {
        Some(match s {
            "kin" => Role::Kin,
            "independent" => Role::Independent,
            "vassal" => Role::Vassal,
            "boss" => Role::Boss,
            "big_boss" => Role::BigBoss,
            "neutral" => Role::Neutral,
            _ => return None,
        })
    }
}

/// One placed faction in an episode roster. `name` is owned so rosters can come from a data file
/// (the JSON authored by tools/campaign-editor), not only from `&'static` literals.
pub struct Faction {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub role: Role,
    pub relation: CampaignRelation,
    pub hostility: CampaignHostility,
    pub betrayal: CampaignBetrayal,
    pub color: [f32; 3],
    pub civ: Civilization,
    /// Bot intelligence override; `None` = engine default. Only the JSON loader sets it.
    pub iq: Option<u32>,
    /// Portrait/perk identity override; `None` = the role default (bosses Caesar,
    /// kin the episode advisor). Only the JSON loader sets it.
    pub leader: Option<Leader>,
    /// Shared by the map nameplate and story dialogue; absent uses the generic portrait.
    pub avatar: Option<String>,
    pub support_interval_seconds: Option<u32>,
    pub alliance_group: Option<String>,
}

impl Faction {
    /// Build a faction; civ is implied by role so it stays consistent between the hardcoded
    /// roster and the JSON loader.
    fn new(name: impl Into<String>, x: u32, y: u32, role: Role, color: [f32; 3]) -> Faction {
        Faction {
            name: name.into(),
            x,
            y,
            role,
            relation: CampaignRelation::Neutral,
            hostility: CampaignHostility::Passive,
            betrayal: CampaignBetrayal::Never,
            color,
            civ: role.civ(),
            iq: None,
            leader: None,
            avatar: None,
            support_interval_seconds: None,
            alliance_group: None,
        }
    }
}

// Rosters are authored only as JSON (assets/campaign/*.json) and built via `parse_roster` →
// `Faction::new`; there are deliberately no hand-rolled `kin()/boss()/…` builders, so there is one
// and only one way to define a faction. `Faction::new` stays private to this module.

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

/// Log the episode roster grouped by role before the engine places it.
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
    let join = |roles: &[Role]| -> String {
        let v: Vec<&str> = factions
            .iter()
            .filter(|f| roles.contains(&f.role))
            .map(|f| f.name.as_str())
            .collect();
        if v.is_empty() {
            "(none)".into()
        } else {
            v.join(", ")
        }
    };
    log::info!(
        "campaign: {} — player ({}) spawns at ({},{}); {} scripted bots",
        episode,
        player_desc,
        player_spawn.0,
        player_spawn.1,
        factions.len()
    );
    log::info!(
        "campaign:   major powers {} | cities {} | client tribes {}",
        join(&[Role::BigBoss]),
        join(&[Role::Boss]),
        join(&[Role::Vassal])
    );
    log::info!(
        "campaign:   INDEPENDENT (gray, 500): {}",
        join(&[Role::Independent])
    );
    log::info!(
        "campaign:   NEUTRAL (own colors): {}",
        join(&[Role::Neutral])
    );
}

/// Turn an episode's faction list into engine-ready scripted spawns (team + color + tier).
pub fn to_scripted(factions: &[Faction]) -> Vec<ScriptedSpawn> {
    factions
        .iter()
        .map(|f| {
            let is_nation = f.role.is_nation();
            let leader = match f.role {
                Role::Boss | Role::BigBoss => Leader::Caesar,
                Role::Kin => Leader::Boudica,
                _ => Leader::default(),
            };
            let leader = f.leader.unwrap_or(leader);
            ScriptedSpawn {
                name: f.name.clone(),
                x: f.x,
                y: f.y,
                color: f.color,
                team: None,
                leader,
                civilization: f.civ,
                is_nation,
                troops: f.role.troops(),
                troop_cap: f.role.troop_cap(),
                iq: f.iq,
                campaign_avatar: Some(f.avatar.clone().unwrap_or_else(|| "null".into())),
                campaign_support_interval_seconds: f.support_interval_seconds,
                campaign_alliance_group: f.alliance_group.clone(),
                campaign_relation: Some(f.relation),
                campaign_hostility: Some(f.hostility),
                campaign_betrayal: Some(f.betrayal),
            }
        })
        .collect()
}

// ---- Data-driven rosters (authored visually by tools/campaign-editor) ----

/// Faction identity, diplomacy, and presentation come from the same editor-authored roster.
#[derive(serde::Deserialize)]
struct RosterEntry {
    name: String,
    x: u32,
    y: u32,
    role: String,
    #[serde(default)]
    relation: CampaignRelation,
    #[serde(default)]
    hostility: CampaignHostility,
    #[serde(default)]
    betrayal: CampaignBetrayal,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    iq: Option<u32>,
    /// Civilization override (`"maya"`, …); absent = the role default.
    #[serde(default)]
    civ: Option<String>,
    /// Leader override (`"lady_six_sky"`, …); absent = the role default.
    #[serde(default)]
    leader: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    support_interval_seconds: Option<u32>,
    #[serde(default)]
    alliance_group: Option<String>,
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

/// Resolve a civilization id from roster JSON (`"maya"`, `"Maya"`,
/// `"Maya Civilization"`, …). `None` = unknown, keep the role default.
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

/// Parse an episode roster from JSON text. `None` on any problem (bad JSON, unknown role, empty
/// list). This is the **single** roster code path — shared by the runtime file override and the
/// embedded committed default — so there is exactly one format and no second way to define a roster.
pub fn parse_roster(text: &str) -> Option<(Vec<Faction>, (u32, u32), [f32; 3])> {
    let rf: RosterFile = serde_json::from_str(text).ok()?;
    let mut factions = Vec::with_capacity(rf.factions.len());
    let mut names = std::collections::HashSet::new();
    for (index, e) in rf.factions.iter().enumerate() {
        if e.name.trim().is_empty() || !names.insert(e.name.as_str()) {
            return None;
        }
        let role = Role::from_name(&e.role)?;
        if e.support_interval_seconds.is_some_and(|seconds| !(5..=600).contains(&seconds))
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
        let mut f = Faction::new(e.name.clone(), e.x, e.y, role, color);
        f.relation = e.relation;
        f.hostility = e.hostility;
        f.betrayal = e.betrayal;
        f.iq = e.iq;
        f.support_interval_seconds = e.support_interval_seconds;
        f.alliance_group = e.alliance_group.clone();
        if let Some(civ_name) = e.civ.as_deref() {
            f.civ = civ_from_id(civ_name)?;
        }
        if let Some(leader_name) = e.leader.as_deref() {
            f.leader = Some(sow_data::commerce::leader_from_id(leader_name)?);
        }
        if let Some(avatar) = e.avatar.as_deref() {
            if !valid_avatar_id(avatar) {
                return None;
            }
            f.avatar = Some(avatar.to_string());
        }
        factions.push(f);
    }
    if factions.is_empty() {
        return None;
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
    let channel = |start| u8::from_str_radix(&hex[start..start + 2], 16).ok().map(|value| value as f32 / 255.0);
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn valid_avatar_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn valid_campaign_group_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}
