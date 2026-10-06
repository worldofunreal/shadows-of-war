/// Closest same-kind building within stack range of the click (matches server logic).
pub fn find_stack_target_tile(
    kind: sow_core::game::BuildingKind,
    click_x: i32,
    click_y: i32,
    map_w: u32,
    my_id: u16,
    buildings: &[sow_core::protocol::BuildingSnapshot],
) -> Option<u32> {
    let stack_dist = sow_core::building::placement::STRUCTURE_MIN_DIST;
    let mut best: Option<(i32, u64, u32)> = None;
    for b in buildings {
        if b.owner_id != my_id || b.kind != kind {
            continue;
        }
        let bx = (b.tile_idx % map_w) as i32;
        let by = (b.tile_idx / map_w) as i32;
        let d = (click_x - bx).abs() + (click_y - by).abs();
        if d > stack_dist {
            continue;
        }
        let cand = (d, b.id, b.tile_idx);
        match best {
            None => best = Some(cand),
            Some((bd, bid, _)) => {
                if d < bd || (d == bd && b.id < bid) {
                    best = Some(cand);
                }
            }
        }
    }
    best.map(|(_, _, tile)| tile)
}

#[derive(Clone, Copy)]
pub struct PlacementQuery<'a> {
    pub kind: sow_core::game::BuildingKind,
    pub click_x: i32,
    pub click_y: i32,
    pub map_w: u32,
    pub map_h: u32,
    pub owners: &'a [u16],
    pub terrain: &'a [u8],
    pub my_id: u16,
    pub buildings: &'a [sow_core::protocol::BuildingSnapshot],
}

const PLACEMENT_CELL_SIZE: u32 = sow_core::building::core::BUILDING_GRID_CELL_SIZE;

#[derive(Default)]
pub struct BuildingPlacementCache {
    tick: Option<u64>,
    map_w: u32,
    map_h: u32,
    grid_w: u32,
    cells: Vec<Vec<usize>>,
    last_key: Option<(u32, u32, sow_core::game::BuildingKind, u16)>,
    last_result: Option<Result<u32, &'static str>>,
}

impl BuildingPlacementCache {
    pub fn resolve(&mut self, tick: u64, query: &PlacementQuery) -> Result<u32, &'static str> {
        let click_idx = if query.click_x >= 0
            && query.click_y >= 0
            && query.click_x < query.map_w as i32
            && query.click_y < query.map_h as i32
        {
            query.click_y as u32 * query.map_w + query.click_x as u32
        } else {
            u32::MAX
        };
        let key = (click_idx, query.map_w, query.kind, query.my_id);
        self.rebuild_index(tick, query);
        if self.last_key == Some(key)
            && let Some(result) = self.last_result
        {
            return result;
        }
        let result = resolve_building_placement_tile_indexed(query, self);
        self.last_key = Some(key);
        self.last_result = Some(result);
        result
    }

    fn rebuild_index(&mut self, tick: u64, query: &PlacementQuery) {
        if self.tick == Some(tick) && self.map_w == query.map_w && self.map_h == query.map_h {
            return;
        }
        self.tick = Some(tick);
        self.map_w = query.map_w;
        self.map_h = query.map_h;
        self.grid_w = query.map_w.div_ceil(PLACEMENT_CELL_SIZE);
        let grid_h = query.map_h.div_ceil(PLACEMENT_CELL_SIZE);
        let cells_len = (self.grid_w * grid_h) as usize;
        if self.cells.len() < cells_len {
            self.cells.resize_with(cells_len, Vec::new);
        }
        for cell in &mut self.cells[..cells_len] {
            cell.clear();
        }
        if query.map_w > 0 {
            let map_area = query.map_w.saturating_mul(query.map_h);
            for (index, building) in query.buildings.iter().enumerate() {
                if building.tile_idx >= map_area {
                    continue;
                }
                let x = building.tile_idx % query.map_w;
                let y = building.tile_idx / query.map_w;
                let cell =
                    ((y / PLACEMENT_CELL_SIZE) * self.grid_w + x / PLACEMENT_CELL_SIZE) as usize;
                self.cells[cell].push(index);
            }
        }
        self.last_key = None;
        self.last_result = None;
    }

    fn any_nearby(
        &self,
        query: &PlacementQuery,
        x: u32,
        y: u32,
        range: u32,
        mut test: impl FnMut(&sow_core::protocol::BuildingSnapshot) -> bool,
    ) -> bool {
        if self.grid_w == 0 || self.map_h == 0 {
            return false;
        }
        let max_x = x.saturating_add(range).min(query.map_w - 1);
        let max_y = y.saturating_add(range).min(query.map_h - 1);
        let min_cell_x = x.saturating_sub(range) / PLACEMENT_CELL_SIZE;
        let max_cell_x = max_x / PLACEMENT_CELL_SIZE;
        let min_cell_y = y.saturating_sub(range) / PLACEMENT_CELL_SIZE;
        let max_cell_y = max_y / PLACEMENT_CELL_SIZE;
        for cell_y in min_cell_y..=max_cell_y {
            for cell_x in min_cell_x..=max_cell_x {
                let cell = (cell_y * self.grid_w + cell_x) as usize;
                for &index in &self.cells[cell] {
                    let Some(building) = query.buildings.get(index) else {
                        continue;
                    };
                    let bx = building.tile_idx % query.map_w;
                    let by = building.tile_idx / query.map_w;
                    if bx.abs_diff(x) <= range && by.abs_diff(y) <= range && test(building) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub fn resolve_build_target_tile(query: &PlacementQuery) -> Result<u32, &'static str> {
    resolve_building_placement_tile(query)
}

pub fn resolve_building_placement_tile(query: &PlacementQuery) -> Result<u32, &'static str> {
    resolve_placement(query, None)
}

fn resolve_building_placement_tile_indexed(
    query: &PlacementQuery,
    index: &BuildingPlacementCache,
) -> Result<u32, &'static str> {
    resolve_placement(query, Some(index))
}

fn footprint_fits(
    query: &PlacementQuery,
    footprint: sow_core::building::BuildingFootprint,
) -> bool {
    for y in footprint.top..footprint.top + footprint.height as i32 {
        for x in footprint.left..footprint.left + footprint.width as i32 {
            if x < 0 || y < 0 || x >= query.map_w as i32 || y >= query.map_h as i32 {
                return false;
            }
            let idx = y as u32 * query.map_w + x as u32;
            if query.owners.get(idx as usize).copied() != Some(query.my_id)
                || query.terrain.get(idx as usize).copied().unwrap_or(0) & 0x80 == 0
            {
                return false;
            }
        }
    }
    true
}

fn footprint_touches_water(
    query: &PlacementQuery,
    footprint: sow_core::building::BuildingFootprint,
) -> bool {
    for y in footprint.top..footprint.top + footprint.height as i32 {
        for x in footprint.left..footprint.left + footprint.width as i32 {
            for (dx, dy) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, -1),
                (-1, -1),
                (1, 1),
                (-1, 1),
            ] {
                let nx = x + dx;
                let ny = y + dy;
                if nx >= 0 && ny >= 0 && nx < query.map_w as i32 && ny < query.map_h as i32 {
                    let idx = ny as u32 * query.map_w + nx as u32;
                    if query.terrain.get(idx as usize).copied().unwrap_or(0) & 0x80 == 0 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn blocked_by_building(
    query: &PlacementQuery,
    index: Option<&BuildingPlacementCache>,
    x: u32,
    y: u32,
    footprint: sow_core::building::BuildingFootprint,
    range: u32,
    enforce_spacing: bool,
) -> bool {
    let mut blocks = |building: &sow_core::protocol::BuildingSnapshot| {
        let bx = (building.tile_idx % query.map_w) as i32;
        let by = (building.tile_idx / query.map_w) as i32;
        let other = sow_core::building::BuildingFootprint::at(building.kind, bx as u32, by as u32);
        if footprint.intersects(other) {
            return true;
        }
        let min_dist = sow_core::building::minimum_building_spacing(query.kind, building.kind);
        let dx = x as i32 - bx;
        let dy = y as i32 - by;
        enforce_spacing && dx * dx + dy * dy < min_dist * min_dist
    };
    if let Some(index) = index {
        index.any_nearby(query, x, y, range, &mut blocks)
    } else {
        query.buildings.iter().any(&mut blocks)
    }
}

fn resolve_placement(
    query: &PlacementQuery,
    index: Option<&BuildingPlacementCache>,
) -> Result<u32, &'static str> {
    let kind = query.kind;
    let click_x = query.click_x;
    let click_y = query.click_y;
    let map_w = query.map_w;
    let map_h = query.map_h;
    let owners = query.owners;
    let terrain = query.terrain;
    let my_id = query.my_id;

    if kind == sow_core::game::BuildingKind::Farm {
        if click_x < 0 || click_y < 0 || click_x >= map_w as i32 || click_y >= map_h as i32 {
            return Err("Tap a lowland tile you own.");
        }
        let tile_idx = click_y as u32 * map_w + click_x as u32;
        let Some(&terrain_byte) = terrain.get(tile_idx as usize) else {
            return Err("Tap a lowland tile you own.");
        };
        let lowland = terrain_byte & 0x80 != 0 && terrain_byte & 0x1f < 10;
        let footprint =
            sow_core::building::BuildingFootprint::at(kind, click_x as u32, click_y as u32);
        let blocked = blocked_by_building(
            query,
            index,
            click_x as u32,
            click_y as u32,
            footprint,
            3,
            false,
        );
        if owners.get(tile_idx as usize).copied() != Some(my_id)
            || !lowland
            || !footprint_fits(query, footprint)
            || blocked
        {
            return Err("Tap an empty lowland tile you own.");
        }
        return Ok(tile_idx);
    }

    let pokayoke_dist = 25;
    let pokayoke_dist_sq = pokayoke_dist * pokayoke_dist;

    let mut found_any_owned = false;
    let mut found_any_land = false;
    let mut found_any_far_enough = false;

    let mut best: Option<(i32, u32)> = None;
    for dy in -pokayoke_dist..=pokayoke_dist {
        for dx in -pokayoke_dist..=pokayoke_dist {
            let tx = click_x + dx;
            let ty = click_y + dy;
            if tx < 0 || tx >= map_w as i32 || ty < 0 || ty >= map_h as i32 {
                continue;
            }
            if (dx * dx + dy * dy) >= pokayoke_dist_sq {
                continue;
            }
            let tile_idx = (ty * map_w as i32 + tx) as u32;

            if owners.get(tile_idx as usize).copied().unwrap_or(0) != my_id {
                continue;
            }
            found_any_owned = true;

            let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
            let is_land = (tile_terrain & 0x80) != 0;
            if !is_land {
                continue;
            }
            found_any_land = true;

            let footprint = sow_core::building::BuildingFootprint::at(kind, tx as u32, ty as u32);
            if !footprint_fits(query, footprint)
                || blocked_by_building(query, index, tx as u32, ty as u32, footprint, 6, true)
            {
                continue;
            }
            found_any_far_enough = true;

            if kind == sow_core::game::BuildingKind::Port
                && !footprint_touches_water(query, footprint)
            {
                continue;
            }
            let distance_sq = dx * dx + dy * dy;
            let candidate = (distance_sq, tile_idx);
            if best.is_none_or(|current| candidate < current) {
                best = Some(candidate);
            }
        }
    }

    let Some((_, tile_idx)) = best else {
        if !found_any_owned {
            return Err("Build inside your own land.");
        }
        if !found_any_land {
            return Err("Structures go on land, not water.");
        }
        if !found_any_far_enough {
            if kind == sow_core::game::BuildingKind::City {
                return Err("Too close to another City! Minimum spacing is 6 tiles.");
            } else {
                return Err(
                    "Too close to another structure! Spacing rules: City requires 6, other structures require 4.",
                );
            }
        }
        return Err("No space nearby!");
    };
    Ok(tile_idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::game::BuildingKind;

    #[test]
    fn placement_index_limits_checks_to_local_buildings() {
        let buildings: Vec<_> = (0..5000u32)
            .map(|tile_idx| sow_core::protocol::BuildingSnapshot {
                id: tile_idx as u64,
                owner_id: 1,
                tile_idx: tile_idx * 2,
                kind: sow_core::game::BuildingKind::Farm,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            })
            .collect();
        let owners = vec![1; 10_000];
        let terrain = vec![0x80; 10_000];
        let query = PlacementQuery {
            kind: sow_core::game::BuildingKind::City,
            click_x: 50,
            click_y: 50,
            map_w: 100,
            map_h: 100,
            owners: &owners,
            terrain: &terrain,
            my_id: 1,
            buildings: &buildings,
        };
        let mut cache = BuildingPlacementCache::default();
        cache.rebuild_index(1, &query);
        let mut checked = 0;
        assert!(!cache.any_nearby(&query, 50, 50, 6, |_| {
            checked += 1;
            false
        }));
        assert!(checked < buildings.len() / 10);
    }

    #[test]
    fn preview_result_is_cached_until_tick_or_hover_changes() {
        let (map_w, map_h, my_id) = (32, 32, 1);
        let owners = vec![my_id; (map_w * map_h) as usize];
        let terrain = vec![0x80; (map_w * map_h) as usize];
        let mut buildings = Vec::new();
        let mut cache = BuildingPlacementCache::default();
        let first = cache
            .resolve(
                10,
                &PlacementQuery {
                    kind: BuildingKind::City,
                    click_x: 16,
                    click_y: 16,
                    map_w,
                    map_h,
                    owners: &owners,
                    terrain: &terrain,
                    my_id,
                    buildings: &buildings,
                },
            )
            .unwrap();
        assert_eq!(first, 16 * map_w + 16);
        assert_eq!(
            cache.resolve(
                10,
                &PlacementQuery {
                    kind: BuildingKind::City,
                    click_x: 16,
                    click_y: 16,
                    map_w,
                    map_h,
                    owners: &owners,
                    terrain: &terrain,
                    my_id,
                    buildings: &buildings,
                }
            ),
            Ok(first)
        );

        buildings.push(sow_core::protocol::BuildingSnapshot {
            id: 1,
            owner_id: my_id,
            tile_idx: first,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        let next = PlacementQuery {
            kind: BuildingKind::City,
            click_x: 16,
            click_y: 16,
            map_w,
            map_h,
            owners: &owners,
            terrain: &terrain,
            my_id,
            buildings: &buildings,
        };
        assert_ne!(cache.resolve(11, &next), Ok(first));
    }

    #[test]
    fn stack_target_uses_owner_kind_nearest_tie_break_and_historical_radius() {
        let map_w = 40;
        let building = |id, owner_id, kind, x, y| sow_core::protocol::BuildingSnapshot {
            id,
            tile_idx: y * map_w + x,
            owner_id,
            kind,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        };
        let buildings = [
            building(1, 2, BuildingKind::City, 10, 10),
            building(8, 1, BuildingKind::City, 12, 10),
            building(9, 1, BuildingKind::City, 11, 10),
            building(2, 1, BuildingKind::Farm, 10, 10),
        ];

        assert_eq!(
            find_stack_target_tile(BuildingKind::City, 10, 10, map_w, 1, &buildings),
            Some(10 * map_w + 11)
        );

        let tied = [
            building(8, 1, BuildingKind::City, 12, 10),
            building(7, 1, BuildingKind::City, 10, 12),
            building(1, 2, BuildingKind::City, 10, 10),
            building(2, 1, BuildingKind::Farm, 10, 10),
        ];
        assert_eq!(
            find_stack_target_tile(BuildingKind::City, 10, 10, map_w, 1, &tied),
            Some(12 * map_w + 10)
        );

        let radius = sow_core::building::placement::STRUCTURE_MIN_DIST as u32;
        let boundary = [building(3, 1, BuildingKind::City, 10 + radius, 10)];
        let outside = [building(4, 1, BuildingKind::City, 11 + radius, 10)];
        assert_eq!(
            find_stack_target_tile(BuildingKind::City, 10, 10, map_w, 1, &boundary),
            Some(10 * map_w + 10 + radius)
        );
        assert_eq!(
            find_stack_target_tile(BuildingKind::City, 10, 10, map_w, 1, &outside),
            None
        );
    }

    #[test]
    fn farm_uses_the_standard_footprint_in_bounds_on_owned_land_without_overlap() {
        let (map_w, map_h, my_id) = (8, 8, 1);
        let owners = vec![my_id; (map_w * map_h) as usize];
        let terrain = vec![0x80; (map_w * map_h) as usize];
        let buildings = Vec::new();
        let query = PlacementQuery {
            kind: BuildingKind::Farm,
            click_x: 2,
            click_y: 2,
            map_w,
            map_h,
            owners: &owners,
            terrain: &terrain,
            my_id,
            buildings: &buildings,
        };
        assert_eq!(resolve_building_placement_tile(&query), Ok(18));

        let mut unowned = owners.clone();
        unowned[0] = 2;
        let invalid_owner = PlacementQuery {
            owners: &unowned,
            ..query
        };
        assert!(resolve_building_placement_tile(&invalid_owner).is_err());

        let mut terrain_with_water = terrain.clone();
        terrain_with_water[3 * map_w as usize] = 0;
        let invalid_terrain = PlacementQuery {
            terrain: &terrain_with_water,
            ..query
        };
        assert!(resolve_building_placement_tile(&invalid_terrain).is_err());

        let edge = PlacementQuery {
            click_x: 1,
            ..query
        };
        assert!(resolve_building_placement_tile(&edge).is_err());

        let overlapping = [sow_core::protocol::BuildingSnapshot {
            id: 1,
            owner_id: my_id,
            tile_idx: 2 * map_w + 5,
            kind: BuildingKind::Farm,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        }];
        let overlap_query = PlacementQuery {
            buildings: &overlapping,
            ..query
        };
        let mut cache = BuildingPlacementCache::default();
        assert!(cache.resolve(1, &overlap_query).is_err());
    }
}
