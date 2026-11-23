/**
 * Buildings, Upgrades, Arsenal Production & Demolition Recipes (`buildings.ts`).
 *
 * Acoustic Identity (Clash of Clans / Rise of Kingdoms Craft):
 *   Every one of the 7 structures carries tactile physical foley when placed
 *   AND a signature flourish when completed:
 *     - City:        Marble foundation slam + royal civic brass & celesta
 *     - Tower:       Granite block seat + iron portcullis latch
 *     - TradingPost:     Forged hammer-on-anvil strike + pressurized steam hiss
 *     - Port:        Oak dock piling thud + multi-bubble harbor splash
 *     - MilitaryCamp:        Warm timber knock + acoustic Karplus-Strong harp fifth
 *     - SAM:         Hydraulic servo + machined double brass lock + radar chime
 *     - MissileSilo: Subterranean steel blast-door slam + bunker lock
 *   `BuildingComplete` blooms with a sweeping harp glissando, D Major9 celesta,
 *   and celebratory gold-coin sparkle in the 1.65 s golden hall.
 */

import {
    playBuffer,
    playFilteredNoise,
    playFmTone,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import {
    playBrassEnsemble,
    playCasinoChime,
    playCoinShower,
    playHarpGliss,
    playHarpPluck,
    playLiquidSplash,
    playSteelImpact,
    playStereoWhoosh,
    playTactilePop,
    playTaikoHit,
} from "../instruments.ts";
import {
    ArsenalActionSubtype,
    BuildingAudioKind,
    BuildingModeSubtype,
    type CueTrigger,
} from "../types.ts";

/** `BuildingPlace` — Heavy Supercell-style structure seating modulated by `BuildingAudioKind`. */
export function synthBuildingPlace(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    // Universal grounded foundation placement punch
    playTaikoHit(v, {
        heavy: kind === BuildingAudioKind.City || kind === BuildingAudioKind.MissileSilo,
        pitch: vary(v.rng, 1.0, 0.05),
        gain: 0.44,
        rimCrack: 0.35,
        wetSend: 0.28,
    });

    switch (kind) {
        case BuildingAudioKind.City:
            // Monumental marble slam + royal civic bronze sparkle
            playTactilePop(v, { material: "stone", pitch: 0.92, delay: 0.008, gain: 0.46, shimmerSend: 0.25 });
            playCasinoChime(v, { freq: 587.33, delay: 0.018, decay: 0.4, gain: 0.26, sparkle: 0.55 });
            playCasinoChime(v, { freq: 880.0, delay: 0.052, decay: 0.42, gain: 0.22, sparkle: 0.6, pan: 0.25 });
            break;

        case BuildingAudioKind.TradingPost:
            // Forged blacksmith hammer-on-anvil + crisp steam hiss
            playSteelImpact(v, { heavy: false, pitch: 1.18, delay: 0.008, gain: 0.42, wetSend: 0.34 });
            playFilteredNoise(v, {
                color: "pink",
                delay: 0.025,
                attack: 0.008,
                decay: 0.14,
                gain: 0.22,
                filterType: "bandpass",
                filterFreq: 2800,
                filterFreqEnd: 1100,
                filterQ: 1.6,
            });
            break;

        case BuildingAudioKind.Port:
            // Heavy oak piling slam + foaming multi-bubble harbor splash
            playBuffer(v, {
                buffer: v.bank.woodThock[1],
                delay: 0.006,
                rate: vary(v.rng, 0.88, 0.05),
                gain: 0.46,
                wetSend: 0.26,
            });
            playLiquidSplash(v, { delay: 0.018, decay: 0.34, pitch: 0.98, gain: 0.34, heavy: false });
            playCasinoChime(v, { freq: 783.99, delay: 0.032, decay: 0.28, gain: 0.16, sparkle: 0.35 });
            break;

        case BuildingAudioKind.MilitaryCamp:
            // Warm carved timber knock + rustic acoustic harp fifth + grain brush
            playTactilePop(v, { material: "wood", pitch: 1.05, popBend: 1.16, delay: 0.006, gain: 0.44 });
            playHarpPluck(v, { freq: 293.66, delay: 0.014, decay: 0.42, gain: 0.28, pan: -0.22 });
            playHarpPluck(v, { freq: 440.0, delay: 0.048, decay: 0.45, gain: 0.24, pan: 0.22 });
            playFilteredNoise(v, {
                color: "parchment",
                delay: 0.01,
                attack: 0.006,
                decay: 0.12,
                gain: 0.14,
                filterType: "bandpass",
                filterFreq: 1850,
                filterFreqEnd: 920,
            });
            break;

        case BuildingAudioKind.SAM:
            // Hydraulic pneumatic seat + double machined brass lock + crisp radar lock
            playFilteredNoise(v, {
                color: "pink",
                attack: 0.004,
                decay: 0.1,
                gain: 0.22,
                filterType: "bandpass",
                filterFreq: 3100,
                filterFreqEnd: 1200,
                filterQ: 2.0,
            });
            playBuffer(v, { buffer: v.bank.brassLatch[0], delay: 0.012, rate: 1.12, gain: 0.36 });
            playBuffer(v, { buffer: v.bank.brassLatch[1], delay: 0.055, rate: 1.24, gain: 0.28 });
            playFmTone(v, {
                carrierFreq: 1174.66,
                carrierFreqEnd: 1479.98,
                modRatio: 2.0,
                modIndex: 0.35,
                delay: 0.05,
                decay: 0.14,
                gain: 0.16,
                shimmerSend: 0.32,
            });
            break;

        case BuildingAudioKind.MissileSilo:
            // Colossal armored blast-door slam + subterranean steel seal
            playSteelImpact(v, { heavy: true, pitch: 0.68, delay: 0.012, gain: 0.48, wetSend: 0.46 });
            playBuffer(v, { buffer: v.bank.brassLatch[1], delay: 0.045, rate: 0.72, gain: 0.38, wetSend: 0.32 });
            playFilteredNoise(v, {
                color: "brown",
                delay: 0.03,
                attack: 0.015,
                decay: 0.26,
                gain: 0.26,
                filterType: "lowpass",
                filterFreq: 680,
                filterFreqEnd: 180,
                wetSend: 0.36,
            });
            break;

        case BuildingAudioKind.Tower:
        default:
            // Fortified stone block + heavy iron portcullis bolt
            playTactilePop(v, { material: "stone", pitch: 1.04, delay: 0.006, gain: 0.46 });
            playBuffer(v, { buffer: v.bank.brassLatch[1], delay: 0.022, rate: 0.96, gain: 0.32, wetSend: 0.25 });
            break;
    }
}

/** `BuildingUpgrade` — Addictive Supercell double craftsmanship tap + ascending golden sparkle! */
export function synthBuildingUpgrade(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Dual ascending stone/smith strikes
    playTactilePop(v, { material: "stone", pitch: 1.04, popBend: 1.2, gain: 0.38, pan: -0.18 });
    playSteelImpact(v, { heavy: false, pitch: 1.26, delay: 0.072, gain: 0.26, pan: 0.18, wetSend: 0.24 });

    // Radiant ascending celesta fifth + luxury coin sparkle!
    playCasinoChime(v, { freq: 587.33, delay: 0.01, decay: 0.26, gain: 0.26, sparkle: 0.55, pan: -0.2 });
    playCasinoChime(v, { freq: 880.0, delay: 0.076, decay: 0.38, gain: 0.3, sparkle: 0.8, pan: 0.22 });
    playCoinShower(v, {
        count: 2,
        delay: 0.085,
        spacing: 0.038,
        pitch: 1.12,
        pitchClimb: 0.06,
        gain: 0.2,
        spread: 0.42,
    });
}

/** `BuildingComplete` — Iconic Clash of Clans / RoK golden completion bloom per `BuildingAudioKind`. */
export function synthBuildingComplete(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    // Satisfying grounding pop
    playTaikoHit(v, { heavy: kind === BuildingAudioKind.City, pitch: 1.08, gain: 0.36, rimCrack: 0.4 });

    const isTactical = kind === BuildingAudioKind.SAM || kind === BuildingAudioKind.MissileSilo;
    const notes =
        kind === BuildingAudioKind.City
            ? [293.66, 369.99, 440.0, 587.33, 739.99, 1174.66] // Grand royal D Maj
            : isTactical
              ? [392.0, 587.33, 880.0, 1174.66] // Crisp stacked fifths online
              : [369.99, 440.0, 587.33, 659.25, 880.0]; // Radiant D Maj9 shimmer

    // Sweeping stereo Karplus-Strong golden harp glissando!
    playHarpGliss(v, notes, {
        delay: 0.012,
        stepSec: 0.036,
        gain: 0.28,
        decay: 0.72,
        panStart: -0.48,
        panEnd: 0.48,
        shimmerSend: 0.62,
    });

    // Glassy celesta crown chord
    const topNotes = notes.slice(-3);
    topNotes.forEach((freq, i) => {
        playCasinoChime(v, {
            freq,
            delay: 0.08 + i * 0.044,
            decay: 0.56 + i * 0.06,
            gain: 0.24,
            sparkle: 0.85,
            pan: (i - 1) * 0.34,
            shimmerSend: 0.68,
        });
    });

    // Celebratory gold sparkle or signature building flourish
    if (kind === BuildingAudioKind.City) {
        playBrassEnsemble(v, {
            freq: 293.66,
            delay: 0.05,
            attack: 0.03,
            hold: 0.06,
            decay: 0.55,
            gain: 0.3,
            brightness: 1.25,
            goldenHall: true,
        });
        playCoinShower(v, { count: 4, delay: 0.12, spacing: 0.04, pitch: 1.08, gain: 0.22, spread: 0.55 });
    } else if (kind === BuildingAudioKind.TradingPost) {
        playSteelImpact(v, { heavy: false, pitch: 1.32, delay: 0.02, gain: 0.24 });
        playFilteredNoise(v, {
            color: "pink",
            delay: 0.05,
            attack: 0.012,
            decay: 0.2,
            gain: 0.18,
            filterType: "bandpass",
            filterFreq: 2100,
            filterFreqEnd: 780,
        });
        playCoinShower(v, { count: 2, delay: 0.14, spacing: 0.04, pitch: 1.1, gain: 0.18 });
    } else if (kind === BuildingAudioKind.Port) {
        playBrassEnsemble(v, {
            freq: 220.0,
            delay: 0.03,
            attack: 0.03,
            decay: 0.38,
            gain: 0.22,
            brightness: 0.95,
            goldenHall: true,
        });
        playLiquidSplash(v, { delay: 0.04, decay: 0.26, gain: 0.2 });
    } else if (isTactical) {
        playBuffer(v, { buffer: v.bank.brassLatch[1], delay: 0.02, rate: 1.15, gain: 0.28 });
        playFmTone(v, {
            carrierFreq: 1479.98,
            modRatio: 2.0,
            modIndex: 0.35,
            delay: 0.14,
            decay: 0.22,
            gain: 0.16,
            shimmerSend: 0.45,
        });
    } else {
        playCoinShower(v, { count: 3, delay: 0.11, spacing: 0.042, pitch: 1.1, gain: 0.2, spread: 0.48 });
    }
}

/**
 * `ArsenalQueued` — Heavy precision mechanical latch for queueing or cancelling arsenal builds.
 * `subtype`: 0 = Warship, 1 = Nuke, 2 = AntiNuke, 3 = Cancel.
 */
export function synthArsenalQueued(v: VoiceRenderCtx, t: CueTrigger): void {
    const action = Math.round(t.subtype);
    if (action === ArsenalActionSubtype.Cancel) {
        playTactilePop(v, { material: "wood", pitch: 0.82, popBend: 0.82, noteHz: 349.23, gain: 0.32 });
        return;
    }

    const freq =
        action === ArsenalActionSubtype.QueueNuke
            ? 440.0
            : action === ArsenalActionSubtype.QueueAntiNuke
              ? 659.25
              : 587.33;

    playTactilePop(v, {
        material: "brass",
        pitch: action === ArsenalActionSubtype.QueueNuke ? 0.92 : 1.12,
        popBend: 1.18,
        noteHz: freq,
        gain: 0.38,
        shimmerSend: 0.22,
    });
    playSteelImpact(v, {
        heavy: action === ArsenalActionSubtype.QueueNuke,
        pitch: action === ArsenalActionSubtype.QueueAntiNuke ? 1.35 : 1.05,
        delay: 0.012,
        gain: 0.18,
        wetSend: 0.2,
    });
    playCasinoChime(v, {
        freq: freq * 1.5,
        delay: 0.02,
        decay: 0.2,
        gain: 0.16,
        sparkle: 0.45,
    });
}

/** `BuildingDemolished` — Structural collapse with crackling masonry & stereo falling stone debris. */
export function synthBuildingDemolished(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTaikoHit(v, { heavy: true, pitch: 0.82, gain: 0.56, wetSend: 0.45 });
    playFilteredNoise(v, {
        color: "crackle",
        delay: 0.01,
        attack: 0.008,
        decay: 0.46,
        gain: 0.42,
        filterType: "lowpass",
        filterFreq: 1180,
        filterFreqEnd: 150,
        wetSend: 0.42,
    });
    // Cascading stereo falling bricks & timber rubble
    const pans = [-0.45, 0.32, -0.2, 0.48];
    for (let i = 0; i < 4; i++) {
        const useStone = i % 2 === 0;
        playBuffer(v, {
            buffer: useStone ? v.bank.stoneClick[i % 2] : v.bank.woodThock[1],
            delay: 0.045 + i * 0.052 + v.rng() * 0.024,
            rate: vary(v.rng, 0.72 + i * 0.11, 0.12),
            gain: 0.28 - i * 0.04,
            pan: pans[i],
            wetSend: 0.35,
        });
    }
}

/**
 * `BuildingUpgradeProgress` — Cozy single-channel artisan hammer/ratchet tick while an upgrade
 * is underway (`subtype`: representative `BuildingAudioKind`, strictly singleton-gated against stacking).
 */
export function synthBuildingUpgradeProgress(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    const beat = Math.floor(v.rng() * 3);
    const tech =
        kind === BuildingAudioKind.SAM ||
        kind === BuildingAudioKind.MissileSilo ||
        kind === BuildingAudioKind.TradingPost;

    // Warm carpenter / mason scaffold tap (rotates pitch slightly per beat for organic cadence)
    playBuffer(v, {
        buffer: v.bank.woodThock[beat % 3],
        rate: vary(v.rng, 0.96 + beat * 0.08, 0.05),
        decay: 0.085,
        gain: 0.24,
        pan: beat === 1 ? -0.18 : 0.14,
        wetSend: 0.22,
    });

    if (tech) {
        // Light precision ratchet / wrench click for industrial & missile buildings
        playBuffer(v, {
            buffer: v.bank.brassLatch[beat % 2],
            delay: 0.018,
            rate: vary(v.rng, 1.08 + (beat % 2) * 0.14, 0.04),
            decay: 0.065,
            gain: 0.2,
            pan: -0.14,
            wetSend: 0.2,
        });
    } else {
        // Crisp forged chisel-on-stone / timber hammer tap on alternate beats
        playBuffer(v, {
            buffer: v.bank.stoneClick[beat % 2],
            delay: 0.022,
            rate: vary(v.rng, 1.04 + beat * 0.07, 0.05),
            decay: 0.08,
            gain: 0.2,
            pan: 0.16,
            wetSend: 0.22,
        });
    }

    // Subtle bright metallic hammer spark on accent beat (every ~3rd tick)
    if (beat === 2) {
        playBuffer(v, {
            buffer: v.bank.steelClash[0],
            delay: 0.012,
            rate: vary(v.rng, 1.48, 0.06),
            decay: 0.075,
            gain: 0.09,
            filterType: "highpass",
            filterFreq: 1800,
            wetSend: 0.28,
        });
    }
}

/**
 * `BuildingModeEnter` — Entering, switching within, or exiting blueprint placement mode.
 * `subtype`: `0` = Enter mode, `1` = Switch building type, `2` = Exit mode.
 */
export function synthBuildingModeEnter(v: VoiceRenderCtx, t: CueTrigger): void {
    const mode = Math.round(t.subtype);
    if (mode === BuildingModeSubtype.Exit) {
        playStereoWhoosh(v, { direction: -1, decay: 0.12, gain: 0.22, bright: false });
        playTactilePop(v, {
            material: "wood",
            pitch: 0.84,
            popBend: 0.84,
            noteHz: 369.99,
            delay: 0.01,
            gain: 0.28,
            shimmerSend: 0.1,
        });
        return;
    }

    if (mode === BuildingModeSubtype.Switch) {
        playTactilePop(v, {
            material: "stone",
            pitch: 1.12,
            popBend: 1.2,
            noteHz: 587.33,
            gain: 0.34,
            shimmerSend: 0.18,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: 0.008,
            rate: vary(v.rng, 1.2, 0.04),
            decay: 0.055,
            gain: 0.18,
        });
        return;
    }

    // Enter building mode: drafting parchment unfurl + compass latch + blueprint chime
    playStereoWhoosh(v, { direction: 1, decay: 0.15, gain: 0.26, bright: true });
    playTactilePop(v, {
        material: "brass",
        pitch: 1.06,
        popBend: 1.22,
        noteHz: 587.33,
        delay: 0.008,
        gain: 0.36,
        shimmerSend: 0.26,
    });
    playCasinoChime(v, {
        freq: 880.0,
        delay: 0.022,
        decay: 0.26,
        gain: 0.18,
        sparkle: 0.5,
        shimmerSend: 0.42,
    });
}

/**
 * `BuildingSelect` — Selecting an existing structure on the map (`BuildingDetails` card opens).
 * Modulated by `BuildingAudioKind` so each structure feels physically recognizable.
 */
export function synthBuildingSelect(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    // Universal tactile card-open seal
    playTactilePop(v, {
        material: kind === BuildingAudioKind.MilitaryCamp || kind === BuildingAudioKind.Port ? "wood" : "stone",
        pitch: 1.08,
        popBend: 1.16,
        gain: 0.32,
        shimmerSend: 0.18,
    });

    switch (kind) {
        case BuildingAudioKind.City:
            playCasinoChime(v, { freq: 587.33, delay: 0.01, decay: 0.32, gain: 0.22, sparkle: 0.55 });
            break;
        case BuildingAudioKind.TradingPost:
            playBuffer(v, {
                buffer: v.bank.steelClash[0],
                delay: 0.008,
                rate: vary(v.rng, 1.35, 0.04),
                decay: 0.095,
                gain: 0.16,
                wetSend: 0.22,
            });
            break;
        case BuildingAudioKind.Port:
            playCasinoChime(v, { freq: 739.99, delay: 0.012, decay: 0.26, gain: 0.18, sparkle: 0.4 });
            break;
        case BuildingAudioKind.MilitaryCamp:
            playHarpPluck(v, { freq: 440.0, delay: 0.01, decay: 0.32, gain: 0.22, shimmerSend: 0.38 });
            break;
        case BuildingAudioKind.SAM:
            playBuffer(v, { buffer: v.bank.brassLatch[0], delay: 0.008, rate: 1.26, gain: 0.24 });
            playCasinoChime(v, { freq: 1174.66, delay: 0.018, decay: 0.18, gain: 0.14, sparkle: 0.3 });
            break;
        case BuildingAudioKind.MissileSilo:
            playBuffer(v, { buffer: v.bank.brassLatch[1], delay: 0.008, rate: 0.86, gain: 0.28, wetSend: 0.24 });
            break;
        case BuildingAudioKind.Tower:
        default:
            playBuffer(v, { buffer: v.bank.brassLatch[0], delay: 0.01, rate: 0.98, gain: 0.22 });
            break;
    }
}
