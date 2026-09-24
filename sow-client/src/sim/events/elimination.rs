use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

pub(crate) struct EliminationEventInfo<'a> {
    pub player_id: u16,
    pub conqueror_id: u16,
    pub pos: (u32, u32),
    pub gold_bounty: u32,
    pub assists: &'a [(u16, u32)],
    pub by_nuke: bool,
}

impl SowApp {
    pub(crate) fn handle_player_eliminated(
        &mut self,
        snap: &SimSnapshot,
        my_id: u16,
        now_instant: web_time::Instant,
        turn_defeats: &mut crate::player_progress::SessionDefeats,
        info: &EliminationEventInfo<'_>,
    ) {
        let player_id = info.player_id;
        let conqueror_id = info.conqueror_id;
        let victim = snap.players.iter().find(|p| p.id == player_id);
        let (elimination_x, elimination_y) = info.pos;
        let mut wx = 0.5;
        let mut wy = 0.5;

        let mut tile_found = false;
        if elimination_x > 0 || elimination_y > 0 {
            wx = elimination_x as f32 + 0.5;
            wy = elimination_y as f32 + 0.5;
            tile_found = true;
        }

        if let Some(target) = victim
            && !tile_found
            && (target.centroid_x > 0.001 || target.centroid_y > 0.001)
        {
            wx = target.centroid_x + 0.5;
            wy = target.centroid_y + 0.5;
            tile_found = true;
        }

        if !tile_found {
            // Fallback: Use conqueror's position as the visual reward point,
            // since the conqueror just claimed the target's last tile.
            if let Some(conqueror) = snap.players.iter().find(|p| p.id == conqueror_id) {
                wx = conqueror.centroid_x + 0.5;
                wy = conqueror.centroid_y + 0.5;
            }
        }

        let victim_type = victim
            .map(|p| p.player_type)
            .unwrap_or(sow_core::player::PlayerType::Bot);
        let name = victim
            .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
            .unwrap_or_else(|| format!("Player {player_id}"));
        let color = victim.map_or([1.0; 3], |p| {
            readable_death_color(
                p.team
                    .map_or(p.color, sow_core::player::team_territory_rgb),
            )
        });

        let seed = (player_id as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(elimination_x.wrapping_mul(1597334977))
            .wrapping_add(elimination_y.wrapping_mul(3512401961));

        let drift_x = (((seed & 0xff) as f32 / 255.0) - 0.5) * 8.0;
        let flight_distance = 15.0 + ((seed >> 8 & 0xff) as f32 / 255.0) * 15.0;
        let icon_scale = 0.8 + ((seed >> 16 & 0xff) as f32 / 255.0) * 0.6;
        crate::app::DeathNameplateAnimation::enqueue(
            &mut self.ui.death_nameplates,
            crate::app::DeathNameplateAnimation {
                name,
                color,
                world_x: wx,
                world_y: wy,
                start_time: now_instant,
                by_nuke: info.by_nuke,
                drift_x,
                flight_distance,
                icon_scale,
            },
        );

        self.sfx.queue_elimination(
            crate::player_sound_type(victim_type),
            seed,
            self.spatial_sound_params(wx, wy),
        );

        if conqueror_id == my_id
            && my_id != 0
            && let Some(victim) = victim
        {
            use sow_core::player::PlayerType;
            match victim.player_type {
                PlayerType::Human => turn_defeats.players += 1,
                PlayerType::Nation => turn_defeats.empires += 1,
                PlayerType::Bot => turn_defeats.tribes += 1,
            }
        }

        if conqueror_id == my_id && my_id != 0 && info.gold_bounty > 0 {
            self.ui.floating_notices.push(crate::app::FloatingNotice {
                text: format!(
                    "+{} Gold",
                    crate::utils::format_number(info.gold_bounty as f64)
                ),
                world_x: wx,
                world_y: wy,
                start_time: now_instant,
                duration: web_time::Duration::from_millis(3000),
                color: crate::rgb(250, 204, 21),
            });
        }
        for (assist_id, assist_gold) in info.assists {
            if *assist_id == my_id && my_id != 0 && *assist_gold > 0 {
                self.ui.floating_notices.push(crate::app::FloatingNotice {
                    text: format!(
                        "+{} Gold (Assist)",
                        crate::utils::format_number(*assist_gold as f64)
                    ),
                    world_x: wx,
                    world_y: wy + 0.5,
                    start_time: now_instant,
                    duration: web_time::Duration::from_millis(3000),
                    color: crate::rgb(180, 220, 100),
                });
            }
        }
    }
}

fn readable_death_color(rgb: [f32; 3]) -> [f32; 3] {
    let luminance = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let factor = if luminance < 0.60 {
        (0.60 - luminance) / (1.0 - luminance).max(0.001)
    } else {
        0.0
    };
    [
        rgb[0] + (1.0 - rgb[0]) * factor,
        rgb[1] + (1.0 - rgb[1]) * factor,
        rgb[2] + (1.0 - rgb[2]) * factor,
    ]
}
