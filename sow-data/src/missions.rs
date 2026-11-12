//! Missions and directives system for player onboarding, engagement, and retention.
//!
//! Two mission categories:
//! 1. **Onboarding ("Camino del Conquistador")**: 6 sequential milestones introducing
//!    core gameplay (Boudica tutorial, structure deployment, weekly rotation heroes,
//!    Team vs Team multiplayer, regional hero mastery, and team victory).
//! 2. **Daily Operations**: 3 rotating daily missions (reset every UTC day) focused
//!    on participation, cooperation, and variety. Never requires an FFA victory
//!    to avoid frustrating new or casual players.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionCategory {
    Onboarding,
    Daily,
    Weekly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionRequirement {
    CompleteTutorial,
    BuildStructures { count: u32 },
    PlayFreeRotationHero { count: u32 },
    PlayTeamGame { count: u32 },
    PlayRegionalHero { region_group: &'static str, count: u32 },
    WinTeamGame { count: u32 },
    PlayMatches { count: u32 },
    EliminateUnits { count: u32 },
    PlayRankedMatches { count: u32 },
    WinRankedMatches { count: u32 },
    DefeatEmpires { count: u32 },
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
}

pub const ONBOARDING_MISSIONS: &[MissionDefinition] = &[
    MissionDefinition {
        id: "onboarding_1_tutorial",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_tutorial_title",
        description_key: "missions.onboarding_tutorial_desc",
        crowns_reward: 50,
        gems_reward: 0,
        target: 1,
        requirement: MissionRequirement::CompleteTutorial,
        icon: "shield",
        order: 1,
    },
    MissionDefinition {
        id: "onboarding_2_structures",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_structures_title",
        description_key: "missions.onboarding_structures_desc",
        crowns_reward: 50,
        gems_reward: 0,
        target: 5,
        requirement: MissionRequirement::BuildStructures { count: 5 },
        icon: "tower",
        order: 2,
    },
    MissionDefinition {
        id: "onboarding_3_rotation_hero",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_rotation_title",
        description_key: "missions.onboarding_rotation_desc",
        crowns_reward: 75,
        gems_reward: 2,
        target: 1,
        requirement: MissionRequirement::PlayFreeRotationHero { count: 1 },
        icon: "rotation",
        order: 3,
    },
    MissionDefinition {
        id: "onboarding_4_team_match",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_team_title",
        description_key: "missions.onboarding_team_desc",
        crowns_reward: 75,
        gems_reward: 2,
        target: 1,
        requirement: MissionRequirement::PlayTeamGame { count: 1 },
        icon: "team",
        order: 4,
    },
    MissionDefinition {
        id: "onboarding_5_regional_hero",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_regional_title",
        description_key: "missions.onboarding_regional_desc",
        crowns_reward: 100,
        gems_reward: 3,
        target: 2,
        requirement: MissionRequirement::PlayRegionalHero {
            region_group: "west",
            count: 2,
        },
        icon: "globe",
        order: 5,
    },
    MissionDefinition {
        id: "onboarding_6_team_win",
        category: MissionCategory::Onboarding,
        title_key: "missions.onboarding_team_win_title",
        description_key: "missions.onboarding_team_win_desc",
        crowns_reward: 150,
        gems_reward: 5,
        target: 1,
        requirement: MissionRequirement::WinTeamGame { count: 1 },
        icon: "crown",
        order: 6,
    },
];

pub const DAILY_MISSION_POOL: &[MissionDefinition] = &[
    MissionDefinition {
        id: "daily_play_matches",
        category: MissionCategory::Daily,
        title_key: "missions.daily_play_matches_title",
        description_key: "missions.daily_play_matches_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 3,
        requirement: MissionRequirement::PlayMatches { count: 3 },
        icon: "swords",
        order: 10,
    },
    MissionDefinition {
        id: "daily_build_structures",
        category: MissionCategory::Daily,
        title_key: "missions.daily_build_structures_title",
        description_key: "missions.daily_build_structures_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 8,
        requirement: MissionRequirement::BuildStructures { count: 8 },
        icon: "tower",
        order: 11,
    },
    MissionDefinition {
        id: "daily_team_game",
        category: MissionCategory::Daily,
        title_key: "missions.daily_team_game_title",
        description_key: "missions.daily_team_game_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::PlayTeamGame { count: 2 },
        icon: "team",
        order: 12,
    },
    MissionDefinition {
        id: "daily_team_win",
        category: MissionCategory::Daily,
        title_key: "missions.daily_team_win_title",
        description_key: "missions.daily_team_win_desc",
        crowns_reward: 50,
        gems_reward: 2,
        target: 1,
        requirement: MissionRequirement::WinTeamGame { count: 1 },
        icon: "crown",
        order: 13,
    },
    MissionDefinition {
        id: "daily_west_heroes",
        category: MissionCategory::Daily,
        title_key: "missions.daily_west_heroes_title",
        description_key: "missions.daily_west_heroes_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::PlayRegionalHero {
            region_group: "west",
            count: 2,
        },
        icon: "globe",
        order: 14,
    },
    MissionDefinition {
        id: "daily_east_heroes",
        category: MissionCategory::Daily,
        title_key: "missions.daily_east_heroes_title",
        description_key: "missions.daily_east_heroes_desc",
        crowns_reward: 35,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::PlayRegionalHero {
            region_group: "east",
            count: 2,
        },
        icon: "globe",
        order: 15,
    },
    MissionDefinition {
        id: "daily_free_rotation",
        category: MissionCategory::Daily,
        title_key: "missions.daily_free_rotation_title",
        description_key: "missions.daily_free_rotation_desc",
        crowns_reward: 40,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::PlayFreeRotationHero { count: 2 },
        icon: "rotation",
        order: 16,
    },
    MissionDefinition {
        id: "daily_eliminate_units",
        category: MissionCategory::Daily,
        title_key: "missions.daily_eliminate_units_title",
        description_key: "missions.daily_eliminate_units_desc",
        crowns_reward: 40,
        gems_reward: 2,
        target: 50,
        requirement: MissionRequirement::EliminateUnits { count: 50 },
        icon: "crosshair",
        order: 17,
    },
    MissionDefinition {
        id: "daily_ranked_duel",
        category: MissionCategory::Daily,
        title_key: "missions.daily_ranked_duel_title",
        description_key: "missions.daily_ranked_duel_desc",
        crowns_reward: 45,
        gems_reward: 2,
        target: 2,
        requirement: MissionRequirement::PlayRankedMatches { count: 2 },
        icon: "swords",
        order: 18,
    },
    MissionDefinition {
        id: "daily_ranked_victory",
        category: MissionCategory::Daily,
        title_key: "missions.daily_ranked_victory_title",
        description_key: "missions.daily_ranked_victory_desc",
        crowns_reward: 50,
        gems_reward: 2,
        target: 1,
        requirement: MissionRequirement::WinRankedMatches { count: 1 },
        icon: "crown",
        order: 19,
    },
    MissionDefinition {
        id: "daily_defeat_empires",
        category: MissionCategory::Daily,
        title_key: "missions.daily_defeat_empires_title",
        description_key: "missions.daily_defeat_empires_desc",
        crowns_reward: 40,
        gems_reward: 1,
        target: 2,
        requirement: MissionRequirement::DefeatEmpires { count: 2 },
        icon: "shield",
        order: 20,
    },
];

pub const WEEKLY_MISSION_POOL: &[MissionDefinition] = &[
    MissionDefinition {
        id: "weekly_ranked_champion",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_ranked_champion_title",
        description_key: "missions.weekly_ranked_champion_desc",
        crowns_reward: 150,
        gems_reward: 5,
        target: 3,
        requirement: MissionRequirement::WinRankedMatches { count: 3 },
        icon: "crown",
        order: 30,
    },
    MissionDefinition {
        id: "weekly_conqueror",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_conqueror_title",
        description_key: "missions.weekly_conqueror_desc",
        crowns_reward: 120,
        gems_reward: 4,
        target: 5,
        requirement: MissionRequirement::DefeatEmpires { count: 5 },
        icon: "swords",
        order: 31,
    },
    MissionDefinition {
        id: "weekly_grand_architect",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_grand_architect_title",
        description_key: "missions.weekly_grand_architect_desc",
        crowns_reward: 120,
        gems_reward: 3,
        target: 25,
        requirement: MissionRequirement::BuildStructures { count: 25 },
        icon: "tower",
        order: 32,
    },
    MissionDefinition {
        id: "weekly_veteran_commander",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_veteran_commander_title",
        description_key: "missions.weekly_veteran_commander_desc",
        crowns_reward: 100,
        gems_reward: 3,
        target: 10,
        requirement: MissionRequirement::PlayMatches { count: 10 },
        icon: "shield",
        order: 33,
    },
    MissionDefinition {
        id: "weekly_blood_and_iron",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_blood_and_iron_title",
        description_key: "missions.weekly_blood_and_iron_desc",
        crowns_reward: 120,
        gems_reward: 4,
        target: 150,
        requirement: MissionRequirement::EliminateUnits { count: 150 },
        icon: "crosshair",
        order: 34,
    },
    MissionDefinition {
        id: "weekly_free_rotation_master",
        category: MissionCategory::Weekly,
        title_key: "missions.weekly_free_rotation_master_title",
        description_key: "missions.weekly_free_rotation_master_desc",
        crowns_reward: 100,
        gems_reward: 3,
        target: 6,
        requirement: MissionRequirement::PlayFreeRotationHero { count: 6 },
        icon: "rotation",
        order: 35,
    },
];

/// Deterministically selects 3 distinct daily missions for a given UTC day.
pub fn daily_missions_for_day(epoch_day: u64) -> Vec<MissionDefinition> {
    let pool_len = DAILY_MISSION_POOL.len();
    if pool_len <= 3 {
        return DAILY_MISSION_POOL.to_vec();
    }
    let mut selected_indices = BTreeSet::new();
    let mut step = 0;
    while selected_indices.len() < 3 {
        let hash = (epoch_day.wrapping_mul(1103515245) + 12345 + step * 7919) as usize;
        let index = hash % pool_len;
        selected_indices.insert(index);
        step += 1;
    }
    selected_indices
        .into_iter()
        .map(|idx| DAILY_MISSION_POOL[idx])
        .collect()
}

/// Deterministically selects 3 distinct weekly missions for a weekly rotation period.
pub fn weekly_missions_for_period(rotation_period: u64) -> Vec<MissionDefinition> {
    let pool_len = WEEKLY_MISSION_POOL.len();
    if pool_len <= 3 {
        return WEEKLY_MISSION_POOL.to_vec();
    }
    let mut selected_indices = BTreeSet::new();
    let mut step = 0;
    while selected_indices.len() < 3 {
        let hash = (rotation_period.wrapping_mul(1664525) + 1013904223 + step * 9973) as usize;
        let index = hash % pool_len;
        selected_indices.insert(index);
        step += 1;
    }
    selected_indices
        .into_iter()
        .map(|idx| WEEKLY_MISSION_POOL[idx])
        .collect()
}

/// Statistics provided by player profile or session telemetry to evaluate missions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissionStats {
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

pub fn evaluate_mission_progress(
    def: &MissionDefinition,
    stats: &MissionStats,
    saved_progress: Option<&MissionRecord>,
) -> MissionView {
    let raw_progress = match def.requirement {
        MissionRequirement::CompleteTutorial => {
            if stats.tutorial_completed || saved_progress.map(|r| r.completed || r.progress >= 1).unwrap_or(false) {
                1
            } else {
                0
            }
        }
        MissionRequirement::BuildStructures { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.total_structures_built);
            p.min(count)
        }
        MissionRequirement::PlayFreeRotationHero { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.free_rotation_matches);
            p.min(count)
        }
        MissionRequirement::PlayTeamGame { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.team_matches);
            p.min(count)
        }
        MissionRequirement::PlayRegionalHero { region_group, count } => {
            let stat = if region_group == "west" {
                stats.west_regional_matches
            } else {
                stats.east_regional_matches
            };
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stat);
            p.min(count)
        }
        MissionRequirement::WinTeamGame { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.team_wins);
            p.min(count)
        }
        MissionRequirement::PlayMatches { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.matches_played);
            p.min(count)
        }
        MissionRequirement::EliminateUnits { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.kills);
            p.min(count)
        }
        MissionRequirement::PlayRankedMatches { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.ranked_matches);
            p.min(count)
        }
        MissionRequirement::WinRankedMatches { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.ranked_wins);
            p.min(count)
        }
        MissionRequirement::DefeatEmpires { count } => {
            let p = saved_progress.map(|r| r.progress).unwrap_or(0).max(stats.empires_defeated);
            p.min(count)
        }
    };

    let target = def.target;
    let progress = raw_progress.min(target);
    let completed = progress >= target || saved_progress.map(|r| r.completed).unwrap_or(false);
    let claimed = saved_progress.map(|r| r.claimed).unwrap_or(false);

    MissionView {
        id: def.id.to_string(),
        category: match def.category {
            MissionCategory::Onboarding => "onboarding".to_string(),
            MissionCategory::Daily => "daily".to_string(),
            MissionCategory::Weekly => "weekly".to_string(),
        },
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
    }
}

/// Builds the complete view of missions for a player: all 6 onboarding missions plus
/// the 3 daily missions for the specified epoch day, and the 3 weekly missions for the current weekly period.
pub fn build_all_missions_view(
    stats: &MissionStats,
    saved_records: &BTreeMap<String, MissionRecord>,
    epoch_day: u64,
) -> Vec<MissionView> {
    let mut views = Vec::new();
    for def in ONBOARDING_MISSIONS {
        let saved = saved_records.get(def.id);
        views.push(evaluate_mission_progress(def, stats, saved));
    }
    let daily_defs = daily_missions_for_day(epoch_day);
    for def in &daily_defs {
        let saved = saved_records.get(def.id);
        views.push(evaluate_mission_progress(def, stats, saved));
    }
    let period = crate::commerce::rotation_period(epoch_day.saturating_mul(86_400));
    let weekly_defs = weekly_missions_for_period(period);
    for def in &weekly_defs {
        let saved = saved_records.get(def.id);
        views.push(evaluate_mission_progress(def, stats, saved));
    }
    views
}

/// Identifies the single active directive that should be featured on the command console.
/// Priority:
/// 1. First completed-but-unclaimed onboarding mission (reward ready!).
/// 2. First in-progress onboarding mission.
/// 3. First completed-but-unclaimed daily mission.
/// 4. First completed-but-unclaimed weekly mission.
/// 5. First in-progress daily mission.
/// 6. First in-progress weekly mission.
pub fn active_command_directive(missions: &[MissionView]) -> Option<MissionView> {
    // 1. Unclaimed onboarding
    if let Some(m) = missions.iter().find(|m| m.category == "onboarding" && m.completed && !m.claimed) {
        return Some(m.clone());
    }
    // 2. Incomplete onboarding
    if let Some(m) = missions.iter().find(|m| m.category == "onboarding" && !m.completed) {
        return Some(m.clone());
    }
    // 3. Unclaimed daily
    if let Some(m) = missions.iter().find(|m| m.category == "daily" && m.completed && !m.claimed) {
        return Some(m.clone());
    }
    // 4. Unclaimed weekly
    if let Some(m) = missions.iter().find(|m| m.category == "weekly" && m.completed && !m.claimed) {
        return Some(m.clone());
    }
    // 5. Incomplete daily
    if let Some(m) = missions.iter().find(|m| m.category == "daily" && !m.completed) {
        return Some(m.clone());
    }
    // 6. Incomplete weekly
    if let Some(m) = missions.iter().find(|m| m.category == "weekly" && !m.completed) {
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
        assert_eq!(total_crowns, 500);
        assert_eq!(total_gems, 12);
    }

    #[test]
    fn daily_missions_are_deterministic_and_unique() {
        let day1 = daily_missions_for_day(20000);
        let day1_again = daily_missions_for_day(20000);
        assert_eq!(day1, day1_again);
        assert_eq!(day1.len(), 3);
        let mut ids = BTreeSet::new();
        for m in &day1 {
            assert!(ids.insert(m.id));
        }

        let day2 = daily_missions_for_day(20001);
        assert_eq!(day2.len(), 3);
    }

    #[test]
    fn directive_prioritizes_rewards_then_onboarding() {
        let stats = MissionStats {
            tutorial_completed: true,
            ..Default::default()
        };
        let views = build_all_missions_view(&stats, &Default::default(), 100);
        // Onboarding 1 is completed and unclaimed
        let directive = active_command_directive(&views).expect("has active directive");
        assert_eq!(directive.id, "onboarding_1_tutorial");
        assert!(directive.completed);
        assert!(!directive.claimed);

        // Claim onboarding 1
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
        assert_eq!(directive2.id, "onboarding_2_structures");
        assert!(!directive2.completed);
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
        assert_eq!(views.len(), 6 + 3 + 3); // 6 onboarding + 3 daily + 3 weekly
        assert_eq!(views.iter().filter(|m| m.category == "weekly").count(), 3);
    }
}
