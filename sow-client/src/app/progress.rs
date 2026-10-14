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
        crate::store_portals::track_product_event("match_ended_client");
        let defeats = self.progress_session_defeats;
        let (kills, deaths, assists) = (me.kills, me.deaths, me.assists);
        let server_owned_tutorial = self.progress_account_id.is_some()
            && self.net.is_offline
            && self.sim.config.tutorial
            && self.ui.tutorial_campaign == crate::campaign::CampaignId::Boudica;
        let server_owned_match =
            self.progress_account_id.is_some() && (!self.net.is_offline || server_owned_tutorial);
        // The relay/database own online results; the database owns the one-time
        // Boudica tutorial reward.
        if server_owned_match {
            log::info!(
                "Server-owned match ended (winner={winner_id}); profile will sync from sow-database on menu return"
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
