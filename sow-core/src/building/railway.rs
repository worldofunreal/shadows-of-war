use super::core::{
    Building, RAILWAY_LINK_DISTANCE, RAILWAY_MAX_CANDIDATES_PER_STATION,
    RAILWAY_MAX_ROUTE_DISTANCE, RailwayLink, RailwayTrack, TradeRouteProgress,
    trading_post_speed_multiplier,
};
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
    matches!(
        building.kind,
        BuildingKind::City | BuildingKind::Port | BuildingKind::TradingPost
    ) && building.active_level() > 0
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
        .iter()
        .filter_map(|(&key, _)| {
            let endpoint_moved = (changed_ids.contains(&key.0) && key.0 & JUNCTION_ID_BIT == 0)
                || (changed_ids.contains(&key.1) && key.1 & JUNCTION_ID_BIT == 0);
            endpoint_moved.then_some(key)
        })
        .collect();
    for key in stale_edges {
        remove_track(key, &mut tracks, track_index, state.map.width);
    }

    let mut station_grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (index, station) in stations.iter().enumerate() {
        let (x, y) = tile_xy(station.tile_idx, state.map.width);
        station_grid
            .entry((x / STATION_GRID_CELL, y / STATION_GRID_CELL))
            .or_default()
            .push(index);
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
    for station in changed {
        let mut remaining_search = PATH_SEARCH_LIMIT;
        let joined_track = connect_station_to_existing_track(
            station,
            &mut tracks,
            track_index,
            state,
            &mut remaining_search,
        );

        if !joined_track {
            nearby_station_candidates(
                station,
                stations,
                &station_grid,
                state.map.width,
                &mut candidates,
            );
            goals.clear();
            goals.extend(
                candidates
                    .iter()
                    .filter(|(_, id, _)| processed.contains(id))
                    .map(|(_, id, tile)| (*id, *tile)),
            );
            match find_land_path_to_any(&state.map, station.tile_idx, &goals, remaining_search) {
                Ok((target_id, path, _explored)) => {
                    if path.len() >= 2 {
                        if let Some(intersection) =
                            first_track_intersection(&path, station.tile_idx, track_index)
                        {
                            let junction = split_tracks_at_tile(
                                intersection,
                                &mut tracks,
                                track_index,
                                state.map.width,
                            );
                            if let Some(end) = path.iter().position(|tile| *tile == intersection)
                                && end > 0
                            {
                                insert_track(
                                    station.id,
                                    junction,
                                    &path[..=end],
                                    &mut tracks,
                                    track_index,
                                    state.map.width,
                                );
                            }
                        } else {
                            insert_track(
                                station.id,
                                target_id,
                                &path,
                                &mut tracks,
                                track_index,
                                state.map.width,
                            );
                        }
                    }
                }
                Err(_explored) => {}
            }
        }
        processed.insert(station.id);
    }

    tracks.into_values().collect()
}

pub fn populate_trading_post_routes(
    stations: &[Building],
    tracks: &[RailwayTrack],
    state: &GameState,
    previous: &[RailwayLink],
) -> Vec<RailwayLink> {
    let station_by_id: HashMap<_, _> = stations
        .iter()
        .map(|station| (station.id, *station))
        .collect();
    let mut graph: HashMap<u64, Vec<(u64, u32)>> = HashMap::new();
    let mut edge_distances = HashMap::new();
    for track in tracks {
        let distance = polyline_distance(&track.path, state.map.width);
        edge_distances.insert(track_key(track.from_node_id, track.to_node_id), distance);
        graph
            .entry(track.from_node_id)
            .or_default()
            .push((track.to_node_id, distance));
        graph
            .entry(track.to_node_id)
            .or_default()
            .push((track.from_node_id, distance));
    }
    let tracks_by_key: HashMap<_, _> = tracks
        .iter()
        .map(|track| (track_key(track.from_node_id, track.to_node_id), track))
        .collect();
    for neighbors in graph.values_mut() {
        neighbors.sort_unstable_by_key(|&(id, distance)| (id, distance));
    }
    let mut node_ids: Vec<_> = graph.keys().copied().collect();
    node_ids.extend(graph.values().flatten().map(|&(id, _)| id));
    node_ids.extend(stations.iter().map(|station| station.id));
    node_ids.sort_unstable();
    node_ids.dedup();
    let node_index: HashMap<_, _> = node_ids
        .iter()
        .enumerate()
        .map(|(index, &id)| (id, index))
        .collect();
    let mut adjacency = vec![Vec::new(); node_ids.len()];
    for (&id, neighbors) in &graph {
        let Some(&index) = node_index.get(&id) else {
            continue;
        };
        adjacency[index].extend(neighbors.iter().filter_map(|&(neighbor, distance)| {
            node_index
                .get(&neighbor)
                .map(|&neighbor_index| (neighbor_index, distance))
        }));
        adjacency[index].sort_unstable_by_key(|&(neighbor, distance)| (neighbor, distance));
    }
    let mut station_by_node = vec![None; node_ids.len()];
    for station in stations {
        if let Some(&index) = node_index.get(&station.id) {
            station_by_node[index] = Some(*station);
        }
    }
    let mut dijkstra = DijkstraScratch::new(node_ids.len());

    let previous_by_post: HashMap<_, _> = previous
        .iter()
        .map(|route| (route.from_building_id, route))
        .collect();
    let mut routes = Vec::new();
    for post in stations
        .iter()
        .filter(|building| building.kind == BuildingKind::TradingPost)
    {
        let Some(&source) = node_index.get(&post.id) else {
            continue;
        };
        let Some((distance, destination, station_path)) = farthest_reachable(
            source,
            &node_ids,
            &adjacency,
            &station_by_node,
            true,
            &mut dijkstra,
        )
        .or_else(|| {
            farthest_reachable(
                source,
                &node_ids,
                &adjacency,
                &station_by_node,
                false,
                &mut dijkstra,
            )
        }) else {
            continue;
        };
        routes.push(make_link(
            *post,
            destination,
            station_path,
            &tracks_by_key,
            &station_by_id,
            distance,
            &edge_distances,
            state,
            previous_by_post.get(&post.id).copied(),
        ));
    }
    routes.sort_by_key(|route| route.from_building_id);
    routes
}

fn make_link(
    post: Building,
    destination: Building,
    station_path: Vec<u64>,
    tracks: &HashMap<(u64, u64), &RailwayTrack>,
    stations: &HashMap<u64, Building>,
    distance: u32,
    edge_distances: &HashMap<(u64, u64), u32>,
    state: &GameState,
    previous: Option<&RailwayLink>,
) -> RailwayLink {
    let speed = trading_post_speed_multiplier(post.level);
    let normal_seconds = (14.0 + f64::from(distance) * 0.70) / speed;
    let per_tick = state.config.per_tick(1.0).max(f64::EPSILON);
    let period_ticks = (normal_seconds * crate::game_config::NORMAL_GAME_SPEED / per_tick)
        .ceil()
        .clamp(1.0, f64::from(u32::MAX)) as u32;
    let station_stop_count = station_path
        .iter()
        .filter_map(|id| stations.get(id))
        .filter(|station| matches!(station.kind, BuildingKind::City | BuildingKind::Port))
        .count()
        .min(usize::from(u16::MAX)) as u16;
    let mut route_path = Vec::new();
    for pair in station_path.windows(2) {
        let Some(track) = tracks.get(&track_key(pair[0], pair[1])) else {
            continue;
        };
        let oriented = if track.from_node_id == pair[0] {
            track.path.iter().copied().collect::<Vec<_>>()
        } else {
            track.path.iter().rev().copied().collect::<Vec<_>>()
        };
        for tile in oriented {
            if route_path.last().copied() != Some(tile) {
                route_path.push(tile);
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
                    matches!(
                        stations[position].kind,
                        BuildingKind::City | BuildingKind::Port
                    )
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

struct DijkstraScratch {
    visit_generation: u32,
    visited: Vec<u32>,
    distances: Vec<u32>,
    previous: Vec<usize>,
    buckets: Vec<Vec<usize>>,
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

fn farthest_reachable(
    source: usize,
    node_ids: &[u64],
    graph: &[Vec<(usize, u32)>],
    stations: &[Option<Building>],
    commercial_only: bool,
    scratch: &mut DijkstraScratch,
) -> Option<(u32, Building, Vec<u64>)> {
    let generation = scratch.begin();
    scratch.visited[source] = generation;
    scratch.distances[source] = 0;
    scratch.previous[source] = source;
    scratch.buckets[0].push(source);
    let mut best: Option<(u32, u64, usize)> = None;
    for distance in 0..=RAILWAY_MAX_ROUTE_DISTANCE {
        let mut cursor = 0;
        while cursor < scratch.buckets[distance as usize].len() {
            let node = scratch.buckets[distance as usize][cursor];
            cursor += 1;
            if scratch.visited[node] != generation || distance != scratch.distances[node] {
                continue;
            }
            if node != source {
                if let Some(station) = stations[node] {
                    let is_commercial =
                        matches!(station.kind, BuildingKind::City | BuildingKind::Port);
                    let better = best.is_none_or(|(best_distance, best_id, _)| {
                        distance > best_distance
                            || (distance == best_distance && station.id < best_id)
                    });
                    if (!commercial_only || is_commercial) && better {
                        best = Some((distance, station.id, node));
                    }
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
    let (distance, _, destination) = best?;
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

fn nearby_station_candidates(
    station: Building,
    stations: &[Building],
    grid: &HashMap<(i32, i32), Vec<usize>>,
    map_width: u32,
    candidates: &mut Vec<(i64, u64, u32)>,
) {
    candidates.clear();
    let (x, y) = tile_xy(station.tile_idx, map_width);
    let (cell_x, cell_y) = (x / STATION_GRID_CELL, y / STATION_GRID_CELL);
    let radius_squared = i64::from(RAILWAY_LINK_DISTANCE).pow(2);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(indices) = grid.get(&(cell_x + dx, cell_y + dy)) else {
                continue;
            };
            for &index in indices {
                let candidate = stations[index];
                if candidate.id == station.id {
                    continue;
                }
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

fn connect_station_to_existing_track(
    station: Building,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    state: &GameState,
    remaining_search: &mut usize,
) -> bool {
    let (x, y) = tile_xy(station.tile_idx, state.map.width);
    if track_index.contains_key(&station.tile_idx) {
        let junction = split_tracks_at_station_tile(
            station.tile_idx,
            station.id,
            tracks,
            track_index,
            state.map.width,
        );
        if junction != station.id {
            insert_track(
                station.id,
                junction,
                &[station.tile_idx],
                tracks,
                track_index,
                state.map.width,
            );
        }
        return true;
    }
    let mut nearby = Vec::new();
    for dy in -RAIL_JOIN_RADIUS..=RAIL_JOIN_RADIUS {
        for dx in -RAIL_JOIN_RADIUS..=RAIL_JOIN_RADIUS {
            let distance_squared = dx * dx + dy * dy;
            if distance_squared > RAIL_JOIN_RADIUS * RAIL_JOIN_RADIUS {
                continue;
            }
            let tx = x + dx;
            let ty = y + dy;
            if tx < 0 || ty < 0 || tx >= state.map.width as i32 || ty >= state.map.height as i32 {
                continue;
            }
            let tile = ty as u32 * state.map.width + tx as u32;
            if track_index.contains_key(&tile) {
                nearby.push((distance_squared, tile));
            }
        }
    }
    nearby.sort_unstable();
    for (_, target_tile) in nearby {
        if *remaining_search == 0 {
            return false;
        }
        let (path, explored) =
            match find_land_path(&state.map, station.tile_idx, target_tile, *remaining_search) {
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
            first_track_intersection(&path, station.tile_idx, track_index).unwrap_or(target_tile);
        let Some(end) = path.iter().position(|tile| *tile == intersection) else {
            continue;
        };
        let junction = split_tracks_at_tile(intersection, tracks, track_index, state.map.width);
        insert_track(
            station.id,
            junction,
            &path[..=end],
            tracks,
            track_index,
            state.map.width,
        );
        return true;
    }
    false
}

fn find_land_path_to_any(
    map: &GameMap,
    start: u32,
    goals: &[(u64, u32)],
    budget: usize,
) -> Result<(u64, Vec<u32>, usize), usize> {
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

    let goal_by_tile: HashMap<u32, u64> =
        goals
            .iter()
            .fold(HashMap::new(), |mut by_tile, &(id, tile)| {
                by_tile
                    .entry(tile)
                    .and_modify(|current| *current = (*current).min(id))
                    .or_insert(id);
                by_tile
            });
    let goal_tiles: Vec<_> = goal_by_tile.keys().copied().collect();
    let (path, explored) = find_path_a_star(map, start, &goal_tiles, budget)?;
    let goal = *path.last().ok_or(explored)?;
    let id = *goal_by_tile.get(&goal).ok_or(explored)?;
    Ok((id, path, explored))
}

fn find_land_path(
    map: &GameMap,
    start: u32,
    goal: u32,
    budget: usize,
) -> Result<(Vec<u32>, usize), usize> {
    if start == goal {
        return Ok((vec![start], 0));
    }
    let (start_x, start_y) = tile_xy(start, map.width);
    let (goal_x, goal_y) = tile_xy(goal, map.width);
    if let Some(path) = l_shaped_land_path(map, start_x, start_y, goal_x, goal_y) {
        return Ok((path, 0));
    }
    find_path_a_star(map, start, &[goal], budget)
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

fn find_path_a_star(
    map: &GameMap,
    start: u32,
    goals: &[u32],
    budget: usize,
) -> Result<(Vec<u32>, usize), usize> {
    if goals.is_empty() || budget == 0 {
        return Err(0);
    }
    let goal_set: HashSet<_> = goals.iter().copied().collect();
    let mut open = BinaryHeap::from([Reverse((heuristic(start, goals, map.width), 0u32, start))]);
    let mut cost = HashMap::from([(start, 0u32)]);
    let mut previous = HashMap::<u32, u32>::new();
    let mut explored = 0;
    while let Some(Reverse((_, distance, tile))) = open.pop() {
        if explored >= budget {
            break;
        }
        if distance != cost[&tile] {
            continue;
        }
        explored += 1;
        if goal_set.contains(&tile) {
            let mut path = vec![tile];
            while path.last().copied() != Some(start) {
                let Some(previous_tile) = path.last().and_then(|tile| previous.get(tile)).copied()
                else {
                    return Err(explored);
                };
                path.push(previous_tile);
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
            if !map
                .terrain
                .get(next as usize)
                .is_some_and(crate::map::MapTile::is_land)
            {
                continue;
            }
            let next_cost = distance + 1;
            if cost.get(&next).is_some_and(|&old| old <= next_cost) {
                continue;
            }
            cost.insert(next, next_cost);
            previous.insert(next, tile);
            open.push(Reverse((
                next_cost + heuristic(next, goals, map.width),
                next_cost,
                next,
            )));
        }
    }
    Err(explored)
}

fn heuristic(tile: u32, goals: &[u32], width: u32) -> u32 {
    let (x, y) = tile_xy(tile, width);
    goals
        .iter()
        .map(|&goal| {
            let (gx, gy) = tile_xy(goal, width);
            (x - gx).unsigned_abs() + (y - gy).unsigned_abs()
        })
        .min()
        .unwrap_or(0)
}

fn first_track_intersection(
    path: &[u32],
    start: u32,
    track_index: &HashMap<u32, Vec<(u64, u64)>>,
) -> Option<u32> {
    path.iter()
        .copied()
        .skip_while(|tile| *tile == start)
        .find(|tile| track_index.contains_key(tile))
}

fn split_tracks_at_tile(
    tile: u32,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    map_width: u32,
) -> u64 {
    split_tracks_at_tile_with_junction(tile, None, tracks, track_index, map_width)
}

fn split_tracks_at_station_tile(
    tile: u32,
    station_id: u64,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    map_width: u32,
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
        replace_track_node(junction, station_id, tracks, track_index, map_width);
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
    split_tracks_at_tile_with_junction(
        tile,
        is_interior.then_some(station_id),
        tracks,
        track_index,
        map_width,
    )
}

fn replace_track_node(
    old_node: u64,
    new_node: u64,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    map_width: u32,
) {
    let edges: Vec<_> = tracks
        .values()
        .filter(|track| track.from_node_id == old_node || track.to_node_id == old_node)
        .cloned()
        .collect();
    for track in edges {
        let path = expand_path(&track.path, map_width);
        remove_track(
            track_key(track.from_node_id, track.to_node_id),
            tracks,
            track_index,
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
        insert_track(from, to, &path, tracks, track_index, map_width);
    }
}

fn split_tracks_at_tile_with_junction(
    tile: u32,
    preferred_junction: Option<u64>,
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    map_width: u32,
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
        remove_track(key, tracks, track_index, map_width);
        insert_track(
            track.from_node_id,
            junction,
            &path[..=index],
            tracks,
            track_index,
            map_width,
        );
        insert_track(
            junction,
            track.to_node_id,
            &path[index..],
            tracks,
            track_index,
            map_width,
        );
    }
    junction
}

fn insert_track(
    from: u64,
    to: u64,
    path: &[u32],
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    map_width: u32,
) {
    if from == to || path.is_empty() {
        return;
    }
    let key = track_key(from, to);
    if tracks.contains_key(&key) {
        return;
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
    if polyline_distance(&compressed, map_width) > RAILWAY_MAX_ROUTE_DISTANCE {
        return;
    }
    tracks.insert(
        key,
        RailwayTrack {
            from_node_id: key.0,
            to_node_id: key.1,
            path: compressed,
        },
    );
    for tile in oriented_path {
        let keys = track_index.entry(tile).or_default();
        if !keys.contains(&key) {
            keys.push(key);
            keys.sort_unstable();
        }
    }
}

fn remove_track(
    key: (u64, u64),
    tracks: &mut BTreeMap<(u64, u64), RailwayTrack>,
    track_index: &mut HashMap<u32, Vec<(u64, u64)>>,
    map_width: u32,
) {
    let Some(track) = tracks.remove(&key) else {
        return;
    };
    for tile in expand_path(&track.path, map_width) {
        if let Some(keys) = track_index.get_mut(&tile) {
            keys.retain(|candidate| *candidate != key);
            if keys.is_empty() {
                track_index.remove(&tile);
            }
        }
    }
}

fn index_tracks(tracks: &[RailwayTrack], map_width: u32) -> HashMap<u32, Vec<(u64, u64)>> {
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
