use crate::app::SowApp;
use sow_core::protocol::Turn;

impl SowApp {
    pub(crate) fn handle_sim_turn(&mut self, turn: Turn) {
        let (mut snap, events) = {
            let Some(e) = self.sim.engine.as_mut() else {
                return;
            };
            e.apply_intents(&turn.intents);
            e.tick();
            let snap = e.build_snapshot();
            let events: Vec<_> = std::mem::take(&mut e.state.events);
            (snap, events)
        };

        let my_id = self.sim.my_player_id.unwrap_or(0);
        let turn_defeats = self.process_tick_events(events, &snap, my_id);

        self.progress_session_defeats.players = self
            .progress_session_defeats
            .players
            .saturating_add(turn_defeats.players);
        self.progress_session_defeats.empires = self
            .progress_session_defeats
            .empires
            .saturating_add(turn_defeats.empires);
        self.progress_session_defeats.tribes = self
            .progress_session_defeats
            .tribes
            .saturating_add(turn_defeats.tribes);

        self.apply_snapshot_fx(&mut snap, my_id);
        self.process_nuke_alerts(&snap);

        let my_team = snap
            .players
            .iter()
            .find(|p| p.id == my_id)
            .and_then(|p| p.team);
        self.maybe_record_match_progress(&snap, snap.winner, snap.winning_team, my_team);

        // Viewport Alerts and one-shot result sound: Victory / Defeat.
        let match_won = snap
            .winner
            .map(|winner| winner == my_id)
            .or_else(|| snap.winning_team.map(|team| Some(team) == my_team));
        if let Some(won) = match_won {
            self.sfx.play_result(won);
            self.ui.trigger_viewport_alert(if won {
                crate::app::ViewportAlertKind::Victory
            } else {
                crate::app::ViewportAlertKind::Defeat
            });
        }

        self.sim.current_snapshot = Some(snap);

        // Recompute Fog of War visibility
        if let Some(ref snap_ref) = self.sim.current_snapshot {
            let owners = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners.as_slice())
                .unwrap_or(&[]);
            self.sim
                .fog_explored
                .blocks
                .resize((self.sim.map_w * self.sim.map_h + 63) as usize / 64, 0);
            self.sim
                .fog_visible
                .blocks
                .resize((self.sim.map_w * self.sim.map_h + 63) as usize / 64, 0);
            let dev = crate::theme::dev_config::DevConfig::get();
            crate::sim::visibility::compute_visibility(
                (self.sim.map_w, self.sim.map_h),
                my_id,
                owners,
                snap_ref,
                &mut self.sim.fog_explored,
                &mut self.sim.fog_visible,
                dev.fog_of_war,
            );
            self.sim.force_fog_upload = true;
        }

        self.time.interp.stamp_applied(web_time::Instant::now());
    }
}
