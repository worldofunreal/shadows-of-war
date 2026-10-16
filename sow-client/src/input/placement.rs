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

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlacementDirection {
    Up,
    Right,
    Down,
    Left,
}

impl PlacementDirection {
    pub(crate) fn offset(self) -> (i32, i32) {
        match self {
            Self::Up => (0, -1),
            Self::Right => (1, 0),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
        }
    }
}

// Shader values: owned land, valid whole foundation, occupied physical cell.
const SITE_OWN_LAND: u32 = 1;
const SITE_VALID: u32 = 2;
const SITE_OCCUPIED: u32 = 4;

pub(crate) fn visible_placement_bounds(
    map: (u32, u32),
    camera: [f32; 2],
    zoom: f32,
    screen: [f32; 2],
) -> [u32; 4] {
    if !zoom.is_finite() || zoom <= 0.0 {
        return [0; 4];
    }
    [
        ((-camera[0] / zoom).floor() - 1.0).clamp(0.0, map.0 as f32) as u32,
        ((-camera[1] / zoom).floor() - 1.0).clamp(0.0, map.1 as f32) as u32,
        (((screen[0] - camera[0]) / zoom).ceil() + 1.0).clamp(0.0, map.0 as f32) as u32,
        (((screen[1] - camera[1]) / zoom).ceil() + 1.0).clamp(0.0, map.1 as f32) as u32,
    ]
}

const PLACEMENT_CELL_SIZE: u32 = sow_core::building::core::BUILDING_GRID_CELL_SIZE;

#[derive(Default)]
pub struct BuildingPlacementCache {
    tick: Option<u64>,
    map_w: u32,
    map_h: u32,
    grid_w: u32,
    cells: Vec<Vec<usize>>,
    buildings_identity: (usize, usize),
    sites_key: Option<(usize, u64, sow_core::game::BuildingKind, u16, [u32; 4])>,
    sites_buildings: Vec<(u64, u32, sow_core::game::BuildingKind)>,
    sites_bounds: [u32; 4],
    sites: Vec<u32>,
    sites_revision: u64,
}

impl BuildingPlacementCache {
    /// Recompute only visible anchors when their spatial inputs change. Preview
    /// movement and gold do not invalidate this map of possible destinations.
    pub(crate) fn update_visible_sites(
        &mut self,
        engine: &mut sow_core::engine::SowEngine,
        kind: sow_core::game::BuildingKind,
        owner: u16,
        bounds: [u32; 4],
    ) {
        let key = (
            engine.state.map.terrain.as_ptr() as usize,
            engine.state.map.ownership_revision(),
            kind,
            owner,
            bounds,
        );
        if self.sites_key == Some(key)
            && self
                .sites_buildings
                .iter()
                .copied()
                .eq(engine.buildings.iter().map(|b| (b.id, b.tile_idx, b.kind)))
        {
            return;
        }
        engine.refresh_building_grid();
        self.sites_key = Some(key);
        self.sites_buildings.clear();
        self.sites_buildings
            .extend(engine.buildings.iter().map(|b| (b.id, b.tile_idx, b.kind)));
        self.sites_bounds = bounds;
        self.sites.clear();
        let map = &engine.state.map;
        for y in bounds[1]..bounds[3] {
            for x in bounds[0]..bounds[2] {
                let own_land = owner != 0
                    && map.owner_id(x, y) == owner
                    && map.terrain[map.ref_id(x, y)].as_byte() & 0x80 != 0;
                let valid = sow_core::building::validate_structure_site(
                    map,
                    owner,
                    kind,
                    y * map.width + x,
                    &engine.building_grid,
                )
                .is_ok();
                let occupied = engine.building_grid.iter_all_in_range(x, y, 3).any(|b| {
                    sow_core::building::BuildingFootprint::at(b.kind, b.x, b.y)
                        .contains(x as i32, y as i32)
                });
                self.sites.push(
                    u32::from(own_land) * SITE_OWN_LAND
                        | u32::from(valid) * SITE_VALID
                        | u32::from(occupied) * SITE_OCCUPIED,
                );
            }
        }
        self.sites_revision = self.sites_revision.wrapping_add(1);
    }

    pub(crate) fn clear_visible_sites(&mut self) {
        if self.sites_key.take().is_some() {
            self.sites.clear();
            self.sites_bounds = [0; 4];
            self.sites_revision = self.sites_revision.wrapping_add(1);
        }
    }

    pub(crate) fn visible_sites(&self) -> ([u32; 4], &[u32], u64) {
        (self.sites_bounds, &self.sites, self.sites_revision)
    }

    #[cfg(test)]
    pub fn resolve(&mut self, tick: u64, query: &PlacementQuery) -> Result<u32, &'static str> {
        self.rebuild_index(tick, query);
        resolve_building_placement_tile_indexed(query, Some(self))
    }

    pub fn is_legal_site(&mut self, tick: u64, query: &PlacementQuery, tile_idx: u32) -> bool {
        self.rebuild_index(tick, query);
        let Some(map_area) = query.map_w.checked_mul(query.map_h) else {
            return false;
        };
        if tile_idx >= map_area || query.map_w == 0 {
            return false;
        }
        let x = tile_idx % query.map_w;
        let y = tile_idx / query.map_w;
        placement_site_status(query, Some(self), x, y).is_ok()
    }

    fn rebuild_index(&mut self, tick: u64, query: &PlacementQuery) {
        let buildings_identity = (query.buildings.as_ptr() as usize, query.buildings.len());
        if self.tick == Some(tick)
            && self.map_w == query.map_w
            && self.map_h == query.map_h
            && self.buildings_identity == buildings_identity
        {
            return;
        }
        self.tick = Some(tick);
        self.buildings_identity = buildings_identity;
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

fn placement_site_status(
    query: &PlacementQuery,
    index: Option<&BuildingPlacementCache>,
    x: u32,
    y: u32,
) -> Result<(), sow_core::building::PlacementError> {
    sow_core::building::validate_building_site(
        query.kind,
        x,
        y,
        query.map_w,
        query.map_h,
        query.my_id,
        |x, y| {
            query
                .owners
                .get((y * query.map_w + x) as usize)
                .copied()
                .unwrap_or(0)
        },
        |x, y| {
            query
                .terrain
                .get((y * query.map_w + x) as usize)
                .copied()
                .unwrap_or(0)
        },
        |footprint| {
            let blocks = |building: &sow_core::protocol::BuildingSnapshot| {
                footprint.intersects(sow_core::building::BuildingFootprint::at(
                    building.kind,
                    building.tile_idx % query.map_w,
                    building.tile_idx / query.map_w,
                ))
            };
            if let Some(index) = index {
                index.any_nearby(query, x, y, 3, blocks)
            } else {
                query.buildings.iter().any(blocks)
            }
        },
    )
}

pub fn resolve_build_target_tile(query: &PlacementQuery) -> Result<u32, &'static str> {
    resolve_building_placement_tile(query)
}

pub fn resolve_building_placement_tile(query: &PlacementQuery) -> Result<u32, &'static str> {
    resolve_building_placement_tile_indexed(query, None)
}

fn resolve_building_placement_tile_indexed(
    query: &PlacementQuery,
    index: Option<&BuildingPlacementCache>,
) -> Result<u32, &'static str> {
    if query.click_x < 0 || query.click_y < 0 {
        return Err(sow_core::building::PlacementError::OutsideMap.reason_key());
    }
    placement_site_status(query, index, query.click_x as u32, query.click_y as u32)
        .map(|()| query.click_y as u32 * query.map_w + query.click_x as u32)
        .map_err(|error| error.reason_key())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::game::BuildingKind;

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn visible_destinations_match_confirmation_and_invalidate_only_spatial_changes() {
        use sow_core::{
            engine::SowEngine, game::GameState, game_config::GameConfig, map::MapTile,
            water_components::WaterComponents,
        };
        let mut state = GameState::new(42, 32, 32, GameConfig::default());
        for y in 0..32 {
            for x in 0..32 {
                state.map.set_owner_id(x, y, 1);
            }
        }
        for y in 0..32 {
            state.map.terrain[(y * 32) as usize] = MapTile::from_byte(0);
        }
        state.map.terrain[8 * 32 + 8] = MapTile::from_byte(0x80 | 12);
        let mut engine = SowEngine::new(state, WaterComponents::default());
        let mut cache = BuildingPlacementCache::default();
        let bounds = [0, 0, 32, 32];
        for kind in BuildingKind::ALL {
            cache.update_visible_sites(&mut engine, kind, 1, bounds);
            let (_, sites, _) = cache.visible_sites();
            for (tile, site) in sites.iter().enumerate() {
                assert_eq!(
                    site & SITE_VALID != 0,
                    sow_core::building::validate_structure_site(
                        &engine.state.map,
                        1,
                        kind,
                        tile as u32,
                        &engine.building_grid
                    )
                    .is_ok(),
                    "visible destination and confirmation differ at {tile}"
                );
            }
        }
        cache.update_visible_sites(&mut engine, BuildingKind::City, 1, bounds);
        let revision = cache.visible_sites().2;
        engine.state.tick += 1;
        cache.update_visible_sites(&mut engine, BuildingKind::City, 1, bounds);
        assert_eq!(
            cache.visible_sites().2,
            revision,
            "time alone does not rebuild destinations"
        );
        engine.state.map.set_owner_id(24, 24, 2);
        cache.update_visible_sites(&mut engine, BuildingKind::City, 1, bounds);
        assert_ne!(
            cache.visible_sites().2,
            revision,
            "ownership invalidates within the same tick"
        );
        assert_eq!(cache.visible_sites().1[24 * 32 + 24] & SITE_VALID, 0);
        engine.buildings.push(sow_core::building::Building {
            id: 1,
            owner_id: 1,
            tile_idx: 16 * 32 + 16,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.building_grid.mark_dirty();
        cache.update_visible_sites(&mut engine, BuildingKind::City, 1, bounds);
        let sites = cache.visible_sites().1;
        assert_eq!(
            sites[16 * 32 + 17] & SITE_VALID,
            0,
            "partial overlap is blocked"
        );
        assert_ne!(
            sites[16 * 32 + 20] & SITE_VALID,
            0,
            "touching edges remain legal"
        );
        assert_ne!(sites[16 * 32 + 16] & SITE_OCCUPIED, 0);
        let cropped = visible_placement_bounds((32, 32), [-96.0, -96.0], 12.0, [48.0, 48.0]);
        assert_eq!(cropped, [7, 7, 13, 13]);
        cache.update_visible_sites(&mut engine, BuildingKind::Farm, 1, cropped);
        assert_eq!(
            cache.visible_sites().1.len(),
            36,
            "only visible anchors are computed"
        );
        cache.clear_visible_sites();
        assert!(cache.visible_sites().1.is_empty());
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
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

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn tutorial_sites_require_owned_land_and_clear_placement_space() {
        let (map_w, map_h, my_id) = (64, 64, 1);
        let owners = vec![my_id; (map_w * map_h) as usize];
        let terrain = vec![0x80; (map_w * map_h) as usize];
        let buildings = [sow_core::protocol::BuildingSnapshot {
            id: 1,
            owner_id: my_id,
            tile_idx: 10 * map_w + 10,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        }];
        let query = PlacementQuery {
            kind: BuildingKind::City,
            click_x: 40,
            click_y: 40,
            map_w,
            map_h,
            owners: &owners,
            terrain: &terrain,
            my_id,
            buildings: &buildings,
        };
        let legal_site = 40 * map_w + 40;
        let adjacent_site = 14 * map_w + 10;
        let occupied_site = buildings[0].tile_idx;
        let mut cache = BuildingPlacementCache::default();

        assert!(cache.is_legal_site(1, &query, legal_site));
        assert_eq!(cache.resolve(1, &query), Ok(legal_site));
        assert!(!cache.is_legal_site(1, &query, occupied_site));
        assert!(cache.is_legal_site(1, &query, adjacent_site));

        let mut foreign_owners = owners.clone();
        foreign_owners[legal_site as usize] = 2;
        let foreign_site = PlacementQuery {
            owners: &foreign_owners,
            ..query
        };
        assert!(!cache.is_legal_site(1, &foreign_site, legal_site));

        let mut water_terrain = terrain.clone();
        water_terrain[legal_site as usize] = 0;
        let water_site = PlacementQuery {
            terrain: &water_terrain,
            ..query
        };
        assert!(!cache.is_legal_site(1, &water_site, legal_site));
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn preview_observes_same_tick_ownership_and_building_changes() {
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
        assert_eq!(cache.resolve(10, &next), Err("hud.build_occupied"));
        let mut foreign_owners = owners.clone();
        foreign_owners[(12 * map_w + 12) as usize] = 2;
        let foreign = PlacementQuery {
            click_x: 12,
            click_y: 12,
            owners: &foreign_owners,
            ..next
        };
        assert_eq!(cache.resolve(10, &foreign), Err("hud.build_owned_land"));
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
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
