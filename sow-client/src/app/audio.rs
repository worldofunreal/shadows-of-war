//! Rust WASM ↔ TypeScript Procedural Audio Bridge & Spatial Projection.
//!
//! ARCHITECTURE
//! ============
//! Shadows of War synthesizes all SFX and GUI cues in TypeScript via Web Audio API
//! (`sow-web/shell/audio/`). This module owns:
//!   1. The shared numeric cue/subtype protocol (`AudioCue`, `AUDIO_EVENT_STRIDE = 6`),
//!      kept in exact lockstep with `sow-web/shell/audio/types.ts`.
//!   2. Camera- and tabletop-aware spatial projection (`spatial_pan_gain`), converting
//!      world coordinates into stereo `pan` ∈ [-0.85, +0.85] and distance/zoom `gain`.
//!   3. Zero-copy frame flush (`flush_audio_batch`), passing a `Float32Array::view`
//!      directly over WASM linear memory to `window.SOW_sfxFlush` once per frame.

use std::cell::RefCell;

use super::state::{InputState, SowApp};

/// Number of `f32` values packed per event in the Rust → JS binary batch buffer:
/// `[cue_id, subtype, pan, gain, intensity, seed]`.
pub const AUDIO_EVENT_STRIDE: usize = 6;

/// Canonical numeric cue IDs shared 1-to-1 with `sow-web/shell/audio/types.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum AudioCue {
    // ── Combat & Land (0–10) ───────────────────────────────────────────
    DeployTroops = 0,
    AttackOrder = 1,
    AttackStalled = 2,
    UnderAttack = 3,
    #[allow(dead_code)]
    LandExpand = 4,
    #[allow(dead_code)]
    LandCombat = 5,
    CounterAttack = 6,
    AttackCancelled = 7,
    CascadeStep = 8,
    KillStreak = 9,
    PlayerEliminated = 10,

    // ── Naval & Trade (11–20) ──────────────────────────────────────────
    TransportLaunch = 11,
    TransportLanded = 12,
    FleetOrder = 13,
    FleetRecallOrScuttle = 14,
    WarshipSpawned = 15,
    NavalShot = 16,
    NavalImpact = 17,
    NavalSink = 18,
    TradeDelivered = 19,
    TradeCaptured = 20,

    // ── Buildings & Economy (21–25) ────────────────────────────────────
    BuildingPlace = 21,
    BuildingUpgrade = 22,
    BuildingComplete = 23,
    ArsenalQueued = 24,
    BuildingDemolished = 25,

    // ── Nuclear & SAM Defense (26–30) ──────────────────────────────────
    NukeArmed = 26,
    NukeLaunch = 27,
    MirvSplit = 28,
    SamIntercept = 29,
    NukeImpact = 30,

    // ── Diplomacy, Abilities & Emotes (31–37) ──────────────────────────
    AllianceRequest = 31,
    AllianceFormed = 32,
    AllianceEnded = 33,
    Betrayal = 34,
    ResourceTransfer = 35,
    Emote = 37,

    // ── Match Lifecycle & Outcomes (38–41) ─────────────────────────────
    MatchPhaseStart = 38,
    ActionError = 39,
    Victory = 40,
    Defeat = 41,

    // ── Building Workflow Extensions (57–59, matching TS `types.ts`) ───
    BuildingUpgradeProgress = 57,
    BuildingModeEnter = 58,
    BuildingSelect = 59,
}

/// Player archetype for elimination cues (matches TS `PlayerEliminated` subtype).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PlayerSoundType {
    Bot = 0,
    Nation = 1,
    Human = 2,
}

/// Distinct building acoustic signatures (matches `BuildingAudioKind` in TS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BuildingSoundKind {
    City = 0,
    Tower = 1,
    Factory = 2,
    Port = 3,
    Farm = 4,
    SAM = 5,
    MissileSilo = 6,
}

/// Resource transfer / diplomatic request subtypes (matches `ResourceAudioSubtype` in TS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ResourceSoundKind {
    Sent = 0,
    Received = 1,
    Requested = 2,
    Rejected = 3,
    HelpCall = 4,
}

/// Arsenal production action subtypes (matches `ArsenalActionSubtype` in TS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ArsenalSoundKind {
    QueueWarship = 0,
    QueueNuke = 1,
    QueueAntiNuke = 2,
    Cancel = 3,
}

/// World position and camera state used to compute stereo pan and attenuation.
#[derive(Clone, Copy, Debug)]
pub struct SpatialSoundParams {
    pub wx: f32,
    pub wy: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
}

/// Computes `(pan, gain)` from world coordinates and camera viewport.
///
/// Unlike the legacy Rust mixer (which silenced all spatial audio at `zoom <= 1.0`),
/// zoom attenuation uses a smooth curve with a `0.28` floor so zoomed-out macro
/// play stays tactile without overcrowding foreground events.
pub fn spatial_pan_gain(spatial: SpatialSoundParams) -> (f32, f32) {
    const ZOOM_FLOOR: f32 = 0.75;
    const ZOOM_FULL: f32 = 8.5;
    const ZOOM_MIN_GAIN: f32 = 0.28;

    let screen_x = spatial.camera_x + spatial.wx * spatial.camera_zoom;
    let p = (screen_x / spatial.screen_w.max(1.0)).clamp(0.0, 1.0);
    let pan = (p * 2.0 - 1.0) * 0.82;

    let zoom = spatial.camera_zoom.max(0.001);
    let world_center_x = (spatial.screen_w * 0.5 - spatial.camera_x) / zoom;
    let world_center_y = (spatial.screen_h * 0.5 - spatial.camera_y) / zoom;
    let dx = spatial.wx - world_center_x;
    let dy = spatial.wy - world_center_y;
    let distance_tiles = (dx * dx + dy * dy).sqrt();
    let half_w = spatial.screen_w / (2.0 * zoom);
    let half_h = spatial.screen_h / (2.0 * zoom);
    let max_dist = (half_w * half_w + half_h * half_h).sqrt() * 1.65;
    let distance_factor = (1.0 - distance_tiles / max_dist.max(1.0)).clamp(0.0, 1.0);
    let distance_attenuation = distance_factor.sqrt();

    let zoom_t = ((spatial.camera_zoom - ZOOM_FLOOR) / (ZOOM_FULL - ZOOM_FLOOR)).clamp(0.0, 1.0);
    let zoom_attenuation = ZOOM_MIN_GAIN + (1.0 - ZOOM_MIN_GAIN) * zoom_t.sqrt();

    let gain = (distance_attenuation * zoom_attenuation).clamp(0.0, 1.0);
    (pan, gain)
}

/// Camera/viewport snapshot copied early to avoid borrow conflicts during render.
#[derive(Clone, Copy, Debug)]
pub struct SpatialAudioCtx {
    camera_x: f32,
    camera_y: f32,
    camera_zoom: f32,
    screen_w: f32,
    screen_h: f32,
}

impl SpatialAudioCtx {
    pub fn from_input(input: &InputState) -> Self {
        Self {
            camera_x: input.camera_x,
            camera_y: input.camera_y,
            camera_zoom: input.camera_zoom,
            screen_w: input.screen_w,
            screen_h: input.screen_h,
        }
    }

    pub fn params(self, wx: f32, wy: f32) -> SpatialSoundParams {
        SpatialSoundParams {
            wx,
            wy,
            camera_x: self.camera_x,
            camera_y: self.camera_y,
            camera_zoom: self.camera_zoom,
            screen_w: self.screen_w,
            screen_h: self.screen_h,
        }
    }
}

impl SowApp {
    #[inline]
    pub(crate) fn spatial_audio_ctx(&self) -> SpatialAudioCtx {
        SpatialAudioCtx::from_input(&self.input)
    }

    #[inline]
    pub(crate) fn spatial_sound_params(&self, wx: f32, wy: f32) -> SpatialSoundParams {
        let [px, py] = self.input.project_board([wx, wy]);
        self.spatial_audio_ctx().params(px, py)
    }
}

pub(crate) fn emote_sound_code(kind: sow_core::emote::EmoteKind) -> u8 {
    use sow_core::emote::EmoteKind;
    match kind {
        EmoteKind::Greet => 0,
        EmoteKind::Help => 1,
        EmoteKind::Threat => 2,
        EmoteKind::Celebrate => 3,
    }
}

pub(crate) fn nuke_kind_level(kind: sow_core::game::NukeKind) -> u8 {
    match kind {
        sow_core::game::NukeKind::AtomBomb => 1,
        sow_core::game::NukeKind::HydrogenBomb => 2,
        sow_core::game::NukeKind::MIRV => 3,
    }
}

thread_local! {
    /// Cached JS `window.SOW_sfxFlush` function reference to avoid `Reflect::get` per frame.
    static SFX_FLUSH_FN: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

fn resolve_sfx_flush() -> Option<js_sys::Function> {
    use wasm_bindgen::JsCast;
    SFX_FLUSH_FN.with(|slot| {
        if let Some(func) = slot.borrow().as_ref() {
            return Some(func.clone());
        }
        let window = web_sys::window()?;
        let val = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("SOW_sfxFlush")).ok()?;
        let func = val.dyn_into::<js_sys::Function>().ok()?;
        *slot.borrow_mut() = Some(func.clone());
        Some(func)
    })
}

/// Flushes a packed batch of `[cue_id, subtype, pan, gain, intensity, seed]` tuples
/// directly to TypeScript via a zero-copy `Float32Array::view` over WASM memory.
///
/// SAFETY: `Float32Array::view` is consumed synchronously by `SOW_sfxFlush` without
/// retaining a reference across any WASM allocation or memory growth.
pub(crate) fn flush_audio_batch(batch: &[f32]) {
    if batch.len() < AUDIO_EVENT_STRIDE {
        return;
    }
    let Some(func) = resolve_sfx_flush() else {
        return;
    };
    let view = unsafe { js_sys::Float32Array::view(batch) };
    let _ = func.call1(&wasm_bindgen::JsValue::NULL, &view);
}
