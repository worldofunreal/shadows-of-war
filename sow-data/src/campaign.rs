use crate::commerce::owns_leader;
use crate::leaders::Leader;

/// First campaign mini-level launched for new players on first boot.
pub const INTRO_EPISODE_ID: &str = "tactics_01_expand";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignEpisode {
    pub id: &'static str,
    pub series_id: &'static str,
    pub prerequisite: Option<&'static str>,
    /// Catalog id of a hero whose ownership (bought or earned) opens this episode.
    pub unlock_leader_id: Option<&'static str>,
    /// Catalog id of the hero who narrates the episode.
    pub advisor_id: &'static str,
    pub map_id: &'static str,
    pub map_width: u32,
    pub map_height: u32,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub title_key: &'static str,
    pub subtitle_key: &'static str,
    /// Completing this episode also marks onboarding intro tutorial complete.
    pub marks_intro_completed: bool,
    /// Legacy server-owned `"tutorial"` receipt exit flow (`boudica` only).
    pub server_owned_tutorial: bool,
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
        id: "tactics_01_expand",
        series_id: "art_of_war",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "europe",
        map_width: 1000,
        map_height: 576,
        title: "Island Strike",
        subtitle: "Tactics I · Sun Tzu",
        title_key: "lobbies.campaign_tactics_01_expand_title",
        subtitle_key: "lobbies.campaign_tactics_01_expand_subtitle",
        marks_intro_completed: true,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "tactics_02_ask_ally",
        series_id: "art_of_war",
        prerequisite: Some("tactics_01_expand"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "bajacalifornia",
        map_width: 848,
        map_height: 1000,
        title: "Friends with Gold",
        subtitle: "Tactics II · Diplomacy & Aid",
        title_key: "lobbies.campaign_tactics_02_ask_ally_title",
        subtitle_key: "lobbies.campaign_tactics_02_ask_ally_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "tactics_03_farm_vs_city",
        series_id: "art_of_war",
        prerequisite: Some("tactics_02_ask_ally"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "europe",
        map_width: 1000,
        map_height: 576,
        title: "Bread or Barracks",
        subtitle: "Tactics III · Farm vs City",
        title_key: "lobbies.campaign_tactics_03_farm_vs_city_title",
        subtitle_key: "lobbies.campaign_tactics_03_farm_vs_city_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "tactics_04_factory_gold",
        series_id: "art_of_war",
        prerequisite: Some("tactics_03_farm_vs_city"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "southeastasia",
        map_width: 1000,
        map_height: 592,
        title: "Compound Coin",
        subtitle: "Tactics IV · Factory Lv.4",
        title_key: "lobbies.campaign_tactics_04_factory_gold_title",
        subtitle_key: "lobbies.campaign_tactics_04_factory_gold_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "boudica",
        series_id: "boudica",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "boudica",
        map_id: "eastanglia",
        map_width: 896,
        map_height: 504,
        title: "Rise of the Iceni",
        subtitle: "Tutorial · Boudica",
        title_key: "lobbies.campaign_boudica_title",
        subtitle_key: "lobbies.campaign_boudica_subtitle",
        marks_intro_completed: true,
        server_owned_tutorial: true,
    },
    CampaignEpisode {
        id: "six_sky_ep1",
        series_id: "lady_six_sky",
        prerequisite: None,
        unlock_leader_id: Some("lady_six_sky"),
        advisor_id: "lady_six_sky",
        map_id: "northamerica",
        map_width: 1000,
        map_height: 516,
        title: "Arrival",
        subtitle: "Lady Six Sky · 682 CE",
        title_key: "lobbies.campaign_six_sky_ep1_title",
        subtitle_key: "lobbies.campaign_six_sky_ep1_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_ep2",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_ep1"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        map_id: "northamerica",
        map_width: 1000,
        map_height: 516,
        title: "Regent's Fire",
        subtitle: "Lady Six Sky · 693 CE",
        title_key: "lobbies.campaign_six_sky_ep2_title",
        subtitle_key: "lobbies.campaign_six_sky_ep2_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_ep3",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_ep2"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        map_id: "northamerica",
        map_width: 1000,
        map_height: 516,
        title: "Moon Goddess",
        subtitle: "Lady Six Sky · 726–741 CE",
        title_key: "lobbies.campaign_six_sky_ep3_title",
        subtitle_key: "lobbies.campaign_six_sky_ep3_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
];

pub const CAMPAIGN_SERIES: &[CampaignSeries] = &[
    CampaignSeries {
        id: "art_of_war",
        episodes: &[
            "tactics_01_expand",
            "tactics_02_ask_ally",
            "tactics_03_farm_vs_city",
            "tactics_04_factory_gold",
        ],
        reward_leader_id: "sun_tzu",
    },
    CampaignSeries {
        id: "lady_six_sky",
        episodes: &["six_sky_ep1", "six_sky_ep2", "six_sky_ep3"],
        reward_leader_id: "lady_six_sky",
    },
];

// Episodes are playable only when listed here. Six Sky ep1 also needs the hero
// (see access_granted).
pub const PUBLISHED_EPISODES: &[&str] = &[
    "tactics_01_expand",
    "tactics_02_ask_ally",
    "tactics_03_farm_vs_city",
    "tactics_04_factory_gold",
    "boudica",
    "six_sky_ep1",
    "six_sky_ep2",
    "six_sky_ep3",
];

pub fn episode(id: &str) -> Option<&'static CampaignEpisode> {
    CAMPAIGN_EPISODES.iter().find(|episode| episode.id == id)
}

pub fn episode_is_published(id: &str) -> bool {
    PUBLISHED_EPISODES.contains(&id)
}

/// Order and hero-ownership gates for an episode. Publishing is checked separately.
pub fn access_granted(
    id: &str,
    completed_episodes: &std::collections::BTreeSet<String>,
    owned_leaders: &std::collections::BTreeSet<String>,
) -> bool {
    episode(id).is_some_and(|episode| {
        episode
            .prerequisite
            .is_none_or(|required| completed_episodes.contains(required))
            && episode
                .unlock_leader_id
                .is_none_or(|leader| owns_leader(owned_leaders, Leader::of(leader)))
    })
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

pub fn earned_leader_for_episode(
    episode_id: &str,
    completed_episodes: &std::collections::BTreeSet<String>,
) -> Option<Leader> {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| {
            series.episodes.contains(&episode_id)
                && series
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
    fn six_sky_unlock_requires_hero_then_every_episode() {
        assert_eq!(
            episode("six_sky_ep2").unwrap().prerequisite,
            Some("six_sky_ep1")
        );
        assert_eq!(
            episode("six_sky_ep3").unwrap().prerequisite,
            Some("six_sky_ep2")
        );
        assert!(episode_is_published("six_sky_ep1"));
        assert!(episode_is_published("six_sky_ep2"));
        assert!(episode_is_published("six_sky_ep3"));
        assert_eq!(episode("unknown"), None);
        let empty = std::collections::BTreeSet::new();
        let owned = std::collections::BTreeSet::from(["lady_six_sky".to_string()]);
        assert!(!access_granted("six_sky_ep1", &empty, &empty));
        assert!(access_granted("six_sky_ep1", &empty, &owned));
        assert!(!access_granted("six_sky_ep2", &empty, &owned));
        assert!(access_granted("boudica", &empty, &empty));
        assert!(!access_granted("unknown", &empty, &owned));

        let mut completed = std::collections::BTreeSet::from(["six_sky_ep1".to_string()]);
        assert_eq!(earned_leader(&completed), None);
        completed.insert("six_sky_ep2".to_string());
        assert_eq!(earned_leader(&completed), None);
        completed.insert("six_sky_ep3".to_string());
        assert_eq!(earned_leader(&completed), Some(Leader::of("lady_six_sky")));
        assert!(access_granted("six_sky_ep2", &completed, &empty));
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
