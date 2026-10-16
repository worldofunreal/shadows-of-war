//! Lifetime player stats persisted via the CrazyGames SDK Data module and sow-database.

use serde::Deserialize;
use sow_core::player::Leader;

pub const STORAGE_KEY: &str = "sow_player_progress";

/// Lazy migration for the 2026-09 currency split (owner decision): local
/// progress saved before the split stores the spendable balance under the
/// legacy `"laurels"` key. Move it to `"crowns"` and drop the legacy key so
/// the new `laurels` (achievement points) field defaults to 0 instead of
/// inheriting the old currency amount. No-op for already-migrated objects;
/// stored bytes are rewritten on the next regular save.
pub fn migrate_legacy_currency_json(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if obj.contains_key("crowns") {
        return;
    }
    if let Some(legacy) = obj.remove("laurels") {
        obj.insert("crowns".to_string(), legacy);
    }
}

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
    #[serde(default)]
    pub leader_xp: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub leader_stats: std::collections::BTreeMap<String, sow_data::profile::LeaderCareerStats>,
    /// Free spendable currency (legacy local saves stored it under "laurels";
    /// [`migrate_legacy_currency_json`] moves it here on load).
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
}

fn deserialize_leader<'de, D>(deserializer: D) -> Result<Option<Leader>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .as_deref()
        .map(|value| {
            sow_data::commerce::leader_from_id(value)
                .ok_or_else(|| serde::de::Error::custom("unknown leader"))
        })
        .transpose()
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
    },
    MatchResultFailed {
        account_id: String,
        match_id: String,
        cycle_id: u32,
        request_id: u64,
        status: Option<u16>,
    },
    ExitRecovery {
        account_id: String,
        match_id: String,
        cycle_id: u32,
        request_id: u64,
        accepted: bool,
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
        self.unlocked_achievements.extend(local_achievements);
        let confirmed_guides = std::mem::replace(&mut self.menu_guides, local_guides);
        self.merge_menu_guides(confirmed_guides);
    }

    pub(crate) fn merge_menu_guides(
        &mut self,
        incoming: std::collections::BTreeMap<String, sow_data::profile::MenuGuideProgress>,
    ) {
        for (id, progress) in incoming {
            let saved = self.menu_guides.get(&id);
            let source_match_id = progress
                .source_match_id
                .clone()
                .or_else(|| saved.and_then(|record| record.source_match_id.clone()));
            let mut merged = sow_data::profile::merge_menu_guide_progress(saved, &progress);
            // These snapshots came from the server; the shared write merger
            // deliberately strips this field from untrusted client requests.
            merged.source_match_id = source_match_id;
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

#[cfg(test)]
mod tests {
    use super::PlayerProgress;
    use sow_core::player::Leader;

    #[test]
    fn tutorial_completion_only_marks_local_completion() {
        let mut progress = PlayerProgress::default();
        assert!(progress.mark_tutorial_completed());
        assert!(!progress.mark_tutorial_completed());
        assert_eq!(progress.intro_completed, Some(true));
        assert_eq!(progress.xp, 0);
        assert_eq!(progress.crowns, 0);
        assert_eq!(progress.laurels, 0);
        assert!(progress.leader_xp.is_empty());
    }

    #[test]
    fn currency_balances_use_their_canonical_local_storage_keys() {
        let progress = PlayerProgress {
            crowns: 725,
            laurels: 40,
            ..Default::default()
        };
        let value = serde_json::to_value(&progress).unwrap();
        assert_eq!(value["crowns"], 725);
        assert_eq!(value["laurels"], 40);
    }

    #[test]
    fn legacy_local_laurels_balance_migrates_to_crowns() {
        // Pre-split local save: "laurels" holds the spendable balance.
        let mut legacy = serde_json::json!({ "laurels": 725, "xp": 10 });
        super::migrate_legacy_currency_json(&mut legacy);
        let progress: PlayerProgress = serde_json::from_value(legacy).unwrap();
        assert_eq!(progress.crowns, 725);
        assert_eq!(progress.laurels, 0);

        // Post-split save: untouched.
        let mut current = serde_json::json!({ "crowns": 100, "laurels": 40 });
        super::migrate_legacy_currency_json(&mut current);
        let progress: PlayerProgress = serde_json::from_value(current).unwrap();
        assert_eq!(progress.crowns, 100);
        assert_eq!(progress.laurels, 40);
    }

    #[test]
    fn random_match_reward_uses_the_resolved_leader_without_changing_preference() {
        let mut progress = PlayerProgress {
            preferred_leader: Some(Leader::Boudica),
            prefer_random_leader: true,
            ..Default::default()
        };
        progress.record_match_with_kda_for_leader(
            true,
            Default::default(),
            0,
            0,
            0,
            Leader::Napoleon,
        );
        assert_eq!(progress.leader_xp.get("Napoleon"), Some(&120));
        assert!(!progress.leader_xp.contains_key("Boudica"));
        assert_eq!(progress.preferred_leader, Some(Leader::Boudica));
        assert!(progress.prefer_random_leader);
    }

    #[test]
    fn random_selection_survives_reload_and_cloud_profile_merge() {
        let local = PlayerProgress {
            preferred_leader: Some(Leader::Boudica),
            prefer_random_leader: true,
            leader_selection_pending: true,
            ..Default::default()
        };
        let mut reloaded: PlayerProgress =
            serde_json::from_str(&serde_json::to_string(&local).unwrap()).unwrap();
        reloaded.merge_boot_profile(PlayerProgress {
            preferred_leader: Some(Leader::Caesar),
            ..Default::default()
        });
        assert_eq!(reloaded.preferred_leader, Some(Leader::Boudica));
        assert!(reloaded.prefer_random_leader);
        assert!(reloaded.leader_selection_pending);
    }

    #[test]
    fn pending_campaign_completion_survives_until_server_confirms_it() {
        let mut local = PlayerProgress::default();
        local.complete_episode("six_sky_ep1");
        local.merge_boot_profile(PlayerProgress::default());
        assert!(local.completed_episodes.contains("six_sky_ep1"));
        assert!(local.pending_campaign_completions.contains("six_sky_ep1"));

        let mut confirmed = PlayerProgress::default();
        confirmed
            .completed_episodes
            .insert("six_sky_ep1".to_string());
        local.merge_boot_profile(confirmed);
        assert!(local.completed_episodes.contains("six_sky_ep1"));
        assert!(!local.pending_campaign_completions.contains("six_sky_ep1"));
    }

    #[test]
    fn assigned_leader_does_not_make_a_new_profile_have_history() {
        let progress = PlayerProgress {
            preferred_leader: Some(Leader::Caesar),
            ..Default::default()
        };
        assert!(!progress.has_history());
    }

    #[test]
    fn completed_tutorial_cannot_be_erased_by_cloud_profile() {
        let mut local = PlayerProgress {
            intro_completed: Some(true),
            xp: 100,
            ..Default::default()
        };
        local.merge_boot_profile(PlayerProgress {
            preferred_leader: Some(Leader::Caesar),
            ..Default::default()
        });
        assert_eq!(local.intro_completed, Some(true));
        assert_eq!(local.xp, 100);
    }

    #[test]
    fn completed_tutorial_stays_completed_when_cloud_has_other_history() {
        let mut local = PlayerProgress {
            intro_completed: Some(true),
            ..Default::default()
        };
        local.merge_boot_profile(PlayerProgress {
            matches_played: 1,
            ..Default::default()
        });
        assert_eq!(local.intro_completed, Some(true));
    }

    #[test]
    fn switching_accounts_does_not_carry_progress_or_campaign_completions() {
        let mut old = PlayerProgress {
            xp: 900,
            crowns: 700,
            gems: 200,
            intro_completed: Some(true),
            completed_episodes: ["six_sky_ep1".to_string()].into(),
            ..Default::default()
        };
        old.replace_account_profile(PlayerProgress {
            gems: 475,
            intro_completed: Some(false),
            ..Default::default()
        });
        assert_eq!(old.xp, 0);
        assert_eq!(old.crowns, 0);
        assert_eq!(old.gems, 475);
        assert_eq!(old.intro_completed, Some(false));
        assert!(!old.completed_episodes.contains("six_sky_ep1"));
    }
}
