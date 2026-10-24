pub mod construction;
pub mod core;
pub mod cost;
pub mod placement;

pub use core::*;
pub use cost::*;
pub use placement::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::BuildingKind;
    use crate::map::{GameMap, MapTile};
    use std::time::Instant;

    fn tiny_owned_map() -> (GameMap, u16) {
        let mut m = GameMap::new(5, 1);
        let owner = 1u16;
        for x in 0..5 {
            m.set_owner_id(x, 0, owner);
            let ri = m.ref_id(x, 0);
            m.terrain[ri] = MapTile::from_byte(0b1000_0000); // land
        }
        // Shore at ends for port tests
        let r0 = m.ref_id(0, 0);
        m.terrain[r0] = MapTile::from_byte(0b1100_0000);
        let r4 = m.ref_id(4, 0);
        m.terrain[r4] = MapTile::from_byte(0b1100_0000);
        (m, owner)
    }

    struct LegacyDefenseGrid {
        cells: Vec<Vec<Building>>,
        grid_w: u32,
        grid_h: u32,
        cell_size: u32,
    }

    fn legacy_defense_grid(
        buildings: &[Building],
        map_width: u32,
        map_height: u32,
        cell_size: u32,
    ) -> LegacyDefenseGrid {
        let grid_w = map_width.div_ceil(cell_size);
        let grid_h = map_height.div_ceil(cell_size);
        let mut cells = vec![Vec::new(); (grid_w * grid_h) as usize];
        for building in buildings {
            if building.kind == BuildingKind::Tower && building.active_level() > 0 {
                let x = building.tile_idx % map_width;
                let y = building.tile_idx / map_width;
                cells[((y / cell_size) * grid_w + x / cell_size) as usize].push(*building);
            }
        }
        LegacyDefenseGrid {
            cells,
            grid_w,
            grid_h,
            cell_size,
        }
    }

    fn legacy_priority_bonus(
        grid: &LegacyDefenseGrid,
        tile_x: u32,
        tile_y: u32,
        map_width: u32,
        target_owner: u16,
        config: &crate::game_config::GameConfig,
    ) -> i64 {
        let mut bonus = 0;
        let max_range = 24;
        let cx_min = tile_x.saturating_sub(max_range) / grid.cell_size;
        let cx_max = ((tile_x + max_range) / grid.cell_size).min(grid.grid_w - 1);
        let cy_min = tile_y.saturating_sub(max_range) / grid.cell_size;
        let cy_max = ((tile_y + max_range) / grid.cell_size).min(grid.grid_h - 1);
        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                for building in &grid.cells[(cy * grid.grid_w + cx) as usize] {
                    if building.owner_id != target_owner {
                        continue;
                    }
                    let bx = building.tile_idx % map_width;
                    let by = building.tile_idx / map_width;
                    if crate::building::hex_distance(
                        tile_x as i32,
                        tile_y as i32,
                        bx as i32,
                        by as i32,
                    ) <= building.defense_range_cfg(config)
                    {
                        bonus += config.tower_priority as i64 * building.active_level() as i64;
                    }
                }
            }
        }
        bonus
    }

    fn measure_defense_tick(
        legacy_grid: &LegacyDefenseGrid,
        grid: &DefenseGrid,
        attacks: &[(u32, u32, u16, u16)],
        config: &crate::game_config::GameConfig,
        legacy: bool,
    ) -> u64 {
        let mut checksum = 0u64;
        for &(x, y, attacker_id, target_owner) in attacks {
            if legacy {
                // The old capture path queried the same tile for cost and losses.
                checksum = checksum.wrapping_add(legacy_priority_bonus(
                    legacy_grid,
                    x,
                    y,
                    512,
                    target_owner,
                    config,
                ) as u64);
                checksum = checksum.wrapping_add(legacy_priority_bonus(
                    legacy_grid,
                    x,
                    y,
                    512,
                    target_owner,
                    config,
                ) as u64);
            } else {
                let defense = grid.influence(x, y, 512, target_owner, attacker_id, 7, config);
                checksum = checksum.wrapping_add(std::hint::black_box(
                    defense.capture_multiplier.to_bits()
                        ^ defense.attacker_loss_multiplier.to_bits(),
                ));
            }

            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
                let nx = (x as i32 + dx) as u32;
                let ny = (y as i32 + dy) as u32;
                let bonus = if legacy {
                    legacy_priority_bonus(legacy_grid, nx, ny, 512, target_owner, config) as u64
                } else {
                    grid.priority_bonus(nx, ny, 512, target_owner, attacker_id, 7, config) as u64
                };
                checksum = checksum.wrapping_add(std::hint::black_box(bonus));
            }
        }
        std::hint::black_box(checksum)
    }

    fn compare_defense_tick(
        legacy_grid: &LegacyDefenseGrid,
        grid: &DefenseGrid,
        attacks: &[(u32, u32, u16, u16)],
        config: &crate::game_config::GameConfig,
    ) -> (u128, u128) {
        let mut old_times = Vec::with_capacity(3);
        let mut new_times = Vec::with_capacity(3);
        for run in 0..3 {
            for legacy in [run % 2 == 0, run % 2 != 0] {
                let start = Instant::now();
                measure_defense_tick(legacy_grid, grid, attacks, config, legacy);
                let elapsed = start.elapsed().as_nanos();
                if legacy {
                    old_times.push(elapsed);
                } else {
                    new_times.push(elapsed);
                }
            }
        }
        old_times.sort_unstable();
        new_times.sort_unstable();
        (old_times[1], new_times[1])
    }

    fn defense_benchmark_case(
        tower_count: usize,
        attack_count: usize,
        distributed: bool,
    ) -> (LegacyDefenseGrid, DefenseGrid, Vec<(u32, u32, u16, u16)>) {
        let mut buildings = Vec::with_capacity(tower_count);
        let mut owner_tiles = vec![Vec::new(); 257];
        for i in 0..tower_count {
            let owner_id = 2 + (i % 255) as u16;
            let (x, y) = if distributed {
                (8 + (i as u32 % 32) * 15, 8 + (i as u32 / 32) * 15)
            } else {
                (240 + i as u32 % 32, 240 + i as u32 / 32)
            };
            let tile_idx = y * 512 + x;
            buildings.push(Building {
                id: i as u64 + 1,
                owner_id,
                tile_idx,
                kind: BuildingKind::Tower,
                level: 1 + (i % 4) as u8,
                under_construction: false,
                ticks_until_complete: 0,
            });
            owner_tiles[owner_id as usize].push((x, y));
        }

        let config = crate::game_config::GameConfig::default();
        let legacy_grid = legacy_defense_grid(&buildings, 512, 512, DEFENSE_GRID_CELL_SIZE);
        let mut grid = DefenseGrid::default();
        grid.rebuild(&buildings, 512, 512, DEFENSE_GRID_CELL_SIZE, &config);
        let attacks = (0..attack_count)
            .map(|i| {
                let attacker_id = 1 + (i % 255) as u16;
                let target_owner = attacker_id + 1;
                let tiles = &owner_tiles[target_owner as usize];
                let (x, y) = tiles[(i / 255) % tiles.len()];
                (x, y, attacker_id, target_owner)
            })
            .collect();
        (legacy_grid, grid, attacks)
    }

    #[test]
    fn valid_land_center_click() {
        let owner = 1u16;
        let mut map = GameMap::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                map.set_owner_id(x, y, owner);
            }
        }
        let grid = BuildingGrid::rebuild_empty(map.width, map.height);
        let mut scratch = crate::engine::PlacementScratch::default();
        let click = 8 * map.width + 8;
        let v = valid_land_structure_indices(
            &map,
            owner,
            click,
            BuildingKind::City,
            &grid,
            &mut scratch,
        );
        assert!(v.contains(&click));
    }

    #[test]
    fn nearest_structure_site_only_returns_legal_closest_foundations() {
        let (w, h, owner) = (32u32, 32u32, 1u16);
        let mut map = GameMap::new(w, h);
        for y in 0..h {
            for x in 0..w {
                map.set_owner_id(x, y, if x < 24 { owner } else { 2 });
            }
        }
        // Water in the first two columns gives ports a coast.
        for y in 0..h {
            for x in 0..2 {
                map.terrain[(y * w + x) as usize] = crate::map::MapTile::from_byte(0);
            }
        }
        let mut grid = BuildingGrid::default();
        grid.rebuild_from_pairs(w, h, &[(12, 16)]);
        for (kind, x, y) in [
            (BuildingKind::City, 12, 16),
            (BuildingKind::City, 28, 16),
            (BuildingKind::Port, 16, 16),
            (BuildingKind::Farm, 0, 0),
        ] {
            let site = nearest_structure_site(&map, owner, kind, x, y, 12, &grid)
                .unwrap_or_else(|| panic!("{kind:?} near ({x}, {y})"));
            assert!(validate_structure_site(&map, owner, kind, site, &grid).is_ok());
            let (sx, sy) = idx_xy(site, w);
            let best = euclid_sq(sx as i64, sy as i64, x as i64, y as i64);
            for tile in 0..w * h {
                let (tx, ty) = idx_xy(tile, w);
                if euclid_sq(tx as i64, ty as i64, x as i64, y as i64) < best {
                    assert!(
                        validate_structure_site(&map, owner, kind, tile, &grid).is_err(),
                        "{kind:?}: closer legal site {tile} skipped"
                    );
                }
            }
        }
        assert_eq!(
            nearest_structure_site(&map, owner, BuildingKind::City, 30, 16, 2, &grid),
            None,
            "foreign land has no site within the radius"
        );
    }

    #[test]
    fn adjoining_city_is_a_valid_site() {
        let (w, h) = (32u32, 32u32);
        let mut m = GameMap::new(w, h);
        let owner = 1u16;
        for y in 0..h {
            for x in 0..w {
                m.set_owner_id(x, y, owner);
            }
        }
        let click = 16 * w + 20;
        let mut grid = BuildingGrid::default();
        grid.rebuild_from_pairs(w, h, &[(15, 16)]);
        let mut scratch = crate::engine::PlacementScratch::default();
        let v =
            valid_land_structure_indices(&m, owner, click, BuildingKind::City, &grid, &mut scratch);
        assert!(v.contains(&click));
        assert!(!v.is_empty());
    }

    #[test]
    fn building_footprints_are_fixed_and_anchor_centered() {
        for kind in BuildingKind::ALL {
            assert_eq!(kind.footprint_dimensions(), (4, 4), "{kind:?}");
        }

        let city = BuildingFootprint::at(BuildingKind::City, 10, 10);
        assert_eq!((city.left, city.top, city.width, city.height), (8, 8, 4, 4));
        assert!(city.contains(10, 10));
        assert!(!city.contains(12, 10));
        for kind in BuildingKind::ALL {
            assert!(
                city.intersects(BuildingFootprint::at(kind, 8, 8)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn historical_recordings_keep_their_spacing_rules() {
        for kind in BuildingKind::ALL {
            assert_eq!(legacy_minimum_building_spacing(BuildingKind::City, kind), 6);
            assert_eq!(legacy_minimum_building_spacing(kind, BuildingKind::City), 6);
        }
        for a in BuildingKind::ALL
            .into_iter()
            .filter(|kind| *kind != BuildingKind::City)
        {
            for b in BuildingKind::ALL
                .into_iter()
                .filter(|kind| *kind != BuildingKind::City)
            {
                assert_eq!(legacy_minimum_building_spacing(a, b), 4);
            }
        }
    }

    #[test]
    fn building_grid_insert_keeps_kind_and_avoids_rebuild() {
        let mut grid = BuildingGrid::rebuild_empty(32, 32);
        grid.insert(10 * 32 + 12, BuildingKind::Factory, 32, 32);
        assert!(!grid.dirty);
        let entry = grid.iter_all_in_range(12, 10, 0).next().unwrap();
        assert_eq!(
            (entry.x, entry.y, entry.kind),
            (12, 10, BuildingKind::Factory)
        );
    }

    #[test]
    fn city_candidate_requires_every_footprint_tile() {
        let (w, h, owner) = (24u32, 24u32, 1u16);
        let mut map = GameMap::new(w, h);
        for y in 0..h {
            for x in 0..w {
                map.set_owner_id(x, y, owner);
            }
        }
        map.set_owner_id(9, 9, 0);
        let grid = BuildingGrid::rebuild_empty(w, h);
        let click = 10 * w + 10;
        let mut scratch = crate::engine::PlacementScratch::default();
        let choices = valid_land_structure_indices(
            &map,
            owner,
            click,
            BuildingKind::City,
            &grid,
            &mut scratch,
        );
        assert!(!choices.contains(&click));
    }

    #[test]
    fn port_requires_water_adjacent_to_its_footprint() {
        let (w, h, owner) = (24u32, 24u32, 1u16);
        let mut map = GameMap::new(w, h);
        for y in 0..h {
            for x in 0..w {
                map.set_owner_id(x, y, owner);
            }
        }
        let click = 10 * w + 10;
        let grid = BuildingGrid::rebuild_empty(w, h);
        let mut scratch = crate::engine::PlacementScratch::default();
        assert!(
            !valid_land_structure_indices(
                &map,
                owner,
                click,
                BuildingKind::Port,
                &grid,
                &mut scratch,
            )
            .contains(&click)
        );

        let water = map.ref_id(7, 9);
        map.terrain[water] = MapTile::from_byte(0);
        let choices = valid_land_structure_indices(
            &map,
            owner,
            click,
            BuildingKind::Port,
            &grid,
            &mut scratch,
        );
        assert!(choices.contains(&click));
    }

    #[test]
    fn direct_structure_choice_accepts_only_the_exact_site() {
        let (w, h) = (48u32, 40u32);
        let mut map = GameMap::new(w, h);
        for y in 0..h {
            for x in 0..w {
                map.set_owner_id(x, y, if x < 40 { 1 } else { 0 });
            }
        }
        for y in 0..h {
            let idx = map.ref_id(24, y);
            map.terrain[idx] = MapTile::from_byte(0);
        }
        let existing = [
            Building {
                id: 1,
                owner_id: 1,
                tile_idx: xy_idx(8, 8, w),
                kind: BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
            Building {
                id: 2,
                owner_id: 1,
                tile_idx: xy_idx(14, 8, w),
                kind: BuildingKind::Factory,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
        ];
        let mut grid = BuildingGrid::default();
        grid.rebuild(existing.iter(), w, h);

        for kind in BuildingKind::ALL {
            for &(x, y) in &[
                (2, 2),
                (8, 8),
                (20, 10),
                (23, 20),
                (30, 30),
                (39, 39),
                (42, 10),
            ] {
                let click = xy_idx(x, y, w);
                let expected = if matches!((x, y), (2, 2) | (20, 10) | (30, 30))
                    && kind != BuildingKind::Port
                {
                    Some(click)
                } else {
                    None
                };
                let actual = resolve_structure_spawn_tile(
                    &map,
                    1,
                    kind,
                    click,
                    &grid,
                    &mut crate::engine::PlacementScratch::default(),
                );
                assert_eq!(actual, expected, "kind={kind:?} click=({x},{y})");
            }
        }
    }

    #[test]
    fn every_building_pair_can_touch_edges_and_corners_but_never_overlap() {
        let mut map = GameMap::new(40, 40);
        for y in 0..40 {
            for x in 0..40 {
                map.set_owner_id(x, y, 1);
            }
        }
        for existing_kind in BuildingKind::ALL {
            let building = Building {
                id: 1,
                owner_id: 1,
                tile_idx: xy_idx(16, 16, 40),
                kind: existing_kind,
                level: 1,
                under_construction: true,
                ticks_until_complete: 1,
            };
            let mut grid = BuildingGrid::default();
            grid.rebuild([&building].into_iter(), 40, 40);
            for kind in BuildingKind::ALL {
                for (x, y) in [(12, 16), (20, 16), (16, 12), (16, 20), (12, 12), (20, 20)] {
                    let result = validate_structure_site(&map, 1, kind, xy_idx(x, y, 40), &grid);
                    assert_eq!(
                        result,
                        if kind == BuildingKind::Port {
                            Err(PlacementError::CoastRequired)
                        } else {
                            Ok(())
                        },
                        "{existing_kind:?} / {kind:?} at {x},{y}"
                    );
                }
                for (x, y) in [(13, 16), (19, 16), (16, 13), (16, 19), (19, 19)] {
                    assert_eq!(
                        validate_structure_site(&map, 1, kind, xy_idx(x, y, 40), &grid),
                        Err(PlacementError::Occupied)
                    );
                }
            }
        }
    }

    #[test]
    fn foundation_checks_all_cells_and_ports_require_real_water() {
        let mut map = GameMap::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                map.set_owner_id(x, y, 1);
            }
        }
        let grid = BuildingGrid::rebuild_empty(20, 20);
        let tile = xy_idx(10, 10, 20);
        for kind in BuildingKind::ALL {
            assert_eq!(
                validate_structure_site(&map, 1, kind, 0, &grid),
                Err(PlacementError::OutsideMap)
            );
        }
        let water = map.ref_id(12, 10);
        map.terrain[water] = MapTile::from_byte(0);
        assert_eq!(
            validate_structure_site(&map, 1, BuildingKind::Port, tile, &grid),
            Ok(())
        );
        map.set_owner_id(8, 8, 2);
        assert_eq!(
            validate_structure_site(&map, 1, BuildingKind::City, tile, &grid),
            Err(PlacementError::OwnLand)
        );
        map.set_owner_id(8, 8, 1);
        let high = map.ref_id(8, 8);
        map.terrain[high] = MapTile::from_byte(0x80 | 10);
        assert_eq!(
            validate_structure_site(&map, 1, BuildingKind::Farm, tile, &grid),
            Err(PlacementError::LowlandRequired)
        );
    }

    #[test]
    fn construction_tick_emits_structure_ready() {
        use crate::engine::SowEngine;
        use crate::game::{GameEvent, GamePhase, GameState};
        use crate::player::Player;
        use crate::water_components::WaterComponents;

        let mut game = GameState::new(3, 5, 1, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;
        let (map, owner) = tiny_owned_map();
        game.map = map;
        game.players.push(Player::new_human(
            owner,
            "c".into(),
            [1.0, 0.0, 0.0],
            &crate::game_config::GameConfig::default(),
        ));
        game.player_lookup.resize(owner as usize + 1, None);
        game.player_lookup[owner as usize] = Some(0);
        let water = WaterComponents::compute(&game.map, |_| {});
        let mut engine = SowEngine::new(game, water);
        engine.buildings.push(Building {
            id: 7,
            owner_id: owner,
            tile_idx: 2,
            kind: BuildingKind::City,
            level: 1,
            under_construction: true,
            ticks_until_complete: 1,
        });
        engine.execute_construction();
        assert!(
            engine
                .state
                .events
                .iter()
                .any(|e| matches!(e, GameEvent::StructureReady { id: 7, .. }))
        );
    }

    #[test]
    fn aggregate_ignores_under_construction() {
        let b = [
            Building {
                id: 1,
                owner_id: 1,
                tile_idx: 0,
                kind: BuildingKind::City,
                level: 1,
                under_construction: true,
                ticks_until_complete: 3,
            },
            Building {
                id: 2,
                owner_id: 1,
                tile_idx: 1,
                kind: BuildingKind::City,
                level: 2,
                under_construction: false,
                ticks_until_complete: 0,
            },
        ];
        let aggs = aggregate_buildings_per_player(b.into_iter(), 2);
        assert_eq!(aggs[1].city_levels, 2);
        assert_eq!(aggs[1].count_city, 2);
        assert_eq!(aggs[1].ready_city_count, 1);
    }

    #[test]
    fn hex_distance_differs_from_manhattan_on_diagonal() {
        let d_hex = hex_distance(10, 10, 17, 17);
        let d_man = manhattan(10, 10, 17, 17);
        assert_eq!(d_man, 14);
        assert_eq!(d_hex, 11);
        assert!(d_hex < d_man);
    }

    #[test]
    fn defense_influence_is_seeded_bounded_and_handles_1000_attacks() {
        let width = 128;
        let center_x = 64;
        let center_y = 64;
        let buildings: Vec<_> = (2..=256)
            .map(|owner_id| Building {
                id: u64::from(owner_id),
                owner_id,
                tile_idx: center_y * width + center_x,
                kind: BuildingKind::Tower,
                level: 4,
                under_construction: false,
                ticks_until_complete: 0,
            })
            .collect();
        let mut grid = DefenseGrid::default();
        let mut cfg = crate::game_config::GameConfig::default();
        cfg.tower_range = 18.0;
        grid.rebuild(&buildings, width, width, DEFENSE_GRID_CELL_SIZE, &cfg);

        let first = grid.influence(center_x, center_y, width, 2, 1, 1234, &cfg);
        assert_eq!(
            first,
            grid.influence(center_x, center_y, width, 2, 1, 1234, &cfg)
        );
        assert!((1.0..=4.0).contains(&first.capture_multiplier));
        assert!(first.priority_bonus > 0.0);

        let edge = grid.influence(center_x + 20, center_y, width, 2, 1, 1234, &cfg);
        assert_eq!(edge.capture_multiplier, 1.0);
        assert_eq!(edge.priority_bonus, 0.0);

        let edge_multipliers: Vec<_> = (2..=256)
            .map(|target_owner| {
                grid.influence(center_x + 19, center_y, width, target_owner, 1, 1234, &cfg)
                    .capture_multiplier
            })
            .collect();
        assert!(edge_multipliers.iter().all(|m| (1.0..=1.6).contains(m)));
        let edge_min = edge_multipliers.iter().copied().fold(f64::MAX, f64::min);
        let edge_max = edge_multipliers.iter().copied().fold(f64::MIN, f64::max);
        assert!(edge_min < edge_max);

        let mut min_multiplier = f64::MAX;
        let mut max_multiplier = f64::MIN;
        for attack in 0..1_000 {
            let target_owner = 2 + (attack % 255) as u16;
            let influence = grid.influence(center_x, center_y, width, target_owner, 1, 1234, &cfg);
            assert!((1.0..=4.0).contains(&influence.capture_multiplier));
            assert!(influence.priority_bonus > 0.0);
            min_multiplier = min_multiplier.min(influence.capture_multiplier);
            max_multiplier = max_multiplier.max(influence.capture_multiplier);
        }
        assert!(min_multiplier < max_multiplier);
    }

    #[test]
    #[ignore = "wall-clock benchmark; run explicitly on an idle machine with --ignored --nocapture"]
    fn defense_query_tick_stays_within_5_percent_at_match_limit_and_stress() {
        let config = crate::game_config::GameConfig::default();
        for (name, towers, attacks, distributed) in [
            ("256 participants", 255, 255, true),
            ("1000 grouped", 1_000, 1_000, false),
            ("1000 distributed", 1_000, 1_000, true),
        ] {
            let (legacy_grid, grid, attack_points) =
                defense_benchmark_case(towers, attacks, distributed);
            let (legacy_ns, optimized_ns) =
                compare_defense_tick(&legacy_grid, &grid, &attack_points, &config);
            assert!(legacy_ns > 0);
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "defense combat tick {name}: old={legacy_ns}ns new={optimized_ns}ns"
            )
            .unwrap();
            assert!(
                optimized_ns * 100 <= legacy_ns * 105,
                "{name}: optimized defense combat tick took {optimized_ns}ns; previous path took {legacy_ns}ns"
            );
        }
    }

    #[test]
    fn structure_build_cost_base_and_cap() {
        let cfg = crate::game_config::GameConfig::default();
        let city_base = cfg.cost_city;
        let cap = city_base * cfg.cost_scale_cap_multiplier;

        assert_eq!(
            structure_build_cost_gold(BuildingKind::City, 0, &cfg),
            city_base
        );
        let at_24 = structure_build_cost_gold(BuildingKind::City, 24, &cfg);
        assert!(at_24 < cap);
        assert!(at_24 > city_base);

        for count in [25, 50, 100] {
            assert_eq!(
                structure_build_cost_gold(BuildingKind::City, count, &cfg),
                cap
            );
        }
    }

    #[test]
    fn structure_build_cost_cap_per_kind() {
        let cfg = crate::game_config::GameConfig::default();
        let cases = [
            (BuildingKind::City, cfg.cost_city),
            (BuildingKind::Tower, cfg.cost_tower),
            (BuildingKind::Factory, cfg.cost_factory),
            (BuildingKind::Port, cfg.cost_port),
        ];
        for (kind, base) in cases {
            assert_eq!(structure_build_cost_gold(kind, 0, &cfg), base);
            assert_eq!(
                structure_build_cost_gold(kind, 100, &cfg),
                base * cfg.cost_scale_cap_multiplier
            );
        }
    }
}
