use crate::leaders::Leader;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignEpisode {
    pub id: &'static str,
    pub prerequisite: Option<&'static str>,
    /// Catalog id of the hero who narrates the episode.
    pub advisor_id: &'static str,
}

impl CampaignEpisode {
    pub fn advisor(&self) -> Leader {
        Leader::of(self.advisor_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignSeries {
    pub id: &'static str,
    pub episodes: &'static [&'static str],
    /// Catalog id of the hero unlocked by finishing the series.
    pub reward_leader_id: &'static str,
}

pub const CAMPAIGN_EPISODES: &[CampaignEpisode] = &[
    CampaignEpisode {
        id: "boudica",
        prerequisite: None,
        advisor_id: "boudica",
    },
    CampaignEpisode {
        id: "six_sky_ep1",
        prerequisite: None,
        advisor_id: "lady_six_sky",
    },
    CampaignEpisode {
        id: "six_sky_ep2",
        prerequisite: Some("six_sky_ep1"),
        advisor_id: "lady_six_sky",
    },
    CampaignEpisode {
        id: "six_sky_ep3",
        prerequisite: Some("six_sky_ep2"),
        advisor_id: "lady_six_sky",
    },
];

pub const CAMPAIGN_SERIES: &[CampaignSeries] = &[CampaignSeries {
    id: "lady_six_sky",
    episodes: &["six_sky_ep1", "six_sky_ep2", "six_sky_ep3"],
    reward_leader_id: "lady_six_sky",
}];

// Six Sky is not published yet. Keep its episode access and server rewards
// closed here until the content owner publishes the campaign.
pub const PUBLISHED_EPISODES: &[&str] = &["boudica"];

pub fn episode(id: &str) -> Option<&'static CampaignEpisode> {
    CAMPAIGN_EPISODES.iter().find(|episode| episode.id == id)
}

pub fn episode_is_published(id: &str) -> bool {
    PUBLISHED_EPISODES.contains(&id)
}

pub fn prerequisite_satisfied(
    id: &str,
    completed_episodes: &std::collections::BTreeSet<String>,
) -> bool {
    episode(id).is_some_and(|episode| {
        episode
            .prerequisite
            .is_none_or(|required| completed_episodes.contains(required))
    })
}

pub fn series_for_episode(id: &str) -> Option<&'static CampaignSeries> {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| series.episodes.contains(&id))
}

pub fn earned_leader(completed_episodes: &std::collections::BTreeSet<String>) -> Option<Leader> {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| {
            series
                .episodes
                .iter()
                .all(|id| completed_episodes.contains(*id))
        })
        .map(|series| Leader::of(series.reward_leader_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_sky_unlock_requires_every_episode_and_remains_unpublished() {
        assert_eq!(
            episode("six_sky_ep2").unwrap().prerequisite,
            Some("six_sky_ep1")
        );
        assert_eq!(
            episode("six_sky_ep3").unwrap().prerequisite,
            Some("six_sky_ep2")
        );
        assert!(!episode_is_published("six_sky_ep1"));
        assert!(!episode_is_published("six_sky_ep2"));
        assert!(!episode_is_published("six_sky_ep3"));
        assert_eq!(episode("unknown"), None);
        let empty = std::collections::BTreeSet::new();
        assert!(prerequisite_satisfied("six_sky_ep1", &empty));
        assert!(!prerequisite_satisfied("six_sky_ep2", &empty));
        assert!(!prerequisite_satisfied("unknown", &empty));

        let mut completed = std::collections::BTreeSet::from(["six_sky_ep1".to_string()]);
        assert_eq!(earned_leader(&completed), None);
        completed.insert("six_sky_ep2".to_string());
        assert_eq!(earned_leader(&completed), None);
        completed.insert("six_sky_ep3".to_string());
        assert_eq!(earned_leader(&completed), Some(Leader::of("lady_six_sky")));
    }

    #[test]
    fn campaign_heroes_exist_in_the_catalog() {
        for episode in CAMPAIGN_EPISODES {
            assert_eq!(episode.advisor().id(), episode.advisor_id);
        }
        for series in CAMPAIGN_SERIES {
            assert_eq!(
                Leader::of(series.reward_leader_id).id(),
                series.reward_leader_id
            );
        }
    }
}
