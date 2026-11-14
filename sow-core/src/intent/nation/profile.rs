use crate::game_config::{AiTuning, BotDifficulty};
use crate::player::PlayerType;
use crate::protocol::GameplayIntent;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum BotDecisionKind {
    Build = 0,
    Attack = 1,
}

#[derive(Clone, Debug)]
pub(super) struct BotDecision {
    pub(super) bot_id: u16,
    pub(super) kind: BotDecisionKind,
    pub(super) intent: GameplayIntent,
}

// ──────────────────────────────────────────────────────────────────────────
// AI TIER — the single source of truth for "what is this entity".
//
// Before this refactor, personality was keyed on `bot_id % N` arithmetic
// left over from when `Bot` was the only AI type. That produced accidental
// élites (e.g. tribe id 400 → IQ 130-181, élite profile, no income handicap)
// and accidental fools ( ghosts with small ids → bottom-tier profile), which
// inverted the entire food chain (tribes eating nations/ghosts).
//
// Tier is now derived from `(player_type, is_ai_controlled)` — the real
// discriminators. bot_id survives only as RNG seed for *intra-tier*
// variety (cadence jitter, profile sub-pick), never as a tier switch.
//
// Food chain (per design):
//   Ghost/Human = TOP    (the goats; pro teammate / the player)
//   Nation      = NEXT   (mid-game food)
//   Tribe       = LOWEST (free-land first; reluctant AI-player initiation on Vanilla)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiTier {
    /// `Human` + `is_ai_controlled` — top-IQ pro teammate ("ghost").
    Ghost,
    /// `Nation` — smart, mid-game challengers.
    Nation,
    /// `Bot` aka tribes — lowest IQ, free-land growth and opportunistic AI wars.
    Tribe,
}

/// Resolve the AI tier for an entity. Returns `None` for real humans (no AI brain).
///
/// This is the ONE function that says what kind of AI something is. Every
// other site (cadence, combat profile, income handicap, send-cost, structure
// targets) must read from this — never re-derive from `bot_id` again.
pub fn ai_tier(player_type: PlayerType, is_ai_controlled: bool) -> Option<AiTier> {
    match (player_type, is_ai_controlled) {
        (PlayerType::Human, false) => None, // real human, no AI
        (PlayerType::Human, true) => Some(AiTier::Ghost),
        (PlayerType::Nation, _) => Some(AiTier::Nation),
        (PlayerType::Bot, _) => Some(AiTier::Tribe),
    }
}

/// Combat/expand posture for an AI entity. One lookup by tier — no bot_id arithmetic.
#[derive(Clone, Copy, Debug)]
pub(super) struct BotAiProfile {
    pub(super) trigger_ratio: f64,
    pub(super) reserve_ratio: f64,
    pub(super) expand_ratio: f64,
    pub(super) refuse_human_chance: i32,
    /// Whether this AI can initiate attacks against player-owned territory.
    pub(super) attacks_players: bool,
}

/// Posture for a tier from the match's tuning. Human targets still pass the
/// shared grace/refusal rules; Vanilla tribes may also fight other AI tiers.
pub(super) fn ai_profile_for(
    ai: &AiTuning,
    tier: AiTier,
    difficulty: BotDifficulty,
) -> BotAiProfile {
    let (posture, attacks_players) = match (tier, difficulty) {
        (AiTier::Ghost, _) => (ai.ghost, true),
        (AiTier::Nation, _) => (ai.nation, true),
        (AiTier::Tribe, BotDifficulty::Vanilla) => (ai.tribe_vanilla, true),
        (AiTier::Tribe, BotDifficulty::Terminator) => (ai.tribe_terminator, true),
    };
    BotAiProfile {
        trigger_ratio: posture.trigger,
        reserve_ratio: posture.reserve,
        expand_ratio: posture.expand,
        refuse_human_chance: posture.refuse_human_chance,
        attacks_players,
    }
}

/// Ranked opponents use the same AI machinery with a fixed, server-assigned skill.
pub(super) fn ranked_ghost_profile(iq: u32) -> BotAiProfile {
    let skill = (f64::from(iq.clamp(60, 180)) - 60.0) / 120.0;
    BotAiProfile {
        trigger_ratio: 0.50 - 0.45 * skill,
        reserve_ratio: 0.35 - 0.33 * skill,
        expand_ratio: 0.15 - 0.13 * skill,
        refuse_human_chance: 0,
        attacks_players: true,
    }
}

/// `Ghost` (human-like top tier), `Nation` (Bot Hero), and `Tribe` (standard bot).
/// Adjusting one field here lets Bot Heroes and Ghosts diverge cleanly without
/// affecting Tribes or leaking side effects across shared IQ gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AiBehaviorPolicy {
    /// Use delayed, proportional counterattacks when attacked instead of synchronous largest-attack defense.
    pub(super) uses_retaliation_ai: bool,
    /// Use Ghost probabilistic alliance acceptance and proposals instead of hard 0.8x/0.5x ratios.
    pub(super) uses_probabilistic_diplomacy: bool,
    /// Ghosts may opportunistically strike a neighboring Tribe below `trigger_ratio` (1-in-3 roll).
    pub(super) uses_tribe_bypass: bool,
    /// May betray bordering human allies under `maybe_betray_for_attack` (unless campaign-pinned).
    pub(super) betrays_human_allies: bool,
    /// Prioritize City/Factory/Farm construction before Towers while economy targets are unmet.
    pub(super) prioritizes_economy_buildings: bool,
    /// Use regional `nation_bot_attack_order` + Terminator `walled_in_bot_targets` probe before Ghost attack loop.
    pub(super) uses_regional_nation_planning: bool,
    /// Launch nuclear strikes (`maybe_launch_nuke`, still gated by `bot_iq >= 100`).
    pub(super) launches_nukes: bool,
    /// Hold half of standard `reserve_ratio` when actively defending territory.
    pub(super) desperate_defense_half_reserve: bool,
}

impl AiBehaviorPolicy {
    pub(super) const fn for_tier(tier: AiTier) -> Self {
        match tier {
            AiTier::Ghost => Self {
                uses_retaliation_ai: true,
                uses_probabilistic_diplomacy: true,
                uses_tribe_bypass: true,
                betrays_human_allies: true,
                prioritizes_economy_buildings: true,
                uses_regional_nation_planning: false,
                launches_nukes: true,
                desperate_defense_half_reserve: true,
            },
            AiTier::Nation => Self {
                uses_retaliation_ai: true,
                uses_probabilistic_diplomacy: true,
                uses_tribe_bypass: false,
                betrays_human_allies: false,
                prioritizes_economy_buildings: true,
                uses_regional_nation_planning: false,
                launches_nukes: true,
                desperate_defense_half_reserve: true,
            },
            AiTier::Tribe => Self {
                uses_retaliation_ai: false,
                uses_probabilistic_diplomacy: false,
                uses_tribe_bypass: false,
                betrays_human_allies: false,
                prioritizes_economy_buildings: false,
                uses_regional_nation_planning: false,
                launches_nukes: false,
                desperate_defense_half_reserve: false,
            },
        }
    }
}

/// Describes which behaviours an AI entity should run this tick.
pub(super) struct AiSlot {
    pub(super) bot_id: u16,
    pub(super) tier: AiTier,
    pub(super) do_attack: bool,
    pub(super) do_structures: bool,
    pub(super) is_under_attack: bool,
    pub(super) ghost_retaliation: Option<GhostRetaliation>,
    pub(super) profile: BotAiProfile,
}

impl AiSlot {
    #[inline]
    pub(super) const fn policy(&self) -> AiBehaviorPolicy {
        AiBehaviorPolicy::for_tier(self.tier)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct GhostRetaliation {
    pub(super) attack_id: u64,
    pub(super) attacker_id: u16,
    pub(super) incoming_troops: f64,
    pub(super) ready_tick: u64,
}

impl GhostRetaliation {
    pub(super) fn is_ready_at(self, tick: u64) -> bool {
        tick >= self.ready_tick
    }
}
