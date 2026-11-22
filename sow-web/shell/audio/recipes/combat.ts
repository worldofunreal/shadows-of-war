/**
 * Combat, Land Expansion, Command & Streak Procedural Recipes.
 *
 * Acoustic Identity:
 *   Taut leather war-drums, gravel/soil footsteps, damped steel/bronze clashes,
 *   and warm D-Pentatonic modal bells for conquest cascades and killstreaks.
 *   Each repeated hit rotates exciter emphasis via `trigger.seed` so rapid battle
 *   sequences feel organic rather than cloned.
 */

import {
    clamp,
    PENTATONIC_D,
    playFilteredNoise,
    playFmTone,
    playModalHit,
    playSubDrop,
    playTone,
    semitones,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import { CombatAudioSubtype, type CueTrigger } from "../types.ts";

/** `DeployTroops` — Double-tap war drum + crisp banner cloth snap during Spawning. */
export function synthDeployTroops(v: VoiceRenderCtx, t: CueTrigger): void {
    const pitchBase = vary(v.rng, 118, 0.08);
    // Primary taut drum thump
    playSubDrop(v, pitchBase * 1.45, pitchBase * 0.58, 0.13, 0.42);
    // Secondary trailing cadence tap (slight organic timing variance)
    const tapDelay = 0.055 + v.rng() * 0.018;
    playSubDrop(v, pitchBase * 1.75, pitchBase * 0.72, 0.11, 0.3, tapDelay);
    // Banner fabric / boots settle transient
    playFilteredNoise(v, {
        color: "pink",
        delay: 0.008,
        attack: 0.004,
        decay: 0.095,
        gain: 0.16,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 1450, 0.16),
        filterFreqEnd: 780,
        filterQ: 1.8,
    });
    // Subtle confirming harmonic pluck (D5) so placement feels crisp
    playModalHit(v, {
        baseFreq: vary(v.rng, 587.33, 0.015),
        ratios: [1, 2.0, 3.01],
        gains: [0.55, 0.25, 0.1],
        decay: 0.14,
        delay: tapDelay,
        gain: 0.12,
    });
    void t;
}

/**
 * `AttackOrder` — Decisive tactical command pulse.
 * `subtype`: 0 = neutral expansion order, 1 = hostile enemy assault.
 * `intensity`: player's attack ratio commitment [0.05..1.0].
 */
export function synthAttackOrder(v: VoiceRenderCtx, t: CueTrigger): void {
    const hostile = t.subtype >= 1;
    const commit = clamp(t.intensity, 0.1, 1.0);
    // Rotate perspective subtly between crisp drum snap and heavier brassy thrust
    const perspectiveVariant = Math.floor(v.rng() * 3);
    const drumStart = vary(v.rng, hostile ? 148 : 132, 0.07) * (1.06 - commit * 0.14);
    const drumEnd = drumStart * (hostile ? 0.38 : 0.48);

    // Command body thump
    playSubDrop(v, drumStart, drumEnd, 0.14 + commit * 0.08, 0.34 + commit * 0.14);

    // Forward directional swoosh (air/marching push)
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.008,
        decay: 0.12 + commit * 0.08,
        gain: 0.16 + commit * 0.08,
        filterType: "bandpass",
        filterFreq: vary(v.rng, hostile ? 900 : 720, 0.14),
        filterFreqEnd: vary(v.rng, hostile ? 2200 : 1480, 0.12),
        filterQ: 1.6,
    });

    if (hostile) {
        // Steel edge / command horn accent for enemy assaults
        const edgeFreq = vary(v.rng, perspectiveVariant === 0 ? 523.25 : perspectiveVariant === 1 ? 587.33 : 440.0, 0.03);
        playFmTone(v, {
            carrierFreq: edgeFreq,
            carrierFreqEnd: edgeFreq * 1.12,
            modRatio: 2.01,
            modIndex: 0.9 + commit * 0.6,
            modIndexEnd: 0.15,
            attack: 0.005,
            decay: 0.14 + commit * 0.07,
            gain: 0.14 + commit * 0.06,
        });
    } else {
        // Lighter wooden-drum cadence for neutral land push
        playModalHit(v, {
            baseFreq: vary(v.rng, 349.23, 0.04),
            ratios: [1, 1.58, 2.32],
            gains: [0.6, 0.28, 0.12],
            decay: 0.09,
            delay: 0.015,
            gain: 0.14,
        });
    }
}

/** `AttackStalled` — Muted minor/tritone drop + braking scrape when a large push dies. */
export function synthAttackStalled(v: VoiceRenderCtx, t: CueTrigger): void {
    const sev = clamp(t.intensity, 0.2, 1.0);
    const root = vary(v.rng, 220.0, 0.04);
    // Two damped descending muted brass/wood tones (A3 -> F3 -> Eb3 feel)
    playTone(v, {
        type: "triangle",
        freq: root,
        freqEnd: root * 0.78,
        attack: 0.008,
        decay: 0.18 + sev * 0.08,
        gain: 0.24 + sev * 0.08,
        filterType: "lowpass",
        filterFreq: 1100,
        filterFreqEnd: 260,
        filterQ: 1.5,
    });
    playTone(v, {
        type: "sawtooth",
        freq: root * 0.75,
        freqEnd: root * 0.56,
        delay: 0.075,
        attack: 0.01,
        decay: 0.22,
        gain: 0.15 + sev * 0.06,
        filterType: "lowpass",
        filterFreq: 720,
        filterFreqEnd: 180,
        filterQ: 1.2,
    });
    playFilteredNoise(v, {
        color: "brown",
        attack: 0.01,
        decay: 0.2,
        gain: 0.18 * sev,
        filterType: "bandpass",
        filterFreq: 420,
        filterFreqEnd: 160,
        filterQ: 1.1,
    });
}

/** `UnderAttack` — Urgent dual war-horn pulse panned toward the threatened front. */
export function synthUnderAttack(v: VoiceRenderCtx, t: CueTrigger): void {
    const sev = clamp(t.intensity, 0.2, 1.0);
    const hornBase = vary(v.rng, 196.0, 0.025); // G3
    for (let i = 0; i < 2; i++) {
        const delay = i * 0.145;
        const pitch = i === 0 ? hornBase : semitones(hornBase, 1); // tense semitone lift
        playTone(v, {
            type: "sawtooth",
            freq: pitch,
            freqEnd: pitch * 0.985,
            delay,
            attack: 0.016,
            decay: 0.16 + sev * 0.06,
            gain: (0.2 + sev * 0.1) * (i === 1 ? 1.08 : 0.92),
            filterType: "lowpass",
            filterFreq: 680 + sev * 650,
            filterFreqEnd: 320,
            filterQ: 2.4,
        });
        // Crisp warning bell transient over the horn
        playFmTone(v, {
            carrierFreq: pitch * 3.0,
            modRatio: 1.414,
            modIndex: 0.85,
            delay,
            attack: 0.004,
            decay: 0.11,
            gain: 0.09 + sev * 0.05,
        });
    }
    playSubDrop(v, 110, 52, 0.18, 0.28 + sev * 0.12, 0);
}

/** `LandExpand` — Soft, tactile territorial capture step over neutral wilderness. */
export function synthLandExpand(v: VoiceRenderCtx, t: CueTrigger): void {
    const scale = clamp(t.intensity, 0.15, 1.0);
    const variant = Math.floor(v.rng() * 3);
    // Low organic earth tap + subtle dry soil brush
    const thump = vary(v.rng, variant === 0 ? 126 : variant === 1 ? 142 : 112, 0.09);
    playSubDrop(v, thump, thump * 0.56, 0.075, 0.16 + scale * 0.08);
    playFilteredNoise(v, {
        color: variant === 2 ? "brown" : "pink",
        attack: 0.003,
        decay: 0.065,
        gain: 0.09 + scale * 0.05,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 1050 + variant * 260, 0.18),
        filterFreqEnd: 520,
        filterQ: 1.4,
    });
}

/**
 * `LandCombat` — Short battlefield clash differentiated by opponent archetype.
 * `subtype`: Tribe (wood/hide drum), Nation (steel/bronze hit), Human (dual tense clash), Counter.
 */
export function synthLandCombat(v: VoiceRenderCtx, t: CueTrigger): void {
    const scale = clamp(t.intensity, 0.2, 1.0);
    const variant = Math.floor(v.rng() * 3);
    const kind = Math.round(t.subtype);

    // Base impact weight
    const bodyHz = vary(v.rng, kind === CombatAudioSubtype.Human ? 148 : 128, 0.09);
    playSubDrop(v, bodyHz, bodyHz * 0.42, 0.1 + scale * 0.05, 0.2 + scale * 0.12);

    if (kind === CombatAudioSubtype.Tribe) {
        // Dry wood-and-hide skirmish percussion
        playModalHit(v, {
            baseFreq: vary(v.rng, 260 + variant * 35, 0.08),
            ratios: [1, 1.62, 2.54],
            gains: [0.62, 0.26, 0.12],
            decay: 0.075,
            gain: 0.16 + scale * 0.06,
        });
    } else if (kind === CombatAudioSubtype.Nation || kind === CombatAudioSubtype.Human) {
        // Metallic steel/shield clash with rotated inharmonic ratio per perspective
        const metalFreq = vary(v.rng, kind === CombatAudioSubtype.Human ? 640 : 490, 0.12);
        const modRatio = variant === 0 ? 1.414 : variant === 1 ? 1.732 : 2.13;
        playFmTone(v, {
            carrierFreq: metalFreq,
            carrierFreqEnd: metalFreq * 0.86,
            modRatio,
            modIndex: 1.5 + scale * 0.9,
            modIndexEnd: 0.15,
            attack: 0.002,
            decay: 0.09 + scale * 0.05,
            gain: 0.14 + scale * 0.07,
        });
        if (kind === CombatAudioSubtype.Human) {
            // Secondary quick counter-clash for PvP intensity
            playFilteredNoise(v, {
                color: "white",
                delay: 0.022 + v.rng() * 0.015,
                attack: 0.002,
                decay: 0.055,
                gain: 0.12 + scale * 0.06,
                filterType: "bandpass",
                filterFreq: vary(v.rng, 2600, 0.2),
                filterQ: 2.2,
            });
        }
    } else {
        // Generic crisp tactical scuff
        playFilteredNoise(v, {
            color: "pink",
            attack: 0.003,
            decay: 0.085,
            gain: 0.15 + scale * 0.07,
            filterType: "bandpass",
            filterFreq: vary(v.rng, 1400, 0.18),
            filterFreqEnd: 600,
            filterQ: 1.5,
        });
    }
}

/** `CounterAttack` — Snappy upward metallic snap signaling immediate counter-push. */
export function synthCounterAttack(v: VoiceRenderCtx, _t: CueTrigger): void {
    const f0 = vary(v.rng, 392.0, 0.03);
    playSubDrop(v, 140, 68, 0.11, 0.28);
    playFmTone(v, {
        carrierFreq: f0,
        carrierFreqEnd: f0 * 1.5,
        modRatio: 1.5,
        modIndex: 1.1,
        modIndexEnd: 0.2,
        attack: 0.004,
        decay: 0.16,
        gain: 0.22,
    });
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.06,
        gain: 0.14,
        filterType: "highpass",
        filterFreq: 2100,
        filterFreqEnd: 4200,
    });
}

/** `AttackCancelled` — Short soft pullback descent. */
export function synthAttackCancelled(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTone(v, {
        type: "triangle",
        freq: vary(v.rng, 392.0, 0.04),
        freqEnd: 246.94,
        attack: 0.004,
        decay: 0.11,
        gain: 0.18,
        filterType: "lowpass",
        filterFreq: 1200,
        filterFreqEnd: 420,
    });
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.003,
        decay: 0.08,
        gain: 0.09,
        filterType: "bandpass",
        filterFreq: 950,
        filterFreqEnd: 480,
    });
}

/**
 * `CascadeStep` — Ascending D-Pentatonic modal chime for tactical surges,
 * captured structures, and territory advance milestones.
 * `subtype`: chain step `1..8`.
 */
export function synthCascadeStep(v: VoiceRenderCtx, t: CueTrigger): void {
    const step = clamp(Math.round(t.subtype), 1, 8);
    const noteIdx = clamp(step + 2, 3, PENTATONIC_D.length - 1);
    const freq = PENTATONIC_D[noteIdx];
    const intensityBoost = 0.22 + (step - 1) * 0.025;

    // Warm resonant modal bell
    playModalHit(v, {
        baseFreq: freq,
        ratios: [1, 2.0, 3.005],
        gains: [0.68, 0.24, 0.08],
        decay: 0.22 + step * 0.02,
        gain: intensityBoost,
    });

    // From step 3+, layer a sparkling fifth shimmer
    if (step >= 3) {
        playTone(v, {
            type: "sine",
            freq: freq * 1.5,
            delay: 0.024,
            attack: 0.005,
            decay: 0.2,
            gain: 0.1 + step * 0.01,
        });
    }

    // High chain momentum adds subtle grounded sub-pulse
    if (step >= 6) {
        playSubDrop(v, 118, 58, 0.16, 0.2, 0);
    }
}

/**
 * `KillStreak` — Heroic multi-kill / streak fanfare stab (`x2`, `x3`, `KING!`, `RAMPAGE!`).
 * `subtype`: chain/tier step `2..8`.
 */
export function synthKillStreak(v: VoiceRenderCtx, t: CueTrigger): void {
    const tier = clamp(Math.round(t.subtype), 2, 8);
    const baseIdx = clamp(tier + 1, 3, PENTATONIC_D.length - 3);
    const notes =
        tier >= 6
            ? [PENTATONIC_D[baseIdx], PENTATONIC_D[baseIdx + 1], PENTATONIC_D[baseIdx + 2]]
            : [PENTATONIC_D[baseIdx], PENTATONIC_D[baseIdx + 2]];

    playSubDrop(v, 130, 52, 0.22, 0.3 + tier * 0.02);
    notes.forEach((freq, idx) => {
        const delay = idx * 0.055;
        playModalHit(v, {
            baseFreq: freq,
            ratios: [1, 2.0, 2.99, 4.02],
            gains: [0.58, 0.26, 0.1, 0.06],
            decay: 0.32 + idx * 0.07,
            delay,
            gain: 0.2 + tier * 0.015,
        });
    });
    if (v.wet) {
        playTone(v, {
            type: "triangle",
            freq: notes[notes.length - 1] * 0.5,
            delay: 0.04,
            attack: 0.01,
            decay: 0.38,
            gain: 0.16,
            dest: v.wet,
        });
    }
}

/**
 * `PlayerEliminated` — Dramatic fall differentiated by victim archetype.
 * `subtype`: 0 = Bot/Tribe, 1 = Nation, 2 = Human player.
 */
export function synthPlayerEliminated(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    const isHuman = kind >= 2;
    const isNation = kind === 1;
    const root = vary(v.rng, isHuman ? 146.83 : isNation ? 164.81 : 185.0, 0.03); // D3 / E3 / F#3

    // Deep impact thud
    playSubDrop(v, isHuman ? 115 : 98, isHuman ? 34 : 44, isHuman ? 0.42 : 0.28, isHuman ? 0.48 : 0.36);

    // Falling bronze / horn resonance
    playTone(v, {
        type: "sawtooth",
        freq: root * 2,
        freqEnd: root,
        attack: 0.008,
        decay: isHuman ? 0.55 : isNation ? 0.38 : 0.24,
        gain: isHuman ? 0.26 : 0.2,
        filterType: "lowpass",
        filterFreq: isHuman ? 1800 : 1200,
        filterFreqEnd: 240,
        filterQ: 1.8,
    });

    // Shattering metallic seal for Nation & Human falls
    if (isNation || isHuman) {
        playFmTone(v, {
            carrierFreq: vary(v.rng, 320, 0.06),
            carrierFreqEnd: 170,
            modRatio: 1.618,
            modIndex: isHuman ? 2.4 : 1.6,
            modIndexEnd: 0.2,
            delay: 0.025,
            decay: isHuman ? 0.45 : 0.28,
            gain: isHuman ? 0.2 : 0.14,
        });
    }

    if (v.wet && (isHuman || isNation)) {
        playFilteredNoise(v, {
            color: "brown",
            delay: 0.02,
            attack: 0.01,
            decay: isHuman ? 0.5 : 0.3,
            gain: isHuman ? 0.22 : 0.14,
            filterType: "lowpass",
            filterFreq: 600,
            dest: v.wet,
        });
    }
}
