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
    let level_weight = sow_core::building::income_level_weight(level.min(4));
    let capacity_level_weight = sow_core::building::income_level_weight(level);
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
                    * level_weight * per_second,
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
        BuildingKind::Factory => {
            let mut metrics = vec![BuildingMetric::new(
                Icon::Gold,
                config.factory_gold_income
                    * sow_core::building::factory_gold_level_weight(level as u8)
                    * hero(District::Factory, Resource::Gold)
                    * per_second,
                "+",
                "/s",
            )];
            if let Some(bonus) = hero_bonus(District::Factory, Resource::Troops) {
                metrics.push(BuildingMetric::new(
                    Icon::Troops,
                    config.hero_factory_troop_income * level * bonus * per_second,
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
        BuildingKind::Farm => vec![
            BuildingMetric::new(
                Icon::Troops,
                config.farm_troop_income
                    * sow_core::building::farm_troop_level_weight(level as u8)
                    * hero(District::Farm, Resource::Troops)
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
    /// False when gold is short; the button still answers a tap with the reason.
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
        let mut next = building_metrics(building.kind, level.saturating_add(1), leader, config);
        let trade_route = if building.kind == BuildingKind::Factory {
            snapshot.railway_links.binary_search_by_key(&building.id, |route| route.from_building_id)
                .ok().map(|i| snapshot.railway_links[i]).filter(|route| !route.blocked)
        } else { None };
        if building.kind == BuildingKind::Factory && trade_route.is_none() {
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
        if building.under_construction {
            let progress = crate::render::world::overlays::construction_progress(
                building.kind,
                level,
                building.reserved_level(),
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
                seconds: seconds(sow_core::building::structure_upgrade_duration_ticks(
                    building.kind,
                    next_level,
                )),
                enabled: cost.is_finite() && self.current_player_gold() >= cost,
                max_offer: self.sim.engine.as_ref().and_then(|engine| {
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
                }),
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

