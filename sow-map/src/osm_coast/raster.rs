fn rasterize_polyline_barrier(
    barriers: &mut [bool],
    width: u32,
    height: u32,
    projected: &[(f32, f32)],
    thickness: f32,
) {
    if projected.len() < 2 {
        return;
    }
    for w in 0..projected.len().saturating_sub(1) {
        let (x0, y0) = projected[w];
        let (x1, y1) = projected[w + 1];
        let steps = ((x1 - x0).hypot(y1 - y0).ceil() as i32).max(1);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let cx = x0 + (x1 - x0) * t;
            let cy = y0 + (y1 - y0) * t;
            let r = thickness;
            for dy in -r.ceil() as i32..=r.ceil() as i32 {
                for dx in -r.ceil() as i32..=r.ceil() as i32 {
                    if (dx * dx + dy * dy) as f32 > r * r {
                        continue;
                    }
                    let px = cx.round() as i32 + dx;
                    let py = cy.round() as i32 + dy;
                    if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                        barriers[(py as u32 * width + px as u32) as usize] = true;
                    }
                }
            }
        }
    }
}

fn dilate_barriers(barriers: &mut [bool], width: u32, height: u32) {
    let w = width as usize;
    let h = height as usize;
    let orig = barriers.to_vec();
    for y in 0..h {
        for x in 0..w {
            if !orig[y * w + x] {
                continue;
            }
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 {
                        barriers[ny as usize * w + nx as usize] = true;
                    }
                }
            }
        }
    }
}

// Sample distance from a coast to the land or sea beside it: past its barrier (radius 5 plus 1 tile of dilation).
const COAST_SIDE_OFFSET: f32 = 8.0;

fn neighbors4(idx: usize, w: usize, h: usize) -> impl Iterator<Item = usize> {
    let x = idx % w;
    let y = idx / w;
    [
        (x > 0).then(|| idx - 1),
        (x + 1 < w).then(|| idx + 1),
        (y > 0).then(|| idx - w),
        (y + 1 < h).then(|| idx + w),
    ]
    .into_iter()
    .flatten()
}

// OSM coastlines run with land on their left. Each region between barriers takes the side
// that its coast samples vote for; barrier tiles take the side of the nearest region.
fn classify_land_by_coast_side(
    segments: &[Vec<(f32, f32)>],
    barriers: &[bool],
    width: u32,
    height: u32,
) -> Vec<bool> {
    let w = width as usize;
    let h = height as usize;
    let mut region = vec![u32::MAX; w * h];
    let mut region_count = 0u32;
    let mut stack = Vec::new();
    for start in 0..w * h {
        if barriers[start] || region[start] != u32::MAX {
            continue;
        }
        region[start] = region_count;
        stack.push(start);
        while let Some(cur) = stack.pop() {
            for next in neighbors4(cur, w, h) {
                if !barriers[next] && region[next] == u32::MAX {
                    region[next] = region_count;
                    stack.push(next);
                }
            }
        }
        region_count += 1;
    }

    // (land votes, sea votes) per region.
    let mut votes = vec![(0u32, 0u32); region_count as usize];
    for line in segments {
        for pair in line.windows(2) {
            let (ax, ay) = pair[0];
            let (bx, by) = pair[1];
            let (dx, dy) = (bx - ax, by - ay);
            let length = dx.hypot(dy);
            if length < f32::EPSILON {
                continue;
            }
            // Left of travel, in image space where y points down.
            let (left_x, left_y) = (dy / length, -dx / length);
            let (mid_x, mid_y) = ((ax + bx) * 0.5, (ay + by) * 0.5);
            for (sign, is_land_side) in [(1.0f32, true), (-1.0, false)] {
                let px = (mid_x + left_x * COAST_SIDE_OFFSET * sign).round() as i32;
                let py = (mid_y + left_y * COAST_SIDE_OFFSET * sign).round() as i32;
                if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                    continue;
                }
                let idx = py as usize * w + px as usize;
                if barriers[idx] {
                    continue;
                }
                let (land, sea) = &mut votes[region[idx] as usize];
                if is_land_side {
                    *land += 1;
                } else {
                    *sea += 1;
                }
            }
        }
    }
    let region_is_land: Vec<bool> = votes.iter().map(|(land, sea)| land > sea).collect();

    let mut land = vec![false; w * h];
    let mut known = barriers.iter().map(|&barrier| !barrier).collect::<Vec<bool>>();
    let mut queue = Vec::new();
    for idx in 0..w * h {
        if known[idx] {
            land[idx] = region_is_land[region[idx] as usize];
            queue.push(idx);
        }
    }
    let mut head = 0;
    while head < queue.len() {
        let cur = queue[head];
        head += 1;
        for next in neighbors4(cur, w, h) {
            if !known[next] {
                known[next] = true;
                land[next] = land[cur];
                queue.push(next);
            }
        }
    }
    land
}

fn tags_way_is_coastline(tags: Option<&Value>) -> bool {
    tags.and_then(|t| t.get("natural")).and_then(|v| v.as_str()) == Some("coastline")
}

fn classify_way(tags: Option<&Value>) -> Option<FillKind> {
    let tags = tags?;
    if is_water_tag(tags) {
        return Some(FillKind::Water);
    }
    if is_land_tag(tags) {
        return Some(FillKind::Land);
    }
    None
}

fn classify_relation(tags: Option<&Value>) -> Option<FillKind> {
    let tags = tags?;
    if is_water_tag(tags) {
        return Some(FillKind::Water);
    }
    None
}

fn is_water_tag(tags: &Value) -> bool {
    let natural = tags.get("natural").and_then(|v| v.as_str());
    if matches!(
        natural,
        Some("water") | Some("bay") | Some("strait") | Some("wetland")
    ) {
        return true;
    }
    if tags.get("landuse").and_then(|v| v.as_str()) == Some("water") {
        return true;
    }
    tags.get("waterway").is_some()
}

fn is_land_tag(tags: &Value) -> bool {
    if is_water_tag(tags) {
        return false;
    }
    if tags.get("landuse").is_some() {
        return true;
    }
    let natural = tags.get("natural").and_then(|v| v.as_str());
    matches!(
        natural,
        Some("wood")
            | Some("grassland")
            | Some("scrub")
            | Some("bare_rock")
            | Some("sand")
            | Some("heath")
            | Some("land")
    )
}

fn fill_polygon<F>(width: u32, height: u32, points: &[(f32, f32)], mut set: F)
where
    F: FnMut(usize),
{
    if points.len() < 3 {
        return;
    }
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for &(_, y) in points {
        let yi = y.floor() as i32;
        min_y = min_y.min(yi);
        max_y = max_y.max(yi);
    }
    min_y = min_y.max(0);
    max_y = max_y.min(height as i32 - 1);

    for y in min_y..=max_y {
        let scan_y = y as f32 + 0.5;
        let mut intersections = Vec::new();
        let n = points.len();
        for i in 0..n {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % n];
            if (y0 <= scan_y && y1 > scan_y) || (y1 <= scan_y && y0 > scan_y) {
                let t = (scan_y - y0) / (y1 - y0);
                let x = x0 + t * (x1 - x0);
                intersections.push(x);
            }
        }
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut i = 0;
        while i + 1 < intersections.len() {
            let x_start = intersections[i].ceil().max(0.0) as i32;
            let x_end = intersections[i + 1].floor().min((width - 1) as f32) as i32;
            for x in x_start..=x_end {
                if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                    set((y as u32 * width + x as u32) as usize);
                }
            }
            i += 2;
        }
    }
}

