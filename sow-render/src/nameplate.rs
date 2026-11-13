//! Pure nameplate placement and capacity.
const AVATAR_TEXT_GAP_SCALE: f32 = 0.16;
const BADGE_GAP: f32 = 3.0;
const BADGE_STACK_GAP: f32 = 2.0;
// OpenFront recalculates player clusters every 20 ticks after land changes.
const LAND_REFRESH_TICKS: u64 = 20;
pub const NAMEPLATE_SAMPLE_TICKS: u64 = 4;
const SPAWN_LOCAL_SIDE: usize = sow_core::config::SPAWN_RADIUS as usize * 2 + 1;
const SPAWN_LOCAL_AREA: usize = SPAWN_LOCAL_SIDE * SPAWN_LOCAL_SIDE;
const HUMAN_AVATAR_SCALE: f32 = 8.0;
const BOT_AVATAR_SCALE: f32 = 6.0;
const BADGE_AVATAR_RATIO: f32 = 0.43;
const TROOPS_SCALE: f32 = 1.30;
const NAMEPLATE_SIZE_DEADZONE: f32 = 0.2;

use crate::text::PreparedText;
use sow_core::player::PlayerType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameplatePresentation {
    Full,
    Compact,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameplateStyle {
    HumanGhost,
    Tribe,
}

impl NameplateStyle {
    pub fn for_player(player_type: PlayerType, is_campaign_faction: bool) -> Self {
        match player_type {
            PlayerType::Human | PlayerType::Nation => Self::HumanGhost,
            PlayerType::Bot if is_campaign_faction => Self::HumanGhost,
            PlayerType::Bot => Self::Tribe,
        }
    }
}

pub trait NameplateCapacityPlan {
    fn has_human_priority(&self) -> bool;
    fn full_instance_count(&self) -> Option<usize>;
    fn select_presentation(&mut self, presentation: NameplatePresentation);
    fn selected_presentation(&self) -> NameplatePresentation;
}

pub fn assign_nameplate_capacity<T: NameplateCapacityPlan>(plans: &mut [T], mut remaining: usize) {
    const COMPACT_INSTANCES: usize = 2;

    for plan in plans.iter_mut().filter(|plan| plan.has_human_priority()) {
        if remaining >= COMPACT_INSTANCES {
            plan.select_presentation(NameplatePresentation::Compact);
            remaining -= COMPACT_INSTANCES;
        } else {
            plan.select_presentation(NameplatePresentation::Hidden);
        }
    }

    for plan in plans.iter_mut().filter(|plan| plan.has_human_priority()) {
        if plan.selected_presentation() != NameplatePresentation::Compact {
            continue;
        }
        let Some(full_count) = plan.full_instance_count() else {
            continue;
        };
        let extra = full_count.saturating_sub(COMPACT_INSTANCES);
        if extra <= remaining {
            remaining -= extra;
            if full_count < COMPACT_INSTANCES {
                remaining += COMPACT_INSTANCES - full_count;
            }
            plan.select_presentation(NameplatePresentation::Full);
        }
    }

    for plan in plans.iter_mut().filter(|plan| !plan.has_human_priority()) {
        if let Some(full_count) = plan.full_instance_count()
            && full_count <= remaining
        {
            remaining -= full_count;
            plan.select_presentation(NameplatePresentation::Full);
        } else if remaining >= COMPACT_INSTANCES {
            remaining -= COMPACT_INSTANCES;
            plan.select_presentation(NameplatePresentation::Compact);
        } else {
            plan.select_presentation(NameplatePresentation::Hidden);
        }
    }
}

#[derive(Clone, Copy)]
pub struct NameplateMetrics {
    render_size: f32,
    avatar_diameter: f32,
    avatar_radius: f32,
    badge_size: f32,
    troops_render_size: f32,
}

impl NameplateMetrics {
    pub fn compute(scaled_size: f32, style: NameplateStyle, show_bot_avatars: bool) -> Self {
        let render_size = if scaled_size.is_finite() {
            scaled_size.max(7.0)
        } else {
            7.0
        };
        let avatar_scale = match style {
            NameplateStyle::HumanGhost => HUMAN_AVATAR_SCALE,
            NameplateStyle::Tribe if show_bot_avatars => BOT_AVATAR_SCALE,
            NameplateStyle::Tribe => 0.0,
        };
        let avatar_diameter = if avatar_scale > 0.0 {
            (render_size * avatar_scale).max(4.0)
        } else {
            0.0
        };
        Self {
            render_size,
            avatar_diameter,
            avatar_radius: avatar_diameter * 0.5,
            badge_size: avatar_diameter * BADGE_AVATAR_RATIO,
            troops_render_size: render_size * TROOPS_SCALE,
        }
    }

    pub fn render_size(self) -> f32 {
        self.render_size
    }

    pub fn avatar_diameter(self) -> f32 {
        self.avatar_diameter
    }

    pub fn avatar_radius(self) -> f32 {
        self.avatar_radius
    }

    pub fn badge_size(self) -> f32 {
        self.badge_size
    }

    pub fn troops_render_size(self) -> f32 {
        self.troops_render_size
    }
}

pub fn fitted_font_px(
    scaled_size: f32,
    fit_scale: f32,
    font_scale: f32,
    show_names: bool,
    show_troops: bool,
) -> f32 {
    if !scaled_size.is_finite() || !fit_scale.is_finite() || fit_scale < 0.0 {
        return 0.0;
    }
    let font_px = scaled_size.max(7.0) * font_size_scale(font_scale) * fit_scale;
    if !font_px.is_finite() {
        return 0.0;
    }
    if show_names {
        font_px
    } else if show_troops {
        font_px * TROOPS_SCALE
    } else {
        f32::INFINITY
    }
}

pub fn font_size_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.max(0.1)
    } else {
        0.1
    }
}

pub fn sample_due(last_tick: Option<u64>, current_tick: u64, my_id_changed: bool) -> bool {
    my_id_changed
        || last_tick.is_none_or(|last_tick| {
            current_tick.saturating_sub(last_tick) >= NAMEPLATE_SAMPLE_TICKS
        })
}

pub fn size_needs_interpolation(from: f32, to: f32) -> bool {
    (to - from).abs() > NAMEPLATE_SIZE_DEADZONE
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapPoint(pub [f32; 2]);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenPoint(pub [f32; 2]);

/// Returns whether a projected land rectangle touches the viewport.
#[inline]
pub fn land_rect_intersects_viewport(
    min: ScreenPoint,
    max: ScreenPoint,
    viewport_width: f32,
    viewport_height: f32,
) -> bool {
    !(max.0[0] < 0.0 || min.0[0] > viewport_width || max.0[1] < 0.0 || min.0[1] > viewport_height)
}

/// Culls off-screen land before running the potentially expensive fog lookup.
#[inline]
pub fn nameplate_visible_after_viewport_and_fog(
    min: ScreenPoint,
    max: ScreenPoint,
    viewport_width: f32,
    viewport_height: f32,
    fog_check: impl FnOnce() -> bool,
) -> bool {
    land_rect_intersects_viewport(min, max, viewport_width, viewport_height) && fog_check()
}

/// Mirrors the nameplate fog rule; callers still cull off-screen land first.
#[inline]
pub fn fog_allows_nameplate(
    fog_enabled: bool,
    is_local_player: bool,
    fog_hidden_phase: bool,
    tile_explored: bool,
) -> bool {
    !fog_enabled || is_local_player || fog_hidden_phase || tile_explored
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldRect(pub [f32; 4]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl TileRect {
    pub fn center(self) -> MapPoint {
        MapPoint([
            (self.x0 as f32 + self.x1 as f32) * 0.5,
            (self.y0 as f32 + self.y1 as f32) * 0.5,
        ])
    }

    pub fn world_bounds(self) -> WorldRect {
        WorldRect([
            self.x0 as f32,
            self.y0 as f32,
            self.x1 as f32,
            self.y1 as f32,
        ])
    }

    fn area(self) -> u64 {
        u64::from(self.x1 - self.x0) * u64::from(self.y1 - self.y0)
    }
}

#[derive(Clone, Copy, Default)]
struct LargestLandmass {
    component_id: u32,
    tile_count: usize,
    bounds: TileRect,
    name_rect: Option<TileRect>,
}

#[derive(Clone, Copy)]
struct HistogramEntry {
    start: usize,
    height: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct MapIdentity {
    width: u32,
    height: u32,
    tile_count: usize,
    terrain_ptr: usize,
    owner_ptr: usize,
}

impl MapIdentity {
    fn of(map: &sow_core::map::GameMap) -> Option<Self> {
        let tile_count = (map.width as usize).checked_mul(map.height as usize)?;
        if map.width == 0
            || map.height == 0
            || map.terrain.len() != tile_count
            || map.owner_states().len() != tile_count
        {
            return None;
        }
        Some(Self {
            width: map.width,
            height: map.height,
            tile_count,
            terrain_ptr: map.terrain.as_ptr() as usize,
            owner_ptr: map.owner_states().as_ptr() as usize,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RebuildPhase {
    PrepareOwners,
    Prepare,
    Components,
    Rectangles,
}

const COMPONENT_ID_CHUNK_TILES: usize = 4096;
const OWNER_PREPARE_CHUNK: usize = 256;
const OWNER_COUNT: usize = sow_core::map::GameMap::PLAYER_ID_MASK as usize + 1;

#[derive(Debug, Default)]
struct ComponentIdBuffer {
    chunks: Vec<Box<[u32]>>,
    len: usize,
}

impl ComponentIdBuffer {
    fn len(&self) -> usize {
        self.len
    }

    fn set_len(&mut self, len: usize) {
        self.len = len;
    }

    fn prepare(&mut self, index: usize) {
        assert!(index < self.len);
        let chunk_index = index / COMPONENT_ID_CHUNK_TILES;
        if chunk_index == self.chunks.len() {
            self.chunks
                .push(vec![0; COMPONENT_ID_CHUNK_TILES].into_boxed_slice());
        }
        self.chunks[chunk_index][index % COMPONENT_ID_CHUNK_TILES] = 0;
    }

    fn get(&self, index: usize) -> u32 {
        assert!(index < self.len);
        self.chunks[index / COMPONENT_ID_CHUNK_TILES][index % COMPONENT_ID_CHUNK_TILES]
    }

    fn set(&mut self, index: usize, value: u32) {
        assert!(index < self.len);
        self.chunks[index / COMPONENT_ID_CHUNK_TILES][index % COMPONENT_ID_CHUNK_TILES] = value;
    }
}

impl PartialEq for ComponentIdBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && (0..self.len).all(|index| self.get(index) == other.get(index))
    }
}

impl Eq for ComponentIdBuffer {}

#[derive(Default)]
struct OwnerIdBuffer {
    chunks: Vec<Box<[u16]>>,
    len: usize,
}

impl OwnerIdBuffer {
    fn capture_snapshot(&mut self, owner_states: &[u16]) {
        self.len = owner_states.len();
        for (chunk_index, values) in owner_states.chunks(COMPONENT_ID_CHUNK_TILES).enumerate() {
            if chunk_index == self.chunks.len() {
                self.chunks
                    .push(vec![0; COMPONENT_ID_CHUNK_TILES].into_boxed_slice());
            }
            self.chunks[chunk_index][..values.len()].copy_from_slice(values);
        }
    }

    fn get(&self, index: usize) -> u16 {
        assert!(index < self.len);
        self.chunks[index / COMPONENT_ID_CHUNK_TILES][index % COMPONENT_ID_CHUNK_TILES]
            & sow_core::map::GameMap::PLAYER_ID_MASK
    }
    fn clear(&mut self) {
        self.len = 0;
    }
}

#[derive(Default)]
struct OwnerLandmassBuffer {
    chunks: Vec<Option<Box<[LargestLandmass]>>>,
    len: usize,
}

impl OwnerLandmassBuffer {
    fn prepare(&mut self, index: usize) {
        self.ensure_chunk(index);
        *self.get_mut(index).unwrap() = LargestLandmass::default();
    }

    fn get(&self, index: usize) -> Option<&LargestLandmass> {
        if index >= self.len {
            return None;
        }
        self.chunks
            .get(index / OWNER_PREPARE_CHUNK)?
            .as_deref()?
            .get(index % OWNER_PREPARE_CHUNK)
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut LargestLandmass> {
        if index >= self.len {
            return None;
        }
        self.chunks
            .get_mut(index / OWNER_PREPARE_CHUNK)?
            .as_deref_mut()?
            .get_mut(index % OWNER_PREPARE_CHUNK)
    }

    fn set(&mut self, index: usize, value: LargestLandmass) {
        self.ensure_chunk(index);
        *self.get_mut(index).unwrap() = value;
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn ensure_chunk(&mut self, index: usize) {
        let chunk_index = index / OWNER_PREPARE_CHUNK;
        if self.chunks.len() <= chunk_index {
            self.chunks.resize_with(chunk_index + 1, || None);
        }
        if self.chunks[chunk_index].is_none() {
            self.chunks[chunk_index] =
                Some(vec![LargestLandmass::default(); OWNER_PREPARE_CHUNK].into_boxed_slice());
        }
        self.len = self.len.max(index + 1);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RectanglePhase {
    ScanRow,
    ScanRuns,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HistogramPhase {
    FindEnd,
    Evaluate,
}

#[derive(Clone, Copy)]
struct ActiveComponent {
    id: u32,
    owner_id: u16,
    count: usize,
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
}

#[derive(Clone, Copy)]
struct HistogramRun {
    component_id: u32,
    owner_id: u16,
    start: usize,
    end: usize,
    column: usize,
    rectangle_start: usize,
    current_height: Option<u32>,
    phase: HistogramPhase,
}

struct RebuildWork {
    identity: MapIdentity,
    ownership_revision: u64,
    tick: u64,
    phase: RebuildPhase,
    owner_prepare_index: usize,
    prepare_index: usize,
    seed_cursor: usize,
    next_component_id: u32,
    active_component: Option<ActiveComponent>,
    row: usize,
    row_cursor: usize,
    row_phase: RectanglePhase,
    run_cursor: usize,
    histogram_run: Option<HistogramRun>,
}

#[derive(Default)]
pub struct NameplateLandCache {
    component_ids: ComponentIdBuffer,
    owner_ids: OwnerIdBuffer,
    flood_stack: Vec<usize>,
    column_heights: Vec<u32>,
    column_components: Vec<u32>,
    histogram_stack: Vec<HistogramEntry>,
    largest_by_owner: OwnerLandmassBuffer,
    map_width: u32,
    map_height: u32,
    last_rebuild_tick: Option<u64>,
    ownership_revision: u64,
    completed_map_identity: Option<MapIdentity>,
    rebuild_work: Option<RebuildWork>,
}

impl NameplateLandCache {
    pub fn rect_for(&self, player_id: u16) -> Option<TileRect> {
        self.largest_by_owner
            .get(usize::from(
                player_id & sow_core::map::GameMap::PLAYER_ID_MASK,
            ))?
            .name_rect
    }

    pub fn is_ready_for(&self, map: &sow_core::map::GameMap) -> bool {
        MapIdentity::of(map).is_some_and(|identity| {
            self.rebuild_work.is_none()
                && self.completed_map_identity == Some(identity)
                && self.ownership_revision == map.ownership_revision()
        })
    }

    pub fn has_result_for(&self, map: &sow_core::map::GameMap) -> bool {
        MapIdentity::of(map).is_some_and(|identity| {
            self.rebuild_work.is_none() && self.completed_map_identity == Some(identity)
        })
    }

    pub fn has_landmass(&self, player_id: u16) -> bool {
        self.largest_by_owner
            .get(usize::from(
                player_id & sow_core::map::GameMap::PLAYER_ID_MASK,
            ))
            .is_some_and(|landmass| landmass.tile_count > 0)
    }

    pub fn update_spawn_anchor(
        &mut self,
        map: &sow_core::map::GameMap,
        player_id: u16,
        spawn_x: u32,
        spawn_y: u32,
    ) -> Option<TileRect> {
        let owner_id = player_id & sow_core::map::GameMap::PLAYER_ID_MASK;
        let landmass = spawn_landmass(map, owner_id, spawn_x, spawn_y, &mut self.flood_stack)?;
        let owner_index = usize::from(owner_id);
        self.largest_by_owner.set(owner_index, landmass);
        landmass.name_rect
    }

    /// Anchors an owner whose whole territory is one fresh spawn blob around `(x, y)` without a
    /// full-map rebuild. Refuses (returns false) when the local flood does not cover all
    /// `tile_count` tiles, so a partial answer is never cached.
    pub fn anchor_fresh_owner(
        &mut self,
        map: &sow_core::map::GameMap,
        player_id: u16,
        x: u32,
        y: u32,
        tile_count: u32,
    ) -> bool {
        let owner_id = player_id & sow_core::map::GameMap::PLAYER_ID_MASK;
        if !self.has_result_for(map) || tile_count as usize > SPAWN_LOCAL_AREA {
            return false;
        }
        let Some(landmass) = spawn_landmass(map, owner_id, x, y, &mut self.flood_stack) else {
            return false;
        };
        if landmass.tile_count != tile_count as usize {
            return false;
        }
        self.largest_by_owner.set(usize::from(owner_id), landmass);
        true
    }

    pub fn needs_rebuild(&self, map: &sow_core::map::GameMap, tick: u64, force: bool) -> bool {
        let Some(identity) = MapIdentity::of(map) else {
            return true;
        };
        self.completed_map_identity != Some(identity)
            || self.map_width != map.width
            || self.map_height != map.height
            || self.component_ids.len() != identity.tile_count
            || force
            || (self.ownership_revision != map.ownership_revision()
                && self
                    .last_rebuild_tick
                    .is_none_or(|last| tick.saturating_sub(last) >= LAND_REFRESH_TICKS))
    }

    pub fn rebuild(&mut self, map: &sow_core::map::GameMap, tick: u64) -> bool {
        let Some(identity) = MapIdentity::of(map) else {
            self.invalidate();
            return false;
        };
        self.begin_rebuild(identity, map.ownership_revision(), tick, map.owner_states());
        loop {
            match self.advance_rebuild(map, tick, None, None) {
                Ok(true) => return true,
                Ok(false) => continue,
                Err(()) => return false,
            }
        }
    }

    pub fn step_rebuild(
        &mut self,
        map: &sow_core::map::GameMap,
        tick: u64,
        budget: web_time::Duration,
    ) -> Result<bool, ()> {
        self.advance_rebuild(map, tick, Some(budget), None)
    }

    fn advance_rebuild(
        &mut self,
        map: &sow_core::map::GameMap,
        tick: u64,
        budget: Option<web_time::Duration>,
        operation_limit: Option<usize>,
    ) -> Result<bool, ()> {
        let started_at = web_time::Instant::now();
        let Some(identity) = MapIdentity::of(map) else {
            self.invalidate();
            return Err(());
        };
        if self.is_ready_for(map) {
            return Ok(true);
        }
        let ownership_revision = map.ownership_revision();
        if !self
            .rebuild_work
            .as_ref()
            .is_some_and(|work| work.identity == identity)
        {
            self.begin_rebuild(identity, ownership_revision, tick, map.owner_states());
        }

        let width = identity.width as usize;
        let height = identity.height as usize;
        let tile_count = identity.tile_count;
        let mut operations = 0usize;

        loop {
            if operation_limit.is_some_and(|limit| operations >= limit) {
                return Ok(false);
            }
            if operations.is_multiple_of(256)
                && budget.is_some_and(|budget| started_at.elapsed() >= budget)
            {
                return Ok(false);
            }

            let phase = self.rebuild_work.as_ref().unwrap().phase;
            match phase {
                RebuildPhase::PrepareOwners => {
                    let index = self.rebuild_work.as_ref().unwrap().owner_prepare_index;
                    if index == OWNER_COUNT {
                        self.rebuild_work.as_mut().unwrap().phase = RebuildPhase::Prepare;
                        continue;
                    }
                    self.largest_by_owner.prepare(index);
                    self.rebuild_work.as_mut().unwrap().owner_prepare_index += 1;
                    operations += 1;
                }
                RebuildPhase::Prepare => {
                    let index = self.rebuild_work.as_ref().unwrap().prepare_index;
                    if index == tile_count {
                        self.rebuild_work.as_mut().unwrap().phase = RebuildPhase::Components;
                        continue;
                    }
                    self.component_ids.prepare(index);
                    self.rebuild_work.as_mut().unwrap().prepare_index += 1;
                    operations += 1;
                }
                RebuildPhase::Components => {
                    let seed = self.rebuild_work.as_ref().unwrap().seed_cursor;
                    if self
                        .rebuild_work
                        .as_ref()
                        .unwrap()
                        .active_component
                        .is_none()
                    {
                        if seed == tile_count {
                            self.column_heights.resize(width, 0);
                            self.column_heights.fill(0);
                            self.column_components.resize(width, 0);
                            self.column_components.fill(0);
                            let work = self.rebuild_work.as_mut().unwrap();
                            work.phase = RebuildPhase::Rectangles;
                            work.row = 0;
                            work.row_cursor = 0;
                            work.row_phase = RectanglePhase::ScanRow;
                            work.run_cursor = 0;
                            continue;
                        }

                        let owner_id = self.owner_ids.get(seed);
                        self.rebuild_work.as_mut().unwrap().seed_cursor += 1;
                        operations += 1;
                        if owner_id == 0
                            || !map.terrain[seed].is_land()
                            || self.component_ids.get(seed) != 0
                        {
                            continue;
                        }
                        let Some(component_id) = self
                            .rebuild_work
                            .as_ref()
                            .unwrap()
                            .next_component_id
                            .checked_add(1)
                        else {
                            self.invalidate();
                            return Err(());
                        };
                        let x = (seed % width) as u32;
                        let y = (seed / width) as u32;
                        self.component_ids.set(seed, component_id);
                        self.flood_stack.push(seed);
                        let work = self.rebuild_work.as_mut().unwrap();
                        work.next_component_id = component_id;
                        work.active_component = Some(ActiveComponent {
                            id: component_id,
                            owner_id,
                            count: 0,
                            min_x: x,
                            max_x: x,
                            min_y: y,
                            max_y: y,
                        });
                        continue;
                    }

                    let Some(index) = self.flood_stack.pop() else {
                        self.invalidate();
                        return Err(());
                    };
                    let x = index % width;
                    let y = index / width;
                    let mut component = self
                        .rebuild_work
                        .as_ref()
                        .unwrap()
                        .active_component
                        .unwrap();
                    component.count += 1;
                    component.min_x = component.min_x.min(x as u32);
                    component.max_x = component.max_x.max(x as u32);
                    component.min_y = component.min_y.min(y as u32);
                    component.max_y = component.max_y.max(y as u32);

                    let first_y = y.saturating_sub(1);
                    let last_y = (y + 1).min(height - 1);
                    let first_x = x.saturating_sub(1);
                    let last_x = (x + 1).min(width - 1);
                    for neighbor_y in first_y..=last_y {
                        let row = neighbor_y * width;
                        for neighbor_x in first_x..=last_x {
                            let neighbor = row + neighbor_x;
                            if self.component_ids.get(neighbor) != 0
                                || !map.terrain[neighbor].is_land()
                                || self.owner_ids.get(neighbor) != component.owner_id
                            {
                                continue;
                            }
                            self.component_ids.set(neighbor, component.id);
                            self.flood_stack.push(neighbor);
                        }
                    }
                    operations += 1;
                    if self.flood_stack.is_empty() {
                        let best = self
                            .largest_by_owner
                            .get_mut(usize::from(component.owner_id))
                            .expect("owner buffers are prepared before component traversal");
                        if component.count > best.tile_count {
                            best.component_id = component.id;
                            best.tile_count = component.count;
                            best.bounds = TileRect {
                                x0: component.min_x,
                                y0: component.min_y,
                                x1: component.max_x + 1,
                                y1: component.max_y + 1,
                            };
                        }
                        self.rebuild_work.as_mut().unwrap().active_component = None;
                    } else {
                        self.rebuild_work.as_mut().unwrap().active_component = Some(component);
                    }
                }
                RebuildPhase::Rectangles => {
                    let row = self.rebuild_work.as_ref().unwrap().row;
                    if row == height {
                        let work = self.rebuild_work.take().unwrap();
                        self.map_width = identity.width;
                        self.map_height = identity.height;
                        self.ownership_revision = work.ownership_revision;
                        self.last_rebuild_tick = Some(work.tick);
                        self.completed_map_identity = Some(work.identity);
                        return Ok(true);
                    }

                    let work = self.rebuild_work.as_ref().unwrap();
                    if work.row_phase == RectanglePhase::ScanRow {
                        if work.row_cursor == width {
                            let work = self.rebuild_work.as_mut().unwrap();
                            work.row_phase = RectanglePhase::ScanRuns;
                            work.run_cursor = 0;
                            continue;
                        }
                        let x = work.row_cursor;
                        let index = row * width + x;
                        let component_id = self.component_ids.get(index);
                        let owner_id = self.owner_ids.get(index);
                        let selected = component_id != 0
                            && self
                                .largest_by_owner
                                .get(usize::from(owner_id))
                                .is_some_and(|landmass| landmass.component_id == component_id);
                        if selected {
                            self.column_heights[x] = if self.column_components[x] == component_id {
                                self.column_heights[x].saturating_add(1)
                            } else {
                                1
                            };
                            self.column_components[x] = component_id;
                        } else {
                            self.column_heights[x] = 0;
                            self.column_components[x] = 0;
                        }
                        self.rebuild_work.as_mut().unwrap().row_cursor += 1;
                        operations += 1;
                        continue;
                    }

                    if let Some(run) = self.rebuild_work.as_ref().unwrap().histogram_run {
                        if run.phase == HistogramPhase::FindEnd {
                            let same_component = run.end < width
                                && self.column_components[run.end] == run.component_id;
                            let run = self
                                .rebuild_work
                                .as_mut()
                                .unwrap()
                                .histogram_run
                                .as_mut()
                                .unwrap();
                            if same_component {
                                run.end += 1;
                            } else {
                                run.phase = HistogramPhase::Evaluate;
                                run.column = run.start;
                            }
                            operations += 1;
                            continue;
                        }

                        if run.column > run.end {
                            let work = self.rebuild_work.as_mut().unwrap();
                            work.run_cursor = run.end;
                            work.histogram_run = None;
                            self.histogram_stack.clear();
                            continue;
                        }

                        let current_height = run.current_height.unwrap_or_else(|| {
                            if run.column == run.end {
                                0
                            } else {
                                self.column_heights[run.column]
                            }
                        });
                        if run.current_height.is_none() {
                            let active_run = self
                                .rebuild_work
                                .as_mut()
                                .unwrap()
                                .histogram_run
                                .as_mut()
                                .unwrap();
                            active_run.current_height = Some(current_height);
                            active_run.rectangle_start = run.column;
                        }
                        let rectangle_start = self
                            .rebuild_work
                            .as_ref()
                            .unwrap()
                            .histogram_run
                            .unwrap()
                            .rectangle_start;

                        if self
                            .histogram_stack
                            .last()
                            .is_some_and(|entry| entry.height > current_height)
                        {
                            let entry = self.histogram_stack.pop().expect("stack was checked");
                            let candidate = TileRect {
                                x0: entry.start as u32,
                                y0: row as u32 + 1 - entry.height,
                                x1: run.column as u32,
                                y1: row as u32 + 1,
                            };
                            let best = self
                                .largest_by_owner
                                .get_mut(usize::from(run.owner_id))
                                .expect("owner buffers are prepared before rectangle traversal");
                            if preferred_name_rect(candidate, best.name_rect, best.bounds) {
                                best.name_rect = Some(candidate);
                            }
                            self.rebuild_work
                                .as_mut()
                                .unwrap()
                                .histogram_run
                                .as_mut()
                                .unwrap()
                                .rectangle_start = entry.start;
                        } else {
                            if current_height > 0
                                && self
                                    .histogram_stack
                                    .last()
                                    .is_none_or(|entry| entry.height < current_height)
                            {
                                self.histogram_stack.push(HistogramEntry {
                                    start: rectangle_start,
                                    height: current_height,
                                });
                            }
                            let active_run = self
                                .rebuild_work
                                .as_mut()
                                .unwrap()
                                .histogram_run
                                .as_mut()
                                .unwrap();
                            active_run.column += 1;
                            active_run.current_height = None;
                        }
                        operations += 1;
                        continue;
                    }

                    let run_cursor = work.run_cursor;
                    if run_cursor == width {
                        let work = self.rebuild_work.as_mut().unwrap();
                        work.row += 1;
                        work.row_cursor = 0;
                        work.row_phase = RectanglePhase::ScanRow;
                        continue;
                    }
                    let component_id = self.column_components[run_cursor];
                    if component_id == 0 {
                        self.rebuild_work.as_mut().unwrap().run_cursor += 1;
                        operations += 1;
                        continue;
                    }
                    let owner_id = self.owner_ids.get(row * width + run_cursor);
                    self.histogram_stack.clear();
                    self.rebuild_work.as_mut().unwrap().histogram_run = Some(HistogramRun {
                        component_id,
                        owner_id,
                        start: run_cursor,
                        end: run_cursor,
                        column: run_cursor,
                        rectangle_start: run_cursor,
                        current_height: None,
                        phase: HistogramPhase::FindEnd,
                    });
                }
            }
        }
    }

    fn begin_rebuild(
        &mut self,
        identity: MapIdentity,
        ownership_revision: u64,
        tick: u64,
        owner_states: &[u16],
    ) {
        self.completed_map_identity = None;
        self.component_ids.set_len(identity.tile_count);
        // Turns mutate the map between frames. Take one atomic owner snapshot so turn activity
        // cannot repeatedly restart a multi-frame cache build.
        self.owner_ids.capture_snapshot(owner_states);
        self.flood_stack.clear();
        self.column_heights.clear();
        self.column_components.clear();
        self.histogram_stack.clear();
        self.rebuild_work = Some(RebuildWork {
            identity,
            ownership_revision,
            tick,
            phase: RebuildPhase::PrepareOwners,
            owner_prepare_index: 0,
            prepare_index: 0,
            seed_cursor: 0,
            next_component_id: 0,
            active_component: None,
            row: 0,
            row_cursor: 0,
            row_phase: RectanglePhase::ScanRow,
            run_cursor: 0,
            histogram_run: None,
        });
    }

    fn invalidate(&mut self) {
        self.completed_map_identity = None;
        self.rebuild_work = None;
        self.owner_ids.clear();
        self.largest_by_owner.clear();
    }
}

fn spawn_landmass(
    map: &sow_core::map::GameMap,
    owner_id: u16,
    spawn_x: u32,
    spawn_y: u32,
    flood_stack: &mut Vec<usize>,
) -> Option<LargestLandmass> {
    let tile_count = (map.width as usize).checked_mul(map.height as usize)?;
    if owner_id == 0
        || spawn_x >= map.width
        || spawn_y >= map.height
        || map.width == 0
        || map.height == 0
        || map.owner_states().len() != tile_count
        || map.terrain.len() != tile_count
    {
        return None;
    }

    let radius = sow_core::config::SPAWN_RADIUS;
    let x0 = spawn_x.saturating_sub(radius);
    let y0 = spawn_y.saturating_sub(radius);
    let x1 = spawn_x
        .saturating_add(radius)
        .saturating_add(1)
        .min(map.width);
    let y1 = spawn_y
        .saturating_add(radius)
        .saturating_add(1)
        .min(map.height);
    let local_width = (x1 - x0) as usize;
    let local_height = (y1 - y0) as usize;
    let mut components = [0u8; SPAWN_LOCAL_AREA];
    let mut next_component = 0u8;
    let mut largest = LargestLandmass::default();
    let mut largest_component = 0u8;

    for seed_y in 0..local_height {
        for seed_x in 0..local_width {
            let seed = seed_y * local_width + seed_x;
            if components[seed] != 0
                || !spawn_tile_owned_by(
                    map,
                    owner_id,
                    spawn_x,
                    spawn_y,
                    x0 + seed_x as u32,
                    y0 + seed_y as u32,
                )
            {
                continue;
            }

            next_component += 1;
            components[seed] = next_component;
            flood_stack.clear();
            flood_stack.push(seed);
            let mut count = 0usize;
            let mut min_x = x0 + seed_x as u32;
            let mut max_x = min_x;
            let mut min_y = y0 + seed_y as u32;
            let mut max_y = min_y;

            while let Some(index) = flood_stack.pop() {
                let local_x = index % local_width;
                let local_y = index / local_width;
                let x = x0 + local_x as u32;
                let y = y0 + local_y as u32;
                count += 1;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);

                for neighbor_y in local_y.saturating_sub(1)..=(local_y + 1).min(local_height - 1) {
                    for neighbor_x in local_x.saturating_sub(1)..=(local_x + 1).min(local_width - 1)
                    {
                        let neighbor = neighbor_y * local_width + neighbor_x;
                        if components[neighbor] != 0
                            || !spawn_tile_owned_by(
                                map,
                                owner_id,
                                spawn_x,
                                spawn_y,
                                x0 + neighbor_x as u32,
                                y0 + neighbor_y as u32,
                            )
                        {
                            continue;
                        }
                        components[neighbor] = next_component;
                        flood_stack.push(neighbor);
                    }
                }
            }

            if count > largest.tile_count {
                largest_component = next_component;
                largest = LargestLandmass {
                    component_id: u32::from(next_component),
                    tile_count: count,
                    bounds: TileRect {
                        x0: min_x,
                        y0: min_y,
                        x1: max_x + 1,
                        y1: max_y + 1,
                    },
                    name_rect: None,
                };
            }
        }
    }

    if largest_component == 0 {
        return None;
    }

    let mut column_heights = [0u32; SPAWN_LOCAL_SIDE];
    for local_y in 0..local_height {
        for local_x in 0..local_width {
            let index = local_y * local_width + local_x;
            column_heights[local_x] = if components[index] == largest_component {
                column_heights[local_x].saturating_add(1)
            } else {
                0
            };
        }
        for left in 0..local_width {
            let mut min_height = u32::MAX;
            for right in left..local_width {
                min_height = min_height.min(column_heights[right]);
                if min_height == 0 {
                    break;
                }
                let candidate = TileRect {
                    x0: x0 + left as u32,
                    y0: y0 + local_y as u32 + 1 - min_height,
                    x1: x0 + right as u32 + 1,
                    y1: y0 + local_y as u32 + 1,
                };
                if preferred_name_rect(candidate, largest.name_rect, largest.bounds) {
                    largest.name_rect = Some(candidate);
                }
            }
        }
    }
    largest.name_rect.map(|_| largest)
}

fn spawn_tile_owned_by(
    map: &sow_core::map::GameMap,
    owner_id: u16,
    spawn_x: u32,
    spawn_y: u32,
    x: u32,
    y: u32,
) -> bool {
    let radius = u64::from(sow_core::config::SPAWN_RADIUS);
    let dx = u64::from(x.abs_diff(spawn_x));
    let dy = u64::from(y.abs_diff(spawn_y));
    if dx > radius || dy > radius || dx * dx + dy * dy > radius * radius {
        return false;
    }
    let index = map.ref_id(x, y);
    map.owner_states()[index] & sow_core::map::GameMap::PLAYER_ID_MASK == owner_id
        && map.terrain[index].is_land()
}

fn preferred_name_rect(
    candidate: TileRect,
    current: Option<TileRect>,
    land_bounds: TileRect,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    let candidate_area = candidate.area();
    let current_area = current.area();
    if candidate_area != current_area {
        return candidate_area > current_area;
    }
    let candidate_distance = rect_center_distance_squared(candidate, land_bounds);
    let current_distance = rect_center_distance_squared(current, land_bounds);
    (candidate_distance, candidate.y0, candidate.x0) < (current_distance, current.y0, current.x0)
}

fn rect_center_distance_squared(rect: TileRect, bounds: TileRect) -> u128 {
    let dx = (i128::from(rect.x0) + i128::from(rect.x1))
        - (i128::from(bounds.x0) + i128::from(bounds.x1));
    let dy = (i128::from(rect.y0) + i128::from(rect.y1))
        - (i128::from(bounds.y0) + i128::from(bounds.y1));
    (dx * dx + dy * dy) as u128
}

pub fn fit_size_to_land(
    content_width: f32,
    content_height: f32,
    land_bounds: WorldRect,
    zoom_scaled: f32,
) -> f32 {
    if !content_width.is_finite()
        || !content_height.is_finite()
        || content_width < 0.0
        || content_height < 0.0
        || !land_bounds.0.into_iter().all(f32::is_finite)
        || !zoom_scaled.is_finite()
    {
        return 0.0;
    }
    let available_width = (land_bounds.0[2] - land_bounds.0[0]).max(0.0) * zoom_scaled.max(0.0);
    let available_height = (land_bounds.0[3] - land_bounds.0[1]).max(0.0) * zoom_scaled.max(0.0);
    if available_width <= 0.0 || available_height <= 0.0 {
        return 0.0;
    }
    let width_scale = if content_width > 0.0 {
        available_width / content_width
    } else {
        1.0
    };
    let height_scale = if content_height > 0.0 {
        available_height / content_height
    } else {
        1.0
    };
    width_scale.min(height_scale).min(1.0).max(0.0)
}

#[derive(Clone, Copy, Default)]
pub struct NameplateStatus {
    pub show_names: bool,
    pub show_troops: bool,
    pub is_me: bool,
    pub is_allied: bool,
    pub has_request: bool,
    pub rank: Option<usize>,
    pub has_traitor: bool,
    pub has_disconnected: bool,
}

pub const NAMEPLATE_MAX_BADGES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    Rank1,
    Rank2,
    Rank3,
    Star,
    Request,
    Allied,
    Traitor,
    Disconnected,
}

#[derive(Clone, Copy)]
pub struct NameplateBadge {
    kind: BadgeKind,
    center: ScreenPoint,
    diameter: f32,
    tint: [f32; 4],
}

impl NameplateBadge {
    pub fn kind(self) -> BadgeKind {
        self.kind
    }

    pub fn center(self) -> ScreenPoint {
        self.center
    }

    pub fn diameter(self) -> f32 {
        self.diameter
    }

    pub fn tint(self) -> [f32; 4] {
        self.tint
    }
}

#[derive(Clone, Copy)]
pub struct NameplateBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl NameplateBounds {
    fn empty() -> Self {
        Self {
            min_x: f32::INFINITY,
            min_y: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            max_y: f32::NEG_INFINITY,
        }
    }

    fn include_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.min_x = self.min_x.min(x0);
        self.min_y = self.min_y.min(y0);
        self.max_x = self.max_x.max(x1);
        self.max_y = self.max_y.max(y1);
    }

    fn include_disc(&mut self, center: ScreenPoint, radius: f32) {
        self.include_rect(
            center.0[0] - radius,
            center.0[1] - radius,
            center.0[0] + radius,
            center.0[1] + radius,
        );
    }

    pub fn extents_about(self, center: ScreenPoint) -> [f32; 2] {
        if !self.min_x.is_finite() {
            return [0.0; 2];
        }
        [
            (center.0[0] - self.min_x).max(self.max_x - center.0[0]),
            (center.0[1] - self.min_y).max(self.max_y - center.0[1]),
        ]
    }
}

#[derive(Clone, Copy)]
pub struct NameplateLayout {
    avatar_center: ScreenPoint,
    avatar_radius: f32,
    badge_size: f32,
    left_x: f32,
    right_x: f32,
    rank_center: ScreenPoint,
    star_center: ScreenPoint,
    text_top: f32,
    item_spacing_y: f32,
    name_size: [f32; 2],
    troops_size: [f32; 2],
    name_visual_bounds: Option<[f32; 4]>,
    troops_visual_bounds: Option<[f32; 4]>,
    troops_icon_size: f32,
    troops_icon_gap: f32,
    badge_effect_padding: f32,
    badges: [Option<NameplateBadge>; NAMEPLATE_MAX_BADGES],
    badge_count: usize,
    stack_step: f32,
}

impl NameplateLayout {
    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        center: ScreenPoint,
        metrics: NameplateMetrics,
        name: &PreparedText,
        troops: &PreparedText,
        font_scale: f32,
        status: NameplateStatus,
        badge_effect_padding: f32,
    ) -> Self {
        let font_scale = font_size_scale(font_scale);
        let name_measure = name.measure_at(metrics.render_size * font_scale);
        let name_visual_bounds = name.visual_bounds_at(metrics.render_size * font_scale);
        let troops_icon_size = metrics.troops_render_size * font_scale;
        let troops_measure = troops.measure_at(troops_icon_size);
        let troops_visual_bounds = troops.visual_bounds_at(troops_icon_size);
        let name_size = [name_measure.width, name_measure.height];
        let troops_size = [
            troops_icon_size + 3.0 + troops_measure.width,
            troops_icon_size.max(troops_measure.height),
        ];
        let render_size = metrics.render_size;
        let avatar_diameter = metrics.avatar_diameter;
        let avatar_radius = metrics.avatar_radius;
        let badge_size = metrics.badge_size;
        let name_size = if status.show_names {
            name_size
        } else {
            [0.0; 2]
        };
        let name_visual_bounds = status.show_names.then_some(name_visual_bounds).flatten();
        let troops_size = if status.show_troops {
            troops_size
        } else {
            [0.0; 2]
        };
        let troops_visual_bounds = status.show_troops.then_some(troops_visual_bounds).flatten();
        let item_spacing_y = if status.show_names && status.show_troops {
            render_size * 0.111
        } else {
            0.0
        };
        let text_height = name_size[1] + item_spacing_y + troops_size[1];
        let avatar_text_gap = if avatar_diameter > 0.0 && text_height > 0.0 {
            render_size * AVATAR_TEXT_GAP_SCALE
        } else {
            0.0
        };
        let total_height = avatar_diameter + avatar_text_gap + text_height;
        let content_top = center.0[1] - total_height * 0.5;
        let avatar_center = ScreenPoint([center.0[0], content_top + avatar_diameter * 0.5]);
        let badge_effect_padding = badge_effect_padding.max(0.0);
        let badge_clearance = BADGE_GAP + badge_effect_padding;
        let badge_half = badge_size * 0.5;
        let stack_step = badge_size + BADGE_STACK_GAP + badge_effect_padding * 2.0;
        let left_x = center.0[0] - avatar_radius - badge_half - badge_clearance;
        let right_x = center.0[0] + avatar_radius + badge_half + badge_clearance;

        let mut layout = Self {
            avatar_center,
            avatar_radius,
            badge_size,
            left_x,
            right_x,
            rank_center: ScreenPoint([
                center.0[0],
                avatar_center.0[1] - avatar_radius - badge_half - badge_clearance,
            ]),
            star_center: ScreenPoint([left_x, avatar_center.0[1]]),
            text_top: content_top + avatar_diameter + avatar_text_gap,
            item_spacing_y,
            name_size,
            troops_size,
            name_visual_bounds,
            troops_visual_bounds,
            troops_icon_size,
            troops_icon_gap: 3.0,
            badge_effect_padding,
            badges: [None; NAMEPLATE_MAX_BADGES],
            badge_count: 0,
            stack_step,
        };
        layout.prepare_badges(status);
        layout
    }

    pub fn scaled_about(self, center: ScreenPoint, scale: f32) -> Self {
        let scale_point = |point: ScreenPoint| {
            ScreenPoint([
                center.0[0] + (point.0[0] - center.0[0]) * scale,
                center.0[1] + (point.0[1] - center.0[1]) * scale,
            ])
        };
        Self {
            avatar_center: scale_point(self.avatar_center),
            avatar_radius: self.avatar_radius * scale,
            badge_size: self.badge_size * scale,
            left_x: center.0[0] + (self.left_x - center.0[0]) * scale,
            right_x: center.0[0] + (self.right_x - center.0[0]) * scale,
            rank_center: scale_point(self.rank_center),
            star_center: scale_point(self.star_center),
            text_top: center.0[1] + (self.text_top - center.0[1]) * scale,
            item_spacing_y: self.item_spacing_y * scale,
            name_size: [self.name_size[0] * scale, self.name_size[1] * scale],
            troops_size: [self.troops_size[0] * scale, self.troops_size[1] * scale],
            name_visual_bounds: self
                .name_visual_bounds
                .map(|bounds| bounds.map(|value| value * scale)),
            troops_visual_bounds: self
                .troops_visual_bounds
                .map(|bounds| bounds.map(|value| value * scale)),
            troops_icon_size: self.troops_icon_size * scale,
            troops_icon_gap: self.troops_icon_gap * scale,
            badge_effect_padding: self.badge_effect_padding * scale,
            badges: self.badges.map(|badge| {
                badge.map(|badge| NameplateBadge {
                    center: scale_point(badge.center),
                    diameter: badge.diameter * scale,
                    ..badge
                })
            }),
            badge_count: self.badge_count,
            stack_step: self.stack_step * scale,
        }
    }

    pub fn side_badge_center(&self, left: bool, stack_slot: usize, is_me: bool) -> ScreenPoint {
        let x = if left { self.left_x } else { self.right_x };
        let y = if left && is_me {
            self.avatar_center.0[1] + self.stack_step
        } else {
            self.avatar_center.0[1] - stack_slot as f32 * self.stack_step
        };
        ScreenPoint([x, y])
    }

    pub fn name_anchor(&self, center: ScreenPoint) -> ScreenPoint {
        ScreenPoint([center.0[0], self.text_top + self.name_size[1] * 0.85])
    }

    pub fn avatar_center(&self) -> ScreenPoint {
        self.avatar_center
    }

    pub fn avatar_radius(&self) -> f32 {
        self.avatar_radius
    }

    pub fn badge_effect_padding(&self) -> f32 {
        self.badge_effect_padding
    }

    pub fn badges(&self) -> &[Option<NameplateBadge>; NAMEPLATE_MAX_BADGES] {
        &self.badges
    }

    pub fn badge_count(&self) -> usize {
        self.badge_count
    }

    pub fn troops_text_anchor(&self, center: ScreenPoint) -> ScreenPoint {
        let row_y = if self.name_size[1] > 0.0 {
            self.text_top + self.name_size[1] + self.item_spacing_y
        } else {
            self.text_top
        };
        let left_x = center.0[0] - self.troops_size[0] * 0.5;
        ScreenPoint([
            left_x + self.troops_icon_size + self.troops_icon_gap,
            row_y + self.troops_size[1] * 0.85,
        ])
    }

    pub fn troops_icon(&self, center: ScreenPoint) -> (ScreenPoint, f32) {
        let row_y = if self.name_size[1] > 0.0 {
            self.text_top + self.name_size[1] + self.item_spacing_y
        } else {
            self.text_top
        };
        let left_x = center.0[0] - self.troops_size[0] * 0.5;
        (
            ScreenPoint([
                left_x + self.troops_icon_size * 0.5,
                row_y + self.troops_icon_size * 0.5,
            ]),
            self.troops_icon_size,
        )
    }

    pub fn visual_bounds(self, center: ScreenPoint, text_padding: f32) -> NameplateBounds {
        let mut bounds = NameplateBounds::empty();
        if let Some([min_x, min_y, max_x, max_y]) = self.name_visual_bounds {
            let anchor = self.name_anchor(center);
            bounds.include_rect(
                anchor.0[0] + min_x - text_padding,
                anchor.0[1] + min_y - text_padding,
                anchor.0[0] + max_x + text_padding,
                anchor.0[1] + max_y + text_padding,
            );
        }
        if let Some([min_x, min_y, max_x, max_y]) = self.troops_visual_bounds {
            let anchor = self.troops_text_anchor(center);
            bounds.include_rect(
                anchor.0[0] + min_x - text_padding,
                anchor.0[1] + min_y - text_padding,
                anchor.0[0] + max_x + text_padding,
                anchor.0[1] + max_y + text_padding,
            );
        }
        if self.troops_icon_size > 0.0 {
            let (icon_center, diameter) = self.troops_icon(center);
            bounds.include_disc(icon_center, diameter * 0.5 + self.badge_effect_padding);
        }
        if self.avatar_radius <= 0.0 {
            return bounds;
        }

        bounds.include_disc(self.avatar_center, self.avatar_radius);
        for badge in self.badges[..self.badge_count].iter().flatten() {
            bounds.include_disc(
                badge.center,
                badge.diameter * 0.5 + self.badge_effect_padding,
            );
        }
        bounds
    }

    fn prepare_badges(&mut self, status: NameplateStatus) {
        if self.avatar_radius <= 0.0 {
            return;
        }
        if let Some(rank) = status.rank {
            let (kind, tint) = match rank {
                1 => (
                    BadgeKind::Rank1,
                    [250.0 / 255.0, 204.0 / 255.0, 21.0 / 255.0, 1.0],
                ),
                2 => (
                    BadgeKind::Rank2,
                    [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
                ),
                _ => (
                    BadgeKind::Rank3,
                    [217.0 / 255.0, 119.0 / 255.0, 6.0 / 255.0, 1.0],
                ),
            };
            let center = self.rank_center;
            let size = self.badge_size;
            self.add_badge(kind, center, size, tint);
        }
        if status.is_me {
            let center = self.star_center;
            let size = self.badge_size;
            self.add_badge(BadgeKind::Star, center, size, [1.0; 4]);
        }
        if status.has_request {
            let center = self.side_badge_center(true, 0, status.is_me);
            let size = self.badge_size;
            self.add_badge(BadgeKind::Request, center, size, [1.0; 4]);
        }
        let mut right_slots = 0usize;
        if status.is_allied {
            let center = self.side_badge_center(false, right_slots, status.is_me);
            let size = self.badge_size;
            self.add_badge(BadgeKind::Allied, center, size, [1.0; 4]);
            right_slots += 1;
        }
        if status.has_traitor {
            let center = self.side_badge_center(false, right_slots, status.is_me);
            let size = self.badge_size;
            self.add_badge(BadgeKind::Traitor, center, size, [1.0; 4]);
        }
        if status.has_disconnected {
            let center = ScreenPoint([
                self.avatar_center.0[0] + self.avatar_radius * 0.6,
                self.avatar_center.0[1] + self.avatar_radius * 0.6,
            ]);
            self.add_badge(BadgeKind::Disconnected, center, self.badge_size, [1.0; 4]);
        }
    }

    fn add_badge(&mut self, kind: BadgeKind, center: ScreenPoint, diameter: f32, tint: [f32; 4]) {
        self.badges[self.badge_count] = Some(NameplateBadge {
            kind,
            center,
            diameter,
            tint,
        });
        self.badge_count += 1;
    }
}

