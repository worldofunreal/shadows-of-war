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
        let victim_type = victim
            .map(|p| p.player_type)
            .unwrap_or(sow_core::player::PlayerType::Bot);
        let reward_data = crate::app::local_gold_reward(
            my_id,
            conqueror_id,
            info.gold_bounty,
            info.assists,
        );
        let show_animation = crate::app::elimination_visible_to_viewer(
            &snap.players,
            my_id,
            player_id,
            conqueror_id,
            reward_data.is_some(),
        );
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

        if !tile_found
            && let Some(conqueror) = snap.players.iter().find(|p| p.id == conqueror_id)
        {
            wx = conqueror.centroid_x + 0.5;
            wy = conqueror.centroid_y + 0.5;
        }
        let seed = (player_id as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(elimination_x.wrapping_mul(1597334977))
            .wrapping_add(elimination_y.wrapping_mul(3512401961));

        if show_animation {
            let name = victim
                .map(|p| sow_core::player::nameplate_display_name(p.id, &p.name, p.player_type))
                .unwrap_or_else(|| format!("Player {player_id}"));
            let [anim_x, anim_y] = self
                .ui
                .nameplates
                .anchor_for(player_id)
                .unwrap_or([wx, wy]);
            let color = victim
                .map(crate::render::world::nameplates::player_color)
                .unwrap_or([0.85, 0.85, 0.85, 1.0]);
            let avatar = victim
                .map(sow_core::player::avatar_identity)
                .unwrap_or(sow_core::player::AvatarIdentity::Fallback);
            let reward = reward_data.map(|(gold, is_assist)| crate::app::DeathNameplateReward {
                amount: format!("+{}", crate::utils::format_number(gold as f64)),
                is_assist,
            });
            crate::app::DeathNameplateAnimation::enqueue(
                &mut self.ui.death_nameplates,
                crate::app::DeathNameplateAnimation {
                    world_x: anim_x,
                    world_y: anim_y,
                    curve_direction: if player_id % 2 == 0 { 1.0 } else { -1.0 },
                    start_time: now_instant,
                    by_nuke: info.by_nuke,
                    name,
                    color,
                    avatar,
                    reward,
                    prepared_name: sow_render::text::PreparedText::default(),
                    prepared_reward: None,
                    text_style_key: None,
                },
            );
        }

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

        if conqueror_id == my_id && my_id != 0 && info.gold_bounty == 0 {
            let name = victim
                .map(|p| sow_core::player::nameplate_display_name(p.id, &p.name, p.player_type))
                .unwrap_or_else(|| format!("Player {player_id}"));
            self.ui.app.hud_state.push_notification_for_players(
                crate::ui::UiText::new("hud.elimination").with("victim", name),
                [Some(my_id), Some(player_id)],
                3,
                Some(format!("elimination:{player_id}")),
                false,
            );
        }
    }
}
