use crate::app::SowApp;

pub(crate) struct SimInitOpts {
    pub config: Box<sow_core::game_config::GameConfig>,
    pub seed: u64,
    pub map_bytes: Vec<u8>,
    pub players: Vec<sow_core::protocol::PlayerInfo>,
    pub map_spawns: Vec<sow_core::map_file::MapSpawn>,
    pub geo_bounds: Option<sow_core::map_file::GeoBounds>,
    pub map_rosters: Vec<sow_core::map_file::MapRosterPreset>,
    pub num_land_tiles: u32,
}

impl SowApp {
    pub(crate) fn handle_sim_init(&mut self, opts: SimInitOpts) {
        let config = opts.config;
        let seed = opts.seed;
        let map_bytes = opts.map_bytes;
        let players = opts.players;
        let map_spawns = opts.map_spawns;
        let geo_bounds = opts.geo_bounds;
        let map_rosters = opts.map_rosters;
        let num_land_tiles = opts.num_land_tiles;
        self.exit_reward_preview = None;
        self.reset_progress_session();
        self.sim.config = (*config).clone();
        let map_w = config.map_width;
        let map_h = config.map_height;
        let mut new_engine = sow_core::engine::initialize_match_engine(
            (*config).clone(),
            seed,
            &map_bytes,
            players,
            map_spawns,
            geo_bounds,
            map_rosters,
            num_land_tiles,
        );
        if self.net.is_offline && new_engine.state.config.tutorial {
            self.sim.tutorial_observation.reset();
            self.sim
                .tutorial_observation
                .observe_sim(&new_engine, self.sim.my_player_id.unwrap_or(0));
        }
        let snap = new_engine.build_snapshot();
        let phase = snap.phase.clone();
        self.sim.current_snapshot = Some(snap);
        self.sim.latest_local_spawn = None;
        self.input.spawn_intent_rate.reset();
        self.sim.engine = Some(new_engine);
        self.time
            .interp
            .set_tick_dur_ms(self.sim.config.tick_rate_ms);
        self.time.interp.stamp_applied(web_time::Instant::now());
        self.sim.offline_tick_timer = 0.0;
        self.sim.offline_last_update = web_time::Instant::now();
        self.ui.mover_scene = crate::render::world::movers::MoverScene::new();

        let spawn_focus = (
            config
                .player_spawn
                .map_or(map_w as f32 * 0.5, |(x, _)| x as f32 + 0.5),
            config
                .player_spawn
                .map_or(map_h as f32 * 0.5, |(_, y)| y as f32 + 0.5),
        );
        let tutorial_frame = crate::campaign::tutorial_camera_frame(
            self.ui.tutorial_campaign,
            &config,
            self.input.screen_w,
            self.input.screen_h,
        );
        let focus = tutorial_frame.map_or(spawn_focus, |frame| frame.0);
        self.input.camera_zoom = 0.5;
        self.input.target_zoom =
            tutorial_frame.map_or(if config.tutorial { 8.0 } else { 0.5 }, |frame| frame.1);
        self.input.camera_x = self.input.screen_w * 0.5 - focus.0 * self.input.camera_zoom;
        self.input.camera_y = self.input.screen_h * 0.5 - focus.1 * self.input.camera_zoom;
        self.sim.map_w = map_w;
        self.sim.map_h = map_h;
        self.clamp_camera_to_map();
        self.input.has_snapped_camera_to_spawn = false;
        self.input.tutorial_camera_focus = config.tutorial;
        self.input.tutorial_zoom_in_events = 0;
        self.input.tutorial_zoom_out_events = 0;
        self.input.camera_focus_target = config.tutorial.then_some(focus);
        self.input.camera_focus_waiting_for_input_release = false;
        if config.tutorial {
            self.input.camera_x = self.input.screen_w * 0.5 - focus.0 * self.input.camera_zoom;
            self.input.camera_y = self.input.screen_h * 0.5 - focus.1 * self.input.camera_zoom;
            self.clamp_camera_to_map();
        }
        self.ui.is_spectating = false;
        self.ui.observing = false;
        self.ui.endgame_cache = None;
        self.sfx.reset_for_match();
        self.sfx.sync_server_phase(&phase);
    }
}
