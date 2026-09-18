const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const shell = __dirname;
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const worldOverlays = fs.readFileSync(path.join(shell, "../../sow-client/src/render/world/overlays.rs"), "utf8");

test("tutorial Continue sends the Rust pause command and owns the dialog event", () => {
    assert.match(tutorial, /setPaused\(false\)/);
    assert.match(tutorial, /send\("set_tutorial_paused", \{ paused: paused \}\)/);
    assert.match(tutorial, /overlay\.addEventListener\("click",[\s\S]*?\}, true\)/);
});

test("map menu sends the Rust-validated session, tile, and action", () => {
    assert.match(hud, /button\.dataset\.mapAction = action/);
    assert.match(hud, /send\("map_menu_action", \{/);
    assert.match(hud, /session: Number\(mapMenu\.dataset\.session\)/);
    assert.match(hud, /tile_idx: Number\(mapMenu\.dataset\.tileIdx\)/);
    assert.match(hud, /action: mapButton\.dataset\.mapAction/);
    assert.match(hud, /menu\.dataset\.renderKey/);
    assert.match(hud, /Math\.min\(Math\.max\(x, minX\), maxX\)/);
    assert.match(hudCss, /\.sow-hud__map-menu\.hidden\s*\{\s*display: none/);
});

test("mobile hover is Rust-owned and the HUD only presents its state", () => {
    assert.match(hud, /var hov = hud\.hovered/);
    assert.match(hud, /hoverCard\.classList\.remove\("hidden"\)/);
    assert.match(hud, /hoverCard\.classList\.add\("hidden"\)/);
    assert.doesNotMatch(hud, /addEventListener\("(?:touch|pointer)(?:start|move|up|cancel)"/);
});

test("tutorial pointer stays in the Rust/Blade render pass", () => {
    assert.match(worldOverlays, /fn render_tutorial_pointer/);
    assert.match(worldOverlays, /text\.push_ring/);
    assert.match(worldOverlays, /text\.push_triangle/);
    assert.doesNotMatch(tutorial, /tutorial\.markers|data-tutorial-marker|SOW_tutorial_marker_update/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-marker/);
});
