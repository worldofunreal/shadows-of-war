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
    pub owned_leaders_count: u64,
    pub owned_leaders: std::collections::BTreeSet<String>,
    pub leader_matches: std::collections::BTreeMap<String, u64>,
    pub leader_wins_map: std::collections::BTreeMap<String, u64>,
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

    // 7. Hero recruitments: recruit_{slug}
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

    // 8. Hero mastery battles: {slug}_mastery_{target}
    if let Some(idx) = id.find("_mastery_") {
        let slug = &id[..idx];
        return get_hero_matches(&totals.leader_matches, slug);
    }

    // 9. Hero victories: {slug}_wins_{target}
    if let Some(idx) = id.find("_wins_") {
        let slug = &id[..idx];
        return get_hero_wins(&totals.leader_wins_map, slug);
    }

    0
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

/// Canonical NFTROPOLY/WOU-ID card token for an unlocked achievement. Same
/// snake_case -> kebab derivation as skins, with a `sow-ach-` prefix.
pub fn achievement_card_token_id(achievement_id: &str) -> String {
    format!("sow-ach-{}", achievement_id.replace('_', "-"))
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

pub fn achievements() -> &'static [AchievementDefinition] {
    all_achievements()
}

pub const ACHIEVEMENTS: &[AchievementDefinition] = &[];

const XP_MATCH: u32 = 20;
const XP_WIN: u32 = 100;
const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;
const XP_PER_ASSIST: u32 = 5;

const CROWNS_PARTICIPATION: u64 = 40;
const CROWNS_WIN: u64 = 100;
const CROWNS_PER_KILL: u64 = 2;
const CROWNS_PER_EMPIRE: u64 = 5;
const CROWNS_PER_ASSIST: u64 = 2;
const CROWNS_TUTORIAL: u64 = 100;

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
// HvN and Ranked 1v1 wins pay half of an FFA win.
const GEMS_PLAYED: u64 = 3;
const GEMS_PLAYED_WITH_DEFEATS: u64 = 4;
const GEMS_WIN_FFA: u64 = 50;
const GEMS_WIN_SIDE: u64 = 25;
/// A match shorter than this pays no participation gems unless it was won,
/// so joining and resigning cannot farm them.
pub const GEMS_MIN_PLAYED_SECONDS: u32 = 300;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GemInput<'a> {
    /// Match queue: "Matchmaking", "Custom" or "Ranked1v1".
    pub queue: &'a str,
    /// Game mode: "FFA", "Teams" or "HumansVsNations".
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
    crate::commerce::leader_from_id(value).map(|leader| leader.name().to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        AchievementTotals, RewardInput, calculate, canonical_leader_name,
        newly_unlocked_achievements,
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
        assert_eq!(loss.crowns, 50);
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
        assert_eq!(win.crowns, 147);
        assert_eq!(win.laurels, 0);
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
        assert_eq!(match_gems(played), 3);
        assert_eq!(match_gems(GemInput { players_defeated: 1, ..played }), 4);
        assert_eq!(match_gems(GemInput { won: true, ..played }), 53);
        assert_eq!(match_gems(GemInput { won: true, mode: "Teams", ..played }), 28);
        assert_eq!(
            match_gems(GemInput { won: true, mode: "HumansVsNations", ..played }),
            28
        );
        assert_eq!(match_gems(GemInput { won: true, queue: "Ranked1v1", ..played }), 28);
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
                crowns: 100,
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
}
