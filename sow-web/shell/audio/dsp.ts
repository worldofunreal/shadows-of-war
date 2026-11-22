/**
 * Procedural DSP Primitives, Harmonic Tuning & Acoustic Perspective Utilities.
 *
 * DESIGN PRINCIPLES
 * =================
 * 1. Click-Free Envelopes: Every amplitude ramp starts at >= 0.0001 with a >= 2ms
 *    attack and decays to 0.0001 before `stop()`, preventing DC pops.
 * 2. Zero Per-Voice Buffer Allocation: `AudioResourceBank` synthesizes white, pink,
 *    and brown noise loops (2.0s each), a warm soft-clipping `WaveShaper` curve, and
 *    a short 0.35s tactical reverb impulse response ONCE when the `AudioContext` boots.
 *    Each noise hit slices from a seed-derived random offset so consecutive impacts
 *    never repeat the exact grain texture.
 * 3. Perspective Modulation: `computePerspective(pan, gain)` models real acoustic
 *    air absorption — distant battlefield events lose high-frequency brittle sheen
 *    (LPF sweeps from ~14 kHz down to ~2.2 kHz), soften their attack, and gain a
 *    higher wet send into the shared reverb bus for geographic depth.
 * 4. Harmonic Cohesion: All pitched cues (cascades, buildings, UI, diplomacy,
 *    victory/defeat) anchor to **D Dorian / D Pentatonic** so overlapping feedback
 *    chimes harmonize instead of clashing.
 */

/** Fast 32-bit deterministic PRNG (`mulberry32`) for reproducible organic variation. */
export type SeededRng = () => number;

export function createRng(seed: number): SeededRng {
    let a = (seed >>> 0) ^ 0x6d2b79f5;
    if (a === 0) a = 0x9e3779b9;
    return () => {
        a = (a + 0x6d2b79f5) | 0;
        let t = Math.imul(a ^ (a >>> 15), 1 | a);
        t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
}

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

export function clamp(value: number, min: number, max: number): number {
    return value < min ? min : value > max ? max : value;
}

/**
 * D Pentatonic scale (D4 → F6) used for escalating cascades, UI confirmations,
 * and harmonic cohesion across overlapping cues.
 */
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
 * Acoustic perspective parameters derived from spatial `pan` and `gain`.
 * Distant events automatically roll off air frequencies and add subtle space.
 */
export interface AcousticPerspective {
    /** Clamped stereo pan in [-0.9, 0.9]. */
    readonly pan: number;
    /** Dry voice loudness scalar in [0.04, 1.0]. */
    readonly dryGain: number;
    /** Air-absorption low-pass cutoff in Hz (2200 Hz far → 15000 Hz close). */
    readonly airCutoffHz: number;
    /** Extra attack softening in seconds (0 close → 0.008s far). */
    readonly attackSoften: number;
    /** Wet send multiplier into the shared tactical reverb (0.04 close → 0.28 far). */
    readonly reverbSend: number;
}

export function computePerspective(pan: number, gain: number, spatial: boolean): AcousticPerspective {
    if (!spatial) {
        return {
            pan: clamp(pan, -0.9, 0.9),
            dryGain: clamp(gain, 0.05, 1.0),
            airCutoffHz: 18000,
            attackSoften: 0,
            reverbSend: 0.06,
        };
    }
    const g = clamp(gain, 0.04, 1.0);
    // Non-linear distance proximity curve: close sounds stay bright, far sounds warm up.
    const proximity = Math.pow(g, 0.72);
    return {
        pan: clamp(pan, -0.88, 0.88),
        dryGain: g,
        airCutoffHz: 2200 + proximity * 12800,
        attackSoften: (1 - proximity) * 0.008,
        reverbSend: 0.05 + (1 - proximity) * 0.23,
    };
}

/** Shared pre-baked buffers and shaping curves created once per `AudioContext`. */
export class AudioResourceBank {
    readonly whiteNoise: AudioBuffer;
    readonly pinkNoise: AudioBuffer;
    readonly brownNoise: AudioBuffer;
    readonly reverbImpulse: AudioBuffer;
    readonly warmCurve: Float32Array<ArrayBuffer>;

    constructor(ctx: AudioContext) {
        const sr = ctx.sampleRate || 44100;
        const noiseLen = Math.floor(sr * 2.0);
        const rng = createRng(0x534f5721);

        // 1. White noise (crisp transients, gunfire, snaps, splashes)
        this.whiteNoise = ctx.createBuffer(1, noiseLen, sr);
        const white = this.whiteNoise.getChannelData(0);
        for (let i = 0; i < noiseLen; i++) {
            white[i] = rng() * 2 - 1;
        }

        // 2. Pink noise (Paul Kellet 7-stage filter — natural wind, water, rocket thrust, crowds)
        this.pinkNoise = ctx.createBuffer(1, noiseLen, sr);
        const pink = this.pinkNoise.getChannelData(0);
        let b0 = 0,
            b1 = 0,
            b2 = 0,
            b3 = 0,
            b4 = 0,
            b5 = 0,
            b6 = 0;
        for (let i = 0; i < noiseLen; i++) {
            const w = white[i];
            b0 = 0.99886 * b0 + w * 0.0555179;
            b1 = 0.99332 * b1 + w * 0.0750759;
            b2 = 0.969 * b2 + w * 0.153852;
            b3 = 0.8665 * b3 + w * 0.3104856;
            b4 = 0.55 * b4 + w * 0.5329522;
            b5 = -0.7616 * b5 - w * 0.016898;
            pink[i] = clamp((b0 + b1 + b2 + b3 + b4 + b5 + b6 + w * 0.5362) * 0.11, -1, 1);
            b6 = w * 0.115926;
        }

        // 3. Brown / Red noise (sub-rumble, masonry collapse, distant thunder, nuclear roar)
        this.brownNoise = ctx.createBuffer(1, noiseLen, sr);
        const brown = this.brownNoise.getChannelData(0);
        let last = 0;
        for (let i = 0; i < noiseLen; i++) {
            last = (last + 0.035 * white[i]) / 1.035;
            brown[i] = clamp(last * 3.1, -1, 1);
        }

        // 4. Synthetic tactical battlefield impulse response (stereo 0.38s warm exponential tail)
        const irLen = Math.floor(sr * 0.38);
        this.reverbImpulse = ctx.createBuffer(2, irLen, sr);
        const irL = this.reverbImpulse.getChannelData(0);
        const irR = this.reverbImpulse.getChannelData(1);
        let lpL = 0;
        let lpR = 0;
        for (let i = 0; i < irLen; i++) {
            const t = i / irLen;
            // Subtle early reflection cluster + smooth exponential decay
            const env = Math.pow(1 - t, 2.7) * (i < sr * 0.015 ? i / (sr * 0.015) : 1);
            // Progressive high-frequency damping over decay time
            const damp = 0.32 - 0.22 * t;
            lpL += damp * ((rng() * 2 - 1) - lpL);
            lpR += damp * ((rng() * 2 - 1) - lpR);
            irL[i] = lpL * env * 0.85;
            irR[i] = lpR * env * 0.85;
        }

        // 5. Soft analog saturation curve (tanh waveshaper for punchy lows without harsh clipping)
        const curveLen = 512;
        const curveBuf = new ArrayBuffer(curveLen * 4);
        this.warmCurve = new Float32Array(curveBuf);
        for (let i = 0; i < curveLen; i++) {
            const x = (i / (curveLen - 1)) * 2 - 1;
            this.warmCurve[i] = Math.tanh(x * 2.2) / Math.tanh(2.2);
        }
    }
}

/**
 * Runtime DSP context passed into every sound recipe.
 * Recipes connect their output nodes into `out` (already panned & air-filtered)
 * and optionally send extra spatial tail into `wet` (the shared reverb bus).
 */
export interface VoiceRenderCtx {
    readonly ctx: AudioContext;
    readonly now: number;
    readonly bank: AudioResourceBank;
    readonly rng: SeededRng;
    readonly out: AudioNode;
    readonly wet: GainNode | null;
    readonly perspective: AcousticPerspective;
    /** Optional saturation node creator for heavy impacts/cannons/nukes. */
    createWarmDrive(driveAmount?: number): WaveShaperNode;
    /** Registers a voice tail duration so the engine tracks active voice lifetime cleanly. */
    registerDuration(seconds: number): void;
}

export interface ToneOptions {
    type?: OscillatorType;
    freq: number;
    freqEnd?: number;
    delay?: number;
    attack?: number;
    decay: number;
    gain: number;
    detuneCents?: number;
    filterType?: BiquadFilterType;
    filterFreq?: number;
    filterFreqEnd?: number;
    filterQ?: number;
    dest?: AudioNode;
}

/** Synthesizes a click-free pitched oscillator with optional frequency & filter sweeps. */
export function playTone(v: VoiceRenderCtx, opt: ToneOptions): void {
    const start = v.now + (opt.delay ?? 0);
    const attack = Math.max(0.002, (opt.attack ?? 0.004) + v.perspective.attackSoften * 0.5);
    const decay = Math.max(0.015, opt.decay);
    const end = start + attack + decay + 0.012;
    const peak = Math.max(0.0002, opt.gain);

    const osc = v.ctx.createOscillator();
    osc.type = opt.type ?? "triangle";
    osc.frequency.setValueAtTime(Math.max(20, opt.freq), start);
    if (opt.freqEnd != null && opt.freqEnd !== opt.freq) {
        osc.frequency.exponentialRampToValueAtTime(Math.max(20, opt.freqEnd), start + attack + decay);
    }
    if (opt.detuneCents) {
        osc.detune.setValueAtTime(opt.detuneCents, start);
    }

    const env = v.ctx.createGain();
    env.gain.setValueAtTime(0.0001, start);
    env.gain.linearRampToValueAtTime(peak, start + attack);
    env.gain.exponentialRampToValueAtTime(0.0001, start + attack + decay);

    let chain: AudioNode = osc;
    if (opt.filterType && opt.filterFreq) {
        const filter = v.ctx.createBiquadFilter();
        filter.type = opt.filterType;
        filter.frequency.setValueAtTime(clamp(opt.filterFreq, 40, 18000), start);
        if (opt.filterFreqEnd != null) {
            filter.frequency.exponentialRampToValueAtTime(
                clamp(opt.filterFreqEnd, 40, 18000),
                start + attack + decay,
            );
        }
        if (opt.filterQ != null) filter.Q.setValueAtTime(opt.filterQ, start);
        chain.connect(filter);
        chain = filter;
    }

    chain.connect(env);
    env.connect(opt.dest ?? v.out);

    osc.start(start);
    osc.stop(end);
    v.registerDuration((opt.delay ?? 0) + attack + decay + 0.02);
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
    dest?: AudioNode;
}

/**
 * 2-operator FM voice: ideal for metallic clashes, bells, sonar pings,
 * hydraulic latches, and crisp tactical telemetry without harsh square waves.
 */
export function playFmTone(v: VoiceRenderCtx, opt: FmToneOptions): void {
    const start = v.now + (opt.delay ?? 0);
    const attack = Math.max(0.002, (opt.attack ?? 0.003) + v.perspective.attackSoften * 0.4);
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
    const endDepth = Math.max(0.001, opt.carrierFreq * (opt.modIndexEnd ?? opt.modIndex * 0.12));
    modGain.gain.setValueAtTime(startDepth, start);
    modGain.gain.exponentialRampToValueAtTime(endDepth, start + attack + decay);

    const env = v.ctx.createGain();
    env.gain.setValueAtTime(0.0001, start);
    env.gain.linearRampToValueAtTime(peak, start + attack);
    env.gain.exponentialRampToValueAtTime(0.0001, start + attack + decay);

    mod.connect(modGain);
    modGain.connect(carrier.frequency);
    carrier.connect(env);
    env.connect(opt.dest ?? v.out);

    mod.start(start);
    carrier.start(start);
    mod.stop(end);
    carrier.stop(end);
    v.registerDuration((opt.delay ?? 0) + attack + decay + 0.02);
}

export interface NoiseOptions {
    color: "white" | "pink" | "brown";
    delay?: number;
    attack?: number;
    decay: number;
    gain: number;
    filterType: BiquadFilterType;
    filterFreq: number;
    filterFreqEnd?: number;
    filterQ?: number;
    playbackRate?: number;
    dest?: AudioNode;
}

/**
 * Filtered noise burst reading from a pre-allocated buffer at a seed-varied offset.
 * Core building block for percussion transients, water splashes, masonry, and blast tails.
 */
export function playFilteredNoise(v: VoiceRenderCtx, opt: NoiseOptions): void {
    const start = v.now + (opt.delay ?? 0);
    const attack = Math.max(0.002, (opt.attack ?? 0.003) + v.perspective.attackSoften);
    const decay = Math.max(0.015, opt.decay);
    const total = attack + decay + 0.012;
    const peak = Math.max(0.0002, opt.gain);

    const buf =
        opt.color === "white"
            ? v.bank.whiteNoise
            : opt.color === "pink"
              ? v.bank.pinkNoise
              : v.bank.brownNoise;

    const src = v.ctx.createBufferSource();
    src.buffer = buf;
    src.loop = true;
    if (opt.playbackRate != null) {
        src.playbackRate.setValueAtTime(clamp(opt.playbackRate, 0.25, 3.0), start);
    }

    const filter = v.ctx.createBiquadFilter();
    filter.type = opt.filterType;
    filter.frequency.setValueAtTime(clamp(opt.filterFreq, 30, 18000), start);
    if (opt.filterFreqEnd != null) {
        filter.frequency.exponentialRampToValueAtTime(
            clamp(opt.filterFreqEnd, 30, 18000),
            start + attack + decay,
        );
    }
    if (opt.filterQ != null) filter.Q.setValueAtTime(opt.filterQ, start);

    const env = v.ctx.createGain();
    env.gain.setValueAtTime(0.0001, start);
    env.gain.linearRampToValueAtTime(peak, start + attack);
    env.gain.exponentialRampToValueAtTime(0.0001, start + attack + decay);

    src.connect(filter);
    filter.connect(env);
    env.connect(opt.dest ?? v.out);

    // Random offset into the 2s loop ensures identical cues never share the exact grain.
    const offset = v.rng() * Math.max(0.1, buf.duration - 0.05);
    src.start(start, offset, total);
    v.registerDuration((opt.delay ?? 0) + total + 0.01);
}

export interface ModalHitOptions {
    baseFreq: number;
    /** Partial frequency ratios (e.g. `[1, 2.01, 3.04]` for warm bell, `[1, 1.58, 2.41]` for wood/stone). */
    ratios: readonly number[];
    /** Relative gains per partial. */
    gains: readonly number[];
    /** Decay duration in seconds for the fundamental; upper partials decay faster. */
    decay: number;
    delay?: number;
    gain: number;
    dest?: AudioNode;
}

/**
 * Physical-modeling modal resonator bank: excites damped inharmonic or harmonic partials
 * for wood blocks, masonry stone, bronze bells, coins, and metallic chimes.
 */
export function playModalHit(v: VoiceRenderCtx, opt: ModalHitOptions): void {
    const count = Math.min(opt.ratios.length, opt.gains.length);
    for (let i = 0; i < count; i++) {
        const partialDecay = opt.decay / Math.pow(opt.ratios[i], 0.55);
        playTone(v, {
            type: "sine",
            freq: opt.baseFreq * opt.ratios[i],
            delay: opt.delay,
            attack: 0.002 + i * 0.0006,
            decay: Math.max(0.025, partialDecay),
            gain: opt.gain * opt.gains[i],
            detuneCents: (v.rng() - 0.5) * (i === 0 ? 5 : 14),
            dest: opt.dest,
        });
    }
}

/** Punchy pitched membrane / sub-bass drop (drums, cannon thump, nuclear shockwave). */
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
        attack: 0.003,
        decay,
        gain,
        dest,
    });
}
