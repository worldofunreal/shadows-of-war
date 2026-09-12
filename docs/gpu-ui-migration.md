# GPU UI Pipeline — Audit & Current Contract

Status: the world nameplate pipeline uses the shared GPU renderer and keeps egui
as a visual fallback. Full UI migration remains planning.

## North star (the principle)

egui's pipeline **works** — it renders emoji and text correctly. Its only problem
is that it is **unperformant** (per-frame galley layout, CPU rasterization,
tessellation, and multi-pass glow redraws). So:

1. **Match egui's visual fidelity.** The GPU path must reproduce what egui draws.
   It must **not** add constraints that degrade the look. (The building shape that
   clips the emoji inside it is exactly the kind of over-constraint we are
   correcting — egui drew the full emoji + outline, unclipped.)
2. **GPU does the work, CPU stays minimal.** Per-frame CPU cost for a glyph/emoji
   should be: metric lookup + position math + push a POD struct. No rasterization,
   no galley, no tessellation, no N-pass overdraw.
3. **Outline/shadow in-shader**, not by redrawing N times (egui does 6×; SDF text
   and the new emoji path do it in 1 instance).

## Current state

| Surface | Path | Status |
|---|---|---|
| Nameplate names + troop counts | `TextRenderer` (SDF font atlas) | ✅ GPU |
| Building level badges | `TextRenderer` | ✅ GPU |
| Projectile troop numbers | `TextRenderer` | ✅ GPU |
| Building shape + emoji | `TextRenderer` (`KIND_EMOJI`) | ✅ GPU |
| Nameplate crown/medals, star ⭐ / disconnect 🔌 | `TextRenderer` (`KIND_EMOJI`) | ✅ GPU |
| Nameplate avatar/category + side badges/express | `TextRenderer` (`KIND_SPRITE`/`KIND_EMOJI`) | ✅ GPU primary, egui fallback |
| Remaining standalone emoji icons outside nameplate (☢️, leaderboard, context menu, avatar, build popover) | egui `try_paint_emoji` (6× redraw) | ❌ egui |
| Inline emoji in text (toasts, HUD counters: `🪙 {}`, `🛡️ {}/{}`, `⚔️ +{}`) | egui text (font/emoji fallback) | ❌ egui |
| Leaderboard / panels / plates / tooltips display text | egui galley | ❌ egui |
| Interactive widgets (buttons, sliders, combos, text edit, scroll, selectable) | egui | ❌ egui |

`TextRenderer` is the single GPU submission pipeline. Its shader keeps separate
branches for the MSDF font atlas, RGBA emoji atlas, sprites, discs, rings, and
rectangles; all branches share the same instanced draw list.

## Nameplate contract

The nameplate orchestrator owns selection and semantic state. Geometry and
painting are owned by the reusable component:

```text
sow-client/src/render/frame/ui.rs::SowApp::render_frame_ui_and_present
    ↓ TextRenderer::begin_frame
sow-client/src/render/world/mod.rs::SowApp::render_world_overlays
    ↓ sow-client/src/render/world/nameplates/render.rs::render
sow-client/src/render/world/nameplates/painter.rs::NameplatePainter
    ├─ NameplateLayout (layout.rs)
    ├─ TextRenderer::push_string / push_emoji (GPU primary)
    └─ sow_ui_kit::widgets::try_paint_emoji_with_style (egui fallback)
    ↓ TextRenderer::draw (frame/ui.rs)
```

`sow-render/src/text/types.rs` defines the shared `TextPaintStyle` and
`OutlineStyle` contract. Emoji quads preserve their logical content bounds and
reserve transparent quad padding for the configured outline/shadow; the shader
maps atlas UVs only through that inner content rectangle. `NameplateStyle` is
the client adapter from `DevConfig` to GPU and egui profiles.

`layout.rs` is the only owner of nameplate geometry proportions. `NameplateStyle`
derives the rendered name/troop sizes and the sword size from the same troop-text
size. `NameplateLayout` consumes those measured bounds; callers must not add a
second `zoom * scale` or icon-size formula.

The current defaults are human avatar `3.6x`, bot/nation avatar `2.2x`, avatar-to-text
gap `0.16x`, troop sword `1.15x` the rendered troop text, and category emoji at `70%`
of the avatar diameter.

On the GPU branch, `NameplatePainter::paint` measures with
`TextRenderer::measure_string` (`TextMeasure { width, height }`) and emits
instances directly; it creates egui galleys only when the GPU renderer is
unavailable. The fallback uses
`prepare_name`, `paint_prepared_name_with_glow`, and
`paint_glow_troops_row_with_style` with the same `NAMEPLATE` emoji profile.

## Verification contract

- `sow-tools/check.sh` is the code guard: workspace check, source-size guard, and
  emoji-pipeline guard.
- `./sow l` is the local Web/WASM visual preview for nameplate iteration.
- `./sow p` is the official Web/WASM/backend production acceptance pipeline; it
  must reach its healthcheck and public-verification stages.
- `./sow a` is Android/Google Play and is outside this pipeline; the owner runs
  it manually.

## Audit findings

Call-site counts (sow-client/src):

- Emoji helpers: `try_paint_emoji` ×22, `paint_emoji_centered` ×3, `emoji_label`
  ×9, `paint_emoji_text_at` ×4, `measure_emoji_text` ×1.
- Text: `painter.text` ×15, `.galley` ×8, `layout_no_wrap` ×11, `paint_glow*` ×10,
  `ui.label` ×32, `ui.heading` ×1.
- Interactive: `ui.button` ×4, `Slider` ×13, `TextEdit` ×2, `ComboBox` ×1,
  `selectable` ×4, `ScrollArea` ×1.

Emoji coverage:
- **Standalone icon emojis are all in the atlas** (`⭐ 🔌 ☢️ ⚔️ 🕊️ 🤝 👑`, all four
  building icons). No garbage-fallback risk in the icon path — the "broken"
  building was the shape-clip constraint, not a missing emoji.
- **Emoji also appear *inline inside text strings*** — toasts (`🎉 You conquered
  {}`, `🤝 Assist on {}`), HUD counters (`🪙 {}`, `🛡️ {}/{}`, `🏭 x{}`),
  victory/help messages. These render as **text** (egui's font+emoji fallback), not
  as atlas lookups. Moving them to GPU requires **mixed text+emoji shaping**.

## The hard parts (what "everything incl. static HUD" actually requires)

1. **Mixed text+emoji runs.** A shaper that splits a string into SDF-glyph runs
   (font atlas) + emoji-sprite runs (color atlas), measured and positioned inline
   on one baseline. Needed for every toast/counter that embeds an emoji.
2. **Layout engine.** egui currently does wrap / align / rows / columns / flex /
   scroll. A minimal layout layer (measure + align + row/col + clip) is needed to
   place HUD panels without egui.
3. **Interaction / hit-testing.** Buttons, sliders, combos, selectable, text edit,
   scroll. This is the framework-level lift: input routing, hover, focus, drag,
   click, IME, caret/selection. This is "write a small UI toolkit" territory.
4. **Z-order.** Today two passes draw in a fixed order (structure → text). For
   panels where rect/emoji/text interleave, we need either a **single merged atlas
   + one instance stream that preserves submission order**, or carefully ordered
   passes. Recommend a merged "draw list" before tackling panels.

## Target architecture

A unified **GPU 2D layer**:
- A retained-free **draw list** API: `push_text`, `push_emoji`, `push_rect`,
  `push_quad(uv)` — all screen-space (world-space variant for map markers).
- Batched into instance streams keyed by atlas; ideally **one merged atlas**
  (SDF font + color emoji + solid white texel for rects) so a single draw call
  preserves submission-order z within a panel.
- Outline / shadow / dilate parameters per instance, resolved in-shader.
- Interactive widgets (only if we go all the way) = a thin immediate-mode layer
  that emits draw-list calls + consumes input; or keep egui for the few
  interactive panels.

## Phased plan

Each phase ships value and is independently testable. Risk rises down the list;
the framework-level interaction work is deliberately last.

| Phase | Scope | Migrates | Risk | Notes |
|---|---|---|---|---|
| 0 | **Done** | nameplates, troop #s, building badges/shapes, status emoji | — | current checkout |
| 1 | **Done** | shared nameplate outline/spacing + building emoji alpha outline | low | one `TextRenderer` emoji geometry contract |
| 2 | **Standalone emoji icons** | remaining `try_paint_emoji` / `paint_emoji_centered` → `push_emoji` (world first, then HUD) | low | kills the 6× redraw |
| 3 | **Display text + inline emoji** | static labels / toasts / counters → GPU; build the **text+emoji shaper** for inline emoji | med | the inline-emoji shaper is the new capability |
| 4 | **Layout helper** | minimal measure/align/row/col/clip for HUD panels | med | enables panels without egui |
| 5 | **Interactive widgets** | buttons, sliders, combos, selectable, scroll, text edit (input/hit-test/focus/IME) | high | the toolkit-level lift; re-evaluate ROI here |
| 6 | **Atlas/coverage** (parallel) | extend SDF font to Latin-1 (intl names); consider merged atlas + true MSDF for crisp titles | med | unblocks names + z-order |

## Recommendation

- Phases 1–4 capture essentially all the **per-frame CPU savings** (the world +
  display HUD is where the cost is). Do these in order.
- Phase 5 is about **fidelity/independence, not performance** — the interactive
  panels are low-count and egui handles them fine. When we reach it, re-decide
  whether pulling them off egui is worth the toolkit build-out, or whether egui
  stays as the "interactive chrome" host.
- Keep the egui path as a **fallback** at every migrated site (the established
  pattern), so no migration can regress visuals.
