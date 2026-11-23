/**
 * Diplomacy, Betrayal, Resource Transfers, Hero Abilities & Emotes (`diplomacy.ts`).
 *
 * Acoustic Identity (Rise of Kingdoms / Clash Royale / Royal Court):
 *   - Alliances & aid bloom with Karplus-Strong orchestral harp glissandos,
 *     wax-seal stone stamps, golden coin drops, and velvet choir swells.
 *   - Betrayal stabs with an unsheathed steel slash, odaiko impact, and a
 *     dissonant dark orchestral brass cluster (`D + Eb + Ab`).
 *   - Hero Abilities proc with distinct MOBA-grade family signatures (Steel/Brass
 *     for Conquest, Foghorn/Surf for Naval, Harp/Coins for Economy, Crystal Shield
 *     for Defense).
 */

import {
    playBuffer,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import {
    playBrassEnsemble,
    playCasinoChime,
    playChoirPad,
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
    AbilityAudioFamily,
    EmoteAudioKind,
    ResourceAudioSubtype,
    type CueTrigger,
} from "../types.ts";

/** `AllianceRequest` — Royal envoy scroll unfurl + inviting Karplus-Strong harp & celesta fifth. */
export function synthAllianceRequest(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: 1, decay: 0.13, gain: 0.2, bright: true });
    playHarpPluck(v, { freq: 587.33, delay: 0.01, decay: 0.52, gain: 0.3, pan: -0.24 });
    playHarpPluck(v, { freq: 880.0, delay: 0.088, decay: 0.62, gain: 0.3, pan: 0.24 });
    playCasinoChime(v, { freq: 880.0, delay: 0.092, decay: 0.48, gain: 0.22, sparkle: 0.65, pan: 0.24 });
}

/** `AllianceFormed` — Royal covenant wax-seal + sweeping golden harp gliss + noble brass & choir bloom. */
export function synthAllianceFormed(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTaikoHit(v, { heavy: false, pitch: 1.0, gain: 0.38, rimCrack: 0.35 });
    playTactilePop(v, { material: "stone", pitch: 0.98, gain: 0.36, shimmerSend: 0.28 });

    playHarpGliss(v, [293.66, 369.99, 440.0, 587.33, 659.25, 739.99, 1174.66], {
        delay: 0.015,
        stepSec: 0.036,
        gain: 0.32,
        decay: 0.85,
        panStart: -0.55,
        panEnd: 0.55,
        shimmerSend: 0.68,
    });
    playBrassEnsemble(v, {
        freq: 293.66,
        delay: 0.04,
        attack: 0.035,
        hold: 0.08,
        decay: 0.65,
        gain: 0.32,
        brightness: 1.18,
        goldenHall: true,
    });
    playChoirPad(v, [293.66, 440.0, 587.33, 739.99], {
        delay: 0.08,
        attack: 0.09,
        decay: 0.95,
        gain: 0.26,
        goldenHall: true,
    });
    playCasinoChime(v, { freq: 1174.66, delay: 0.19, decay: 0.72, gain: 0.24, sparkle: 0.9 });
}

/** `AllianceEnded` — Dignified parchment fold + warm descending acoustic harp cadence. */
export function synthAllianceEnded(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: -1, decay: 0.14, gain: 0.2, bright: false });
    playHarpPluck(v, { freq: 440.0, delay: 0.01, decay: 0.36, gain: 0.28, pan: 0.2, shimmerSend: 0.3 });
    playHarpPluck(v, { freq: 349.23, delay: 0.085, decay: 0.38, gain: 0.26, pan: 0, shimmerSend: 0.3 });
    playHarpPluck(v, { freq: 293.66, delay: 0.17, decay: 0.52, gain: 0.28, pan: -0.2, shimmerSend: 0.36 });
}

/** `Betrayal` — Chilling cinema stab: steel dagger slash + odaiko slam + dissonant dark brass cluster! */
export function synthBetrayal(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Razor-sharp unsheathed blade slash
    playSteelImpact(v, { heavy: false, pitch: 1.42, gain: 0.46, pan: -0.25, wetSend: 0.55 });
    playSteelImpact(v, { heavy: true, pitch: 0.85, delay: 0.018, gain: 0.36, pan: 0.28, wetSend: 0.55 });

    // Colossal cinema gut-punch drum
    playTaikoHit(v, { heavy: true, pitch: 0.82, gain: 0.72, rimCrack: 0.7, wetSend: 0.6 });

    // Spine-chilling dark orchestral dissonant cluster (D3 + Eb3 + Ab3)
    const cluster = [146.83, 155.56, 207.65];
    cluster.forEach((freq, i) => {
        playBrassEnsemble(v, {
            freq,
            freqEnd: freq * 0.96,
            delay: 0.012 + i * 0.016,
            attack: 0.018,
            hold: 0.1,
            decay: 0.82,
            gain: 0.32,
            brightness: 1.22,
            width: 0.52,
            goldenHall: false,
        });
    });
    playChoirPad(v, [146.83, 155.56, 207.65], {
        delay: 0.06,
        attack: 0.06,
        decay: 0.92,
        gain: 0.24,
        filterFreq: 1150,
        goldenHall: false,
    });
}

/**
 * `ResourceTransfer` — Aid sent, aid received (casino coins!), requested, rejected, or emergency SOS.
 * `subtype`: `ResourceAudioSubtype`.
 */
export function synthResourceTransfer(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    if (kind === ResourceAudioSubtype.HelpCall) {
        // Urgent twin brass-beacon + ringing bell cutting clearly through battle
        for (let i = 0; i < 2; i++) {
            const d = i * 0.145;
            playBrassEnsemble(v, {
                freq: 392.0,
                freqEnd: 493.88,
                delay: d,
                attack: 0.016,
                hold: 0.03,
                decay: 0.16,
                gain: 0.28,
                brightness: 1.3,
                goldenHall: false,
            });
            playCasinoChime(v, {
                freq: 987.77,
                delay: d,
                decay: 0.24,
                gain: 0.24,
                sparkle: 0.55,
            });
        }
        return;
    }

    if (kind === ResourceAudioSubtype.Rejected) {
        playTactilePop(v, { material: "wood", pitch: 0.78, popBend: 0.84, gain: 0.32 });
        playTactilePop(v, { material: "wood", pitch: 0.68, popBend: 0.8, delay: 0.072, gain: 0.28 });
        return;
    }

    if (kind === ResourceAudioSubtype.Received) {
        // Pure casino gold aid arrival!
        playTactilePop(v, { material: "wood", pitch: 1.12, popBend: 1.24, gain: 0.3 });
        playCoinShower(v, {
            count: 4,
            delay: 0.014,
            spacing: 0.038,
            pitch: 1.08,
            pitchClimb: 0.055,
            gain: 0.3,
            spread: 0.56,
        });
        [587.33, 880.0, 1174.66].forEach((freq, i) => {
            playCasinoChime(v, {
                freq,
                delay: 0.02 + i * 0.048,
                decay: 0.42,
                gain: 0.24,
                sparkle: 0.8,
                pan: (i - 1) * 0.3,
                shimmerSend: 0.64,
            });
        });
        return;
    }

    if (kind === ResourceAudioSubtype.Requested) {
        playStereoWhoosh(v, { direction: 1, decay: 0.1, gain: 0.18, bright: true });
        playHarpPluck(v, { freq: 659.25, decay: 0.34, gain: 0.26, pan: -0.18 });
        playCasinoChime(v, { freq: 880.0, delay: 0.06, decay: 0.32, gain: 0.22, sparkle: 0.5, pan: 0.18 });
        return;
    }

    // ResourceAudioSubtype.Sent — tactile dispatch whoosh + warm coin pouch clink
    playStereoWhoosh(v, { direction: 1, decay: 0.12, gain: 0.22, bright: true });
    playTactilePop(v, { material: "wood", pitch: 1.06, popBend: 1.15, gain: 0.28 });
    playCoinShower(v, { count: 2, delay: 0.02, spacing: 0.036, pitch: 1.02, gain: 0.22, spread: 0.36 });
}

/** `HeroAbility` — MOBA / Lilith hero ultimate proc modulated by `AbilityAudioFamily`. */
export function synthHeroAbility(v: VoiceRenderCtx, t: CueTrigger): void {
    const family = Math.round(t.subtype);
    const aggressive =
        family === AbilityAudioFamily.Strength || family === AbilityAudioFamily.Conquest;
    const naval = family === AbilityAudioFamily.Naval;
    const economy = family === AbilityAudioFamily.Income || family === AbilityAudioFamily.City;

    if (aggressive) {
        // Warlord battle cry: taiko + blazing steel + triumphant brass surge!
        playTaikoHit(v, { heavy: true, pitch: 1.04, gain: 0.56, rimCrack: 0.75 });
        playSteelImpact(v, { heavy: false, pitch: 1.24, delay: 0.006, gain: 0.36, wetSend: 0.42 });
        playBrassEnsemble(v, {
            freq: 293.66,
            freqEnd: 440.0,
            delay: 0.015,
            attack: 0.02,
            hold: 0.06,
            decay: 0.44,
            gain: 0.42,
            brightness: 1.38,
            goldenHall: true,
        });
        playCasinoChime(v, { freq: 1174.66, delay: 0.04, decay: 0.46, gain: 0.24, sparkle: 0.85 });
        return;
    }

    if (naval) {
        // Admiral armada proc: deep foghorn + crashing surf + bronze ship bell
        playBrassEnsemble(v, {
            freq: 174.61,
            delay: 0.01,
            attack: 0.03,
            hold: 0.08,
            decay: 0.5,
            gain: 0.4,
            brightness: 1.08,
            goldenHall: true,
        });
        playLiquidSplash(v, { delay: 0.012, decay: 0.42, pitch: 0.92, gain: 0.38, heavy: true });
        playCasinoChime(v, { freq: 880.0, delay: 0.04, decay: 0.48, gain: 0.26, sparkle: 0.7 });
        return;
    }

    if (economy) {
        // Sovereign golden treasury proc: harp gliss + coin burst + celesta shimmer!
        playTactilePop(v, { material: "stone", pitch: 1.12, gain: 0.32 });
        playHarpGliss(v, [440.0, 587.33, 659.25, 739.99, 880.0, 1174.66], {
            delay: 0.01,
            stepSec: 0.03,
            gain: 0.3,
            decay: 0.68,
            shimmerSend: 0.66,
        });
        playCoinShower(v, { count: 4, delay: 0.04, spacing: 0.036, pitch: 1.12, pitchClimb: 0.05, gain: 0.28, spread: 0.55 });
        playCasinoChime(v, { freq: 1318.51, delay: 0.12, decay: 0.56, gain: 0.26, sparkle: 0.95 });
        return;
    }

    // Defense / Diplomacy / Generic: radiant arcane shield & royal crest bloom
    playBuffer(v, { buffer: v.bank.brassLatch[1], rate: 1.1, gain: 0.3 });
    playHarpGliss(v, [392.0, 587.33, 739.99, 880.0], {
        delay: 0.01,
        stepSec: 0.032,
        gain: 0.28,
        decay: 0.62,
        shimmerSend: 0.62,
    });
    playChoirPad(v, [293.66, 440.0, 739.99], {
        delay: 0.02,
        attack: 0.05,
        decay: 0.62,
        gain: 0.26,
        goldenHall: true,
    });
    playCasinoChime(v, { freq: 1174.66, delay: 0.06, decay: 0.52, gain: 0.24, sparkle: 0.85 });
}

/** `Emote` — Charismatic Clash-Royale-style musical gestures (`Greet`, `Help`, `Threat`, `Celebrate`). */
export function synthEmote(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    switch (kind) {
        case EmoteAudioKind.Greet:
            playTactilePop(v, { material: "wood", pitch: 1.18, popBend: 1.24, noteHz: 587.33, gain: 0.32 });
            playHarpPluck(v, { freq: 739.99, delay: 0.055, decay: 0.32, gain: 0.26, pan: -0.2 });
            playCasinoChime(v, { freq: 880.0, delay: 0.095, decay: 0.36, gain: 0.24, sparkle: 0.6, pan: 0.2 });
            break;

        case EmoteAudioKind.Help:
            for (let i = 0; i < 2; i++) {
                playBrassEnsemble(v, {
                    freq: 392.0,
                    freqEnd: 493.88,
                    delay: i * 0.12,
                    attack: 0.015,
                    hold: 0.02,
                    decay: 0.14,
                    gain: 0.26,
                    brightness: 1.28,
                    goldenHall: false,
                });
                playCasinoChime(v, { freq: 987.77, delay: i * 0.12, decay: 0.2, gain: 0.22, sparkle: 0.5 });
            }
            break;

        case EmoteAudioKind.Threat:
            playTaikoHit(v, { heavy: false, pitch: 0.88, gain: 0.44, rimCrack: 0.5 });
            playSteelImpact(v, { heavy: false, pitch: 1.02, delay: 0.012, gain: 0.28, wetSend: 0.32 });
            playBrassEnsemble(v, {
                freq: vary(v.rng, 196.0, 0.02),
                freqEnd: 155.56,
                delay: 0.015,
                attack: 0.018,
                hold: 0.04,
                decay: 0.28,
                gain: 0.34,
                brightness: 1.05,
                goldenHall: false,
            });
            break;

        case EmoteAudioKind.Celebrate:
        default:
            playTactilePop(v, { material: "wood", pitch: 1.22, popBend: 1.3, gain: 0.28 });
            playHarpGliss(v, [587.33, 739.99, 880.0, 1174.66], {
                delay: 0.01,
                stepSec: 0.032,
                gain: 0.26,
                decay: 0.52,
                shimmerSend: 0.62,
            });
            playCoinShower(v, { count: 3, delay: 0.04, spacing: 0.036, pitch: 1.14, gain: 0.24, spread: 0.5 });
            playCasinoChime(v, { freq: 1174.66, delay: 0.11, decay: 0.44, gain: 0.24, sparkle: 0.85 });
            break;
    }
}
