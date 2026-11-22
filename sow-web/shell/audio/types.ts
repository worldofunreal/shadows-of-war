/**
 * Procedural Audio Architecture — Shared Types & Rust WASM ↔ TS Binary Protocol.
 *
 * ARCHITECTURAL OVERVIEW
 * ======================
 * Shadows of War generates 100% of its SFX and GUI cues procedurally via Web Audio
 * in TypeScript. No WAV/MP3 SFX assets are bundled in WASM or fetched over HTTP.
 *
 * Coordinate & Data Flow:
 *   1. Rust WASM (`sow-client/src/app/sfx.rs`) remains the single authority for
 *      gameplay rules, per-event cooldowns, and camera-relative spatial projection
 *      (`pan` ∈ [-0.85, +0.85], `gain` ∈ [0, 1]).
 *   2. Admitted events are appended into a reusable flat `Vec<f32>` in WASM linear
 *      memory with fixed stride `AUDIO_EVENT_STRIDE = 6` floats (24 bytes per cue):
 *        `[cueId, subtype, pan, gain, intensity, seed]`
 *   3. Once per frame, Rust performs a single zero-copy FFI call:
 *        `window.SOW_sfxFlush(Float32Array::view(&queue))`
 *      JS reads the view synchronously (zero heap allocations, zero JSON) and
 *      dispatches each cue into Web Audio's native audio thread.
 *   4. GUI/UX cues from the browser shell (`main_menu.shell.ts`, `main_menu.hud.ts`,
 *      `main_menu.tutorial.ts`) invoke `playUiCue(...)` directly on the same
 *      `AudioEngine` singleton, sharing one `AudioContext`, master limiter, volume
 *      sliders, and portal mute state.
 *
 * IMPORTANT: Any ID added or renumbered in `AudioCue` or subtype constants MUST
 * stay in exact lockstep with `sow-client/src/app/audio.rs`.
 */

/** Number of `f32` values packed per event in the Rust → JS binary batch buffer. */
export const AUDIO_EVENT_STRIDE = 6;

/**
 * Canonical numeric cue identifiers shared between Rust (`sow-client/src/app/audio.rs`)
 * and TypeScript (`sow-web/shell/audio/`).
 */
export const AudioCue = {
    // ── Combat & Land (0–10) ───────────────────────────────────────────
    DeployTroops: 0,
    AttackOrder: 1,
    AttackStalled: 2,
    UnderAttack: 3,
    LandExpand: 4,
    LandCombat: 5,
    CounterAttack: 6,
    AttackCancelled: 7,
    CascadeStep: 8,
    KillStreak: 9,
    PlayerEliminated: 10,

    // ── Naval & Trade (11–20) ──────────────────────────────────────────
    TransportLaunch: 11,
    TransportLanded: 12,
    FleetOrder: 13,
    FleetRecallOrScuttle: 14,
    WarshipSpawned: 15,
    NavalShot: 16,
    NavalImpact: 17,
    NavalSink: 18,
    TradeDelivered: 19,
    TradeCaptured: 20,

    // ── Buildings & Economy (21–25) ────────────────────────────────────
    BuildingPlace: 21,
    BuildingUpgrade: 22,
    BuildingComplete: 23,
    ArsenalQueued: 24,
    BuildingDemolished: 25,

    // ── Nuclear & SAM Defense (26–30) ──────────────────────────────────
    NukeArmed: 26,
    NukeLaunch: 27,
    MirvSplit: 28,
    SamIntercept: 29,
    NukeImpact: 30,

    // ── Diplomacy, Abilities & Emotes (31–37) ──────────────────────────
    AllianceRequest: 31,
    AllianceFormed: 32,
    AllianceEnded: 33,
    Betrayal: 34,
    ResourceTransfer: 35,
    HeroAbility: 36,
    Emote: 37,

    // ── Match Lifecycle & Outcomes (38–41) ─────────────────────────────
    MatchPhaseStart: 38,
    ActionError: 39,
    Victory: 40,
    Defeat: 41,

    // ── GUI / UX-UI (42–56) ────────────────────────────────────────────
    UiClick: 42,
    UiTabSwitch: 43,
    UiModalOpen: 44,
    UiModalClose: 45,
    UiRadialOpen: 46,
    UiRadialClose: 47,
    UiSliderTick: 48,
    UiToast: 49,
    UiMatchFound: 50,
    UiPurchaseSuccess: 51,
    UiHeroEquip: 52,
    UiCampaignStep: 53,
    UiCelebrationFanfare: 54,
    UiCelebrationFly: 55,
    UiCelebrationImpact: 56,
} as const;

export type AudioCueId = (typeof AudioCue)[keyof typeof AudioCue];

/** Building subtype codes matching `sow_core::game::BuildingKind` (`#[repr(u8)]`). */
export const BuildingAudioKind = {
    City: 0,
    Tower: 1,
    Factory: 2,
    Port: 3,
    Farm: 4,
    SAM: 5,
    MissileSilo: 6,
} as const;

/** Opponent archetype codes for `LandCombat` and `PlayerEliminated`. */
export const CombatAudioSubtype = {
    Wilderness: 0,
    Tribe: 1,
    Nation: 2,
    Human: 3,
    Counter: 4,
} as const;

/** Nuclear weapon level codes matching `ProjectileKind::Nuke { level }` / `NukeKind`. */
export const NukeAudioLevel = {
    Atom: 1,
    Hydrogen: 2,
    MirvCarrier: 3,
    MirvWarhead: 4,
} as const;

/** Fleet order / management subtype codes. */
export const FleetOrderSubtype = {
    Move: 0,
    Patrol: 1,
    Recall: 0,
    Scuttle: 1,
} as const;

/** Arsenal queue subtype codes. */
export const ArsenalActionSubtype = {
    QueueWarship: 0,
    QueueNuke: 1,
    QueueAntiNuke: 2,
    Cancel: 3,
} as const;

/** Resource transfer / request subtype codes. */
export const ResourceAudioSubtype = {
    Sent: 0,
    Received: 1,
    Requested: 2,
    Rejected: 3,
    HelpCall: 4,
} as const;

/** Hero ability visual/audio cue family matching `sow_core::geo_entities::Cue`. */
export const AbilityAudioFamily = {
    Strength: 0,
    Conquest: 1,
    Income: 2,
    Tower: 3,
    City: 4,
    Losses: 5,
    Alliance: 6,
    Retreat: 7,
    Naval: 8,
} as const;

/** Emote subtype codes matching `sow_core::emote::EmoteKind`. */
export const EmoteAudioKind = {
    Greet: 0,
    Help: 1,
    Threat: 2,
    Celebrate: 3,
} as const;

/** Match phase start subtype codes. */
export const MatchPhaseAudioKind = {
    Spawning: 0,
    Playing: 1,
} as const;

/** Voice budget categories controlling polyphony limits and priority behavior. */
export type AudioCategory = "combat" | "naval" | "building" | "nuke" | "diplomacy" | "match" | "ui";

/**
 * Priority determines whether a cue may bypass soft background voice caps and
 * whether it applies subtle temporary ducking to lower-priority combat ambience.
 */
export type AudioPriority = "background" | "normal" | "foreground" | "critical";

/** Normalized per-trigger parameters decoded from the WASM batch or UI caller. */
export interface CueTrigger {
    readonly cue: AudioCueId;
    /** Semantic variant (building type, nuke level, cascade step 1..8, emote kind, etc.). */
    readonly subtype: number;
    /** Stereo placement in [-1, 1]; 0 for centered UI and non-spatial fanfares. */
    readonly pan: number;
    /** Spatial attenuation or cue-level loudness scalar in [0, 1]. */
    readonly gain: number;
    /** Continuous expressive modulation (commitment, troop ratio, severity, slider pct) in [0, 1]. */
    readonly intensity: number;
    /** 24-bit deterministic seed for perspective/transient variation across repeats. */
    readonly seed: number;
}

/** Settings mirrored from Rust `SettingsState` and browser shell controls. */
export interface AudioSettingsSnapshot {
    mute_all?: boolean;
    music_volume?: number;
    sfx_volume?: number;
}

/** Global handle installed on `window.SOW_audio` for portals, shell, and diagnostics. */
export interface SowAudioPublicApi {
    readonly version: number;
    unlock(): void;
    playUi(cue: AudioCueId, subtype?: number, intensity?: number, pan?: number): void;
    syncSettings(settings: AudioSettingsSnapshot | null | undefined): void;
    syncPortalMute(): void;
    setTabHidden(hidden: boolean): void;
    getContext(): AudioContext | null;
    ensureContext(): AudioContext | null;
    getMusicBus(): GainNode | null;
    getActiveVoiceCount(): number;
}
