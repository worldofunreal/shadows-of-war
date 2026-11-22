/**
 * Diplomacy, Betrayal, Resource Transfers, Hero Abilities & Emotes Recipes.
 *
 * Acoustic Identity:
 *   Warm D-Dorian harp/bell intervals for alliances & aid, dramatic dissonant bronze
 *   stabs for betrayals, hero-family-tinted harmonic blooms for leader abilities, and
 *   concise expressive vocal/tonal gestures for map emotes.
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
import {
    AbilityAudioFamily,
    EmoteAudioKind,
    ResourceAudioSubtype,
    type CueTrigger,
} from "../types.ts";

/** `AllianceRequest` — Warm inviting harp/bell fifth (`D5 → A5`). */
export function synthAllianceRequest(v: VoiceRenderCtx, _t: CueTrigger): void {
    const notes = [587.33, 880.0];
    notes.forEach((freq, idx) => {
        playModalHit(v, {
            baseFreq: vary(v.rng, freq, 0.008),
            ratios: [1, 2.0, 3.0],
            gains: [0.68, 0.24, 0.08],
            decay: 0.28 + idx * 0.08,
            delay: idx * 0.085,
            gain: 0.18,
        });
    });
}

/** `AllianceFormed` — Consistant warm D-Major covenant chord (`D4, A4, D5, F#5`). */
export function synthAllianceFormed(v: VoiceRenderCtx, _t: CueTrigger): void {
    const notes = [293.66, 440.0, 587.33, 739.99];
    notes.forEach((freq, idx) => {
        playModalHit(v, {
            baseFreq: freq,
            ratios: [1, 2.0, 3.01],
            gains: [0.62, 0.26, 0.12],
            decay: 0.42 + idx * 0.06,
            delay: idx * 0.048,
            gain: 0.17,
        });
    });
    if (v.wet) {
        playTone(v, {
            type: "triangle",
            freq: 293.66,
            attack: 0.04,
            decay: 0.52,
            gain: 0.14,
            dest: v.wet,
        });
    }
}

/** `AllianceEnded` — Brief, neutral descending cadence (`A4 → D4`). */
export function synthAllianceEnded(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTone(v, {
        type: "triangle",
        freq: 440.0,
        attack: 0.006,
        decay: 0.18,
        gain: 0.16,
        filterType: "lowpass",
        filterFreq: 1400,
    });
    playTone(v, {
        type: "triangle",
        freq: 293.66,
        delay: 0.1,
        attack: 0.008,
        decay: 0.24,
        gain: 0.17,
        filterType: "lowpass",
        filterFreq: 1000,
    });
}

/** `Betrayal` — Dramatic dissonant bronze stab (D4 + Eb4 + Ab4) + heavy drum hit. */
export function synthBetrayal(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSubDrop(v, 118, 36, 0.36, 0.46);
    // Dissonant blade-draw bronze cluster
    const cluster = [293.66, 311.13, 415.3]; // D4, Eb4, Ab4
    cluster.forEach((freq, idx) => {
        playTone(v, {
            type: "sawtooth",
            freq,
            freqEnd: freq * 0.96,
            delay: idx * 0.015,
            attack: 0.006,
            decay: 0.48,
            gain: 0.16,
            filterType: "lowpass",
            filterFreq: 2200,
            filterFreqEnd: 340,
            filterQ: 2.2,
        });
    });
    // Steel scrape transient
    playFmTone(v, {
        carrierFreq: 740,
        carrierFreqEnd: 460,
        modRatio: 1.618,
        modIndex: 2.8,
        modIndexEnd: 0.3,
        attack: 0.003,
        decay: 0.24,
        gain: 0.18,
    });
    if (v.wet) {
        playTone(v, {
            type: "sawtooth",
            freq: 146.83,
            attack: 0.02,
            decay: 0.55,
            gain: 0.16,
            filterType: "lowpass",
            filterFreq: 640,
            dest: v.wet,
        });
    }
}

/**
 * `ResourceTransfer` — Aid sent, aid received, request, declined, or emergency help call.
 * `subtype`: `ResourceAudioSubtype`.
 */
export function synthResourceTransfer(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    if (kind === ResourceAudioSubtype.HelpCall) {
        // Urgent clear double-beacon call
        for (let i = 0; i < 2; i++) {
            playFmTone(v, {
                carrierFreq: 783.99,
                carrierFreqEnd: 987.77,
                modRatio: 2.0,
                modIndex: 0.55,
                delay: i * 0.12,
                attack: 0.004,
                decay: 0.13,
                gain: 0.17,
            });
        }
        return;
    }
    if (kind === ResourceAudioSubtype.Rejected) {
        playTone(v, {
            type: "triangle",
            freq: 329.63,
            freqEnd: 246.94,
            attack: 0.004,
            decay: 0.14,
            gain: 0.16,
        });
        return;
    }
    if (kind === ResourceAudioSubtype.Received) {
        // Warm ascending treasury/reinforcement chime
        [587.33, 880.0, 1174.66].forEach((freq, idx) => {
            playModalHit(v, {
                baseFreq: freq,
                ratios: [1, 2.0, 3.02],
                gains: [0.64, 0.26, 0.1],
                decay: 0.2,
                delay: idx * 0.052,
                gain: 0.16,
            });
        });
        return;
    }
    // Sent or Requested: warm tactile dispatch pulse
    playModalHit(v, {
        baseFreq: kind === ResourceAudioSubtype.Requested ? 659.25 : 523.25,
        ratios: [1, 2.0],
        gains: [0.7, 0.3],
        decay: 0.16,
        gain: 0.17,
    });
    playTone(v, {
        type: "sine",
        freq: kind === ResourceAudioSubtype.Requested ? 783.99 : 659.25,
        delay: 0.055,
        attack: 0.004,
        decay: 0.14,
        gain: 0.12,
    });
}

/** `HeroAbility` — Heroic signature shimmer modulated by `AbilityAudioFamily`. */
export function synthHeroAbility(v: VoiceRenderCtx, t: CueTrigger): void {
    const family = Math.round(t.subtype);
    const aggressive =
        family === AbilityAudioFamily.Strength || family === AbilityAudioFamily.Conquest;
    const naval = family === AbilityAudioFamily.Naval;
    const economy =
        family === AbilityAudioFamily.Income || family === AbilityAudioFamily.City;

    const root = aggressive ? 392.0 : naval ? 349.23 : economy ? 587.33 : 440.0;
    playFmTone(v, {
        carrierFreq: vary(v.rng, root, 0.015),
        carrierFreqEnd: root * 1.5,
        modRatio: aggressive ? 1.5 : 2.0,
        modIndex: aggressive ? 1.3 : 0.65,
        modIndexEnd: 0.1,
        attack: 0.012,
        decay: 0.3,
        gain: 0.19,
    });
    playModalHit(v, {
        baseFreq: root * 2.0,
        ratios: [1, 1.5, 2.0],
        gains: [0.55, 0.28, 0.17],
        decay: 0.34,
        delay: 0.04,
        gain: 0.15,
    });
}

/** `Emote` — Concise expressive gesture for `Greet`, `Help`, `Threat`, `Celebrate`. */
export function synthEmote(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    switch (kind) {
        case EmoteAudioKind.Greet:
            playModalHit(v, {
                baseFreq: 587.33,
                ratios: [1, 2.0],
                gains: [0.7, 0.3],
                decay: 0.14,
                gain: 0.16,
            });
            playModalHit(v, {
                baseFreq: 783.99,
                ratios: [1, 2.0],
                gains: [0.7, 0.3],
                decay: 0.16,
                delay: 0.07,
                gain: 0.16,
            });
            break;

        case EmoteAudioKind.Help:
            for (let i = 0; i < 2; i++) {
                playTone(v, {
                    type: "triangle",
                    freq: 783.99,
                    freqEnd: 987.77,
                    delay: i * 0.1,
                    attack: 0.004,
                    decay: 0.1,
                    gain: 0.17,
                });
            }
            break;

        case EmoteAudioKind.Threat:
            playSubDrop(v, 145, 72, 0.18, 0.24);
            playTone(v, {
                type: "sawtooth",
                freq: 196.0,
                freqEnd: 155.56,
                attack: 0.01,
                decay: 0.2,
                gain: 0.16,
                filterType: "lowpass",
                filterFreq: 820,
                filterFreqEnd: 260,
            });
            break;

        case EmoteAudioKind.Celebrate:
        default:
            [587.33, 739.99, 880.0, 1174.66].forEach((freq, idx) => {
                playModalHit(v, {
                    baseFreq: freq,
                    ratios: [1, 2.0],
                    gains: [0.7, 0.3],
                    decay: 0.18,
                    delay: idx * 0.042,
                    gain: 0.14,
                });
            });
            break;
    }
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.004,
        decay: 0.05,
        gain: 0.05,
        filterType: "bandpass",
        filterFreq: 1600,
    });
}
