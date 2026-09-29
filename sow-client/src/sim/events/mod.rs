mod combat;
mod elimination;

use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;
use std::collections::BTreeMap;

impl SowApp {
    pub(crate) fn process_tick_events(
        &mut self,
        events: Vec<sow_core::game::GameEvent>,
        snap: &SimSnapshot,
        my_id: u16,
    ) -> crate::player_progress::SessionDefeats {
        let mut turn_defeats = crate::player_progress::SessionDefeats::default();
        let mut played_combat_this_tick = false;
        let mut wilderness_tiles = 0_u32;
        let mut enemy_tiles = BTreeMap::<u16, u32>::new();
        let now_instant = web_time::Instant::now();
        self.sfx.sync_server_phase(&snap.phase);
        self.sfx.begin_tick();
        for event in events {
            match event {
                sow_core::game::GameEvent::TransportShipLanded { owner_id, x, y } => {
                    let color = snap
                        .players
                        .iter()
                        .find(|player| player.id == owner_id)
                        .map(|player| {
                            player
                                .team
                                .map_or(player.color, sow_core::player::team_territory_rgb)
                        })
                        .unwrap_or([0.13, 0.83, 0.94]);
                    if self.ui.transport_impacts.len() == crate::app::MAX_TRANSPORT_IMPACTS {
                        self.ui.transport_impacts.pop_front();
                    }
                    self.ui
                        .transport_impacts
                        .push_back(crate::app::TransportImpact {
                            tile_x: x,
                            tile_y: y,
                            color,
                            start_time: now_instant,
                        });
                    if my_id != 0 && owner_id == my_id {
                        self.ui.app.hud_state.push_notification_for_players(
                            crate::ui::UiText::new("hud.transport_landed"),
                            [Some(my_id), None],
                            2,
                            Some("transport-landed".into()),
                            false,
                        );
                    }
                }
                sow_core::game::GameEvent::PlayerEliminated {
                    player_id,
                    conqueror_id,
                    gold_bounty,
                    elimination_x,
                    elimination_y,
                    assists,
                    by_nuke,
                } => {
                    self.handle_player_eliminated(
                        snap,
                        my_id,
                        now_instant,
                        &mut turn_defeats,
                        &elimination::EliminationEventInfo {
                            player_id,
                            conqueror_id,
                            pos: (elimination_x, elimination_y),
                            gold_bounty,
                            assists: &assists,
                            by_nuke,
                        },
                    );
                    if conqueror_id == my_id && my_id != 0 {
                        self.ui
                            .trigger_viewport_alert(crate::app::ViewportAlertKind::ConquerPlayer);
                    }
                }
                sow_core::game::GameEvent::TileCaptured {
                    x,
                    y,
                    new_owner,
                    previous_owner,
                    troops,
                } => {
                    if new_owner == my_id && my_id != 0 {
                        if previous_owner == 0 {
                            wilderness_tiles = wilderness_tiles.saturating_add(1);
                        } else {
                            let count = enemy_tiles.entry(previous_owner).or_default();
                            *count = count.saturating_add(1);
                        }
                    }
                    self.handle_tile_captured(
                        snap,
                        my_id,
                        &mut played_combat_this_tick,
                        &combat::CaptureEventInfo {
                            pos: (x, y),
                            new_owner,
                            previous_owner,
                            troops,
                        },
                    );
                }
                sow_core::game::GameEvent::StructureSpawned { owner_id, .. }
                    if my_id != 0 && owner_id == my_id =>
                {
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.structure_started"),
                        [Some(my_id), None],
                        2,
                        None,
                        false,
                    );
                }
                sow_core::game::GameEvent::StructureReady { id, .. }
                    if my_id != 0 && snap.buildings.iter().any(|building| building.id == id && building.owner_id == my_id) =>
                {
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.structure_ready"),
                        [Some(my_id), None],
                        2,
                        None,
                        false,
                    );
                }
                sow_core::game::GameEvent::StructureUpgraded { id, .. }
                    if my_id != 0 && snap.buildings.iter().any(|building| building.id == id && building.owner_id == my_id) =>
                {
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.structure_upgraded"),
                        [Some(my_id), None],
                        2,
                        None,
                        false,
                    );
                }
                sow_core::game::GameEvent::TileUpgraded { tile_idx, .. }
                    if my_id != 0 && snap.dirty_tiles.iter().any(|tile| tile.index == tile_idx && tile.new_owner == my_id) =>
                {
                    self.ui.app.hud_state.push_notification_for_players(
                        crate::ui::UiText::new("hud.tile_upgraded"),
                        [Some(my_id), None],
                        2,
                        None,
                        false,
                    );
                }
                _ => {}
            }
        }
        if wilderness_tiles > 0 {
            self.ui.app.hud_state.push_notification_for_players(
                crate::ui::UiText::new("hud.wilderness_expanded")
                    .with("count", wilderness_tiles.to_string()),
                [Some(my_id), None],
                2,
                Some("wilderness-expanded".into()),
                true,
            );
        }
        for (enemy_id, count) in enemy_tiles {
            self.ui.app.hud_state.push_notification_for_players(
                crate::ui::UiText::new("hud.enemy_territory_captured")
                    .with("count", count.to_string()),
                [Some(my_id), Some(enemy_id)],
                2,
                Some(format!("enemy-territory:{enemy_id}")),
                true,
            );
        }
        self.sfx.flush_tick(now_instant);
        turn_defeats
    }
}
