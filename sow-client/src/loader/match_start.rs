use crate::app::SowApp;
use sow_core::game_config::GameConfig;

#[cfg(target_arch = "wasm32")]
use crate::ClientPhase;

impl SowApp {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn finish_boot_to_main_menu(&mut self) {
        self.ui.app.splash_state.done = true;
        self.ui.app.phase = ClientPhase::MainMenu;
        crate::store_portals::load_stop();
        crate::store_portals::gameplay_stop();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn finish_boot_route(&mut self) {
        if self.progress.is_first_game() && self.boot_campaign_pending.is_some() {
            return;
        }
        crate::store_portals::load_stop();
        if !self.progress.is_first_game() {
            log::info!("Portal boot: returning player -> main menu");
            crate::store_portals::gameplay_stop();
            self.ui.app.splash_state.done = true;
            self.ui.app.phase = ClientPhase::MainMenu;
        } else {
            log::info!("Portal boot: new player -> JavaScript campaign bootstrap");
            self.ui.app.main_menu_state.host_private_pending = false;
            let campaign = crate::campaign::CampaignId::Boudica;
            self.boot_campaign_pending = Some(campaign.episode_id().to_string());
            self.begin_enter_game_loader(campaign.advisor());
            crate::store_portals::gameplay_stop();
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn start_campaign_episode_from_web(
        &mut self,
        campaign: crate::campaign::CampaignId,
        roster: serde_json::Value,
        match_config: serde_json::Value,
    ) -> Result<(), String> {
        let expected_map = match campaign {
            crate::campaign::CampaignId::Boudica => "eastanglia",
            crate::campaign::CampaignId::SixSkyEp1
            | crate::campaign::CampaignId::SixSkyEp2
            | crate::campaign::CampaignId::SixSkyEp3 => "northamerica",
        };
        if roster.get("map").and_then(serde_json::Value::as_str) != Some(expected_map) {
            return Err("Campaign roster references the wrong map.".into());
        }
        let roster_text = serde_json::to_string(&roster)
            .map_err(|_| "Campaign roster could not be read.".to_string())?;
        let (factions, player_spawn, player_color) = crate::campaign::parse_roster(&roster_text)
            .ok_or_else(|| "Campaign roster is invalid.".to_string())?;
        let (map_width, map_height) = match campaign {
            crate::campaign::CampaignId::Boudica => (896, 504),
            crate::campaign::CampaignId::SixSkyEp1
            | crate::campaign::CampaignId::SixSkyEp2
            | crate::campaign::CampaignId::SixSkyEp3 => (1000, 516),
        };
        if player_spawn.0 >= map_width
            || player_spawn.1 >= map_height
            || factions
                .iter()
                .any(|faction| faction.x >= map_width || faction.y >= map_height)
        {
            return Err("Campaign roster contains an out-of-bounds spawn.".into());
        }
        let options = match_config
            .as_object()
            .ok_or_else(|| "Campaign settings are invalid.".to_string())?;
        for key in options.keys() {
            if !matches!(
                key.as_str(),
                "buildings_enabled"
                    | "starting_troops"
                    | "buildings_unlock_after_defeated"
                    | "campaign_support"
            ) {
                return Err(format!("Campaign setting is not allowed: {key}"));
            }
        }
        let buildings_enabled = options
            .get("buildings_enabled")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| "Campaign buildings_enabled is invalid.".to_string())?;
        let starting_troops = options
            .get("starting_troops")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite() && (1.0..=100_000.0).contains(value))
            .ok_or_else(|| "Campaign starting_troops is invalid.".to_string())?;
        let resolve_faction_display_name = |reference: &str| {
            factions
                .iter()
                .find(|faction| faction.id == reference || faction.name == reference)
                .map(|faction| faction.name.clone())
        };
        let buildings_unlock_after_defeated = options
            .get("buildings_unlock_after_defeated")
            .map(|value| {
                value
                    .as_str()
                    .and_then(&resolve_faction_display_name)
                    .ok_or_else(|| "Campaign building unlock target is invalid.".to_string())
            })
            .transpose()?;
        let campaign_support = options
            .get("campaign_support")
            .map(|value| {
                let support = value
                    .as_object()
                    .ok_or_else(|| "Campaign support settings are invalid.".to_string())?;
                if support
                    .keys()
                    .any(|key| !matches!(key.as_str(), "after_defeated" | "share_percent"))
                {
                    return Err("Campaign support contains an unknown setting.".to_string());
                }
                let after_defeated = support
                    .get("after_defeated")
                    .and_then(serde_json::Value::as_str)
                    .and_then(&resolve_faction_display_name)
                    .ok_or_else(|| "Campaign support milestone is invalid.".to_string())?;
                let share_percent = support
                    .get("share_percent")
                    .and_then(serde_json::Value::as_u64)
                    .filter(|value| (1..=100).contains(value))
                    .ok_or_else(|| "Campaign support share must be 1–100 percent.".to_string())?
                    as u8;
                Ok(sow_core::game_config::CampaignSupport {
                    after_defeated,
                    share_percent,
                })
            })
            .transpose()?;
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let (leader, civilization) = match campaign {
            crate::campaign::CampaignId::Boudica => (
                sow_core::player::Leader::Boudica,
                sow_core::player::Civilization::Iceni,
            ),
            crate::campaign::CampaignId::SixSkyEp1
            | crate::campaign::CampaignId::SixSkyEp2
            | crate::campaign::CampaignId::SixSkyEp3 => (
                sow_core::player::Leader::LadySixSky,
                sow_core::player::Civilization::Maya,
            ),
        };
        self.ui.tutorial_campaign = campaign;
        crate::campaign::log_plan_for(
            campaign.menu_title(),
            campaign.menu_subtitle(),
            player_spawn,
            &factions,
        );
        self.start_offline_match(
            GameConfig {
                map_name: expected_map.to_string(),
                bot_count: 0,
                nation_count: 0,
                seed,
                random_spawn: true,
                player_leader: leader,
                player_civilization: civilization,
                scripted_spawns: crate::campaign::to_scripted(&factions),
                player_spawn: Some(player_spawn),
                player_team: None,
                campaign_player_color: Some(player_color),
                starting_troops,
                global_speed_multiplier: 0.5,
                buildings_enabled,
                buildings_unlock_after_defeated,
                campaign_support,
                ..Default::default()
            },
            true,
        );
        Ok(())
    }

    pub(crate) fn start_offline_match(&mut self, mut config: GameConfig, tutorial: bool) {
        let leader = if tutorial {
            config.player_leader
        } else {
            self.ui.app.main_menu_state.selected_leader
        };
        config.player_leader = leader;
        config.player_civilization = leader.civilization();
        self.net.is_offline = true;
        self.sim.offline_tick_timer = 0.0;
        self.sim.offline_last_update = web_time::Instant::now();
        self.sim.paused = false;
        self.sim.tutorial_observation.reset();
        self.net.client = None;
        self.net.current_ping_ms = None;
        self.begin_enter_game_loader(leader);
        self.sim.my_player_id = Some(1);
        self.sim.my_lobby_id = Some(0);
        // Ride the tutorial signal in the match config. Engine initialization derives the active
        // mode from this single flag, while the browser owns the step state and presentation.
        config.tutorial = tutorial;
        if tutorial {
            log::info!(
                "tutorial: {} started (map={})",
                self.ui.tutorial_campaign.episode_id(),
                config.map_name
            );
        }
    }
}
