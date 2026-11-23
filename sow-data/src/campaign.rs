use crate::commerce::owns_leader;
use crate::leaders::Leader;

/// First campaign mini-level launched for new players on first boot.
pub const INTRO_EPISODE_ID: &str = "six_sky_maya_01";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignEpisode {
    pub id: &'static str,
    pub series_id: &'static str,
    pub prerequisite: Option<&'static str>,
    /// Catalog id of a hero whose ownership (bought or earned) opens this episode.
    pub unlock_leader_id: Option<&'static str>,
    /// Catalog id of the hero who narrates the episode.
    pub advisor_id: &'static str,
    /// Optional art slug and display name featured on the campaign episode card.
    /// This is presentation metadata; it does not add a selectable hero.
    pub featured_hero_slug: Option<&'static str>,
    pub featured_hero_name: Option<&'static str>,
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

    pub fn has_local_peten_map(&self) -> bool {
        matches!(
            self.id,
            "six_sky_maya_03" | "six_sky_maya_04" | "six_sky_maya_05"
                | "six_sky_maya_06" | "six_sky_maya_07" | "six_sky_maya_08"
        )
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
        id: "boudica",
        series_id: "boudica",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "boudica",
        featured_hero_slug: None,
        featured_hero_name: None,
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
        id: "six_sky_maya_01",
        series_id: "lady_six_sky",
        prerequisite: None,
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("kinichil_kab"),
        featured_hero_name: Some("K’inichil Kab"),
        map_id: "peten_clearing",
        map_width: 176,
        map_height: 100,
        title: "Moonlit Arrival",
        subtitle: "Naranjo, Petén · 682–693 CE",
        title_key: "lobbies.campaign_six_sky_maya_01_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_01_subtitle",
        marks_intro_completed: true,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_maya_02",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_maya_01"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("kahk_tiliw_child_693"),
        featured_hero_name: Some("K’ahk’ Tiliw Chan Chaak · 693"),
        map_id: "bital_pass",
        map_width: 224,
        map_height: 156,
        title: "A Crown for a Child",
        subtitle: "Naranjo · 688–693 CE",
        title_key: "lobbies.campaign_six_sky_maya_02_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_02_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_maya_03",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_maya_02"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("siyaj_kawiil"),
        featured_hero_name: Some("Siyaj K’awiil"),
        map_id: "peten_yellow_rabbit",
        map_width: 224,
        map_height: 156,
        title: "Yellow Rabbit, Two Headlines",
        subtitle: "Naranjo · 695 CE",
        title_key: "lobbies.campaign_six_sky_maya_03_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_03_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_maya_04",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_maya_03"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("six_sky"),
        featured_hero_name: Some("Lady Six Sky"),
        map_id: "peten_komkom",
        map_width: 224,
        map_height: 156,
        title: "Komkom Keeps Receipts",
        subtitle: "Naranjo · 696 CE",
        title_key: "lobbies.campaign_six_sky_maya_04_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_04_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_maya_05",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_maya_04"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("six_sky"),
        featured_hero_name: Some("Lady Six Sky"),
        map_id: "peten_bahlam_jol",
        map_width: 224,
        map_height: 156,
        title: "Bahlam Jol Makes a Point",
        subtitle: "Naranjo · 697 CE",
        title_key: "lobbies.campaign_six_sky_maya_05_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_05_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_maya_06",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_maya_05"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("itzamnaaj_bahlam"),
        featured_hero_name: Some("Itzamnaaj Bahlam"),
        map_id: "peten_ucanal",
        map_width: 224,
        map_height: 156,
        title: "Stela 24 Has a Version",
        subtitle: "Ucanal · 698–702 CE",
        title_key: "lobbies.campaign_six_sky_maya_06_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_06_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_maya_07",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_maya_06"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("kahk_tiliw"),
        featured_hero_name: Some("K’ahk’ Tiliw Chan Chaak"),
        map_id: "peten_dos_pilas",
        map_width: 224,
        map_height: 156,
        title: "The Stones Go Quiet",
        subtitle: "Naranjo · 714–726 CE",
        title_key: "lobbies.campaign_six_sky_maya_07_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_07_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
    CampaignEpisode {
        id: "six_sky_maya_08",
        series_id: "lady_six_sky",
        prerequisite: Some("six_sky_maya_07"),
        unlock_leader_id: None,
        advisor_id: "lady_six_sky",
        featured_hero_slug: Some("yikin_chan_kawiil"),
        featured_hero_name: Some("Yik’in Chan K’awiil"),
        map_id: "peten_tikal",
        map_width: 224,
        map_height: 156,
        title: "Tikal: The Nuclear Footnote",
        subtitle: "Alternate history · 744 CE",
        title_key: "lobbies.campaign_six_sky_maya_08_title",
        subtitle_key: "lobbies.campaign_six_sky_maya_08_subtitle",
        marks_intro_completed: false,
        server_owned_tutorial: false,
    },
];

pub const CAMPAIGN_SERIES: &[CampaignSeries] = &[
    CampaignSeries {
        id: "lady_six_sky",
        episodes: &[
            "six_sky_maya_01",
            "six_sky_maya_02",
            "six_sky_maya_03",
            "six_sky_maya_04",
            "six_sky_maya_05",
            "six_sky_maya_06",
            "six_sky_maya_07",
            "six_sky_maya_08",
        ],
        reward_leader_id: Some("lady_six_sky"),
        reward_skin_id: None,
        hero_choice_pool: &[],
    },
];

// Episodes are playable only when listed here.
pub const PUBLISHED_EPISODES: &[&str] = &[
    "boudica",
    "six_sky_maya_01",
    "six_sky_maya_02",
    "six_sky_maya_03",
    "six_sky_maya_04",
    "six_sky_maya_05",
    "six_sky_maya_06",
    "six_sky_maya_07",
    "six_sky_maya_08",
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

/// Completion gate for the onboarding campaign and the Daily / Weekly Ops it unlocks.
pub fn tactics_completed(completed_episodes: &std::collections::BTreeSet<String>) -> bool {
    CAMPAIGN_SERIES
        .iter()
        .find(|series| series.id == "lady_six_sky")
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
    fn six_sky_campaign_is_the_open_eight_episode_intro() {
        assert_eq!(INTRO_EPISODE_ID, "six_sky_maya_01");
        for number in 1..=8 {
            let id = format!("six_sky_maya_{number:02}");
            let expected = (number > 1).then(|| format!("six_sky_maya_{:02}", number - 1));
            assert!(episode_is_published(&id));
            assert_eq!(episode(&id).unwrap().prerequisite, expected.as_deref());
        }
        assert_eq!(episode("unknown"), None);
        let empty = std::collections::BTreeSet::new();
        assert!(access_granted("six_sky_maya_01", &empty, &empty));
        assert!(!access_granted("six_sky_maya_02", &empty, &empty));
        assert!(access_granted("boudica", &empty, &empty));
        assert!(!access_granted("unknown", &empty, &empty));

        let mut completed = std::collections::BTreeSet::new();
        assert_eq!(earned_leader(&completed), None);
        for number in 1..=7 {
            completed.insert(format!("six_sky_maya_{number:02}"));
        }
        assert!(!tactics_completed(&completed));
        assert_eq!(earned_leader(&completed), None);
        completed.insert("six_sky_maya_08".to_string());
        assert!(tactics_completed(&completed));
        assert_eq!(earned_leader(&completed), Some(Leader::of("lady_six_sky")));
        assert!(access_granted("six_sky_maya_02", &completed, &empty));
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
}
