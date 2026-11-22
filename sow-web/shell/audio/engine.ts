/**
 * Procedural Audio Engine — Single Shared Web Audio Mastering Graph & Voice Scheduler.
 *
 * SIGNAL GRAPH
 * ============
 *   [Spatial SFX Voice] ─► airLPF ─► StereoPanner ─► sfxBus (sfx_volume) ──┐
 *   [UI / HUD Voice]    ───────────────────────────► uiBus  (sfx_volume) ──┼─► DynamicsCompressor ─► masterGain ─► destination
 *   [Menu Music Decks]  ───────────────────────────► musicBus (music_vol) ─┘
 *   [Wet Sends]         ─► ConvolverNode (0.38s IR) ─► reverbBus (0.32) ───┘
 *
 * PERFORMANCE GUARANTEES
 * ======================
 * 1. `flushWasmBatch(Float32Array)` reads the zero-copy linear memory slice directly
 *    with a plain index loop — no intermediate arrays, no JSON, no string keys.
 * 2. Active voice tracking uses a small flat expiration table (`activeVoices`) pruned
 *    on demand — zero `setTimeout` churn per sound.
 * 3. When the browser tab is hidden (`visibilitychange`), master gain smoothly ramps
 *    to 0 and incoming gameplay batches are discarded so returning to the tab never
 *    causes a wall of buffered combat sounds.
 * 4. Portal ad-breaks (`SOW_adPlaying`) and portal mute (`SOW_PORTAL_MUTE_AUDIO`)
 *    silence `masterGain` directly without overwriting the player's saved settings.
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
    readonly version = 1;

    private ctx: AudioContext | null = null;
    private bank: AudioResourceBank | null = null;
    private masterGain: GainNode | null = null;
    private compressor: DynamicsCompressorNode | null = null;
    private sfxBus: GainNode | null = null;
    private uiBus: GainNode | null = null;
    private musicBus: GainNode | null = null;
    private reverbConvolver: ConvolverNode | null = null;
    private reverbReturn: GainNode | null = null;

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

    /** Lazily creates the single shared `AudioContext`, master limiter, buses, and noise/IR bank. */
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

        const compressor = ctx.createDynamicsCompressor();
        compressor.threshold.value = -10;
        compressor.knee.value = 14;
        compressor.ratio.value = 4.5;
        compressor.attack.value = 0.004;
        compressor.release.value = 0.14;

        const masterGain = ctx.createGain();
        masterGain.gain.value = this.effectiveMasterGain();

        const sfxBus = ctx.createGain();
        sfxBus.gain.value = this.sfxVolume;

        const uiBus = ctx.createGain();
        uiBus.gain.value = this.sfxVolume;

        const musicBus = ctx.createGain();
        musicBus.gain.value = this.musicVolume;

        const bank = new AudioResourceBank(ctx);

        let convolver: ConvolverNode | null = null;
        let reverbReturn: GainNode | null = null;
        try {
            convolver = ctx.createConvolver();
            convolver.buffer = bank.reverbImpulse;
            reverbReturn = ctx.createGain();
            reverbReturn.gain.value = this.sfxVolume * 0.34;
            convolver.connect(reverbReturn);
            reverbReturn.connect(compressor);
        } catch {
            convolver = null;
            reverbReturn = null;
        }

        sfxBus.connect(compressor);
        uiBus.connect(compressor);
        musicBus.connect(compressor);
        compressor.connect(masterGain);
        masterGain.connect(ctx.destination);

        this.ctx = ctx;
        this.bank = bank;
        this.compressor = compressor;
        this.masterGain = masterGain;
        this.sfxBus = sfxBus;
        this.uiBus = uiBus;
        this.musicBus = musicBus;
        this.reverbConvolver = convolver;
        this.reverbReturn = reverbReturn;
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
        if (this.reverbReturn) {
            this.reverbReturn.gain.setTargetAtTime(this.sfxVolume * 0.34, now, ramp);
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
        this.sfxBus.gain.linearRampToValueAtTime(target * 0.68, now + 0.03);
        this.sfxBus.gain.exponentialRampToValueAtTime(Math.max(0.001, target), now + 0.42);
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
            // Allow first click that unlocks context to still schedule smoothly
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
        voiceGain.gain.setValueAtTime(clamp(def.gain * perspective.dryGain, 0.001, 1.25), now);

        let tail: AudioNode = voiceGain;
        if (def.spatial) {
            if (perspective.airCutoffHz < 14500) {
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

        // Optional reverb wet send for spatial depth & fanfares
        let wetGain: GainNode | null = null;
        if (this.reverbConvolver && perspective.reverbSend > 0.01) {
            wetGain = ctx.createGain();
            wetGain.gain.setValueAtTime(clamp(def.gain * perspective.reverbSend, 0.001, 0.55), now);
            wetGain.connect(this.reverbConvolver);
        }

        if (def.priority === "critical" && def.category !== "ui") {
            this.triggerCriticalDuck(now);
        }

        let maxDurationSec = 0.08;
        const renderCtx: VoiceRenderCtx = {
            ctx,
            now,
            bank,
            rng: createRng(trigger.seed || 1),
            out: voiceGain,
            wet: wetGain,
            perspective,
            createWarmDrive: () => {
                const shaper = ctx.createWaveShaper();
                shaper.curve = bank.warmCurve;
                shaper.oversample = "2x";
                return shaper;
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
