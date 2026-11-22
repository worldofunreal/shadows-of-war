//! Gameplay SFX Director — Cooldown Gating, Priority Selection & Zero-Copy Frame Batching.
//!
//! Rather than synthesizing or mixing audio inside WASM, `SfxDirector` acts as a
//! deterministic gatekeeper for gameplay events:
//!   1. Filters events by client/server phase and per-cue cooldowns.
//!   2. Projects spatial events into `(pan, gain)` using camera & tabletop transforms.
//!   3. Appends compact 6-float records (`[cue_id, subtype, pan, gain, intensity, seed]`)
//!      into a reusable `pending: Vec<f32>` buffer.
//!   4. Flushes the entire frame's cues in a single zero-copy FFI call (`flush_frame`).

use std::collections::HashMap;

use sow_core::game::GamePhase;
use web_time::{Duration, Instant};

use crate::app::audio::{
    AUDIO_EVENT_STRIDE, ArsenalSoundKind, AudioCue, BuildingSoundKind, CombatSoundKind,
    PlayerSoundType, ResourceSoundKind, SpatialSoundParams, flush_audio_batch, spatial_pan_gain,
};

const DEPLOY_GAP: Duration = Duration::from_millis(160);
const ATTACK_ORDER_GAP: Duration = Duration::from_millis(130);
const ATTACK_STALLED_GAP: Duration = Duration::from_millis(600);
const PLACEMENT_GAP: Duration = Duration::from_millis(180);
const COMPLETION_GAP: Duration = Duration::from_millis(320);
const UNDER_ATTACK_GAP: Duration = Duration::from_secs(7);
const ELIMINATION_GAP: Duration = Duration::from_millis(550);
const NUKE_GAP: Duration = Duration::from_millis(320);
const ERROR_GAP: Duration = Duration::from_millis(220);
const FLEET_GAP: Duration = Duration::from_millis(180);
const TRADE_GAP: Duration = Duration::from_millis(450);
const DIPLOMACY_GAP: Duration = Duration::from_millis(350);
const EMOTE_GAP: Duration = Duration::from_millis(320);
const ABILITY_GAP: Duration = Duration::from_millis(450);

#[derive(Clone, Copy)]
struct PendingElimination {
    player_type: PlayerSoundType,
    seed: u32,
    spatial: SpatialSoundParams,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrackedPhase {
    Lobby,
    Spawning,
    Playing,
    GameOver,
}

impl From<&GamePhase> for TrackedPhase {
    fn from(value: &GamePhase) -> Self {
        match value {
            GamePhase::Lobby => Self::Lobby,
            GamePhase::Spawning { .. } => Self::Spawning,
            GamePhase::Playing => Self::Playing,
            GamePhase::GameOver => Self::GameOver,
        }
    }
}

pub(crate) struct SfxDirector {
    client_active: bool,
    phase: TrackedPhase,
    phase_initialized: bool,
    pending: Vec<f32>,
    seed_seq: u32,
    last_deploy: Option<Instant>,
    last_attack_order: Option<Instant>,
    last_attack_stalled: Option<Instant>,
    last_placement: Option<Instant>,
    last_completion: Option<Instant>,
    last_demolition: Option<Instant>,
    last_under_attack: Option<Instant>,
    last_elimination: Option<Instant>,
    last_nuke_launch: Option<Instant>,
    last_nuke_impact: Option<Instant>,
    last_mirv_split: Option<Instant>,
    last_sam_intercept: Option<Instant>,
    last_fleet: Option<Instant>,
    last_naval_shot: Option<Instant>,
    last_naval_impact: Option<Instant>,
    last_naval_sink: Option<Instant>,
    last_trade: Option<Instant>,
    last_diplomacy: Option<Instant>,
    last_emote: Option<Instant>,
    last_ability: Option<Instant>,
    last_error: Option<Instant>,
    last_combat: HashMap<CombatSoundKind, Instant>,
    pending_elimination: Option<PendingElimination>,
    under_attack_active: bool,
    result_played: bool,
    cascade_chain: u8,
    last_cascade_at: Option<Instant>,
}

impl Default for SfxDirector {
    fn default() -> Self {
        Self {
            client_active: false,
            phase: TrackedPhase::Lobby,
            phase_initialized: false,
            pending: Vec::with_capacity(64 * AUDIO_EVENT_STRIDE),
            seed_seq: 0x534f_5701,
            last_deploy: None,
            last_attack_order: None,
            last_attack_stalled: None,
            last_placement: None,
            last_completion: None,
            last_demolition: None,
            last_under_attack: None,
            last_elimination: None,
            last_nuke_launch: None,
            last_nuke_impact: None,
            last_mirv_split: None,
            last_sam_intercept: None,
            last_fleet: None,
            last_naval_shot: None,
            last_naval_impact: None,
            last_naval_sink: None,
            last_trade: None,
            last_diplomacy: None,
            last_emote: None,
            last_ability: None,
            last_error: None,
            last_combat: HashMap::new(),
            pending_elimination: None,
            under_attack_active: false,
            result_played: false,
            cascade_chain: 0,
            last_cascade_at: None,
        }
    }
}

impl SfxDirector {
    pub(crate) fn reset_for_match(&mut self) {
        let mut fresh = Self::default();
        std::mem::swap(&mut fresh.pending, &mut self.pending);
        fresh.pending.clear();
        *self = fresh;
    }

    pub(crate) fn set_client_active(&mut self, active: bool) {
        self.client_active = active;
        if !active {
            self.under_attack_active = false;
            self.pending.clear();
        }
    }

    pub(crate) fn sync_server_phase(&mut self, phase: &GamePhase) {
        let next = TrackedPhase::from(phase);
        let prev = self.phase;
        self.phase = next;
        if next != TrackedPhase::Playing {
            self.under_attack_active = false;
        }
        if !self.phase_initialized {
            self.phase_initialized = true;
            if self.client_active && next == TrackedPhase::Spawning {
                self.push_ui(AudioCue::MatchPhaseStart, 0, 0.7, None);
            }
            return;
        }
        if self.client_active && prev != next {
            match next {
                TrackedPhase::Spawning => self.push_ui(AudioCue::MatchPhaseStart, 0, 0.7, None),
                TrackedPhase::Playing => self.push_ui(AudioCue::MatchPhaseStart, 1, 1.0, None),
                _ => {}
            }
        }
    }

    pub(crate) fn begin_tick(&mut self) {
        self.pending_elimination = None;
    }

    pub(crate) fn flush_tick(&mut self, now: Instant) {
        let Some(pending) = self.pending_elimination.take() else {
            return;
        };
        if !self.client_active
            || self.result_played
            || !Self::open_gap(&mut self.last_elimination, now, ELIMINATION_GAP)
        {
            return;
        }
        self.push_spatial(
            AudioCue::PlayerEliminated,
            pending.player_type as u8,
            pending.spatial,
            1.0,
            Some(pending.seed),
        );
    }

    /// Flushes all queued cues for the current frame into TypeScript in one FFI call.
    pub(crate) fn flush_frame(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        flush_audio_batch(&self.pending);
        self.pending.clear();
    }

    // ── Combat & Territory ─────────────────────────────────────────────

    /// Allowed during BOTH `Spawning` and `Playing` (fixes legacy silent spawn bug).
    pub(crate) fn play_deploy(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.client_active
            || self.result_played
            || !matches!(self.phase, TrackedPhase::Spawning | TrackedPhase::Playing)
            || !Self::open_gap(&mut self.last_deploy, now, DEPLOY_GAP)
        {
            return;
        }
        self.push_spatial(AudioCue::DeployTroops, 0, spatial, 0.8, None);
    }

    pub(crate) fn play_attack_order(
        &mut self,
        now: Instant,
        commitment: f32,
        against_enemy: bool,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_attack_order, now, ATTACK_ORDER_GAP) {
            return;
        }
        self.push_spatial(
            AudioCue::AttackOrder,
            u8::from(against_enemy),
            spatial,
            commitment.clamp(0.05, 1.0),
            None,
        );
    }

    pub(crate) fn play_attack_stalled(
        &mut self,
        now: Instant,
        severity: f32,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed()
            || !Self::open_gap(&mut self.last_attack_stalled, now, ATTACK_STALLED_GAP)
        {
            return;
        }
        self.push_spatial(
            AudioCue::AttackStalled,
            0,
            spatial,
            severity.clamp(0.15, 1.0),
            None,
        );
    }

    pub(crate) fn play_land_combat(
        &mut self,
        now: Instant,
        kind: CombatSoundKind,
        troops: f32,
        seed: u32,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() {
            return;
        }
        let gap = match kind {
            CombatSoundKind::Wilderness => Duration::from_millis(320),
            CombatSoundKind::Tribe => Duration::from_millis(340),
            CombatSoundKind::Nation => Duration::from_millis(420),
            CombatSoundKind::Human => Duration::from_millis(480),
            CombatSoundKind::Counter => Duration::from_millis(360),
        };
        if !self
            .last_combat
            .get(&kind)
            .is_none_or(|last| now.duration_since(*last) >= gap)
        {
            return;
        }
        self.last_combat.insert(kind, now);
        let intensity = (troops / 4000.0).clamp(0.15, 1.0).sqrt();
        if kind == CombatSoundKind::Wilderness {
            self.push_spatial(AudioCue::LandExpand, 0, spatial, intensity, Some(seed));
        } else {
            self.push_spatial(AudioCue::LandCombat, kind as u8, spatial, intensity, Some(seed));
        }
    }

    pub(crate) fn play_counter_attack(&mut self) {
        if !self.playing_allowed() {
            return;
        }
        self.push_ui(AudioCue::CounterAttack, 0, 0.9, None);
    }

    pub(crate) fn play_attack_cancelled(&mut self) {
        if !self.playing_allowed() {
            return;
        }
        self.push_ui(AudioCue::AttackCancelled, 0, 0.6, None);
    }

    pub(crate) fn update_under_attack(
        &mut self,
        now: Instant,
        active: bool,
        newly_relevant: bool,
        severity: f32,
        spatial: Option<SpatialSoundParams>,
    ) {
        if !active {
            self.under_attack_active = false;
            return;
        }
        let entered = !self.under_attack_active;
        self.under_attack_active = true;
        let Some(spatial) = spatial else {
            return;
        };
        if !self.playing_allowed()
            || (!entered && !newly_relevant)
            || !Self::open_gap(&mut self.last_under_attack, now, UNDER_ATTACK_GAP)
        {
            return;
        }
        self.push_spatial(
            AudioCue::UnderAttack,
            0,
            spatial,
            severity.clamp(0.15, 1.0),
            None,
        );
    }

    pub(crate) fn queue_elimination(
        &mut self,
        player_type: PlayerSoundType,
        seed: u32,
        spatial: SpatialSoundParams,
    ) {
        if !self.client_active || self.result_played {
            return;
        }
        let replace = self.pending_elimination.is_none_or(|pending| {
            (player_type as u8) > (pending.player_type as u8)
        });
        if replace {
            self.pending_elimination = Some(PendingElimination {
                player_type,
                seed,
                spatial,
            });
        }
    }

    pub(crate) fn play_cascade(
        &mut self,
        now: Instant,
        chain_step: u8,
        spatial: SpatialSoundParams,
    ) {
        if !self.client_active || self.result_played {
            return;
        }
        let step = if self
            .last_cascade_at
            .is_some_and(|last| now.duration_since(last).as_secs_f32() <= 4.5)
        {
            self.cascade_chain.saturating_add(1).max(chain_step).clamp(1, 8)
        } else {
            chain_step.clamp(1, 8)
        };
        self.cascade_chain = step;
        self.last_cascade_at = Some(now);
        self.push_spatial(AudioCue::CascadeStep, step, spatial, f32::from(step) / 8.0, None);
    }

    pub(crate) fn play_kill_streak(&mut self, chain_step: u8, spatial: SpatialSoundParams) {
        if !self.client_active || self.result_played {
            return;
        }
        let step = chain_step.clamp(2, 8);
        self.push_spatial(AudioCue::KillStreak, step, spatial, f32::from(step) / 8.0, None);
    }

    // ── Naval & Trade ──────────────────────────────────────────────────

    pub(crate) fn play_transport_launch(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_fleet, now, FLEET_GAP) {
            return;
        }
        self.push_spatial(AudioCue::TransportLaunch, 0, spatial, 0.8, None);
    }

    pub(crate) fn play_transport_landed(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_fleet, now, FLEET_GAP) {
            return;
        }
        self.push_spatial(AudioCue::TransportLanded, 0, spatial, 0.8, None);
    }

    pub(crate) fn play_fleet_order(
        &mut self,
        now: Instant,
        patrol: bool,
        spatial: Option<SpatialSoundParams>,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_fleet, now, FLEET_GAP) {
            return;
        }
        if let Some(s) = spatial {
            self.push_spatial(AudioCue::FleetOrder, u8::from(patrol), s, 0.8, None);
        } else {
            self.push_ui(AudioCue::FleetOrder, u8::from(patrol), 0.8, None);
        }
    }

    pub(crate) fn play_fleet_recall_or_scuttle(&mut self, scuttle: bool) {
        if !self.playing_allowed() {
            return;
        }
        self.push_ui(AudioCue::FleetRecallOrScuttle, u8::from(scuttle), 0.8, None);
    }

    pub(crate) fn play_warship_spawned(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_completion, now, COMPLETION_GAP) {
            return;
        }
        self.push_spatial(AudioCue::WarshipSpawned, 0, spatial, 0.9, None);
    }

    pub(crate) fn play_naval_shot(&mut self, now: Instant, seed: u32, spatial: SpatialSoundParams) {
        if !self.playing_allowed()
            || !Self::open_gap(&mut self.last_naval_shot, now, Duration::from_millis(140))
        {
            return;
        }
        self.push_spatial(AudioCue::NavalShot, 0, spatial, 0.75, Some(seed));
    }

    pub(crate) fn play_naval_impact(
        &mut self,
        now: Instant,
        sunk: bool,
        seed: u32,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() {
            return;
        }
        if sunk {
            if !Self::open_gap(&mut self.last_naval_sink, now, Duration::from_millis(380)) {
                return;
            }
            self.push_spatial(AudioCue::NavalSink, 0, spatial, 1.0, Some(seed));
        } else {
            if !Self::open_gap(&mut self.last_naval_impact, now, Duration::from_millis(170)) {
                return;
            }
            self.push_spatial(AudioCue::NavalImpact, 0, spatial, 0.8, Some(seed));
        }
    }

    pub(crate) fn play_trade_delivered(
        &mut self,
        now: Instant,
        gold: f32,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_trade, now, TRADE_GAP) {
            return;
        }
        let richness = (gold / 1500.0).clamp(0.2, 1.0);
        self.push_spatial(AudioCue::TradeDelivered, 0, spatial, richness, None);
    }

    pub(crate) fn play_trade_captured(
        &mut self,
        now: Instant,
        won_prize: bool,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_trade, now, TRADE_GAP) {
            return;
        }
        self.push_spatial(AudioCue::TradeCaptured, u8::from(won_prize), spatial, 0.8, None);
    }

    // ── Buildings & Economy ────────────────────────────────────────────

    pub(crate) fn play_placement(
        &mut self,
        now: Instant,
        kind: BuildingSoundKind,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_placement, now, PLACEMENT_GAP) {
            return;
        }
        self.push_spatial(AudioCue::BuildingPlace, kind as u8, spatial, 0.85, None);
    }

    pub(crate) fn play_upgrade_start(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_placement, now, PLACEMENT_GAP) {
            return;
        }
        self.push_spatial(AudioCue::BuildingUpgrade, 0, spatial, 0.85, None);
    }

    pub(crate) fn play_completion(
        &mut self,
        now: Instant,
        kind: BuildingSoundKind,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_completion, now, COMPLETION_GAP) {
            return;
        }
        self.push_spatial(AudioCue::BuildingComplete, kind as u8, spatial, 0.9, None);
    }

    pub(crate) fn play_arsenal_queued(&mut self, kind: ArsenalSoundKind) {
        if !self.playing_allowed() {
            return;
        }
        self.push_ui(AudioCue::ArsenalQueued, kind as u8, 0.8, None);
    }

    pub(crate) fn play_building_demolished(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.playing_allowed()
            || !Self::open_gap(&mut self.last_demolition, now, Duration::from_millis(280))
        {
            return;
        }
        self.push_spatial(AudioCue::BuildingDemolished, 0, spatial, 0.85, None);
    }

    // ── Nuclear & SAM Defense ──────────────────────────────────────────

    pub(crate) fn play_nuke_armed(&mut self, level: u8) {
        if !self.playing_allowed() {
            return;
        }
        self.push_ui(AudioCue::NukeArmed, level, 0.9, None);
    }

    pub(crate) fn play_nuke_launch(
        &mut self,
        now: Instant,
        level: u8,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_nuke_launch, now, NUKE_GAP) {
            return;
        }
        self.push_spatial(AudioCue::NukeLaunch, level, spatial, 1.0, None);
    }

    pub(crate) fn play_mirv_split(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_mirv_split, now, NUKE_GAP) {
            return;
        }
        self.push_spatial(AudioCue::MirvSplit, 3, spatial, 1.0, None);
    }

    pub(crate) fn play_sam_intercept(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.playing_allowed()
            || !Self::open_gap(&mut self.last_sam_intercept, now, Duration::from_millis(180))
        {
            return;
        }
        self.push_spatial(AudioCue::SamIntercept, 0, spatial, 0.9, None);
    }

    pub(crate) fn play_nuke_impact(
        &mut self,
        now: Instant,
        level: u8,
        spatial: SpatialSoundParams,
    ) {
        if !self.playing_allowed()
            || !Self::open_gap(&mut self.last_nuke_impact, now, Duration::from_millis(220))
        {
            return;
        }
        let intensity = match level {
            2 => 1.0,
            1 => 0.85,
            _ => 0.72,
        };
        self.push_spatial(AudioCue::NukeImpact, level, spatial, intensity, None);
    }

    // ── Diplomacy, Abilities & Emotes ──────────────────────────────────

    pub(crate) fn play_alliance_request(&mut self, now: Instant) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_diplomacy, now, DIPLOMACY_GAP) {
            return;
        }
        self.push_ui(AudioCue::AllianceRequest, 0, 0.85, None);
    }

    pub(crate) fn play_alliance_formed(&mut self, now: Instant) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_diplomacy, now, DIPLOMACY_GAP) {
            return;
        }
        self.push_ui(AudioCue::AllianceFormed, 0, 0.95, None);
    }

    pub(crate) fn play_alliance_ended(&mut self, now: Instant) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_diplomacy, now, DIPLOMACY_GAP) {
            return;
        }
        self.push_ui(AudioCue::AllianceEnded, 0, 0.75, None);
    }

    pub(crate) fn play_betrayal(&mut self) {
        if !self.playing_allowed() {
            return;
        }
        self.push_ui(AudioCue::Betrayal, 0, 1.0, None);
    }

    pub(crate) fn play_resource_transfer(&mut self, now: Instant, kind: ResourceSoundKind) {
        if !self.playing_allowed()
            || !Self::open_gap(&mut self.last_diplomacy, now, Duration::from_millis(220))
        {
            return;
        }
        self.push_ui(AudioCue::ResourceTransfer, kind as u8, 0.8, None);
    }

    pub(crate) fn play_hero_ability(
        &mut self,
        now: Instant,
        family: u8,
        spatial: Option<SpatialSoundParams>,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_ability, now, ABILITY_GAP) {
            return;
        }
        if let Some(s) = spatial {
            self.push_spatial(AudioCue::HeroAbility, family, s, 0.85, None);
        } else {
            self.push_ui(AudioCue::HeroAbility, family, 0.85, None);
        }
    }

    pub(crate) fn play_emote(
        &mut self,
        now: Instant,
        kind: u8,
        spatial: Option<SpatialSoundParams>,
    ) {
        if !self.playing_allowed() || !Self::open_gap(&mut self.last_emote, now, EMOTE_GAP) {
            return;
        }
        if let Some(s) = spatial {
            self.push_spatial(AudioCue::Emote, kind, s, 0.8, None);
        } else {
            self.push_ui(AudioCue::Emote, kind, 0.8, None);
        }
    }

    // ── Errors & Match Outcome ─────────────────────────────────────────

    pub(crate) fn play_action_error(&mut self, now: Instant) {
        if !self.client_active || !Self::open_gap(&mut self.last_error, now, ERROR_GAP) {
            return;
        }
        self.push_ui(AudioCue::ActionError, 0, 0.75, None);
    }

    pub(crate) fn play_result(&mut self, won: bool) {
        if !self.client_active || self.result_played {
            return;
        }
        self.result_played = true;
        self.push_ui(if won { AudioCue::Victory } else { AudioCue::Defeat }, 0, 1.0, None);
    }

    // ── Internal Helpers ───────────────────────────────────────────────

    fn playing_allowed(&self) -> bool {
        self.client_active && self.phase == TrackedPhase::Playing && !self.result_played
    }

    fn next_seed(&mut self, explicit: Option<u32>) -> u32 {
        if let Some(s) = explicit {
            return s & 0x00FF_FFFF;
        }
        self.seed_seq = self.seed_seq.wrapping_mul(1664525).wrapping_add(1013904223);
        self.seed_seq & 0x00FF_FFFF
    }

    fn push_spatial(
        &mut self,
        cue: AudioCue,
        subtype: u8,
        spatial: SpatialSoundParams,
        intensity: f32,
        seed: Option<u32>,
    ) {
        let (pan, gain) = spatial_pan_gain(spatial);
        if gain <= 0.02 {
            return;
        }
        let s = self.next_seed(seed);
        self.pending.extend_from_slice(&[
            cue as u16 as f32,
            f32::from(subtype),
            pan,
            gain,
            intensity.clamp(0.0, 1.0),
            s as f32,
        ]);
    }

    fn push_ui(&mut self, cue: AudioCue, subtype: u8, intensity: f32, seed: Option<u32>) {
        let s = self.next_seed(seed);
        self.pending.extend_from_slice(&[
            cue as u16 as f32,
            f32::from(subtype),
            0.0,
            1.0,
            intensity.clamp(0.0, 1.0),
            s as f32,
        ]);
    }

    fn open_gap(last: &mut Option<Instant>, now: Instant, gap: Duration) -> bool {
        if last.is_some_and(|previous| now.duration_since(previous) < gap) {
            return false;
        }
        *last = Some(now);
        true
    }
}
