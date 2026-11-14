//! Campaign data validation and conversion. The browser owns episode JSON and sends a validated
//! roster to the engine as `GameConfig.scripted_spawns`.

use sow_core::game_config::ScriptedSpawn;
use sow_core::player::Leader;
use sow_core::protocol::{CampaignRelation, Team};

/// Which scripted campaign the running tutorial match belongs to. Boudica is
/// the first-run teaching intro; the Six Sky episodes are the retention chain
/// that follows it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CampaignId(&'static sow_data::campaign::CampaignEpisode);

impl Default for CampaignId {
    fn default() -> Self {
        Self::intro()
    }
}

impl CampaignId {
    pub fn intro() -> Self {
        Self::from_episode_id(sow_data::campaign::INTRO_EPISODE_ID)
            .expect("intro campaign episode must exist in catalog")
    }

    pub fn is_boudica(self) -> bool {
        self.0.id == "boudica"
    }

    pub fn all() -> impl Iterator<Item = CampaignId> {
        sow_data::campaign::CAMPAIGN_EPISODES.iter().map(Self)
    }

    /// Stable id for campaign progress and analytics events.
    pub fn episode_id(self) -> &'static str {
        self.0.id
    }

    pub fn from_episode_id(id: &str) -> Option<Self> {
        sow_data::campaign::episode(id).map(Self)
    }

    /// Map asset needed to start this episode. Used to overlap first-run map
    /// download with the browser's campaign roster and trigger requests.
    pub fn map_id(self) -> &'static str {
        self.0.map_id
    }

    pub fn map_dimensions(self) -> (u32, u32) {
        (self.0.map_width, self.0.map_height)
    }

    pub fn menu_title(self) -> &'static str {
        self.0.title
    }

    pub fn menu_subtitle(self) -> &'static str {
        self.0.subtitle
    }

    pub fn menu_title_text(self) -> crate::ui::UiText {
        crate::ui::UiText::new(self.0.title_key)
    }

    pub fn menu_subtitle_text(self) -> crate::ui::UiText {
        crate::ui::UiText::new(self.0.subtitle_key)
    }

    pub fn is_intro_tutorial(self) -> bool {
        self.0.marks_intro_completed
    }

    pub fn is_server_owned_tutorial(self) -> bool {
        self.0.server_owned_tutorial
    }

    pub fn intro_completed(progress: &crate::player_progress::PlayerProgress) -> bool {
        progress.intro_completed.unwrap_or(false)
            || progress
                .completed_episodes
                .contains(sow_data::campaign::INTRO_EPISODE_ID)
            || progress.completed_episodes.contains("boudica")
    }

    pub fn is_completed(self, progress: &crate::player_progress::PlayerProgress) -> bool {
        if self.0.server_owned_tutorial {
            progress.intro_completed.unwrap_or(false)
                || progress.completed_episodes.contains(self.0.id)
        } else {
            progress.completed_episodes.contains(self.0.id)
        }
    }

    pub fn is_unlocked(self, progress: &crate::player_progress::PlayerProgress) -> bool {
        sow_data::campaign::episode_is_published(self.0.id)
            && sow_data::campaign::access_granted(
                self.0.id,
                &progress.completed_episodes,
                &progress.owned_leaders,
            )
    }

    pub fn advisor(self) -> Leader {
        self.0.advisor()
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
    /// Bot intelligence override; `None` = engine default. Only the JSON loader sets it.
    pub iq: Option<u32>,
    pub leader: Leader,
    /// Shared by the map nameplate and story dialogue; absent uses the generic portrait.
    pub avatar: Option<String>,
    pub support_interval_seconds: Option<u32>,
    pub gold_loot_bonus: Option<u32>,
    pub gold_loot_override: Option<u32>,
    pub alliance_group: Option<String>,
    pub can_attack_player: Option<bool>,
    pub assault_force: bool,
    /// Builds nothing until the player's contact response is decided.
    pub hold_structures_until_contact: bool,
    /// Accepts every alliance request from the player without AI thresholds.
    pub accepts_alliance_requests: bool,
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
            campaign_can_attack_player: f.can_attack_player,
            campaign_assault_force: f.assault_force,
            campaign_hold_structures_until_contact: f.hold_structures_until_contact,
            campaign_accepts_alliance_requests: f.accepts_alliance_requests,
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
    can_attack_player: Option<bool>,
    #[serde(default)]
    assault_force: bool,
    #[serde(default)]
    hold_structures_until_contact: bool,
    #[serde(default)]
    accepts_alliance_requests: bool,
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

/// Minimum tutorial approach size, in CSS pixels per tile, to keep focused nameplates readable.
pub(crate) const TUTORIAL_MIN_TILE_CSS: f32 = 48.0;
const TUTORIAL_OPENING_MAX_TILE_CSS: f32 = 30.0;

pub(crate) fn tutorial_camera_frame(
    campaign: CampaignId,
    config: &sow_core::game_config::GameConfig,
    screen_w: f32,
    screen_h: f32,
) -> Option<((f32, f32), f32)> {
    if !config.tutorial || !campaign.is_boudica() {
        return None;
    }
    let (player_x, player_y) = config.player_spawn?;
    let target = config
        .scripted_spawns
        .iter()
        .find(|spawn| spawn.campaign_faction_id.as_deref() == Some("roman_outpost"))?;
    let (center, zoom) = camera_frame(
        (player_x as f32 + 0.5, player_y as f32 + 0.5),
        (target.x as f32 + 0.5, target.y as f32 + 0.5),
        (screen_w, screen_h),
        (config.map_width, config.map_height),
        0.0,
    );
    let dpr = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    let min_zoom = crate::camera_zoom_lower_bound(
        screen_w,
        screen_h,
        config.map_width,
        config.map_height,
    );
    Some((
        center,
        zoom.min(TUTORIAL_OPENING_MAX_TILE_CSS * dpr).max(min_zoom),
    ))
}

/// Center and zoom that fit two world points with margin. Tiles stay at least `min_tile_css`
/// CSS pixels wide when the screen allows it; pass 0 for a wide shot with no readability floor.
pub(crate) fn camera_frame(
    a: (f32, f32),
    b: (f32, f32),
    (screen_w, screen_h): (f32, f32),
    (map_w, map_h): (u32, u32),
    min_tile_css: f32,
) -> ((f32, f32), f32) {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    let center = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    // Fit both points with margin in CSS pixels, then keep tiles at a readable size on any screen.
    let dpr = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    let fit_css =
        ((screen_w.max(1.0) / dpr) / (dx + 7.0)).min((screen_h.max(1.0) / dpr) / (dy + 7.0));
    let fit_zoom = (fit_css.max(min_tile_css) * dpr)
        .min(crate::camera_zoom_upper_bound(screen_w, screen_h));
    let min_zoom = crate::camera_zoom_lower_bound(screen_w, screen_h, map_w, map_h);
    (center, fit_zoom.max(min_zoom))
}

/// Romans are the Boudica story's enemy: a roster `civ` naming the Roman
/// Empire (`"Roman Empire"`, `"roman-empire"`, …) cannot be neutral.
fn is_roman(civ: &str) -> bool {
    civ.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .eq("romanempire".chars())
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
        if e.civ.trim().is_empty() || (is_roman(&e.civ) && e.relation == CampaignRelation::Neutral)
        {
            return None;
        }
        let leader = sow_data::commerce::leader_from_any_id(&e.leader)?;
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
            iq: e.iq,
            leader,
            avatar: e.avatar.clone(),
            support_interval_seconds: e.support_interval_seconds,
            gold_loot_bonus: e.gold_loot_bonus,
            gold_loot_override: e.gold_loot_override,
            alliance_group: e.alliance_group.clone(),
            can_attack_player: e.can_attack_player,
            assault_force: e.assault_force,
            hold_structures_until_contact: e.hold_structures_until_contact,
            accepts_alliance_requests: e.accepts_alliance_requests,
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

