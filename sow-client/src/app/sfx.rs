use std::collections::HashMap;

use sow_audio::{BuildingSoundKind, CombatSoundKind, PlayerSoundType, SpatialSoundParams};
use sow_core::game::GamePhase;
use web_time::{Duration, Instant};

const DEPLOY_GAP: Duration = Duration::from_millis(180);
const PLACEMENT_GAP: Duration = Duration::from_millis(250);
const ORDINARY_GAP: Duration = Duration::from_millis(160);
const COMPLETION_GAP: Duration = Duration::from_millis(500);
const UNDER_ATTACK_GAP: Duration = Duration::from_secs(8);
const ELIMINATION_GAP: Duration = Duration::from_millis(700);
const NUKE_GAP: Duration = Duration::from_millis(800);
const CRITICAL_SILENCE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
struct PendingElimination {
    player_type: PlayerSoundType,
    seed: u32,
    spatial: SpatialSoundParams,
}

#[derive(Default)]
pub(crate) struct SfxDirector {
    client_active: bool,
    server_playing: bool,
    last_ordinary: Option<Instant>,
    last_deploy: Option<Instant>,
    last_placement: Option<Instant>,
    last_completion: Option<Instant>,
    last_under_attack: Option<Instant>,
    last_elimination: Option<Instant>,
    last_nuke_launch: Option<Instant>,
    last_nuke_impact: Option<Instant>,
    last_combat: HashMap<CombatSoundKind, Instant>,
    critical_silence_until: Option<Instant>,
    pending_elimination: Option<PendingElimination>,
    under_attack_active: bool,
    result_played: bool,
}

impl SfxDirector {
    pub(crate) fn reset_for_match(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn set_client_active(&mut self, active: bool) {
        self.client_active = active;
        if !active {
            self.under_attack_active = false;
        }
    }

    pub(crate) fn sync_server_phase(&mut self, phase: &GamePhase) {
        self.server_playing = matches!(phase, GamePhase::Playing);
        if !self.server_playing {
            self.under_attack_active = false;
        }
    }

    pub(crate) fn begin_tick(&mut self) {
        self.pending_elimination = None;
    }

    pub(crate) fn flush_tick(&mut self, now: Instant) {
        let Some(pending) = self.pending_elimination.take() else {
            return;
        };
        if !self.gameplay_allowed(now)
            || !Self::gap_open(self.last_elimination, now, ELIMINATION_GAP)
        {
            return;
        }
        self.last_elimination = Some(now);
        self.mark_critical(now);
        sow_audio::play_death_sound(pending.player_type, pending.seed, pending.spatial);
    }

    pub(crate) fn play_deploy(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.ordinary_allowed(now) || !Self::open_gap(&mut self.last_deploy, now, DEPLOY_GAP) {
            return;
        }
        self.last_ordinary = Some(now);
        sow_audio::play_deploy_sound(spatial);
    }

    pub(crate) fn play_placement(
        &mut self,
        now: Instant,
        kind: BuildingSoundKind,
        spatial: SpatialSoundParams,
    ) {
        if !self.ordinary_allowed(now)
            || !Self::open_gap(&mut self.last_placement, now, PLACEMENT_GAP)
        {
            return;
        }
        self.last_ordinary = Some(now);
        sow_audio::play_building_placement_sound(kind, spatial);
    }

    pub(crate) fn play_completion(
        &mut self,
        now: Instant,
        kind: BuildingSoundKind,
        spatial: SpatialSoundParams,
    ) {
        if !self.gameplay_allowed(now)
            || !Self::open_gap(&mut self.last_completion, now, COMPLETION_GAP)
        {
            return;
        }
        self.mark_critical(now);
        sow_audio::play_building_completed_sound(kind, spatial);
    }

    pub(crate) fn play_combat(
        &mut self,
        now: Instant,
        kind: CombatSoundKind,
        troops: f32,
        seed: u32,
        spatial: SpatialSoundParams,
    ) {
        if !self.ordinary_allowed(now) {
            return;
        }
        let gap = match kind {
            CombatSoundKind::WildernessExpansion | CombatSoundKind::AttackTribe => {
                Duration::from_millis(350)
            }
            CombatSoundKind::AttackEmpire => Duration::from_millis(550),
            CombatSoundKind::AttackHuman => Duration::from_millis(750),
            CombatSoundKind::CounterAttack => Duration::from_millis(500),
        };
        if !self
            .last_combat
            .get(&kind)
            .is_none_or(|last| now.duration_since(*last) >= gap)
        {
            return;
        }
        self.last_combat.insert(kind, now);
        self.last_ordinary = Some(now);
        sow_audio::play_combat_sound(kind, troops, seed, spatial);
    }

    pub(crate) fn update_under_attack(
        &mut self,
        now: Instant,
        active: bool,
        newly_relevant: bool,
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
        if !self.gameplay_allowed(now)
            || (!entered && !newly_relevant)
            || !Self::open_gap(&mut self.last_under_attack, now, UNDER_ATTACK_GAP)
        {
            return;
        }
        self.mark_critical(now);
        sow_audio::play_under_attack_sound(spatial);
    }

    pub(crate) fn queue_elimination(
        &mut self,
        player_type: PlayerSoundType,
        seed: u32,
        spatial: SpatialSoundParams,
    ) {
        if !self.client_active || !self.server_playing {
            return;
        }
        let replace = self.pending_elimination.is_none_or(|pending| {
            elimination_rank(player_type) > elimination_rank(pending.player_type)
        });
        if replace {
            self.pending_elimination = Some(PendingElimination {
                player_type,
                seed,
                spatial,
            });
        }
    }

    pub(crate) fn play_nuke_launch(&mut self, now: Instant, spatial: SpatialSoundParams) {
        if !self.gameplay_allowed(now) || !Self::open_gap(&mut self.last_nuke_launch, now, NUKE_GAP) {
            return;
        }
        self.mark_critical(now);
        sow_audio::play_nuke_launch_sound(spatial);
    }

    pub(crate) fn play_nuke_impact(
        &mut self,
        now: Instant,
        level: u8,
        spatial: SpatialSoundParams,
    ) {
        if !self.gameplay_allowed(now) || !Self::open_gap(&mut self.last_nuke_impact, now, NUKE_GAP) {
            return;
        }
        self.mark_critical(now);
        sow_audio::play_nuke_impact_sound(level, spatial);
    }

    pub(crate) fn play_result(&mut self, won: bool) {
        if !self.client_active || self.result_played {
            return;
        }
        self.result_played = true;
        if won {
            sow_audio::play_victory_sound();
        } else {
            sow_audio::play_defeat_sound();
        }
    }

    fn gameplay_allowed(&self, now: Instant) -> bool {
        self.client_active && self.server_playing && !self.critical_active(now)
    }

    fn ordinary_allowed(&self, now: Instant) -> bool {
        self.gameplay_allowed(now) && Self::gap_open(self.last_ordinary, now, ORDINARY_GAP)
    }

    fn critical_active(&self, now: Instant) -> bool {
        self.critical_silence_until.is_some_and(|until| until > now)
    }

    fn mark_critical(&mut self, now: Instant) {
        self.critical_silence_until = Some(now + CRITICAL_SILENCE);
    }

    fn gap_open(last: Option<Instant>, now: Instant, gap: Duration) -> bool {
        last.is_none_or(|previous| now.duration_since(previous) >= gap)
    }

    fn open_gap(last: &mut Option<Instant>, now: Instant, gap: Duration) -> bool {
        if !Self::gap_open(*last, now, gap) {
            return false;
        }
        *last = Some(now);
        true
    }
}

fn elimination_rank(player_type: PlayerSoundType) -> u8 {
    match player_type {
        PlayerSoundType::Bot => 0,
        PlayerSoundType::Nation => 1,
        PlayerSoundType::Human => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silent_spatial() -> SpatialSoundParams {
        SpatialSoundParams {
            wx: 0.0,
            wy: 0.0,
            camera_x: 0.0,
            camera_y: 0.0,
            camera_zoom: 1.0,
            screen_w: 1280.0,
            screen_h: 720.0,
        }
    }

    #[test]
    fn gameplay_sfx_are_silent_during_spawning() {
        let mut director = SfxDirector::default();
        director.set_client_active(true);
        director.sync_server_phase(&GamePhase::Spawning { end_tick: 10 });
        director.play_deploy(Instant::now(), silent_spatial());
        assert!(director.last_deploy.is_none());
    }

    #[test]
    fn combat_gap_suppresses_bursts() {
        let mut director = SfxDirector::default();
        director.set_client_active(true);
        director.sync_server_phase(&GamePhase::Playing);
        let now = Instant::now();

        director.play_combat(
            now,
            CombatSoundKind::AttackTribe,
            500.0,
            1,
            silent_spatial(),
        );
        let first = director
            .last_combat
            .get(&CombatSoundKind::AttackTribe)
            .copied();
        director.play_combat(
            now + Duration::from_millis(100),
            CombatSoundKind::AttackTribe,
            500.0,
            2,
            silent_spatial(),
        );
        assert_eq!(
            director
                .last_combat
                .get(&CombatSoundKind::AttackTribe)
                .copied(),
            first
        );
        director.play_combat(
            now + Duration::from_millis(350),
            CombatSoundKind::AttackTribe,
            500.0,
            3,
            silent_spatial(),
        );
        assert_ne!(
            director
                .last_combat
                .get(&CombatSoundKind::AttackTribe)
                .copied(),
            first
        );
    }

    #[test]
    fn elimination_keeps_the_highest_priority_event_in_a_tick() {
        let mut director = SfxDirector::default();
        director.set_client_active(true);
        director.sync_server_phase(&GamePhase::Playing);
        director.begin_tick();
        director.queue_elimination(PlayerSoundType::Bot, 1, silent_spatial());
        director.queue_elimination(PlayerSoundType::Human, 2, silent_spatial());
        assert!(matches!(
            director.pending_elimination,
            Some(PendingElimination {
                player_type: PlayerSoundType::Human,
                ..
            })
        ));
    }
}
