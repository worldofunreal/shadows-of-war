/**
 * Procedural Audio Engine — Studio Mastering Graph & Zero-Copy Voice Scheduler.
 *
 * STUDIO SIGNAL GRAPH
 * ===================
 *   [Spatial SFX Voice] ─► airLPF ─► StereoPanner ─► sfxBus ─► sfxToneEQ ────────┐
 *   [UI / UX Voice]     ───────────────────────────► uiBus  ─► uiToneEQ  ────────┤
 *   [Menu Music Decks]  ───────────────────────────► musicBus ───────────────────┼─► GlueCompressor ─► SoftLimiter ─► masterGain ─► destination
 *   [Arena Sends]       ─► Convolver (0.85 s) ─────► arenaReturn  ───────────────┤
 *   [Golden Hall Sends] ─► Convolver (1.65 s) ─────► casinoReturn ───────────────┘
 *
 * TONE SHAPING
 * ============
 * - `sfxToneEQ`: HPF 28 Hz (clears sub mud) → +2.6 dB @ 105 Hz (laptop/mobile chest punch)
 *   → -1.8 dB @ 320 Hz (removes boxy plastic mids) → +2.2 dB @ 4.8 kHz (crisp air).
 * - `uiToneEQ`:  HPF 55 Hz → +1.8 dB @ 2.6 kHz (wood/stone/glass tactile clarity)
 *   → +2.8 dB @ 6.5 kHz (luxury Supercell / casino sparkle).
 * - `SoftLimiter`: Transparent soft-ceiling WaveShaper keeps dense battles & jackpot
 *   cascades loud, warm, and broadcast-clean with zero digital clipping.
 *
 * PERFORMANCE GUARANTEES
 * ======================
 * 1. `flushWasmBatch(Float32Array)` reads WASM linear memory directly with a plain index
 *    loop — zero intermediate arrays, zero JSON, zero string keys.
 * 2. Active voice tracking uses a flat expiration table pruned on demand — zero `setTimeout`.
 * 3. Hidden tabs (`visibilitychange`) smoothly mute and discard queued batches.
 * 4. Portal ad-breaks (`SOW_adPlaying`) and portal mute (`SOW_PORTAL_MUTE_AUDIO`)
 *    silence `masterGain` directly without overwriting saved player preferences.
 */

import {
    AudioResourceBank,
    clamp,
    computePerspective,
    createRng,
    type VoiceRenderCtx,
} from "./dsp.ts";
import {
    CATEGORY_VOICE_BUDGET,
    CUE_REGISTRY,
    MAX_TOTAL_VOICES,
    type CueDefinition,
} from "./registry.ts";
import {
    AUDIO_EVENT_STRIDE,
    type AudioCategory,
    type AudioCueId,
    type AudioSettingsSnapshot,
    type CueTrigger,
    type SowAudioPublicApi,
} from "./types.ts";

interface ActiveVoiceEntry {
    category: AudioCategory;
    expiresAtMs: number;
}

/** Pure voice-budget & cooldown gate extracted so unit tests can exercise it without DOM AudioContext. */
export class VoiceBudgetGate {
    private readonly lastPlayedMs = new Map<number, number>();
    private readonly active: ActiveVoiceEntry[] = [];

    prune(nowMs: number): void {
        let write = 0;
        for (let i = 0; i < this.active.length; i++) {
            if (this.active[i].expiresAtMs > nowMs) {
                this.active[write++] = this.active[i];
            }
        }
        this.active.length = write;
    }

    admit(cue: AudioCueId, def: CueDefinition, nowMs: number): boolean {
        const prev = this.lastPlayedMs.get(cue);
        if (prev != null && nowMs - prev < def.cooldownMs) {
            return false;
        }
        this.prune(nowMs);

        const total = this.active.length;
        if (def.priority !== "critical" && total >= MAX_TOTAL_VOICES) {
            return false;
        }

        let catCount = 0;
        for (let i = 0; i < total; i++) {
            if (this.active[i].category === def.category) catCount++;
        }
        const limit = CATEGORY_VOICE_BUDGET[def.category];
        if (def.priority === "background" && catCount >= Math.max(1, limit - 1)) {
            return false;
        }
        if (def.priority !== "critical" && catCount >= limit) {
            return false;
        }

        this.lastPlayedMs.set(cue, nowMs);
        return true;
    }

    record(category: AudioCategory, expiresAtMs: number): void {
        this.active.push({ category, expiresAtMs });
    }

    count(nowMs: number): number {
        this.prune(nowMs);
        return this.active.length;
    }

    reset(): void {
        this.lastPlayedMs.clear();
        this.active.length = 0;
    }
}

export class AudioEngine implements SowAudioPublicApi {
    readonly version = 2;

    private ctx: AudioContext | null = null;
    private bank: AudioResourceBank | null = null;
    private masterGain: GainNode | null = null;
    private compressor: DynamicsCompressorNode | null = null;
    private sfxBus: GainNode | null = null;
    private uiBus: GainNode | null = null;
    private musicBus: GainNode | null = null;
    private arenaConvolver: ConvolverNode | null = null;
    private arenaReturn: GainNode | null = null;
    private casinoConvolver: ConvolverNode | null = null;
    private casinoReturn: GainNode | null = null;

    private muteAll = false;
    private sfxVolume = 0.8;
    private musicVolume = 0.8;
    private tabHidden = false;
    private uiSeedCounter = 0x13572468;
    private readonly gate = new VoiceBudgetGate();
    private listenersInstalled = false;

    constructor() {
        this.installLifecycleListeners();
    }

    private installLifecycleListeners(): void {
        if (this.listenersInstalled || typeof window === "undefined" || typeof document === "undefined") {
            return;
        }
        this.listenersInstalled = true;

        const onGesture = () => {
            this.unlock();
        };
        window.addEventListener("pointerdown", onGesture, { capture: true, passive: true });
        window.addEventListener("keydown", onGesture, { capture: true, passive: true });
        window.addEventListener("touchstart", onGesture, { capture: true, passive: true });

        document.addEventListener("visibilitychange", () => {
            this.setTabHidden(document.visibilityState === "hidden");
        });
    }

    private isPortalMuted(): boolean {
        if (typeof window === "undefined") return false;
        return Boolean(window.SOW_adPlaying || window.SOW_PORTAL_MUTE_AUDIO);
    }

    private effectiveMasterGain(): number {
        if (this.muteAll || this.tabHidden || this.isPortalMuted()) return 0;
        return 1;
    }

    getContext(): AudioContext | null {
        return this.ctx;
    }

    getMusicBus(): GainNode | null {
        this.ensureContext();
        return this.musicBus;
    }

    getActiveVoiceCount(): number {
        return this.gate.count(typeof performance !== "undefined" ? performance.now() : Date.now());
    }

    /** Builds a series biquad tone-shaping strip (`head -> ... -> dest`). */
    private buildEqStrip(
        ctx: AudioContext,
        stages: readonly { type: BiquadFilterType; freq: number; q?: number; gain?: number }[],
        dest: AudioNode,
    ): AudioNode {
        let head: AudioNode | null = null;
        let prev: AudioNode | null = null;
        for (const s of stages) {
            const f = ctx.createBiquadFilter();
            f.type = s.type;
            f.frequency.value = s.freq;
            if (s.q != null) f.Q.value = s.q;
            if (s.gain != null) f.gain.value = s.gain;
            if (!head) head = f;
            if (prev) prev.connect(f);
            prev = f;
        }
        if (prev) prev.connect(dest);
        return head ?? dest;
    }

    /** Lazily creates the single shared `AudioContext`, mastering chain, dual IRs, and foley bank. */
    ensureContext(): AudioContext | null {
        if (this.ctx) return this.ctx;
        if (typeof window === "undefined" || typeof window.AudioContext !== "function") {
            return null;
        }
        let ctx: AudioContext;
        try {
            ctx = new window.AudioContext({ latencyHint: "interactive" });
        } catch {
            try {
                ctx = new window.AudioContext();
            } catch {
                return null;
            }
        }

        const bank = new AudioResourceBank(ctx);

        // Master broadcast glue compressor
        const compressor = ctx.createDynamicsCompressor();
        compressor.threshold.value = -12;
        compressor.knee.value = 12;
        compressor.ratio.value = 3.6;
        compressor.attack.value = 0.003;
        compressor.release.value = 0.115;

        // Protective warm soft-ceiling limiter (prevents harsh digital clip on stacked climaxes)
        const softLimiter = ctx.createWaveShaper();
        softLimiter.curve = bank.softLimiterCurve;
        softLimiter.oversample = "2x";

        const masterGain = ctx.createGain();
        masterGain.gain.value = this.effectiveMasterGain();

        // Bus 1: Gameplay SFX -> Studio Punch & Air EQ -> Compressor
        const sfxBus = ctx.createGain();
        sfxBus.gain.value = this.sfxVolume;
        const sfxEqIn = this.buildEqStrip(
            ctx,
            [
                { type: "highpass", freq: 28, q: 0.7 },
                { type: "peaking", freq: 105, q: 0.9, gain: 2.6 },
                { type: "peaking", freq: 320, q: 1.1, gain: -1.8 },
                { type: "highshelf", freq: 4800, gain: 2.2 },
            ],
            compressor,
        );
        sfxBus.connect(sfxEqIn);

        // Bus 2: UI / UX -> Tactile Presence & Casino Sparkle EQ -> Compressor
        const uiBus = ctx.createGain();
        uiBus.gain.value = this.sfxVolume;
        const uiEqIn = this.buildEqStrip(
            ctx,
            [
                { type: "highpass", freq: 55, q: 0.7 },
                { type: "peaking", freq: 2600, q: 0.85, gain: 1.8 },
                { type: "highshelf", freq: 6500, gain: 2.8 },
            ],
            compressor,
        );
        uiBus.connect(uiEqIn);

        // Bus 3: Menu Music (passes transparently into master glue)
        const musicBus = ctx.createGain();
        musicBus.gain.value = this.musicVolume;
        musicBus.connect(compressor);

        // Dual stereo convolution spaces: Fortress Arena (0.85 s) + Golden Casino Hall (1.65 s)
        let arenaConvolver: ConvolverNode | null = null;
        let arenaReturn: GainNode | null = null;
        let casinoConvolver: ConvolverNode | null = null;
        let casinoReturn: GainNode | null = null;
        try {
            arenaConvolver = ctx.createConvolver();
            arenaConvolver.buffer = bank.arenaImpulse;
            arenaReturn = ctx.createGain();
            arenaReturn.gain.value = this.sfxVolume * 0.56;
            const arenaHp = ctx.createBiquadFilter();
            arenaHp.type = "highpass";
            arenaHp.frequency.value = 135;
            arenaConvolver.connect(arenaHp);
            arenaHp.connect(arenaReturn);
            arenaReturn.connect(compressor);

            casinoConvolver = ctx.createConvolver();
            casinoConvolver.buffer = bank.casinoHallImpulse;
            casinoReturn = ctx.createGain();
            casinoReturn.gain.value = this.sfxVolume * 0.54;
            const casinoHp = ctx.createBiquadFilter();
            casinoHp.type = "highpass";
            casinoHp.frequency.value = 210;
            casinoConvolver.connect(casinoHp);
            casinoHp.connect(casinoReturn);
            casinoReturn.connect(compressor);
        } catch {
            arenaConvolver = null;
            arenaReturn = null;
            casinoConvolver = null;
            casinoReturn = null;
        }

        compressor.connect(softLimiter);
        softLimiter.connect(masterGain);
        masterGain.connect(ctx.destination);

        this.ctx = ctx;
        this.bank = bank;
        this.compressor = compressor;
        this.masterGain = masterGain;
        this.sfxBus = sfxBus;
        this.uiBus = uiBus;
        this.musicBus = musicBus;
        this.arenaConvolver = arenaConvolver;
        this.arenaReturn = arenaReturn;
        this.casinoConvolver = casinoConvolver;
        this.casinoReturn = casinoReturn;
        return ctx;
    }

    unlock(): void {
        const ctx = this.ensureContext();
        if (!ctx) return;
        if (ctx.state === "suspended") {
            void ctx.resume().catch(() => {});
        }
        this.applyBusGains();
    }

    syncSettings(settings: AudioSettingsSnapshot | null | undefined): void {
        if (!settings) return;
        if (typeof settings.mute_all === "boolean") {
            this.muteAll = settings.mute_all;
        }
        if (settings.sfx_volume != null && Number.isFinite(Number(settings.sfx_volume))) {
            this.sfxVolume = clamp(Number(settings.sfx_volume), 0, 1);
        }
        if (settings.music_volume != null && Number.isFinite(Number(settings.music_volume))) {
            this.musicVolume = clamp(Number(settings.music_volume), 0, 1);
        }
        this.applyBusGains();
    }

    syncPortalMute(): void {
        this.applyBusGains();
    }

    setTabHidden(hidden: boolean): void {
        this.tabHidden = hidden;
        if (hidden) {
            this.gate.reset();
        }
        this.applyBusGains();
    }

    private applyBusGains(): void {
        if (!this.ctx || !this.masterGain) return;
        const now = this.ctx.currentTime;
        const ramp = 0.025;
        this.masterGain.gain.setTargetAtTime(this.effectiveMasterGain(), now, ramp);
        if (this.sfxBus) {
            this.sfxBus.gain.setTargetAtTime(this.sfxVolume, now, ramp);
        }
        if (this.uiBus) {
            this.uiBus.gain.setTargetAtTime(this.sfxVolume, now, ramp);
        }
        if (this.musicBus) {
            this.musicBus.gain.setTargetAtTime(this.musicVolume, now, ramp);
        }
        if (this.arenaReturn) {
            this.arenaReturn.gain.setTargetAtTime(this.sfxVolume * 0.56, now, ramp);
        }
        if (this.casinoReturn) {
            this.casinoReturn.gain.setTargetAtTime(this.sfxVolume * 0.54, now, ramp);
        }
    }

    /**
     * Subtle temporary ducking of `sfxBus` on critical battlefield moments (nukes, betrayal,
     * victory/defeat) so high-drama cues cut cleanly through active combat without muting them.
     */
    private triggerCriticalDuck(now: number): void {
        if (!this.sfxBus || this.sfxVolume <= 0.001) return;
        const target = this.sfxVolume;
        this.sfxBus.gain.cancelScheduledValues(now);
        this.sfxBus.gain.setValueAtTime(target, now);
        this.sfxBus.gain.linearRampToValueAtTime(target * 0.65, now + 0.028);
        this.sfxBus.gain.exponentialRampToValueAtTime(Math.max(0.001, target), now + 0.46);
    }

    /**
     * Zero-copy batch consumer called once per frame by Rust WASM (`SOW_sfxFlush`).
     * Reads `AUDIO_EVENT_STRIDE = 6` floats per cue: `[cueId, subtype, pan, gain, intensity, seed]`.
     */
    flushWasmBatch(batch: Float32Array): void {
        if (this.tabHidden || this.muteAll || this.sfxVolume <= 0.001 || this.isPortalMuted()) {
            return;
        }
        const len = batch.length;
        if (len < AUDIO_EVENT_STRIDE) return;

        const ctx = this.ensureContext();
        if (!ctx || ctx.state !== "running") return;

        const nowMs = typeof performance !== "undefined" ? performance.now() : Date.now();
        for (let i = 0; i + AUDIO_EVENT_STRIDE <= len; i += AUDIO_EVENT_STRIDE) {
            const cueId = Math.round(batch[i]) as AudioCueId;
            const def = CUE_REGISTRY[cueId];
            if (!def) continue;

            this.renderTrigger(
                def,
                {
                    cue: cueId,
                    subtype: batch[i + 1],
                    pan: batch[i + 2],
                    gain: batch[i + 3],
                    intensity: batch[i + 4],
                    seed: batch[i + 5] >>> 0,
                },
                nowMs,
            );
        }
    }

    /** Direct entry point for GUI/HUD/Menu micro-interactions in TypeScript. */
    playUi(cue: AudioCueId, subtype = 0, intensity = 0.5, pan = 0): void {
        if (this.tabHidden || this.muteAll || this.sfxVolume <= 0.001 || this.isPortalMuted()) {
            return;
        }
        const def = CUE_REGISTRY[cue];
        if (!def) return;

        const ctx = this.ensureContext();
        if (!ctx) return;
        if (ctx.state === "suspended") {
            void ctx.resume().catch(() => {});
        }

        this.uiSeedCounter = (this.uiSeedCounter + 0x9e3779b9) >>> 0;
        const nowMs = typeof performance !== "undefined" ? performance.now() : Date.now();
        this.renderTrigger(
            def,
            {
                cue,
                subtype,
                pan,
                gain: 1.0,
                intensity,
                seed: this.uiSeedCounter,
            },
            nowMs,
        );
    }

    private renderTrigger(def: CueDefinition, trigger: CueTrigger, nowMs: number): void {
        if (!this.ctx || !this.bank || !this.sfxBus || !this.uiBus) return;
        if (!this.gate.admit(trigger.cue, def, nowMs)) return;

        const ctx = this.ctx;
        const bank = this.bank;
        const now = ctx.currentTime + 0.004;
        const perspective = computePerspective(trigger.pan, trigger.gain, def.spatial);
        const targetBus = def.category === "ui" ? this.uiBus : this.sfxBus;

        // Per-voice output chain: [voiceGain] -> (optional airLPF -> StereoPanner) -> targetBus
        const voiceGain = ctx.createGain();
        voiceGain.gain.setValueAtTime(clamp(def.gain * perspective.dryGain, 0.001, 1.3), now);

        let tail: AudioNode = voiceGain;
        if (def.spatial) {
            if (perspective.airCutoffHz < 15000) {
                const airFilter = ctx.createBiquadFilter();
                airFilter.type = "lowpass";
                airFilter.frequency.setValueAtTime(perspective.airCutoffHz, now);
                airFilter.Q.setValueAtTime(0.65, now);
                tail.connect(airFilter);
                tail = airFilter;
            }
            if (typeof ctx.createStereoPanner === "function") {
                const panner = ctx.createStereoPanner();
                panner.pan.setValueAtTime(perspective.pan, now);
                tail.connect(panner);
                tail = panner;
            }
        }
        tail.connect(targetBus);

        // Lazy dual spatial reverb sends (only allocated when a recipe actually sends to them)
        const arenaConvolver = this.arenaConvolver;
        const casinoConvolver = this.casinoConvolver;
        let wetGain: GainNode | null = null;
        let shimmerGain: GainNode | null = null;

        if (def.priority === "critical" && def.category !== "ui") {
            this.triggerCriticalDuck(now);
        }

        let maxDurationSec = 0.08;
        const spatialWidthScale = def.spatial ? 0.55 : 1.0;

        const renderCtx: VoiceRenderCtx = {
            ctx,
            now,
            bank,
            rng: createRng(trigger.seed || 1),
            out: voiceGain,
            get wet(): GainNode | null {
                if (!arenaConvolver) return null;
                if (!wetGain) {
                    wetGain = ctx.createGain();
                    const arenaSend = def.spatial
                        ? 0.38 + (1 - perspective.dryGain) * 0.44
                        : 0.48;
                    wetGain.gain.setValueAtTime(clamp(def.gain * arenaSend, 0.05, 0.9), now);
                    wetGain.connect(arenaConvolver);
                }
                return wetGain;
            },
            get shimmer(): GainNode | null {
                if (!casinoConvolver) return null;
                if (!shimmerGain) {
                    shimmerGain = ctx.createGain();
                    const hallSend = def.spatial ? 0.58 * perspective.dryGain : 0.72;
                    shimmerGain.gain.setValueAtTime(clamp(def.gain * hallSend, 0.05, 0.85), now);
                    shimmerGain.connect(casinoConvolver);
                }
                return shimmerGain;
            },
            perspective,
            createWarmDrive: () => {
                const shaper = ctx.createWaveShaper();
                shaper.curve = bank.warmCurve;
                shaper.oversample = "2x";
                return shaper;
            },
            createPanNode: (panOffset: number, dest?: AudioNode) => {
                const target = dest ?? voiceGain;
                if (typeof ctx.createStereoPanner !== "function") return target;
                const p = ctx.createStereoPanner();
                p.pan.setValueAtTime(clamp(panOffset * spatialWidthScale, -0.92, 0.92), now);
                p.connect(target);
                return p;
            },
            registerDuration: (seconds: number) => {
                if (seconds > maxDurationSec) maxDurationSec = seconds;
            },
        };

        try {
            def.synth(renderCtx, trigger);
            this.gate.record(def.category, nowMs + Math.ceil(maxDurationSec * 1000));
        } catch {
            // Never allow an audio error to disturb gameplay or UI loops.
        }
    }
}

export const audioEngine = new AudioEngine();
