/**
 * Naval Warfare, Amphibious Transport & Maritime Trade Recipes.
 *
 * Acoustic Identity:
 *   Band-passed pink-noise water sweeps, crisp powder-crack warship cannons with
 *   warm saturation, metallic hull strikes, low-pass sinking bubbles, sonar helm
 *   pings, and sparkling D6 modal coin chimes for maritime trade.
 */

import {
    clamp,
    playFilteredNoise,
    playFmTone,
    playModalHit,
    playSubDrop,
    playTone,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import { FleetOrderSubtype, type CueTrigger } from "../types.ts";

/** `TransportLaunch` — Short bosun/nautical whistle + hull sliding into water. */
export function synthTransportLaunch(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Hull water splash
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.01,
        decay: 0.22,
        gain: 0.22,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 680, 0.15),
        filterFreqEnd: 1450,
        filterQ: 1.8,
    });
    // Crisp maritime whistle sweep
    const f0 = vary(v.rng, 880, 0.04);
    playTone(v, {
        type: "sine",
        freq: f0,
        freqEnd: f0 * 1.24,
        delay: 0.012,
        attack: 0.015,
        decay: 0.14,
        gain: 0.13,
    });
}

/** `TransportLanded` — Keel beaching thump on shore + receding wave wash. */
export function synthTransportLanded(v: VoiceRenderCtx, _t: CueTrigger): void {
    playSubDrop(v, vary(v.rng, 138, 0.08), 58, 0.11, 0.26);
    playFilteredNoise(v, {
        color: "pink",
        delay: 0.01,
        attack: 0.008,
        decay: 0.2,
        gain: 0.18,
        filterType: "bandpass",
        filterFreq: 1350,
        filterFreqEnd: 460,
        filterQ: 1.3,
    });
}

/**
 * `FleetOrder` — Clean naval helm / sonar confirmation.
 * `subtype`: 0 = Move, 1 = Patrol (dual ping).
 */
export function synthFleetOrder(v: VoiceRenderCtx, t: CueTrigger): void {
    const patrol = Math.round(t.subtype) === FleetOrderSubtype.Patrol;
    const base = vary(v.rng, 659.25, 0.025);
    playFmTone(v, {
        carrierFreq: base,
        carrierFreqEnd: patrol ? base : base * 1.18,
        modRatio: 2.0,
        modIndex: 0.45,
        attack: 0.004,
        decay: 0.15,
        gain: 0.16,
    });
    if (patrol) {
        playFmTone(v, {
            carrierFreq: base * 1.25,
            modRatio: 2.0,
            modIndex: 0.4,
            delay: 0.1,
            attack: 0.004,
            decay: 0.16,
            gain: 0.14,
        });
    }
}

/**
 * `FleetRecallOrScuttle` — Recall bridge bell vs. voluntary scuttling sink.
 * `subtype`: 0 = Recall, 1 = Scuttle.
 */
export function synthFleetRecallOrScuttle(v: VoiceRenderCtx, t: CueTrigger): void {
    const scuttle = Math.round(t.subtype) === FleetOrderSubtype.Scuttle;
    if (!scuttle) {
        // Dual descending bridge bell
        playModalHit(v, {
            baseFreq: 783.99,
            ratios: [1, 2.01, 2.98],
            gains: [0.65, 0.25, 0.1],
            decay: 0.18,
            gain: 0.18,
        });
        playModalHit(v, {
            baseFreq: 587.33,
            ratios: [1, 2.01, 2.98],
            gains: [0.65, 0.25, 0.1],
            decay: 0.22,
            delay: 0.09,
            gain: 0.16,
        });
        return;
    }
    // Scuttle: muffled hull breach + descending water bubbles
    playSubDrop(v, 110, 38, 0.24, 0.3);
    playFilteredNoise(v, {
        color: "brown",
        attack: 0.015,
        decay: 0.32,
        gain: 0.22,
        filterType: "bandpass",
        filterFreq: 520,
        filterFreqEnd: 170,
        filterQ: 3.0,
    });
}

/** `WarshipSpawned` — Deep dry-dock brass horn + heavy hull launch splash. */
export function synthWarshipSpawned(v: VoiceRenderCtx, _t: CueTrigger): void {
    const horn = vary(v.rng, 146.83, 0.02); // D3
    playTone(v, {
        type: "sawtooth",
        freq: horn,
        freqEnd: horn * 0.99,
        attack: 0.035,
        decay: 0.42,
        gain: 0.22,
        filterType: "lowpass",
        filterFreq: 620,
        filterFreqEnd: 280,
        filterQ: 2.0,
    });
    playTone(v, {
        type: "triangle",
        freq: horn * 1.5,
        delay: 0.03,
        attack: 0.025,
        decay: 0.36,
        gain: 0.14,
        filterType: "lowpass",
        filterFreq: 900,
    });
    playFilteredNoise(v, {
        color: "pink",
        delay: 0.02,
        attack: 0.012,
        decay: 0.28,
        gain: 0.2,
        filterType: "bandpass",
        filterFreq: 750,
        filterFreqEnd: 380,
        filterQ: 1.2,
    });
}

/** `NavalShot` — Crisp deck cannon bark with warm saturation and rotated perspective. */
export function synthNavalShot(v: VoiceRenderCtx, _t: CueTrigger): void {
    const drive = v.createWarmDrive();
    drive.connect(v.out);

    // Powder crack transient
    playFilteredNoise(v, {
        color: "white",
        attack: 0.002,
        decay: 0.055,
        gain: 0.28,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 1950, 0.22),
        filterFreqEnd: 620,
        filterQ: 1.1,
        dest: drive,
    });
    // Cannon barrel thump
    const kick = vary(v.rng, 185, 0.11);
    playSubDrop(v, kick, kick * 0.28, 0.14, 0.38, 0, drive);
}

/** `NavalImpact` — Metallic hull strike + sea spray splash. */
export function synthNavalImpact(v: VoiceRenderCtx, _t: CueTrigger): void {
    const metalFreq = vary(v.rng, 340, 0.15);
    playFmTone(v, {
        carrierFreq: metalFreq,
        carrierFreqEnd: metalFreq * 0.72,
        modRatio: 1.57,
        modIndex: 2.2,
        modIndexEnd: 0.2,
        attack: 0.002,
        decay: 0.14,
        gain: 0.22,
    });
    playFilteredNoise(v, {
        color: "pink",
        delay: 0.01,
        attack: 0.004,
        decay: 0.18,
        gain: 0.2,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 1650, 0.18),
        filterFreqEnd: 520,
        filterQ: 1.5,
    });
}

/** `NavalSink` — Water-muffled magazine blast + groaning steel plunge + bubbles. */
export function synthNavalSink(v: VoiceRenderCtx, _t: CueTrigger): void {
    const drive = v.createWarmDrive();
    drive.connect(v.out);

    playSubDrop(v, vary(v.rng, 130, 0.08), 34, 0.36, 0.44, 0, drive);
    playFilteredNoise(v, {
        color: "brown",
        attack: 0.008,
        decay: 0.42,
        gain: 0.3,
        filterType: "lowpass",
        filterFreq: 920,
        filterFreqEnd: 160,
        dest: drive,
    });
    // Metallic groan descending
    playFmTone(v, {
        carrierFreq: vary(v.rng, 240, 0.08),
        carrierFreqEnd: 92,
        modRatio: 1.38,
        modIndex: 1.9,
        modIndexEnd: 0.3,
        delay: 0.05,
        decay: 0.34,
        gain: 0.18,
    });
    // Plume splash tail
    playFilteredNoise(v, {
        color: "pink",
        delay: 0.06,
        attack: 0.02,
        decay: 0.36,
        gain: 0.18,
        filterType: "bandpass",
        filterFreq: 1100,
        filterFreqEnd: 320,
        filterQ: 2.2,
    });
}

/** `TradeDelivered` — Bright golden treasury chime when a trade ship docks. */
export function synthTradeDelivered(v: VoiceRenderCtx, t: CueTrigger): void {
    const richness = clamp(t.intensity, 0.2, 1.0);
    const notes = [1174.66, 1396.91, 1760.0]; // D6, F6, A6
    const count = richness > 0.65 ? 3 : 2;
    for (let i = 0; i < count; i++) {
        playModalHit(v, {
            baseFreq: vary(v.rng, notes[i], 0.012),
            ratios: [1, 2.02, 3.12],
            gains: [0.62, 0.26, 0.12],
            decay: 0.18 + i * 0.04,
            delay: i * 0.055,
            gain: 0.14 + richness * 0.05,
        });
    }
}

/** `TradeCaptured` — Boarding clash (`subtype`: 1 = you captured prize, 0 = enemy raided you). */
export function synthTradeCaptured(v: VoiceRenderCtx, t: CueTrigger): void {
    const wonPrize = t.subtype >= 1;
    playFmTone(v, {
        carrierFreq: wonPrize ? 587.33 : 329.63,
        carrierFreqEnd: wonPrize ? 880.0 : 220.0,
        modRatio: 1.5,
        modIndex: 1.2,
        attack: 0.004,
        decay: 0.18,
        gain: 0.2,
    });
    playFilteredNoise(v, {
        color: "pink",
        attack: 0.004,
        decay: 0.14,
        gain: 0.14,
        filterType: "bandpass",
        filterFreq: 1300,
        filterFreqEnd: 580,
    });
}
