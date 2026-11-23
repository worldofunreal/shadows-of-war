/**
 * Naval Warfare, Amphibious Transport & Maritime Trade Recipes (`naval.ts`).
 *
 * Acoustic Identity (Boom Beach / Rise of Kingdoms / Luxury Maritime):
 *   - Real 54-bubble Minnaert `waterSplash` surf & plumes (never flat noise hiss).
 *   - Broadsides punch with tube-driven gunpowder crackle (`playCinematicBlast`)
 *     and oak deck recoil.
 *   - Warships announce with deep stereo dry-dock brass foghorns & anchor steel.
 *   - Maritime trade deliveries shower players with physical gold coin cascades
 *     (`playCoinShower`) and crystalline celesta chimes.
 */

import {
    clamp,
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
    playCoinShower,
    playHarpPluck,
    playLiquidSplash,
    playSteelImpact,
    playTactilePop,
    playTaikoHit,
} from "../instruments.ts";
import { FleetOrderSubtype, type CueTrigger } from "../types.ts";

/** `TransportLaunch` — Hull timber splash into water + authentic breathy nautical bosun whistle. */
export function synthTransportLaunch(v: VoiceRenderCtx, _t: CueTrigger): void {
    playBuffer(v, {
        buffer: v.bank.woodThock[1],
        rate: vary(v.rng, 0.88, 0.06),
        gain: 0.34,
        wetSend: 0.24,
    });
    playLiquidSplash(v, { delay: 0.01, decay: 0.36, pitch: 1.04, gain: 0.36, heavy: false });

    // Breathy maritime bosun pipe (warm breath chiff + harmonic whistle sweep, never bare sine)
    const f0 = vary(v.rng, 987.77, 0.03); // B5
    playTone(v, {
        wave: v.bank.choirWave,
        type: "sine",
        freq: f0,
        freqEnd: f0 * 1.19,
        delay: 0.02,
        attack: 0.016,
        hold: 0.02,
        decay: 0.14,
        gain: 0.14,
        filterType: "bandpass",
        filterFreq: 1400,
        filterQ: 1.4,
        shimmerSend: 0.28,
    });
    playFilteredNoise(v, {
        color: "parchment",
        delay: 0.018,
        attack: 0.014,
        decay: 0.12,
        gain: 0.1,
        filterType: "bandpass",
        filterFreq: f0 * 1.1,
        filterFreqEnd: f0 * 1.35,
        filterQ: 4.5,
    });
}

/** `TransportLanded` — Heavy wooden keel beaching on wet sand + receding surf wash. */
export function synthTransportLanded(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTaikoHit(v, { heavy: false, pitch: 0.94, gain: 0.38, rimCrack: 0.35 });
    playBuffer(v, {
        buffer: v.bank.woodThock[1],
        delay: 0.005,
        rate: vary(v.rng, 0.76, 0.06),
        gain: 0.42,
        wetSend: 0.24,
    });
    playLiquidSplash(v, { delay: 0.015, decay: 0.32, pitch: 0.92, gain: 0.28, heavy: false });
}

/**
 * `FleetOrder` — Machined brass ship-engine telegraph latch + crisp bridge bell/sonar ping.
 * `subtype`: 0 = Move, 1 = Patrol (dual telegraph confirmation).
 */
export function synthFleetOrder(v: VoiceRenderCtx, t: CueTrigger): void {
    const patrol = Math.round(t.subtype) === FleetOrderSubtype.Patrol;
    playTactilePop(v, {
        material: "brass",
        pitch: 1.05,
        gain: 0.34,
        shimmerSend: 0.18,
    });
    const base = vary(v.rng, 739.99, 0.015); // F#5
    playCasinoChime(v, {
        freq: base,
        delay: 0.012,
        decay: 0.26,
        gain: 0.24,
        sparkle: 0.35,
        pan: patrol ? -0.2 : 0,
    });
    if (patrol) {
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: 0.095,
            rate: 1.22,
            gain: 0.24,
            pan: 0.22,
        });
        playCasinoChime(v, {
            freq: 880.0, // A5
            delay: 0.105,
            decay: 0.3,
            gain: 0.24,
            sparkle: 0.45,
            pan: 0.24,
        });
    }
}

/**
 * `FleetRecallOrScuttle` — Twin bronze ship's bridge bells (`0`) vs. hull-breach scuttle (`1`).
 */
export function synthFleetRecallOrScuttle(v: VoiceRenderCtx, t: CueTrigger): void {
    const scuttle = Math.round(t.subtype) === FleetOrderSubtype.Scuttle;
    if (!scuttle) {
        // Authentic twin ship's brass bells
        playBuffer(v, { buffer: v.bank.brassLatch[0], rate: 0.95, gain: 0.22 });
        playCasinoChime(v, { freq: 880.0, delay: 0.006, decay: 0.34, gain: 0.28, sparkle: 0.4, pan: -0.18 });
        playCasinoChime(v, { freq: 587.33, delay: 0.105, decay: 0.42, gain: 0.28, sparkle: 0.35, pan: 0.18 });
        return;
    }
    // Scuttle: deep muffled hull breach + tearing steel + foaming underwater bubbles
    playTaikoHit(v, { heavy: true, pitch: 0.78, gain: 0.46, wetSend: 0.38 });
    playBuffer(v, {
        buffer: v.bank.steelClash[1],
        delay: 0.02,
        rate: 0.82,
        rateEnd: 0.46,
        gain: 0.34,
        wetSend: 0.36,
    });
    playLiquidSplash(v, { delay: 0.03, decay: 0.52, pitch: 0.76, gain: 0.44, heavy: true });
}

/** `WarshipSpawned` — Colossal dry-dock brass foghorn + anchor steel + heavy hull launch splash. */
export function synthWarshipSpawned(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Deep stereo dry-dock battleship horn (D3 + low octave reinforcement)
    playBrassEnsemble(v, {
        freq: 146.83, // D3
        freqEnd: 144.5,
        attack: 0.045,
        hold: 0.12,
        decay: 0.62,
        gain: 0.48,
        brightness: 0.92,
        width: 0.46,
        goldenHall: false,
    });
    playBrassEnsemble(v, {
        freq: 220.0, // A3 harmonic fifth
        delay: 0.04,
        attack: 0.04,
        hold: 0.08,
        decay: 0.5,
        gain: 0.26,
        brightness: 1.05,
        goldenHall: false,
    });
    // Heavy anchor chain & steel hull clank
    playSteelImpact(v, { heavy: true, pitch: 0.86, delay: 0.015, gain: 0.26, wetSend: 0.36 });
    // Massive dry-dock hull launch splash
    playLiquidSplash(v, { delay: 0.025, decay: 0.48, pitch: 0.84, gain: 0.42, heavy: true });
}

/** `NavalShot` — Boom Beach / AAA deck cannon broadside with powder crack & timber recoil. */
export function synthNavalShot(v: VoiceRenderCtx, _t: CueTrigger): void {
    playCinematicBlast(v, {
        scale: vary(v.rng, 0.46, 0.1),
        tailSec: 0.24,
        gain: 0.52,
    });
    // Crisp wooden gun-carriage deck recoil
    playBuffer(v, {
        buffer: v.bank.woodThock[1],
        delay: 0.008,
        rate: vary(v.rng, 0.86, 0.08),
        gain: 0.28,
    });
}

/** `NavalImpact` — Cannonball crunching forged steel hull + sea-spray plume. */
export function synthNavalImpact(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSteelImpact(v, {
        heavy: true,
        pitch: vary(v.rng, 0.98, 0.14),
        gain: 0.44,
        wetSend: 0.34,
    });
    playBuffer(v, {
        buffer: v.bank.woodThock[1],
        delay: 0.006,
        rate: vary(v.rng, 0.78, 0.1),
        gain: 0.28,
    });
    playLiquidSplash(v, {
        delay: 0.014,
        decay: 0.32,
        pitch: vary(v.rng, 1.02, 0.1),
        gain: 0.34,
        heavy: false,
    });
}

/** `NavalSink` — Magazine blast + groaning twisted hull plunge + huge water plume. */
export function synthNavalSink(v: VoiceRenderCtx, _t: CueTrigger): void {
    playCinematicBlast(v, { scale: 0.72, tailSec: 0.56, gain: 0.62 });
    // Groaning collapsing iron hull bending downward
    playBuffer(v, {
        buffer: v.bank.steelClash[1],
        delay: 0.045,
        rate: vary(v.rng, 0.84, 0.07),
        rateEnd: 0.42,
        gain: 0.44,
        wetSend: 0.48,
    });
    // Towering sea plume & bubbles
    playLiquidSplash(v, { delay: 0.055, decay: 0.62, pitch: 0.78, gain: 0.48, heavy: true });
}

/** `TradeDelivered` — Pure Casino Gold Treasury arrival! Pier knock + stereo gold coin shower + celesta! */
export function synthTradeDelivered(v: VoiceRenderCtx, t: CueTrigger): void {
    const richness = clamp(t.intensity, 0.2, 1.0);
    // Warm timber dock arrival tap
    playTactilePop(v, { material: "wood", pitch: 1.04, popBend: 1.15, gain: 0.24 });

    // Real physical spinning gold coin cascade!
    playCoinShower(v, {
        count: richness > 0.7 ? 5 : richness > 0.4 ? 4 : 3,
        delay: 0.012,
        spacing: 0.038,
        pitch: 1.06,
        pitchClimb: 0.055,
        gain: 0.28 + richness * 0.1,
        spread: 0.58,
    });

    // Radiant D Major9 celesta & harp treasury gleam
    const notes = richness > 0.6 ? [739.99, 1174.66, 1479.98] : [880.0, 1174.66];
    notes.forEach((freq, i) => {
        playCasinoChime(v, {
            freq,
            delay: 0.02 + i * 0.052,
            decay: 0.38 + i * 0.06,
            gain: 0.22 + richness * 0.06,
            sparkle: 0.75,
            pan: -0.25 + i * 0.25,
            shimmerSend: 0.62,
        });
    });
    if (richness > 0.55) {
        playHarpPluck(v, {
            freq: 587.33,
            delay: 0.01,
            decay: 0.48,
            gain: 0.2,
            shimmerSend: 0.52,
        });
    }
}

/** `TradeCaptured` — Corsair boarding clash (`subtype`: 1 = won gold prize, 0 = raided by enemy). */
export function synthTradeCaptured(v: VoiceRenderCtx, t: CueTrigger): void {
    const wonPrize = t.subtype >= 1;
    playSteelImpact(v, { heavy: false, pitch: wonPrize ? 1.18 : 0.88, gain: 0.32, wetSend: 0.3 });
    playLiquidSplash(v, { delay: 0.01, decay: 0.24, pitch: 1.08, gain: 0.24, heavy: false });

    if (wonPrize) {
        // Exciting captured treasure burst!
        playCoinShower(v, {
            count: 4,
            delay: 0.025,
            spacing: 0.036,
            pitch: 1.1,
            pitchClimb: 0.05,
            gain: 0.3,
            spread: 0.55,
        });
        playCasinoChime(v, { freq: 1174.66, delay: 0.04, decay: 0.42, gain: 0.26, sparkle: 0.8 });
    } else {
        // Tense raided brass warning drop
        playFmTone(v, {
            carrierFreq: 311.13,
            carrierFreqEnd: 207.65,
            modRatio: 1.5,
            modIndex: 1.1,
            delay: 0.02,
            decay: 0.24,
            gain: 0.22,
            wetSend: 0.32,
        });
    }
}
