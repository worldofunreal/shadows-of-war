# Building render pipeline

The world building layer is display-only. Gameplay state remains in
`SimSnapshot::buildings`.

All current logic lives in `sow-client/src/render/world/overlays.rs`, invoked
from the world overlay pass (`render_overlays`, re-exported by
`sow-client/src/render/world/mod.rs`) that runs each frame from
`sow-client/src/render/frame/mod.rs`. The former `buildings/` module
(`render.rs`, `metrics.rs`, `cluster.rs`, `overlays.rs`) was removed in
`93b7ff4a`.

```text
SimSnapshot::buildings
        |
        v
render/frame/mod.rs  -->  world/overlays.rs::render_overlays
                                 |
                                 +--> render_buildings (line ~1127)
                                 |        |
                                 |        +--> BuildingLod::for_zoom (line ~964)
                                 |        |        - keeps the close-zoom scale curve
                                 |        |          (BUILDING_LOD_START_ZOOM / RANGE)
                                 |        |        - compact = natural marker size
                                 |        |          drops under BUILDING_MIN_MARKER_SIZE
                                 |        |        - chooses cluster_cell_size from
                                 |        |          BUILDING_CLUSTER_TARGET_SIZE / zoom
                                 |        |
                                 |        +--> grid clustering via ClusterKey
                                 |        |        - compact groups bucketed by
                                 |        |          (grid_x, grid_y, owner, kind, level)
                                 |        |        - non-compact: exact individual records
                                 |        |
                                 |        +--> building image markers:
                                 |        |    Blade GPU TextRenderer using the 8×4 atlas at
                                 |        |    assets/gameplay/buildings/building_atlas.png
                                 |        +--> level labels and 300 ms completion sparkle:
                                 |             shared text/emoji atlas at
                                 |             assets/gameplay/emoji/atlas_opt.webp
                                 |
                                 +--> BuildingRenderCache
                                          - reuses the clustered building set while
                                            zoom/cluster_cell_size are unchanged
```

## Invariants

- `BuildingLod::for_zoom` is the single source of building marker scale and
  clustering granularity. Do not add another `zoom_scaled * scale` formula to
  `overlays.rs` or a sibling painter.
- `building_scale` is the manual scale control; it does not disable the
  compact-LOD minimum marker size.
- Compact groups are separated by map cell, owner, building kind, and active
  level. Grouped records have no individual id or hover data.
- Individual records preserve id, modules, tile index, and current interaction.
- Placement previews (`render_building_placement_preview`) use the same
  `BuildingRenderCache` path so preview and live markers stay consistent.
- Hover/hit-testing reads `snapshot.buildings` directly (same source as
  `crate::input::find_building_*`); rendering never mutates gameplay state.
- Building levels use their fixed cell in the 22-sprite image atlas and stay in
  the same GPU text batch as labels and markers. A 300 ms GPU-rendered sparkle
  marks a finished upgrade using the existing emoji atlas. The animation is
  tracked per building, not per map tile.

When changing this pipeline, update `BuildingLod` first, then the callers in
`render_buildings`.
