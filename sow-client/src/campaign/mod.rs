//! Campaign scripting. An *episode* places a fixed roster of named, teamed, colored bots on a
//! map; the engine consumes it as `GameConfig.scripted_spawns` (see `sow_core::game_config::
//! ScriptedSpawn`). Adding an episode = a new submodule returning `Vec<Faction>` + the player's
//! homeland tile. A faction's `Role` fixes its **team, color, AI, and troop tier** in one place,
//! so allegiance + difficulty read at a glance and stay consistent across episodes.

pub mod boudica;
pub mod dialog;
pub mod lady_six_sky;

use sow_core::game_config::ScriptedSpawn;
use sow_core::player::{Civilization, Leader};
use sow_core::protocol::Team;

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

/// A faction's role fixes its team, color, AI, and troop tier. The difficulty ladder runs
/// Independent (500) → Vassal (1 000) → Boss (2 500) → BigBoss (5 000); the player (Boudica)
/// starts at 1 000 and grows by conquest, so the ladder climbs as the campaign progresses.
#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    /// Boudica's kin/allies — Team Red, Boudica's color, passive (config troops).
    Kin,
    /// Lone clan — gray, no team, passive, **500** (the easy first-blood targets).
    Independent,
    /// Rome's client tribe — Team Blue, passive, **1 000** (a "vassal" of the bosses).
    Vassal,
    /// A Roman city — Team Blue, expanding nation, **2 500**.
    Boss,
    /// Rome itself — Team Blue, expanding nation, **5 000** (the apex).
    BigBoss,
    /// Unaligned bystander — its own color, no team, passive, **500**.
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
    /// `(team, expanding-nation?)`. Only the bosses + big boss actively expand.
    fn team_and_ai(self) -> (Option<Team>, bool) {
        match self {
            Role::Kin => (Some(Team::Red), false),
            Role::Vassal => (Some(Team::Blue), false),
            Role::Boss | Role::BigBoss => (Some(Team::Blue), true),
            Role::Independent | Role::Neutral => (None, false),
        }
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
    pub civ: Civilization,
    /// Bot intelligence override; `None` = engine default. Only the JSON loader sets it.
    pub iq: Option<u32>,
    /// Portrait/perk identity override; `None` = the role default (bosses Caesar,
    /// kin the episode advisor). Only the JSON loader sets it.
    pub leader: Option<Leader>,
}

impl Faction {
    /// Build a faction; civ is implied by role so it stays consistent between the hardcoded
    /// roster and the JSON loader.
    fn new(name: impl Into<String>, x: u32, y: u32, role: Role) -> Faction {
        Faction {
            name: name.into(),
            x,
            y,
            role,
            civ: role.civ(),
            iq: None,
            leader: None,
        }
    }
}

// Rosters are authored only as JSON (assets/campaign/*.json) and built via `parse_roster` →
// `Faction::new`; there are deliberately no hand-rolled `kin()/boss()/…` builders, so there is one
// and only one way to define a faction. `Faction::new` stays private to this module.

/// Boudica's territory color — kin share it so the rebellion reads as one bloc.
/// (Mirrors `Leader::Boudica.filler_rgb()`.)
pub const ALLY_COLOR: [f32; 3] = [0.88, 0.42, 0.12];
const ROME_BLUE: [f32; 3] = [0.20, 0.45, 0.95];
const TRIBE_GRAY: [f32; 3] = [0.58, 0.58, 0.62];

/// Distinct own-colors for the neutral bystander factions (Welsh tribes, Gaul).
const NEUTRAL_PALETTE: [[f32; 3]; 6] = [
    [0.30, 0.65, 0.45], // green
    [0.66, 0.55, 0.25], // ochre
    [0.55, 0.35, 0.66], // purple
    [0.25, 0.60, 0.62], // teal
    [0.72, 0.45, 0.40], // clay
    [0.45, 0.50, 0.72], // slate
];

/// The human's team in the rebellion (Boudica leads Red).
pub const PLAYER_TEAM: Team = Team::Red;

/// Log the episode roster grouped by allegiance, so the console shows at a glance who is on
/// which side before the engine places them. (The engine then logs each actual placement.)
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
        "campaign:   TEAM RED (us): [player] + kin {}",
        join(&[Role::Kin])
    );
    log::info!(
        "campaign:   TEAM BLUE (foes): big-boss {} | bosses {} | vassals {}",
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
    let mut neutral_i = 0usize;
    factions
        .iter()
        .map(|f| {
            let (team, is_nation) = f.role.team_and_ai();
            let color = match f.role {
                Role::Kin => ALLY_COLOR,
                Role::Vassal | Role::Boss | Role::BigBoss => ROME_BLUE,
                Role::Independent => TRIBE_GRAY,
                Role::Neutral => {
                    let c = NEUTRAL_PALETTE[neutral_i % NEUTRAL_PALETTE.len()];
                    neutral_i += 1;
                    c
                }
            };
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
                color,
                team,
                leader,
                civilization: f.civ,
                is_nation,
                troops: f.role.troops(),
                troop_cap: f.role.troop_cap(),
                iq: f.iq,
            }
        })
        .collect()
}

// ---- Data-driven rosters (authored visually by tools/campaign-editor) ----

/// One faction in a JSON roster file. Names/positions/roles only; everything else (team, color,
/// troops, civ) is derived from `role`, exactly like the hardcoded builders — so the JSON stays a
/// thin authoring surface and can't drift the balance model.
#[derive(serde::Deserialize)]
struct RosterEntry {
    name: String,
    x: u32,
    y: u32,
    role: String,
    #[serde(default)]
    iq: Option<u32>,
    /// Civilization override (`"maya"`, …); absent = the role default.
    #[serde(default)]
    civ: Option<String>,
    /// Leader override (`"lady_six_sky"`, …); absent = the role default.
    #[serde(default)]
    leader: Option<String>,
}

#[derive(serde::Deserialize)]
struct RosterFile {
    #[serde(default)]
    player_spawn: Option<(u32, u32)>,
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
pub fn parse_roster(text: &str) -> Option<(Vec<Faction>, (u32, u32))> {
    let rf: RosterFile = serde_json::from_str(text).ok()?;
    let factions: Vec<Faction> = rf
        .factions
        .iter()
        .filter_map(|e| {
            Role::from_name(&e.role).map(|role| {
                let mut f = Faction::new(e.name.clone(), e.x, e.y, role);
                f.iq = e.iq;
                if let Some(civ) = e.civ.as_deref().and_then(civ_from_id) {
                    f.civ = civ;
                }
                if let Some(leader) = e
                    .leader
                    .as_deref()
                    .and_then(sow_data::commerce::leader_from_id)
                {
                    f.leader = Some(leader);
                }
                f
            })
        })
        .collect();
    if factions.is_empty() {
        return None;
    }
    Some((factions, rf.player_spawn.unwrap_or((696, 45))))
}

/// Load a roster from a JSON file on disk (native authoring loop). Thin wrapper over `parse_roster`.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_roster_json(path: &str) -> Option<(Vec<Faction>, (u32, u32))> {
    parse_roster(&std::fs::read_to_string(path).ok()?)
}
