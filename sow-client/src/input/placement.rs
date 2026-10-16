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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Build-mode controls in physical px, one source for drawing and tapping so a
/// drawn button and its tap area cannot drift apart. Arrows are a drag hint
/// beside the foundation; cancel and confirm sit above it, clear of the finger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlacementControls {
    pub center: [f32; 2],
    /// Half side of the square that starts a drag (never below a 48px target).
    pub grab_half: f32,
    pub arrow: f32,
    pub arrows: [(PlacementDirection, [f32; 2]); 4],
    pub button: f32,
    pub cancel: [f32; 2],
    pub confirm: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlacementButton {
    Cancel,
    Confirm,
}

// ponytail: fixed top inset for the HUD top bar; read the bar's real height if it grows.
const PLACEMENT_TOP_INSET: f32 = 64.0;

pub(crate) fn placement_controls(
    kind: sow_core::game::BuildingKind,
    tile_idx: u32,
    map_w: u32,
    camera: [f32; 2],
    zoom: f32,
    sf: f32,
) -> PlacementControls {
    let map_w = map_w.max(1);
    let footprint =
        sow_core::building::BuildingFootprint::at(kind, tile_idx % map_w, tile_idx / map_w);
    let center = [
        camera[0] + (footprint.left as f32 + footprint.width as f32 * 0.5) * zoom,
        camera[1] + (footprint.top as f32 + footprint.height as f32 * 0.5) * zoom,
    ];
    let grab_half = (footprint.width.max(footprint.height) as f32 * 0.5 * zoom).max(24.0 * sf);
    let arrow = 28.0 * sf;
    let button = 52.0 * sf;
    let gap = 6.0 * sf;
    let arrow_offset = grab_half + gap + arrow * 0.5;
    let row_offset = grab_half + gap + arrow + 2.0 * gap + button * 0.5;
    // No room above under the top bar: the row flips below the foundation.
    let row_y = if center[1] - row_offset - button * 0.5 >= PLACEMENT_TOP_INSET * sf {
        center[1] - row_offset
    } else {
        center[1] + row_offset
    };
    let row_dx = button * 0.5 + 2.0 * gap;
    PlacementControls {
        center,
        grab_half,
        arrow,
        arrows: [
            (
                PlacementDirection::Up,
                [center[0], center[1] - arrow_offset],
            ),
            (
                PlacementDirection::Right,
                [center[0] + arrow_offset, center[1]],
            ),
            (
                PlacementDirection::Down,
                [center[0], center[1] + arrow_offset],
            ),
            (
                PlacementDirection::Left,
                [center[0] - arrow_offset, center[1]],
            ),
        ],
        button,
        cancel: [center[0] - row_dx, row_y],
        confirm: [center[0] + row_dx, row_y],
    }
}

impl PlacementControls {
    pub(crate) fn button_at(&self, x: f32, y: f32) -> Option<PlacementButton> {
        let half = self.button * 0.5;
        [
            (PlacementButton::Cancel, self.cancel),
            (PlacementButton::Confirm, self.confirm),
        ]
        .into_iter()
        .find(|(_, [cx, cy])| (x - cx).abs() <= half && (y - cy).abs() <= half)
        .map(|(button, _)| button)
    }

    pub(crate) fn grabs(&self, x: f32, y: f32) -> bool {
        (x - self.center[0]).abs() <= self.grab_half && (y - self.center[1]).abs() <= self.grab_half
    }
}

impl crate::app::SowApp {
    /// Controls for the live (not yet submitted) preview at the current camera.
    pub(crate) fn placement_controls(&self) -> Option<PlacementControls> {
        let placement = self
            .ui
            .app
            .hud_state
            .building_placement
            .filter(|p| !p.pending)?;
        Some(placement_controls(
            placement.kind,
            placement.tile_idx,
            self.sim.map_w,
            [self.input.camera_x, self.input.camera_y],
            self.input.camera_zoom,
            crate::web_canvas::device_pixel_ratio() as f32,
        ))
    }
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
}

impl BuildingPlacementCache {
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
    fn controls_sit_above_the_foundation_and_hit_test_what_is_drawn() {
        for sf in [1.0, 2.0, 3.0] {
            for zoom in [2.0 * sf, 12.0 * sf, 40.0 * sf] {
                let camera = 400.0 * sf - 50.0 * zoom;
                let c = placement_controls(
                    BuildingKind::City,
                    50 * 100 + 50,
                    100,
                    [camera, camera],
                    zoom,
                    sf,
                );
                let row_top = c.cancel[1] - c.button * 0.5;
                assert!(row_top >= PLACEMENT_TOP_INSET * sf || c.cancel[1] > c.center[1]);
                assert_eq!(c.cancel[1], c.confirm[1]);
                assert!(c.cancel[0] < c.center[0] && c.confirm[0] > c.center[0]);
                assert_eq!(
                    c.button_at(c.confirm[0], c.confirm[1]),
                    Some(PlacementButton::Confirm)
                );
                assert_eq!(
                    c.button_at(c.cancel[0], c.cancel[1]),
                    Some(PlacementButton::Cancel)
                );
                assert_eq!(c.button_at(c.center[0], c.center[1]), None);
                assert!(c.grabs(c.center[0], c.center[1]));
                for (_, arrow) in c.arrows {
                    assert_eq!(
                        c.button_at(arrow[0], arrow[1]),
                        None,
                        "arrows are only a hint"
                    );
                    assert!(!c.grabs(arrow[0], arrow[1]));
                }
                assert!(
                    !c.grabs(c.confirm[0], c.confirm[1]),
                    "a button tap never drags"
                );
            }
        }
        let top = placement_controls(BuildingKind::City, 2 * 100 + 50, 100, [0.0, 0.0], 12.0, 1.0);
        assert!(
            top.confirm[1] > top.center[1],
            "no room above: the row flips below"
        );
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
