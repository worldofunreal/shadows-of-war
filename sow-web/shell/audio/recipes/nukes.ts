/**
 * Nuclear Arsenal & SAM Air-Defense Procedural Recipes (`nukes.ts`).
 *
 * Acoustic Identity (Blockbuster Cinema / Modern Tactical Warfare):
 *   - `NukeArmed`:    Guarded master-arm brass toggle + ominous subterranean brass
 *                     power hum + escalating targeting lock pulses per warhead class.
 *   - `NukeLaunch`:   Silo blast-door release + tube-driven rocket ignition punch +
 *                     wide stereo crackling afterburner roar + air-raid brass swell.
 *   - `MirvSplit`:    Stratospheric pyrotechnic bolt crack + 4 stereo-scattered
 *                     atmospheric reentry warhead trails (`L → R`).
 *   - `SamIntercept`: Crisp high-altitude flak crack + metallic shrapnel spray.
 *   - `NukeImpact`:   5-stage Hollywood shockwave (`playCinematicBlast`) scaling from
 *                     tight MIRV strike to earth-shaking double-shock Hydrogen detonation.
 */

import {
    playBuffer,
    playFilteredNoise,
    playFmTone,
    playTone,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import {
    playBrassEnsemble,
    playCasinoChime,
    playCinematicBlast,
    playSteelImpact,
    playTaikoHit,
} from "../instruments.ts";
import { NukeAudioLevel, type CueTrigger } from "../types.ts";

/** `NukeArmed` — Master-arm safety switch flip + subterranean hum (`subtype`: 1=Atom, 2=Hydrogen, 3=MIRV). */
export function synthNukeArmed(v: VoiceRenderCtx, t: CueTrigger): void {
    const level = Math.round(t.subtype) || NukeAudioLevel.Atom;
    const isHydro = level === NukeAudioLevel.Hydrogen;
    const isMirv = level === NukeAudioLevel.MirvCarrier;

    // Guarded nuclear toggle flip + heavy bunker latch
    playBuffer(v, { buffer: v.bank.brassLatch[1], rate: 1.05, gain: 0.44, wetSend: 0.22 });
    playBuffer(v, { buffer: v.bank.brassLatch[0], delay: 0.032, rate: 1.32, gain: 0.28 });

    // Ominous subterranean silo power swell
    playBrassEnsemble(v, {
        freq: isMirv ? 110.0 : isHydro ? 130.81 : 146.83,
        delay: 0.015,
        attack: 0.04,
        hold: 0.08,
        decay: 0.36,
        gain: 0.34,
        brightness: 0.85,
        goldenHall: false,
    });

    // Escalating crisp telemetry target-lock pings
    const pulses = isMirv ? 3 : isHydro ? 2 : 1;
    const baseHz = isMirv ? 659.25 : isHydro ? 739.99 : 880.0;
    for (let i = 0; i < pulses; i++) {
        playFmTone(v, {
            carrierFreq: baseHz * (1 + i * 0.14),
            carrierFreqEnd: baseHz * (1.12 + i * 0.14),
            modRatio: 2.0,
            modIndex: 0.42,
            delay: 0.025 + i * 0.072,
            attack: 0.003,
            decay: 0.12,
            gain: 0.18,
            pan: pulses > 1 ? -0.3 + (0.6 * i) / (pulses - 1) : 0,
            shimmerSend: 0.34,
        });
    }
}

/** `NukeLaunch` — Silo blast-door release + booster ignition punch + crackling stereo thrust roar. */
export function synthNukeLaunch(v: VoiceRenderCtx, t: CueTrigger): void {
    const level = Math.round(t.subtype) || NukeAudioLevel.Atom;
    const heavy = level === NukeAudioLevel.Hydrogen || level === NukeAudioLevel.MirvCarrier;
    const drive = v.createWarmDrive();
    drive.connect(v.out);

    // Blast door unlock + silo ignition kick
    playSteelImpact(v, { heavy: true, pitch: 0.72, gain: 0.36, wetSend: 0.4 });
    playTaikoHit(v, { heavy: true, pitch: heavy ? 0.76 : 0.88, delay: 0.01, gain: heavy ? 0.66 : 0.54, wetSend: 0.5 });

    // Crackling stereo rocket engine roar sweeping skyward
    playFilteredNoise(v, {
        color: "crackle",
        delay: 0.015,
        attack: 0.028,
        decay: heavy ? 0.88 : 0.65,
        gain: heavy ? 0.54 : 0.44,
        filterType: "bandpass",
        filterFreq: 340,
        filterFreqEnd: heavy ? 2450 : 1850,
        filterQ: 0.9,
        wetSend: 0.52,
        dest: drive,
    });

    // Dramatic rising air-raid brass tension swell
    playBrassEnsemble(v, {
        freq: heavy ? 174.61 : 220.0,
        freqEnd: heavy ? 261.63 : 329.63,
        delay: 0.05,
        attack: 0.06,
        hold: 0.08,
        decay: heavy ? 0.62 : 0.46,
        gain: 0.32,
        brightness: 1.12,
        goldenHall: false,
    });
}

/** `MirvSplit` — Stratospheric pyrotechnic carrier separation + 4 stereo-panned diving warhead trails. */
export function synthMirvSplit(v: VoiceRenderCtx, _t: CueTrigger): void {
    // High-altitude pyrotechnic bolt crack + metallic fairing shed
    playCinematicBlast(v, { scale: 0.52, tailSec: 0.28, gain: 0.48 });
    playSteelImpact(v, { heavy: false, pitch: 1.38, delay: 0.008, gain: 0.34, wetSend: 0.42 });

    // 4 diverging atmospheric reentry trails fanning across L -> R
    const pans = [-0.68, -0.24, 0.24, 0.68];
    for (let i = 0; i < 4; i++) {
        const delay = 0.03 + i * 0.044;
        const startHz = vary(v.rng, 1680 - i * 130, 0.07);
        // Atmospheric drag air-whistle (high-Q resonant noise + subtle harmonic glide — never raw NES blip!)
        playFilteredNoise(v, {
            color: "parchment",
            delay,
            attack: 0.012,
            decay: 0.28,
            gain: 0.24,
            filterType: "bandpass",
            filterFreq: startHz * 1.4,
            filterFreqEnd: startHz * 0.42,
            filterQ: 4.2,
            pan: pans[i],
            wetSend: 0.44,
        });
        playTone(v, {
            wave: v.bank.choirWave,
            type: "sine",
            freq: startHz,
            freqEnd: startHz * 0.38,
            delay,
            attack: 0.01,
            decay: 0.24,
            gain: 0.08,
            pan: pans[i],
            wetSend: 0.38,
        });
    }
}

/** `SamIntercept` — Razor-sharp high-altitude flak airburst + shattering metallic shrapnel spray. */
export function synthSamIntercept(v: VoiceRenderCtx, _t: CueTrigger): void {
    const drive = v.createWarmDrive();
    drive.connect(v.out);

    // Blinding crisp high-altitude flak crack (clean sky punch, zero muddy sub-boom)
    playFilteredNoise(v, {
        color: "crackle",
        attack: 0.0015,
        decay: 0.16,
        gain: 0.52,
        filterType: "highpass",
        filterFreq: 520,
        filterFreqEnd: 240,
        wetSend: 0.42,
        dest: drive,
    });
    playTaikoHit(v, { heavy: false, pitch: 1.38, gain: 0.38, rimCrack: 0.85, wetSend: 0.35 });

    // Shattering missile casing shrapnel across stereo
    playSteelImpact(v, { heavy: false, pitch: 1.32, delay: 0.006, gain: 0.38, pan: -0.32, wetSend: 0.46 });
    playSteelImpact(v, { heavy: false, pitch: 1.54, delay: 0.022, gain: 0.26, pan: 0.36, wetSend: 0.48 });

    // Subtle crisp defensive confirmation glint so intercepting feels rewarding!
    playCasinoChime(v, {
        freq: 1174.66,
        delay: 0.03,
        decay: 0.24,
        gain: 0.15,
        sparkle: 0.6,
        shimmerSend: 0.45,
    });
}

/**
 * `NukeImpact` — 5-stage Hollywood nuclear detonation scaled by warhead class.
 * `subtype`: 1 = AtomBomb, 2 = HydrogenBomb (massive double-shock), 4 = MIRV warhead (tight brutal punch).
 */
export function synthNukeImpact(v: VoiceRenderCtx, t: CueTrigger): void {
    const level = Math.round(t.subtype) || NukeAudioLevel.Atom;
    const isHydro = level === NukeAudioLevel.Hydrogen;
    const isMirv = level === NukeAudioLevel.MirvWarhead;

    playCinematicBlast(v, {
        scale: isHydro ? 1.42 : isMirv ? 0.78 : 1.08,
        tailSec: isHydro ? 1.55 : isMirv ? 0.64 : 1.08,
        doubleShock: isHydro,
        gain: isHydro ? 0.9 : isMirv ? 0.68 : 0.8,
    });

    // Superheated steel/shockwave tear at detonation core
    playSteelImpact(v, {
        heavy: true,
        pitch: isHydro ? 0.58 : isMirv ? 0.92 : 0.72,
        delay: 0.005,
        gain: isHydro ? 0.42 : 0.28,
        wetSend: 0.6,
    });

    // Cinema apocalyptic low brass drone resonance under Hydrogen & Atom blasts
    if (!isMirv) {
        playBrassEnsemble(v, {
            freq: isHydro ? 73.42 : 98.0, // D2 / G2 seismic dark horn
            freqEnd: isHydro ? 55.0 : 73.42,
            delay: 0.06,
            attack: 0.08,
            hold: 0.14,
            decay: isHydro ? 1.25 : 0.82,
            gain: isHydro ? 0.38 : 0.26,
            brightness: 0.68,
            width: 0.55,
            goldenHall: false,
        });
    }
}
