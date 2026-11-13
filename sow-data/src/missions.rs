//! Missions and directives system for player onboarding, engagement, and retention.
//!
//! Three mission tiers forming a D1 -> D3 -> D7 retention funnel:
//! 1. **Onboarding ("Camino del Conquistador", Lv 1+)**: 6 sequential milestones;
//!    Step N+1 unlocks only after Step N is claimed.
//! 2. **Daily Operations (Lv 3+)**: 3 lane-balanced daily missions (period-scoped to UTC day)
//!    with 1 free daily swap + optional rewarded-ad swap.
//! 3. **Weekly Operations (Lv 5+)**: 3 lane-balanced weekly missions (period-scoped to rotation week)
//!    with 1 free weekly swap + optional rewarded-ad swap.

use crate::geo_entities::{Rarity, Region};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const DAILY_UNLOCK_LEVEL: u32 = 3;
pub const WEEKLY_UNLOCK_LEVEL: u32 = 5;
pub const MAX_FREE_SWAPS_PER_PERIOD: u8 = 1;
pub const MAX_TOTAL_SWAPS_PER_PERIOD: u8 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    #[default]
    Ffa,
    Teams,
    HumansVsNations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionCategory {
    Onboarding,
    Daily,
    Weekly,
}

pub fn min_level_for_category(category: MissionCategory) -> u32 {
    match category {
        MissionCategory::Onboarding => 1,
        MissionCategory::Daily => DAILY_UNLOCK_LEVEL,
        MissionCategory::Weekly => WEEKLY_UNLOCK_LEVEL,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceAxis {
    Onboarding,
    Mode,
    Region,
    Rarity,
    Skill,
    Macro,
    Combat,
    Discovery,
}

impl PreferenceAxis {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Onboarding => "onboarding",
            Self::Mode => "mode",
            Self::Region => "region",
            Self::Rarity => "rarity",
            Self::Skill => "skill",
            Self::Macro => "macro",
            Self::Combat => "combat",
            Self::Discovery => "discovery",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionActionTarget {
    #[default]
    Battle,
    Ranked,
    HeroesOceania,
    HeroesEurope,
    HeroesAsia,
    HeroesAmericas,
    HeroesAfrica,
    HeroesCommon,
    HeroesRare,
    HeroesEpic,
    HeroesLegendary,
    HeroesFreeRotation,
    HeroesAll,
    Teams,
    Hnv,
    Ffa,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionRequirement {
    CompleteTutorial,
    BuildStructures {
        count: u32,
    },
    PlayFreeRotationHero {
        count: u32,
    },
    PlayMode {
        mode: MatchMode,
        count: u32,
    },
    WinMode {
        mode: MatchMode,
        count: u32,
    },
    SurviveInMode {
        mode: MatchMode,
        min_seconds: u32,
        count: u32,
    },
    PeakTerritoryRank {
        mode: Option<MatchMode>,
        max_rank: u16,
        min_contenders: u16,
        count: u32,
    },
    PlayWithHeroConstraints {
        region: Option<Region>,
        rarity: Option<Rarity>,
        count: u32,
    },
    WinWithConstraints {
        mode: Option<MatchMode>,
        region: Option<Region>,
        rarity: Option<Rarity>,
        count: u32,
    },
    DistinctHeroes {
        count: u32,
    },
    PlayMatches {
        count: u32,
    },
    WinMatches {
        count: u32,
    },
    EliminateUnits {
        count: u32,
    },
    PlayRankedMatches {
        count: u32,
    },
    WinRankedMatches {
        count: u32,
    },
    DefeatEmpires {
        count: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionDefinition {
    pub id: &'static str,
    pub category: MissionCategory,
    pub title_key: &'static str,
    pub description_key: &'static str,
    pub crowns_reward: u64,
    pub gems_reward: u64,
    pub target: u32,
    pub requirement: MissionRequirement,
    pub icon: &'static str,
    pub order: u32,
    /// Diagnostic lane (`0 = mode/skill`, `1 = hero/region/rarity`, `2 = macro/combat/alt`).
    pub lane: u8,
    pub axis: PreferenceAxis,
    pub tag: &'static str,
    pub action_target: MissionActionTarget,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionRecord {
    pub id: String,
    pub progress: u32,
    pub completed: bool,
    pub claimed: bool,
    #[serde(default)]
    pub claimed_at: Option<u64>,
    #[serde(default)]
    pub doubled_by_ad: bool,
    #[serde(default)]
    pub seen_keys: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionView {
    pub id: String,
    pub category: String,
    pub title_key: String,
    pub description_key: String,
    pub target: u32,
    pub progress: u32,
    pub completed: bool,
    pub claimed: bool,
    pub crowns_reward: u64,
    pub gems_reward: u64,
    pub icon: String,
    pub order: u32,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub unlock_level: u32,
    #[serde(default)]
    pub axis: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub action_target: MissionActionTarget,
    #[serde(default)]
    pub swappable: bool,
}

pub const ONBOARDING_MISSIONS: &[MissionDefinition] = &[
    MissionDefinition {
        id: "onboarding_1_tutorial",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_tutorial_title",
        description_key: "missions.onboarding_tutorial_desc",
        crowns_reward: 40,
        gems_reward: 0,
        target: 1,
        requirement: MissionRequirement::CompleteTutorial,
        icon: "shield",
        order: 1,
        lane: 0,
        axis: PreferenceAxis::Onboarding,
        tag: "tutorial",
        action_target: MissionActionTarget::Battle,
    },
    MissionDefinition {
        id: "onboarding_2_baptism",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_baptism_title",
        description_key: "missions.onboarding_baptism_desc",
        crowns_reward: 40,
        gems_reward: 0,
        target: 2,
        requirement: MissionRequirement::PlayMatches { count: 2 },
        icon: "swords",
        order: 2,
        lane: 0,
        axis: PreferenceAxis::Onboarding,
        tag: "play_matches",
        action_target: MissionActionTarget::Battle,
    },
    MissionDefinition {
        id: "onboarding_3_rotation_hero",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_rotation_title",
        description_key: "missions.onboarding_rotation_desc",
        crowns_reward: 50,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::PlayFreeRotationHero { count: 2 },
        icon: "rotation",
        order: 3,
        lane: 1,
        axis: PreferenceAxis::Onboarding,
        tag: "free_rotation",
        action_target: MissionActionTarget::HeroesFreeRotation,
    },
    MissionDefinition {
        id: "onboarding_4_team_match",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_team_title",
        description_key: "missions.onboarding_team_desc",
        crowns_reward: 50,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::PlayMode {
            mode: MatchMode::Teams,
            count: 2,
        },
        icon: "team",
        order: 4,
        lane: 0,
        axis: PreferenceAxis::Onboarding,
        tag: "teams",
        action_target: MissionActionTarget::Teams,
    },
    MissionDefinition {
        id: "onboarding_5_empires",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_empires_title",
        description_key: "missions.onboarding_empires_desc",
        crowns_reward: 60,
        gems_reward: 1,
        target: 3,
        requirement: MissionRequirement::DefeatEmpires { count: 3 },
        icon: "shield",
        order: 5,
        lane: 2,
        axis: PreferenceAxis::Onboarding,
        tag: "defeat_empires",
        action_target: MissionActionTarget::Battle,
    },
    MissionDefinition {
        id: "onboarding_6_victories",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_victories_title",
        description_key: "missions.onboarding_victories_desc",
        crowns_reward: 80,
        gems_reward: 2,
        target: 2,
        requirement: MissionRequirement::WinMatches { count: 2 },
        icon: "crown",
        order: 6,
        lane: 0,
        axis: PreferenceAxis::Onboarding,
        tag: "wins",
        action_target: MissionActionTarget::Battle,
    },
];

pub const DAILY_MISSION_POOL: &[MissionDefinition] = &[
    // Lane 0: Mode & Territory Skill (zero hero paywall)
    MissionDefinition {
        id: "daily_ffa_survive_3m",
        category: MissionCategory::Daily,
        title_key: "missions.daily_ffa_survive_3m_title",
        description_key: "missions.daily_ffa_survive_3m_desc",
        crowns_reward: 30,
        gems_reward: 0,
        target: 1,
        requirement: MissionRequirement::SurviveInMode {
            mode: MatchMode::Ffa,
            min_seconds: 180,
            count: 1,
        },
        icon: "shield",
        order: 10,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "ffa",
        action_target: MissionActionTarget::Ffa,
    },
    MissionDefinition {
        id: "daily_ffa_top3",
        category: MissionCategory::Daily,
        title_key: "missions.daily_ffa_top3_title",
        description_key: "missions.daily_ffa_top3_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 1,
        requirement: MissionRequirement::PeakTerritoryRank {
            mode: Some(MatchMode::Ffa),
            max_rank: 3,
            min_contenders: 6,
            count: 1,
        },
        icon: "crown",
        order: 11,
        lane: 0,
        axis: PreferenceAxis::Skill,
        tag: "territory_top3",
        action_target: MissionActionTarget::Ffa,
    },
    MissionDefinition {
        id: "daily_peak_first",
        category: MissionCategory::Daily,
        title_key: "missions.daily_peak_first_title",
        description_key: "missions.daily_peak_first_desc",
        crowns_reward: 40,
        gems_reward: 1,
        target: 1,
        requirement: MissionRequirement::PeakTerritoryRank {
            mode: None,
            max_rank: 1,
            min_contenders: 4,
            count: 1,
        },
        icon: "crown",
        order: 12,
        lane: 0,
        axis: PreferenceAxis::Skill,
        tag: "territory_first",
        action_target: MissionActionTarget::Battle,
    },
    MissionDefinition {
        id: "daily_team_game",
        category: MissionCategory::Daily,
        title_key: "missions.daily_team_game_title",
        description_key: "missions.daily_team_game_desc",
        crowns_reward: 25,
        gems_reward: 0,
        target: 2,
        requirement: MissionRequirement::PlayMode {
            mode: MatchMode::Teams,
            count: 2,
        },
        icon: "team",
        order: 13,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "teams",
        action_target: MissionActionTarget::Teams,
    },
    MissionDefinition {
        id: "daily_hnv_win",
        category: MissionCategory::Daily,
        title_key: "missions.daily_hnv_win_title",
        description_key: "missions.daily_hnv_win_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 1,
        requirement: MissionRequirement::WinMode {
            mode: MatchMode::HumansVsNations,
            count: 1,
        },
        icon: "shield",
        order: 14,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "hnv",
        action_target: MissionActionTarget::Hnv,
    },
    MissionDefinition {
        id: "daily_ranked_3",
        category: MissionCategory::Daily,
        title_key: "missions.daily_ranked_3_title",
        description_key: "missions.daily_ranked_3_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 3,
        requirement: MissionRequirement::PlayRankedMatches { count: 3 },
        icon: "swords",
        order: 15,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "ranked",
        action_target: MissionActionTarget::Ranked,
    },
    MissionDefinition {
        id: "daily_ranked_victory",
        category: MissionCategory::Daily,
        title_key: "missions.daily_ranked_victory_title",
        description_key: "missions.daily_ranked_victory_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 1,
        requirement: MissionRequirement::WinRankedMatches { count: 1 },
        icon: "crown",
        order: 16,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "ranked_win",
        action_target: MissionActionTarget::Ranked,
    },
    // Lane 1: Hero Constraints (Region / Rarity / Free Rotation / Combo)
    MissionDefinition {
        id: "daily_teams_epic_win",
        category: MissionCategory::Daily,
        title_key: "missions.daily_teams_epic_win_title",
        description_key: "missions.daily_teams_epic_win_desc",
        crowns_reward: 45,
        gems_reward: 1,
        target: 1,
        requirement: MissionRequirement::WinWithConstraints {
            mode: Some(MatchMode::Teams),
            region: None,
            rarity: Some(Rarity::Epic),
            count: 1,
        },
        icon: "crown",
        order: 17,
        lane: 1,
        axis: PreferenceAxis::Rarity,
        tag: "teams_epic",
        action_target: MissionActionTarget::HeroesEpic,
    },
    MissionDefinition {
        id: "daily_oceania_3",
        category: MissionCategory::Daily,
        title_key: "missions.daily_oceania_3_title",
        description_key: "missions.daily_oceania_3_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 3,
        requirement: MissionRequirement::PlayWithHeroConstraints {
            region: Some(Region::Oceania),
            rarity: None,
            count: 3,
        },
        icon: "rotation",
        order: 18,
        lane: 1,
        axis: PreferenceAxis::Region,
        tag: "oceania",
        action_target: MissionActionTarget::HeroesOceania,
    },
    MissionDefinition {
        id: "daily_europe_3",
        category: MissionCategory::Daily,
        title_key: "missions.daily_europe_3_title",
        description_key: "missions.daily_europe_3_desc",
        crowns_reward: 30,
        gems_reward: 0,
        target: 3,
        requirement: MissionRequirement::PlayWithHeroConstraints {
            region: Some(Region::Europe),
            rarity: None,
            count: 3,
        },
        icon: "shield",
        order: 19,
        lane: 1,
        axis: PreferenceAxis::Region,
        tag: "europe",
        action_target: MissionActionTarget::HeroesEurope,
    },
    MissionDefinition {
        id: "daily_asia_3",
        category: MissionCategory::Daily,
        title_key: "missions.daily_asia_3_title",
        description_key: "missions.daily_asia_3_desc",
        crowns_reward: 30,
        gems_reward: 0,
        target: 3,
        requirement: MissionRequirement::PlayWithHeroConstraints {
            region: Some(Region::Asia),
            rarity: None,
            count: 3,
        },
        icon: "swords",
        order: 20,
        lane: 1,
        axis: PreferenceAxis::Region,
        tag: "asia",
        action_target: MissionActionTarget::HeroesAsia,
    },
    MissionDefinition {
        id: "daily_americas_3",
        category: MissionCategory::Daily,
        title_key: "missions.daily_americas_3_title",
        description_key: "missions.daily_americas_3_desc",
        crowns_reward: 30,
        gems_reward: 0,
        target: 3,
        requirement: MissionRequirement::PlayWithHeroConstraints {
            region: Some(Region::Americas),
            rarity: None,
            count: 3,
        },
        icon: "swords",
        order: 21,
        lane: 1,
        axis: PreferenceAxis::Region,
        tag: "americas",
        action_target: MissionActionTarget::HeroesAmericas,
    },
    MissionDefinition {
        id: "daily_africa_3",
        category: MissionCategory::Daily,
        title_key: "missions.daily_africa_3_title",
        description_key: "missions.daily_africa_3_desc",
        crowns_reward: 30,
        gems_reward: 0,
        target: 3,
        requirement: MissionRequirement::PlayWithHeroConstraints {
            region: Some(Region::Africa),
            rarity: None,
            count: 3,
        },
        icon: "shield",
        order: 22,
        lane: 1,
        axis: PreferenceAxis::Region,
        tag: "africa",
        action_target: MissionActionTarget::HeroesAfrica,
    },
    MissionDefinition {
        id: "daily_common_win",
        category: MissionCategory::Daily,
        title_key: "missions.daily_common_win_title",
        description_key: "missions.daily_common_win_desc",
        crowns_reward: 30,
        gems_reward: 0,
        target: 1,
        requirement: MissionRequirement::WinWithConstraints {
            mode: None,
            region: None,
            rarity: Some(Rarity::Common),
            count: 1,
        },
        icon: "crown",
        order: 23,
        lane: 1,
        axis: PreferenceAxis::Rarity,
        tag: "common",
        action_target: MissionActionTarget::HeroesCommon,
    },
    MissionDefinition {
        id: "daily_rare_play",
        category: MissionCategory::Daily,
        title_key: "missions.daily_rare_play_title",
        description_key: "missions.daily_rare_play_desc",
        crowns_reward: 25,
        gems_reward: 0,
        target: 2,
        requirement: MissionRequirement::PlayWithHeroConstraints {
            region: None,
            rarity: Some(Rarity::Rare),
            count: 2,
        },
        icon: "shield",
        order: 24,
        lane: 1,
        axis: PreferenceAxis::Rarity,
        tag: "rare",
        action_target: MissionActionTarget::HeroesRare,
    },
    MissionDefinition {
        id: "daily_free_rotation",
        category: MissionCategory::Daily,
        title_key: "missions.daily_free_rotation_title",
        description_key: "missions.daily_free_rotation_desc",
        crowns_reward: 30,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::PlayFreeRotationHero { count: 2 },
        icon: "rotation",
        order: 25,
        lane: 1,
        axis: PreferenceAxis::Discovery,
        tag: "rotation",
        action_target: MissionActionTarget::HeroesFreeRotation,
    },
    // Lane 2: Macro, Combat & Hero Diversity
    MissionDefinition {
        id: "daily_build_structures",
        category: MissionCategory::Daily,
        title_key: "missions.daily_build_structures_title",
        description_key: "missions.daily_build_structures_desc",
        crowns_reward: 25,
        gems_reward: 0,
        target: 6,
        requirement: MissionRequirement::BuildStructures { count: 6 },
        icon: "shield",
        order: 26,
        lane: 2,
        axis: PreferenceAxis::Macro,
        tag: "structures",
        action_target: MissionActionTarget::Battle,
    },
    MissionDefinition {
        id: "daily_defeat_empires",
        category: MissionCategory::Daily,
        title_key: "missions.daily_defeat_empires_title",
        description_key: "missions.daily_defeat_empires_desc",
        crowns_reward: 30,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::DefeatEmpires { count: 2 },
        icon: "swords",
        order: 27,
        lane: 2,
        axis: PreferenceAxis::Combat,
        tag: "combat",
        action_target: MissionActionTarget::Battle,
    },
    MissionDefinition {
        id: "daily_variety_2",
        category: MissionCategory::Daily,
        title_key: "missions.daily_variety_2_title",
        description_key: "missions.daily_variety_2_desc",
        crowns_reward: 30,
        gems_reward: 0,
        target: 2,
        requirement: MissionRequirement::DistinctHeroes { count: 2 },
        icon: "rotation",
        order: 28,
        lane: 2,
        axis: PreferenceAxis::Discovery,
        tag: "distinct_heroes",
        action_target: MissionActionTarget::HeroesAll,
    },
];

pub const WEEKLY_MISSION_POOL: &[MissionDefinition] = &[
    // Lane 0: Core Mode Mastery & Territory Dominance
    MissionDefinition {
        id: "weekly_ranked_champion",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_ranked_champion_title",
        description_key: "missions.weekly_ranked_champion_desc",
        crowns_reward: 120,
        gems_reward: 3,
        target: 3,
        requirement: MissionRequirement::WinRankedMatches { count: 3 },
        icon: "crown",
        order: 40,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "ranked",
        action_target: MissionActionTarget::Ranked,
    },
    MissionDefinition {
        id: "weekly_team_striker",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_team_striker_title",
        description_key: "missions.weekly_team_striker_desc",
        crowns_reward: 105,
        gems_reward: 2,
        target: 4,
        requirement: MissionRequirement::WinMode {
            mode: MatchMode::Teams,
            count: 4,
        },
        icon: "team",
        order: 41,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "teams",
        action_target: MissionActionTarget::Teams,
    },
    MissionDefinition {
        id: "weekly_hnv_defender",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_hnv_defender_title",
        description_key: "missions.weekly_hnv_defender_desc",
        crowns_reward: 110,
        gems_reward: 2,
        target: 2,
        requirement: MissionRequirement::WinMode {
            mode: MatchMode::HumansVsNations,
            count: 2,
        },
        icon: "shield",
        order: 42,
        lane: 0,
        axis: PreferenceAxis::Mode,
        tag: "hnv",
        action_target: MissionActionTarget::Hnv,
    },
    MissionDefinition {
        id: "weekly_peak_first_2",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_peak_first_2_title",
        description_key: "missions.weekly_peak_first_2_desc",
        crowns_reward: 115,
        gems_reward: 3,
        target: 2,
        requirement: MissionRequirement::PeakTerritoryRank {
            mode: None,
            max_rank: 1,
            min_contenders: 4,
            count: 2,
        },
        icon: "crown",
        order: 43,
        lane: 0,
        axis: PreferenceAxis::Skill,
        tag: "territory_first",
        action_target: MissionActionTarget::Battle,
    },
    // Lane 1: Roster Exploration & Rarity
    MissionDefinition {
        id: "weekly_variety_heroes",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_variety_heroes_title",
        description_key: "missions.weekly_variety_heroes_desc",
        crowns_reward: 95,
        gems_reward: 2,
        target: 3,
        requirement: MissionRequirement::DistinctHeroes { count: 3 },
        icon: "rotation",
        order: 44,
        lane: 1,
        axis: PreferenceAxis::Discovery,
        tag: "distinct_heroes",
        action_target: MissionActionTarget::HeroesAll,
    },
    MissionDefinition {
        id: "weekly_legendary_3",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_legendary_3_title",
        description_key: "missions.weekly_legendary_3_desc",
        crowns_reward: 100,
        gems_reward: 2,
        target: 3,
        requirement: MissionRequirement::PlayWithHeroConstraints {
            region: None,
            rarity: Some(Rarity::Legendary),
            count: 3,
        },
        icon: "crown",
        order: 45,
        lane: 1,
        axis: PreferenceAxis::Rarity,
        tag: "legendary",
        action_target: MissionActionTarget::HeroesLegendary,
    },
    // Lane 2: Endurance & Conquest
    MissionDefinition {
        id: "weekly_ffa_endurance",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_ffa_endurance_title",
        description_key: "missions.weekly_ffa_endurance_desc",
        crowns_reward: 95,
        gems_reward: 2,
        target: 4,
        requirement: MissionRequirement::SurviveInMode {
            mode: MatchMode::Ffa,
            min_seconds: 180,
            count: 4,
        },
        icon: "shield",
        order: 46,
        lane: 2,
        axis: PreferenceAxis::Mode,
        tag: "ffa",
        action_target: MissionActionTarget::Ffa,
    },
    MissionDefinition {
        id: "weekly_conqueror",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_conqueror_title",
        description_key: "missions.weekly_conqueror_desc",
        crowns_reward: 100,
        gems_reward: 2,
        target: 5,
        requirement: MissionRequirement::DefeatEmpires { count: 5 },
        icon: "swords",
        order: 47,
        lane: 2,
        axis: PreferenceAxis::Combat,
        tag: "combat",
        action_target: MissionActionTarget::Battle,
    },
];

pub fn mission_by_id(id: &str) -> Option<&'static MissionDefinition> {
    ONBOARDING_MISSIONS
        .iter()
        .chain(DAILY_MISSION_POOL.iter())
        .chain(WEEKLY_MISSION_POOL.iter())
        .find(|def| def.id == id)
}

#[derive(Clone, Copy, Debug)]
pub struct PlayabilityContext<'a> {
    pub owned_leaders: &'a BTreeSet<String>,
    pub free_rotation: &'a [crate::leaders::Leader],
    pub crowns: u64,
    pub gems: u64,
}

fn constraint_matches_leader(
    region: Option<Region>,
    rarity: Option<Rarity>,
    leader: crate::leaders::Leader,
) -> bool {
    region.is_none_or(|r| leader.character().region == r)
        && rarity.is_none_or(|r| leader.rarity() == r)
}

fn leader_is_affordable(leader: crate::leaders::Leader, ctx: &PlayabilityContext<'_>) -> bool {
    let price_c = leader.price_crowns();
    let price_g = leader.price_gems();
    (price_c > 0 && ctx.crowns >= price_c) || (price_g > 0 && ctx.gems >= price_g)
}

/// True when the player can immediately play this mission with an already-unlocked,
/// permanently free, or currently free-rotation commander.
pub fn is_mission_currently_playable(
    def: &MissionDefinition,
    playability: Option<&PlayabilityContext<'_>>,
) -> bool {
    let Some(ctx) = playability else {
        return true;
    };
    let (region, rarity) = match def.requirement {
        MissionRequirement::PlayWithHeroConstraints { region, rarity, .. }
        | MissionRequirement::WinWithConstraints { region, rarity, .. } => (region, rarity),
        MissionRequirement::PlayFreeRotationHero { .. } => return !ctx.free_rotation.is_empty(),
        _ => return true,
    };
    crate::leaders::Leader::all().any(|leader| {
        constraint_matches_leader(region, rarity, leader)
            && crate::commerce::leader_available(leader, ctx.owned_leaders, ctx.free_rotation)
    })
}

/// True when the mission is immediately playable OR the player holds enough Crowns/Gems
/// to unlock at least one matching commander.
pub fn is_mission_playable_or_affordable(
    def: &MissionDefinition,
    playability: Option<&PlayabilityContext<'_>>,
) -> bool {
    if is_mission_currently_playable(def, playability) {
        return true;
    }
    let Some(ctx) = playability else {
        return true;
    };
    let (region, rarity) = match def.requirement {
        MissionRequirement::PlayWithHeroConstraints { region, rarity, .. }
        | MissionRequirement::WinWithConstraints { region, rarity, .. } => (region, rarity),
        _ => return true,
    };
    crate::leaders::Leader::all().any(|leader| {
        constraint_matches_leader(region, rarity, leader) && leader_is_affordable(leader, ctx)
    })
}

fn hash_pick(seed: u64, step: usize, count: usize) -> usize {
    if count <= 1 {
        return 0;
    }
    let mixed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
        .wrapping_add((step as u64).wrapping_mul(0x9e3779b97f4a7c15));
    ((mixed >> 16) as usize) % count
}

/// Deterministic per-account salt for daily/weekly mission hand dealing.
pub fn account_deal_salt(account_id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in account_id.as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn pick_lane_candidate(
    pool: &'static [MissionDefinition],
    lane: u8,
    seed: u64,
    used_ids: &BTreeSet<&'static str>,
    used_tags: &BTreeSet<&'static str>,
    playability: Option<&PlayabilityContext<'_>>,
) -> Option<&'static MissionDefinition> {
    let pick_from = |require_lane: bool, strict_playable: bool, allow_affordable: bool| -> Option<&'static MissionDefinition> {
        let candidates: Vec<&'static MissionDefinition> = pool
            .iter()
            .filter(|def| {
                (!require_lane || def.lane == lane)
                    && !used_ids.contains(def.id)
                    && !used_tags.contains(def.tag)
                    && if strict_playable {
                        is_mission_currently_playable(def, playability)
                    } else if allow_affordable {
                        is_mission_playable_or_affordable(def, playability)
                    } else {
                        true
                    }
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        Some(candidates[hash_pick(seed, lane as usize, candidates.len())])
    };

    pick_from(true, true, false)
        .or_else(|| pick_from(true, false, true))
        .or_else(|| pick_from(true, false, false))
        .or_else(|| pick_from(false, true, false))
        .or_else(|| pick_from(false, false, false))
}

fn deal_hand(
    pool: &'static [MissionDefinition],
    seed: u64,
    playability: Option<&PlayabilityContext<'_>>,
) -> Vec<MissionDefinition> {
    if pool.len() <= 3 {
        return pool.to_vec();
    }
    let mut chosen = Vec::with_capacity(3);
    let mut used_ids = BTreeSet::new();
    let mut used_tags = BTreeSet::new();
    for lane in 0u8..3u8 {
        if let Some(def) =
            pick_lane_candidate(pool, lane, seed, &used_ids, &used_tags, playability)
        {
            used_ids.insert(def.id);
            used_tags.insert(def.tag);
            chosen.push(*def);
        }
    }
    chosen
}

/// Deterministically selects 3 lane-balanced daily missions for a given UTC day.
pub fn daily_missions_for_day(epoch_day: u64) -> Vec<MissionDefinition> {
    deal_hand(DAILY_MISSION_POOL, epoch_day, None)
}

/// Deterministically selects 3 lane-balanced weekly missions for a weekly rotation period.
pub fn weekly_missions_for_period(rotation_period: u64) -> Vec<MissionDefinition> {
    deal_hand(
        WEEKLY_MISSION_POOL,
        rotation_period.wrapping_add(0x55aa_55aa),
        None,
    )
}

pub fn deal_daily_hand_for_player(
    epoch_day: u64,
    salt: u64,
    playability: Option<&PlayabilityContext<'_>>,
) -> Vec<MissionDefinition> {
    deal_hand(DAILY_MISSION_POOL, epoch_day ^ salt, playability)
}

pub fn deal_weekly_hand_for_player(
    rotation_period: u64,
    salt: u64,
    playability: Option<&PlayabilityContext<'_>>,
) -> Vec<MissionDefinition> {
    deal_hand(
        WEEKLY_MISSION_POOL,
        rotation_period.wrapping_add(0x55aa_55aa) ^ salt,
        playability,
    )
}

fn resolve_pool_hand(
    pool: &'static [MissionDefinition],
    saved_ids: &mut Vec<String>,
    excluded_ids: &BTreeSet<String>,
    seed: u64,
    playability: Option<&PlayabilityContext<'_>>,
) -> Vec<&'static MissionDefinition> {
    saved_ids.retain(|id| pool.iter().any(|def| def.id == id.as_str()));
    let mut deduped = Vec::with_capacity(3);
    let mut used_ids: BTreeSet<&'static str> = excluded_ids
        .iter()
        .filter_map(|id| pool.iter().find(|def| def.id == id.as_str()).map(|d| d.id))
        .collect();
    let mut used_tags = BTreeSet::new();
    for id in saved_ids.iter() {
        if let Some(def) = pool.iter().find(|d| d.id == id.as_str()) {
            if used_ids.insert(def.id) {
                used_tags.insert(def.tag);
                deduped.push(def.id.to_string());
            }
        }
        if deduped.len() == 3 {
            break;
        }
    }
    let mut newly_added = Vec::new();
    for lane in 0u8..3u8 {
        if deduped.len() >= 3 {
            break;
        }
        if let Some(def) =
            pick_lane_candidate(pool, lane, seed, &used_ids, &used_tags, playability)
        {
            used_ids.insert(def.id);
            used_tags.insert(def.tag);
            deduped.push(def.id.to_string());
            newly_added.push(def);
        }
    }
    *saved_ids = deduped;
    newly_added
}

/// Statistics provided by player profile or session telemetry to evaluate missions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissionStats {
    pub player_level: u32,
    pub tutorial_completed: bool,
    pub total_structures_built: u32,
    pub free_rotation_matches: u32,
    pub team_matches: u32,
    pub team_wins: u32,
    pub west_regional_matches: u32,
    pub east_regional_matches: u32,
    pub matches_played: u32,
    pub kills: u32,
    pub ranked_matches: u32,
    pub ranked_wins: u32,
    pub empires_defeated: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchMissionContext {
    pub player_level: u32,
    pub mode: MatchMode,
    pub is_ranked: bool,
    pub used_free_rotation: bool,
    pub leader_id: Option<&'static str>,
    pub leader_region: Option<Region>,
    pub leader_rarity: Option<Rarity>,
    pub won: bool,
    pub kills: u32,
    pub empires_defeated: u32,
    pub alive_seconds: u32,
    pub peak_territory_rank: Option<u16>,
    pub total_contenders: u16,
    pub structures_built: u32,
}

pub fn category_str(category: MissionCategory) -> &'static str {
    match category {
        MissionCategory::Onboarding => "onboarding",
        MissionCategory::Daily => "daily",
        MissionCategory::Weekly => "weekly",
    }
}

fn locked_mission_view(def: &MissionDefinition) -> MissionView {
    MissionView {
        id: def.id.to_string(),
        category: category_str(def.category).to_string(),
        title_key: def.title_key.to_string(),
        description_key: def.description_key.to_string(),
        target: def.target,
        progress: 0,
        completed: false,
        claimed: false,
        crowns_reward: def.crowns_reward,
        gems_reward: def.gems_reward,
        icon: def.icon.to_string(),
        order: def.order,
        locked: true,
        unlock_level: min_level_for_category(def.category),
        axis: def.axis.as_str().to_string(),
        tag: def.tag.to_string(),
        action_target: def.action_target,
        swappable: false,
    }
}

pub fn evaluate_mission_progress(
    def: &MissionDefinition,
    stats: &MissionStats,
    saved_progress: Option<&MissionRecord>,
) -> MissionView {
    let unlock_level = min_level_for_category(def.category);
    let effective_level = stats.player_level.max(1);
    if effective_level < unlock_level {
        return locked_mission_view(def);
    }

    let saved_val = saved_progress.map(|r| r.progress).unwrap_or(0);
    let raw_progress = match def.requirement {
        MissionRequirement::CompleteTutorial => {
            if stats.tutorial_completed
                || saved_progress
                    .map(|r| r.completed || r.progress >= 1)
                    .unwrap_or(false)
            {
                1
            } else {
                0
            }
        }
        MissionRequirement::DistinctHeroes { .. } => saved_progress
            .map(|r| r.progress.max(r.seen_keys.len() as u32))
            .unwrap_or(0),
        _ => saved_val,
    };

    let target = def.target;
    let progress = raw_progress.min(target);
    let completed = progress >= target || saved_progress.map(|r| r.completed).unwrap_or(false);
    let claimed = saved_progress.map(|r| r.claimed).unwrap_or(false);
    let swappable = def.category != MissionCategory::Onboarding && !completed && !claimed;

    MissionView {
        id: def.id.to_string(),
        category: category_str(def.category).to_string(),
        title_key: def.title_key.to_string(),
        description_key: def.description_key.to_string(),
        target,
        progress,
        completed,
        claimed,
        crowns_reward: def.crowns_reward,
        gems_reward: def.gems_reward,
        icon: def.icon.to_string(),
        order: def.order,
        locked: false,
        unlock_level,
        axis: def.axis.as_str().to_string(),
        tag: def.tag.to_string(),
        action_target: def.action_target,
        swappable,
    }
}

/// Clears stale daily/weekly mission records when the active UTC day or weekly period changes.
pub fn sync_period_missions(
    records: &mut BTreeMap<String, MissionRecord>,
    daily_day: &mut u64,
    weekly_period: &mut u64,
    epoch_day: u64,
    rotation_period: u64,
) {
    if *daily_day != epoch_day {
        records.retain(|id, _| !id.starts_with("daily_"));
        *daily_day = epoch_day;
    }
    if *weekly_period != rotation_period {
        records.retain(|id, _| !id.starts_with("weekly_"));
        *weekly_period = rotation_period;
    }
}

pub struct HandSyncState<'a> {
    pub records: &'a mut BTreeMap<String, MissionRecord>,
    pub daily_day: &'a mut u64,
    pub daily_mission_ids: &'a mut Vec<String>,
    pub daily_swaps_used: &'a mut u8,
    pub daily_swapped_out_ids: &'a mut BTreeSet<String>,
    pub weekly_period: &'a mut u64,
    pub weekly_mission_ids: &'a mut Vec<String>,
    pub weekly_swaps_used: &'a mut u8,
    pub weekly_swapped_out_ids: &'a mut BTreeSet<String>,
}

/// Synchronizes period rollovers and fills unlocked Daily/Weekly active hands with playable missions.
/// Returns definitions of newly dealt missions (for unlocked tiers only) so analytics can record offers.
pub fn sync_hands_for_periods(
    state: HandSyncState<'_>,
    epoch_day: u64,
    rotation_period: u64,
    player_level: u32,
    deal_salt: u64,
    playability: Option<&PlayabilityContext<'_>>,
) -> Vec<&'static MissionDefinition> {
    let effective_level = player_level.max(1);
    let target_daily_day = if effective_level >= DAILY_UNLOCK_LEVEL {
        epoch_day
    } else {
        *state.daily_day
    };
    let target_weekly_period = if effective_level >= WEEKLY_UNLOCK_LEVEL {
        rotation_period
    } else {
        *state.weekly_period
    };
    let day_changed = effective_level >= DAILY_UNLOCK_LEVEL && *state.daily_day != epoch_day;
    let week_changed =
        effective_level >= WEEKLY_UNLOCK_LEVEL && *state.weekly_period != rotation_period;

    sync_period_missions(
        state.records,
        state.daily_day,
        state.weekly_period,
        target_daily_day,
        target_weekly_period,
    );

    if day_changed {
        state.daily_mission_ids.clear();
        *state.daily_swaps_used = 0;
        state.daily_swapped_out_ids.clear();
    }
    if week_changed {
        state.weekly_mission_ids.clear();
        *state.weekly_swaps_used = 0;
        state.weekly_swapped_out_ids.clear();
    }

    let mut newly_dealt = Vec::new();
    if effective_level >= DAILY_UNLOCK_LEVEL {
        newly_dealt.extend(resolve_pool_hand(
            DAILY_MISSION_POOL,
            state.daily_mission_ids,
            state.daily_swapped_out_ids,
            epoch_day ^ deal_salt,
            playability,
        ));
    }
    if effective_level >= WEEKLY_UNLOCK_LEVEL {
        newly_dealt.extend(resolve_pool_hand(
            WEEKLY_MISSION_POOL,
            state.weekly_mission_ids,
            state.weekly_swapped_out_ids,
            rotation_period.wrapping_add(0x55aa_55aa) ^ deal_salt,
            playability,
        ));
    }
    newly_dealt
}

/// Selects a playable replacement candidate from `DAILY_MISSION_POOL` or `WEEKLY_MISSION_POOL`
/// when swapping an active mission out of the player's current hand.
pub fn select_swap_replacement(
    old_def: &MissionDefinition,
    active_ids: &[String],
    swapped_out_ids: &BTreeSet<String>,
    seed: u64,
    playability: Option<&PlayabilityContext<'_>>,
) -> Result<&'static MissionDefinition, &'static str> {
    let pool = match old_def.category {
        MissionCategory::Daily => DAILY_MISSION_POOL,
        MissionCategory::Weekly => WEEKLY_MISSION_POOL,
        MissionCategory::Onboarding => return Err("onboarding missions cannot be swapped"),
    };
    let excluded: BTreeSet<&str> = active_ids
        .iter()
        .map(String::as_str)
        .chain(swapped_out_ids.iter().map(String::as_str))
        .chain(std::iter::once(old_def.id))
        .collect();
    let active_tags: BTreeSet<&str> = active_ids
        .iter()
        .filter(|id| id.as_str() != old_def.id)
        .filter_map(|id| pool.iter().find(|def| def.id == id.as_str()).map(|d| d.tag))
        .collect();

    let diff_axis_tags: Vec<&'static MissionDefinition> = pool
        .iter()
        .filter(|def| {
            !excluded.contains(def.id)
                && !active_tags.contains(def.tag)
                && def.axis != old_def.axis
                && def.tag != old_def.tag
                && is_mission_currently_playable(def, playability)
        })
        .collect();
    if !diff_axis_tags.is_empty() {
        return Ok(diff_axis_tags[hash_pick(seed, 0, diff_axis_tags.len())]);
    }

    let diff_tag_playable: Vec<&'static MissionDefinition> = pool
        .iter()
        .filter(|def| {
            !excluded.contains(def.id)
                && !active_tags.contains(def.tag)
                && def.tag != old_def.tag
                && is_mission_currently_playable(def, playability)
        })
        .collect();
    if !diff_tag_playable.is_empty() {
        return Ok(diff_tag_playable[hash_pick(seed, 1, diff_tag_playable.len())]);
    }

    let any_playable: Vec<&'static MissionDefinition> = pool
        .iter()
        .filter(|def| !excluded.contains(def.id) && is_mission_currently_playable(def, playability))
        .collect();
    if !any_playable.is_empty() {
        return Ok(any_playable[hash_pick(seed, 2, any_playable.len())]);
    }

    Err("no swap candidate available")
}

fn match_delta_for_requirement(req: MissionRequirement, ctx: &MatchMissionContext) -> u32 {
    match req {
        MissionRequirement::CompleteTutorial => 0,
        MissionRequirement::BuildStructures { .. } => ctx.structures_built,
        MissionRequirement::PlayFreeRotationHero { .. } => u32::from(ctx.used_free_rotation),
        MissionRequirement::PlayMode { mode, .. } => u32::from(ctx.mode == mode),
        MissionRequirement::WinMode { mode, .. } => u32::from(ctx.won && ctx.mode == mode),
        MissionRequirement::SurviveInMode {
            mode, min_seconds, ..
        } => u32::from(ctx.mode == mode && ctx.alive_seconds >= min_seconds),
        MissionRequirement::PeakTerritoryRank {
            mode,
            max_rank,
            min_contenders,
            ..
        } => u32::from(
            mode.is_none_or(|m| m == ctx.mode)
                && ctx.total_contenders >= min_contenders
                && ctx.peak_territory_rank.is_some_and(|rank| rank <= max_rank),
        ),
        MissionRequirement::PlayWithHeroConstraints { region, rarity, .. } => u32::from(
            region.is_none_or(|r| ctx.leader_region == Some(r))
                && rarity.is_none_or(|r| ctx.leader_rarity == Some(r)),
        ),
        MissionRequirement::WinWithConstraints {
            mode,
            region,
            rarity,
            ..
        } => u32::from(
            ctx.won
                && mode.is_none_or(|m| ctx.mode == m)
                && region.is_none_or(|r| ctx.leader_region == Some(r))
                && rarity.is_none_or(|r| ctx.leader_rarity == Some(r)),
        ),
        MissionRequirement::DistinctHeroes { .. } => 0, // handled via seen_keys
        MissionRequirement::PlayMatches { .. } => 1,
        MissionRequirement::WinMatches { .. } => u32::from(ctx.won),
        MissionRequirement::EliminateUnits { .. } => ctx.kills,
        MissionRequirement::PlayRankedMatches { .. } => u32::from(ctx.is_ranked),
        MissionRequirement::WinRankedMatches { .. } => u32::from(ctx.is_ranked && ctx.won),
        MissionRequirement::DefeatEmpires { .. } => ctx.empires_defeated,
    }
}

fn apply_match_delta(
    records: &mut BTreeMap<String, MissionRecord>,
    def: &MissionDefinition,
    ctx: &MatchMissionContext,
) -> bool {
    if matches!(def.requirement, MissionRequirement::DistinctHeroes { .. }) {
        let Some(leader_id) = ctx.leader_id.filter(|id| !id.is_empty()) else {
            return false;
        };
        let entry = records
            .entry(def.id.to_string())
            .or_insert_with(|| MissionRecord {
                id: def.id.to_string(),
                ..Default::default()
            });
        if entry.claimed || entry.completed {
            return false;
        }
        if !entry.seen_keys.insert(leader_id.to_string()) {
            return false;
        }
        entry.progress = (entry.seen_keys.len() as u32).min(def.target);
        if entry.progress >= def.target {
            entry.completed = true;
            return true;
        }
        return false;
    }

    let delta = match_delta_for_requirement(def.requirement, ctx);
    if delta == 0 {
        return false;
    }
    let entry = records
        .entry(def.id.to_string())
        .or_insert_with(|| MissionRecord {
            id: def.id.to_string(),
            ..Default::default()
        });
    if entry.claimed || entry.completed {
        return false;
    }
    entry.progress = entry.progress.saturating_add(delta).min(def.target);
    if entry.progress >= def.target {
        entry.completed = true;
        return true;
    }
    false
}

/// Active hand definitions helper given persisted IDs (falls back to deterministic period deal).
pub fn active_defs_for_hand(pool: &[MissionDefinition], active_ids: &[String], fallback: Vec<MissionDefinition>) -> Vec<MissionDefinition> {
    if active_ids.is_empty() {
        return fallback;
    }
    let mut resolved: Vec<MissionDefinition> = active_ids
        .iter()
        .filter_map(|id| pool.iter().find(|def| def.id == id.as_str()).copied())
        .collect();
    for def in fallback {
        if resolved.len() >= 3 {
            break;
        }
        if !resolved.iter().any(|existing| existing.id == def.id) {
            resolved.push(def);
        }
    }
    resolved
}

/// Advances unlocked, active missions after a verified match finishes (deterministic default-hand wrapper).
pub fn advance_missions_after_match(
    records: &mut BTreeMap<String, MissionRecord>,
    daily_day: &mut u64,
    weekly_period: &mut u64,
    epoch_day: u64,
    rotation_period: u64,
    ctx: &MatchMissionContext,
) {
    let mut daily_ids = Vec::new();
    let mut daily_swaps = 0;
    let mut daily_out = BTreeSet::new();
    let mut weekly_ids = Vec::new();
    let mut weekly_swaps = 0;
    let mut weekly_out = BTreeSet::new();
    advance_missions_after_match_with_hands(
        HandSyncState {
            records,
            daily_day,
            daily_mission_ids: &mut daily_ids,
            daily_swaps_used: &mut daily_swaps,
            daily_swapped_out_ids: &mut daily_out,
            weekly_period,
            weekly_mission_ids: &mut weekly_ids,
            weekly_swaps_used: &mut weekly_swaps,
            weekly_swapped_out_ids: &mut weekly_out,
        },
        epoch_day,
        rotation_period,
        0,
        None,
        ctx,
    );
}

pub struct MatchAdvanceOutcome {
    pub newly_dealt: Vec<&'static MissionDefinition>,
    pub newly_completed: Vec<&'static MissionDefinition>,
}

/// Advances unlocked, persisted-hand missions after a verified match finishes.
pub fn advance_missions_after_match_with_hands(
    state: HandSyncState<'_>,
    epoch_day: u64,
    rotation_period: u64,
    deal_salt: u64,
    playability: Option<&PlayabilityContext<'_>>,
    ctx: &MatchMissionContext,
) -> MatchAdvanceOutcome {
    let newly_dealt = sync_hands_for_periods(
        HandSyncState {
            records: state.records,
            daily_day: state.daily_day,
            daily_mission_ids: state.daily_mission_ids,
            daily_swaps_used: state.daily_swaps_used,
            daily_swapped_out_ids: state.daily_swapped_out_ids,
            weekly_period: state.weekly_period,
            weekly_mission_ids: state.weekly_mission_ids,
            weekly_swaps_used: state.weekly_swaps_used,
            weekly_swapped_out_ids: state.weekly_swapped_out_ids,
        },
        epoch_day,
        rotation_period,
        ctx.player_level,
        deal_salt,
        playability,
    );

    let mut newly_completed = Vec::new();

    // Only the first unclaimed onboarding step can advance.
    if let Some(active_onboarding) = ONBOARDING_MISSIONS
        .iter()
        .find(|def| !state.records.get(def.id).map(|r| r.claimed).unwrap_or(false))
    {
        if apply_match_delta(state.records, active_onboarding, ctx) {
            if let Some(def_ref) = mission_by_id(active_onboarding.id) {
                newly_completed.push(def_ref);
            }
        }
    }

    let effective_level = ctx.player_level.max(1);
    if effective_level >= DAILY_UNLOCK_LEVEL {
        let daily_defs = active_defs_for_hand(
            DAILY_MISSION_POOL,
            state.daily_mission_ids,
            deal_daily_hand_for_player(epoch_day, deal_salt, playability),
        );
        for def in &daily_defs {
            if apply_match_delta(state.records, def, ctx) {
                if let Some(def_ref) = mission_by_id(def.id) {
                    newly_completed.push(def_ref);
                }
            }
        }
    }

    if effective_level >= WEEKLY_UNLOCK_LEVEL {
        let weekly_defs = active_defs_for_hand(
            WEEKLY_MISSION_POOL,
            state.weekly_mission_ids,
            deal_weekly_hand_for_player(rotation_period, deal_salt, playability),
        );
        for def in &weekly_defs {
            if apply_match_delta(state.records, def, ctx) {
                if let Some(def_ref) = mission_by_id(def.id) {
                    newly_completed.push(def_ref);
                }
            }
        }
    }

    MatchAdvanceOutcome {
        newly_dealt,
        newly_completed,
    }
}

/// Builds the complete view of missions using persisted active hands when present.
pub fn build_all_missions_view_with_hands(
    stats: &MissionStats,
    saved_records: &BTreeMap<String, MissionRecord>,
    epoch_day: u64,
    daily_mission_ids: &[String],
    weekly_mission_ids: &[String],
    deal_salt: u64,
    playability: Option<&PlayabilityContext<'_>>,
) -> Vec<MissionView> {
    let mut views = Vec::new();
    let mut prev_claimed = true;
    for def in ONBOARDING_MISSIONS {
        let saved = saved_records.get(def.id);
        let already_claimed = saved.map(|r| r.claimed).unwrap_or(false);
        let view = if !prev_claimed && !already_claimed {
            locked_mission_view(def)
        } else {
            evaluate_mission_progress(def, stats, saved)
        };
        prev_claimed = view.claimed;
        views.push(view);
    }

    let effective_level = stats.player_level.max(1);
    let daily_defs = active_defs_for_hand(
        DAILY_MISSION_POOL,
        daily_mission_ids,
        deal_daily_hand_for_player(epoch_day, deal_salt, playability),
    );
    for def in &daily_defs {
        if effective_level < DAILY_UNLOCK_LEVEL {
            views.push(locked_mission_view(def));
        } else {
            let saved = saved_records.get(def.id);
            views.push(evaluate_mission_progress(def, stats, saved));
        }
    }

    let period = crate::commerce::rotation_period(epoch_day.saturating_mul(86_400));
    let weekly_defs = active_defs_for_hand(
        WEEKLY_MISSION_POOL,
        weekly_mission_ids,
        deal_weekly_hand_for_player(period, deal_salt, playability),
    );
    for def in &weekly_defs {
        if effective_level < WEEKLY_UNLOCK_LEVEL {
            views.push(locked_mission_view(def));
        } else {
            let saved = saved_records.get(def.id);
            views.push(evaluate_mission_progress(def, stats, saved));
        }
    }
    views
}

/// Builds the complete view of missions using deterministic default hands for backward-compatible callers.
pub fn build_all_missions_view(
    stats: &MissionStats,
    saved_records: &BTreeMap<String, MissionRecord>,
    epoch_day: u64,
) -> Vec<MissionView> {
    build_all_missions_view_with_hands(stats, saved_records, epoch_day, &[], &[], 0, None)
}

/// Identifies the single active directive that should be featured on the command console.
/// Priority (skipping locked missions):
/// 1. First completed-but-unclaimed onboarding mission (reward ready!).
/// 2. First unlocked in-progress onboarding mission.
/// 3. First unlocked completed-but-unclaimed daily mission.
/// 4. First unlocked completed-but-unclaimed weekly mission.
/// 5. First unlocked in-progress daily mission.
/// 6. First unlocked in-progress weekly mission.
pub fn active_command_directive(missions: &[MissionView]) -> Option<MissionView> {
    // 1. Unclaimed onboarding
    if let Some(m) = missions
        .iter()
        .find(|m| !m.locked && m.category == "onboarding" && m.completed && !m.claimed)
    {
        return Some(m.clone());
    }
    // 2. Incomplete onboarding
    if let Some(m) = missions
        .iter()
        .find(|m| !m.locked && m.category == "onboarding" && !m.completed)
    {
        return Some(m.clone());
    }
    // 3. Unclaimed daily
    if let Some(m) = missions
        .iter()
        .find(|m| !m.locked && m.category == "daily" && m.completed && !m.claimed)
    {
        return Some(m.clone());
    }
    // 4. Unclaimed weekly
    if let Some(m) = missions
        .iter()
        .find(|m| !m.locked && m.category == "weekly" && m.completed && !m.claimed)
    {
        return Some(m.clone());
    }
    // 5. Incomplete daily
    if let Some(m) = missions
        .iter()
        .find(|m| !m.locked && m.category == "daily" && !m.completed)
    {
        return Some(m.clone());
    }
    // 6. Incomplete weekly
    if let Some(m) = missions
        .iter()
        .find(|m| !m.locked && m.category == "weekly" && !m.completed)
    {
        return Some(m.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_missions_are_sequential_and_balanced() {
        assert_eq!(ONBOARDING_MISSIONS.len(), 6);
        let total_crowns: u64 = ONBOARDING_MISSIONS.iter().map(|m| m.crowns_reward).sum();
        let total_gems: u64 = ONBOARDING_MISSIONS.iter().map(|m| m.gems_reward).sum();
        assert_eq!(total_crowns, 320);
        assert_eq!(total_gems, 5);
    }

    #[test]
    fn daily_missions_are_deterministic_and_unique_per_lane() {
        for day in 20000..20020 {
            let hand = daily_missions_for_day(day);
            assert_eq!(hand.len(), 3);
            let mut ids = BTreeSet::new();
            let mut tags = BTreeSet::new();
            let mut lanes = BTreeSet::new();
            for m in &hand {
                assert!(ids.insert(m.id));
                assert!(tags.insert(m.tag));
                lanes.insert(m.lane);
            }
            assert_eq!(lanes.len(), 3, "day {day} must cover all 3 diagnostic lanes");
        }
        assert_eq!(daily_missions_for_day(20000), daily_missions_for_day(20000));
    }

    #[test]
    fn playability_guard_blocks_unowned_hero_locks_on_empty_wallet() {
        let oceania_def = mission_by_id("daily_oceania_3").unwrap();
        let epic_teams_def = mission_by_id("daily_teams_epic_win").unwrap();
        let owned = BTreeSet::new();
        // Pick a rotation containing zero Oceania & zero Epic heroes if possible:
        let europe_common = [crate::leaders::Leader::of("boudica")];
        let locked_ctx = PlayabilityContext {
            owned_leaders: &owned,
            free_rotation: &europe_common,
            crowns: 0,
            gems: 0,
        };
        assert!(!is_mission_currently_playable(oceania_def, Some(&locked_ctx)));
        assert!(!is_mission_currently_playable(epic_teams_def, Some(&locked_ctx)));

        // Adding Kamehameha (Oceania) or owning an Epic hero unlocks them.
        let oceania_rot = [crate::leaders::Leader::of("kamehameha")];
        let oceania_ctx = PlayabilityContext {
            owned_leaders: &owned,
            free_rotation: &oceania_rot,
            crowns: 0,
            gems: 0,
        };
        assert!(is_mission_currently_playable(oceania_def, Some(&oceania_ctx)));

        // Even with an empty rotation, dealing hands guarantees playable missions and distinct tags.
        let mut owned_some = BTreeSet::new();
        owned_some.insert("boudica".to_string());
        for salt in 0..50 {
            let hand = deal_daily_hand_for_player(
                20100,
                salt,
                Some(&PlayabilityContext {
                    owned_leaders: &owned_some,
                    free_rotation: &europe_common,
                    crowns: 0,
                    gems: 0,
                }),
            );
            assert_eq!(hand.len(), 3);
            for def in hand {
                assert!(
                    is_mission_currently_playable(&def, Some(&locked_ctx)),
                    "dealt unplayable mission {} with empty wallet",
                    def.id
                );
            }
        }
    }

    #[test]
    fn directive_prioritizes_rewards_then_sequential_onboarding() {
        let stats = MissionStats {
            player_level: 1,
            tutorial_completed: true,
            ..Default::default()
        };
        let views = build_all_missions_view(&stats, &Default::default(), 100);
        let directive = active_command_directive(&views).expect("has active directive");
        assert_eq!(directive.id, "onboarding_1_tutorial");
        assert!(directive.completed);
        assert!(!directive.claimed);
        assert!(views.iter().find(|m| m.id == "onboarding_2_baptism").unwrap().locked);
        assert!(views.iter().filter(|m| m.category == "daily").all(|m| m.locked));
        assert!(views.iter().filter(|m| m.category == "weekly").all(|m| m.locked));

        let mut saved = BTreeMap::new();
        saved.insert(
            "onboarding_1_tutorial".to_string(),
            MissionRecord {
                id: "onboarding_1_tutorial".to_string(),
                progress: 1,
                completed: true,
                claimed: true,
                ..Default::default()
            },
        );
        let views2 = build_all_missions_view(&stats, &saved, 100);
        let directive2 = active_command_directive(&views2).expect("has active directive");
        assert_eq!(directive2.id, "onboarding_2_baptism");
        assert!(!directive2.completed);
        assert!(!directive2.locked);
        assert!(views2.iter().find(|m| m.id == "onboarding_3_rotation_hero").unwrap().locked);
    }

    #[test]
    fn daily_and_weekly_unlock_by_level_and_reset_by_period() {
        let mut records = BTreeMap::new();
        let mut daily_day = 0;
        let mut weekly_period = 0;

        advance_missions_after_match(
            &mut records,
            &mut daily_day,
            &mut weekly_period,
            100,
            10,
            &MatchMissionContext {
                player_level: 1,
                mode: MatchMode::Teams,
                is_ranked: false,
                used_free_rotation: true,
                won: true,
                kills: 15,
                empires_defeated: 2,
                ..Default::default()
            },
        );
        assert!(!records.contains_key("onboarding_2_baptism"));
        assert!(records.keys().all(|k| !k.starts_with("daily_")));

        records.insert(
            "onboarding_1_tutorial".to_string(),
            MissionRecord {
                id: "onboarding_1_tutorial".to_string(),
                progress: 1,
                completed: true,
                claimed: true,
                ..Default::default()
            },
        );
        advance_missions_after_match(
            &mut records,
            &mut daily_day,
            &mut weekly_period,
            100,
            10,
            &MatchMissionContext {
                player_level: 3,
                mode: MatchMode::Teams,
                is_ranked: false,
                used_free_rotation: true,
                won: true,
                kills: 10,
                empires_defeated: 2,
                structures_built: 6,
                peak_territory_rank: Some(1),
                total_contenders: 8,
                leader_id: Some("boudica"),
                leader_region: Some(Region::Europe),
                leader_rarity: Some(Rarity::Common),
                alive_seconds: 200,
            },
        );
        assert_eq!(records["onboarding_2_baptism"].progress, 1);
        assert!(records.keys().any(|k| k.starts_with("daily_")));
        assert!(records.keys().all(|k| !k.starts_with("weekly_")));

        sync_period_missions(&mut records, &mut daily_day, &mut weekly_period, 101, 10);
        assert!(records.keys().all(|k| !k.starts_with("daily_")));
        assert_eq!(records["onboarding_2_baptism"].progress, 1);
    }

    #[test]
    fn telemetry_distinct_heroes_hnv_separation_and_swap_work() {
        let mut records = BTreeMap::new();
        let mut daily_day = 20000;
        let mut daily_ids = vec![
            "daily_ffa_survive_3m".to_string(),
            "daily_ffa_top3".to_string(),
            "daily_team_game".to_string(),
        ];
        let mut daily_swaps = 0u8;
        let mut daily_out = BTreeSet::new();
        let mut weekly_period = 10;
        let mut weekly_ids = vec![
            "weekly_variety_heroes".to_string(),
            "weekly_hnv_defender".to_string(),
            "weekly_peak_first_2".to_string(),
        ];
        let mut weekly_swaps = 0u8;
        let mut weekly_out = BTreeSet::new();

        // HnV match must NOT count as Teams even though team is present on HnV!
        // Also tests 2-contender guard on Top-3 FFA vs >=6 contender match.
        advance_missions_after_match_with_hands(
            HandSyncState {
                records: &mut records,
                daily_day: &mut daily_day,
                daily_mission_ids: &mut daily_ids,
                daily_swaps_used: &mut daily_swaps,
                daily_swapped_out_ids: &mut daily_out,
                weekly_period: &mut weekly_period,
                weekly_mission_ids: &mut weekly_ids,
                weekly_swaps_used: &mut weekly_swaps,
                weekly_swapped_out_ids: &mut weekly_out,
            },
            20000,
            10,
            0,
            None,
            &MatchMissionContext {
                player_level: 5,
                mode: MatchMode::HumansVsNations,
                won: true,
                leader_id: Some("boudica"),
                peak_territory_rank: Some(1),
                total_contenders: 6,
                alive_seconds: 210,
                ..Default::default()
            },
        );
        assert!(!records.contains_key("daily_team_game"));
        assert_eq!(records["weekly_hnv_defender"].progress, 1);
        assert_eq!(records["weekly_variety_heroes"].progress, 1);
        assert_eq!(records["weekly_peak_first_2"].progress, 1);

        // Playing the same hero again does NOT increment DistinctHeroes, but another hero does.
        for leader in ["boudica", "caesar", "cleopatra"] {
            advance_missions_after_match_with_hands(
                HandSyncState {
                    records: &mut records,
                    daily_day: &mut daily_day,
                    daily_mission_ids: &mut daily_ids,
                    daily_swaps_used: &mut daily_swaps,
                    daily_swapped_out_ids: &mut daily_out,
                    weekly_period: &mut weekly_period,
                    weekly_mission_ids: &mut weekly_ids,
                    weekly_swaps_used: &mut weekly_swaps,
                    weekly_swapped_out_ids: &mut weekly_out,
                },
                20000,
                10,
                0,
                None,
                &MatchMissionContext {
                    player_level: 5,
                    mode: MatchMode::Ffa,
                    leader_id: Some(leader),
                    peak_territory_rank: Some(2),
                    total_contenders: 8,
                    alive_seconds: 185,
                    ..Default::default()
                },
            );
        }
        assert!(records["weekly_variety_heroes"].completed);
        assert!(records["daily_ffa_survive_3m"].completed);
        assert!(records["daily_ffa_top3"].completed);

        // Swap replacement never picks already-active, swapped-out, or onboarding missions.
        let swap_target = mission_by_id("daily_team_game").unwrap();
        let rep = select_swap_replacement(
            swap_target,
            &daily_ids,
            &daily_out,
            42,
            None,
        )
        .unwrap();
        assert_ne!(rep.id, "daily_team_game");
        assert!(!daily_ids.contains(&rep.id.to_string()));
    }

    #[test]
    fn weekly_missions_are_deterministic_and_unique() {
        let period1 = weekly_missions_for_period(10);
        let period1_again = weekly_missions_for_period(10);
        assert_eq!(period1, period1_again);
        assert_eq!(period1.len(), 3);
        let mut ids = BTreeSet::new();
        for m in &period1 {
            assert!(ids.insert(m.id));
        }

        let views = build_all_missions_view(&MissionStats::default(), &Default::default(), 100);
        assert_eq!(views.len(), 6 + 3 + 3);
        assert_eq!(views.iter().filter(|m| m.category == "weekly").count(), 3);
    }
}
