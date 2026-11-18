//! Deterministic hierarchical water routing over the map's exact 4-connected topology.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;

use crate::map::GameMap;

const MAGNITUDE_MASK: u8 = 0x1f;
const COST_SCALE: u32 = 100;
const BASE_COST: u32 = COST_SCALE;
const NAV_CLUSTER_SIZE: u32 = 32;
const NO_REGION: u32 = u32::MAX;
const CARDINAL_DELTAS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, -1), (0, 1)];
const LOS_MIN_MAG_PASS1: u8 = 2;
const LOS_MIN_MAG_PASS2: u8 = 3;
const WATER_PATH_MEMO_MAX_ENTRIES: usize = 4_096;
const WATER_PATH_MEMO_MAX_BYTES: usize = 12_000_000;

#[inline]
fn is_water_min_mag(map: &GameMap, tile: u32, min_magnitude: u8) -> bool {
    map.terrain.get(tile as usize).is_some_and(|t| {
        !t.is_land() && (t.as_byte() & MAGNITUDE_MASK) >= min_magnitude
    })
}

/// Strictly 4-connected cardinal line-of-sight over water with depth >= `min_magnitude`.
/// Matches `openfrontio`'s `SmoothingWaterTransformer.canSee` semantics so routes cut
/// cleanly through open water away from shallow shorelines while staying cardinal.
fn can_see_cardinal(map: &GameMap, from: u32, to: u32, min_magnitude: u8) -> bool {
    let width = map.width;
    if width == 0 {
        return false;
    }
    if !is_water_min_mag(map, from, min_magnitude) || !is_water_min_mag(map, to, min_magnitude) {
        return false;
    }
    let mut x = (from % width) as i32;
    let mut y = (from / width) as i32;
    let x1 = (to % width) as i32;
    let y1 = (to / width) as i32;

    let dx = (x1 - x).abs();
    let dy = (y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let max_steps = (dx + dy + 2) as usize;
    for _ in 0..=max_steps {
        let tile = y as u32 * width + x as u32;
        if !is_water_min_mag(map, tile, min_magnitude) {
            return false;
        }
        if x == x1 && y == y1 {
            return true;
        }
        let e2 = err * 2;
        let move_x = e2 > -dy;
        let move_y = e2 < dx;
        if move_x && move_y {
            let ix = x + sx;
            let inter_x = y as u32 * width + ix as u32;
            if is_water_min_mag(map, inter_x, min_magnitude) {
                x = ix;
                y += sy;
                err = err - dy + dx;
            } else {
                let iy = y + sy;
                let inter_y = iy as u32 * width + x as u32;
                if !is_water_min_mag(map, inter_y, min_magnitude) {
                    return false;
                }
                x += sx;
                y = iy;
                err = err + dx - dy;
            }
        } else {
            if move_x {
                x += sx;
                err -= dy;
            }
            if move_y {
                y += sy;
                err += dx;
            }
        }
    }
    false
}

/// Trace the exact 4-connected cardinal water segment checked by [`can_see_cardinal`],
/// appending every tile strictly after `from` up to and including `to`.
fn trace_cardinal_los_after_start(
    map: &GameMap,
    from: u32,
    to: u32,
    min_magnitude: u8,
    out: &mut Vec<u32>,
) -> bool {
    let width = map.width;
    if width == 0 {
        return false;
    }
    let start_len = out.len();
    let mut x = (from % width) as i32;
    let mut y = (from / width) as i32;
    let x1 = (to % width) as i32;
    let y1 = (to / width) as i32;

    let dx = (x1 - x).abs();
    let dy = (y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let max_steps = (dx + dy + 2) as usize;
    for _ in 0..=max_steps {
        if x == x1 && y == y1 {
            return true;
        }
        let e2 = err * 2;
        let move_x = e2 > -dy;
        let move_y = e2 < dx;
        if move_x && move_y {
            let ix = x + sx;
            let inter_x = y as u32 * width + ix as u32;
            if is_water_min_mag(map, inter_x, min_magnitude) {
                out.push(inter_x);
                x = ix;
                y += sy;
                err = err - dy + dx;
            } else {
                let iy = y + sy;
                let inter_y = iy as u32 * width + x as u32;
                if !is_water_min_mag(map, inter_y, min_magnitude) {
                    out.truncate(start_len);
                    return false;
                }
                out.push(inter_y);
                x += sx;
                y = iy;
                err = err + dx - dy;
            }
            let next_tile = y as u32 * width + x as u32;
            if !is_water_min_mag(map, next_tile, min_magnitude) {
                out.truncate(start_len);
                return false;
            }
            out.push(next_tile);
        } else {
            if move_x {
                x += sx;
                err -= dy;
            }
            if move_y {
                y += sy;
                err += dx;
            }
            let next_tile = y as u32 * width + x as u32;
            if !is_water_min_mag(map, next_tile, min_magnitude) {
                out.truncate(start_len);
                return false;
            }
            out.push(next_tile);
        }
    }
    out.truncate(start_len);
    false
}

fn los_smooth_cardinal(map: &GameMap, path: &[u32], min_magnitude: u8) -> Vec<u32> {
    if path.len() <= 2 {
        return path.to_vec();
    }
    let mut result = Vec::with_capacity(path.len());
    result.push(path[0]);
    let last_idx = path.len() - 1;
    let mut current = 0usize;

    while current < last_idx {
        let mut farthest = current + 1;
        if current + 2 <= last_idx && is_water_min_mag(map, path[current], min_magnitude) {
            let mut lo = current + 2;
            let mut hi = last_idx;
            while lo <= hi {
                let mid = (lo + hi) >> 1;
                if can_see_cardinal(map, path[current], path[mid], min_magnitude) {
                    farthest = mid;
                    lo = mid + 1;
                } else {
                    hi = mid - 1;
                }
            }
        }
        if farthest > current + 1
            && trace_cardinal_los_after_start(
                map,
                path[current],
                path[farthest],
                min_magnitude,
                &mut result,
            )
        {
            current = farthest;
        } else {
            current += 1;
            result.push(path[current]);
        }
    }
    result
}

/// Two-pass binary-search LOS smoother over the water interior of a route,
/// preserving exact land shoreline endpoints and 4-connected cardinal steps.
fn smooth_water_path(map: &GameMap, path: Vec<u32>) -> Vec<u32> {
    if path.len() <= 3 {
        return path;
    }
    let lead_land = map
        .terrain
        .get(path[0] as usize)
        .is_some_and(|t| t.is_land());
    let trail_land = map
        .terrain
        .get(*path.last().unwrap() as usize)
        .is_some_and(|t| t.is_land());
    let water_start = usize::from(lead_land);
    let water_end = path.len().saturating_sub(usize::from(trail_land));
    if water_end <= water_start + 2 {
        return path;
    }
    let water = &path[water_start..water_end];
    // Fast skip for synthetic test maps whose water tiles have magnitude 0.
    if !water.iter().any(|&t| is_water_min_mag(map, t, LOS_MIN_MAG_PASS1)) {
        return path;
    }
    let pass1 = los_smooth_cardinal(map, water, LOS_MIN_MAG_PASS1);
    let pass2 = los_smooth_cardinal(map, &pass1, LOS_MIN_MAG_PASS2);
    if !lead_land && !trail_land {
        return pass2;
    }
    let mut out = Vec::with_capacity(pass2.len() + usize::from(lead_land) + usize::from(trail_land));
    if lead_land {
        out.push(path[0]);
    }
    out.extend(pass2);
    if trail_land {
        out.push(*path.last().unwrap());
    }
    out
}

#[inline]
fn magnitude_penalty(magnitude: u8) -> u32 {
    // Water magnitude is distance to land / 2. Only hugging the beach costs
    // extra; open sea is as cheap as coastal water so routes cross it straight.
    if magnitude < 3 { 2 * COST_SCALE } else { 0 }
}

#[inline]
fn cross_tie_breaker(
    nx: u32,
    ny: u32,
    goal_x: u32,
    goal_y: u32,
    start_x: u32,
    start_y: u32,
    cross_norm: u32,
) -> u32 {
    let dx_goal = goal_x as i64 - start_x as i64;
    let dy_goal = goal_y as i64 - start_y as i64;
    let dx_n = nx as i64 - goal_x as i64;
    let dy_n = ny as i64 - goal_y as i64;
    let cross = (dx_goal * dy_n - dy_goal * dx_n).wrapping_abs() as u64;
    let cn = cross_norm.max(1) as u64;
    ((cross * (COST_SCALE - 1) as u64) / cn / cn) as u32
}

/// Open heap node: max-heap by `Ord` pops the smallest `f_score` first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AStarNode {
    f_score: u32,
    insert_seq: u32,
    idx: u32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.f_score.cmp(&self.f_score) {
            Ordering::Equal => match other.insert_seq.cmp(&self.insert_seq) {
                Ordering::Equal => other.idx.cmp(&self.idx),
                o => o,
            },
            o => o,
        }
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug)]
struct WaterNavEdge {
    to_region: u32,
    from_tile: u32,
    to_tile: u32,
}

/// Sparse graph of locally connected water regions. Edges are real cardinal
/// water crossings between neighboring 32×32 clusters.
#[derive(Clone, Debug, Default)]
struct WaterNavGraph {
    width: u32,
    tile_regions: Vec<u32>,
    region_tiles: Vec<u32>,
    region_components: Vec<u32>,
    edge_offsets: Vec<u32>,
    edges: Vec<WaterNavEdge>,
}

impl WaterNavGraph {
    fn build(map: &GameMap) -> Self {
        let width = map.width;
        let height = map.height;
        let mut graph = Self {
            width,
            tile_regions: vec![NO_REGION; map.terrain.len()],
            ..Self::default()
        };
        if width == 0 || height == 0 {
            return graph;
        }

        // Label each truly connected water region inside its own cluster.
        let mut queue = Vec::with_capacity((NAV_CLUSTER_SIZE * NAV_CLUSTER_SIZE) as usize);
        for start in 0..map.terrain.len() {
            if map.terrain[start].is_land() || graph.tile_regions[start] != NO_REGION {
                continue;
            }
            let start_tile = start as u32;
            let cluster_x = (start_tile % width) / NAV_CLUSTER_SIZE;
            let cluster_y = (start_tile / width) / NAV_CLUSTER_SIZE;
            let region = graph.region_tiles.len() as u32;
            graph.region_tiles.push(start_tile);
            graph.tile_regions[start] = region;
            queue.clear();
            queue.push(start_tile);

            let mut head = 0;
            while head < queue.len() {
                let tile = queue[head];
                head += 1;
                for_each_cardinal_neighbor(width, height, tile, |neighbor| {
                    let x = neighbor % width;
                    let y = neighbor / width;
                    let index = neighbor as usize;
                    if x / NAV_CLUSTER_SIZE != cluster_x
                        || y / NAV_CLUSTER_SIZE != cluster_y
                        || graph.tile_regions[index] != NO_REGION
                        || map.terrain[index].is_land()
                    {
                        return;
                    }
                    graph.tile_regions[index] = region;
                    queue.push(neighbor);
                });
            }
        }

        // Store every real water crossing between neighboring clusters.
        let mut directed_edges = Vec::<(u32, u32, u32, u32)>::new();
        let mut add_crossing = |a: u32, b: u32| {
            let from = graph.tile_regions[a as usize];
            let to = graph.tile_regions[b as usize];
            if from != NO_REGION && to != NO_REGION {
                directed_edges.push((from, to, a, b));
                directed_edges.push((to, from, b, a));
            }
        };

        let cluster_cols = width.div_ceil(NAV_CLUSTER_SIZE);
        let cluster_rows = height.div_ceil(NAV_CLUSTER_SIZE);
        for cluster_x in 0..cluster_cols.saturating_sub(1) {
            let x = (cluster_x + 1) * NAV_CLUSTER_SIZE - 1;
            for y in 0..height {
                let a = y * width + x;
                let b = a + 1;
                if !map.terrain[a as usize].is_land() && !map.terrain[b as usize].is_land() {
                    add_crossing(a, b);
                }
            }
        }
        for cluster_y in 0..cluster_rows.saturating_sub(1) {
            let y = (cluster_y + 1) * NAV_CLUSTER_SIZE - 1;
            for x in 0..width {
                let a = y * width + x;
                let b = a + width;
                if !map.terrain[a as usize].is_land() && !map.terrain[b as usize].is_land() {
                    add_crossing(a, b);
                }
            }
        }
        directed_edges.sort_unstable();

        let region_count = graph.region_tiles.len();
        graph.edge_offsets = vec![0; region_count + 1];
        for (from, _, _, _) in &directed_edges {
            graph.edge_offsets[*from as usize + 1] += 1;
        }
        for index in 1..graph.edge_offsets.len() {
            graph.edge_offsets[index] += graph.edge_offsets[index - 1];
        }
        graph.edges = directed_edges
            .into_iter()
            .map(|(_, to_region, from_tile, to_tile)| WaterNavEdge {
                to_region,
                from_tile,
                to_tile,
            })
            .collect();

        // Cache connectivity so disconnected orders reject without traversing
        // a potentially large component on every request.
        graph.region_components = vec![NO_REGION; region_count];
        let mut component_id = 0u32;
        for root in 0..region_count {
            if graph.region_components[root] != NO_REGION {
                continue;
            }
            graph.region_components[root] = component_id;
            queue.clear();
            queue.push(root as u32);
            let mut head = 0;
            while head < queue.len() {
                let current = queue[head] as usize;
                head += 1;
                for edge in &graph.edges
                    [graph.edge_offsets[current] as usize..graph.edge_offsets[current + 1] as usize]
                {
                    let next = edge.to_region as usize;
                    if graph.region_components[next] == NO_REGION {
                        graph.region_components[next] = component_id;
                        queue.push(next as u32);
                    }
                }
            }
            component_id += 1;
        }
        graph
    }

    /// Crossing between two regions that keeps `from -> crossing -> goal`
    /// shortest, so routes stay near the straight line instead of snapping to
    /// the first open tile of every cluster border. Ties keep edge order.
    fn edge_tiles(
        &self,
        from_region: u32,
        to_region: u32,
        from_tiles: &[u32],
        goal_tile: u32,
    ) -> Option<(u32, u32)> {
        let start = *self.edge_offsets.get(from_region as usize)? as usize;
        let end = *self.edge_offsets.get(from_region as usize + 1)? as usize;
        let w = self.width;
        let dist = |a: u32, b: u32| manhattan(a % w, a / w, b % w, b / w);
        self.edges[start..end]
            .iter()
            .filter(|edge| edge.to_region == to_region)
            .min_by_key(|edge| {
                from_tiles
                    .iter()
                    .map(|&from_tile| dist(from_tile, edge.from_tile))
                    .min()
                    .unwrap_or(u32::MAX)
                    .saturating_add(dist(edge.to_tile, goal_tile))
            })
            .map(|edge| (edge.from_tile, edge.to_tile))
    }
}

/// Return adjacent water tiles in a stable cardinal order. Land endpoints are
/// allowed because fleet orders begin/end on shoreline tiles.
fn water_candidates(map: &GameMap, tile: u32) -> ([u32; 4], usize) {
    let mut candidates = [NO_REGION; 4];
    if tile as usize >= map.terrain.len() || map.width == 0 {
        return (candidates, 0);
    }
    if !map.terrain[tile as usize].is_land() {
        candidates[0] = tile;
        return (candidates, 1);
    }

    let x = tile % map.width;
    let y = tile / map.width;
    let neighbors = [
        x.checked_sub(1).map(|nx| y * map.width + nx),
        (x + 1 < map.width).then_some(y * map.width + x + 1),
        y.checked_sub(1).map(|ny| ny * map.width + x),
        (y + 1 < map.height).then_some((y + 1) * map.width + x),
    ];
    let mut count = 0;
    for neighbor in neighbors.into_iter().flatten() {
        if !map.terrain[neighbor as usize].is_land() {
            candidates[count] = neighbor;
            count += 1;
        }
    }
    (candidates, count)
}

#[inline]
fn for_each_cardinal_neighbor(width: u32, height: u32, tile: u32, mut visit: impl FnMut(u32)) {
    if width == 0 {
        return;
    }
    let x = tile % width;
    let y = tile / width;
    for &(dx, dy) in &CARDINAL_DELTAS {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
            visit(ny as u32 * width + nx as u32);
        }
    }
}

/// Reusable navigation and A* scratch for one static match map.
#[derive(Debug, Clone)]
pub struct WaterAStar {
    width: u32,
    height: u32,
    terrain_identity: usize,
    stamp: u32,
    pub(crate) closed_stamp: Vec<u32>,
    pub(crate) gscore_stamp: Vec<u32>,
    pub(crate) gscore: Vec<u32>,
    pub(crate) came_from: Vec<i32>,
    pub(crate) heap: BinaryHeap<AStarNode>,
    heuristic_weight: u32,
    push_seq: u32,
    navigation_graph: Option<Arc<WaterNavGraph>>,
    route_regions: Vec<u32>,
    route_crossings: Vec<(u32, u32)>,
}

impl Default for WaterAStar {
    fn default() -> Self {
        Self::new()
    }
}

impl WaterAStar {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            terrain_identity: 0,
            stamp: 0,
            closed_stamp: Vec::new(),
            gscore_stamp: Vec::new(),
            gscore: Vec::new(),
            came_from: Vec::new(),
            heap: BinaryHeap::new(),
            heuristic_weight: 1,
            push_seq: 0,
            navigation_graph: None,
            route_regions: Vec::new(),
            route_crossings: Vec::new(),
        }
    }

    pub fn ensure_capacity(&mut self, map: &GameMap) {
        let terrain_identity = map.terrain.as_ptr() as usize;
        let map_changed = self.width != map.width
            || self.height != map.height
            || self.terrain_identity != terrain_identity
            || self
                .navigation_graph
                .as_ref()
                .is_none_or(|graph| graph.tile_regions.len() != map.terrain.len());
        let node_count = map.terrain.len();
        self.closed_stamp.resize(node_count, 0);
        self.gscore_stamp.resize(node_count, 0);
        self.gscore.resize(node_count, 0);
        self.came_from.resize(node_count, -1);

        if map_changed {
            self.closed_stamp.fill(0);
            self.gscore_stamp.fill(0);
            self.stamp = 0;
            self.heap.clear();
            self.width = map.width;
            self.height = map.height;
            self.terrain_identity = terrain_identity;
            self.navigation_graph = Some(Arc::new(WaterNavGraph::build(map)));
        }
    }

    fn next_stamp(&mut self) -> u32 {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.closed_stamp.fill(0);
            self.gscore_stamp.fill(0);
            self.stamp = 1;
        }
        self.stamp
    }

    fn goal_regions(graph: &WaterNavGraph, map: &GameMap, goal: u32) -> ([u32; 4], usize) {
        let mut regions = [NO_REGION; 4];
        let mut region_count = 0;
        let (tiles, tile_count) = water_candidates(map, goal);
        for tile in tiles.into_iter().take(tile_count) {
            let Some(&region) = graph.tile_regions.get(tile as usize) else {
                continue;
            };
            if region == NO_REGION || regions[..region_count].contains(&region) {
                continue;
            }
            regions[region_count] = region;
            region_count += 1;
        }
        (regions, region_count)
    }

    fn endpoints_share_component(
        graph: &WaterNavGraph,
        map: &GameMap,
        starts: &[u32],
        goal_regions: &[u32],
    ) -> bool {
        for &start in starts {
            let (tiles, count) = water_candidates(map, start);
            for tile in tiles.into_iter().take(count) {
                let Some(&region) = graph.tile_regions.get(tile as usize) else {
                    continue;
                };
                if region != NO_REGION
                    && goal_regions.iter().any(|&goal| {
                        graph.region_components[region as usize]
                            == graph.region_components[goal as usize]
                    })
                {
                    return true;
                }
            }
        }
        false
    }

    fn graph_heuristic(graph: &WaterNavGraph, region: u32, goals: &[u32]) -> u32 {
        let tile = graph.region_tiles[region as usize];
        let x = (tile % graph.width) / NAV_CLUSTER_SIZE;
        let y = (tile / graph.width) / NAV_CLUSTER_SIZE;
        goals
            .iter()
            .map(|&goal| {
                let goal_tile = graph.region_tiles[goal as usize];
                let goal_x = (goal_tile % graph.width) / NAV_CLUSTER_SIZE;
                let goal_y = (goal_tile / graph.width) / NAV_CLUSTER_SIZE;
                x.abs_diff(goal_x) + y.abs_diff(goal_y)
            })
            .min()
            .unwrap_or(0)
    }

    fn find_graph_route(
        &mut self,
        graph: &WaterNavGraph,
        map: &GameMap,
        starts: &[u32],
        goals: &[u32],
    ) -> Option<u32> {
        let stamp = self.next_stamp();
        self.heap.clear();
        self.push_seq = 0;
        for &start in starts {
            let (tiles, count) = water_candidates(map, start);
            for tile in tiles.into_iter().take(count) {
                let Some(&region) = graph.tile_regions.get(tile as usize) else {
                    continue;
                };
                if region == NO_REGION || self.gscore_stamp[region as usize] == stamp {
                    continue;
                }
                self.gscore[region as usize] = 0;
                self.gscore_stamp[region as usize] = stamp;
                self.came_from[region as usize] = -1;
                self.push_seq = self.push_seq.wrapping_add(1);
                self.heap.push(AStarNode {
                    f_score: Self::graph_heuristic(graph, region, goals),
                    insert_seq: self.push_seq,
                    idx: region,
                });
            }
        }

        while let Some(node) = self.heap.pop() {
            let current = node.idx as usize;
            if self.closed_stamp[current] == stamp {
                continue;
            }
            self.closed_stamp[current] = stamp;
            if goals.contains(&node.idx) {
                return Some(node.idx);
            }

            let current_g = self.gscore[current];
            let edge_start = graph.edge_offsets[current] as usize;
            let edge_end = graph.edge_offsets[current + 1] as usize;
            for edge in &graph.edges[edge_start..edge_end] {
                let next = edge.to_region as usize;
                if self.closed_stamp[next] == stamp {
                    continue;
                }
                let tentative_g = current_g.saturating_add(1);
                if self.gscore_stamp[next] != stamp || tentative_g < self.gscore[next] {
                    self.came_from[next] = current as i32;
                    self.gscore[next] = tentative_g;
                    self.gscore_stamp[next] = stamp;
                    self.push_seq = self.push_seq.wrapping_add(1);
                    self.heap.push(AStarNode {
                        f_score: tentative_g.saturating_add(Self::graph_heuristic(
                            graph,
                            edge.to_region,
                            goals,
                        )),
                        insert_seq: self.push_seq,
                        idx: edge.to_region,
                    });
                }
            }
        }
        None
    }

    fn start_tiles_in_region(
        graph: &WaterNavGraph,
        map: &GameMap,
        starts: &[u32],
        region: u32,
    ) -> Vec<(u32, u32)> {
        let mut candidates = Vec::new();
        for &start in starts {
            let (tiles, count) = water_candidates(map, start);
            for water_tile in tiles.into_iter().take(count) {
                if graph.tile_regions.get(water_tile as usize) == Some(&region) {
                    candidates.push((start, water_tile));
                }
            }
        }
        candidates
    }

    fn goal_tile_in_region(
        graph: &WaterNavGraph,
        map: &GameMap,
        goal: u32,
        region: u32,
    ) -> Option<u32> {
        let (tiles, count) = water_candidates(map, goal);
        tiles
            .into_iter()
            .take(count)
            .find(|tile| graph.tile_regions.get(*tile as usize) == Some(&region))
    }

    pub fn find_path(&mut self, map: &GameMap, starts: &[u32], goal: u32) -> Option<Vec<u32>> {
        if starts.is_empty() || map.width == 0 || map.height == 0 {
            return None;
        }
        if goal as usize >= map.terrain.len() {
            return None;
        }
        if starts.contains(&goal) {
            return Some(vec![goal]);
        }

        self.ensure_capacity(map);
        // Fast path: direct open-water cardinal line-of-sight skips cluster A* completely.
        if starts.len() == 1
            && can_see_cardinal(map, starts[0], goal, LOS_MIN_MAG_PASS1)
        {
            let mut direct = Vec::with_capacity(
                manhattan(
                    starts[0] % map.width,
                    starts[0] / map.width,
                    goal % map.width,
                    goal / map.width,
                ) as usize
                    + 1,
            );
            direct.push(starts[0]);
            if trace_cardinal_los_after_start(
                map,
                starts[0],
                goal,
                LOS_MIN_MAG_PASS1,
                &mut direct,
            ) {
                return Some(direct);
            }
        }
        let graph = Arc::clone(self.navigation_graph.as_ref()?);
        let (goal_regions, goal_count) = Self::goal_regions(&graph, map, goal);
        if goal_count == 0 {
            return None;
        }
        let goals = &goal_regions[..goal_count];

        // Disconnected endpoints reject immediately. If the exact graph says
        // connected but route recovery fails, use a full cardinal search so a
        // coarse abstraction can never report a false "no route".
        if !Self::endpoints_share_component(&graph, map, starts, goals) {
            return None;
        }
        let Some(goal_region) = self.find_graph_route(&graph, map, starts, goals) else {
            return self
                .find_tile_path(map, None, starts, goal)
                .map(|p| smooth_water_path(map, p));
        };

        self.route_regions.clear();
        let mut current = goal_region as i32;
        for _ in 0..=graph.region_tiles.len() {
            if current < 0 {
                break;
            }
            let region = current as u32;
            self.route_regions.push(region);
            current = self.came_from[region as usize];
        }
        if self
            .route_regions
            .last()
            .is_none_or(|&region| self.came_from[region as usize] >= 0)
        {
            return self
                .find_tile_path(map, None, starts, goal)
                .map(|p| smooth_water_path(map, p));
        }
        self.route_regions.reverse();

        let source_candidates =
            Self::start_tiles_in_region(&graph, map, starts, self.route_regions[0]);
        if source_candidates.is_empty() {
            return self
                .find_tile_path(map, None, starts, goal)
                .map(|p| smooth_water_path(map, p));
        }
        let source_water_tiles: Vec<u32> = source_candidates
            .iter()
            .map(|(_, water_tile)| *water_tile)
            .collect();
        let Some(&last_region) = self.route_regions.last() else {
            return self
                .find_tile_path(map, None, starts, goal)
                .map(|p| smooth_water_path(map, p));
        };
        let Some(goal_water) = Self::goal_tile_in_region(&graph, map, goal, last_region) else {
            return self
                .find_tile_path(map, None, starts, goal)
                .map(|p| smooth_water_path(map, p));
        };

        self.route_crossings.clear();
        let mut previous_water = None;
        for pair in self.route_regions.windows(2) {
            let crossing = match previous_water {
                Some(tile) => graph.edge_tiles(
                    pair[0],
                    pair[1],
                    std::slice::from_ref(&tile),
                    goal_water,
                ),
                None => graph.edge_tiles(pair[0], pair[1], &source_water_tiles, goal_water),
            };
            let Some(crossing) = crossing else {
                return self
                    .find_tile_path(map, None, starts, goal)
                    .map(|p| smooth_water_path(map, p));
            };
            previous_water = Some(crossing.1);
            self.route_crossings.push(crossing);
        }

        let mut path = Vec::with_capacity(self.route_regions.len().saturating_mul(16));
        let mut index = 0usize;
        while index < self.route_regions.len() {
            if index > 0 {
                let current_water = self.route_crossings[index - 1].1;
                if is_water_min_mag(map, current_water, LOS_MIN_MAG_PASS1) {
                    // Try jumping directly to goal_water across open sea.
                    if can_see_cardinal(map, current_water, goal_water, LOS_MIN_MAG_PASS1)
                        && trace_cardinal_los_after_start(
                            map,
                            current_water,
                            goal_water,
                            LOS_MIN_MAG_PASS1,
                            &mut path,
                        )
                    {
                        break;
                    }
                    // Otherwise binary-search the farthest cluster crossing with open-water LOS.
                    if index + 1 < self.route_crossings.len() {
                        let mut lo = index + 1;
                        let mut hi = self.route_crossings.len() - 1;
                        let mut best_k = None;
                        while lo <= hi {
                            let mid = (lo + hi) >> 1;
                            if can_see_cardinal(
                                map,
                                current_water,
                                self.route_crossings[mid].0,
                                LOS_MIN_MAG_PASS1,
                            ) {
                                best_k = Some(mid);
                                lo = mid + 1;
                            } else {
                                if mid == 0 {
                                    break;
                                }
                                hi = mid - 1;
                            }
                        }
                        if let Some(k) = best_k
                            && trace_cardinal_los_after_start(
                                map,
                                current_water,
                                self.route_crossings[k].0,
                                LOS_MIN_MAG_PASS1,
                                &mut path,
                            )
                        {
                            path.push(self.route_crossings[k].1);
                            index = k + 1;
                            continue;
                        }
                    }
                }
            }

            let region = self.route_regions[index];
            let previous_start = index
                .checked_sub(1)
                .map(|previous| self.route_crossings[previous].1);
            let segment_starts = match previous_start.as_ref() {
                Some(tile) => std::slice::from_ref(tile),
                None => source_water_tiles.as_slice(),
            };
            let segment_goal = if index < self.route_crossings.len() {
                self.route_crossings[index].0
            } else {
                goal_water
            };
            let Some(segment) =
                self.find_tile_path(map, Some((&graph, region)), segment_starts, segment_goal)
            else {
                return self
                    .find_tile_path(map, None, starts, goal)
                    .map(|p| smooth_water_path(map, p));
            };
            if index == 0 {
                let Some(&source_water) = segment.first() else {
                    return self
                        .find_tile_path(map, None, starts, goal)
                        .map(|p| smooth_water_path(map, p));
                };
                let Some(source_tile) = source_candidates
                    .iter()
                    .find(|(_, water_tile)| *water_tile == source_water)
                    .map(|(source_tile, _)| *source_tile)
                else {
                    return self
                        .find_tile_path(map, None, starts, goal)
                        .map(|p| smooth_water_path(map, p));
                };
                if source_tile != source_water {
                    path.push(source_tile);
                }
                path.extend(segment);
            } else {
                path.extend(segment.into_iter().skip(1));
            }
            if index < self.route_crossings.len() {
                path.push(self.route_crossings[index].1);
            }
            index += 1;
        }
        if goal_water != goal {
            path.push(goal);
        }
        Some(smooth_water_path(map, path))
    }

    fn find_tile_path(
        &mut self,
        map: &GameMap,
        region_filter: Option<(&WaterNavGraph, u32)>,
        starts: &[u32],
        goal: u32,
    ) -> Option<Vec<u32>> {
        let width = map.width;
        let height = map.height;
        if width == 0 || height == 0 || goal as usize >= map.terrain.len() {
            return None;
        }
        if let Some((graph, region)) = region_filter {
            if graph.tile_regions.get(goal as usize) != Some(&region) {
                return None;
            }
        }

        let stamp = self.next_stamp();
        let mut first_start = None;
        self.heap.clear();
        self.push_seq = 0;
        let goal_x = goal % width;
        let goal_y = goal / width;

        for &start in starts {
            if start as usize >= map.terrain.len()
                || region_filter.is_some_and(|(graph, region)| {
                    graph.tile_regions.get(start as usize) != Some(&region)
                })
            {
                continue;
            }
            first_start.get_or_insert(start);
            let index = start as usize;
            if self.gscore_stamp[index] == stamp {
                continue;
            }
            self.gscore[index] = 0;
            self.gscore_stamp[index] = stamp;
            self.came_from[index] = -1;
            self.push_seq = self.push_seq.wrapping_add(1);
            self.heap.push(AStarNode {
                f_score: self
                    .heuristic_weight
                    .saturating_mul(BASE_COST)
                    .saturating_mul(manhattan(start % width, start / width, goal_x, goal_y)),
                insert_seq: self.push_seq,
                idx: start,
            });
        }
        let start = first_start?;
        let start_x = start % width;
        let start_y = start / width;
        let cross_norm = manhattan(start_x, start_y, goal_x, goal_y).max(1);
        let use_cross_tie_breaker = starts.len() == 1;

        while let Some(node) = self.heap.pop() {
            let current = node.idx as usize;
            if self.closed_stamp[current] == stamp {
                continue;
            }
            self.closed_stamp[current] = stamp;
            if node.idx == goal {
                return Some(self.build_path(goal as usize));
            }

            let current_g = self.gscore[current];
            for_each_cardinal_neighbor(width, height, node.idx, |neighbor| {
                let index = neighbor as usize;
                if region_filter.is_some_and(|(graph, region)| graph.tile_regions[index] != region)
                {
                    return;
                }
                let terrain = map.terrain[index];
                if neighbor != goal && terrain.is_land() {
                    return;
                }
                if self.closed_stamp[index] == stamp {
                    return;
                }

                let cost =
                    BASE_COST.saturating_add(magnitude_penalty(terrain.as_byte() & MAGNITUDE_MASK));
                let tentative_g = current_g.saturating_add(cost);
                if self.gscore_stamp[index] != stamp || tentative_g < self.gscore[index] {
                    self.came_from[index] = current as i32;
                    self.gscore[index] = tentative_g;
                    self.gscore_stamp[index] = stamp;
                    let h = self
                        .heuristic_weight
                        .saturating_mul(BASE_COST)
                        .saturating_mul(manhattan(
                            neighbor % width,
                            neighbor / width,
                            goal_x,
                            goal_y,
                        ));
                    let cross = if use_cross_tie_breaker {
                        cross_tie_breaker(
                            neighbor % width,
                            neighbor / width,
                            goal_x,
                            goal_y,
                            start_x,
                            start_y,
                            cross_norm,
                        )
                    } else {
                        0
                    };
                    self.push_seq = self.push_seq.wrapping_add(1);
                    self.heap.push(AStarNode {
                        f_score: tentative_g.saturating_add(h).saturating_add(cross),
                        insert_seq: self.push_seq,
                        idx: neighbor,
                    });
                }
            });
        }
        None
    }

    fn build_path(&self, goal: usize) -> Vec<u32> {
        let mut path = Vec::new();
        let mut current = goal as i32;
        while current >= 0 {
            let tile = current as u32;
            path.push(tile);
            current = self.came_from[tile as usize];
        }
        path.reverse();
        path
    }
}

#[inline]
fn manhattan(x1: u32, y1: u32, x2: u32, y2: u32) -> u32 {
    x1.abs_diff(x2).saturating_add(y1.abs_diff(y2))
}

/// Deterministic bounded memo of single-source water paths keyed by `(start_tile, goal_tile)`.
/// Terrain is static across a match, so repeating port-to-port trade voyages reuse the smoothed
/// `Arc<Vec<u32>>` path in O(1) without re-running A* or copying tiles.
#[derive(Default, Clone)]
pub struct WaterPathMemo {
    terrain_identity: usize,
    width: u32,
    height: u32,
    terrain_len: usize,
    entries: HashMap<(u32, u32), Option<Arc<Vec<u32>>>>,
    order: VecDeque<(u32, u32)>,
    live_bytes: usize,
}

impl WaterPathMemo {
    fn entry_bytes(path: &Option<Arc<Vec<u32>>>) -> usize {
        match path {
            Some(p) => p.len().saturating_mul(std::mem::size_of::<u32>()).max(16),
            None => 16,
        }
    }

    fn sync_map(&mut self, map: &GameMap) {
        let terrain_identity = map.terrain.as_ptr() as usize;
        if self.terrain_identity != terrain_identity
            || self.width != map.width
            || self.height != map.height
            || self.terrain_len != map.terrain.len()
        {
            self.entries.clear();
            self.order.clear();
            self.live_bytes = 0;
            self.terrain_identity = terrain_identity;
            self.width = map.width;
            self.height = map.height;
            self.terrain_len = map.terrain.len();
        }
    }

    pub fn get(&mut self, map: &GameMap, start: u32, goal: u32) -> Option<Option<Arc<Vec<u32>>>> {
        self.sync_map(map);
        self.entries.get(&(start, goal)).cloned()
    }

    pub fn insert(&mut self, map: &GameMap, start: u32, goal: u32, value: Option<Arc<Vec<u32>>>) {
        self.sync_map(map);
        let key = (start, goal);
        let bytes = Self::entry_bytes(&value);
        if let Some(prev) = self.entries.insert(key, value) {
            self.live_bytes = self.live_bytes.saturating_sub(Self::entry_bytes(&prev));
            self.live_bytes = self.live_bytes.saturating_add(bytes);
            return;
        }
        self.order.push_back(key);
        self.live_bytes = self.live_bytes.saturating_add(bytes);
        while self.entries.len() > WATER_PATH_MEMO_MAX_ENTRIES
            || self.live_bytes > WATER_PATH_MEMO_MAX_BYTES
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.live_bytes = self.live_bytes.saturating_sub(Self::entry_bytes(&removed));
            }
        }
    }
}

/// Shared scratch buffer for water A* and bounded port-to-port path memoization.
#[derive(Default, Clone)]
pub struct WaterPathfinderScratch {
    pub astar: WaterAStar,
    pub memo: WaterPathMemo,
}

impl WaterPathfinderScratch {
    /// Cached smoothed water route between two tiles. Returns a shared `Arc<Vec<u32>>` in O(1)
    /// on repeated port-to-port queries.
    pub fn find_path_cached(
        &mut self,
        map: &GameMap,
        start: u32,
        goal: u32,
    ) -> Option<Arc<Vec<u32>>> {
        if let Some(cached) = self.memo.get(map, start, goal) {
            return cached;
        }
        let computed = self.astar.find_path(map, &[start], goal).map(Arc::new);
        self.memo.insert(map, start, goal, computed.clone());
        computed
    }
}

/// Bresenham line rasterization on an offset hex grid. Returns a path from
/// `src` to `dst` (inclusive). Used for projectile flight paths that ignore terrain.
pub fn bresenham_line(src: u32, dst: u32, width: u32) -> Vec<u32> {
    let sx = (src % width) as i32;
    let sy = (src / width) as i32;
    let ex = (dst % width) as i32;
    let ey = (dst / width) as i32;

    let dx = (ex - sx).abs();
    let dy = (ey - sy).abs();
    let sign_x: i32 = if ex > sx { 1 } else { -1 };
    let sign_y: i32 = if ey > sy { 1 } else { -1 };
    let mut err = dx - dy;

    let mut cx = sx;
    let mut cy = sy;

    let mut path = Vec::with_capacity((dx + dy + 1) as usize);
    loop {
        path.push(cy as u32 * width + cx as u32);
        if cx == ex && cy == ey {
            break;
        }
        let e2 = err * 2;
        if e2 > -dy {
            err -= dy;
            cx += sign_x;
        }
        if e2 < dx {
            err += dx;
            cy += sign_y;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{GameMap, MapTile};

    fn set_water(map: &mut GameMap, x: u32, y: u32) {
        map.terrain[(y * map.width + x) as usize] = MapTile::from_byte(0b0010_0000);
    }

    fn river_around_decoy_water() -> GameMap {
        let mut map = GameMap::new(96, 112);
        for y in 40..=80 {
            set_water(&mut map, 15, y);
            set_water(&mut map, 80, y);
        }
        for x in 15..=80 {
            set_water(&mut map, x, 80);
        }
        for y in 0..map.height {
            for x in 81..map.width {
                set_water(&mut map, x, y);
            }
        }
        for x in [24, 40, 56, 72] {
            set_water(&mut map, x, 40);
        }
        map
    }

    fn assert_cardinal_water_path(map: &GameMap, path: &[u32], land_endpoints: bool) {
        assert!(!path.is_empty());
        for pair in path.windows(2) {
            let ax = pair[0] % map.width;
            let ay = pair[0] / map.width;
            let bx = pair[1] % map.width;
            let by = pair[1] / map.width;
            assert_eq!(ax.abs_diff(bx) + ay.abs_diff(by), 1);
        }
        if land_endpoints && path.len() > 2 {
            for &tile in &path[1..path.len() - 1] {
                assert!(!map.terrain[tile as usize].is_land());
            }
        } else if !land_endpoints {
            assert!(
                path.iter()
                    .all(|&tile| !map.terrain[tile as usize].is_land())
            );
        }
    }

    #[test]
    fn connected_river_reaches_ocean_past_disconnected_wet_chunks() {
        let map = river_around_decoy_water();
        let start = 40 * map.width + 15;
        let goal = 40 * map.width + 80;
        let mut pathfinder = WaterAStar::new();

        let path = pathfinder.find_path(&map, &[start], goal).unwrap();
        assert_eq!(path.first(), Some(&start));
        assert_eq!(path.last(), Some(&goal));
        assert_cardinal_water_path(&map, &path, false);

        let graph = Arc::clone(pathfinder.navigation_graph.as_ref().unwrap());
        let repeated_path = pathfinder.find_path(&map, &[start], goal).unwrap();
        assert_eq!(repeated_path, path);
        assert!(Arc::ptr_eq(
            &graph,
            pathfinder.navigation_graph.as_ref().unwrap()
        ));
    }

    #[test]
    fn open_sea_route_across_clusters_has_no_detour() {
        let mut map = GameMap::new(128, 128);
        for y in 0..map.height {
            for x in 0..map.width {
                set_water(&mut map, x, y);
            }
        }
        let (sx, sy, gx, gy) = (5, 100, 120, 10);
        let mut pathfinder = WaterAStar::new();

        let path = pathfinder
            .find_path(&map, &[sy * map.width + sx], gy * map.width + gx)
            .unwrap();
        assert_cardinal_water_path(&map, &path, false);
        assert_eq!(path.len() as u32, manhattan(sx, sy, gx, gy) + 1);
    }

    #[test]
    fn disconnected_and_diagonal_water_have_no_route() {
        for ((sx, sy), (gx, gy)) in [((5, 5), (58, 58)), ((31, 31), (32, 32))] {
            let mut map = GameMap::new(64, 64);
            set_water(&mut map, sx, sy);
            set_water(&mut map, gx, gy);
            let mut pathfinder = WaterAStar::new();

            assert!(
                pathfinder
                    .find_path(&map, &[sy * map.width + sx], gy * map.width + gx)
                    .is_none(),
                "water at ({sx}, {sy}) cannot navigate to ({gx}, {gy})"
            );
        }
    }

    #[test]
    fn shoreline_endpoints_attach_to_their_cardinal_water_neighbors() {
        let mut map = river_around_decoy_water();
        let start_shore = 40 * map.width + 14;
        let goal_shore = 40 * map.width + 81;
        map.terrain[start_shore as usize] = MapTile::from_byte(0b1100_0000);
        map.terrain[goal_shore as usize] = MapTile::from_byte(0b1100_0000);
        let mut pathfinder = WaterAStar::new();

        let path = pathfinder
            .find_path(&map, &[start_shore], goal_shore)
            .unwrap();
        assert_eq!(path.first(), Some(&start_shore));
        assert_eq!(path.last(), Some(&goal_shore));
        assert_cardinal_water_path(&map, &path, true);
    }

    #[test]
    fn exact_fallback_finds_a_route_if_the_abstract_graph_is_incomplete() {
        let map = river_around_decoy_water();
        let start = 40 * map.width + 15;
        let goal = 40 * map.width + 80;
        let mut pathfinder = WaterAStar::new();
        pathfinder.ensure_capacity(&map);
        let graph = Arc::make_mut(pathfinder.navigation_graph.as_mut().unwrap());
        graph.edges.clear();
        graph.edge_offsets.fill(0);

        let path = pathfinder.find_path(&map, &[start], goal).unwrap();
        assert_eq!(path.first(), Some(&start));
        assert_eq!(path.last(), Some(&goal));
        assert_cardinal_water_path(&map, &path, false);
    }
}
