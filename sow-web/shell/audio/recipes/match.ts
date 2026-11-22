/**
 * Match Lifecycle, Errors, Victory & Defeat Procedural Recipes.
 *
 * Acoustic Identity:
 *   - `MatchPhaseStart`: Atmospheric deployment pulse (`Spawning`) and stirring
 *     D-Dorian war horn + kettle drum when battle opens (`Playing`).
 *   - `ActionError`: Soft damped double-thud that signals invalid/insufficient action
 *     clearly without harsh buzzer fatigue.
 *   - `Victory`: Heroic D-Major brass & modal bell fanfare with 2 seeded phrasing
 *     variants so repeated victories feel fresh.
 *   - `Defeat`: Solemn D-Minor descending cadence with warming low-pass closure.
 */

import {
    playFilteredNoise,
    playModalHit,
    playSubDrop,
    playTone,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import { MatchPhaseAudioKind, type CueTrigger } from "../types.ts";

/** `MatchPhaseStart` — `subtype`: 0 = Spawning begins, 1 = Battle starts (`Playing`). */
export function synthMatchPhaseStart(v: VoiceRenderCtx, t: CueTrigger): void {
    const isPlaying = Math.round(t.subtype) === MatchPhaseAudioKind.Playing;
    if (!isPlaying) {
        // Spawning phase opens: warm tactical map settle + calm fifth
        playSubDrop(v, 110, 58, 0.24, 0.24);
        playModalHit(v, {
            baseFreq: 293.66,
            ratios: [1, 1.5, 2.0],
            gains: [0.55, 0.3, 0.15],
            decay: 0.45,
            delay: 0.02,
            gain: 0.18,
        });
        return;
    }

    // Playing phase opens: war drum + rallying horn (D3 -> A3 -> D4)
    playSubDrop(v, 128, 46, 0.32, 0.42);
    playSubDrop(v, 146, 54, 0.24, 0.3, 0.13);
    const notes = [146.83, 220.0, 293.66];
    notes.forEach((freq, idx) => {
        const delay = 0.04 + idx * 0.12;
        playTone(v, {
            type: "sawtooth",
            freq,
            attack: 0.028,
            decay: idx === 2 ? 0.58 : 0.24,
            gain: idx === 2 ? 0.22 : 0.16,
            filterType: "lowpass",
            filterFreq: idx === 2 ? 1450 : 960,
            filterFreqEnd: 420,
            filterQ: 1.6,
        });
    });
    if (v.wet) {
        playModalHit(v, {
            baseFreq: 587.33,
            ratios: [1, 2.0],
            gains: [0.65, 0.35],
            decay: 0.6,
            delay: 0.28,
            gain: 0.18,
            dest: v.wet,
        });
    }
}

/** `ActionError` — Compact, non-abrasive damped double-thud for rejected actions. */
export function synthActionError(v: VoiceRenderCtx, _t: CueTrigger): void {
    const base = vary(v.rng, 168, 0.04);
    playTone(v, {
        type: "triangle",
        freq: base,
        freqEnd: base * 0.78,
        attack: 0.003,
        decay: 0.065,
        gain: 0.2,
        filterType: "lowpass",
        filterFreq: 640,
    });
    playTone(v, {
        type: "triangle",
        freq: base * 0.84,
        freqEnd: base * 0.64,
        delay: 0.062,
        attack: 0.003,
        decay: 0.085,
        gain: 0.18,
        filterType: "lowpass",
        filterFreq: 520,
    });
}

/** `Victory` — Noble D-Major brass & modal bell fanfare with seeded phrasing variations. */
export function synthVictory(v: VoiceRenderCtx, _t: CueTrigger): void {
    const alt = v.rng() > 0.5;
    // Grounding timpani rolls
    playSubDrop(v, 118, 46, 0.34, 0.42, 0);
    playSubDrop(v, 132, 48, 0.46, 0.46, 0.34);

    // Two noble D-Major melodic contours
    const phrase = alt
        ? [
              { f: 293.66, d: 0.02, len: 0.28 }, // D4
              { f: 369.99, d: 0.13, len: 0.26 }, // F#4
              { f: 440.0, d: 0.24, len: 0.3 }, // A4
              { f: 587.33, d: 0.36, len: 0.85 }, // D5
              { f: 739.99, d: 0.4, len: 0.82 }, // F#5
              { f: 880.0, d: 0.44, len: 0.88 }, // A5
          ]
        : [
              { f: 293.66, d: 0.02, len: 0.24 }, // D4
              { f: 440.0, d: 0.12, len: 0.24 }, // A4
              { f: 587.33, d: 0.22, len: 0.26 }, // D5
              { f: 659.25, d: 0.33, len: 0.22 }, // E5
              { f: 739.99, d: 0.44, len: 0.86 }, // F#5
              { f: 1174.66, d: 0.48, len: 0.9 }, // D6
          ];

    for (const note of phrase) {
        // Warm brass pad layer
        playTone(v, {
            type: "sawtooth",
            freq: note.f,
            delay: note.d,
            attack: 0.024,
            decay: note.len,
            gain: 0.13,
            filterType: "lowpass",
            filterFreq: 1950,
            filterFreqEnd: 720,
            filterQ: 1.2,
        });
        // Resonant modal bell attack
        playModalHit(v, {
            baseFreq: note.f,
            ratios: [1, 2.0, 3.0],
            gains: [0.6, 0.28, 0.12],
            decay: note.len * 0.95,
            delay: note.d,
            gain: 0.15,
        });
    }

    if (v.wet) {
        [293.66, 440.0, 587.33].forEach((freq) => {
            playTone(v, {
                type: "triangle",
                freq,
                delay: 0.36,
                attack: 0.06,
                decay: 0.95,
                gain: 0.12,
                dest: v.wet!,
            });
        });
    }
    playFilteredNoise(v, {
        color: "pink",
        delay: 0.34,
        attack: 0.02,
        decay: 0.45,
        gain: 0.12,
        filterType: "highpass",
        filterFreq: 2800,
    });
}

/** `Defeat` — Solemn D-Minor descending cadence (`A4 → F4 → E4 → D4`). */
export function synthDefeat(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSubDrop(v, 96, 36, 0.55, 0.4, 0);
    const notes = [
        { f: 440.0, d: 0.02, len: 0.34 }, // A4
        { f: 349.23, d: 0.22, len: 0.36 }, // F4
        { f: 329.63, d: 0.42, len: 0.34 }, // E4
        { f: 293.66, d: 0.62, len: 0.85 }, // D4
        { f: 146.83, d: 0.64, len: 0.9 }, // D3 bass anchor
    ];
    for (const note of notes) {
        playTone(v, {
            type: "sawtooth",
            freq: note.f,
            freqEnd: note.f * 0.985,
            delay: note.d,
            attack: 0.032,
            decay: note.len,
            gain: 0.14,
            filterType: "lowpass",
            filterFreq: 1050,
            filterFreqEnd: 240,
            filterQ: 1.1,
        });
        playTone(v, {
            type: "triangle",
            freq: note.f,
            delay: note.d,
            attack: 0.02,
            decay: note.len * 0.9,
            gain: 0.12,
        });
    }
    if (v.wet) {
        playTone(v, {
            type: "triangle",
            freq: 146.83,
            delay: 0.6,
            attack: 0.08,
            decay: 0.95,
            gain: 0.16,
            dest: v.wet,
        });
    }
}
