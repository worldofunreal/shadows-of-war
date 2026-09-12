use super::super::*;
use super::layout::{NameplateMetrics, nameplate_font_px};
use super::painter::{NameplateInput, NameplatePainter, NameplateStyle};

pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    gfx: &mut crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("world_nameplates"),
    ));
    let painter = &painter;
    let sf = ctx.sf;
    let dot_r = ctx.dot_r;
    let wall_secs = ctx.wall_secs;
    let visible_players = ctx.visible_players;

    if visible_players.is_empty() {
        return;
    }

    let Some(snap) = &sim.current_snapshot else {
        return;
    };

    // visible_players is pre-sorted in mod.rs (local player last, then humans, nations, presence)
    let mut full_labels_drawn = 0;

    let visual_config = ClientVisualConfig::default();
    let ui_text_scale = visual_config.ui_text_scale;
    let zoom_scaled_local = input.camera_zoom / sf;

    // Frame-constant trig — computed once, reused by every player
    let heart_flash_alpha = ((wall_secs * 12.0).cos() * 0.5 + 0.5) as f32;

    // Hoist my_player lookup — avoids O(n) scan per player
    let my_id = sim.my_player_id.unwrap_or(0);
    let my_player = snap.players.iter().find(|p| p.id == my_id);

    // World-rank badges (crown/medals): id -> 1-based leaderboard position
    let rank_of: std::collections::HashMap<u16, usize> = ui
        .leaderboard_rankings
        .iter()
        .enumerate()
        .map(|(i, r)| (r.id, i + 1))
        .collect();

    let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
    let show_bot_avatars = dev.vfx_bot_avatars;
    let show_names = dev.vfx_nameplate_names;
    let show_troops = dev.vfx_nameplate_troops;
    let nameplate_style = NameplateStyle::from_dev(&dev, sf);
    let mut nameplate_painter =
        NameplatePainter::new(painter, gfx.text_renderer.as_mut(), sf, nameplate_style);

    for vp in visible_players {
        let player = vp.player;
        let center = vp.center;
        let pc = vp.pc;

        let is_me = player.id == my_id;
        let is_human = player.player_type == sow_core::player::PlayerType::Human;

        // Tutorial only — TEMPORARY player-nameplate simplification. In the tutorial the local
        // player's nameplate is JUST the avatar, centered on the territory anchor so it nests inside
        // the centered tutorial pointer ring: no star / name / troops / status. Gated on
        // `tutorial_active`, which is false in every normal solo/MP match (tutorial isolation
        // contract + single-derive gate), so the standard gameplay nameplate path below is never
        // touched. To retire this, delete the whole block — never weaken the gate.
        if ui.tutorial_active && is_me && ui.tutorial_step_idx == 0 {
            nameplate_painter.paint_tutorial_avatar(
                center,
                20.0,
                player.id,
                &player.name,
                player.player_type,
                player.color,
                player.leader,
                &ui.app.asset_loader,
            );
            continue;
        }

        // Far-zoom declutter: below nameplate_hide_zoom every non-human plate is a disc.
        // Above it, plates big enough to read (see show_full below) draw in full.
        if zoom_scaled_local < visual_config.nameplate_hide_zoom && !is_me && !is_human {
            nameplate_painter.paint_lod_dot(center, dot_r, pc);
            continue;
        }

        let scaled_size = nameplate_font_px(
            vp.nameplate_size,
            zoom_scaled_local,
            is_human,
            &visual_config,
        ) * ui_text_scale;

        // A non-human plate whose NATURAL size is sub-readable dots out instead of being
        // floored up to 7px: flooring turns hundreds of tiny tribes/nations into full plates
        // (two egui text layouts each, every frame), which wasm cannot absorb — this was the
        // web-only slowdown. The budget caps the worst case for ALL non-humans, not just bots;
        // plates are sorted big-first, so the budget goes to the most visible ones.
        let show_full = if is_human {
            true
        } else {
            scaled_size >= 7.0 && full_labels_drawn < 80
        };

        if show_full {
            if !is_human {
                full_labels_drawn += 1;
            }

            let metrics =
                NameplateMetrics::compute(scaled_size, player.player_type, show_bot_avatars);
            // Check alliance status with the player
            let mut is_allied = false;
            let mut is_heart_flashing = false;
            let mut has_req = false;
            if my_id != player.id
                && let Some(me) = my_player
            {
                if me.alliances.contains(&player.id) {
                    is_allied = true;
                    let timer = me.alliance_timers.get(&player.id).copied().unwrap_or(2400);
                    let has_pending_proposal = me.alliance_requests.contains(&player.id)
                        || player.alliance_requests.contains(&my_id);
                    if timer <= 300 && !has_pending_proposal {
                        is_heart_flashing = true;
                    }
                } else if me.alliance_requests.contains(&player.id) {
                    has_req = true;
                }
            }

            let is_disconnected = player.disconnected;
            let betrayal_flash = !is_disconnected && player.traitor;

            let display_name = if player.player_type == sow_core::player::PlayerType::Bot {
                if player.name.is_empty() {
                    format!("Tribe {}", player.id.saturating_sub(199))
                } else {
                    player.name.clone()
                }
            } else {
                sow_core::player::display_name(player.id, &player.name, player.player_type)
            };
            let troops_str = sow_ui_kit::utils::format_number(player.troops);
            let vibrant_color = crate::hud::nameplate::ensure_readable_nameplate_color(
                player
                    .team
                    .map_or(player.color, sow_core::player::team_territory_rgb),
            );
            nameplate_painter.paint(NameplateInput {
                center,
                metrics,
                player_id: player.id,
                player_name: &player.name,
                player_type: player.player_type,
                player_color: player.color,
                leader: &player.leader,
                active_emoji: player.active_emoji.as_deref(),
                display_name: &display_name,
                troops: &troops_str,
                vibrant_color,
                is_me,
                is_allied,
                is_heart_flashing,
                has_req,
                betrayal_flash,
                is_disconnected,
                heart_flash_alpha,
                rank_1based: rank_of.get(&player.id).copied(),
                show_names,
                show_troops,
                asset_loader: &ui.app.asset_loader,
            });
            continue;
        } else {
            nameplate_painter.paint_lod_dot(center, dot_r, pc);
        }
    }
}
