use crate::app::SowApp;

pub(crate) struct CaptureEventInfo {
    pub new_owner: u16,
    pub previous_owner: u16,
}

impl SowApp {
    pub(crate) fn handle_tile_captured(
        &mut self,
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
