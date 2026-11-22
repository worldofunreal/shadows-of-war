//! Shared, deterministic rewards math for local client progress and database settlement.
//!
//! Two earned currencies, by product decision (owner, 2026-09):
//! - **Crowns** — the free spendable currency (leader unlocks, store).
//! - **Laurels** — achievement points. Earned, never spent; the Google Play
//!   "Laurel Hoard" achievement tracks them.
//!
//! Gems are the premium currency. Matches pay a small, server-verified gem
//! bonus (`match_gems`); purchases remain the main source.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RewardInput {
    pub won: bool,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub kills: u32,
    pub assists: u32,
    pub tutorial: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchReward {
    pub xp: u32,
    pub leader_xp: u32,
    /// Free spendable currency.
    pub crowns: u64,
    /// Achievement points (never spent).
    pub laurels: u64,
    /// Premium currency bonus. Only server-verified public results pay it.
    pub gems: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AchievementDefinition {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub points: u64,
    pub target: u64,
}

/// Extra reward paid when an achievement unlocks, on top of its laurels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AchievementReward {
    pub crowns: u64,
    pub gems: u64,
    /// Catalog id of a hero granted by the achievement.
    pub hero: Option<&'static str>,
    /// Public id of a skin granted by the achievement.
    pub skin: Option<&'static str>,
}

/// Reward table for achievements that pay more than laurels. Kept apart from
/// the catalog so the ~1,600 literal definitions stay untouched.
pub fn achievement_rewards(id: &str) -> Option<AchievementReward> {
    match id {
        "skin_5" => Some(AchievementReward {
            gems: 25,
            ..Default::default()
        }),
        "skin_9" => Some(AchievementReward {
            gems: 50,
            ..Default::default()
        }),
        _ => None,
    }
}

/// Gems paid when an achievement grants a hero the player already owns:
/// 22.5% of that hero's gem price. A duplicate is worth a fifth of the hero,
/// never its full price.
pub fn achievement_duplicate_gems(leader: crate::leaders::Leader) -> u64 {
    leader.price_gems().saturating_mul(9) / 40
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AchievementTotals {
    pub matches_played: u64,
    pub wins: u64,
    pub laurels: u64,
    pub leader_matches_played: u64,
    pub leader_wins: u64,
    pub distinct_leaders: u64,
    pub best_leader_xp: u64,
    pub kills: u64,
    pub assists: u64,
    pub players_defeated: u64,
    pub empires_defeated: u64,
    pub tribes_defeated: u64,
    pub completed_episodes: u64,
    pub advanced_tactics_completed: u64,
    pub owned_leaders_count: u64,
    pub owned_leaders: std::collections::BTreeSet<String>,
    pub skins_owned: u64,
    pub leader_matches: std::collections::BTreeMap<String, u64>,
    pub leader_wins_map: std::collections::BTreeMap<String, u64>,
}

/// The small, phase-specific snapshot used to attach match progress without
/// rebuilding the full achievement totals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchGoalCounters {
    pub matches_played: u64,
    pub wins: u64,
    pub hero_matches: u64,
    pub hero_wins: u64,
    pub kills: u64,
    pub assists: u64,
    pub players_defeated: u64,
    pub empires_defeated: u64,
    pub tribes_defeated: u64,
}

fn get_hero_matches(leader_matches: &std::collections::BTreeMap<String, u64>, slug: &str) -> u64 {
    if let Some(character) = crate::geo_entities::by_id(slug) {
        if let Some(hero) = &character.hero {
            if let Some(&val) = leader_matches.get(&hero.name) {
                return val;
            }
        }
        if let Some(&val) = leader_matches.get(&character.name) {
            return val;
        }
    }
    if let Some(&val) = leader_matches.get(slug) {
        return val;
    }
    for (k, &val) in leader_matches {
        if k.eq_ignore_ascii_case(slug) {
            return val;
        }
    }
    0
}

fn get_hero_wins(leader_wins: &std::collections::BTreeMap<String, u64>, slug: &str) -> u64 {
    if let Some(character) = crate::geo_entities::by_id(slug) {
        if let Some(hero) = &character.hero {
            if let Some(&val) = leader_wins.get(&hero.name) {
                return val;
            }
        }
        if let Some(&val) = leader_wins.get(&character.name) {
            return val;
        }
    }
    if let Some(&val) = leader_wins.get(slug) {
        return val;
    }
    for (k, &val) in leader_wins {
        if k.eq_ignore_ascii_case(slug) {
            return val;
        }
    }
    0
}

pub fn achievement_progress(id: &str, totals: &AchievementTotals) -> u64 {
    // 1. Google Play / Core 9
    match id {
        "first_command" | "battle_hardened" => return totals.matches_played,
        "first_victory" | "victory_march" => return totals.wins,
        "laurel_hoard" => return totals.laurels,
        "commander_victorious" => return totals.leader_wins,
        "veteran_commander" => return totals.leader_matches_played,
        "banner_collector" => return totals.distinct_leaders,
        "leader_path" => return totals.best_leader_xp,
        _ => {}
    }

    // 2. Global Battles & Victories
    if id.starts_with("battles_") {
        return totals.matches_played;
    }
    if id.starts_with("victories_") {
        return totals.wins;
    }

    // 3. Combat
    if id.starts_with("kills_") {
        return totals.kills;
    }
    if id.starts_with("assists_") {
        return totals.assists;
    }
    if id.starts_with("players_") {
        return totals.players_defeated;
    }
    if id.starts_with("empires_") {
        return totals.empires_defeated;
    }
    if id.starts_with("tribes_") {
        return totals.tribes_defeated;
    }

    // 4. Campaign
    if id == "grand_tactician" {
        return totals.advanced_tactics_completed;
    }
    if id.starts_with("campaign_") {
        return totals.completed_episodes;
    }

    // 5. Laurels
    if id.starts_with("laurels_") {
        return totals.laurels;
    }

    // 6. Roster milestones
    if id.starts_with("roster_") {
        return totals.owned_leaders_count;
    }

    // 7. Skin collection milestones
    if id.starts_with("skin_") {
        return totals.skins_owned;
    }

    // 8. Hero recruitments: recruit_{slug}
    if let Some(slug) = id.strip_prefix("recruit_") {
        if totals.owned_leaders.contains(slug) {
            return 1;
        }
        if let Some(character) = crate::geo_entities::by_id(slug) {
            if let Some(hero) = &character.hero {
                if totals.owned_leaders.contains(&hero.name) {
                    return 1;
                }
            }
            if totals.owned_leaders.contains(&character.name) {
                return 1;
            }
        }
        return 0;
    }

    // 9. Hero mastery battles: {slug}_mastery_{target}
    if let Some(idx) = id.find("_mastery_") {
        let slug = &id[..idx];
        return get_hero_matches(&totals.leader_matches, slug);
    }

    // 10. Hero victories: {slug}_wins_{target}
    if let Some(idx) = id.find("_wins_") {
        let slug = &id[..idx];
        return get_hero_wins(&totals.leader_wins_map, slug);
    }

    0
}

#[derive(Clone, Copy)]
struct GoalMilestone {
    id: &'static str,
    target: u64,
}

#[derive(Default)]
struct GoalMilestoneIndex {
    chains: std::collections::HashMap<String, Vec<GoalMilestone>>,
    hero_by_leader: std::collections::HashMap<String, &'static str>,
}

fn goal_chain_for_achievement(id: &str) -> Option<String> {
    let chain = match id {
        "first_command" | "battle_hardened" => "series:battles",
        "first_victory" | "victory_march" => "series:victories",
        _ if id.starts_with("battles_") => "series:battles",
        _ if id.starts_with("victories_") => "series:victories",
        _ if id.starts_with("kills_") => "series:kills",
        _ if id.starts_with("assists_") => "series:assists",
        _ if id.starts_with("players_") => "series:players",
        _ if id.starts_with("empires_") => "series:empires",
        _ if id.starts_with("tribes_") => "series:tribes",
        _ => {
            if let Some(index) = id.find("_mastery_") {
                return Some(format!("hero_mastery:{}", &id[..index]));
            }
            if let Some(index) = id.find("_wins_") {
                return Some(format!("hero_wins:{}", &id[..index]));
            }
            return None;
        }
    };
    Some(chain.to_string())
}

fn goal_milestone_index() -> &'static GoalMilestoneIndex {
    static INDEX: std::sync::OnceLock<GoalMilestoneIndex> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = GoalMilestoneIndex::default();
        for achievement in all_achievements() {
            if let Some(chain) = goal_chain_for_achievement(achievement.id) {
                index.chains.entry(chain).or_default().push(GoalMilestone {
                    id: achievement.id,
                    target: achievement.target,
                });
            }
        }
        for milestones in index.chains.values_mut() {
            milestones.sort_by(|left, right| {
                left.target
                    .cmp(&right.target)
                    .then_with(|| left.id.cmp(right.id))
            });
        }
        for character in crate::geo_entities::all() {
            if let Some(hero) = &character.hero {
                for name in [
                    character.id.as_str(),
                    character.name.as_str(),
                    hero.name.as_str(),
                ] {
                    index
                        .hero_by_leader
                        .insert(name.to_ascii_lowercase(), character.id.as_str());
                }
            }
        }
        index
    })
}

fn indexed_goal_advance(
    index: &GoalMilestoneIndex,
    chain_id: &str,
    before: u64,
    after: u64,
) -> Option<crate::profile::GoalAdvance> {
    if after <= before {
        return None;
    }
    let milestones = index.chains.get(chain_id)?;
    let next_index = milestones.partition_point(|milestone| milestone.target <= before);
    if next_index == milestones.len() {
        return None;
    }
    // When one receipt crosses multiple thresholds, use its highest crossed
    // milestone as the row while the receipt retains every unlocked ID.
    let reached_end = milestones.partition_point(|milestone| milestone.target <= after);
    let selected_index = if reached_end > next_index {
        reached_end - 1
    } else {
        next_index
    };
    let selected = milestones[selected_index];
    let previous_target = selected_index
        .checked_sub(1)
        .map_or(0, |previous| milestones[previous].target);
    Some(crate::profile::GoalAdvance {
        chain_id: chain_id.to_string(),
        achievement_id: selected.id.to_string(),
        progress_before: before,
        progress_after: after,
        previous_target,
        target: selected.target,
    })
}

fn credited_hero_slug(index: &GoalMilestoneIndex, leader: &str) -> Option<&'static str> {
    index
        .hero_by_leader
        .get(&leader.to_ascii_lowercase())
        .copied()
}

/// Match participation changes only the overall battle chain and the credited
/// hero's mastery chain. The indexed lookup avoids walking the full catalog.
pub fn participation_goal_advances(
    before: &MatchGoalCounters,
    after: &MatchGoalCounters,
    leader: &str,
) -> Vec<crate::profile::GoalAdvance> {
    let index = goal_milestone_index();
    let mut advances = Vec::with_capacity(2);
    if let Some(advance) = indexed_goal_advance(
        index,
        "series:battles",
        before.matches_played,
        after.matches_played,
    ) {
        advances.push(advance);
    }
    if let Some(slug) = credited_hero_slug(index, leader) {
        let chain_id = format!("hero_mastery:{slug}");
        if let Some(advance) =
            indexed_goal_advance(index, &chain_id, before.hero_matches, after.hero_matches)
        {
            advances.push(advance);
        }
    }
    advances
}

/// Verified match results add two victory counters and five combat counters.
/// Each changed counter performs one binary search over its short milestone list.
pub fn verified_goal_advances(
    before: &MatchGoalCounters,
    after: &MatchGoalCounters,
    leader: &str,
) -> Vec<crate::profile::GoalAdvance> {
    let index = goal_milestone_index();
    let mut advances = Vec::with_capacity(7);
    for (chain_id, before_value, after_value) in [
        ("series:victories", before.wins, after.wins),
        ("series:kills", before.kills, after.kills),
        ("series:assists", before.assists, after.assists),
        (
            "series:players",
            before.players_defeated,
            after.players_defeated,
        ),
        (
            "series:empires",
            before.empires_defeated,
            after.empires_defeated,
        ),
        (
            "series:tribes",
            before.tribes_defeated,
            after.tribes_defeated,
        ),
    ] {
        if let Some(advance) = indexed_goal_advance(index, chain_id, before_value, after_value) {
            advances.push(advance);
        }
    }
    if let Some(slug) = credited_hero_slug(index, leader) {
        let chain_id = format!("hero_wins:{slug}");
        if let Some(advance) =
            indexed_goal_advance(index, &chain_id, before.hero_wins, after.hero_wins)
        {
            advances.push(advance);
        }
    }
    advances
}

/// Project server achievement awards without changing the caller's profile.
pub fn newly_unlocked_achievements(
    totals: &AchievementTotals,
    already_unlocked: &std::collections::BTreeSet<String>,
) -> Vec<AchievementDefinition> {
    let mut unlocked = already_unlocked.clone();
    let mut newly_unlocked = Vec::new();
    let mut running_laurels = totals.laurels;
    // Point milestones count the points awarded in this same refresh, so they
    // are checked after every other achievement and until none cascades.
    for achievement in all_achievements()
        .iter()
        .filter(|achievement| achievement.category != "laurels")
    {
        if achievement_progress(achievement.id, totals) >= achievement.target
            && unlocked.insert(achievement.id.to_string())
        {
            newly_unlocked.push(*achievement);
            running_laurels = running_laurels.saturating_add(achievement.points);
        }
    }
    loop {
        let before = newly_unlocked.len();
        for achievement in all_achievements()
            .iter()
            .filter(|achievement| achievement.category == "laurels")
        {
            if running_laurels >= achievement.target
                && unlocked.insert(achievement.id.to_string())
            {
                newly_unlocked.push(*achievement);
                running_laurels = running_laurels.saturating_add(achievement.points);
            }
        }
        if newly_unlocked.len() == before {
            break newly_unlocked;
        }
    }
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn all_achievements() -> &'static [AchievementDefinition] {
    static ALL: std::sync::OnceLock<Vec<AchievementDefinition>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let mut list = Vec::with_capacity(1600);

        // --- Canonical Google Play / Core 9 Achievements ---
        list.extend_from_slice(&[
            AchievementDefinition {
                id: "first_command",
                category: "battles",
                title: "First Command",
                description: "Complete your first match.",
                points: 10,
                target: 1,
            },
            AchievementDefinition {
                id: "first_victory",
                category: "victories",
                title: "First Victory",
                description: "Win a verified match.",
                points: 25,
                target: 1,
            },
            AchievementDefinition {
                id: "battle_hardened",
                category: "battles",
                title: "Battle Hardened",
                description: "Complete 10 matches.",
                points: 25,
                target: 10,
            },
            AchievementDefinition {
                id: "victory_march",
                category: "victories",
                title: "Victory March",
                description: "Win 10 verified matches.",
                points: 50,
                target: 10,
            },
            AchievementDefinition {
                id: "laurel_hoard",
                category: "laurels",
                title: "Laurel Hoard",
                description: "Earn 100 achievement points.",
                points: 50,
                target: 100,
            },
            AchievementDefinition {
                id: "commander_victorious",
                category: "victories",
                title: "Commander Victorious",
                description: "Win with a commander.",
                points: 25,
                target: 1,
            },
            AchievementDefinition {
                id: "veteran_commander",
                category: "commanders",
                title: "Veteran Commander",
                description: "Complete 10 matches with commanders.",
                points: 50,
                target: 10,
            },
            AchievementDefinition {
                id: "banner_collector",
                category: "commanders",
                title: "Banner Collector",
                description: "Command three different leaders.",
                points: 50,
                target: 3,
            },
            AchievementDefinition {
                id: "leader_path",
                category: "commanders",
                title: "Leader Path",
                description: "Earn 1,000 leader experience.",
                points: 100,
                target: 1_000,
            },
        ]);

        // --- Tiered Battles Matrix ---
        list.extend_from_slice(&[
            AchievementDefinition { id: "battles_3", category: "battles", title: "Vanguard", description: "Complete 3 matches.", points: 15, target: 3 },
            AchievementDefinition { id: "battles_25", category: "battles", title: "Ironclad", description: "Complete 25 matches.", points: 50, target: 25 },
            AchievementDefinition { id: "battles_50", category: "battles", title: "Centurion", description: "Complete 50 matches.", points: 75, target: 50 },
            AchievementDefinition { id: "battles_100", category: "battles", title: "Warlord", description: "Complete 100 matches.", points: 100, target: 100 },
            AchievementDefinition { id: "battles_250", category: "battles", title: "Warmaster", description: "Complete 250 matches.", points: 200, target: 250 },
            AchievementDefinition { id: "battles_500", category: "battles", title: "Juggernaut", description: "Complete 500 matches.", points: 350, target: 500 },
            AchievementDefinition { id: "battles_1000", category: "battles", title: "Oblivion", description: "Complete 1,000 matches.", points: 1000, target: 1_000 },
        ]);

        // --- Tiered Victories Matrix ---
        list.extend_from_slice(&[
            AchievementDefinition { id: "victories_3", category: "victories", title: "Blood Oath", description: "Win 3 verified matches.", points: 30, target: 3 },
            AchievementDefinition { id: "victories_25", category: "victories", title: "Triumph", description: "Win 25 verified matches.", points: 75, target: 25 },
            AchievementDefinition { id: "victories_50", category: "victories", title: "Conqueror", description: "Win 50 verified matches.", points: 100, target: 50 },
            AchievementDefinition { id: "victories_100", category: "victories", title: "Overlord", description: "Win 100 verified matches.", points: 150, target: 100 },
            AchievementDefinition { id: "victories_250", category: "victories", title: "Dominion", description: "Win 250 verified matches.", points: 250, target: 250 },
            AchievementDefinition { id: "victories_500", category: "victories", title: "Imperator", description: "Win 500 verified matches.", points: 500, target: 500 },
            AchievementDefinition { id: "victories_1000", category: "victories", title: "Sovereign", description: "Win 1,000 verified matches.", points: 1000, target: 1_000 },
        ]);

        // --- Combat Supremacy ---
        list.extend_from_slice(&[
            AchievementDefinition { id: "kills_10", category: "combat", title: "First Blood", description: "Defeat 10 enemy units.", points: 20, target: 10 },
            AchievementDefinition { id: "kills_25", category: "combat", title: "Skirmisher", description: "Defeat 25 enemy units.", points: 35, target: 25 },
            AchievementDefinition { id: "kills_50", category: "combat", title: "Reaper", description: "Defeat 50 enemy units.", points: 50, target: 50 },
            AchievementDefinition { id: "kills_100", category: "combat", title: "Slayer", description: "Defeat 100 enemy units.", points: 75, target: 100 },
            AchievementDefinition { id: "kills_250", category: "combat", title: "Executioner", description: "Defeat 250 enemy units.", points: 150, target: 250 },
            AchievementDefinition { id: "kills_500", category: "combat", title: "Warbringer", description: "Defeat 500 enemy units.", points: 250, target: 500 },
            AchievementDefinition { id: "kills_1000", category: "combat", title: "Death Incarnate", description: "Defeat 1,000 enemy units.", points: 500, target: 1_000 },
            AchievementDefinition { id: "kills_2500", category: "combat", title: "Doom of Empires", description: "Defeat 2,500 enemy units.", points: 750, target: 2_500 },
            AchievementDefinition { id: "kills_5000", category: "combat", title: "Lord of the Grave", description: "Defeat 5,000 enemy units.", points: 1000, target: 5_000 },
            AchievementDefinition { id: "assists_10", category: "combat", title: "Wingman", description: "Record 10 combat assists.", points: 15, target: 10 },
            AchievementDefinition { id: "assists_50", category: "combat", title: "Vanguard Shield", description: "Record 50 combat assists.", points: 50, target: 50 },
            AchievementDefinition { id: "assists_250", category: "combat", title: "Tactical Hand", description: "Record 250 combat assists.", points: 150, target: 250 },
            AchievementDefinition { id: "assists_1000", category: "combat", title: "Master of Cohorts", description: "Record 1,000 combat assists.", points: 500, target: 1_000 },
            AchievementDefinition { id: "players_5", category: "combat", title: "Duelist Initiate", description: "Eliminate 5 rival commanders.", points: 15, target: 5 },
            AchievementDefinition { id: "players_10", category: "combat", title: "Duelist", description: "Eliminate 10 rival commanders.", points: 30, target: 10 },
            AchievementDefinition { id: "players_25", category: "combat", title: "Champion Slayer", description: "Eliminate 25 rival commanders.", points: 50, target: 25 },
            AchievementDefinition { id: "players_50", category: "combat", title: "Crown Breaker", description: "Eliminate 50 rival commanders.", points: 100, target: 50 },
            AchievementDefinition { id: "players_100", category: "combat", title: "Sovereign Hunter", description: "Eliminate 100 rival commanders.", points: 200, target: 100 },
            AchievementDefinition { id: "players_250", category: "combat", title: "Kingmaker", description: "Eliminate 250 rival commanders.", points: 500, target: 250 },
            AchievementDefinition { id: "empires_5", category: "combat", title: "Border Raider", description: "Overthrow 5 rival empires.", points: 25, target: 5 },
            AchievementDefinition { id: "empires_10", category: "combat", title: "Empire Slayer", description: "Overthrow 10 rival empires.", points: 50, target: 10 },
            AchievementDefinition { id: "empires_25", category: "combat", title: "Dynasty Fall", description: "Overthrow 25 rival empires.", points: 100, target: 25 },
            AchievementDefinition { id: "empires_50", category: "combat", title: "World Breaker", description: "Overthrow 50 rival empires.", points: 250, target: 50 },
            AchievementDefinition { id: "empires_100", category: "combat", title: "Scourge of Realms", description: "Overthrow 100 rival empires.", points: 500, target: 100 },
            AchievementDefinition { id: "tribes_10", category: "combat", title: "Chieftain Bane", description: "Subjugate 10 native tribes.", points: 25, target: 10 },
            AchievementDefinition { id: "tribes_25", category: "combat", title: "Subjugator", description: "Subjugate 25 native tribes.", points: 50, target: 25 },
            AchievementDefinition { id: "tribes_50", category: "combat", title: "Steppe Dominator", description: "Subjugate 50 native tribes.", points: 100, target: 50 },
            AchievementDefinition { id: "tribes_100", category: "combat", title: "Overlord of the Wild", description: "Subjugate 100 native tribes.", points: 250, target: 100 },
        ]);

        // --- Campaign Chronicles ---
        list.extend_from_slice(&[
            AchievementDefinition { id: "campaign_1", category: "campaign", title: "Pathfinder", description: "Complete 1 campaign episode.", points: 25, target: 1 },
            AchievementDefinition { id: "campaign_3", category: "campaign", title: "Trailblazer", description: "Complete 3 campaign episodes.", points: 35, target: 3 },
            AchievementDefinition { id: "campaign_5", category: "campaign", title: "Saga Victor", description: "Complete 5 campaign episodes.", points: 50, target: 5 },
            AchievementDefinition { id: "campaign_10", category: "campaign", title: "Epic Historian", description: "Complete 10 campaign episodes.", points: 75, target: 10 },
            AchievementDefinition { id: "campaign_15", category: "campaign", title: "Chronicles of War", description: "Complete 15 campaign episodes.", points: 100, target: 15 },
            AchievementDefinition { id: "campaign_25", category: "campaign", title: "Grand Campaigner", description: "Complete 25 campaign episodes.", points: 150, target: 25 },
            AchievementDefinition { id: "campaign_40", category: "campaign", title: "Master of History", description: "Complete 40 campaign episodes.", points: 250, target: 40 },
            AchievementDefinition { id: "grand_tactician", category: "campaign", title: "Grand Tactician", description: "Complete the Advanced Tactics campaign.", points: 200, target: 3 },
        ]);

        // --- Laurels & Prestige ---
        list.extend_from_slice(&[
            AchievementDefinition { id: "laurels_250", category: "laurels", title: "Gilded Crown", description: "Earn 250 achievement points.", points: 50, target: 250 },
            AchievementDefinition { id: "laurels_500", category: "laurels", title: "Imperial Grace", description: "Earn 500 achievement points.", points: 100, target: 500 },
            AchievementDefinition { id: "laurels_1000", category: "laurels", title: "Vault of Glory", description: "Earn 1,000 achievement points.", points: 200, target: 1_000 },
            AchievementDefinition { id: "laurels_2500", category: "laurels", title: "Golden Emperor", description: "Earn 2,500 achievement points.", points: 500, target: 2_500 },
            AchievementDefinition { id: "laurels_5000", category: "laurels", title: "Immortal Renown", description: "Earn 5,000 achievement points.", points: 1000, target: 5_000 },
            AchievementDefinition { id: "laurels_10000", category: "laurels", title: "Demigod of War", description: "Earn 10,000 achievement points.", points: 2500, target: 10_000 },
        ]);

        // --- Hero Roster Milestones ---
        list.extend_from_slice(&[
            AchievementDefinition { id: "roster_1", category: "commanders", title: "First Recruit", description: "Command or own 1 hero.", points: 25, target: 1 },
            AchievementDefinition { id: "roster_3", category: "commanders", title: "Cohort", description: "Command or own 3 heroes.", points: 50, target: 3 },
            AchievementDefinition { id: "roster_5", category: "commanders", title: "Council of War", description: "Command or own 5 heroes.", points: 75, target: 5 },
            AchievementDefinition { id: "roster_10", category: "commanders", title: "Grand Legion", description: "Command or own 10 heroes.", points: 100, target: 10 },
            AchievementDefinition { id: "roster_25", category: "commanders", title: "High Command", description: "Command or own 25 heroes.", points: 200, target: 25 },
            AchievementDefinition { id: "roster_50", category: "commanders", title: "Army of Legends", description: "Command or own 50 heroes.", points: 350, target: 50 },
            AchievementDefinition { id: "roster_80", category: "commanders", title: "Pantheon of War", description: "Command or own 80 heroes.", points: 1000, target: 80 },
        ]);

        // --- Skin Collection Milestones ---
        list.extend_from_slice(&[
            AchievementDefinition { id: "skin_1", category: "skins", title: "First Skin", description: "Acquire your first skin.", points: 25, target: 1 },
            AchievementDefinition { id: "skin_5", category: "skins", title: "Skin Collector", description: "Acquire 5 skins.", points: 75, target: 5 },
            AchievementDefinition { id: "skin_9", category: "skins", title: "Skin Connoisseur", description: "Acquire 9 skins.", points: 150, target: 9 },
        ]);

        // --- All 93 Heroes: Recruitment, Battles Mastery, and Victories ---
        let mastery_tiers: [(u64, u64, &str); 8] = [
            (3, 15, "Vanguard"),
            (10, 30, "Centurion"),
            (25, 50, "Veteran"),
            (50, 75, "Champion"),
            (100, 100, "Warlord"),
            (250, 200, "Warmaster"),
            (500, 350, "Juggernaut"),
            (1000, 1000, "Ascendant"),
        ];

        let win_tiers: [(u64, u64, &str); 6] = [
            (1, 25, "Victor"),
            (5, 50, "Triumph"),
            (25, 100, "Conqueror"),
            (100, 200, "Overlord"),
            (500, 500, "Imperator"),
            (1000, 1000, "Sovereign"),
        ];

        for character in crate::geo_entities::all() {
            if let Some(hero) = &character.hero {
                let slug = character.id.as_str();
                let name = hero.name.as_str();

                // 1. Recruit Hero
                list.push(AchievementDefinition {
                    id: leak_str(format!("recruit_{slug}")),
                    category: "commanders",
                    title: leak_str(format!("Recruit {name}")),
                    description: leak_str(format!("Unlock or recruit {name} into your vanguard.")),
                    points: 25,
                    target: 1,
                });

                // 2. Mastery Battles (3 to 1,000)
                for (target, points, tier_title) in mastery_tiers {
                    let title = match (slug, target) {
                        ("boudica", 3) => "Wildfire",
                        ("boudica", 10) => "Fury",
                        ("boudica", 25) => "Iceni",
                        ("boudica", 50) => "Uprising",
                        ("boudica", 100) => "Vengeance",
                        ("boudica", 250) => "War Queen",
                        ("boudica", 500) => "Blood Mother",
                        ("boudica", 1000) => "Nemesis",
                        ("caesar", 3) => "Tribune",
                        ("caesar", 10) => "Centurion",
                        ("caesar", 25) => "Legatus",
                        ("caesar", 50) => "Rubicon",
                        ("caesar", 100) => "Imperium",
                        ("caesar", 250) => "Dictator",
                        ("caesar", 500) => "Colossus",
                        ("caesar", 1000) => "Veni Vidi Vici",
                        ("lady_six_sky", 3) => "Eclipse",
                        ("lady_six_sky", 10) => "Sun Priest",
                        ("lady_six_sky", 25) => "Starlight",
                        ("lady_six_sky", 50) => "Wikal",
                        ("lady_six_sky", 100) => "Cosmos",
                        ("lady_six_sky", 250) => "Serpent",
                        ("lady_six_sky", 500) => "Sky Empress",
                        ("lady_six_sky", 1000) => "Ascension",
                        ("alexander", 3) => "Phaistos",
                        ("alexander", 10) => "Phalanx",
                        ("alexander", 25) => "Gordian",
                        ("alexander", 50) => "Issus",
                        ("alexander", 100) => "Alexandria",
                        ("alexander", 250) => "Hegemon",
                        ("alexander", 500) => "Amun",
                        ("alexander", 1000) => "World's End",
                        ("genghis_khan", 3) => "Rider",
                        ("genghis_khan", 10) => "Steppe Wolf",
                        ("genghis_khan", 25) => "Karakorum",
                        ("genghis_khan", 50) => "Golden Horde",
                        ("genghis_khan", 100) => "Tengri",
                        ("genghis_khan", 250) => "Great Khan",
                        ("genghis_khan", 500) => "Earth Shaker",
                        ("genghis_khan", 1000) => "Scourge of God",
                        ("leonidas", 3) => "Spartiate",
                        ("leonidas", 10) => "Shield Wall",
                        ("leonidas", 25) => "Molon Labe",
                        ("leonidas", 50) => "Hot Gates",
                        ("leonidas", 100) => "Thermopylae",
                        ("leonidas", 250) => "Lion of Sparta",
                        ("leonidas", 500) => "Immortal Bane",
                        ("leonidas", 1000) => "Last Stand",
                        ("sun_tzu", 3) => "Strategist",
                        ("sun_tzu", 10) => "The Art of War",
                        ("sun_tzu", 25) => "Subtle Dragon",
                        ("sun_tzu", 50) => "Silent Wind",
                        ("sun_tzu", 100) => "Formless",
                        ("sun_tzu", 250) => "Grand Tactician",
                        ("sun_tzu", 500) => "Supreme Art",
                        ("sun_tzu", 1000) => "Master of War",
                        ("cleopatra", 3) => "Nile Lotus",
                        ("cleopatra", 10) => "Pharaoh's Grace",
                        ("cleopatra", 25) => "Alexandrine",
                        ("cleopatra", 50) => "Siren of Kings",
                        ("cleopatra", 100) => "Ptolemaic Throne",
                        ("cleopatra", 250) => "Queen of Kings",
                        ("cleopatra", 500) => "Isis Reborn",
                        ("cleopatra", 1000) => "Eternal Nile",
                        ("hannibal", 3) => "Barca",
                        ("hannibal", 10) => "Alpine March",
                        ("hannibal", 25) => "Cannae",
                        ("hannibal", 50) => "Double Envelopment",
                        ("hannibal", 100) => "Rome's Terror",
                        ("hannibal", 250) => "Carthaginian Lion",
                        ("hannibal", 500) => "Ad Portas",
                        ("hannibal", 1000) => "Thunderbolt",
                        ("napoleon", 3) => "Corsican",
                        ("napoleon", 10) => "Artillery Officer",
                        ("napoleon", 25) => "Austerlitz",
                        ("napoleon", 50) => "Sun of Austerlitz",
                        ("napoleon", 100) => "Grande Armée",
                        ("napoleon", 250) => "First Consul",
                        ("napoleon", 500) => "Emperor of France",
                        ("napoleon", 1000) => "Eagle of Destiny",
                        _ => leak_str(format!("{name} {tier_title}")),
                    };

                    list.push(AchievementDefinition {
                        id: leak_str(format!("{slug}_mastery_{target}")),
                        category: "commanders",
                        title,
                        description: leak_str(format!("Command {name} in {target} battles.")),
                        points,
                        target,
                    });
                }

                // 3. Hero Victories (1 to 1,000)
                for (target, points, tier_title) in win_tiers {
                    list.push(AchievementDefinition {
                        id: leak_str(format!("{slug}_wins_{target}")),
                        category: "victories",
                        title: leak_str(format!("{name} {tier_title}")),
                        description: leak_str(format!("Win {target} battles commanding {name}.")),
                        points,
                        target,
                    });
                }
            }
        }

        list
    })
}

/// XP required to advance from `level` to `level + 1`.
/// Starts at 100 XP (Lv 1 -> 2), reaches ~205 XP at Lv 10, and grows by ~120-140 XP every 10 levels.
pub fn xp_for_level(level: u32) -> u32 {
    if level <= 1 {
        return 100;
    }
    let raw = 100.0 + 11.0 * ((level - 1) as f64).powf(1.05);
    ((raw / 5.0).round() as u32) * 5
}

/// Cumulative XP required to reach `level` (level 1 requires 0 XP).
pub fn total_xp_for_level(level: u32) -> u32 {
    let mut total: u32 = 0;
    for l in 1..level {
        total = total.saturating_add(xp_for_level(l));
    }
    total
}

/// Player level for a given cumulative `xp`.
pub fn level_from_xp(xp: u32) -> u32 {
    let mut level: u32 = 1;
    let mut remaining = xp;
    loop {
        let req = xp_for_level(level);
        if remaining < req {
            return level;
        }
        remaining -= req;
        level = level.saturating_add(1);
    }
}

const XP_MATCH: u32 = 20;
const XP_WIN: u32 = 100;
const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;
const XP_PER_ASSIST: u32 = 5;

const CROWNS_PARTICIPATION: u64 = 10;
const CROWNS_WIN: u64 = 30;
const CROWNS_PER_KILL: u64 = 1;
const CROWNS_PER_EMPIRE: u64 = 2;
const CROWNS_PER_ASSIST: u64 = 1;
const CROWNS_TUTORIAL: u64 = 50;

pub fn calculate(input: RewardInput) -> MatchReward {
    if input.tutorial {
        return MatchReward {
            xp: 100,
            leader_xp: 100,
            crowns: CROWNS_TUTORIAL,
            laurels: 0,
            gems: 0,
        };
    }

    let mut xp = XP_MATCH;
    xp = xp.saturating_add(input.players_defeated.saturating_mul(XP_PER_PLAYER));
    xp = xp.saturating_add(input.empires_defeated.saturating_mul(XP_PER_EMPIRE));
    xp = xp.saturating_add(input.tribes_defeated.saturating_mul(XP_PER_TRIBE));
    xp = xp.saturating_add(input.assists.saturating_mul(XP_PER_ASSIST));
    if input.won {
        xp = xp.saturating_add(XP_WIN);
    }

    let mut crowns = CROWNS_PARTICIPATION;
    if input.won {
        crowns = crowns.saturating_add(CROWNS_WIN);
    }
    crowns = crowns.saturating_add((input.kills as u64).saturating_mul(CROWNS_PER_KILL));
    crowns =
        crowns.saturating_add((input.empires_defeated as u64).saturating_mul(CROWNS_PER_EMPIRE));
    crowns = crowns.saturating_add((input.assists as u64).saturating_mul(CROWNS_PER_ASSIST));

    MatchReward {
        xp,
        leader_xp: xp,
        crowns,
        // Laurels are achievement points, never match currency. They are
        // awarded only when the server unlocks an achievement.
        laurels: 0,
        gems: 0,
    }
}

// Gem bonus per verified match (owner decision 2026-11): a small amount for
// playing, more for a win, the most for the hardest win (public FFA). Teams,
// Co-op and Ranked 1v1 wins pay half of an FFA win.
// Gems are scarce on purpose: they pay about a third of the 2026-11 amounts.
const GEMS_PLAYED: u64 = 0;
const GEMS_PLAYED_WITH_DEFEATS: u64 = 0;
const GEMS_WIN_FFA: u64 = 3;
const GEMS_WIN_SIDE: u64 = 1;
/// A match shorter than this pays no participation gems unless it was won,
/// so joining and resigning cannot farm them.
pub const GEMS_MIN_PLAYED_SECONDS: u32 = 300;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GemInput<'a> {
    /// Match queue: "Matchmaking", "Custom" or "Ranked1v1".
    pub queue: &'a str,
    /// Game mode: "FFA", "Teams" or "Coop".
    pub mode: &'a str,
    pub won: bool,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub played_seconds: u32,
}

/// Gems for one verified participant. Custom lobbies pay none: their host
/// controls the roster, so a win there proves nothing.
pub fn match_gems(input: GemInput<'_>) -> u64 {
    let public = input.queue == "Matchmaking";
    if !public && input.queue != "Ranked1v1" {
        return 0;
    }
    let mut gems = 0;
    if input.won || input.played_seconds >= GEMS_MIN_PLAYED_SECONDS {
        gems = if input.players_defeated.saturating_add(input.empires_defeated) > 0 {
            GEMS_PLAYED_WITH_DEFEATS
        } else {
            GEMS_PLAYED
        };
    }
    if input.won {
        gems += if public && input.mode == "FFA" {
            GEMS_WIN_FFA
        } else {
            GEMS_WIN_SIDE
        };
    }
    gems
}

pub fn canonical_leader_name(value: &str) -> Option<String> {
    crate::commerce::leader_from_any_id(value).map(|leader| leader.name().to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        AchievementTotals, MatchGoalCounters, RewardInput, calculate, canonical_leader_name,
        newly_unlocked_achievements, participation_goal_advances, verified_goal_advances,
    };

    #[test]
    fn reward_math_is_deterministic_for_win_and_loss() {
        let loss = calculate(RewardInput {
            kills: 3,
            assists: 2,
            ..Default::default()
        });
        assert_eq!(loss.xp, 30);
        assert_eq!(loss.leader_xp, 30);
        assert_eq!(loss.crowns, 15);
        assert_eq!(loss.laurels, 0);

        let win = calculate(RewardInput {
            won: true,
            players_defeated: 2,
            empires_defeated: 1,
            assists: 1,
            ..Default::default()
        });
        assert_eq!(win.xp, 163);
        assert_eq!(win.leader_xp, 163);
        assert_eq!(win.crowns, 43);
        assert_eq!(win.laurels, 0);
    }

    #[test]
    fn match_goal_progress_uses_only_changed_indexed_chains() {
        let before = MatchGoalCounters {
            matches_played: 2,
            hero_matches: 2,
            ..Default::default()
        };
        let after = MatchGoalCounters {
            matches_played: 3,
            hero_matches: 3,
            ..Default::default()
        };
        let participation = participation_goal_advances(&before, &after, "Boudica");
        assert_eq!(participation.len(), 2);
        let battles = participation
            .iter()
            .find(|advance| advance.chain_id == "series:battles")
            .unwrap();
        assert_eq!(battles.achievement_id, "battles_3");
        assert_eq!(battles.progress_before, 2);
        assert_eq!(battles.progress_after, 3);
        assert_eq!(battles.previous_target, 1);
        assert_eq!(battles.target, 3);
        let mastery = participation
            .iter()
            .find(|advance| advance.chain_id == "hero_mastery:boudica")
            .unwrap();
        assert_eq!(mastery.achievement_id, "boudica_mastery_3");

        let before = MatchGoalCounters {
            kills: 9,
            assists: 9,
            players_defeated: 4,
            empires_defeated: 4,
            tribes_defeated: 4,
            ..Default::default()
        };
        let after = MatchGoalCounters {
            wins: 1,
            hero_wins: 1,
            kills: 10,
            assists: 10,
            players_defeated: 5,
            empires_defeated: 5,
            tribes_defeated: 5,
            ..Default::default()
        };
        let verified = verified_goal_advances(&before, &after, "Boudica");
        assert_eq!(verified.len(), 7);
        assert!(verified.iter().any(|advance| {
            advance.chain_id == "series:victories" && advance.achievement_id == "first_victory"
        }));
        assert!(verified.iter().any(|advance| {
            advance.chain_id == "hero_wins:boudica" && advance.achievement_id == "boudica_wins_1"
        }));
    }

    #[test]
    fn indexed_match_goal_records_chain_completion_and_stops_after_last_tier() {
        let first = MatchGoalCounters::default();
        let crossed_multiple = MatchGoalCounters {
            matches_played: 25,
            hero_matches: 25,
            ..Default::default()
        };
        let advances = participation_goal_advances(&first, &crossed_multiple, "Boudica");
        let battles = advances
            .iter()
            .find(|advance| advance.chain_id == "series:battles")
            .unwrap();
        assert_eq!(battles.achievement_id, "battles_25");
        assert_eq!(battles.progress_before, 0);
        assert_eq!(battles.progress_after, 25);
        assert_eq!(battles.previous_target, 10);

        let before = MatchGoalCounters {
            matches_played: 500,
            hero_matches: 500,
            ..Default::default()
        };
        let completed = MatchGoalCounters {
            matches_played: 1_000,
            hero_matches: 1_000,
            ..Default::default()
        };
        let advances = participation_goal_advances(&before, &completed, "Boudica");
        assert_eq!(advances.len(), 2);
        assert!(
            advances
                .iter()
                .all(|advance| { advance.target == 1_000 && advance.previous_target == 500 })
        );
        let beyond = MatchGoalCounters {
            matches_played: 1_001,
            hero_matches: 1_001,
            ..Default::default()
        };
        assert!(participation_goal_advances(&completed, &beyond, "Boudica").is_empty());
    }

    #[test]
    fn match_gems_pay_small_play_bonus_and_scale_wins_by_difficulty() {
        use super::{GEMS_MIN_PLAYED_SECONDS, GemInput, match_gems};
        let played = GemInput {
            queue: "Matchmaking",
            mode: "FFA",
            played_seconds: GEMS_MIN_PLAYED_SECONDS,
            ..Default::default()
        };
        assert_eq!(match_gems(played), 0);
        assert_eq!(match_gems(GemInput { players_defeated: 1, ..played }), 0);
        assert_eq!(match_gems(GemInput { won: true, ..played }), 3);
        assert_eq!(match_gems(GemInput { won: true, mode: "Teams", ..played }), 1);
        assert_eq!(
            match_gems(GemInput { won: true, mode: "Coop", ..played }),
            1
        );
        assert_eq!(match_gems(GemInput { won: true, queue: "Ranked1v1", ..played }), 1);
        // Join-and-resign and host-controlled lobbies pay nothing.
        assert_eq!(match_gems(GemInput { played_seconds: 60, ..played }), 0);
        assert_eq!(match_gems(GemInput { won: true, queue: "Custom", ..played }), 0);
    }

    #[test]
    fn tutorial_reward_is_fixed_and_leader_names_are_whitelisted() {
        assert_eq!(
            calculate(RewardInput {
                tutorial: true,
                ..Default::default()
            }),
            super::MatchReward {
                xp: 100,
                leader_xp: 100,
                crowns: 50,
                laurels: 0,
                gems: 0,
            }
        );
        assert_eq!(canonical_leader_name("Boudica").as_deref(), Some("Boudica"));
        assert!(canonical_leader_name("not-a-leader").is_none());
    }

    #[test]
    fn achievement_projection_is_shared_and_does_not_repeat_awards() {
        let totals = AchievementTotals {
            matches_played: 1,
            leader_matches_played: 1,
            ..Default::default()
        };
        let unlocked = newly_unlocked_achievements(&totals, &Default::default());
        assert_eq!(unlocked.iter().map(|item| item.points).sum::<u64>(), 10);
        let already_unlocked = ["first_command".to_string()].into_iter().collect();
        assert!(newly_unlocked_achievements(&totals, &already_unlocked).is_empty());
    }

    #[test]
    fn skin_milestones_unlock_from_owned_skins_and_pay_currency() {
        let totals = AchievementTotals {
            skins_owned: 1,
            ..Default::default()
        };
        let unlocked = newly_unlocked_achievements(&totals, &Default::default());
        assert!(unlocked.iter().any(|item| item.id == "skin_1"));
        assert_eq!(super::achievement_rewards("skin_1"), None);

        let totals = AchievementTotals {
            skins_owned: 5,
            ..Default::default()
        };
        let unlocked = newly_unlocked_achievements(&totals, &Default::default());
        assert!(unlocked.iter().any(|item| item.id == "skin_5"));
        assert_eq!(
            super::achievement_rewards("skin_5").map(|reward| reward.gems),
            Some(25)
        );

        let totals = AchievementTotals {
            skins_owned: 9,
            ..Default::default()
        };
        let unlocked = newly_unlocked_achievements(&totals, &Default::default());
        assert!(unlocked.iter().any(|item| item.id == "skin_9"));
        assert_eq!(
            super::achievement_rewards("skin_9").map(|reward| reward.gems),
            Some(50)
        );
    }

    #[test]
    fn duplicate_hero_reward_is_a_fraction_of_the_hero_price() {
        let leader = crate::leaders::Leader::all()
            .find(|leader| leader.price_gems() > 0)
            .expect("the catalog has a paid hero");
        let gems = super::achievement_duplicate_gems(leader);
        assert_eq!(gems, leader.price_gems() * 9 / 40);
        assert!(gems < leader.price_gems());
    }

    #[test]
    fn progressive_level_curve_matches_milestones() {
        use super::{level_from_xp, total_xp_for_level, xp_for_level};
        assert_eq!(xp_for_level(1), 100);
        assert_eq!(xp_for_level(2), 110);
        assert_eq!(xp_for_level(10), 210);
        assert_eq!(xp_for_level(20), 340);
        assert_eq!(xp_for_level(50), 755);
        assert_eq!(xp_for_level(100), 1470);

        assert_eq!(total_xp_for_level(1), 0);
        assert_eq!(total_xp_for_level(2), 100);
        assert_eq!(total_xp_for_level(3), 210);
        assert_eq!(total_xp_for_level(5), 470);

        assert_eq!(level_from_xp(0), 1);
        assert_eq!(level_from_xp(99), 1);
        assert_eq!(level_from_xp(100), 2);
        assert_eq!(level_from_xp(209), 2);
        assert_eq!(level_from_xp(210), 3);
        assert_eq!(level_from_xp(470), 5);
    }
}
