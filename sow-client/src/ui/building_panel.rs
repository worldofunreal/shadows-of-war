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
            Icon::Clock => "clock",
            _ => "upgrade-sparkle",
        }
    }

    pub(crate) fn label(&self) -> String {
        self.value.map_or_else(String::new, |value| {
            let number = if (self.icon == Icon::Gold && self.unit == "/s")
                || (self.icon == Icon::Shield && self.unit.is_empty() && value.fract() != 0.0)
            {
                format!("{value:.2}")
            } else if self.icon == Icon::Speed
                && self.unit == "%"
                && (value - value.round()).abs() > 1e-3
            {
                format!("{value:.1}")
            } else {
                crate::utils::format_number(value)
            };
            format!("{}{}{}", self.prefix, number, self.unit)
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
    let level_weight = sow_core::building::city_income_level_weight(level);
    let city_gold_level_weight = sow_core::building::city_gold_level_weight(level);
    let capacity_level_weight = sow_core::building::city_capacity_level_weight(level);
    let district_capacity = config.district_max_troops
        * sow_core::building::district_capacity_level_weight(level);
    let level = f64::from(level);
    let per_second = config.global_speed_multiplier;
    let hero = |district, resource| leader.resource_bonus_multiplier(Source::District(district), resource);
    let hero_bonus = |district, resource| {
        leader.resource_bonus_hero_bonus(Source::District(district), resource)
    };
    let mut metrics = match kind {
        BuildingKind::City => {
            let mut metrics = vec![
                BuildingMetric::new(
                    Icon::Troops,
                    config.city_max_troops
                        * capacity_level_weight
                        * leader.city_troop_capacity_multiplier(),
                    "+",
                    " cap",
                ),
                BuildingMetric::new(
                    Icon::Troops,
                    config.city_troop_income
                        * level_weight
                        * hero(District::City, Resource::Troops)
                        * per_second,
                    "+",
                    "/s",
                ),
            ];
            metrics.push(BuildingMetric::new(
                Icon::Gold,
                (config.city_gold_income * hero(District::City, Resource::Gold)
                    + config.hero_city_gold_income
                        * hero_bonus(District::City, Resource::Gold).unwrap_or(0.0))
                    * city_gold_level_weight * per_second,
                "+",
                "/s",
            ));
            metrics
        }
        BuildingKind::Port => {
            let weight = sow_core::building::port_income_level_weight(level as u8);
            let metrics = vec![
                BuildingMetric::new(
                    Icon::Boat,
                    f64::from(sow_core::building::port_fleet_slots(level as u8)),
                    "+",
                    "",
                ),
                BuildingMetric::new(
                    Icon::Boat,
                    f64::from(sow_core::building::port_dock_capacity(level as u8)),
                    "+",
                    " docks",
                ),
                BuildingMetric::new(
                    Icon::Shield,
                    sow_core::building::port_repair_per_second(level as u8),
                    "+",
                    " heal/s",
                ),
                BuildingMetric::new(Icon::Speed, level, "+", "%"),
                BuildingMetric::new(
                    Icon::Gold,
                    config.port_gold_income
                        * weight
                        * hero(District::Port, Resource::Gold)
                        * per_second,
                    "+",
                    "/s",
                ),
                BuildingMetric::new(
                    Icon::Troops,
                    (config.port_troop_income * weight * hero(District::Port, Resource::Troops)
                        + config.hero_port_troop_income * level.min(4.0)
                            * hero_bonus(District::Port, Resource::Troops).unwrap_or(0.0))
                        * per_second,
                    "+",
                    "/s",
                ),
            ];
            metrics
        }
        BuildingKind::TradingPost => {
            let mut metrics = vec![BuildingMetric::new(
                Icon::Gold,
                config.trading_post_gold_income
                    * sow_core::building::trading_post_gold_level_weight(level as u8)
                    * hero(District::TradingPost, Resource::Gold)
                    * per_second,
                "+",
                "/s",
            )];
            if let Some(bonus) = hero_bonus(District::TradingPost, Resource::Troops) {
                metrics.push(BuildingMetric::new(
                    Icon::Troops,
                    config.hero_trading_post_troop_income * level * bonus * per_second,
                    "+",
                    "/s",
                ));
            }
            metrics
        }
        BuildingKind::Tower => {
            let range = f64::from(sow_core::building::tower_range_for_level(
                level as u8,
                config,
                leader.tower_range_multiplier(),
            ));
            let defense_weight = sow_core::building::tower_defense_weight(level as u8, config);
            let stats = vec![
                BuildingMetric::new(
                    Icon::Shield,
                    (defense_weight * config.tower_strength).clamp(0.0, 0.50)
                        * 100.0
                        * leader.tower_loss_multiplier(sow_core::geo_entities::Side::Defender),
                    "+",
                    "%",
                ),
                BuildingMetric::new(
                    Icon::Shield,
                    sow_core::building::tower_capture_weight(level as u8),
                    "×",
                    "",
                ),
                BuildingMetric::new(Icon::Range, range, "", ""),
            ];
            stats
        }
        BuildingKind::MilitaryCamp => vec![
            BuildingMetric::new(
                Icon::Troops,
                config.military_camp_troop_income
                    * sow_core::building::military_camp_troop_level_weight(level as u8)
                    * hero(District::MilitaryCamp, Resource::Troops)
                    * per_second,
                "+",
                "/s",
            ),
            BuildingMetric::new(
                Icon::Gold,
                config.military_camp_gold_income
                    * sow_core::building::military_camp_gold_level_weight(level as u8)
                    * hero(District::MilitaryCamp, Resource::Gold)
                    * per_second,
                "+",
                "/s",
            ),
            BuildingMetric::new(
                Icon::Speed,
                (sow_core::building::camp_conquest_multiplier(
                    sow_core::building::camp_capture_level_weight(level as u8)
                        * config.camp_capture_base_points.max(0.0),
                ) - 1.0) * 100.0,
                "+",
                "%",
            ),
        ],
        BuildingKind::MissileSilo => vec![BuildingMetric::new(Icon::Shield, level, "×", "")],
        BuildingKind::SAM => {
            let mut stats = vec![BuildingMetric::new(
                Icon::Range,
                f64::from(sow_core::execution::sam::sam_range(level as u8)),
                "",
                "",
            )];
            stats.push(BuildingMetric::new(
                Icon::Shield,
                sow_core::execution::sam::sam_salvo_size(level as u8) as f64,
                "×",
                "",
            ));
            if config.progressive_defense_upgrades {
                let cooldown = sow_core::execution::sam::sam_cooldown_ticks(level as u8, config);
                let shots_per_minute =
                    60_000.0 / (f64::from(cooldown) * f64::from(config.tick_rate_ms));
                stats.push(BuildingMetric::new(Icon::Shield, shots_per_minute, "", "/min"));
            }
            stats
        }
    };
    if kind.grants_building_troop_capacity() && district_capacity > 0.0 {
        metrics.push(BuildingMetric::new(Icon::Troops, district_capacity, "+", " cap"));
    }
    metrics
}

/// What the next level adds: only the effects that change, as "+" amounts.
pub(crate) fn upgrade_gains(
    current: &[BuildingMetric],
    next: &[BuildingMetric],
) -> Vec<BuildingMetric> {
    next.iter()
        .filter_map(|metric| match (
            current.iter().find(|before| before.icon == metric.icon && before.unit == metric.unit),
            metric.value,
        ) {
            (Some(before), Some(after)) => {
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

fn metric_income_rate(metrics: &[BuildingMetric], icon: Icon, speed: f64) -> f64 {
    metrics
        .iter()
        .filter(|metric| metric.icon == icon && metric.unit == "/s")
        .filter_map(|metric| metric.value)
        .sum::<f64>()
        / speed.max(f64::EPSILON)
}

fn apply_credited_income(
    metrics: &mut [BuildingMetric],
    gold_before_building: f64,
    troops_before_building: f64,
    rate_multiplier: f64,
    speed: f64,
) {
    for metric in metrics {
        let Some(value) = metric.value.filter(|_| metric.unit == "/s") else {
            continue;
        };
        let (baseline, cap, excess) = match metric.icon {
            Icon::Gold => (
                gold_before_building,
                sow_core::execution::income_rates::GOLD_PASSIVE_FULL_RATE_CAP,
                sow_core::execution::income_rates::GOLD_PASSIVE_EXCESS_RATE,
            ),
            Icon::Troops => (
                troops_before_building,
                sow_core::execution::income_rates::TROOP_PASSIVE_FULL_RATE_CAP,
                sow_core::execution::income_rates::TROOP_PASSIVE_EXCESS_RATE,
            ),
            _ => continue,
        };
        let contribution = value / speed.max(f64::EPSILON) * rate_multiplier;
        let credited = sow_core::execution::income_rates::diminishing_passive_income(
            baseline + contribution,
            cap,
            excess,
        ) - sow_core::execution::income_rates::diminishing_passive_income(
            baseline, cap, excess,
        );
        metric.value = Some(credited.max(0.0) * speed);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UpgradeOffer {
    pub next_level: u8,
    pub cost: f64,
    pub seconds: f64,
    /// False while the building is busy or when the player cannot afford it.
    pub enabled: bool,
    /// Shown only when gold already covers two or more levels.
    pub max_offer: Option<MaxUpgradeOffer>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MaxUpgradeOffer {
    pub target_level: u8,
    pub cost: f64,
    pub seconds: f64,
    pub is_cap_max: bool,
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
    input: &crate::app::InputState,
    sf: f32,
    screen_h: f32,
) -> BuildingPanelLayout {
    let sf = sf.max(0.01);
    let (center, radius_logical) =
        crate::render::world::overlays::building_screen_anchor_and_radius(
            kind, tile_idx, map_w, input, sf,
        );
    let center = [center[0] * sf, center[1] * sf];
    let half = radius_logical * sf;
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
            .find(|building| {
                building.tile_idx == menu.tile_idx
                    && building.owner_id == my_id
                    && building.kind == sow_core::game::BuildingKind::City
            })
            .or_else(|| {
                snapshot
                    .buildings
                    .iter()
                    .find(|building| building.tile_idx == menu.tile_idx && building.owner_id == my_id)
            })?;
        let leader = snapshot
            .players
            .iter()
            .find(|player| player.id == my_id)
            .map(|player| player.leader)
            .unwrap_or_default();
        let config = &self.sim.config;
        let level = building.active_level();
        let mut current = building_metrics(building.kind, level, leader, config);
        let seconds = |ticks: u32| f64::from(ticks) * config.tick_rate_ms as f64 / 1000.0;
        let reserved_level = building.level.max(building.upgrade_target_level);
        let next_level = reserved_level.saturating_add(1);
        let mut next = building_metrics(building.kind, next_level, leader, config);
        let trade_route = if building.kind == BuildingKind::TradingPost {
            snapshot.railway_links.binary_search_by_key(&building.id, |route| route.from_building_id)
                .ok().map(|i| snapshot.railway_links[i]).filter(|route| !route.blocked)
        } else { None };
        if let Some(route) = trade_route {
            let bonus = route.distance_bonus(self.sim.map_w);
            for metric in current.iter_mut().chain(next.iter_mut()) {
                if metric.icon == Icon::Gold && metric.unit == "/s" {
                    if let Some(value) = metric.value.as_mut() {
                        *value *= bonus;
                    }
                }
            }
        }
        if building.kind == BuildingKind::TradingPost && trade_route.is_none() {
            for metrics in [&mut current, &mut next] {
                metrics.retain(|metric| metric.icon != Icon::Gold);
            }
        }
        if let Some(engine) = self.sim.engine.as_ref()
            && let Some(player) = engine.state.player(my_id)
        {
            let agg = engine
                .building_aggregates
                .get(usize::from(my_id))
                .copied()
                .unwrap_or_default();
            let trade_ships = engine
                .fleets
                .iter()
                .filter(|fleet| {
                    fleet.owner_id == my_id
                        && fleet.unit_type == sow_core::game::UnitType::TradeShip
                        && fleet.captured_trade_return.is_none()
                })
                .count() as u32;
            let council = sow_core::execution::income_rates::tafaifa_council_multiplier(
                leader,
                sow_core::execution::hero_effects::living_allies(&engine.state, my_id),
            );
            let bot_multiplier = if player.player_type == sow_core::player::PlayerType::Bot {
                0.75
            } else {
                1.0
            };
            let rate_multiplier = council * bot_multiplier;
            let (_, standard_troops) =
                sow_core::execution::income_rates::imperial_standard_income(agg, leader, config);
            let gold_total = (sow_core::execution::income_rates::gold_income_per_second(
                player.tile_count,
                agg,
                leader,
                config,
            ) * council
                + sow_core::execution::income_rates::alliance_gold_per_second(
                    player.alliances.len(),
                    leader,
                    config,
                )) * bot_multiplier
                + sow_core::execution::income_rates::trade_income_per_second(
                    trade_ships, leader, config,
                )
                + engine.hero_gold_per_second(my_id);
            let troops_total = (sow_core::execution::income_rates::troop_income_per_second(
                player.tile_count,
                agg,
                leader,
                config,
            ) * council
                + sow_core::execution::income_rates::alliance_troops_per_second(
                    player.alliances.len(),
                    leader,
                    config,
                )
                + standard_troops)
                * bot_multiplier
                + sow_core::execution::income_rates::trade_troops_per_second(
                    trade_ships, leader, config,
                );
            let gold_before_building = (gold_total
                - metric_income_rate(&current, Icon::Gold, config.global_speed_multiplier)
                    * rate_multiplier)
            .max(0.0);
            let troops_before_building = (troops_total
                - metric_income_rate(&current, Icon::Troops, config.global_speed_multiplier)
                    * rate_multiplier)
            .max(0.0);
            apply_credited_income(
                &mut current,
                gold_before_building,
                troops_before_building,
                rate_multiplier,
                config.global_speed_multiplier,
            );
            apply_credited_income(
                &mut next,
                gold_before_building,
                troops_before_building,
                rate_multiplier,
                config.global_speed_multiplier,
            );
        }
        if let Some(route) = trade_route
            && let Some(rate) = current.iter().find(|metric| metric.icon == Icon::Gold).and_then(|metric| metric.value)
        {
            current.push(BuildingMetric::new(Icon::Gold, rate * seconds(route.period_ticks), "+", " / trip"));
        }
        let upgrade_ticks = |target_level| {
            let ticks = sow_core::building::structure_upgrade_duration_ticks(
                building.kind,
                target_level,
            );
            if config.tutorial {
                (ticks / 2).max(1)
            } else {
                ticks
            }
        };
        let queued_upgrade_ticks = building
            .under_construction
            .then(|| {
                (building.level.saturating_add(1)..=reserved_level)
                    .fold(building.ticks_until_complete, |total, target_level| {
                        total.saturating_add(upgrade_ticks(target_level))
                    })
            });
        let snapshot_arsenal_ticks = non_warship_arsenal_ticks(building.arsenal_build);
        let engine_arsenal_ticks = self
            .sim
            .engine
            .as_ref()
            .and_then(|engine| engine.building_productions.get(&building.id))
            .and_then(|production| non_warship_arsenal_ticks(production.arsenal_build));
        let arsenal_ticks = snapshot_arsenal_ticks.or(engine_arsenal_ticks);
        let busy_ticks = queued_upgrade_ticks.or(arsenal_ticks);
        let playing = matches!(
            &snapshot.phase,
            sow_core::game::GamePhase::Playing
        );
        if building.under_construction && level == 0 {
            let progress = crate::render::world::overlays::construction_progress(
                building.kind,
                level,
                building.ticks_until_complete,
            );
            return Some(BuildingPanel {
                tile_idx: building.tile_idx,
                kind: building.kind,
                chips: current,
                upgrade: None,
                remaining_seconds: busy_ticks.map(seconds),
                progress,
            });
        }
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
        let cost = self
            .map_menu_cost(
                crate::input::map_click::MapMenuAction::UpgradeStructure,
                building.tile_idx,
            )
            .0
            .unwrap_or(f64::INFINITY);
        let mut chips = upgrade_gains(&current, &next);
        if building.kind == BuildingKind::City {
            let capacity = config.city_max_troops * leader.city_troop_capacity_multiplier();
            let troops = sow_core::building::city_upgrade_reinforcements(next_level, capacity);
            if troops > 0.0 {
                chips.push(BuildingMetric::new(Icon::Troops, troops, "+", ""));
            }
        }
        Some(BuildingPanel {
            tile_idx: building.tile_idx,
            kind: building.kind,
            chips,
            upgrade: Some(UpgradeOffer {
                next_level,
                cost,
                seconds: busy_ticks.map_or_else(
                    || seconds(upgrade_ticks(next_level)),
                    seconds,
                ),
                enabled: playing
                    && busy_ticks.is_none()
                    && cost.is_finite()
                    && self.current_player_gold() >= cost,
                max_offer: if !playing || busy_ticks.is_some() {
                    None
                } else {
                    self.sim.engine.as_ref().and_then(|engine| {
                        let cap = self
                            .campaign_building_max_level(building.kind)
                            .min(building.kind.max_level());
                        let (target, total_cost, ticks) = engine.max_affordable_upgrade_level(
                            building.id,
                            self.current_player_gold(),
                            cap,
                        )?;
                        (target > next_level).then(|| MaxUpgradeOffer {
                            target_level: target,
                            cost: total_cost,
                            seconds: seconds(ticks),
                            is_cap_max: target >= cap,
                        })
                    })
                },
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
            &self.input,
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

#[inline]
fn non_warship_arsenal_ticks(build: Option<sow_core::building::CityArsenalBuild>) -> Option<u32> {
    build
        .filter(|build| build.kind != sow_core::game::CityArsenalKind::Warship)
        .map(|build| build.ticks_until_complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_input_state(camera_x: f32, camera_y: f32, zoom: f32, screen_w: f32, screen_h: f32) -> crate::app::InputState {
        crate::app::InputState {
            tabletop: None,
            camera_x,
            camera_y,
            camera_zoom: zoom,
            target_zoom: zoom,
            screen_w,
            screen_h,
            dragging: false,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            hover_pointer: crate::app::HoverPointer::Mouse,
            active_touches: std::collections::HashMap::new(),
            map_pointer_start: None,
            last_pinch_state: None,
            touch_pan_residual: (0.0, 0.0),
            map_context_menu: None,
            map_context_menu_session: 0,
            last_right_press: None,
            building_drag: None,
            shift_pressed: false,
            has_snapped_camera_to_spawn: false,
            spawn_intent_rate: Default::default(),
            key_pan_up: false,
            key_pan_down: false,
            key_pan_left: false,
            key_pan_right: false,
            camera_focus_target: None,
            camera_focus_waiting_for_input_release: false,
            tutorial_camera_focus: false,
            camera_shot: None,
            tutorial_zoom_in_events: 0,
            tutorial_zoom_out_events: 0,
            tutorial_camera_drag_events: 0,
            tutorial_camera_key_pan_events: 0,
            tutorial_camera_drag_recorded: false,
            input_focused: false,
        }
    }

    #[test]
    fn non_warship_arsenal_ticks_ignores_port_warships() {
        let warship = sow_core::building::CityArsenalBuild {
            kind: sow_core::game::CityArsenalKind::Warship,
            ticks_until_complete: 42,
            total_ticks: 100,
            warship_source_tile: None,
            warship_target_tile: None,
            paid_gold_bits: 1_000.0_f64.to_bits(),
        };
        let nuke = sow_core::building::CityArsenalBuild {
            kind: sow_core::game::CityArsenalKind::Nuke,
            ticks_until_complete: 77,
            total_ticks: 100,
            warship_source_tile: None,
            warship_target_tile: None,
            paid_gold_bits: 10_000.0_f64.to_bits(),
        };
        assert_eq!(non_warship_arsenal_ticks(Some(warship)), None);
        assert_eq!(non_warship_arsenal_ticks(Some(nuke)), Some(77));
    }

    #[test]
    fn building_panel_layout_clamps_high_zoom_offset_within_viewport() {
        let map_w = 100u32;
        let tile_idx = 50 * map_w + 50;
        let [wx, wy] = crate::render::world::overlays::building_world_center(
            BuildingKind::Port,
            tile_idx,
            map_w,
        );
        let zoom = 200.0;
        let screen_w = 1280.0;
        let screen_h = 720.0;
        let camera_x = screen_w * 0.5 - wx * zoom;
        let camera_y = screen_h * 0.5 - wy * zoom;
        let input = test_input_state(camera_x, camera_y, zoom, screen_w, screen_h);
        let layout = building_panel_layout(BuildingKind::Port, tile_idx, map_w, &input, 1.0, screen_h);
        // Without an upper bound on radius, 2x2 footprint at zoom=200 gives half=200px;
        // clamped at 24% of viewport height (172.8px), keeping the panel anchored near the building.
        let expected_max_half = screen_h * 0.24;
        let offset = (layout.chips_center[1] - screen_h * 0.5).abs();
        assert!(
            offset <= expected_max_half + 8.0 + 14.0 + 0.1,
            "chips offset {offset} should stay bounded by capped anchor radius"
        );
        assert!((layout.chips_center[0] - screen_w * 0.5).abs() < 0.01);
    }

    #[test]
    fn military_camp_upgrade_gains_show_distinct_fractional_percentages() {
        let cfg = sow_core::game_config::GameConfig::default();
        let leader = sow_core::player::Leader::default();
        let expected_deltas = ["+2%", "+2.5%", "+3%", "+3.5%", "+3.5%", "+4%", "+4.5%"];
        for (idx, expected) in expected_deltas.iter().enumerate() {
            let level = (idx + 1) as u8;
            let current = building_metrics(BuildingKind::MilitaryCamp, level, leader, &cfg);
            let next = building_metrics(BuildingKind::MilitaryCamp, level + 1, leader, &cfg);
            let gains = upgrade_gains(&current, &next);
            let speed_chip = gains
                .iter()
                .find(|m| m.icon == Icon::Speed && m.unit == "%")
                .expect("speed gain chip");
            assert_eq!(speed_chip.label(), *expected, "level {level} -> {}", level + 1);
        }
    }
}

