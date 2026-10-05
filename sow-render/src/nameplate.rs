const AVATAR_TEXT_GAP_SCALE: f32 = 0.16;
const BADGE_GAP: f32 = 3.0;
const BADGE_STACK_GAP: f32 = 2.0;
// OpenFront recalculates player clusters every 20 ticks after land changes.
const LAND_REFRESH_TICKS: u64 = 20;
pub const NAMEPLATE_SAMPLE_TICKS: u64 = 4;
const HUMAN_AVATAR_SCALE: f32 = 4.0;
const BOT_AVATAR_SCALE: f32 = 3.0;
const NATION_AVATAR_SCALE: f32 = 3.6;
const BADGE_SCALE: f32 = 1.8;
const TROOPS_SCALE: f32 = 1.30;
const NAMEPLATE_SIZE_DEADZONE: f32 = 0.2;

use crate::text::PreparedText;
use sow_core::player::PlayerType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameplatePresentation {
    Full,
    Compact,
    Hidden,
}

pub trait NameplateCapacityPlan {
    fn is_human(&self) -> bool;
    fn full_instance_count(&self) -> Option<usize>;
    fn select_presentation(&mut self, presentation: NameplatePresentation);
    fn selected_presentation(&self) -> NameplatePresentation;
}

pub fn assign_nameplate_capacity<T: NameplateCapacityPlan>(plans: &mut [T], mut remaining: usize) {
    const COMPACT_INSTANCES: usize = 2;

    for plan in plans.iter_mut().filter(|plan| plan.is_human()) {
        if remaining >= COMPACT_INSTANCES {
            plan.select_presentation(NameplatePresentation::Compact);
            remaining -= COMPACT_INSTANCES;
        } else {
            plan.select_presentation(NameplatePresentation::Hidden);
        }
    }

    for plan in plans.iter_mut().filter(|plan| plan.is_human()) {
        if plan.selected_presentation() != NameplatePresentation::Compact {
            continue;
        }
        let Some(full_count) = plan.full_instance_count() else {
            continue;
        };
        let extra = full_count.saturating_sub(COMPACT_INSTANCES);
        if extra <= remaining {
            remaining -= extra;
            if full_count < COMPACT_INSTANCES {
                remaining += COMPACT_INSTANCES - full_count;
            }
            plan.select_presentation(NameplatePresentation::Full);
        }
    }

    for plan in plans.iter_mut().filter(|plan| !plan.is_human()) {
        if let Some(full_count) = plan.full_instance_count()
            && full_count <= remaining
        {
            remaining -= full_count;
            plan.select_presentation(NameplatePresentation::Full);
        } else if remaining >= COMPACT_INSTANCES {
            remaining -= COMPACT_INSTANCES;
            plan.select_presentation(NameplatePresentation::Compact);
        } else {
            plan.select_presentation(NameplatePresentation::Hidden);
        }
    }
}

#[derive(Clone, Copy)]
pub struct NameplateMetrics {
    render_size: f32,
    avatar_diameter: f32,
    avatar_radius: f32,
    badge_size: f32,
    troops_render_size: f32,
}

impl NameplateMetrics {
    pub fn compute(scaled_size: f32, player_type: PlayerType, show_bot_avatars: bool) -> Self {
        let render_size = if scaled_size.is_finite() {
            scaled_size.max(7.0)
        } else {
            7.0
        };
        let avatar_scale = match player_type {
            PlayerType::Human => HUMAN_AVATAR_SCALE,
            PlayerType::Bot if show_bot_avatars => BOT_AVATAR_SCALE,
            PlayerType::Nation => NATION_AVATAR_SCALE,
            PlayerType::Bot => 0.0,
        };
        let avatar_diameter = if avatar_scale > 0.0 {
            (render_size * avatar_scale).max(4.0)
        } else {
            0.0
        };
        Self {
            render_size,
            avatar_diameter,
            avatar_radius: avatar_diameter * 0.5,
            badge_size: avatar_diameter.max(render_size * BADGE_SCALE),
            troops_render_size: render_size * TROOPS_SCALE,
        }
    }

    pub fn render_size(self) -> f32 {
        self.render_size
    }

    pub fn avatar_diameter(self) -> f32 {
        self.avatar_diameter
    }

    pub fn avatar_radius(self) -> f32 {
        self.avatar_radius
    }

    pub fn badge_size(self) -> f32 {
        self.badge_size
    }

    pub fn troops_render_size(self) -> f32 {
        self.troops_render_size
    }
}

pub fn fitted_font_px(
    scaled_size: f32,
    fit_scale: f32,
    font_scale: f32,
    show_names: bool,
    show_troops: bool,
) -> f32 {
    if !scaled_size.is_finite() || !fit_scale.is_finite() || fit_scale < 0.0 {
        return 0.0;
    }
    let font_px = scaled_size.max(7.0) * font_size_scale(font_scale) * fit_scale;
    if !font_px.is_finite() {
        return 0.0;
    }
    if show_names {
        font_px
    } else if show_troops {
        font_px * TROOPS_SCALE
    } else {
        f32::INFINITY
    }
}

pub fn font_size_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.max(0.1)
    } else {
        0.1
    }
}

pub fn sample_due(last_tick: Option<u64>, current_tick: u64, my_id_changed: bool) -> bool {
    my_id_changed
        || last_tick.is_none_or(|last_tick| {
            current_tick.saturating_sub(last_tick) >= NAMEPLATE_SAMPLE_TICKS
        })
}

pub fn size_needs_interpolation(from: f32, to: f32) -> bool {
    (to - from).abs() > NAMEPLATE_SIZE_DEADZONE
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapPoint(pub [f32; 2]);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenPoint(pub [f32; 2]);

/// Returns whether a projected land rectangle touches the viewport.
#[inline]
pub fn land_rect_intersects_viewport(
    min: ScreenPoint,
    max: ScreenPoint,
    viewport_width: f32,
    viewport_height: f32,
) -> bool {
    !(max.0[0] < 0.0 || min.0[0] > viewport_width || max.0[1] < 0.0 || min.0[1] > viewport_height)
}

/// Culls off-screen land before running the potentially expensive fog lookup.
#[inline]
pub fn nameplate_visible_after_viewport_and_fog(
    min: ScreenPoint,
    max: ScreenPoint,
    viewport_width: f32,
    viewport_height: f32,
    fog_check: impl FnOnce() -> bool,
) -> bool {
    land_rect_intersects_viewport(min, max, viewport_width, viewport_height) && fog_check()
}

/// Mirrors the nameplate fog rule; callers still cull off-screen land first.
#[inline]
pub fn fog_allows_nameplate(
    fog_enabled: bool,
    is_local_player: bool,
    fog_hidden_phase: bool,
    tile_explored: bool,
) -> bool {
    !fog_enabled || is_local_player || fog_hidden_phase || tile_explored
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldRect(pub [f32; 4]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl TileRect {
    pub fn center(self) -> MapPoint {
        MapPoint([
            (self.x0 as f32 + self.x1 as f32) * 0.5,
            (self.y0 as f32 + self.y1 as f32) * 0.5,
        ])
    }

    pub fn world_bounds(self) -> WorldRect {
        WorldRect([
            self.x0 as f32,
            self.y0 as f32,
            self.x1 as f32,
            self.y1 as f32,
        ])
    }

    fn area(self) -> u64 {
        u64::from(self.x1 - self.x0) * u64::from(self.y1 - self.y0)
    }
}

#[derive(Clone, Copy, Default)]
struct LargestLandmass {
    component_id: u32,
    tile_count: usize,
    bounds: TileRect,
    name_rect: Option<TileRect>,
}

#[derive(Clone, Copy)]
struct HistogramEntry {
    start: usize,
    height: u32,
}

#[derive(Default)]
pub struct NameplateLandCache {
    component_ids: Vec<u32>,
    flood_stack: Vec<usize>,
    column_heights: Vec<u32>,
    column_components: Vec<u32>,
    histogram_stack: Vec<HistogramEntry>,
    largest_by_owner: Vec<LargestLandmass>,
    map_width: u32,
    map_height: u32,
    last_rebuild_tick: Option<u64>,
    ownership_revision: u64,
}

impl NameplateLandCache {
    pub fn rect_for(&self, player_id: u16) -> Option<TileRect> {
        self.largest_by_owner
            .get(usize::from(
                player_id & sow_core::map::GameMap::PLAYER_ID_MASK,
            ))?
            .name_rect
    }

    pub fn has_landmass(&self, player_id: u16) -> bool {
        self.largest_by_owner
            .get(usize::from(
                player_id & sow_core::map::GameMap::PLAYER_ID_MASK,
            ))
            .is_some_and(|landmass| landmass.tile_count > 0)
    }

    pub fn needs_rebuild(&self, map: &sow_core::map::GameMap, tick: u64, force: bool) -> bool {
        self.map_width != map.width
            || self.map_height != map.height
            || self.component_ids.len() != map.owner_states().len()
            || map.terrain.len() != map.owner_states().len()
            || force
            || (self.ownership_revision != map.ownership_revision()
                && self
                    .last_rebuild_tick
                    .is_none_or(|last| tick.saturating_sub(last) >= LAND_REFRESH_TICKS))
    }

    pub fn rebuild(&mut self, map: &sow_core::map::GameMap, tick: u64) -> bool {
        let Some(tile_count) = (map.width as usize).checked_mul(map.height as usize) else {
            self.largest_by_owner.clear();
            return false;
        };
        if map.width == 0
            || map.height == 0
            || map.terrain.len() != tile_count
            || map.owner_states().len() != tile_count
        {
            self.largest_by_owner.clear();
            return false;
        }

        self.largest_by_owner.resize_with(
            usize::from(sow_core::map::GameMap::PLAYER_ID_MASK) + 1,
            Default::default,
        );
        self.largest_by_owner.fill(LargestLandmass::default());
        self.component_ids.resize(tile_count, 0);
        self.component_ids.fill(0);
        self.flood_stack.clear();

        let width = map.width as usize;
        let height = map.height as usize;
        let mut next_component_id = 0u32;

        // Flood-fill each owned land component once. Row-major seeds make equal
        // components deterministic: the first component for an owner wins.
        for seed in 0..tile_count {
            let owner_id = map.owner_states()[seed] & sow_core::map::GameMap::PLAYER_ID_MASK;
            if owner_id == 0 || !map.terrain[seed].is_land() || self.component_ids[seed] != 0 {
                continue;
            }
            let Some(component_id) = next_component_id.checked_add(1) else {
                self.largest_by_owner.fill(LargestLandmass::default());
                return false;
            };
            next_component_id = component_id;
            self.component_ids[seed] = component_id;
            self.flood_stack.push(seed);

            let mut count = 0usize;
            let mut min_x = (seed % width) as u32;
            let mut max_x = min_x;
            let mut min_y = (seed / width) as u32;
            let mut max_y = min_y;
            while let Some(index) = self.flood_stack.pop() {
                let x = index % width;
                let y = index / width;
                count += 1;
                min_x = min_x.min(x as u32);
                max_x = max_x.max(x as u32);
                min_y = min_y.min(y as u32);
                max_y = max_y.max(y as u32);

                let first_y = y.saturating_sub(1);
                let last_y = (y + 1).min(height - 1);
                let first_x = x.saturating_sub(1);
                let last_x = (x + 1).min(width - 1);
                for neighbor_y in first_y..=last_y {
                    let row = neighbor_y * width;
                    for neighbor_x in first_x..=last_x {
                        let neighbor = row + neighbor_x;
                        if self.component_ids[neighbor] != 0
                            || !map.terrain[neighbor].is_land()
                            || map.owner_states()[neighbor] & sow_core::map::GameMap::PLAYER_ID_MASK
                                != owner_id
                        {
                            continue;
                        }
                        self.component_ids[neighbor] = component_id;
                        self.flood_stack.push(neighbor);
                    }
                }
            }

            let best = &mut self.largest_by_owner[usize::from(owner_id)];
            if count > best.tile_count {
                best.component_id = component_id;
                best.tile_count = count;
                best.bounds = TileRect {
                    x0: min_x,
                    y0: min_y,
                    x1: max_x + 1,
                    y1: max_y + 1,
                };
            }
        }

        self.column_heights.resize(width, 0);
        self.column_heights.fill(0);
        self.column_components.resize(width, 0);
        self.column_components.fill(0);

        // Exact maximal-rectangle scan, split at component boundaries so a plate
        // cannot bridge water or another player's territory.
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let index = row + x;
                let component_id = self.component_ids[index];
                let owner_id = map.owner_states()[index] & sow_core::map::GameMap::PLAYER_ID_MASK;
                let selected = component_id != 0
                    && self.largest_by_owner[usize::from(owner_id)].component_id == component_id;
                if selected {
                    self.column_heights[x] = if self.column_components[x] == component_id {
                        self.column_heights[x].saturating_add(1)
                    } else {
                        1
                    };
                    self.column_components[x] = component_id;
                } else {
                    self.column_heights[x] = 0;
                    self.column_components[x] = 0;
                }
            }
            let mut x = 0usize;
            while x < width {
                let component_id = self.column_components[x];
                if component_id == 0 {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < width && self.column_components[x] == component_id {
                    x += 1;
                }
                let end = x;
                let owner_id =
                    map.owner_states()[row + start] & sow_core::map::GameMap::PLAYER_ID_MASK;
                self.histogram_stack.clear();
                for column in start..=end {
                    let current_height = if column == end {
                        0
                    } else {
                        self.column_heights[column]
                    };
                    let mut rectangle_start = column;
                    while self
                        .histogram_stack
                        .last()
                        .is_some_and(|entry| entry.height > current_height)
                    {
                        let entry = self.histogram_stack.pop().expect("stack was checked");
                        let candidate = TileRect {
                            x0: entry.start as u32,
                            y0: y as u32 + 1 - entry.height,
                            x1: column as u32,
                            y1: y as u32 + 1,
                        };
                        let best = &mut self.largest_by_owner[usize::from(owner_id)];
                        if preferred_name_rect(candidate, best.name_rect, best.bounds) {
                            best.name_rect = Some(candidate);
                        }
                        rectangle_start = entry.start;
                    }
                    if current_height > 0
                        && self
                            .histogram_stack
                            .last()
                            .is_none_or(|entry| entry.height < current_height)
                    {
                        self.histogram_stack.push(HistogramEntry {
                            start: rectangle_start,
                            height: current_height,
                        });
                    }
                }
            }
        }
        self.map_width = map.width;
        self.map_height = map.height;
        self.ownership_revision = map.ownership_revision();
        self.last_rebuild_tick = Some(tick);
        true
    }
}

fn preferred_name_rect(
    candidate: TileRect,
    current: Option<TileRect>,
    land_bounds: TileRect,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    let candidate_area = candidate.area();
    let current_area = current.area();
    if candidate_area != current_area {
        return candidate_area > current_area;
    }
    let candidate_distance = rect_center_distance_squared(candidate, land_bounds);
    let current_distance = rect_center_distance_squared(current, land_bounds);
    (candidate_distance, candidate.y0, candidate.x0) < (current_distance, current.y0, current.x0)
}

fn rect_center_distance_squared(rect: TileRect, bounds: TileRect) -> u128 {
    let dx = (i128::from(rect.x0) + i128::from(rect.x1))
        - (i128::from(bounds.x0) + i128::from(bounds.x1));
    let dy = (i128::from(rect.y0) + i128::from(rect.y1))
        - (i128::from(bounds.y0) + i128::from(bounds.y1));
    (dx * dx + dy * dy) as u128
}

pub fn fit_size_to_land(
    content_width: f32,
    content_height: f32,
    land_bounds: WorldRect,
    zoom_scaled: f32,
) -> f32 {
    if !content_width.is_finite()
        || !content_height.is_finite()
        || content_width < 0.0
        || content_height < 0.0
        || !land_bounds.0.into_iter().all(f32::is_finite)
        || !zoom_scaled.is_finite()
    {
        return 0.0;
    }
    let available_width = (land_bounds.0[2] - land_bounds.0[0]).max(0.0) * zoom_scaled.max(0.0);
    let available_height = (land_bounds.0[3] - land_bounds.0[1]).max(0.0) * zoom_scaled.max(0.0);
    if available_width <= 0.0 || available_height <= 0.0 {
        return 0.0;
    }
    let width_scale = if content_width > 0.0 {
        available_width / content_width
    } else {
        1.0
    };
    let height_scale = if content_height > 0.0 {
        available_height / content_height
    } else {
        1.0
    };
    width_scale.min(height_scale).min(1.0).max(0.0)
}

#[derive(Clone, Copy, Default)]
pub struct NameplateStatus {
    pub show_names: bool,
    pub show_troops: bool,
    pub is_me: bool,
    pub is_allied: bool,
    pub has_request: bool,
    pub rank: Option<usize>,
    pub has_traitor: bool,
    pub has_active_emoji: bool,
    pub has_disconnected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    Rank1,
    Rank2,
    Rank3,
    Star,
    Request,
    Allied,
    Traitor,
    Active,
    Disconnected,
}

#[derive(Clone, Copy)]
pub struct NameplateBadge {
    kind: BadgeKind,
    center: ScreenPoint,
    diameter: f32,
    tint: [f32; 4],
}

impl NameplateBadge {
    pub fn kind(self) -> BadgeKind {
        self.kind
    }

    pub fn center(self) -> ScreenPoint {
        self.center
    }

    pub fn diameter(self) -> f32 {
        self.diameter
    }

    pub fn tint(self) -> [f32; 4] {
        self.tint
    }
}

#[derive(Clone, Copy)]
pub struct NameplateBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl NameplateBounds {
    fn empty() -> Self {
        Self {
            min_x: f32::INFINITY,
            min_y: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            max_y: f32::NEG_INFINITY,
        }
    }

    fn include_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.min_x = self.min_x.min(x0);
        self.min_y = self.min_y.min(y0);
        self.max_x = self.max_x.max(x1);
        self.max_y = self.max_y.max(y1);
    }

    fn include_disc(&mut self, center: ScreenPoint, radius: f32) {
        self.include_rect(
            center.0[0] - radius,
            center.0[1] - radius,
            center.0[0] + radius,
            center.0[1] + radius,
        );
    }

    pub fn extents_about(self, center: ScreenPoint) -> [f32; 2] {
        if !self.min_x.is_finite() {
            return [0.0; 2];
        }
        [
            (center.0[0] - self.min_x).max(self.max_x - center.0[0]),
            (center.0[1] - self.min_y).max(self.max_y - center.0[1]),
        ]
    }
}

#[derive(Clone, Copy)]
pub struct NameplateLayout {
    avatar_center: ScreenPoint,
    avatar_radius: f32,
    badge_size: f32,
    left_x: f32,
    right_x: f32,
    rank_center: ScreenPoint,
    star_center: ScreenPoint,
    text_top: f32,
    item_spacing_y: f32,
    name_size: [f32; 2],
    troops_size: [f32; 2],
    name_visual_bounds: Option<[f32; 4]>,
    troops_visual_bounds: Option<[f32; 4]>,
    troops_icon_size: f32,
    troops_icon_gap: f32,
    badge_effect_padding: f32,
    badges: [Option<NameplateBadge>; 7],
    badge_count: usize,
    stack_step: f32,
    express_offset: f32,
}

impl NameplateLayout {
    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        center: ScreenPoint,
        metrics: NameplateMetrics,
        name: &PreparedText,
        troops: &PreparedText,
        font_scale: f32,
        status: NameplateStatus,
        badge_effect_padding: f32,
    ) -> Self {
        let font_scale = font_size_scale(font_scale);
        let name_measure = name.measure_at(metrics.render_size * font_scale);
        let name_visual_bounds = name.visual_bounds_at(metrics.render_size * font_scale);
        let troops_icon_size = metrics.troops_render_size * font_scale;
        let troops_measure = troops.measure_at(troops_icon_size);
        let troops_visual_bounds = troops.visual_bounds_at(troops_icon_size);
        let name_size = [name_measure.width, name_measure.height];
        let troops_size = [
            troops_icon_size + 3.0 + troops_measure.width,
            troops_icon_size.max(troops_measure.height),
        ];
        let render_size = metrics.render_size;
        let avatar_diameter = metrics.avatar_diameter;
        let avatar_radius = metrics.avatar_radius;
        let badge_size = metrics.badge_size;
        let name_size = if status.show_names {
            name_size
        } else {
            [0.0; 2]
        };
        let name_visual_bounds = status.show_names.then_some(name_visual_bounds).flatten();
        let troops_size = if status.show_troops {
            troops_size
        } else {
            [0.0; 2]
        };
        let troops_visual_bounds = status.show_troops.then_some(troops_visual_bounds).flatten();
        let item_spacing_y = if status.show_names && status.show_troops {
            render_size * 0.111
        } else {
            0.0
        };
        let text_height = name_size[1] + item_spacing_y + troops_size[1];
        let avatar_text_gap = if avatar_diameter > 0.0 && text_height > 0.0 {
            render_size * AVATAR_TEXT_GAP_SCALE
        } else {
            0.0
        };
        let total_height = avatar_diameter + avatar_text_gap + text_height;
        let content_top = center.0[1] - total_height * 0.5;
        let avatar_center = ScreenPoint([center.0[0], content_top + avatar_diameter * 0.5]);
        let avatar_visual_radius = avatar_visual_radius(avatar_radius);
        let badge_effect_padding = badge_effect_padding.max(0.0);
        let badge_clearance = BADGE_GAP + badge_effect_padding;
        let badge_half = badge_size * 0.5;
        let stack_step = badge_size + BADGE_STACK_GAP + badge_effect_padding * 2.0;
        let left_x = center.0[0] - avatar_visual_radius - badge_half - badge_clearance;
        let right_x = center.0[0] + avatar_visual_radius + badge_half + badge_clearance;

        let mut layout = Self {
            avatar_center,
            avatar_radius,
            badge_size,
            left_x,
            right_x,
            rank_center: ScreenPoint([
                center.0[0],
                avatar_center.0[1] - avatar_visual_radius - badge_half - badge_clearance,
            ]),
            star_center: ScreenPoint([left_x, avatar_center.0[1]]),
            text_top: content_top + avatar_diameter + avatar_text_gap,
            item_spacing_y,
            name_size,
            troops_size,
            name_visual_bounds,
            troops_visual_bounds,
            troops_icon_size,
            troops_icon_gap: 3.0,
            badge_effect_padding,
            badges: [None; 7],
            badge_count: 0,
            stack_step,
            express_offset: 2.0,
        };
        layout.prepare_badges(status);
        layout
    }

    pub fn scaled_about(self, center: ScreenPoint, scale: f32) -> Self {
        let scale_point = |point: ScreenPoint| {
            ScreenPoint([
                center.0[0] + (point.0[0] - center.0[0]) * scale,
                center.0[1] + (point.0[1] - center.0[1]) * scale,
            ])
        };
        Self {
            avatar_center: scale_point(self.avatar_center),
            avatar_radius: self.avatar_radius * scale,
            badge_size: self.badge_size * scale,
            left_x: center.0[0] + (self.left_x - center.0[0]) * scale,
            right_x: center.0[0] + (self.right_x - center.0[0]) * scale,
            rank_center: scale_point(self.rank_center),
            star_center: scale_point(self.star_center),
            text_top: center.0[1] + (self.text_top - center.0[1]) * scale,
            item_spacing_y: self.item_spacing_y * scale,
            name_size: [self.name_size[0] * scale, self.name_size[1] * scale],
            troops_size: [self.troops_size[0] * scale, self.troops_size[1] * scale],
            name_visual_bounds: self
                .name_visual_bounds
                .map(|bounds| bounds.map(|value| value * scale)),
            troops_visual_bounds: self
                .troops_visual_bounds
                .map(|bounds| bounds.map(|value| value * scale)),
            troops_icon_size: self.troops_icon_size * scale,
            troops_icon_gap: self.troops_icon_gap * scale,
            badge_effect_padding: self.badge_effect_padding * scale,
            badges: self.badges.map(|badge| {
                badge.map(|badge| NameplateBadge {
                    center: scale_point(badge.center),
                    diameter: badge.diameter * scale,
                    ..badge
                })
            }),
            badge_count: self.badge_count,
            stack_step: self.stack_step * scale,
            express_offset: self.express_offset * scale,
        }
    }

    pub fn side_badge_center(&self, left: bool, stack_slot: usize, is_me: bool) -> ScreenPoint {
        let x = if left { self.left_x } else { self.right_x };
        let y = if left && is_me {
            self.avatar_center.0[1] + self.stack_step
        } else {
            self.avatar_center.0[1] - stack_slot as f32 * self.stack_step
        };
        ScreenPoint([x, y])
    }

    pub fn express_center(&self, right_stack_slots: usize) -> ScreenPoint {
        ScreenPoint([
            self.right_x + self.express_offset,
            self.side_badge_center(false, right_stack_slots, false).0[1] - self.badge_size * 0.5,
        ])
    }

    pub fn name_anchor(&self, center: ScreenPoint) -> ScreenPoint {
        ScreenPoint([center.0[0], self.text_top + self.name_size[1] * 0.85])
    }

    pub fn avatar_center(&self) -> ScreenPoint {
        self.avatar_center
    }

    pub fn avatar_radius(&self) -> f32 {
        self.avatar_radius
    }

    pub fn badge_effect_padding(&self) -> f32 {
        self.badge_effect_padding
    }

    pub fn badges(&self) -> &[Option<NameplateBadge>; 7] {
        &self.badges
    }

    pub fn badge_count(&self) -> usize {
        self.badge_count
    }

    pub fn troops_text_anchor(&self, center: ScreenPoint) -> ScreenPoint {
        let row_y = if self.name_size[1] > 0.0 {
            self.text_top + self.name_size[1] + self.item_spacing_y
        } else {
            self.text_top
        };
        let left_x = center.0[0] - self.troops_size[0] * 0.5;
        ScreenPoint([
            left_x + self.troops_icon_size + self.troops_icon_gap,
            row_y + self.troops_size[1] * 0.85,
        ])
    }

    pub fn troops_icon(&self, center: ScreenPoint) -> (ScreenPoint, f32) {
        let row_y = if self.name_size[1] > 0.0 {
            self.text_top + self.name_size[1] + self.item_spacing_y
        } else {
            self.text_top
        };
        let left_x = center.0[0] - self.troops_size[0] * 0.5;
        (
            ScreenPoint([
                left_x + self.troops_icon_size * 0.5,
                row_y + self.troops_icon_size * 0.5,
            ]),
            self.troops_icon_size,
        )
    }

    pub fn visual_bounds(self, center: ScreenPoint, text_padding: f32) -> NameplateBounds {
        let mut bounds = NameplateBounds::empty();
        if let Some([min_x, min_y, max_x, max_y]) = self.name_visual_bounds {
            let anchor = self.name_anchor(center);
            bounds.include_rect(
                anchor.0[0] + min_x - text_padding,
                anchor.0[1] + min_y - text_padding,
                anchor.0[0] + max_x + text_padding,
                anchor.0[1] + max_y + text_padding,
            );
        }
        if let Some([min_x, min_y, max_x, max_y]) = self.troops_visual_bounds {
            let anchor = self.troops_text_anchor(center);
            bounds.include_rect(
                anchor.0[0] + min_x - text_padding,
                anchor.0[1] + min_y - text_padding,
                anchor.0[0] + max_x + text_padding,
                anchor.0[1] + max_y + text_padding,
            );
        }
        if self.troops_icon_size > 0.0 {
            let (icon_center, diameter) = self.troops_icon(center);
            bounds.include_disc(icon_center, diameter * 0.5 + self.badge_effect_padding);
        }
        if self.avatar_radius <= 0.0 {
            return bounds;
        }

        bounds.include_disc(self.avatar_center, avatar_visual_radius(self.avatar_radius));
        for badge in self.badges[..self.badge_count].iter().flatten() {
            bounds.include_disc(
                badge.center,
                badge.diameter * 0.5 + self.badge_effect_padding,
            );
        }
        bounds
    }

    fn prepare_badges(&mut self, status: NameplateStatus) {
        if self.avatar_radius <= 0.0 {
            return;
        }
        if let Some(rank) = status.rank {
            let (kind, tint) = match rank {
                1 => (
                    BadgeKind::Rank1,
                    [250.0 / 255.0, 204.0 / 255.0, 21.0 / 255.0, 1.0],
                ),
                2 => (
                    BadgeKind::Rank2,
                    [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
                ),
                _ => (
                    BadgeKind::Rank3,
                    [217.0 / 255.0, 119.0 / 255.0, 6.0 / 255.0, 1.0],
                ),
            };
            let center = self.rank_center;
            let size = self.badge_size;
            self.add_badge(kind, center, size, tint);
        }
        if status.is_me {
            let center = self.star_center;
            let size = self.badge_size;
            self.add_badge(BadgeKind::Star, center, size, [1.0; 4]);
        }
        if status.has_request {
            let center = self.side_badge_center(true, 0, status.is_me);
            let size = self.badge_size;
            self.add_badge(BadgeKind::Request, center, size, [1.0; 4]);
        }
        let mut right_slots = 0usize;
        if status.is_allied {
            let center = self.side_badge_center(false, right_slots, status.is_me);
            let size = self.badge_size;
            self.add_badge(BadgeKind::Allied, center, size, [1.0; 4]);
            right_slots += 1;
        }
        if status.has_traitor {
            let center = self.side_badge_center(false, right_slots, status.is_me);
            let size = self.badge_size;
            self.add_badge(BadgeKind::Traitor, center, size, [1.0; 4]);
            right_slots += 1;
        }
        if status.has_active_emoji {
            let center = self.express_center(right_slots);
            let size = self.badge_size;
            self.add_badge(BadgeKind::Active, center, size, [1.0; 4]);
        }
        if status.has_disconnected {
            let center = ScreenPoint([
                self.avatar_center.0[0] + self.avatar_radius * 0.6,
                self.avatar_center.0[1] + self.avatar_radius * 0.6,
            ]);
            let diameter = self.avatar_radius * 0.8;
            self.add_badge(BadgeKind::Disconnected, center, diameter, [1.0; 4]);
        }
    }

    fn add_badge(&mut self, kind: BadgeKind, center: ScreenPoint, diameter: f32, tint: [f32; 4]) {
        self.badges[self.badge_count] = Some(NameplateBadge {
            kind,
            center,
            diameter,
            tint,
        });
        self.badge_count += 1;
    }
}

pub fn fit_bounds_to_land(
    bounds: NameplateBounds,
    center: ScreenPoint,
    land_bounds: WorldRect,
    zoom_scaled: f32,
) -> f32 {
    let extents = bounds.extents_about(center);
    fit_size_to_land(extents[0] * 2.0, extents[1] * 2.0, land_bounds, zoom_scaled)
}

pub(crate) fn avatar_visual_radius(radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let border = radius * 0.12;
    radius + border * 0.3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_map(width: u32, height: u32) -> sow_core::map::GameMap {
        let mut map = sow_core::map::GameMap::new(width, height);
        map.terrain.fill(sow_core::map::MapTile::from_byte(0));
        map
    }

    fn own_tile(map: &mut sow_core::map::GameMap, x: u32, y: u32, owner: u16) {
        let index = map.ref_id(x, y);
        map.terrain[index] = sow_core::map::MapTile::from_byte(0b1000_0000);
        map.set_owner_id(x, y, owner);
    }

    #[test]
    fn chooses_the_largest_island_and_keeps_its_rectangle_on_owned_land() {
        let mut map = test_map(10, 8);
        for y in 1..=5 {
            for x in 1..=5 {
                if (x, y) != (3, 3) {
                    own_tile(&mut map, x, y, 1);
                }
            }
        }
        own_tile(&mut map, 9, 7, 1);
        let hole = map.ref_id(3, 3);
        assert!(!map.terrain[hole].is_land());
        assert_eq!(map.owner_id(3, 3), 0);

        let mut cache = NameplateLandCache::default();
        assert!(cache.needs_rebuild(&map, 0, false));
        assert!(cache.rebuild(&map, 0));
        assert!(!cache.needs_rebuild(&map, 0, false));
        assert!(cache.has_landmass(1));
        assert!(!cache.has_landmass(2));
        map.set_owner_id(1, 1, 1);
        assert!(!cache.needs_rebuild(&map, 1, false));
        map.set_owner_id(1, 1, 2);
        assert!(!cache.needs_rebuild(&map, LAND_REFRESH_TICKS - 1, false));
        assert!(cache.needs_rebuild(&map, LAND_REFRESH_TICKS, false));
        assert!(cache.needs_rebuild(&map, 1, true));
        assert!(cache.rebuild(&map, LAND_REFRESH_TICKS));
        assert!(!cache.needs_rebuild(&map, LAND_REFRESH_TICKS, false));
        assert_eq!(cache.component_ids[hole], 0);
        let rect = cache.rect_for(1).expect("largest landmass rectangle");
        assert!(rect.area() < 25, "selected rectangle: {rect:?}");
        assert!(rect.x0 >= 1 && rect.x1 <= 6 && rect.y0 >= 1 && rect.y1 <= 6);
        for y in rect.y0..rect.y1 {
            for x in rect.x0..rect.x1 {
                let index = map.ref_id(x, y);
                assert!(map.terrain[index].is_land());
                assert_eq!(map.owner_id(x, y), 1);
            }
        }
    }

    #[test]
    fn equal_islands_choose_the_first_row_major_component() {
        let mut map = test_map(9, 5);
        for (x0, y0) in [(0, 0), (7, 3)] {
            for y in y0..y0 + 2 {
                for x in x0..x0 + 2 {
                    own_tile(&mut map, x, y, 4);
                }
            }
        }
        let global_center = [4, 2];
        let global_center_index = map.ref_id(global_center[0], global_center[1]);
        assert!(!map.terrain[global_center_index].is_land());

        let mut cache = NameplateLandCache::default();
        assert!(cache.rebuild(&map, 0));
        let rect = cache.rect_for(4).expect("largest component rectangle");
        assert_eq!(
            rect,
            TileRect {
                x0: 0,
                y0: 0,
                x1: 2,
                y1: 2,
            }
        );
        assert_eq!(rect.center(), MapPoint([1.0, 1.0]));
        assert_eq!(map.owner_id(1, 1), 4);
        assert_eq!(cache.rect_for(2), None);
    }

    #[test]
    fn rectangle_never_includes_water_or_enemy_land() {
        let mut map = test_map(5, 3);
        for y in 0..3 {
            for x in 0..5 {
                own_tile(&mut map, x, y, 1);
            }
        }
        let water = map.ref_id(2, 1);
        map.terrain[water] = sow_core::map::MapTile::from_byte(0);
        map.set_owner_id(2, 1, 0);
        own_tile(&mut map, 3, 1, 2);

        let mut cache = NameplateLandCache::default();
        assert!(cache.rebuild(&map, 0));
        let rect = cache.rect_for(1).expect("owned-land rectangle");
        assert_eq!(
            rect,
            TileRect {
                x0: 0,
                y0: 0,
                x1: 2,
                y1: 3,
            }
        );
        for y in rect.y0..rect.y1 {
            for x in rect.x0..rect.x1 {
                let index = map.ref_id(x, y);
                assert!(map.terrain[index].is_land());
                assert_eq!(map.owner_id(x, y), 1);
            }
        }
    }

    #[test]
    fn changing_islands_updates_the_anchor_without_a_water_midpoint() {
        let mut map = test_map(12, 4);
        for (x0, x1) in [(1, 3), (9, 11)] {
            for y in 1..3 {
                for x in x0..x1 {
                    own_tile(&mut map, x, y, 1);
                }
            }
        }

        let mut cache = NameplateLandCache::default();
        assert!(cache.rebuild(&map, 0));
        let first_anchor = cache.rect_for(1).expect("first island").center();
        assert_eq!(first_anchor, MapPoint([2.0, 2.0]));
        let upgraded_tile = map.ref_id(1, 1) as u32;
        map.tile_upgrades.insert(upgraded_tile, 1);
        assert!(!cache.needs_rebuild(&map, LAND_REFRESH_TICKS - 1, false));

        for y in 1..3 {
            for x in 1..3 {
                map.set_owner_id(x, y, 0);
            }
        }
        assert!(!cache.needs_rebuild(&map, LAND_REFRESH_TICKS - 1, false));
        assert!(cache.needs_rebuild(&map, LAND_REFRESH_TICKS, false));
        assert!(cache.rebuild(&map, LAND_REFRESH_TICKS));

        let second_anchor = cache.rect_for(1).expect("remaining island").center();
        assert_eq!(second_anchor, MapPoint([10.0, 2.0]));
        assert_ne!(second_anchor, MapPoint([6.0, 2.0]));
        assert!(!map.terrain[map.ref_id(6, 2)].is_land());
    }

    #[test]
    fn rectangle_ties_prefer_bbox_center_then_row_major_order() {
        let mut map = test_map(4, 4);
        for x in 0..4 {
            own_tile(&mut map, x, 0, 1);
        }
        for y in 1..4 {
            own_tile(&mut map, 2, y, 1);
        }
        let mut cache = NameplateLandCache::default();
        assert!(cache.rebuild(&map, 0));
        assert_eq!(
            cache.rect_for(1),
            Some(TileRect {
                x0: 2,
                y0: 0,
                x1: 3,
                y1: 4,
            })
        );

        let mut map = test_map(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                if (x, y) != (1, 1) {
                    own_tile(&mut map, x, y, 1);
                }
            }
        }
        let mut cache = NameplateLandCache::default();
        assert!(cache.rebuild(&map, 0));
        assert_eq!(
            cache.rect_for(1),
            Some(TileRect {
                x0: 0,
                y0: 0,
                x1: 3,
                y1: 1,
            })
        );
    }

    #[test]
    fn player_without_owned_land_has_no_nameplate_rectangle() {
        let map = test_map(4, 4);
        let mut cache = NameplateLandCache::default();
        assert!(cache.rebuild(&map, 0));
        assert!(!cache.has_landmass(1));
        assert_eq!(cache.rect_for(1), None);
    }

    #[test]
    fn full_nameplate_and_badges_fit_the_owned_rectangle_at_each_zoom() {
        let center = ScreenPoint([40.0, 50.0]);
        let metrics = NameplateMetrics::compute(16.0, PlayerType::Human, true);
        let empty_text = PreparedText::default();
        let undecorated_status = NameplateStatus {
            ..Default::default()
        };
        let layout = NameplateLayout::compute(
            center,
            metrics,
            &empty_text,
            &empty_text,
            1.0,
            undecorated_status,
            2.0,
        );
        let undecorated = layout.visual_bounds(center, 2.0);
        let full_status = NameplateStatus {
            show_names: false,
            show_troops: false,
            is_me: true,
            is_allied: true,
            has_request: true,
            rank: Some(1),
            has_traitor: true,
            has_active_emoji: true,
            has_disconnected: true,
            ..Default::default()
        };
        let full_layout = NameplateLayout::compute(
            center,
            metrics,
            &empty_text,
            &empty_text,
            1.0,
            full_status,
            2.0,
        );
        let full = full_layout.visual_bounds(center, 2.0);
        let full_extents = full.extents_about(center);
        let undecorated_extents = undecorated.extents_about(center);
        assert!(full_extents[0] > undecorated_extents[0]);
        assert!(full_extents[1] > undecorated_extents[1]);
        assert_eq!(full_layout.badge_count, 7);
        for badge in full_layout.badges[..full_layout.badge_count]
            .iter()
            .flatten()
        {
            let extents = full.extents_about(center);
            let radius = badge.diameter * 0.5 + full_layout.badge_effect_padding;
            assert!(extents[0] + 0.001 >= (badge.center.0[0] - center.0[0]).abs() + radius);
            assert!(extents[1] + 0.001 >= (badge.center.0[1] - center.0[1]).abs() + radius);
        }

        let land_bounds = WorldRect([10.0, 20.0, 13.0, 22.0]);
        for zoom_scaled in [1.0, 4.0, 12.0] {
            let scale = fit_bounds_to_land(full, center, land_bounds, zoom_scaled);
            let scaled_layout = full_layout.scaled_about(center, scale);
            assert!(scale > 0.0 && scale <= 1.0);
            assert!(scaled_layout.badge_effect_padding <= layout.badge_effect_padding);
            assert!(full_extents[0] * 2.0 * scale <= 3.0 * zoom_scaled + 0.001);
            assert!(full_extents[1] * 2.0 * scale <= 2.0 * zoom_scaled + 0.001);
        }
    }

    #[test]
    fn fit_scale_accounts_for_zoom_and_hides_invalid_land_bounds() {
        assert!(
            (fit_size_to_land(40.0, 20.0, WorldRect([10.0, 20.0, 13.0, 22.0]), 4.0) - 0.3).abs()
                < 0.001
        );
        assert_eq!(
            fit_size_to_land(10.0, 10.0, WorldRect([4.0, 4.0, 4.0, 8.0]), 5.0),
            0.0
        );
    }

    #[test]
    fn nameplate_metrics_preserve_human_bot_and_nation_avatar_sizes() {
        let human = NameplateMetrics::compute(14.0, PlayerType::Human, true);
        let bot = NameplateMetrics::compute(14.0, PlayerType::Bot, true);
        let nation = NameplateMetrics::compute(14.0, PlayerType::Nation, true);
        assert_eq!(human.avatar_diameter, 14.0 * HUMAN_AVATAR_SCALE);
        assert_eq!(bot.avatar_diameter, 14.0 * BOT_AVATAR_SCALE);
        assert_eq!(nation.avatar_diameter, 14.0 * NATION_AVATAR_SCALE);
        assert_eq!(human.badge_size(), human.avatar_diameter());
        assert_eq!(bot.badge_size(), bot.avatar_diameter());
        assert_eq!(nation.badge_size(), nation.avatar_diameter());
    }

    #[test]
    fn nameplate_lod_uses_the_final_fitted_font_size() {
        assert_eq!(fitted_font_px(8.0, 1.0, 1.0, true, true), 8.0);
        assert!(fitted_font_px(8.0, 0.5, 1.0, true, true) < 7.0);
        assert_eq!(
            fitted_font_px(8.0, 0.5, 1.0, false, true),
            8.0 * 0.5 * TROOPS_SCALE
        );
        assert!(fitted_font_px(8.0, 0.5, 1.0, false, true) < 7.0);
        assert!(fitted_font_px(8.0, 0.1, 1.0, false, false).is_infinite());
    }

    #[test]
    fn viewport_culling_keeps_partial_land_and_rejects_fully_outside_rectangles() {
        let viewport = (100.0, 80.0);
        for (min, max) in [
            ([-5.0, 10.0], [5.0, 20.0]),
            ([95.0, 10.0], [105.0, 20.0]),
            ([10.0, -5.0], [20.0, 5.0]),
            ([10.0, 75.0], [20.0, 85.0]),
            ([0.0, 0.0], [0.0, 0.0]),
        ] {
            assert!(land_rect_intersects_viewport(
                ScreenPoint(min),
                ScreenPoint(max),
                viewport.0,
                viewport.1,
            ));
        }

        for (min, max) in [
            ([-20.0, 10.0], [-1.0, 20.0]),
            ([101.0, 10.0], [120.0, 20.0]),
            ([10.0, -20.0], [20.0, -1.0]),
            ([10.0, 81.0], [20.0, 100.0]),
        ] {
            assert!(!land_rect_intersects_viewport(
                ScreenPoint(min),
                ScreenPoint(max),
                viewport.0,
                viewport.1,
            ));
        }
    }

    #[test]
    fn fog_visibility_preserves_local_spawning_and_explored_nameplates() {
        assert!(fog_allows_nameplate(false, false, false, false));
        assert!(fog_allows_nameplate(true, true, false, false));
        assert!(fog_allows_nameplate(true, false, true, false));
        assert!(fog_allows_nameplate(true, false, false, true));
        assert!(!fog_allows_nameplate(true, false, false, false));
    }

    #[test]
    fn offscreen_land_skips_the_fog_lookup_but_partial_land_reaches_it() {
        let fog_queried = std::cell::Cell::new(false);
        assert!(!nameplate_visible_after_viewport_and_fog(
            ScreenPoint([-20.0, 10.0]),
            ScreenPoint([-1.0, 20.0]),
            100.0,
            80.0,
            || {
                fog_queried.set(true);
                true
            },
        ));
        assert!(!fog_queried.get());

        assert!(!nameplate_visible_after_viewport_and_fog(
            ScreenPoint([-5.0, 10.0]),
            ScreenPoint([5.0, 20.0]),
            100.0,
            80.0,
            || {
                fog_queried.set(true);
                false
            },
        ));
        assert!(fog_queried.get());
    }

    #[test]
    fn nameplate_sampling_keeps_four_tick_updates_and_player_switches() {
        assert!(sample_due(None, 0, false));
        assert!(!sample_due(Some(0), 1, false));
        assert!(!sample_due(Some(0), 3, false));
        assert!(sample_due(Some(0), 4, false));
        assert!(sample_due(Some(4), 4, true));
    }

    #[test]
    fn nameplate_size_deadzone_skips_small_growth() {
        assert!(!size_needs_interpolation(10.0, 10.2));
        assert!(size_needs_interpolation(10.0, 10.21));
    }

    #[test]
    fn avatar_frame_bounds_scale_with_the_prepared_avatar() {
        let radius = 32.0;
        for scale in [1.0, 0.75, 0.5, 0.25] {
            assert!(
                (avatar_visual_radius(radius * scale) - avatar_visual_radius(radius) * scale).abs()
                    < 1e-5
            );
        }
    }

    struct TestCapacityPlan {
        human: bool,
        full_count: Option<usize>,
        selected: NameplatePresentation,
    }

    impl NameplateCapacityPlan for TestCapacityPlan {
        fn is_human(&self) -> bool {
            self.human
        }

        fn full_instance_count(&self) -> Option<usize> {
            self.full_count
        }

        fn select_presentation(&mut self, presentation: NameplatePresentation) {
            self.selected = presentation;
        }

        fn selected_presentation(&self) -> NameplatePresentation {
            self.selected
        }
    }

    fn capacity_plan(human: bool, full_count: Option<usize>) -> TestCapacityPlan {
        TestCapacityPlan {
            human,
            full_count,
            selected: NameplatePresentation::Hidden,
        }
    }

    #[test]
    fn capacity_keeps_human_compact_plates_before_bots_and_upgrades_in_order() {
        let mut plans = [
            capacity_plan(false, Some(1)),
            capacity_plan(true, Some(5)),
            capacity_plan(true, Some(5)),
        ];
        assign_nameplate_capacity(&mut plans, 7);
        assert_eq!(plans[0].selected, NameplatePresentation::Hidden);
        assert_eq!(plans[1].selected, NameplatePresentation::Full);
        assert_eq!(plans[2].selected, NameplatePresentation::Compact);
    }

    #[test]
    fn capacity_never_selects_a_full_plate_that_exceeds_the_available_budget() {
        let mut plans = [capacity_plan(true, Some(12))];
        assign_nameplate_capacity(&mut plans, 2);
        assert_eq!(plans[0].selected, NameplatePresentation::Compact);
    }
}
