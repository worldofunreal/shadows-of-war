import { describe, expect, it } from "bun:test";
import { computePerspective, createRng, DORIAN_D, PENTATONIC_D, vary } from "./dsp.ts";
import { VoiceBudgetGate } from "./engine.ts";
import { CATEGORY_VOICE_BUDGET, CUE_REGISTRY, MAX_TOTAL_VOICES } from "./registry.ts";
import { AUDIO_EVENT_STRIDE, AudioCue, type AudioCueId } from "./types.ts";

describe("Procedural Audio Registry & Protocol", () => {
    it("leaves HeroAbility throttling to the per-owner Rust gate", () => {
        expect(AudioCue.HeroAbility).toBe(36);
        const gate = new VoiceBudgetGate();
        const definition = CUE_REGISTRY[AudioCue.HeroAbility];
        expect(definition.cooldownMs).toBe(0);
        expect(gate.admit(AudioCue.HeroAbility, definition, 1000)).toBe(true);
        gate.record(definition.category, 1000);
        expect(gate.admit(AudioCue.HeroAbility, definition, 1050)).toBe(true);
    });

    it("registers every numeric AudioCue ID contiguously from 0..N-1", () => {
        const ids = Object.values(AudioCue).sort((a, b) => a - b);
        expect(ids.length).toBeGreaterThanOrEqual(45);
        for (let i = 0; i < ids.length; i++) {
            expect(ids[i]).toBe(i);
            const def = CUE_REGISTRY[ids[i] as AudioCueId];
            expect(def).toBeDefined();
            expect(typeof def.synth).toBe("function");
            expect(def.gain).toBeGreaterThan(0);
            expect(def.gain).toBeLessThanOrEqual(1);
            expect(def.cooldownMs).toBeGreaterThanOrEqual(0);
        }
        expect(AUDIO_EVENT_STRIDE).toBe(6);
    });

    it("keeps D-Pentatonic and D-Dorian tuning tables strictly ascending", () => {
        for (let i = 1; i < PENTATONIC_D.length; i++) {
            expect(PENTATONIC_D[i]).toBeGreaterThan(PENTATONIC_D[i - 1]);
        }
        for (let i = 1; i < DORIAN_D.length; i++) {
            expect(DORIAN_D[i]).toBeGreaterThan(DORIAN_D[i - 1]);
        }
    });
});

describe("Deterministic RNG & Acoustic Perspective", () => {
    it("produces deterministic variation within bounded spread", () => {
        const a = createRng(123456);
        const b = createRng(123456);
        for (let i = 0; i < 20; i++) {
            const va = vary(a, 440, 0.08);
            const vb = vary(b, 440, 0.08);
            expect(va).toBe(vb);
            expect(va).toBeGreaterThanOrEqual(440 * 0.92);
            expect(va).toBeLessThanOrEqual(440 * 1.08);
        }
    });

    it("rolls off high frequencies and increases reverb send with distance", () => {
        const close = computePerspective(0.3, 1.0, true);
        const far = computePerspective(-0.95, 0.15, true);
        expect(close.airCutoffHz).toBeGreaterThan(far.airCutoffHz);
        expect(far.reverbSend).toBeGreaterThan(close.reverbSend);
        expect(far.attackSoften).toBeGreaterThan(close.attackSoften);
        expect(far.pan).toBeGreaterThanOrEqual(-0.88);
        expect(close.pan).toBeLessThanOrEqual(0.88);
    });
});

describe("VoiceBudgetGate", () => {
    it("enforces per-cue cooldowns and lets critical cues bypass soft background limits", () => {
        const gate = new VoiceBudgetGate();
        const expand = CUE_REGISTRY[AudioCue.LandExpand];
        const nuke = CUE_REGISTRY[AudioCue.NukeImpact];

        // First expand passes, immediate repeat within cooldown is rejected
        expect(gate.admit(AudioCue.LandExpand, expand, 1000)).toBe(true);
        gate.record(expand.category, 1400);
        expect(gate.admit(AudioCue.LandExpand, expand, 1050)).toBe(false);
        expect(gate.admit(AudioCue.LandExpand, expand, 1350)).toBe(true);

        // Fill up global voice cap with background voices
        for (let i = 0; i < MAX_TOTAL_VOICES; i++) {
            gate.record("combat", 5000);
        }
        expect(gate.admit(AudioCue.LandCombat, CUE_REGISTRY[AudioCue.LandCombat], 2000)).toBe(false);
        // Critical nuke still breaks through
        expect(gate.admit(AudioCue.NukeImpact, nuke, 2000)).toBe(true);
        // After voices expire, budget recovers cleanly
        expect(gate.count(6000)).toBe(0);
        expect(CATEGORY_VOICE_BUDGET.combat).toBeGreaterThan(0);
    });
});
