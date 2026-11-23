/**
 * Combat, Land Expansion, Command & Streak Procedural Recipes (`combat.ts`).
 *
 * Acoustic Identity (Clash Royale / Rise of Kingdoms / Casino Combo Ladder):
 *   - Tactical deployment & attacks hit with real `taikoDrum` leather slaps,
 *     banner cloth sweeps, and forged `steelClash` steel-on-shield sparks.
 *   - `LandExpand` uses rotated carved-wood/marble ASMR pops so rapid border
 *     growth feels tactile and non-fatiguing.
 *   - `CascadeStep` (1..8) and `KillStreak` (2..8) escalate like a luxury slot
 *     multiplier / 3-crown streak: climbing D Major9 celesta chimes, harp plucks,
 *     physical gold coin cascades, and cinema drums.
 */

import {
    clamp,
    MAJOR9_D,
    playBuffer,
    playFilteredNoise,
    semitones,
    vary,
    type VoiceRenderCtx,
} from "../dsp.ts";
import {
    playBrassEnsemble,
    playCasinoChime,
    playChoirPad,
    playCinematicBlast,
    playCoinShower,
    playHarpGliss,
    playHarpPluck,
    playSteelImpact,
    playStereoWhoosh,
    playTactilePop,
    playTaikoHit,
} from "../instruments.ts";
import { CombatAudioSubtype, type CueTrigger } from "../types.ts";

/** `DeployTroops` — Authoritative Clash Royale war-drum double-slam + banner snap + royal crest lock. */
export function synthDeployTroops(v: VoiceRenderCtx, _t: CueTrigger): void {
    const pitch = vary(v.rng, 1.04, 0.06);
    const tapDelay = 0.062 + v.rng() * 0.016;

    // Primary heavy war-drum drop + trailing rallying tap
    playTaikoHit(v, { heavy: false, pitch, gain: 0.56, rimCrack: 0.55, wetSend: 0.34 });
    playTaikoHit(v, { heavy: false, pitch: pitch * 1.18, delay: tapDelay, gain: 0.4, rimCrack: 0.4 });

    // Banner cloth unfurl
    playStereoWhoosh(v, { direction: 1, delay: 0.008, decay: 0.13, gain: 0.24, bright: false });

    // Confirming royal stone emblem + bright harmonic chime
    playTactilePop(v, {
        material: "stone",
        pitch: 1.08,
        delay: tapDelay,
        gain: 0.28,
        shimmerSend: 0.18,
    });
    playCasinoChime(v, {
        freq: vary(v.rng, 587.33, 0.01), // D5
        delay: tapDelay + 0.008,
        decay: 0.28,
        gain: 0.2,
        sparkle: 0.4,
    });
}

const ATTACK_COMBO_WINDOW_SEC = 1.25;
let lastAttackOrderCtxSec = -10;
let attackComboStep = 0;

/**
 * `AttackOrder` — Layered Foley-to-Harmony tap-to-attack (inspired by `UiHeroEquip`
 * and `LandCombat` Human/Nation, with ZERO boomy sub-bass so rapid tapping stays crisp).
 *
 * Bit-packed `subtype` (`(targetVariant << 2) | archetype`):
 *   - `archetype` (`0..3`):
 *       `0` Wilderness — agile jade/wood pop + silk flick + harp/celesta shimmer
 *       `1` Tribe      — taut carved-wood rim crack + bronze latch + tribal pentatonic snap
 *       `2` Nation     — disciplined forged steel + brassLatch[1] + tight royal brass accent
 *       `3` Human      — exhilarating dual stereo steel riposte (L→R) + brass lock + PvP glint
 *   - `targetVariant` (`0..7`): stable per-target timbre & root color so each opponent
 *     (and each wilderness sector) has a distinct, recognizable signature.
 *   - Live combo: consecutive taps within `1.25s` climb `D Major9` (`comboStep 0..7`).
 */
export function synthAttackOrder(v: VoiceRenderCtx, t: CueTrigger): void {
    const rawSub = Math.max(0, Math.round(t.subtype));
    const archetype = rawSub & 0x03;
    const targetVariant = rawSub >= 4 ? (rawSub >> 2) & 0x07 : Math.floor(v.rng() * 8);
    const commit = clamp(t.intensity, 0.15, 1.0);

    // Live combo progression across rapid consecutive taps
    const ctxNow = v.ctx.currentTime;
    const dt = ctxNow - lastAttackOrderCtxSec;
    if (dt > 0.01 && dt <= ATTACK_COMBO_WINDOW_SEC) {
        attackComboStep = Math.min(7, attackComboStep + 1);
    } else if (dt > ATTACK_COMBO_WINDOW_SEC || dt < 0) {
        attackComboStep = 0;
    }
    lastAttackOrderCtxSec = ctxNow;

    const comboScale = [587.33, 659.25, 739.99, 880.0, 987.77, 1174.66, 1318.51, 1479.98]; // D5..F#6
    const comboRatio = attackComboStep / 7;
    // Per-target unique harmonic & pitch signature (0..7)
    const targetPitchBias = 0.92 + targetVariant * 0.024;
    const targetNote = comboScale[(targetVariant + attackComboStep) % comboScale.length];
    const flip = (Math.floor(v.rng() * 2) + attackComboStep) % 2 === 0 ? 1 : -1;

    if (archetype === CombatAudioSubtype.Wilderness) {
        // 0 · Wilderness: crisp carved wood/stone contact + airy flick + joyful D Major9 combo chime
        const woodIdx = targetVariant % 3;
        playBuffer(v, {
            buffer: woodIdx === 0 ? v.bank.stoneClick[0] : v.bank.woodThock[woodIdx],
            rate: vary(v.rng, 1.08 * targetPitchBias + comboRatio * 0.12, 0.03),
            decay: 0.042,
            gain: 0.25 + commit * 0.08,
            filterType: "highpass",
            filterFreq: 420,
            shimmerSend: 0.16,
        });
        playFilteredNoise(v, {
            color: "parchment",
            delay: 0.002,
            attack: 0.003,
            decay: 0.048,
            gain: 0.12 + commit * 0.06,
            filterType: "bandpass",
            filterFreq: 1600 * targetPitchBias,
            filterFreqEnd: 3400 * targetPitchBias,
            filterQ: 1.5,
            pan: flip * 0.12,
        });
        playBuffer(v, {
            buffer: v.bank.harpPluck,
            delay: 0.007,
            rate: clamp(vary(v.rng, targetNote, 0.006) / 293.6648, 0.5, 4.2),
            decay: 0.15 + commit * 0.05,
            gain: 0.17 + commit * 0.07 + comboRatio * 0.05,
            filterType: "highpass",
            filterFreq: 420,
            pan: -flip * 0.14,
            shimmerSend: 0.34,
        });
        if (attackComboStep >= 2) {
            playCasinoChime(v, {
                freq: targetNote,
                delay: 0.014,
                decay: 0.16,
                gain: 0.075 + comboRatio * 0.055,
                sparkle: 0.35 + comboRatio * 0.25,
                shimmerSend: 0.38,
            });
        }
        return;
    }

    if (archetype === CombatAudioSubtype.Tribe) {
        // 1 · Tribe: taut wood-and-hide rim snap + mechanical bronze latch + tribal pentatonic accent
        const woodIdx = (targetVariant % 2) + 1; // woodThock[1] or [2]
        playBuffer(v, {
            buffer: v.bank.woodThock[woodIdx],
            rate: vary(v.rng, 0.98 * targetPitchBias + comboRatio * 0.1, 0.04),
            decay: 0.068,
            gain: 0.34 + commit * 0.1,
            filterType: "highpass",
            filterFreq: 190, // tight chest punch, zero boomy sub mud
            wetSend: 0.22,
        });
        playBuffer(v, {
            buffer: v.bank.woodThock[2],
            delay: 0.011,
            rate: vary(v.rng, 1.38 * targetPitchBias, 0.05),
            decay: 0.036,
            gain: 0.22 + commit * 0.08,
            filterType: "highpass",
            filterFreq: 760,
            pan: flip * 0.22,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[targetVariant % 2],
            delay: 0.016,
            rate: vary(v.rng, 1.06 + comboRatio * 0.16, 0.04),
            decay: 0.048,
            gain: 0.2 + commit * 0.08,
            shimmerSend: 0.18,
        });
        playCasinoChime(v, {
            freq: targetNote * 0.75,
            delay: 0.02,
            decay: 0.14,
            gain: 0.09 + comboRatio * 0.06,
            sparkle: 0.28 + comboRatio * 0.2,
            shimmerSend: 0.28,
        });
        return;
    }

    if (archetype === CombatAudioSubtype.Nation) {
        // 2 · Nation: compact UiHeroEquip-inspired disciplined steel + brassLatch + tight brass lock
        playBuffer(v, {
            buffer: v.bank.woodThock[2],
            rate: vary(v.rng, 1.15 * targetPitchBias, 0.04),
            decay: 0.042,
            gain: 0.22 + commit * 0.07,
            filterType: "highpass",
            filterFreq: 260,
        });
        playSteelImpact(v, {
            heavy: false,
            pitch: vary(v.rng, 1.06 * targetPitchBias + comboRatio * 0.12, 0.05),
            delay: 0.005,
            gain: 0.22 + commit * 0.08,
            wetSend: 0.22,
        });
        playBuffer(v, {
            buffer: v.bank.brassLatch[1],
            delay: 0.013,
            rate: vary(v.rng, 0.98 + targetVariant * 0.03 + comboRatio * 0.12, 0.03),
            decay: 0.055,
            gain: 0.26 + commit * 0.08,
            shimmerSend: 0.2,
        });
        // Tight royal brass harmonic accent (high-passed cleanly so it never muddies rapid taps)
        const brassNotes = [293.66, 329.63, 369.99, 440.0]; // D4, E4, F#4, A4
        const brassFreq = brassNotes[(targetVariant + Math.floor(attackComboStep / 2)) % brassNotes.length];
        playBrassEnsemble(v, {
            freq: brassFreq,
            delay: 0.02,
            attack: 0.014,
            hold: 0.016,
            decay: 0.13 + commit * 0.04,
            gain: 0.16 + commit * 0.06,
            brightness: 1.2 + comboRatio * 0.25,
            goldenHall: true,
        });
        playCasinoChime(v, {
            freq: targetNote,
            delay: 0.024,
            decay: 0.18,
            gain: 0.1 + comboRatio * 0.06,
            sparkle: 0.45,
            shimmerSend: 0.36,
        });
        return;
    }

    // 3 · Human (PvP): exhilarating dual stereo steel riposte (L→R) + brass lock + combo sparkle!
    playSteelImpact(v, {
        heavy: false,
        pitch: vary(v.rng, 1.1 * targetPitchBias + comboRatio * 0.12, 0.05),
        gain: 0.24 + commit * 0.09,
        pan: -0.34 * flip,
        wetSend: 0.24,
    });
    playBuffer(v, {
        buffer: v.bank.brassLatch[targetVariant % 2],
        delay: 0.008,
        rate: vary(v.rng, 1.12 + comboRatio * 0.16, 0.03),
        decay: 0.048,
        gain: 0.24 + commit * 0.08,
        shimmerSend: 0.22,
    });
    playSteelImpact(v, {
        heavy: false,
        pitch: vary(v.rng, 1.3 * targetPitchBias + comboRatio * 0.15, 0.05),
        delay: 0.024,
        gain: 0.2 + commit * 0.08,
        pan: 0.36 * flip,
        wetSend: 0.28,
    });
    playCasinoChime(v, {
        freq: targetNote,
        delay: 0.028,
        decay: 0.2,
        gain: 0.13 + commit * 0.06 + comboRatio * 0.06,
        sparkle: 0.55 + comboRatio * 0.25,
        shimmerSend: 0.42,
    });
}

/** `AttackStalled` — Muted shield-wall halt, heavy wood crunch, and descending dark horn sigh. */
export function synthAttackStalled(v: VoiceRenderCtx, t: CueTrigger): void {
    const sev = clamp(t.intensity, 0.25, 1.0);
    playBuffer(v, {
        buffer: v.bank.woodThock[1],
        rate: vary(v.rng, 0.68, 0.06),
        gain: 0.42 + sev * 0.12,
        wetSend: 0.24,
    });
    playBrassEnsemble(v, {
        freq: vary(v.rng, 220.0, 0.03), // A3
        freqEnd: 164.81, // E3
        delay: 0.02,
        attack: 0.022,
        hold: 0.02,
        decay: 0.28 + sev * 0.08,
        gain: 0.26 + sev * 0.1,
        brightness: 0.72,
        goldenHall: false,
    });
    playFilteredNoise(v, {
        color: "brown",
        attack: 0.012,
        decay: 0.22,
        gain: 0.22 * sev,
        filterType: "bandpass",
        filterFreq: 420,
        filterFreqEnd: 140,
        filterQ: 1.0,
    });
}

/** `UnderAttack` — Spine-tingling dual stereo war-horn alert + fortress drum + alarm bronze bell. */
export function synthUnderAttack(v: VoiceRenderCtx, t: CueTrigger): void {
    const sev = clamp(t.intensity, 0.25, 1.0);
    const base = vary(v.rng, 196.0, 0.02); // G3

    // Urgent war-drum heartbeat
    playTaikoHit(v, { heavy: true, pitch: 0.9, gain: 0.5 + sev * 0.14, wetSend: 0.42 });
    playTaikoHit(v, { heavy: false, pitch: 1.02, delay: 0.16, gain: 0.36 + sev * 0.1 });

    // Tense minor-second stereo brass blasts (G3 -> Ab3 classic battle alarm)
    for (let i = 0; i < 2; i++) {
        const delay = i * 0.165;
        const pitch = i === 0 ? base : semitones(base, 1);
        playBrassEnsemble(v, {
            freq: pitch,
            freqEnd: pitch * 0.988,
            delay,
            attack: 0.022,
            hold: 0.05,
            decay: 0.24 + sev * 0.08,
            gain: (0.36 + sev * 0.14) * (i === 1 ? 1.12 : 0.94),
            brightness: 1.15 + sev * 0.25,
            goldenHall: false,
        });
        // Ringing fortress steel warning accent
        playSteelImpact(v, {
            heavy: false,
            pitch: i === 0 ? 1.1 : 1.24,
            delay,
            gain: 0.16 + sev * 0.06,
            wetSend: 0.4,
        });
    }
}

/**
 * `LandExpand` — Addictive, non-fatiguing ASMR territorial capture pop.
 * Rotates across 3 carved-wood/ivory perspectives so rapid expansion feels crisp & organic.
 */
export function synthLandExpand(v: VoiceRenderCtx, t: CueTrigger): void {
    const scale = clamp(t.intensity, 0.2, 1.0);
    const variant = Math.floor(v.rng() * 3);
    const notes = [369.99, 440.0, 587.33]; // F#4, A4, D5 warm soft triad

    playTactilePop(v, {
        material: variant === 2 ? "stone" : "wood",
        pitch: vary(v.rng, 0.96 + variant * 0.1, 0.06),
        popBend: 1.15,
        noteHz: notes[variant],
        gain: 0.28 + scale * 0.14,
        shimmerSend: 0.1,
    });
    // Subtle soil/parchment brush
    playFilteredNoise(v, {
        color: "parchment",
        attack: 0.004,
        decay: 0.065,
        gain: 0.09 + scale * 0.05,
        filterType: "bandpass",
        filterFreq: vary(v.rng, 1150 + variant * 240, 0.15),
        filterFreqEnd: 520,
        filterQ: 1.2,
    });
}

/**
 * `LandCombat` — Visceral battlefield skirmish differentiated by opponent archetype.
 * `subtype`: Tribe (wood/hide drum), Nation (forged steel clash), Human (dual stereo PvP riposte).
 */
export function synthLandCombat(v: VoiceRenderCtx, t: CueTrigger): void {
    const scale = clamp(t.intensity, 0.2, 1.0);
    const variant = Math.floor(v.rng() * 3);
    const kind = Math.round(t.subtype);

    // Compact battlefield impact weight
    playTaikoHit(v, {
        heavy: false,
        pitch: vary(v.rng, kind === CombatAudioSubtype.Human ? 1.14 : 1.0, 0.08),
        gain: 0.24 + scale * 0.16,
        rimCrack: kind === CombatAudioSubtype.Tribe ? 0.6 : 0.25,
        wetSend: 0.22,
    });

    if (kind === CombatAudioSubtype.Tribe) {
        // Taut wood-and-hide skirmish drum + spear knock
        playBuffer(v, {
            buffer: v.bank.woodThock[variant],
            rate: vary(v.rng, 0.92 + variant * 0.15, 0.08),
            gain: 0.34 + scale * 0.12,
            wetSend: 0.24,
        });
    } else if (kind === CombatAudioSubtype.Nation || kind === CombatAudioSubtype.Human) {
        // Forged sword-on-shield steel clash
        playSteelImpact(v, {
            heavy: variant === 2,
            pitch: vary(v.rng, kind === CombatAudioSubtype.Human ? 1.12 : 0.96, 0.12),
            gain: 0.26 + scale * 0.14,
            pan: kind === CombatAudioSubtype.Human ? -0.35 : 0,
            wetSend: 0.28,
        });
        if (kind === CombatAudioSubtype.Human) {
            // Secondary swift counter-parry on opposite ear for exhilarating PvP crunch!
            playSteelImpact(v, {
                heavy: false,
                pitch: vary(v.rng, 1.28, 0.1),
                delay: 0.032 + v.rng() * 0.02,
                gain: 0.22 + scale * 0.12,
                pan: 0.38,
                wetSend: 0.32,
            });
        }
    } else {
        // Crisp tactical skirmish rustle & shield knock
        playBuffer(v, {
            buffer: v.bank.woodThock[1],
            rate: vary(v.rng, 0.88, 0.1),
            gain: 0.28 + scale * 0.1,
            wetSend: 0.22,
        });
        playFilteredNoise(v, {
            color: "crackle",
            attack: 0.003,
            decay: 0.075,
            gain: 0.14 + scale * 0.08,
            filterType: "bandpass",
            filterFreq: vary(v.rng, 1650, 0.18),
            filterFreqEnd: 620,
            filterQ: 1.3,
        });
    }
}

/** `CounterAttack` — Electrifying unsheathed-steel riposte + heroic brass surge. */
export function synthCounterAttack(v: VoiceRenderCtx, _t: CueTrigger): void {
    playTaikoHit(v, { heavy: false, pitch: 1.12, gain: 0.46, rimCrack: 0.65 });
    playSteelImpact(v, { heavy: false, pitch: 1.28, delay: 0.005, gain: 0.36, wetSend: 0.34 });
    playStereoWhoosh(v, { direction: 1, decay: 0.12, gain: 0.26, bright: true });
    playBrassEnsemble(v, {
        freq: 392.0, // G4
        freqEnd: 587.33, // D5 triumphant thrust
        delay: 0.015,
        attack: 0.016,
        hold: 0.02,
        decay: 0.22,
        gain: 0.3,
        brightness: 1.35,
        goldenHall: true,
    });
}

/** `AttackCancelled` — Cushioned directional pullback whoosh + muted wooden recall knock. */
export function synthAttackCancelled(v: VoiceRenderCtx, _t: CueTrigger): void {
    playStereoWhoosh(v, { direction: -1, decay: 0.12, gain: 0.22, bright: false });
    playTactilePop(v, {
        material: "wood",
        pitch: 0.8,
        popBend: 0.82,
        noteHz: 293.66,
        delay: 0.012,
        gain: 0.3,
    });
}

/**
 * `CascadeStep` — Pure Supercell / Casino Combo Escalator (`subtype`: chain step `1..8`).
 * Climbs `MAJOR9_D` while dynamically stacking harp plucks, gold coin clinks,
 * stereo sparkle, and chest-thumping taiko weight at peak momentum!
 */
export function synthCascadeStep(v: VoiceRenderCtx, t: CueTrigger): void {
    const step = clamp(Math.round(t.subtype), 1, 8);
    const idx = clamp(step + 3, 4, MAJOR9_D.length - 2);
    const freq = MAJOR9_D[idx];
    const ratio = (step - 1) / 7;

    // Tactile pop anchor so every combo hit feels physically crisp
    playTactilePop(v, {
        material: step >= 5 ? "stone" : "wood",
        pitch: 1.0 + ratio * 0.42,
        popBend: 1.24,
        gain: 0.28 + ratio * 0.1,
        shimmerSend: 0.18 + ratio * 0.2,
    });

    // Glassy celesta main pitch climbing the golden ladder
    playCasinoChime(v, {
        freq,
        decay: 0.34 + ratio * 0.32,
        gain: 0.3 + ratio * 0.14,
        sparkle: 0.35 + ratio * 0.65,
        shimmerSend: 0.42 + ratio * 0.3,
    });

    // Step 3+: Karplus-Strong harp fifth blooms alongside!
    if (step >= 3) {
        playHarpPluck(v, {
            freq: MAJOR9_D[Math.min(idx + 1, MAJOR9_D.length - 1)],
            delay: 0.022,
            decay: 0.44 + ratio * 0.22,
            gain: 0.24 + ratio * 0.08,
            pan: (step % 2 === 0 ? 1 : -1) * 0.35,
            shimmerSend: 0.55,
        });
    }

    // Step 4+: Casino gold coin clinks begin sprinkling in!
    if (step >= 4) {
        playCoinShower(v, {
            count: step >= 7 ? 3 : step >= 5 ? 2 : 1,
            delay: 0.018,
            spacing: 0.032,
            pitch: 1.02 + ratio * 0.22,
            pitchClimb: 0.05,
            gain: 0.2 + ratio * 0.1,
            spread: 0.5,
        });
    }

    // Step 6+: Grounding taiko chest punch + upper harmony chord!
    if (step >= 6) {
        playTaikoHit(v, { heavy: step >= 8, pitch: 1.02, gain: 0.42 + (step - 6) * 0.08, rimCrack: 0.5 });
        playCasinoChime(v, {
            freq: MAJOR9_D[Math.min(idx + 2, MAJOR9_D.length - 1)],
            delay: 0.042,
            decay: 0.58,
            gain: 0.22,
            sparkle: 0.95,
            pan: 0.38,
            shimmerSend: 0.72,
        });
    }
}

/**
 * `KillStreak` — Heroic Jackpot / Multi-Kill Fanfare Stab (`subtype`: tier `2..8`).
 * Scales dramatically from crisp x2 brass+chime up to a full `RAMPAGE!` celebration burst!
 */
export function synthKillStreak(v: VoiceRenderCtx, t: CueTrigger): void {
    const tier = clamp(Math.round(t.subtype), 2, 8);
    const level = (tier - 2) / 6;
    const baseIdx = clamp(tier + 2, 4, MAJOR9_D.length - 4);
    const chord =
        tier >= 5
            ? [MAJOR9_D[baseIdx], MAJOR9_D[baseIdx + 1], MAJOR9_D[baseIdx + 2], MAJOR9_D[baseIdx + 3]]
            : [MAJOR9_D[baseIdx], MAJOR9_D[baseIdx + 2]];

    // Deep cinema impact slam
    playTaikoHit(v, {
        heavy: tier >= 4,
        pitch: 1.04 - level * 0.1,
        gain: 0.52 + level * 0.16,
        rimCrack: 0.75,
        wetSend: 0.45,
    });

    // Heroic orchestral brass stab
    playBrassEnsemble(v, {
        freq: chord[0] * 0.5,
        delay: 0.015,
        attack: 0.02,
        hold: 0.05,
        decay: 0.38 + level * 0.28,
        gain: 0.34 + level * 0.12,
        brightness: 1.2 + level * 0.3,
        goldenHall: true,
    });

    // Ascending celesta sparkle cascade
    chord.forEach((freq, i) => {
        playCasinoChime(v, {
            freq,
            delay: 0.015 + i * 0.044,
            decay: 0.45 + i * 0.08,
            gain: 0.28 + level * 0.08,
            sparkle: 0.65 + level * 0.35,
            pan: -0.4 + (chord.length > 1 ? (0.8 * i) / (chord.length - 1) : 0.4),
            shimmerSend: 0.66,
        });
    });

    // Casino gold coin burst scaling with streak tier!
    playCoinShower(v, {
        count: Math.min(6, 2 + Math.floor(tier * 0.65)),
        delay: 0.045,
        spacing: 0.036,
        pitch: 1.05 + level * 0.18,
        pitchClimb: 0.05,
        gain: 0.22 + level * 0.1,
        spread: 0.6,
    });

    // High-tier RAMPAGE / KING sweeps a golden harp gliss!
    if (tier >= 5) {
        playHarpGliss(v, chord, {
            delay: 0.03,
            stepSec: 0.03,
            gain: 0.28,
            decay: 0.72,
            shimmerSend: 0.68,
        });
    }
}

/**
 * `PlayerEliminated` — Monumental kingdom fall differentiated by victim archetype.
 * `subtype`: 0 = Bot/Tribe, 1 = Nation, 2 = Human player.
 */
export function synthPlayerEliminated(v: VoiceRenderCtx, t: CueTrigger): void {
    const kind = Math.round(t.subtype);
    const isHuman = kind >= 2;
    const isNation = kind === 1;

    if (isHuman) {
        // Full cinema kingdom-collapse event!
        playCinematicBlast(v, { scale: 0.72, tailSec: 0.75, gain: 0.56 });
        playSteelImpact(v, { heavy: true, pitch: 0.78, delay: 0.015, gain: 0.44, wetSend: 0.55 });
        playBrassEnsemble(v, {
            freq: 146.83, // D3 monumental dark brass
            freqEnd: 110.0, // A2 solemn drop
            delay: 0.04,
            attack: 0.03,
            hold: 0.12,
            decay: 0.85,
            gain: 0.46,
            brightness: 0.92,
            goldenHall: false,
        });
        playChoirPad(v, [146.83, 174.61, 220.0], {
            delay: 0.08,
            attack: 0.08,
            decay: 0.95,
            gain: 0.26,
            filterFreq: 1100,
            goldenHall: false,
        });
        // Royal conquest reward coin ping on top so eliminating a rival STILL feels triumphant!
        playCoinShower(v, {
            count: 3,
            delay: 0.14,
            spacing: 0.042,
            pitch: 1.0,
            gain: 0.2,
            spread: 0.48,
        });
        return;
    }

    // Nation (1) or Bot/Tribe (0) elimination
    playTaikoHit(v, {
        heavy: true,
        pitch: isNation ? 0.88 : 0.98,
        gain: isNation ? 0.62 : 0.48,
        rimCrack: 0.6,
        wetSend: 0.48,
    });
    playBuffer(v, {
        buffer: v.bank.stoneClick[1],
        delay: 0.012,
        rate: 0.72,
        gain: 0.36,
        wetSend: 0.35,
    });
    if (isNation) {
        playSteelImpact(v, { heavy: true, pitch: 0.9, delay: 0.02, gain: 0.34, wetSend: 0.44 });
        playBrassEnsemble(v, {
            freq: 164.81, // E3
            freqEnd: 146.83, // D3
            delay: 0.035,
            attack: 0.025,
            hold: 0.04,
            decay: 0.48,
            gain: 0.34,
            brightness: 0.88,
            goldenHall: false,
        });
    } else {
        playFilteredNoise(v, {
            color: "crackle",
            delay: 0.015,
            attack: 0.008,
            decay: 0.28,
            gain: 0.24,
            filterType: "lowpass",
            filterFreq: 950,
            filterFreqEnd: 180,
            wetSend: 0.3,
        });
    }
}
