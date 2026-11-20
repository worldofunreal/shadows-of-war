use crate::commerce::owns_leader;
use crate::leaders::Leader;

/// First campaign mini-level launched for new players on first boot.
pub const INTRO_EPISODE_ID: &str = "boudica";

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
    /// Catalog id of the hero unlocked by finishing the series, if fixed.
    pub reward_leader_id: Option<&'static str>,
    /// Exclusive cosmetic skin id unlocked by finishing the series.
    pub reward_skin_id: Option<&'static str>,
    /// Candidate Common heroes offered as a 1-of-N free reward upon completion.
    pub hero_choice_pool: &'static [&'static str],
}

pub const ADVANCED_TACTICS_HERO_CHOICES: &[&str] = &["joan_of_arc", "lakshmibai", "alexander"];

pub const CAMPAIGN_EPISODES: &[CampaignEpisode] = &[
    CampaignEpisode {
        id: "tactics_01_economy",
        series_id: "tactics",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "europe",
        map_width: 1600,
        map_height: 920,
        title: "Farm & Forge",
        subtitle: "Tactics I · Economy & Buildings",
        title_key: "lobbies.campaign_tactics_01_economy_title",
        subtitle_key: "lobbies.campaign_tactics_01_economy_subtitle",
        marks_intro_completed: true,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "tactics_02_defense",
        series_id: "tactics",
        prerequisite: Some("tactics_01_economy"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "bajacalifornia",
        map_width: 1356,
        map_height: 1600,
        title: "Hold the Line",
        subtitle: "Tactics II · Towers & Troop Rate",
        title_key: "lobbies.campaign_tactics_02_defense_title",
        subtitle_key: "lobbies.campaign_tactics_02_defense_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "tactics_03_amphibious",
        series_id: "tactics",
        prerequisite: Some("tactics_02_defense"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "europe",
        map_width: 1600,
        map_height: 920,
        title: "Across the Strait",
        subtitle: "Tactics III · Amphibious Assault",
        title_key: "lobbies.campaign_tactics_03_amphibious_title",
        subtitle_key: "lobbies.campaign_tactics_03_amphibious_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "adv_tactics_01_naval",
        series_id: "advanced_tactics",
        prerequisite: Some("tactics_03_amphibious"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "southeastasia",
        map_width: 1600,
        map_height: 952,
        title: "Rule the Waves",
        subtitle: "Advanced I · Port & Warship",
        title_key: "lobbies.campaign_adv_tactics_01_naval_title",
        subtitle_key: "lobbies.campaign_adv_tactics_01_naval_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "adv_tactics_02_atomic",
        series_id: "advanced_tactics",
        prerequisite: Some("adv_tactics_01_naval"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "northamerica",
        map_width: 1600,
        map_height: 824,
        title: "Atomic Age",
        subtitle: "Advanced II · Nukes & SAM",
        title_key: "lobbies.campaign_adv_tactics_02_atomic_title",
        subtitle_key: "lobbies.campaign_adv_tactics_02_atomic_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "adv_tactics_03_skirmish",
        series_id: "advanced_tactics",
        prerequisite: Some("adv_tactics_02_atomic"),
        unlock_leader_id: None,
        advisor_id: "sun_tzu",
        map_id: "africa",
        map_width: 1532,
        map_height: 1600,
        title: "Total War",
        subtitle: "Advanced III · Bot Skirmish",
        title_key: "lobbies.campaign_adv_tactics_03_skirmish_title",
        subtitle_key: "lobbies.campaign_adv_tactics_03_skirmish_subtitle",
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
        map_width: 1600,
        map_height: 900,
        title: "Rise of the Iceni",
        subtitle: "Tutorial · Boudica",
        title_key: "lobbies.campaign_boudica_title",
        subtitle_key: "lobbies.campaign_boudica_subtitle",
        marks_intro_completed: true,
        server_owned_tutorial: true,
    },
    CampaignEpisode {
        id: "blitz_05",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "europe",
        map_width: 1600,
        map_height: 920,
        title: "Iron Gate",
        subtitle: "Blitz · Level 5 · Boss",
        title_key: "lobbies.campaign_blitz_05_title",
        subtitle_key: "lobbies.campaign_blitz_05_subtitle",
        marks_intro_completed: true,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_06",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "bajacalifornia",
        map_width: 1356,
        map_height: 1600,
        title: "Domino Shore",
        subtitle: "Blitz · Level 6 · Cascade",
        title_key: "lobbies.campaign_blitz_06_title",
        subtitle_key: "lobbies.campaign_blitz_06_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_07",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "southeastasia",
        map_width: 1600,
        map_height: 952,
        title: "Twin Flank",
        subtitle: "Blitz · Level 7 · Flank",
        title_key: "lobbies.campaign_blitz_07_title",
        subtitle_key: "lobbies.campaign_blitz_07_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_08",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "africa",
        map_width: 1532,
        map_height: 1600,
        title: "Mint Rush",
        subtitle: "Blitz · Level 8 · Eco Rush",
        title_key: "lobbies.campaign_blitz_08_title",
        subtitle_key: "lobbies.campaign_blitz_08_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_09",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "northamerica",
        map_width: 1600,
        map_height: 824,
        title: "Highland Breach",
        subtitle: "Blitz · Level 9 · High Stakes",
        title_key: "lobbies.campaign_blitz_09_title",
        subtitle_key: "lobbies.campaign_blitz_09_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_10",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "europe",
        map_width: 1600,
        map_height: 920,
        title: "Citadel Storm",
        subtitle: "Blitz · Level 10 · Boss",
        title_key: "lobbies.campaign_blitz_10_title",
        subtitle_key: "lobbies.campaign_blitz_10_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_11",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "bajacalifornia",
        map_width: 1356,
        map_height: 1600,
        title: "Chain Break",
        subtitle: "Blitz · Level 11 · Cascade",
        title_key: "lobbies.campaign_blitz_11_title",
        subtitle_key: "lobbies.campaign_blitz_11_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_12",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "southeastasia",
        map_width: 1600,
        map_height: 952,
        title: "Desert Pincer",
        subtitle: "Blitz · Level 12 · Flank",
        title_key: "lobbies.campaign_blitz_12_title",
        subtitle_key: "lobbies.campaign_blitz_12_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_13",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "africa",
        map_width: 1532,
        map_height: 1600,
        title: "Foundry Surge",
        subtitle: "Blitz · Level 13 · Eco Rush",
        title_key: "lobbies.campaign_blitz_13_title",
        subtitle_key: "lobbies.campaign_blitz_13_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_14",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "northamerica",
        map_width: 1600,
        map_height: 824,
        title: "Crimson Ridge",
        subtitle: "Blitz · Level 14 · High Stakes",
        title_key: "lobbies.campaign_blitz_14_title",
        subtitle_key: "lobbies.campaign_blitz_14_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_15",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "europe",
        map_width: 1600,
        map_height: 920,
        title: "Imperial Crown",
        subtitle: "Blitz · Level 15 · Boss",
        title_key: "lobbies.campaign_blitz_15_title",
        subtitle_key: "lobbies.campaign_blitz_15_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_16",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "bajacalifornia",
        map_width: 1356,
        map_height: 1600,
        title: "Swift Sweep",
        subtitle: "Blitz · Level 16 · Cascade",
        title_key: "lobbies.campaign_blitz_16_title",
        subtitle_key: "lobbies.campaign_blitz_16_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_17",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "southeastasia",
        map_width: 1600,
        map_height: 952,
        title: "Allied Hammer",
        subtitle: "Blitz · Level 17 · Flank",
        title_key: "lobbies.campaign_blitz_17_title",
        subtitle_key: "lobbies.campaign_blitz_17_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_18",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "africa",
        map_width: 1532,
        map_height: 1600,
        title: "Golden Forge",
        subtitle: "Blitz · Level 18 · Eco Rush",
        title_key: "lobbies.campaign_blitz_18_title",
        subtitle_key: "lobbies.campaign_blitz_18_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_19",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "northamerica",
        map_width: 1600,
        map_height: 824,
        title: "Final Bastion",
        subtitle: "Blitz · Level 19 · High Stakes",
        title_key: "lobbies.campaign_blitz_19_title",
        subtitle_key: "lobbies.campaign_blitz_19_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "blitz_20",
        series_id: "conquest_blitz",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "caesar",
        map_id: "europe",
        map_width: 1600,
        map_height: 920,
        title: "Grand Conquest",
        subtitle: "Blitz · Level 20 · Boss",
        title_key: "lobbies.campaign_blitz_20_title",
        subtitle_key: "lobbies.campaign_blitz_20_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_ep1",
        series_id: "lady_six_sky",
        prerequisite: None,
        unlock_leader_id: Some("lady_six_sky"),
        advisor_id: "lady_six_sky",
        map_id: "northamerica",
        map_width: 1600,
        map_height: 824,
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
        map_width: 1600,
        map_height: 824,
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
        map_width: 1600,
        map_height: 824,
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
        id: "tactics",
        episodes: &[
            "tactics_01_economy",
            "tactics_02_defense",
            "tactics_03_amphibious",
        ],
        reward_leader_id: Some("sun_tzu"),
        reward_skin_id: None,
        hero_choice_pool: &[],
    },
    CampaignSeries {
        id: "advanced_tactics",
        episodes: &[
            "adv_tactics_01_naval",
            "adv_tactics_02_atomic",
            "adv_tactics_03_skirmish",
        ],
        reward_leader_id: None,
        reward_skin_id: Some("imperial_vanguard"),
        hero_choice_pool: ADVANCED_TACTICS_HERO_CHOICES,
    },
    CampaignSeries {
        id: "conquest_blitz",
        episodes: &[
            "blitz_05", "blitz_06", "blitz_07", "blitz_08", "blitz_09", "blitz_10",
            "blitz_11", "blitz_12", "blitz_13", "blitz_14", "blitz_15", "blitz_16",
            "blitz_17", "blitz_18", "blitz_19", "blitz_20",
        ],
        reward_leader_id: Some("caesar"),
        reward_skin_id: None,
        hero_choice_pool: &[],
    },
    CampaignSeries {
        id: "lady_six_sky",
        episodes: &["six_sky_ep1", "six_sky_ep2", "six_sky_ep3"],
        reward_leader_id: Some("lady_six_sky"),
        reward_skin_id: None,
        hero_choice_pool: &[],
    },
];

// Episodes are playable only when listed here. Six Sky ep1 also needs the hero
// (see access_granted).
pub const PUBLISHED_EPISODES: &[&str] = &[
    "tactics_01_economy",
    "tactics_02_defense",
    "tactics_03_amphibious",
    "adv_tactics_01_naval",
    "adv_tactics_02_atomic",
    "adv_tactics_03_skirmish",
    "boudica",
    "blitz_05",
    "blitz_06",
    "blitz_07",
    "blitz_08",
    "blitz_09",
    "blitz_10",
    "blitz_11",
    "blitz_12",
    "blitz_13",
    "blitz_14",
    "blitz_15",
    "blitz_16",
    "blitz_17",
    "blitz_18",
    "blitz_19",
    "blitz_20",
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

pub fn series_completed_count(
    series_id: &str,
    completed_episodes: &std::collections::BTreeSet<String>,
) -> u32 {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| series.id == series_id)
        .map(|series| {
            series
                .episodes
                .iter()
                .filter(|id| completed_episodes.contains(**id))
                .count() as u32
        })
        .unwrap_or(0)
}

pub fn tactics_completed(completed_episodes: &std::collections::BTreeSet<String>) -> bool {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| series.id == "tactics")
        .is_some_and(|series| {
            series
                .episodes
                .iter()
                .all(|id| completed_episodes.contains(*id))
        })
}

pub fn advanced_tactics_completed(completed_episodes: &std::collections::BTreeSet<String>) -> bool {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| series.id == "advanced_tactics")
        .is_some_and(|series| {
            series
                .episodes
                .iter()
                .all(|id| completed_episodes.contains(*id))
        })
}

pub fn earned_skin_for_episode(
    episode_id: &str,
    completed_episodes: &std::collections::BTreeSet<String>,
) -> Option<&'static str> {
    CAMPAIGN_SERIES.iter().find_map(|series| {
        (series.episodes.contains(&episode_id)
            && series
                .episodes
                .iter()
                .all(|id| completed_episodes.contains(*id)))
        .then_some(series.reward_skin_id)
        .flatten()
    })
}

pub fn earned_leader(completed_episodes: &std::collections::BTreeSet<String>) -> Option<Leader> {
    CAMPAIGN_SERIES
        .iter()
        .filter_map(|series| {
            series
                .episodes
                .iter()
                .all(|id| completed_episodes.contains(*id))
                .then_some(series.reward_leader_id)
                .flatten()
        })
        .next()
        .map(Leader::of)
}

pub fn earned_leader_for_episode(
    episode_id: &str,
    completed_episodes: &std::collections::BTreeSet<String>,
) -> Option<Leader> {
    CAMPAIGN_SERIES
        .iter()
        .filter_map(|series| {
            (series.episodes.contains(&episode_id)
                && series
                    .episodes
                    .iter()
                    .all(|id| completed_episodes.contains(*id)))
            .then_some(series.reward_leader_id)
            .flatten()
        })
        .next()
        .map(Leader::of)
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
            if let Some(leader_id) = series.reward_leader_id {
                assert_eq!(Leader::of(leader_id).id(), leader_id);
            }
            for choice_id in series.hero_choice_pool {
                assert_eq!(Leader::of(choice_id).id(), *choice_id);
            }
        }
    }

    #[test]
    fn conquest_blitz_nodes_5_through_20_are_published_and_open() {
        let empty = std::collections::BTreeSet::new();
        for n in 5..=20u32 {
            let id = format!("blitz_{n:02}");
            let ep = episode(&id).unwrap_or_else(|| panic!("missing episode {id}"));
            assert!(episode_is_published(&id), "{id} must be published");
            assert_eq!(ep.prerequisite, None);
            assert!(access_granted(&id, &empty, &empty), "{id} must be unlocked");
            assert_eq!(ep.series_id, "conquest_blitz");
        }
        let mut completed: std::collections::BTreeSet<String> = (5..=20)
            .map(|n| format!("blitz_{n:02}"))
            .collect();
        assert_eq!(earned_leader(&completed), Some(Leader::of("caesar")));
        completed.remove("blitz_20");
        assert_eq!(earned_leader(&completed), None);
    }
}
