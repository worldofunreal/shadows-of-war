fn apply_ocean_and_shoreline(grid: &mut [MapTile], width: u32, height: u32) {
    let w = width as usize;
    let h = height as usize;
    if grid.is_empty() {
        return;
    }

    let mut visited = vec![false; grid.len()];
    let mut water_bodies: Vec<Vec<usize>> = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if visited[idx] || grid[idx].is_land() {
                continue;
            }
            let mut body = Vec::new();
            let mut stack = vec![idx];
            visited[idx] = true;
            while let Some(cur) = stack.pop() {
                body.push(cur);
                let cx = cur % w;
                let cy = cur / w;
                for (dx, dy) in [(0i32, 1), (1, 0), (0, -1), (-1, 0)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let ni = ny as usize * w + nx as usize;
                    if !visited[ni] && !grid[ni].is_land() {
                        visited[ni] = true;
                        stack.push(ni);
                    }
                }
            }
            water_bodies.push(body);
        }
    }

    water_bodies.sort_by_key(|b| std::cmp::Reverse(b.len()));
    if let Some(largest) = water_bodies.first() {
        for &idx in largest {
            let mut byte = grid[idx].as_byte();
            byte |= OCEAN_WATER;
            grid[idx] = MapTile::from_byte(byte);
        }
    }

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let is_land = grid[idx].is_land();
            let mut is_shore = false;
            for (dx, dy) in [(0i32, 1), (1, 0), (0, -1), (-1, 0)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    is_shore = true;
                    break;
                }
                let ni = ny as usize * w + nx as usize;
                if grid[ni].is_land() != is_land {
                    is_shore = true;
                    break;
                }
            }
            if is_shore {
                let mut byte = grid[idx].as_byte();
                byte |= 0b01000000;
                grid[idx] = MapTile::from_byte(byte);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_polygon_marks_interior() {
        let width = 20u32;
        let height = 20u32;
        let mut grid = vec![MapTile::from_byte(OCEAN_WATER); (width * height) as usize];
        let tri = vec![(5.0, 5.0), (15.0, 5.0), (10.0, 15.0)];
        fill_polygon(width, height, &tri, |idx| {
            grid[idx] = MapTile::from_byte(LAND_PLAINS);
        });
        assert!(grid[(10 * width + 10) as usize].is_land());
        assert!(!grid[0].is_land());
    }

    #[test]
    fn water_polygon_over_land() {
        let width = 30u32;
        let height = 30u32;
        let mut grid = vec![MapTile::from_byte(LAND_PLAINS); (width * height) as usize];
        let pond = vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)];
        fill_polygon(width, height, &pond, |idx| {
            grid[idx] = MapTile::from_byte(PURE_WATER);
        });
        assert!(grid[(15 * width + 15) as usize].is_water());
        assert!(grid[(2 * width + 2) as usize].is_land());
    }

    #[test]
    fn coast_loop_with_land_on_its_left_encloses_land() {
        let width = 40u32;
        let height = 40u32;
        let mut barriers = vec![false; (width * height) as usize];

        // Walked so that the square's inside is on the left, as OSM coastlines are.
        let coast = vec![
            (30.0, 10.0),
            (10.0, 10.0),
            (10.0, 30.0),
            (30.0, 30.0),
            (30.0, 10.0),
        ];
        rasterize_polyline_barrier(&mut barriers, width, height, &coast, 5.0);
        dilate_barriers(&mut barriers, width, height);

        let land = classify_land_by_coast_side(&[coast], &barriers, width, height);

        assert!(land[(20 * width + 20) as usize]);
        assert!(!land[0]);
        assert!(!land[(39 * width + 39) as usize]);
    }

    #[test]
    fn coast_crossing_the_box_puts_land_on_its_left() {
        let width = 40u32;
        let height = 40u32;
        let mut barriers = vec![false; (width * height) as usize];

        // Travelling east with land on the left puts the land north of the coast.
        let coast = vec![(0.0, 20.0), (39.0, 20.0)];
        rasterize_polyline_barrier(&mut barriers, width, height, &coast, 5.0);
        dilate_barriers(&mut barriers, width, height);

        let land = classify_land_by_coast_side(&[coast], &barriers, width, height);

        assert!(land[(5 * width + 20) as usize]);
        assert!(!land[(35 * width + 20) as usize]);
    }

    #[test]
    fn no_coastline_means_no_land_from_coast_votes() {
        let barriers = vec![false; 400];
        let land = classify_land_by_coast_side(&[], &barriers, 20, 20);
        assert!(land.iter().all(|&is_land| !is_land));
    }
}
