/**
 * Cue Registry — Single Source of Truth Mapping `AudioCueId` → Recipe & Mixing Rules.
 *
 * Every cue defines:
 *   - `synth`:      Procedural Web Audio recipe function.
 *   - `category`:   Budget group (`combat`, `naval`, `building`, `nuke`, `diplomacy`, `match`, `ui`).
 *   - `priority`:   Determines whether soft polyphony limits or ducking apply.
 *   - `spatial`:    If true, applies StereoPanner + air-absorption LPF + distance reverb send.
 *   - `cooldownMs`: Minimum time between repeats of this exact cue inside JS (safety net).
 *   - `gain`:       Default mix trim so every family sits balanced below the master limiter.
 */

import type { VoiceRenderCtx } from "./dsp.ts";
import {
    synthBuildingComplete,
    synthBuildingDemolished,
    synthBuildingPlace,
    synthBuildingUpgrade,
    synthArsenalQueued,
} from "./recipes/buildings.ts";
import {
    synthAttackCancelled,
    synthAttackOrder,
    synthAttackStalled,
    synthCascadeStep,
    synthCounterAttack,
    synthDeployTroops,
    synthKillStreak,
    synthLandCombat,
    synthLandExpand,
    synthPlayerEliminated,
    synthUnderAttack,
} from "./recipes/combat.ts";
import {
    synthAllianceEnded,
    synthAllianceFormed,
    synthAllianceRequest,
    synthBetrayal,
    synthEmote,
    synthHeroAbility,
    synthResourceTransfer,
} from "./recipes/diplomacy.ts";
import {
    synthActionError,
    synthDefeat,
    synthMatchPhaseStart,
    synthVictory,
} from "./recipes/match.ts";
import {
    synthFleetOrder,
    synthFleetRecallOrScuttle,
    synthNavalImpact,
    synthNavalShot,
    synthNavalSink,
    synthTradeCaptured,
    synthTradeDelivered,
    synthTransportLanded,
    synthTransportLaunch,
    synthWarshipSpawned,
} from "./recipes/naval.ts";
import {
    synthMirvSplit,
    synthNukeArmed,
    synthNukeImpact,
    synthNukeLaunch,
    synthSamIntercept,
} from "./recipes/nukes.ts";
import {
    synthUiCampaignStep,
    synthUiCelebrationFanfare,
    synthUiCelebrationFly,
    synthUiCelebrationImpact,
    synthUiClick,
    synthUiHeroEquip,
    synthUiMatchFound,
    synthUiModalClose,
    synthUiModalOpen,
    synthUiPurchaseSuccess,
    synthUiRadialClose,
    synthUiRadialOpen,
    synthUiSliderTick,
    synthUiTabSwitch,
    synthUiToast,
} from "./recipes/ui.ts";
import {
    AudioCue,
    type AudioCategory,
    type AudioCueId,
    type AudioPriority,
    type CueTrigger,
} from "./types.ts";

export type CueSynthFn = (v: VoiceRenderCtx, trigger: CueTrigger) => void;

export interface CueDefinition {
    readonly synth: CueSynthFn;
    readonly category: AudioCategory;
    readonly priority: AudioPriority;
    readonly spatial: boolean;
    readonly cooldownMs: number;
    readonly gain: number;
}

/** Max concurrent active voices allowed per category (critical cues bypass soft caps). */
export const CATEGORY_VOICE_BUDGET: Record<AudioCategory, number> = {
    combat: 5,
    naval: 5,
    building: 4,
    nuke: 6,
    diplomacy: 3,
    match: 3,
    ui: 5,
};

/** Global hard ceiling on simultaneous procedural voices to protect mobile CPUs. */
export const MAX_TOTAL_VOICES = 18;

export const CUE_REGISTRY: Record<AudioCueId, CueDefinition> = {
    // ── Combat & Land ──────────────────────────────────────────────────
    [AudioCue.DeployTroops]: {
        synth: synthDeployTroops,
        category: "combat",
        priority: "normal",
        spatial: true,
        cooldownMs: 140,
        gain: 0.78,
    },
    [AudioCue.AttackOrder]: {
        synth: synthAttackOrder,
        category: "combat",
        priority: "normal",
        spatial: true,
        cooldownMs: 110,
        gain: 0.78,
    },
    [AudioCue.AttackStalled]: {
        synth: synthAttackStalled,
        category: "combat",
        priority: "normal",
        spatial: true,
        cooldownMs: 550,
        gain: 0.74,
    },
    [AudioCue.UnderAttack]: {
        synth: synthUnderAttack,
        category: "combat",
        priority: "critical",
        spatial: true,
        cooldownMs: 1800,
        gain: 0.9,
    },
    [AudioCue.LandExpand]: {
        synth: synthLandExpand,
        category: "combat",
        priority: "background",
        spatial: true,
        cooldownMs: 280,
        gain: 0.5,
    },
    [AudioCue.LandCombat]: {
        synth: synthLandCombat,
        category: "combat",
        priority: "background",
        spatial: true,
        cooldownMs: 240,
        gain: 0.62,
    },
    [AudioCue.CounterAttack]: {
        synth: synthCounterAttack,
        category: "combat",
        priority: "foreground",
        spatial: false,
        cooldownMs: 200,
        gain: 0.82,
    },
    [AudioCue.AttackCancelled]: {
        synth: synthAttackCancelled,
        category: "combat",
        priority: "normal",
        spatial: false,
        cooldownMs: 120,
        gain: 0.68,
    },
    [AudioCue.CascadeStep]: {
        synth: synthCascadeStep,
        category: "combat",
        priority: "foreground",
        spatial: true,
        cooldownMs: 95,
        gain: 0.84,
    },
    [AudioCue.KillStreak]: {
        synth: synthKillStreak,
        category: "combat",
        priority: "critical",
        spatial: true,
        cooldownMs: 260,
        gain: 0.92,
    },
    [AudioCue.PlayerEliminated]: {
        synth: synthPlayerEliminated,
        category: "combat",
        priority: "critical",
        spatial: true,
        cooldownMs: 420,
        gain: 0.92,
    },

    // ── Naval & Trade ──────────────────────────────────────────────────
    [AudioCue.TransportLaunch]: {
        synth: synthTransportLaunch,
        category: "naval",
        priority: "normal",
        spatial: true,
        cooldownMs: 180,
        gain: 0.75,
    },
    [AudioCue.TransportLanded]: {
        synth: synthTransportLanded,
        category: "naval",
        priority: "normal",
        spatial: true,
        cooldownMs: 220,
        gain: 0.72,
    },
    [AudioCue.FleetOrder]: {
        synth: synthFleetOrder,
        category: "naval",
        priority: "normal",
        spatial: true,
        cooldownMs: 130,
        gain: 0.74,
    },
    [AudioCue.FleetRecallOrScuttle]: {
        synth: synthFleetRecallOrScuttle,
        category: "naval",
        priority: "normal",
        spatial: false,
        cooldownMs: 180,
        gain: 0.75,
    },
    [AudioCue.WarshipSpawned]: {
        synth: synthWarshipSpawned,
        category: "naval",
        priority: "foreground",
        spatial: true,
        cooldownMs: 360,
        gain: 0.84,
    },
    [AudioCue.NavalShot]: {
        synth: synthNavalShot,
        category: "naval",
        priority: "background",
        spatial: true,
        cooldownMs: 130,
        gain: 0.62,
    },
    [AudioCue.NavalImpact]: {
        synth: synthNavalImpact,
        category: "naval",
        priority: "background",
        spatial: true,
        cooldownMs: 160,
        gain: 0.66,
    },
    [AudioCue.NavalSink]: {
        synth: synthNavalSink,
        category: "naval",
        priority: "foreground",
        spatial: true,
        cooldownMs: 380,
        gain: 0.86,
    },
    [AudioCue.TradeDelivered]: {
        synth: synthTradeDelivered,
        category: "naval",
        priority: "background",
        spatial: true,
        cooldownMs: 450,
        gain: 0.58,
    },
    [AudioCue.TradeCaptured]: {
        synth: synthTradeCaptured,
        category: "naval",
        priority: "normal",
        spatial: true,
        cooldownMs: 350,
        gain: 0.74,
    },

    // ── Buildings & Economy ────────────────────────────────────────────
    [AudioCue.BuildingPlace]: {
        synth: synthBuildingPlace,
        category: "building",
        priority: "normal",
        spatial: true,
        cooldownMs: 140,
        gain: 0.8,
    },
    [AudioCue.BuildingUpgrade]: {
        synth: synthBuildingUpgrade,
        category: "building",
        priority: "normal",
        spatial: true,
        cooldownMs: 140,
        gain: 0.8,
    },
    [AudioCue.BuildingComplete]: {
        synth: synthBuildingComplete,
        category: "building",
        priority: "foreground",
        spatial: true,
        cooldownMs: 260,
        gain: 0.86,
    },
    [AudioCue.ArsenalQueued]: {
        synth: synthArsenalQueued,
        category: "building",
        priority: "normal",
        spatial: false,
        cooldownMs: 90,
        gain: 0.72,
    },
    [AudioCue.BuildingDemolished]: {
        synth: synthBuildingDemolished,
        category: "building",
        priority: "normal",
        spatial: true,
        cooldownMs: 260,
        gain: 0.78,
    },

    // ── Nuclear & SAM Defense ──────────────────────────────────────────
    [AudioCue.NukeArmed]: {
        synth: synthNukeArmed,
        category: "nuke",
        priority: "foreground",
        spatial: false,
        cooldownMs: 160,
        gain: 0.8,
    },
    [AudioCue.NukeLaunch]: {
        synth: synthNukeLaunch,
        category: "nuke",
        priority: "critical",
        spatial: true,
        cooldownMs: 350,
        gain: 0.94,
    },
    [AudioCue.MirvSplit]: {
        synth: synthMirvSplit,
        category: "nuke",
        priority: "critical",
        spatial: true,
        cooldownMs: 300,
        gain: 0.92,
    },
    [AudioCue.SamIntercept]: {
        synth: synthSamIntercept,
        category: "nuke",
        priority: "foreground",
        spatial: true,
        cooldownMs: 180,
        gain: 0.86,
    },
    [AudioCue.NukeImpact]: {
        synth: synthNukeImpact,
        category: "nuke",
        priority: "critical",
        spatial: true,
        cooldownMs: 220,
        gain: 1.0,
    },

    // ── Diplomacy, Abilities & Emotes ──────────────────────────────────
    [AudioCue.AllianceRequest]: {
        synth: synthAllianceRequest,
        category: "diplomacy",
        priority: "foreground",
        spatial: false,
        cooldownMs: 600,
        gain: 0.8,
    },
    [AudioCue.AllianceFormed]: {
        synth: synthAllianceFormed,
        category: "diplomacy",
        priority: "foreground",
        spatial: false,
        cooldownMs: 500,
        gain: 0.86,
    },
    [AudioCue.AllianceEnded]: {
        synth: synthAllianceEnded,
        category: "diplomacy",
        priority: "normal",
        spatial: false,
        cooldownMs: 450,
        gain: 0.75,
    },
    [AudioCue.Betrayal]: {
        synth: synthBetrayal,
        category: "diplomacy",
        priority: "critical",
        spatial: false,
        cooldownMs: 800,
        gain: 0.96,
    },
    [AudioCue.ResourceTransfer]: {
        synth: synthResourceTransfer,
        category: "diplomacy",
        priority: "normal",
        spatial: false,
        cooldownMs: 220,
        gain: 0.78,
    },
    [AudioCue.HeroAbility]: {
        synth: synthHeroAbility,
        category: "diplomacy",
        priority: "foreground",
        spatial: true,
        cooldownMs: 450,
        gain: 0.82,
    },
    [AudioCue.Emote]: {
        synth: synthEmote,
        category: "diplomacy",
        priority: "normal",
        spatial: true,
        cooldownMs: 320,
        gain: 0.74,
    },

    // ── Match Lifecycle & Outcomes ─────────────────────────────────────
    [AudioCue.MatchPhaseStart]: {
        synth: synthMatchPhaseStart,
        category: "match",
        priority: "critical",
        spatial: false,
        cooldownMs: 1000,
        gain: 0.9,
    },
    [AudioCue.ActionError]: {
        synth: synthActionError,
        category: "match",
        priority: "normal",
        spatial: false,
        cooldownMs: 180,
        gain: 0.74,
    },
    [AudioCue.Victory]: {
        synth: synthVictory,
        category: "match",
        priority: "critical",
        spatial: false,
        cooldownMs: 1500,
        gain: 0.96,
    },
    [AudioCue.Defeat]: {
        synth: synthDefeat,
        category: "match",
        priority: "critical",
        spatial: false,
        cooldownMs: 1500,
        gain: 0.92,
    },

    // ── GUI / UX-UI ────────────────────────────────────────────────────
    [AudioCue.UiClick]: {
        synth: synthUiClick,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 35,
        gain: 0.64,
    },
    [AudioCue.UiTabSwitch]: {
        synth: synthUiTabSwitch,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 55,
        gain: 0.62,
    },
    [AudioCue.UiModalOpen]: {
        synth: synthUiModalOpen,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 70,
        gain: 0.66,
    },
    [AudioCue.UiModalClose]: {
        synth: synthUiModalClose,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 70,
        gain: 0.62,
    },
    [AudioCue.UiRadialOpen]: {
        synth: synthUiRadialOpen,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 65,
        gain: 0.68,
    },
    [AudioCue.UiRadialClose]: {
        synth: synthUiRadialClose,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 65,
        gain: 0.6,
    },
    [AudioCue.UiSliderTick]: {
        synth: synthUiSliderTick,
        category: "ui",
        priority: "background",
        spatial: false,
        cooldownMs: 32,
        gain: 0.56,
    },
    [AudioCue.UiToast]: {
        synth: synthUiToast,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 260,
        gain: 0.64,
    },
    [AudioCue.UiMatchFound]: {
        synth: synthUiMatchFound,
        category: "ui",
        priority: "foreground",
        spatial: false,
        cooldownMs: 900,
        gain: 0.86,
    },
    [AudioCue.UiPurchaseSuccess]: {
        synth: synthUiPurchaseSuccess,
        category: "ui",
        priority: "foreground",
        spatial: false,
        cooldownMs: 400,
        gain: 0.84,
    },
    [AudioCue.UiHeroEquip]: {
        synth: synthUiHeroEquip,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 120,
        gain: 0.74,
    },
    [AudioCue.UiCampaignStep]: {
        synth: synthUiCampaignStep,
        category: "ui",
        priority: "background",
        spatial: false,
        cooldownMs: 90,
        gain: 0.52,
    },
    [AudioCue.UiCelebrationFanfare]: {
        synth: synthUiCelebrationFanfare,
        category: "ui",
        priority: "foreground",
        spatial: false,
        cooldownMs: 350,
        gain: 0.88,
    },
    [AudioCue.UiCelebrationFly]: {
        synth: synthUiCelebrationFly,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 55,
        gain: 0.72,
    },
    [AudioCue.UiCelebrationImpact]: {
        synth: synthUiCelebrationImpact,
        category: "ui",
        priority: "normal",
        spatial: false,
        cooldownMs: 45,
        gain: 0.8,
    },
};
