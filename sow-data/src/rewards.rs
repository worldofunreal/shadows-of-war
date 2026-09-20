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

const LAURELS_PARTICIPATION: u64 = 5;
const LAURELS_WIN: u64 = 15;
const LAURELS_PER_KILL: u64 = 1;
const LAURELS_PER_EMPIRE: u64 = 2;
const LAURELS_TUTORIAL: u64 = 100;

pub fn calculate(input: RewardInput) -> MatchReward {
    if input.tutorial {
        return MatchReward {
            xp: 100,
            leader_xp: 100,
            crowns: CROWNS_TUTORIAL,
            laurels: LAURELS_TUTORIAL,
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

    let mut laurels = LAURELS_PARTICIPATION;
    if input.won {
        laurels = laurels.saturating_add(LAURELS_WIN);
    }
    laurels = laurels.saturating_add((input.kills as u64).saturating_mul(LAURELS_PER_KILL));
    laurels =
        laurels.saturating_add((input.empires_defeated as u64).saturating_mul(LAURELS_PER_EMPIRE));

    MatchReward {
        xp,
        leader_xp: xp,
        crowns,
        laurels,
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
        assert_eq!(loss.laurels, 8);

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
        assert_eq!(win.laurels, 22);
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
                laurels: 100,
            }
        );
        assert_eq!(canonical_leader_name("Boudica").as_deref(), Some("Boudica"));
        assert!(canonical_leader_name("not-a-leader").is_none());
    }
}
