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

/// Hero tribe ability: crushing a tribe shows the hero's ability icon.
fn tribe_ability_hero(
    me: Option<sow_core::player::Leader>,
    victim_type: sow_core::player::PlayerType,
) -> Option<sow_core::player::Leader> {
    me.filter(|leader| {
        victim_type == sow_core::player::PlayerType::Bot
            && leader.attack_ability_targets(sow_core::geo_entities::Target::Tribe)
    })
}

impl SowApp {
    pub(crate) fn record_combat_kill_feedback(
        &mut self,
        wx: f32,
        wy: f32,
        gender: Option<sow_data::geo_entities::Gender>,
        now_instant: web_time::Instant,
    ) {
        if let Some((step, title)) = self.ui.record_kill_callout(wx, wy, gender, now_instant) {
            self.sfx
                .play_cascade(now_instant, step, self.spatial_sound_params(wx, wy));
            let streak = self.ui.combat_streaks.kill_streak;
            if streak > self.progress.best_match_conquest_streak {
                self.progress.best_match_conquest_streak = streak;
            }
            if title == "RAMPAGE!" {
                self.progress.rampage_count = self.progress.rampage_count.saturating_add(1);
            }
            crate::store_portals::track_product_event_with(
                "conquest_streak_milestone",
                &serde_json::json!({
                    "title": title,
                    "streak": streak,
                    "multi": self.ui.combat_streaks.multi_kill_count,
                }),
            );
        }
    }

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
        let reward_data =
            crate::app::local_gold_reward(my_id, conqueror_id, info.gold_bounty, info.assists);
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

        if !tile_found && let Some(conqueror) = snap.players.iter().find(|p| p.id == conqueror_id) {
            wx = conqueror.centroid_x + 0.5;
            wy = conqueror.centroid_y + 0.5;
        }
        let seed = (player_id as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(elimination_x.wrapping_mul(1597334977))
            .wrapping_add(elimination_y.wrapping_mul(3512401961));

        let [anim_x, anim_y] = self.ui.nameplates.anchor_for(player_id).unwrap_or([wx, wy]);
        let color = victim
            .map(crate::render::world::nameplates::player_color)
            .unwrap_or([0.85, 0.85, 0.85, 1.0]);
        if crate::theme::dev_config::DevConfig::with(|dev| dev.vfx_blasts) {
            let map_w = self.sim.map_w.max(1);
            let map_h = self.sim.map_h.max(1);
            let tx = (anim_x.floor().max(0.0) as u32).min(map_w.saturating_sub(1));
            let ty = (anim_y.floor().max(0.0) as u32).min(map_h.saturating_sub(1));
            let tile_idx = ty.saturating_mul(map_w).saturating_add(tx);
            let death_mode = if info.by_nuke {
                2.0
            } else if victim_type != sow_core::player::PlayerType::Bot
                || victim.is_some_and(|p| p.is_campaign_faction)
            {
                1.0
            } else {
                0.0
            };
            let vis_owner = if show_animation { my_id } else { conqueror_id };
            self.ui.blast_scene.spawn_entity_death(
                anim_x,
                anim_y,
                tile_idx,
                vis_owner,
                [color[0], color[1], color[2]],
                death_mode,
                now_instant,
            );
        }

        if show_animation {
            let name = victim
                .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                .unwrap_or_else(|| format!("Player {player_id}"));
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
                    casualties: None,
                    reward,
                    prepared_name: sow_render::text::PreparedText::default(),
                    prepared_casualties: None,
                    prepared_reward: None,
                    text_style_key: None,
                },
            );
        }

        if my_id != 0 && (player_id == my_id || conqueror_id == my_id || reward_data.is_some()) {
            self.sfx.queue_elimination(
                crate::player_sound_type(victim_type),
                seed,
                self.spatial_sound_params(wx, wy),
            );
            if conqueror_id == my_id {
                let leader_gender = snap
                    .players
                    .iter()
                    .find(|p| p.id == my_id)
                    .and_then(|me| me.leader.character().gender);
                self.record_combat_kill_feedback(wx, wy, leader_gender, now_instant);
            }
            if player_id == my_id {
                self.ui.reset_kill_streaks();
            }
        }

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

        let tribe_ability_hero = tribe_ability_hero(
            snap.players
                .iter()
                .find(|p| p.id == my_id)
                .map(|me| me.leader),
            victim_type,
        );
        if conqueror_id == my_id
            && my_id != 0
            && (info.gold_bounty == 0 || tribe_ability_hero.is_some())
        {
            let name = victim
                .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                .unwrap_or_else(|| format!("Player {player_id}"));
            let mut text = crate::ui::UiText::new("hud.elimination").with("victim", name);
            if let Some(hero) = tribe_ability_hero {
                text = text.with("hero_art", hero.art_slug());
            }
            self.ui.app.hud_state.push_notification_for_players(
                text,
                [Some(my_id), Some(player_id)],
                3,
                Some(format!("elimination:{player_id}")),
                false,
            );
        }
    }
}
