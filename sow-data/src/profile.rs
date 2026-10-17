//! Durable public-profile records.
//!
//! Account aggregates remain in Valkey/`PLAYERS_TABLE`.  This module stores
//! the append-only pieces that cannot be reconstructed from an aggregate:
//! match history, public identity indexes, seasons, and ladder ratings.

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
    /// Last committed replay turn included when this player voluntarily left.
    /// A later whole-match result must not replace this player's personal result.
    #[serde(default)]
    pub personal_closure_turn: Option<u64>,
    pub created_at: u64,
    /// Applied is written atomically with the balance update. Pending lives
    /// in the relay's durable participant/replay spool until the server accepts it.
    #[serde(default)]
    pub status: RewardSettlementStatus,
    /// Presentation acknowledgement only. It never controls whether the
    /// reward is applied.
    #[serde(default)]
    pub presented_at: Option<u64>,
    /// Replay verification is independent of presentation and settlement.
    #[serde(default)]
    pub verification_status: Option<ReplayVerificationStatus>,
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

pub fn merge_menu_guide_progress(
    saved: Option<&MenuGuideProgress>,
    incoming: &MenuGuideProgress,
) -> MenuGuideProgress {
    let mut merged = incoming.clone();
    if let Some(saved) = saved {
        merged.completed_steps.extend(saved.completed_steps.iter().cloned());
        if !incoming.completed_steps.is_superset(&saved.completed_steps) || saved.status.is_terminal() {
            merged.current_step = saved.current_step.clone();
        }
        if saved.status.is_terminal() {
            merged.status = saved.status;
        }
        if merged.selected_leader.is_none() {
            merged.selected_leader = saved.selected_leader.clone();
        }
        merged.source_match_id = saved.source_match_id.clone();
    } else {
        // Enrollment is server-owned; a client cannot create guide eligibility.
        merged.source_match_id = None;
    }
    merged
}

pub fn validate_menu_guide_progress(
    guide_id: &str,
    progress: &MenuGuideProgress,
) -> Result<(), String> {
    let catalog: serde_json::Value = serde_json::from_str(include_str!(
        "../../assets/guides/menu.triggers.json"
    ))
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
    if progress.current_step.as_deref().is_some_and(|step| !steps.contains(step)) {
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
    /// Last committed replay turn included when this player voluntarily left.
    #[serde(default)]
    pub personal_closure_turn: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchParticipantClosure {
    pub account_id: String,
    pub turn_number: u64,
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
    pub participants: Vec<MatchParticipantRecord>,
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

/// Server-owned return progress. Receiving an exit and verifying its reward
/// are separate operations; neither depends on the lifetime of a game socket.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchExitState {
    Requested,
    AwaitingResult,
    Verifying,
    Confirmed,
    Rejected,
}

#[derive(Serialize, Deserialize)]
pub struct MatchExitCommand {
    pub match_id: String,
    pub account_id: String,
    #[serde(default)]
    pub player_id: Option<u16>,
    #[serde(default)]
    pub owner_verified: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchWork {
    // Keep the persisted tag so pending recoveries survive the rollout.
    #[serde(rename = "exit")]
    RecoveryExit { match_id: String, account_id: String },
    Participant {
        match_id: String,
        account_id: String,
        closure_turn: u64,
        replay_sha256: String,
        rejected: bool,
    },
    Replay { match_id: String },
    Progress { match_id: String, sequence: u64, sha256: String, closing: bool },
}

impl MatchWork {
    pub fn match_id(&self) -> &str {
        match self {
            Self::RecoveryExit { match_id, .. } | Self::Participant { match_id, .. }
                | Self::Replay { match_id } | Self::Progress { match_id, .. } => match_id,
        }
    }

    pub fn key(&self) -> String {
        match self {
            Self::RecoveryExit { match_id, account_id } => format!("exit:{match_id}:{account_id}"),
            Self::Participant { match_id, account_id, .. } => format!("participant:{match_id}:{account_id}"),
            // Numeric keys preserve the existing durable whole-replay queue.
            Self::Replay { match_id } => match_id.clone(),
            Self::Progress { match_id, sequence, .. } => format!("progress:{match_id}:{sequence:010}"),
        }
    }
}

/// Authenticated relay journal delivery. Turn bytes keep sow-data independent
/// of the simulation crate; only the server's trusted replay reader decodes them.
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

impl MatchProgress {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&(&self.match_id, self.sequence, self.first_turn,
            &self.previous_sha256, &self.replay_data, &self.participant_closures, self.finished))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
        Self { work, state: MatchExitState::Requested, retry_at: 0, last_error: None,
            queued_at_ms:std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64, urgent:false }
    }

    pub fn decode(key: &str, bytes: &[u8]) -> Result<Self, serde_json::Error> {
        if bytes == b"1" && key.parse::<u64>().is_ok() {
            Ok(Self::new(MatchWork::Replay { match_id: key.to_string() }))
        } else {
            serde_json::from_slice(bytes)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifiedMatchResult {
    pub match_id: String,
    pub duration_seconds: u32,
    pub winner_player_id: Option<u16>,
    pub winning_team: Option<String>,
    /// Set only by the verifier's personal-exit path.
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
    pub participants: Vec<PublicMatchParticipant>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicProfileView {
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
        validate_menu_guide_progress,
    };
    use std::collections::BTreeSet;

    #[test]
    fn menu_guide_progress_uses_published_ids_and_keeps_terminal_state() {
        let saved = MenuGuideProgress {
            completed_steps: BTreeSet::from(["welcome_name".to_string()]),
            status: MenuGuideStatus::Completed,
            selected_leader: Some("boudica".to_string()),
            source_match_id: None,
            current_step: Some("welcome_heroes".to_string()),
        };
        let stale = MenuGuideProgress {
            completed_steps: BTreeSet::from(["welcome_heroes".to_string()]),
            status: MenuGuideStatus::Active,
            selected_leader: None,
            source_match_id: Some("client-fake-match".to_string()),
            current_step: Some("welcome_name".to_string()),
        };
        let merged = merge_menu_guide_progress(Some(&saved), &stale);
        assert_eq!(merged.status, MenuGuideStatus::Completed);
        assert_eq!(merged.completed_steps.len(), 2);
        assert_eq!(merged.selected_leader.as_deref(), Some("boudica"));
        assert_eq!(merged.source_match_id, None);

        let saved_first_match = MenuGuideProgress {
            source_match_id: Some("first-match".to_string()),
            ..Default::default()
        };
        let stale_first_match = MenuGuideProgress {
            source_match_id: Some("later-client-match".to_string()),
            ..Default::default()
        };
        assert_eq!(
            merge_menu_guide_progress(Some(&saved_first_match), &stale_first_match)
                .source_match_id
                .as_deref(),
            Some("first-match")
        );

        let invalid = MenuGuideProgress {
            completed_steps: BTreeSet::from(["unknown".to_string()]),
            ..Default::default()
        };
        assert!(validate_menu_guide_progress("welcome", &invalid).is_err());
        assert!(validate_menu_guide_progress("unknown", &MenuGuideProgress::default()).is_err());

        let unfinished = MenuGuideProgress {
            completed_steps: BTreeSet::from(["welcome_name".to_string()]),
            status: MenuGuideStatus::Completed,
            ..Default::default()
        };
        assert!(validate_menu_guide_progress("welcome", &unfinished).is_err());
        let finished = MenuGuideProgress {
            completed_steps: BTreeSet::from(["welcome_complete".to_string()]),
            status: MenuGuideStatus::Completed,
            ..Default::default()
        };
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
