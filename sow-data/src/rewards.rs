//! Shared, deterministic rewards math used by the client preview and database.
//!
//! Two earned currencies, by product decision (owner, 2026-09):
//! - **Crowns** — the free spendable currency (leader unlocks, store).
//! - **Laurels** — achievement points. Earned, never spent; the Google Play
//!   "Laurel Hoard" achievement tracks them.
//! Gems are the premium currency and are never produced here.

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AchievementDefinition {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub points: u64,
    pub target: u64,
}

pub const ACHIEVEMENTS: &[AchievementDefinition] = &[
    AchievementDefinition {
        id: "first_command",
        title: "First Command",
        description: "Complete your first match.",
        points: 10,
        target: 1,
    },
    AchievementDefinition {
        id: "first_victory",
        title: "First Victory",
        description: "Win an authoritative match.",
        points: 25,
        target: 1,
    },
    AchievementDefinition {
        id: "battle_hardened",
        title: "Battle Hardened",
        description: "Complete 10 matches.",
        points: 25,
        target: 10,
    },
    AchievementDefinition {
        id: "victory_march",
        title: "Victory March",
        description: "Win 10 authoritative matches.",
        points: 50,
        target: 10,
    },
    AchievementDefinition {
        id: "laurel_hoard",
        title: "Laurel Hoard",
        description: "Earn 100 achievement points.",
        points: 50,
        target: 100,
    },
    AchievementDefinition {
        id: "commander_victorious",
        title: "Commander Victorious",
        description: "Win with a commander.",
        points: 25,
        target: 1,
    },
    AchievementDefinition {
        id: "veteran_commander",
        title: "Veteran Commander",
        description: "Complete 10 matches with commanders.",
        points: 50,
        target: 10,
    },
    AchievementDefinition {
        id: "banner_collector",
        title: "Banner Collector",
        description: "Command three different leaders.",
        points: 50,
        target: 3,
    },
    AchievementDefinition {
        id: "leader_path",
        title: "Leader Path",
        description: "Earn 1,000 leader experience.",
        points: 100,
        target: 1_000,
    },
];

const XP_MATCH: u32 = 20;
const XP_WIN: u32 = 100;
const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;
const XP_PER_ASSIST: u32 = 5;

const CROWNS_PARTICIPATION: u64 = 25;
const CROWNS_WIN: u64 = 75;
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
    }
}

pub fn canonical_leader_name(value: &str) -> Option<String> {
    crate::commerce::leader_from_id(value).map(|leader| leader.name().to_string())
}

#[cfg(test)]
mod tests {
    use super::{RewardInput, calculate, canonical_leader_name};

    #[test]
    fn reward_math_is_deterministic_for_win_and_loss() {
        let loss = calculate(RewardInput {
            kills: 3,
            assists: 2,
            ..Default::default()
        });
        assert_eq!(loss.xp, 30);
        assert_eq!(loss.leader_xp, 30);
        assert_eq!(loss.crowns, 35);
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
        assert_eq!(win.crowns, 107);
        assert_eq!(win.laurels, 0);
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
            }
        );
        assert_eq!(canonical_leader_name("Boudica").as_deref(), Some("Boudica"));
        assert!(canonical_leader_name("not-a-leader").is_none());
    }
}
