use crate::app::SowApp;

impl SowApp {
    pub(crate) fn handle_sim_shutdown(&mut self) {
        self.sim.engine = None;
        self.sim.current_snapshot = None;
        self.ui.mover_scene = crate::render::world::movers::MoverScene::new();
        self.ui.transport_target_markers.clear();
        self.ui.transport_target_seen.clear();
        self.ui.transport_target_snapshot_tick = None;
        self.ui.transport_impacts.clear();
    }
}
