use crate::metadata_db::{
    MATCHES_TABLE, PLAYER_MATCH_INDEX_TABLE, PLAYERS_TABLE, PUBLIC_PROFILES_TABLE,
    SEASON_RATINGS_TABLE, SEASONS_TABLE,
};
use crate::profile::{
    LeaderCareerStats, MatchRecord, PublicLeaderSummary, PublicLeaderboardEntry, PublicMatchDetail,
    PublicMatchParticipant, PublicMatchSummary, PublicProfileIndex, PublicProfileSummary,
    PublicProfileView, PublicRatingView, SeasonRating, SeasonRecord, public_handle, win_rate,
};
use log::{error, info};
use redb::ReadableTable;
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};

const ANALYTICS_UNIQUE: &str = "sow:analytics:unique_users";
const ANALYTICS_ACTIVE_PREFIX: &str = "sow:analytics:active:";
const ANALYTICS_DAU_TTL_SECS: i64 = 35 * 24 * 3600;
const ANALYTICS_EVENT_COUNT_PREFIX: &str = "sow:analytics:event:";
const ANALYTICS_EVENT_USERS_PREFIX: &str = "sow:analytics:event_users:";
const ANALYTICS_COHORT_PREFIX: &str = "sow:analytics:cohort:";
const ANALYTICS_ACTIVATED_PREFIX: &str = "sow:analytics:activated:";
const ANALYTICS_RETENTION_TTL_SECS: i64 = 90 * 24 * 3600;

/// SET index of all bot account_ids — populated by `seed_bot_pool`, used for
/// pool introspection and analytics. Account records carry a canonical
/// display_name field; the bot allocator may choose its presentation name.
const BOT_POOL_KEY: &str = "sow:bot:pool";

const ACCOUNT_ID_HEX_LEN: usize = 32;
pub const DISPLAY_NAME_MAX_CHARS: usize = crate::name_policy::MAX_CHARS;

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

#[derive(Clone, Debug)]
pub struct MatchOutcomeKda {
    pub defeats: SessionDefeats,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub leader: Option<String>,
}

/// Deserialize one stored account, applying the legacy currency migration
/// (pre-split `"laurels"` spendable balance → `"crowns"`) at this single
/// chokepoint. Every stored-account parse in the workspace goes through here.
pub fn parse_account_with_migration(raw: &[u8]) -> serde_json::Result<PlayerAccount> {
    let mut value: serde_json::Value = serde_json::from_slice(raw)?;
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
    pub matches_played: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub preferred_leader: Option<String>,
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
    pub intro_completed: bool,
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
            matches_played: 0,
            players_defeated: 0,
            empires_defeated: 0,
            tribes_defeated: 0,
            preferred_leader: None,
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
            unlocked_achievements: std::collections::BTreeSet::new(),
            playgames_sync_revision: 0,
            playgames_synced_revision: 0,
            intro_completed: false,
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

    pub fn apply_reward(&mut self, leader: &str, reward: crate::rewards::MatchReward) {
        self.add_xp(reward.xp);
        let entry = self.leader_xp.entry(leader.to_string()).or_default();
        *entry = entry.saturating_add(reward.leader_xp);
        let stats = self.leader_stats.entry(leader.to_string()).or_default();
        stats.xp = stats.xp.saturating_add(reward.leader_xp);
        self.crowns = self.crowns.saturating_add(reward.crowns);
        self.laurels = self.laurels.saturating_add(reward.laurels);
    }

    fn achievement_progress(&self, id: &str) -> u64 {
        match id {
            "first_command" | "battle_hardened" => self.matches_played as u64,
            "first_victory" | "victory_march" => self.wins as u64,
            "laurel_hoard" => self.laurels,
            "commander_victorious" => self
                .leader_stats
                .values()
                .map(|stats| stats.wins)
                .sum::<u32>() as u64,
            "veteran_commander" => self
                .leader_stats
                .values()
                .map(|stats| stats.matches_played)
                .sum::<u32>() as u64,
            "banner_collector" => self.leader_stats.len() as u64,
            "leader_path" => self.leader_xp.values().copied().max().unwrap_or_default() as u64,
            _ => 0,
        }
    }

    /// Unlock achievements from server-owned aggregates. The set and laurel
    /// total are updated together, so retries cannot pay an achievement twice.
    pub fn refresh_achievements(&mut self) {
        for achievement in crate::rewards::ACHIEVEMENTS {
            let unlocked = self.achievement_progress(achievement.id) >= achievement.target;
            if unlocked && self.unlocked_achievements.insert(achievement.id.to_string()) {
                self.laurels = self.laurels.saturating_add(achievement.points);
            }
        }
    }

    pub fn achievement_views(&self) -> Vec<crate::profile::AchievementView> {
        crate::rewards::ACHIEVEMENTS
            .iter()
            .map(|achievement| crate::profile::AchievementView {
                id: achievement.id.to_string(),
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

    pub fn record_match_with_kda(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
    ) {
        self.record_match_with_leader(won, defeats, kills, deaths, assists, None);
    }

    pub fn record_match_with_leader(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
        leader: Option<&str>,
    ) {
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

        let reward = crate::rewards::calculate(crate::rewards::RewardInput {
            won,
            players_defeated: defeats.players,
            empires_defeated: defeats.empires,
            tribes_defeated: defeats.tribes,
            kills,
            assists,
            ..Default::default()
        });
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
        self.apply_reward(&leader, reward);
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
    /// Remove the anonymous ownership proof before returning an account from
    /// a public HTTP endpoint. The hash remains present in storage and in
    /// authenticated internal responses.
    pub fn without_auth_secret(mut self) -> Self {
        self.auth_secret_hash = None;
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
    pub crazygames_api_key: Option<String>,
    pub metadata_db: Option<std::sync::Arc<redb::Database>>,
}

fn utc_date_string() -> String {
    crate::events::utc_date_string()
}

impl PlayerDb {
    pub fn new(
        redis_url: &str,
        crazygames_api_key: Option<String>,
        metadata_db: Option<std::sync::Arc<redb::Database>>,
    ) -> Self {
        let client = Client::open(redis_url).expect("Failed to connect to Valkey/Redis");
        info!("Successfully initialized Valkey database connector client.");
        Self {
            client,
            crazygames_api_key,
            metadata_db,
        }
    }

    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, redis::RedisError> {
        self.client.get_multiplexed_async_connection().await
    }

    /// Crate-internal connection accessor for sibling server modules
    /// (moderation, future operator tooling). Auth stays with the callers.
    pub(crate) async fn client_conn(
        &self,
    ) -> Result<redis::aio::MultiplexedConnection, redis::RedisError> {
        self.get_connection().await
    }

    pub fn save_player_account_to_redb(&self, account: &PlayerAccount) {
        if let Some(ref db) = self.metadata_db
            && let Ok(write_txn) = db.begin_write()
        {
            if let Ok(mut table) = write_txn.open_table(PLAYERS_TABLE)
                && let Ok(json) = serde_json::to_string(account)
                && let Err(error) = table.insert(account.id.as_str(), json.as_bytes())
            {
                error!("Failed to persist account {} to REDB: {error}", account.id);
            }
            if let Ok(mut table) = write_txn.open_table(PUBLIC_PROFILES_TABLE) {
                let index = PublicProfileIndex {
                    account_id: account.id.clone(),
                    display_name: account.display_name.clone(),
                    kind: format!("{:?}", account.kind),
                    updated_at: account.updated_at,
                };
                if let Ok(json) = serde_json::to_vec(&index)
                    && let Err(error) = table.insert(index.account_id.as_str(), json.as_slice())
                {
                    error!(
                        "Failed to persist public profile index {}: {error}",
                        account.id
                    );
                }
            }
            if let Err(error) = write_txn.commit() {
                error!("Failed to commit account {} to REDB: {error}", account.id);
            }
        }
    }

    /// Persist one completed match and its per-player lookup keys. The match
    /// row is idempotent, so relay retries cannot create duplicate history.
    pub fn save_match_record(
        &self,
        record: &MatchRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
        };
        let write_txn = db.begin_write()?;
        let mut matches = write_txn.open_table(MATCHES_TABLE)?;
        let json = serde_json::to_vec(record)?;
        matches.insert(record.match_id.as_str(), json.as_slice())?;
        drop(matches);

        let mut index = write_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
        for participant in &record.participants {
            let key = format!(
                "player:{}:{:020}:{}",
                participant.account_id, record.completed_at, record.match_id
            );
            index.insert(key.as_str(), record.match_id.as_bytes())?;
        }
        drop(index);
        write_txn.commit()?;
        Ok(())
    }

    pub fn save_season_rating(
        &self,
        rating: &SeasonRating,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
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

    fn load_season_rating(
        &self,
        account_id: &str,
        queue: &str,
        mode: &str,
    ) -> Result<Option<SeasonRating>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(None);
        };
        let key = format!(
            "rating:{}:{}:{}:{}",
            account_id,
            crate::profile::CURRENT_SEASON_ID,
            queue,
            mode
        );
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        let Some(value) = table.get(key.as_str())? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(value.value())?))
    }

    fn pair_update(winner: &mut SeasonRating, loser: &mut SeasonRating) {
        // Weng-Lin/OpenSkill update for one decisive comparison. FFA and team
        // results are reduced to pairwise comparisons, keeping the stored
        // ladder independent of client-provided UI values.
        let beta = 25.0 / 6.0;
        let c = (2.0 * beta * beta + winner.sigma * winner.sigma + loser.sigma * loser.sigma)
            .sqrt()
            .max(0.0001);
        let t = (winner.mu - loser.mu) / c;
        let normal = |x: f64| (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
        let cdf = |x: f64| 1.0 / (1.0 + (-1.702 * x).exp());
        let v = normal(t) / cdf(t).max(1e-9);
        let w = v * (v + t);
        let winner_var = winner.sigma * winner.sigma;
        let loser_var = loser.sigma * loser.sigma;
        winner.mu += winner_var / c * v;
        loser.mu -= loser_var / c * v;
        winner.sigma *= (1.0 - winner_var / (c * c) * w).max(0.01).sqrt();
        loser.sigma *= (1.0 - loser_var / (c * c) * w).max(0.01).sqrt();
    }

    fn apply_ratings(
        &self,
        record: &mut MatchRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !record.rating_eligible {
            return Ok(());
        }
        let human_indexes = record
            .participants
            .iter()
            .enumerate()
            .filter(|(_, participant)| !participant.is_bot)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut ratings = human_indexes
            .iter()
            .map(|index| {
                let participant = &record.participants[*index];
                let rating = self
                    .load_season_rating(&participant.account_id, &record.queue, &record.mode)?
                    .unwrap_or(SeasonRating {
                        schema_version: 1,
                        account_id: participant.account_id.clone(),
                        season_id: record.season_id,
                        queue: record.queue.clone(),
                        mode: record.mode.clone(),
                        games_played: 0,
                        wins: 0,
                        mu: 25.0,
                        sigma: 8.333,
                        score: 0,
                        tier: "Provisional".to_string(),
                        division: None,
                        peak_score: 0,
                        placements_complete: false,
                        updated_at: 0,
                    });
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(rating)
            })
            .collect::<Result<Vec<_>, _>>()?;

        if record.mode == "HumansVsNations" && human_indexes.len() == 1 {
            let mut ai = SeasonRating {
                mu: 25.0,
                sigma: 1.0,
                ..ratings[0].clone()
            };
            ai.account_id.clear();
            let human_won = record.participants[human_indexes[0]].won;
            if human_won {
                Self::pair_update(&mut ratings[0], &mut ai);
            } else {
                Self::pair_update(&mut ai, &mut ratings[0]);
            }
        } else {
            for left in 0..ratings.len() {
                for right in left + 1..ratings.len() {
                    let left_won = record.participants[human_indexes[left]].won;
                    let right_won = record.participants[human_indexes[right]].won;
                    if left_won == right_won {
                        continue;
                    }
                    if left_won {
                        let (winner, loser) = ratings.split_at_mut(right);
                        Self::pair_update(&mut winner[left], &mut loser[0]);
                    } else {
                        let (winner, loser) = ratings.split_at_mut(right);
                        Self::pair_update(&mut loser[0], &mut winner[left]);
                    }
                }
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for (index, rating) in ratings.iter_mut().enumerate() {
            let old_score = if rating.games_played == 0 {
                crate::profile::ladder_score(25.0, 8.333)
            } else {
                rating.score
            };
            let participant = &mut record.participants[human_indexes[index]];
            rating.games_played = rating.games_played.saturating_add(1);
            if participant.won {
                rating.wins = rating.wins.saturating_add(1);
            }
            rating.score = crate::profile::ladder_score(rating.mu, rating.sigma);
            rating.placements_complete = rating.games_played >= 5;
            let (tier, division) = crate::profile::tier_for_score(rating.score);
            rating.tier = if rating.placements_complete {
                tier.to_string()
            } else {
                "Provisional".to_string()
            };
            rating.division = if rating.placements_complete {
                division
            } else {
                None
            };
            rating.peak_score = rating.peak_score.max(rating.score);
            rating.updated_at = now;
            participant.rating_delta = Some(rating.score as i16 - old_score as i16);
            self.save_season_rating(rating)?;
        }
        Ok(())
    }

    pub fn save_season(
        &self,
        season: &SeasonRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
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
        let Some(db) = &self.metadata_db else {
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
        let Some(db) = &self.metadata_db else {
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
        let Some(db) = &self.metadata_db else {
            return Ok(Vec::new());
        };
        let prefix = format!("player:{account_id}:");
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
        let mut match_ids = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            if key.value().starts_with(&prefix)
                && let Ok(match_id) = std::str::from_utf8(value.value())
            {
                match_ids.push(match_id.to_string());
            }
        }
        drop(table);
        drop(read_txn);
        match_ids.sort_by(|left, right| right.cmp(left));
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
            rating_delta: participant.rating_delta,
        })
    }

    fn public_summary(account: &PlayerAccount) -> PublicProfileSummary {
        PublicProfileSummary {
            account_id: account.id.clone(),
            handle: public_handle(&account.display_name, &account.id),
            display_name: account.display_name.clone(),
            level: account.profile.level,
            matches_played: account.profile.matches_played,
            wins: account.profile.wins,
            win_rate: win_rate(account.profile.wins, account.profile.matches_played),
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
        let Some(index) = self.public_index(account_id)? else {
            return Ok(None);
        };
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, &index.account_id).await?;
        if account.kind == AccountKind::Bot {
            return Ok(None);
        }
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
                wins: stats.wins,
                win_rate: win_rate(stats.wins, stats.matches_played),
                kills: stats.kills,
                deaths: stats.deaths,
                assists: stats.assists,
                xp: stats.xp,
            })
            .collect::<Vec<_>>();
        leaders.sort_by_key(|left| std::cmp::Reverse(left.xp));
        let recent_matches = self
            .match_history_for_account(&account.id, 0, 10, None, None)?
            .iter()
            .filter_map(|record| Self::public_match_summary(&account.id, record))
            .collect();
        Ok(Some(PublicProfileView {
            account_id: account.id.clone(),
            handle: public_handle(&account.display_name, &account.id),
            display_name: account.display_name,
            level: account.profile.level,
            matches_played: account.profile.matches_played,
            wins: account.profile.wins,
            win_rate: win_rate(account.profile.wins, account.profile.matches_played),
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
        }))
    }

    pub async fn playgames_sync_snapshot(
        &self,
        account_id: &str,
    ) -> Result<Option<PlayGamesSyncSnapshot>, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, account_id).await?;
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
            wins: account.profile.wins,
            revision: account.profile.playgames_sync_revision,
            synced_revision: account.profile.playgames_synced_revision,
        }))
    }

    pub async fn mark_playgames_sync(
        &self,
        account_id: &str,
        revision: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let account = Self::update_account_atomic(
            &mut con,
            &Self::account_key(account_id),
            |account| {
                if account.profile.playgames_sync_revision == revision {
                    account.profile.playgames_synced_revision = revision;
                    account.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            },
        )
        .await?;
        self.save_player_account_to_redb(&account);
        Ok(())
    }

    pub async fn search_public_profiles(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<PublicProfileSummary>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
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
        let mut con = self.get_connection().await?;
        let mut result = Vec::with_capacity(indexes.len());
        for index in indexes {
            if let Ok(account) = Self::load_account(&mut con, &index.account_id).await
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
        let Some(index) = self.public_index(account_id)? else {
            return Ok(None);
        };
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, &index.account_id).await?;
        if account.kind == AccountKind::Bot {
            return Ok(None);
        }
        Ok(Some(
            self.match_history_for_account(
                &account.id,
                offset,
                limit.saturating_add(1),
                queue,
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
            rating_eligible: record.rating_eligible,
            participants,
        }))
    }

    pub fn ensure_current_season(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
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
        let Some(db) = &self.metadata_db else {
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
        let Some(index) = self.public_index(account_id)? else {
            return Ok(None);
        };
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, &index.account_id).await?;
        if account.kind == AccountKind::Bot {
            return Ok(None);
        }
        let Some(db) = &self.metadata_db else {
            return Ok(Some(Vec::new()));
        };
        let prefix = format!("rating:{}:", account.id);
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        let mut ratings = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            if key.value().starts_with(&prefix) {
                ratings.push(serde_json::from_slice::<SeasonRating>(value.value())?);
            }
        }
        drop(table);
        drop(read_txn);
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
        let Some(db) = &self.metadata_db else {
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
        let mut con = self.get_connection().await?;
        let mut entries = Vec::new();
        for rating in ratings {
            let Ok(account) = Self::load_account(&mut con, &rating.account_id).await else {
                continue;
            };
            if account.kind == AccountKind::Bot {
                continue;
            }
            entries.push((rating, account.id, account.display_name));
        }
        entries.sort_by_key(|left| std::cmp::Reverse(left.0.score));
        Ok(entries
            .into_iter()
            .take(limit)
            .enumerate()
            .map(
                |(index, (rating, account_id, display_name))| PublicLeaderboardEntry {
                    rank: index as u32 + 1,
                    account_id: account_id.clone(),
                    handle: public_handle(&display_name, &account_id),
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
    ) -> Result<Vec<crate::profile::PublicVictoryLeaderboardEntry>, Box<dyn std::error::Error + Send + Sync>>
    {
        let Some(db) = &self.metadata_db else {
            return Ok(Vec::new());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PUBLIC_PROFILES_TABLE)?;
        let account_ids = table
            .iter()?
            .filter_map(|item| {
                let (_, value) = item.ok()?;
                let index: PublicProfileIndex = serde_json::from_slice(value.value()).ok()?;
                (index.kind != "Bot").then_some(index.account_id)
            })
            .collect::<Vec<_>>();
        drop(table);
        drop(read_txn);

        let mut con = self.get_connection().await?;
        let mut accounts = Vec::new();
        for account_id in account_ids {
            if let Ok(account) = Self::load_account(&mut con, &account_id).await
                && account.kind != AccountKind::Bot
            {
                accounts.push(account);
            }
        }
        accounts.sort_by(|left, right| {
            right
                .profile
                .wins
                .cmp(&left.profile.wins)
                .then_with(|| right.profile.matches_played.cmp(&left.profile.matches_played))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(accounts
            .into_iter()
            .skip(offset)
            .take(limit)
            .enumerate()
            .map(|(index, account)| crate::profile::PublicVictoryLeaderboardEntry {
                rank: (offset + index + 1) as u32,
                handle: public_handle(&account.display_name, &account.id),
                account_id: account.id,
                level: account.profile.level,
                matches_played: account.profile.matches_played,
                wins: account.profile.wins,
            })
            .collect())
    }

    /// Key format for looking up canonical account ID by platform identity
    fn identity_key(provider: &str, external_id: &str) -> String {
        format!("sow:player:identity:{}:{}", provider, external_id)
    }

    fn environment_identity_key(environment: &str, provider: &str, external_id: &str) -> String {
        format!(
            "sow:player:identity:{}:{}:{}",
            environment, provider, external_id
        )
    }

    /// Key format for loading/saving full player account info
    fn account_key(account_id: &str) -> String {
        format!("sow:player:account:{}", account_id)
    }

    async fn load_account(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let acc_key = Self::account_key(account_id);
        let acc_json: Option<String> = con.get(&acc_key).await?;
        let Some(acc_json) = acc_json else {
            return Err("Account not found".into());
        };
        Ok(parse_account_with_migration(acc_json.as_bytes())?)
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
        let mut con = self.get_connection().await?;
        let updated =
            Self::update_account_atomic(&mut con, &Self::account_key(&account.id), |account| {
                if account.profile.preferred_leader.is_none() {
                    account.profile.preferred_leader = Some(assigned.clone());
                    account.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            })
            .await?;
        self.save_player_account_to_redb(&updated);
        Ok(updated)
    }

    async fn ensure_human_onboarding(
        &self,
        account: PlayerAccount,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.ensure_starting_leader(account).await?;
        self.ensure_welcome_grant(account).await
    }

    /// Replace one account only if the JSON read by this request is still the
    /// value in Valkey. This closes the rename/stats lost-update race without
    /// introducing another state store or a process-local lock.
    async fn update_account_atomic<F>(
        con: &mut redis::aio::MultiplexedConnection,
        acc_key: &str,
        mut mutate: F,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut(&mut PlayerAccount),
    {
        let compare_and_set = redis::Script::new(
            r#"if redis.call('GET', KEYS[1]) == ARGV[1] then
                    redis.call('SET', KEYS[1], ARGV[2])
                    return 1
                end
                return 0"#,
        );
        for _ in 0..5 {
            let Some(expected_json): Option<String> = con.get(acc_key).await? else {
                return Err("Account not found".into());
            };
            let mut account = parse_account_with_migration(expected_json.as_bytes())?;
            mutate(&mut account);
            let updated_json = serde_json::to_string(&account)?;
            let replaced: i32 = compare_and_set
                .key(acc_key)
                .arg(&expected_json)
                .arg(&updated_json)
                .invoke_async(con)
                .await?;
            if replaced == 1 {
                return Ok(account);
            }
        }
        Err("concurrent account update; retry".into())
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
        let mut con = self.get_connection().await?;
        let updated = Self::update_account_atomic(
            &mut con,
            &Self::account_key(&account.id),
            |account| {
                if account.kind != AccountKind::Human {
                    return;
                }
                if account
                    .profile
                    .purchase_history
                    .contains_key(crate::commerce::WELCOME_GRANT_ID)
                {
                    return;
                }
                let migrated_legacy = account
                    .profile
                    .purchase_history
                    .contains_key(crate::commerce::LEGACY_ANONYMOUS_WELCOME_GRANT_ID);
                if !migrated_legacy {
                    account.profile.gems = account
                        .profile
                        .gems
                        .saturating_add(crate::commerce::WELCOME_GEMS);
                }
                account.profile.purchase_history.insert(
                    crate::commerce::WELCOME_GRANT_ID.to_string(),
                    PurchaseRecord {
                        id: crate::commerce::WELCOME_GRANT_ID.to_string(),
                        provider: "system".to_string(),
                        environment: if migrated_legacy {
                            "migration".to_string()
                        } else {
                            "welcome".to_string()
                        },
                        product_id: "sow_welcome_gems_1600".to_string(),
                        transaction_id: None,
                        status: if migrated_legacy {
                            "migrated".to_string()
                        } else {
                            "granted".to_string()
                        },
                        acquired_at: now,
                        updated_at: now,
                    },
                );
                account.updated_at = now;
                granted = true;
            },
        )
        .await?;
        if granted {
            self.save_player_account_to_redb(&updated);
        }
        Ok(updated)
    }

    /// Migrate every human account in the live account store. The account
    /// marker makes this safe to rerun on every service start.
    pub async fn migrate_welcome_gems_to_existing_humans(
        &self,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Valkey is the live account store. Scanning it also covers accounts
        // created before the REDB mirror existed or while that mirror was
        // unavailable.
        let mut con = self.get_connection().await?;
        let mut cursor = 0_u64;
        let mut account_ids = Vec::new();
        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("sow:player:account:*")
                .arg("COUNT")
                .arg(500)
                .query_async(&mut con)
                .await?;
            account_ids.extend(
                keys.into_iter()
                    .filter_map(|key| key.strip_prefix("sow:player:account:").map(str::to_string)),
            );
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        let mut migrated = 0_u64;
        for account_id in account_ids {
            let Ok(account) = Self::load_account(&mut con, &account_id).await else {
                continue;
            };
            if account.kind != AccountKind::Human
                || account
                    .profile
                    .purchase_history
                    .contains_key(crate::commerce::WELCOME_GRANT_ID)
            {
                continue;
            }
            let updated = self.ensure_welcome_grant(account).await?;
            if updated
                .profile
                .purchase_history
                .contains_key(crate::commerce::WELCOME_GRANT_ID)
            {
                migrated = migrated.saturating_add(1);
            }
        }
        Ok(migrated)
    }

    async fn record_analytics(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
        is_new: bool,
    ) -> Result<(), redis::RedisError> {
        if is_new {
            let _: () = con.pfadd(ANALYTICS_UNIQUE, account_id).await?;
            // The HyperLogLog is a probabilistic counter with no per-member
            // removal, so it must stay bounded by the same 90-day retention
            // window as every other analytics key (see Privacy Policy).
            let _: () = con
                .expire(ANALYTICS_UNIQUE, ANALYTICS_RETENTION_TTL_SECS)
                .await?;
        }
        let day_key = format!("{ANALYTICS_ACTIVE_PREFIX}{}", utc_date_string());
        let _: () = con.pfadd(&day_key, account_id).await?;
        let _: () = con.expire(&day_key, ANALYTICS_DAU_TTL_SECS).await?;
        let active_key = Self::daily_active_key(&utc_date_string());
        let _: () = con.sadd(&active_key, account_id).await?;
        let _: () = con.expire(&active_key, ANALYTICS_DAU_TTL_SECS).await?;
        Ok(())
    }

    async fn record_activation_in_connection(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
        date: &str,
    ) -> Result<(), redis::RedisError> {
        let activated_key = format!("{ANALYTICS_ACTIVATED_PREFIX}{account_id}");
        let first_match: bool = con.set_nx(&activated_key, date).await?;
        if first_match {
            let cohort_key = format!("{ANALYTICS_COHORT_PREFIX}{date}");
            let _: () = redis::pipe()
                .expire(&activated_key, ANALYTICS_RETENTION_TTL_SECS)
                .sadd(&cohort_key, account_id)
                .expire(&cohort_key, ANALYTICS_RETENTION_TTL_SECS)
                .query_async(con)
                .await?;
        }
        Ok(())
    }

    /// Record a client-originated product event in hot counters and exact
    /// daily activity sets. The JSONL sink remains the durable event source;
    /// these keys make DAU/funnel/retention queries cheap without a vendor.
    pub async fn record_product_event(
        &self,
        name: &str,
        account_id: Option<&str>,
    ) -> Result<(), redis::RedisError> {
        let mut con = self.get_connection().await?;
        let date = utc_date_string();
        let count_key = format!("{ANALYTICS_EVENT_COUNT_PREFIX}{date}:{name}");
        let _: u64 = con.incr(&count_key, 1_u64).await?;
        let _: () = con.expire(&count_key, ANALYTICS_RETENTION_TTL_SECS).await?;

        let Some(account_id) = account_id else {
            return Ok(());
        };
        let is_bot: i8 = con.sismember(BOT_POOL_KEY, account_id).await?;
        if is_bot == 1 {
            return Ok(());
        }

        let event_users_key = format!("{ANALYTICS_EVENT_USERS_PREFIX}{date}:{name}");
        let active_key = Self::daily_active_key(&date);
        let _: () = redis::pipe()
            .sadd(&event_users_key, account_id)
            .expire(&event_users_key, ANALYTICS_RETENTION_TTL_SECS)
            .sadd(&active_key, account_id)
            .expire(&active_key, ANALYTICS_RETENTION_TTL_SECS)
            .query_async(&mut con)
            .await?;

        if matches!(name, "match_ended" | "match_ended_client") {
            Self::record_activation_in_connection(&mut con, account_id, &date).await?;
        }
        Ok(())
    }

    /// Mark every human participant in an authoritative completed match as
    /// active and put first-time completers into the activation cohort.
    pub async fn record_match_activation(
        &self,
        account_ids: &[String],
    ) -> Result<(), redis::RedisError> {
        let mut con = self.get_connection().await?;
        let date = utc_date_string();
        for account_id in account_ids {
            let is_bot: i8 = con.sismember(BOT_POOL_KEY, account_id).await?;
            if is_bot == 1 {
                continue;
            }
            let active_key = Self::daily_active_key(&date);
            let _: () = redis::pipe()
                .sadd(&active_key, account_id)
                .expire(&active_key, ANALYTICS_RETENTION_TTL_SECS)
                .query_async(&mut con)
                .await?;
            Self::record_activation_in_connection(&mut con, account_id, &date).await?;
        }
        Ok(())
    }

    /// Permanently erase one account: the Valkey account record plus its
    /// identity mappings, the redb mirror rows (`PLAYERS_TABLE` and the
    /// public-profile index), and the account's membership in every dated
    /// analytics set. Operator-only — see `POST /internal/profile/delete`.
    ///
    /// Deliberately out of scope: match-history rows (aggregate competitive
    /// record), JSONL event lines (pseudonymous `session_id` + `account_id`
    /// pairs that age out via the 90-day file rotation once the account
    /// record they point to is gone), and the HyperLogLog unique counter
    /// (probabilistic structure with no per-member removal, bounded by its
    /// own 90-day key TTL).
    pub async fn delete_account(
        &self,
        account_id: &str,
    ) -> Result<DeleteAccountReport, Box<dyn std::error::Error + Send + Sync>> {
        if account_id.len() != ACCOUNT_ID_HEX_LEN
            || !account_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid account_id".into());
        }
        let mut con = self.get_connection().await?;
        let account: Option<PlayerAccount> = Self::load_account(&mut con, account_id).await.ok();

        let mut keys = vec![Self::account_key(account_id)];
        // Owned block list and per-day report counters belong to the account
        // and go with it. Memberships in *other* accounts' block sets are
        // stale-but-harmless (they match a gone account) and are left alone.
        keys.push(format!("sow:blocks:{account_id}"));
        if let Some(ref account) = account {
            for identity in &account.linked_identities {
                keys.push(Self::environment_identity_key(
                    &identity.environment,
                    &identity.provider,
                    &identity.external_id,
                ));
                keys.push(Self::identity_key(
                    &identity.provider,
                    &identity.external_id,
                ));
            }
        }
        let mut keys_removed = 0_u64;
        for key in &keys {
            keys_removed += con.del::<_, u64>(key).await.unwrap_or(0);
        }

        // The redb mirror is best-effort: Valkey is the live source of truth,
        // so a missing mirror row is a normal no-op, not a failure.
        let mut redb_rows_removed = 0_u32;
        if let Some(ref db) = self.metadata_db
            && let Ok(write_txn) = db.begin_write()
        {
            if let Ok(mut table) = write_txn.open_table(PLAYERS_TABLE) {
                match table.remove(account_id) {
                    Ok(Some(_)) => redb_rows_removed += 1,
                    Ok(None) => {}
                    Err(error) => {
                        error!("Failed to erase account {account_id} from REDB: {error}");
                    }
                }
            }
            if let Ok(mut table) = write_txn.open_table(PUBLIC_PROFILES_TABLE) {
                match table.remove(account_id) {
                    Ok(Some(_)) => redb_rows_removed += 1,
                    Ok(None) => {}
                    Err(error) => {
                        error!("Failed to erase public profile {account_id} from REDB: {error}");
                    }
                }
            }
            if let Err(error) = write_txn.commit() {
                error!("Failed to commit account erasure {account_id} to REDB: {error}");
            }
        }

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
            found: account.is_some(),
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
        let mut cursor = 0_u64;
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("sow:player:account:*")
                .arg("COUNT")
                .arg(256)
                .query_async(&mut con)
                .await?;
            for key in keys {
                let Some(raw): Option<String> = con.get(&key).await? else {
                    continue;
                };
                let Ok(account) = parse_account_with_migration(raw.as_bytes()) else {
                    error!("Skipping malformed account during test reset: {key}");
                    continue;
                };
                if account.kind == AccountKind::Human {
                    human_ids.insert(account.id.clone());
                    collect_purchase_markers(&account, &mut purchase_markers);
                }
            }
            cursor = next;
            if cursor == 0 {
                break;
            }
        }

        let bots_preserved: u64 = con.scard(BOT_POOL_KEY).await?;
        let mut player_rows = Vec::new();
        let mut public_profile_rows = Vec::new();
        let mut match_rows = Vec::new();
        let mut rating_rows = Vec::new();
        let mut match_index_rows = Vec::new();
        if let Some(db) = &self.metadata_db {
            let read_txn = db.begin_read()?;
            {
                let table = read_txn.open_table(PLAYERS_TABLE)?;
                for item in table.iter()? {
                    let (key, value) = item?;
                    let account_id = key.value().to_string();
                    if let Ok(account) = parse_account_with_migration(value.value()) {
                        if account.kind == AccountKind::Human {
                            human_ids.insert(account_id.clone());
                            collect_purchase_markers(&account, &mut purchase_markers);
                        }
                    }
                    if human_ids.contains(&account_id) {
                        player_rows.push(account_id);
                    }
                }
            }
            {
                let table = read_txn.open_table(PUBLIC_PROFILES_TABLE)?;
                for item in table.iter()? {
                    let (key, _) = item?;
                    if human_ids.contains(key.value()) {
                        public_profile_rows.push(key.value().to_string());
                    }
                }
            }
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
            redis_delete_keys.insert(Self::account_key(account_id));
            redis_delete_keys.insert(format!("sow:blocks:{account_id}"));
            redis_delete_keys.insert(format!("sow:analytics:activated:{account_id}"));
            redis_delete_keys.insert(format!("sow:profile:{account_id}"));
        }

        for pattern in [
            "sow:player:identity:*",
            "sow:player:wou:*",
            "sow:profile:*",
            "sow:analytics:activated:*",
        ] {
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
                    let remove = if key.starts_with("sow:analytics:activated:")
                        || key.starts_with("sow:profile:")
                    {
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

        let redb_rows_removed = (player_rows.len()
            + public_profile_rows.len()
            + match_rows.len()
            + rating_rows.len()
            + match_index_rows.len()) as u64;
        if !dry_run {
            for key in &redis_delete_keys {
                let _: u64 = con.del(key).await?;
            }
            if let Some(db) = &self.metadata_db {
                let write_txn = db.begin_write()?;
                {
                    let mut table = write_txn.open_table(PLAYERS_TABLE)?;
                    for key in &player_rows {
                        table.remove(key.as_str())?;
                    }
                }
                {
                    let mut table = write_txn.open_table(PUBLIC_PROFILES_TABLE)?;
                    for key in &public_profile_rows {
                        table.remove(key.as_str())?;
                    }
                }
                {
                    let mut table = write_txn.open_table(MATCHES_TABLE)?;
                    for key in &match_rows {
                        table.remove(key.as_str())?;
                    }
                }
                {
                    let mut table = write_txn.open_table(SEASON_RATINGS_TABLE)?;
                    for key in &rating_rows {
                        table.remove(key.as_str())?;
                    }
                }
                {
                    let mut table = write_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
                    for key in &match_index_rows {
                        table.remove(key.as_str())?;
                    }
                }
                write_txn.commit()?;
            }
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

    /// Return a compact, authenticated-only analytics snapshot for operators.
    /// Retention uses exact daily sets, not HLL intersections.
    pub async fn analytics_summary(
        &self,
        requested_days: u32,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let days = requested_days.clamp(1, 90) as i64;
        let today = utc_date_string();
        let names = [
            "landing_visit",
            "play_now_click",
            "shell_loaded",
            "matchmaking_joined",
            "match_started",
            "match_ended",
        ];
        let mut con = self.get_connection().await?;
        let mut daily = Vec::new();
        let mut funnel = serde_json::Map::new();
        for offset in 0..days {
            let date = crate::events::shift_date(&today, -offset).ok_or("invalid UTC date")?;
            let mut counts = serde_json::Map::new();
            for name in names {
                let key = format!("{ANALYTICS_EVENT_COUNT_PREFIX}{date}:{name}");
                // GET on a missing counter returns nil, which is a normal
                // zero-count day—not a Redis failure.
                let count: u64 = con.get::<_, Option<u64>>(&key).await?.unwrap_or(0);
                counts.insert(name.to_string(), serde_json::json!(count));
                let total = funnel
                    .entry(name.to_string())
                    .or_insert_with(|| serde_json::json!(0_u64));
                *total = serde_json::json!(total.as_u64().unwrap_or(0).saturating_add(count));
            }
            let active_key = Self::daily_active_key(&date);
            let active: usize = con.scard(&active_key).await?;
            daily.push(serde_json::json!({
                "date": date,
                "active_users": active,
                "events": counts,
            }));
        }

        let mut eligible_cohorts = 0_u64;
        let mut d1_returned = 0_u64;
        let mut d7_returned = 0_u64;
        for offset in 7..days {
            let cohort_date =
                crate::events::shift_date(&today, -offset).ok_or("invalid cohort date")?;
            let members: Vec<String> = con
                .smembers(format!("{ANALYTICS_COHORT_PREFIX}{cohort_date}"))
                .await?;
            if members.is_empty() {
                continue;
            }
            eligible_cohorts += members.len() as u64;
            let day_one = crate::events::shift_date(&cohort_date, 1).ok_or("invalid D1 date")?;
            let day_seven = crate::events::shift_date(&cohort_date, 7).ok_or("invalid D7 date")?;
            for account_id in members {
                if con
                    .sismember::<_, _, bool>(Self::daily_active_key(&day_one), &account_id)
                    .await?
                {
                    d1_returned += 1;
                }
                if con
                    .sismember::<_, _, bool>(Self::daily_active_key(&day_seven), &account_id)
                    .await?
                {
                    d7_returned += 1;
                }
            }
        }
        Ok(serde_json::json!({
            "generated_at": today,
            "days": days,
            "funnel": funnel,
            "daily": daily,
            "retention": {
                "eligible_activated_players": eligible_cohorts,
                "d1_returned": d1_returned,
                "d7_returned": d7_returned,
            }
        }))
    }

    /// Key holding the SET of account_ids active on a UTC date. Exact
    /// membership (unlike the DAU HyperLogLog) is what makes D1/Dn retention
    /// computable.
    fn daily_active_key(date: &str) -> String {
        format!("sow:active:{date}")
    }

    /// Membership check against the bot pool index — used to exclude synthetic
    /// players from analytics ingestion and human counters.
    pub async fn is_bot_account_checked(
        &self,
        account_id: &str,
    ) -> Result<bool, redis::RedisError> {
        let mut con = self.get_connection().await?;
        Ok(con.sismember::<_, _, i8>(BOT_POOL_KEY, account_id).await? == 1)
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
        let Ok(mut con) = self.get_connection().await else {
            return 0;
        };
        let mut humans = 0usize;
        for id in player_ids {
            let is_bot: i8 = match con.sismember(BOT_POOL_KEY, id).await {
                Ok(value) => value,
                Err(error) => {
                    error!("bot-pool lookup failed while counting players: {error}");
                    continue;
                }
            };
            if is_bot == 0 {
                humans += 1;
            }
        }
        humans
    }

    /// Read a match's registered roster and exit order before finalize deletes them.
    pub async fn match_participants(
        &self,
        match_id: &str,
    ) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let players_json: Option<String> = con.get(format!("sow:match:{match_id}:players")).await?;
        let exits: Vec<String> = con
            .lrange(format!("sow:match:{match_id}:exits"), 0, -1)
            .await?;
        let players = players_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Ok((players, exits))
    }

    /// Record exact daily activity for an account (retention cohorts).
    pub async fn mark_daily_activity(&self, account_id: &str) {
        let Ok(mut con) = self.get_connection().await else {
            return;
        };
        let date = utc_date_string();
        let key = Self::daily_active_key(&date);
        let _: Result<(), _> = redis::pipe()
            .sadd(&key, account_id)
            .expire(&key, ANALYTICS_DAU_TTL_SECS)
            .query_async(&mut con)
            .await;
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
        let mut con = self.get_connection().await?;
        let id_key = Self::environment_identity_key(&environment, &provider, &external_id);
        let Some(account_id) = con.get::<_, Option<String>>(&id_key).await? else {
            return Ok(None);
        };
        let Ok(account) = Self::load_account(&mut con, &account_id).await else {
            return Ok(None);
        };
        let _: () = Self::record_analytics(&mut con, &account.id, false).await?;
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

        let mut con = self.get_connection().await?;
        let id_key = Self::environment_identity_key(&environment, &provider, &external_id);

        let mapped_identity = con.get::<_, Option<String>>(&id_key).await?;
        if let Some(mapped_id) = &mapped_identity
            && mapped_id != account_id
        {
            return Err("provider identity is already linked to another account_id".into());
        }

        if mapped_identity.is_some() {
            let account = Self::load_account(&mut con, account_id).await?;
            let account = self
                .bind_provider_identity(&mut con, account, &provider, &environment, &external_id)
                .await?;
            return self.ensure_human_onboarding(account).await;
        }

        // A canonical account may already have a SOW anonymous record. Keep
        // that progress and attach the verified provider instead of creating
        // a second game account.
        if let Some(raw) = con
            .get::<_, Option<String>>(Self::account_key(account_id))
            .await?
        {
            let account: PlayerAccount = serde_json::from_str(&raw)?;
            if account.kind == AccountKind::Bot {
                return Err("canonical account_id belongs to a bot".into());
            }
            let account = self
                .bind_provider_identity(&mut con, account, &provider, &environment, &external_id)
                .await?;
            let _: () = con.set(&id_key, account_id).await?;
            return self.ensure_human_onboarding(account).await;
        }

        let identity = LinkedIdentity {
            provider: provider.clone(),
            external_id: external_id.clone(),
            environment: environment.clone(),
        };
        let kind = if provider == "bot" {
            AccountKind::Bot
        } else {
            AccountKind::Human
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let new_account = PlayerAccount {
            id: account_id.to_string(),
            display_name: generated_display_name(),
            profile: PlayerProfile::for_account(account_id),
            linked_identities: vec![identity.clone()],
            kind,
            auth_secret_hash: None,
            created_at: now,
            updated_at: now,
        };
        let account_json = serde_json::to_string(&new_account)?;
        let claim = redis::Script::new(
            r#"if redis.call('EXISTS', KEYS[1]) == 1 then
                        local existing = redis.call('GET', KEYS[2])
                        if existing then return 'existing:' .. existing end
                        return 'collision'
                    end
                    local existing = redis.call('GET', KEYS[2])
                    if existing then return 'existing:' .. existing end
                    redis.call('SET', KEYS[1], ARGV[1])
                    redis.call('SET', KEYS[2], ARGV[2])
                    return 'created:' .. ARGV[2]"#,
        );
        let result: String = claim
            .key(Self::account_key(account_id))
            .key(&id_key)
            .arg(account_json)
            .arg(account_id)
            .invoke_async(&mut con)
            .await?;
        if result == "collision" {
            return Err("account_id already exists without this provider mapping".into());
        }
        let Some(claimed_id) = result
            .strip_prefix("created:")
            .or_else(|| result.strip_prefix("existing:"))
        else {
            return Err("canonical identity claim returned an invalid result".into());
        };
        if result.starts_with("existing:") {
            if claimed_id != account_id {
                return Err("provider identity is already linked to another account_id".into());
            }
            let account = Self::load_account(&mut con, claimed_id).await?;
            let account = self
                .bind_provider_identity(&mut con, account, &provider, &environment, &external_id)
                .await?;
            return self.ensure_human_onboarding(account).await;
        }
        let _: () = Self::record_analytics(&mut con, account_id, true).await?;
        let new_account = self.ensure_human_onboarding(new_account).await?;
        self.save_player_account_to_redb(&new_account);
        info!(
            "Created SOW account {} for canonical account_id",
            new_account.id
        );
        Ok(new_account)
    }

    async fn bind_provider_identity(
        &self,
        con: &mut redis::aio::MultiplexedConnection,
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
        let updated =
            Self::update_account_atomic(con, &Self::account_key(&account.id), |account| {
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
        self.save_player_account_to_redb(&updated);
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
        let provider = "bot".to_string();
        let mut con = self.get_connection().await?;
        let id_key = Self::environment_identity_key(&environment, &provider, &external_id);

        // 1. Try to find existing account ID mapped to this identity
        if let Some(account_id) = con.get::<_, Option<String>>(&id_key).await?
            && let Ok(account) = Self::load_account(&mut con, &account_id).await
        {
            let _: () = Self::record_analytics(&mut con, &account.id, false).await?;
            return self.ensure_starting_leader(account).await;
        }

        // 2. Not found, create new stable account ID and register
        let random_id = format!("{:032x}", rand::random::<u128>());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let identity = LinkedIdentity {
            provider,
            external_id,
            environment,
        };

        // Bot identities (provider == "bot") are marked at creation. All
        // Provider identities are human accounts unless explicitly seeded as bots.
        let kind = if identity.provider == "bot" {
            AccountKind::Bot
        } else {
            AccountKind::Human
        };

        let new_account = PlayerAccount {
            id: random_id.clone(),
            display_name: generated_display_name(),
            profile: PlayerProfile::for_account(&random_id),
            linked_identities: vec![identity.clone()],
            kind,
            auth_secret_hash: None,
            created_at: now,
            updated_at: now,
        };

        let acc_key = Self::account_key(&random_id);
        let acc_json = serde_json::to_string(&new_account)?;

        // Store account and map identity atomically
        redis::pipe()
            .set(&acc_key, acc_json)
            .set(&id_key, &random_id)
            .query_async::<()>(&mut con)
            .await?;

        let _: () = Self::record_analytics(&mut con, &random_id, true).await?;

        info!(
            "Created new account {} in Valkey for identity {:?}/{:?}",
            new_account.id, identity.provider, identity.external_id
        );
        self.save_player_account_to_redb(&new_account);
        Ok(new_account)
    }

    /// Load or create an anonymous account using the canonical account ID.
    /// New anonymous accounts do not create a provider identity index.
    pub async fn get_or_create_anonymous(
        &self,
        account_id: Option<&str>,
        requested_display_name: Option<&str>,
        account_auth_secret: Option<&str>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;

        if let Some(account_id) = account_id.map(str::trim).filter(|id| !id.is_empty()) {
            if !is_valid_account_id(account_id) {
                return Err("account_id is invalid".into());
            }
            let account = Self::load_account(&mut con, account_id).await?;
            if !account.linked_identities.is_empty() {
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
                let acc_key = Self::account_key(account_id);
                let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
                    if account.display_name.trim().is_empty() {
                        account.display_name = display_name.clone();
                        account.updated_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                    }
                })
                .await?;
                self.save_player_account_to_redb(&account);
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
            let acc_key = Self::account_key(&random_id);
            let acc_json = serde_json::to_string(&account)?;

            // SETNX makes the canonical account key collision-safe without a
            // second identity mapping or a distributed lock.
            if con.set_nx::<_, _, bool>(&acc_key, acc_json).await? {
                let account = self.ensure_welcome_grant(account).await?;
                let _: () = Self::record_analytics(&mut con, &random_id, true).await?;
                self.save_player_account_to_redb(&account);
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
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);
        let plain = format!(
            "{:032x}{:032x}",
            rand::random::<u128>(),
            rand::random::<u128>()
        );
        let hash = blake3::hash(plain.as_bytes()).to_hex().to_string();
        let mut revealed: Option<String> = None;
        let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
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
        if revealed.is_some() {
            self.save_player_account_to_redb(&account);
        }
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
            return Err("account_id must be exactly 32 hexadecimal characters".to_string());
        }
        let mut con = self
            .get_connection()
            .await
            .map_err(|e| format!("database unavailable: {e}"))?;
        let account = Self::load_account(&mut con, account_id)
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

    pub async fn resolve_leader_for_account(
        &self,
        account_id: &str,
        requested_leader: Option<&str>,
    ) -> Result<crate::commerce::LeaderResolution, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, account_id).await?;
        Ok(crate::commerce::resolve_leader(
            requested_leader,
            account_id,
            &account.profile.owned_leaders,
            crate::commerce::current_rotation_period(),
        ))
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
        let mut con = self.get_connection().await?;
        if con.exists::<_, bool>(Self::account_key(reference)).await? {
            return Ok(Some(reference.to_string()));
        }
        Ok(None)
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
        let mut con = self.get_connection().await?;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(&account_id), |account| {
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
                } else if let Some(leader) = crate::commerce::direct_leader_for_product(product_id)
                {
                    let leader_id = crate::commerce::leader_id(leader).to_string();
                    if !account.profile.owned_leaders.contains(&leader_id) {
                        account.profile.owned_leaders.insert(leader_id.clone());
                        account.profile.purchased_leaders.insert(leader_id);
                    }
                } else if let Some(skin) = crate::commerce::direct_skin_for_product(product_id) {
                    if !account.profile.owned_skins.contains(&skin.id) {
                        account.profile.owned_skins.insert(skin.id.clone());
                        account.profile.purchased_skins.insert(skin.id);
                    }
                } else if product_id == "sow_offer_genghis_khan_royal_lattice" {
                    let leader_id =
                        crate::commerce::leader_id(crate::leaders::Leader::GenghisKhan).to_string();
                    if !account.profile.owned_leaders.contains(&leader_id) {
                        account.profile.owned_leaders.insert(leader_id.clone());
                        account.profile.purchased_leaders.insert(leader_id);
                    }
                    if !account.profile.owned_skins.contains("royal_lattice") {
                        account
                            .profile
                            .owned_skins
                            .insert("royal_lattice".to_string());
                        account
                            .profile
                            .purchased_skins
                            .insert("royal_lattice".to_string());
                    }
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
        if granted {
            self.save_player_account_to_redb(&account);
        }
        Ok((account, granted))
    }

    pub async fn purchase_history_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<PurchaseRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, account_id).await?;
        let mut records = account
            .profile
            .purchase_history
            .into_values()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(records)
    }

    pub async fn purchase_record_for_account(
        &self,
        account_id: &str,
        purchase_id: &str,
    ) -> Result<Option<PurchaseRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, account_id).await?;
        Ok(account.profile.purchase_history.get(purchase_id).cloned())
    }

    pub async fn record_stripe_purchase(
        &self,
        purchase_user_id: &str,
        session_id: &str,
        product_id: &str,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let account_id = self
            .account_id_from_reference(purchase_user_id)
            .await?
            .ok_or("Stripe purchase user is unknown")?;
        let session_id = session_id.trim();
        if session_id.is_empty() || session_id.len() > 256 {
            return Err("Stripe session id is invalid".into());
        }
        if !crate::commerce::is_store_product(product_id) {
            return Err("unknown Stripe product".into());
        }
        let purchase_id = format!("stripe:{session_id}");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut con = self.get_connection().await?;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(&account_id), |account| {
                if let Some(existing) = account.profile.purchase_history.get_mut(&purchase_id) {
                    if existing.status == "granted" || existing.status == "revoked" {
                        return;
                    }
                    existing.status = status.to_string();
                    existing.updated_at = now;
                    return;
                }
                account.profile.purchase_history.insert(
                    purchase_id.clone(),
                    PurchaseRecord {
                        id: purchase_id.clone(),
                        provider: "stripe".to_string(),
                        environment: "PRODUCTION".to_string(),
                        product_id: product_id.to_string(),
                        transaction_id: Some(session_id.to_string()),
                        status: status.to_string(),
                        acquired_at: now,
                        updated_at: now,
                    },
                );
                account.updated_at = now;
            })
            .await?;
        self.save_player_account_to_redb(&account);
        Ok(())
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
        let mut con = self.get_connection().await?;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(&account_id), |account| {
                if account
                    .profile
                    .processed_revenuecat_events
                    .contains(event_id)
                {
                    return;
                }
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(event_id.to_string());
                if let Some(transaction_id) = transaction_id
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    account
                        .profile
                        .processed_revenuecat_events
                        .insert(format!("transaction:{transaction_id}"));
                }
                if let Some(gems) = crate::commerce::gem_amount_for_product(product_id) {
                    account.profile.gems = account.profile.gems.saturating_sub(gems);
                } else if let Some(leader) = crate::commerce::direct_leader_for_product(product_id)
                {
                    let leader_id = crate::commerce::leader_id(leader).to_string();
                    if account.profile.purchased_leaders.remove(&leader_id) {
                        account.profile.owned_leaders.remove(&leader_id);
                    }
                } else if let Some(skin) = crate::commerce::direct_skin_for_product(product_id) {
                    if account.profile.purchased_skins.remove(&skin.id) {
                        account.profile.owned_skins.remove(&skin.id);
                        if account.profile.selected_skin.as_deref() == Some(skin.id.as_str()) {
                            account.profile.selected_skin = None;
                        }
                    }
                } else if product_id == "sow_offer_genghis_khan_royal_lattice" {
                    let leader_id =
                        crate::commerce::leader_id(crate::leaders::Leader::GenghisKhan).to_string();
                    if account.profile.purchased_leaders.remove(&leader_id) {
                        account.profile.owned_leaders.remove(&leader_id);
                    }
                    if account.profile.purchased_skins.remove("royal_lattice") {
                        account.profile.owned_skins.remove("royal_lattice");
                        if account.profile.selected_skin.as_deref() == Some("royal_lattice") {
                            account.profile.selected_skin = None;
                        }
                    }
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
        if revoked {
            self.save_player_account_to_redb(&account);
        }
        Ok((account, revoked))
    }

    pub async fn unlock_leader(
        &self,
        account_id: &str,
        requested_leader: &str,
        currency: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let leader = crate::commerce::leader_from_id(requested_leader).ok_or("unknown leader")?;
        let leader_id = crate::commerce::leader_id(leader).to_string();
        let (cost, use_gems) = match currency {
            "gems" => (crate::commerce::LEADER_UNLOCK_COST_GEMS, true),
            "" | "crowns" => (crate::commerce::LEADER_UNLOCK_COST_CROWNS, false),
            "laurels" => return Err("laurels are achievement points, not spendable".into()),
            _ => return Err("invalid leader unlock currency".into()),
        };
        let period = crate::commerce::current_rotation_period();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let purchase_id = format!("in_game:leader:{leader_id}");
        let mut con = self.get_connection().await?;
        let mut failure = None;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(account_id), |account| {
                if account.profile.owned_leaders.contains(&leader_id) {
                    failure = Some("leader already owned");
                } else if crate::commerce::leader_available(
                    leader,
                    &account.profile.owned_leaders,
                    period,
                ) {
                    failure = Some("leader is currently free in rotation");
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
        self.save_player_account_to_redb(&account);
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
        let mut con = self.get_connection().await?;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(account_id), |account| {
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
        self.save_player_account_to_redb(&account);
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
        let mut con = self.get_connection().await?;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(account_id), |account| {
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
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    pub async fn grant_revenuecat_gems(
        &self,
        event_id: &str,
        purchase_user_id: &str,
        product_id: &str,
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
        let gems = crate::commerce::gem_amount_for_product(product_id)
            .ok_or("unknown RevenueCat product")?;
        let mut con = self.get_connection().await?;
        let mut granted = false;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(&account_id), |account| {
                if account
                    .profile
                    .processed_revenuecat_events
                    .contains(event_id)
                {
                    return;
                }
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(event_id.to_string());
                account.profile.gems = account.profile.gems.saturating_add(gems);
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                granted = true;
            })
            .await?;
        if granted {
            self.save_player_account_to_redb(&account);
        }
        Ok((account, granted))
    }

    /// Reverse a refunded, voided, or charged-back gem purchase: deduct the
    /// bundle amount with a floor of zero. Deduplicated by refund event id in
    /// the same `processed_revenuecat_events` set as grants, so a retried
    /// notification cannot double-deduct. A matching grant is not required —
    /// a refund for an already-spent bundle simply zeroes the balance and is
    /// logged for operator review. Suspension for chargeback abuse stays a
    /// manual operator decision, never automatic.
    pub async fn revoke_revenuecat_gems(
        &self,
        event_id: &str,
        purchase_user_id: &str,
        product_id: &str,
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
        let gems = crate::commerce::gem_amount_for_product(product_id)
            .ok_or("unknown RevenueCat product")?;
        let mut con = self.get_connection().await?;
        let mut revoked = false;
        let account =
            Self::update_account_atomic(&mut con, &Self::account_key(&account_id), |account| {
                if account
                    .profile
                    .processed_revenuecat_events
                    .contains(event_id)
                {
                    return;
                }
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(event_id.to_string());
                account.profile.gems = account.profile.gems.saturating_sub(gems);
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                revoked = true;
            })
            .await?;
        if revoked {
            self.save_player_account_to_redb(&account);
        }
        Ok((account, revoked))
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
        let mut con = self.get_connection().await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut account_ids = Vec::with_capacity(external_ids.len());

        for external_id in external_ids {
            let id_key = Self::identity_key("bot", &external_id);

            // 1. Fast path: identity already mapped.
            if let Some(account_id) = con.get::<_, Option<String>>(&id_key).await? {
                account_ids.push(account_id);
                continue;
            }

            // 2. Create new bot account.
            let random_id = format!("{:032x}", rand::random::<u128>());
            let identity = LinkedIdentity {
                provider: "bot".to_string(),
                external_id: external_id.clone(),
                environment: "production".to_string(),
            };
            let account = PlayerAccount {
                id: random_id.clone(),
                display_name: generated_display_name(),
                profile: PlayerProfile::for_account(&random_id),
                linked_identities: vec![identity.clone()],
                kind: AccountKind::Bot,
                auth_secret_hash: None,
                created_at: now,
                updated_at: now,
            };
            let acc_key = Self::account_key(&random_id);
            let acc_json = serde_json::to_string(&account)?;

            redis::pipe()
                .set(&acc_key, acc_json)
                .set(&id_key, &random_id)
                .sadd(BOT_POOL_KEY, &random_id)
                .query_async::<()>(&mut con)
                .await?;

            self.save_player_account_to_redb(&account);
            account_ids.push(random_id);
        }

        info!(
            "[bot-pool] seed resolved {} bot accounts",
            account_ids.len()
        );
        Ok(account_ids)
    }

    /// Update player profile stats
    pub async fn update_profile(
        &self,
        account_id: &str,
        profile: PlayerProfile,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);

        if con.exists(&acc_key).await? {
            let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
                account.profile = profile.clone();
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            })
            .await?;
            self.save_player_account_to_redb(&account);
            Ok(account)
        } else {
            Err("Account not found".into())
        }
    }

    /// Update the SOW display name after the request handler has authenticated
    /// the account through its anonymous or platform identity.
    pub async fn update_display_name(
        &self,
        account_id: &str,
        display_name: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if !is_valid_account_id(account_id) {
            return Err("account_id must be exactly 32 hexadecimal characters".into());
        }
        let display_name = normalize_display_name(display_name).map_err(str::to_string)?;
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);
        let Some(_acc_json) = con.get::<_, Option<String>>(&acc_key).await? else {
            return Err("Account not found".into());
        };
        let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
            account.display_name = display_name.clone();
            account.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        })
        .await?;
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    /// Record a match outcome with KDA stats from relay-logged client submissions.
    pub async fn record_match_outcome_with_kda(
        &self,
        account_id: &str,
        won: bool,
        kda: MatchOutcomeKda,
        preferred_leader: Option<String>,
        settlement_id: Option<&str>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);

        if con.exists(&acc_key).await? {
            let settlement_id = settlement_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let defeats = kda.defeats;
            let kills = kda.kills;
            let deaths = kda.deaths;
            let assists = kda.assists;
            let reported_leader = kda.leader;
            let leader = preferred_leader.clone().or(reported_leader.clone());
            let canonical_leader = leader
                .as_deref()
                .and_then(crate::rewards::canonical_leader_name);
            let reward = crate::rewards::calculate(crate::rewards::RewardInput {
                won,
                players_defeated: defeats.players,
                empires_defeated: defeats.empires,
                tribes_defeated: defeats.tribes,
                kills,
                assists,
                ..Default::default()
            });
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
                if let Some(id) = settlement_id.as_deref()
                    && account.profile.reward_receipts.contains_key(id)
                {
                    return;
                }
                account.profile.record_match_with_leader(
                    won,
                    defeats,
                    kills,
                    deaths,
                    assists,
                    canonical_leader.as_deref(),
                );
                if let Some(leader) = canonical_leader
                    .as_deref()
                    .and_then(crate::commerce::leader_from_id)
                {
                    account.profile.preferred_leader =
                        Some(crate::commerce::leader_wire_id(leader).to_string());
                }
                let laurels_before = account.profile.laurels;
                account.profile.refresh_achievements();
                account.profile.playgames_sync_revision = account
                    .profile
                    .playgames_sync_revision
                    .saturating_add(1);
                if let Some(id) = settlement_id.as_deref() {
                    account.profile.reward_receipts.insert(
                        id.to_string(),
                        crate::profile::RewardReceipt {
                            id: id.to_string(),
                            match_id: id.to_string(),
                            xp: reward.xp,
                            leader_xp: reward.leader_xp,
                            crowns: reward.crowns,
                            laurels: reward
                                .laurels
                                .saturating_add(account.profile.laurels.saturating_sub(laurels_before)),
                            created_at: now,
                            status: crate::profile::RewardSettlementStatus::Applied,
                            presented_at: None,
                        },
                    );
                }
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            })
            .await?;
            self.save_player_account_to_redb(&account);
            Ok(account)
        } else {
            Err("Account not found".into())
        }
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
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);
        let mut changed = false;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
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
        })
        .await?;
        if changed {
            self.save_player_account_to_redb(&account);
        }
        Ok(account)
    }

    /// Complete the offline tutorial once and grant its fixed onboarding reward.
    /// The account secret is verified by the HTTP handler before this method runs.
    pub async fn complete_anonymous_tutorial(
        &self,
        account_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);
        let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
            if account.profile.intro_completed {
                return;
            }
            account.profile.intro_completed = true;
            let leader = account
                .profile
                .preferred_leader
                .as_deref()
                .and_then(crate::commerce::leader_from_id)
                .unwrap_or(crate::leaders::Leader::Boudica);
            account.profile.preferred_leader =
                Some(crate::commerce::leader_wire_id(leader).to_string());
            let reward = crate::rewards::calculate(crate::rewards::RewardInput {
                tutorial: true,
                ..Default::default()
            });
            let laurels_before = account.profile.laurels;
            account.profile.apply_reward(leader.name(), reward);
            account.profile.refresh_achievements();
            account.profile.reward_receipts.insert(
                "tutorial".to_string(),
                crate::profile::RewardReceipt {
                    id: "tutorial".to_string(),
                    match_id: "tutorial".to_string(),
                    xp: reward.xp,
                    leader_xp: reward.leader_xp,
                    crowns: reward.crowns,
                    laurels: reward
                        .laurels
                        .saturating_add(account.profile.laurels.saturating_sub(laurels_before)),
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    status: crate::profile::RewardSettlementStatus::Applied,
                    presented_at: None,
                },
            );
            account.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        })
        .await?;
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    /// Register expected players for an upcoming match.
    pub async fn register_match_start(
        &self,
        match_id: &str,
        player_ids: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let key = format!("sow:match:{match_id}:players");
        let players_json = serde_json::to_string(player_ids)?;
        redis::pipe()
            .set(&key, players_json)
            .expire(&key, 3600)
            .query_async::<()>(&mut con)
            .await?;
        info!(
            "Registered match {match_id} with {} players",
            player_ids.len()
        );
        Ok(())
    }

    /// Finalize a match from relay-logged exit order in Valkey. The optional
    /// lobby snapshot is retained in the durable public history record.
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
        if self.load_match_record(match_id)?.is_some() {
            let _: () = con.set(&finalized_key, "1").await?;
            return Ok(Vec::new());
        }

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

        let players_key = format!("sow:match:{match_id}:players");
        let exits_key = format!("sow:match:{match_id}:exits");
        let players_json: Option<String> = con.get(&players_key).await?;
        let Some(players_json) = players_json else {
            // No account players registered (e.g. bot-only matches): the
            // replay was already archived by the handler; there is no stats
            // state to finalize, so treat it as a clean success.
            return Ok(Vec::new());
        };
        let players: Vec<String> = serde_json::from_str(&players_json)?;
        if players.is_empty() {
            return Ok(Vec::new());
        }

        let exits: Vec<String> = con.lrange(&exits_key, 0, -1).await?;
        let lobby_value = lobby_json
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
            .unwrap_or_default();
        let lobby_players = lobby_value
            .get("players")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let queue = lobby_value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .filter(|kind| !kind.is_empty())
            .unwrap_or("Matchmaking")
            .to_string();
        let config = lobby_value.get("config");
        let mode = config
            .and_then(|value| value.get("game_mode"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("FFA")
            .to_string();
        let map_name = config
            .and_then(|value| value.get("map_name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("world")
            .to_string();
        let raw_player = |account_id: &str| {
            lobby_players.iter().find(|player| {
                player
                    .get("database_account_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(account_id)
            })
        };
        let team_for = |account_id: &str| {
            raw_player(account_id)
                .and_then(|player| player.get("team"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };
        // The relay currently transports replay data but does not verify it
        // with the deterministic game engine. Until that verifier is wired
        // here, no client-submitted KDA or winner can create extra rewards or
        // wins. The ledger still records that every account participated.
        let winner: Option<String> = None;
        let winning_team: Option<String> = None;
        let completed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut participant_records = Vec::with_capacity(players.len());
        let mut playgames_outcomes = Vec::with_capacity(players.len());
        for account_id in &players {
            let raw = raw_player(account_id);
            let raw_leader = raw
                .and_then(|player| player.get("leader"))
                .and_then(serde_json::Value::as_str)
                .and_then(crate::rewards::canonical_leader_name);
            let team = team_for(account_id);
            let won = false;
            let defeats = SessionDefeats::default();
            let kills = 0;
            let deaths = 0;
            let assists = 0;
            let leader = raw_leader;
            let placement = exits
                .iter()
                .position(|exit| exit == account_id)
                .map(|position| (exits.len().saturating_sub(position)) as u16)
                .unwrap_or(1);
            let account = Self::load_account(&mut con, account_id).await?;
            let reward = crate::rewards::calculate(crate::rewards::RewardInput {
                won,
                players_defeated: defeats.players,
                empires_defeated: defeats.empires,
                tribes_defeated: defeats.tribes,
                kills,
                assists,
                ..Default::default()
            });
            participant_records.push(crate::profile::MatchParticipantRecord {
                account_id: account_id.clone(),
                display_name: account.display_name,
                is_bot: account.kind == AccountKind::Bot,
                leader: leader.clone(),
                team,
                placement,
                won,
                kills,
                deaths,
                assists,
                players_defeated: defeats.players,
                empires_defeated: defeats.empires,
                tribes_defeated: defeats.tribes,
                xp: reward.xp,
                leader_xp: reward.leader_xp,
                crowns: reward.crowns,
                laurels: reward.laurels,
                rating_delta: None,
            });

            match self
                .record_match_outcome_with_kda(
                    account_id,
                    won,
                    MatchOutcomeKda {
                        defeats,
                        kills,
                        deaths,
                        assists,
                        leader: leader.clone(),
                    },
                    None,
                    Some(match_id),
                )
                .await
            {
                Ok(account) => {
                    self.submit_crazygames_score(&account).await;
                    playgames_outcomes.push(PlayGamesMatchOutcome {
                        account_id: account_id.clone(),
                        won,
                        wins: account.profile.wins,
                        sync_revision: account.profile.playgames_sync_revision,
                        unlocked_achievements: account
                            .profile
                            .unlocked_achievements
                            .iter()
                            .cloned()
                            .collect(),
                    });
                }
                Err(e) => {
                    let _: Result<(), _> = con.del(&settling_key).await;
                    return Err(format!("failed to settle {account_id}: {e}").into());
                }
            }
        }

        let mut record = MatchRecord {
            schema_version: 1,
            match_id: match_id.to_string(),
            season_id: crate::profile::CURRENT_SEASON_ID,
            queue,
            mode,
            map_name,
            started_at: completed_at,
            completed_at,
            duration_seconds: 0,
            winner_account_id: winner,
            winning_team,
            verified: false,
            rating_eligible: false,
            participants: participant_records,
        };
        self.apply_ratings(&mut record)?;
        if let Err(error) = self.save_match_record(&record) {
            let _: Result<(), _> = con.del(&settling_key).await;
            return Err(error);
        }

        let mut stats_keys: Vec<String> = Vec::new();
        for account_id in &players {
            stats_keys.push(format!("sow:match:{match_id}:stats:{account_id}"));
        }

        let _: () = redis::pipe()
            .set(&finalized_key, "1")
            .del(&settling_key)
            .del(&players_key)
            .del(&exits_key)
            .query_async::<()>(&mut con)
            .await?;

        for key in stats_keys {
            let _: () = con.del(&key).await?;
        }
        for account_id in &players {
            let report_key = format!("sow:match:{match_id}:report:{account_id}");
            let _: () = con.del(&report_key).await?;
        }

        info!("Finalized match {match_id} with {} players", players.len());
        Ok(playgames_outcomes)
    }

    pub async fn submit_crazygames_score(&self, account: &PlayerAccount) {
        let Some(api_key) = &self.crazygames_api_key else {
            return;
        };

        if let Some(cg_identity) = account
            .linked_identities
            .iter()
            .find(|li| li.provider == "crazygames")
        {
            let score = account.profile.xp;
            let user_id = &cg_identity.external_id;
            info!("Submitting CrazyGames leaderboard score for user {user_id}: {score}");
            if let Err(e) = crate::crazygames::submit_score(api_key, user_id, score).await {
                error!("Failed to submit CrazyGames leaderboard score: {e}");
            }
        }
    }
}

fn is_valid_account_id(value: &str) -> bool {
    let value = value.trim();
    (value.len() == ACCOUNT_ID_HEX_LEN && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        || (value.len() == 36
            && value.bytes().enumerate().all(|(index, byte)| {
                matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                    || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
            }))
}

#[cfg(test)]
mod tests {
    use super::{
        DISPLAY_NAME_MAX_CHARS, LeaderCareerStats, PlayerProfile, generated_display_name,
        is_valid_account_id, normalize_display_name,
    };

    #[test]
    fn validates_canonical_account_ids() {
        assert!(is_valid_account_id("0123456789abcdef0123456789abcdef"));
        assert!(is_valid_account_id("52948534-1c9f-4def-bc8e-80390d3287a6"));
        // The removed guest_<hex> format is intentionally rejected; only the
        // 32-hex canonical account ID is accepted by the live API.
        assert!(!is_valid_account_id(
            "guest_0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_valid_account_id("not-an-account"));
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
            ..Default::default()
        };
        profile.leader_stats.insert(
            "Caesar".to_string(),
            LeaderCareerStats {
                matches_played: 1,
                wins: 1,
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
    fn laurel_hoard_can_unlock_from_the_same_server_update() {
        let mut profile = PlayerProfile {
            matches_played: 10,
            wins: 10,
            ..Default::default()
        };
        profile.leader_stats.insert(
            "Caesar".to_string(),
            LeaderCareerStats {
                matches_played: 10,
                wins: 10,
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
        assert_eq!(account.profile.crowns, 725, "legacy balance must become crowns");
        assert_eq!(account.profile.laurels, 0, "points must not inherit the legacy balance");

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
        let account: super::PlayerAccount = serde_json::from_str(json).unwrap();
        let public = serde_json::to_value(account.without_auth_secret()).unwrap();
        assert!(public.get("auth_secret_hash").is_none());
    }
}
