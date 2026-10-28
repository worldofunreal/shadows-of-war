use crate::profile::{
    LeaderCareerStats, MatchRecord, MatchStartRecord, MenuGuideProgress, PublicLeaderSummary,
    PublicLeaderboardEntry, PublicMatchDetail, PublicMatchParticipant, PublicMatchSummary,
    PublicProfileIndex, PublicProfileSummary, PublicProfileView, PublicRatingView,
    ReplayVerificationStatus, SeasonRating, SeasonRecord, VerifiedMatchResult, is_valid_account_id,
    public_handle, win_rate,
};
use crate::redb_store::{
    MATCH_STARTS_TABLE, MATCHES_TABLE, PENDING_REPLAY_VERIFICATIONS_TABLE,
    PLAYER_MATCH_INDEX_TABLE, PUBLIC_PROFILES_TABLE, RANKED_BOTS_TABLE, SEASON_RATINGS_TABLE, SEASONS_TABLE,
    VERIFIED_VICTORY_BOARD_TABLE,
};
use log::{error, info};
use redb::{ReadableTable, WriteTransaction};
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use sha2::Digest;

fn replay_directory() -> std::path::PathBuf {
    std::env::var_os("SOW_REPLAY_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("replays"))
}

fn match_id_from_artifact_name(name: &std::ffi::OsStr) -> Option<u64> {
    let name = name.to_str()?;
    let digits = name.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || (digits < name.len() && !matches!(name.as_bytes()[digits], b'.' | b'-')) {
        return None;
    }
    name[..digits].parse().ok()
}

fn max_replay_artifact_match_id(
    directory: &std::path::Path,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut maximum = 0;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            maximum = maximum.max(match_id_from_artifact_name(&entry.file_name()).unwrap_or(0));
        }
    }
    Ok(maximum)
}

fn read_match_id_high_water(
    directory: &std::path::Path,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let path = directory.join(".match-id-high-water");
    match std::fs::read_to_string(path) {
        Ok(value) => Ok(value.trim().parse()?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.into()),
    }
}

fn record_match_id_high_water(
    directory: &std::path::Path,
    match_id: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    // ponytail: one data-service process allocates match IDs; use a DB allocator if that changes.
    static HIGH_WATER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = HIGH_WATER_LOCK.lock().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::Other, "match ID marker lock poisoned")
    })?;
    let path = directory.join(".match-id-high-water");
    if read_match_id_high_water(directory)? >= match_id {
        return Ok(());
    }
    std::fs::create_dir_all(directory)?;
    let temp = directory.join(format!(
        ".match-id-high-water.{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    writeln!(file, "{match_id}")?;
    file.sync_all()?;
    drop(file);
    if let Err(error) = std::fs::rename(&temp, &path) {
        let _ = std::fs::remove_file(&temp);
        return Err(error.into());
    }
    std::fs::File::open(directory)?.sync_all()?;
    Ok(())
}

fn recovered_match_id_seed(
    database_maximum: u64,
    replay_directory: &std::path::Path,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let maximum = database_maximum
        .max(max_replay_artifact_match_id(replay_directory)?)
        .max(read_match_id_high_water(replay_directory)?);
    if maximum > 0 {
        record_match_id_high_water(replay_directory, maximum)?;
    }
    maximum
        .checked_add(1)
        .ok_or_else(|| "match ID space exhausted".into())
}

#[path = "account_store.rs"]
mod account_store;

fn verified_victory_key(wins: u32, account_id: &str) -> String {
    format!("{:010}:{account_id}", u32::MAX.saturating_sub(wins))
}

fn load_match_record_in_transaction(
    txn: &WriteTransaction,
    match_id: &str,
) -> Result<Option<MatchRecord>, Box<dyn std::error::Error + Send + Sync>> {
    let matches = txn.open_table(MATCHES_TABLE)?;
    matches
        .get(match_id)?
        .map(|value| serde_json::from_slice(value.value()).map_err(Into::into))
        .transpose()
}

fn write_match_record_in_transaction(
    txn: &WriteTransaction,
    record: &MatchRecord,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let previous = load_match_record_in_transaction(txn, &record.match_id)?;
    {
        let mut index = txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
        if let Some(previous) = &previous {
            for participant in &previous.participants {
                let key = format!(
                    "player:{}:{:020}:{}",
                    participant.account_id, previous.completed_at, previous.match_id
                );
                index.remove(key.as_str())?;
            }
        }
        for participant in &record.participants {
            let key = format!(
                "player:{}:{:020}:{}",
                participant.account_id, record.completed_at, record.match_id
            );
            index.insert(key.as_str(), record.match_id.as_bytes())?;
        }
    }
    let bytes = serde_json::to_vec(record)?;
    txn.open_table(MATCHES_TABLE)?
        .insert(record.match_id.as_str(), bytes.as_slice())?;
    Ok(())
}

fn queue_match_work_in_transaction(
    txn: &WriteTransaction,
    work: crate::profile::MatchWork,
) -> Result<crate::profile::MatchWorkRecord, Box<dyn std::error::Error + Send + Sync>> {
    use crate::profile::MatchWorkRecord;

    if work.match_id().parse::<u64>().is_err() {
        return Err("invalid match id".into());
    }
    let key = work.key();
    let mut table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
    if let Some(value) = table.get(key.as_str())? {
        let existing = serde_json::from_slice::<crate::profile::MatchWorkRecord>(value.value())?;
        if existing.work != work {
            return Err("pending result conflicts with its saved state".into());
        }
        return Ok(existing);
    }
    let record = MatchWorkRecord::new(work);
    let bytes = serde_json::to_vec(&record)?;
    table.insert(key.as_str(), bytes.as_slice())?;
    Ok(record)
}

/// SET index of all bot account_ids — populated by `seed_bot_pool`, used for
/// pool introspection and analytics. Account records carry a canonical
/// display_name field; the bot allocator may choose its presentation name.
pub const DISPLAY_NAME_MAX_CHARS: usize = crate::name_policy::MAX_CHARS;
const ANALYTICS_TTL_SECONDS: i64 = 90 * 24 * 60 * 60;

const RECORD_ANALYTICS_EVENT: &str = r#"
if not redis.call('SET', KEYS[1], '1', 'NX', 'EX', ARGV[1]) then return 0 end
redis.call('INCR', KEYS[2]); redis.call('EXPIRE', KEYS[2], ARGV[1])
redis.call('SADD', KEYS[3], ARGV[3]); redis.call('EXPIRE', KEYS[3], ARGV[1])
redis.call('SADD', KEYS[14], ARGV[3]); redis.call('EXPIRE', KEYS[14], ARGV[1])
local build = ARGV[5]
if build == '' then build = 'unknown' end
if not redis.call('SISMEMBER', KEYS[11], build) then
  if redis.call('SCARD', KEYS[11]) < 16 then
    redis.call('SADD', KEYS[11], build)
  else
    build = 'other'
  end
end
local event_name = ARGV[7]
redis.call('HINCRBY', KEYS[12], build .. '|' .. event_name, 1)
redis.call('EXPIRE', KEYS[12], ARGV[1])
local device = ARGV[6]
if device == '' then device = 'unknown' end
redis.call('HINCRBY', KEYS[13], device .. '|' .. event_name, 1)
redis.call('EXPIRE', KEYS[13], ARGV[1])
local subject = ARGV[4]
if subject ~= '' then
  redis.call('SADD', KEYS[4], subject); redis.call('EXPIRE', KEYS[4], ARGV[1])
  redis.call('SADD', KEYS[5], subject); redis.call('EXPIRE', KEYS[5], ARGV[1])
  local first_day = redis.call('GET', KEYS[6])
  if not first_day then
    first_day = ARGV[2]
    redis.call('SET', KEYS[6], first_day, 'EX', ARGV[1])
    local cohort_key = KEYS[7] .. first_day
    redis.call('SADD', cohort_key, subject); redis.call('EXPIRE', cohort_key, ARGV[1])
  end
  local delta = tonumber(ARGV[2]) - tonumber(first_day)
  if delta == 1 then
    local retention_key = KEYS[8] .. first_day
    redis.call('SADD', retention_key, subject); redis.call('EXPIRE', retention_key, ARGV[1])
  end
  if delta == 3 then
    local retention_key = KEYS[9] .. first_day
    redis.call('SADD', retention_key, subject); redis.call('EXPIRE', retention_key, ARGV[1])
  end
  if delta == 7 then
    local retention_key = KEYS[10] .. first_day
    redis.call('SADD', retention_key, subject); redis.call('EXPIRE', retention_key, ARGV[1])
  end
end
if (event_name == 'tutorial_step' or event_name == 'tutorial_exit_early') and ARGV[8] ~= '' then
  redis.call('HINCRBY', KEYS[15], ARGV[8], 1)
  redis.call('EXPIRE', KEYS[15], ARGV[1])
end
return 1
"#;

#[derive(Clone, Copy, Debug)]
struct RetentionCohort {
    offset: i64,
    eligible: u64,
    returned: [u64; 3],
}

fn summarize_retention(today: &str, cohorts: &[RetentionCohort]) -> serde_json::Value {
    [1_i64, 3, 7]
        .into_iter()
        .enumerate()
        .map(|(slot, window)| {
            let matured = cohorts.iter().filter(|cohort| cohort.offset > window);
            let mut matured_cohorts = 0_u64;
            let mut eligible = 0_u64;
            let mut returned = 0_u64;
            for cohort in matured {
                matured_cohorts = matured_cohorts.saturating_add(1);
                eligible = eligible.saturating_add(cohort.eligible);
                returned = returned.saturating_add(cohort.returned[slot]);
            }
            let status = if matured_cohorts == 0 {
                "not_matured"
            } else if eligible == 0 {
                "no_eligible_accounts"
            } else {
                "ready"
            };
            let cutoff = crate::events::shift_date(today, -(window + 1));
            (
                format!("d{window}"),
                serde_json::json!({
                    "status": status,
                    "cohort_cutoff_date": cutoff,
                    "matured_cohorts": matured_cohorts,
                    "eligible_accounts": eligible,
                    "returned_accounts": returned,
                    "rate": if eligible == 0 { serde_json::Value::Null } else { serde_json::json!(returned as f64 / eligible as f64) },
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

fn aggregate_tutorial_steps(
    rows: &[std::collections::HashMap<String, u64>],
) -> Vec<serde_json::Value> {
    let mut totals = std::collections::BTreeMap::<(String, u64, String), [u64; 3]>::new();
    for row in rows {
        for (field, count) in row {
            let mut parts = field.split('|');
            let (Some(episode), Some(step), Some(index), Some(action), None) = (
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
            ) else {
                continue;
            };
            let Ok(index) = index.parse::<u64>() else {
                continue;
            };
            let Some(slot) = (match action {
                "start" => Some(0),
                "complete" => Some(1),
                "fail" => Some(2),
                _ => None,
            }) else {
                continue;
            };
            let value = totals
                .entry((episode.to_string(), index, step.to_string()))
                .or_default();
            value[slot] = value[slot].saturating_add(*count);
        }
    }
    totals
        .into_iter()
        .map(|((episode_id, step_index, step_id), counts)| {
            serde_json::json!({
                "episode_id": episode_id,
                "step_id": step_id,
                "step_index": step_index,
                "started": counts[0],
                "completed": counts[1],
                "exited": counts[2],
                "completion_rate": if counts[0] == 0 { serde_json::Value::Null } else { serde_json::json!(counts[1] as f64 / counts[0] as f64) },
            })
        })
        .collect()
}

/// Generate the initial presentation name only when the client did not send one.
/// The account ID remains the sole stable identity key.
fn generated_display_name() -> String {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        % 1000;
    format!("ANON{suffix:03}")
}

/// Normalize a player-facing name without making it an identity key.
/// The account ID remains the stable identity; this value is presentation data.
pub fn normalize_display_name(value: &str) -> Result<String, &'static str> {
    let normalized: String = value
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(DISPLAY_NAME_MAX_CHARS)
        .collect();
    if normalized.is_empty() {
        return Err("display_name cannot be empty");
    }
    if !crate::name_policy::is_allowed(&normalized) {
        return Err("display_name contains blocked language");
    }
    Ok(normalized)
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct SessionDefeats {
    pub players: u32,
    pub empires: u32,
    pub tribes: u32,
}

/// Deserialize one stored account, migrating legacy `id` and currency fields
/// at this single chokepoint. Every stored-account parse goes through here.
pub fn parse_account_with_migration(raw: &[u8]) -> serde_json::Result<PlayerAccount> {
    let mut value: serde_json::Value = serde_json::from_slice(raw)?;
    if let Some(account) = value.as_object_mut()
        && !account.contains_key("account_id")
        && let Some(id) = account.remove("id")
    {
        account.insert("account_id".to_string(), id);
    }
    if let Some(profile) = value.get_mut("profile").and_then(|p| p.as_object_mut()) {
        migrate_legacy_currency(profile);
    }
    serde_json::from_value(value)
}

/// Lazy migration for the 2026-09 currency split (owner decision): profiles
/// saved before the split store the spendable balance under the legacy
/// `"laurels"` key. Move it to `"crowns"` and drop the legacy key so the new
/// `laurels` (achievement points) field deserializes to its default 0 instead
/// of inheriting the old currency amount. Deterministic, order-independent,
/// a no-op for already-migrated objects; stored bytes are only rewritten on
/// the next regular save.
fn migrate_legacy_currency(profile: &mut serde_json::Map<String, serde_json::Value>) {
    if profile.contains_key("crowns") {
        return;
    }
    if let Some(legacy) = profile.remove("laurels") {
        profile.insert("crowns".to_string(), legacy);
    }
}

#[derive(Clone, Debug)]
pub struct PlayGamesMatchOutcome {
    pub account_id: String,
    pub won: bool,
    pub wins: u32,
    pub sync_revision: u64,
    pub unlocked_achievements: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PlayGamesSyncSnapshot {
    pub unlocked_achievements: Vec<String>,
    pub wins: u32,
    pub revision: u64,
    pub synced_revision: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerProfile {
    pub xp: u32,
    pub level: u32,
    pub wins: u32,
    /// Victories confirmed by deterministic server replay verification.
    #[serde(default)]
    pub verified_wins: u32,
    pub matches_played: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub preferred_leader: Option<String>,
    #[serde(default)]
    pub prefer_random_leader: bool,
    #[serde(default)]
    pub completed_episodes: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
    #[serde(default)]
    pub leader_xp: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub leader_stats: std::collections::BTreeMap<String, LeaderCareerStats>,
    /// Free spendable currency (legacy profiles stored it under "laurels";
    /// `parse_account_with_migration` moves it here on load).
    #[serde(default)]
    pub crowns: u64,
    /// Achievement points — earned, never spent.
    #[serde(default)]
    pub laurels: u64,
    #[serde(default)]
    pub gems: u64,
    #[serde(default)]
    pub owned_leaders: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub owned_skins: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub selected_skin: Option<String>,
    #[serde(default)]
    pub processed_revenuecat_events: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub purchased_leaders: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub purchased_skins: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub purchase_history: std::collections::BTreeMap<String, PurchaseRecord>,
    /// Durable per-match settlement receipts. The receipt key is the match
    /// id, so a retry or a concurrent finalize can only apply it once.
    #[serde(default)]
    pub reward_receipts: std::collections::BTreeMap<String, crate::profile::RewardReceipt>,
    /// Server-persisted progress for menu journeys.
    #[serde(default)]
    pub menu_guides: std::collections::BTreeMap<String, MenuGuideProgress>,
    /// Server-unlocked achievements. The client can display these values but
    /// cannot create or advance them.
    #[serde(default)]
    pub unlocked_achievements: std::collections::BTreeSet<String>,
    /// Durable external-mirror cursor. The server profile remains canonical;
    /// Google Play retries any revision that has not been confirmed.
    #[serde(default)]
    pub playgames_sync_revision: u64,
    #[serde(default)]
    pub playgames_synced_revision: u64,
    #[serde(default)]
    pub crazygames_synced_revision: u64,
    #[serde(default)]
    pub intro_completed: bool,
    /// Present only for managed ranked opponents, never for public fillers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranked_bot: Option<RankedBotProfile>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RankedBotProfile {
    pub iq: u32,
    pub leader: String,
    pub paused: bool,
    #[serde(default)]
    pub reserved_match_id: Option<String>,
    #[serde(default)]
    pub manual_changes: Vec<RankedBotChange>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RankedBotChange {
    pub changed_at: u64,
    pub actor: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

#[derive(Clone, Debug)]
pub enum RankedBotManualUpdate {
    SetPaused { paused: bool, actor: String },
    SetIq { iq: u32, actor: String },
    SetRating { score: u16, actor: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PurchaseRecord {
    pub id: String,
    pub provider: String,
    pub environment: String,
    pub product_id: String,
    #[serde(default)]
    pub transaction_id: Option<String>,
    pub status: String,
    pub acquired_at: u64,
    pub updated_at: u64,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self {
            xp: 0,
            level: 1,
            wins: 0,
            verified_wins: 0,
            matches_played: 0,
            players_defeated: 0,
            empires_defeated: 0,
            tribes_defeated: 0,
            preferred_leader: None,
            prefer_random_leader: false,
            completed_episodes: std::collections::BTreeSet::new(),
            kills: 0,
            deaths: 0,
            assists: 0,
            leader_xp: std::collections::BTreeMap::new(),
            leader_stats: std::collections::BTreeMap::new(),
            crowns: 0,
            laurels: 0,
            gems: 0,
            owned_leaders: std::collections::BTreeSet::new(),
            owned_skins: std::collections::BTreeSet::new(),
            selected_skin: None,
            processed_revenuecat_events: std::collections::BTreeSet::new(),
            purchased_leaders: std::collections::BTreeSet::new(),
            purchased_skins: std::collections::BTreeSet::new(),
            purchase_history: std::collections::BTreeMap::new(),
            reward_receipts: std::collections::BTreeMap::new(),
            menu_guides: std::collections::BTreeMap::new(),
            unlocked_achievements: std::collections::BTreeSet::new(),
            playgames_sync_revision: 0,
            playgames_synced_revision: 0,
            crazygames_synced_revision: 0,
            intro_completed: false,
            ranked_bot: None,
        }
    }
}

impl PlayerProfile {
    pub fn for_account(account_id: &str) -> Self {
        let mut profile = Self::default();
        let leader = crate::commerce::assigned_leader_for_account(
            account_id,
            crate::commerce::current_rotation_period(),
        );
        profile.preferred_leader = Some(crate::commerce::leader_wire_id(leader).to_string());
        profile
    }

    pub fn sync_level(&mut self) {
        self.level = 1 + self.xp / 100;
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
        self.sync_level();
    }

    fn apply_reward(&mut self, leader: &str, reward: crate::rewards::MatchReward) {
        self.add_xp(reward.xp);
        let entry = self.leader_xp.entry(leader.to_string()).or_default();
        *entry = entry.saturating_add(reward.leader_xp);
        let stats = self.leader_stats.entry(leader.to_string()).or_default();
        stats.xp = stats.xp.saturating_add(reward.leader_xp);
        self.crowns = self.crowns.saturating_add(reward.crowns);
        self.laurels = self.laurels.saturating_add(reward.laurels);
    }

    /// One ledger operation for every server-owned reward. Balances, newly
    /// unlocked achievements, and the presentation receipt move together.
    fn apply_reward_receipt(
        &mut self,
        receipt_id: &str,
        match_id: &str,
        leader: &str,
        reward: crate::rewards::MatchReward,
        created_at: u64,
    ) -> bool {
        if self.reward_receipts.contains_key(receipt_id) {
            return false;
        }
        let level_before = self.level;
        let laurels_before = self.laurels;
        self.apply_reward(leader, reward);
        self.refresh_achievements();
        self.playgames_sync_revision = self.playgames_sync_revision.saturating_add(1);
        self.reward_receipts.insert(
            receipt_id.to_string(),
            crate::profile::RewardReceipt {
                id: receipt_id.to_string(),
                match_id: match_id.to_string(),
                xp: reward.xp,
                leader_xp: reward.leader_xp,
                crowns: reward.crowns,
                laurels: reward
                    .laurels
                    .saturating_add(self.laurels.saturating_sub(laurels_before)),
                unlocked_leaders: Vec::new(),
                level_before: Some(level_before),
                level_after: Some(self.level),
                personal_closure_turn: None,
                created_at,
                status: crate::profile::RewardSettlementStatus::Applied,
                presented_at: None,
                verification_status: None,
                verified_result_sha256: None,
            },
        );
        true
    }

    fn achievement_progress(&self, id: &str) -> u64 {
        crate::rewards::achievement_progress(
            id,
            crate::rewards::AchievementTotals {
                matches_played: self.matches_played as u64,
                wins: self.verified_wins as u64,
                laurels: self.laurels,
                leader_matches_played: self
                    .leader_stats
                    .values()
                    .map(|stats| stats.matches_played as u64)
                    .sum(),
                leader_wins: self
                    .leader_stats
                    .values()
                    .map(|stats| stats.verified_wins as u64)
                    .sum(),
                distinct_leaders: self.leader_stats.len() as u64,
                best_leader_xp: self.leader_xp.values().copied().max().unwrap_or_default() as u64,
            },
        )
    }

    /// Unlock achievements from server-owned aggregates. The set and laurel
    /// total are updated together, so retries cannot pay an achievement twice.
    pub fn refresh_achievements(&mut self) {
        let totals = crate::rewards::AchievementTotals {
            matches_played: self.matches_played as u64,
            wins: self.verified_wins as u64,
            laurels: self.laurels,
            leader_matches_played: self
                .leader_stats
                .values()
                .map(|stats| stats.matches_played as u64)
                .sum(),
            leader_wins: self
                .leader_stats
                .values()
                .map(|stats| stats.verified_wins as u64)
                .sum(),
            distinct_leaders: self.leader_stats.len() as u64,
            best_leader_xp: self.leader_xp.values().copied().max().unwrap_or_default() as u64,
        };
        for achievement in
            crate::rewards::newly_unlocked_achievements(totals, &self.unlocked_achievements)
        {
            self.unlocked_achievements
                .insert(achievement.id.to_string());
            self.laurels = self.laurels.saturating_add(achievement.points);
        }
    }

    pub fn achievement_views(&self) -> Vec<crate::profile::AchievementView> {
        crate::rewards::ACHIEVEMENTS
            .iter()
            .map(|achievement| crate::profile::AchievementView {
                id: achievement.id.to_string(),
                category: achievement.category.to_string(),
                title: achievement.title.to_string(),
                description: achievement.description.to_string(),
                points: achievement.points,
                progress: self
                    .achievement_progress(achievement.id)
                    .min(achievement.target),
                target: achievement.target,
                unlocked: self.unlocked_achievements.contains(achievement.id),
            })
            .collect()
    }

    fn record_match_stats_with_leader(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
        leader: Option<&str>,
    ) -> String {
        self.matches_played = self.matches_played.saturating_add(1);
        if won {
            self.wins = self.wins.saturating_add(1);
        }
        self.players_defeated = self.players_defeated.saturating_add(defeats.players);
        self.empires_defeated = self.empires_defeated.saturating_add(defeats.empires);
        self.tribes_defeated = self.tribes_defeated.saturating_add(defeats.tribes);
        self.kills = self.kills.saturating_add(kills);
        self.deaths = self.deaths.saturating_add(deaths);
        self.assists = self.assists.saturating_add(assists);

        let leader = leader
            .and_then(crate::rewards::canonical_leader_name)
            .or_else(|| {
                self.preferred_leader
                    .as_deref()
                    .and_then(crate::rewards::canonical_leader_name)
            })
            .unwrap_or_else(|| "Caesar".to_string());
        let leader_stats = self.leader_stats.entry(leader.clone()).or_default();
        leader_stats.matches_played = leader_stats.matches_played.saturating_add(1);
        if won {
            leader_stats.wins = leader_stats.wins.saturating_add(1);
        }
        leader_stats.kills = leader_stats.kills.saturating_add(kills);
        leader_stats.deaths = leader_stats.deaths.saturating_add(deaths);
        leader_stats.assists = leader_stats.assists.saturating_add(assists);
        leader
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerAccount {
    /// The sole stable account identifier. WOU-ID linked accounts use the same
    /// UUID; anonymous accounts use their existing local identifier.
    #[serde(rename = "account_id")]
    pub id: String,
    /// Mutable player-facing name. Never used as an identity key.
    // Missing names are normalized when the account is created; this field is
    // never used as an identity key.
    #[serde(default)]
    pub display_name: String,
    pub profile: PlayerProfile,
    pub linked_identities: Vec<LinkedIdentity>,
    /// Account kind. Missing kind fields in older records deserialize as Human.
    /// Bots are accounts with `kind = Bot` — they have profiles, accumulate
    /// stats, and serve as the persistent identity pool for internal fillers.
    #[serde(default)]
    pub kind: AccountKind,
    /// BLAKE3 hash (hex) of the anonymous account secret. Only the hash is
    /// persisted; the plaintext is revealed to the client exactly once, when
    /// minted. Proves account ownership on `JoinWithAuth`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_secret_hash: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl PlayerAccount {
    /// Remove storage-only ownership and verified-result fingerprints before
    /// returning an account outside the data service.
    pub fn without_auth_secret(mut self) -> Self {
        self.auth_secret_hash = None;
        for receipt in self.profile.reward_receipts.values_mut() {
            receipt.verified_result_sha256 = None;
        }
        self
    }
}

/// Distinguishes real players from persistent bot accounts. Bots are
/// full-fledged accounts (they have stats, profiles) — they just aren't
/// driven by a human. Used for stat filtering, leaderboards, display.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccountKind {
    #[default]
    Human,
    Bot,
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct LinkedIdentity {
    pub provider: String, // "crazygames" for verified players or "bot" for server fillers
    pub external_id: String, // Stable unique ID from the platform
    #[serde(default = "default_identity_environment")]
    pub environment: String,
}

fn default_identity_environment() -> String {
    "production".to_string()
}

/// Result of an operator-driven account erasure (privacy deletion
/// requests). `found` reports whether the account record still existed;
/// analytics residue is scrubbed regardless so orphaned memberships
/// cannot survive a re-request.
#[derive(Serialize, Debug)]
pub struct DeleteAccountReport {
    pub account_id: String,
    pub found: bool,
    pub keys_removed: u64,
    pub redb_rows_removed: u32,
    pub analytics_sets_scrubbed: u64,
}

/// Operator-only reset report for disposable human test accounts. Bots and
/// static game data are deliberately outside this operation.
#[derive(Serialize, Debug)]
pub struct ResetHumanAccountsReport {
    pub dry_run: bool,
    pub human_accounts: u64,
    pub bots_preserved: u64,
    pub purchase_records: u64,
    pub redis_keys_removed: u64,
    pub redis_members_removed: u64,
    pub redb_rows_removed: u64,
    pub match_rows_removed: u64,
    pub rating_rows_removed: u64,
}

fn collect_purchase_markers(
    account: &PlayerAccount,
    markers: &mut std::collections::BTreeSet<String>,
) {
    for purchase_id in account.profile.purchase_history.keys() {
        markers.insert(format!("{}:history:{purchase_id}", account.id));
    }
    for leader_id in &account.profile.purchased_leaders {
        markers.insert(format!("{}:leader:{leader_id}", account.id));
    }
    for skin_id in &account.profile.purchased_skins {
        markers.insert(format!("{}:skin:{skin_id}", account.id));
    }
}

#[derive(Clone)]
pub struct PlayerDb {
    client: Client,
    /// One shared connection for the whole process.
    ///
    /// `redis::aio::MultiplexedConnection` is built to be created once and
    /// cloned cheaply: every clone multiplexes its commands over the *same*
    /// socket. The previous behaviour called
    /// `get_multiplexed_async_connection()` on every one of the ~49 async
    /// call sites, paying a fresh connect + handshake + AUTH each time.
    ///
    /// `OnceCell` keeps `PlayerDb::new` synchronous (so no call site or
    /// `AppState` construction changes) while still connecting lazily, and it
    /// caches *only* success — a transient connect failure is retried on the
    /// next call instead of poisoning the handle.
    connection: std::sync::Arc<tokio::sync::OnceCell<redis::aio::MultiplexedConnection>>,
    pub crazygames_api_key: Option<String>,
    pub redb_db: Option<std::sync::Arc<redb::Database>>,
}

impl PlayerDb {
    pub fn new(
        redis_url: &str,
        crazygames_api_key: Option<String>,
        redb_db: Option<std::sync::Arc<redb::Database>>,
    ) -> Self {
        let client = Client::open(redis_url).expect("Failed to connect to Valkey/Redis");
        info!("Successfully initialized Valkey database connector client.");
        Self {
            client,
            connection: std::sync::Arc::new(tokio::sync::OnceCell::new()),
            crazygames_api_key,
            redb_db,
        }
    }

    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, redis::RedisError> {
        let connection = self
            .connection
            .get_or_try_init(|| async {
                let connection = self.client.get_multiplexed_async_connection().await?;
                info!("Established shared multiplexed Valkey connection.");
                Ok::<_, redis::RedisError>(connection)
            })
            .await?;
        Ok(connection.clone())
    }

    /// Establish storage before accepting requests; an unavailable account
    /// authority must never fall back to a stale cache.
    pub async fn warm_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.migrate_legacy_redb_profiles()?;
        self.get_connection().await?;
        Ok(())
    }

    pub async fn account_by_id(
        &self,
        account_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        self.load_account(account_id).await
    }

    /// Return the server-owned current-season 1v1 rating, including its
    /// initial value for an account that has not played yet.
    pub async fn ranked_rating(
        &self,
        account_id: &str,
    ) -> Result<SeasonRating, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.load_account(account_id).await?;
        let initial_mmr = match account.kind {
            AccountKind::Human => 1000.0,
            AccountKind::Bot => {
                let bot = account.profile.ranked_bot.as_ref().ok_or("account is not a ranked bot")?;
                1000.0 + f64::from(bot.iq.saturating_sub(60)) * 20.0
            }
        };
        let key = format!("rating:{}:{}:Ranked1v1:FFA", account.id, crate::profile::CURRENT_SEASON_ID);
        let Some(db) = &self.redb_db else {
            return Err("rating database unavailable".into());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        if let Some(value) = table.get(key.as_str())? {
            let rating = serde_json::from_slice::<SeasonRating>(value.value())?;
            if rating.account_id != account.id || rating.season_id != crate::profile::CURRENT_SEASON_ID
                || rating.queue != "Ranked1v1" || rating.mode != "FFA" || !rating.mu.is_finite()
            {
                return Err("stored ranked rating identity is invalid".into());
            }
            return Ok(rating);
        }
        Ok(crate::profile::initial_ranked_rating(account.id, initial_mmr, 0))
    }

    pub async fn available_ranked_bots(
        &self,
    ) -> Result<Vec<(PlayerAccount, SeasonRating)>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("ranked bot database unavailable".into());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(RANKED_BOTS_TABLE)?;
        let mut account_ids = Vec::new();
        for row in table.iter()? {
            let (key, _) = row?;
            account_ids.push(key.value().to_string());
        }
        drop(table);
        drop(read_txn);
        let rating_txn = db.begin_read()?;
        let ratings = rating_txn.open_table(SEASON_RATINGS_TABLE)?;
        let mut stored_ratings = std::collections::HashMap::new();
        for account_id in &account_ids {
            let key = format!("rating:{}:{}:Ranked1v1:FFA", account_id, crate::profile::CURRENT_SEASON_ID);
            if let Some(value) = ratings.get(key.as_str())? {
                stored_ratings.insert(account_id.clone(), serde_json::from_slice::<SeasonRating>(value.value())?);
            }
        }
        drop(ratings);
        drop(rating_txn);
        let mut candidates = Vec::new();
        for account_id in account_ids {
            let account = self.load_account(&account_id).await?;
            let Some(bot) = account.profile.ranked_bot.as_ref() else {
                return Err(format!("ranked bot index points to non-ranked account {}", account.id).into());
            };
            if account.kind != AccountKind::Bot || bot.paused || bot.reserved_match_id.is_some() {
                continue;
            }
            let rating = match stored_ratings.remove(&account.id) {
                Some(rating) => rating,
                None => crate::profile::initial_ranked_rating(
                    account.id.clone(),
                    1000.0 + f64::from(bot.iq.saturating_sub(60)) * 20.0,
                    0,
                ),
            };
            if !rating.mu.is_finite() || rating.account_id != account.id {
                return Err(format!("invalid ranked bot rating for {}", account.id).into());
            }
            candidates.push((account, rating));
        }
        Ok(candidates)
    }

    /// Read the settled, server-recorded rating change for one verified duel.
    pub fn ranked_match_result(
        &self,
        account_id: &str,
        match_id: &str,
    ) -> Result<Option<crate::profile::RankedResultPresentation>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(record) = self.load_match_record(match_id)? else {
            return Ok(None);
        };
        if record.queue != "Ranked1v1" || !record.verified || !record.rating_eligible {
            return Ok(None);
        }
        let Some(participant) = record.participants.iter().find(|item| item.account_id == account_id) else {
            return Ok(None);
        };
        let (Some(score_before), Some(score_after), Some(tier_before), Some(tier_after)) = (
            participant.rating_before,
            participant.rating_after,
            participant.tier_before.as_ref(),
            participant.tier_after.as_ref(),
        ) else {
            return Ok(None);
        };
        Ok(Some(crate::profile::RankedResultPresentation {
            ladder: Some(crate::profile::ranked_ladder()),
            score_before,
            score_after,
            rating_delta: participant.rating_delta.unwrap_or_default(),
            tier_before: tier_before.clone(),
            tier_after: tier_after.clone(),
            division_before: participant.division_before.clone(),
            division_after: participant.division_after.clone(),
            games_played: participant.games_played_after.unwrap_or_default(),
            placements_complete: participant.placements_complete_after.unwrap_or(false),
            points_breakdown: participant.ranked_points_breakdown.clone(),
        }))
    }

    /// Record an already validated batch in one Redis round trip. The Lua
    /// script makes deduplication and every counter/membership update atomic.
    pub async fn record_analytics_batch(
        &self,
        events: &[crate::events::AnalyticsEvent],
    ) -> Result<Vec<bool>, redis::RedisError> {
        if events.is_empty() {
            return Ok(Vec::new());
        }
        let day_number = crate::events::utc_day_number();
        let date = crate::events::shift_date("1970-01-01", day_number).ok_or_else(|| {
            redis::RedisError::from((redis::ErrorKind::TypeError, "invalid UTC day"))
        })?;
        let mut con = self.get_connection().await?;
        let mut pipe = redis::pipe();
        for event in events {
            let portal = event.portal.as_deref().unwrap_or("site");
            let subject = event.subject_id.as_deref().unwrap_or_default();
            let date_prefix = format!("{portal}:{date}");
            let first_key = format!("sow:analytics:first_seen:v2:{portal}:{subject}");
            let cohort_prefix = format!("sow:analytics:cohort:v2:{portal}:");
            let tutorial_step = event
                .props
                .as_ref()
                .filter(|_| matches!(event.name.as_str(), "tutorial_step" | "tutorial_exit_early"))
                .and_then(serde_json::Value::as_object)
                .and_then(|props| {
                    Some(format!(
                        "{}|{}|{}|{}",
                        props.get("episode_id")?.as_str()?,
                        props.get("step_id")?.as_str()?,
                        props.get("step_index")?.as_u64()?,
                        props.get("action")?.as_str()?
                    ))
                })
                .unwrap_or_default();
            pipe.cmd("EVAL")
                .arg(RECORD_ANALYTICS_EVENT)
                .arg(15)
                .arg(format!(
                    "sow:analytics:event_id:{portal}:{}",
                    event.event_id
                ))
                .arg(format!("sow:analytics:event:{date_prefix}:{}", event.name))
                .arg(format!("sow:analytics:sessions:{date_prefix}"))
                .arg(format!("sow:analytics:active_accounts:{date_prefix}"))
                .arg(format!(
                    "sow:analytics:event_users:{date_prefix}:{}",
                    event.name
                ))
                .arg(first_key)
                .arg(cohort_prefix)
                .arg(format!("sow:analytics:retention:v2:{portal}:d1:"))
                .arg(format!("sow:analytics:retention:v2:{portal}:d3:"))
                .arg(format!("sow:analytics:retention:v2:{portal}:d7:"))
                .arg(format!("sow:analytics:builds:{portal}:{date}"))
                .arg(format!("sow:analytics:by_build:{portal}:{date}"))
                .arg(format!("sow:analytics:by_device:{portal}:{date}"))
                .arg(format!(
                    "sow:analytics:event_sessions:{date_prefix}:{}",
                    event.name
                ))
                .arg(format!("sow:analytics:tutorial_steps:{portal}:{date}"))
                .arg(ANALYTICS_TTL_SECONDS)
                .arg(day_number)
                .arg(&event.session_id)
                .arg(subject)
                .arg(event.build.as_deref().unwrap_or("unknown"))
                .arg(event.device_class.as_deref().unwrap_or("unknown"))
                .arg(&event.name)
                .arg(tutorial_step);
        }
        let outcomes: Vec<i64> = pipe.query_async(&mut con).await?;
        outcomes
            .into_iter()
            .map(|outcome| match outcome {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(redis::RedisError::from((
                    redis::ErrorKind::TypeError,
                    "unexpected analytics event result",
                ))),
            })
            .collect()
    }

    /// Remove pseudonymous analytics memberships derived from a deleted
    /// account. The IDs are portal-scoped HMACs created by the ingest owner.
    pub async fn erase_analytics_subjects(
        &self,
        subject_ids: &[String],
    ) -> Result<u64, redis::RedisError> {
        if subject_ids.is_empty() {
            return Ok(0);
        }
        let mut con = self.get_connection().await?;
        let mut removed = 0_u64;
        for portal in ["site", "poki", "crazygames", "jest", "android"] {
            for subject_id in subject_ids {
                for key in [
                    format!("sow:analytics:first_seen:{portal}:{subject_id}"),
                    format!("sow:analytics:first_seen:v2:{portal}:{subject_id}"),
                ] {
                    let deleted: u64 = con.del(key).await?;
                    removed = removed.saturating_add(deleted);
                }
            }
        }
        let mut cursor = 0_u64;
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("sow:analytics:*:*")
                .arg("COUNT")
                .arg(256)
                .query_async(&mut con)
                .await?;
            for key in keys {
                if [
                    "sow:analytics:active_accounts:",
                    "sow:analytics:event_users:",
                    "sow:analytics:cohort:",
                    "sow:analytics:retention:",
                ]
                .iter()
                .any(|prefix| key.starts_with(prefix))
                {
                    let count: u64 = con.srem(&key, subject_ids).await?;
                    removed = removed.saturating_add(count);
                }
            }
            cursor = next;
            if cursor == 0 {
                break;
            }
        }
        Ok(removed)
    }

    /// Private operator snapshot; only the owned website has first-party
    /// ingestion enabled. Other sources are explicitly marked unavailable.
    pub async fn analytics_summary(
        &self,
        requested_days: u32,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let days = requested_days.clamp(1, 90) as i64;
        let today_day = crate::events::utc_day_number();
        let today = crate::events::shift_date("1970-01-01", today_day).ok_or("invalid UTC date")?;
        let portal = "site";
        let mut con = self.get_connection().await?;
        let mut pipe = redis::pipe();
        let mut daily = Vec::new();
        let mut value_index = 0;
        for offset in 0..days {
            let date = crate::events::shift_date(&today, -offset).ok_or("invalid UTC date")?;
            let prefix = format!("{portal}:{date}");
            let start = value_index;
            pipe.cmd("SCARD")
                .arg(format!("sow:analytics:sessions:{prefix}"))
                .cmd("SCARD")
                .arg(format!("sow:analytics:active_accounts:{prefix}"));
            value_index += 2;
            for name in crate::events::EVENT_NAMES {
                pipe.cmd("GET")
                    .arg(format!("sow:analytics:event:{prefix}:{name}"));
                pipe.cmd("SCARD")
                    .arg(format!("sow:analytics:event_sessions:{prefix}:{name}"));
                value_index += 2;
            }
            daily.push((date, start));
        }
        let values: Vec<Option<u64>> = pipe.query_async(&mut con).await?;
        let mut daily_rows = Vec::with_capacity(days as usize);
        let tutorial_dates = daily
            .iter()
            .map(|(date, _)| date.clone())
            .collect::<Vec<_>>();
        let mut funnel = serde_json::Map::new();
        let mut funnel_sessions = serde_json::Map::new();
        let mut dimensions_pipe = redis::pipe();
        for (date, _) in &daily {
            dimensions_pipe
                .cmd("HGETALL")
                .arg(format!("sow:analytics:by_build:{portal}:{date}"))
                .cmd("HGETALL")
                .arg(format!("sow:analytics:by_device:{portal}:{date}"));
        }
        let dimensions: Vec<std::collections::HashMap<String, u64>> =
            dimensions_pipe.query_async(&mut con).await?;
        let mut by_build =
            std::collections::HashMap::<String, std::collections::HashMap<String, u64>>::new();
        let mut by_device =
            std::collections::HashMap::<String, std::collections::HashMap<String, u64>>::new();
        for (index, dimension_rows) in dimensions.iter().enumerate() {
            for (field, count) in dimension_rows {
                let Some((dimension, event)) = field.split_once('|') else {
                    continue;
                };
                let target = if index % 2 == 0 {
                    &mut by_build
                } else {
                    &mut by_device
                };
                let total = target
                    .entry(dimension.to_string())
                    .or_default()
                    .entry(event.to_string())
                    .or_default();
                *total = total.saturating_add(*count);
            }
        }
        for (date, start) in daily {
            let mut events = serde_json::Map::new();
            let mut event_sessions = serde_json::Map::new();
            for (index, name) in crate::events::EVENT_NAMES.iter().enumerate() {
                let count = values
                    .get(start + 2 + index * 2)
                    .copied()
                    .flatten()
                    .unwrap_or(0);
                let sessions = values
                    .get(start + 3 + index * 2)
                    .copied()
                    .flatten()
                    .unwrap_or(0);
                events.insert((*name).to_string(), serde_json::json!(count));
                event_sessions.insert((*name).to_string(), serde_json::json!(sessions));
                let total = funnel
                    .entry((*name).to_string())
                    .or_insert_with(|| serde_json::json!(0_u64));
                *total = serde_json::json!(total.as_u64().unwrap_or(0).saturating_add(count));
                let session_total = funnel_sessions
                    .entry((*name).to_string())
                    .or_insert_with(|| serde_json::json!(0_u64));
                *session_total =
                    serde_json::json!(session_total.as_u64().unwrap_or(0).saturating_add(sessions));
            }
            daily_rows.push(serde_json::json!({
                "date": date,
                "active_sessions": values.get(start).copied().flatten().unwrap_or(0),
                "active_accounts": values.get(start + 1).copied().flatten().unwrap_or(0),
                "events": events,
                "event_sessions": event_sessions,
            }));
        }

        let mut retention_pipe = redis::pipe();
        let mut cohort_offsets = Vec::new();
        let mut retention_index = 0;
        for offset in 1..days {
            let cohort_day = today_day - offset;
            let base = retention_index;
            retention_pipe
                .cmd("SCARD")
                .arg(format!("sow:analytics:cohort:v2:{portal}:{cohort_day}"));
            retention_index += 1;
            for window in [1, 3, 7] {
                if offset > window {
                    retention_pipe.cmd("SCARD").arg(format!(
                        "sow:analytics:retention:v2:{portal}:d{window}:{cohort_day}"
                    ));
                    retention_index += 1;
                }
            }
            cohort_offsets.push((offset, base));
        }
        let retention_values: Vec<Option<u64>> = retention_pipe.query_async(&mut con).await?;
        let mut retention_cohorts = Vec::with_capacity(cohort_offsets.len());
        for (offset, base) in cohort_offsets {
            let eligible = retention_values.get(base).copied().flatten().unwrap_or(0);
            let mut index = base + 1;
            let mut returned = [0_u64; 3];
            for (slot, window) in [1_i64, 3, 7].into_iter().enumerate() {
                if offset > window {
                    returned[slot] = retention_values.get(index).copied().flatten().unwrap_or(0);
                    index += 1;
                }
            }
            retention_cohorts.push(RetentionCohort {
                offset,
                eligible,
                returned,
            });
        }
        let retention = summarize_retention(&today, &retention_cohorts);

        let mut tutorial_steps_pipe = redis::pipe();
        for date in &tutorial_dates {
            tutorial_steps_pipe
                .cmd("HGETALL")
                .arg(format!("sow:analytics:tutorial_steps:{portal}:{date}"));
        }
        let tutorial_step_rows: Vec<std::collections::HashMap<String, u64>> =
            tutorial_steps_pipe.query_async(&mut con).await?;
        let tutorial_steps = aggregate_tutorial_steps(&tutorial_step_rows);

        Ok(serde_json::json!({
            "generated_at": today,
            "days": days,
            "sources": {
                "site": { "status": "connected", "source": "first-party endpoint" },
                "poki": { "status": "platform-only", "source": "Poki developer dashboard" },
                "crazygames": { "status": "platform-only", "source": "CrazyGames dashboard" },
                "jest": { "status": "platform-only", "source": "Jest dashboard" },
                "android": { "status": "disabled-pending-disclosure", "source": "Google Play Console" },
            },
            "funnel": funnel,
            "funnel_sessions": funnel_sessions,
            "dimensions": {
                "build": by_build,
                "device_class": by_device,
            },
            "daily": daily_rows,
            "retention": { "site": retention },
            "tutorial_steps": tutorial_steps,
        }))
    }

    /// Crate-internal connection accessor for sibling server modules
    /// (moderation, future operator tooling). Auth stays with the callers.
    pub(crate) async fn client_conn(
        &self,
    ) -> Result<redis::aio::MultiplexedConnection, redis::RedisError> {
        self.get_connection().await
    }

    /// Persist one completed match and its per-player lookup keys. The match
    /// row is idempotent, so relay retries cannot create duplicate history.
    pub fn save_match_record(
        &self,
        record: &MatchRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.persist_match_record(record, false, None)
    }

    fn save_match_record_allow_closed_update(
        &self,
        record: &MatchRecord,
        account_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.persist_match_record(record, false, Some(account_id))
    }

    #[cfg(test)]
    fn save_provisional_match_record(
        &self,
        record: &MatchRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.persist_match_record(record, true, None)
    }

    fn persist_match_record(
        &self,
        record: &MatchRecord,
        merge_provisional: bool,
        replace_closed_account: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("Redb database unavailable".into());
        };
        let write_txn = db.begin_write()?;
        let matches = write_txn.open_table(MATCHES_TABLE)?;
        let previous = matches
            .get(record.match_id.as_str())?
            .map(|value| serde_json::from_slice::<MatchRecord>(value.value()))
            .transpose()?;
        if merge_provisional && previous.as_ref().is_some_and(|saved| !saved.provisional) {
            return Ok(());
        }
        let mut next = record.clone();
        if merge_provisional {
            if let Some(saved) = &previous {
                next = saved.clone();
                next.completed_at = record.completed_at;
                next.duration_seconds = record.duration_seconds;
                for participant in &record.participants {
                    if let Some(existing) = next
                        .participants
                        .iter_mut()
                        .find(|existing| existing.account_id == participant.account_id)
                    {
                        *existing = participant.clone();
                    } else {
                        next.participants.push(participant.clone());
                    }
                }
            }
            next.provisional = true;
            next.verified = false;
        } else if let Some(saved) = &previous {
            for participant in saved
                .participants
                .iter()
                .filter(|participant| participant.personal_closure_turn.is_some())
                .filter(|participant| {
                    Some(participant.account_id.as_str()) != replace_closed_account
                })
            {
                if let Some(current) = next
                    .participants
                    .iter_mut()
                    .find(|current| current.account_id == participant.account_id)
                {
                    *current = participant.clone();
                } else {
                    next.participants.push(participant.clone());
                }
            }
        }
        drop(matches);
        write_match_record_in_transaction(&write_txn, &next)?;
        write_txn.commit()?;
        Ok(())
    }

    pub fn save_season_rating(
        &self,
        rating: &SeasonRating,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("Redb database unavailable".into());
        };
        let key = format!(
            "rating:{}:{}:{}:{}",
            rating.account_id, rating.season_id, rating.queue, rating.mode
        );
        let write_txn = db.begin_write()?;
        let mut table = write_txn.open_table(SEASON_RATINGS_TABLE)?;
        let json = serde_json::to_vec(rating)?;
        table.insert(key.as_str(), json.as_slice())?;
        drop(table);
        write_txn.commit()?;
        Ok(())
    }

    pub fn save_season(
        &self,
        season: &SeasonRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("Redb database unavailable".into());
        };
        let key = format!("season:{:08}", season.id);
        let write_txn = db.begin_write()?;
        let mut table = write_txn.open_table(SEASONS_TABLE)?;
        let json = serde_json::to_vec(season)?;
        table.insert(key.as_str(), json.as_slice())?;
        drop(table);
        write_txn.commit()?;
        Ok(())
    }

    fn public_index(
        &self,
        account_id: &str,
    ) -> Result<Option<PublicProfileIndex>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Ok(None);
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PUBLIC_PROFILES_TABLE)?;
        let Some(value) = table.get(account_id)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(value.value())?))
    }

    fn load_match_record(
        &self,
        match_id: &str,
    ) -> Result<Option<MatchRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Ok(None);
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(MATCHES_TABLE)?;
        let Some(value) = table.get(match_id)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(value.value())?))
    }

    fn match_history_for_account(
        &self,
        account_id: &str,
        offset: usize,
        limit: usize,
        queue: Option<&str>,
        mode: Option<&str>,
    ) -> Result<Vec<MatchRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Ok(Vec::new());
        };
        let prefix = format!("player:{account_id}:");
        let upper = format!("{prefix}\u{10ffff}");
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
        let mut match_ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in table.range(prefix.as_str()..upper.as_str())?.rev() {
            let (key, value) = item?;
            if !key.value().starts_with(&prefix) {
                break;
            }
            if let Ok(match_id) = std::str::from_utf8(value.value())
                && seen.insert(match_id.to_string())
            {
                match_ids.push(match_id.to_string());
            }
        }
        drop(table);
        drop(read_txn);
        let mut skipped = 0usize;
        let mut records = Vec::with_capacity(limit.min(match_ids.len().saturating_sub(offset)));
        for match_id in match_ids {
            if let Some(record) = self.load_match_record(&match_id)?
                && queue.is_none_or(|value| value == record.queue)
                && mode.is_none_or(|value| value == record.mode)
            {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                records.push(record);
                if records.len() >= limit {
                    break;
                }
            }
        }
        Ok(records)
    }

    fn public_match_summary(account_id: &str, record: &MatchRecord) -> Option<PublicMatchSummary> {
        let participant = record
            .participants
            .iter()
            .find(|participant| participant.account_id == account_id)?;
        Some(PublicMatchSummary {
            match_id: record.match_id.clone(),
            completed_at: record.completed_at,
            queue: record.queue.clone(),
            mode: record.mode.clone(),
            map_name: record.map_name.clone(),
            duration_seconds: record.duration_seconds,
            placement: participant.placement,
            won: participant.won,
            leader: participant.leader.clone(),
            kills: participant.kills,
            deaths: participant.deaths,
            assists: participant.assists,
            players_defeated: participant.players_defeated,
            verified: record.verified,
            provisional: record.provisional,
            rating_delta: participant.rating_delta,
            ranked_points_breakdown: participant.ranked_points_breakdown.clone(),
            ranked_bot_iq: record.ranked_bot_iq,
        })
    }

    fn public_summary(account: &PlayerAccount) -> PublicProfileSummary {
        PublicProfileSummary {
            account_id: account.id.clone(),
            handle: public_handle(&account.display_name, &account.id),
            display_name: account.display_name.clone(),
            level: account.profile.level,
            matches_played: account.profile.matches_played,
            wins: account.profile.verified_wins,
            win_rate: win_rate(
                account.profile.verified_wins,
                account.profile.matches_played,
            ),
            kills: account.profile.kills,
            deaths: account.profile.deaths,
            assists: account.profile.assists,
        }
    }

    /// Public profile DTO with server-owned achievements and laurel points.
    pub async fn public_profile(
        &self,
        account_id: &str,
    ) -> Result<Option<PublicProfileView>, Box<dyn std::error::Error + Send + Sync>> {
        let account = match self.public_index(account_id)? {
            Some(index) => self.load_account(&index.account_id).await?,
            None => match self.load_account(account_id).await {
                Ok(account) if account.kind == AccountKind::Bot && account.profile.ranked_bot.is_some() => account,
                _ => return Ok(None),
            },
        };
        if account.kind == AccountKind::Bot && account.profile.ranked_bot.is_none() {
            return Ok(None);
        }
        let ranked_bot_iq = account.profile.ranked_bot.as_ref().map(|bot| bot.iq);
        let ranked_bot = account.kind == AccountKind::Bot && ranked_bot_iq.is_some();
        let mut leader_stats = account.profile.leader_stats.clone();
        for (leader, xp) in &account.profile.leader_xp {
            leader_stats
                .entry(leader.clone())
                .or_insert_with(|| LeaderCareerStats {
                    xp: *xp,
                    ..Default::default()
                });
        }
        let mut leaders = leader_stats
            .into_iter()
            .map(|(leader, stats)| PublicLeaderSummary {
                leader,
                matches_played: stats.matches_played,
                wins: stats.verified_wins,
                win_rate: win_rate(stats.verified_wins, stats.matches_played),
                kills: stats.kills,
                deaths: stats.deaths,
                assists: stats.assists,
                xp: stats.xp,
            })
            .collect::<Vec<_>>();
        leaders.sort_by(|left, right| {
            right
                .matches_played
                .cmp(&left.matches_played)
                .then_with(|| right.wins.cmp(&left.wins))
                .then_with(|| right.xp.cmp(&left.xp))
        });
        let recent_matches = self
            .match_history_for_account(
                &account.id,
                0,
                10,
                ranked_bot.then_some("Ranked1v1"),
                None,
            )?
            .iter()
            .filter_map(|record| Self::public_match_summary(&account.id, record))
            .collect();
        let ratings = self.public_ratings(&account.id).await?.unwrap_or_default();
        Ok(Some(PublicProfileView {
            ranked_ladder: Some(crate::profile::ranked_ladder()),
            account_id: account.id.clone(),
            handle: public_handle(&account.display_name, &account.id),
            display_name: account.display_name,
            is_bot: ranked_bot,
            ranked_bot_iq,
            level: account.profile.level,
            matches_played: account.profile.matches_played,
            wins: account.profile.verified_wins,
            win_rate: win_rate(
                account.profile.verified_wins,
                account.profile.matches_played,
            ),
            kills: account.profile.kills,
            deaths: account.profile.deaths,
            assists: account.profile.assists,
            players_defeated: account.profile.players_defeated,
            empires_defeated: account.profile.empires_defeated,
            tribes_defeated: account.profile.tribes_defeated,
            preferred_leader: account.profile.preferred_leader.clone(),
            laurels: account.profile.laurels,
            achievements: account.profile.achievement_views(),
            leaders,
            recent_matches,
            ratings,
        }))
    }

    pub async fn playgames_sync_snapshot(
        &self,
        account_id: &str,
    ) -> Result<Option<PlayGamesSyncSnapshot>, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.load_account(account_id).await?;
        if account.kind == AccountKind::Bot {
            return Ok(None);
        }
        Ok(Some(PlayGamesSyncSnapshot {
            unlocked_achievements: account
                .profile
                .unlocked_achievements
                .iter()
                .cloned()
                .collect(),
            wins: account.profile.verified_wins,
            revision: account.profile.playgames_sync_revision,
            synced_revision: account.profile.playgames_synced_revision,
        }))
    }

    pub async fn mark_playgames_sync(
        &self,
        account_id: &str,
        revision: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.update_account_atomic(account_id, &[], |account| {
            if account.profile.playgames_sync_revision == revision {
                account.profile.playgames_synced_revision = revision;
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            }
        })
        .await?;
        Ok(())
    }

    pub async fn search_public_profiles(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<PublicProfileSummary>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Ok(Vec::new());
        };
        let needle = query.trim().to_lowercase();
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PUBLIC_PROFILES_TABLE)?;
        let mut indexes = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let index: PublicProfileIndex = serde_json::from_slice(value.value())?;
            if index.kind == "Bot" {
                continue;
            }
            if needle.is_empty()
                || index.display_name.to_lowercase().contains(&needle)
                || index.account_id.to_lowercase().contains(&needle)
            {
                indexes.push(index);
            }
            if indexes.len() >= limit {
                break;
            }
        }
        drop(table);
        drop(read_txn);
        let mut result = Vec::with_capacity(indexes.len());
        for index in indexes {
            if let Ok(account) = self.load_account(&index.account_id).await
                && account.kind != AccountKind::Bot
            {
                result.push(Self::public_summary(&account));
            }
        }
        Ok(result)
    }

    pub async fn public_match_history(
        &self,
        account_id: &str,
        offset: usize,
        limit: usize,
        queue: Option<&str>,
        mode: Option<&str>,
    ) -> Result<Option<Vec<PublicMatchSummary>>, Box<dyn std::error::Error + Send + Sync>> {
        let account = match self.public_index(account_id)? {
            Some(index) => self.load_account(&index.account_id).await?,
            None => match self.load_account(account_id).await {
                Ok(account)
                    if account.kind == AccountKind::Bot
                        && account.profile.ranked_bot.is_some() => account,
                _ => return Ok(None),
            },
        };
        let ranked_bot = account.kind == AccountKind::Bot && account.profile.ranked_bot.is_some();
        if account.kind == AccountKind::Bot && !ranked_bot {
            return Ok(None);
        }
        if ranked_bot && queue.is_some_and(|value| value != "Ranked1v1") {
            return Ok(Some(Vec::new()));
        }
        Ok(Some(
            self.match_history_for_account(
                &account.id,
                offset,
                limit.saturating_add(1),
                if ranked_bot { Some("Ranked1v1") } else { queue },
                mode,
            )?
            .iter()
            .filter_map(|record| Self::public_match_summary(&account.id, record))
            .collect(),
        ))
    }

    pub fn public_match_detail(
        &self,
        match_id: &str,
    ) -> Result<Option<PublicMatchDetail>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(record) = self.load_match_record(match_id)? else {
            return Ok(None);
        };
        let winner_account_id = record.winner_account_id.clone();
        let participants = record
            .participants
            .iter()
            .map(|participant| PublicMatchParticipant {
                account_id: participant.account_id.clone(),
                handle: public_handle(&participant.display_name, &participant.account_id),
                is_bot: participant.is_bot,
                leader: participant.leader.clone(),
                team: participant.team.clone(),
                placement: participant.placement,
                won: participant.won,
                kills: participant.kills,
                deaths: participant.deaths,
                assists: participant.assists,
                players_defeated: participant.players_defeated,
                verified: record.verified,
                rating_delta: participant.rating_delta,
                rating_before: participant.rating_before,
                rating_after: participant.rating_after,
                ranked_points_breakdown: participant.ranked_points_breakdown.clone(),
            })
            .collect();
        Ok(Some(PublicMatchDetail {
            match_id: record.match_id,
            completed_at: record.completed_at,
            queue: record.queue,
            mode: record.mode,
            map_name: record.map_name,
            duration_seconds: record.duration_seconds,
            winner_account_id,
            winning_team: record.winning_team,
            verified: record.verified,
            provisional: record.provisional,
            rating_eligible: record.rating_eligible,
            ranked_bot_iq: record.ranked_bot_iq,
            participants,
        }))
    }

    pub fn ensure_current_season(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("Redb database unavailable".into());
        };
        let key = format!("season:{:08}", crate::profile::CURRENT_SEASON_ID);
        let read_txn = db.begin_read()?;
        let exists = {
            let table = read_txn.open_table(SEASONS_TABLE)?;
            table.get(key.as_str())?.is_some()
        };
        drop(read_txn);
        if !exists {
            self.save_season(&SeasonRecord {
                id: crate::profile::CURRENT_SEASON_ID,
                name: crate::profile::CURRENT_SEASON_NAME.to_string(),
                status: "active".to_string(),
                starts_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                ends_at: None,
            })?;
        }
        Ok(())
    }

    pub fn seasons(&self) -> Result<Vec<SeasonRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Ok(Vec::new());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASONS_TABLE)?;
        let mut seasons = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            seasons.push(serde_json::from_slice(value.value())?);
        }
        seasons.sort_by_key(|season: &SeasonRecord| season.id);
        Ok(seasons)
    }

    pub async fn public_ratings(
        &self,
        account_id: &str,
    ) -> Result<Option<Vec<PublicRatingView>>, Box<dyn std::error::Error + Send + Sync>> {
        let indexed = self.public_index(account_id)?;
        let account = match indexed {
            Some(index) => self.load_account(&index.account_id).await?,
            None => match self.load_account(account_id).await {
                Ok(account) if account.kind == AccountKind::Bot && account.profile.ranked_bot.is_some() => account,
                _ => return Ok(None),
            },
        };
        let Some(db) = &self.redb_db else {
            return Ok(Some(Vec::new()));
        };
        let prefix = format!("rating:{}:", account.id);
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        let mut ratings = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            if key.value().starts_with(&prefix) {
                let rating = serde_json::from_slice::<SeasonRating>(value.value())?;
                if account.kind == AccountKind::Human || rating.queue == "Ranked1v1" {
                    ratings.push(rating);
                }
            }
        }
        drop(table);
        drop(read_txn);
        if !ratings.iter().any(|rating| {
            rating.season_id == crate::profile::CURRENT_SEASON_ID
                && rating.queue == "Ranked1v1"
                && rating.mode == "FFA"
        }) {
            ratings.push(self.ranked_rating(&account.id).await?);
        }
        ratings.sort_by(|left, right| {
            right
                .season_id
                .cmp(&left.season_id)
                .then_with(|| left.queue.cmp(&right.queue))
                .then_with(|| left.mode.cmp(&right.mode))
        });
        Ok(Some(
            ratings
                .into_iter()
                .map(|rating| PublicRatingView {
                    season_id: rating.season_id,
                    season_name: if rating.season_id == crate::profile::CURRENT_SEASON_ID {
                        crate::profile::CURRENT_SEASON_NAME.to_string()
                    } else {
                        format!("Season {}", rating.season_id)
                    },
                    queue: rating.queue,
                    mode: rating.mode,
                    games_played: rating.games_played,
                    wins: rating.wins,
                    placements_complete: rating.placements_complete,
                    score: rating.score,
                    tier: rating.tier,
                    division: rating.division,
                    peak_score: rating.peak_score,
                })
                .collect(),
        ))
    }

    pub async fn public_leaderboard(
        &self,
        queue: &str,
        mode: &str,
        limit: usize,
    ) -> Result<Vec<PublicLeaderboardEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Ok(Vec::new());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        let mut ratings = Vec::new();
        let marker = format!(":{}:{}:", crate::profile::CURRENT_SEASON_ID, queue);
        for item in table.iter()? {
            let (key, value) = item?;
            let key = key.value();
            if key.contains(&marker) && key.ends_with(&format!(":{mode}")) {
                ratings.push(serde_json::from_slice::<SeasonRating>(value.value())?);
            }
        }
        drop(table);
        drop(read_txn);
        let mut entries = Vec::new();
        for rating in ratings {
            let Ok(account) = self.load_account(&rating.account_id).await else {
                continue;
            };
            if account.kind == AccountKind::Bot
                && (queue != "Ranked1v1" || account.profile.ranked_bot.is_none())
            {
                continue;
            }
            entries.push((rating, account.id, account.display_name, account.kind == AccountKind::Bot));
        }
        entries.sort_by_key(|left| std::cmp::Reverse(left.0.score));
        Ok(entries
            .into_iter()
            .take(limit)
            .enumerate()
            .map(
                |(index, (rating, account_id, display_name, is_bot))| PublicLeaderboardEntry {
                    rank: index as u32 + 1,
                    account_id: account_id.clone(),
                    handle: public_handle(&display_name, &account_id),
                    is_bot,
                    queue: rating.queue,
                    mode: rating.mode,
                    score: rating.score,
                    tier: rating.tier,
                    division: rating.division,
                    games_played: rating.games_played,
                    wins: rating.wins,
                },
            )
            .collect())
    }

    /// Cross-platform victories board sourced from the canonical account
    /// aggregate. It deliberately excludes bots and never trusts a client
    /// leaderboard submission.
    pub async fn public_victory_leaderboard(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<
        Vec<crate::profile::PublicVictoryLeaderboardEntry>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let Some(db) = &self.redb_db else {
            return Ok(Vec::new());
        };
        let offset = offset.min(100_000);
        // The public handler asks for one lookahead row to avoid advertising
        // an empty final page when the result count is an exact multiple.
        let limit = limit.min(101);
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(VERIFIED_VICTORY_BOARD_TABLE)?;
        let mut entries = Vec::with_capacity(limit);
        for (index, item) in table.iter()?.skip(offset).take(limit).enumerate() {
            let (_, value) = item?;
            let mut entry: crate::profile::PublicVictoryLeaderboardEntry =
                serde_json::from_slice(value.value())?;
            entry.rank = (offset + index + 1) as u32;
            entries.push(entry);
        }
        Ok(entries)
    }

    async fn ensure_starting_leader(
        &self,
        account: PlayerAccount,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if account.profile.preferred_leader.is_some() {
            return Ok(account);
        }
        let assigned =
            crate::commerce::leader_wire_id(crate::commerce::assigned_leader_for_account(
                &account.id,
                crate::commerce::current_rotation_period(),
            ))
            .to_string();
        let updated = self
            .update_account_atomic(&account.id, &[], |account| {
                if account.profile.preferred_leader.is_none() {
                    account.profile.preferred_leader = Some(assigned.clone());
                    account.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            })
            .await?;
        Ok(updated)
    }

    async fn ensure_human_onboarding(
        &self,
        account: PlayerAccount,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.ensure_starting_leader(account).await?;
        let account = self.ensure_welcome_grant(account).await?;
        self.ensure_pending_tutorial_guide(account).await
    }

    async fn ensure_pending_tutorial_guide(
        &self,
        account: PlayerAccount,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let saved = account.profile.menu_guides.get("welcome");
        let normalized = saved.map(|guide| {
            crate::profile::normalize_welcome_guide_progress(guide, account.profile.matches_played)
        });
        let enroll = saved.is_none()
            && account.profile.matches_played == 0
            && account
                .profile
                .reward_receipts
                .get("tutorial")
                .is_some_and(|receipt| receipt.presented_at.is_none());
        if saved == normalized.as_ref() && !enroll {
            return Ok(account);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let account = self
            .update_account_atomic(&account.id, &["tutorial"], |account| {
                if let Some(guide) = account.profile.menu_guides.get_mut("welcome") {
                    *guide = crate::profile::normalize_welcome_guide_progress(
                        guide,
                        account.profile.matches_played,
                    );
                    account.updated_at = now;
                } else if account.profile.matches_played == 0
                    && account
                        .profile
                        .reward_receipts
                        .get("tutorial")
                        .is_some_and(|receipt| receipt.presented_at.is_none())
                {
                    account
                        .profile
                        .menu_guides
                        .insert("welcome".to_string(), MenuGuideProgress::default());
                    account.updated_at = now;
                }
            })
            .await?;
        Ok(account)
    }

    pub async fn save_menu_guide_progress(
        &self,
        account_id: &str,
        guide_id: &str,
        progress: MenuGuideProgress,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        crate::profile::validate_menu_guide_progress(guide_id, &progress)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut bot_account = false;
        let mut guide_not_enrolled = false;
        let account = self
            .update_account_atomic(account_id, &[], |account| {
                if account.kind != AccountKind::Human {
                    bot_account = true;
                    return;
                }
                let saved = account.profile.menu_guides.get(guide_id);
                if saved.is_none() {
                    guide_not_enrolled = true;
                    return;
                }
                let merged = crate::profile::merge_menu_guide_progress(saved, &progress);
                let merged = if guide_id == "welcome" {
                    crate::profile::normalize_welcome_guide_progress(
                        &merged,
                        account.profile.matches_played,
                    )
                } else {
                    merged
                };
                account
                    .profile
                    .menu_guides
                    .insert(guide_id.to_string(), merged);
                account.updated_at = now;
            })
            .await?;
        if bot_account {
            return Err("menu guides are only available for human accounts".into());
        }
        if guide_not_enrolled {
            return Err("menu guide is not enrolled".into());
        }
        Ok(account)
    }
    async fn ensure_welcome_grant(
        &self,
        account: PlayerAccount,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if account.kind != AccountKind::Human {
            return Ok(account);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut granted = false;
        let mut grant_error = None;
        let updated = self
            .update_account_atomic(
                &account.id,
                &[crate::commerce::WELCOME_GRANT_ID],
                |account| {
                    granted = false;
                    grant_error = None;
                    match Self::apply_welcome_grant(account, now) {
                        Ok(applied) => granted = applied,
                        Err(error) => grant_error = Some(error),
                    }
                },
            )
            .await?;
        if let Some(error) = grant_error {
            return Err(error.into());
        }
        Ok(updated)
    }

    fn apply_welcome_grant(account: &mut PlayerAccount, now: u64) -> Result<bool, &'static str> {
        if account.kind != AccountKind::Human
            || account
                .profile
                .purchase_history
                .contains_key(crate::commerce::WELCOME_GRANT_ID)
        {
            return Ok(false);
        }
        account.profile.gems = account
            .profile
            .gems
            .checked_add(crate::commerce::WELCOME_GEMS)
            .ok_or("gem balance overflow")?;
        account.profile.purchase_history.insert(
            crate::commerce::WELCOME_GRANT_ID.to_string(),
            PurchaseRecord {
                id: crate::commerce::WELCOME_GRANT_ID.to_string(),
                provider: "system".to_string(),
                environment: "starter".to_string(),
                product_id: "sow_welcome_gems_475".to_string(),
                transaction_id: None,
                status: "granted".to_string(),
                acquired_at: now,
                updated_at: now,
            },
        );
        account.updated_at = now;
        Ok(true)
    }

    /// Complete the one-time starter grant for accounts that existed before
    /// per-account onboarding became the normal path. A saved cursor resumes
    /// safely after a restart; each grant is independently deduplicated.
    pub async fn migrate_welcome_gems_to_existing_humans(
        &self,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        const COMPLETE: &str = "migration:welcome-gems-v1";
        const CURSOR: &str = "migration:welcome-gems-cursor";
        const BATCH: usize = 256;
        if self.migration_value_sync(COMPLETE)?.is_some() {
            return Ok(0);
        }
        let mut after = self.migration_value_sync(CURSOR)?;
        let mut total_migrated = 0_u64;
        loop {
            let batch = self.account_batch_sync(after.as_deref(), BATCH)?;
            if batch.is_empty() {
                self.set_migration_value_sync(COMPLETE, b"1")?;
                break;
            }
            let mut migrated = 0_u64;
            for account in &batch {
                if account.kind == AccountKind::Human {
                    let updated = self.ensure_welcome_grant(account.clone()).await?;
                    if updated
                        .profile
                        .purchase_history
                        .contains_key(crate::commerce::WELCOME_GRANT_ID)
                    {
                        migrated = migrated.saturating_add(1);
                    }
                }
            }
            after = batch.last().map(|account| account.id.clone());
            if let Some(cursor) = after.as_deref() {
                self.set_migration_value_sync(CURSOR, cursor.as_bytes())?;
            }
            total_migrated = total_migrated.saturating_add(migrated);
        }
        Ok(total_migrated)
    }

    /// Permanently erase the Redb account and its account-owned rows, then
    /// remove Valkey moderation and analytics memberships. The authenticated
    /// deletion handlers also remove this account's legacy JSONL event lines.
    pub async fn delete_account(
        &self,
        account_id: &str,
    ) -> Result<DeleteAccountReport, Box<dyn std::error::Error + Send + Sync>> {
        let account_id = account_id.trim();
        if !is_valid_account_id(account_id) {
            return Err("invalid account_id".into());
        }
        let mut con = self.get_connection().await?;
        let (found, redb_rows_removed) = self.delete_account_sync(account_id)?;
        // Moderation blocks and analytics remain Valkey-owned; account
        // balances, identities, and receipts are removed atomically in Redb.
        let keys_removed: u64 = con.del(format!("sow:blocks:{account_id}")).await?;

        // Scrub dated analytics memberships across every analytics key family.
        let mut analytics_sets_scrubbed = 0_u64;
        for pattern in [
            "sow:analytics:event_users:*",
            "sow:analytics:activated:*",
            "sow:analytics:cohort:*",
            "sow:analytics:active:*",
            "sow:active:*",
        ] {
            let mut cursor = 0_u64;
            loop {
                let scanned: Result<(u64, Vec<String>), redis::RedisError> = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(pattern)
                    .arg("COUNT")
                    .arg(256)
                    .query_async(&mut con)
                    .await;
                let (next, set_keys) = scanned?;
                for set_key in set_keys {
                    let scrubbed: u32 = con.srem(&set_key, account_id).await.unwrap_or(0);
                    analytics_sets_scrubbed += u64::from(scrubbed);
                }
                cursor = next;
                if cursor == 0 {
                    break;
                }
            }
        }

        Ok(DeleteAccountReport {
            account_id: account_id.to_string(),
            found,
            keys_removed,
            redb_rows_removed,
            analytics_sets_scrubbed,
        })
    }

    /// Preview or remove every human test account while preserving the bot
    /// pool, static catalogues, and any account that already has purchase
    /// state. The caller must run the preview first and provide the exact
    /// confirmation through the operator-only HTTP route.
    pub async fn reset_human_accounts(
        &self,
        dry_run: bool,
    ) -> Result<ResetHumanAccountsReport, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let mut human_ids = std::collections::BTreeSet::new();
        let mut purchase_markers = std::collections::BTreeSet::new();
        let accounts = self.all_accounts_sync()?;
        let mut bots_preserved = 0_u64;
        for account in &accounts {
            if account.kind == AccountKind::Human {
                human_ids.insert(account.id.clone());
                collect_purchase_markers(account, &mut purchase_markers);
            } else if account.kind == AccountKind::Bot {
                bots_preserved = bots_preserved.saturating_add(1);
            }
        }
        let mut player_rows = Vec::new();
        let mut match_rows = Vec::new();
        let mut rating_rows = Vec::new();
        let mut match_index_rows = Vec::new();
        if let Some(db) = &self.redb_db {
            let read_txn = db.begin_read()?;
            player_rows.extend(human_ids.iter().cloned());
            {
                let table = read_txn.open_table(MATCHES_TABLE)?;
                for item in table.iter()? {
                    let (key, value) = item?;
                    let Ok(record) = serde_json::from_slice::<MatchRecord>(value.value()) else {
                        continue;
                    };
                    let includes_human = record
                        .winner_account_id
                        .as_deref()
                        .is_some_and(|id| human_ids.contains(id))
                        || record
                            .participants
                            .iter()
                            .any(|participant| human_ids.contains(&participant.account_id));
                    if includes_human {
                        match_rows.push(key.value().to_string());
                    }
                }
            }
            {
                let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
                for item in table.iter()? {
                    let (key, value) = item?;
                    let account_id = serde_json::from_slice::<SeasonRating>(value.value())
                        .ok()
                        .map(|rating| rating.account_id);
                    let key_matches = human_ids
                        .iter()
                        .any(|id| key.value().starts_with(&format!("rating:{id}:")));
                    if account_id
                        .as_deref()
                        .is_some_and(|id| human_ids.contains(id))
                        || key_matches
                    {
                        rating_rows.push(key.value().to_string());
                    }
                }
            }
            {
                let table = read_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
                for item in table.iter()? {
                    let (key, value) = item?;
                    let key_matches = human_ids
                        .iter()
                        .any(|id| key.value().starts_with(&format!("player:{id}:")));
                    let value_matches = std::str::from_utf8(value.value())
                        .ok()
                        .is_some_and(|match_id| match_rows.iter().any(|id| id == match_id));
                    if key_matches || value_matches {
                        match_index_rows.push(key.value().to_string());
                    }
                }
            }
        }

        let purchase_records = purchase_markers.len() as u64;
        if purchase_records > 0 && !dry_run {
            return Err("human test data includes purchase records; reset refused".into());
        }

        let mut redis_delete_keys = std::collections::BTreeSet::new();
        for account_id in &human_ids {
            redis_delete_keys.insert(format!("sow:blocks:{account_id}"));
            redis_delete_keys.insert(format!("sow:analytics:activated:{account_id}"));
        }

        for pattern in ["sow:analytics:activated:*"] {
            let mut scan_cursor = 0_u64;
            loop {
                let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(scan_cursor)
                    .arg("MATCH")
                    .arg(pattern)
                    .arg("COUNT")
                    .arg(256)
                    .query_async(&mut con)
                    .await?;
                for key in keys {
                    let remove = if key.starts_with("sow:analytics:activated:") {
                        key.rsplit(':')
                            .next()
                            .is_some_and(|id| human_ids.contains(id))
                    } else {
                        con.get::<_, Option<String>>(&key)
                            .await?
                            .is_some_and(|id| human_ids.contains(&id))
                    };
                    if remove {
                        redis_delete_keys.insert(key);
                    }
                }
                scan_cursor = next;
                if scan_cursor == 0 {
                    break;
                }
            }
        }

        // These keys are aggregate metrics (HyperLogLog strings), not
        // account records. They cannot remove individual members, so a full
        // test-data reset removes the aggregate through the Redis API.
        let mut scan_cursor = 0_u64;
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(scan_cursor)
                .arg("MATCH")
                .arg("sow:analytics:active:*")
                .arg("COUNT")
                .arg(256)
                .query_async(&mut con)
                .await?;
            redis_delete_keys.extend(keys);
            scan_cursor = next;
            if scan_cursor == 0 {
                break;
            }
        }

        let set_patterns = [
            "sow:blocks:*",
            "sow:analytics:event_users:*",
            "sow:analytics:cohort:*",
            "sow:active:*",
        ];
        let mut redis_members_removed = 0_u64;
        for pattern in set_patterns {
            let mut scan_cursor = 0_u64;
            loop {
                let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(scan_cursor)
                    .arg("MATCH")
                    .arg(pattern)
                    .arg("COUNT")
                    .arg(256)
                    .query_async(&mut con)
                    .await?;
                for key in keys {
                    let members: Vec<String> = con.smembers(&key).await?;
                    let remove_owner_key = key
                        .strip_prefix("sow:blocks:")
                        .is_some_and(|id| human_ids.contains(id));
                    if remove_owner_key {
                        redis_delete_keys.insert(key);
                        continue;
                    }
                    redis_members_removed = redis_members_removed.saturating_add(
                        members
                            .iter()
                            .filter(|member| human_ids.contains(*member))
                            .count() as u64,
                    );
                    if !dry_run {
                        for member in members {
                            if human_ids.contains(&member) {
                                let _: u32 = con.srem(&key, &member).await?;
                            }
                        }
                    }
                }
                scan_cursor = next;
                if scan_cursor == 0 {
                    break;
                }
            }
        }

        let mut redb_rows_removed = 0_u64;
        if !dry_run {
            for key in &redis_delete_keys {
                let _: u64 = con.del(key).await?;
            }
            if let Some(db) = &self.redb_db {
                let write_txn = db.begin_write()?;
                for account_id in &player_rows {
                    let (_, removed) = account_store::delete_account_rows(&write_txn, account_id)?;
                    redb_rows_removed = redb_rows_removed.saturating_add(u64::from(removed));
                }
                {
                    let mut table = write_txn.open_table(MATCHES_TABLE)?;
                    for key in &match_rows {
                        if table.remove(key.as_str())?.is_some() {
                            redb_rows_removed = redb_rows_removed.saturating_add(1);
                        }
                    }
                }
                {
                    let mut table = write_txn.open_table(SEASON_RATINGS_TABLE)?;
                    for key in &rating_rows {
                        if table.remove(key.as_str())?.is_some() {
                            redb_rows_removed = redb_rows_removed.saturating_add(1);
                        }
                    }
                }
                {
                    let mut table = write_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
                    for key in &match_index_rows {
                        if table.remove(key.as_str())?.is_some() {
                            redb_rows_removed = redb_rows_removed.saturating_add(1);
                        }
                    }
                }
                write_txn.commit()?;
            }
        } else {
            redb_rows_removed =
                (player_rows.len() + match_rows.len() + rating_rows.len() + match_index_rows.len())
                    as u64;
        }

        Ok(ResetHumanAccountsReport {
            dry_run,
            human_accounts: human_ids.len() as u64,
            bots_preserved,
            purchase_records,
            redis_keys_removed: redis_delete_keys.len() as u64,
            redis_members_removed,
            redb_rows_removed,
            match_rows_removed: match_rows.len() as u64,
            rating_rows_removed: rating_rows.len() as u64,
        })
    }

    /// Membership check against the bot pool index — used to exclude synthetic
    /// players from analytics ingestion and human counters.
    pub async fn is_bot_account_checked(
        &self,
        account_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.is_bot_account_sync(account_id)?)
    }

    pub async fn is_bot_account(&self, account_id: &str) -> bool {
        match self.is_bot_account_checked(account_id).await {
            Ok(is_bot) => is_bot,
            Err(error) => {
                error!("Bot-pool membership lookup failed for {account_id}: {error}");
                false
            }
        }
    }

    /// Count non-bot account_ids in a match roster.
    pub async fn count_human_players(&self, player_ids: &[String]) -> usize {
        match self.count_human_accounts_sync(player_ids) {
            Ok(count) => count,
            Err(error) => {
                error!("human participant count failed: {error}");
                0
            }
        }
    }

    /// Read the immutable roster registered before the relay handoff.
    pub async fn match_participants_with_lobby(
        &self,
        match_id: &str,
        lobby_json: Option<&str>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(lobby_json) = lobby_json {
            validate_lobby_match_id(match_id, lobby_json)?;
        }
        let players = self
            .match_start_record(match_id)?
            .map(|record| record.player_ids)
            .unwrap_or_default();
        if players.is_empty() {
            return Err("trusted match roster unavailable; keeping settlement retryable".into());
        }
        Ok(players)
    }

    /// Freeze one human player's receipt at the relay's committed exit turn.
    /// Only the deterministic verifier may add replay-derived rewards later.
    pub async fn note_match_participant_closure(
        &self,
        match_id: &str,
        account_id: &str,
        closure_turn: u64,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let start = self
            .match_start_record(match_id)?
            .ok_or("trusted match registration unavailable")?;
        if !start.player_ids.iter().any(|player| player == account_id) {
            return Err("account is not registered in this match".into());
        }
        let relay_player = start
            .metadata
            .get("relay_players")
            .and_then(serde_json::Value::as_array)
            .and_then(|players| {
                players.iter().find(|player| {
                    player
                        .get("database_account_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(account_id)
                        && player
                            .get("is_internal")
                            .and_then(serde_json::Value::as_bool)
                            != Some(true)
                })
            })
            .ok_or("account is not a registered human participant")?;
        let leader = relay_player
            .get("leader")
            .and_then(serde_json::Value::as_str)
            .and_then(crate::rewards::canonical_leader_name);
        let existing = self.account_with_events(account_id, &[match_id]).await?;
        let closure_was_saved = existing
            .profile
            .reward_receipts
            .get(match_id)
            .is_some_and(|receipt| receipt.personal_closure_turn == Some(closure_turn))
            && self.load_match_record(match_id)?.is_some_and(|record| {
                record.participants.iter().any(|participant| {
                    participant.account_id == account_id
                        && participant.personal_closure_turn == Some(closure_turn)
                })
            });
        if closure_was_saved {
            return Ok(existing);
        }
        let account = self
            .record_match_participation(account_id, leader.clone(), match_id)
            .await?;
        if account.kind != AccountKind::Human {
            return Err("only human participants can close a personal receipt".into());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut conflicting_turn = false;
        let account = self
            .update_account_atomic(account_id, &[match_id], |account| {
                let Some(receipt) = account.profile.reward_receipts.get_mut(match_id) else {
                    conflicting_turn = true;
                    return;
                };
                if receipt
                    .personal_closure_turn
                    .is_some_and(|saved| saved != closure_turn)
                {
                    conflicting_turn = true;
                    return;
                }
                if receipt.personal_closure_turn != Some(closure_turn) {
                    receipt.personal_closure_turn = Some(closure_turn);
                    account.updated_at = now;
                }
            })
            .await?;
        if conflicting_turn {
            return Err("personal receipt closure conflicts with its saved state".into());
        }

        let config = start.metadata.get("config");
        let started_at = start
            .metadata
            .get("started_at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(now);
        let participant = crate::profile::MatchParticipantRecord {
            account_id: account.id.clone(),
            display_name: account.display_name.clone(),
            is_bot: false,
            leader,
            team: relay_player
                .get("team")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            personal_closure_turn: Some(closure_turn),
            ..Default::default()
        };
        let mut record = self
            .load_match_record(match_id)?
            .unwrap_or_else(|| MatchRecord {
                schema_version: 1,
                match_id: match_id.to_string(),
                season_id: crate::profile::CURRENT_SEASON_ID,
                queue: start
                    .metadata
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("Matchmaking")
                    .to_string(),
                mode: config
                    .and_then(|value| value.get("game_mode"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("FFA")
                    .to_string(),
                map_name: config
                    .and_then(|value| value.get("map_name"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("world")
                    .to_string(),
                started_at,
                completed_at: now,
                duration_seconds: now.saturating_sub(started_at).min(u32::MAX as u64) as u32,
                winner_account_id: None,
                winning_team: None,
                verified: false,
                provisional: true,
                rating_eligible: false,
                ranked_bot_iq: config
                    .and_then(|value| value.get("ranked_bot_iq"))
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok()),
                participants: vec![participant.clone()],
            });
        if !record
            .participants
            .iter()
            .any(|participant| participant.account_id == account_id)
        {
            record.participants.push(participant);
        } else if let Some(saved) = record
            .participants
            .iter_mut()
            .find(|participant| participant.account_id == account_id)
        {
            saved.personal_closure_turn = Some(closure_turn);
        }
        self.save_match_record_allow_closed_update(&record, account_id)?;
        Ok(account)
    }

    /// Record a terminal rejection for one closed participant. This is used
    /// when the trusted relay cannot provide a replay prefix or the verifier
    /// rejects that prefix; it never changes another participant's receipt.
    pub async fn reject_match_participant_result(
        &self,
        match_id: &str,
        account_id: &str,
        closure_turn: u64,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut conflicting_state = false;
        let account = self
            .update_account_atomic(account_id, &[match_id], |account| {
                let Some(receipt) = account.profile.reward_receipts.get_mut(match_id) else {
                    conflicting_state = true;
                    return;
                };
                if receipt.personal_closure_turn != Some(closure_turn) {
                    conflicting_state = true;
                    return;
                }
                if receipt.verification_status == Some(ReplayVerificationStatus::Verified) {
                    return;
                }
                if receipt.verification_status != Some(ReplayVerificationStatus::Rejected) {
                    receipt.verification_status = Some(ReplayVerificationStatus::Rejected);
                    account.updated_at = now;
                }
            })
            .await?;
        if conflicting_state {
            return Err("personal receipt rejection conflicts with its saved state".into());
        }
        Ok(account)
    }

    pub async fn apply_verified_match_participant_results(
        &self,
        outcomes: &[crate::profile::VerifiedParticipantClosure],
        notification: Option<crate::profile::MatchResultControl>,
    ) -> Result<
        Vec<Option<(PlayGamesMatchOutcome, PlayerAccount)>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        if outcomes.is_empty() {
            return Ok(Vec::new());
        }
        let match_id = outcomes[0].result.match_id.clone();
        if notification
            .as_ref()
            .is_some_and(|command| command.match_id != match_id)
        {
            return Err("invalid verified result: notification belongs to another match".into());
        }
        let start = self
            .match_start_record(&match_id)?
            .ok_or("trusted match registration unavailable")?;
        let mut account_ids = Vec::with_capacity(outcomes.len());
        let mut unique_accounts = std::collections::BTreeSet::new();
        for closure in outcomes {
            if closure.result.match_id != match_id {
                return Err(
                    "invalid verified result: closure batch contains multiple matches".into(),
                );
            }
            closure
                .result
                .personal_closure_turn
                .ok_or("invalid verified result: personal closure turn missing")?;
            validate_verified_match_result(&start, &closure.result)
                .map_err(Box::<dyn std::error::Error + Send + Sync>::from)?;
            if !closure
                .result
                .participants
                .iter()
                .any(|participant| participant.account_id == closure.account_id)
            {
                return Err("invalid verified result: closed participant is missing".into());
            }
            if !unique_accounts.insert(closure.account_id.as_str()) {
                return Err("invalid verified result: duplicate closed participant".into());
            }
            if !start.player_ids.iter().any(|id| id == &closure.account_id) {
                return Err("account is not registered in this match".into());
            }
            account_ids.push(closure.account_id.clone());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let bonus_id = format!("verified_bonus:{match_id}");
        let closures = outcomes.to_vec();
        let start_for_write = start.clone();
        let notification_for_write = notification;
        let (applied, accounts) = self
            .update_accounts_atomic(
                account_ids,
                vec![match_id.clone(), bonus_id],
                move |txn, accounts| {
                    let mut record = load_match_record_in_transaction(txn, &match_id)?
                        .unwrap_or_else(|| provisional_match_record(&start_for_write, now));
                    let mut applied = Vec::with_capacity(closures.len());
                    for (closure, account) in closures.iter().zip(accounts.iter_mut()) {
                        if account.kind != AccountKind::Human {
                            return Err("invalid verified result: closed participant is not human".into());
                        }
                        let participant = closure
                            .result
                            .participants
                            .iter()
                            .find(|participant| participant.account_id == closure.account_id)
                            .ok_or("invalid verified result: closed participant is missing")?;
                        if record.verified
                            && !match_record_has_participant_result(
                                &record,
                                participant,
                                closure.result.personal_closure_turn,
                            )
                        {
                            return Err("invalid verified result: saved participant result conflicts".into());
                        }
                        apply_match_participation_to_account(
                            account,
                            participant.leader.as_deref(),
                            &match_id,
                            now,
                        );
                        let turn = closure.result.personal_closure_turn.unwrap_or_default();
                        let receipt = account
                            .profile
                            .reward_receipts
                            .get_mut(&match_id)
                            .ok_or("invalid verified result: participation receipt is missing")?;
                        if receipt
                            .personal_closure_turn
                            .is_some_and(|saved| saved != turn)
                        {
                            return Err("invalid verified result: personal closure conflicts with saved state".into());
                        }
                        receipt.personal_closure_turn = Some(turn);
                        let did_apply = apply_verified_participant_reward_to_account(
                            account,
                            &closure.result,
                            participant,
                            Some(turn),
                            now,
                        )
                        .map_err(Box::<dyn std::error::Error + Send + Sync>::from)?;
                        update_match_record_participant(
                            &mut record,
                            account,
                            participant,
                            Some(turn),
                        );
                        if record.provisional {
                            record.duration_seconds =
                                record.duration_seconds.max(closure.result.duration_seconds);
                        }
                        applied.push(did_apply);
                    }
                    write_match_record_in_transaction(txn, &record)?;
                    if let Some(command) = notification_for_write {
                        queue_match_work_in_transaction(
                            txn,
                            crate::profile::MatchWork::Notify { command },
                        )?;
                    }
                    Ok(applied)
                },
            )
            .await?;

        Ok(outcomes
            .iter()
            .zip(accounts)
            .zip(applied)
            .map(|((closure, account), did_apply)| {
                if !did_apply {
                    return None;
                }
                let participant = closure
                    .result
                    .participants
                    .iter()
                    .find(|participant| participant.account_id == closure.account_id)?;
                Some((
                    PlayGamesMatchOutcome {
                        account_id: account.id.clone(),
                        won: participant.won,
                        wins: account.profile.verified_wins,
                        sync_revision: account.profile.playgames_sync_revision,
                        unlocked_achievements: account
                            .profile
                            .unlocked_achievements
                            .iter()
                            .cloned()
                            .collect(),
                    },
                    account,
                ))
            })
            .collect())
    }

    /// Load an existing provider account without creating a new one. This is
    /// scoped to an environment. New human accounts must arrive through the
    /// canonical WOU-ID account_id bridge.
    pub async fn get_existing_with_environment(
        &self,
        provider: String,
        environment: String,
        external_id: String,
    ) -> Result<Option<PlayerAccount>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(account_id) = self
            .identity_account(&environment, &provider, &external_id)
            .await?
        else {
            return Ok(None);
        };
        let account = self.load_account(&account_id).await?;
        Ok(Some(self.ensure_human_onboarding(account).await?))
    }

    /// Read an existing provider mapping without creating a provider-owned
    /// account. Production and debug are the only identity environments used
    /// by the shipped clients.
    pub async fn get_existing_identity(
        &self,
        provider: String,
        external_id: String,
    ) -> Result<Option<PlayerAccount>, Box<dyn std::error::Error + Send + Sync>> {
        for environment in ["production", "debug"] {
            if let Some(account) = self
                .get_existing_with_environment(
                    provider.clone(),
                    environment.to_string(),
                    external_id.clone(),
                )
                .await?
            {
                return Ok(Some(account));
            }
        }
        Ok(None)
    }

    /// Resolve a verified identity through the single canonical account ID.
    /// Existing SOW progress is retained; conflicting mappings fail instead of
    /// silently merging or creating a second account.
    pub async fn get_or_create_for_account_id(
        &self,
        account_id: &str,
        provider: String,
        environment: String,
        external_id: String,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let account_id = account_id.trim();
        if !is_valid_account_id(account_id) {
            return Err("invalid canonical account_id".into());
        }
        if provider.trim().is_empty()
            || environment.trim().is_empty()
            || external_id.trim().is_empty()
        {
            return Err("canonical identity fields are incomplete".into());
        }
        let mapped = self
            .identity_account(&environment, &provider, &external_id)
            .await?;
        if mapped.as_deref().is_some_and(|id| id != account_id) {
            return Err("provider identity is already linked to another account_id".into());
        }
        let account = if mapped.is_some() {
            self.load_account(account_id).await?
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.insert_account(PlayerAccount {
                id: account_id.to_string(),
                display_name: generated_display_name(),
                profile: PlayerProfile::for_account(account_id),
                linked_identities: vec![LinkedIdentity {
                    provider: provider.clone(),
                    environment: environment.clone(),
                    external_id: external_id.clone(),
                }],
                kind: if provider == "bot" {
                    AccountKind::Bot
                } else {
                    AccountKind::Human
                },
                auth_secret_hash: None,
                created_at: now,
                updated_at: now,
            })
            .await?
            .0
        };
        if account.kind == AccountKind::Bot && provider != "bot" {
            return Err("canonical account_id belongs to a bot".into());
        }
        let account = self
            .bind_provider_identity(account, &provider, &environment, &external_id)
            .await?;
        self.ensure_human_onboarding(account).await
    }

    async fn bind_provider_identity(
        &self,
        account: PlayerAccount,
        provider: &str,
        environment: &str,
        external_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if account.linked_identities.iter().any(|identity| {
            identity.provider == provider
                && identity.external_id == external_id
                && identity.environment == environment
        }) {
            return Ok(account);
        }
        let updated = self
            .update_account_atomic(&account.id, &[], |account| {
                if !account.linked_identities.iter().any(|identity| {
                    identity.provider == provider
                        && identity.external_id == external_id
                        && identity.environment == environment
                }) {
                    account.linked_identities.push(LinkedIdentity {
                        provider: provider.to_string(),
                        external_id: external_id.to_string(),
                        environment: environment.to_string(),
                    });
                }
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            })
            .await?;
        Ok(updated)
    }

    /// Get or create the technical bot account for a verified bot identity.
    /// Human identities never use this path; they require a canonical WOU-ID
    /// account_id through `get_or_create_for_account_id`.
    pub async fn get_or_create_bot_account(
        &self,
        environment: String,
        external_id: String,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(id) = self
            .identity_account(&environment, "bot", &external_id)
            .await?
        {
            let account = self.load_account(&id).await?;
            if account.kind != AccountKind::Bot {
                return Err("bot identity belongs to a human account".into());
            }
            return self.ensure_starting_leader(account).await;
        }
        loop {
            let id = format!("{:032x}", rand::random::<u128>());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let inserted = self
                .insert_account(PlayerAccount {
                    profile: PlayerProfile::for_account(&id),
                    id,
                    display_name: generated_display_name(),
                    linked_identities: vec![LinkedIdentity {
                        provider: "bot".into(),
                        environment: environment.clone(),
                        external_id: external_id.clone(),
                    }],
                    kind: AccountKind::Bot,
                    auth_secret_hash: None,
                    created_at: now,
                    updated_at: now,
                })
                .await;
            match inserted {
                Ok((account, true)) => return self.ensure_starting_leader(account).await,
                Ok((_, false)) => continue,
                Err(error) => {
                    // Another seed request may have claimed this identity first.
                    if let Some(id) = self
                        .identity_account(&environment, "bot", &external_id)
                        .await?
                    {
                        let account = self.load_account(&id).await?;
                        if account.kind != AccountKind::Bot {
                            return Err("bot identity belongs to a human account".into());
                        }
                        return self.ensure_starting_leader(account).await;
                    }
                    return Err(error);
                }
            }
        }
    }

    /// Load or create an anonymous account using the canonical account ID.
    /// New anonymous accounts do not create a provider identity index.
    pub async fn get_or_create_anonymous(
        &self,
        account_id: Option<&str>,
        requested_display_name: Option<&str>,
        account_auth_secret: Option<&str>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(account_id) = account_id.map(str::trim).filter(|id| !id.is_empty()) {
            if !is_valid_account_id(account_id) {
                return Err("account_id is invalid".into());
            }
            let account = self.load_account(account_id).await?;
            if account.kind != AccountKind::Human || !account.linked_identities.is_empty() {
                return Err("account is not anonymous".into());
            }
            if let Some(stored_hash) = account.auth_secret_hash.as_deref()
                && account_auth_secret.is_none_or(|secret| {
                    blake3::hash(secret.as_bytes()).to_hex().to_string() != stored_hash
                })
            {
                return Err("invalid secret".into());
            }
            let account = self.ensure_human_onboarding(account).await?;
            if account.display_name.trim().is_empty() {
                let display_name = requested_display_name
                    .map(normalize_display_name)
                    .transpose()
                    .map_err(|error| error.to_string())?
                    .unwrap_or_else(generated_display_name);
                let account = self
                    .update_account_atomic(account_id, &[], |account| {
                        if account.display_name.trim().is_empty() {
                            account.display_name = display_name.clone();
                            account.updated_at = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                        }
                    })
                    .await?;
                return Ok(account);
            }
            return Ok(account);
        }

        let display_name = requested_display_name
            .map(normalize_display_name)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(generated_display_name);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        loop {
            let random_id = format!("{:032x}", rand::random::<u128>());
            let account = PlayerAccount {
                id: random_id.clone(),
                display_name: display_name.clone(),
                profile: PlayerProfile::for_account(&random_id),
                linked_identities: Vec::new(),
                kind: AccountKind::Human,
                auth_secret_hash: None,
                created_at: now,
                updated_at: now,
            };
            let (account, created) = self.insert_account(account).await?;
            if created {
                let account = self.ensure_welcome_grant(account).await?;
                info!("Created anonymous account {}", account.id);
                return Ok(account);
            }
        }
    }

    /// Mint an anonymous account secret if the account has none yet. Persists
    /// only the BLAKE3 hash; returns the plaintext exactly once (the caller
    /// hands it to the client, which stores it as its ownership proof).
    /// Idempotent: an account that already has a secret returns `None`.
    pub async fn ensure_auth_secret(
        &self,
        account_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let plain = format!(
            "{:032x}{:032x}",
            rand::random::<u128>(),
            rand::random::<u128>()
        );
        let hash = blake3::hash(plain.as_bytes()).to_hex().to_string();
        let mut revealed: Option<String> = None;
        self.update_account_atomic(account_id, &[], |account| {
            if account.auth_secret_hash.is_none() {
                account.auth_secret_hash = Some(hash.clone());
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                revealed = Some(plain.clone());
            }
        })
        .await?;
        Ok(revealed)
    }

    /// Verify an anonymous ownership proof: the presented secret must hash to
    /// the stored `auth_secret_hash`. Returns the account id on success.
    pub async fn verify_anonymous_secret(
        &self,
        account_id: &str,
        secret: &str,
    ) -> Result<String, String> {
        if !is_valid_account_id(account_id) {
            return Err("invalid account_id".to_string());
        }
        let account = self
            .load_account(account_id)
            .await
            .map_err(|_| "account not found".to_string())?;
        match account.auth_secret_hash.as_deref() {
            Some(stored) if blake3::hash(secret.as_bytes()).to_hex().to_string() == stored => {
                Ok(account_id.to_string())
            }
            Some(_) => Err("invalid secret".to_string()),
            None => Err("account has no secret; fetch /profile/anonymous to mint one".to_string()),
        }
    }

    pub async fn selected_skin_style_for_account(
        &self,
        account_id: &str,
    ) -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.load_account(account_id).await?;
        Ok(crate::commerce::skin_style_for_profile(
            &account.profile.owned_skins,
            account.profile.selected_skin.as_deref(),
        ))
    }

    pub async fn resolve_leader_for_account(
        &self,
        account_id: &str,
        requested_leader: Option<&str>,
    ) -> Result<crate::commerce::LeaderResolution, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.load_account(account_id).await?;
        Ok(crate::commerce::resolve_leader(
            requested_leader,
            account_id,
            &account.profile.owned_leaders,
            crate::commerce::current_rotation_period(),
        ))
    }

    pub async fn draw_leader_for_account(
        &self,
        account_id: &str,
    ) -> Result<crate::leaders::Leader, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.load_account(account_id).await?;
        crate::commerce::draw_available_leader(
            &account.profile.owned_leaders,
            crate::commerce::current_rotation_period(),
        )
        .ok_or_else(|| "no available leaders".into())
    }

    pub async fn update_leader_selection(
        &self,
        account_id: &str,
        preferred_leader: Option<&str>,
        prefer_random_leader: bool,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let requested = if prefer_random_leader {
            None
        } else {
            Some(
                crate::commerce::leader_from_id(
                    preferred_leader.ok_or("leader selection is required")?,
                )
                .ok_or("unknown leader")?,
            )
        };
        let period = crate::commerce::current_rotation_period();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let account = self
            .update_account_atomic(account_id, &[], |account| {
                if let Some(leader) = requested {
                    account.profile.preferred_leader =
                        Some(crate::commerce::leader_wire_id(leader).to_string());
                    account.profile.prefer_random_leader = false;
                } else {
                    if account.profile.preferred_leader.is_none() {
                        let assigned =
                            crate::commerce::assigned_leader_for_account(account_id, period);
                        account.profile.preferred_leader =
                            Some(crate::commerce::leader_wire_id(assigned).to_string());
                    }
                    account.profile.prefer_random_leader = true;
                }
                account.updated_at = now;
            })
            .await?;
        Ok(account)
    }

    pub async fn complete_campaign_episode(
        &self,
        account_id: &str,
        episode_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let episode = crate::campaign::episode(episode_id).ok_or("unknown campaign episode")?;
        if !crate::campaign::episode_is_published(episode.id) {
            return Err("campaign episode is not published".into());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut failure = None;
        let account = self
            .update_account_atomic(
                account_id,
                &[&format!("campaign:{}", episode.id)],
                |account| {
                    if account.kind != AccountKind::Human {
                        failure = Some("campaign progress requires a human account");
                        return;
                    }
                    if account.profile.completed_episodes.contains(episode.id) {
                        return;
                    }
                    if !crate::campaign::prerequisite_satisfied(
                        episode.id,
                        &account.profile.completed_episodes,
                    ) {
                        failure = Some("campaign episodes must be completed in order");
                        return;
                    }

                    account
                        .profile
                        .completed_episodes
                        .insert(episode.id.to_string());
                    let reward = crate::rewards::calculate(crate::rewards::RewardInput {
                        tutorial: true,
                        ..Default::default()
                    });
                    let receipt_id = format!("campaign:{}", episode.id);
                    let unlocked_leader = crate::campaign::earned_leader(
                        &account.profile.completed_episodes,
                    )
                    .filter(|leader| {
                        !account
                            .profile
                            .owned_leaders
                            .contains(crate::commerce::leader_id(*leader))
                    });
                    if let Some(leader) = unlocked_leader {
                        account
                            .profile
                            .owned_leaders
                            .insert(crate::commerce::leader_id(leader).to_string());
                    }
                    account.profile.apply_reward_receipt(
                        &receipt_id,
                        &receipt_id,
                        episode.advisor().name(),
                        reward,
                        now,
                    );
                    if let Some(leader) = unlocked_leader
                        && let Some(receipt) = account.profile.reward_receipts.get_mut(&receipt_id)
                    {
                        receipt
                            .unlocked_leaders
                            .push(crate::commerce::leader_id(leader).to_string());
                    }
                    account.updated_at = now;
                },
            )
            .await?;
        if let Some(error) = failure {
            return Err(error.into());
        }
        Ok(account)
    }

    /// Resolve a canonical account ID. Display handles and non-canonical
    /// identity namespaces are not accepted for authenticated operations.
    pub async fn account_id_from_reference(
        &self,
        reference: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let reference = reference.trim();
        if !is_valid_account_id(reference) {
            return Ok(None);
        }
        Ok(self
            .account_exists_sync(reference)?
            .then(|| reference.to_string()))
    }

    /// Deliver one RevenueCat product exactly once. The event id is the
    /// fallback id; when a store transaction id is available it is the
    /// stronger deduplication key because provider retries can create more
    /// than one webhook event for the same purchase.
    pub async fn grant_revenuecat_product(
        &self,
        event_id: &str,
        purchase_user_id: &str,
        product_id: &str,
        transaction_id: Option<&str>,
        environment: &str,
    ) -> Result<(PlayerAccount, bool), Box<dyn std::error::Error + Send + Sync>> {
        let account_id = self
            .account_id_from_reference(purchase_user_id)
            .await?
            .ok_or("app_user_id must be a known account_id")?;
        let event_id = event_id.trim();
        if event_id.is_empty() || event_id.len() > 256 {
            return Err("RevenueCat event id is invalid".into());
        }
        let product_id = product_id.trim();
        if !crate::commerce::is_store_product(product_id) {
            return Err("unknown RevenueCat product".into());
        }
        let delivery_key = transaction_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("transaction:{value}"))
            .unwrap_or_else(|| format!("event:{event_id}"));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut granted = false;
        let account = self
            .update_account_atomic(&account_id, &[event_id, &delivery_key], |account| {
                if account
                    .profile
                    .processed_revenuecat_events
                    .contains(&delivery_key)
                    || account
                        .profile
                        .processed_revenuecat_events
                        .contains(event_id)
                {
                    return;
                }
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(delivery_key.clone());
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(event_id.to_string());
                if let Some(gems) = crate::commerce::gem_amount_for_product(product_id) {
                    account.profile.gems = account.profile.gems.saturating_add(gems);
                }
                account.profile.purchase_history.insert(
                    delivery_key.clone(),
                    PurchaseRecord {
                        id: delivery_key.clone(),
                        provider: "revenuecat".to_string(),
                        environment: environment.to_string(),
                        product_id: product_id.to_string(),
                        transaction_id: transaction_id
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(str::to_string),
                        status: "granted".to_string(),
                        acquired_at: now,
                        updated_at: now,
                    },
                );
                account.updated_at = now;
                granted = true;
            })
            .await?;
        Ok((account, granted))
    }

    pub async fn purchase_history_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<PurchaseRecord>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.purchase_history_sync(account_id)?)
    }

    pub async fn purchase_record_for_account(
        &self,
        account_id: &str,
        purchase_id: &str,
    ) -> Result<Option<PurchaseRecord>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.purchase_record_sync(account_id, purchase_id)?)
    }

    pub async fn revoke_revenuecat_product(
        &self,
        event_id: &str,
        purchase_user_id: &str,
        product_id: &str,
        transaction_id: Option<&str>,
        environment: &str,
    ) -> Result<(PlayerAccount, bool), Box<dyn std::error::Error + Send + Sync>> {
        let account_id = self
            .account_id_from_reference(purchase_user_id)
            .await?
            .ok_or("app_user_id must be a known account_id")?;
        let event_id = event_id.trim();
        if event_id.is_empty() || event_id.len() > 256 {
            return Err("RevenueCat event id is invalid".into());
        }
        let product_id = product_id.trim();
        if !crate::commerce::is_store_product(product_id) {
            return Err("unknown RevenueCat product".into());
        }
        let delivery_key = transaction_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("transaction:{value}"))
            .unwrap_or_else(|| format!("event:{event_id}"));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut revoked = false;
        let account = self
            .update_account_atomic(&account_id, &[event_id, &delivery_key], |account| {
                if account
                    .profile
                    .processed_revenuecat_events
                    .contains(&delivery_key)
                    || account
                        .profile
                        .processed_revenuecat_events
                        .contains(event_id)
                {
                    return;
                }
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(delivery_key.clone());
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(event_id.to_string());
                if let Some(gems) = crate::commerce::gem_amount_for_product(product_id) {
                    account.profile.gems = account.profile.gems.saturating_sub(gems);
                }
                account.profile.purchase_history.insert(
                    delivery_key.clone(),
                    PurchaseRecord {
                        id: delivery_key.clone(),
                        provider: "revenuecat".to_string(),
                        environment: environment.to_string(),
                        product_id: product_id.to_string(),
                        transaction_id: transaction_id
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(str::to_string),
                        status: "revoked".to_string(),
                        acquired_at: account
                            .profile
                            .purchase_history
                            .get(&delivery_key)
                            .map(|record| record.acquired_at)
                            .unwrap_or(now),
                        updated_at: now,
                    },
                );
                account.updated_at = now;
                revoked = true;
            })
            .await?;
        Ok((account, revoked))
    }

    pub async fn unlock_leader(
        &self,
        account_id: &str,
        requested_leader: &str,
        currency: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let leader = crate::commerce::leader_from_id(requested_leader).ok_or("unknown leader")?;
        if leader.always_free() {
            return Err("leader is permanently free".into());
        }
        let leader_id = crate::commerce::leader_id(leader).to_string();
        let (cost, use_gems) = match currency {
            "gems" => (crate::commerce::LEADER_UNLOCK_COST_GEMS, true),
            "" | "crowns" => (crate::commerce::LEADER_UNLOCK_COST_CROWNS, false),
            "laurels" => return Err("laurels are achievement points, not spendable".into()),
            _ => return Err("invalid leader unlock currency".into()),
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let purchase_id = format!("in_game:leader:{leader_id}");
        let mut failure = None;
        let account = self
            .update_account_atomic(account_id, &[&purchase_id], |account| {
                if account.profile.owned_leaders.contains(&leader_id) {
                    failure = Some("leader already owned");
                } else if (if use_gems {
                    account.profile.gems
                } else {
                    account.profile.crowns
                }) < cost
                {
                    failure = Some(if use_gems {
                        "insufficient gems"
                    } else {
                        "insufficient crowns"
                    });
                } else {
                    if use_gems {
                        account.profile.gems -= cost;
                    } else {
                        account.profile.crowns -= cost;
                    }
                    account.profile.owned_leaders.insert(leader_id.clone());
                    account.profile.purchase_history.insert(
                        purchase_id.clone(),
                        PurchaseRecord {
                            id: purchase_id.clone(),
                            provider: "in_game".to_string(),
                            environment: "local".to_string(),
                            product_id: format!("sow_leader_unlock_{leader_id}"),
                            transaction_id: None,
                            status: "granted".to_string(),
                            acquired_at: now,
                            updated_at: now,
                        },
                    );
                    account.updated_at = now;
                }
            })
            .await?;
        if let Some(error) = failure {
            return Err(error.into());
        }
        Ok(account)
    }

    pub async fn unlock_skin_with_gems(
        &self,
        account_id: &str,
        requested_skin: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let skin = crate::commerce::skin_by_id(requested_skin).ok_or("unknown skin")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let purchase_id = format!("in_game:skin:{}", skin.id);
        let mut failure = None;
        let account = self
            .update_account_atomic(account_id, &[&purchase_id], |account| {
                if account.profile.owned_skins.contains(&skin.id) {
                    failure = Some("skin already owned");
                } else if account.profile.gems < skin.cost_gems {
                    failure = Some("insufficient gems");
                } else {
                    account.profile.gems -= skin.cost_gems;
                    account.profile.owned_skins.insert(skin.id.clone());
                    account.profile.purchase_history.insert(
                        purchase_id.clone(),
                        PurchaseRecord {
                            id: purchase_id.clone(),
                            provider: "in_game".to_string(),
                            environment: "local".to_string(),
                            product_id: format!("sow_skin_unlock_{}", skin.id),
                            transaction_id: None,
                            status: "granted".to_string(),
                            acquired_at: now,
                            updated_at: now,
                        },
                    );
                    account.updated_at = now;
                }
            })
            .await?;
        if let Some(error) = failure {
            return Err(error.into());
        }
        Ok(account)
    }

    pub async fn equip_skin(
        &self,
        account_id: &str,
        requested_skin: Option<&str>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let requested_skin = requested_skin.map(str::trim).filter(|id| !id.is_empty());
        if let Some(skin_id) = requested_skin
            && crate::commerce::skin_by_id(skin_id).is_none()
        {
            return Err("unknown skin".into());
        }
        let mut failure = None;
        let account = self
            .update_account_atomic(account_id, &[], |account| {
                if let Some(skin_id) = requested_skin {
                    if !account.profile.owned_skins.contains(skin_id) {
                        failure = Some("skin is not owned");
                        return;
                    }
                    account.profile.selected_skin = Some(skin_id.to_string());
                } else {
                    account.profile.selected_skin = None;
                }
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            })
            .await?;
        if let Some(error) = failure {
            return Err(error.into());
        }
        Ok(account)
    }

    pub async fn grant_manual_gems(
        &self,
        account_id: &str,
        gems: u64,
        request_id: &str,
    ) -> Result<(PlayerAccount, bool), Box<dyn std::error::Error + Send + Sync>> {
        let account_id = account_id.trim();
        if !is_valid_account_id(account_id) {
            return Err("invalid account_id".into());
        }
        if gems == 0 {
            return Err("gem amount must be greater than zero".into());
        }
        let request_id = request_id.trim();
        if !is_valid_gem_grant_request_id(request_id) {
            return Err("invalid request_id".into());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut granted = false;
        let mut grant_error = None;
        let account = self
            .update_account_atomic(
                account_id,
                &[&format!("operator_gem_grant:{request_id}")],
                |account| {
                    granted = false;
                    grant_error = None;
                    match Self::apply_manual_gem_grant(account, gems, request_id, now) {
                        Ok(applied) => granted = applied,
                        Err(error) => grant_error = Some(error),
                    }
                },
            )
            .await?;
        if let Some(error) = grant_error {
            return Err(error.into());
        }
        Ok((account, granted))
    }

    fn apply_manual_gem_grant(
        account: &mut PlayerAccount,
        gems: u64,
        request_id: &str,
        now: u64,
    ) -> Result<bool, &'static str> {
        if account.kind != AccountKind::Human {
            return Err("account is not a human player");
        }
        if gems == 0 {
            return Err("gem amount must be greater than zero");
        }
        if !is_valid_gem_grant_request_id(request_id) {
            return Err("invalid request_id");
        }
        let grant_id = format!("operator_gem_grant:{request_id}");
        let product_id = format!("sow_operator_gem_grant_{gems}");
        if let Some(existing) = account.profile.purchase_history.get(&grant_id) {
            return if existing.provider == "operator" && existing.product_id == product_id {
                Ok(false)
            } else {
                Err("request_id already used for a different grant")
            };
        }
        account.profile.gems = account
            .profile
            .gems
            .checked_add(gems)
            .ok_or("gem balance overflow")?;
        account.profile.purchase_history.insert(
            grant_id.clone(),
            PurchaseRecord {
                id: grant_id,
                provider: "operator".to_string(),
                environment: "manual".to_string(),
                product_id,
                transaction_id: Some(request_id.to_string()),
                status: "granted".to_string(),
                acquired_at: now,
                updated_at: now,
            },
        );
        account.updated_at = now;
        Ok(true)
    }

    /// Seed the persistent bot-account pool. For each external_id, performs
    /// a get-or-create with `provider = "bot"` (so created accounts carry
    /// `kind = Bot`). Idempotent — re-running with the same external_ids
    /// returns the existing account_ids without duplication. Also maintains
    /// the `sow:bot:pool` SET as an index of all bot account_ids for
    /// analytics / debugging.
    ///
    /// Bot display names remain server-managed; the account record still has a
    /// canonical display_name field for schema consistency.
    pub async fn seed_bot_pool(
        &self,
        external_ids: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut account_ids = Vec::with_capacity(external_ids.len());
        for external_id in external_ids {
            let account = self
                .get_or_create_bot_account("production".into(), external_id)
                .await?;
            account_ids.push(account.id);
        }
        info!(
            "[bot-pool] seed resolved {} bot accounts",
            account_ids.len()
        );
        Ok(account_ids)
    }

    /// Resolve a separate ranked identity and initialize its fixed profile once.
    pub async fn get_or_create_ranked_bot(
        &self,
        external_id: &str,
        display_name: &str,
        leader: &str,
        iq: u32,
        actor: Option<&str>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if external_id.is_empty() || external_id.len() > 100 || !(60..=180).contains(&iq) || iq % 20 != 0 {
            return Err("invalid ranked bot identity or IQ".into());
        }
        let actor = actor.map(str::trim).filter(|value| !value.is_empty());
        if actor.is_some_and(|value| value.len() > 100 || value.chars().any(char::is_control)) {
            return Err("invalid ranked bot change actor".into());
        }
        let display_name = normalize_display_name(display_name).map_err(str::to_string)?;
        let leader = crate::rewards::canonical_leader_name(leader).ok_or("invalid ranked bot leader")?;
        let account = self.get_or_create_bot_account("production".into(), format!("ranked:{external_id}")).await?;
        let account = self.update_account_atomic(&account.id, &[], |account| {
            if account.kind == AccountKind::Bot && account.profile.ranked_bot.is_none() {
                let created_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                account.display_name = display_name.clone();
                account.profile.ranked_bot = Some(RankedBotProfile {
                    iq,
                    leader: leader.clone(),
                    paused: false,
                    reserved_match_id: None,
                    manual_changes: actor.map(|actor| vec![RankedBotChange {
                        changed_at: created_at,
                        actor: actor.to_string(),
                        field: "created".to_string(),
                        old_value: String::new(),
                        new_value: format!("{}; IQ {iq}; leader {leader}", display_name),
                    }]).unwrap_or_default(),
                });
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            }
        }).await?;
        if let Some(db) = &self.redb_db {
            let txn = db.begin_write()?;
            txn.open_table(RANKED_BOTS_TABLE)?
                .insert(account.id.as_str(), b"1".as_slice())?;
            txn.commit()?;
        }
        Ok(account)
    }

    pub async fn ranked_bots(
        &self,
    ) -> Result<Vec<(PlayerAccount, SeasonRating)>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("ranked bot database unavailable".into());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(RANKED_BOTS_TABLE)?;
        let ids = table.iter()?.map(|row| {
            row.map(|(key, _)| key.value().to_string())
        }).collect::<Result<Vec<_>, _>>()?;
        drop(table);
        drop(read_txn);
        let mut bots = Vec::with_capacity(ids.len());
        for id in ids {
            let account = self.load_account(&id).await?;
            if account.kind != AccountKind::Bot || account.profile.ranked_bot.is_none() {
                return Err(format!("ranked bot index points to a non-ranked account {}", account.id).into());
            }
            let rating = self.ranked_rating(&account.id).await?;
            bots.push((account, rating));
        }
        Ok(bots)
    }

    pub async fn update_ranked_bot(
        &self,
        account_id: &str,
        update: RankedBotManualUpdate,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if !is_valid_account_id(account_id) {
            return Err("invalid ranked bot account id".into());
        }
        let actor = match &update {
            RankedBotManualUpdate::SetPaused { actor, .. }
            | RankedBotManualUpdate::SetIq { actor, .. }
            | RankedBotManualUpdate::SetRating { actor, .. } => actor.trim().to_string(),
        };
        if actor.is_empty() || actor.len() > 100 || actor.chars().any(char::is_control) {
            return Err("invalid ranked bot change actor".into());
        }
        if let RankedBotManualUpdate::SetIq { iq, .. } = &update
            && (!(60..=180).contains(iq) || iq % 20 != 0)
        {
            return Err("ranked bot IQ must be 60 through 180 in steps of 20".into());
        }
        if let RankedBotManualUpdate::SetRating { score, .. } = &update
            && !(800..=4000).contains(score)
        {
            return Err("ranked bot rating must be between 800 and 4000".into());
        }
        let account_id = account_id.to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let (_, mut accounts) = self.update_accounts_atomic(
            vec![account_id.clone()],
            Vec::new(),
            move |txn, accounts| {
                let account = &mut accounts[0];
                if account.kind != AccountKind::Bot {
                    return Err("ranked bot account is not a bot".into());
                }
                let bot = account.profile.ranked_bot.as_mut().ok_or("account is not a ranked bot")?;
                let (field, old_value, new_value) = match update {
                    RankedBotManualUpdate::SetPaused { paused, .. } => {
                        let old = bot.paused.to_string();
                        bot.paused = paused;
                        ("paused", old, paused.to_string())
                    }
                    RankedBotManualUpdate::SetIq { iq, .. } => {
                        if !(60..=180).contains(&iq) || iq % 20 != 0 {
                            return Err("ranked bot IQ must be 60 through 180 in steps of 20".into());
                        }
                        let old = bot.iq.to_string();
                        bot.iq = iq;
                        ("iq", old, iq.to_string())
                    }
                    RankedBotManualUpdate::SetRating { score, .. } => {
                        if !(800..=4000).contains(&score) {
                            return Err("ranked bot rating must be between 800 and 4000".into());
                        }
                        let key = format!("rating:{}:{}:Ranked1v1:FFA", account_id, crate::profile::CURRENT_SEASON_ID);
                        let mut ratings = txn.open_table(SEASON_RATINGS_TABLE)?;
                        let mut rating = match ratings.get(key.as_str())? {
                            Some(value) => serde_json::from_slice::<SeasonRating>(value.value())?,
                            None => crate::profile::initial_ranked_rating(
                                account_id.clone(),
                                1000.0 + f64::from(bot.iq.saturating_sub(60)) * 20.0,
                                now,
                            ),
                        };
                        let old = rating.score.to_string();
                        rating.mu = f64::from(score);
                        rating.score = score;
                        let (tier, division) = crate::profile::tier_for_score(score);
                        rating.tier = tier.to_string();
                        rating.division = division;
                        rating.peak_score = rating.peak_score.max(score);
                        rating.updated_at = now;
                        let encoded = serde_json::to_vec(&rating)?;
                        ratings.insert(key.as_str(), encoded.as_slice())?;
                        ("rating", old, score.to_string())
                    }
                };
                let changes = &mut bot.manual_changes;
                if old_value != new_value {
                    changes.push(RankedBotChange {
                        changed_at: now,
                        actor: actor.trim().to_string(),
                        field: field.to_string(),
                        old_value,
                        new_value,
                    });
                    account.updated_at = now;
                }
                Ok(())
            },
        ).await?;
        Ok(accounts.remove(0))
    }

    /// Reserve a managed bot for exactly one match, atomically with its account.
    pub async fn reserve_ranked_bot(
        &self,
        account_id: &str,
        match_id: &str,
    ) -> Result<Option<PlayerAccount>, Box<dyn std::error::Error + Send + Sync>> {
        if !is_valid_account_id(account_id) || match_id.parse::<u64>().is_err() {
            return Err("invalid ranked bot reservation".into());
        }
        let account = self.update_account_atomic(account_id, &[], |account| {
            if account.kind == AccountKind::Bot
                && let Some(bot) = &mut account.profile.ranked_bot
                && !bot.paused && bot.reserved_match_id.is_none()
            {
                bot.reserved_match_id = Some(match_id.to_string());
            }
        }).await?;
        Ok(account.profile.ranked_bot.as_ref()
            .filter(|bot| !bot.paused && bot.reserved_match_id.as_deref() == Some(match_id))
            .map(|_| account.clone()))
    }

    /// Release only the reservation belonging to this match.
    pub async fn release_ranked_bot(
        &self,
        account_id: &str,
        match_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !is_valid_account_id(account_id) || match_id.parse::<u64>().is_err() {
            return Err("invalid ranked bot reservation".into());
        }
        self.update_account_atomic(account_id, &[], |account| {
            if account.kind == AccountKind::Bot
                && let Some(bot) = &mut account.profile.ranked_bot
                && bot.reserved_match_id.as_deref() == Some(match_id)
            {
                bot.reserved_match_id = None;
            }
        }).await?;
        Ok(())
    }

    pub async fn recover_ranked_bot_reservations(
        &self,
        live_match_ids: &[String],
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("ranked bot database unavailable".into());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(RANKED_BOTS_TABLE)?;
        let account_ids = table.iter()?.map(|row| {
            row.map(|(key, _)| key.value().to_string())
        }).collect::<Result<Vec<_>, _>>()?;
        drop(table);
        drop(read_txn);
        if account_ids.is_empty() {
            return Ok(0);
        }
        let live_match_ids = live_match_ids.iter().cloned().collect::<std::collections::BTreeSet<_>>();
        let (released, _) = self.update_accounts_atomic(
            account_ids,
            Vec::new(),
            move |_, accounts| {
                let mut released = 0usize;
                for account in accounts {
                    if let Some(bot) = &mut account.profile.ranked_bot
                        && bot.reserved_match_id.as_ref().is_some_and(|id| !live_match_ids.contains(id))
                    {
                        bot.reserved_match_id = None;
                        account.updated_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                        released += 1;
                    }
                }
                Ok(released)
            },
        ).await?;
        Ok(released)
    }
    /// Update the SOW display name after the request handler has authenticated
    /// the account through its anonymous or platform identity.
    pub async fn update_display_name(
        &self,
        account_id: &str,
        display_name: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if !is_valid_account_id(account_id) {
            return Err("invalid account_id".into());
        }
        let display_name = normalize_display_name(display_name).map_err(str::to_string)?;
        let account = self
            .update_account_atomic(account_id, &[], |account| {
                account.display_name = display_name.clone();
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            })
            .await?;
        Ok(account)
    }

    /// Award the fixed participation receipt using only the registered leader.
    pub async fn record_match_participation(
        &self,
        account_id: &str,
        leader: Option<String>,
        match_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let existing = self.load_account(account_id).await?;
        if existing.kind == AccountKind::Bot {
            return Ok(existing);
        }
        let match_id = match_id.trim();
        if match_id.is_empty() {
            return Err("match id is required".into());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let account = self
            .update_account_atomic(account_id, &[match_id], |account| {
                apply_match_participation_to_account(account, leader.as_deref(), match_id, now);
            })
            .await?;
        Ok(account)
    }

    pub async fn mark_match_reward_verification_status(
        &self,
        match_id: &str,
        status: ReplayVerificationStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start = self
            .match_start_record(match_id)?
            .ok_or("trusted match registration unavailable")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for account_id in start.player_ids {
            let account = self.account_with_events(&account_id, &[match_id]).await?;
            if account.kind == AccountKind::Bot {
                continue;
            }
            if !account.profile.reward_receipts.contains_key(match_id) {
                if status == ReplayVerificationStatus::Rejected {
                    // Historical registrations can lack a participation receipt; rejection grants nothing.
                    continue;
                }
                return Err(format!("participation receipt is missing for {account_id}").into());
            }
            if account
                .profile
                .reward_receipts
                .get(match_id)
                .is_some_and(|receipt| receipt.personal_closure_turn.is_some())
            {
                continue;
            }
            let mut conflicting_status = false;
            self.update_account_atomic(&account_id, &[match_id], |account| {
                if let Some(receipt) = account.profile.reward_receipts.get_mut(match_id) {
                    match receipt.verification_status {
                        Some(current)
                            if current != ReplayVerificationStatus::Pending
                                && current != status =>
                        {
                            conflicting_status = true;
                        }
                        current if current != Some(status) => {
                            receipt.verification_status = Some(status);
                            account.updated_at = now;
                        }
                        _ => {}
                    }
                }
            })
            .await?;
            if conflicting_status {
                return Err(
                    format!("match reward verification already resolved for {account_id}").into(),
                );
            }
        }
        Ok(())
    }

    /// Mark reward receipts as shown in the main menu. This endpoint is
    /// presentation-only: the receipt and its currency are created atomically
    /// during settlement and cannot be created or removed by an ACK.
    pub async fn acknowledge_reward_receipts(
        &self,
        account_id: &str,
        receipt_ids: &[String],
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if receipt_ids.len() > 32 {
            return Err("too many reward receipts".into());
        }
        let receipt_ids = receipt_ids
            .iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty() && id.len() <= 128)
            .collect::<std::collections::BTreeSet<_>>();
        let mut changed = false;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let account = self
            .update_account_atomic(
                account_id,
                &receipt_ids.iter().map(String::as_str).collect::<Vec<_>>(),
                |account| {
                    for id in &receipt_ids {
                        if let Some(receipt) = account.profile.reward_receipts.get_mut(id)
                            && receipt.presented_at.is_none()
                        {
                            receipt.presented_at = Some(now);
                            receipt.status = crate::profile::RewardSettlementStatus::Presented;
                            changed = true;
                        }
                    }
                    if changed {
                        account.updated_at = now;
                    }
                },
            )
            .await?;
        Ok(account)
    }

    /// Complete the offline tutorial once and grant its fixed onboarding reward.
    /// The account secret is verified by the HTTP handler before this method runs.
    pub async fn complete_tutorial(
        &self,
        account_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let account = self
            .update_account_atomic(account_id, &["tutorial"], |account| {
                if account.kind != AccountKind::Human {
                    return;
                }
                // The receipt, not an old completion flag, proves this was paid.
                if account.profile.reward_receipts.contains_key("tutorial") {
                    if account
                        .profile
                        .reward_receipts
                        .get("tutorial")
                        .is_some_and(|receipt| receipt.presented_at.is_none())
                    {
                        if !account.profile.menu_guides.contains_key("welcome") {
                            account.profile.menu_guides.insert(
                                "welcome".to_string(),
                                crate::profile::normalize_welcome_guide_progress(
                                    &MenuGuideProgress::default(),
                                    account.profile.matches_played,
                                ),
                            );
                            account.updated_at = now;
                        }
                    }
                    return;
                }
                account.profile.intro_completed = true;
                let leader = account
                    .profile
                    .preferred_leader
                    .as_deref()
                    .and_then(crate::commerce::leader_from_id)
                    .unwrap_or_else(|| {
                        crate::commerce::assigned_leader_for_account(
                            account_id,
                            crate::commerce::current_rotation_period(),
                        )
                    });
                account.profile.preferred_leader =
                    Some(crate::commerce::leader_wire_id(leader).to_string());
                let reward = crate::rewards::calculate(crate::rewards::RewardInput {
                    tutorial: true,
                    ..Default::default()
                });
                account.profile.apply_reward_receipt(
                    "tutorial",
                    "tutorial",
                    leader.name(),
                    reward,
                    now,
                );
                account
                    .profile
                    .menu_guides
                    .entry("welcome".to_string())
                    .or_insert_with(|| {
                        crate::profile::normalize_welcome_guide_progress(
                            &MenuGuideProgress::default(),
                            account.profile.matches_played,
                        )
                    });
                account.updated_at = now;
            })
            .await?;
        Ok(account)
    }

    /// Register expected players for an upcoming match.
    /// Recover the single orchestrator's sequence from durable registrations and legacy history.
    /// ponytail: one orchestrator owns allocation; use a database allocator before adding another.
    pub fn next_match_id_seed(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        self.next_match_id_seed_in(&replay_directory())
    }

    fn next_match_id_seed_in(
        &self,
        replay_directory: &std::path::Path,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_read()?;
        let mut maximum = 0u64;
        for definition in [MATCH_STARTS_TABLE, MATCHES_TABLE] {
            let table = txn.open_table(definition)?;
            for entry in table.iter()? {
                let (key, _) = entry?;
                if let Ok(id) = key.value().parse::<u64>() {
                    maximum = maximum.max(id);
                }
            }
        }
        recovered_match_id_seed(maximum, replay_directory)
    }

    pub async fn register_match_start(
        &self,
        match_id: &str,
        player_ids: &[String],
        metadata: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if match_id.parse::<u64>().is_err()
            || match_id.len() > 20
            || player_ids.is_empty()
            || player_ids.len() > crate::MAX_MATCH_PARTICIPANTS
            || player_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != player_ids.len()
        {
            return Err("invalid match registration".into());
        }
        self.save_match_start_record(MatchStartRecord {
            match_id: match_id.to_string(),
            player_ids: player_ids.to_vec(),
            metadata,
            replay_status: ReplayVerificationStatus::Pending,
        })?;
        info!(
            "Registered match {match_id} with {} players",
            player_ids.len()
        );
        Ok(())
    }

    fn save_match_start_record(
        &self,
        record: MatchStartRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("Redb database unavailable".into());
        };
        if let Some(existing) = self.match_start_record(&record.match_id)? {
            if existing.player_ids != record.player_ids || existing.metadata != record.metadata {
                return Err("match registration metadata changed".into());
            }
            return Ok(());
        }
        record_match_id_high_water(&replay_directory(), record.match_id.parse()?)?;
        let write_txn = db.begin_write()?;
        let mut table = write_txn.open_table(MATCH_STARTS_TABLE)?;
        if let Some(existing) = table.get(record.match_id.as_str())? {
            let existing: MatchStartRecord = serde_json::from_slice(existing.value())?;
            if existing.player_ids != record.player_ids || existing.metadata != record.metadata {
                return Err("match registration metadata changed".into());
            }
            return Ok(());
        }
        let json = serde_json::to_vec(&record)?;
        table.insert(record.match_id.as_str(), json.as_slice())?;
        drop(table);
        write_txn.commit()?;
        Ok(())
    }

    pub fn match_start_record(
        &self,
        match_id: &str,
    ) -> Result<Option<MatchStartRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Ok(None);
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(MATCH_STARTS_TABLE)?;
        let Some(value) = table.get(match_id)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(value.value())?))
    }

    pub fn registered_human_participant(
        &self,
        match_id: &str,
        account_id: &str,
    ) -> Result<MatchStartRecord, Box<dyn std::error::Error + Send + Sync>> {
        let start = self
            .match_start_record(match_id)?
            .ok_or("trusted match registration unavailable")?;
        let human = start.player_ids.iter().any(|id| id == account_id)
            && start
                .metadata
                .get("relay_players")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|players| {
                    players.iter().any(|player| {
                        player
                            .get("database_account_id")
                            .and_then(serde_json::Value::as_str)
                            == Some(account_id)
                            && player
                                .get("is_internal")
                                .and_then(serde_json::Value::as_bool)
                                != Some(true)
                    })
                });
        if !human {
            return Err("account is not a registered human participant".into());
        }
        Ok(start)
    }

    pub fn match_work(
        &self,
        key: &str,
    ) -> Result<Option<crate::profile::MatchWorkRecord>, Box<dyn std::error::Error + Send + Sync>>
    {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        table
            .get(key)?
            .map(|value| {
                serde_json::from_slice::<crate::profile::MatchWorkRecord>(value.value())
                    .map_err(Into::into)
            })
            .transpose()
    }

    /// A duplicate key must describe the same work that is still pending.
    pub fn queue_match_work(
        &self,
        work: crate::profile::MatchWork,
    ) -> Result<crate::profile::MatchWorkRecord, Box<dyn std::error::Error + Send + Sync>> {
        self.queue_match_work_batch(vec![work])?
            .into_iter()
            .next()
            .ok_or_else(|| "match work queue returned no record".into())
    }

    /// Queue a recovered sequence in one Redb write transaction. Duplicate
    /// keys still have to describe the same durable work.
    pub fn queue_match_work_batch(
        &self,
        works: Vec<crate::profile::MatchWork>,
    ) -> Result<Vec<crate::profile::MatchWorkRecord>, Box<dyn std::error::Error + Send + Sync>>
    {
        use crate::profile::MatchWorkRecord;
        if works.is_empty() {
            return Ok(Vec::new());
        }
        let mut match_ids = std::collections::BTreeSet::new();
        for work in &works {
            let match_id = work.match_id();
            if match_id.parse::<u64>().is_err() {
                return Err("invalid match id".into());
            }
            match_ids.insert(match_id);
        }
        for match_id in match_ids {
            self.match_start_record(match_id)?
                .ok_or("trusted match registration unavailable")?;
        }
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_write()?;
        let mut table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        let mut records = Vec::with_capacity(works.len());
        for work in works {
            let key = work.key();
            if let Some(value) = table.get(key.as_str())? {
                let existing =
                    serde_json::from_slice::<crate::profile::MatchWorkRecord>(value.value())?;
                if existing.work != work {
                    return Err("pending result conflicts with its saved state".into());
                }
                records.push(existing);
            } else {
                let record = MatchWorkRecord::new(work);
                let bytes = serde_json::to_vec(&record)?;
                table.insert(key.as_str(), bytes.as_slice())?;
                records.push(record);
            }
        }
        drop(table);
        txn.commit()?;
        Ok(records)
    }

    pub fn save_match_work(
        &self,
        record: &crate::profile::MatchWorkRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_write()?;
        let mut table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        let key = record.work.key();
        // Never resurrect a job already completed by another operation.
        if table.get(key.as_str())?.is_some() {
            let bytes = serde_json::to_vec(record)?;
            table.insert(key.as_str(), bytes.as_slice())?;
        }
        drop(table);
        txn.commit()?;
        Ok(())
    }

    pub fn remove_match_work(
        &self,
        key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_write()?;
        let mut table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        table.remove(key)?;
        drop(table);
        txn.commit()?;
        Ok(())
    }

    pub fn remove_confirmed_match_notifications(
        &self,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_write()?;
        let mut table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        let keys = {
            let mut keys = Vec::new();
            for entry in table.range("notify:"..)? {
                let (key, value) = entry?;
                if !key.value().starts_with("notify:") {
                    break;
                }
                let job = serde_json::from_slice::<crate::profile::MatchWorkRecord>(value.value())?;
                if job.state == crate::profile::MatchExitState::Confirmed {
                    keys.push(key.value().to_string());
                }
            }
            keys
        };
        for key in &keys {
            table.remove(key.as_str())?;
        }
        drop(table);
        txn.commit()?;
        Ok(keys.len())
    }

    pub fn pending_match_work(
        &self,
        now: u64,
    ) -> Result<Vec<crate::profile::MatchWorkRecord>, Box<dyn std::error::Error + Send + Sync>>
    {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        let mut jobs = Vec::new();
        for entry in table.iter()? {
            let (_, value) = entry?;
            let job = serde_json::from_slice::<crate::profile::MatchWorkRecord>(value.value())?;
            if job.state == crate::profile::MatchExitState::Confirmed {
                continue;
            }
            jobs.push(job);
        }
        // Only the earliest pending chunk may advance a match. A later
        // closure promotes that match's remaining prefix, never skips it.
        let mut owners = std::collections::HashMap::<String, (u64, bool)>::new();
        for job in &jobs {
            if let crate::profile::MatchWork::Progress {
                match_id,
                sequence,
                closing,
                ..
            } = &job.work
            {
                let owner = owners.entry(match_id.clone()).or_insert((*sequence, false));
                owner.0 = owner.0.min(*sequence);
                owner.1 |= closing;
            }
        }
        jobs.retain_mut(|job| {
            if let crate::profile::MatchWork::Progress {
                match_id, sequence, ..
            } = &job.work
            {
                let (first, urgent) = owners[match_id];
                job.urgent = urgent;
                if *sequence != first {
                    return false;
                }
            }
            job.retry_at <= now
        });
        jobs.sort_by_key(|job| {
            (
                match job.work {
                    crate::profile::MatchWork::Progress { .. } => {
                        if job.urgent {
                            1
                        } else {
                            3
                        }
                    }
                    crate::profile::MatchWork::Archive { .. } => 4,
                    crate::profile::MatchWork::Notify { .. } => 0,
                },
                job.queued_at_ms,
            )
        });
        // Keep participant closures and live progress moving alongside each other.
        let (mut checks, mut progress, mut archives, mut notifications) = (0, 0, 0, 0);
        jobs.retain(|job| {
            let count =
                if matches!(job.work, crate::profile::MatchWork::Progress { .. }) && !job.urgent {
                    &mut progress
                } else if matches!(job.work, crate::profile::MatchWork::Archive { .. }) {
                    &mut archives
                } else if matches!(job.work, crate::profile::MatchWork::Notify { .. }) {
                    &mut notifications
                } else {
                    &mut checks
                };
            *count += 1;
            *count
                <= if matches!(job.work, crate::profile::MatchWork::Archive { .. }) {
                    1
                } else {
                    32
                }
        });
        Ok(jobs)
    }

    /// All uncommitted calculations, including jobs deferred for a retry.
    /// Deployment must not use the scheduler's bounded, ready-only view.
    pub fn pending_reward_calculations(
        &self,
    ) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
        use crate::profile::{MatchExitState, MatchWork, MatchWorkRecord};
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        let mut calculations = std::collections::BTreeSet::new();
        let mut payments = std::collections::BTreeSet::new();
        for entry in table.iter()? {
            let (_, value) = entry?;
            let job: MatchWorkRecord = serde_json::from_slice(value.value())?;
            if job.state != MatchExitState::Confirmed
                && let MatchWork::Progress {
                    match_id, closing, ..
                } = job.work
            {
                calculations.insert(match_id.clone());
                if closing {
                    payments.insert(match_id);
                }
            }
        }
        Ok((
            calculations.into_iter().collect(),
            payments.into_iter().collect(),
        ))
    }

    pub fn pending_match_progress(
        &self,
        match_id: &str,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error + Send + Sync>> {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
        let prefix = format!("progress:{match_id}:");
        let mut pending = Vec::new();
        for entry in table.range(prefix.as_str()..)? {
            let (key, value) = entry?;
            if !key.value().starts_with(&prefix) {
                break;
            }
            if let crate::profile::MatchWork::Progress { sequence, .. } =
                serde_json::from_slice::<crate::profile::MatchWorkRecord>(value.value())?.work
            {
                pending.push(sequence);
            }
        }
        Ok(pending)
    }

    pub fn finish_match_verification(
        &self,
        match_id: &str,
        status: ReplayVerificationStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.redb_db else {
            return Err("Redb database unavailable".into());
        };
        let write_txn = db.begin_write()?;
        let mut starts = write_txn.open_table(MATCH_STARTS_TABLE)?;
        let Some(value) = starts.get(match_id)? else {
            return Err("trusted match registration unavailable".into());
        };
        let mut record: MatchStartRecord = serde_json::from_slice(value.value())?;
        drop(value);
        if record.replay_status == ReplayVerificationStatus::Pending
            || record.replay_status == status
        {
            record.replay_status = status;
        }
        let json = serde_json::to_vec(&record)?;
        starts.insert(match_id, json.as_slice())?;
        drop(starts);
        write_txn.commit()?;
        Ok(())
    }

    /// Finalize a match from its immutable registration. Relay metadata is
    /// accepted only as a match-id check, never as a player roster or result.
    pub async fn finalize_match(
        &self,
        match_id: &str,
    ) -> Result<Vec<PlayGamesMatchOutcome>, Box<dyn std::error::Error + Send + Sync>> {
        self.finalize_match_with_lobby(match_id, None).await
    }

    pub async fn finalize_match_with_lobby(
        &self,
        match_id: &str,
        lobby_json: Option<&str>,
    ) -> Result<Vec<PlayGamesMatchOutcome>, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let finalized_key = format!("sow:match:{match_id}:finalized");
        if con.exists(&finalized_key).await? {
            return Ok(Vec::new());
        }
        if self
            .load_match_record(match_id)?
            .is_some_and(|record| !record.provisional)
        {
            let _: () = con.set(&finalized_key, "1").await?;
            return Ok(Vec::new());
        }
        let start = self
            .match_start_record(match_id)?
            .ok_or("trusted match registration unavailable; keeping finalization retryable")?;

        // Only one worker assembles the match at a time. The per-account
        // receipt below remains the real duplicate-payment guard if this
        // process expires or restarts during the transaction.
        let settling_key = format!("sow:match:{match_id}:settling");
        let claimed: bool = redis::cmd("SET")
            .arg(&settling_key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(300)
            .query_async(&mut con)
            .await?;
        if !claimed {
            return Ok(Vec::new());
        }

        let players = start.player_ids.clone();
        if players.is_empty() {
            let _: Result<(), _> = con.del(&settling_key).await;
            return Err("trusted match roster unavailable; keeping finalization retryable".into());
        }

        if let Some(lobby_json) = lobby_json {
            validate_lobby_match_id(match_id, lobby_json)?;
        }
        let lobby_players = start
            .metadata
            .get("relay_players")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let completed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let start_for_write = start.clone();
        let players_for_write = players.clone();
        let account_ids = players.clone();
        let match_id_for_write = match_id.to_string();
        let update = self
            .update_accounts_atomic(
                account_ids,
                vec![match_id_for_write.clone()],
                move |txn, accounts| {
                    let mut participant_records = Vec::with_capacity(accounts.len());
                    for (account_id, account) in players_for_write.iter().zip(accounts.iter_mut()) {
                        let raw = lobby_players.iter().find(|player| {
                            player
                                .get("database_account_id")
                                .and_then(serde_json::Value::as_str)
                                == Some(account_id.as_str())
                        });
                        let leader = raw
                            .and_then(|player| player.get("leader"))
                            .and_then(serde_json::Value::as_str)
                            .and_then(crate::rewards::canonical_leader_name);
                        let team = raw
                            .and_then(|player| player.get("team"))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string);
                        apply_match_participation_to_account(
                            account,
                            leader.as_deref(),
                            &match_id_for_write,
                            completed_at,
                        );
                        let reward = if account.kind == AccountKind::Bot {
                            crate::rewards::MatchReward::default()
                        } else {
                            crate::rewards::calculate(crate::rewards::RewardInput::default())
                        };
                        participant_records.push(crate::profile::MatchParticipantRecord {
                            account_id: account_id.clone(),
                            display_name: account.display_name.clone(),
                            is_bot: account.kind == AccountKind::Bot,
                            leader,
                            team,
                            xp: reward.xp,
                            leader_xp: reward.leader_xp,
                            crowns: reward.crowns,
                            laurels: reward.laurels,
                            personal_closure_turn: account
                                .profile
                                .reward_receipts
                                .get(&match_id_for_write)
                                .and_then(|receipt| receipt.personal_closure_turn),
                            ..Default::default()
                        });
                    }
                    let mut record = provisional_match_record(&start_for_write, completed_at);
                    record.provisional = false;
                    record.participants = participant_records;
                    if let Some(previous) =
                        load_match_record_in_transaction(txn, &match_id_for_write)?
                    {
                        for closed in previous
                            .participants
                            .iter()
                            .filter(|participant| participant.personal_closure_turn.is_some())
                        {
                            if let Some(current) = record
                                .participants
                                .iter_mut()
                                .find(|current| current.account_id == closed.account_id)
                            {
                                *current = closed.clone();
                            }
                        }
                    }
                    write_match_record_in_transaction(txn, &record)
                },
            )
            .await;
        if let Err(error) = update {
            let _: Result<(), _> = con.del(&settling_key).await;
            return Err(error);
        }

        let _: () = redis::pipe()
            .set(&finalized_key, "1")
            .del(&settling_key)
            .query_async::<()>(&mut con)
            .await?;

        info!("Finalized match {match_id} with {} players", players.len());
        Ok(Vec::new())
    }

    pub async fn apply_verified_match_result(
        &self,
        result: &VerifiedMatchResult,
        notification: Option<crate::profile::MatchResultControl>,
    ) -> Result<
        (Vec<PlayGamesMatchOutcome>, Vec<PlayerAccount>),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        if result.personal_closure_turn.is_some() {
            return Err(
                "invalid verified result: full match result contains a personal closure".into(),
            );
        }
        let start = self
            .match_start_record(&result.match_id)?
            .ok_or("trusted match registration unavailable")?;
        validate_verified_match_result(&start, result)
            .map_err(|error| Box::<dyn std::error::Error + Send + Sync>::from(error))?;
        if notification
            .as_ref()
            .is_some_and(|command| command.match_id != result.match_id)
        {
            return Err("invalid verified result: notification belongs to another match".into());
        }
        let account_ids = result
            .participants
            .iter()
            .map(|participant| participant.account_id.clone())
            .collect::<Vec<_>>();
        let match_id = result.match_id.clone();
        let bonus_id = format!("verified_bonus:{match_id}");
        let result = result.clone();
        let result_for_write = result.clone();
        let start_for_write = start.clone();
        let notification_for_write = notification;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let (include, accounts) = self
            .update_accounts_atomic(
                account_ids,
                vec![match_id.clone(), bonus_id],
                move |txn, accounts| {
                    let mut record = load_match_record_in_transaction(txn, &match_id)?
                        .unwrap_or_else(|| provisional_match_record(&start_for_write, now));
                    let registered = start_for_write
                        .player_ids
                        .iter()
                        .map(String::as_str)
                        .collect::<std::collections::BTreeSet<_>>();
                    let recorded = record
                        .participants
                        .iter()
                        .map(|participant| participant.account_id.as_str())
                        .collect::<std::collections::BTreeSet<_>>();
                    if recorded.len() != record.participants.len()
                        || !recorded.is_subset(&registered)
                        || ((!record.provisional || record.verified) && recorded != registered)
                    {
                        return Err("invalid verified result: match history roster differs from registration".into());
                    }
                    let mut include = Vec::with_capacity(result_for_write.participants.len());
                    for (participant, account) in
                        result_for_write.participants.iter().zip(accounts.iter_mut())
                    {
                        let receipt = account.profile.reward_receipts.get(&match_id);
                        let decision = verified_receipt_decision(receipt, None);
                        if account.kind != AccountKind::Bot
                            && should_skip_full_match_participant(receipt, decision)
                        {
                            include.push(false);
                            continue;
                        }
                        if record.verified
                            && !match_record_has_participant_result(&record, participant, None)
                        {
                            return Err("invalid verified result: saved participant result conflicts".into());
                        }
                        if record.verified {
                            include.push(false);
                            continue;
                        }
                        apply_match_participation_to_account(
                            account,
                            participant.leader.as_deref(),
                            &match_id,
                            now,
                        );
                        update_match_record_participant(
                            &mut record,
                            account,
                            participant,
                            None,
                        );
                        if record.queue == "Ranked1v1" {
                            apply_ranked_bot_stats_to_account(account, participant, now);
                        }
                        include.push(
                            apply_verified_participant_reward_to_account(
                                account,
                                &result_for_write,
                                participant,
                                None,
                                now,
                            )
                            .map_err(Box::<dyn std::error::Error + Send + Sync>::from)?,
                        );
                    }
                    record.duration_seconds = result_for_write.duration_seconds;
                    record.winner_account_id = result_for_write
                        .winner_player_id
                        .and_then(|winner| {
                            result_for_write
                                .participants
                                .iter()
                                .find(|participant| participant.player_id == winner)
                        })
                        .map(|participant| participant.account_id.clone());
                    record.winning_team = result_for_write.winning_team.clone();
                    if record.queue == "Ranked1v1" && !record.rating_eligible {
                        settle_ranked_ratings(txn, &mut record, &result_for_write, accounts, now)?;
                    }
                    if record.queue == "Ranked1v1" && record.rating_eligible {
                        for account in accounts.iter_mut() {
                            if let Some(bot) = &mut account.profile.ranked_bot
                                && bot.reserved_match_id.as_deref() == Some(match_id.as_str())
                            {
                                bot.reserved_match_id = None;
                            }
                        }
                    }
                    record.verified = true;
                    record.provisional = false;
                    write_match_record_in_transaction(txn, &record)?;
                    if let Some(command) = notification_for_write {
                        queue_match_work_in_transaction(
                            txn,
                            crate::profile::MatchWork::Notify { command },
                        )?;
                    }
                    Ok(include)
                },
            )
            .await?;

        let mut playgames_outcomes = Vec::new();
        let mut mirrored_accounts = Vec::new();
        for ((participant, account), include) in
            result.participants.iter().zip(accounts).zip(include)
        {
            if !include {
                continue;
            }
            playgames_outcomes.push(PlayGamesMatchOutcome {
                account_id: account.id.clone(),
                won: participant.won,
                wins: account.profile.verified_wins,
                sync_revision: account.profile.playgames_sync_revision,
                unlocked_achievements: account
                    .profile
                    .unlocked_achievements
                    .iter()
                    .cloned()
                    .collect(),
            });
            mirrored_accounts.push(account);
        }
        Ok((playgames_outcomes, mirrored_accounts))
    }

    pub fn pending_crazygames_scores(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let db = self.redb_db.as_ref().ok_or("Redb database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(crate::redb_store::PENDING_CRAZYGAMES_SCORES_TABLE)?;
        table
            .iter()?
            .take(32)
            .map(|row| {
                row.map(|(key, _)| key.value().to_string())
                    .map_err(Into::into)
            })
            .collect()
    }

    /// The same persisted profile revision drives both external mirrors.
    /// The pending index is saved in the reward's own REDB transaction.
    pub async fn retry_crazygames_scores(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(api_key) = &self.crazygames_api_key else {
            return Ok(());
        };
        for account_id in self.pending_crazygames_scores()? {
            let account = self.account_by_id(&account_id).await?;
            let Some(identity) = account
                .linked_identities
                .iter()
                .find(|identity| identity.provider == "crazygames")
            else {
                continue;
            };
            let revision = account.profile.playgames_sync_revision;
            if let Err(error) =
                crate::crazygames::submit_score(api_key, &identity.external_id, account.profile.xp)
                    .await
            {
                log::warn!("CrazyGames score remains pending account={account_id}: {error}");
                continue;
            }
            self.update_account_atomic(&account_id, &[], |account| {
                account.profile.crazygames_synced_revision =
                    account.profile.crazygames_synced_revision.max(revision);
            })
            .await?;
        }
        Ok(())
    }
}

fn validate_lobby_match_id(
    match_id: &str,
    lobby_json: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lobby: serde_json::Value = serde_json::from_str(lobby_json)?;
    let lobby_id = lobby
        .get("lobby_id")
        .and_then(serde_json::Value::as_u64)
        .map(|id| id.to_string())
        .or_else(|| {
            lobby
                .get("lobby_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    if lobby_id.as_deref() != Some(match_id) {
        return Err("match metadata does not belong to this match".into());
    }
    Ok(())
}

fn validate_verified_match_result(
    start: &MatchStartRecord,
    result: &VerifiedMatchResult,
) -> Result<(), String> {
    if start.match_id != result.match_id {
        return Err("invalid verified result: match id differs from registration".to_string());
    }
    if start.replay_status == ReplayVerificationStatus::Rejected {
        return Err("invalid verified result: replay was already rejected".to_string());
    }
    let trusted_players = start
        .metadata
        .get("relay_players")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            "invalid verified result: trusted match roster metadata unavailable".to_string()
        })?;
    let mut trusted = std::collections::BTreeMap::new();
    for player in trusted_players {
        let player_id = player
            .get("player_id")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(|| "invalid verified result: trusted player id is invalid".to_string())?;
        let account_id = player
            .get("database_account_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                "invalid verified result: trusted player account is missing".to_string()
            })?
            .to_string();
        let team = player
            .get("team")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let leader = player
            .get("leader")
            .and_then(serde_json::Value::as_str)
            .and_then(crate::rewards::canonical_leader_name);
        if trusted
            .insert(player_id, (account_id, team, leader))
            .is_some()
        {
            return Err("invalid verified result: duplicate trusted player id".to_string());
        }
    }

    let mut seen_ids = std::collections::BTreeSet::new();
    let mut result_accounts = std::collections::BTreeSet::new();
    for participant in &result.participants {
        if !seen_ids.insert(participant.player_id)
            || !result_accounts.insert(participant.account_id.as_str())
        {
            return Err("invalid verified result: duplicate participant".to_string());
        }
        let Some((account_id, team, leader)) = trusted.get(&participant.player_id) else {
            return Err(
                "invalid verified result: player is not in the registered roster".to_string(),
            );
        };
        if account_id != &participant.account_id
            || team.as_deref() != participant.team.as_deref()
            || leader.as_deref() != participant.leader.as_deref()
        {
            return Err(
                "invalid verified result: participant differs from registered roster".to_string(),
            );
        }
        let should_win = match (result.winner_player_id, result.winning_team.as_deref()) {
            (Some(_), Some(team)) => participant.team.as_deref() == Some(team),
            (Some(winner), None) => participant.player_id == winner,
            (None, None) => false,
            (None, Some(_)) => {
                return Err(
                    "invalid verified result: winning team has no winning player".to_string(),
                );
            }
        };
        if participant.won != should_win {
            return Err(
                "invalid verified result: victory differs from registered team".to_string(),
            );
        }
    }
    if result
        .winner_player_id
        .is_some_and(|winner| !trusted.contains_key(&winner))
    {
        return Err("invalid verified result: winner is not in the registered roster".to_string());
    }
    if let (Some(winner), Some(winning_team)) =
        (result.winner_player_id, result.winning_team.as_deref())
        && trusted
            .get(&winner)
            .and_then(|(_, team, _)| team.as_deref())
            != Some(winning_team)
    {
        return Err(
            "invalid verified result: winner does not belong to the winning team".to_string(),
        );
    }
    let registered_accounts = start
        .player_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let metadata_accounts = trusted
        .values()
        .map(|(account_id, _, _)| account_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if registered_accounts != metadata_accounts
        || (result.personal_closure_turn.is_none() && registered_accounts != result_accounts)
        || (result.personal_closure_turn.is_some() && result_accounts.len() != 1)
    {
        return Err("invalid verified result: roster is incomplete".to_string());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerifiedReceiptDecision {
    Apply,
    AlreadyClosed,
    Reject,
}

fn verified_receipt_decision(
    receipt: Option<&crate::profile::RewardReceipt>,
    personal_closure_turn: Option<u64>,
) -> VerifiedReceiptDecision {
    let Some(receipt) = receipt else {
        return VerifiedReceiptDecision::Reject;
    };
    if receipt.verification_status == Some(ReplayVerificationStatus::Rejected) {
        return VerifiedReceiptDecision::Reject;
    }
    match (receipt.personal_closure_turn, personal_closure_turn) {
        (Some(_), None) => VerifiedReceiptDecision::AlreadyClosed,
        (Some(saved), Some(expected)) if saved == expected => VerifiedReceiptDecision::Apply,
        (None, None) => VerifiedReceiptDecision::Apply,
        _ => VerifiedReceiptDecision::Reject,
    }
}

fn should_skip_full_match_participant(
    receipt: Option<&crate::profile::RewardReceipt>,
    decision: VerifiedReceiptDecision,
) -> bool {
    decision == VerifiedReceiptDecision::AlreadyClosed
        || (decision == VerifiedReceiptDecision::Reject
            && receipt.is_some_and(|receipt| receipt.personal_closure_turn.is_some()))
}

fn settle_ranked_ratings(
    txn: &WriteTransaction,
    record: &mut MatchRecord,
    result: &VerifiedMatchResult,
    accounts: &[PlayerAccount],
    now: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if result.participants.len() != 2 || accounts.len() != 2 || record.mode != "FFA"
        || result.winning_team.is_some()
        || result.winner_player_id.is_some_and(|winner| !result.participants.iter().any(|p| p.player_id == winner))
    {
        return Err("invalid verified ranked result".into());
    }
    let mut table = txn.open_table(SEASON_RATINGS_TABLE)?;
    let mut ratings = Vec::with_capacity(2);
    let mut keys = Vec::with_capacity(2);
    for (participant, account) in result.participants.iter().zip(accounts) {
        if participant.account_id != account.id {
            return Err("ranked participant account mismatch".into());
        }
        let key = format!("rating:{}:{}:Ranked1v1:FFA", account.id, record.season_id);
        let rating = match table.get(key.as_str())? {
            Some(value) => serde_json::from_slice::<SeasonRating>(value.value())?,
            None => {
                let initial = match account.kind {
                    AccountKind::Bot => {
                        let bot = account.profile.ranked_bot.as_ref().ok_or("ranked bot profile missing")?;
                        1000.0 + f64::from(bot.iq.saturating_sub(60)) * 20.0
                    }
                    AccountKind::Human => 1000.0,
                };
                crate::profile::initial_ranked_rating(account.id.clone(), initial, now)
            }
        };
        if rating.account_id != account.id || rating.season_id != record.season_id || rating.schema_version != 2 {
            return Err("ranked rating identity mismatch".into());
        }
        keys.push(key);
        ratings.push(rating);
    }
    let before = ratings.clone();
    for i in 0..2 {
        let player = &result.participants[i];
        let outcome = match result.winner_player_id {
            Some(winner) if winner == player.player_id => 1.0,
            Some(_) => 0.0,
            None => 0.5,
        };
        let points_breakdown = crate::profile::ranked_points_breakdown(&before[i], &before[1 - i], outcome);
        crate::profile::apply_ranked_result(&mut ratings[i], &before[1 - i], outcome, now)?;
        let participant = record.participants.iter_mut().find(|p| p.account_id == player.account_id)
            .ok_or("ranked history participant missing")?;
        participant.rating_delta = Some((i32::from(ratings[i].score) - i32::from(before[i].score))
            .clamp(i16::MIN as i32, i16::MAX as i32) as i16);
        participant.rating_before = Some(before[i].score);
        participant.rating_after = Some(ratings[i].score);
        participant.tier_before = Some(before[i].tier.clone());
        participant.tier_after = Some(ratings[i].tier.clone());
        participant.division_before = before[i].division.clone();
        participant.division_after = ratings[i].division.clone();
        participant.games_played_after = Some(ratings[i].games_played);
        participant.placements_complete_after = Some(ratings[i].placements_complete);
        participant.ranked_points_breakdown = Some(points_breakdown);
        let encoded = serde_json::to_vec(&ratings[i])?;
        table.insert(keys[i].as_str(), encoded.as_slice())?;
    }
    record.rating_eligible = true;
    Ok(())
}

fn provisional_match_record(start: &MatchStartRecord, now: u64) -> MatchRecord {
    let metadata = &start.metadata;
    let config = metadata.get("config");
    let started_at = metadata
        .get("started_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(now);
    MatchRecord {
        schema_version: 1,
        match_id: start.match_id.clone(),
        season_id: crate::profile::CURRENT_SEASON_ID,
        queue: metadata
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("Matchmaking")
            .to_string(),
        mode: config
            .and_then(|value| value.get("game_mode"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("FFA")
            .to_string(),
        map_name: config
            .and_then(|value| value.get("map_name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("world")
            .to_string(),
        started_at,
        completed_at: now,
        duration_seconds: now.saturating_sub(started_at).min(u32::MAX as u64) as u32,
        winner_account_id: None,
        winning_team: None,
        verified: false,
        provisional: true,
        rating_eligible: false,
        ranked_bot_iq: config
            .and_then(|value| value.get("ranked_bot_iq"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        participants: Vec::new(),
    }
}

fn update_match_record_participant(
    record: &mut MatchRecord,
    account: &PlayerAccount,
    participant: &crate::profile::VerifiedMatchParticipant,
    personal_closure_turn: Option<u64>,
) {
    let earned = crate::rewards::calculate(crate::rewards::RewardInput {
        won: participant.won,
        players_defeated: participant.players_defeated,
        empires_defeated: participant.empires_defeated,
        tribes_defeated: participant.tribes_defeated,
        kills: participant.kills,
        assists: participant.assists,
        ..Default::default()
    });
    let recorded = record
        .participants
        .iter_mut()
        .find(|entry| entry.account_id == participant.account_id);
    let recorded = if let Some(recorded) = recorded {
        recorded
    } else {
        record
            .participants
            .push(crate::profile::MatchParticipantRecord {
                account_id: account.id.clone(),
                display_name: account.display_name.clone(),
                is_bot: account.kind == AccountKind::Bot,
                leader: participant.leader.clone(),
                team: participant.team.clone(),
                ..Default::default()
            });
        record
            .participants
            .last_mut()
            .expect("participant was appended")
    };
    recorded.won = participant.won;
    recorded.kills = participant.kills;
    recorded.deaths = participant.deaths;
    recorded.assists = participant.assists;
    recorded.players_defeated = participant.players_defeated;
    recorded.empires_defeated = participant.empires_defeated;
    recorded.tribes_defeated = participant.tribes_defeated;
    recorded.placement = if participant.won { 1 } else { 0 };
    recorded.xp = earned.xp;
    recorded.leader_xp = earned.leader_xp;
    recorded.crowns = earned.crowns;
    recorded.laurels = earned.laurels;
    if personal_closure_turn.is_some() {
        recorded.personal_closure_turn = personal_closure_turn;
    }
}

fn match_record_has_participant_result(
    record: &MatchRecord,
    participant: &crate::profile::VerifiedMatchParticipant,
    personal_closure_turn: Option<u64>,
) -> bool {
    let Some(saved) = record
        .participants
        .iter()
        .find(|entry| entry.account_id == participant.account_id)
    else {
        return false;
    };
    let earned = crate::rewards::calculate(crate::rewards::RewardInput {
        won: participant.won,
        players_defeated: participant.players_defeated,
        empires_defeated: participant.empires_defeated,
        tribes_defeated: participant.tribes_defeated,
        kills: participant.kills,
        assists: participant.assists,
        ..Default::default()
    });
    saved.personal_closure_turn == personal_closure_turn
        && saved.leader == participant.leader
        && saved.team == participant.team
        && saved.won == participant.won
        && saved.kills == participant.kills
        && saved.deaths == participant.deaths
        && saved.assists == participant.assists
        && saved.players_defeated == participant.players_defeated
        && saved.empires_defeated == participant.empires_defeated
        && saved.tribes_defeated == participant.tribes_defeated
        && saved.placement == if participant.won { 1 } else { 0 }
        && saved.xp == earned.xp
        && saved.leader_xp == earned.leader_xp
        && saved.crowns == earned.crowns
        && saved.laurels == earned.laurels
}

fn apply_match_participation_to_account(
    account: &mut PlayerAccount,
    leader: Option<&str>,
    match_id: &str,
    now: u64,
) {
    if account.kind == AccountKind::Bot {
        return;
    }
    let canonical_leader = leader.and_then(crate::rewards::canonical_leader_name);
    if let Some(receipt) = account.profile.reward_receipts.get_mut(match_id) {
        if receipt.verification_status.is_none() {
            receipt.verification_status = Some(ReplayVerificationStatus::Pending);
            account.updated_at = now;
        }
        return;
    }
    let effective_leader = account.profile.record_match_stats_with_leader(
        false,
        SessionDefeats::default(),
        0,
        0,
        0,
        canonical_leader.as_deref(),
    );
    if let Some(guide) = account.profile.menu_guides.get_mut("welcome") {
        *guide =
            crate::profile::normalize_welcome_guide_progress(guide, account.profile.matches_played);
    }
    account.profile.apply_reward_receipt(
        match_id,
        match_id,
        &effective_leader,
        crate::rewards::calculate(crate::rewards::RewardInput::default()),
        now,
    );
    if let Some(receipt) = account.profile.reward_receipts.get_mut(match_id) {
        receipt.verification_status = Some(ReplayVerificationStatus::Pending);
    }
    account.updated_at = now;
}

fn apply_ranked_bot_stats_to_account(
    account: &mut PlayerAccount,
    participant: &crate::profile::VerifiedMatchParticipant,
    now: u64,
) {
    if account.kind != AccountKind::Bot || account.profile.ranked_bot.is_none() {
        return;
    }
    let leader = account.profile.record_match_stats_with_leader(
        participant.won,
        SessionDefeats {
            players: participant.players_defeated,
            empires: participant.empires_defeated,
            tribes: participant.tribes_defeated,
        },
        participant.kills,
        participant.deaths,
        participant.assists,
        participant.leader.as_deref(),
    );
    if participant.won {
        account.profile.verified_wins = account.profile.verified_wins.saturating_add(1);
        if let Some(stats) = account.profile.leader_stats.get_mut(&leader) {
            stats.verified_wins = stats.verified_wins.saturating_add(1);
        }
    }
    account.updated_at = now;
}

fn apply_verified_participant_reward_to_account(
    account: &mut PlayerAccount,
    result: &VerifiedMatchResult,
    participant: &crate::profile::VerifiedMatchParticipant,
    personal_closure_turn: Option<u64>,
    now: u64,
) -> Result<bool, String> {
    if account.kind == AccountKind::Bot {
        return Ok(false);
    }
    let receipt = account.profile.reward_receipts.get(&result.match_id);
    match verified_receipt_decision(receipt, personal_closure_turn) {
        VerifiedReceiptDecision::AlreadyClosed => return Ok(false),
        VerifiedReceiptDecision::Reject => {
            return Err(
                "invalid verified result: participation receipt is missing or conflicts".into(),
            );
        }
        VerifiedReceiptDecision::Apply => {}
    }

    let earned = crate::rewards::calculate(crate::rewards::RewardInput {
        won: participant.won,
        players_defeated: participant.players_defeated,
        empires_defeated: participant.empires_defeated,
        tribes_defeated: participant.tribes_defeated,
        kills: participant.kills,
        assists: participant.assists,
        ..Default::default()
    });
    let participation = crate::rewards::calculate(crate::rewards::RewardInput::default());
    let bonus = crate::rewards::MatchReward {
        xp: earned.xp.saturating_sub(participation.xp),
        leader_xp: earned.leader_xp.saturating_sub(participation.leader_xp),
        crowns: earned.crowns.saturating_sub(participation.crowns),
        laurels: earned.laurels.saturating_sub(participation.laurels),
    };
    let result_hash = verified_participant_result_hash(result, participant, personal_closure_turn)?;
    let bonus_id = format!("verified_bonus:{}", result.match_id);
    if let Some(saved) = account.profile.reward_receipts.get(&bonus_id) {
        if saved.match_id != result.match_id
            || saved.xp != bonus.xp
            || saved.leader_xp != bonus.leader_xp
            || saved.crowns != bonus.crowns
            || saved.laurels < bonus.laurels
            || saved.verification_status != Some(ReplayVerificationStatus::Verified)
            || saved
                .verified_result_sha256
                .as_deref()
                .is_some_and(|saved_hash| saved_hash != result_hash.as_str())
        {
            return Err(
                "invalid verified result: saved reward conflicts with verified result".into(),
            );
        }
        let mut changed = false;
        if let Some(receipt) = account.profile.reward_receipts.get_mut(&bonus_id)
            && receipt.verified_result_sha256.is_none()
        {
            receipt.verified_result_sha256 = Some(result_hash);
            changed = true;
        }
        if let Some(receipt) = account.profile.reward_receipts.get_mut(&result.match_id) {
            if receipt.verification_status != Some(ReplayVerificationStatus::Verified) {
                receipt.verification_status = Some(ReplayVerificationStatus::Verified);
                changed = true;
            }
        }
        if changed {
            account.updated_at = now;
        }
        return Ok(true);
    }

    let receipt = account
        .profile
        .reward_receipts
        .get_mut(&result.match_id)
        .ok_or_else(|| "invalid verified result: participation receipt is missing".to_string())?;
    receipt.verification_status = Some(ReplayVerificationStatus::Verified);

    let leader = participant
        .leader
        .as_deref()
        .and_then(crate::rewards::canonical_leader_name)
        .or_else(|| account.profile.preferred_leader.clone())
        .unwrap_or_else(|| "Caesar".to_string());
    account.profile.players_defeated = account
        .profile
        .players_defeated
        .saturating_add(participant.players_defeated);
    account.profile.empires_defeated = account
        .profile
        .empires_defeated
        .saturating_add(participant.empires_defeated);
    account.profile.tribes_defeated = account
        .profile
        .tribes_defeated
        .saturating_add(participant.tribes_defeated);
    account.profile.kills = account.profile.kills.saturating_add(participant.kills);
    account.profile.deaths = account.profile.deaths.saturating_add(participant.deaths);
    account.profile.assists = account.profile.assists.saturating_add(participant.assists);
    let leader_stats = account
        .profile
        .leader_stats
        .entry(leader.clone())
        .or_default();
    leader_stats.kills = leader_stats.kills.saturating_add(participant.kills);
    leader_stats.deaths = leader_stats.deaths.saturating_add(participant.deaths);
    leader_stats.assists = leader_stats.assists.saturating_add(participant.assists);
    if participant.won {
        account.profile.wins = account.profile.wins.saturating_add(1);
        account.profile.verified_wins = account.profile.verified_wins.saturating_add(1);
        leader_stats.wins = leader_stats.wins.saturating_add(1);
        leader_stats.verified_wins = leader_stats.verified_wins.saturating_add(1);
    }
    account
        .profile
        .apply_reward_receipt(&bonus_id, &result.match_id, &leader, bonus, now);
    if let Some(receipt) = account.profile.reward_receipts.get_mut(&bonus_id) {
        receipt.verification_status = Some(ReplayVerificationStatus::Verified);
        receipt.verified_result_sha256 = Some(result_hash);
    }
    account.updated_at = now;
    Ok(true)
}

fn verified_participant_result_hash(
    result: &VerifiedMatchResult,
    participant: &crate::profile::VerifiedMatchParticipant,
    personal_closure_turn: Option<u64>,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(
        result.match_id.as_str(),
        result.duration_seconds,
        result.winner_player_id,
        result.winning_team.as_deref(),
        personal_closure_turn,
        participant,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hex::encode(sha2::Sha256::digest(bytes)))
}

fn is_valid_gem_grant_request_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    #[test]
    fn match_progress_work_survives_restart_and_four_concurrent_deliveries() {
        use crate::profile::{MatchStartRecord, MatchWork, ReplayVerificationStatus};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("returns.redb");
        let account = "0123456789abcdef0123456789abcdef";
        let work = MatchWork::Progress {
            match_id: "42".into(),
            sequence: 0,
            sha256: "a".repeat(64),
            closing: false,
        };
        {
            let db = super::PlayerDb::new(
                "redis://127.0.0.1/",
                None,
                Some(std::sync::Arc::new(
                    crate::redb_store::init_database(&path).unwrap(),
                )),
            );
            db.save_match_start_record(MatchStartRecord { match_id:"42".into(), player_ids:vec![account.into()],
                metadata:serde_json::json!({"relay_players":[{"database_account_id":account,"is_internal":false}]}),
                replay_status:ReplayVerificationStatus::Pending }).unwrap();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
            std::thread::scope(|scope| {
                for _ in 0..4 {
                    let db = &db;
                    let work = &work;
                    let barrier = barrier.clone();
                    scope.spawn(move || {
                        barrier.wait();
                        db.queue_match_work(work.clone()).unwrap();
                    });
                }
            });
            assert_eq!(db.pending_match_work(u64::MAX).unwrap().len(), 1);
            let mut deferred = db.match_work(&work.key()).unwrap().unwrap();
            deferred.retry_at = u64::MAX;
            db.save_match_work(&deferred).unwrap();
            assert!(db.pending_match_work(0).unwrap().is_empty());
            assert_eq!(
                db.pending_reward_calculations().unwrap(),
                (vec!["42".into()], Vec::new()),
                "deferred calculations must still block replacement"
            );

            let mut conflict = work.clone();
            if let MatchWork::Progress { sha256, .. } = &mut conflict {
                *sha256 = "b".repeat(64);
            }
            assert!(db.queue_match_work(conflict).is_err());
        }
        let db = super::PlayerDb::new(
            "redis://127.0.0.1/",
            None,
            Some(std::sync::Arc::new(
                crate::redb_store::init_database(&path).unwrap(),
            )),
        );
        assert_eq!(db.match_work(&work.key()).unwrap().unwrap().work, work);
        for id in 100..180 {
            db.save_match_start_record(MatchStartRecord { match_id:id.to_string(), player_ids:vec![account.into()],
                metadata:serde_json::json!({"relay_players":[{"database_account_id":account,"is_internal":false}]}), replay_status:ReplayVerificationStatus::Pending }).unwrap();
            db.queue_match_work(MatchWork::Progress {
                match_id: id.to_string(),
                sequence: 0,
                sha256: "c".repeat(64),
                closing: true,
            })
            .unwrap();
        }
        let pending = db.pending_match_work(u64::MAX).unwrap();
        assert_eq!(pending.len(), 33);
        assert!(
            pending
                .iter()
                .any(|job| matches!(job.work, MatchWork::Progress { closing: true, .. }))
        );
        assert!(pending.iter().any(|job| job.work == work));
        for turn in 0..80 {
            db.queue_match_work(MatchWork::Notify {
                command: crate::profile::MatchResultControl {
                    match_id: "42".into(),
                    terminal_turn: Some(turn),
                    participant_closures: Vec::new(),
                },
            })
            .unwrap();
        }
        let pending = db.pending_match_work(u64::MAX).unwrap();
        assert_eq!(
            pending
                .iter()
                .filter(|job| matches!(job.work, MatchWork::Notify { .. }))
                .count(),
            32
        );
        assert!(
            pending.iter().any(|job| job.work == work),
            "queued notifications must not displace simulation work"
        );
        let mut delivered = pending
            .iter()
            .find(|job| matches!(job.work, MatchWork::Notify { .. }))
            .unwrap()
            .clone();
        delivered.state = crate::profile::MatchExitState::Confirmed;
        db.save_match_work(&delivered).unwrap();
        assert_eq!(db.remove_confirmed_match_notifications().unwrap(), 1);
        assert!(db.match_work(&delivered.work.key()).unwrap().is_none());
        assert_eq!(
            db.queue_match_work(delivered.work.clone()).unwrap().state,
            crate::profile::MatchExitState::Requested
        );
        assert_eq!(
            db.pending_match_work(u64::MAX)
                .unwrap()
                .iter()
                .filter(|job| matches!(job.work, MatchWork::Notify { .. }))
                .count(),
            32
        );
        db.remove_match_work(&work.key()).unwrap();
        assert!(db.match_work(&work.key()).unwrap().is_none());
    }
    use super::{
        AccountKind, DISPLAY_NAME_MAX_CHARS, LeaderCareerStats, MatchRecord, PlayerAccount,
        PlayerDb, PlayerProfile, PurchaseRecord, VerifiedReceiptDecision, generated_display_name,
        is_valid_account_id, is_valid_gem_grant_request_id, normalize_display_name,
        should_skip_full_match_participant, validate_lobby_match_id, verified_receipt_decision,
        verified_victory_key,
    };

    fn test_account(kind: AccountKind) -> PlayerAccount {
        PlayerAccount {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            display_name: "Tester".to_string(),
            profile: PlayerProfile::default(),
            linked_identities: Vec::new(),
            kind,
            auth_secret_hash: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn ranked_bot_victory_updates_both_ratings_in_one_transaction() {
        use crate::profile::{
            MatchParticipantRecord, MatchStartRecord, ReplayVerificationStatus,
            VerifiedMatchParticipant, VerifiedMatchResult,
        };

        let temp = tempfile::tempdir().unwrap();
        let redb = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("ranked-bot-result.redb")).unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(redb));
        let mut human = test_account(AccountKind::Human);
        human.id = "0123456789abcdef0123456789abcdef".into();
        let mut bot = test_account(AccountKind::Bot);
        bot.id = "fedcba9876543210fedcba9876543210".into();
        bot.profile.ranked_bot = Some(super::RankedBotProfile {
            iq: 60,
            leader: "Caesar".into(),
            paused: false,
            reserved_match_id: Some("9100".into()),
            manual_changes: Vec::new(),
        });
        let start = MatchStartRecord {
            match_id: "9100".into(),
            player_ids: vec![human.id.clone(), bot.id.clone()],
            metadata: serde_json::json!({
                "kind": "Ranked1v1",
                "started_at": 1,
                "config": {"game_mode": "FFA", "map_name": "world", "ranked_bot_iq": 60}
            }),
            replay_status: ReplayVerificationStatus::Pending,
        };
        let mut record = super::provisional_match_record(&start, 10);
        record.participants = vec![
            MatchParticipantRecord {
                account_id: human.id.clone(),
                display_name: human.display_name.clone(),
                ..Default::default()
            },
            MatchParticipantRecord {
                account_id: bot.id.clone(),
                display_name: bot.display_name.clone(),
                is_bot: true,
                ..Default::default()
            },
        ];
        let result = VerifiedMatchResult {
            match_id: start.match_id.clone(),
            winner_player_id: Some(2),
            participants: vec![
                VerifiedMatchParticipant {
                    player_id: 1,
                    account_id: human.id.clone(),
                    ..Default::default()
                },
                VerifiedMatchParticipant {
                    player_id: 2,
                    account_id: bot.id.clone(),
                    won: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let txn = db.redb_db.as_ref().unwrap().begin_write().unwrap();
        super::settle_ranked_ratings(&txn, &mut record, &result, &[human, bot], 10).unwrap();
        txn.commit().unwrap();

        let read = db.redb_db.as_ref().unwrap().begin_read().unwrap();
        let table = read.open_table(super::SEASON_RATINGS_TABLE).unwrap();
        let human_key = format!(
            "rating:{}:{}:Ranked1v1:FFA",
            record.participants[0].account_id, record.season_id
        );
        let bot_key = format!(
            "rating:{}:{}:Ranked1v1:FFA",
            record.participants[1].account_id, record.season_id
        );
        let human_rating: crate::profile::SeasonRating =
            serde_json::from_slice(table.get(human_key.as_str()).unwrap().unwrap().value()).unwrap();
        let bot_rating: crate::profile::SeasonRating =
            serde_json::from_slice(table.get(bot_key.as_str()).unwrap().unwrap().value()).unwrap();
        assert!(record.rating_eligible);
        assert_eq!(record.participants[0].rating_delta, Some(0));
        assert!(record.participants[1].rating_delta.unwrap() > 0);
        for participant in &record.participants {
            let breakdown = participant.ranked_points_breakdown.as_ref().unwrap();
            assert_eq!(Some(breakdown.total()), participant.rating_delta);
        }
        assert_eq!(human_rating.games_played, 1);
        assert_eq!(bot_rating.games_played, 1);
        assert!(human_rating.mu < 1000.0);
        assert!(bot_rating.mu > 1000.0);
    }

    #[tokio::test]
    async fn ranked_bot_profile_stats_and_history_are_public_and_idempotent() {
        use crate::profile::{
            MatchStartRecord, ReplayVerificationStatus, VerifiedMatchParticipant,
            VerifiedMatchResult,
        };

        let temp = tempfile::tempdir().unwrap();
        let redb = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("ranked-bot-profile.redb")).unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(redb));
        let mut human = test_account(AccountKind::Human);
        human.id = "0123456789abcdef0123456789abcdef".into();
        let mut bot = test_account(AccountKind::Bot);
        bot.id = "fedcba9876543210fedcba9876543210".into();
        bot.display_name = "Ranked Bot IQ 60".into();
        bot.profile.ranked_bot = Some(super::RankedBotProfile {
            iq: 60,
            leader: "Caesar".into(),
            paused: false,
            reserved_match_id: Some("912345".into()),
            manual_changes: Vec::new(),
        });
        db.insert_account_sync(human.clone()).unwrap();
        db.insert_account_sync(bot.clone()).unwrap();

        let start = MatchStartRecord {
            match_id: "912345".into(),
            player_ids: vec![human.id.clone(), bot.id.clone()],
            metadata: serde_json::json!({
                "kind": "Ranked1v1",
                "started_at": 1,
                "config": {"game_mode": "FFA", "map_name": "world", "ranked_bot_iq": 60},
                "relay_players": [
                    {"player_id": 1, "database_account_id": human.id, "is_internal": false, "leader": "Caesar", "team": null},
                    {"player_id": 2, "database_account_id": bot.id, "is_internal": true, "leader": "Caesar", "team": null}
                ]
            }),
            replay_status: ReplayVerificationStatus::Pending,
        };
        db.save_match_start_record(start.clone()).unwrap();
        let result = VerifiedMatchResult {
            match_id: start.match_id,
            duration_seconds: 60,
            winner_player_id: Some(2),
            participants: vec![
                VerifiedMatchParticipant {
                    player_id: 1,
                    account_id: human.id.clone(),
                    leader: Some("Caesar".into()),
                    ..Default::default()
                },
                VerifiedMatchParticipant {
                    player_id: 2,
                    account_id: bot.id.clone(),
                    leader: Some("Caesar".into()),
                    won: true,
                    kills: 4,
                    deaths: 1,
                    assists: 2,
                    players_defeated: 1,
                    tribes_defeated: 3,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        db.apply_verified_match_result(&result, None).await.unwrap();
        let profile = db.public_profile(&bot.id).await.unwrap().unwrap();
        assert!(profile.is_bot);
        assert_eq!(profile.ranked_bot_iq, Some(60));
        assert_eq!(profile.matches_played, 1);
        assert_eq!(profile.wins, 1);
        assert_eq!(profile.kills, 4);
        assert_eq!(profile.deaths, 1);
        assert_eq!(profile.assists, 2);
        assert_eq!(profile.tribes_defeated, 3);
        assert_eq!(profile.recent_matches.len(), 1);
        assert_eq!(profile.recent_matches[0].queue, "Ranked1v1");
        assert_eq!(
            db.public_match_history(&bot.id, 0, 20, None, None)
                .await
                .unwrap()
                .unwrap()
                .len(),
            1
        );
        assert!(db
            .public_match_history(&bot.id, 0, 20, Some("Matchmaking"), None)
            .await
            .unwrap()
            .unwrap()
            .is_empty());

        db.apply_verified_match_result(&result, None).await.unwrap();
        let saved_bot = db.load_account(&bot.id).await.unwrap();
        assert_eq!(saved_bot.profile.matches_played, 1);
        assert_eq!(saved_bot.profile.verified_wins, 1);
        assert_eq!(saved_bot.profile.kills, 4);
    }

    #[tokio::test]
    async fn verified_closure_batch_is_atomic_idempotent_and_rejects_changed_results() {
        use crate::profile::{
            MatchResultControl, MatchStartRecord, ProgressClosure, ReplayVerificationStatus,
            VerifiedMatchParticipant, VerifiedMatchResult, VerifiedParticipantClosure,
        };
        let temp = tempfile::tempdir().unwrap();
        let metadata = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("batched-rewards.redb")).unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata));
        let alice = "0123456789abcdef0123456789abcdef";
        let bob = "fedcba9876543210fedcba9876543210";
        for account_id in [alice, bob] {
            let mut account = test_account(AccountKind::Human);
            account.id = account_id.to_string();
            db.insert_account_sync(account).unwrap();
        }
        db.save_match_start_record(MatchStartRecord {
            match_id: "4201".into(),
            player_ids: vec![alice.into(), bob.into()],
            metadata: serde_json::json!({
                "kind": "Custom",
                "started_at": 100,
                "config": {"game_mode": "FFA", "map_name": "world"},
                "relay_players": [
                    {"player_id": 1, "database_account_id": alice, "is_internal": false, "leader": "Caesar", "team": null},
                    {"player_id": 2, "database_account_id": bob, "is_internal": false, "leader": "Caesar", "team": null}
                ]
            }),
            replay_status: ReplayVerificationStatus::Pending,
        })
        .unwrap();

        let closures = vec![
            VerifiedParticipantClosure {
                account_id: alice.into(),
                reason: "voluntary".into(),
                result: VerifiedMatchResult {
                    match_id: "4201".into(),
                    duration_seconds: 3600,
                    winner_player_id: Some(1),
                    personal_closure_turn: Some(30),
                    participants: vec![VerifiedMatchParticipant {
                        player_id: 1,
                        account_id: alice.into(),
                        leader: Some("Caesar".into()),
                        won: true,
                        kills: 3,
                        players_defeated: 1,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            },
            VerifiedParticipantClosure {
                account_id: bob.into(),
                reason: "timeout".into(),
                result: VerifiedMatchResult {
                    match_id: "4201".into(),
                    duration_seconds: 3600,
                    winner_player_id: Some(1),
                    personal_closure_turn: Some(31),
                    participants: vec![VerifiedMatchParticipant {
                        player_id: 2,
                        account_id: bob.into(),
                        leader: Some("Caesar".into()),
                        kills: 1,
                        deaths: 1,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            },
        ];
        let notification = MatchResultControl {
            match_id: "4201".into(),
            terminal_turn: Some(31),
            participant_closures: vec![
                ProgressClosure {
                    account_id: alice.into(),
                    turn_number: 30,
                    reason: "voluntary".into(),
                },
                ProgressClosure {
                    account_id: bob.into(),
                    turn_number: 31,
                    reason: "timeout".into(),
                },
            ],
        };
        let first = db
            .apply_verified_match_participant_results(&closures, Some(notification.clone()))
            .await
            .unwrap();
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(Option::is_some));

        let snapshot = |account_id: &str| {
            let account = db.load_account_sync(account_id).unwrap();
            (
                account.profile.xp,
                account.profile.crowns,
                account.profile.laurels,
                account.profile.matches_played,
                account.profile.kills,
                account.profile.wins,
                account.profile.reward_receipts.clone(),
            )
        };
        let alice_after_first = snapshot(alice);
        let bob_after_first = snapshot(bob);
        assert_eq!(alice_after_first.3, 1);
        assert_eq!(alice_after_first.5, 1);
        assert_eq!(bob_after_first.3, 1);
        assert_eq!(bob_after_first.5, 0);
        let pending = db.pending_match_work(u64::MAX).unwrap();
        let notifications = pending
            .iter()
            .filter_map(|job| match &job.work {
                crate::profile::MatchWork::Notify { command } => Some(command),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0], &notification);
        for (account_id, turn) in [(alice, 30), (bob, 31)] {
            let account = db.load_account_sync(account_id).unwrap();
            let participation = account.profile.reward_receipts.get("4201").unwrap();
            let bonus = account
                .profile
                .reward_receipts
                .get("verified_bonus:4201")
                .unwrap();
            assert_eq!(participation.personal_closure_turn, Some(turn));
            assert_eq!(
                participation.verification_status,
                Some(ReplayVerificationStatus::Verified)
            );
            assert!(bonus.verified_result_sha256.is_some());
        }

        db.apply_verified_match_participant_results(&closures, Some(notification.clone()))
            .await
            .unwrap();
        assert_eq!(snapshot(alice), alice_after_first);
        assert_eq!(snapshot(bob), bob_after_first);

        let mut conflict = closures.clone();
        conflict[1].result.participants[0].kills = 9;
        assert!(
            db.apply_verified_match_participant_results(&conflict, Some(notification))
                .await
                .is_err()
        );
        assert_eq!(snapshot(alice), alice_after_first);
        assert_eq!(snapshot(bob), bob_after_first);
        let record = db.load_match_record("4201").unwrap().unwrap();
        assert_eq!(record.participants.len(), 2);
        assert!(record.provisional);

        db.save_match_start_record(MatchStartRecord {
            match_id: "4202".into(),
            player_ids: vec![alice.into(), bob.into()],
            metadata: serde_json::json!({
                "kind": "Custom",
                "started_at": 100,
                "config": {"game_mode": "FFA", "map_name": "world"},
                "relay_players": [
                    {"player_id": 1, "database_account_id": alice, "is_internal": false, "leader": "Caesar", "team": null},
                    {"player_id": 2, "database_account_id": bob, "is_internal": false, "leader": "Caesar", "team": null}
                ]
            }),
            replay_status: ReplayVerificationStatus::Pending,
        })
        .unwrap();
        let final_result = VerifiedMatchResult {
            match_id: "4202".into(),
            duration_seconds: 3600,
            winner_player_id: Some(1),
            participants: vec![
                VerifiedMatchParticipant {
                    player_id: 1,
                    account_id: alice.into(),
                    leader: Some("Caesar".into()),
                    won: true,
                    kills: 4,
                    players_defeated: 1,
                    ..Default::default()
                },
                VerifiedMatchParticipant {
                    player_id: 2,
                    account_id: bob.into(),
                    leader: Some("Caesar".into()),
                    deaths: 1,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let final_notification = MatchResultControl {
            match_id: "4202".into(),
            terminal_turn: Some(40),
            participant_closures: Vec::new(),
        };
        let (final_outcomes, _) = db
            .apply_verified_match_result(&final_result, Some(final_notification.clone()))
            .await
            .unwrap();
        assert_eq!(final_outcomes.len(), 2);
        for account_id in [alice, bob] {
            let account = db.load_account_sync(account_id).unwrap();
            assert!(account.profile.reward_receipts.contains_key("4202"));
            assert!(
                account
                    .profile
                    .reward_receipts
                    .get("verified_bonus:4202")
                    .unwrap()
                    .verified_result_sha256
                    .is_some()
            );
        }
        let final_record = db.load_match_record("4202").unwrap().unwrap();
        assert!(final_record.verified);
        assert!(!final_record.provisional);
        let after_final_alice = snapshot(alice);
        let after_final_bob = snapshot(bob);
        let final_pending = db.pending_match_work(u64::MAX).unwrap();
        assert_eq!(
            final_pending
                .iter()
                .filter(|job| matches!(&job.work, crate::profile::MatchWork::Notify { .. }))
                .count(),
            2
        );
        db.apply_verified_match_result(&final_result, Some(final_notification))
            .await
            .unwrap();
        assert_eq!(snapshot(alice), after_final_alice);
        assert_eq!(snapshot(bob), after_final_bob);
    }

    #[tokio::test]
    async fn one_match_closure_batch_settles_128_accounts_once() {
        use crate::profile::{
            MatchResultControl, MatchStartRecord, ProgressClosure, ReplayVerificationStatus,
            VerifiedMatchParticipant, VerifiedMatchResult, VerifiedParticipantClosure,
        };
        const PARTICIPANTS: usize = 128;
        let temp = tempfile::tempdir().unwrap();
        let metadata = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("128-participant-rewards.redb"))
                .unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata));
        let account_ids = (1..=PARTICIPANTS)
            .map(|id| format!("{id:032x}"))
            .collect::<Vec<_>>();
        for account_id in &account_ids {
            let mut account = test_account(AccountKind::Human);
            account.id = account_id.clone();
            db.insert_account_sync(account).unwrap();
        }
        let roster = account_ids
            .iter()
            .enumerate()
            .map(|(index, account_id)| {
                serde_json::json!({
                    "player_id": index + 1,
                    "database_account_id": account_id,
                    "is_internal": false,
                    "leader": "Caesar",
                    "team": null
                })
            })
            .collect::<Vec<_>>();
        db.save_match_start_record(MatchStartRecord {
            match_id: "4300".into(),
            player_ids: account_ids.clone(),
            metadata: serde_json::json!({
                "kind": "Custom",
                "started_at": 100,
                "config": {"game_mode": "FFA", "map_name": "world"},
                "relay_players": roster
            }),
            replay_status: ReplayVerificationStatus::Pending,
        })
        .unwrap();

        let closures = account_ids
            .iter()
            .enumerate()
            .map(|(index, account_id)| VerifiedParticipantClosure {
                account_id: account_id.clone(),
                reason: if index == 0 {
                    "match_ended"
                } else {
                    "defeated"
                }
                .into(),
                result: VerifiedMatchResult {
                    match_id: "4300".into(),
                    duration_seconds: 7200,
                    winner_player_id: Some(1),
                    personal_closure_turn: Some(1000 + index as u64),
                    participants: vec![VerifiedMatchParticipant {
                        player_id: index as u16 + 1,
                        account_id: account_id.clone(),
                        leader: Some("Caesar".into()),
                        won: index == 0,
                        kills: (index % 4) as u32,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            })
            .collect::<Vec<_>>();
        let notification = MatchResultControl {
            match_id: "4300".into(),
            terminal_turn: Some(1127),
            participant_closures: closures
                .iter()
                .map(|closure| ProgressClosure {
                    account_id: closure.account_id.clone(),
                    turn_number: closure.result.personal_closure_turn.unwrap(),
                    reason: closure.reason.clone(),
                })
                .collect(),
        };
        let applied = db
            .apply_verified_match_participant_results(&closures, Some(notification.clone()))
            .await
            .unwrap();
        assert_eq!(applied.len(), PARTICIPANTS);
        assert!(applied.iter().all(Option::is_some));
        let balances = account_ids
            .iter()
            .map(|account_id| {
                let account = db.load_account_sync(account_id).unwrap();
                assert_eq!(account.profile.matches_played, 1);
                assert_eq!(
                    account
                        .profile
                        .reward_receipts
                        .get("4300")
                        .unwrap()
                        .verification_status,
                    Some(ReplayVerificationStatus::Verified)
                );
                (
                    account.profile.xp,
                    account.profile.crowns,
                    account.profile.laurels,
                    account.profile.reward_receipts,
                )
            })
            .collect::<Vec<_>>();
        db.apply_verified_match_participant_results(&closures, Some(notification))
            .await
            .unwrap();
        for (account_id, before) in account_ids.iter().zip(balances) {
            let account = db.load_account_sync(account_id).unwrap();
            assert_eq!(account.profile.matches_played, 1);
            assert_eq!(account.profile.xp, before.0);
            assert_eq!(account.profile.crowns, before.1);
            assert_eq!(account.profile.laurels, before.2);
            assert_eq!(account.profile.reward_receipts, before.3);
        }
    }

    #[test]
    fn achievement_views_expose_nine_categorized_server_achievements() {
        let mut profile = PlayerProfile::default();
        profile.matches_played = 1;
        let views = profile.achievement_views();
        assert_eq!(views.len(), 9);
        assert_eq!(
            views
                .iter()
                .map(|view| view.category.as_str())
                .collect::<Vec<_>>(),
            [
                "battles",
                "victories",
                "battles",
                "victories",
                "laurels",
                "victories",
                "commanders",
                "commanders",
                "commanders",
            ]
        );
        assert_eq!(views[0].progress, 1);
        assert!(!views[0].unlocked);
    }

    #[test]
    fn external_score_pending_is_saved_with_the_reward_and_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("scores.redb");
        let mut account = test_account(AccountKind::Human);
        account.linked_identities.push(super::LinkedIdentity {
            provider: "crazygames".into(),
            external_id: "audit-platform-player".into(),
            environment: "production".into(),
        });
        account.profile.playgames_sync_revision = 3;
        {
            let db = super::PlayerDb::new(
                "redis://127.0.0.1/",
                None,
                Some(std::sync::Arc::new(
                    crate::redb_store::init_database(&path).unwrap(),
                )),
            );
            assert!(db.insert_account_sync(account.clone()).unwrap().1);
            assert_eq!(
                db.pending_crazygames_scores().unwrap(),
                vec![account.id.clone()]
            );
        }
        let db = super::PlayerDb::new(
            "redis://127.0.0.1/",
            None,
            Some(std::sync::Arc::new(
                crate::redb_store::init_database(&path).unwrap(),
            )),
        );
        assert_eq!(
            db.pending_crazygames_scores().unwrap(),
            vec![account.id.clone()]
        );
        account.profile.crazygames_synced_revision = 2;
        db.update_account_atomic_sync(&account.id, &[], |stored| {
            stored.profile.crazygames_synced_revision = 2;
        })
        .unwrap();
        assert_eq!(
            db.pending_crazygames_scores().unwrap(),
            vec![account.id.clone()],
            "an older platform acknowledgment cannot clear newer progress"
        );
        account.profile.crazygames_synced_revision = 3;
        db.update_account_atomic_sync(&account.id, &[], |stored| {
            stored.profile.crazygames_synced_revision = 3;
        })
        .unwrap();
        assert!(db.pending_crazygames_scores().unwrap().is_empty());
    }

    #[test]
    fn validates_canonical_account_ids() {
        assert!(is_valid_account_id("0123456789abcdef0123456789abcdef"));
        assert!(is_valid_account_id("52948534-1c9f-4def-bc8e-80390d3287a6"));
        // The removed guest_<hex> format is intentionally rejected; account
        // IDs use either compact hex or the dashed UUID form.
        assert!(!is_valid_account_id(
            "guest_0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_valid_account_id("not-an-account"));
    }

    #[test]
    fn profile_match_index_is_account_scoped_chronological_and_updates_one_provisional_row() {
        let temp = tempfile::tempdir().unwrap();
        let metadata = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("profile.redb")).unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata));
        let alice = "0123456789abcdef0123456789abcdef";
        let bob = "fedcba9876543210fedcba9876543210";
        let make_record = |id: &str, at: u64, accounts: &[&str], provisional: bool| MatchRecord {
            schema_version: 1,
            match_id: id.to_string(),
            season_id: 1,
            queue: "Matchmaking".to_string(),
            mode: "FFA".to_string(),
            map_name: "world".to_string(),
            started_at: at.saturating_sub(60),
            completed_at: at,
            duration_seconds: 60,
            winner_account_id: None,
            winning_team: None,
            verified: !provisional,
            provisional,
            rating_eligible: false,
            ranked_bot_iq: None,
            participants: accounts
                .iter()
                .map(|account_id| crate::profile::MatchParticipantRecord {
                    account_id: (*account_id).to_string(),
                    display_name: (*account_id).to_string(),
                    ..Default::default()
                })
                .collect(),
        };

        db.save_provisional_match_record(&make_record("m1", 100, &[alice], true))
            .unwrap();
        db.save_provisional_match_record(&make_record("m2", 200, &[bob], true))
            .unwrap();
        db.save_provisional_match_record(&make_record("m3", 300, &[alice], true))
            .unwrap();
        db.save_provisional_match_record(&make_record("m1", 400, &[alice, bob], true))
            .unwrap();

        let alice_history = db
            .match_history_for_account(alice, 0, 10, None, None)
            .unwrap();
        assert_eq!(
            alice_history
                .iter()
                .map(|record| record.match_id.as_str())
                .collect::<Vec<_>>(),
            ["m1", "m3"]
        );
        let bob_history = db
            .match_history_for_account(bob, 0, 10, None, None)
            .unwrap();
        assert_eq!(
            bob_history
                .iter()
                .map(|record| record.match_id.as_str())
                .collect::<Vec<_>>(),
            ["m1", "m2"]
        );

        db.save_match_record(&make_record("m1", 500, &[alice, bob], false))
            .unwrap();
        db.save_provisional_match_record(&make_record("m1", 600, &[alice], true))
            .unwrap();
        let saved = db.load_match_record("m1").unwrap().unwrap();
        assert!(!saved.provisional);
        assert!(saved.verified);
        assert_eq!(saved.completed_at, 500);
        let history = db
            .match_history_for_account(alice, 0, 10, None, None)
            .unwrap();
        assert_eq!(
            history
                .iter()
                .filter(|record| record.match_id == "m1")
                .count(),
            1
        );
        assert_eq!(history[0].match_id, "m1");
    }

    #[tokio::test]
    async fn verified_victory_leaderboard_pages_keep_order_and_global_ranks() {
        let temp = tempfile::tempdir().unwrap();
        let metadata = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("profile.redb")).unwrap(),
        );
        let write_txn = metadata.begin_write().unwrap();
        {
            let mut table = write_txn
                .open_table(crate::redb_store::VERIFIED_VICTORY_BOARD_TABLE)
                .unwrap();
            for (account_id, wins) in [("alice", 20), ("bob", 10), ("carol", 3)] {
                let entry = crate::profile::PublicVictoryLeaderboardEntry {
                    rank: 0,
                    account_id: account_id.to_string(),
                    handle: account_id.to_string(),
                    level: 1,
                    matches_played: wins,
                    wins,
                };
                let json = serde_json::to_vec(&entry).unwrap();
                let key = verified_victory_key(wins, account_id);
                table.insert(key.as_str(), json.as_slice()).unwrap();
            }
        }
        write_txn.commit().unwrap();

        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata));
        let first = db.public_victory_leaderboard(0, 2).await.unwrap();
        let second = db.public_victory_leaderboard(2, 2).await.unwrap();
        assert_eq!(
            first.iter().map(|entry| entry.wins).collect::<Vec<_>>(),
            [20, 10]
        );
        assert_eq!(
            first.iter().map(|entry| entry.rank).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(
            second.iter().map(|entry| entry.wins).collect::<Vec<_>>(),
            [3]
        );
        assert_eq!(second[0].rank, 3);
    }

    #[test]
    fn starter_gems_top_up_legacy_accounts_once_and_skip_bots() {
        let mut existing = test_account(AccountKind::Human);
        existing.profile.gems = 25;
        for id in ["welcome_gems_v1", "anonymous_welcome_gems_v1"] {
            existing.profile.purchase_history.insert(
                id.to_string(),
                PurchaseRecord {
                    id: id.to_string(),
                    provider: "system".to_string(),
                    environment: "legacy".to_string(),
                    product_id: "legacy".to_string(),
                    transaction_id: None,
                    status: "granted".to_string(),
                    acquired_at: 0,
                    updated_at: 0,
                },
            );
        }
        assert!(PlayerDb::apply_welcome_grant(&mut existing, 1).unwrap());
        assert_eq!(existing.profile.gems, 500);
        assert!(!PlayerDb::apply_welcome_grant(&mut existing, 2).unwrap());
        assert_eq!(existing.profile.gems, 500);

        let mut fresh = test_account(AccountKind::Human);
        assert!(PlayerDb::apply_welcome_grant(&mut fresh, 1).unwrap());
        assert_eq!(fresh.profile.gems, 475);

        let mut bot = test_account(AccountKind::Bot);
        assert!(!PlayerDb::apply_welcome_grant(&mut bot, 1).unwrap());
        assert_eq!(bot.profile.gems, 0);
    }

    #[test]
    fn manual_gem_grants_are_audited_and_idempotent() {
        let request_id = "abcdef0123456789abcdef0123456789";
        assert!(is_valid_gem_grant_request_id(request_id));
        assert!(!is_valid_gem_grant_request_id("not-a-request"));

        let mut account = test_account(AccountKind::Human);
        assert!(PlayerDb::apply_manual_gem_grant(&mut account, 475, request_id, 1).unwrap());
        assert_eq!(account.profile.gems, 475);
        assert!(!PlayerDb::apply_manual_gem_grant(&mut account, 475, request_id, 2).unwrap());
        assert_eq!(account.profile.gems, 475);
        assert!(PlayerDb::apply_manual_gem_grant(&mut account, 450, request_id, 3).is_err());
        assert_eq!(account.profile.gems, 475);
        assert_eq!(
            account
                .profile
                .purchase_history
                .values()
                .next()
                .unwrap()
                .provider,
            "operator"
        );
        assert!(PlayerDb::apply_manual_gem_grant(&mut account, 0, request_id, 4).is_err());

        let mut bot = test_account(AccountKind::Bot);
        assert!(PlayerDb::apply_manual_gem_grant(&mut bot, 475, request_id, 1).is_err());
        assert_eq!(bot.profile.gems, 0);
    }

    #[tokio::test]
    async fn revenuecat_refunds_are_deduplicated_by_transaction_in_redb() {
        let temp = tempfile::tempdir().unwrap();
        let metadata = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("revenuecat-refund.redb")).unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata));
        let mut account = test_account(AccountKind::Human);
        account.profile.gems = 1000;
        let account_id = account.id.clone();
        db.insert_account_sync(account).unwrap();

        let first = db
            .revoke_revenuecat_product(
                "refund-event-one",
                &account_id,
                "sow_gems_500",
                Some("store-transaction-one"),
                "PRODUCTION",
            )
            .await
            .unwrap();
        assert!(first.1);
        assert_eq!(first.0.profile.gems, 500);

        let retry = db
            .revoke_revenuecat_product(
                "refund-event-retry",
                &account_id,
                "sow_gems_500",
                Some("store-transaction-one"),
                "PRODUCTION",
            )
            .await
            .unwrap();
        assert!(!retry.1);
        assert_eq!(retry.0.profile.gems, 500);
        assert_eq!(
            retry.0.profile.purchase_history["transaction:store-transaction-one"].status,
            "revoked"
        );
    }

    #[test]
    fn normalizes_display_names_without_using_them_as_ids() {
        assert_eq!(normalize_display_name("  Alice  ").unwrap(), "Alice");
        assert_eq!(
            normalize_display_name(&"x".repeat(DISPLAY_NAME_MAX_CHARS + 4))
                .unwrap()
                .chars()
                .count(),
            DISPLAY_NAME_MAX_CHARS
        );
        assert!(normalize_display_name("\n\t").is_err());
        assert!(generated_display_name().starts_with("ANON"));
    }

    #[test]
    fn missing_display_name_defaults_cleanly() {
        let json = r#"{
            "account_id":"0123456789abcdef0123456789abcdef",
            "profile":{"xp":0,"level":1,"wins":0,"matches_played":0,
              "players_defeated":0,"empires_defeated":0,"tribes_defeated":0,
              "preferred_leader":null},
            "linked_identities":[],"created_at":0,"updated_at":0
        }"#;
        let account: super::PlayerAccount = serde_json::from_str(json).unwrap();
        assert!(account.display_name.is_empty());
        assert!(account.profile.leader_xp.is_empty());
        assert_eq!(account.profile.crowns, 0);
        assert_eq!(account.profile.laurels, 0);
        assert!(!account.profile.intro_completed);
    }

    #[test]
    fn legacy_account_id_field_migrates_from_id() {
        let json = r#"{
            "id":"0123456789abcdef0123456789abcdef",
            "profile":{"xp":0,"level":1,"wins":0,"matches_played":0,
              "players_defeated":0,"empires_defeated":0,"tribes_defeated":0,
              "preferred_leader":null},
            "linked_identities":[],"created_at":0,"updated_at":0
        }"#;
        let account = super::parse_account_with_migration(json.as_bytes()).unwrap();
        assert_eq!(account.id, "0123456789abcdef0123456789abcdef");
    }

    #[test]
    fn currency_balances_use_their_canonical_keys() {
        let mut profile = super::PlayerProfile::default();
        profile.crowns = 725;
        profile.laurels = 40;
        let value = serde_json::to_value(&profile).unwrap();
        assert_eq!(value["crowns"], 725);
        assert_eq!(value["laurels"], 40);
    }

    #[test]
    fn achievements_are_server_owned_and_awarded_once() {
        let mut profile = PlayerProfile {
            matches_played: 1,
            wins: 1,
            verified_wins: 1,
            ..Default::default()
        };
        profile.leader_stats.insert(
            "Caesar".to_string(),
            LeaderCareerStats {
                matches_played: 1,
                wins: 1,
                verified_wins: 1,
                ..Default::default()
            },
        );

        profile.refresh_achievements();
        assert!(profile.unlocked_achievements.contains("first_command"));
        assert!(profile.unlocked_achievements.contains("first_victory"));
        assert!(
            profile
                .unlocked_achievements
                .contains("commander_victorious")
        );
        assert_eq!(profile.laurels, 60);

        let laurels = profile.laurels;
        profile.refresh_achievements();
        assert_eq!(profile.laurels, laurels);
    }

    #[test]
    fn reward_receipt_ledger_pays_a_match_only_once() {
        let mut profile = PlayerProfile::default();
        let reward = crate::rewards::MatchReward {
            xp: 25,
            leader_xp: 25,
            crowns: 10,
            laurels: 0,
        };
        assert!(profile.apply_reward_receipt("match-7", "match-7", "Boudica", reward, 1));
        let balance = (
            profile.xp,
            profile.crowns,
            profile.reward_receipts.len(),
            profile.playgames_sync_revision,
        );
        assert!(!profile.apply_reward_receipt("match-7", "match-7", "Boudica", reward, 2));
        assert_eq!(
            (
                profile.xp,
                profile.crowns,
                profile.reward_receipts.len(),
                profile.playgames_sync_revision,
            ),
            balance
        );
        assert_eq!(profile.playgames_sync_revision, 1);
    }

    #[test]
    fn multiplayer_participation_closes_welcome_without_enrolling_another_guide() {
        use crate::profile::{MenuGuideProgress, MenuGuideStatus};
        for status in [
            MenuGuideStatus::Active,
            MenuGuideStatus::Completed,
            MenuGuideStatus::Skipped,
        ] {
            let mut account = test_account(AccountKind::Human);
            account.profile.menu_guides.insert(
                "welcome".to_string(),
                MenuGuideProgress {
                    status,
                    ..Default::default()
                },
            );
            super::apply_match_participation_to_account(
                &mut account,
                Some("Boudica"),
                "first-match",
                1,
            );
            assert!(
                !account
                    .profile
                    .menu_guides
                    .contains_key("first_multiplayer")
            );
            assert_eq!(
                account.profile.menu_guides["welcome"].status,
                if status == MenuGuideStatus::Active {
                    MenuGuideStatus::Skipped
                } else {
                    status
                }
            );
            super::apply_match_participation_to_account(
                &mut account,
                Some("Boudica"),
                "first-match",
                2,
            );
            assert_eq!(account.profile.matches_played, 1);
        }
    }

    #[test]
    fn match_id_seed_survives_restart_and_uses_numeric_legacy_ids() {
        let temp = tempfile::tempdir().unwrap();
        let metadata = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("profile.redb")).unwrap(),
        );
        let replay_directory = temp.path().join("replays");
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata.clone()));
        assert_eq!(db.next_match_id_seed_in(&replay_directory).unwrap(), 1);
        let txn = metadata.begin_write().unwrap();
        {
            let mut starts = txn
                .open_table(crate::redb_store::MATCH_STARTS_TABLE)
                .unwrap();
            for id in ["9", "100"] {
                starts.insert(id, b"{}".as_slice()).unwrap();
            }
            let mut history = txn.open_table(crate::redb_store::MATCHES_TABLE).unwrap();
            history.insert("9000", b"{}".as_slice()).unwrap();
            history
                .insert("legacy-nonnumeric", b"{}".as_slice())
                .unwrap();
        }
        txn.commit().unwrap();
        let restarted = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata));
        assert_eq!(
            restarted.next_match_id_seed_in(&replay_directory).unwrap(),
            9001
        );
    }

    #[test]
    fn match_id_seed_recovers_from_replay_files_and_survives_redb_restore() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("replays");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("44.replay"), b"old replay").unwrap();
        std::fs::write(directory.join("9002.progress.00000001.pending"), b"pending").unwrap();
        std::fs::write(directory.join("9003-account.closure.7.paid"), b"paid").unwrap();
        std::fs::write(directory.join("not-a-match.replay"), b"ignored").unwrap();
        std::fs::create_dir(directory.join("9100.replay")).unwrap();

        assert_eq!(
            super::recovered_match_id_seed(10, &directory).unwrap(),
            9004
        );
        for entry in std::fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() && entry.file_name() != ".match-id-high-water" {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }
        std::fs::remove_dir(directory.join("9100.replay")).unwrap();

        assert_eq!(
            super::recovered_match_id_seed(10, &directory).unwrap(),
            9004
        );
    }

    #[test]
    fn full_match_save_preserves_personally_closed_participant_record() {
        let temp = tempfile::tempdir().unwrap();
        let metadata = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("profile.redb")).unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1:6379/0", None, Some(metadata));
        let alice = "0123456789abcdef0123456789abcdef";
        let bob = "fedcba9876543210fedcba9876543210";
        let mut closed = MatchRecord {
            schema_version: 1,
            match_id: "m-closed".to_string(),
            season_id: 1,
            queue: "Matchmaking".to_string(),
            mode: "FFA".to_string(),
            map_name: "world".to_string(),
            started_at: 10,
            completed_at: 20,
            duration_seconds: 10,
            winner_account_id: None,
            winning_team: None,
            verified: false,
            provisional: true,
            rating_eligible: false,
            ranked_bot_iq: None,
            participants: vec![
                crate::profile::MatchParticipantRecord {
                    account_id: alice.to_string(),
                    xp: 73,
                    crowns: 41,
                    personal_closure_turn: Some(8),
                    ..Default::default()
                },
                crate::profile::MatchParticipantRecord {
                    account_id: bob.to_string(),
                    ..Default::default()
                },
            ],
        };
        db.save_provisional_match_record(&closed).unwrap();

        closed.provisional = false;
        closed.verified = true;
        closed.completed_at = 30;
        closed.duration_seconds = 20;
        closed.participants[0].xp = 0;
        closed.participants[0].crowns = 0;
        closed.participants[0].personal_closure_turn = None;
        closed.participants[1].xp = 55;
        db.save_match_record(&closed).unwrap();

        let saved = db.load_match_record("m-closed").unwrap().unwrap();
        let alice_saved = saved
            .participants
            .iter()
            .find(|participant| participant.account_id == alice)
            .unwrap();
        assert_eq!(alice_saved.xp, 73);
        assert_eq!(alice_saved.crowns, 41);
        assert_eq!(alice_saved.personal_closure_turn, Some(8));
        assert_eq!(
            saved
                .participants
                .iter()
                .find(|participant| participant.account_id == bob)
                .unwrap()
                .xp,
            55
        );
    }

    #[test]
    fn full_match_verification_cannot_repay_a_closed_player() {
        let receipt = crate::profile::RewardReceipt {
            personal_closure_turn: Some(8),
            ..Default::default()
        };
        assert_eq!(
            verified_receipt_decision(Some(&receipt), None),
            VerifiedReceiptDecision::AlreadyClosed
        );
        assert_eq!(
            verified_receipt_decision(Some(&receipt), Some(8)),
            VerifiedReceiptDecision::Apply
        );
        let visible_verified = crate::profile::RewardReceipt {
            verification_status: Some(crate::profile::ReplayVerificationStatus::Verified),
            ..receipt.clone()
        };
        assert_eq!(
            verified_receipt_decision(Some(&visible_verified), Some(8)),
            VerifiedReceiptDecision::Apply,
            "retry must finish checked storage even when the live receipt is already verified"
        );
        assert_eq!(
            verified_receipt_decision(Some(&receipt), Some(9)),
            VerifiedReceiptDecision::Reject
        );
        assert!(should_skip_full_match_participant(
            Some(&receipt),
            VerifiedReceiptDecision::AlreadyClosed
        ));
        let rejected_closed = crate::profile::RewardReceipt {
            personal_closure_turn: Some(8),
            verification_status: Some(crate::profile::ReplayVerificationStatus::Rejected),
            ..Default::default()
        };
        assert!(should_skip_full_match_participant(
            Some(&rejected_closed),
            verified_receipt_decision(Some(&rejected_closed), None)
        ));
        assert_eq!(
            verified_receipt_decision(None, None),
            VerifiedReceiptDecision::Reject
        );
    }

    #[test]
    fn relay_match_metadata_must_name_the_registered_match() {
        let lobby = serde_json::json!({"lobby_id": 41}).to_string();
        assert!(validate_lobby_match_id("41", &lobby).is_ok());
        assert!(validate_lobby_match_id("42", &lobby).is_err());
    }

    #[test]
    fn laurel_hoard_can_unlock_from_the_same_server_update() {
        let mut profile = PlayerProfile {
            matches_played: 10,
            wins: 10,
            verified_wins: 10,
            ..Default::default()
        };
        profile.leader_stats.insert(
            "Caesar".to_string(),
            LeaderCareerStats {
                matches_played: 10,
                wins: 10,
                verified_wins: 10,
                ..Default::default()
            },
        );

        profile.refresh_achievements();
        assert!(profile.unlocked_achievements.contains("laurel_hoard"));
        assert!(profile.laurels >= 100);
    }

    #[test]
    fn legacy_account_laurels_balance_migrates_to_crowns_on_parse() {
        // Pre-split stored account: profile.laurels holds the spendable balance.
        let legacy = r#"{
            "account_id":"0123456789abcdef0123456789abcdef",
            "profile":{"xp":0,"level":1,"wins":0,"matches_played":0,
              "players_defeated":0,"empires_defeated":0,"tribes_defeated":0,
              "preferred_leader":null,"laurels":725},
            "linked_identities":[],"created_at":0,"updated_at":0
        }"#;
        let account = super::parse_account_with_migration(legacy.as_bytes()).unwrap();
        assert_eq!(
            account.profile.crowns, 725,
            "legacy balance must become crowns"
        );
        assert_eq!(
            account.profile.laurels, 0,
            "points must not inherit the legacy balance"
        );

        // Post-split account: both balances survive untouched.
        let current = r#"{
            "account_id":"0123456789abcdef0123456789abcdef",
            "profile":{"xp":0,"level":1,"wins":0,"matches_played":0,
              "players_defeated":0,"empires_defeated":0,"tribes_defeated":0,
              "preferred_leader":null,"crowns":100,"laurels":40},
            "linked_identities":[],"created_at":0,"updated_at":0
        }"#;
        let account = super::parse_account_with_migration(current.as_bytes()).unwrap();
        assert_eq!(account.profile.crowns, 100);
        assert_eq!(account.profile.laurels, 40);
    }

    #[test]
    fn public_account_projection_does_not_serialize_secret_hash() {
        let json = r#"{
            "account_id":"0123456789abcdef0123456789abcdef",
            "profile":{"xp":0,"level":1,"wins":0,"matches_played":0,
              "players_defeated":0,"empires_defeated":0,"tribes_defeated":0,
              "preferred_leader":null},
            "linked_identities":[],"auth_secret_hash":"private","created_at":0,"updated_at":0
        }"#;
        let mut account: super::PlayerAccount = serde_json::from_str(json).unwrap();
        account.profile.reward_receipts.insert(
            "match-1".into(),
            crate::profile::RewardReceipt {
                verified_result_sha256: Some("private-result-digest".into()),
                ..Default::default()
            },
        );
        let public = serde_json::to_value(account.without_auth_secret()).unwrap();
        assert!(public.get("auth_secret_hash").is_none());
        assert!(
            public
                .pointer("/profile/reward_receipts/match-1/verified_result_sha256")
                .is_none()
        );
    }

    #[test]
    fn retention_uses_only_fully_elapsed_return_days() {
        let report = super::summarize_retention(
            "2026-10-08",
            &[
                super::RetentionCohort {
                    offset: 1,
                    eligible: 5,
                    returned: [5, 5, 5],
                },
                super::RetentionCohort {
                    offset: 2,
                    eligible: 10,
                    returned: [4, 0, 0],
                },
                super::RetentionCohort {
                    offset: 4,
                    eligible: 20,
                    returned: [10, 8, 0],
                },
                super::RetentionCohort {
                    offset: 8,
                    eligible: 30,
                    returned: [20, 15, 10],
                },
            ],
        );
        assert_eq!(report["d1"]["eligible_accounts"], 60);
        assert_eq!(report["d1"]["returned_accounts"], 34);
        assert_eq!(report["d1"]["cohort_cutoff_date"], "2026-10-06");
        assert_eq!(report["d1"]["status"], "ready");
        assert_eq!(report["d3"]["eligible_accounts"], 50);
        assert_eq!(report["d3"]["returned_accounts"], 23);
        assert_eq!(report["d7"]["eligible_accounts"], 30);
        assert_eq!(report["d7"]["returned_accounts"], 10);
    }

    #[test]
    fn immature_retention_is_not_reported_as_zero_percent() {
        let report = super::summarize_retention(
            "2026-10-08",
            &[super::RetentionCohort {
                offset: 1,
                eligible: 5,
                returned: [0; 3],
            }],
        );
        assert_eq!(report["d1"]["status"], "not_matured");
        assert!(report["d1"]["rate"].is_null());
    }

    #[test]
    fn tutorial_step_counts_include_explicit_exit_without_inflating_completion() {
        let rows = vec![
            std::collections::HashMap::from([
                ("boudica|claim_wilderness|2|start".to_string(), 8),
                ("boudica|claim_wilderness|2|complete".to_string(), 5),
                ("boudica|claim_wilderness|2|fail".to_string(), 2),
            ]),
            std::collections::HashMap::from([
                ("boudica|claim_wilderness|2|start".to_string(), 2),
                ("boudica|claim_wilderness|2|complete".to_string(), 1),
                ("boudica|claim_wilderness|2|fail".to_string(), 1),
                ("invalid|field".to_string(), 100),
            ]),
        ];
        let rows = super::aggregate_tutorial_steps(&rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["started"], 10);
        assert_eq!(rows[0]["completed"], 6);
        assert_eq!(rows[0]["exited"], 3);
        assert_eq!(rows[0]["completion_rate"], 0.6);
    }
}
