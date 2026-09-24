//! Transport ship execution as `WarpFleet` (water path + landing).
//!
//! The launch decision is gated by [`WaterComponents`]: a boat is only viable when
//! the sender owns a shoreline tile on the **same connected water body** as the
//! target's shoreline. Uses connected-component queries to handle lakes, rivers,
//! and disjoint oceans instead of relying on an arbitrary Manhattan radius.

use std::fmt;

use crate::map::{GameMap, TerrainType};
use crate::pathfinding::WaterPathfinderScratch;
use crate::water_components::WaterComponents;

/// Resolved water-only route for a transport launch (spawn shore → landing shore).
#[derive(Debug, Clone, PartialEq)]
pub struct FleetRoute {
    pub src_tile: u32,
    pub landing_tile: u32,
    pub path: Vec<u32>,
}

/// Typed failure for fleet routing (client preflight + sim must share this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetLaunchError {
    /// `target_tile` out of map bounds.
    InvalidTile,
    /// `target_owner == player_id`.
    SelfTarget,
    /// Launcher has no shoreline on any water component.
    NoWaterAccess,
    /// Enemy/neutral target requires a player row that is missing.
    TargetPlayerNotFound { target_owner: u16 },
    /// No landing shoreline reachable on shared water components.
    NoLandingShore,
    /// No owned launch shore on the landing tile's water component.
    NoLaunchShore { component: u32 },
    /// Water A* found no path between `src_tile` and `landing_tile`.
    NoWaterPath,
    /// No **ready** Port — a port is required before transport launches.
    NoPort,
}

impl fmt::Display for FleetLaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FleetLaunchError::InvalidTile => write!(f, "invalid target tile"),
            FleetLaunchError::SelfTarget => write!(f, "cannot boat to own tile"),
            FleetLaunchError::NoWaterAccess => write!(f, "no shoreline water access"),
            FleetLaunchError::TargetPlayerNotFound { target_owner } => {
                write!(f, "target player {target_owner} not found")
            }
            FleetLaunchError::NoLandingShore => {
                write!(f, "no landing shore on shared water body")
            }
            FleetLaunchError::NoLaunchShore { component } => {
                write!(f, "no launch shore on water component {component}")
            }
            FleetLaunchError::NoWaterPath => write!(f, "no water path to landing"),
            FleetLaunchError::NoPort => write!(f, "fleet requires a completed port"),
        }
    }
}

/// Shared route resolution for `LaunchFleet` — used by simulation and client preflight
/// so right-click / KeyB cannot diverge from `apply_launch_fleet_intent`.
///
/// `target_border`: required when `target_owner != 0` (enemy player's `border_tiles`);
/// ignored for neutral (`target_owner == 0`).
pub fn resolve_fleet_route(
    map: &GameMap,
    water_components: &WaterComponents,
    path_scratch: &mut WaterPathfinderScratch,
    player_id: u16,
    target: (u16, u32),
    border_tiles: &crate::bitset::DenseBitSet,
    target_border: Option<&crate::bitset::DenseBitSet>,
) -> Result<FleetRoute, FleetLaunchError> {
    let (target_owner, target_tile) = target;
    let map_area = map.width.saturating_mul(map.height);
    if map_area == 0 || target_tile >= map_area {
        return Err(FleetLaunchError::InvalidTile);
    }
    if target_owner == player_id {
        return Err(FleetLaunchError::SelfTarget);
    }

    let my_comps = player_water_components(map, water_components, player_id, border_tiles);
    if my_comps.is_empty() {
        return Err(FleetLaunchError::NoWaterAccess);
    }

    let landing = if target_owner != 0 {
        let tb = target_border.ok_or(FleetLaunchError::TargetPlayerNotFound { target_owner })?;
        closest_target_shore_for_player(
            map,
            water_components,
            &my_comps,
            target_owner,
            tb,
            target_tile,
        )
    } else {
        closest_neutral_shore_on_components(map, water_components, &my_comps, (target_tile, 200))
    };
    let landing = landing.ok_or(FleetLaunchError::NoLandingShore)?;

    let landing_comp = water_components.component_of(landing);
    let src = best_shore_spawn_for_transport(
        map,
        player_id,
        border_tiles,
        landing,
        Some((water_components, landing_comp)),
    )
    .ok_or(FleetLaunchError::NoLaunchShore {
        component: landing_comp,
    })?;

    let path = path_scratch
        .astar
        .find_path(map, &[src], landing)
        .ok_or(FleetLaunchError::NoWaterPath)?;

    Ok(FleetRoute {
        src_tile: src,
        landing_tile: landing,
        path,
    })
}

/// Diagnostic variant used by bot profiling builds; it calls the production resolver
/// unchanged and counts the shore entries that its selection loop visits.
#[cfg(feature = "ai-metrics")]
pub fn resolve_fleet_route_with_metrics(
    map: &GameMap,
    water_components: &WaterComponents,
    path_scratch: &mut WaterPathfinderScratch,
    player_id: u16,
    target: (u16, u32),
    border_tiles: &crate::bitset::DenseBitSet,
    target_border: Option<&crate::bitset::DenseBitSet>,
    shoreline_candidates_examined: &mut u64,
) -> Result<FleetRoute, FleetLaunchError> {
    let result = resolve_fleet_route(
        map,
        water_components,
        path_scratch,
        player_id,
        target,
        border_tiles,
        target_border,
    );
    if matches!(
        &result,
        Ok(_)
            | Err(FleetLaunchError::NoLandingShore)
            | Err(FleetLaunchError::NoLaunchShore { .. })
            | Err(FleetLaunchError::NoWaterPath)
    ) {
        let my_comps = player_water_components(map, water_components, player_id, border_tiles);
        if target.0 == 0 {
            *shoreline_candidates_examined += my_comps
                .iter()
                .filter_map(|&component| water_components.shoreline_tiles.get(component as usize))
                .map(|shores| shores.len() as u64)
                .sum::<u64>();
        } else if let Some(target_border) = target_border {
            *shoreline_candidates_examined += target_border.count_ones() as u64;
        }
    }
    result
}

/// Collect the set of water components this player can launch from (deduplicated,
/// sorted ascending for determinism). Empty iff the player owns no shore adjacent
/// to any water tile — the "cannot build transport" condition.
pub fn player_water_components(
    map: &GameMap,
    components: &WaterComponents,
    player_id: u16,
    border_tiles: &crate::bitset::DenseBitSet,
) -> Vec<u32> {
    let w = map.width;
    let mut out: Vec<u32> = Vec::new();
    for idx in border_tiles.ones() {
        let x = idx % w;
        let y = idx / w;
        if map.owner_id(x, y) != player_id {
            continue;
        }
        let t = map.terrain[idx as usize];
        if !t.is_land() || !t.is_shoreline() {
            continue;
        }
        let c = components.component_of(idx);
        if c == 0 {
            continue;
        }
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out.sort_unstable();
    out
}

#[inline]
fn manhattan_idx(w: u32, a: u32, b: u32) -> u32 {
    let ax = a % w;
    let ay = a / w;
    let bx = b % w;
    let by = b / w;
    ax.abs_diff(bx) + ay.abs_diff(by)
}

#[inline]
fn components_match(my_comps: &[u32], c: u32) -> bool {
    if c == 0 {
        return false;
    }
    my_comps.binary_search(&c).is_ok()
}

/// Resolve the landing tile for a player-owned target (component check;
/// no arbitrary Manhattan radius — connectivity is the gate).
///
/// Iterates the target player's cached `border_tiles` (O(perimeter)), keeps only
/// shoreline tiles on a water component the caller can reach, returns the one
/// closest (Manhattan) to `click_tile`. Deterministic tie-break: smallest index.
pub fn closest_target_shore_for_player(
    map: &GameMap,
    components: &WaterComponents,
    my_water_comps: &[u32],
    target_owner: u16,
    target_border_tiles: &crate::bitset::DenseBitSet,
    click_tile: u32,
) -> Option<u32> {
    if my_water_comps.is_empty() {
        return None;
    }
    let w = map.width;
    let mut best: Option<(u32, u32)> = None;
    for idx in target_border_tiles.ones() {
        let x = idx % w;
        let y = idx / w;
        if map.owner_id(x, y) != target_owner {
            continue;
        }
        let t = map.terrain[idx as usize];
        if !t.is_land() || !t.is_shoreline() {
            continue;
        }
        let c = components.component_of(idx);
        if !components_match(my_water_comps, c) {
            continue;
        }
        let d = manhattan_idx(w, idx, click_tile);
        match best {
            None => best = Some((d, idx)),
            Some((bd, bi)) => {
                if d < bd || (d == bd && idx < bi) {
                    best = Some((d, idx));
                }
            }
        }
    }
    best.map(|(_, i)| i)
}

/// Resolve the landing tile for a neutral-owned target from the static shoreline index.
/// The component and Manhattan-distance filters match the old bounded BFS, including
/// its smallest-index tie-break.
pub fn closest_neutral_shore_on_components(
    map: &GameMap,
    components: &WaterComponents,
    my_water_comps: &[u32],
    target_params: (u32, u32),
) -> Option<u32> {
    let (click_tile, max_dist) = target_params;
    if my_water_comps.is_empty() {
        return None;
    }
    let w = map.width;
    let h = map.height;
    if w == 0 || h == 0 {
        return None;
    }
    let area = w * h;
    if click_tile >= area {
        return None;
    }

    let cx = click_tile % w;
    let cy = click_tile / w;
    let mut best: Option<(u32, u32)> = None;

    for &component in my_water_comps {
        let Some(shores) = components.shoreline_tiles.get(component as usize) else {
            continue;
        };
        for &idx in shores {
            let x = idx % w;
            let y = idx / w;
            if map.owner_id(x, y) != 0 || cx.abs_diff(x) + cy.abs_diff(y) > max_dist {
                continue;
            }
            let d = cx.abs_diff(x) + cy.abs_diff(y);
            match best {
                None => best = Some((d, idx)),
                Some((bd, bi)) if d < bd || (d == bd && idx < bi) => {
                    best = Some((d, idx));
                }
                Some(_) => {}
            }
        }
    }

    best.map(|(_, i)| i)
}

/// Troop loss fraction when a fleet returns to own shore (`malusForRetreat = 25`).
pub const FLEET_RETREAT_SHORE_MALUS: f64 = 0.25;

/// Best owned shoreline tile, closest (Manhattan) to `reference_idx`, **and**
/// restricted to the same water component as `reference_idx` when
/// `components` is provided.
/// `SpatialQuery.closestShoreByWater`: a TransportShip can only launch from one
/// of my shores that actually touches the same water body as the landing.
///
/// Passing `components=None` falls back to the pure "touches water" check and
/// is only kept for the retreat path, where we want the nearest friendly shore
/// regardless of which component — a fleet can rebase on any of our coasts.
pub fn best_shore_spawn_for_transport(
    map: &GameMap,
    player_id: u16,
    border_tiles: &crate::bitset::DenseBitSet,
    reference_idx: u32,
    components: Option<(&WaterComponents, u32)>,
) -> Option<u32> {
    let w = map.width;
    let rx = reference_idx % w;
    let ry = reference_idx / w;

    let mut best: Option<(u32, u32)> = None;

    for idx in border_tiles.ones() {
        let x = idx % w;
        let y = idx / w;
        if map.owner_id(x, y) != player_id {
            continue;
        }
        let t = map.terrain[idx as usize];
        if !t.is_land() || !t.is_shoreline() {
            continue;
        }

        match components {
            Some((comps, target_comp)) => {
                if comps.component_of(idx) != target_comp {
                    continue;
                }
            }
            None => {
                let mut touches_water = false;
                map.for_each_neighbor(x, y, |nx, ny| {
                    let tt = map.terrain_type(nx, ny);
                    if tt == TerrainType::Water || tt == TerrainType::Lake {
                        touches_water = true;
                    }
                });
                if !touches_water {
                    continue;
                }
            }
        }

        let dist = rx.abs_diff(x) + ry.abs_diff(y);
        match best {
            None => best = Some((dist, idx)),
            Some((bd, bi)) => {
                if dist < bd || (dist == bd && idx < bi) {
                    best = Some((dist, idx));
                }
            }
        }
    }

    best.map(|(_, i)| i)
}

/// Moving fleet over water (1 tile / tick).
#[derive(Debug, Clone)]
pub struct WarpFleet {
    pub id: u64,
    pub owner_id: u16,
    pub target_owner: u16,
    pub unit_type: crate::game::UnitType,
    pub troops: f64,
    pub src_tile: u32,
    pub dst_tile: u32,
    pub retreat_dst: Option<u32>,
    pub path: std::sync::Arc<Vec<u32>>,
    pub path_cursor: usize,
    pub current_tile: u32,
    pub retreating: bool,
    pub flow_target: Option<u32>,
}

impl WarpFleet {
    pub fn new(
        id: u64,
        owner_id: u16,
        target_owner: u16,
        unit_type: crate::game::UnitType,
        troops: f64,
        endpoints: (u32, u32),
        path: Vec<u32>,
    ) -> Self {
        let (src_tile, dst_tile) = endpoints;
        let current_tile = path.first().copied().unwrap_or(src_tile);
        let path_cursor = 0;
        Self {
            id,
            owner_id,
            target_owner,
            unit_type,
            troops,
            src_tile,
            dst_tile,
            retreat_dst: None,
            path: std::sync::Arc::new(path),
            path_cursor,
            current_tile,
            retreating: false,
            flow_target: None,
        }
    }
}

#[cfg(all(test, feature = "ai-metrics"))]
mod metrics_tests {
    use super::{resolve_fleet_route, resolve_fleet_route_with_metrics};
    use crate::bitset::DenseBitSet;
    use crate::map::{GameMap, MapTile};
    use crate::pathfinding::WaterPathfinderScratch;
    use crate::water_components::WaterComponents;

    #[test]
    fn route_metric_counts_only_the_candidates_scanned() {
        let mut map = GameMap::new(3, 1);
        map.terrain[0] = MapTile::from_byte(0xC0);
        map.terrain[1] = MapTile::from_byte(0x20);
        map.terrain[2] = MapTile::from_byte(0xC0);
        map.set_owner_id(0, 0, 1);
        map.compute_shorelines();
        let components = WaterComponents::compute(&map, |_| {});
        let mut border = DenseBitSet::new();
        border.insert(0);
        let mut regular_scratch = WaterPathfinderScratch::default();
        let mut metric_scratch = WaterPathfinderScratch::default();
        let mut examined = 0;

        let regular = resolve_fleet_route(
            &map,
            &components,
            &mut regular_scratch,
            1,
            (0, 2),
            &border,
            None,
        );
        let measured = resolve_fleet_route_with_metrics(
            &map,
            &components,
            &mut metric_scratch,
            1,
            (0, 2),
            &border,
            None,
            &mut examined,
        );

        assert_eq!(measured, regular);
        assert_eq!(examined, 2);
    }
}
