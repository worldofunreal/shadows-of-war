#[cfg(test)]
mod placement_tests {
    use crate::input::placement::*;
    use sow_core::game::BuildingKind;
    use sow_core::protocol::BuildingSnapshot;
    fn land_terrain() -> Vec<u8> {
        vec![0x80; 32 * 32]
    }

    fn owned_map(w: u32, h: u32, owner: u16) -> Vec<u16> {
        vec![owner; (w * h) as usize]
    }

    fn city_snapshot(id: u64, owner: u16, tile_idx: u32) -> BuildingSnapshot {
        BuildingSnapshot {
            id,
            owner_id: owner,
            tile_idx,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        }
    }

    #[test]
    fn click_on_city_resolves_to_another_valid_site() {
        let map_w = 32u32;
        let map_h = 32u32;
        let my_id = 1u16;
        let city_tile = 10 * map_w + 10;
        let buildings = vec![city_snapshot(1, my_id, city_tile)];
        let owners = owned_map(map_w, map_h, my_id);
        let terrain = land_terrain();

        let resolved = resolve_build_target_tile(&PlacementQuery {
            kind: BuildingKind::City,
            click_x: 10,
            click_y: 10,
            map_w,
            map_h,
            owners: &owners,
            terrain: &terrain,
            my_id,
            buildings: &buildings,
        })
        .expect("click near a city should find an open site");

        assert_ne!(resolved, city_tile);
    }

    #[test]
    fn click_far_from_city_snaps_to_spawn_tile() {
        let map_w = 32u32;
        let map_h = 32u32;
        let my_id = 1u16;
        let city_tile = 5 * map_w + 5;
        let buildings = vec![city_snapshot(1, my_id, city_tile)];
        let owners = owned_map(map_w, map_h, my_id);
        let terrain = land_terrain();

        let resolved = resolve_build_target_tile(&PlacementQuery {
            kind: BuildingKind::City,
            click_x: 20,
            click_y: 20,
            map_w,
            map_h,
            owners: &owners,
            terrain: &terrain,
            my_id,
            buildings: &buildings,
        })
        .expect("click far from city should find spawn tile");

        assert_ne!(resolved, city_tile);
    }
}

#[cfg(test)]
mod camera_offset_tests {
    use crate::input::surface::clamp_camera_offset;

    #[test]
    fn focus_offset_obeys_map_edges_and_centers_small_maps() {
        assert_eq!(clamp_camera_offset(100.0, 300.0, -250.0), -200.0);
        assert_eq!(clamp_camera_offset(100.0, 300.0, 20.0), 0.0);
        assert_eq!(clamp_camera_offset(100.0, 50.0, 20.0), 25.0);
    }
}
