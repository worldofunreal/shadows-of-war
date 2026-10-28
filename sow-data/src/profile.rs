//! Durable public-profile records.
//!
//! Account hot fields live in Redb's `PLAYERS_TABLE`; reward, purchase, and
//! provider-event histories live in its keyed account-event table. This module
//! defines the durable profile records and their public views.

use serde::{Deserialize, Serialize};

pub const CURRENT_SEASON_ID: u32 = 1;
pub const CURRENT_SEASON_NAME: &str = "Season 1";

/// Account IDs accepted by profile and settlement APIs: compact hexadecimal
/// IDs and the dashed UUID form used by older account records.
pub fn is_valid_account_id(value: &str) -> bool {
    let value = value.trim();
    (value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        || (value.len() == 36
            && value.bytes().enumerate().all(|(index, byte)| {
                matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                    || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
            }))
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RewardSettlementStatus {
    Pending,
    Applied,
    Presented,
}

impl Default for RewardSettlementStatus {
    fn default() -> Self {
        Self::Applied
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct RewardReceipt {
    pub id: String,
    pub match_id: String,
    pub xp: u32,
    pub leader_xp: u32,
    pub crowns: u64,
    pub laurels: u64,
    #[serde(default)]
    pub unlocked_leaders: Vec<String>,
    /// Player level immediately before and after this receipt was applied.
    /// Older receipts omit these fields and deserialize as `None`.
    #[serde(default)]
    pub level_before: Option<u32>,
    #[serde(default)]
    pub level_after: Option<u32>,
    /// Server-verified turn at which this participant's closure was recorded.
    /// A later whole-match result must preserve the participant's result.
    #[serde(default)]
    pub personal_closure_turn: Option<u64>,
    pub created_at: u64,
    /// Applied is written atomically with the balance update. Pending progress
    /// stays on the relay until the server accepts the saved turns.
    #[serde(default)]
    pub status: RewardSettlementStatus,
    /// Presentation acknowledgement only. It never controls whether the
    /// reward is applied.
    #[serde(default)]
    pub presented_at: Option<u64>,
    /// Replay verification is independent of presentation and settlement.
    #[serde(default)]
    pub verification_status: Option<ReplayVerificationStatus>,
    /// Binds an applied match bonus to the exact server-verified participant result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_result_sha256: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuGuideStatus {
    #[default]
    Active,
    Completed,
    Skipped,
}

impl MenuGuideStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Skipped)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuGuideProgress {
    #[serde(default)]
    pub completed_steps: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub current_step: Option<String>,
    #[serde(default)]
    pub status: MenuGuideStatus,
    #[serde(default)]
    pub selected_leader: Option<String>,
    /// First server-registered match that made this guide available.
    /// Client progress writes cannot set or replace this value.
    #[serde(default)]
    pub source_match_id: Option<String>,
}

pub fn normalize_welcome_guide_progress(
    progress: &MenuGuideProgress,
    matches_played: u32,
) -> MenuGuideProgress {
    let status = if progress.status.is_terminal() {
        progress.status
    } else if progress.completed_steps.contains("welcome_heroes")
        || progress.completed_steps.contains("welcome_complete")
    {
        MenuGuideStatus::Completed
    } else if matches_played > 0 {
        MenuGuideStatus::Skipped
    } else {
        MenuGuideStatus::Active
    };
    MenuGuideProgress {
        completed_steps: if status == MenuGuideStatus::Completed {
            ["welcome_heroes".to_string(), "welcome_complete".to_string()]
                .into_iter()
                .collect()
        } else {
            Default::default()
        },
        current_step: (status == MenuGuideStatus::Active).then(|| "welcome_heroes".to_string()),
        status,
        selected_leader: progress.selected_leader.clone(),
        ..Default::default()
    }
}

pub fn merge_menu_guide_progress(
    saved: Option<&MenuGuideProgress>,
    incoming: &MenuGuideProgress,
) -> MenuGuideProgress {
    if let Some(saved) = saved {
        let saved = normalize_welcome_guide_progress(saved, 0);
        if saved.status.is_terminal() {
            return saved;
        }
    }
    normalize_welcome_guide_progress(incoming, 0)
}

pub fn validate_menu_guide_progress(
    guide_id: &str,
    progress: &MenuGuideProgress,
) -> Result<(), String> {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../assets/guides/menu.triggers.json"))
            .map_err(|error| format!("menu guide definitions are invalid: {error}"))?;
    let guide = catalog
        .get("menu_guides")
        .and_then(|guides| guides.get(guide_id))
        .ok_or_else(|| "unknown menu guide".to_string())?;
    let step_ids = guide
        .get("step_ids")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "menu guide step definitions are invalid".to_string())?;
    let steps = step_ids
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if steps.len() != step_ids.len()
        || progress
            .completed_steps
            .iter()
            .any(|step| !steps.contains(step.as_str()))
    {
        return Err("unknown menu guide step".to_string());
    }
    if progress
        .current_step
        .as_deref()
        .is_some_and(|step| !steps.contains(step))
    {
        return Err("unknown current menu guide step".to_string());
    }
    if progress.status == MenuGuideStatus::Completed
        && step_ids
            .last()
            .and_then(serde_json::Value::as_str)
            .is_none_or(|end_step| !progress.completed_steps.contains(end_step))
    {
        return Err("completed menu guide must include its end step".to_string());
    }
    if guide_id != "welcome" && progress.selected_leader.is_some() {
        return Err("selected_leader is only valid for the welcome guide".to_string());
    }
    if progress
        .selected_leader
        .as_deref()
        .is_some_and(|leader| crate::commerce::leader_from_id(leader).is_none())
    {
        return Err("unknown selected leader".to_string());
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AchievementView {
    pub id: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub points: u64,
    pub progress: u64,
    pub target: u64,
    pub unlocked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct LeaderCareerStats {
    pub matches_played: u32,
    pub wins: u32,
    #[serde(default)]
    pub verified_wins: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub xp: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MatchParticipantRecord {
    /// The single canonical account identifier.
    pub account_id: String,
    pub display_name: String,
    pub is_bot: bool,
    pub leader: Option<String>,
    pub team: Option<String>,
    pub placement: u16,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub xp: u32,
    pub leader_xp: u32,
    #[serde(default)]
    pub crowns: u64,
    #[serde(default)]
    pub laurels: u64,
    pub rating_delta: Option<i16>,
    #[serde(default)]
    pub rating_before: Option<u16>,
    #[serde(default)]
    pub rating_after: Option<u16>,
    #[serde(default)]
    pub tier_before: Option<String>,
    #[serde(default)]
    pub tier_after: Option<String>,
    #[serde(default)]
    pub division_before: Option<String>,
    #[serde(default)]
    pub division_after: Option<String>,
    #[serde(default)]
    pub games_played_after: Option<u32>,
    #[serde(default)]
    pub placements_complete_after: Option<bool>,
    #[serde(default)]
    pub ranked_points_breakdown: Option<RankedPointsBreakdown>,
    /// Last committed replay turn included when this player voluntarily left.
    #[serde(default)]
    pub personal_closure_turn: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MatchRecord {
    pub schema_version: u8,
    pub match_id: String,
    pub season_id: u32,
    pub queue: String,
    pub mode: String,
    pub map_name: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub duration_seconds: u32,
    pub winner_account_id: Option<String>,
    pub winning_team: Option<String>,
    pub verified: bool,
    #[serde(default)]
    pub provisional: bool,
    pub rating_eligible: bool,
    #[serde(default)]
    pub ranked_bot_iq: Option<u32>,
    pub participants: Vec<MatchParticipantRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RankedResultPresentation {
    pub score_before: u16,
    pub score_after: u16,
    pub rating_delta: i16,
    pub tier_before: String,
    pub tier_after: String,
    pub division_before: Option<String>,
    pub division_after: Option<String>,
    pub games_played: u32,
    pub placements_complete: bool,
    #[serde(default)]
    pub ladder: Option<RankedLadder>,
    #[serde(default)]
    pub points_breakdown: Option<RankedPointsBreakdown>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct RankedPointsBreakdown {
    pub base: i16,
    pub opponent: i16,
    pub league: i16,
    pub streak: i16,
    pub protection: i16,
    pub limit: i16,
    #[serde(default)]
    pub protected_loss: bool,
}

impl RankedPointsBreakdown {
    pub fn total(&self) -> i16 {
        self.base + self.opponent + self.league + self.streak + self.protection + self.limit
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RankedStep {
    pub score: u16,
    pub tier: String,
    pub division: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RankedLadder {
    pub steps: Vec<RankedStep>,
    pub max_score: u16,
}

/// Presentation boundaries come from the same owner that assigns every rank.
pub fn ranked_ladder() -> RankedLadder {
    let mut steps = Vec::<RankedStep>::new();
    for score in (800..=4000).step_by(100) {
        let (tier, division) = tier_for_score(score);
        if steps.last().is_none_or(|last| last.tier != tier || last.division != division) {
            steps.push(RankedStep { score, tier: tier.to_string(), division });
        }
    }
    RankedLadder { steps, max_score: 4000 }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayVerificationStatus {
    #[default]
    Pending,
    Verified,
    Rejected,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MatchStartRecord {
    pub match_id: String,
    pub player_ids: Vec<String>,
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub replay_status: ReplayVerificationStatus,
}

/// Current server status for a relay-recorded multiplayer result.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchExitState {
    Requested,
    AwaitingResult,
    Verifying,
    Confirmed,
    Rejected,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchWork {
    Progress {
        match_id: String,
        sequence: u64,
        sha256: String,
        closing: bool,
    },
    Archive {
        match_id: String,
        sequence: u64,
        sha256: String,
    },
    Notify {
        command: MatchResultControl,
    },
}

impl MatchWork {
    pub fn match_id(&self) -> &str {
        match self {
            Self::Progress { match_id, .. } => match_id,
            Self::Archive { match_id, .. } => match_id,
            Self::Notify { command } => &command.match_id,
        }
    }

    pub fn key(&self) -> String {
        match self {
            Self::Progress {
                match_id, sequence, ..
            } => format!("progress:{match_id}:{sequence:010}"),
            Self::Archive { match_id, .. } => format!("archive:{match_id}"),
            Self::Notify { command } => format!(
                "notify:{}:{}",
                command.match_id,
                blake3::hash(&serde_json::to_vec(command).expect("result command serializes"))
                    .to_hex()
            ),
        }
    }
}

/// Durable server turn batch. Opaque bytes keep storage independent of the
/// simulation crate; only the authoritative server decodes the turns.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MatchProgress {
    pub match_id: String,
    pub sequence: u64,
    pub first_turn: u64,
    pub previous_sha256: String,
    pub sha256: String,
    pub replay_data: Vec<u8>,
    pub participant_closures: Vec<ProgressClosure>,
    pub finished: bool,
}

/// Opaque turn batch forwarded by the relay. The data service assigns its
/// hash-chain position and durably records it before acknowledging the relay.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RelayMatchProgress {
    pub match_id: String,
    pub sequence: u64,
    pub first_turn: u64,
    pub replay_data: Vec<u8>,
    pub participant_closures: Vec<ProgressClosure>,
    pub finished: bool,
    #[serde(default)]
    pub recording_failed: bool,
}

impl MatchProgress {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&(
            &self.match_id,
            self.sequence,
            self.first_turn,
            &self.previous_sha256,
            &self.replay_data,
            &self.participant_closures,
            self.finished,
        ))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MatchResultControl {
    pub match_id: String,
    pub terminal_turn: Option<u64>,
    pub participant_closures: Vec<ProgressClosure>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProgressClosure {
    pub account_id: String,
    pub turn_number: u64,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct VerifiedProgress {
    pub match_id: String,
    pub sequence: u64,
    pub sha256: String,
    pub complete: bool,
    pub verified_turns: u64,
    pub terminal_turn: Option<u64>,
    pub participants: Vec<VerifiedParticipantClosure>,
    pub result: Option<VerifiedMatchResult>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerifiedParticipantClosure {
    pub account_id: String,
    pub reason: String,
    pub result: VerifiedMatchResult,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MatchWorkRecord {
    pub work: MatchWork,
    pub state: MatchExitState,
    pub retry_at: u64,
    pub last_error: Option<String>,
    #[serde(default)]
    pub queued_at_ms: u64,
    #[serde(default)]
    pub urgent: bool,
}

impl MatchWorkRecord {
    pub fn new(work: MatchWork) -> Self {
        Self {
            work,
            state: MatchExitState::Requested,
            retry_at: 0,
            last_error: None,
            queued_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            urgent: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifiedMatchResult {
    pub match_id: String,
    pub duration_seconds: u32,
    pub winner_player_id: Option<u16>,
    pub winning_team: Option<String>,
    /// Turn of the server-verified participant closure, when present.
    #[serde(default)]
    pub personal_closure_turn: Option<u64>,
    pub participants: Vec<VerifiedMatchParticipant>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifiedMatchParticipant {
    pub player_id: u16,
    pub account_id: String,
    pub leader: Option<String>,
    pub team: Option<String>,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SeasonRating {
    pub schema_version: u8,
    pub account_id: String,
    pub season_id: u32,
    pub queue: String,
    pub mode: String,
    pub games_played: u32,
    pub wins: u32,
    pub mu: f64,
    pub sigma: f64,
    pub score: u16,
    pub tier: String,
    pub division: Option<String>,
    pub peak_score: u16,
    pub placements_complete: bool,
    #[serde(default)]
    pub win_streak: u32,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SeasonRecord {
    pub id: u32,
    pub name: String,
    pub status: String,
    pub starts_at: u64,
    pub ends_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PublicProfileIndex {
    pub account_id: String,
    pub display_name: String,
    pub kind: String,
    #[serde(default)]
    pub verified_wins: u32,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicProfileSummary {
    pub account_id: String,
    pub handle: String,
    pub display_name: String,
    pub level: u32,
    pub matches_played: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicLeaderSummary {
    pub leader: String,
    pub matches_played: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub xp: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicMatchSummary {
    pub match_id: String,
    pub completed_at: u64,
    pub queue: String,
    pub mode: String,
    pub map_name: String,
    pub duration_seconds: u32,
    pub placement: u16,
    pub won: bool,
    pub leader: Option<String>,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub verified: bool,
    #[serde(default)]
    pub provisional: bool,
    pub rating_delta: Option<i16>,
    #[serde(default)]
    pub ranked_points_breakdown: Option<RankedPointsBreakdown>,
    #[serde(default)]
    pub ranked_bot_iq: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicMatchParticipant {
    pub account_id: String,
    pub handle: String,
    pub is_bot: bool,
    pub leader: Option<String>,
    pub team: Option<String>,
    pub placement: u16,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub verified: bool,
    pub rating_delta: Option<i16>,
    #[serde(default)]
    pub rating_before: Option<u16>,
    #[serde(default)]
    pub rating_after: Option<u16>,
    #[serde(default)]
    pub ranked_points_breakdown: Option<RankedPointsBreakdown>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicMatchDetail {
    pub match_id: String,
    pub completed_at: u64,
    pub queue: String,
    pub mode: String,
    pub map_name: String,
    pub duration_seconds: u32,
    pub winner_account_id: Option<String>,
    pub winning_team: Option<String>,
    pub verified: bool,
    #[serde(default)]
    pub provisional: bool,
    pub rating_eligible: bool,
    #[serde(default)]
    pub ranked_bot_iq: Option<u32>,
    pub participants: Vec<PublicMatchParticipant>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicProfileView {
    #[serde(default)]
    pub ranked_ladder: Option<RankedLadder>,
    pub account_id: String,
    pub handle: String,
    pub display_name: String,
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default)]
    pub ranked_bot_iq: Option<u32>,
    pub level: u32,
    pub matches_played: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub preferred_leader: Option<String>,
    #[serde(default)]
    pub laurels: u64,
    #[serde(default)]
    pub achievements: Vec<AchievementView>,
    pub leaders: Vec<PublicLeaderSummary>,
    pub recent_matches: Vec<PublicMatchSummary>,
    #[serde(default)]
    pub ratings: Vec<PublicRatingView>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicRatingView {
    pub season_id: u32,
    pub season_name: String,
    pub queue: String,
    pub mode: String,
    pub games_played: u32,
    pub wins: u32,
    pub placements_complete: bool,
    pub score: u16,
    pub tier: String,
    pub division: Option<String>,
    pub peak_score: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicLeaderboardEntry {
    pub rank: u32,
    pub account_id: String,
    pub handle: String,
    #[serde(default)]
    pub is_bot: bool,
    pub queue: String,
    pub mode: String,
    pub score: u16,
    pub tier: String,
    pub division: Option<String>,
    pub games_played: u32,
    pub wins: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicVictoryLeaderboardEntry {
    pub rank: u32,
    pub account_id: String,
    pub handle: String,
    pub level: u32,
    pub matches_played: u32,
    pub wins: u32,
}

pub fn public_handle(display_name: &str, account_id: &str) -> String {
    let suffix = account_id
        .chars()
        .take(4)
        .collect::<String>()
        .to_uppercase();
    format!("{}#{suffix}", display_name.trim())
}

pub fn win_rate(wins: u32, matches: u32) -> f32 {
    if matches == 0 {
        0.0
    } else {
        (wins as f32 / matches as f32 * 100.0).round() / 100.0
    }
}

pub fn kda(kills: u32, deaths: u32, assists: u32) -> f32 {
    ((kills + assists) as f32 / deaths.max(1) as f32 * 100.0).round() / 100.0
}

pub fn ladder_score(mu: f64, sigma: f64) -> u16 {
    let ordinal = mu - 3.0 * sigma;
    (1500.0 + ordinal * 50.0).round().clamp(800.0, 4000.0) as u16
}

/// Update one ranked 1v1 rating using the original Glicko formula.
/// The caller supplies both ratings as they stood before the match.
pub fn ranked_glicko(rating: f64, deviation: f64, opponent: f64, opponent_deviation: f64, score: f64) -> (f64, f64) {
    let q = 10_f64.ln() / 400.0;
    let deviation = deviation.clamp(50.0, 350.0);
    let opponent_deviation = opponent_deviation.clamp(50.0, 350.0);
    let expected = ranked_expected_score(rating, opponent, opponent_deviation);
    let g = ranked_glicko_g(opponent_deviation);
    let precision = 1.0 / (deviation * deviation) + q * q * g * g * expected * (1.0 - expected);
    let next = rating + q * g * (score - expected) / precision;
    (next, (1.0 / precision).sqrt().clamp(50.0, 350.0))
}

fn ranked_glicko_g(opponent_deviation: f64) -> f64 {
    let q = 10_f64.ln() / 400.0;
    let deviation = opponent_deviation.clamp(50.0, 350.0);
    1.0 / (1.0 + 3.0 * q * q * deviation * deviation / std::f64::consts::PI.powi(2)).sqrt()
}

pub fn ranked_expected_score(rating: f64, opponent: f64, opponent_deviation: f64) -> f64 {
    let g = ranked_glicko_g(opponent_deviation);
    1.0 / (1.0 + 10_f64.powf(-g * (rating - opponent) / 400.0))
}

const RANKED_POINT_CURVE: [(u16, i16, i16); 8] = [
    (800, 28, 0),
    (1200, 26, 0),
    (1600, 24, 0),
    (2000, 22, 10),
    (2400, 20, 16),
    (2800, 18, 22),
    (3200, 18, 26),
    (4000, 16, 30),
];

fn ranked_point_base(score: u16, won: bool) -> i16 {
    for pair in RANKED_POINT_CURVE.windows(2) {
        let (start, win, loss) = pair[0];
        let (end, next_win, next_loss) = pair[1];
        if score <= end {
            if !won && score < 2000 {
                return 0;
            }
            let progress = f64::from(score.saturating_sub(start)) / f64::from(end - start);
            let start_value = if won { win } else { loss };
            let end_value = if won { next_win } else { next_loss };
            return (f64::from(start_value) + f64::from(end_value - start_value) * progress).round() as i16;
        }
    }
    if won { 16 } else { 30 }
}

pub fn ranked_points_breakdown(
    rating: &SeasonRating,
    opponent: &SeasonRating,
    result: f64,
) -> RankedPointsBreakdown {
    if result == 0.5 {
        return RankedPointsBreakdown::default();
    }
    if result == 0.0 && (!rating.placements_complete || rating.score < 2000) {
        return RankedPointsBreakdown { protected_loss: true, ..Default::default() };
    }

    let won = result == 1.0;
    let expected = ranked_expected_score(rating.mu, opponent.mu, opponent.sigma);
    let opponent_adjustment = (16.0 * (0.5 - expected)).round() as i16;
    let league_adjustment = if rating.score >= 2000 {
        ((rating.mu - f64::from(rating.score)) / 100.0).round().clamp(-6.0, 6.0) as i16
    } else {
        0
    };
    let base = if won { ranked_point_base(rating.score, true) } else { -ranked_point_base(rating.score, false) };
    let streak = if won { rating.win_streak.saturating_add(1).min(3) as i16 } else { 0 };
    let raw = base + opponent_adjustment + league_adjustment + streak;
    let mut breakdown = RankedPointsBreakdown {
        base,
        opponent: opponent_adjustment,
        league: league_adjustment,
        streak,
        ..Default::default()
    };
    if won {
        let bounded = raw.clamp(10, 40);
        breakdown.limit = bounded - raw;
        let actual = (i32::from(rating.score) + i32::from(bounded)).min(4000) - i32::from(rating.score);
        breakdown.limit += actual as i16 - bounded;
    } else {
        let bounded = raw.clamp(-40, -2);
        breakdown.limit = bounded - raw;
        let actual = (i32::from(rating.score) + i32::from(bounded)).max(2000) - i32::from(rating.score);
        breakdown.protection = actual as i16 - bounded;
    }
    breakdown
}

/// Create the initial rating for a participant in the ranked 1v1 queue.
pub fn initial_ranked_rating(account_id: String, initial_mmr: f64, now: u64) -> SeasonRating {
    let score = initial_mmr.round().clamp(800.0, 4000.0) as u16;
    let (tier, division) = tier_for_score(score);
    SeasonRating {
        schema_version: 2,
        account_id,
        season_id: CURRENT_SEASON_ID,
        queue: "Ranked1v1".to_string(),
        mode: "FFA".to_string(),
        games_played: 0,
        wins: 0,
        mu: initial_mmr,
        sigma: 350.0,
        score,
        tier: tier.to_string(),
        division,
        peak_score: score,
        placements_complete: false,
        win_streak: 0,
        updated_at: now,
    }
}

/// Apply a verified result using the opponent's rating before this match.
pub fn apply_ranked_result(rating: &mut SeasonRating, opponent: &SeasonRating, result: f64, now: u64) -> Result<(), &'static str> {
    if rating.queue != "Ranked1v1" || opponent.queue != "Ranked1v1"
        || rating.season_id != opponent.season_id || rating.account_id == opponent.account_id
        || ![0.0, 0.5, 1.0].contains(&result)
        || !rating.mu.is_finite() || !rating.sigma.is_finite()
        || !opponent.mu.is_finite() || !opponent.sigma.is_finite()
    {
        return Err("invalid ranked result");
    }
    let points = ranked_points_breakdown(rating, opponent, result);
    let (mu, sigma) = ranked_glicko(rating.mu, rating.sigma, opponent.mu, opponent.sigma, result);
    rating.mu = mu;
    rating.sigma = sigma;
    rating.score = (i32::from(rating.score) + i32::from(points.total())).clamp(800, 4000) as u16;
    let (tier, division) = tier_for_score(rating.score);
    rating.tier = tier.to_string();
    rating.division = division;
    rating.games_played = rating.games_played.saturating_add(1);
    rating.wins = rating.wins.saturating_add(u32::from(result == 1.0));
    match result {
        1.0 => rating.win_streak = rating.win_streak.saturating_add(1).min(3),
        0.0 => rating.win_streak = 0,
        _ => {}
    }
    rating.placements_complete = rating.games_played >= 5;
    rating.peak_score = rating.peak_score.max(rating.score);
    rating.updated_at = now;
    Ok(())
}

pub fn tier_for_score(score: u16) -> (&'static str, Option<String>) {
    let tier = match score {
        0..=1199 => "Bronze",
        1200..=1599 => "Silver",
        1600..=1999 => "Gold",
        2000..=2399 => "Platinum",
        2400..=2799 => "Diamond",
        2800..=3199 => "Master",
        _ => "Grandmaster",
    };
    let division = if (800..2800).contains(&score) {
        let within = ((score.saturating_sub(800)) % 400) / 100;
        Some(
            match within {
                0 => "IV",
                1 => "III",
                2 => "II",
                _ => "I",
            }
            .to_string(),
        )
    } else {
        None
    };
    (tier, division)
}

#[cfg(test)]
mod tests {
    use super::{
        MenuGuideProgress, MenuGuideStatus, RewardReceipt, merge_menu_guide_progress,
        normalize_welcome_guide_progress, validate_menu_guide_progress,
    };
    use std::collections::BTreeSet;

    #[test]
    fn presentation_ladder_matches_every_score_and_boundary() {
        let ladder = super::ranked_ladder();
        assert_eq!(ladder.steps.len(), 22);
        for score in 800..=ladder.max_score {
            let shown = ladder.steps.iter().rev().find(|step| step.score <= score).unwrap();
            let (tier, division) = super::tier_for_score(score);
            assert_eq!((&*shown.tier, &shown.division), (tier, &division));
        }
        assert_eq!(ladder.steps.last().unwrap().score, 3200);
    }

    #[test]
    fn ranked_results_update_both_players_from_their_previous_ratings() {
        let mut winner = super::initial_ranked_rating("winner".into(), 1000.0, 1);
        let mut loser = super::initial_ranked_rating("loser".into(), 1000.0, 1);
        let winner_before = winner.clone();
        let loser_before = loser.clone();
        super::apply_ranked_result(&mut winner, &loser_before, 1.0, 2).unwrap();
        super::apply_ranked_result(&mut loser, &winner_before, 0.0, 2).unwrap();
        assert!(winner.mu > 1000.0 && loser.mu < 1000.0);
        assert_eq!(winner.wins, 1);
        assert_eq!(loser.wins, 0);
        assert!(!winner.placements_complete);
        for _ in 0..4 {
            super::apply_ranked_result(&mut winner, &loser_before, 0.5, 3).unwrap();
        }
        assert!(winner.placements_complete);
        let before = winner.clone();
        assert!(super::apply_ranked_result(&mut winner, &loser_before, 0.25, 4).is_err());
        assert_eq!(winner, before);
    }

    fn ranked_rating(score: u16, mu: f64, games: u32, streak: u32) -> super::SeasonRating {
        let mut rating = super::initial_ranked_rating(
            format!("rating-{score}-{mu}-{games}-{streak}"),
            mu,
            1,
        );
        rating.score = score;
        let (tier, division) = super::tier_for_score(score);
        rating.tier = tier.into();
        rating.division = division;
        rating.games_played = games;
        rating.placements_complete = games >= 5;
        rating.win_streak = streak;
        rating.sigma = 80.0;
        rating
    }

    #[test]
    fn ranked_point_gains_change_with_rival_and_streak() {
        let player = ranked_rating(1400, 1400.0, 5, 1);
        let stronger = ranked_rating(1550, 1550.0, 5, 0);
        let weaker = ranked_rating(1250, 1250.0, 5, 0);
        let favored = super::ranked_points_breakdown(&player, &stronger, 1.0);
        let underdog = super::ranked_points_breakdown(&player, &weaker, 1.0);
        assert_eq!((favored.base, favored.opponent, favored.streak, favored.total()), (25, 3, 2, 30));
        assert_eq!((underdog.base, underdog.opponent, underdog.streak, underdog.total()), (25, -3, 2, 24));
        let veteran_streak = ranked_rating(1400, 1400.0, 5, 8);
        assert_eq!(super::ranked_points_breakdown(&veteran_streak, &stronger, 1.0).streak, 3);
    }

    #[test]
    fn ranked_losses_are_protected_during_placements_and_through_gold() {
        let player = ranked_rating(1000, 1000.0, 0, 3);
        let opponent = ranked_rating(1000, 1000.0, 5, 0);
        let placement_loss = super::ranked_points_breakdown(&player, &opponent, 0.0);
        assert!(placement_loss.protected_loss);
        assert_eq!(placement_loss.total(), 0);

        let gold = ranked_rating(1999, 1999.0, 5, 0);
        let gold_loss = super::ranked_points_breakdown(&gold, &opponent, 0.0);
        assert!(gold_loss.protected_loss);
        assert_eq!(gold_loss.total(), 0);
    }

    #[test]
    fn ranked_platinum_losses_scale_by_rival_and_stop_at_the_protected_floor() {
        let player = ranked_rating(2200, 2200.0, 5, 0);
        let stronger = ranked_rating(2350, 2350.0, 5, 0);
        let weaker = ranked_rating(2050, 2050.0, 5, 0);
        let against_stronger = super::ranked_points_breakdown(&player, &stronger, 0.0);
        let against_weaker = super::ranked_points_breakdown(&player, &weaker, 0.0);
        assert_eq!(against_stronger.total(), -10);
        assert_eq!(against_weaker.total(), -16);

        let near_floor = ranked_rating(2003, 2003.0, 5, 0);
        let floor_result = super::ranked_points_breakdown(&near_floor, &near_floor, 0.0);
        assert_eq!(floor_result.total(), -3);
        assert_eq!(i32::from(near_floor.score) + i32::from(floor_result.total()), 2000);
        assert_eq!(floor_result.total(), floor_result.base + floor_result.opponent + floor_result.league + floor_result.streak + floor_result.protection + floor_result.limit);
    }

    #[test]
    fn ranked_mmr_moves_separately_while_points_stay_protected_and_streaks_settle_once() {
        let mut player = ranked_rating(1000, 1000.0, 0, 0);
        let opponent = ranked_rating(1000, 1000.0, 5, 0);
        super::apply_ranked_result(&mut player, &opponent, 0.0, 2).unwrap();
        assert_eq!(player.score, 1000);
        assert!(player.mu < 1000.0);
        assert_eq!(player.win_streak, 0);

        player.win_streak = 2;
        let streak_before_draw = player.win_streak;
        super::apply_ranked_result(&mut player, &opponent, 0.5, 3).unwrap();
        assert_eq!(player.win_streak, streak_before_draw);
        super::apply_ranked_result(&mut player, &opponent, 1.0, 4).unwrap();
        assert_eq!(player.win_streak, 3);
    }

    #[test]
    fn legacy_ranked_records_load_without_streak_or_point_breakdown() {
        let rating: super::SeasonRating = serde_json::from_str(
            r#"{"schema_version":2,"account_id":"legacy","season_id":1,"queue":"Ranked1v1","mode":"FFA","games_played":4,"wins":2,"mu":1000.0,"sigma":100.0,"score":1000,"tier":"Bronze","division":"II","peak_score":1000,"placements_complete":false,"updated_at":1}"#,
        ).unwrap();
        assert_eq!(rating.win_streak, 0);

        let participant: super::MatchParticipantRecord = serde_json::from_str(
            r#"{"account_id":"legacy","display_name":"Legacy","is_bot":false,"leader":null,"team":null,"placement":2,"won":false,"kills":0,"deaths":0,"assists":0,"players_defeated":0,"empires_defeated":0,"tribes_defeated":0,"xp":0,"leader_xp":0,"crowns":0,"laurels":0,"rating_delta":-4,"rating_before":1000,"rating_after":996}"#,
        ).unwrap();
        assert_eq!(participant.ranked_points_breakdown, None);
    }

    #[test]
    fn welcome_progress_retires_legacy_steps_and_preserves_terminal_states() {
        for (status, steps, matches, expected) in [
            (
                MenuGuideStatus::Active,
                vec!["welcome_name"],
                0,
                MenuGuideStatus::Active,
            ),
            (
                MenuGuideStatus::Active,
                vec!["welcome_name", "welcome_heroes"],
                0,
                MenuGuideStatus::Completed,
            ),
            (MenuGuideStatus::Active, vec![], 1, MenuGuideStatus::Skipped),
            (
                MenuGuideStatus::Completed,
                vec!["welcome_complete"],
                1,
                MenuGuideStatus::Completed,
            ),
            (
                MenuGuideStatus::Skipped,
                vec!["welcome_name"],
                0,
                MenuGuideStatus::Skipped,
            ),
        ] {
            let legacy = MenuGuideProgress {
                completed_steps: steps.into_iter().map(str::to_string).collect(),
                current_step: Some("welcome_purchase_confirm".to_string()),
                selected_leader: Some("boudica".to_string()),
                status,
                ..Default::default()
            };
            let normalized = normalize_welcome_guide_progress(&legacy, matches);
            assert_eq!(normalized.status, expected);
            assert_eq!(
                normalized.current_step.as_deref(),
                (expected == MenuGuideStatus::Active).then_some("welcome_heroes")
            );
            assert_eq!(normalized.selected_leader.as_deref(), Some("boudica"));
            assert!(validate_menu_guide_progress("welcome", &normalized).is_ok());
            if expected.is_terminal() {
                assert_eq!(
                    merge_menu_guide_progress(Some(&normalized), &MenuGuideProgress::default()),
                    normalized
                );
            }
        }
    }

    #[test]
    fn welcome_writes_use_only_published_ids_and_a_complete_ending() {
        for id in ["unknown", "welcome_name", "first_profile"] {
            let invalid = MenuGuideProgress {
                completed_steps: BTreeSet::from([id.to_string()]),
                ..Default::default()
            };
            assert!(validate_menu_guide_progress("welcome", &invalid).is_err());
        }
        assert!(
            validate_menu_guide_progress("first_multiplayer", &MenuGuideProgress::default())
                .is_err()
        );
        let unfinished = MenuGuideProgress {
            status: MenuGuideStatus::Completed,
            ..Default::default()
        };
        assert!(validate_menu_guide_progress("welcome", &unfinished).is_err());
        let finished = normalize_welcome_guide_progress(&unfinished, 0);
        assert!(validate_menu_guide_progress("welcome", &finished).is_ok());
    }

    #[test]
    fn old_reward_receipts_load_without_level_or_closure_fields() {
        let receipt: RewardReceipt = serde_json::from_str(
            r#"{"id":"m1","match_id":"m1","xp":20,"leader_xp":20,"crowns":25,"laurels":0,"created_at":1}"#,
        )
        .unwrap();
        assert_eq!(receipt.level_before, None);
        assert_eq!(receipt.level_after, None);
        assert_eq!(receipt.personal_closure_turn, None);
    }
}
