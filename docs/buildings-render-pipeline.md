# Building render pipeline

The world building layer is display-only. Gameplay state remains in
`SimSnapshot::buildings`.

```text
SimSnapshot::buildings
        |
        v
buildings/render.rs::render
        |
        +--> metrics::BuildingLod::for_zoom
        |        - keeps the close-zoom scale curve
        |        - chooses adaptive clustering from projected screen size
        |
        +--> cluster::collect_rendered_buildings
        |        - returns exact buildings when not compact
        |        - returns screen-sized groups when compact
        |
        +--> metrics::BuildingVisualMetrics::for_building
                 - owns marker size, level size, and level offset
                 |
                 +--> emoji: GPU TextRenderer or egui fallback
                 +--> overlays.rs: level badge using the same metrics
```

## Invariants

- `BuildingVisualMetrics` is the only source of marker/badge geometry.
- `building_scale` is the manual scale control; it does not disable the
  compact-LOD minimum marker size.
- Compact groups are separated by map cell, owner, building kind, and active
  level. Grouped records have no individual id or hover data.
- Individual records preserve id, modules, tile index, and current interaction.
- The building LOD does not use `RAILWAYS_HIDE_FLOOR`, upgrade plates, or
  outline styling.

When changing this pipeline, update `metrics.rs` first. Do not add another
`zoom_scaled * scale` formula to `render.rs`, `overlays.rs`, or a sibling
building painter.
