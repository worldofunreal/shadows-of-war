//! Lifetime player stats persisted via the CrazyGames SDK Data module and sow-database.

use serde::Deserialize;
use sow_core::player::Leader;

pub const STORAGE_KEY: &str = "sow_player_progress";

#[derive(Default, Clone, Copy, Debug)]
pub struct SessionDefeats {
    pub players: u32,
    pub empires: u32,
    pub tribes: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug, PartialEq)]
pub struct PlayerProgress {
    pub xp: u32,
    pub level: u32,
    pub wins: u32,
    pub matches_played: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    #[serde(default, deserialize_with = "deserialize_leader")]
    pub preferred_leader: Option<Leader>,
    #[serde(default)]
    pub prefer_random_leader: bool,
    #[serde(default)]
    pub leader_selection_pending: bool,
    pub intro_completed: Option<bool>,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
    #[serde(default, deserialize_with = "deserialize_leader_xp")]
    pub leader_xp: std::collections::BTreeMap<String, u32>,
    #[serde(default, deserialize_with = "deserialize_leader_stats")]
    pub leader_stats: std::collections::BTreeMap<String, sow_data::profile::LeaderCareerStats>,
    /// Free spendable currency.
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
    /// Campaign episodes finished or awaiting server confirmation.
    #[serde(default)]
    pub completed_episodes: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub pending_campaign_completions: std::collections::BTreeSet<String>,
    /// Server-created match receipts used only for main-menu presentation.
    #[serde(default)]
    pub reward_receipts: std::collections::BTreeMap<String, sow_data::profile::RewardReceipt>,
    #[serde(default)]
    pub menu_guides: std::collections::BTreeMap<String, sow_data::profile::MenuGuideProgress>,
    #[serde(default)]
    pub unlocked_achievements: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub missions_progress: std::collections::BTreeMap<String, sow_data::missions::MissionRecord>,
    #[serde(default)]
    pub daily_missions_day: u64,
    #[serde(default)]
    pub weekly_missions_period: u64,
    #[serde(default)]
    pub daily_mission_ids: Vec<String>,
    #[serde(default)]
    pub weekly_mission_ids: Vec<String>,
    #[serde(default)]
    pub daily_swaps_used: u8,
    #[serde(default)]
    pub weekly_swaps_used: u8,
    #[serde(default)]
    pub daily_swapped_out_ids: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub weekly_swapped_out_ids: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub menu_tips_disabled: bool,
    #[serde(default)]
    pub total_structures_built: u32,
    #[serde(default)]
    pub ranked_matches: u32,
    #[serde(default)]
    pub ranked_wins: u32,
}

fn deserialize_leader<'de, D>(deserializer: D) -> Result<Option<Leader>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    // Saved profiles can outlive heroes removed from the current catalog.
    Ok(value
        .as_deref()
        .and_then(sow_data::commerce::leader_from_any_id))
}

fn deserialize_leader_xp<'de, D>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<String, u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let saved = std::collections::BTreeMap::<String, u32>::deserialize(deserializer)?;
    let mut current = std::collections::BTreeMap::new();
    for (saved_name, xp) in saved {
        let name = sow_data::commerce::leader_from_any_id(&saved_name)
            .map_or(saved_name, |leader| leader.name().to_owned());
        let total = current.entry(name).or_insert(0u32);
        *total = total.saturating_add(xp);
    }
    Ok(current)
}

fn deserialize_leader_stats<'de, D>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<String, sow_data::profile::LeaderCareerStats>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let saved: std::collections::BTreeMap<String, sow_data::profile::LeaderCareerStats> =
        Deserialize::deserialize(deserializer)?;
    let mut current = std::collections::BTreeMap::new();
    for (saved_name, stats) in saved {
        let name = sow_data::commerce::leader_from_any_id(&saved_name)
            .map_or(saved_name, |leader| leader.name().to_owned());
        let total = current
            .entry(name)
            .or_insert_with(sow_data::profile::LeaderCareerStats::default);
        total.matches_played = total.matches_played.saturating_add(stats.matches_played);
        total.wins = total.wins.saturating_add(stats.wins);
        total.verified_wins = total.verified_wins.saturating_add(stats.verified_wins);
        total.kills = total.kills.saturating_add(stats.kills);
        total.deaths = total.deaths.saturating_add(stats.deaths);
        total.assists = total.assists.saturating_add(stats.assists);
        total.xp = total.xp.saturating_add(stats.xp);
    }
    Ok(current)
}

#[derive(Clone, Debug)]
pub enum DbEvent {
    MatchResultLoaded {
        account_id: String,
        match_id: String,
        cycle_id: u32,
        request_id: u64,
        display_name: String,
        provider: String,
        progress: Option<PlayerProgress>,
        state: sow_data::profile::MatchExitState,
        closure_recorded: bool,
        ranked_result: Option<sow_data::profile::RankedResultPresentation>,
    },
    MatchResultFailed {
        account_id: String,
        match_id: String,
        cycle_id: u32,
        request_id: u64,
        status: Option<u16>,
    },
    ProfileLoaded {
        progress: PlayerProgress,
        account_id: String,
        display_name: String,
        provider: String,
        request_id: u64,
    },
    DisplayNameSaved {
        account_id: String,
        display_name: String,
        request_id: u64,
    },
    DisplayNameSaveFailed {
        request_id: u64,
        status: Option<u16>,
    },
    LoadFailed {
        request_id: u64,
        provider: String,
        status: Option<u16>,
    },
    TutorialCompletionFailed {
        request_id: u64,
        status: Option<u16>,
    },
    ProfileViewLoaded {
        account_id: String,
        view: sow_data::profile::PublicProfileView,
    },
    ProfileLoadFailed {
        account_id: String,
        status: Option<u16>,
    },
    ProfileHistoryLoaded {
        account_id: String,
        items: Vec<sow_data::profile::PublicMatchSummary>,
        next_cursor: Option<usize>,
    },
    ProfileSearchLoaded {
        query: String,
        items: Vec<sow_data::profile::PublicProfileSummary>,
    },
    MatchDetailLoaded {
        match_id: String,
        detail: sow_data::profile::PublicMatchDetail,
    },
    ProfileOperationFailed {
        account_id: Option<String>,
        operation: String,
    },
    StoreProfileLoaded {
        account_id: String,
        progress: PlayerProgress,
        operation: String,
    },
    StoreActionFailed {
        account_id: String,
        operation: String,
        status: Option<u16>,
    },
    RewardReceiptsAcked {
        account_id: String,
        receipt_ids: Vec<String>,
    },
}

impl PlayerProgress {
    pub fn is_first_game(&self) -> bool {
        self.matches_played == 0 && !self.intro_completed.unwrap_or(false)
    }

    pub fn complete_intro(&mut self) {
        self.intro_completed = Some(true);
    }

    fn apply_reward(&mut self, leader: Leader, reward: sow_data::rewards::MatchReward) {
        self.add_xp(reward.xp);
        let entry = self.leader_xp.entry(leader.name().to_string()).or_default();
        *entry = entry.saturating_add(reward.leader_xp);
        self.crowns = self.crowns.saturating_add(reward.crowns);
        self.laurels = self.laurels.saturating_add(reward.laurels);
    }

    pub fn mark_tutorial_completed(&mut self) -> bool {
        if self.intro_completed.unwrap_or(false) {
            return false;
        }
        self.complete_intro();
        true
    }

    /// Keep a local completion pending until the server confirms its rewards.
    pub fn complete_episode(&mut self, episode_id: &str) -> bool {
        if !self.completed_episodes.insert(episode_id.to_string()) {
            return false;
        }
        self.pending_campaign_completions
            .insert(episode_id.to_string());
        true
    }

    pub fn has_history(&self) -> bool {
        self.matches_played > 0
            || self.wins > 0
            || self.xp > 0
            || self.intro_completed.unwrap_or(false)
            || self.crowns > 0
            || self.laurels > 0
            || self.gems > 0
            || !self.owned_leaders.is_empty()
            || !self.owned_skins.is_empty()
            || !self.leader_xp.is_empty()
            || !self.completed_episodes.is_empty()
            || !self.reward_receipts.is_empty()
            || !self.unlocked_achievements.is_empty()
    }

    /// Prefer cloud profile when it has history; preserve this account's pending
    /// episode and selection requests until the server confirms them.
    pub fn merge_boot_profile(&mut self, cloud: PlayerProgress) {
        let local_intro_completed = self.intro_completed.unwrap_or(false);
        let local_episodes = std::mem::take(&mut self.completed_episodes);
        let local_pending_episodes = std::mem::take(&mut self.pending_campaign_completions);
        let local_selection = (
            self.preferred_leader,
            self.prefer_random_leader,
            self.leader_selection_pending,
        );
        let local_receipts = std::mem::take(&mut self.reward_receipts);
        let local_achievements = std::mem::take(&mut self.unlocked_achievements);
        let local_guides = std::mem::take(&mut self.menu_guides);
        let local_missions = std::mem::take(&mut self.missions_progress);
        let server_episodes = cloud.completed_episodes.clone();
        if cloud.has_history() || !self.has_history() {
            *self = cloud;
        }
        if local_intro_completed || self.intro_completed.unwrap_or(false) {
            self.intro_completed = Some(true);
        }
        self.pending_campaign_completions
            .extend(local_pending_episodes);
        self.pending_campaign_completions.extend(
            local_episodes
                .iter()
                .filter(|id| !server_episodes.contains(*id))
                .cloned(),
        );
        self.pending_campaign_completions
            .retain(|id| !server_episodes.contains(id));
        self.completed_episodes.extend(local_episodes);
        if local_selection.2 {
            self.preferred_leader = local_selection.0;
            self.prefer_random_leader = local_selection.1;
            self.leader_selection_pending = true;
        }
        for (id, receipt) in local_receipts {
            self.reward_receipts.entry(id).or_insert(receipt);
        }
        // Claims and completions come from the server only. The client never sets them,
        // so a local record the cloud lacks can only hold an unconfirmed claim: keep its
        // progress and drop the flags.
        for (id, local_record) in local_missions {
            match self.missions_progress.get_mut(&id) {
                Some(entry) => entry.progress = entry.progress.max(local_record.progress),
                None => {
                    self.missions_progress.insert(
                        id,
                        sow_data::missions::MissionRecord {
                            completed: false,
                            claimed: false,
                            claimed_at: None,
                            doubled_by_ad: false,
                            ..local_record
                        },
                    );
                }
            }
        }
        self.unlocked_achievements.extend(local_achievements);
        let confirmed_guides = std::mem::replace(&mut self.menu_guides, local_guides);
        self.merge_menu_guides(confirmed_guides);
    }

    pub(crate) fn merge_menu_guides(
        &mut self,
        incoming: std::collections::BTreeMap<String, sow_data::profile::MenuGuideProgress>,
    ) {
        self.menu_guides
            .retain(|id, _| sow_data::profile::is_known_menu_guide_id(id));
        for (id, progress) in incoming {
            if !sow_data::profile::is_known_menu_guide_id(&id) {
                continue;
            }
            let saved = self.menu_guides.get(&id);
            let source_match_id = progress
                .source_match_id
                .clone()
                .or_else(|| saved.and_then(|record| record.source_match_id.clone()));
            let mut merged =
                sow_data::profile::merge_menu_guide_progress_for_id(&id, saved, &progress);
            // Server snapshots may establish this context; client writes cannot.
            merged.source_match_id = source_match_id;
            let merged = sow_data::profile::normalize_guide_progress_for_id(
                &id,
                &merged,
                self.matches_played,
            );
            self.menu_guides.insert(id, merged);
        }
    }

    pub fn replace_account_profile(&mut self, cloud: PlayerProgress) {
        *self = cloud;
    }

    pub fn sync_level(&mut self) {
        self.level = 1 + self.xp / 100;
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
        self.sync_level();
    }

    pub fn record_match_with_kda_for_leader(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
        leader: Leader,
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
        let reward = sow_data::rewards::calculate(sow_data::rewards::RewardInput {
            won,
            players_defeated: defeats.players,
            empires_defeated: defeats.empires,
            tribes_defeated: defeats.tribes,
            kills,
            assists,
            ..Default::default()
        });
        self.apply_reward(leader, reward);
    }
}

