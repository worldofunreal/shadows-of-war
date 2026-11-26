use super::core::{
    Building, RAILWAY_LINK_DISTANCE, RAILWAY_MAX_CANDIDATES_PER_STATION,
    RAILWAY_MAX_ROUTE_DISTANCE, RailwayLink, RailwayTrack, TradeRouteProgress,
    trading_post_speed_multiplier,
};
use super::placement::BuildingFootprint;
use crate::game::{BuildingKind, GameState};
use crate::game_config::{GameConfig, NORMAL_GAME_SPEED};
use crate::map::{CARDINAL_NEIGHBOR_DELTAS, GameMap};
use std::cmp::{Reverse, max};
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};

const STATION_GRID_CELL: i32 = RAILWAY_LINK_DISTANCE as i32;
const RAIL_JOIN_RADIUS: i32 = 3;
const PATH_SEARCH_LIMIT: usize = 512;
const JUNCTION_ID_BIT: u64 = 1 << 63;

pub fn is_railway_station(building: &Building) -> bool {
    building.kind.is_enabled() && building.active_level() > 0
}

#[inline]
pub fn trading_post_period_ticks(distance: u32, level: u8, config: &GameConfig) -> u32 {
    let speed = trading_post_speed_multiplier(level);
    let normal_seconds = (14.0 + f64::from(distance) * 0.70) / speed;
    let per_tick = config.per_tick(1.0).max(f64::EPSILON);
    (normal_seconds * NORMAL_GAME_SPEED / per_tick)
        .ceil()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

/// Adjusts `opened_at` across a period change (`old_period -> new_period`) so the
/// client's periodic cart position stays visually continuous at `tick`.
#[inline]
pub fn phase_continuous_opened_at(
    tick: u64,
    old_opened_at: u64,
    old_period: u32,
    new_period: u32,
) -> u64 {
    if old_period == 0 || new_period == 0 || old_period == new_period {
        return old_opened_at;
    }
    let elapsed = tick.saturating_sub(old_opened_at) % u64::from(old_period);
    let offset = ((elapsed as f64 / f64::from(old_period)) * f64::from(new_period))
        .round()
        .clamp(0.0, f64::from(new_period.saturating_sub(1))) as u64;
    tick.saturating_sub(offset)
}

/// Computes the tile currently occupied by a cart rendered from `(tick - opened_at) % period_ticks`.
pub fn cart_tile_at_tick(link: &RailwayLink, tick: u64, map_width: u32) -> Option<u32> {
    if link.period_ticks == 0 || link.route_path.len() < 2 || map_width == 0 {
        return link.route_path.first().copied();
    }
    let total_len = polyline_distance(&link.route_path, map_width);
    if total_len == 0 {
        return link.route_path.first().copied();
    }
    let elapsed = tick.saturating_sub(link.opened_at) % u64::from(link.period_ticks);
    let cycle = f64::from(total_len) * 2.0;
    let unfolded = (elapsed as f64 / f64::from(link.period_ticks)) * cycle;
    let forward = unfolded <= f64::from(total_len);
    let mut remaining = if forward {
        unfolded
    } else {
        cycle - unfolded
    };
    let pair_count = link.route_path.len() - 1;
    for step in 0..pair_count {
        let idx = if forward {
            step
        } else {
            pair_count - step - 1
        };
        let (a, b) = if forward {
            (link.route_path[idx], link.route_path[idx + 1])
        } else {
            (link.route_path[idx + 1], link.route_path[idx])
        };
        let (ax, ay) = tile_xy(a, map_width);
        let (bx, by) = tile_xy(b, map_width);
        let leg = f64::from((ax - bx).unsigned_abs() + (ay - by).unsigned_abs());
        if remaining <= leg || leg == 0.0 {
            let t = if leg > 0.0 {
                (remaining / leg).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let cx = (f64::from(ax) + f64::from(bx - ax) * t).round() as i32;
            let cy = (f64::from(ay) + f64::from(by - ay) * t).round() as i32;
            if cx >= 0 && cy >= 0 {
                return Some(cy as u32 * map_width + cx as u32);
            }
            return Some(a);
        }
        remaining -= leg;
    }
    link.route_path.last().copied()
}

#[inline]
fn is_commercial_station_kind(kind: BuildingKind) -> bool {
    matches!(
        kind,
        BuildingKind::City | BuildingKind::Port | BuildingKind::TradingPost
    )
}

#[derive(Clone, Default)]
struct TrackGraphCache {
    ready: bool,
    node_ids: Vec<u64>,
    node_index: HashMap<u64, usize>,
    adjacency: Vec<Vec<(usize, u32)>>,
    station_by_node: Vec<Option<Building>>,
}

impl TrackGraphCache {
    fn clear(&mut self) {
        self.ready = false;
        self.node_ids.clear();
        self.node_index.clear();
        self.adjacency.clear();
        self.station_by_node.clear();
    }

    fn ensure_node(&mut self, id: u64, station: Option<Building>) -> usize {
        if let Some(&idx) = self.node_index.get(&id) {
            if let Some(st) = station {
                self.station_by_node[idx] = Some(st);
            }
            return idx;
        }
        let idx = self.node_ids.len();
        self.node_ids.push(id);
        self.node_index.insert(id, idx);
        self.adjacency.push(Vec::new());
        self.station_by_node.push(station);
        idx
    }

    fn upsert_edge(
        &mut self,
        a: u64,
        b: u64,
        dist: u32,
        stations_by_id: &HashMap<u64, Building>,
    ) {
        if !self.ready {
            return;
        }
        let ia = self.ensure_node(a, stations_by_id.get(&a).copied());
        let ib = self.ensure_node(b, stations_by_id.get(&b).copied());
        let insert_sorted = |adj: &mut Vec<(usize, u32)>,
                             target: usize,
                             weight: u32,
                             node_ids: &[u64]| {
            if let Some(slot) = adj.iter_mut().find(|(idx, _)| *idx == target) {
                slot.1 = weight;
            } else {
                adj.push((target, weight));
            }
            adj.sort_unstable_by_key(|&(idx, d)| (node_ids[idx], d));
        };
        insert_sorted(&mut self.adjacency[ia], ib, dist, &self.node_ids);
        insert_sorted(&mut self.adjacency[ib], ia, dist, &self.node_ids);
    }

    fn remove_edge(&mut self, a: u64, b: u64) {
        if !self.ready {
            return;
        }
        let (Some(&ia), Some(&ib)) = (self.node_index.get(&a), self.node_index.get(&b)) else {
            return;
        };
        self.adjacency[ia].retain(|&(idx, _)| idx != ib);
        self.adjacency[ib].retain(|&(idx, _)| idx != ia);
    }

    fn set_station(&mut self, id: u64, station: Option<Building>) {
        if !self.ready {
            return;
        }
        if let Some(&idx) = self.node_index.get(&id) {
            self.station_by_node[idx] = station;
        } else if let Some(st) = station {
            self.ensure_node(id, Some(st));
        }
    }
}

/// Persistent non-serialized scratch & graph cache for incremental O(local) railway updates.
#[derive(Clone, Default)]
pub struct RailwayRuntimeCache {
    pub ready: bool,
    tracks: BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: HashMap<(u64, u64), u32>,
    track_index: HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: Vec<u16>,
    stations_by_id: HashMap<u64, Building>,
    station_ids_sorted: Vec<u64>,
    processed_ids: HashSet<u64>,
    station_grid: HashMap<(i32, i32), Vec<u64>>,
    graph: TrackGraphCache,
    pub link_index_by_post: HashMap<u64, usize>,
    post_ids_by_edge: HashMap<(u64, u64), Vec<u64>>,
    dijkstra: DijkstraScratch,
    astar: AStarScratch,
    candidates_scratch: Vec<(i64, u64, u32)>,
    goals_scratch: Vec<(u64, u32)>,
    nearby_scratch: Vec<(bool, u32, u32)>,
    snapped_nodes_scratch: HashSet<u64>,
}

/// Summary of an incremental track-network synchronization pass.
pub struct RailwayNetworkDelta {
    pub tracks_mutated: bool,
    pub dirty_nodes: Vec<u64>,
    pub removed_station_ids: Vec<u64>,
    pub added_or_moved_ids: Vec<u64>,
    pub only_non_commercial_splits: bool,
    pub split_edge_keys: Vec<(u64, u64)>,
}

impl RailwayRuntimeCache {
    pub fn hydrate(
        &mut self,
        stations: &[Building],
        tracks: &[RailwayTrack],
        links: &[RailwayLink],
        map_width: u32,
        map_height: u32,
    ) {
        self.tracks.clear();
        self.edge_distances.clear();
        let map_len = (map_width as usize).saturating_mul(map_height as usize);
        self.tile_counts.clear();
        self.tile_counts.resize(map_len, 0);
        for track in tracks {
            let key = track_key(track.from_node_id, track.to_node_id);
            self.edge_distances
                .insert(key, polyline_distance(&track.path, map_width));
            self.tracks.insert(key, track.clone());
        }
        self.track_index = index_tracks_with_counts(tracks, map_width, &mut self.tile_counts);
        self.stations_by_id.clear();
        self.station_ids_sorted.clear();
        self.processed_ids.clear();
        self.station_grid.clear();
        for &station in stations {
            self.stations_by_id.insert(station.id, station);
            self.station_ids_sorted.push(station.id);
            self.processed_ids.insert(station.id);
            if map_width > 0 {
                let (x, y) = tile_xy(station.tile_idx, map_width);
                self.station_grid
                    .entry((x / STATION_GRID_CELL, y / STATION_GRID_CELL))
                    .or_default()
                    .push(station.id);
            }
        }
        self.station_ids_sorted.sort_unstable();
        self.station_ids_sorted.dedup();
        self.rebuild_link_index(links, &[]);
        self.graph.clear();
        self.ready = true;
    }

    pub fn rebuild_link_index(
        &mut self,
        links: &[RailwayLink],
        pending: &[super::core::PendingTradeRoute],
    ) {
        self.link_index_by_post.clear();
        self.post_ids_by_edge.clear();
        for (idx, link) in links.iter().enumerate() {
            self.link_index_by_post.insert(link.from_building_id, idx);
            if link.period_ticks > 0 {
                for pair in link.station_path.windows(2) {
                    let key = track_key(pair[0], pair[1]);
                    let posts = self.post_ids_by_edge.entry(key).or_default();
                    if posts.last().copied() != Some(link.from_building_id) {
                        posts.push(link.from_building_id);
                    }
                }
            }
            if let Some(pending_route) = pending.get(idx).and_then(|p| p.route.as_ref())
                && pending_route.period_ticks > 0
            {
                for pair in pending_route.station_path.windows(2) {
                    let key = track_key(pair[0], pair[1]);
                    let posts = self.post_ids_by_edge.entry(key).or_default();
                    if !posts.contains(&pending_route.from_building_id) {
                        posts.push(pending_route.from_building_id);
                    }
                }
            }
        }
    }

    /// Collects Trading Post IDs whose current route traverses any of `edges`.
    pub fn affected_posts_for_edges(&self, edges: &[(u64, u64)]) -> Vec<u64> {
        let mut posts = Vec::new();
        for edge in edges {
            if let Some(list) = self.post_ids_by_edge.get(edge) {
                posts.extend_from_slice(list);
            }
        }
        posts.sort_unstable();
        posts.dedup();
        posts
    }

    /// Updates station metadata (owner/level) in place when station topology did not change.
    pub fn sync_station_metadata(&mut self, stations: &[Building]) {
        for &station in stations {
            self.stations_by_id.insert(station.id, station);
            self.graph.set_station(station.id, Some(station));
        }
    }

    /// Incrementally updates tracks, spatial grid, and dirty node set for changed/removed stations.
    pub fn apply_topology_delta(
        &mut self,
        stations: &[Building],
        state: &GameState,
        active_links: &[RailwayLink],
        pending_links: &[super::core::PendingTradeRoute],
        force_full_rebuild_from_empty: bool,
    ) -> RailwayNetworkDelta {
        let map_w = state.map.width;
        let map_h = state.map.height;
        if map_w == 0 || map_h == 0 {
            self.tracks.clear();
            self.edge_distances.clear();
            self.track_index.clear();
            self.tile_counts.clear();
            self.stations_by_id.clear();
            self.station_ids_sorted.clear();
            self.processed_ids.clear();
            self.station_grid.clear();
            self.graph.clear();
            self.ready = true;
            return RailwayNetworkDelta {
                tracks_mutated: true,
                dirty_nodes: Vec::new(),
                removed_station_ids: Vec::new(),
                added_or_moved_ids: Vec::new(),
                only_non_commercial_splits: false,
                split_edge_keys: Vec::new(),
            };
        }
        let map_len = (map_w as usize).saturating_mul(map_h as usize);
        if self.tile_counts.len() != map_len {
            self.tile_counts.resize(map_len, 0);
        }

        if force_full_rebuild_from_empty {
            self.tracks.clear();
            self.edge_distances.clear();
            self.track_index.clear();
            self.tile_counts.fill(0);
            self.stations_by_id.clear();
            self.station_ids_sorted.clear();
            self.processed_ids.clear();
            self.station_grid.clear();
            self.graph.clear();
        }

        // Stations passed by SowEngine are sorted by id; linear merge finds removed/added/moved
        // stations in O(N) integer comparisons with zero HashMap allocations.
        debug_assert!(stations.windows(2).all(|w| w[0].id <= w[1].id));
        let mut removed_ids = Vec::new();
        let mut changed = Vec::new();
        let mut relocated_ids = Vec::new();
        let mut old_i = 0usize;
        let mut new_i = 0usize;
        while old_i < self.station_ids_sorted.len() && new_i < stations.len() {
            let old_id = self.station_ids_sorted[old_i];
            let new_st = stations[new_i];
            if old_id < new_st.id {
                if let Some(old_st) = self.stations_by_id.get(&old_id) {
                    removed_ids.push((old_id, old_st.tile_idx));
                }
                old_i += 1;
            } else if new_st.id < old_id {
                changed.push(new_st);
                new_i += 1;
            } else {
                if let Some(old_st) = self.stations_by_id.get(&old_id) {
                    if !same_station_location(old_st, &new_st) {
                        changed.push(new_st);
                        relocated_ids.push(new_st.id);
                    } else if old_st != &new_st {
                        self.stations_by_id.insert(new_st.id, new_st);
                        self.graph.set_station(new_st.id, Some(new_st));
                    }
                }
                old_i += 1;
                new_i += 1;
            }
        }
        while old_i < self.station_ids_sorted.len() {
            let old_id = self.station_ids_sorted[old_i];
            if let Some(old_st) = self.stations_by_id.get(&old_id) {
                removed_ids.push((old_id, old_st.tile_idx));
            }
            old_i += 1;
        }
        while new_i < stations.len() {
            changed.push(stations[new_i]);
            new_i += 1;
        }

        if !removed_ids.is_empty() || !changed.is_empty() {
            self.station_ids_sorted.clear();
            self.station_ids_sorted.extend(stations.iter().map(|s| s.id));
        }

        let mut tracks_mutated = false;
        let mut dirty_nodes = Vec::new();
        let mut dirty_seen = HashSet::new();
        let mut mark_dirty = |id: u64| {
            if dirty_seen.insert(id) {
                dirty_nodes.push(id);
            }
        };

        // Remove grid entries for removed or relocated stations, and mark removed stations dirty.
        for &(old_id, old_tile) in &removed_ids {
            self.stations_by_id.remove(&old_id);
            self.processed_ids.remove(&old_id);
            self.graph.set_station(old_id, None);
            let (ox, oy) = tile_xy(old_tile, map_w);
            if let Some(cell) = self
                .station_grid
                .get_mut(&(ox / STATION_GRID_CELL, oy / STATION_GRID_CELL))
            {
                cell.retain(|&id| id != old_id);
            }
            mark_dirty(old_id);
        }

        for &station in &changed {
            self.processed_ids.remove(&station.id);
            if let Some(old_station) = self.stations_by_id.insert(station.id, station) {
                let (ox, oy) = tile_xy(old_station.tile_idx, map_w);
                if let Some(cell) = self
                    .station_grid
                    .get_mut(&(ox / STATION_GRID_CELL, oy / STATION_GRID_CELL))
                {
                    cell.retain(|&id| id != station.id);
                }
            }
            self.graph.set_station(station.id, Some(station));
            let (x, y) = tile_xy(station.tile_idx, map_w);
            let cell = self
                .station_grid
                .entry((x / STATION_GRID_CELL, y / STATION_GRID_CELL))
                .or_default();
            if !cell.contains(&station.id) {
                cell.push(station.id);
            }
            mark_dirty(station.id);
        }

        // Remove stale tracks only for endpoints that actually relocated (rare).
        let mut any_relocated_edges = false;
        if !relocated_ids.is_empty() {
            let stale_edges: Vec<(u64, u64)> = self
                .tracks
                .keys()
                .copied()
                .filter(|&(a, b)| {
                    (relocated_ids.binary_search(&a).is_ok() && a & JUNCTION_ID_BIT == 0)
                        || (relocated_ids.binary_search(&b).is_ok() && b & JUNCTION_ID_BIT == 0)
                })
                .collect();
            for key in stale_edges {
                any_relocated_edges = true;
                mark_dirty(key.0);
                mark_dirty(key.1);
                remove_track_cached(
                    key,
                    &mut self.tracks,
                    &mut self.edge_distances,
                    &mut self.track_index,
                    &mut self.tile_counts,
                    Some(&mut self.graph),
                    map_w,
                );
                tracks_mutated = true;
            }
        }

        let mut split_edge_keys = Vec::new();
        let mut only_non_commercial_splits =
            removed_ids.is_empty() && !any_relocated_edges && !changed.is_empty();
        let mut added_or_moved_ids = Vec::with_capacity(changed.len());
        for station in changed {
            added_or_moved_ids.push(station.id);
            if is_commercial_station_kind(station.kind) {
                only_non_commercial_splits = false;
            }
            let outcome = connect_station_to_network(
                station,
                state,
                &self.stations_by_id,
                &self.station_grid,
                &self.processed_ids,
                &mut self.tracks,
                &mut self.edge_distances,
                &mut self.track_index,
                &mut self.tile_counts,
                Some(&mut self.graph),
                &mut self.astar,
                &mut self.candidates_scratch,
                &mut self.goals_scratch,
                &mut self.nearby_scratch,
                &mut self.snapped_nodes_scratch,
                &mut dirty_nodes,
                &mut dirty_seen,
            );
            if outcome.tracks_mutated {
                tracks_mutated = true;
            }
            if only_non_commercial_splits {
                if outcome.connected_via_fallback || outcome.split_edges.len() > 1 {
                    only_non_commercial_splits = false;
                } else {
                    split_edge_keys.extend(outcome.split_edges);
                }
            }
            self.processed_ids.insert(station.id);
        }

        // Prune completely isolated wasteland components whenever any station was removed
        // and no live station or active/pending train itinerary touches that component.
        if !removed_ids.is_empty() && !self.tracks.is_empty() {
            if self.prune_isolated_dead_components(active_links, pending_links, map_w) {
                tracks_mutated = true;
            }
        }

        if force_full_rebuild_from_empty {
            self.graph.clear();
        }
        self.ready = true;
        RailwayNetworkDelta {
            tracks_mutated,
            dirty_nodes,
            removed_station_ids: removed_ids.into_iter().map(|(id, _)| id).collect(),
            added_or_moved_ids,
            only_non_commercial_splits,
            split_edge_keys,
        }
    }

    fn prune_isolated_dead_components(
        &mut self,
        active_links: &[RailwayLink],
        pending_links: &[super::core::PendingTradeRoute],
        map_w: u32,
    ) -> bool {
        let mut adj: HashMap<u64, Vec<u64>> = HashMap::new();
        for &(a, b) in self.tracks.keys() {
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
        let mut alive = HashSet::new();
        let mut queue = Vec::new();
        for &station_id in self.stations_by_id.keys() {
            if alive.insert(station_id) {
                queue.push(station_id);
            }
        }
        for link in active_links {
            if link.period_ticks == 0 {
                continue;
            }
            for &node in &link.station_path {
                if alive.insert(node) {
                    queue.push(node);
                }
            }
        }
        for pending in pending_links {
            if let Some(route) = &pending.route {
                for &node in &route.station_path {
                    if alive.insert(node) {
                        queue.push(node);
                    }
                }
            }
        }
        while let Some(node) = queue.pop() {
            if let Some(neighbors) = adj.get(&node) {
                for &next in neighbors {
                    if alive.insert(next) {
                        queue.push(next);
                    }
                }
            }
        }
        let dead_edges: Vec<(u64, u64)> = self
            .tracks
            .keys()
            .copied()
            .filter(|(a, b)| !alive.contains(a) && !alive.contains(b))
            .collect();
        let any_pruned = !dead_edges.is_empty();
        for key in dead_edges {
            remove_track_cached(
                key,
                &mut self.tracks,
                &mut self.edge_distances,
                &mut self.track_index,
                &mut self.tile_counts,
                Some(&mut self.graph),
                map_w,
            );
        }
        any_pruned
    }

    pub fn tracks_vec(&self) -> Vec<RailwayTrack> {
        self.tracks.values().cloned().collect()
    }

    pub fn ensure_graph(&mut self) {
        if self.graph.ready {
            self.dijkstra.prepare(self.graph.node_ids.len());
            return;
        }
        self.graph.node_ids.clear();
        self.graph
            .node_ids
            .extend(self.tracks.keys().flat_map(|&(a, b)| [a, b]));
        self.graph
            .node_ids
            .extend(self.stations_by_id.keys().copied());
        self.graph.node_ids.sort_unstable();
        self.graph.node_ids.dedup();

        self.graph.node_index.clear();
        self.graph.node_index.reserve(self.graph.node_ids.len());
        for (idx, &id) in self.graph.node_ids.iter().enumerate() {
            self.graph.node_index.insert(id, idx);
        }

        self.graph.adjacency.clear();
        self.graph
            .adjacency
            .resize_with(self.graph.node_ids.len(), Vec::new);
        for (&(a, b), &dist) in &self.edge_distances {
            if let (Some(&ia), Some(&ib)) = (
                self.graph.node_index.get(&a),
                self.graph.node_index.get(&b),
            ) {
                self.graph.adjacency[ia].push((ib, dist));
                self.graph.adjacency[ib].push((ia, dist));
            }
        }
        for neighbors in &mut self.graph.adjacency {
            neighbors.sort_unstable_by_key(|&(idx, dist)| (self.graph.node_ids[idx], dist));
        }

        self.graph.station_by_node.clear();
        self.graph
            .station_by_node
            .resize(self.graph.node_ids.len(), None);
        for (&id, &station) in &self.stations_by_id {
            if let Some(&idx) = self.graph.node_index.get(&id) {
                self.graph.station_by_node[idx] = Some(station);
            }
        }
        self.dijkstra.prepare(self.graph.node_ids.len());
        self.graph.ready = true;
    }

    /// Collects all Trading Post IDs within `RAILWAY_MAX_ROUTE_DISTANCE` hops of `seeds`
    /// in a single bounded multi-source Dijkstra pass.
    pub fn collect_affected_trading_posts(&mut self, seeds: &[u64]) -> Vec<u64> {
        self.ensure_graph();
        if seeds.is_empty() || self.graph.node_ids.is_empty() {
            return Vec::new();
        }
        let gen_id = self.dijkstra.begin();
        let mut any_seed = false;
        for &seed_id in seeds {
            if let Some(&idx) = self.graph.node_index.get(&seed_id) {
                if self.dijkstra.visited[idx] != gen_id {
                    self.dijkstra.visited[idx] = gen_id;
                    self.dijkstra.distances[idx] = 0;
                    self.dijkstra.buckets[0].push(idx);
                    any_seed = true;
                }
            }
        }
        if !any_seed {
            return Vec::new();
        }
        let mut affected = Vec::new();
        for distance in 0..=RAILWAY_MAX_ROUTE_DISTANCE {
            let mut cursor = 0;
            while cursor < self.dijkstra.buckets[distance as usize].len() {
                let node = self.dijkstra.buckets[distance as usize][cursor];
                cursor += 1;
                if self.dijkstra.visited[node] != gen_id
                    || distance != self.dijkstra.distances[node]
                {
                    continue;
                }
                if let Some(station) = self.graph.station_by_node[node]
                    && station.kind == BuildingKind::TradingPost
                {
                    affected.push(station.id);
                }
                for &(next, edge_distance) in &self.graph.adjacency[node] {
                    let next_distance = distance.saturating_add(edge_distance);
                    if next_distance > RAILWAY_MAX_ROUTE_DISTANCE
                        || (self.dijkstra.visited[next] == gen_id
                            && self.dijkstra.distances[next] <= next_distance)
                    {
                        continue;
                    }
                    self.dijkstra.visited[next] = gen_id;
                    self.dijkstra.distances[next] = next_distance;
                    self.dijkstra.buckets[next_distance as usize].push(next);
                }
            }
        }
        affected.sort_unstable();
        affected.dedup();
        affected
    }

    /// Computes the optimal route link for a single `TradingPost` using the cached graph & Dijkstra scratch.
    pub fn plan_single_trading_post_route(
        &mut self,
        post: Building,
        state: &GameState,
        previous: Option<&RailwayLink>,
    ) -> Option<RailwayLink> {
        self.ensure_graph();
        let &source = self.graph.node_index.get(&post.id)?;
        let (distance, destination, station_path) = farthest_reachable_dual(
            source,
            &self.graph.node_ids,
            &self.graph.adjacency,
            &self.graph.station_by_node,
            &mut self.dijkstra,
        )?;
        Some(make_link_cached(
            post,
            destination,
            station_path,
            &self.tracks,
            &self.stations_by_id,
            distance,
            &self.edge_distances,
            state,
            previous,
        ))
    }
}

pub fn populate_railway_network(
    stations: &[Building],
    previous_stations: &[Building],
    state: &GameState,
    previous_tracks: &[RailwayTrack],
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    track_index_ready: &mut bool,
) -> Vec<RailwayTrack> {
    if state.map.width == 0 || state.map.height == 0 {
        track_index.clear();
        *track_index_ready = true;
        return Vec::new();
    }
    if !*track_index_ready {
        *track_index = index_tracks(previous_tracks, state.map.width);
        *track_index_ready = true;
    }

    let mut tracks: BTreeMap<(u64, u64), RailwayTrack> = previous_tracks
        .iter()
        .cloned()
        .map(|track| (track_key(track.from_node_id, track.to_node_id), track))
        .collect();
    let mut edge_distances: HashMap<(u64, u64), u32> = tracks
        .iter()
        .map(|(&key, track)| (key, polyline_distance(&track.path, state.map.width)))
        .collect();
    let old_by_id: HashMap<u64, Building> = previous_stations
        .iter()
        .map(|building| (building.id, *building))
        .collect();
    let mut changed: Vec<_> = stations
        .iter()
        .filter(|station| {
            old_by_id
                .get(&station.id)
                .is_none_or(|old| !same_station_location(old, station))
        })
        .copied()
        .collect();
    changed.sort_by_key(|station| station.id);
    let changed_ids: HashSet<u64> = changed.iter().map(|station| station.id).collect();

    let stale_edges: Vec<_> = tracks
        .keys()
        .copied()
        .filter(|key| {
            let endpoint_moved = (changed_ids.contains(&key.0) && key.0 & JUNCTION_ID_BIT == 0)
                || (changed_ids.contains(&key.1) && key.1 & JUNCTION_ID_BIT == 0);
            endpoint_moved
        })
        .collect();
    let mut empty_counts = Vec::new();
    for key in stale_edges {
        remove_track_cached(
            key,
            &mut tracks,
            &mut edge_distances,
            track_index,
            &mut empty_counts,
            None,
            state.map.width,
        );
    }

    let mut stations_by_id = HashMap::with_capacity(stations.len());
    let mut station_grid: HashMap<(i32, i32), Vec<u64>> = HashMap::new();
    for &station in stations {
        stations_by_id.insert(station.id, station);
        let (x, y) = tile_xy(station.tile_idx, state.map.width);
        station_grid
            .entry((x / STATION_GRID_CELL, y / STATION_GRID_CELL))
            .or_default()
            .push(station.id);
    }

    // Keep the network sparse: a station adds one connection, not a mesh of
    // every nearby station. Dead stations leave their physical track behind.
    let mut processed: HashSet<u64> = stations
        .iter()
        .filter(|station| !changed_ids.contains(&station.id))
        .map(|station| station.id)
        .collect();
    let mut candidates = Vec::with_capacity(RAILWAY_MAX_CANDIDATES_PER_STATION * 2);
    let mut goals = Vec::with_capacity(RAILWAY_MAX_CANDIDATES_PER_STATION);
    let mut astar = AStarScratch::default();
    let mut nearby_scratch = Vec::new();
    let mut snapped_nodes_scratch = HashSet::new();
    let mut dummy_dirty = Vec::new();
    let mut dummy_seen = HashSet::new();
    for station in changed {
        connect_station_to_network(
            station,
            state,
            &stations_by_id,
            &station_grid,
            &processed,
            &mut tracks,
            &mut edge_distances,
            track_index,
            &mut empty_counts,
            None,
            &mut astar,
            &mut candidates,
            &mut goals,
            &mut nearby_scratch,
            &mut snapped_nodes_scratch,
            &mut dummy_dirty,
            &mut dummy_seen,
        );
        processed.insert(station.id);
    }

    tracks.into_values().collect()
}

struct ConnectOutcome {
    tracks_mutated: bool,
    connected_via_fallback: bool,
    split_edges: Vec<(u64, u64)>,
}

#[allow(clippy::too_many_arguments)]
fn connect_station_to_network(
    station: Building,
    state: &GameState,
    stations_by_id: &HashMap<u64, Building>,
    station_grid: &HashMap<(i32, i32), Vec<u64>>,
    processed: &HashSet<u64>,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: &mut HashMap<(u64, u64), u32>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: &mut Vec<u16>,
    mut graph: Option<&mut TrackGraphCache>,
    astar: &mut AStarScratch,
    candidates: &mut Vec<(i64, u64, u32)>,
    goals: &mut Vec<(u64, u32)>,
    nearby_scratch: &mut Vec<(bool, u32, u32)>,
    snapped_nodes: &mut HashSet<u64>,
    dirty_nodes: &mut Vec<u64>,
    dirty_seen: &mut HashSet<u64>,
) -> ConnectOutcome {
    let map_w = state.map.width;
    let mut remaining_search = PATH_SEARCH_LIMIT;
    let existing_outcome = connect_station_to_existing_track_cached(
        station,
        tracks,
        edge_distances,
        track_index,
        tile_counts,
        graph.as_deref_mut(),
        stations_by_id,
        state,
        &mut remaining_search,
        astar,
        nearby_scratch,
        snapped_nodes,
        dirty_nodes,
        dirty_seen,
    );
    if existing_outcome.tracks_mutated {
        return existing_outcome;
    }

    nearby_station_candidates_cached(station, stations_by_id, station_grid, map_w, candidates);
    goals.clear();
    goals.extend(
        candidates
            .iter()
            .filter(|(_, id, _)| processed.contains(id))
            .map(|(_, id, tile)| (*id, *tile)),
    );
    if let Ok((target_id, path, _explored)) = find_land_path_to_any_cached(
        &state.map,
        station.tile_idx,
        goals,
        remaining_search,
        astar,
    ) && path.len() >= 2
    {
        if let Some(intersection) =
            first_track_intersection(&path, station.tile_idx, tile_counts, track_index)
        {
            let junction = split_tracks_at_tile_cached(
                intersection,
                None,
                tracks,
                edge_distances,
                track_index,
                tile_counts,
                graph.as_deref_mut(),
                stations_by_id,
                map_w,
                dirty_nodes,
                dirty_seen,
            );
            if let Some(end) = path.iter().position(|tile| *tile == intersection)
                && end > 0
                && insert_track_cached(
                    station.id,
                    junction,
                    &path[..=end],
                    tracks,
                    edge_distances,
                    track_index,
                    tile_counts,
                    graph.as_deref_mut(),
                    stations_by_id,
                    map_w,
                )
            {
                if dirty_seen.insert(station.id) {
                    dirty_nodes.push(station.id);
                }
                if dirty_seen.insert(junction) {
                    dirty_nodes.push(junction);
                }
            }
            return ConnectOutcome {
                tracks_mutated: true,
                connected_via_fallback: true,
                split_edges: Vec::new(),
            };
        } else if insert_track_cached(
            station.id,
            target_id,
            &path,
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph,
            stations_by_id,
            map_w,
        ) {
            if dirty_seen.insert(station.id) {
                dirty_nodes.push(station.id);
            }
            if dirty_seen.insert(target_id) {
                dirty_nodes.push(target_id);
            }
            return ConnectOutcome {
                tracks_mutated: true,
                connected_via_fallback: true,
                split_edges: Vec::new(),
            };
        }
    }
    ConnectOutcome {
        tracks_mutated: false,
        connected_via_fallback: false,
        split_edges: Vec::new(),
    }
}

pub fn populate_trading_post_routes(
    stations: &[Building],
    tracks: &[RailwayTrack],
    state: &GameState,
    previous: &[RailwayLink],
) -> Vec<RailwayLink> {
    let mut cache = RailwayRuntimeCache::default();
    cache.hydrate(stations, tracks, previous, state.map.width, state.map.height);
    cache.ensure_graph();

    let previous_by_post: HashMap<_, _> = previous
        .iter()
        .map(|route| (route.from_building_id, route))
        .collect();
    let mut routes = Vec::new();
    for post in stations
        .iter()
        .filter(|building| building.kind == BuildingKind::TradingPost)
    {
        if let Some(link) = cache.plan_single_trading_post_route(
            *post,
            state,
            previous_by_post.get(&post.id).copied(),
        ) {
            routes.push(link);
        }
    }
    routes.sort_by_key(|route| route.from_building_id);
    routes
}

#[allow(clippy::too_many_arguments)]
fn make_link_cached(
    post: Building,
    destination: Building,
    station_path: Vec<u64>,
    tracks: &BTreeMap<(u64, u64), RailwayTrack>,
    stations: &HashMap<u64, Building>,
    distance: u32,
    edge_distances: &HashMap<(u64, u64), u32>,
    state: &GameState,
    previous: Option<&RailwayLink>,
) -> RailwayLink {
    let period_ticks = trading_post_period_ticks(distance, post.level, &state.config);
    let station_stop_count = station_path
        .iter()
        .filter_map(|id| stations.get(id))
        .filter(|station| station.id != post.id && station.kind.is_enabled())
        .count()
        .min(usize::from(u16::MAX)) as u16;
    let mut route_path = Vec::new();
    for pair in station_path.windows(2) {
        let Some(track) = tracks.get(&track_key(pair[0], pair[1])) else {
            continue;
        };
        if track.from_node_id == pair[0] {
            for &tile in &track.path {
                if route_path.last().copied() != Some(tile) {
                    route_path.push(tile);
                }
            }
        } else {
            for &tile in track.path.iter().rev() {
                if route_path.last().copied() != Some(tile) {
                    route_path.push(tile);
                }
            }
        }
    }
    let mut route = RailwayLink {
        from_building_id: post.id,
        to_building_id: destination.id,
        from_tile: post.tile_idx,
        owner_id: post.owner_id,
        level: post.level,
        distance,
        station_path_edge_distances: station_path
            .windows(2)
            .map(|pair| {
                edge_distances
                    .get(&track_key(pair[0], pair[1]))
                    .copied()
                    .unwrap_or_default()
                    .min(u32::from(u16::MAX)) as u16
            })
            .collect(),
        station_stop_count,
        route_path,
        station_path,
        period_ticks,
        opened_at: state.tick,
    };
    if let Some(previous) = previous.filter(|previous| same_route(previous, &route)) {
        if previous.level == route.level && previous.period_ticks == route.period_ticks {
            route.opened_at = previous.opened_at;
        } else {
            route.opened_at = phase_continuous_opened_at(
                state.tick,
                previous.opened_at,
                previous.period_ticks,
                route.period_ticks,
            );
        }
    }
    route
}

pub fn same_route(left: &RailwayLink, right: &RailwayLink) -> bool {
    left.from_building_id == right.from_building_id
        && left.to_building_id == right.to_building_id
        && left.owner_id == right.owner_id
        && left.distance == right.distance
        && left.station_path == right.station_path
        && left.route_path == right.route_path
        && left.station_path_edge_distances == right.station_path_edge_distances
}

pub fn initial_trade_route_progress(
    state: &GameState,
    route: &RailwayLink,
    stations: &[Building],
) -> TradeRouteProgress {
    let next_station_index = next_trade_stop_index(route, stations, 0);
    reschedule_trade_route_progress(
        state,
        route,
        TradeRouteProgress {
            from_station_index: 0,
            next_station_index: next_station_index as u16,
            returning: false,
            cargo_loaded: false,
            next_arrival_tick: state.tick,
            tick_remainder_micros: 0,
        },
    )
}

pub fn next_trade_stop_index(route: &RailwayLink, stations: &[Building], from: usize) -> usize {
    let last = route.station_path.len().saturating_sub(1);
    route
        .station_path
        .iter()
        .enumerate()
        .skip(from.saturating_add(1))
        .find_map(|(index, id)| {
            stations
                .binary_search_by_key(id, |station| station.id)
                .ok()
                .filter(|&position| {
                    stations[position].kind.is_enabled()
                })
                .map(|_| index)
        })
        .unwrap_or(last)
}

pub fn reschedule_trade_route_progress(
    state: &GameState,
    route: &RailwayLink,
    mut progress: TradeRouteProgress,
) -> TradeRouteProgress {
    let last = route.station_path.len().saturating_sub(1);
    if last == 0 {
        return progress;
    }
    let from = if progress.returning {
        last
    } else {
        usize::from(progress.from_station_index).min(last)
    };
    let to = usize::from(progress.next_station_index).min(last);
    let dwell = if from == 0 || progress.returning {
        7.0
    } else {
        0.0
    };
    let (ticks, remainder) = railway_leg_ticks(
        &state.config,
        route,
        from,
        to,
        dwell,
        progress.tick_remainder_micros,
    );
    progress.next_arrival_tick = state.tick.saturating_add(ticks);
    progress.tick_remainder_micros = remainder;
    progress
}

pub fn railway_leg_ticks(
    config: &GameConfig,
    route: &RailwayLink,
    from_station_index: usize,
    to_station_index: usize,
    dwell_seconds: f64,
    remainder_micros: i32,
) -> (u64, i32) {
    let start = from_station_index.min(to_station_index);
    let end = from_station_index.max(to_station_index);
    let distance: u32 = route
        .station_path_edge_distances
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .map(|&distance| u32::from(distance))
        .sum();
    let seconds =
        (dwell_seconds + f64::from(distance) * 0.35) / trading_post_speed_multiplier(route.level);
    let accumulated = seconds * NORMAL_GAME_SPEED / config.per_tick(1.0).max(f64::EPSILON)
        * 1_000_000.0
        + f64::from(remainder_micros);
    let ticks = (accumulated / 1_000_000.0)
        .ceil()
        .clamp(1.0, u64::MAX as f64) as u64;
    let remainder = (accumulated - ticks as f64 * 1_000_000.0).round() as i32;
    (ticks, remainder)
}

fn same_station_location(left: &Building, right: &Building) -> bool {
    left.kind == right.kind && left.tile_idx == right.tile_idx
}

#[derive(Clone)]
struct DijkstraScratch {
    visit_generation: u32,
    visited: Vec<u32>,
    distances: Vec<u32>,
    previous: Vec<usize>,
    buckets: Vec<Vec<usize>>,
}

impl Default for DijkstraScratch {
    fn default() -> Self {
        Self::new(0)
    }
}

impl DijkstraScratch {
    fn new(node_count: usize) -> Self {
        Self {
            visit_generation: 0,
            visited: vec![0; node_count],
            distances: vec![u32::MAX; node_count],
            previous: vec![usize::MAX; node_count],
            buckets: (0..=RAILWAY_MAX_ROUTE_DISTANCE)
                .map(|_| Vec::new())
                .collect(),
        }
    }

    fn prepare(&mut self, node_count: usize) {
        if self.visited.len() < node_count {
            self.visited.resize(node_count, 0);
            self.distances.resize(node_count, u32::MAX);
            self.previous.resize(node_count, usize::MAX);
        }
        if self.buckets.len() != RAILWAY_MAX_ROUTE_DISTANCE as usize + 1 {
            self.buckets = (0..=RAILWAY_MAX_ROUTE_DISTANCE)
                .map(|_| Vec::new())
                .collect();
        }
    }

    fn begin(&mut self) -> u32 {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.visit_generation = self.visit_generation.wrapping_add(1);
        if self.visit_generation == 0 {
            self.visited.fill(0);
            self.visit_generation = 1;
        }
        self.visit_generation
    }
}

#[derive(Clone, Default)]
pub struct AStarScratch {
    open: BinaryHeap<Reverse<(u32, u32, u32)>>,
    seen_gen: Vec<u32>,
    cost: Vec<u32>,
    previous: Vec<u32>,
    goal_gen: Vec<u32>,
    current_gen: u32,
    current_goal_gen: u32,
    goal_xy: Vec<(i32, i32)>,
    goal_tiles: Vec<u32>,
}

impl AStarScratch {
    fn prepare_arrays(&mut self, map_len: usize) {
        if self.seen_gen.len() < map_len {
            self.seen_gen.resize(map_len, 0);
            self.cost.resize(map_len, u32::MAX);
            self.previous.resize(map_len, u32::MAX);
            self.goal_gen.resize(map_len, 0);
        }
        self.current_gen = self.current_gen.wrapping_add(1);
        if self.current_gen == 0 {
            self.seen_gen.fill(0);
            self.current_gen = 1;
        }
        self.current_goal_gen = self.current_goal_gen.wrapping_add(1);
        if self.current_goal_gen == 0 {
            self.goal_gen.fill(0);
            self.current_goal_gen = 1;
        }
    }
}

/// Single-pass dual-target bucketed Dijkstra: tracks the farthest reachable City/Port and falls back
/// to the farthest reachable TradingPost without rerunning Dijkstra when no City/Port exists.
fn farthest_reachable_dual(
    source: usize,
    node_ids: &[u64],
    graph: &[Vec<(usize, u32)>],
    stations: &[Option<Building>],
    scratch: &mut DijkstraScratch,
) -> Option<(u32, Building, Vec<u64>)> {
    let generation = scratch.begin();
    scratch.visited[source] = generation;
    scratch.distances[source] = 0;
    scratch.previous[source] = source;
    scratch.buckets[0].push(source);
    let mut best_commercial: Option<(u32, u64, usize)> = None;
    let mut best_any: Option<(u32, u64, usize)> = None;
    for distance in 0..=RAILWAY_MAX_ROUTE_DISTANCE {
        let mut cursor = 0;
        while cursor < scratch.buckets[distance as usize].len() {
            let node = scratch.buckets[distance as usize][cursor];
            cursor += 1;
            if scratch.visited[node] != generation || distance != scratch.distances[node] {
                continue;
            }
            if node != source
                && let Some(station) = stations[node]
            {
                let is_better = |cur: Option<(u32, u64, usize)>| {
                    cur.is_none_or(|(best_distance, best_id, _)| {
                        distance > best_distance
                            || (distance == best_distance && station.id < best_id)
                    })
                };
                if is_better(best_any) {
                    best_any = Some((distance, station.id, node));
                }
                if matches!(station.kind, BuildingKind::City | BuildingKind::Port)
                    && is_better(best_commercial)
                {
                    best_commercial = Some((distance, station.id, node));
                }
            }
            for &(next, edge_distance) in &graph[node] {
                let next_distance = distance.saturating_add(edge_distance);
                if next_distance > RAILWAY_MAX_ROUTE_DISTANCE
                    || (scratch.visited[next] == generation
                        && scratch.distances[next] <= next_distance)
                {
                    continue;
                }
                scratch.visited[next] = generation;
                scratch.distances[next] = next_distance;
                scratch.previous[next] = node;
                scratch.buckets[next_distance as usize].push(next);
            }
        }
    }
    let (distance, _, destination) = best_commercial.or(best_any)?;
    let mut path = vec![destination];
    while *path.last()? != source {
        path.push(scratch.previous[*path.last()?]);
    }
    path.reverse();
    Some((
        distance,
        stations[destination]?,
        path.into_iter().map(|index| node_ids[index]).collect(),
    ))
}

fn nearby_station_candidates_cached(
    station: Building,
    stations_by_id: &HashMap<u64, Building>,
    grid: &HashMap<(i32, i32), Vec<u64>>,
    map_width: u32,
    candidates: &mut Vec<(i64, u64, u32)>,
) {
    candidates.clear();
    let (x, y) = tile_xy(station.tile_idx, map_width);
    let (cell_x, cell_y) = (x / STATION_GRID_CELL, y / STATION_GRID_CELL);
    let radius_squared = i64::from(RAILWAY_LINK_DISTANCE).pow(2);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(ids) = grid.get(&(cell_x + dx, cell_y + dy)) else {
                continue;
            };
            for &cand_id in ids {
                if cand_id == station.id {
                    continue;
                }
                let Some(&candidate) = stations_by_id.get(&cand_id) else {
                    continue;
                };
                let (candidate_x, candidate_y) = tile_xy(candidate.tile_idx, map_width);
                let distance_squared =
                    i64::from(x - candidate_x).pow(2) + i64::from(y - candidate_y).pow(2);
                if distance_squared > radius_squared {
                    continue;
                }
                candidates.push((distance_squared, candidate.id, candidate.tile_idx));
            }
        }
    }
    if candidates.len() > RAILWAY_MAX_CANDIDATES_PER_STATION {
        candidates.select_nth_unstable_by_key(RAILWAY_MAX_CANDIDATES_PER_STATION, |candidate| {
            (candidate.0, candidate.1)
        });
        candidates.truncate(RAILWAY_MAX_CANDIDATES_PER_STATION);
    }
    candidates.sort_unstable_by_key(|candidate| (candidate.0, candidate.1));
}

#[inline]
fn has_track_at(
    tile: u32,
    tile_counts: &[u16],
    track_index: &HashMap<u32, Vec<(u64, u64)>>,
) -> bool {
    if !tile_counts.is_empty() {
        tile_counts.get(tile as usize).copied().unwrap_or(0) > 0
    } else {
        track_index.contains_key(&tile)
    }
}

#[allow(clippy::too_many_arguments)]
fn connect_station_to_existing_track_cached(
    station: Building,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: &mut HashMap<(u64, u64), u32>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: &mut Vec<u16>,
    mut graph: Option<&mut TrackGraphCache>,
    stations_by_id: &HashMap<u64, Building>,
    state: &GameState,
    remaining_search: &mut usize,
    astar: &mut AStarScratch,
    nearby: &mut Vec<(bool, u32, u32)>,
    snapped_nodes: &mut HashSet<u64>,
    dirty_nodes: &mut Vec<u64>,
    dirty_seen: &mut HashSet<u64>,
) -> ConnectOutcome {
    let (x, y) = tile_xy(station.tile_idx, state.map.width);
    let footprint = BuildingFootprint::at(station.kind, x.max(0) as u32, y.max(0) as u32);
    snapped_nodes.clear();
    snapped_nodes.insert(station.id);
    let mut split_edges = Vec::new();
    let mut tracks_mutated = false;

    if has_track_at(station.tile_idx, tile_counts, track_index) {
        if let Some(existing_keys) = track_index.get(&station.tile_idx) {
            for &(a, b) in existing_keys {
                snapped_nodes.insert(a);
                snapped_nodes.insert(b);
                split_edges.push((a, b));
            }
        }
        let junction = split_tracks_at_station_tile_cached(
            station.tile_idx,
            station.id,
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            stations_by_id,
            state.map.width,
            dirty_nodes,
            dirty_seen,
        );
        snapped_nodes.insert(junction);
        if junction != station.id
            && insert_track_cached(
                station.id,
                junction,
                &[station.tile_idx],
                tracks,
                edge_distances,
                track_index,
                tile_counts,
                graph.as_deref_mut(),
                stations_by_id,
                state.map.width,
            )
        {
            if dirty_seen.insert(station.id) {
                dirty_nodes.push(station.id);
            }
            if dirty_seen.insert(junction) {
                dirty_nodes.push(junction);
            }
        }
        tracks_mutated = true;
    }

    nearby.clear();
    for dy in -RAIL_JOIN_RADIUS..=RAIL_JOIN_RADIUS {
        for dx in -RAIL_JOIN_RADIUS..=RAIL_JOIN_RADIUS {
            let distance_squared = (dx * dx + dy * dy) as u32;
            if distance_squared == 0 || distance_squared > (RAIL_JOIN_RADIUS * RAIL_JOIN_RADIUS) as u32 {
                continue;
            }
            let tx = x + dx;
            let ty = y + dy;
            if tx < 0 || ty < 0 || tx >= state.map.width as i32 || ty >= state.map.height as i32 {
                continue;
            }
            let tile = ty as u32 * state.map.width + tx as u32;
            if has_track_at(tile, tile_counts, track_index) {
                nearby.push((!footprint.contains(tx, ty), distance_squared, tile));
            }
        }
    }
    nearby.sort_unstable();
    for i in 0..nearby.len() {
        let target_tile = nearby[i].2;
        let has_unconnected_track = track_index.get(&target_tile).is_some_and(|keys| {
            keys.iter()
                .any(|&(a, b)| !snapped_nodes.contains(&a) && !snapped_nodes.contains(&b))
        });
        if !has_unconnected_track {
            continue;
        }
        if *remaining_search == 0 {
            break;
        }
        let (path, explored) = match find_land_path_cached(
            &state.map,
            station.tile_idx,
            target_tile,
            *remaining_search,
            astar,
        ) {
            Ok(path) => path,
            Err(explored) => {
                *remaining_search = remaining_search.saturating_sub(explored);
                continue;
            }
        };
        *remaining_search = remaining_search.saturating_sub(explored);
        if path.len() < 2 {
            continue;
        }
        let intersection =
            first_track_intersection(&path, station.tile_idx, tile_counts, track_index)
                .unwrap_or(target_tile);
        let Some(end) = path.iter().position(|tile| *tile == intersection) else {
            continue;
        };
        if let Some(existing_keys) = track_index.get(&intersection) {
            for &(a, b) in existing_keys {
                snapped_nodes.insert(a);
                snapped_nodes.insert(b);
                if !split_edges.contains(&(a, b)) {
                    split_edges.push((a, b));
                }
            }
        }
        let junction = split_tracks_at_tile_cached(
            intersection,
            None,
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            stations_by_id,
            state.map.width,
            dirty_nodes,
            dirty_seen,
        );
        snapped_nodes.insert(junction);
        if insert_track_cached(
            station.id,
            junction,
            &path[..=end],
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            stations_by_id,
            state.map.width,
        ) {
            if dirty_seen.insert(station.id) {
                dirty_nodes.push(station.id);
            }
            if dirty_seen.insert(junction) {
                dirty_nodes.push(junction);
            }
        }
        tracks_mutated = true;
    }
    ConnectOutcome {
        tracks_mutated,
        connected_via_fallback: false,
        split_edges,
    }
}

fn find_land_path_to_any_cached(
    map: &GameMap,
    start: u32,
    goals: &[(u64, u32)],
    budget: usize,
    astar: &mut AStarScratch,
) -> Result<(u64, Vec<u32>, usize), usize> {
    if goals.is_empty() {
        return Err(0);
    }
    let (start_x, start_y) = tile_xy(start, map.width);
    for &(id, goal) in goals {
        let (goal_x, goal_y) = tile_xy(goal, map.width);
        if (start_x - goal_x).abs() + (start_y - goal_y).abs() > RAILWAY_MAX_ROUTE_DISTANCE as i32 {
            continue;
        }
        if let Some(path) = l_shaped_land_path(map, start_x, start_y, goal_x, goal_y) {
            return Ok((id, path, 0));
        }
    }

    astar.goal_tiles.clear();
    for &(_, tile) in goals {
        if !astar.goal_tiles.contains(&tile) {
            astar.goal_tiles.push(tile);
        }
    }
    let goal_tiles = std::mem::take(&mut astar.goal_tiles);
    let result = find_path_a_star_cached(map, start, &goal_tiles, budget, astar);
    astar.goal_tiles = goal_tiles;
    let (path, explored) = result?;
    let goal = *path.last().ok_or(explored)?;
    let id = goals
        .iter()
        .filter(|&&(_, t)| t == goal)
        .map(|&(id, _)| id)
        .min()
        .ok_or(explored)?;
    Ok((id, path, explored))
}

fn find_land_path_cached(
    map: &GameMap,
    start: u32,
    goal: u32,
    budget: usize,
    astar: &mut AStarScratch,
) -> Result<(Vec<u32>, usize), usize> {
    if start == goal {
        return Ok((vec![start], 0));
    }
    let (start_x, start_y) = tile_xy(start, map.width);
    let (goal_x, goal_y) = tile_xy(goal, map.width);
    if let Some(path) = l_shaped_land_path(map, start_x, start_y, goal_x, goal_y) {
        return Ok((path, 0));
    }
    find_path_a_star_cached(map, start, &[goal], budget, astar)
}

fn l_shaped_land_path(map: &GameMap, ax: i32, ay: i32, bx: i32, by: i32) -> Option<Vec<u32>> {
    for (cx, cy) in [(bx, ay), (ax, by)] {
        let mut path = Vec::new();
        if append_axis(map, ax, ay, cx, cy, &mut path).is_none()
            || append_axis(map, cx, cy, bx, by, &mut path).is_none()
        {
            continue;
        }
        if path.len().saturating_sub(1) <= RAILWAY_MAX_ROUTE_DISTANCE as usize {
            return Some(path);
        }
    }
    None
}

fn append_axis(
    map: &GameMap,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    path: &mut Vec<u32>,
) -> Option<()> {
    if x0 != x1 && y0 != y1 {
        return None;
    }
    let (mut x, mut y) = (x0, y0);
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    loop {
        if x < 0 || y < 0 || x >= map.width as i32 || y >= map.height as i32 {
            return None;
        }
        let tile = y as u32 * map.width + x as u32;
        if !map
            .terrain
            .get(tile as usize)
            .is_some_and(crate::map::MapTile::is_land)
        {
            return None;
        }
        if path.last().copied() != Some(tile) {
            path.push(tile);
        }
        if x == x1 && y == y1 {
            return Some(());
        }
        x += sx;
        y += sy;
    }
}

fn find_path_a_star_cached(
    map: &GameMap,
    start: u32,
    goals: &[u32],
    budget: usize,
    scratch: &mut AStarScratch,
) -> Result<(Vec<u32>, usize), usize> {
    let map_len = (map.width as usize).saturating_mul(map.height as usize);
    if goals.is_empty() || budget == 0 || start as usize >= map_len {
        return Err(0);
    }
    scratch.open.clear();
    scratch.goal_xy.clear();
    scratch.prepare_arrays(map_len);
    let gen_id = scratch.current_gen;
    let goal_gen = scratch.current_goal_gen;

    for &g in goals {
        if (g as usize) < map_len {
            scratch.goal_gen[g as usize] = goal_gen;
            scratch.goal_xy.push(tile_xy(g, map.width));
        }
    }
    if scratch.goal_xy.is_empty() {
        return Err(0);
    }

    let (sx, sy) = tile_xy(start, map.width);
    scratch
        .open
        .push(Reverse((heuristic_xy(sx, sy, &scratch.goal_xy), 0u32, start)));
    scratch.seen_gen[start as usize] = gen_id;
    scratch.cost[start as usize] = 0u32;
    scratch.previous[start as usize] = start;
    let mut explored = 0;
    while let Some(Reverse((_, distance, tile))) = scratch.open.pop() {
        if explored >= budget {
            break;
        }
        let tile_usize = tile as usize;
        if scratch.seen_gen[tile_usize] != gen_id || scratch.cost[tile_usize] != distance {
            continue;
        }
        explored += 1;
        if scratch.goal_gen[tile_usize] == goal_gen {
            let mut path = vec![tile];
            let mut cur = tile;
            while cur != start {
                let prev = scratch.previous[cur as usize];
                if prev == u32::MAX || prev == cur {
                    return Err(explored);
                }
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return if path.len().saturating_sub(1) <= RAILWAY_MAX_ROUTE_DISTANCE as usize {
                Ok((path, explored))
            } else {
                Err(explored)
            };
        }
        if distance >= RAILWAY_MAX_ROUTE_DISTANCE {
            continue;
        }
        let x = (tile % map.width) as i32;
        let y = (tile / map.width) as i32;
        for (dx, dy) in CARDINAL_NEIGHBOR_DELTAS {
            let next_x = x + dx;
            let next_y = y + dy;
            if next_x < 0 || next_y < 0 || next_x >= map.width as i32 || next_y >= map.height as i32
            {
                continue;
            }
            let next = next_y as u32 * map.width + next_x as u32;
            let next_usize = next as usize;
            if !map
                .terrain
                .get(next_usize)
                .is_some_and(crate::map::MapTile::is_land)
            {
                continue;
            }
            let next_cost = distance + 1;
            if scratch.seen_gen[next_usize] == gen_id && scratch.cost[next_usize] <= next_cost {
                continue;
            }
            scratch.seen_gen[next_usize] = gen_id;
            scratch.cost[next_usize] = next_cost;
            scratch.previous[next_usize] = tile;
            scratch.open.push(Reverse((
                next_cost + heuristic_xy(next_x, next_y, &scratch.goal_xy),
                next_cost,
                next,
            )));
        }
    }
    Err(explored)
}

#[inline]
fn heuristic_xy(x: i32, y: i32, goals: &[(i32, i32)]) -> u32 {
    goals
        .iter()
        .map(|&(gx, gy)| (x - gx).unsigned_abs() + (y - gy).unsigned_abs())
        .min()
        .unwrap_or(0)
}

fn first_track_intersection(
    path: &[u32],
    start: u32,
    tile_counts: &[u16],
    track_index: &HashMap<u32, Vec<(u64, u64)>>,
) -> Option<u32> {
    path.iter()
        .copied()
        .skip_while(|tile| *tile == start)
        .find(|tile| has_track_at(*tile, tile_counts, track_index))
}

#[allow(clippy::too_many_arguments)]
fn split_tracks_at_station_tile_cached(
    tile: u32,
    station_id: u64,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: &mut HashMap<(u64, u64), u32>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: &mut Vec<u16>,
    mut graph: Option<&mut TrackGraphCache>,
    stations_by_id: &HashMap<u64, Building>,
    map_width: u32,
    dirty_nodes: &mut Vec<u64>,
    dirty_seen: &mut HashSet<u64>,
) -> u64 {
    let mut endpoint_junctions: Vec<_> = track_index
        .get(&tile)
        .into_iter()
        .flatten()
        .filter_map(|key| tracks.get(key))
        .flat_map(|track| {
            let path = expand_path(&track.path, map_width);
            let mut endpoints = Vec::with_capacity(2);
            if path.first() == Some(&tile) && track.from_node_id & JUNCTION_ID_BIT != 0 {
                endpoints.push(track.from_node_id);
            }
            if path.last() == Some(&tile) && track.to_node_id & JUNCTION_ID_BIT != 0 {
                endpoints.push(track.to_node_id);
            }
            endpoints
        })
        .collect();
    endpoint_junctions.sort_unstable();
    endpoint_junctions.dedup();
    for junction in endpoint_junctions {
        replace_track_node_cached(
            junction,
            station_id,
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            stations_by_id,
            map_width,
            dirty_nodes,
            dirty_seen,
        );
    }
    let is_interior = track_index
        .get(&tile)
        .into_iter()
        .flatten()
        .filter_map(|key| tracks.get(key))
        .any(|track| {
            let path = expand_path(&track.path, map_width);
            path.iter()
                .position(|&current| current == tile)
                .is_some_and(|index| index > 0 && index + 1 < path.len())
        });
    split_tracks_at_tile_cached(
        tile,
        is_interior.then_some(station_id),
        tracks,
        edge_distances,
        track_index,
        tile_counts,
        graph,
        stations_by_id,
        map_width,
        dirty_nodes,
        dirty_seen,
    )
}

#[allow(clippy::too_many_arguments)]
fn replace_track_node_cached(
    old_node: u64,
    new_node: u64,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: &mut HashMap<(u64, u64), u32>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: &mut Vec<u16>,
    mut graph: Option<&mut TrackGraphCache>,
    stations_by_id: &HashMap<u64, Building>,
    map_width: u32,
    dirty_nodes: &mut Vec<u64>,
    dirty_seen: &mut HashSet<u64>,
) {
    let edges: Vec<_> = tracks
        .values()
        .filter(|track| track.from_node_id == old_node || track.to_node_id == old_node)
        .cloned()
        .collect();
    for track in edges {
        let path = expand_path(&track.path, map_width);
        remove_track_cached(
            track_key(track.from_node_id, track.to_node_id),
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            map_width,
        );
        let from = if track.from_node_id == old_node {
            new_node
        } else {
            track.from_node_id
        };
        let to = if track.to_node_id == old_node {
            new_node
        } else {
            track.to_node_id
        };
        if insert_track_cached(
            from,
            to,
            &path,
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            stations_by_id,
            map_width,
        ) {
            if dirty_seen.insert(from) {
                dirty_nodes.push(from);
            }
            if dirty_seen.insert(to) {
                dirty_nodes.push(to);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn split_tracks_at_tile_cached(
    tile: u32,
    preferred_junction: Option<u64>,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: &mut HashMap<(u64, u64), u32>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: &mut Vec<u16>,
    mut graph: Option<&mut TrackGraphCache>,
    stations_by_id: &HashMap<u64, Building>,
    map_width: u32,
    dirty_nodes: &mut Vec<u64>,
    dirty_seen: &mut HashSet<u64>,
) -> u64 {
    let keys = track_index.get(&tile).cloned().unwrap_or_default();
    let endpoint = keys.iter().find_map(|key| {
        let track = tracks.get(key)?;
        let path = expand_path(&track.path, map_width);
        ((path.first() == Some(&tile)).then_some(track.from_node_id))
            .or_else(|| (path.last() == Some(&tile)).then_some(track.to_node_id))
    });
    let junction = preferred_junction
        .or(endpoint)
        .unwrap_or(JUNCTION_ID_BIT | u64::from(tile));
    for key in keys {
        let Some(track) = tracks.get(&key).cloned() else {
            continue;
        };
        let path = expand_path(&track.path, map_width);
        let Some(index) = path.iter().position(|&current| current == tile) else {
            continue;
        };
        if index == 0 || index + 1 == path.len() {
            continue;
        }
        remove_track_cached(
            key,
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            map_width,
        );
        if insert_track_cached(
            track.from_node_id,
            junction,
            &path[..=index],
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            stations_by_id,
            map_width,
        ) && dirty_seen.insert(track.from_node_id)
        {
            dirty_nodes.push(track.from_node_id);
        }
        if insert_track_cached(
            junction,
            track.to_node_id,
            &path[index..],
            tracks,
            edge_distances,
            track_index,
            tile_counts,
            graph.as_deref_mut(),
            stations_by_id,
            map_width,
        ) && dirty_seen.insert(track.to_node_id)
        {
            dirty_nodes.push(track.to_node_id);
        }
        if dirty_seen.insert(junction) {
            dirty_nodes.push(junction);
        }
    }
    junction
}

#[allow(clippy::too_many_arguments)]
fn insert_track_cached(
    from: u64,
    to: u64,
    path: &[u32],
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: &mut HashMap<(u64, u64), u32>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: &mut Vec<u16>,
    graph: Option<&mut TrackGraphCache>,
    stations_by_id: &HashMap<u64, Building>,
    map_width: u32,
) -> bool {
    if from == to || path.is_empty() {
        return false;
    }
    let key = track_key(from, to);
    if tracks.contains_key(&key) {
        return false;
    }
    let oriented_path = if from < to {
        path.to_vec()
    } else {
        path.iter().rev().copied().collect()
    };
    let compressed = if oriented_path.len() == 1 {
        oriented_path.clone()
    } else {
        compress_path(&oriented_path, map_width)
    };
    let dist = polyline_distance(&compressed, map_width);
    if dist > RAILWAY_MAX_ROUTE_DISTANCE {
        return false;
    }
    tracks.insert(
        key,
        RailwayTrack {
            from_node_id: key.0,
            to_node_id: key.1,
            path: compressed,
        },
    );
    edge_distances.insert(key, dist);
    if let Some(graph) = graph {
        graph.upsert_edge(key.0, key.1, dist, stations_by_id);
    }
    for tile in oriented_path {
        let keys = track_index.entry(tile).or_default();
        if !keys.contains(&key) {
            keys.push(key);
            keys.sort_unstable();
            if let Some(count) = tile_counts.get_mut(tile as usize) {
                *count = count.saturating_add(1);
            }
        }
    }
    true
}

fn remove_track_cached(
    key: (u64, u64),
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    edge_distances: &mut HashMap<(u64, u64), u32>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    tile_counts: &mut Vec<u16>,
    graph: Option<&mut TrackGraphCache>,
    map_width: u32,
) {
    let Some(track) = tracks.remove(&key) else {
        return;
    };
    edge_distances.remove(&key);
    if let Some(graph) = graph {
        graph.remove_edge(key.0, key.1);
    }
    for tile in expand_path(&track.path, map_width) {
        if let Some(keys) = track_index.get_mut(&tile) {
            let before = keys.len();
            keys.retain(|candidate| *candidate != key);
            if keys.len() != before
                && let Some(count) = tile_counts.get_mut(tile as usize)
            {
                *count = count.saturating_sub(1);
            }
            if keys.is_empty() {
                track_index.remove(&tile);
            }
        }
    }
}

pub fn index_tracks(tracks: &[RailwayTrack], map_width: u32) -> HashMap<u32, Vec<(u64, u64)>> {
    let mut index = HashMap::<u32, Vec<(u64, u64)>>::new();
    for track in tracks {
        let key = track_key(track.from_node_id, track.to_node_id);
        for tile in expand_path(&track.path, map_width) {
            index.entry(tile).or_default().push(key);
        }
    }
    for keys in index.values_mut() {
        keys.sort_unstable();
        keys.dedup();
    }
    index
}

fn index_tracks_with_counts(
    tracks: &[RailwayTrack],
    map_width: u32,
    tile_counts: &mut Vec<u16>,
) -> HashMap<u32, Vec<(u64, u64)>> {
    let index = index_tracks(tracks, map_width);
    for (&tile, keys) in &index {
        if let Some(slot) = tile_counts.get_mut(tile as usize) {
            *slot = keys.len().min(usize::from(u16::MAX)) as u16;
        }
    }
    index
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RailPlacementPreview {
    /// Existing track segment keys (`(from.min(to), from.max(to))`) touched by footprint or radius-3 snap.
    pub snap_tracks: Vec<(u64, u64)>,
    /// Proposed new track tile paths shown semi-transparently before confirmation.
    pub ghost_paths: Vec<Vec<u32>>,
}

/// Computes read-only OpenFront-style placement preview data (`snap_tracks` + `ghost_paths`)
/// for placing `kind` at `tile_idx`.
pub fn preview_building_rail_connection(
    track_index: &HashMap<u32, Vec<(u64, u64)>>,
    station_tiles: &[(u64, u32)],
    map: &GameMap,
    kind: BuildingKind,
    tile_idx: u32,
    astar: &mut AStarScratch,
) -> RailPlacementPreview {
    if !kind.is_enabled()
        || map.width == 0
        || map.height == 0
        || !map
            .terrain
            .get(tile_idx as usize)
            .is_some_and(crate::map::MapTile::is_land)
    {
        return RailPlacementPreview::default();
    }
    let mut preview = RailPlacementPreview::default();
    let (x, y) = tile_xy(tile_idx, map.width);
    let footprint = BuildingFootprint::at(kind, x.max(0) as u32, y.max(0) as u32);
    let mut seen_nodes: HashSet<u64> = HashSet::new();

    if let Some(keys) = track_index.get(&tile_idx) {
        for &(a, b) in keys {
            let key = track_key(a, b);
            if !preview.snap_tracks.contains(&key) {
                preview.snap_tracks.push(key);
            }
            seen_nodes.insert(a);
            seen_nodes.insert(b);
        }
    }

    let mut nearby: Vec<(bool, u32, u32)> = Vec::new();
    for dy in -RAIL_JOIN_RADIUS..=RAIL_JOIN_RADIUS {
        for dx in -RAIL_JOIN_RADIUS..=RAIL_JOIN_RADIUS {
            let dist_sq = (dx * dx + dy * dy) as u32;
            if dist_sq == 0 || dist_sq > (RAIL_JOIN_RADIUS * RAIL_JOIN_RADIUS) as u32 {
                continue;
            }
            let tx = x + dx;
            let ty = y + dy;
            if tx < 0 || ty < 0 || tx >= map.width as i32 || ty >= map.height as i32 {
                continue;
            }
            let tile = ty as u32 * map.width + tx as u32;
            if track_index.contains_key(&tile) {
                nearby.push((!footprint.contains(tx, ty), dist_sq, tile));
            }
        }
    }
    nearby.sort_unstable();
    let mut budget = PATH_SEARCH_LIMIT;
    for (_, _, target_tile) in nearby {
        let Some(keys) = track_index.get(&target_tile) else {
            continue;
        };
        let has_new_line = keys
            .iter()
            .any(|&(a, b)| !seen_nodes.contains(&a) && !seen_nodes.contains(&b));
        if !has_new_line {
            continue;
        }
        if budget == 0 {
            break;
        }
        match find_land_path_cached(map, tile_idx, target_tile, budget, astar) {
            Ok((path, explored)) => {
                budget = budget.saturating_sub(explored);
                if path.len() >= 2 {
                    let intersection = first_track_intersection(&path, tile_idx, &[], track_index)
                        .unwrap_or(target_tile);
                    if let Some(end) = path.iter().position(|&t| t == intersection) {
                        preview.ghost_paths.push(path[..=end].to_vec());
                    }
                    if let Some(int_keys) = track_index.get(&intersection) {
                        for &(a, b) in int_keys {
                            let key = track_key(a, b);
                            if !preview.snap_tracks.contains(&key) {
                                preview.snap_tracks.push(key);
                            }
                            seen_nodes.insert(a);
                            seen_nodes.insert(b);
                        }
                    }
                }
            }
            Err(explored) => {
                budget = budget.saturating_sub(explored);
            }
        }
    }

    if !preview.snap_tracks.is_empty() {
        return preview;
    }

    let radius_sq = i64::from(RAILWAY_LINK_DISTANCE).pow(2);
    let mut candidates: Vec<(i64, u64, u32)> = Vec::new();
    for &(cand_id, cand_tile) in station_tiles {
        let (cx, cy) = tile_xy(cand_tile, map.width);
        let d_sq = i64::from(x - cx).pow(2) + i64::from(y - cy).pow(2);
        if d_sq > 0 && d_sq <= radius_sq {
            candidates.push((d_sq, cand_id, cand_tile));
        }
    }
    if candidates.len() > RAILWAY_MAX_CANDIDATES_PER_STATION {
        candidates.select_nth_unstable_by_key(RAILWAY_MAX_CANDIDATES_PER_STATION, |c| (c.0, c.1));
        candidates.truncate(RAILWAY_MAX_CANDIDATES_PER_STATION);
    }
    candidates.sort_unstable_by_key(|c| (c.0, c.1));
    let goals: Vec<(u64, u32)> = candidates.into_iter().map(|(_, id, tile)| (id, tile)).collect();
    if let Ok((_, path, _)) = find_land_path_to_any_cached(map, tile_idx, &goals, budget, astar)
        && path.len() >= 2
    {
        if let Some(intersection) = first_track_intersection(&path, tile_idx, &[], track_index) {
            if let Some(end) = path.iter().position(|&t| t == intersection) && end > 0 {
                preview.ghost_paths.push(path[..=end].to_vec());
            }
            if let Some(keys) = track_index.get(&intersection) {
                for &(a, b) in keys {
                    let key = track_key(a, b);
                    if !preview.snap_tracks.contains(&key) {
                        preview.snap_tracks.push(key);
                    }
                }
            }
        } else {
            preview.ghost_paths.push(path);
        }
    }
    preview
}

pub fn compress_path(path: &[u32], map_width: u32) -> Vec<u32> {
    if path.len() <= 2 || map_width == 0 {
        return path.to_vec();
    }
    let mut compressed = vec![path[0]];
    for window in path.windows(3) {
        let a = tile_xy(window[0], map_width);
        let b = tile_xy(window[1], map_width);
        let c = tile_xy(window[2], map_width);
        if (b.0 - a.0, b.1 - a.1) != (c.0 - b.0, c.1 - b.1) {
            compressed.push(window[1]);
        }
    }
    compressed.push(*path.last().unwrap_or(&path[0]));
    compressed
}

pub fn expand_path(turns: &[u32], map_width: u32) -> Vec<u32> {
    if turns.len() <= 1 || map_width == 0 {
        return turns.to_vec();
    }
    let mut path = Vec::new();
    for pair in turns.windows(2) {
        let (x0, y0) = tile_xy(pair[0], map_width);
        let (x1, y1) = tile_xy(pair[1], map_width);
        let (mut x, mut y) = (x0, y0);
        loop {
            if x >= 0 && y >= 0 {
                let tile = y as u32 * map_width + x as u32;
                if path.last().copied() != Some(tile) {
                    path.push(tile);
                }
            }
            if x == x1 && y == y1 {
                break;
            }
            if x != x1 {
                x += (x1 - x).signum();
            } else {
                y += (y1 - y).signum();
            }
        }
    }
    path
}

pub fn polyline_distance(path: &[u32], map_width: u32) -> u32 {
    if map_width == 0 {
        return 0;
    }
    path.windows(2)
        .map(|pair| {
            let (x0, y0) = tile_xy(pair[0], map_width);
            let (x1, y1) = tile_xy(pair[1], map_width);
            x0.abs_diff(x1).saturating_add(y0.abs_diff(y1))
        })
        .sum()
}

fn tile_xy(tile: u32, map_width: u32) -> (i32, i32) {
    ((tile % map_width) as i32, (tile / map_width) as i32)
}

fn track_key(from: u64, to: u64) -> (u64, u64) {
    (from.min(to), max(from, to))
}

#[cfg(test)]
mod incremental_tests {
    use super::*;
    use crate::building::PendingTradeRoute;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
    use crate::map::MapTile;
    use wyrand::WyRand;

    fn land_game(w: u32, h: u32) -> GameState {
        let mut state = GameState::new(7, w, h, GameConfig::default());
        for y in 0..h {
            for x in 0..w {
                state.map.set_owner_id(x, y, 1);
                let r = state.map.ref_id(x, y);
                state.map.terrain[r] = MapTile::from_byte(0b1000_0000);
            }
        }
        state
    }

    #[test]
    fn incremental_railway_matches_full_recompute_parity() {
        let (w, h) = (180u32, 180u32);
        let mut state = land_game(w, h);
        let mut rng = WyRand::new(0xC0FF_EE11);
        let mut stations: Vec<Building> = Vec::new();
        let mut prev_stations: Vec<Building> = Vec::new();
        let mut full_tracks: Vec<RailwayTrack> = Vec::new();
        let mut full_index = HashMap::new();
        let mut full_ready = false;

        let mut cache = RailwayRuntimeCache::default();
        let mut inc_links: Vec<RailwayLink> = Vec::new();
        let mut next_id = 1u64;

        for step in 0..120u64 {
            state.tick = step + 1;
            // Add 2 stations per step, occasionally capture or upgrade an existing station.
            for _ in 0..2 {
                let rx = (rng.rand() % (w as u64 - 8)) as u32 + 4;
                let ry = (rng.rand() % (h as u64 - 8)) as u32 + 4;
                let tile = ry * w + rx;
                if stations.iter().any(|s| s.tile_idx == tile) {
                    continue;
                }
                let kind = match rng.rand() % 6 {
                    0 => BuildingKind::City,
                    1 => BuildingKind::Port,
                    2 => BuildingKind::Tower,
                    3 => BuildingKind::Farm,
                    _ => BuildingKind::TradingPost,
                };
                stations.push(Building {
                    id: next_id,
                    owner_id: ((rng.rand() % 4) + 1) as u16,
                    tile_idx: tile,
                    kind,
                    level: if kind == BuildingKind::TradingPost { 1 } else { 1 },
                    under_construction: false,
                    ticks_until_complete: 0,
                });
                next_id += 1;
            }
            if !stations.is_empty() && step % 5 == 0 {
                let idx = (rng.rand() as usize) % stations.len();
                stations[idx].owner_id = ((stations[idx].owner_id) % 4) + 1;
                if stations[idx].kind == BuildingKind::TradingPost && stations[idx].level < 8 {
                    stations[idx].level += 1;
                }
            }
            stations.sort_by_key(|s| s.id);

            // Reference full computation
            full_tracks = populate_railway_network(
                &stations,
                &prev_stations,
                &state,
                &full_tracks,
                &mut full_index,
                &mut full_ready,
            );
            let full_links =
                populate_trading_post_routes(&stations, &full_tracks, &state, &[]);
            prev_stations = stations.clone();

            // Incremental computation
            let pending: Vec<PendingTradeRoute> = vec![PendingTradeRoute::default(); inc_links.len()];
            let delta = cache.apply_topology_delta(
                &stations,
                &state,
                &inc_links,
                &pending,
                step == 0,
            );
            let inc_tracks = cache.tracks_vec();
            assert_eq!(
                inc_tracks, full_tracks,
                "tracks diverged at step {step}"
            );

            let affected: HashSet<u64> = if delta.only_non_commercial_splits {
                cache
                    .affected_posts_for_edges(&delta.split_edge_keys)
                    .into_iter()
                    .collect()
            } else {
                cache
                    .collect_affected_trading_posts(&delta.dirty_nodes)
                    .into_iter()
                    .collect()
            };
            let old_map: HashMap<u64, RailwayLink> = inc_links
                .iter()
                .map(|l| (l.from_building_id, l.clone()))
                .collect();
            let mut next_inc = Vec::new();
            for post in stations
                .iter()
                .filter(|s| s.kind == BuildingKind::TradingPost)
            {
                if affected.contains(&post.id) || !old_map.contains_key(&post.id) {
                    if let Some(link) = cache.plan_single_trading_post_route(*post, &state, None) {
                        next_inc.push(link);
                    }
                } else if let Some(mut kept) = old_map.get(&post.id).cloned() {
                    kept.owner_id = post.owner_id;
                    kept.level = post.level;
                    kept.period_ticks =
                        trading_post_period_ticks(kept.distance, post.level, &state.config);
                    kept.opened_at = state.tick;
                    next_inc.push(kept);
                }
            }
            next_inc.sort_by_key(|l| l.from_building_id);
            inc_links = next_inc;
            cache.rebuild_link_index(&inc_links, &[]);
            // Compare normalized link definitions (ignoring opened_at timestamps on unchanged routes).
            let norm = |links: &[RailwayLink]| -> Vec<(u64, u64, u16, u8, u32, Vec<u64>, Vec<u32>, u16, u32)> {
                links
                    .iter()
                    .map(|l| {
                        (
                            l.from_building_id,
                            l.to_building_id,
                            l.owner_id,
                            l.level,
                            l.distance,
                            l.station_path.clone(),
                            l.route_path.clone(),
                            l.station_stop_count,
                            l.period_ticks,
                        )
                    })
                    .collect()
            };
            assert_eq!(
                norm(&inc_links),
                norm(&full_links),
                "routes diverged at step {step}"
            );
        }
    }

    #[test]
    fn upgrade_and_capture_preserve_moving_cart_while_nuke_interrupts_it() {
        use crate::engine::SowEngine;
        use crate::game::GamePhase;
        use crate::player::Player;
        use crate::water_components::WaterComponents;

        let w = 64u32;
        let h = 64u32;
        let cfg = GameConfig::default();
        let mut state = GameState::new(1, w, h, cfg.clone());
        state.phase = GamePhase::Playing;
        for y in 0..h {
            for x in 0..w {
                state.map.set_owner_id(x, y, 1);
                let ri = state.map.ref_id(x, y);
                state.map.terrain[ri] = MapTile::from_byte(0x80);
            }
        }
        state.register_player(Player::new_human(1, "P1".into(), [1.0, 0.0, 0.0], &cfg));
        state.register_player(Player::new_human(2, "P2".into(), [0.0, 0.0, 1.0], &cfg));
        state.place_spawn(1, 10, 32);
        state.place_spawn(2, 50, 32);

        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = SowEngine::new(state, water);
        engine.add_building(Building {
            id: 1,
            owner_id: 1,
            tile_idx: 32 * w + 8,
            kind: BuildingKind::TradingPost,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.add_building(Building {
            id: 2,
            owner_id: 1,
            tile_idx: 32 * w + 32,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.add_building(Building {
            id: 3,
            owner_id: 1,
            tile_idx: 32 * w + 52,
            kind: BuildingKind::Port,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.building_aggregates_dirty = true;
        engine.refresh_building_aggregates();

        // Advance mid-trip, then upgrade the TradingPost and capture City 2: cart stays active.
        engine.state.tick = 40;
        let old_period = engine.railway_links[0].period_ticks;
        let old_opened = engine.railway_links[0].opened_at;
        engine.buildings[0].level = 4;
        engine.buildings[1].owner_id = 2;
        engine.building_aggregates_dirty = true;
        engine.refresh_building_aggregates();
        let upgraded = &engine.railway_links[0];
        assert!(upgraded.period_ticks < old_period);
        assert_eq!(upgraded.to_building_id, 3);
        let old_phase = ((40 - old_opened) % u64::from(old_period)) as f64 / f64::from(old_period);
        let new_phase = ((40 - upgraded.opened_at) % u64::from(upgraded.period_ticks)) as f64
            / f64::from(upgraded.period_ticks);
        assert!(
            (old_phase - new_phase).abs() < 0.05,
            "visual phase jumped on upgrade: {old_phase} vs {new_phase}"
        );

        // Nuke hitting the moving cart directly interrupts/resets the trip at the tick of impact.
        let cart_tile = cart_tile_at_tick(&engine.railway_links[0], 60, w).expect("cart tile");
        engine.state.tick = 60;
        engine.detonate_nuke(cart_tile % w, cart_tile / w, 4, 8, 2);
        engine.refresh_building_aggregates();
        if let Some(link) = engine.railway_links.first() {
            assert_eq!(link.opened_at, 60);
        }

        // Nuke destroying the destination Port interrupts the trip immediately too.
        engine.state.tick = 85;
        engine.detonate_nuke(52, 32, 4, 8, 2);
        engine.refresh_building_aggregates();
        let link = &engine.railway_links[0];
        assert_ne!(link.to_building_id, 3);
        assert_eq!(link.opened_at, 85);
    }

    #[test]
    #[ignore = "release microbenchmark; run with cargo test --release -p sow-core --lib bench_railway_incremental_ops -- --ignored --nocapture"]
    fn bench_railway_incremental_ops() {
        use crate::engine::SowEngine;
        use crate::game::GamePhase;
        use crate::player::Player;
        use crate::water_components::WaterComponents;
        use std::time::Instant;

        let (w, h) = (600u32, 600u32);
        let cfg = GameConfig::default();
        let mut state = GameState::new(9, w, h, cfg.clone());
        state.phase = GamePhase::Playing;
        for y in 0..h {
            for x in 0..w {
                state.map.set_owner_id(x, y, 1);
                let ri = state.map.ref_id(x, y);
                state.map.terrain[ri] = MapTile::from_byte(0x80);
            }
        }
        state.register_player(Player::new_human(1, "P1".into(), [1.0, 0.0, 0.0], &cfg));
        state.register_player(Player::new_human(2, "P2".into(), [0.0, 0.0, 1.0], &cfg));
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = SowEngine::new(state, water);

        let mut id = 1u64;
        for gy in 0..32u32 {
            for gx in 0..32u32 {
                let kind = if (gx + gy) % 3 == 0 {
                    BuildingKind::City
                } else {
                    BuildingKind::TradingPost
                };
                engine.add_building(Building {
                    id,
                    owner_id: 1,
                    tile_idx: (20 + gy * 17) * w + (20 + gx * 17),
                    kind,
                    level: 1,
                    under_construction: false,
                    ticks_until_complete: 0,
                });
                id += 1;
            }
        }
        engine.building_aggregates_dirty = true;
        engine.refresh_building_aggregates();
        assert!(engine.railway_links.len() > 600);

        let city_idx = engine
            .buildings
            .iter()
            .position(|b| b.kind == BuildingKind::City)
            .unwrap();
        let t0 = Instant::now();
        for i in 0..100u32 {
            engine.state.tick += 1;
            engine.buildings[city_idx].owner_id = if i % 2 == 0 { 2 } else { 1 };
            engine.building_aggregates_dirty = true;
            engine.refresh_building_aggregates();
        }
        let city_cap_ns = t0.elapsed().as_nanos() / 100;

        let tp_idx = engine
            .buildings
            .iter()
            .position(|b| b.kind == BuildingKind::TradingPost)
            .unwrap();
        let t1 = Instant::now();
        for i in 0..100u32 {
            engine.state.tick += 1;
            engine.buildings[tp_idx].level = ((i % 7) + 1) as u8;
            engine.building_aggregates_dirty = true;
            engine.refresh_building_aggregates();
        }
        let tp_up_ns = t1.elapsed().as_nanos() / 100;

        let t2 = Instant::now();
        for i in 0..100u32 {
            engine.state.tick += 1;
            engine.buildings[tp_idx].owner_id = if i % 2 == 0 { 2 } else { 1 };
            engine.building_aggregates_dirty = true;
            engine.refresh_building_aggregates();
        }
        let tp_cap_ns = t2.elapsed().as_nanos() / 100;

        let t3 = Instant::now();
        for i in 0..20u32 {
            engine.state.tick += 1;
            engine.add_building(Building {
                id: id + u64::from(i),
                owner_id: 1,
                tile_idx: (25 + i * 17) * w + 25,
                kind: BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            });
            engine.building_aggregates_dirty = true;
            engine.refresh_building_aggregates();
        }
        let add_station_us = t3.elapsed().as_micros() / 20;

        eprintln!(
            "stations={} links={} tracks={} | city_capture={}us tp_upgrade={}us tp_capture={}us add_station={}us",
            engine.buildings.len(),
            engine.railway_links.len(),
            engine.railway_tracks.len(),
            city_cap_ns / 1_000,
            tp_up_ns / 1_000,
            tp_cap_ns / 1_000,
            add_station_us,
        );
    }

    #[test]
    #[ignore = "release match benchmark; run with cargo test --release -p sow-core --lib bench_late_game_match_ticks -- --ignored --nocapture"]
    fn bench_late_game_match_ticks() {
        use crate::engine::SowEngine;
        use crate::game::GamePhase;
        use crate::game_config::BotDifficulty;
        use crate::player::Player;
        use crate::water_components::WaterComponents;
        use std::time::Instant;

        for (label, diff) in [
            ("Terminator + Rail", BotDifficulty::Terminator),
            ("Vanilla    + Rail", BotDifficulty::Vanilla),
        ] {
            let (w, h) = (600u32, 600u32);
            let mut cfg = GameConfig::default();
            cfg.random_spawn = true;
            cfg.bot_difficulty = diff;
            let mut state = GameState::new(0xDEAD_BEEF, w, h, cfg.clone());
            state.phase = GamePhase::Playing;
            for y in 24..h {
                for x in 0..w {
                    state.map.terrain[(y * w + x) as usize] = MapTile::from_byte(128 | 10);
                }
            }
            state.total_land_tiles = w * (h - 24);
            let water = WaterComponents::compute(&state.map, |_| {});

            for i in 0..80u32 {
                let pid = (i + 1) as u16;
                let gx = (i % 10) * 56 + 28;
                let gy = (i / 10) * 66 + 50;
                let mut p = if i < 20 {
                    let mut h = Player::new_human(pid, format!("G{pid}"), [0.5, 0.5, 0.5], &cfg);
                    h.is_ai_controlled = true;
                    h.iq = 175;
                    h
                } else if i < 40 {
                    let mut n = Player::new_nation(pid, format!("N{pid}"), [0.5, 0.5, 0.5], &cfg);
                    n.iq = 155;
                    n
                } else {
                    Player::new_bot(pid, format!("T{pid}"), [0.5, 0.5, 0.5], &cfg)
                };
                p.gold = 3_000.0;
                p.troops = 10_000.0;
                state.register_player(p);
                state.place_spawn(pid, gx, gy);
            }

            let mut engine = SowEngine::new(state, water);
            let origin = Instant::now();
            let mut sum_tot = 0u64;
            let mut sum_inc = 0u64;
            let mut sum_ai = 0u64;
            let mut sum_con = 0u64;
            let mut sum_cmb = 0u64;
            let mut sum_flt = 0u64;
            let mut sum_snp = 0u64;
            let mut max_tick = 0u64;
            let mut flt_over_500 = 0u64;

            for _ in 1..=1500u64 {
                let prof = engine.tick_profiled(|| origin.elapsed().as_micros() as u64);
                let s0 = Instant::now();
                let _snap = engine.build_snapshot();
                let snp = s0.elapsed().as_micros() as u64;
                if prof.fleets_us > 500 {
                    flt_over_500 += 1;
                }
                sum_tot += prof.total_us;
                sum_inc += prof.income_us;
                sum_ai += prof.ai_think_us;
                sum_con += prof.construction_us;
                sum_cmb += prof.combat_us;
                sum_flt += prof.fleets_us;
                sum_snp += snp;
                max_tick = max_tick.max(prof.total_us);
            }
            let tps = engine
                .buildings
                .iter()
                .filter(|b| b.kind == BuildingKind::TradingPost)
                .count();
            eprintln!(
                "{label}: bld={} tp={} tracks={} links={} | avg_tot={}us (inc={} ai={} con={} cmb={} flt={} snp={}) | max_tick={}us | flt>500us={}",
                engine.buildings.len(),
                tps,
                engine.railway_tracks.len(),
                engine.railway_links.len(),
                sum_tot / 1500,
                sum_inc / 1500,
                sum_ai / 1500,
                sum_con / 1500,
                sum_cmb / 1500,
                sum_flt / 1500,
                sum_snp / 1500,
                max_tick,
                flt_over_500,
            );
        }
    }

    #[test]
    fn building_bridging_two_tracks_connects_both_lines_and_preview_highlights() {
        let (w, h) = (64u32, 64u32);
        let state = land_game(w, h);
        // Line A on y=16 (id 1 TP at x=8 -> id 2 Farm at x=24).
        // Line B on y=19 (id 3 Tower at x=8 -> id 4 City at x=24).
        let base_stations = vec![
            Building {
                id: 1,
                owner_id: 1,
                tile_idx: 16 * w + 8,
                kind: BuildingKind::TradingPost,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
            Building {
                id: 2,
                owner_id: 1,
                tile_idx: 16 * w + 24,
                kind: BuildingKind::Farm,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
            Building {
                id: 3,
                owner_id: 2,
                tile_idx: 19 * w + 8,
                kind: BuildingKind::Tower,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
            Building {
                id: 4,
                owner_id: 2,
                tile_idx: 19 * w + 24,
                kind: BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
        ];
        // Note: stations 1..2 are separated from stations 3..4 only vertically by 3 tiles (y=16 vs y=19),
        // wait: station 3 at (8,19) is within RAIL_JOIN_RADIUS=3 of station 1 at (8,16)! So shift Line B endpoints away horizontally.
        let mut base = base_stations;
        base[2].tile_idx = 19 * w + 10;
        base[3].tile_idx = 19 * w + 32;
        // Connect Line A first, then Line B separately (station 3 -> station 4 along y=19).
        let mut index = HashMap::new();
        let mut ready = false;
        let line_a_tracks =
            populate_railway_network(&base[..2], &[], &state, &[], &mut index, &mut ready);
        let mut line_b_index = HashMap::new();
        let mut line_b_ready = false;
        let line_b_tracks =
            populate_railway_network(&base[2..], &[], &state, &[], &mut line_b_index, &mut line_b_ready);
        let mut combined_tracks = line_a_tracks;
        combined_tracks.extend(line_b_tracks);
        let combined_index = index_tracks(&combined_tracks, w);
        let station_tiles: Vec<(u64, u32)> = base.iter().map(|s| (s.id, s.tile_idx)).collect();

        // Placing a bridge building at (16, 17) touches Line A (dy=-1, inside 2x2 footprint) and Line B (dy=+2, within RAIL_JOIN_RADIUS=3).
        let mut astar = AStarScratch::default();
        let preview = preview_building_rail_connection(
            &combined_index,
            &station_tiles,
            &state.map,
            BuildingKind::Tower,
            17 * w + 16,
            &mut astar,
        );
        assert_eq!(preview.snap_tracks.len(), 2, "preview should highlight both tracks");
        assert_eq!(preview.ghost_paths.len(), 2, "preview should draw spurs to both tracks");

        // Now place that Tower in incremental cache starting from Line A + Line B.
        let mut cache = RailwayRuntimeCache::default();
        cache.hydrate(&base, &combined_tracks, &[], w, h);
        let mut all_stations = base;
        all_stations.push(Building {
            id: 5,
            owner_id: 1,
            tile_idx: 17 * w + 16,
            kind: BuildingKind::Tower,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        let delta = cache.apply_topology_delta(&all_stations, &state, &[], &[], false);
        assert!(delta.tracks_mutated);
        let link = cache
            .plan_single_trading_post_route(all_stations[0], &state, None)
            .expect("TP on Line A should now route across tower 5 to City on Line B");
        assert_eq!(link.to_building_id, 4);
        assert!(link.station_path.contains(&5));
    }

    #[test]
    fn non_commercial_split_preserves_route_parity_without_dijkstra() {
        let (w, h) = (64u32, 64u32);
        let state = land_game(w, h);
        let mut stations = vec![
            Building {
                id: 1,
                owner_id: 1,
                tile_idx: 20 * w + 8,
                kind: BuildingKind::TradingPost,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
            Building {
                id: 2,
                owner_id: 1,
                tile_idx: 20 * w + 28,
                kind: BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            },
        ];
        let mut cache = RailwayRuntimeCache::default();
        let delta0 = cache.apply_topology_delta(&stations, &state, &[], &[], true);
        assert!(delta0.tracks_mutated);
        let link0 = cache
            .plan_single_trading_post_route(stations[0], &state, None)
            .unwrap();
        let inc_links = vec![link0];
        cache.rebuild_link_index(&inc_links, &[]);

        // Place a Tower directly on the track at (18, 20): triggers the non-commercial single-split fast path.
        stations.push(Building {
            id: 3,
            owner_id: 1,
            tile_idx: 20 * w + 18,
            kind: BuildingKind::Tower,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        let pending = vec![PendingTradeRoute::default()];
        let delta1 = cache.apply_topology_delta(&stations, &state, &inc_links, &pending, false);
        assert!(delta1.only_non_commercial_splits);
        let affected = cache.affected_posts_for_edges(&delta1.split_edge_keys);
        assert_eq!(affected, vec![1]);
        let link1 = cache
            .plan_single_trading_post_route(stations[0], &state, Some(&inc_links[0]))
            .unwrap();
        assert_eq!(link1.station_path, vec![1, 3, 2]);
        assert_eq!(link1.distance, 20);
        let full = populate_trading_post_routes(&stations, &cache.tracks_vec(), &state, &[]);
        assert_eq!(link1.station_path, full[0].station_path);
        assert_eq!(link1.station_stop_count, full[0].station_stop_count);
    }
}
