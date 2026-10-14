use crate::leaders::Leader;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignEpisode {
    pub id: &'static str,
    pub prerequisite: Option<&'static str>,
    pub advisor: Leader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignSeries {
    pub id: &'static str,
    pub episodes: &'static [&'static str],
    pub reward_leader: Leader,
}

pub const CAMPAIGN_EPISODES: &[CampaignEpisode] = &[
    CampaignEpisode { id: "boudica", prerequisite: None, advisor: Leader::Boudica },
    CampaignEpisode { id: "six_sky_ep1", prerequisite: None, advisor: Leader::LadySixSky },
    CampaignEpisode { id: "six_sky_ep2", prerequisite: Some("six_sky_ep1"), advisor: Leader::LadySixSky },
    CampaignEpisode { id: "six_sky_ep3", prerequisite: Some("six_sky_ep2"), advisor: Leader::LadySixSky },
];

pub const CAMPAIGN_SERIES: &[CampaignSeries] = &[CampaignSeries {
    id: "lady_six_sky",
    episodes: &["six_sky_ep1", "six_sky_ep2", "six_sky_ep3"],
    reward_leader: Leader::LadySixSky,
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
    CAMPAIGN_SERIES.iter().find(|series| series.episodes.contains(&id))
}

pub fn earned_leader(completed_episodes: &std::collections::BTreeSet<String>) -> Option<Leader> {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| series.episodes.iter().all(|id| completed_episodes.contains(*id)))
        .map(|series| series.reward_leader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_sky_unlock_requires_every_episode_and_remains_unpublished() {
        assert_eq!(episode("six_sky_ep2").unwrap().prerequisite, Some("six_sky_ep1"));
        assert_eq!(episode("six_sky_ep3").unwrap().prerequisite, Some("six_sky_ep2"));
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
        assert_eq!(earned_leader(&completed), Some(Leader::LadySixSky));
    }
}
