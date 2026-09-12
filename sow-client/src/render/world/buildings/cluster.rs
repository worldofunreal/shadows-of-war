use super::metrics::BuildingLod;
use crate::render::world::movers::tile_to_world;
use sow_core::game::BuildingKind;

pub(super) struct RenderedBuilding {
    pub bx: f32,
    pub by: f32,
    pub kind: BuildingKind,
    pub active_level: u8,
    pub target_level: u8,
    pub under_construction: bool,
    pub ticks_until_complete: u32,
    pub count: usize,
    pub owner_id: u16,
    pub id: Option<u64>,
    pub modules: Option<sow_core::building::CityModules>,
    pub tile_idx: Option<u32>,
}

pub(super) fn collect_rendered_buildings(
    snap: &sow_core::protocol::SimSnapshot,
    map_w: u32,
    lod: BuildingLod,
) -> Vec<RenderedBuilding> {
    let cell_size = lod.cluster_cell_size;

    let building_count = snap.buildings.len();
    let mut rendered_buildings = Vec::with_capacity(building_count);

    if cell_size > 1.0 {
        #[derive(Hash, PartialEq, Eq)]
        struct ClusterKey {
            grid_x: i32,
            grid_y: i32,
            owner_id: u16,
            kind: BuildingKind,
            level: u8,
        }
        let mut clusters: std::collections::HashMap<ClusterKey, (f32, f32, usize)> =
            std::collections::HashMap::with_capacity(building_count / 4);

        for b in &snap.buildings {
            let (bx, by) = tile_to_world(b.tile_idx, map_w);
            let tile_x = (b.tile_idx % map_w) as f32;
            let tile_y = (b.tile_idx / map_w) as f32;

            let grid_x = (tile_x / cell_size) as i32;
            let grid_y = (tile_y / cell_size) as i32;
            let display_level = b.active_level();

            let key = ClusterKey {
                grid_x,
                grid_y,
                owner_id: b.owner_id,
                kind: b.kind,
                level: display_level,
            };

            let entry = clusters.entry(key).or_insert((0.0, 0.0, 0));
            entry.0 += bx;
            entry.1 += by;
            entry.2 += 1;
        }

        for (key, (sum_bx, sum_by, count)) in clusters {
            rendered_buildings.push(RenderedBuilding {
                bx: sum_bx / count as f32,
                by: sum_by / count as f32,
                kind: key.kind,
                active_level: key.level,
                target_level: key.level,
                under_construction: false,
                ticks_until_complete: 0,
                count,
                owner_id: key.owner_id,
                id: None,
                modules: None,
                tile_idx: None,
            });
        }
    } else {
        for b in &snap.buildings {
            let (bx, by) = tile_to_world(b.tile_idx, map_w);
            rendered_buildings.push(RenderedBuilding {
                bx,
                by,
                kind: b.kind,
                active_level: b.active_level(),
                target_level: b.level,
                under_construction: b.under_construction,
                ticks_until_complete: b.ticks_until_complete,
                count: 1,
                owner_id: b.owner_id,
                id: Some(b.id),
                modules: Some(b.modules),
                tile_idx: Some(b.tile_idx),
            });
        }
    }

    // Depth sort bottom-to-top (and left-to-right) to make overlaps completely stable and prevent flickering

    rendered_buildings
}
