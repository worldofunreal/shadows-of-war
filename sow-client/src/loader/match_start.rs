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
            crate::store_portals::track_product_event_with(
                "boot_route_decision",
                &serde_json::json!({ "route": "menu" }),
            );
            self.ui.app.splash_state.done = true;
            self.ui.app.phase = ClientPhase::MainMenu;
        } else {
            log::info!("Portal boot: new player -> JavaScript campaign bootstrap");
            crate::store_portals::track_product_event_with(
                "boot_route_decision",
                &serde_json::json!({ "route": "intro" }),
            );
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
                    | "starting_gold"
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
        let starting_gold = options
            .get("starting_gold")
            .map(|value| {
                value
                    .as_f64()
                    .filter(|value| value.is_finite() && (0.0..=1_000_000.0).contains(value))
                    .ok_or_else(|| "Campaign starting_gold is invalid.".to_string())
            })
            .transpose()?
            .unwrap_or_else(|| GameConfig::default().starting_gold);
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
                starting_gold,
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
        self.ui.tutorial_waiting_for_first_attack = tutorial
            && self.ui.tutorial_campaign == crate::campaign::CampaignId::Boudica
            && !config.scripted_spawns.is_empty();
        self.ui.tutorial_paused_action = None;
        self.sim.paused = self.ui.tutorial_waiting_for_first_attack;
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
            crate::store_portals::track_product_event_with(
                "tutorial_start",
                &serde_json::json!({ "episode_id": self.ui.tutorial_campaign.episode_id() }),
            );
        }

        let map_id = crate::ui::asset_loader::AssetLoader::map_key(&config.map_name);
        self.ui.app.main_menu_state.downloading_map_name = Some(map_id.clone());

        config.map_name = map_id.clone();
        if let Some(catalog) = &self.ui.app.asset_loader.map_catalog {
            if let Some(entry) = sow_core::maps::catalog_lookup(catalog, &map_id) {
                config.map_width = entry.width;
                config.map_height = entry.height;
                config.map_name = entry.key.clone();
            } else {
                log::debug!("Map '{}' not in catalog.bin", map_id);
            }
        }
        if let Some(payload) =
            sow_core::maps::load_map_br_payload(&map_id, crate::map_cache::load(&map_id))
            && let Ok(map_file) = sow_core::maps::load_map_from_payload(&payload)
        {
            config.map_width = map_file.width;
            config.map_height = map_file.height;
            self.ui
                .app
                .asset_loader
                .maps
                .insert(map_id.clone(), payload);
        }

        let start_msg = sow_core::protocol::ServerStartMessage {
            lobby_id: None,
            config: config.clone(),
            my_player_id: Some(1),
            seed: config.seed,
            players: vec![sow_core::protocol::PlayerInfo {
                id: 1,
                name: {
                    if tutorial {
                        leader.name().to_string()
                    } else {
                        let name = &self.ui.app.main_menu_state.player_name;
                        let tag = &self.ui.app.main_menu_state.clan_tag;
                        if tag.is_empty() {
                            name.clone()
                        } else {
                            format!("[{}] {}", tag, name)
                        }
                    }
                },
                color: leader.filler_rgb(),
                player_type: sow_core::player::PlayerType::Human,
                team: config.player_team.or(match config.game_mode.as_str() {
                    "Teams" | "HumansVsNations" => Some(sow_core::protocol::Team::Red),
                    _ => None,
                }),
                spawn_x: 0,
                spawn_y: 0,
                civilization: leader.civilization(),
                leader,
                skin_style: sow_data::commerce::skin_style_for_profile(
                    &self.progress.owned_skins,
                    self.progress.selected_skin.as_deref(),
                ),
                is_ai_controlled: false,
            }],
            missed_turns: vec![],
            relay_port: None,
            relay_host: None,
        };
        self.tasks.engine_init_queued_msg = Some(start_msg);

        if self.ui.app.asset_loader.has_map(&map_id) {
            self.ui.app.main_menu_state.cached_map = self.ui.app.asset_loader.take_map(&map_id);
            self.ui.app.main_menu_state.cached_map_key = Some(map_id.clone());
            self.ui.app.main_menu_state.is_downloading_map = false;
        } else {
            self.ui.app.main_menu_state.is_downloading_map = true;
            self.ui.app.main_menu_state.cached_map = None;
            self.ui.app.main_menu_state.cached_map_key = None;
            let maps_base = self.asset_config.maps_base.clone();
            let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_id);
            let tx = self.tasks.map_tx.clone();
            let request = ehttp::Request::get(&url);
            let map_name_for_closure = map_id.clone();
            let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));

            ehttp::streaming::fetch(
                request,
                move |result: ehttp::Result<ehttp::streaming::Part>| match result {
                    Ok(ehttp::streaming::Part::Response(res)) => {
                        if !res.ok {
                            let _ = tx.send(crate::MapDownloadEvent::Error(format!(
                                "HTTP Error: {}",
                                res.status
                            )));
                            return std::ops::ControlFlow::Break(());
                        }
                        let cl = res
                            .headers
                            .get("content-length")
                            .or_else(|| res.headers.get("Content-Length"));
                        if let Some(cl_str) = cl
                            && let Ok(b) = cl_str.parse::<usize>()
                        {
                            *total_bytes.lock().unwrap() = b;
                        }
                        std::ops::ControlFlow::Continue(())
                    }
                    Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                        if chunk.is_empty() {
                            let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
                            let _ = tx.send(crate::MapDownloadEvent::MapReady(
                                map_name_for_closure.clone(),
                                final_bytes,
                            ));
                            return std::ops::ControlFlow::Break(());
                        }
                        let mut acc = accumulated.lock().unwrap();
                        acc.extend_from_slice(&chunk);
                        let total = *total_bytes.lock().unwrap();
                        let pct = if total > 0 {
                            ((acc.len() as f64 / total as f64) * 100.0) as u8
                        } else {
                            0
                        };
                        let _ = tx.send(crate::MapDownloadEvent::Progress(
                            map_name_for_closure.clone(),
                            pct,
                        ));
                        std::ops::ControlFlow::Continue(())
                    }
                    Err(e) => {
                        let _ = tx.send(crate::MapDownloadEvent::Error(e.to_string()));
                        std::ops::ControlFlow::Break(())
                    }
                },
            );
        }
    }
}
