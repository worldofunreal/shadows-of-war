/**
 * GUI, HUD & UX Micro-Interaction Procedural Recipes (`ui.ts`).
 *
 * Acoustic Identity (Supercell / Lilith Games / Luxury Casino Craft):
 *   - Buttons & controls feel physical: carved rosewood, polished marble, and
 *     machined brass latches with bouncy pitch springs — ZERO raw NES beeps.
 *   - Navigation & drawers glide with stereo Haas parchment/velvet whooshes.
 *   - Rewards, unlocks, store purchases, and celebration counters unleash the
 *     signature golden dopamine stack: Karplus-Strong harp glissandos, glassy
 *     celesta D Major9 chords, and physical gold coin cascades blooming into
 *     the 1.65 s `casinoHallIR` shimmer reverb.
 */

import {
    CELESTA_REF_HZ,
    clamp,
    HARP_REF_HZ,
    MAJOR9_D,
    playBuffer,
    playFilteredNoise,
    playSubDrop,
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
    playSteelImpact,
    playStereoWhoosh,
    playTactilePop,
    playTaikoHit,
} from "../instruments.ts";
import {
    UiBattleLaunchSubtype,
    UiHeroBrowseSubtype,
    UiHoverSubtype,
    UiInfoTabSubtype,
    UiPressSubtype,
    UiPurchaseActionSubtype,
    type CueTrigger,
} from "../types.ts";

/** `UiClick` — Tactile carved-wood / royal-stone button (`subtype`: 0=default, 1=primary CTA, 2=back/cancel). */
export function synthUiClick(v: VoiceRenderCtx, t: CueTrigger): void {
    const variant = Math.round(t.subtype);

    if (variant === 1) {
        // Primary CTA (Play / Confirm / Buy): authoritative royal marble-and-brass pop + golden glint
        playTactilePop(v, {
            material: "stone",
            pitch: 1.06,
            popBend: 1.24,
            noteHz: 587.33, // D5
            gain: 0.44,
            shimmerSend: 0.24,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: 0.008,
            rate: vary(v.rng, 1.18, 0.04),
            decay: 0.055,
            gain: 0.16,
            shimmerSend: 0.18,
        });
        playCasinoChime(v, {
            freq: vary(v.rng, 880.0, 0.008), // A5 royal glint
            delay: 0.01,
            decay: 0.22,
            gain: 0.18,
            sparkle: 0.45,
            shimmerSend: 0.42,
        });
        return;
    }

    if (variant === 2) {
        // Back / Cancel: soft cushioned descending rosewood tock
        playTactilePop(v, {
            material: "wood",
            pitch: 0.84,
            popBend: 0.82,
            noteHz: 392.0, // G4
            gain: 0.34,
            shimmerSend: 0.08,
        });
        return;
    }

    // Default UI button: addictive Supercell carved-wood/ivory bouncy pop
    const subtleNotes = [587.33, 659.25, 587.33, 739.99];
    const note = subtleNotes[Math.floor(v.rng() * subtleNotes.length)];
    playTactilePop(v, {
        material: "wood",
        pitch: 1.1,
        popBend: 1.18,
        noteHz: note,
        gain: 0.38,
        shimmerSend: 0.12,
    });
}

/** `UiTabSwitch` — Silky stereo directional card glide + precision brass detent (`subtype`: 0=neutral, 1=fwd, 2=back). */
export function synthUiTabSwitch(v: VoiceRenderCtx, t: CueTrigger): void {
    const back = Math.round(t.subtype) === 2;
    playStereoWhoosh(v, {
        direction: back ? -1 : 1,
        decay: 0.11,
        gain: 0.24,
        bright: true,
    });
    playTactilePop(v, {
        material: "brass",
        pitch: back ? 0.92 : 1.14,
        popBend: back ? 0.9 : 1.14,
        noteHz: back ? 440.0 : 659.25,
        delay: 0.012,
        gain: 0.26,
        pan: back ? -0.18 : 0.18,
        shimmerSend: 0.16,
    });
}

/** `UiModalOpen` — Velvet drawer whoosh + crisp marble latch + warm golden harp lift. */
export function synthUiModalOpen(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: 1, decay: 0.15, gain: 0.26, bright: true });
    playTactilePop(v, {
        material: "stone",
        pitch: 0.96,
        popBend: 1.15,
        delay: 0.008,
        gain: 0.24,
        shimmerSend: 0.15,
    });
    playHarpPluck(v, {
        freq: 587.33, // D5
        delay: 0.014,
        decay: 0.34,
        gain: 0.2,
        pan: -0.22,
        shimmerSend: 0.42,
    });
    playHarpPluck(v, {
        freq: 880.0, // A5
        delay: 0.052,
        decay: 0.4,
        gain: 0.18,
        pan: 0.22,
        shimmerSend: 0.48,
    });
}

/** `UiModalClose` — Cushioned reverse glide + gentle rosewood settle. */
export function synthUiModalClose(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: -1, decay: 0.12, gain: 0.22, bright: false });
    playTactilePop(v, {
        material: "wood",
        pitch: 0.78,
        popBend: 0.84,
        noteHz: 369.99, // F#4 warm resolution
        delay: 0.015,
        gain: 0.28,
        shimmerSend: 0.1,
    });
}

/** `UiRadialOpen` — Machined brass compass wheel unfolding with triple clockwork micro-ticks. */
export function synthUiRadialOpen(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: 1, decay: 0.11, gain: 0.2, bright: true });
    const rates = [0.92, 1.1, 1.3];
    const pans = [-0.34, 0, 0.34];
    for (let i = 0; i < 3; i++) {
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: i * 0.022,
            rate: vary(v.rng, rates[i], 0.03),
            decay: 0.055,
            gain: 0.2 + i * 0.04,
            pan: pans[i],
            shimmerSend: 0.2,
        });
    }
    playCasinoChime(v, {
        freq: 739.99, // F#5 subtle gleam
        delay: 0.035,
        decay: 0.2,
        gain: 0.13,
        sparkle: 0.3,
    });
}

/** `UiRadialClose` — Crisp magnetic brass retract latch. */
export function synthUiRadialClose(v: VoiceRenderCtx, _t: CueTrigger): void {
    playBuffer(v, {
        buffer: v.bank.brassLatch[1],
        rate: vary(v.rng, 0.96, 0.04),
        rateEnd: 0.76,
        decay: 0.075,
        gain: 0.26,
        shimmerSend: 0.12,
    });
    playTactilePop(v, {
        material: "wood",
        pitch: 0.86,
        popBend: 0.85,
        delay: 0.012,
        gain: 0.18,
    });
}

/** `UiSliderTick` — Tactile marimba/wood detent climbing musically along D Major9 (`intensity` ∈ [0..1]). */
export function synthUiSliderTick(v: VoiceRenderCtx, t: CueTrigger): void {
    const ratio = clamp(t.intensity, 0, 1);
    const noteIdx = Math.round(ratio * (MAJOR9_D.length - 4)) + 2;
    const freq = MAJOR9_D[clamp(noteIdx, 0, MAJOR9_D.length - 1)];

    playBuffer(v, {
        buffer: v.bank.woodThock[2],
        rate: 0.82 + ratio * 0.62,
        decay: 0.045,
        gain: 0.24,
        shimmerSend: 0.1,
    });
    playCasinoChime(v, {
        freq,
        decay: 0.11,
        gain: 0.12,
        sparkle: 0.18,
        shimmerSend: 0.2,
    });
}

/** `UiToast` — Glassy casino-grade notification chime scaled by priority (`subtype`: 1..4). */
export function synthUiToast(v: VoiceRenderCtx, t: CueTrigger): void {
    const prio = clamp(Math.round(t.subtype), 1, 4);
    const baseNote = prio >= 4 ? 880.0 : prio === 3 ? 739.99 : 587.33;

    playTactilePop(v, {
        material: "wood",
        pitch: 1.25,
        gain: 0.18,
    });
    playCasinoChime(v, {
        freq: baseNote,
        decay: 0.34,
        gain: 0.24 + prio * 0.025,
        sparkle: 0.45 + prio * 0.1,
        pan: -0.15,
    });
    if (prio >= 3) {
        playCasinoChime(v, {
            freq: baseNote * 1.25, // radiant major third / fifth lift
            delay: 0.068,
            decay: 0.42,
            gain: 0.26 + prio * 0.02,
            sparkle: 0.7,
            pan: 0.18,
        });
    }
    if (prio >= 4) {
        playCoinShower(v, {
            count: 2,
            delay: 0.08,
            spacing: 0.04,
            pitch: 1.15,
            gain: 0.15,
            spread: 0.35,
        });
    }
}

/** `UiMatchFound` — Iconic Clash Royale / RoK "BATTLE READY!" taiko + war brass + golden bell. */
export function synthUiMatchFound(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Monumental chest slam
    playTaikoHit(v, { heavy: true, pitch: 0.96, gain: 0.62, rimCrack: 0.7, wetSend: 0.48 });

    // Heroic stereo brass call (D4 -> A4)
    playBrassEnsemble(v, {
        freq: 293.66, // D4
        delay: 0.02,
        attack: 0.025,
        hold: 0.06,
        decay: 0.36,
        gain: 0.42,
        brightness: 1.15,
        goldenHall: true,
    });
    playBrassEnsemble(v, {
        freq: 440.0, // A4
        delay: 0.14,
        attack: 0.025,
        hold: 0.12,
        decay: 0.62,
        gain: 0.48,
        brightness: 1.32,
        goldenHall: true,
    });

    // Radiant royal anvil/celesta crowning ring
    [587.33, 880.0, 1174.66].forEach((freq, i) => {
        playCasinoChime(v, {
            freq,
            delay: 0.13 + i * 0.045,
            decay: 0.65,
            gain: 0.24,
            sparkle: 0.75,
            pan: (i - 1) * 0.35,
            shimmerSend: 0.65,
        });
    });
}

/** `UiPurchaseSuccess` — PURE CASINO JACKPOT / SUPERCELL CHEST UNLOCK! */
export function synthUiPurchaseSuccess(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Punchy chest lock burst
    playTaikoHit(v, { heavy: false, pitch: 1.08, gain: 0.48, rimCrack: 0.65 });
    playTactilePop(v, { material: "stone", pitch: 1.15, gain: 0.36 });

    // Sweeping stereo Karplus-Strong golden harp glissando (D Major9)
    playHarpGliss(v, [293.66, 369.99, 440.0, 587.33, 659.25, 739.99, 880.0, 1174.66], {
        delay: 0.015,
        stepSec: 0.032,
        gain: 0.32,
        decay: 0.78,
        panStart: -0.58,
        panEnd: 0.58,
        shimmerSend: 0.62,
    });

    // Sparkling physical gold coin cascade!
    playCoinShower(v, {
        count: 5,
        delay: 0.09,
        spacing: 0.042,
        pitch: 1.04,
        pitchClimb: 0.045,
        gain: 0.28,
        spread: 0.62,
    });

    // Crown celesta D Major9 bloom + velvet choir pad
    [587.33, 739.99, 1174.66, 1318.51].forEach((freq, i) => {
        playCasinoChime(v, {
            freq,
            delay: 0.18 + i * 0.038,
            decay: 0.75,
            gain: 0.24,
            sparkle: 0.9,
            pan: -0.45 + i * 0.3,
            shimmerSend: 0.72,
        });
    });
    playChoirPad(v, [293.66, 440.0, 739.99], {
        delay: 0.14,
        attack: 0.08,
        decay: 0.95,
        gain: 0.24,
        goldenHall: true,
    });
}

/** `UiHeroEquip` — Heavy royal armor & crest seal lock when selecting a leader or skin. */
export function synthUiHeroEquip(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTaikoHit(v, { heavy: false, pitch: 1.05, gain: 0.45, rimCrack: 0.45 });
    playSteelImpact(v, { heavy: false, pitch: 1.2, delay: 0.006, gain: 0.22, wetSend: 0.28 });
    playBuffer(v, {
        buffer: v.bank.brassLatch[1],
        delay: 0.014,
        rate: 1.02,
        gain: 0.32,
        shimmerSend: 0.2,
    });
    // Proud royal fifth bloom
    playBrassEnsemble(v, {
        freq: 293.66,
        delay: 0.025,
        attack: 0.022,
        hold: 0.04,
        decay: 0.34,
        gain: 0.28,
        brightness: 1.15,
        goldenHall: true,
    });
    playCasinoChime(v, {
        freq: 880.0,
        delay: 0.03,
        decay: 0.36,
        gain: 0.22,
        sparkle: 0.6,
    });
}

/** `UiCampaignStep` — Crisp war-room parchment turn + carved ivory tap. */
export function synthUiCampaignStep(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: 1, decay: 0.085, gain: 0.18, bright: false });
    playTactilePop(v, {
        material: "wood",
        pitch: 1.18,
        popBend: 1.08,
        delay: 0.008,
        gain: 0.22,
        shimmerSend: 0.08,
    });
}

/** `UiCelebrationFanfare` — Full AAA Supercell / Lilith mission & level-up celebration fanfare! */
export function synthUiCelebrationFanfare(v: VoiceRenderCtx, _t: CueTrigger): void {
    // Timpani roll into grand slam
    playTaikoHit(v, { heavy: false, pitch: 0.92, delay: 0, gain: 0.36 });
    playTaikoHit(v, { heavy: false, pitch: 1.04, delay: 0.085, gain: 0.44 });
    playTaikoHit(v, { heavy: true, pitch: 1.0, delay: 0.18, gain: 0.64, rimCrack: 0.8, wetSend: 0.5 });

    // Triumphant stereo brass call (D4 -> F#4 -> A4 -> D5!)
    const brassPhrase = [
        { f: 293.66, d: 0.02, len: 0.22, br: 1.05 },
        { f: 369.99, d: 0.11, len: 0.22, br: 1.15 },
        { f: 440.0, d: 0.2, len: 0.26, br: 1.25 },
        { f: 587.33, d: 0.31, len: 0.78, br: 1.4 },
    ];
    for (const n of brassPhrase) {
        playBrassEnsemble(v, {
            freq: n.f,
            delay: n.d,
            attack: 0.022,
            hold: 0.04,
            decay: n.len,
            gain: 0.34,
            brightness: n.br,
            goldenHall: true,
        });
    }

    // Golden harp glissando sweeping across the horizon
    playHarpGliss(v, [440.0, 587.33, 659.25, 739.99, 880.0, 1174.66, 1479.98], {
        delay: 0.22,
        stepSec: 0.034,
        gain: 0.32,
        decay: 0.88,
        panStart: -0.62,
        panEnd: 0.62,
        shimmerSend: 0.68,
    });

    // Casino gold coin shower & glittering celesta crown
    playCoinShower(v, {
        count: 6,
        delay: 0.3,
        spacing: 0.045,
        pitch: 1.06,
        pitchClimb: 0.04,
        gain: 0.28,
        spread: 0.68,
    });
    [739.99, 880.0, 1174.66, 1318.51].forEach((freq, i) => {
        playCasinoChime(v, {
            freq,
            delay: 0.34 + i * 0.042,
            decay: 0.85,
            gain: 0.25,
            sparkle: 0.95,
            pan: -0.45 + i * 0.3,
            shimmerSend: 0.75,
        });
    });
    playChoirPad(v, [293.66, 440.0, 587.33, 739.99], {
        delay: 0.3,
        attack: 0.1,
        decay: 1.1,
        gain: 0.28,
        goldenHall: true,
    });
}

/** `UiCelebrationFly` — Sparkling magical arc as reward cards/gems fly toward HUD counters. */
export function synthUiCelebrationFly(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: 1, decay: 0.26, gain: 0.24, bright: true });
    playHarpPluck(v, {
        freq: vary(v.rng, 739.99, 0.025),
        delay: 0.01,
        decay: 0.32,
        gain: 0.22,
        pan: -0.32,
        shimmerSend: 0.55,
    });
    playCasinoChime(v, {
        freq: vary(v.rng, 1174.66, 0.03),
        delay: 0.06,
        decay: 0.3,
        gain: 0.18,
        sparkle: 0.85,
        pan: 0.32,
        shimmerSend: 0.62,
    });
}

/** `UiCelebrationImpact` — Addictive casino counter *ka-ching!* when each flying reward lands. */
export function synthUiCelebrationImpact(v: VoiceRenderCtx, _t: CueTrigger): void {
    const sparkleNotes = [1174.66, 1318.51, 1479.98, 1760.0]; // D6, E6, F#6, A6
    const freq = sparkleNotes[Math.floor(v.rng() * sparkleNotes.length)];

    // Punchy tactile bubble pop + chest weight
    playSubDrop(v, 175, 76, 0.075, 0.26);
    playTactilePop(v, {
        material: "wood",
        pitch: 1.28,
        popBend: 1.32,
        gain: 0.32,
        shimmerSend: 0.22,
    });
    // Sparkling celesta + real twin gold coin clink
    playCasinoChime(v, {
        freq,
        delay: 0.006,
        decay: 0.34,
        gain: 0.32,
        sparkle: 0.9,
        shimmerSend: 0.62,
    });
    playCoinShower(v, {
        count: 2,
        delay: 0.012,
        spacing: 0.032,
        pitch: 1.14,
        pitchClimb: 0.06,
        gain: 0.26,
        spread: 0.45,
    });
}

// ── Subtle AAA Menu, Battle & Hero Micro-Interactions (60–65) ────────────────

/**
 * `UiHover` — Feather-light velvet / ivory hover micro-tick (mouse/pen only).
 * `subtype`: `0` = standard button/nav, `1` = primary Battle/Purchase CTA,
 *            `2` = hero/lobby/episode card (`intensity` ∈ [0..1] glides softly in pitch).
 */
export function synthUiHover(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    const slot = clamp(t.intensity, 0, 1);

    if (kind === UiHoverSubtype.Primary) {
        // Primary CTA hover: silky airy glass whisper
        playBuffer(v, {
            buffer: v.bank.woodThock[2],
            rate: vary(v.rng, 1.48, 0.03),
            decay: 0.022,
            gain: 0.1,
            filterType: "highpass",
            filterFreq: 1700,
        });
        playBuffer(v, {
            buffer: v.bank.celestaBell,
            delay: 0.004,
            rate: clamp(1174.66 / CELESTA_REF_HZ, 0.5, 3.8),
            decay: 0.085,
            gain: 0.065,
            filterType: "highpass",
            filterFreq: 2100,
            shimmerSend: 0.3,
        });
        return;
    }

    if (kind === UiHoverSubtype.Card) {
        // Hero / Lobby / Episode card hover: luxury ivory piano-tile brush
        const idx = clamp(Math.round(slot * 5) + 5, 5, MAJOR9_D.length - 2);
        const noteHz = MAJOR9_D[idx];
        playBuffer(v, {
            buffer: v.bank.woodThock[2],
            rate: vary(v.rng, 1.08 + slot * 0.44, 0.03),
            decay: 0.025,
            gain: 0.12,
            filterType: "highpass",
            filterFreq: 1250,
        });
        playBuffer(v, {
            buffer: v.bank.harpPluck,
            delay: 0.003,
            rate: clamp(noteHz / HARP_REF_HZ, 0.5, 3.8),
            decay: 0.09,
            gain: 0.055,
            filterType: "highpass",
            filterFreq: 680,
            shimmerSend: 0.24,
        });
        return;
    }

    // Standard button / tab / dropdown hover: barely-there velvet air brush
    playBuffer(v, {
        buffer: v.bank.woodThock[2],
        rate: vary(v.rng, 1.38, 0.04),
        decay: 0.02,
        gain: 0.1,
        filterType: "highpass",
        filterFreq: 1650,
    });
    playFilteredNoise(v, {
        color: "parchment",
        attack: 0.003,
        decay: 0.022,
        gain: 0.045,
        filterType: "bandpass",
        filterFreq: 2800,
        filterFreqEnd: 3600,
        filterQ: 2.0,
    });
}

/**
 * `UiPress` — Subtle mechanical key depression (`pointerdown`, 0 ms tactile weight).
 * Paired with click release cues to form a cohesive two-part physical switch.
 * `subtype`: `0` = standard, `1` = primary CTA, `2` = soft/back/close.
 */
export function synthUiPress(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    if (kind === UiPressSubtype.Primary) {
        // Authoritative marble-and-brass mechanical switch depression
        playBuffer(v, {
            buffer: v.bank.stoneClick[0],
            rate: vary(v.rng, 0.84, 0.03),
            rateEnd: 0.72,
            decay: 0.032,
            gain: 0.24,
            filterType: "lowpass",
            filterFreq: 1550,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: 0.004,
            rate: vary(v.rng, 0.88, 0.03),
            decay: 0.024,
            gain: 0.11,
            filterType: "lowpass",
            filterFreq: 2200,
        });
        return;
    }

    if (kind === UiPressSubtype.Soft) {
        // Cushioned ghost/back depression
        playBuffer(v, {
            buffer: v.bank.woodThock[1],
            rate: vary(v.rng, 0.76, 0.03),
            decay: 0.024,
            gain: 0.15,
            filterType: "lowpass",
            filterFreq: 860,
        });
        return;
    }

    // Standard carved-wood key depression (soft downward tuck)
    playBuffer(v, {
        buffer: v.bank.woodThock[0],
        rate: vary(v.rng, 0.86, 0.03),
        rateEnd: 0.74,
        decay: 0.026,
        gain: 0.19,
        filterType: "lowpass",
        filterFreq: 1120,
    });
}

/**
 * `UiHeroBrowse` — Subtle aristocratic card flip when browsing heroes or campaign episodes.
 * `subtype`: `0` = available/unlocked hero, `1` = locked hero, `2` = random hero.
 * `intensity`: normalized roster position [0..1] for delicate harmonic variety.
 */
export function synthUiHeroBrowse(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    const slot = clamp(t.intensity, 0, 1);

    if (kind === UiHeroBrowseSubtype.Locked) {
        // Locked commander preview: cooler damped slate settle + delicate brass lock clink
        playBuffer(v, {
            buffer: v.bank.stoneClick[1],
            rate: vary(v.rng, 0.9, 0.03),
            decay: 0.058,
            gain: 0.24,
            filterType: "lowpass",
            filterFreq: 2200,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: 0.01,
            rate: vary(v.rng, 1.12, 0.03),
            decay: 0.048,
            gain: 0.16,
            shimmerSend: 0.14,
        });
        return;
    }

    if (kind === UiHeroBrowseSubtype.Random) {
        // Random hero option: playful crisp wood flick + twin rising harp shimmer
        playTactilePop(v, { material: "wood", pitch: 1.18, popBend: 1.24, gain: 0.22 });
        playHarpPluck(v, { freq: 587.33, delay: 0.008, decay: 0.26, gain: 0.17, pan: -0.2, shimmerSend: 0.42 });
        playHarpPluck(v, { freq: 880.0, delay: 0.044, decay: 0.34, gain: 0.18, pan: 0.2, shimmerSend: 0.5 });
        return;
    }

    // Unlocked hero browse: silky portrait glide + polished marble detent + single harp harmonic
    const noteIdx = clamp(Math.round(slot * 5) + 5, 5, MAJOR9_D.length - 3);
    const noteHz = MAJOR9_D[noteIdx];
    playStereoWhoosh(v, { direction: 1, decay: 0.09, gain: 0.14, bright: true });
    playBuffer(v, {
        buffer: v.bank.stoneClick[0],
        delay: 0.004,
        rate: vary(v.rng, 1.04 + slot * 0.12, 0.03),
        decay: 0.056,
        gain: 0.24,
        shimmerSend: 0.18,
    });
    playHarpPluck(v, {
        freq: noteHz,
        delay: 0.01,
        decay: 0.32,
        gain: 0.16,
        pan: (slot - 0.5) * 0.32,
        shimmerSend: 0.42,
    });
}

/**
 * `UiInfoTab` — Intimate Hero Info dossier tab flip, filter chip snap, or settings switch.
 * `subtype`: `0` = dossier/profile/saga tab flip, `1` = filter chip / segmented pill,
 *            `2` = toggle switch (`intensity >= 0.5` = ON, `< 0.5` = OFF).
 */
export function synthUiInfoTab(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    const ratio = clamp(t.intensity, 0, 1);

    if (kind === UiInfoTabSubtype.Switch) {
        const on = ratio >= 0.5;
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            rate: vary(v.rng, on ? 1.24 : 0.84, 0.03),
            decay: 0.046,
            gain: 0.22,
            shimmerSend: on ? 0.2 : 0.08,
        });
        playCasinoChime(v, {
            freq: on ? 739.99 : 440.0,
            delay: 0.008,
            decay: 0.13,
            gain: 0.095,
            sparkle: on ? 0.3 : 0.1,
            shimmerSend: 0.24,
        });
        return;
    }

    if (kind === UiInfoTabSubtype.Chip) {
        // Filter chip / segmented option / dropdown pick: crisp carved-wood micro-snap
        playBuffer(v, {
            buffer: v.bank.woodThock[2],
            rate: vary(v.rng, 1.06 + ratio * 0.26, 0.035),
            decay: 0.044,
            gain: 0.25,
            shimmerSend: 0.14,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: 0.005,
            rate: vary(v.rng, 1.18, 0.04),
            decay: 0.034,
            gain: 0.1,
        });
        return;
    }

    // Dossier / Info Panel tab flip (Overview / Biography / Achievements): silky vellum page turn
    playFilteredNoise(v, {
        color: "parchment",
        attack: 0.005,
        decay: 0.072,
        gain: 0.14,
        filterType: "bandpass",
        filterFreq: 1450,
        filterFreqEnd: 2950,
        filterQ: 1.5,
        pan: (ratio - 0.5) * 0.36,
        shimmerSend: 0.16,
    });
    playBuffer(v, {
        buffer: v.bank.brassLatch[0],
        delay: 0.006,
        rate: vary(v.rng, 1.08 + ratio * 0.24, 0.025),
        decay: 0.044,
        gain: 0.18,
    });
    const tabNotes = [587.33, 739.99, 880.0]; // D5, F#5, A5
    const tabFreq = tabNotes[clamp(Math.round(ratio * 2), 0, 2)];
    playCasinoChime(v, {
        freq: tabFreq,
        delay: 0.01,
        decay: 0.17,
        gain: 0.11,
        sparkle: 0.28,
        shimmerSend: 0.32,
    });
}

/**
 * `UiPurchaseAction` — Subtle luxury purchase funnel cues (before `UiPurchaseSuccess` jackpot).
 * `subtype`: `0` = open purchase modal, `1` = confirm purchase / spend, `2` = denied / insufficient.
 */
export function synthUiPurchaseAction(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    if (kind === UiPurchaseActionSubtype.Denied) {
        // Dignified soft wooden double-tap (never a harsh buzzer)
        playBuffer(v, {
            buffer: v.bank.woodThock[1],
            rate: vary(v.rng, 0.78, 0.03),
            decay: 0.06,
            gain: 0.26,
        });
        playBuffer(v, {
            buffer: v.bank.woodThock[1],
            delay: 0.068,
            rate: vary(v.rng, 0.66, 0.03),
            decay: 0.07,
            gain: 0.24,
        });
        return;
    }

    if (kind === UiPurchaseActionSubtype.Confirm) {
        // Authoritative purchase confirm: royal marble seal + twin gold coin clink + rising harp
        playTactilePop(v, { material: "stone", pitch: 1.06, popBend: 1.2, gain: 0.34, shimmerSend: 0.24 });
        playCoinShower(v, {
            count: 2,
            delay: 0.012,
            spacing: 0.038,
            pitch: 1.08,
            pitchClimb: 0.06,
            gain: 0.22,
            spread: 0.42,
        });
        playHarpPluck(v, { freq: 587.33, delay: 0.014, decay: 0.3, gain: 0.16, pan: -0.2 });
        playHarpPluck(v, { freq: 880.0, delay: 0.052, decay: 0.38, gain: 0.18, pan: 0.2 });
        return;
    }

    // Open purchase modal: velvet clasp latch + airy reveal + single gold coin glint
    playStereoWhoosh(v, { direction: 1, decay: 0.12, gain: 0.16, bright: true });
    playBuffer(v, {
        buffer: v.bank.brassLatch[0],
        delay: 0.006,
        rate: vary(v.rng, 1.08, 0.03),
        decay: 0.06,
        gain: 0.24,
        shimmerSend: 0.2,
    });
    playBuffer(v, {
        buffer: v.bank.goldCoin[0],
        delay: 0.016,
        rate: vary(v.rng, 1.08, 0.03),
        decay: 0.24,
        gain: 0.17,
        filterType: "highpass",
        filterFreq: 1600,
        shimmerSend: 0.48,
    });
    playCasinoChime(v, { freq: 739.99, delay: 0.018, decay: 0.28, gain: 0.13, sparkle: 0.4 });
}

/**
 * `UiBattleLaunch` — Battle menu submenus, match launch CTA, and tactile map/seed controls.
 * `subtype`: `0` = enter submenu (Campaign/Browser/Create), `1` = launch battle/lobby CTA,
 *            `2` = tactile step (cycle map, dice seed, copy code).
 */
export function synthUiBattleLaunch(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);

    if (kind === UiBattleLaunchSubtype.TactileStep) {
        // Crisp mechanical map-card / seed-dice double detent
        playBuffer(v, {
            buffer: v.bank.woodThock[2],
            rate: vary(v.rng, 1.14, 0.06),
            decay: 0.042,
            gain: 0.22,
            pan: -0.14,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[0],
            delay: 0.022,
            rate: vary(v.rng, 1.24, 0.06),
            decay: 0.044,
            gain: 0.17,
            pan: 0.14,
            shimmerSend: 0.16,
        });
        return;
    }

    if (kind === UiBattleLaunchSubtype.Launch) {
        // Subtle, stirring "To Battle!" confirm: stone/brass crest + silk banner rush + noble chord
        playTactilePop(v, { material: "stone", pitch: 1.06, popBend: 1.22, gain: 0.36, shimmerSend: 0.26 });
        playBuffer(v, {
            buffer: v.bank.brassLatch[1],
            delay: 0.01,
            rate: vary(v.rng, 1.08, 0.03),
            decay: 0.068,
            gain: 0.2,
        });
        playStereoWhoosh(v, { direction: 1, delay: 0.008, decay: 0.16, gain: 0.2, bright: true });
        playHarpPluck(v, { freq: 587.33, delay: 0.012, decay: 0.38, gain: 0.2, pan: -0.24, shimmerSend: 0.48 });
        playCasinoChime(v, {
            freq: 880.0,
            delay: 0.036,
            decay: 0.4,
            gain: 0.2,
            sparkle: 0.6,
            pan: 0.24,
            shimmerSend: 0.56,
        });
        return;
    }

    // Enter Battle submenu (Campaign / Custom Games / Create Game): war-table blueprint unfold
    playStereoWhoosh(v, { direction: 1, decay: 0.13, gain: 0.19, bright: true });
    playBuffer(v, {
        buffer: v.bank.stoneClick[0],
        delay: 0.008,
        rate: vary(v.rng, 1.04, 0.03),
        decay: 0.062,
        gain: 0.26,
        shimmerSend: 0.18,
    });
    playHarpPluck(v, { freq: 440.0, delay: 0.014, decay: 0.28, gain: 0.15, pan: -0.18, shimmerSend: 0.36 });
    playHarpPluck(v, { freq: 587.33, delay: 0.048, decay: 0.34, gain: 0.16, pan: 0.18, shimmerSend: 0.42 });
}
