//! Deterministic hierarchical water routing over the map's exact 4-connected topology.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

use crate::map::GameMap;

const MAGNITUDE_MASK: u8 = 0x1f;
const COST_SCALE: u32 = 100;
const BASE_COST: u32 = COST_SCALE;
const NAV_CLUSTER_SIZE: u32 = 32;
const NO_REGION: u32 = u32::MAX;
const CARDINAL_DELTAS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, -1), (0, 1)];

#[inline]
fn magnitude_penalty(magnitude: u8) -> u32 {
    if magnitude < 3 {
        10 * COST_SCALE
    } else if magnitude <= 10 {
        0
    } else {
        COST_SCALE
    }
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

    fn edge_tiles(&self, from_region: u32, to_region: u32) -> Option<(u32, u32)> {
        let start = *self.edge_offsets.get(from_region as usize)? as usize;
        let end = *self.edge_offsets.get(from_region as usize + 1)? as usize;
        self.edges[start..end]
            .iter()
            .find(|edge| edge.to_region == to_region)
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
            heuristic_weight: 5,
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

    fn first_start_in_region(
        graph: &WaterNavGraph,
        map: &GameMap,
        starts: &[u32],
        region: u32,
    ) -> Option<(u32, u32)> {
        for &start in starts {
            let (tiles, count) = water_candidates(map, start);
            for water_tile in tiles.into_iter().take(count) {
                if graph.tile_regions.get(water_tile as usize) == Some(&region) {
                    return Some((start, water_tile));
                }
            }
        }
        None
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
            return self.find_tile_path(map, None, starts, goal);
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
            return self.find_tile_path(map, None, starts, goal);
        }
        self.route_regions.reverse();

        self.route_crossings.clear();
        for pair in self.route_regions.windows(2) {
            let Some(crossing) = graph.edge_tiles(pair[0], pair[1]) else {
                return self.find_tile_path(map, None, starts, goal);
            };
            self.route_crossings.push(crossing);
        }

        let Some((source_tile, source_water)) =
            Self::first_start_in_region(&graph, map, starts, self.route_regions[0])
        else {
            return self.find_tile_path(map, None, starts, goal);
        };
        let Some(&last_region) = self.route_regions.last() else {
            return self.find_tile_path(map, None, starts, goal);
        };
        let Some(goal_water) = Self::goal_tile_in_region(&graph, map, goal, last_region) else {
            return self.find_tile_path(map, None, starts, goal);
        };

        let mut path = Vec::with_capacity(self.route_regions.len().saturating_mul(8));
        if source_tile != source_water {
            path.push(source_tile);
        }
        for index in 0..self.route_regions.len() {
            let region = self.route_regions[index];
            let segment_start = if index == 0 {
                source_water
            } else {
                self.route_crossings[index - 1].1
            };
            let segment_goal = if index < self.route_crossings.len() {
                self.route_crossings[index].0
            } else {
                goal_water
            };
            let Some(segment) =
                self.find_tile_path(map, Some((&graph, region)), &[segment_start], segment_goal)
            else {
                return self.find_tile_path(map, None, starts, goal);
            };
            if index == 0 {
                path.extend(segment);
            } else {
                path.extend(segment.into_iter().skip(1));
            }
            if index < self.route_crossings.len() {
                path.push(self.route_crossings[index].1);
            }
        }
        if goal_water != goal {
            path.push(goal);
        }
        Some(path)
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
                    let cross = cross_tie_breaker(
                        neighbor % width,
                        neighbor / width,
                        goal_x,
                        goal_y,
                        start_x,
                        start_y,
                        cross_norm,
                    );
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

/// Shared scratch buffer for water A* (insert as `Resource` on the Bevy app).
#[derive(Default, Clone)]
pub struct WaterPathfinderScratch {
    pub astar: WaterAStar,
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
    fn separate_water_bodies_have_no_route() {
        let mut map = GameMap::new(64, 64);
        set_water(&mut map, 5, 5);
        set_water(&mut map, 58, 58);
        let mut pathfinder = WaterAStar::new();

        assert!(
            pathfinder
                .find_path(&map, &[5 * map.width + 5], 58 * map.width + 58)
                .is_none()
        );
    }

    #[test]
    fn diagonal_water_contact_is_not_a_navigable_connection() {
        let mut map = GameMap::new(64, 64);
        set_water(&mut map, 31, 31);
        set_water(&mut map, 32, 32);
        let mut pathfinder = WaterAStar::new();

        assert!(
            pathfinder
                .find_path(&map, &[31 * map.width + 31], 32 * map.width + 32)
                .is_none()
        );
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
