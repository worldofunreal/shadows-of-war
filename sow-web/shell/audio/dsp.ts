/**
 * Procedural DSP Primitives, Harmonic Vocabulary & Acoustic Perspective Utilities.
 *
 * DESIGN PRINCIPLES
 * =================
 * 1. Click-Free Envelopes: Every amplitude ramp starts at >= 0.0001 with >= 1.5 ms
 *    attack and decays to 0.0001 before `stop()`, preventing DC pops.
 * 2. Physical Buffers First: Prefer `playBuffer` over bare oscillators for foley,
 *    transients, coins, celesta, harp, drums, steel, water, and crackle. Use custom
 *    Fourier `PeriodicWave` tables (`v.bank.brassWave`, `choirWave`, `marimbaWave`)
 *    whenever tonal synthesis is needed — never raw buzzy NES waveforms.
 * 3. Perspective Modulation: `computePerspective(pan, gain)` models acoustic air
 *    absorption — distant battlefield events warm up (LPF ~15.5 kHz → ~2.4 kHz),
 *    soften their attack, and send more energy into the stereo arena convolution space.
 * 4. Elevated Harmonic Cohesion:
 *      - `PENTATONIC_D` / `DORIAN_D`: Heroic strategy identity & combat calls.
 *      - `MAJOR9_D` / `LYDIAN_GOLD_D`: Euphoric Supercell / Lilith / Casino reward
 *        voicings (`D, F#, A, C#, E, F#, A, B, D6, E6, F#6`) that make upgrades,
 *        cascades, coins, purchases, and victories feel radiant and luxurious.
 */

import {
    AudioResourceBank,
    clamp,
    createRng,
    type SeededRng,
} from "./synth_bank.ts";

export {
    AudioResourceBank,
    CELESTA_REF_HZ,
    clamp,
    createRng,
    HARP_REF_HZ,
    type SeededRng,
} from "./synth_bank.ts";

/** Returns `base * (1 + uniform[-spread, +spread])` using `rng`. */
export function vary(rng: SeededRng, base: number, spread: number): number {
    return base * (1 + (rng() * 2 - 1) * spread);
}

/** Shifts `freqHz` by `cents` (100 cents = 1 semitone). */
export function detuneHz(freqHz: number, cents: number): number {
    return freqHz * Math.pow(2, cents / 1200);
}

/** Shifts `freqHz` by `semitones`. */
export function semitones(freqHz: number, steps: number): number {
    return freqHz * Math.pow(2, steps / 12);
}

/** D Pentatonic scale (D4 → F6) — ascending table for cascades & tactical steps. */
export const PENTATONIC_D: readonly number[] = [
    293.66, // D4
    349.23, // F4
    392.0, // G4
    440.0, // A4
    523.25, // C5
    587.33, // D5
    698.46, // F5
    783.99, // G5
    880.0, // A5
    1046.5, // C6
    1174.66, // D6
    1396.91, // F6
];

/** D Dorian frequencies (D4 → D6) for diplomatic, heroic, and modal cues. */
export const DORIAN_D: readonly number[] = [
    293.66, // D4
    329.63, // E4
    349.23, // F4
    392.0, // G4
    440.0, // A4
    493.88, // B4
    523.25, // C5
    587.33, // D5
    659.25, // E5
    698.46, // F5
    783.99, // G5
    880.0, // A5
    987.77, // B5
    1046.5, // C6
    1174.66, // D6
];

/**
 * Radiant D Major9 / Lydian Gold scale (`D4 → F#6`) — the signature euphoria palette
 * for Supercell / Lilith / Casino reward bursts, coin showers, upgrades, and victories.
 */
export const MAJOR9_D: readonly number[] = [
    293.66, // D4  (root)
    369.99, // F#4 (major 3rd)
    440.0, // A4  (5th)
    493.88, // B4  (6th)
    554.37, // C#5 (maj7)
    587.33, // D5  (octave)
    659.25, // E5  (9th — luxury golden tension!)
    739.99, // F#5 (10th)
    880.0, // A5  (12th)
    987.77, // B5  (13th)
    1108.73, // C#6
    1174.66, // D6
    1318.51, // E6
    1479.98, // F#6
];

/**
 * Acoustic perspective parameters derived from spatial `pan` and `gain`.
 * Distant events automatically roll off air frequencies and bloom into stereo space.
 */
export interface AcousticPerspective {
    /** Clamped stereo pan in [-0.9, 0.9]. */
    readonly pan: number;
    /** Dry voice loudness scalar in [0.04, 1.0]. */
    readonly dryGain: number;
    /** Air-absorption low-pass cutoff in Hz (2400 Hz far → 16000 Hz close). */
    readonly airCutoffHz: number;
    /** Extra attack softening in seconds (0 close → 0.007 s far). */
    readonly attackSoften: number;
    /** Wet send multiplier into the shared arena reverb (0.08 close → 0.34 far). */
    readonly reverbSend: number;
}

export function computePerspective(pan: number, gain: number, spatial: boolean): AcousticPerspective {
    if (!spatial) {
        return {
            pan: clamp(pan, -0.9, 0.9),
            dryGain: clamp(gain, 0.05, 1.0),
            airCutoffHz: 18500,
            attackSoften: 0,
            reverbSend: 0.1,
        };
    }
    const g = clamp(gain, 0.04, 1.0);
    const proximity = Math.pow(g, 0.72);
    return {
        pan: clamp(pan, -0.88, 0.88),
        dryGain: g,
        airCutoffHz: 2400 + proximity * 13600,
        attackSoften: (1 - proximity) * 0.007,
        reverbSend: 0.08 + (1 - proximity) * 0.26,
    };
}

/**
 * Runtime DSP context passed into every sound recipe.
 * Recipes connect dry output to `out`, battlefield ambience to `wet` (arena IR),
 * and golden sparkle/fanfare bloom to `shimmer` (1.65 s casino hall IR).
 */
export interface VoiceRenderCtx {
    readonly ctx: AudioContext;
    readonly now: number;
    readonly bank: AudioResourceBank;
    readonly rng: SeededRng;
    readonly out: AudioNode;
    /** Tactical outdoor/fortress arena reverb send (0.85 s stereo IR). */
    readonly wet: GainNode | null;
    /** Lush golden casino hall shimmer reverb send (1.65 s stereo IR). */
    readonly shimmer: GainNode | null;
    readonly perspective: AcousticPerspective;
    /** Creates an asymmetric tube-warmth WaveShaper for punchy impacts/cannons/drums. */
    createWarmDrive(): WaveShaperNode;
    /** Creates an optional relative stereo panner node feeding `dest` (defaults to `out`). */
    createPanNode(panOffset: number, dest?: AudioNode): AudioNode;
    /** Registers voice tail duration so the engine tracks active voice lifetime accurately. */
    registerDuration(seconds: number): void;
}

export interface BufferPlayOptions {
    buffer: AudioBuffer;
    delay?: number;
    /** Playback pitch multiplier (e.g. `targetHz / refHz`). */
    rate?: number;
    /** Optional exponential pitch bend target by the end of `decay`. */
    rateEnd?: number;
    /** Random start offset into looping textures (ignored for one-shot hits). */
    randomOffset?: boolean;
    loop?: boolean;
    attack?: number;
    /** Envelope decay duration; defaults to buffer duration / rate for one-shots. */
    decay?: number;
    gain: number;
    filterType?: BiquadFilterType;
    filterFreq?: number;
    filterFreqEnd?: number;
    filterQ?: number;
    /** Local stereo offset in [-1, 1] for lush stereo chord/sparkle width. */
    pan?: number;
    /** Extra send amount into `v.wet` (arena space), 0..1. */
    wetSend?: number;
    /** Extra send amount into `v.shimmer` (golden casino hall), 0..1. */
    shimmerSend?: number;
    dest?: AudioNode;
}

/**
 * Plays a pre-baked physical-modeling or foley `AudioBuffer` with sample-accurate
 * envelope, optional filter sweep, pitch bend, stereo offset, and dual reverb sends.
 */
export function playBuffer(v: VoiceRenderCtx, opt: BufferPlayOptions): void {
    const start = v.now + (opt.delay ?? 0);
    const rate = clamp(opt.rate ?? 1.0, 0.15, 4.0);
    const naturalDur = opt.buffer.duration / rate;
    const attack = Math.max(0.001, (opt.attack ?? 0.0015) + v.perspective.attackSoften * 0.35);
    const decay = opt.decay != null ? Math.max(0.015, opt.decay) : Math.max(0.02, naturalDur);
    const total = opt.loop ? attack + decay + 0.015 : Math.min(naturalDur + 0.015, attack + decay + 0.015);
    const peak = Math.max(0.0002, opt.gain);

    const src = v.ctx.createBufferSource();
    src.buffer = opt.buffer;
    src.loop = Boolean(opt.loop);
    src.playbackRate.setValueAtTime(rate, start);
    if (opt.rateEnd != null && opt.rateEnd !== rate) {
        src.playbackRate.exponentialRampToValueAtTime(
            clamp(opt.rateEnd, 0.15, 4.0),
            start + Math.min(decay, total),
        );
    }

    let chain: AudioNode = src;
    if (opt.filterType && opt.filterFreq) {
        const filter = v.ctx.createBiquadFilter();
        filter.type = opt.filterType;
        filter.frequency.setValueAtTime(clamp(opt.filterFreq, 30, 19000), start);
        if (opt.filterFreqEnd != null) {
            filter.frequency.exponentialRampToValueAtTime(
                clamp(opt.filterFreqEnd, 30, 19000),
                start + attack + decay,
            );
        }
        if (opt.filterQ != null) filter.Q.setValueAtTime(opt.filterQ, start);
        chain.connect(filter);
        chain = filter;
    }

    const env = v.ctx.createGain();
    if (opt.attack != null && opt.attack > 0.002) {
        env.gain.setValueAtTime(0.0001, start);
        env.gain.linearRampToValueAtTime(peak, start + attack);
    } else {
        // Preserve ultra-crisp 0-ms physical foley transients from baked buffers
        env.gain.setValueAtTime(peak, start);
    }
    if (opt.decay != null || opt.loop) {
        env.gain.setValueAtTime(peak, start + attack);
        env.gain.exponentialRampToValueAtTime(0.0001, start + attack + decay);
    }

    chain.connect(env);
    const target =
        opt.pan != null && Math.abs(opt.pan) > 0.01
            ? v.createPanNode(opt.pan, opt.dest)
            : (opt.dest ?? v.out);
    env.connect(target);

    if (opt.wetSend && opt.wetSend > 0.005 && v.wet) {
        const send = v.ctx.createGain();
        send.gain.setValueAtTime(clamp(opt.wetSend, 0.001, 1.0), start);
        env.connect(send);
        send.connect(v.wet);
    }
    if (opt.shimmerSend && opt.shimmerSend > 0.005 && v.shimmer) {
        const send = v.ctx.createGain();
        send.gain.setValueAtTime(clamp(opt.shimmerSend, 0.001, 1.0), start);
        env.connect(send);
        send.connect(v.shimmer);
    }

    if (opt.randomOffset) {
        const offset = v.rng() * Math.max(0.05, opt.buffer.duration - 0.06);
        src.start(start, offset, total);
    } else if (opt.loop) {
        src.start(start, 0, total);
    } else {
        src.start(start);
        if (opt.decay != null && attack + decay + 0.02 < naturalDur) {
            src.stop(start + attack + decay + 0.02);
        }
    }
    v.registerDuration((opt.delay ?? 0) + total + 0.02);
}

export interface ToneOptions {
    type?: OscillatorType;
    /** Custom Fourier `PeriodicWave` (takes precedence over `type` when non-null). */
    wave?: PeriodicWave | null;
    freq: number;
    freqEnd?: number;
    /** Optional fast initial pitch scoop/snap duration in seconds (`freqStart` -> `freq`). */
    freqStart?: number;
    pitchSnapTime?: number;
    delay?: number;
    attack?: number;
    hold?: number;
    decay: number;
    gain: number;
    detuneCents?: number;
    filterType?: BiquadFilterType;
    filterFreq?: number;
    filterFreqEnd?: number;
    filterQ?: number;
    pan?: number;
    wetSend?: number;
    shimmerSend?: number;
    dest?: AudioNode;
}

/** Synthesizes a click-free pitched voice supporting custom Fourier wavetables & stereo pan. */
export function playTone(v: VoiceRenderCtx, opt: ToneOptions): void {
    const start = v.now + (opt.delay ?? 0);
    const attack = Math.max(0.002, (opt.attack ?? 0.004) + v.perspective.attackSoften * 0.45);
    const hold = Math.max(0, opt.hold ?? 0);
    const decay = Math.max(0.015, opt.decay);
    const totalEnv = attack + hold + decay;
    const end = start + totalEnv + 0.015;
    const peak = Math.max(0.0002, opt.gain);

    const osc = v.ctx.createOscillator();
    if (opt.wave) {
        osc.setPeriodicWave(opt.wave);
    } else {
        osc.type = opt.type ?? "triangle";
    }

    const baseFreq = Math.max(20, opt.freq);
    if (opt.freqStart != null && opt.pitchSnapTime && opt.pitchSnapTime > 0) {
        osc.frequency.setValueAtTime(Math.max(20, opt.freqStart), start);
        osc.frequency.exponentialRampToValueAtTime(baseFreq, start + opt.pitchSnapTime);
    } else {
        osc.frequency.setValueAtTime(baseFreq, start);
    }
    if (opt.freqEnd != null && opt.freqEnd !== opt.freq) {
        const rampFrom = opt.pitchSnapTime ? start + opt.pitchSnapTime : start;
        osc.frequency.setValueAtTime(baseFreq, rampFrom);
        osc.frequency.exponentialRampToValueAtTime(Math.max(20, opt.freqEnd), start + totalEnv);
    }
    if (opt.detuneCents) {
        osc.detune.setValueAtTime(opt.detuneCents, start);
    }

    const env = v.ctx.createGain();
    env.gain.setValueAtTime(0.0001, start);
    env.gain.linearRampToValueAtTime(peak, start + attack);
    if (hold > 0) {
        env.gain.setValueAtTime(peak, start + attack + hold);
    }
    env.gain.exponentialRampToValueAtTime(0.0001, start + totalEnv);

    let chain: AudioNode = osc;
    if (opt.filterType && opt.filterFreq) {
        const filter = v.ctx.createBiquadFilter();
        filter.type = opt.filterType;
        filter.frequency.setValueAtTime(clamp(opt.filterFreq, 35, 19000), start);
        if (opt.filterFreqEnd != null) {
            filter.frequency.exponentialRampToValueAtTime(
                clamp(opt.filterFreqEnd, 35, 19000),
                start + totalEnv,
            );
        }
        if (opt.filterQ != null) filter.Q.setValueAtTime(opt.filterQ, start);
        chain.connect(filter);
        chain = filter;
    }

    chain.connect(env);
    const target =
        opt.pan != null && Math.abs(opt.pan) > 0.01
            ? v.createPanNode(opt.pan, opt.dest)
            : (opt.dest ?? v.out);
    env.connect(target);

    if (opt.wetSend && opt.wetSend > 0.005 && v.wet) {
        const send = v.ctx.createGain();
        send.gain.setValueAtTime(clamp(opt.wetSend, 0.001, 1.0), start);
        env.connect(send);
        send.connect(v.wet);
    }
    if (opt.shimmerSend && opt.shimmerSend > 0.005 && v.shimmer) {
        const send = v.ctx.createGain();
        send.gain.setValueAtTime(clamp(opt.shimmerSend, 0.001, 1.0), start);
        env.connect(send);
        send.connect(v.shimmer);
    }

    osc.start(start);
    osc.stop(end);
    v.registerDuration((opt.delay ?? 0) + totalEnv + 0.02);
}

export interface FmToneOptions {
    carrierFreq: number;
    carrierFreqEnd?: number;
    modRatio: number;
    modIndex: number;
    modIndexEnd?: number;
    carrierType?: OscillatorType;
    modType?: OscillatorType;
    delay?: number;
    attack?: number;
    decay: number;
    gain: number;
    pan?: number;
    wetSend?: number;
    shimmerSend?: number;
    dest?: AudioNode;
}

/** 2-operator FM voice for metallic bell sheen, sonar/radar chimes, and steel resonances. */
export function playFmTone(v: VoiceRenderCtx, opt: FmToneOptions): void {
    const start = v.now + (opt.delay ?? 0);
    const attack = Math.max(0.002, (opt.attack ?? 0.003) + v.perspective.attackSoften * 0.35);
    const decay = Math.max(0.02, opt.decay);
    const end = start + attack + decay + 0.015;
    const peak = Math.max(0.0002, opt.gain);

    const carrier = v.ctx.createOscillator();
    carrier.type = opt.carrierType ?? "sine";
    carrier.frequency.setValueAtTime(Math.max(20, opt.carrierFreq), start);
    if (opt.carrierFreqEnd != null && opt.carrierFreqEnd !== opt.carrierFreq) {
        carrier.frequency.exponentialRampToValueAtTime(
            Math.max(20, opt.carrierFreqEnd),
            start + attack + decay,
        );
    }

    const mod = v.ctx.createOscillator();
    mod.type = opt.modType ?? "sine";
    mod.frequency.setValueAtTime(Math.max(10, opt.carrierFreq * opt.modRatio), start);

    const modGain = v.ctx.createGain();
    const startDepth = Math.max(0.001, opt.carrierFreq * opt.modIndex);
    const endDepth = Math.max(0.001, opt.carrierFreq * (opt.modIndexEnd ?? opt.modIndex * 0.1));
    modGain.gain.setValueAtTime(startDepth, start);
    modGain.gain.exponentialRampToValueAtTime(endDepth, start + attack + decay);

    const env = v.ctx.createGain();
    env.gain.setValueAtTime(0.0001, start);
    env.gain.linearRampToValueAtTime(peak, start + attack);
    env.gain.exponentialRampToValueAtTime(0.0001, start + attack + decay);

    mod.connect(modGain);
    modGain.connect(carrier.frequency);
    carrier.connect(env);

    const target =
        opt.pan != null && Math.abs(opt.pan) > 0.01
            ? v.createPanNode(opt.pan, opt.dest)
            : (opt.dest ?? v.out);
    env.connect(target);

    if (opt.wetSend && opt.wetSend > 0.005 && v.wet) {
        const send = v.ctx.createGain();
        send.gain.setValueAtTime(clamp(opt.wetSend, 0.001, 1.0), start);
        env.connect(send);
        send.connect(v.wet);
    }
    if (opt.shimmerSend && opt.shimmerSend > 0.005 && v.shimmer) {
        const send = v.ctx.createGain();
        send.gain.setValueAtTime(clamp(opt.shimmerSend, 0.001, 1.0), start);
        env.connect(send);
        send.connect(v.shimmer);
    }

    mod.start(start);
    carrier.start(start);
    mod.stop(end);
    carrier.stop(end);
    v.registerDuration((opt.delay ?? 0) + attack + decay + 0.02);
}

export interface NoiseOptions {
    color: "white" | "pink" | "brown" | "crackle" | "splash" | "parchment";
    delay?: number;
    attack?: number;
    decay: number;
    gain: number;
    filterType: BiquadFilterType;
    filterFreq: number;
    filterFreqEnd?: number;
    filterQ?: number;
    playbackRate?: number;
    pan?: number;
    wetSend?: number;
    shimmerSend?: number;
    dest?: AudioNode;
}

/** Filtered noise/texture burst reading from a pre-allocated stereo/mono loop at a random offset. */
export function playFilteredNoise(v: VoiceRenderCtx, opt: NoiseOptions): void {
    const buf =
        opt.color === "white"
            ? v.bank.whiteNoise
            : opt.color === "pink"
              ? v.bank.pinkNoise
              : opt.color === "brown"
                ? v.bank.brownNoise
                : opt.color === "crackle"
                  ? v.bank.explosionCrackle
                  : opt.color === "splash"
                    ? v.bank.waterSplash
                    : v.bank.parchmentWhoosh;

    playBuffer(v, {
        buffer: buf,
        delay: opt.delay,
        rate: opt.playbackRate ?? 1.0,
        randomOffset: true,
        loop: true,
        attack: Math.max(0.002, opt.attack ?? 0.003),
        decay: opt.decay,
        gain: opt.gain,
        filterType: opt.filterType,
        filterFreq: opt.filterFreq,
        filterFreqEnd: opt.filterFreqEnd,
        filterQ: opt.filterQ,
        pan: opt.pan,
        wetSend: opt.wetSend,
        shimmerSend: opt.shimmerSend,
        dest: opt.dest,
    });
}

export interface ModalHitOptions {
    baseFreq: number;
    ratios: readonly number[];
    gains: readonly number[];
    decay: number;
    delay?: number;
    gain: number;
    pan?: number;
    shimmerSend?: number;
    dest?: AudioNode;
}

/** Multi-partial modal resonator bank for custom tuned metallic/wood resonances. */
export function playModalHit(v: VoiceRenderCtx, opt: ModalHitOptions): void {
    const count = Math.min(opt.ratios.length, opt.gains.length);
    for (let i = 0; i < count; i++) {
        const partialDecay = opt.decay / Math.pow(opt.ratios[i], 0.58);
        playTone(v, {
            type: "sine",
            freq: opt.baseFreq * opt.ratios[i],
            delay: opt.delay,
            attack: 0.002 + i * 0.0005,
            decay: Math.max(0.025, partialDecay),
            gain: opt.gain * opt.gains[i],
            detuneCents: (v.rng() - 0.5) * (i === 0 ? 4 : 12),
            pan: opt.pan,
            shimmerSend: i === 0 ? opt.shimmerSend : undefined,
            dest: opt.dest,
        });
    }
}

/** Punchy pitched membrane / sub-bass drop (drums, cannon thump, shockwaves). */
export function playSubDrop(
    v: VoiceRenderCtx,
    startHz: number,
    endHz: number,
    decay: number,
    gain: number,
    delay = 0,
    dest?: AudioNode,
): void {
    playTone(v, {
        type: "sine",
        freq: startHz,
        freqEnd: endHz,
        delay,
        attack: 0.002,
        decay,
        gain,
        dest,
    });
}
