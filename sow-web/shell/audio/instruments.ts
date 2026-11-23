/**
 * AAA Studio Macro-Instruments (`instruments.ts`).
 *
 * High-level musical & foley building blocks shared across all gameplay and UI
 * recipes. Every instrument combines physical-modeling micro-buffers, custom
 * Fourier wavetables, stereo width, transient shaping, and dual studio reverbs
 * (`arenaIR` + `casinoHallIR`) so cues feel Supercell / Lilith / Casino-grade
 * rather than synthetic oscillators.
 */

import {
    CELESTA_REF_HZ,
    clamp,
    HARP_REF_HZ,
    playBuffer,
    playFilteredNoise,
    playFmTone,
    playSubDrop,
    playTone,
    vary,
    type VoiceRenderCtx,
} from "./dsp.ts";

// ── 1. Tactile Wood / Ivory / Stone / Brass UI Pop ───────────────────────────

export interface TactilePopOptions {
    material: "wood" | "stone" | "brass";
    /** Base pitch multiplier (1.0 = natural, >1 brighter, <1 deeper). */
    pitch?: number;
    /** Optional upward/downward spring pitch ratio for bouncy UI pop. */
    popBend?: number;
    /** Optional subtle warm marimba note Hz blended behind the foley click. */
    noteHz?: number;
    delay?: number;
    gain: number;
    pan?: number;
    shimmerSend?: number;
}

/**
 * Signature Supercell / Lilith tactile UI sound: combines a pre-baked physical
 * mallet/stone/latch foley transient with a tight pitch-bent body pop and
 * optional woody marimba harmonic warmth. Never sounds like a raw sine beep.
 */
export function playTactilePop(v: VoiceRenderCtx, opt: TactilePopOptions): void {
    const delay = opt.delay ?? 0;
    const pitch = vary(v.rng, opt.pitch ?? 1.0, 0.035);
    const variant = Math.floor(v.rng() * 3);

    const buf =
        opt.material === "wood"
            ? v.bank.woodThock[variant % 3]
            : opt.material === "stone"
              ? v.bank.stoneClick[variant % 2]
              : v.bank.brassLatch[variant % 2];

    // Layer 1: Physical surface foley transient
    playBuffer(v, {
        buffer: buf,
        delay,
        rate: pitch,
        rateEnd: opt.popBend ? pitch * opt.popBend : undefined,
        decay: opt.material === "wood" ? 0.095 : 0.115,
        gain: opt.gain * 0.92,
        pan: opt.pan,
        shimmerSend: opt.shimmerSend ?? 0.06,
    });

    // Layer 2: Tight tactile low-mid "thock/bubble" body (22 ms, physical wood/stone weight)
    const bodyStart = opt.material === "brass" ? 245 : opt.material === "stone" ? 185 : 210;
    const bodyEnd = opt.popBend && opt.popBend > 1 ? bodyStart * 1.42 : bodyStart * 0.54;
    playTone(v, {
        wave: v.bank.marimbaWave,
        type: "triangle",
        freq: bodyStart * pitch,
        freqEnd: bodyEnd * pitch,
        delay,
        attack: 0.002,
        decay: 0.024,
        gain: opt.gain * 0.3,
        filterType: "lowpass",
        filterFreq: 860,
        pan: opt.pan,
    });

    // Layer 3: Subtle brief woody marimba sheen (42 ms, zero lingering beep)
    if (opt.noteHz) {
        playTone(v, {
            wave: v.bank.marimbaWave,
            type: "triangle",
            freq: vary(v.rng, opt.noteHz, 0.012),
            freqStart: opt.noteHz * (opt.popBend && opt.popBend < 1 ? 1.05 : 0.94),
            pitchSnapTime: 0.012,
            delay: delay + 0.002,
            attack: 0.002,
            decay: 0.044,
            gain: opt.gain * 0.19,
            filterType: "lowpass",
            filterFreq: 2200,
            pan: opt.pan,
            shimmerSend: opt.shimmerSend ?? 0.12,
        });
    }
}

// ── 2. Luxury Casino / Supercell Celesta & Sparkle Chime ─────────────────────

export interface CasinoChimeOptions {
    freq: number;
    delay?: number;
    decay?: number;
    gain: number;
    /** Adds wide stereo octave shimmer glitter (0..1). */
    sparkle?: number;
    pan?: number;
    shimmerSend?: number;
}

/**
 * Lush glassy Celesta / Vibraphone bell pitched cleanly from `celestaBell` +
 * optional wide stereo octave sparkle blooming into the 1.65 s golden hall.
 */
export function playCasinoChime(v: VoiceRenderCtx, opt: CasinoChimeOptions): void {
    const delay = opt.delay ?? 0;
    const baseRate = clamp(opt.freq / CELESTA_REF_HZ, 0.25, 3.8);
    const sparkle = opt.sparkle ?? 0.45;
    const pan = opt.pan ?? 0;

    // Warm glassy fundamental mallet strike
    playBuffer(v, {
        buffer: v.bank.celestaBell,
        delay,
        rate: vary(v.rng, baseRate, 0.004),
        decay: opt.decay ?? 0.55,
        gain: opt.gain,
        pan,
        shimmerSend: opt.shimmerSend ?? 0.48,
    });

    // Wide stereo upper-octave fairy-dust shimmer (opposite pan + 9 ms Haas spread for lush width)
    if (sparkle > 0.05 && baseRate * 2 <= 3.9) {
        const oppPan = Math.abs(pan) > 0.15 ? -pan * 0.9 : (v.rng() > 0.5 ? 0.44 : -0.44);
        playBuffer(v, {
            buffer: v.bank.celestaBell,
            delay: delay + 0.009,
            rate: baseRate * 2.004,
            decay: (opt.decay ?? 0.48) * 0.55,
            gain: opt.gain * 0.32 * sparkle,
            filterType: "highpass",
            filterFreq: 1850,
            pan: clamp(oppPan, -0.85, 0.85),
            shimmerSend: 0.68,
        });
    }
}

// ── 3. Karplus-Strong Concert Harp Pluck & Glissando ─────────────────────────

export interface HarpPluckOptions {
    freq: number;
    delay?: number;
    decay?: number;
    gain: number;
    pan?: number;
    shimmerSend?: number;
}

/** Plays a single Karplus-Strong physical plucked harp string at exact pitch. */
export function playHarpPluck(v: VoiceRenderCtx, opt: HarpPluckOptions): void {
    const rate = clamp(opt.freq / HARP_REF_HZ, 0.35, 3.8);
    playBuffer(v, {
        buffer: v.bank.harpPluck,
        delay: opt.delay ?? 0,
        rate: vary(v.rng, rate, 0.004),
        decay: opt.decay ?? 0.72,
        gain: opt.gain,
        pan: opt.pan,
        shimmerSend: opt.shimmerSend ?? 0.44,
    });
}

/**
 * Rapid sweeping stereo orchestral harp glissando (`L → R` or `R → L`).
 * Signature flourish for building completions, alliances, purchases, and victories.
 */
export function playHarpGliss(
    v: VoiceRenderCtx,
    notes: readonly number[],
    opt: {
        delay?: number;
        stepSec?: number;
        gain: number;
        decay?: number;
        panStart?: number;
        panEnd?: number;
        shimmerSend?: number;
    },
): void {
    const baseDelay = opt.delay ?? 0;
    const step = opt.stepSec ?? 0.036;
    const panStart = opt.panStart ?? -0.45;
    const panEnd = opt.panEnd ?? 0.45;
    const lastIdx = Math.max(1, notes.length - 1);

    for (let i = 0; i < notes.length; i++) {
        const u = i / lastIdx;
        // Subtle human timing rubato & crescendo toward the top sparkle note
        const jitter = (v.rng() - 0.5) * 0.004;
        const vel = 0.78 + 0.32 * u;
        playHarpPluck(v, {
            freq: notes[i],
            delay: baseDelay + i * step + Math.max(0, jitter),
            decay: (opt.decay ?? 0.65) + u * 0.25,
            gain: opt.gain * vel,
            pan: panStart + (panEnd - panStart) * u,
            shimmerSend: opt.shimmerSend ?? 0.52,
        });
    }
}

// ── 4. Casino Gold Coin Shower ───────────────────────────────────────────────

export interface CoinShowerOptions {
    /** Number of individual physical coins (1..7). */
    count: number;
    delay?: number;
    /** Base interval between coin clinks in seconds (default 0.038). */
    spacing?: number;
    /** Pitch center (1.0 = sovereign gold, 1.15 = bright sparkling coins). */
    pitch?: number;
    /** Progressive ascending pitch climb per coin for slot-machine excitement. */
    pitchClimb?: number;
    gain: number;
    spread?: number;
}

/**
 * Cascading physical gold coin clinks with beating disc eigenmodes and stereo scatter.
 * Delivers instant Supercell / casino dopamine without ever sounding synthetic.
 */
export function playCoinShower(v: VoiceRenderCtx, opt: CoinShowerOptions): void {
    const count = clamp(Math.round(opt.count), 1, 8);
    const baseDelay = opt.delay ?? 0;
    const spacing = opt.spacing ?? 0.038;
    const pitchBase = opt.pitch ?? 1.0;
    const climb = opt.pitchClimb ?? 0.035;
    const spread = opt.spread ?? 0.52;

    let t = baseDelay;
    for (let i = 0; i < count; i++) {
        const buf = v.bank.goldCoin[(i + Math.floor(v.rng() * 3)) % 3];
        const pan = (v.rng() * 2 - 1) * spread;
        const rate = vary(v.rng, pitchBase * (1 + i * climb), 0.045);
        const accent = i === count - 1 ? 1.12 : 0.8 + v.rng() * 0.3;

        playBuffer(v, {
            buffer: buf,
            delay: t,
            rate,
            decay: 0.28 + v.rng() * 0.14,
            gain: opt.gain * accent,
            filterType: "highpass",
            filterFreq: 1400,
            pan,
            shimmerSend: 0.46,
        });

        // Natural gravity/bounce spacing jitter
        t += spacing * (0.72 + v.rng() * 0.55);
    }
}

// ── 5. Stereo Orchestral Brass / War Horn Ensemble ───────────────────────────

export interface BrassEnsembleOptions {
    freq: number;
    freqEnd?: number;
    delay?: number;
    attack?: number;
    hold?: number;
    decay: number;
    gain: number;
    /** Brightness multiplier for the brass bell sweep (0.6 = dark horn, 1.4 = heroic trumpet). */
    brightness?: number;
    /** Stereo width of the 3-voice unison ensemble (default 0.38). */
    width?: number;
    /** Use lush hall shimmer (true for Victory/Alliance) vs arena reverb (false for War Horns). */
    goldenHall?: boolean;
}

/**
 * 3-voice stereo-detuned orchestral brass section using custom Fourier `brassWave`,
 * natural lip-embouchure upward pitch scoop (-22 cents -> 0 in 34 ms), brass breath
 * chiff transient, and sweeping resonant bell filter. Replaces raw NES sawtooths.
 */
export function playBrassEnsemble(v: VoiceRenderCtx, opt: BrassEnsembleOptions): void {
    const delay = opt.delay ?? 0;
    const attack = Math.max(0.016, opt.attack ?? 0.032);
    const hold = opt.hold ?? 0.04;
    const decay = Math.max(0.12, opt.decay);
    const bright = clamp(opt.brightness ?? 1.0, 0.5, 1.7);
    const width = opt.width ?? 0.36;

    // 3 detuned unison brass players across L / C / R
    const voices = [
        { cents: 0, pan: 0, gainScale: 0.44 },
        { cents: -9.5, pan: -width, gainScale: 0.3 },
        { cents: 10.2, pan: width, gainScale: 0.3 },
    ];

    const startCutoff = clamp(opt.freq * 2.2 * bright, 260, 1800);
    const peakCutoff = clamp(opt.freq * 6.8 * bright, 680, 5200);
    const endCutoff = clamp(opt.freq * 2.6 * bright, 300, 1600);

    for (const voice of voices) {
        const f = opt.freq * Math.pow(2, voice.cents / 1200);
        // Embouchure scoop: real brass players lip slightly into the note over ~32 ms
        const fScoop = f * 0.986;

        playTone(v, {
            wave: v.bank.brassWave,
            type: "sawtooth",
            freq: f,
            freqStart: fScoop,
            pitchSnapTime: 0.034,
            freqEnd: opt.freqEnd ? opt.freqEnd * Math.pow(2, voice.cents / 1200) : undefined,
            delay: delay + (voice.pan === 0 ? 0 : 0.004),
            attack,
            hold,
            decay,
            gain: opt.gain * voice.gainScale,
            filterType: "lowpass",
            filterFreq: peakCutoff,
            filterFreqEnd: endCutoff,
            filterQ: 1.55,
            pan: voice.pan,
            wetSend: opt.goldenHall ? 0.14 : 0.42,
            shimmerSend: opt.goldenHall ? 0.48 : 0.08,
        });
    }

    // Subtle brass mouthpiece breath "chiff" at attack (makes it sound acoustic!)
    playFilteredNoise(v, {
        color: "pink",
        delay,
        attack: 0.008,
        decay: Math.min(0.085, attack * 2.2),
        gain: opt.gain * 0.18,
        filterType: "bandpass",
        filterFreq: clamp(startCutoff * 1.4, 500, 2600),
        filterFreqEnd: clamp(peakCutoff * 0.65, 400, 1800),
        filterQ: 2.0,
        wetSend: opt.goldenHall ? 0.08 : 0.25,
    });
}

// ── 6. Velvet Stereo Choir / Harmonic Pad ────────────────────────────────────

/** Lush stereo detuned pad (`choirWave`) for Victory, Alliance, Defeat, and Celebration tails. */
export function playChoirPad(
    v: VoiceRenderCtx,
    freqs: readonly number[],
    opt: {
        delay?: number;
        attack?: number;
        decay: number;
        gain: number;
        filterFreq?: number;
        goldenHall?: boolean;
    },
): void {
    const delay = opt.delay ?? 0;
    const attack = opt.attack ?? 0.075;
    const perNoteGain = opt.gain / Math.max(1, Math.sqrt(freqs.length));

    for (let i = 0; i < freqs.length; i++) {
        const pan = freqs.length > 1 ? -0.42 + (0.84 * i) / (freqs.length - 1) : 0;
        const f = freqs[i];
        playTone(v, {
            wave: v.bank.choirWave,
            type: "triangle",
            freq: f,
            detuneCents: (i % 2 === 0 ? -5.5 : 5.5),
            delay: delay + i * 0.012,
            attack,
            decay: opt.decay,
            gain: perNoteGain,
            filterType: "lowpass",
            filterFreq: opt.filterFreq ?? 1850,
            filterFreqEnd: (opt.filterFreq ?? 1850) * 0.45,
            filterQ: 0.7,
            pan,
            wetSend: opt.goldenHall ? 0.1 : 0.45,
            shimmerSend: opt.goldenHall ? 0.62 : 0.12,
        });
    }
}

// ── 7. Cinema War-Taiko Drum Impact ──────────────────────────────────────────

export interface TaikoHitOptions {
    /** `0` = punchy tactical war drum, `1` = monumental deep odaiko slam. */
    heavy?: boolean;
    pitch?: number;
    delay?: number;
    gain: number;
    /** Optional bright wooden rim crack for decisive commands (0..1). */
    rimCrack?: number;
    wetSend?: number;
    pan?: number;
}

/**
 * Monumental AAA war-drum hit: layers the physical `taikoDrum` membrane buffer
 * with an asymmetric tube-driven chest punch so bass hits hard on every speaker.
 */
export function playTaikoHit(v: VoiceRenderCtx, opt: TaikoHitOptions): void {
    const delay = opt.delay ?? 0;
    const heavy = Boolean(opt.heavy);
    const rate = vary(v.rng, opt.pitch ?? 1.0, 0.055);
    const drive = v.createWarmDrive();
    drive.connect(opt.pan ? v.createPanNode(opt.pan) : v.out);

    // Layer 1: Pre-baked nonlinear Bessel taiko membrane + calfskin slap
    playBuffer(v, {
        buffer: v.bank.taikoDrum[heavy ? 1 : 0],
        delay,
        rate,
        gain: opt.gain * 0.92,
        wetSend: opt.wetSend ?? (heavy ? 0.44 : 0.24),
        dest: drive,
    });

    // Layer 2: Focused sub-chest weight
    playSubDrop(
        v,
        (heavy ? 115 : 138) * rate,
        (heavy ? 36 : 48) * rate,
        heavy ? 0.34 : 0.19,
        opt.gain * (heavy ? 0.56 : 0.4),
        delay,
        drive,
    );

    // Layer 3: Optional sharp wooden rim/stick crack
    if (opt.rimCrack && opt.rimCrack > 0.05) {
        playBuffer(v, {
            buffer: v.bank.woodThock[2],
            delay,
            rate: vary(v.rng, 1.35 * rate, 0.08),
            decay: 0.055,
            gain: opt.gain * 0.42 * opt.rimCrack,
            filterType: "highpass",
            filterFreq: 750,
            pan: opt.pan,
            wetSend: 0.2,
        });
    }
}

// ── 8. Forged Steel / Armor Clash ────────────────────────────────────────────

export interface SteelImpactOptions {
    /** `0` = bright sword/anvil clash, `1` = heavy battleship/armor hull strike. */
    heavy?: boolean;
    pitch?: number;
    delay?: number;
    gain: number;
    pan?: number;
    wetSend?: number;
}

/** Multi-layer forged steel clash (`steelClash` buffer + transient spark + optional FM sheen). */
export function playSteelImpact(v: VoiceRenderCtx, opt: SteelImpactOptions): void {
    const delay = opt.delay ?? 0;
    const heavy = Boolean(opt.heavy);
    const rate = vary(v.rng, opt.pitch ?? 1.0, 0.1);

    playBuffer(v, {
        buffer: v.bank.steelClash[heavy ? 1 : 0],
        delay,
        rate,
        gain: opt.gain,
        pan: opt.pan,
        wetSend: opt.wetSend ?? 0.28,
    });

    // Crisp metal-on-metal bright spark ring
    playFmTone(v, {
        carrierFreq: vary(v.rng, heavy ? 420 : 1180, 0.12) * rate,
        carrierFreqEnd: (heavy ? 240 : 860) * rate,
        modRatio: heavy ? 1.57 : 2.31,
        modIndex: 1.9,
        modIndexEnd: 0.15,
        delay,
        attack: 0.002,
        decay: heavy ? 0.16 : 0.12,
        gain: opt.gain * 0.28,
        pan: opt.pan,
        wetSend: opt.wetSend ?? 0.25,
    });
}

// ── 9. Hollywood 4-Stage Cinematic Explosion / Cannon Blast ────────────────────

export interface CinematicBlastOptions {
    /** Overall scale: 0.45 = deck cannon, 0.75 = ship sink/MIRV, 1.0 = Atom, 1.35 = Hydrogen. */
    scale: number;
    delay?: number;
    /** Duration of roaring fireball tail in seconds. */
    tailSec: number;
    /** Add secondary atmospheric pressure slapback (iconic Hydrogen double-shock). */
    doubleShock?: boolean;
    gain: number;
}

/**
 * 4-stage time-aligned Hollywood blast:
 *   1. 0 ms:   Razor-sharp gunpowder/EMP whip crack transient
 *   2. 4 ms:   Tube-saturated concussive chest shockwave
 *   3. 14 ms:  Wide stereo crackling fireball (`explosionCrackle`) sweeping down
 *   4. 45 ms:  Rolling canyon/arena reverb thunder tail
 */
export function playCinematicBlast(v: VoiceRenderCtx, opt: CinematicBlastOptions): void {
    const delay = opt.delay ?? 0;
    const s = clamp(opt.scale, 0.3, 1.5);
    const drive = v.createWarmDrive();
    drive.connect(v.out);

    // Stage 1: Blinding transient snap/crack (cuts cleanly through busy mixes)
    playFilteredNoise(v, {
        color: "white",
        delay,
        attack: 0.0015,
        decay: 0.045 + s * 0.055,
        gain: opt.gain * (0.48 + s * 0.16),
        filterType: "bandpass",
        filterFreq: vary(v.rng, 2400 - s * 480, 0.15),
        filterFreqEnd: 420,
        filterQ: 0.9,
        dest: drive,
    });

    // Stage 2: Tube-saturated concussive shockwave punch
    const fStart = vary(v.rng, 165 - s * 38, 0.08);
    const fEnd = Math.max(20, 44 - s * 14);
    playSubDrop(v, fStart, fEnd, 0.16 + s * 0.42, opt.gain * (0.64 + s * 0.16), delay + 0.004, drive);

    // Warm chest-punch harmonic body (so you FEEL the boom on laptop speakers too!)
    playBuffer(v, {
        buffer: v.bank.taikoDrum[s >= 0.85 ? 1 : 0],
        delay: delay + 0.003,
        rate: vary(v.rng, clamp(1.18 - s * 0.34, 0.55, 1.25), 0.07),
        decay: 0.25 + s * 0.25,
        gain: opt.gain * 0.54,
        dest: drive,
    });

    // Stage 3: Wide stereo crackling fireball & masonry debris roar
    playFilteredNoise(v, {
        color: "crackle",
        delay: delay + 0.012,
        attack: 0.012,
        decay: opt.tailSec,
        gain: opt.gain * (0.58 + s * 0.14),
        filterType: "lowpass",
        filterFreq: clamp(2900 - s * 750, 950, 3200),
        filterFreqEnd: clamp(180 - s * 55, 85, 220),
        filterQ: 0.85,
        playbackRate: vary(v.rng, clamp(1.15 - s * 0.22, 0.65, 1.2), 0.08),
        wetSend: 0.42 + s * 0.18,
        dest: drive,
    });

    // Optional Stage 3b: Secondary pressure-wave shock slapback (Hydrogen Bomb)
    if (opt.doubleShock) {
        playSubDrop(v, 88, 22, 0.72, opt.gain * 0.55, delay + 0.165, drive);
        playFilteredNoise(v, {
            color: "crackle",
            delay: delay + 0.17,
            attack: 0.025,
            decay: opt.tailSec * 0.85,
            gain: opt.gain * 0.38,
            filterType: "lowpass",
            filterFreq: 860,
            filterFreqEnd: 95,
            wetSend: 0.65,
            dest: drive,
        });
    }

    // Stage 4: Rolling distant canyon thunder into arena space
    if (v.wet) {
        playFilteredNoise(v, {
            color: "brown",
            delay: delay + 0.04,
            attack: 0.035,
            decay: opt.tailSec * 0.92,
            gain: opt.gain * (0.34 + s * 0.14),
            filterType: "lowpass",
            filterFreq: 560,
            filterFreqEnd: 110,
            dest: v.wet,
        });
    }
}

// ── 10. Multi-Bubble Liquid Water Splash ─────────────────────────────────────

export function playLiquidSplash(
    v: VoiceRenderCtx,
    opt: {
        delay?: number;
        decay?: number;
        pitch?: number;
        gain: number;
        heavy?: boolean;
        wetSend?: number;
    },
): void {
    const delay = opt.delay ?? 0;
    playFilteredNoise(v, {
        color: "splash",
        delay,
        attack: 0.008,
        decay: opt.decay ?? 0.34,
        gain: opt.gain,
        filterType: "bandpass",
        filterFreq: vary(v.rng, opt.heavy ? 1150 : 1580, 0.14),
        filterFreqEnd: opt.heavy ? 360 : 540,
        filterQ: 0.85,
        playbackRate: vary(v.rng, opt.pitch ?? (opt.heavy ? 0.85 : 1.05), 0.08),
        wetSend: opt.wetSend ?? 0.26,
    });
    if (opt.heavy) {
        playSubDrop(v, vary(v.rng, 128, 0.08), 46, 0.18, opt.gain * 0.45, delay);
    }
}

// ── 11. Silky Stereo Directional Whoosh ──────────────────────────────────────

/** Wide silky air/cloth/card swipe with L/R staggered motion for tactile UX & banners. */
export function playStereoWhoosh(
    v: VoiceRenderCtx,
    opt: {
        /** `1` = ascending/opening (L→R), `-1` = descending/closing (R→L). */
        direction: 1 | -1;
        delay?: number;
        decay?: number;
        gain: number;
        /** Brighter silky shimmer for cards/magic vs warm cloth for banners. */
        bright?: boolean;
    },
): void {
    const delay = opt.delay ?? 0;
    const decay = opt.decay ?? 0.14;
    const dir = opt.direction;
    const startHz = dir === 1 ? (opt.bright ? 950 : 620) : (opt.bright ? 2900 : 1950);
    const endHz = dir === 1 ? (opt.bright ? 3400 : 2150) : (opt.bright ? 780 : 480);

    // Lead ear
    playFilteredNoise(v, {
        color: "parchment",
        delay,
        attack: Math.max(0.006, decay * 0.18),
        decay,
        gain: opt.gain * 0.62,
        filterType: "bandpass",
        filterFreq: startHz,
        filterFreqEnd: endHz,
        filterQ: 1.4,
        pan: dir === 1 ? -0.45 : 0.45,
        shimmerSend: opt.bright ? 0.22 : 0.08,
    });

    // Trailing ear (11 ms Haas motion creates vivid luxury stereo sweep!)
    playFilteredNoise(v, {
        color: "parchment",
        delay: delay + 0.011,
        attack: Math.max(0.006, decay * 0.2),
        decay: decay * 0.95,
        gain: opt.gain * 0.56,
        filterType: "bandpass",
        filterFreq: startHz * 1.12,
        filterFreqEnd: endHz * 1.08,
        filterQ: 1.4,
        pan: dir === 1 ? 0.45 : -0.45,
        shimmerSend: opt.bright ? 0.26 : 0.08,
    });
}
