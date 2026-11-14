use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

pub(crate) struct CaptureEventInfo {
    pub pos: (u32, u32),
    pub new_owner: u16,
    pub previous_owner: u16,
    pub troops: f64,
}

impl SowApp {
    pub(crate) fn handle_tile_captured(
        &mut self,
        _snap: &SimSnapshot,
        my_id: u16,
        played_combat_this_tick: &mut bool,
        info: &CaptureEventInfo,
    ) {
        if *played_combat_this_tick || my_id == 0 {
            return;
        }
        if info.new_owner != my_id && info.previous_owner != my_id {
            return;
        }
        *played_combat_this_tick = true;
    }
}
