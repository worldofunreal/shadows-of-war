use crate::app::SowApp;
use crate::app::audio::CombatSoundKind;
use sow_core::protocol::SimSnapshot;

pub(crate) struct CaptureEventInfo {
    pub x: u32,
    pub y: u32,
    pub new_owner: u16,
    pub previous_owner: u16,
    pub troops: f64,
}

impl SowApp {
    pub(crate) fn handle_tile_captured(
        &mut self,
        snap: &SimSnapshot,
        my_id: u16,
        now: web_time::Instant,
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

        let kind = if info.new_owner == my_id {
            if info.previous_owner == 0 {
                CombatSoundKind::Wilderness
            } else {
                match snap
                    .players
                    .iter()
                    .find(|p| p.id == info.previous_owner)
                    .map(|p| p.player_type)
                {
                    Some(sow_core::player::PlayerType::Human) => CombatSoundKind::Human,
                    Some(sow_core::player::PlayerType::Nation) => CombatSoundKind::Nation,
                    _ => CombatSoundKind::Tribe,
                }
            }
        } else {
            CombatSoundKind::Counter
        };

        let seed = (info.x.wrapping_mul(73856093))
            ^ (info.y.wrapping_mul(19349663))
            ^ u32::from(info.previous_owner)
            ^ (u32::from(info.new_owner) << 16);
        let spatial = self.spatial_sound_params(info.x as f32 + 0.5, info.y as f32 + 0.5);
        self.sfx
            .play_land_combat(now, kind, info.troops as f32, seed, spatial);
    }
}
