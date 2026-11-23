/**
 * Procedural Audio Public Entrypoint & Window Bridge Installer.
 *
 * Imported early by `sow-web/shell/main_menu.ts` so `window.SOW_sfxFlush` and
 * `window.SOW_audio` are already installed before WASM finishes initializing.
 */

import { audioEngine } from "./engine.ts";
import {
    AudioCue,
    type AudioCueId,
    type AudioSettingsSnapshot,
} from "./types.ts";

export {
    AbilityAudioFamily,
    ArsenalActionSubtype,
    AUDIO_EVENT_STRIDE,
    AudioCue,
    BuildingAudioKind,
    BuildingModeSubtype,
    CombatAudioSubtype,
    EmoteAudioKind,
    FleetOrderSubtype,
    MatchPhaseAudioKind,
    NukeAudioLevel,
    ResourceAudioSubtype,
    UiBattleLaunchSubtype,
    UiHeroBrowseSubtype,
    UiHoverSubtype,
    UiInfoTabSubtype,
    UiPressSubtype,
    UiPurchaseActionSubtype,
    type AudioCategory,
    type AudioCueId,
    type AudioPriority,
    type AudioSettingsSnapshot,
    type CueTrigger,
    type SowAudioPublicApi,
} from "./types.ts";

export { audioEngine, AudioEngine, VoiceBudgetGate } from "./engine.ts";
export {
    consumeScreenSwitchSoundSuppression,
    installSurfaceHoverPressAudio,
    MenuSfx,
    siblingSlotRatio,
    suppressNextScreenSwitchSound,
} from "./menu_sfx.ts";
export {
    CATEGORY_VOICE_BUDGET,
    CUE_REGISTRY,
    MAX_TOTAL_VOICES,
    type CueDefinition,
} from "./registry.ts";

/** Plays a procedural UI/UX micro-interaction cue through the shared engine. */
export function playUiCue(cue: AudioCueId, subtype = 0, intensity = 0.5, pan = 0): void {
    audioEngine.playUi(cue, subtype, intensity, pan);
}

/** Applies persisted or live-dragged settings (`mute_all`, `sfx_volume`, `music_volume`). */
export function syncAudioSettings(settings: AudioSettingsSnapshot | null | undefined): void {
    audioEngine.syncSettings(settings);
}

/** Syncs portal mute / ad-break flags onto the master audio bus. */
export function syncPortalMute(): void {
    audioEngine.syncPortalMute();
}

/** Installs global hooks used by Rust WASM (`SOW_sfxFlush`) and portal bridges (`SOW_audio`). */
export function installAudioBridge(): void {
    if (typeof window === "undefined") return;
    window.SOW_audio = audioEngine;
    window.SOW_sfxFlush = (batch: Float32Array) => {
        audioEngine.flushWasmBatch(batch);
    };
}

installAudioBridge();
void AudioCue;
