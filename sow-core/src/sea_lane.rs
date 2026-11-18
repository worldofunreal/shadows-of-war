use serde::{Deserialize, Serialize};

/// A pre-calculated water path between two ports.
/// Analogous to `Railroad` but for water. Geographic (not owner-specific).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SeaLane {
    pub id: u64,
    /// Building ID of port A.
    pub port_a_id: u64,
    /// Building ID of port B.
    pub port_b_id: u64,
    /// Water tile indices from port A's adjacent water tile to port B's adjacent water tile.
    pub path: Vec<u32>,
}

impl SeaLane {
    #[inline]
    pub fn distance(&self) -> u32 {
        self.path.len() as u32
    }
}

/// Find the first water tile adjacent to a Port's 2×2 footprint at `port_idx`.
pub(crate) fn adjacent_water_tile(map: &crate::map::GameMap, land_idx: u32) -> Option<u32> {
    let mut found = None;
    crate::building::for_each_structure_coastal_water_tile(
        map,
        crate::game::BuildingKind::Port,
        land_idx,
        |tile| {
            if found.is_none() {
                found = Some(tile);
            }
        },
    );
    found
}

/// Find the first coastal water tile adjacent to a Port that belongs to a valid water component.
pub(crate) fn adjacent_water_tile_with_component(
    map: &crate::map::GameMap,
    water: &crate::water_components::WaterComponents,
    land_idx: u32,
) -> Option<(u32, u32)> {
    let mut found = None;
    crate::building::for_each_structure_coastal_water_tile(
        map,
        crate::game::BuildingKind::Port,
        land_idx,
        |tile| {
            if found.is_none() {
                let comp = water.component_of(tile);
                if comp > 0 {
                    found = Some((tile, comp));
                }
            }
        },
    );
    found
}

/// Recompute all sea lanes between completed ports on the same water component.
/// Called when `building_aggregates_dirty` is true (port built/destroyed/captured).
pub fn update_sea_lanes(engine: &mut crate::engine::SowEngine) {
    use crate::game::BuildingKind;

    if engine.sea_lanes_dirty {
        let mut ports: Vec<(u64, u32)> = Vec::new(); // (building_id, tile_idx)
        for b in &engine.buildings {
            if !b.under_construction && b.kind == BuildingKind::Port && b.active_level() >= 1 {
                ports.push((b.id, b.tile_idx));
            }
        }

        if ports.len() < 2 {
            engine.state.sea_lanes = std::sync::Arc::new(Vec::new());
            engine.sea_lane_calc = None;
            engine.sea_lanes_dirty = false;
            return;
        }

        // Group ports by water component for O(1) connectivity check
        let mut port_components: Vec<(u64, u32, u32)> = Vec::new(); // (id, tile, component)
        for &(id, tile) in &ports {
            if let Some(water_tile) = adjacent_water_tile(&engine.state.map, tile) {
                let comp = engine.water.component_of(water_tile);
                if comp > 0 {
                    port_components.push((id, tile, comp));
                }
            }
        }

        engine.sea_lane_calc = Some((0, Vec::new(), port_components));
        engine.sea_lanes_dirty = false;
    }

    if let Some((i, mut lanes, port_components)) = engine.sea_lane_calc.take() {
        if i < port_components.len() {
            let mut lane_id = lanes.len() as u64;
            let (id_a, tile_a, comp_a) = port_components[i];

            for &(id_b, tile_b, comp_b) in port_components.iter().skip(i + 1) {
                // Must share the same water body
                if comp_a != comp_b {
                    continue;
                }

                // Skip if already connected
                let already_connected = lanes.iter().any(|l: &SeaLane| {
                    (l.port_a_id == id_a && l.port_b_id == id_b)
                        || (l.port_a_id == id_b && l.port_b_id == id_a)
                });
                if already_connected {
                    continue;
                }

                // Distance sanity check
                let w = engine.state.map.width;
                let dx = (tile_a % w) as i32 - (tile_b % w) as i32;
                let dy = (tile_a / w) as i32 - (tile_b / w) as i32;
                if dx * dx + dy * dy > 250_000 {
                    continue;
                }

                // Get water entry points
                let water_a = match adjacent_water_tile(&engine.state.map, tile_a) {
                    Some(t) => t,
                    None => continue,
                };
                let water_b = match adjacent_water_tile(&engine.state.map, tile_b) {
                    Some(t) => t,
                    None => continue,
                };

                // A* pathfind on water (reuses engine scratch buffers)
                if let Some(path) =
                    engine
                        .path_scratch
                        .astar
                        .find_path(&engine.state.map, &[water_a], water_b)
                    && path.len() <= 1000
                {
                    lanes.push(SeaLane {
                        id: lane_id,
                        port_a_id: id_a,
                        port_b_id: id_b,
                        path,
                    });
                    lane_id += 1;
                }
            }

            // Save state to continue on next tick
            engine.sea_lane_calc = Some((i + 1, lanes, port_components));
        } else {
            // Finished all ports! Apply
            engine.state.sea_lanes = std::sync::Arc::new(lanes);
            engine.sea_lane_calc = None;
        }
    }
}

/// Pre-built adjacency list for a slice of [`SeaLane`]s so repeated queries do not rebuild it.
#[derive(Default, Clone)]
pub(crate) struct SeaLaneAdjacency {
    adj: std::collections::HashMap<u64, Vec<(u64, usize)>>,
}

impl SeaLaneAdjacency {
    pub(crate) fn build(sea_lanes: &[SeaLane]) -> Self {
        let mut adj: std::collections::HashMap<u64, Vec<(u64, usize)>> =
            std::collections::HashMap::new();
        for (idx, lane) in sea_lanes.iter().enumerate() {
            adj.entry(lane.port_a_id)
                .or_default()
                .push((lane.port_b_id, idx));
            adj.entry(lane.port_b_id)
                .or_default()
                .push((lane.port_a_id, idx));
        }
        Self { adj }
    }
}

/// Route a fleet through a pre-built sea lane adjacency graph from `src_port_id` to `dst_port_id`.
pub(crate) fn route_through_adjacency(
    adjacency: &SeaLaneAdjacency,
    sea_lanes: &[SeaLane],
    src_port_id: u64,
    dst_port_id: u64,
) -> Option<Vec<u32>> {
    if src_port_id == dst_port_id {
        return Some(Vec::new());
    }
    if !adjacency.adj.contains_key(&src_port_id) || !adjacency.adj.contains_key(&dst_port_id) {
        return None;
    }

    let mut dist: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
    let mut prev: std::collections::HashMap<u64, (u64, usize, bool)> =
        std::collections::HashMap::new();
    let mut heap = std::collections::BinaryHeap::new();

    dist.insert(src_port_id, 0);
    heap.push(std::cmp::Reverse((0u32, src_port_id)));

    while let Some(std::cmp::Reverse((cost, port))) = heap.pop() {
        if port == dst_port_id {
            break;
        }
        if cost > *dist.get(&port).unwrap_or(&u32::MAX) {
            continue;
        }
        if let Some(neighbors) = adjacency.adj.get(&port) {
            for &(neighbor, lane_idx) in neighbors {
                let lane = &sea_lanes[lane_idx];
                let edge_cost = lane.distance();
                let new_cost = cost + edge_cost;
                if new_cost < *dist.get(&neighbor).unwrap_or(&u32::MAX) {
                    dist.insert(neighbor, new_cost);
                    let reversed = lane.port_b_id == port;
                    prev.insert(neighbor, (port, lane_idx, reversed));
                    heap.push(std::cmp::Reverse((new_cost, neighbor)));
                }
            }
        }
    }

    if !dist.contains_key(&dst_port_id) {
        return None;
    }

    let mut lane_segments: Vec<(usize, bool)> = Vec::new();
    let mut current = dst_port_id;
    while current != src_port_id {
        let &(from, lane_idx, reversed) = prev.get(&current)?;
        lane_segments.push((lane_idx, reversed));
        current = from;
    }
    lane_segments.reverse();

    let mut full_path = Vec::new();
    for (lane_idx, reversed) in lane_segments {
        let lane = &sea_lanes[lane_idx];
        if reversed {
            for &tile in lane.path.iter().rev() {
                if full_path.last() != Some(&tile) {
                    full_path.push(tile);
                }
            }
        } else {
            for &tile in &lane.path {
                if full_path.last() != Some(&tile) {
                    full_path.push(tile);
                }
            }
        }
    }

    Some(full_path)
}

/// Route a fleet through the sea lane graph from a source port to a destination port.
/// Returns the concatenated water-tile path, or None if no route exists.
pub fn route_through_lanes(
    sea_lanes: &[SeaLane],
    src_port_id: u64,
    dst_port_id: u64,
) -> Option<Vec<u32>> {
    if src_port_id == dst_port_id {
        return Some(Vec::new());
    }
    let adjacency = SeaLaneAdjacency::build(sea_lanes);
    route_through_adjacency(&adjacency, sea_lanes, src_port_id, dst_port_id)
}
