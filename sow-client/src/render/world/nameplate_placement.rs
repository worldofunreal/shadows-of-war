const AVATAR_TEXT_GAP_SCALE: f32 = 0.16;
const BADGE_GAP: f32 = 3.0;
const BADGE_STACK_GAP: f32 = 2.0;
// OpenFront recalculates player clusters every 20 ticks after land changes.
const LAND_REFRESH_TICKS: u64 = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TileRect {
    pub(crate) x0: u32,
    pub(crate) y0: u32,
    pub(crate) x1: u32,
    pub(crate) y1: u32,
}

impl TileRect {
    pub(crate) fn center(self) -> [f32; 2] {
        [
            (self.x0 as f32 + self.x1 as f32) * 0.5,
            (self.y0 as f32 + self.y1 as f32) * 0.5,
        ]
    }

    pub(crate) fn world_bounds(self) -> [f32; 4] {
        [
            self.x0 as f32,
            self.y0 as f32,
            self.x1 as f32,
            self.y1 as f32,
        ]
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
pub(crate) struct NameplateLandCache {
    component_ids: Vec<u32>,
    flood_stack: Vec<usize>,
    column_heights: Vec<u32>,
    column_components: Vec<u32>,
    histogram_stack: Vec<HistogramEntry>,
    largest_by_owner: Vec<LargestLandmass>,
    map_width: u32,
    map_height: u32,
    last_rebuild_tick: Option<u64>,
    dirty: bool,
}

impl NameplateLandCache {
    pub(crate) fn rect_for(&self, player_id: u16) -> Option<TileRect> {
        self.largest_by_owner
            .get(usize::from(
                player_id & sow_core::map::GameMap::PLAYER_ID_MASK,
            ))?
            .name_rect
    }

    pub(crate) fn has_landmass(&self, player_id: u16) -> bool {
        self.largest_by_owner
            .get(usize::from(
                player_id & sow_core::map::GameMap::PLAYER_ID_MASK,
            ))
            .is_some_and(|landmass| landmass.tile_count > 0)
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn needs_rebuild(
        &self,
        map: &sow_core::map::GameMap,
        tick: u64,
        force: bool,
    ) -> bool {
        self.map_width != map.width
            || self.map_height != map.height
            || self.component_ids.len() != map.state.len()
            || map.terrain.len() != map.state.len()
            || (self.dirty
                && (force
                    || self
                        .last_rebuild_tick
                        .is_none_or(|last| tick.saturating_sub(last) >= LAND_REFRESH_TICKS)))
    }

    pub(crate) fn rebuild(&mut self, map: &sow_core::map::GameMap, tick: u64) -> bool {
        let Some(tile_count) = (map.width as usize).checked_mul(map.height as usize) else {
            self.largest_by_owner.clear();
            self.dirty = true;
            return false;
        };
        if map.width == 0
            || map.height == 0
            || map.terrain.len() != tile_count
            || map.state.len() != tile_count
        {
            self.largest_by_owner.clear();
            self.dirty = true;
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
            let owner_id = map.state[seed] & sow_core::map::GameMap::PLAYER_ID_MASK;
            if owner_id == 0 || !map.terrain[seed].is_land() || self.component_ids[seed] != 0 {
                continue;
            }
            let Some(component_id) = next_component_id.checked_add(1) else {
                self.largest_by_owner.fill(LargestLandmass::default());
                self.dirty = true;
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
                            || map.state[neighbor] & sow_core::map::GameMap::PLAYER_ID_MASK
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
                let owner_id = map.state[index] & sow_core::map::GameMap::PLAYER_ID_MASK;
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
                let owner_id = map.state[row + start] & sow_core::map::GameMap::PLAYER_ID_MASK;
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
        self.last_rebuild_tick = Some(tick);
        self.dirty = false;
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

pub(crate) fn fit_size_to_land(
    content_width: f32,
    content_height: f32,
    land_bounds: [f32; 4],
    zoom_scaled: f32,
) -> f32 {
    let available_width = (land_bounds[2] - land_bounds[0]).max(0.0) * zoom_scaled.max(0.0);
    let available_height = (land_bounds[3] - land_bounds[1]).max(0.0) * zoom_scaled.max(0.0);
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
pub(crate) struct NameplateStatus {
    pub(crate) show_names: bool,
    pub(crate) show_troops: bool,
    pub(crate) is_me: bool,
    pub(crate) is_allied: bool,
    pub(crate) has_request: bool,
    pub(crate) has_rank: bool,
    pub(crate) has_traitor: bool,
    pub(crate) has_active_emoji: bool,
    pub(crate) has_disconnected: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct NameplateBounds {
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

    fn include_disc(&mut self, center: [f32; 2], radius: f32) {
        self.include_rect(
            center[0] - radius,
            center[1] - radius,
            center[0] + radius,
            center[1] + radius,
        );
    }

    pub(crate) fn extents_about(self, center: [f32; 2]) -> [f32; 2] {
        if !self.min_x.is_finite() {
            return [0.0; 2];
        }
        [
            (center[0] - self.min_x).max(self.max_x - center[0]),
            (center[1] - self.min_y).max(self.max_y - center[1]),
        ]
    }
}

#[derive(Clone, Copy)]
pub(crate) struct NameplateLayout {
    pub(crate) avatar_center: [f32; 2],
    pub(crate) avatar_radius: f32,
    pub(crate) badge_size: f32,
    left_x: f32,
    right_x: f32,
    pub(crate) rank_center: [f32; 2],
    pub(crate) star_center: [f32; 2],
    pub(crate) text_top: f32,
    pub(crate) item_spacing_y: f32,
    pub(crate) name_size: [f32; 2],
    pub(crate) troops_size: [f32; 2],
    pub(crate) badge_effect_padding: f32,
    stack_step: f32,
    express_offset: f32,
}

impl NameplateLayout {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compute(
        center: [f32; 2],
        render_size: f32,
        avatar_diameter: f32,
        avatar_radius: f32,
        badge_size: f32,
        name_size: [f32; 2],
        troops_size: [f32; 2],
        show_names: bool,
        show_troops: bool,
        badge_effect_padding: f32,
    ) -> Self {
        let name_size = if show_names { name_size } else { [0.0; 2] };
        let troops_size = if show_troops { troops_size } else { [0.0; 2] };
        let item_spacing_y = if show_names && show_troops {
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
        let content_top = center[1] - total_height * 0.5;
        let avatar_center = [center[0], content_top + avatar_diameter * 0.5];
        let avatar_visual_radius = avatar_visual_radius(avatar_radius);
        let badge_effect_padding = badge_effect_padding.max(0.0);
        let badge_clearance = BADGE_GAP + badge_effect_padding;
        let badge_half = badge_size * 0.5;
        let stack_step = badge_size + BADGE_STACK_GAP + badge_effect_padding * 2.0;
        let left_x = center[0] - avatar_visual_radius - badge_half - badge_clearance;
        let right_x = center[0] + avatar_visual_radius + badge_half + badge_clearance;

        Self {
            avatar_center,
            avatar_radius,
            badge_size,
            left_x,
            right_x,
            rank_center: [
                center[0],
                avatar_center[1] - avatar_visual_radius - badge_half - badge_clearance,
            ],
            star_center: [left_x, avatar_center[1]],
            text_top: content_top + avatar_diameter + avatar_text_gap,
            item_spacing_y,
            name_size,
            troops_size,
            badge_effect_padding,
            stack_step,
            express_offset: 2.0,
        }
    }

    pub(crate) fn scaled_about(self, center: [f32; 2], scale: f32) -> Self {
        let scale_point = |point: [f32; 2]| {
            [
                center[0] + (point[0] - center[0]) * scale,
                center[1] + (point[1] - center[1]) * scale,
            ]
        };
        Self {
            avatar_center: scale_point(self.avatar_center),
            avatar_radius: self.avatar_radius * scale,
            badge_size: self.badge_size * scale,
            left_x: center[0] + (self.left_x - center[0]) * scale,
            right_x: center[0] + (self.right_x - center[0]) * scale,
            rank_center: scale_point(self.rank_center),
            star_center: scale_point(self.star_center),
            text_top: center[1] + (self.text_top - center[1]) * scale,
            item_spacing_y: self.item_spacing_y * scale,
            name_size: [self.name_size[0] * scale, self.name_size[1] * scale],
            troops_size: [self.troops_size[0] * scale, self.troops_size[1] * scale],
            badge_effect_padding: self.badge_effect_padding * scale,
            stack_step: self.stack_step * scale,
            express_offset: self.express_offset * scale,
        }
    }

    pub(crate) fn side_badge_center(&self, left: bool, stack_slot: usize, is_me: bool) -> [f32; 2] {
        let x = if left { self.left_x } else { self.right_x };
        let y = if left && is_me {
            self.avatar_center[1] + self.stack_step
        } else {
            self.avatar_center[1] - stack_slot as f32 * self.stack_step
        };
        [x, y]
    }

    pub(crate) fn express_center(&self, right_stack_slots: usize) -> [f32; 2] {
        [
            self.right_x + self.express_offset,
            self.side_badge_center(false, right_stack_slots, false)[1] - self.badge_size * 0.5,
        ]
    }

    pub(crate) fn visual_bounds(
        self,
        center: [f32; 2],
        status: NameplateStatus,
        badge_padding: f32,
        text_padding: f32,
    ) -> NameplateBounds {
        let mut bounds = NameplateBounds::empty();
        if status.show_names {
            let text_height = self.name_size[1];
            bounds.include_rect(
                center[0] - self.name_size[0] * 0.5 - text_padding,
                self.text_top - text_height * 0.2 - text_padding,
                center[0] + self.name_size[0] * 0.5 + text_padding,
                self.text_top + text_height * 1.05 + text_padding,
            );
        }
        if status.show_troops {
            let row_y = if status.show_names {
                self.text_top + self.name_size[1] + self.item_spacing_y
            } else {
                self.text_top
            };
            let text_height = self.troops_size[1];
            bounds.include_rect(
                center[0] - self.troops_size[0] * 0.5 - text_padding,
                row_y - text_height * 0.2 - text_padding,
                center[0] + self.troops_size[0] * 0.5 + text_padding,
                row_y + text_height * 1.05 + text_padding,
            );
        }
        if self.avatar_radius <= 0.0 {
            return bounds;
        }

        bounds.include_disc(self.avatar_center, avatar_visual_radius(self.avatar_radius));
        if status.has_disconnected {
            bounds.include_disc(
                [
                    self.avatar_center[0] + self.avatar_radius * 0.6,
                    self.avatar_center[1] + self.avatar_radius * 0.6,
                ],
                self.avatar_radius * 0.4 + badge_padding,
            );
        }
        let mut include_badge = |badge_center: [f32; 2]| {
            bounds.include_disc(badge_center, self.badge_size * 0.5 + badge_padding);
        };
        if status.has_rank {
            include_badge(self.rank_center);
        }
        if status.is_me {
            include_badge(self.star_center);
        }
        if status.has_request {
            include_badge(self.side_badge_center(true, 0, status.is_me));
        }
        let mut right_slots = 0usize;
        if status.is_allied {
            include_badge(self.side_badge_center(false, right_slots, status.is_me));
            right_slots += 1;
        }
        if status.has_traitor {
            include_badge(self.side_badge_center(false, right_slots, status.is_me));
            right_slots += 1;
        }
        if status.has_active_emoji {
            include_badge(self.express_center(right_slots));
        }
        bounds
    }
}

pub(crate) fn fit_bounds_to_land(
    bounds: NameplateBounds,
    center: [f32; 2],
    land_bounds: [f32; 4],
    zoom_scaled: f32,
) -> f32 {
    let extents = bounds.extents_about(center);
    fit_size_to_land(extents[0] * 2.0, extents[1] * 2.0, land_bounds, zoom_scaled)
}

pub(crate) fn avatar_visual_radius(radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let border = (radius * 0.12).max(1.0);
    radius + border * 0.8
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
        cache.mark_dirty();
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
        assert_eq!(rect.center(), [3.5, 2.0]);
        assert_eq!(rect.world_bounds(), [1.0, 1.0, 6.0, 3.0]);
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
        assert_eq!(rect.center(), [1.0, 1.0]);
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
        assert_eq!(first_anchor, [2.0, 2.0]);

        for y in 1..3 {
            for x in 1..3 {
                map.set_owner_id(x, y, 0);
            }
        }
        cache.mark_dirty();
        assert!(!cache.needs_rebuild(&map, LAND_REFRESH_TICKS - 1, false));
        assert!(cache.needs_rebuild(&map, LAND_REFRESH_TICKS, false));
        assert!(cache.rebuild(&map, LAND_REFRESH_TICKS));

        let second_anchor = cache.rect_for(1).expect("remaining island").center();
        assert_eq!(second_anchor, [10.0, 2.0]);
        assert_ne!(second_anchor, [6.0, 2.0]);
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
        let center = [40.0, 50.0];
        let layout = NameplateLayout::compute(
            center,
            16.0,
            64.0,
            32.0,
            28.8,
            [40.0, 10.0],
            [30.0, 10.0],
            true,
            true,
            2.0,
        );
        let undecorated = layout.visual_bounds(
            center,
            NameplateStatus {
                show_names: true,
                show_troops: true,
                ..Default::default()
            },
            2.0,
            2.0,
        );
        let full_status = NameplateStatus {
            show_names: true,
            show_troops: true,
            is_me: true,
            is_allied: true,
            has_request: true,
            has_rank: true,
            has_traitor: true,
            has_active_emoji: true,
            has_disconnected: true,
        };
        let full = layout.visual_bounds(center, full_status, 2.0, 2.0);
        let full_extents = full.extents_about(center);
        let undecorated_extents = undecorated.extents_about(center);
        assert!(full_extents[0] > undecorated_extents[0]);
        assert!(full_extents[1] > undecorated_extents[1]);

        let land_bounds = [10.0, 20.0, 13.0, 22.0];
        for zoom_scaled in [1.0, 4.0, 12.0] {
            let scale = fit_bounds_to_land(full, center, land_bounds, zoom_scaled);
            let scaled_layout = layout.scaled_about(center, scale);
            assert!(scale > 0.0 && scale <= 1.0);
            assert!(scaled_layout.badge_effect_padding <= layout.badge_effect_padding);
            assert!(full_extents[0] * 2.0 * scale <= 3.0 * zoom_scaled + 0.001);
            assert!(full_extents[1] * 2.0 * scale <= 2.0 * zoom_scaled + 0.001);
        }
    }

    #[test]
    fn fit_scale_accounts_for_zoom_and_hides_invalid_land_bounds() {
        assert!((fit_size_to_land(40.0, 20.0, [10.0, 20.0, 13.0, 22.0], 4.0) - 0.3).abs() < 0.001);
        assert_eq!(fit_size_to_land(10.0, 10.0, [4.0, 4.0, 4.0, 8.0], 5.0), 0.0);
    }
}
