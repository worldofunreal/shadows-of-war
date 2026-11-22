/**
 * Nuclear Arsenal & SAM Air-Defense Procedural Recipes.
 *
 * Acoustic Identity:
 *   Multi-stage physical sound design with strict headroom discipline:
 *     - `NukeArmed`:    Tactical silo lock & warning hum scaled by warhead class
 *     - `NukeLaunch`:   Rocket ignition crack + rising jet roar + distant warning sweep
 *     - `MirvSplit`:    High-altitude pneumatic separation pop + diverging whistle cluster
 *     - `SamIntercept`: Crisp mid-air flak/airburst crack (no ground sub-boom)
 *     - `NukeImpact`:   Sub-drop shockwave + saturated crack + brown-noise thunder + reverb tail,
 *                       distinctly weighted for Atom (1), Hydrogen (2), and MIRV warhead (4).
 */

import {
    playFilteredNoise,
    playFmTone,
    playSubDrop,
    playTone,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import { NukeAudioLevel, type CueTrigger } from "../types.ts";

/** `NukeArmed` — Silo safety disarm & targeting lock (`subtype`: 1=Atom, 2=Hydrogen, 3=MIRV). */
export function synthNukeArmed(v: VoiceRenderCtx, t: CueTrigger): void {
    const level = Math.round(t.subtype) || NukeAudioLevel.Atom;
    const base = level === NukeAudioLevel.Hydrogen ? 293.66 : level === NukeAudioLevel.MirvCarrier ? 246.94 : 349.23;

    // Mechanical safety toggle click
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.03,
        gain: 0.14,
        filterType: "bandpass",
        filterFreq: 3100,
        filterQ: 2.5,
    });

    // Tactical lock tone(s)
    const pulses = level === NukeAudioLevel.MirvCarrier ? 3 : level === NukeAudioLevel.Hydrogen ? 2 : 1;
    for (let i = 0; i < pulses; i++) {
        playFmTone(v, {
            carrierFreq: base * (1 + i * 0.12),
            carrierFreqEnd: base * (1.15 + i * 0.14),
            modRatio: 2.0,
            modIndex: 0.7,
            delay: 0.02 + i * 0.065,
            attack: 0.004,
            decay: 0.11,
            gain: 0.17,
        });
    }
}

/** `NukeLaunch` — Booster ignition + filtered thruster roar + distant klaxon sweep. */
export function synthNukeLaunch(v: VoiceRenderCtx, t: CueTrigger): void {
    const level = Math.round(t.subtype) || NukeAudioLevel.Atom;
    const heavy = level === NukeAudioLevel.Hydrogen || level === NukeAudioLevel.MirvCarrier;
    const drive = v.createWarmDrive();
    drive.connect(v.out);

    // Silo ignition thump
    playSubDrop(v, heavy ? 115 : 135, 36, heavy ? 0.32 : 0.24, heavy ? 0.42 : 0.34, 0, drive);

    // Ascending rocket thrust roar
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.025,
        decay: heavy ? 0.65 : 0.48,
        gain: heavy ? 0.32 : 0.26,
        filterType: "bandpass",
        filterFreq: 320,
        filterFreqEnd: heavy ? 2100 : 1650,
        filterQ: 1.1,
        dest: drive,
    });

    // Subtle rising launch siren harmonic
    playTone(v, {
        type: "triangle",
        freq: heavy ? 220 : 280,
        freqEnd: heavy ? 520 : 620,
        delay: 0.04,
        attack: 0.04,
        decay: heavy ? 0.46 : 0.34,
        gain: 0.14,
        filterType: "lowpass",
        filterFreq: 1400,
    });

    if (v.wet) {
        playFilteredNoise(v, {
            color: "brown",
            delay: 0.05,
            attack: 0.04,
            decay: 0.55,
            gain: 0.2,
            filterType: "lowpass",
            filterFreq: 520,
            dest: v.wet,
        });
    }
}

/** `MirvSplit` — High-altitude carrier separation + diverging warhead whistles. */
export function synthMirvSplit(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Pneumatic separation crack
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.09,
        gain: 0.28,
        filterType: "bandpass",
        filterFreq: 2200,
        filterFreqEnd: 780,
        filterQ: 1.4,
    });
    playSubDrop(v, 180, 62, 0.16, 0.28);

    // 4 staggered descending micro-whistles simulating warhead dispersal
    for (let i = 0; i < 4; i++) {
        const startHz = vary(v.rng, 1480 - i * 110, 0.08);
        playTone(v, {
            type: "sine",
            freq: startHz,
            freqEnd: startHz * 0.42,
            delay: 0.025 + i * 0.042,
            attack: 0.006,
            decay: 0.22,
            gain: 0.11,
        });
    }
}

/** `SamIntercept` — Sharp high-altitude airburst flash (dry crack + metallic shrapnel). */
export function synthSamIntercept(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Clean flak crack without heavy ground-shaking sub
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.13,
        gain: 0.3,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 1850, 0.15),
        filterFreqEnd: 620,
        filterQ: 1.3,
    });
    playFmTone(v, {
        carrierFreq: vary(v.rng, 680, 0.12),
        carrierFreqEnd: 240,
        modRatio: 2.71,
        modIndex: 2.2,
        modIndexEnd: 0.15,
        attack: 0.002,
        decay: 0.14,
        gain: 0.2,
    });
    playSubDrop(v, 190, 78, 0.09, 0.2);
}

/**
 * `NukeImpact` — Multi-stage nuclear detonation scaled by warhead class.
 * `subtype`: 1 = AtomBomb, 2 = HydrogenBomb (massive), 4 = MIRV warhead (punchy/tight).
 */
export function synthNukeImpact(v: VoiceRenderCtx, t: CueTrigger): void {
    const level = Math.round(t.subtype) || NukeAudioLevel.Atom;
    const isHydro = level === NukeAudioLevel.Hydrogen;
    const isMirv = level === NukeAudioLevel.MirvWarhead;
    const drive = v.createWarmDrive();
    drive.connect(v.out);

    // Stage 1: Initial flash crack (white noise transient)
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: isHydro ? 0.14 : 0.09,
        gain: isHydro ? 0.42 : isMirv ? 0.3 : 0.36,
        filterType: "bandpass",
        filterFreq: vary(v.rng, isHydro ? 1400 : 1800, 0.14),
        filterFreqEnd: 320,
        filterQ: 0.9,
        dest: drive,
    });

    // Stage 2: Physical shockwave sub-drop
    playSubDrop(
        v,
        vary(v.rng, isHydro ? 96 : isMirv ? 112 : 104, 0.06),
        isHydro ? 22 : isMirv ? 32 : 26,
        isHydro ? 0.85 : isMirv ? 0.38 : 0.62,
        isHydro ? 0.62 : isMirv ? 0.46 : 0.54,
        0,
        drive,
    );

    // Stage 3: Expanding pressure thunder (brown noise rumble)
    playFilteredNoise(v, {
        color: "brown",
        delay: 0.025,
        attack: 0.02,
        decay: isHydro ? 1.35 : isMirv ? 0.55 : 0.95,
        gain: isHydro ? 0.48 : isMirv ? 0.32 : 0.4,
        filterType: "lowpass",
        filterFreq: isHydro ? 680 : 820,
        filterFreqEnd: 95,
        dest: drive,
    });

    // Stage 4: Fallout debris / atmospheric hiss tail into reverb
    if (v.wet) {
        playFilteredNoise(v, {
            color: "pink",
            delay: 0.06,
            attack: 0.05,
            decay: isHydro ? 1.15 : isMirv ? 0.42 : 0.78,
            gain: isHydro ? 0.32 : isMirv ? 0.18 : 0.24,
            filterType: "bandpass",
            filterFreq: 520,
            filterFreqEnd: 180,
            filterQ: 0.8,
            dest: v.wet,
        });
    }
}
