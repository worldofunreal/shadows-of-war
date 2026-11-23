/**
 * Match Lifecycle, Tactile Errors, Victory & Defeat Recipes (`match.ts`).
 *
 * Acoustic Identity (Blockbuster Strategy / Supercell / Royal Triumph):
 *   - `MatchPhaseStart`: War-room parchment & marble setup (`Spawning`), erupting
 *     into cinema taiko rolls, stereo orchestral war horns, and gleaming steel (`Playing`).
 *   - `ActionError`: Soft tactile double wooden *bup-bup* (zero buzzer fatigue).
 *   - `Victory`: Full AAA triumphant D Major9 orchestral brass + Karplus-Strong harp
 *     glissando + casino gold coin shower + velvet choir in the 1.65 s golden hall.
 *   - `Defeat`: Cinematic solemn D-minor orchestral brass & choir descent over deep
 *     funeral war-drums and fading arena wind.
 */

import {
    playFilteredNoise,
    type VoiceRenderCtx,
} from "../dsp.ts";
import {
    playBrassEnsemble,
    playCasinoChime,
    playChoirPad,
    playCoinShower,
    playHarpGliss,
    playSteelImpact,
    playStereoWhoosh,
    playTactilePop,
    playTaikoHit,
} from "../instruments.ts";
import { MatchPhaseAudioKind, type CueTrigger } from "../types.ts";

/** `MatchPhaseStart` — `subtype`: 0 = Spawning begins, 1 = Battle starts (`Playing`). */
export function synthMatchPhaseStart(v: VoiceRenderCtx, t: CueTrigger): void {
    const isPlaying = Math.round(t.subtype) === MatchPhaseAudioKind.Playing;

    if (!isPlaying) {
        // Spawning phase opens: war-map unfurl + marble piece placement + inviting harp triad
        playStereoWhoosh(v, { direction: 1, decay: 0.22, gain: 0.24, bright: false });
        playTactilePop(v, { material: "stone", pitch: 0.94, delay: 0.015, gain: 0.36 });
        playHarpGliss(v, [293.66, 440.0, 587.33], {
            delay: 0.03,
            stepSec: 0.065,
            gain: 0.28,
            decay: 0.72,
            panStart: -0.35,
            panEnd: 0.35,
            shimmerSend: 0.56,
        });
        return;
    }

    // Playing phase opens: war-taiko roll + heroic stereo war horns + steel flash!
    playTaikoHit(v, { heavy: false, pitch: 0.94, delay: 0, gain: 0.44, rimCrack: 0.5 });
    playTaikoHit(v, { heavy: false, pitch: 1.06, delay: 0.11, gain: 0.48, rimCrack: 0.65 });
    playTaikoHit(v, { heavy: true, pitch: 0.96, delay: 0.24, gain: 0.72, rimCrack: 0.85, wetSend: 0.54 });
    playSteelImpact(v, { heavy: false, pitch: 1.24, delay: 0.245, gain: 0.32, wetSend: 0.45 });

    const hornNotes = [
        { f: 146.83, d: 0.03, len: 0.28, br: 1.0 }, // D3
        { f: 220.0, d: 0.14, len: 0.28, br: 1.15 }, // A3
        { f: 293.66, d: 0.25, len: 0.76, br: 1.35 }, // D4 rallying roar
    ];
    for (const n of hornNotes) {
        playBrassEnsemble(v, {
            freq: n.f,
            delay: n.d,
            attack: 0.026,
            hold: 0.06,
            decay: n.len,
            gain: n.f > 250 ? 0.46 : 0.34,
            brightness: n.br,
            width: 0.45,
            goldenHall: true,
        });
    }
    playCasinoChime(v, { freq: 587.33, delay: 0.26, decay: 0.64, gain: 0.24, sparkle: 0.7 });
    playCasinoChime(v, { freq: 880.0, delay: 0.31, decay: 0.68, gain: 0.22, sparkle: 0.8, pan: 0.3 });
}

/** `ActionError` — Supercell-style tactile double carved-wood *bup-bup* (zero harsh buzzer). */
export function synthActionError(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTactilePop(v, {
        material: "wood",
        pitch: 0.76,
        popBend: 0.84,
        noteHz: 220.0,
        gain: 0.38,
        shimmerSend: 0.05,
    });
    playTactilePop(v, {
        material: "wood",
        pitch: 0.64,
        popBend: 0.78,
        noteHz: 185.0,
        delay: 0.072,
        gain: 0.36,
        shimmerSend: 0.05,
    });
}

/** `Victory` — Full AAA Royal Triumph & Mega-Jackpot Fanfare with 2 seeded arrangements! */
export function synthVictory(v: VoiceRenderCtx, _t: CueTrigger): void {
    const alt = v.rng() > 0.5;

    // Rolling cinema timpani crescendo into triumphant double-slam
    playTaikoHit(v, { heavy: false, pitch: 0.92, delay: 0, gain: 0.38 });
    playTaikoHit(v, { heavy: false, pitch: 1.0, delay: 0.09, gain: 0.44 });
    playTaikoHit(v, { heavy: false, pitch: 1.08, delay: 0.18, gain: 0.5, rimCrack: 0.6 });
    playTaikoHit(v, { heavy: true, pitch: 0.96, delay: 0.36, gain: 0.74, rimCrack: 0.85, wetSend: 0.55 });

    // Two noble D Major9 orchestral brass contours
    const brassPhrase = alt
        ? [
              { f: 293.66, d: 0.02, len: 0.24, br: 1.1 }, // D4
              { f: 369.99, d: 0.13, len: 0.24, br: 1.2 }, // F#4
              { f: 440.0, d: 0.24, len: 0.28, br: 1.3 }, // A4
              { f: 587.33, d: 0.37, len: 0.98, br: 1.48 }, // D5 crown!
          ]
        : [
              { f: 293.66, d: 0.02, len: 0.22, br: 1.1 }, // D4
              { f: 440.0, d: 0.13, len: 0.22, br: 1.2 }, // A4
              { f: 587.33, d: 0.24, len: 0.24, br: 1.32 }, // D5
              { f: 659.25, d: 0.36, len: 0.22, br: 1.4 }, // E5 (golden 9th lift!)
              { f: 739.99, d: 0.48, len: 0.96, br: 1.5 }, // F#5 glorious peak!
          ];

    for (const note of brassPhrase) {
        playBrassEnsemble(v, {
            freq: note.f,
            delay: note.d,
            attack: 0.024,
            hold: 0.05,
            decay: note.len,
            gain: 0.36,
            brightness: note.br,
            width: 0.46,
            goldenHall: true,
        });
    }

    // Bass brass foundation on the climax downbeat
    const climaxAt = alt ? 0.36 : 0.47;
    playBrassEnsemble(v, {
        freq: 146.83, // D3 heroic pedal
        delay: climaxAt,
        attack: 0.03,
        hold: 0.15,
        decay: 1.05,
        gain: 0.38,
        brightness: 1.1,
        goldenHall: true,
    });

    // Sweeping golden Karplus-Strong harp glissando across the full stereo field!
    playHarpGliss(v, [293.66, 369.99, 440.0, 587.33, 659.25, 739.99, 880.0, 1174.66, 1479.98], {
        delay: climaxAt - 0.05,
        stepSec: 0.032,
        gain: 0.34,
        decay: 1.0,
        panStart: -0.68,
        panEnd: 0.68,
        shimmerSend: 0.72,
    });

    // Casino gold jackpot coin shower + shimmering celesta crown chord
    playCoinShower(v, {
        count: 7,
        delay: climaxAt + 0.02,
        spacing: 0.046,
        pitch: 1.04,
        pitchClimb: 0.042,
        gain: 0.28,
        spread: 0.72,
    });
    [587.33, 739.99, 880.0, 1174.66, 1318.51].forEach((freq, i) => {
        playCasinoChime(v, {
            freq,
            delay: climaxAt + i * 0.042,
            decay: 0.96,
            gain: 0.24,
            sparkle: 0.95,
            pan: -0.5 + i * 0.25,
            shimmerSend: 0.78,
        });
    });

    // Lush velvet royal choir sustain
    playChoirPad(v, [293.66, 440.0, 587.33, 739.99, 880.0], {
        delay: climaxAt,
        attack: 0.11,
        decay: 1.25,
        gain: 0.32,
        goldenHall: true,
    });
}

/** `Defeat` — Cinematic solemn D-minor orchestral brass & choir fall over funeral drums. */
export function synthDefeat(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Deep solemn funeral odaiko strokes
    playTaikoHit(v, { heavy: true, pitch: 0.74, delay: 0, gain: 0.6, wetSend: 0.58 });
    playTaikoHit(v, { heavy: true, pitch: 0.66, delay: 0.62, gain: 0.54, wetSend: 0.62 });

    const notes = [
        { f: 440.0, d: 0.02, len: 0.34, br: 0.92 }, // A4
        { f: 349.23, d: 0.22, len: 0.36, br: 0.85 }, // F4
        { f: 329.63, d: 0.42, len: 0.34, br: 0.78 }, // E4
        { f: 293.66, d: 0.63, len: 0.98, br: 0.72 }, // D4
    ];
    for (const n of notes) {
        playBrassEnsemble(v, {
            freq: n.f,
            freqEnd: n.f * 0.986,
            delay: n.d,
            attack: 0.036,
            hold: 0.05,
            decay: n.len,
            gain: 0.3,
            brightness: n.br,
            width: 0.42,
            goldenHall: false,
        });
    }

    // Deep tragic D3 bass horn + dark velvet choir resolution
    playBrassEnsemble(v, {
        freq: 146.83, // D3
        freqEnd: 144.0,
        delay: 0.62,
        attack: 0.06,
        hold: 0.12,
        decay: 1.05,
        gain: 0.36,
        brightness: 0.68,
        width: 0.5,
        goldenHall: false,
    });
    playChoirPad(v, [146.83, 220.0, 293.66, 349.23], {
        delay: 0.56,
        attack: 0.14,
        decay: 1.15,
        gain: 0.28,
        filterFreq: 1120,
        goldenHall: false,
    });

    // Fading battlefield dusk wind
    playFilteredNoise(v, {
        color: "parchment",
        delay: 0.4,
        attack: 0.18,
        decay: 1.0,
        gain: 0.16,
        filterType: "bandpass",
        filterFreq: 540,
        filterFreqEnd: 210,
        filterQ: 1.4,
        wetSend: 0.55,
    });
}
