//! What tapping your own building shows, drawn in the world under it.
//! Icons and numbers only, so it reads the same in every language.

use sow_core::game::BuildingKind;
use sow_render::text::NameplateStatusSprite as Icon;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BuildingMetric {
    pub icon: Icon,
    /// `None` marks an unlocked ability shown by its icon alone.
    pub value: Option<f64>,
    pub prefix: &'static str,
    pub unit: &'static str,
}

impl BuildingMetric {
    const fn new(icon: Icon, value: f64, prefix: &'static str, unit: &'static str) -> Self {
        Self {
            icon,
            value: Some(value),
            prefix,
            unit,
        }
    }

    pub(crate) const fn icon_name(&self) -> &'static str {
        match self.icon {
            Icon::Troops => "troops",
            Icon::Boat => "boat",
            Icon::Speed => "speed",
            Icon::Gold => "gold",
            Icon::Shield => "shield",
            Icon::Range => "range",
            Icon::Radiation => "radiation",
            _ => "upgrade-sparkle",
        }
    }

    pub(crate) fn label(&self) -> String {
        self.value.map_or_else(String::new, |value| {
            format!(
                "{}{}{}",
                self.prefix,
                crate::utils::format_number(value),
                self.unit
            )
        })
    }
}

/// Effects a building grants at `level`; the single source for the panel and its gains.
pub(crate) fn building_metrics(
    kind: BuildingKind,
    level: u8,
    leader: sow_core::player::Leader,
    config: &sow_core::game_config::GameConfig,
) -> Vec<BuildingMetric> {
    if level == 0 {
        return Vec::new();
    }
    use sow_core::geo_entities::{District, Resource, Source};
    let level_weight = sow_core::building::income_level_weight(level);
    let level = f64::from(level);
    let per_minute = config.global_speed_multiplier * 60.0;
    let hero = |district, resource| leader.resource_bonus_multiplier(Source::District(district), resource);
    match kind {
        BuildingKind::City => vec![
            BuildingMetric::new(
                Icon::Troops,
                config.city_max_troops * level * leader.city_troop_capacity_multiplier(),
                "+",
                "",
            ),
            BuildingMetric::new(
                Icon::Troops,
                config.city_troop_income
                    * level
                    * hero(District::City, Resource::Troops)
                    * per_minute,
                "+",
                "",
            ),
        ],
        BuildingKind::Port => vec![
            BuildingMetric::new(Icon::Boat, level, "+", ""),
            BuildingMetric::new(Icon::Speed, level, "+", "%"),
            BuildingMetric::new(
                Icon::Gold,
                config.port_gold_income
                    * level_weight
                    * hero(District::Port, Resource::Gold)
                    * per_minute,
                "+",
                "",
            ),
            // Merchant income with trade routes, at most.
            BuildingMetric::new(
                Icon::Gold,
                config.trade_ship_gold_income * level * per_minute,
                "+",
                "",
            ),
        ],
        BuildingKind::Factory => vec![BuildingMetric::new(
            Icon::Gold,
            config.factory_gold_income
                * level
                * hero(District::Factory, Resource::Gold)
                * per_minute,
            "+",
            "",
        )],
        BuildingKind::Tower => {
            let range = f64::from(sow_core::building::tower_range_for_level(
                level as u8,
                config,
                leader.tower_range_multiplier(),
            ));
            let mut stats = vec![
                BuildingMetric::new(
                    Icon::Shield,
                    level * 5.0 * leader.tower_loss_multiplier(sow_core::geo_entities::Side::Defender),
                    "+",
                    "%",
                ),
                BuildingMetric::new(Icon::Range, range, "", ""),
            ];
            if level as u8 >= BuildingKind::Tower.max_level() {
                stats.push(BuildingMetric {
                    icon: Icon::Radiation,
                    value: None,
                    prefix: "",
                    unit: "",
                });
            }
            stats
        }
        BuildingKind::Farm => vec![
            BuildingMetric::new(
                Icon::Troops,
                config.farm_troop_income
                    * level_weight
                    * hero(District::Farm, Resource::Troops)
                    * per_minute,
                "+",
                "",
            ),
            BuildingMetric::new(
                Icon::Gold,
                config.farm_gold_income
                    * level_weight
                    * hero(District::Farm, Resource::Gold)
                    * per_minute,
                "+",
                "",
            ),
        ],
    }
}

/// What the next level adds: only the effects that change, as "+" amounts.
pub(crate) fn upgrade_gains(
    current: &[BuildingMetric],
    next: &[BuildingMetric],
) -> Vec<BuildingMetric> {
    next.iter()
        .enumerate()
        .filter_map(|(index, metric)| match (current.get(index), metric.value) {
            (Some(before), Some(after)) if before.icon == metric.icon => {
                let gain = after - before.value.unwrap_or(0.0);
                (gain.abs() > 1e-6).then_some(BuildingMetric {
                    value: Some(gain),
                    prefix: "+",
                    ..*metric
                })
            }
            _ => Some(*metric),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UpgradeOffer {
    pub next_level: u8,
    pub cost: f64,
    pub seconds: f64,
    /// False when gold is short; the button still answers a tap with the reason.
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BuildingPanel {
    pub tile_idx: u32,
    pub kind: BuildingKind,
    pub chips: Vec<BuildingMetric>,
    pub upgrade: Option<UpgradeOffer>,
    /// Seconds left while the building is being built or upgraded.
    pub remaining_seconds: Option<f64>,
    /// Completion progress `0.0..=1.0` while the building is being built or upgraded.
    pub progress: Option<f32>,
}

/// Panel rows in physical px. Drawing and tapping read this one layout.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BuildingPanelLayout {
    pub chips_center: [f32; 2],
    pub chip_height: f32,
    pub button_center: [f32; 2],
    pub button_size: [f32; 2],
}

// ponytail: fixed bottom inset for the building dock; read the dock's real height if it grows.
const PANEL_BOTTOM_INSET: f32 = 96.0;

pub(crate) fn building_panel_layout(
    kind: BuildingKind,
    tile_idx: u32,
    map_w: u32,
    camera: [f32; 2],
    zoom: f32,
    sf: f32,
    screen_h: f32,
) -> BuildingPanelLayout {
    let map_w = map_w.max(1);
    let footprint =
        sow_core::building::BuildingFootprint::at(kind, tile_idx % map_w, tile_idx / map_w);
    let center = [
        camera[0] + (footprint.left as f32 + footprint.width as f32 * 0.5) * zoom,
        camera[1] + (footprint.top as f32 + footprint.height as f32 * 0.5) * zoom,
    ];
    let half = (footprint.height as f32 * 0.5 * zoom).max(24.0 * sf);
    let gap = 8.0 * sf;
    let chip_height = 28.0 * sf;
    let button_size = [212.0 * sf, 52.0 * sf];
    let chips_offset = half + gap + chip_height * 0.5;
    let button_offset = chips_offset + chip_height * 0.5 + gap + button_size[1] * 0.5;
    // No room above the dock: the panel opens above the building instead.
    let below =
        center[1] + button_offset + button_size[1] * 0.5 <= screen_h - PANEL_BOTTOM_INSET * sf;
    let side = if below { 1.0 } else { -1.0 };
    BuildingPanelLayout {
        chips_center: [center[0], center[1] + side * chips_offset],
        chip_height,
        button_center: [center[0], center[1] + side * button_offset],
        button_size,
    }
}

#[allow(dead_code)]
impl BuildingPanelLayout {
    pub(crate) fn button_contains(&self, x: f32, y: f32) -> bool {
        (x - self.button_center[0]).abs() <= self.button_size[0] * 0.5
            && (y - self.button_center[1]).abs() <= self.button_size[1] * 0.5
    }
}

impl crate::app::SowApp {
    /// The open panel for your own tapped building, from the live snapshot.
    pub(crate) fn building_panel(&self) -> Option<BuildingPanel> {
        let menu = self
            .input
            .map_context_menu
            .filter(|menu| menu.view == crate::app::MapContextMenuView::BuildingDetails)?;
        let snapshot = self.sim.current_snapshot.as_ref()?;
        let my_id = self.sim.my_player_id?;
        let building = snapshot
            .buildings
            .iter()
            .find(|building| building.tile_idx == menu.tile_idx && building.owner_id == my_id)?;
        let leader = snapshot
            .players
            .iter()
            .find(|player| player.id == my_id)
            .map(|player| player.leader)
            .unwrap_or_default();
        let config = &self.sim.config;
        let level = building.active_level();
        let current = building_metrics(building.kind, level, leader, config);
        let seconds = |ticks: u32| f64::from(ticks) * config.tick_rate_ms as f64 / 1000.0;
        if building.under_construction {
            let progress = crate::render::world::overlays::construction_progress(
                building.kind,
                level,
                building.level,
                building.ticks_until_complete,
            );
            return Some(BuildingPanel {
                tile_idx: building.tile_idx,
                kind: building.kind,
                chips: current,
                upgrade: None,
                remaining_seconds: Some(seconds(building.ticks_until_complete)),
                progress,
            });
        }
        let next_level = level.saturating_add(1);
        // Maxed or campaign-locked levels hide the button, as the radial hides them.
        if next_level > building.kind.max_level()
            || next_level > self.campaign_building_max_level(building.kind)
        {
            return Some(BuildingPanel {
                tile_idx: building.tile_idx,
                kind: building.kind,
                chips: current,
                upgrade: None,
                remaining_seconds: None,
                progress: None,
            });
        }
        let next = building_metrics(building.kind, next_level, leader, config);
        let cost = self
            .map_menu_cost(
                crate::input::map_click::MapMenuAction::UpgradeStructure,
                building.tile_idx,
            )
            .0
            .unwrap_or(f64::INFINITY);
        Some(BuildingPanel {
            tile_idx: building.tile_idx,
            kind: building.kind,
            chips: upgrade_gains(&current, &next),
            upgrade: Some(UpgradeOffer {
                next_level,
                cost,
                seconds: seconds(sow_core::building::structure_upgrade_duration_ticks(
                    building.kind,
                    next_level,
                )),
                enabled: cost.is_finite() && self.current_player_gold() >= cost,
            }),
            remaining_seconds: None,
            progress: None,
        })
    }

    /// Layout of the open panel at the current camera.
    pub(crate) fn building_panel_layout(&self) -> Option<(BuildingPanel, BuildingPanelLayout)> {
        let panel = self.building_panel()?;
        let layout = building_panel_layout(
            panel.kind,
            panel.tile_idx,
            self.sim.map_w,
            [self.input.camera_x, self.input.camera_y],
            self.input.camera_zoom,
            crate::web_canvas::device_pixel_ratio() as f32,
            self.input.screen_h,
        );
        Some((panel, layout))
    }

    /// True when (x, y) lands on the open panel's upgrade button.
    pub(crate) fn building_panel_button_at(&self, _x: f32, _y: f32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn metrics_keep_effect_values_compact_and_accurate() {
        let config = sow_core::game_config::GameConfig::default();
        let caesar = sow_core::player::Leader::of("caesar");
        let port = building_metrics(BuildingKind::Port, 2, caesar, &config);
        assert!(port.contains(&BuildingMetric::new(Icon::Boat, 2.0, "+", "")));
        assert!(port.contains(&BuildingMetric::new(Icon::Gold, 30.0, "+", "")));
        assert!(port.contains(&BuildingMetric::new(Icon::Gold, 24.0, "+", "")));
        let cleopatra = building_metrics(
            BuildingKind::Port,
            2,
            sow_core::player::Leader::of("cleopatra"),
            &config,
        );
        assert!(cleopatra.contains(&BuildingMetric::new(Icon::Gold, 45.0, "+", "")));
        let farm = building_metrics(BuildingKind::Farm, 2, caesar, &config);
        assert!(farm.contains(&BuildingMetric::new(Icon::Troops, 100.0, "+", "")));
        assert!(farm.contains(&BuildingMetric::new(Icon::Gold, 10.0, "+", "")));
        let tower = building_metrics(BuildingKind::Tower, 2, caesar, &config);
        assert!(tower.contains(&BuildingMetric::new(Icon::Shield, 10.0, "+", "%")));
        assert!(tower.contains(&BuildingMetric::new(Icon::Range, 8.0, "", "")));

        let gains = upgrade_gains(
            &building_metrics(BuildingKind::Tower, 1, caesar, &config),
            &tower,
        );
        assert!(gains.contains(&BuildingMetric::new(Icon::Shield, 5.0, "+", "%")));
        assert!(gains.contains(&BuildingMetric::new(Icon::Range, 2.0, "+", "")));
        let max = BuildingKind::Tower.max_level();
        let top = upgrade_gains(
            &building_metrics(BuildingKind::Tower, max - 1, caesar, &config),
            &building_metrics(BuildingKind::Tower, max, caesar, &config),
        );
        assert!(
            top.iter()
                .any(|metric| metric.icon == Icon::Radiation && metric.value.is_none()),
            "the max level shows the unlocked interception"
        );
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn panel_opens_below_unless_the_dock_is_in_the_way_and_taps_hit_what_is_drawn() {
        for sf in [1.0, 2.0, 3.0] {
            let zoom = 12.0 * sf;
            let camera = 300.0 * sf - 20.0 * zoom;
            let layout = building_panel_layout(
                BuildingKind::City,
                20 * 100 + 20,
                100,
                [camera, camera],
                zoom,
                sf,
                900.0 * sf,
            );
            let center = 300.0 * sf;
            assert!(
                layout.chips_center[1] > center && layout.button_center[1] > layout.chips_center[1]
            );
            assert!(layout.button_contains(layout.button_center[0], layout.button_center[1]));
            assert!(
                !layout.button_contains(center, center),
                "the building itself is not the button"
            );
            let low = building_panel_layout(
                BuildingKind::City,
                20 * 100 + 20,
                100,
                [camera, camera],
                zoom,
                sf,
                360.0 * sf,
            );
            assert!(low.button_center[1] < low.chips_center[1] && low.chips_center[1] < center);
        }
    }
}
