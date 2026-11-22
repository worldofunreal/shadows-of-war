/**
 * Buildings, Upgrades, Arsenal Production & Demolition Recipes.
 *
 * Acoustic Identity:
 *   Each of the 7 `BuildingKind` structures has its own tactile material signature
 *   when placed AND when completed:
 *     - City:         Monumental stone foundation + warm bronze civic bell
 *     - Tower:        Dry stone lock + defensive iron bolt
 *     - Factory:      Anvil strike + steam/gear cadence
 *     - Port:         Timber dock thud + harbour splash & bell
 *     - Farm:         Warm hollow wood knock + earthy rustle
 *     - SAM:          Hydraulic latch + crisp radar lock chirp
 *     - MissileSilo:  Heavy steel blast-door seal + deep subterranean hum
 */

import {
    playFilteredNoise,
    playFmTone,
    playModalHit,
    playSubDrop,
    playTone,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import { ArsenalActionSubtype, BuildingAudioKind, type CueTrigger } from "../types.ts";

/** `BuildingPlace` — Tactile structure seating modulated by `BuildingAudioKind`. */
export function synthBuildingPlace(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    // Universal grounded placement thump
    playSubDrop(v, vary(v.rng, 140, 0.06), 54, 0.11, 0.28);

    switch (kind) {
        case BuildingAudioKind.City:
            // Heavy stone block + subtle warm civic overtone
            playModalHit(v, {
                baseFreq: vary(v.rng, 293.66, 0.02),
                ratios: [1, 1.5, 2.02],
                gains: [0.6, 0.26, 0.14],
                decay: 0.22,
                delay: 0.015,
                gain: 0.22,
            });
            break;

        case BuildingAudioKind.Factory:
            // Metallic anvil ping + mechanical seat
            playFmTone(v, {
                carrierFreq: vary(v.rng, 520, 0.06),
                modRatio: 2.38,
                modIndex: 1.7,
                modIndexEnd: 0.18,
                delay: 0.012,
                decay: 0.17,
                gain: 0.2,
            });
            break;

        case BuildingAudioKind.Port:
            // Timber knock + shore splash
            playModalHit(v, {
                baseFreq: vary(v.rng, 240, 0.05),
                ratios: [1, 1.58, 2.42],
                gains: [0.65, 0.25, 0.1],
                decay: 0.12,
                gain: 0.2,
            });
            playFilteredNoise(v, {
                color: "pink",
                delay: 0.018,
                attack: 0.008,
                decay: 0.18,
                gain: 0.16,
                filterType: "bandpass",
                filterFreq: 980,
                filterFreqEnd: 460,
                filterQ: 1.5,
            });
            break;

        case BuildingAudioKind.Farm:
            // Warm organic wood knock + soft grain brush
            playModalHit(v, {
                baseFreq: vary(v.rng, 320, 0.05),
                ratios: [1, 1.52, 2.28],
                gains: [0.7, 0.22, 0.08],
                decay: 0.11,
                gain: 0.19,
            });
            playFilteredNoise(v, {
                color: "pink",
                attack: 0.005,
                decay: 0.09,
                gain: 0.1,
                filterType: "bandpass",
                filterFreq: 1650,
                filterFreqEnd: 920,
            });
            break;

        case BuildingAudioKind.SAM:
            // Crisp pneumatic latch + subtle radar ping
            playFilteredNoise(v, {
                color: "white",
                attack: 0.003,
                decay: 0.08,
                gain: 0.15,
                filterType: "bandpass",
                filterFreq: 2400,
                filterFreqEnd: 1100,
                filterQ: 2.2,
            });
            playTone(v, {
                type: "sine",
                freq: 880,
                freqEnd: 1174.66,
                delay: 0.03,
                attack: 0.003,
                decay: 0.09,
                gain: 0.14,
            });
            break;

        case BuildingAudioKind.MissileSilo:
            // Heavy armored blast door + subterranean lock
            playSubDrop(v, 95, 36, 0.24, 0.36, 0.01);
            playFmTone(v, {
                carrierFreq: 165,
                carrierFreqEnd: 130,
                modRatio: 1.9,
                modIndex: 2.0,
                modIndexEnd: 0.25,
                delay: 0.02,
                decay: 0.22,
                gain: 0.2,
            });
            break;

        case BuildingAudioKind.Tower:
        default:
            // Fortified stone/iron bolt click
            playModalHit(v, {
                baseFreq: vary(v.rng, 370, 0.05),
                ratios: [1, 1.72, 2.76],
                gains: [0.58, 0.28, 0.14],
                decay: 0.14,
                delay: 0.01,
                gain: 0.2,
            });
            break;
    }
}

/** `BuildingUpgrade` — Ascending dual hammer/chisel confirmation. */
export function synthBuildingUpgrade(v: VoiceRenderCtx, _t: CueTrigger): void {
    const f0 = vary(v.rng, 440.0, 0.03);
    playModalHit(v, {
        baseFreq: f0,
        ratios: [1, 2.01, 3.04],
        gains: [0.6, 0.28, 0.12],
        decay: 0.12,
        gain: 0.19,
    });
    playModalHit(v, {
        baseFreq: f0 * 1.26, // major third lift
        ratios: [1, 2.01, 3.04],
        gains: [0.6, 0.28, 0.12],
        decay: 0.18,
        delay: 0.068,
        gain: 0.21,
    });
}

/** `BuildingComplete` — Satisfying harmonic completion chord tailored per `BuildingAudioKind`. */
export function synthBuildingComplete(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    // Harmonic triad/dyad in D Dorian / Major
    const chord =
        kind === BuildingAudioKind.City
            ? [293.66, 440.0, 587.33, 739.99] // D major civic bloom
            : kind === BuildingAudioKind.SAM || kind === BuildingAudioKind.MissileSilo
              ? [392.0, 587.33, 880.0] // clean fifths tactical online
              : [349.23, 440.0, 587.33]; // warm D-minor/Dorian lift

    chord.forEach((freq, idx) => {
        playModalHit(v, {
            baseFreq: vary(v.rng, freq, 0.008),
            ratios: [1, 2.0, 3.01],
            gains: [0.64, 0.26, 0.1],
            decay: 0.34 + idx * 0.05,
            delay: idx * 0.052,
            gain: 0.16,
        });
    });

    // Material flourish
    if (kind === BuildingAudioKind.Factory) {
        playFilteredNoise(v, {
            color: "pink",
            delay: 0.04,
            attack: 0.012,
            decay: 0.18,
            gain: 0.14,
            filterType: "bandpass",
            filterFreq: 1800,
            filterFreqEnd: 700,
        });
    } else if (kind === BuildingAudioKind.Port) {
        playTone(v, {
            type: "sine",
            freq: 293.66,
            delay: 0.02,
            attack: 0.02,
            decay: 0.32,
            gain: 0.15,
        });
    } else if (kind === BuildingAudioKind.SAM || kind === BuildingAudioKind.MissileSilo) {
        playFmTone(v, {
            carrierFreq: 1174.66,
            modRatio: 2.0,
            modIndex: 0.4,
            delay: 0.12,
            decay: 0.18,
            gain: 0.12,
        });
    }
}

/**
 * `ArsenalQueued` — Mechanical assembly click for queueing or cancelling warship/nuke production.
 * `subtype`: 0 = Warship, 1 = Nuke, 2 = AntiNuke, 3 = Cancel.
 */
export function synthArsenalQueued(v: VoiceRenderCtx, t: CueTrigger): void {
    const action = Math.round(t.subtype);
    if (action === ArsenalActionSubtype.Cancel) {
        playTone(v, {
            type: "triangle",
            freq: 440,
            freqEnd: 293.66,
            attack: 0.003,
            decay: 0.09,
            gain: 0.16,
        });
        return;
    }
    const base =
        action === ArsenalActionSubtype.QueueNuke
            ? 392.0
            : action === ArsenalActionSubtype.QueueAntiNuke
              ? 523.25
              : 440.0;
    playFmTone(v, {
        carrierFreq: vary(v.rng, base, 0.025),
        carrierFreqEnd: base * 1.25,
        modRatio: 2.0,
        modIndex: 0.75,
        attack: 0.003,
        decay: 0.1,
        gain: 0.18,
    });
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.035,
        gain: 0.09,
        filterType: "bandpass",
        filterFreq: 2600,
        filterQ: 2.0,
    });
}

/** `BuildingDemolished` — Masonry rumble and structural collapse. */
export function synthBuildingDemolished(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSubDrop(v, vary(v.rng, 108, 0.08), 38, 0.28, 0.34);
    playFilteredNoise(v, {
        color: "brown",
        attack: 0.006,
        decay: 0.36,
        gain: 0.28,
        filterType: "lowpass",
        filterFreq: 860,
        filterFreqEnd: 160,
    });
    // Falling debris clatter
    for (let i = 0; i < 3; i++) {
        playModalHit(v, {
            baseFreq: vary(v.rng, 220 + i * 75, 0.15),
            ratios: [1, 1.67, 2.45],
            gains: [0.6, 0.25, 0.15],
            decay: 0.09,
            delay: 0.04 + i * 0.045 + v.rng() * 0.02,
            gain: 0.11,
        });
    }
}
