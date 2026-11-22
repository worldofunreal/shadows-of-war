/**
 * GUI, HUD & UX Micro-Interaction Procedural Recipes.
 *
 * Acoustic Identity:
 *   Clean, low-fatigue tactile instruments tuned to D-Pentatonic/Dorian so rapid
 *   navigation, sliders, radial menus, toasts, store unlocks, and reward
 *   choreographies feel crisp, premium, and cohesive with in-match SFX.
 *   All UI cues route through `uiBus` (subtle level, zero harsh square beeps).
 */

import {
    clamp,
    PENTATONIC_D,
    playFilteredNoise,
    playFmTone,
    playModalHit,
    playSubDrop,
    playTone,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import type { CueTrigger } from "../types.ts";

/** `UiClick` — Crisp tactile key/stone micro-click (`subtype`: 0=default, 1=primary CTA, 2=back/cancel). */
export function synthUiClick(v: VoiceRenderCtx, t: CueTrigger): void {
    const variant = Math.round(t.subtype);
    const freq =
        variant === 1
            ? vary(v.rng, 698.46, 0.025)
            : variant === 2
              ? vary(v.rng, 440.0, 0.025)
              : vary(v.rng, 587.33, 0.03);

    playModalHit(v, {
        baseFreq: freq,
        ratios: [1, 1.92],
        gains: [0.72, 0.28],
        decay: variant === 1 ? 0.055 : 0.038,
        gain: variant === 1 ? 0.16 : 0.12,
    });
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.018,
        gain: 0.05,
        filterType: "bandpass",
        filterFreq: vary(v.rng, variant === 2 ? 1700 : 2800, 0.15),
        filterQ: 2.0,
    });
}

/** `UiTabSwitch` — Light directional air slide + soft tonal tap (`subtype`: 0=neutral, 1=forward, 2=back). */
export function synthUiTabSwitch(v: VoiceRenderCtx, t: CueTrigger): void {
    const dir = Math.round(t.subtype);
    const startHz = dir === 2 ? 620 : 480;
    const endHz = dir === 2 ? 440 : 660;
    playTone(v, {
        type: "sine",
        freq: vary(v.rng, startHz, 0.02),
        freqEnd: endHz,
        attack: 0.004,
        decay: 0.065,
        gain: 0.11,
    });
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.005,
        decay: 0.055,
        gain: 0.07,
        filterType: "bandpass",
        filterFreq: dir === 2 ? 1500 : 1100,
        filterFreqEnd: dir === 2 ? 900 : 1850,
        filterQ: 1.8,
    });
}

/** `UiModalOpen` — Soft lifting chime + gentle paper/air swell. */
export function synthUiModalOpen(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTone(v, {
        type: "triangle",
        freq: 440.0,
        freqEnd: 587.33,
        attack: 0.008,
        decay: 0.09,
        gain: 0.12,
        filterType: "lowpass",
        filterFreq: 1800,
    });
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.006,
        decay: 0.07,
        gain: 0.06,
        filterType: "bandpass",
        filterFreq: 1000,
        filterFreqEnd: 2100,
        filterQ: 1.6,
    });
}

/** `UiModalClose` — Soft inverted closing breath. */
export function synthUiModalClose(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTone(v, {
        type: "triangle",
        freq: 523.25,
        freqEnd: 349.23,
        attack: 0.005,
        decay: 0.075,
        gain: 0.11,
        filterType: "lowpass",
        filterFreq: 1400,
        filterFreqEnd: 620,
    });
}

/** `UiRadialOpen` — Crisp tactical compass/wheel click opening on the map. */
export function synthUiRadialOpen(v: VoiceRenderCtx, _t: CueTrigger): void {
    playFmTone(v, {
        carrierFreq: vary(v.rng, 587.33, 0.02),
        carrierFreqEnd: 783.99,
        modRatio: 2.0,
        modIndex: 0.5,
        attack: 0.003,
        decay: 0.075,
        gain: 0.13,
    });
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.022,
        gain: 0.06,
        filterType: "bandpass",
        filterFreq: 3200,
        filterQ: 2.4,
    });
}

/** `UiRadialClose` — Subtle retracting compass latch. */
export function synthUiRadialClose(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTone(v, {
        type: "sine",
        freq: 523.25,
        freqEnd: 349.23,
        attack: 0.003,
        decay: 0.055,
        gain: 0.09,
    });
}

/** `UiSliderTick` — Mechanical detent tick whose pitch tracks slider value (`intensity` ∈ [0..1]). */
export function synthUiSliderTick(v: VoiceRenderCtx, t: CueTrigger): void {
    const ratio = clamp(t.intensity, 0, 1);
    const freq = 420 + ratio * 460;
    playModalHit(v, {
        baseFreq: vary(v.rng, freq, 0.012),
        ratios: [1, 2.0],
        gains: [0.75, 0.25],
        decay: 0.032,
        gain: 0.11,
    });
}

/** `UiToast` — HUD notification ping scaled by priority (`subtype`: 1..4). */
export function synthUiToast(v: VoiceRenderCtx, t: CueTrigger): void {
    const prio = clamp(Math.round(t.subtype), 1, 4);
    const idx = prio >= 4 ? 6 : prio === 3 ? 5 : 4;
    const freq = PENTATONIC_D[idx];
    playModalHit(v, {
        baseFreq: freq,
        ratios: [1, 2.0],
        gains: [0.7, 0.3],
        decay: 0.15,
        gain: 0.11 + prio * 0.015,
    });
    if (prio >= 3) {
        playModalHit(v, {
            baseFreq: PENTATONIC_D[idx + 1],
            ratios: [1, 2.0],
            gains: [0.7, 0.3],
            decay: 0.16,
            delay: 0.065,
            gain: 0.11 + prio * 0.015,
        });
    }
}

/** `UiMatchFound` — Firm resonant matchmaking ready cue. */
export function synthUiMatchFound(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSubDrop(v, 130, 56, 0.22, 0.28);
    [440.0, 587.33, 880.0].forEach((freq, idx) => {
        playModalHit(v, {
            baseFreq: freq,
            ratios: [1, 2.0, 3.01],
            gains: [0.62, 0.26, 0.12],
            decay: 0.32 + idx * 0.05,
            delay: idx * 0.065,
            gain: 0.18,
        });
    });
}

/** `UiPurchaseSuccess` — Sparkling crystalline store unlock chord. */
export function synthUiPurchaseSuccess(v: VoiceRenderCtx, _t: CueTrigger): void {
    [587.33, 739.99, 880.0, 1174.66].forEach((freq, idx) => {
        playModalHit(v, {
            baseFreq: freq,
            ratios: [1, 2.0, 3.02],
            gains: [0.58, 0.28, 0.14],
            decay: 0.34 + idx * 0.05,
            delay: idx * 0.05,
            gain: 0.17,
        });
    });
    if (v.wet) {
        playTone(v, {
            type: "sine",
            freq: 1174.66,
            delay: 0.14,
            attack: 0.02,
            decay: 0.45,
            gain: 0.14,
            dest: v.wet,
        });
    }
}

/** `UiHeroEquip` — Satisfying seal/armor lock when selecting a leader or skin. */
export function synthUiHeroEquip(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSubDrop(v, 150, 68, 0.12, 0.22);
    playModalHit(v, {
        baseFreq: vary(v.rng, 587.33, 0.015),
        ratios: [1, 1.5, 2.02],
        gains: [0.6, 0.25, 0.15],
        decay: 0.2,
        delay: 0.015,
        gain: 0.17,
    });
}

/** `UiCampaignStep` — Subtle dispatch/parchment click on campaign dialogue progression. */
export function synthUiCampaignStep(v: VoiceRenderCtx, _t: CueTrigger): void {
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.004,
        decay: 0.048,
        gain: 0.075,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 1650, 0.14),
        filterFreqEnd: 920,
        filterQ: 1.6,
    });
    playTone(v, {
        type: "sine",
        freq: vary(v.rng, 587.33, 0.02),
        attack: 0.003,
        decay: 0.045,
        gain: 0.07,
    });
}

/** `UiCelebrationFanfare` — Rich procedural replacement for mission/reward fanfare. */
export function synthUiCelebrationFanfare(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSubDrop(v, 98, 44, 0.36, 0.34);
    const notes = [293.66, 440.0, 587.33, 739.99, 880.0, 1174.66];
    notes.forEach((freq, idx) => {
        playModalHit(v, {
            baseFreq: freq,
            ratios: [1, 2.0, 3.01],
            gains: [0.6, 0.28, 0.12],
            decay: 0.42 + idx * 0.04,
            delay: idx * 0.072,
            gain: 0.16,
        });
    });
    if (v.wet) {
        playTone(v, {
            type: "triangle",
            freq: 587.33,
            delay: 0.15,
            attack: 0.04,
            decay: 0.65,
            gain: 0.14,
            dest: v.wet,
        });
    }
}

/** `UiCelebrationFly` — Light ascending arc whoosh as reward cards fly to counters. */
export function synthUiCelebrationFly(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTone(v, {
        type: "sine",
        freq: vary(v.rng, 440, 0.04),
        freqEnd: vary(v.rng, 1174.66, 0.04),
        attack: 0.012,
        decay: 0.32,
        gain: 0.1,
    });
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.01,
        decay: 0.24,
        gain: 0.06,
        filterType: "bandpass",
        filterFreq: 800,
        filterFreqEnd: 2600,
        filterQ: 2.0,
    });
}

/** `UiCelebrationImpact` — Sparkling counter impact when a reward card lands. */
export function synthUiCelebrationImpact(v: VoiceRenderCtx, _t: CueTrigger): void {
    playModalHit(v, {
        baseFreq: vary(v.rng, 1174.66, 0.03),
        ratios: [1, 1.5, 2.02],
        gains: [0.58, 0.28, 0.14],
        decay: 0.22,
        gain: 0.17,
    });
    playSubDrop(v, 165, 82, 0.08, 0.14);
}
