use super::state::{ExitRewardPreview, SowApp};

impl SowApp {
    fn store_exit_reward_preview(
        &mut self,
        receipt_id: impl Into<String>,
        reward: sow_data::rewards::MatchReward,
    ) {
        let Some(account_id) = self.progress_account_id.clone() else {
            return;
        };
        self.exit_reward_preview = Some(ExitRewardPreview {
            receipt_id: receipt_id.into(),
            account_id,
            reward,
            base_xp: self.progress.xp,
            base_level: self.progress.level.max(1),
            base_crowns: self.progress.crowns,
            base_laurels: self.progress.laurels,
        });
    }

    pub(crate) fn capture_online_reward_preview(
        &mut self,
        match_id: u64,
        leader: sow_core::player::Leader,
    ) {
        let mut reward =
            sow_data::rewards::calculate(sow_data::rewards::RewardInput::default());
        reward.laurels = self.progress.preview_participation_laurels(leader, reward);
        self.store_exit_reward_preview(match_id.to_string(), reward);
    }

    pub(crate) fn capture_tutorial_reward_preview(&mut self) {
        self.store_exit_reward_preview(
            "tutorial",
            sow_data::rewards::calculate(sow_data::rewards::RewardInput {
                tutorial: true,
                ..Default::default()
            }),
        );
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
        crate::store_portals::measure("match", "round", "complete");

        let won = if let Some(team) = winning_team {
            my_team == Some(team)
        } else {
            winner_id == my_id
        };
        let defeats = self.progress_session_defeats;
        let (kills, deaths, assists) = (me.kills, me.deaths, me.assists);
        crate::analytics::track_with(
            "match_ended_client",
            serde_json::json!({
                "won": won,
                "offline": self.net.is_offline,
                "tutorial": self.sim.config.tutorial,
                "kills": kills,
            }),
        );
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

        self.progress.preferred_leader = Some(self.ui.app.main_menu_state.selected_leader);
        self.progress
            .record_match_with_kda(won, defeats, kills, deaths, assists);
        self.save_local_progress();
        log::info!(
            "Recorded local match progress: won={won}, defeats={defeats:?}, level={}",
            self.progress.level
        );
    }
}
