//! Alliance request lifecycle, cooldowns, and betrayal heuristics.

use crate::player::{Player, PlayerId, PlayerType};
use crate::rng::NextIntExt;
use wyrand::WyRand;

/// Pending request lifetime (~20s at 100ms/tick).
pub const ALLIANCE_REQUEST_TTL_TICKS: u32 = 200;
/// Minimum delay before the same pair can get another outgoing request (~30s).
pub const ALLIANCE_REQUEST_COOLDOWN_TICKS: u32 = 300;
/// Minimum delay before the same pair can send another resource request (~30s at 100ms/tick).
pub const RESOURCE_REQUEST_COOLDOWN_TICKS: u32 = 300;
/// Maximum concurrent pending AI-originated help/resource asks targeting one human player.
pub const AI_MAX_PENDING_REQUESTS_PER_HUMAN: usize = 3;
/// Percentage chance an AI in an extreme emergency actually asks allies instead of staying silent.
pub const AI_EMERGENCY_HELP_ASK_CHANCE_PCT: i32 = 30;
/// A resource request or trade offer left unanswered this long is withdrawn (~30s).
pub const RESOURCE_REQUEST_TTL_TICKS: u32 = 300;
/// Lifetime of an emote-triggered Help Call (~30s at 100ms/tick).
pub const HELP_CALL_TTL_TICKS: u32 = 300;
/// Minimum delay before the same player can trigger another Help Call (~30s at 100ms/tick).
pub const HELP_CALL_COOLDOWN_TICKS: u32 = 300;
/// Minimum human-like thinking delay for allied Ghosts (~2.0s at 100ms/tick).
pub const GHOST_HELP_DELAY_MIN_TICKS: u32 = 20;
/// Maximum human-like thinking delay for allied Ghosts (~6.0s at 100ms/tick).
pub const GHOST_HELP_DELAY_MAX_TICKS: u32 = 60;
/// Last ~30s of an alliance: renewal proposals may be attempted (throttled by RNG).
pub const ALLIANCE_RENEWAL_WINDOW_TICKS: u32 = 300;
/// Minimum time between betrayals for one bot (~60s).
pub const BETRAYAL_COOLDOWN_TICKS: u32 = 600;
/// Traitor stigma duration for accept logic (~30s).
pub const TRAITOR_STATUS_TICKS: u32 = 300;
/// Bot retaliation window after breaking an alliance (~10s).
pub const BOT_BETRAYAL_RETALIATION_TICKS: u32 = 100;
/// Human retaliation window after breaking an alliance (~30s).
pub const HUMAN_BETRAYAL_RETALIATION_TICKS: u32 = 300;
/// Fresh alliances are protected from betrayal (~60s).
pub const ALLY_GRACE_AFTER_FORM_TICKS: u32 = 600;
/// Alliance duration on accept (must match intent handlers).
pub const ALLIANCE_DURATION_TICKS: u32 = 2400;

/// Campaign alliances have no timer and are permanent; only timed alliances renew.
#[inline]
pub fn alliance_can_renew(is_allied: bool, remaining_ticks: Option<u32>) -> bool {
    is_allied && remaining_ticks.is_some_and(|ticks| ticks <= ALLIANCE_RENEWAL_WINDOW_TICKS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AllianceProposal {
    pub proposer: PlayerId,
    pub target: PlayerId,
    pub created_tick: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllianceActionState {
    Request,
    Accept,
    Pending,
    Active,
    Renew,
}

impl AllianceActionState {
    pub const fn resolve(
        is_allied: bool,
        in_renewal_window: bool,
        has_incoming_request: bool,
        has_outgoing_request: bool,
    ) -> Self {
        if has_incoming_request {
            Self::Accept
        } else if has_outgoing_request {
            Self::Pending
        } else if is_allied && in_renewal_window {
            Self::Renew
        } else if is_allied {
            Self::Active
        } else {
            Self::Request
        }
    }

    pub const fn can_act(self) -> bool {
        matches!(self, Self::Request | Self::Accept | Self::Renew)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Accept => "accept",
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Renew => "renew",
        }
    }
}

#[inline]
pub fn can_retaliate_against(player: &Player) -> bool {
    player.betrayal_retaliation_ticks > 0
}

/// Chaotic diplomats: well below 1% of typical bot id space.
#[inline]
pub fn is_chaotic_diplomat(bot_id: PlayerId) -> bool {
    bot_id.is_multiple_of(512)
}

#[inline]
pub fn is_traitor_active(player: &Player, tick: u32) -> bool {
    player.traitor && player.traitor_tick > tick
}

#[inline]
pub fn alliance_is_young(player: &Player, ally_id: PlayerId) -> bool {
    let timer = player.alliance_timers.get(&ally_id).copied().unwrap_or(0);
    timer > ALLIANCE_DURATION_TICKS.saturating_sub(ALLY_GRACE_AFTER_FORM_TICKS)
}

/// Nations should not spam standard tribes; chaotic bots may rarely try.
pub fn is_valid_alliance_target(
    proposer_id: PlayerId,
    _neighbor_id: PlayerId,
    neighbor_type: PlayerType,
) -> bool {
    match neighbor_type {
        PlayerType::Human | PlayerType::Nation => true,
        // Tribes have one explicit low-IQ tier now; only an explicitly
        // chaotic proposer may form this otherwise unusual alliance.
        PlayerType::Bot => is_chaotic_diplomat(proposer_id),
    }
}

/// Proposal RNG ceiling (0–100 roll must be `<` this value).
pub fn alliance_propose_roll_cap(bot_id: PlayerId, bot_iq: u32, renewing: bool) -> i32 {
    if renewing {
        return 25;
    }
    if is_chaotic_diplomat(bot_id) {
        return 8;
    }
    if bot_iq >= 130 {
        return 4;
    }
    if bot_iq >= 100 {
        return 3;
    }
    2
}

/// ~90% reject traitors when evaluating inbound requests.
pub fn should_reject_traitor_request(requestor: &Player, tick: u32, roll_0_100: i32) -> bool {
    is_traitor_active(requestor, tick) && roll_0_100 >= 10
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GhostHelpOutcome {
    Help,
    Ignore,
    Decline,
}

#[inline]
pub fn ghost_think_delay_ticks(
    seed: u64,
    ghost_id: PlayerId,
    caller_id: PlayerId,
    created_tick: u32,
    salt: u64,
) -> u32 {
    let mut rng = WyRand::new(
        seed ^ u64::from(ghost_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(caller_id).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ u64::from(created_tick).wrapping_mul(0xA076_1D64_78BD_642F)
            ^ salt,
    );
    rng.next_int(
        GHOST_HELP_DELAY_MIN_TICKS as i32,
        GHOST_HELP_DELAY_MAX_TICKS as i32 + 1,
    ) as u32
}

/// Computes contextual emergency aid `(gold, troops)` from `helper` to `caller`,
/// prioritizing troops when `caller` is under attack or low while keeping reserves.
pub fn compute_help_aid(helper: &Player, caller: &Player, caller_under_attack: bool) -> (f64, f64) {
    let caller_room = (caller.max_troops - caller.troops).max(0.0);
    let helper_spare_troops = (helper.troops - 1.0).max(0.0);
    let troop_reserve = (helper.max_troops * 0.20).max(1.0);
    let surplus_troops = (helper.troops - troop_reserve)
        .max(0.0)
        .min(helper_spare_troops)
        .min(caller_room);
    let gold_reserve = 2_000.0;
    let surplus_gold = (helper.gold - gold_reserve).max(0.0);

    let caller_troop_ratio = if caller.max_troops > 0.0 {
        caller.troops / caller.max_troops
    } else {
        1.0
    };
    let caller_needs_troops =
        caller_under_attack || caller_troop_ratio < 0.35 || caller.troops <= 500.0;
    let caller_needs_gold = caller.gold < 30_000.0 || !caller_needs_troops || caller_room < 10.0;

    let (mut gold, mut troops) = (0.0, 0.0);
    if caller_needs_troops && surplus_troops >= 1.0 {
        troops = (surplus_troops * 0.25)
            .floor()
            .min(caller_room)
            .max(1.0)
            .min(surplus_troops);
        if caller_needs_gold && surplus_gold >= 100.0 {
            gold = (surplus_gold * 0.15).floor();
        }
    } else {
        if surplus_gold >= 100.0 {
            gold = (surplus_gold * 0.20).floor();
        }
        if caller_room >= 50.0 && surplus_troops >= 50.0 && caller_troop_ratio < 0.70 {
            troops = (surplus_troops * 0.15).floor();
        }
    }

    if gold <= 0.0 && troops <= 0.0 {
        let fallback_gold = (helper.gold.max(0.0) * 0.10).floor();
        let fallback_troops = (helper_spare_troops.min(caller_room) * 0.10).floor();
        return (fallback_gold, fallback_troops);
    }
    (gold, troops)
}

/// Immediate deterministic roll (`true` = send aid, `false` = deny immediately) for allied Bots.
#[allow(clippy::too_many_arguments)]
pub fn roll_bot_help_decision(
    seed: u64,
    bot_id: PlayerId,
    caller_id: PlayerId,
    call_id: u64,
    helper: &Player,
    caller: &Player,
    helper_under_attack: bool,
    caller_under_attack: bool,
    tick: u32,
) -> bool {
    let mut rng = WyRand::new(
        seed ^ u64::from(bot_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(caller_id).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ call_id.wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ 0x424F_5448_454C_5001,
    );
    let roll = rng.next_int(0, 100);
    if should_reject_traitor_request(caller, tick, roll) {
        return false;
    }
    let (aid_g, aid_t) = compute_help_aid(helper, caller, caller_under_attack);
    if aid_g <= 0.0 && aid_t <= 0.0 {
        return false;
    }
    let helper_ratio = if helper.max_troops > 0.0 {
        helper.troops / helper.max_troops
    } else {
        0.0
    };
    let mut chance: i32 = if helper_under_attack || helper_ratio < 0.25 {
        20
    } else if helper_ratio >= 0.45 || helper.gold >= 80_000.0 {
        70
    } else {
        45
    };
    if caller_under_attack {
        chance += 15;
    }
    roll < chance.clamp(5, 95)
}

/// Delayed human-like roll for allied Ghosts (`Help`, silent `Ignore`, or `Decline`).
#[allow(clippy::too_many_arguments)]
pub fn roll_ghost_help_outcome(
    seed: u64,
    ghost_id: PlayerId,
    caller_id: PlayerId,
    call_id: u64,
    helper: &Player,
    caller: &Player,
    helper_under_attack: bool,
    caller_under_attack: bool,
    tick: u32,
) -> GhostHelpOutcome {
    let mut rng = WyRand::new(
        seed ^ u64::from(ghost_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(caller_id).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ call_id.wrapping_mul(0x94D0_49BB_1331_11EB)
            ^ 0x4748_4F53_5448_454C,
    );
    let roll = rng.next_int(0, 100);
    if should_reject_traitor_request(caller, tick, roll) {
        return if roll < 70 {
            GhostHelpOutcome::Ignore
        } else {
            GhostHelpOutcome::Decline
        };
    }
    let (aid_g, aid_t) = compute_help_aid(helper, caller, caller_under_attack);
    if aid_g <= 0.0 && aid_t <= 0.0 {
        return if roll < 85 {
            GhostHelpOutcome::Ignore
        } else {
            GhostHelpOutcome::Decline
        };
    }
    let helper_ratio = if helper.max_troops > 0.0 {
        helper.troops / helper.max_troops
    } else {
        0.0
    };
    let (help_cap, ignore_cap) = if helper_under_attack || helper_ratio < 0.25 {
        (15, 80)
    } else if helper_ratio >= 0.45 || helper.gold >= 80_000.0 {
        let help = if caller_under_attack { 80 } else { 70 };
        (help, 95)
    } else {
        let help = if caller_under_attack { 55 } else { 45 };
        (help, 90)
    };
    if roll < help_cap {
        GhostHelpOutcome::Help
    } else if roll < ignore_cap {
        GhostHelpOutcome::Ignore
    } else {
        GhostHelpOutcome::Decline
    }
}

/// Delayed human-like evaluation for Ghosts answering explicit `RequestResources` / `OfferTrade`.
pub fn roll_ghost_resource_request_outcome(
    seed: u64,
    ghost_id: PlayerId,
    requester_id: PlayerId,
    created_tick: u32,
    helper: &Player,
    requester: &Player,
    requested_troops: f64,
    helper_under_attack: bool,
) -> GhostHelpOutcome {
    let mut rng = WyRand::new(
        seed ^ u64::from(ghost_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(requester_id).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ u64::from(created_tick).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ 0x4748_5245_5155_4553,
    );
    let roll = rng.next_int(0, 100);
    if should_reject_traitor_request(requester, created_tick, roll) {
        return if roll < 70 {
            GhostHelpOutcome::Ignore
        } else {
            GhostHelpOutcome::Decline
        };
    }
    let helper_ratio = if helper.max_troops > 0.0 {
        helper.troops / helper.max_troops
    } else {
        0.0
    };
    let (help_cap, ignore_cap) = if helper_under_attack || helper_ratio < 0.20 {
        (10, 75)
    } else if helper.troops >= requested_troops * 2.0 {
        (80, 95)
    } else if helper_ratio >= 0.40 || helper.gold >= 80_000.0 {
        (45, 85)
    } else {
        (20, 75)
    };
    if roll < help_cap {
        GhostHelpOutcome::Help
    } else if roll < ignore_cap {
        GhostHelpOutcome::Ignore
    } else {
        GhostHelpOutcome::Decline
    }
}

/// Probabilistic restraint when an AI faces an extreme emergency:
/// most crises stay silent ("better not bother") rather than flooding allies.
pub fn roll_ai_emergency_help_ask(seed: u64, ai_id: PlayerId, tick: u32) -> bool {
    let mut rng = WyRand::new(
        seed ^ u64::from(ai_id).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(tick).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ 0x4149_4153_4B5F_484C,
    );
    rng.next_int(0, 100) < AI_EMERGENCY_HELP_ASK_CHANCE_PCT
}

/// Betrayal only when attack logic would target an ally — not on a diplomacy timer.
pub fn maybe_betray_for_attack(
    bot: &Player,
    ally: &Player,
    bordering_player_count: usize,
    current_tick: u32,
    betray_cooldown_until: Option<u32>,
    rng: &mut WyRand,
) -> bool {
    if betray_cooldown_until.is_some_and(|until| current_tick < until) {
        return false;
    }
    if !ally.alive || ally.disconnected {
        return false;
    }
    if bot.team.is_some() && bot.team == ally.team {
        return false;
    }
    if alliance_is_young(bot, ally.id) {
        return false;
    }

    let me_troops = bot.troops.max(1.0);
    let ally_troops = ally.troops.max(1.0);
    let ally_max = ally.max_troops.max(1.0);

    // Critically weak (e.g. post-nuke), and we are stronger.
    if ally.troops < ally_max * 0.2 && me_troops > ally_troops {
        return true;
    }

    // Punish known traitors that are not much stronger.
    if is_traitor_active(ally, current_tick) && ally_troops < me_troops * 1.2 {
        return true;
    }

    // Only bordering polity and we dominate (3×).
    if bordering_player_count == 1 && ally_troops * 3.0 < me_troops {
        return true;
    }

    // Rare extreme opportunism (not the default 2× rule).
    if bot.iq >= 130 && me_troops >= ally_troops * 10.0 && rng.next_int(0, 100) < 2 {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_config::GameConfig;

    fn sample_player(id: u16, iq: u32) -> Player {
        let mut p = Player::new_bot(
            id,
            format!("P{id}"),
            [1.0, 0.0, 0.0],
            &GameConfig::default(),
        );
        p.iq = iq;
        p
    }

    #[test]
    fn young_alliance_blocks_betrayal() {
        let mut bot = sample_player(1, 140);
        bot.alliances.push(2);
        bot.alliance_timers.insert(2, ALLIANCE_DURATION_TICKS);
        let ally = sample_player(2, 80);
        let mut rng = WyRand::new(1);
        assert!(!maybe_betray_for_attack(
            &bot, &ally, 1, 100, None, &mut rng,
        ));
    }

    #[test]
    fn only_border_three_x_betrays() {
        let mut bot = sample_player(1, 140);
        bot.troops = 3000.0;
        bot.alliances.push(2);
        bot.alliance_timers.insert(2, 100);
        let mut ally = sample_player(2, 80);
        ally.troops = 500.0;
        let mut rng = WyRand::new(1);
        assert!(maybe_betray_for_attack(
            &bot, &ally, 1, 5000, None, &mut rng,
        ));
    }

    #[test]
    fn standard_tribe_not_valid_target_for_nation() {
        assert!(!is_valid_alliance_target(8, 15, PlayerType::Bot,));
        assert!(is_valid_alliance_target(512, 100, PlayerType::Bot,));
    }

    #[test]
    fn only_timed_alliances_can_renew() {
        assert!(!alliance_can_renew(true, None));
        assert!(alliance_can_renew(
            true,
            Some(ALLIANCE_RENEWAL_WINDOW_TICKS)
        ));
        assert!(!alliance_can_renew(
            true,
            Some(ALLIANCE_RENEWAL_WINDOW_TICKS + 1)
        ));
        assert!(!alliance_can_renew(false, Some(0)));
    }

    #[test]
    fn traitor_requests_mostly_rejected() {
        let mut requestor = sample_player(2, 100);
        requestor.traitor = true;
        requestor.traitor_tick = 10_000;
        assert!(should_reject_traitor_request(&requestor, 100, 50));
        assert!(!should_reject_traitor_request(&requestor, 100, 5));
    }
}
