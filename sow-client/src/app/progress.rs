use super::state::SowApp;

impl SowApp {
    pub(crate) fn complete_boudica_tutorial(&mut self) {
        if self.progress.mark_tutorial_completed() {
            self.save_local_progress();
        }
        // A local completion marker is not proof that the server paid the reward.
        if !self.progress.reward_receipts.contains_key("tutorial") {
            self.persist_tutorial_completion();
        }
    }

    pub(crate) fn reset_progress_session(&mut self) {
        self.progress_match_recorded = false;
        self.progress_session_defeats = crate::player_progress::SessionDefeats::default();
        self.portal_match_started_at = None;
        self.portal_join_started_at = None;
        self.portal_match_mode = "other";
    }

    pub(crate) fn maybe_record_match_progress(
        &mut self,
        snap: &sow_core::protocol::SimSnapshot,
        winner: Option<u16>,
        winning_team: Option<sow_core::protocol::Team>,
        my_team: Option<sow_core::protocol::Team>,
    ) {
        if self.progress_match_recorded {
            return;
        }
        let my_id = self.sim.my_player_id.unwrap_or(0);
        if my_id == 0 {
            return;
        }

        let Some(me) = snap.players.iter().find(|p| p.id == my_id) else {
            return;
        };
        let eliminated = !me.alive && me.has_spawned;
        let Some(winner_id) = winner.or_else(|| eliminated.then_some(0)) else {
            return;
        };
        self.progress_match_recorded = true;
        let won = if let Some(team) = winning_team {
            my_team == Some(team)
        } else {
            winner_id == my_id
        };
        let outcome = if won { "win" } else { "loss" };
        let duration_bucket = crate::store_portals::duration_bucket(self.portal_match_started_at);
        crate::store_portals::gameplay_stop_match_ended();
        crate::store_portals::track_product_event_with(
            "match_ended_client",
            &serde_json::json!({
                "mode": self.portal_match_mode,
                "outcome": outcome,
                "duration_bucket": duration_bucket,
            }),
        );
        let defeats = self.progress_session_defeats;
        let (kills, deaths, assists) = (me.kills, me.deaths, me.assists);
        let server_owned_tutorial = self.progress_account_id.is_some()
            && self.net.is_offline
            && self.sim.config.tutorial
            && self.ui.tutorial_campaign.is_server_owned_tutorial();
        let server_owned_match =
            self.progress_account_id.is_some() && (!self.net.is_offline || server_owned_tutorial);
        // The relay/database own online results; the database owns the one-time
        // Boudica tutorial reward.
        if server_owned_match {
            if !self.net.is_offline {
                self.prepare_match_result(snap.tick.saturating_sub(1));
            }
            log::info!(
                "Server-owned result detected (winner={winner_id}); preparing rewards while the endgame panel remains open"
            );
            return;
        }

        self.progress.record_match_with_kda_for_leader(
            won,
            defeats,
            kills,
            deaths,
            assists,
            self.sim.config.player_leader,
        );
        self.save_local_progress();
        log::info!(
            "Recorded local match progress: won={won}, defeats={defeats:?}, level={}",
            self.progress.level
        );
    }
}
