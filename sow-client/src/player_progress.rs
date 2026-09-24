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
    pub intro_completed: Option<bool>,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
    #[serde(default)]
    pub leader_xp: std::collections::BTreeMap<String, u32>,
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
    /// Campaign episodes finished (stable ids, e.g. `"six_sky_ep1"`).
    /// Local-first: merged (union) with any cloud profile, never overwritten.
    #[serde(default)]
    pub completed_episodes: std::collections::BTreeSet<String>,
    /// Server-created match receipts used only for main-menu presentation.
    #[serde(default)]
    pub reward_receipts:
        std::collections::BTreeMap<String, sow_data::profile::RewardReceipt>,
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
    ProfileRatingsLoaded {
        account_id: String,
        items: Vec<sow_data::profile::PublicRatingView>,
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

    pub fn apply_reward(&mut self, leader: Leader, reward: sow_data::rewards::MatchReward) {
        self.add_xp(reward.xp);
        let entry = self.leader_xp.entry(leader.name().to_string()).or_default();
        *entry = entry.saturating_add(reward.leader_xp);
        self.crowns = self.crowns.saturating_add(reward.crowns);
        self.laurels = self.laurels.saturating_add(reward.laurels);
    }

    pub fn complete_tutorial_with_reward(&mut self) -> bool {
        if self.intro_completed.unwrap_or(false) {
            return false;
        }
        self.complete_intro();
        self.apply_reward(
            Leader::Boudica,
            sow_data::rewards::calculate(sow_data::rewards::RewardInput {
                tutorial: true,
                ..Default::default()
            }),
        );
        true
    }

    /// Mark a campaign episode complete with the same reward weight as the
    /// teaching intro (100 crowns; laurels come only from server achievements):
    /// finishing an episode is the retention backbone, and a full saga lands
    /// near one free leader unlock.
    /// Idempotent per episode id.
    pub fn complete_episode(&mut self, episode_id: &str, leader: Leader) -> bool {
        if !self.completed_episodes.insert(episode_id.to_string()) {
            return false;
        }
        self.apply_reward(
            leader,
            sow_data::rewards::calculate(sow_data::rewards::RewardInput {
                tutorial: true,
                ..Default::default()
            }),
        );
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

    /// Prefer cloud profile when it has history; otherwise keep local/CG portal data.
    /// Episode completions always merge by union: the server never tracks them,
    /// so a cloud overwrite must not erase local saga progress.
    pub fn merge_boot_profile(&mut self, cloud: PlayerProgress) {
        let local_intro_completed = self.intro_completed.unwrap_or(false);
        let local_episodes = std::mem::take(&mut self.completed_episodes);
        let local_receipts = std::mem::take(&mut self.reward_receipts);
        let local_achievements = std::mem::take(&mut self.unlocked_achievements);
        if cloud.has_history() || !self.has_history() {
            *self = cloud;
        }
        if local_intro_completed || self.intro_completed.unwrap_or(false) {
            self.intro_completed = Some(true);
        }
        self.completed_episodes.extend(local_episodes);
        for (id, receipt) in local_receipts {
            self.reward_receipts.entry(id).or_insert(receipt);
        }
        self.unlocked_achievements.extend(local_achievements);
    }

    pub fn sync_level(&mut self) {
        self.level = 1 + self.xp / 100;
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
        self.sync_level();
    }

    pub fn record_match_with_kda(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
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
        self.apply_reward(self.preferred_leader.unwrap_or(Leader::Caesar), reward);
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerProgress, SessionDefeats};
    use sow_core::player::Leader;

    #[test]
    fn tutorial_completion_is_idempotent_and_persists_reward() {
        let mut progress = PlayerProgress::default();
        assert!(progress.complete_tutorial_with_reward());
        assert!(!progress.complete_tutorial_with_reward());
        assert_eq!(progress.intro_completed, Some(true));
        assert_eq!(progress.xp, 100);
        assert_eq!(progress.crowns, 100);
        assert_eq!(progress.laurels, 0);
        assert_eq!(progress.leader_xp.get("Boudica"), Some(&100));
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
    fn match_reward_tracks_the_selected_leader_without_changing_gold() {
        let mut progress = PlayerProgress {
            preferred_leader: Some(Leader::Boudica),
            ..Default::default()
        };
        progress.record_match_with_kda(
            true,
            SessionDefeats {
                players: 1,
                ..Default::default()
            },
            2,
            1,
            1,
        );
        assert_eq!(progress.matches_played, 1);
        assert_eq!(progress.leader_xp.get("Boudica"), Some(&140));
        assert_eq!(progress.crowns, 106);
        assert_eq!(progress.laurels, 0);
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
}
