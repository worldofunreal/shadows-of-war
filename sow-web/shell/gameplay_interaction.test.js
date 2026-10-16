const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = __dirname;
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const campaignEngine = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
const campaignView = fs.readFileSync(path.join(shell, "sow-campaign-view.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const campaignEditor = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.js"), "utf8");
const campaignEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"), "utf8");
const siteDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/main.rs"), "utf8");
const prodDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/prod.rs"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const indexTemplate = fs.readFileSync(path.join(shell, "index.html.template"), "utf8");
const windowInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/window.rs"), "utf8");
const intentInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/intents.rs"), "utf8");
const hudStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/hud/state.rs"), "utf8");
const matchStartSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/match_start.rs"), "utf8");
const simUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/update.rs"), "utf8");
const simEventsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/events/mod.rs"), "utf8");
const eliminationSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/events/elimination.rs"), "utf8");
const snapshotFxSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/snapshot_fx.rs"), "utf8");
const actionsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/interact/actions.rs"), "utf8");
const cameraFrameUiSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/ui.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const settingsStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/settings/mod.rs"), "utf8");
const uiAppSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/app.rs"), "utf8");
const intentApplySource = fs.readFileSync(path.join(shell, "../../sow-core/src/intent/apply.rs"), "utf8");
const appStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/state.rs"), "utf8");
const buildingOverlaySource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/world/overlays.rs"), "utf8");
const mapClick = fs.readFileSync(path.join(shell, "../../sow-client/src/input/map_click.rs"), "utf8");
const frameSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/mod.rs"), "utf8");
const mapShader = fs.readFileSync(path.join(shell, "../../sow-render/src/shaders/map.wgsl"), "utf8");
function makeHudDocument() {
    const doc = { activeElement: null, nodes: Object.create(null) };
    class Element {
        constructor(tagName) {
            this.tagName = tagName;
            this.children = [];
            this.parentNode = null;
            this.dataset = {};
            this.attributes = {};
            this.hidden = false;
            this.disabled = false;
            this.isConnected = true;
            this.textContent = "";
            this.insertions = 0;
            const styles = new Map(), classes = new Set();
            this.style = { getPropertyValue: key => styles.get(key) || "", setProperty: (key, value) => styles.set(key, value), removeProperty: key => styles.delete(key) };
            this.classList = {
                add: name => classes.add(name),
                contains: name => classes.has(name),
                toggle(name, on) { if (on) classes.add(name); else classes.delete(name); }
            };
        }
        append(...items) { items.forEach(item => this.appendChild(item)); }
        appendChild(item) { return this.insertBefore(item, null); }
        insertBefore(item, before) {
            item.remove();
            const index = before ? this.children.indexOf(before) : this.children.length;
            this.children.splice(index, 0, item);
            item.parentNode = this;
            item.isConnected = true;
            this.insertions += 1;
            return item;
        }
        remove() {
            if (!this.parentNode) return;
            if (this.contains(doc.activeElement)) doc.activeElement = null;
            const siblings = this.parentNode.children;
            siblings.splice(siblings.indexOf(this), 1);
            this.parentNode = null;
            this.isConnected = false;
        }
        replaceChildren(...items) { this.children.slice().forEach(item => item.remove()); this.append(...items); }
        contains(item) { return item === this || this.children.some(child => child.contains(item)); }
        focus() { doc.activeElement = this; }
        getClientRects() { return !this.isConnected || this.hidden || this.classList.contains("hidden") || this.parentNode && !this.parentNode.getClientRects().length ? [] : [{}]; }
        closest(selector) {
            if (selector === "[data-command]") return this.dataset.command ? this : this.parentNode?.closest(selector) || null;
            return null;
        }
        querySelectorAll(selector) {
            return this.children.flatMap(child => [child, ...child.querySelectorAll(selector)])
                .filter(child => selector === "button:not(:disabled)" && child.tagName.toLowerCase() === "button" && !child.disabled);
        }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) { return name === "src" ? this.src || null : this.attributes[name] || null; }
        removeAttribute(name) { delete this.attributes[name]; if (name === "src") this.src = ""; }
    }
    doc.createElement = tag => new Element(tag);
    doc.getElementById = id => doc.nodes[id] || null;
    return { doc, Element };
}
function makePlayerDialogsHarness() {
    const { doc, Element } = makeHudDocument();
    const calls = [], listeners = {};
    const context = vm.createContext({
        document: doc, hudRoot: new Element("root"), hudDevLayer: null,
        leaderboardOpen: false, inboxOpen: false, settingsOpen: false, mapMenuOpen: false, isOver: false,
        window: { SOW_LOCALE: "en", SOW_menu_command: raw => calls.push(JSON.parse(raw)), addEventListener: (name, listener) => { listeners[name] = listener; } },
        asset: name => "/assets/" + name,
        SOW_t: key => context.window.SOW_LOCALE + ":" + key
    });
    const part = (start, end) => {
        const begin = hud.indexOf(start), finish = hud.indexOf(end, begin + start.length);
        assert.ok(begin >= 0 && finish > begin, start);
        return hud.slice(begin, finish);
    };
    vm.runInContext(
        part("    var hudState =", "    function send(")
        + part("    function send(", "    function asset(")
        + part("    function bindAvatarFallback(", "    function clearNotificationCard(")
        + part("    function createPlayerIdentity(", "    function createInboxRequest(")
        + part("        function handleHudClick(event) {", '        hudRoot.addEventListener("click", handleHudClick)')
        + part("    function handleHudStateUpdate(", "    window.SOW_onStateUpdate ="), context
    );
    const canvas = doc.nodes.blade = new Element("canvas");
    canvas.focus();
    const makeRefs = () => {
        const element = (tag, id, command) => {
            const node = new Element(tag);
            node.id = id;
            if (command) node.dataset.command = command;
            doc.nodes[id] = node;
            return node;
        };
        const refs = {
            transfer: element("aside", "sow-hud-transfer"),
            transferTarget: element("div", "sow-hud-transfer-target"),
            transferClose: element("button", "sow-hud-transfer-close", "close_transfer"),
            transferGold: element("input", "sow-hud-transfer-gold"),
            transferTroops: element("input", "sow-hud-transfer-troops"),
            transferIdentity: context.createPlayerIdentity(),
            betrayal: element("div", "sow-hud-betrayal-modal"),
            betrayalTarget: element("div", "sow-hud-betrayal-target"),
            betrayalClose: element("button", "sow-hud-betrayal-close", "cancel_betrayal"),
            betrayalCancel: element("button", "sow-hud-betrayal-cancel", "cancel_betrayal"),
            betrayalConfirm: element("button", "sow-hud-betrayal-confirm", "confirm_betrayal"),
            betrayalIdentity: context.createPlayerIdentity()
        };
        refs.transfer.classList.add("hidden");
        refs.betrayal.classList.add("hidden");
        refs.transferTarget.append(refs.transferIdentity.element);
        refs.betrayalTarget.append(refs.betrayalIdentity.element);
        refs.transfer.append(refs.transferClose, refs.transferTarget, refs.transferGold, refs.transferTroops);
        refs.betrayal.append(refs.betrayalClose, refs.betrayalTarget, refs.betrayalCancel, refs.betrayalConfirm);
        context.hudRoot.append(refs.transfer, refs.betrayal);
        return refs;
    };
    context.hudRefs = makeRefs();
    const renderStart = hud.indexOf("    function renderHud(");
    const transferStart = hud.indexOf("        if (hudRefs.transfer) {", renderStart);
    const transferEnd = hud.indexOf("        renderNotifications(", transferStart);
    const overlayStart = hud.indexOf("        hudRoot.dataset.overlayOpen = String(Boolean(", renderStart);
    const overlayEnd = hud.indexOf("        var dockHidden", overlayStart);
    assert.ok(transferStart > renderStart && transferEnd > transferStart && overlayStart > transferEnd && overlayEnd > overlayStart);
    const redraw = new vm.Script(hud.slice(transferStart, transferEnd) + hud.slice(overlayStart, overlayEnd));
    context.renderHud = () => {
        if (context.hudInitialized === false) { context.hudRefs = makeRefs(); context.hudInitialized = true; }
        context.hud = context.hudState.hud;
        redraw.runInContext(context);
    };
    context.hudInitialized = true;
    context.hudRoot.addEventListener = (name, listener) => { listeners[name] = listener; };
    vm.runInContext(part('    if (hudRoot) hudRoot.addEventListener("keydown",', '    window.addEventListener("resize", updateHudSafeArea);'), context);
    return {
        context, doc, calls, listeners, canvas,
        update: (hudValue, extra = {}) => context.handleHudStateUpdate(JSON.stringify({ phase: "Playing", hud: { attack_ratio: .25, ...hudValue }, ...extra })),
        click: command => context.handleHudClick({ target: { closest: selector => selector === "[data-command]" ? { dataset: { command, playerId: "2" } } : null } }),
        key: (key, shiftKey = false) => { const event = { key, shiftKey, prevented: false, preventDefault() { this.prevented = true; }, stopPropagation() {} }; listeners.keydown(event); return event; }
    };
}

test("gameplay chrome keeps dev tools and separate FPS/ping rows at the bottom of the top-right panel", () => {
    const statusRight = hud.slice(hud.indexOf("sow-hud__status-right"), hud.indexOf("</header>"));
    const exitAt = statusRight.indexOf("prompt_surrender");
    const settingsAt = statusRight.indexOf("toggle_settings");
    const inboxAt = statusRight.indexOf("toggle_inbox");
    const rankingsAt = statusRight.indexOf("toggle_leaderboard");
    const devAt = statusRight.indexOf("sow-hud-dev-btn");
    const fpsAt = statusRight.indexOf("sow-hud-fps");
    const pingAt = statusRight.indexOf("sow-hud-ping");
    assert.ok(exitAt > 0 && exitAt < statusRight.indexOf("toggle_settings")
        && exitAt < statusRight.indexOf("toggle_inbox")
        && exitAt < statusRight.indexOf("toggle_leaderboard"),
        "exit stays first in the vertical panel");
    assert.ok(settingsAt > exitAt && inboxAt > settingsAt && rankingsAt > inboxAt);
    assert.ok(devAt > rankingsAt && fpsAt > devAt && pingAt > fpsAt,
        "dev tools, FPS, and ping stay after the exit and navigation controls");
    assert.match(statusRight, /class="sow-hud__diagnostics"[\s\S]*sow-hud-dev-btn[\s\S]*sow-hud-fps[\s\S]*sow-hud-ping/,
        "FPS and ping remain separate rows in the shared toolbar panel");
    assert.doesNotMatch(hud, /sow-hud__status-metrics|sow-hud__diagnostics-button/,
        "there is no separate bottom-right diagnostics panel");
    assert.match(hud, /hudDevLayer\.appendChild\(hudRoot\.querySelector\("#sow-hud-dev-sidebar"\)\)/,
        "only the expandable Dev Tools panel is lifted above the quest layer");
    assert.match(hudCss, /#sow-hud-dev-layer \{[^}]*z-index: 220/);
    assert.match(hudCss, /\.sow-hud__diagnostics \{[^}]*flex-direction: column/);
    assert.doesNotMatch(hud, /sow-hud-resource-row|sow-hud-res-rate|sow-hud-res-gold|sow-hud-troop-fill/);
    assert.match(hudCss, /\.sow-hud__status-right \{[^}]*flex-direction: column/);
    assert.match(hudCss, /@media \(min-width: 721px\) \{\s*\.sow-hud__notifications \{[^}]*top: var\(--sow-hud-notifications-top, var\(--sow-hud-top-edge\)\);[^}]*inset-inline-start: var\(--sow-hud-notifications-inline-start, max\(16px, var\(--sow-sal\)\)\);[^}]*transform: none;/);
    const desktopRules = [...hudCss.matchAll(/@media \(min-width: 721px\) \{([^}]*)\}/g)];
    assert.ok(desktopRules.some(([, rules]) => rules.includes("sow-hud__notifications")));
    assert.match(hudCss, /\.sow-hud__notifications\.is-positioned \{[^}]*inset-inline-start: var\(--sow-hud-notifications-inline-start\)/);
    assert.doesNotMatch(hudCss, /\.sow-hud__notifications \{\s*top: auto;\s*bottom: calc\(max\(8px, var\(--sow-sab\)\) \+ var\(--sow-dock-reserve\)\)/);
    assert.match(hud, /new window\.ResizeObserver\(function \(\) \{\s*syncCompactHudBounds\(\);\s*syncNotificationPlacement\(\);\s*\}\)/);
    assert.match(hud, /new window\.MutationObserver\(function \(\)/);
    assert.doesNotMatch(hud.slice(hud.indexOf("    function renderHud("), hud.indexOf("    function handleHudStateUpdate(")), /notificationLayout|scheduleNotificationPlacement|syncNotificationPlacement/);
    assert.match(hud, /var activeHudPanel = null;/);
    assert.match(hud, /function setHudPanel\(panel\)/);
    assert.doesNotMatch(hud, /var (leaderboardOpen|inboxOpen|hudSettingsOpen) = false;/);
    assert.match(hud, /class="sow-hud__panel sow-hud__settings hidden"/);
    assert.match(hud, /class="sow-hud__panel sow-hud__leaderboard hidden"/);
    assert.match(hud, /setHudPanel\("leaderboard"\)/);
    assert.match(hud, /setHudPanel\("inbox"\)/);
    assert.match(hud, /setHudPanel\("settings"\)/);
    assert.match(hudCss, /--sow-hud-toolbar-side-offset: 66px; \/\* 54px toolbar \+ 12px gap \*\//);
    assert.match(hudCss, /\.sow-hud__panel \{[^}]*top: var\(--sow-hud-top-edge\);[^}]*inset-inline-end: calc\(var\(--sow-hud-right-edge\) \+ var\(--sow-hud-toolbar-side-offset\)\)/);
    assert.match(hudCss, /@media \(max-width: 720px\)[^]*?\.sow-hud__panel \{[^}]*top: calc\(max\(8px, var\(--sow-sat\)\) \+ 186px\)/);
    assert.match(hudCss, /#sow-hud\[data-compact="true"\] \.sow-hud__panel \{[^}]*top: calc\(var\(--sow-hud-top-edge\) \+ 50px\)[^}]*bottom: max\(6px, var\(--sow-sab\)\)[^}]*max-height: none/);
    assert.doesNotMatch(hudCss, /\.sow-hud__leaderboard,\s*\.sow-hud__panel \{ top: 50px/);
    assert.doesNotMatch(hudCss, /\.sow-hud__leaderboard \{ top: calc\(max\(12px, var\(--sow-sat\)\) \+ 272px\)/);
    assert.match(campaignView, /objective\.hidden = modal \|\| context\.hideObjective === true/);
});


test("FPS/ping and developer tools settings reach the shared HUD and persist locally", () => {
    assert.match(settingsStateSource, /pub show_fps_ping: bool/);
    assert.match(settingsStateSource, /show_fps_ping: false/);
    assert.match(settingsStateSource, /sow_settings_v1_show_fps_ping/);
    assert.match(settingsStateSource, /sow_settings_v1_show_dev_tools/);
    assert.match(settingsStateSource, /get_item\(Self::SHOW_FPS_PING_KEY\)/);
    assert.match(settingsStateSource, /set_item\(key,/);
    assert.match(uiAppSource, /SettingsState::load_local\(\)/);
    assert.match(webMenu, /SetShowFpsPing\s*\{\s*value: bool/);
    assert.match(webMenu, /WebMenuCommand::SetShowFpsPing[\s\S]*?settings_state\.show_fps_ping = value;[\s\S]*?persist_local\(\)/);
    assert.match(webMenu, /WebMenuCommand::SetShowDevTools[\s\S]*?settings_state\.show_dev_tools = value;[\s\S]*?show_dev_sidebar = value;/);
    assert.match(webMenu, /settings_show_fps_ping: app\.ui\.app\.settings_state\.show_fps_ping/);
    assert.equal((webMenu.match(/"show_fps_ping": app\.ui\.app\.settings_state\.show_fps_ping/g) || []).length, 2);
    assert.equal((webMenu.match(/"show_dev_tools_available": cfg!\(any\(feature = "dev", debug_assertions\)\)/g) || []).length, 2);
    assert.doesNotMatch(webMenu, /"available": app\.ui\.app\.settings_state\.show_dev_tools/);
    assert.match(webMenu, /"dev_tools": dev_tools_payload\(app\)/);
    assert.match(siteDistSource, /fn build_local_preview[\s\S]*?compile_wasm\(paths, true\)/);
    assert.match(siteDistSource, /fn cmd_jest[\s\S]*?compile_wasm\(paths, false\)/);
    assert.match(prodDistSource, /fn build_web[\s\S]*?compile_wasm\(paths, true\)/);
    assert.match(shellSource, /data-setting='show_fps_ping'/);
    assert.match(shellSource, /settings\.show_dev_tools_available \? "<label class='sow-menu__form-field'>"/);
    assert.match(shellSource, /set_show_fps_ping/);
    assert.match(hud, /SOW_t\("hud\.show_fps_ping"\)/);
    assert.match(hud, /data-hud-setting="show_fps_ping"/);
    assert.match(hud, /hudState\.settings && hudState\.settings\.show_dev_tools_available/);
    assert.match(hud, /var showDevTools = settings\.show_dev_tools_available && settings\.show_dev_tools/);
    assert.match(hud, /hudRefs\.devBtn\.classList\.toggle\("hidden", !showDevTools\)/);
    assert.match(hud, /hudRefs\.devSidebar\.classList\.toggle\("hidden", !showDevTools \|\| !devSidebarOpen\)/);
    assert.match(hud, /devSidebarOpen = hudState\.hud\.dev_tools\.open/);
    assert.match(hud, /set_show_fps_ping/);
    assert.match(hud, /if \(input\.checked\) \{\s*setHudPanel\(null\);\s*renderHud\(\);/);
    assert.match(hud, /hudRefs\.fps\.hidden = settings\.show_fps_ping !== true/);
    assert.match(hud, /hudRefs\.ping\.hidden = !hasPing/);
    assert.doesNotMatch(hudCss, /sow-hud__fps\s*\{\s*display:\s*none/);

});


test("Dev Tools expose live shader tuning, extended numeric values, resets, and effect preview", () => {
    const shaderParams = fs.readFileSync(path.join(shell, "../../sow-render/src/shader_params.rs"), "utf8");
    assert.match(shaderParams, /"water\.depth_scale"/);
    assert.match(shaderParams, /"water\.shore_fade_width"/);
    assert.match(shaderParams, /"water\.shore_strength"/);
    assert.match(shaderParams, /"vfx\.alerts"/);
    assert.match(shaderParams, /"border\.contested\.brightness"/);
    assert.match(shaderParams, /"alert\.edge_softness"/);
    assert.match(shaderParams, /"threat\.troop_cap"/);
    assert.match(shaderParams, /"threat\.max_radius"/);
    assert.match(shaderParams, /"territory\.skin\.stripe\.enabled"/);
    assert.match(shaderParams, /"territory\.skin\.grid\.enabled"/);
    assert.match(shaderParams, /"territory\.skin\.diamond\.enabled"/);
    assert.match(shaderParams, /"expansion\.local\.reach_scale"/);
    assert.match(shaderParams, /"expansion\.other\.reach_scale"/);
    assert.match(shaderParams, /"combat\.pvp\.reach_scale"/);
    assert.match(shaderParams, /"mover\.transport\.minimum_px"/);
    assert.match(shaderParams, /"mover\.trade_ship\.minimum_px"/);
    assert.match(shaderParams, /"mover\.warship\.minimum_px"/);
    assert.match(shaderParams, /"mover\.nuke\.minimum_px"/);
    assert.match(shaderParams, /"mover\.sam\.minimum_px"/);
    assert.match(hud, /range\.min = String\(value < rangeMin \? value - margin : rangeMin\)/);
    assert.match(hud, /range\.max = String\(value > rangeMax \? value \+ margin : rangeMax\)/);
    assert.match(hud, /data-dev-min="' \+ def\.min \+ '" data-dev-max="' \+ def\.max/);
    assert.match(hud, /if \(range && !editingControl\)[\s\S]*var min = Number\(range\.dataset\.devMin\)[\s\S]*range\.min = String\(value < min \? value - margin : min\)/);
    assert.match(hud, /Now " \+ format\(current\) \+ " · Default " \+ format\(def\.default\)/);
    assert.match(hud, /data-dev-field="' \+ id/);
    assert.match(hud, /Reset all shader settings/);
    assert.match(hud, /data-dev-preview="phase"/);
    assert.match(hud, /data-dev-preview="target"/);
    assert.match(hud, /hudDevLayer\.addEventListener\("change"[\s\S]*?set_dev_vfx_preview/);
    assert.match(webMenu, /decode_dev_config_value[\s\S]*?if !value\.is_finite\(\)/);
    assert.match(webMenu, /WebMenuCommand::SetDevVfxPreview/);
    assert.match(webMenu, /field: Option<String>/);
    assert.match(appStateSource, /pub struct DevVfxPreview/);
    assert.match(frameSource, /preview_time/);
    assert.match(mapShader, /globals\.preview_effect == 8\.0/);
    assert.match(mapShader, /__MAP_EXPANSION_LOCAL_REACH_SCALE__/);
    assert.match(mapShader, /__MAP_EXPANSION_OTHER_REACH_SCALE__/);
    assert.match(mapShader, /__MAP_COMBAT_PVP_REACH_SCALE__/);
    assert.match(mapShader, /fn ocean_shore_edge\([\s\S]*get_cell_terrain\(get_hex_neighbor\(cell_hex, i\)\)[\s\S]*MAP_WATER_SHORE_STRENGTH/);
});


test("compact HUD density follows the available horizontal frame and restores full size", () => {
    const start = hud.indexOf("    function compactHudDensity(");
    const end = hud.indexOf("\n    function writeHudDensity", start);
    assert.ok(start >= 0 && end > start);
    const compactHudDensity = vm.runInNewContext(hud.slice(start, end) + "\ncompactHudDensity;");
    for (const [width, height, expected] of [
        [568, 320, 0.65],
        [844, 390, 0.65],
        [960, 540, 0.75],
        [1024, 600, 600 / 720],
        [1366, 768, 1],
        [1920, 1080, 1],
        [390, 844, 1]
    ]) {
        assert.ok(Math.abs(compactHudDensity(width, height) - expected) < 0.0001, `${width}x${height}`);
    }
    assert.equal(compactHudDensity(0, 0), 1);
    assert.match(hud, /isEditingHudField && compactHudDensityValue !== null/);
    assert.match(hud, /document\.addEventListener\("focusout"/);
    assert.match(hud, /writeHudDensity\(document\.getElementById\("sow-story"\), density, true\)/);
    assert.doesNotMatch(hudCss, /@media \(orientation: landscape\) and \(max-height: 560px\)/);
    assert.match(hudCss, /min-block-size: 44px/);
    assert.match(hudCss, /min-block-size: 32px/);
    assert.match(hudCss, /font-size: var\(--sow-hud-text-primary-size\)/);
    assert.match(hudCss, /font-size: var\(--sow-hud-text-secondary-size\)/);
    assert.match(hudCss, /--sow-hud-layout-operations-max-height/);
    assert.match(storyCss, /zoom: var\(--sow-hud-hit-target-zoom\)/);
});


test("desktop notification anchor follows the visible quest, then the tutorial plate, then the safe corner", () => {
    const start = hud.indexOf("    function notificationAnchor(");
    const end = hud.indexOf("\n    function scheduleNotificationPlacement", start);
    assert.ok(start >= 0 && end > start);
    const notificationAnchor = vm.runInNewContext(hud.slice(start, end) + "\nnotificationAnchor;");
    const story = { hidden: false, getBoundingClientRect: () => ({ left: 0, top: 0 }) };
    const objectiveRect = { left: 16, right: 296, top: 100, bottom: 190, width: 280, height: 90 };
    const objective = { hidden: false, getBoundingClientRect: () => objectiveRect };
    const nameplate = { hidden: false, getBoundingClientRect: () => ({ left: 16, right: 296, top: 12, bottom: 84, width: 280, height: 72 }) };
    const serialize = value => JSON.parse(JSON.stringify(value));

    assert.deepEqual(serialize(notificationAnchor(story, objective, nameplate, 12)), {
        source: "objective", left: 16, right: 296, top: 202
    });
    objectiveRect.height = 140;
    objectiveRect.bottom = 240;
    assert.equal(notificationAnchor(story, objective, nameplate, 12).top, 252, "a resized quest moves the stack below its new bottom");
    assert.deepEqual(
        serialize(notificationAnchor(story, objective, nameplate, 12)),
        serialize(notificationAnchor(story, objective, nameplate, 12)),
        "repeated layout observations do not accumulate position changes"
    );
    objective.hidden = true;
    assert.deepEqual(serialize(notificationAnchor(story, objective, nameplate, 12)), {
        source: "nameplate", left: 16, right: 296, top: 96
    });
    nameplate.hidden = true;
    assert.deepEqual(serialize(notificationAnchor(story, objective, nameplate, 12)), {
        source: "safe", left: null, right: null, top: null
    });
    story.hidden = true;
    nameplate.hidden = false;
    assert.equal(notificationAnchor(story, objective, nameplate, 12).source, "nameplate");
    story.hidden = false;
    objective.hidden = false;
    assert.equal(notificationAnchor(story, objective, nameplate, 12).top, 252, "the quest reappearance restores the correct anchor");

    assert.match(hudCss, /\.sow-hud__notifications \{\s*position: absolute;\s*top: var\(--sow-hud-notifications-top, 72px\);\s*left: 50%;\s*display: grid;\s*gap: 6px;/);
    const placementStart = hud.indexOf("    function syncNotificationPlacement(");
    const placementEnd = hud.indexOf("\n    function ensureHudDom", placementStart);
    assert.ok(placementStart >= 0 && placementEnd > placementStart);
    assert.match(hud.slice(placementStart, placementEnd), /notificationAnchor\(story, objective, hudRefs\.nameplate, 12\)/);
    assert.match(hud.slice(placementStart, placementEnd), /notifications\.style\.setProperty\("--sow-hud-notifications-top"/);
    assert.match(hud.slice(placementStart, placementEnd), /notifications\.style\.setProperty\("--sow-hud-notifications-inline-start"/);
    assert.doesNotMatch(hud.slice(placementStart, placementEnd), /if\s*\(\s*window\.innerWidth\s*<=\s*720\s*\)\s*\{[^}]*return;/);
    assert.match(hudCss, /#sow-hud\[data-compact="true"\] \.sow-hud__notifications \{ top: var\(--sow-hud-notifications-top, var\(--sow-hud-layout-top-reserve, 72px\)\); \}/,
        "compact landscape keeps the nameplate or quest anchor computed by the HUD");
    assert.match(hudCss, /#sow-hud\[data-compact="true"\] \.sow-hud__notifications\.is-positioned \{[^}]*inset-inline-start: var\(--sow-hud-notifications-inline-start\); inset-inline-end: auto;/,
        "compact CSS cannot override the dynamic horizontal anchor");
});


test("combat operations reuse snapshots and intents across desktop and mobile HUD", () => {
    assert.doesNotMatch(hudStateSource, /^\s*(attacks|fleets):\s*Vec</m, "the HUD state does not retain unused attack or fleet vectors");
    assert.match(webMenu, /let operations = snapshot[\s\S]*build_combat_operations_payload\(/);
    assert.match(webMenu, /"operations": operations/);
    assert.match(webMenu, /combat_operations_snapshot_tick\(snapshot, my_pid, app\.sim\.engine\.as_ref\(\)\)/);
    assert.match(webMenu, /combat_operations_tick,/);
    assert.match(webMenu, /fn combat_operations_snapshot_tick\(/);
    assert.match(webMenu, /unit_type == sow_core::game::UnitType::TransportShip/);
    assert.match(webMenu, /WebMenuCommand::CounterAttack \{ target_player_id \}/);
    assert.match(webMenu, /fn incoming_combat_troops\(/);
    assert.match(webMenu, /available_troops\.max\(0\.0\) \* f64::from\(ratio\.clamp\(0\.05, 1\.0\)\)/);
    assert.match(webMenu, /attack_ratio/);
    assert.match(intentApplySource, /wf\.unit_type != crate::game::UnitType::TransportShip/);
    assert.match(hud, /function renderCombatOperations\(operations\)/);
    assert.match(hud, /renderAvatarIdentity\([\s\S]*operation\.avatar[\s\S]*kind: "fallback"/);
    assert.match(hud, /function renderAvatarIdentity\(image, emblem, presentation\)/);
    assert.match(hud, /aria-label="' \+ SOW_t\("hud\.combat_operations"\)/);
    assert.doesNotMatch(hud, /sow-hud__operations-header|retreat_cost|operation-penalty/);
    assert.match(hud, /var canCounter = incoming && !retreating/);
    assert.match(hud, /pendingCounterAttacks\[playerId\]/);
    assert.match(hud, /send\("counter_attack", \{ target_player_id: counterTarget \}\)/);
    assert.match(hud, /send\("focus_world", \{ x: focusX, y: focusY \}\)/);
    assert.match(hudCss, /\.sow-hud__operations \{[^}]*bottom: var\(--sow-hud-layout-operations-bottom, calc\(max\(14px, var\(--sow-sab\)\) \+ var\(--sow-dock-reserve\)\)\)/);
    assert.match(hudCss, /\.sow-hud__operation-action \{[^}]*width: 44px; height: 44px/);
    assert.match(hudCss, /@media \(max-width: 599px\) and \(orientation: portrait\)[\s\S]*?\.sow-hud__operations \{[^}]*width: min\(560px, calc\(100vw - 124px\)\)/);
    assert.match(hudCss, /\.sow-hud__dock-stack \{[^}]*flex-direction: column-reverse/);
    assert.match(hudCss, /#sow-hud\[data-compact="true"\] \.sow-hud__operations \{[^}]*max-height: var\(--sow-hud-layout-operations-max-height, 20dvh\)/);
    assert.match(hudCss, /@media \(orientation: portrait\)\s*\{\s*#sow-hud\[data-compact="true"\] \.sow-hud__operations \{\s*width: min\(560px, calc\(100vw - 124px\)\)/);
    assert.match(hudCss, /\.sow-hud__operation-emblem \{ font-size: 18px; line-height: 1; \}/);
    assert.equal((hudCss.match(/#sow-hud\[data-overlay-open="true"\] \.sow-hud__operations\s*\{/g) || []).length, 1);
    assert.match(hudCss, /@media \(hover: none\) and \(pointer: coarse\) \{\s*#sow-hud\[data-overlay-open="true"\] \.sow-hud__operations \{ display: none; \}/);
    assert.match(hudCss, /\.sow-hud__operations\[hidden\] \{ display: none !important; \}/);

});


test("combat operations survive HUD and locale rebuilds, reuse rows, and reset on match exit", () => {
    class Element {
        constructor(tagName) {
            this.tagName = tagName;
            this.children = [];
            this.parentNode = null;
            this.dataset = {};
            this.attributes = {};
            this.classList = { toggle() {}, add() {} };
            this.hidden = false;
            this.textContent = "";
            this.replacements = 0;
        }
        append(...items) { items.forEach(item => this.appendChild(item)); }
        appendChild(item) {
            if (item.tagName === "#fragment") {
                item.children.slice().forEach(child => this.appendChild(child));
                return item;
            }
            if (item.parentNode) item.parentNode.children.splice(item.parentNode.children.indexOf(item), 1);
            item.parentNode = this;
            this.children.push(item);
            return item;
        }
        replaceChildren(...items) {
            this.children.forEach(item => { item.parentNode = null; });
            this.children = [];
            this.replacements += 1;
            this.append(...items);
        }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) { return name === "src" ? this.src || null : this.attributes[name] || null; }
        removeAttribute(name) { delete this.attributes[name]; if (name === "src") this.src = ""; }
        querySelector() { return null; }
        querySelectorAll(selector) {
            const match = selector.match(/^\[data-command="([^"]+)"\]$/);
            if (!match) return [];
            const command = match[1];
            return this.children.flatMap(child => [child, ...child.querySelectorAll(selector)])
                .filter(child => child.dataset.command === command);
        }
        closest(selector) { return selector === "[data-command]" && this.dataset.command ? this : null; }
    }
    const calls = [];
    const listeners = {};
    let locale = "en";
    let elements = new Map();
    const context = vm.createContext({
        hudRoot: new Element("root"), hudDevLayer: null,
        document: {
            createElement: tag => new Element(tag),
            createDocumentFragment: () => new Element("#fragment"),
            getElementById: id => elements.get(id) || null,
            querySelector: () => null
        },
        window: {
            addEventListener: (type, listener) => { listeners[type] = listener; },
            SOW_menu_command: raw => calls.push(JSON.parse(raw))
        },
        asset: name => "/assets/" + name,
        hudIcon: name => name,
        formatNameplateTroops: String,
        SOW_t: key => locale + ":" + key,
        clearCompactHudDensity() {}, clearMapFeedback() {}, clearNotificationCard() {}
    });
    vm.runInContext(hud.slice(hud.indexOf("    var hudState ="), hud.indexOf("    function send(")), context);
    const refsStart = hud.indexOf("        hudRefs = {");
    const refsEnd = hud.indexOf("\n        };", refsStart) + "\n        };".length;
    assert.ok(refsStart >= 0 && refsEnd > refsStart);
    const refsScript = new vm.Script(hud.slice(refsStart, refsEnd));
    const rendererStart = hud.indexOf("    function createCombatOperationRow(");
    const renderHudStart = hud.indexOf("\n    function renderHud(", rendererStart);
    const avatarStart = hud.indexOf("    function bindAvatarFallback(");
    const avatarEnd = hud.indexOf("\n    function clearNotificationCard(", avatarStart);
    const clickStart = hud.indexOf("        function handleHudClick(event) {");
    const clickEnd = hud.indexOf('        hudRoot.addEventListener("click", handleHudClick)', clickStart);
    vm.runInContext(
        hud.slice(hud.indexOf("    function send("), hud.indexOf("    function setHudPanel("))
        + hud.slice(avatarStart, avatarEnd)
        + hud.slice(rendererStart, renderHudStart)
        + hud.slice(clickStart, clickEnd), context
    );
    const inactiveEnd = hud.indexOf("        ensureHudDom();", renderHudStart);
    vm.runInContext(hud.slice(renderHudStart, inactiveEnd) + "\n    }\nthis.hideHud = renderHud;", context);
    context.renderHud = () => {
        if (!context.hudRefs) {
            elements = new Map([
                ["sow-hud-operations", new Element("aside")],
                ["sow-hud-operations-list", new Element("list")]
            ]);
            refsScript.runInContext(context);
        }
        context.hudRoot.hidden = false;
        context.renderCombatOperations(context.hudState.hud.operations);
    };
    const localeStart = hud.indexOf('    window.addEventListener("sow:locale-change", function () {');
    const localeEnd = hud.indexOf("\n    });", localeStart) + "\n    });".length;
    vm.runInContext(hud.slice(localeStart, localeEnd), context);
    const operations = [
        { kind: "attack", id: 7, direction: "incoming", player_id: 2, name: "Caesar", troops: 100, avatar: { kind: "portrait", slug: "caesar" }, focus_x: 3, focus_y: 4 },
        { kind: "attack", id: 8, direction: "outgoing", player_id: 0, neutral: true, troops: 200, avatar: { kind: "emblem", symbol: "🌱" } },
        { kind: "fleet", id: 9, direction: "outgoing", player_id: 3, name: "Ally", troops: 50, avatar: { kind: "emblem", symbol: "🛡️" } }
    ];
    const click = button => context.handleHudClick({ target: button });
    const counters = () => calls.filter(call => call.type === "counter_attack");
    context.hudState = { phase: "Playing", hud: { operations } };
    context.renderHud();
    const firstList = context.hudRefs.operationRows;
    const firstRows = firstList.children.slice();
    const counter = firstRows[0].querySelectorAll('[data-command="counter_attack"]')[0];
    assert.equal(firstRows.length, 3);
    assert.equal(context.hudRefs.operations.hidden, false);
    assert.equal(firstRows[0].children[0].children[0].src, "/assets/gameplay/avatars/caesar.webp");
    assert.equal(firstRows[1].children[0].children[1].textContent, "🌱");
    assert.equal(firstRows[2].querySelectorAll('[data-command="recall_fleet"]').length, 1);
    context.renderHud();
    assert.equal(firstList.replacements, 1, "unchanged operations do not remount rows");
    firstRows.forEach((row, index) => assert.equal(firstList.children[index], row));
    operations[0].troops = 120;
    operations[0].focus_x = 5;
    context.renderHud();
    assert.equal(firstRows[0].querySelectorAll('[data-command="counter_attack"]')[0], counter);
    assert.equal(counter.dataset.incomingTroops, "120");
    assert.equal(firstRows[0].children[2].textContent, "120");
    click(firstRows[0].querySelectorAll('[data-command="focus_world"]')[0]);
    assert.deepEqual(calls[0], { type: "focus_world", x: 5, y: 4 });
    click(counter);
    click(counter);
    assert.equal(counters().length, 1);

    locale = "es";
    listeners["sow:locale-change"]();
    const rebuiltList = context.hudRefs.operationRows;
    assert.notEqual(rebuiltList, firstList);
    assert.equal(rebuiltList.children.length, 3, "identical operation IDs populate the new HUD");
    rebuiltList.children.forEach((row, index) => {
        assert.notEqual(row, firstRows[index]);
        assert.equal(row.parentNode, rebuiltList);
    });
    assert.equal(rebuiltList.children[1].children[1].children[0].textContent, "es:hud.land");
    const rebuiltCounter = rebuiltList.children[0].querySelectorAll('[data-command="counter_attack"]')[0];
    assert.equal(rebuiltCounter.getAttribute("aria-label"), "es:hud.map_action_attack");
    assert.equal(rebuiltCounter.disabled, true, "a rebuild preserves pending commands");
    click(rebuiltCounter);
    assert.equal(counters().length, 1, "locale rebuild cannot resend a pending counterattack");

    context.renderCombatOperations(operations.slice(1));
    assert.equal(rebuiltList.children.length, 2);
    assert.equal(rebuiltList.children[0].children[1].children[0].textContent, "es:hud.land");
    assert.equal(Object.keys(context.pendingCounterAttacks).length, 0);
    context.renderCombatOperations([...operations, { kind: "fleet", id: 10, direction: "incoming", player_id: 4, name: "Enemy", troops: 30 }]);
    assert.equal(rebuiltList.children.length, 4);
    context.renderCombatOperations([]);
    assert.equal(rebuiltList.children.length, 0);
    assert.equal(context.hudRefs.operations.hidden, true);

    context.renderHud();
    click(context.hudRefs.operationRows.children[0].querySelectorAll('[data-command="counter_attack"]')[0]);
    assert.equal(counters().length, 2);
    context.hudState = { phase: "MainMenu" };
    context.hideHud();
    assert.equal(context.hudRoot.hidden, true);
    assert.equal(context.hudRefs.operationRows.children.length, 0);
    assert.equal(context.hudRefs.operations.hidden, true);
    assert.equal(Object.keys(context.pendingCounterAttacks).length, 0);
    context.hudState = { phase: "Playing", hud: { operations } };
    context.renderHud();
    assert.equal(context.hudRefs.operationRows.children.length, 3);
    assert.equal(context.hudRefs.operationRows.children[0].querySelectorAll('[data-command="counter_attack"]')[0].disabled, false);
    assert.equal(counters().length, 2, "rendering never sends gameplay commands");
});


test("all HUD message classes have one explicit presentation", () => {
    const local = [
        "action_unavailable", "attack_out_of_range", "fleet_need_troops", "fleet_invalid_target",
        "fleet_own_target", "fleet_teammate", "fleet_alliance", "fleet_capacity_full",
        "fleet_no_water_access", "fleet_no_landing_shore", "fleet_no_water_path", "need_gold",
        "build_owned_land", "build_land", "build_outside_map", "build_occupied", "build_coast_required", "build_lowland_required",
        "build_no_space", "building_in_progress", "spawn_too_close", "spawn_rate_limited",
        "alliance_request_pending", "alliance_renewal_pending", "resources_allies_only"
    ];
    const global = [
        "elimination", "attack_incoming",
        "alliance_request", "alliance_request_sent", "resource_request", "alliance_formed", "alliance_renewed",
        "betrayal", "alliance_ended", "nuke_struck", "resource_received_both",
        "resource_received_gold", "resource_received_troops", "resource_request_declined",
        "resource_sent_both", "resource_sent_gold", "resource_sent_troops", "resource_send_failed"
    ];
    const silent = [
        "attack_launched", "observer_feedback", "water_feedback", "transport_landed",
        "structure_started", "structure_ready", "structure_upgraded",
        "wilderness_expanded", "enemy_territory_captured", "resource_request_sent"
    ];
    const all = [...local, ...global, ...silent];
    assert.equal(local.length, 25);
    assert.equal(global.length, 18);
    assert.equal(silent.length, 10);
    assert.equal(new Set(all).size, all.length);
    const emitters = [mapClick, simEventsSource, eliminationSource, snapshotFxSource, simUpdateSource].join("\n");
    const placementRules = fs.readFileSync(path.join(shell, "../../sow-core/src/building/placement.rs"), "utf8");
    const localEmitters = [mapClick, intentInput, placementRules].join("\n");
    for (const key of local) assert.ok(localEmitters.includes(`hud.${key}`), `missing local feedback: ${key}`);
    for (const key of global) assert.ok(emitters.includes(`hud.${key}`), `missing global message: ${key}`);
    for (const key of silent) assert.ok(!emitters.includes(`hud.${key}`), `silent message still emitted: ${key}`);
    assert.ok(!emitters.includes("hud.elimination_bounty"), "gold bounty uses the staged Rust world effect, not a duplicate HUD card");
    assert.ok(!emitters.includes("hud.elimination_assist"), "assist gold uses the staged Rust world effect, not a duplicate HUD card");
    assert.ok(eliminationSource.includes("DeathNameplateReward"));
    assert.ok(eliminationSource.includes('"+{} Gold"'));
    assert.ok(eliminationSource.includes('"+{} Gold (Assist)"'));

});


test("HUD notifications coalesce, prioritize, reuse both entity avatars, and expire within fixed bounds", () => {
    const start = hud.indexOf("    function createNotificationCard(container, contextual) {");
    const end = hud.indexOf("\n    function renderHud(", start);
    assert.ok(start >= 0 && end > start);
    const avatarSlot = () => ({
        wrapper: { hidden: true, className: "" },
        image: {
            hidden: true,
            src: "",
            getAttribute(name) { return name === "src" ? this.src : null; },
            removeAttribute(name) { if (name === "src") this.src = ""; }
        },
        emblem: { hidden: true, textContent: "" }
    });
    const cards = Array.from({ length: 3 }, () => ({
        card: { hidden: true, className: "" },
        seal: { innerHTML: "" },
        avatars: [avatarSlot(), avatarSlot()],
        copy: { textContent: "" },
        renderKey: ""
    }));
    let now = 1000;
    const timers = [];
    const context = {
        hudRefs: { notifications: {} },
        hudRoot: { dataset: {} },
        notificationCursor: 0,
        notificationTimer: null,
        activeNotifications: [],
        notificationCards: cards,
        Date: { now: () => now },
        asset: path => "/assets/" + path,
        hudIcon: name => name,
        SOW_t: (key, values = {}) => key === "hud.alliance_request_sent"
            ? "You sent an alliance request to " + values.name
            : key,
        window: {
            SOWCampaignView: vm.runInNewContext(campaignView + "\nSOWCampaignView;"),
            SOW_LOCALE: "en",
            innerWidth: 1366,
            innerHeight: 768,
            clearTimeout(timer) { timer.cleared = true; },
            setTimeout(callback, delay) {
                const timer = { callback, delay, cleared: false };
                timers.push(timer);
                return timer;
            }
        }
    };
    const renderer = hud.slice(start, end) + "\nthis.renderNotifications = renderNotifications;";
    vm.runInNewContext(renderer, context);
    const add = (id, key, priority, group, values, avatars, sumValues = false) => {
        now += 100;
        context.renderNotifications([{
            id, key, priority, group, values: values || {}, avatars: avatars || [],
            sum_values: sumValues, age_ms: 0
        }]);
    };

    add(1, "hud.resource_received_gold", 3, "resource:received:7", { gold: "100" }, [
        { kind: "portrait", slug: "boudica" }, { kind: "portrait", slug: "caesar" }
    ], true);
    const firstTimerCount = timers.length;
    context.renderNotifications([], false);
    assert.equal(timers.length, firstTimerCount, "unchanged HUD updates must not reset the expiry timer");
    add(2, "hud.resource_received_troops", 3, "resource:received:7", { troops: "25" }, [
        { kind: "portrait", slug: "boudica" }, { kind: "portrait", slug: "caesar" }
    ], true);
    assert.equal(context.activeNotifications.length, 1);
    assert.equal(context.activeNotifications[0].entry.key, "hud.resource_received_both");
    assert.equal(context.activeNotifications[0].entry.values.gold, "100");
    assert.equal(context.activeNotifications[0].entry.values.troops, "25");
    assert.deepEqual(cards[0].avatars.map(item => item.wrapper.hidden), [false, false]);
    assert.deepEqual(cards[0].avatars.map(item => item.image.src), [
        "/assets/gameplay/avatars/boudica.webp", "/assets/gameplay/avatars/caesar.webp"
    ]);
    assert.deepEqual(cards[0].avatars.map(item => item.emblem.hidden), [true, true]);
    assert.match(cards[0].avatars[1].wrapper.className, /--overlap/);

    const portraitPair = [
        { kind: "portrait", slug: "caesar" }, { kind: "portrait", slug: "boudica" }
    ];
    add(3, "hud.nuke_struck", 4, "nuke:9:7", {}, portraitPair);
    add(4, "hud.structure_ready", 2, "building:7", {});
    add(5, "hud.resource_request_declined", 1, "resource-rejected:7", {});
    assert.equal(context.activeNotifications.length, 3, "a burst must never create a backlog");
    assert.ok(context.activeNotifications.some(item => item.group === "nuke:9:7"));
    assert.ok(context.activeNotifications.some(item => item.group === "resource:received:7"));
    assert.ok(!context.activeNotifications.some(item => item.group === "resource-rejected:7"), "lower-priority feedback must yield to active event cards");
    const timersBeforeAttack = timers.length;
    add(6, "hud.attack_incoming", 4, "incoming-attack:7", { count: "2" }, portraitPair);
    add(7, "hud.attack_incoming", 4, "incoming-attack:7", { count: "3" }, portraitPair);
    assert.equal(context.activeNotifications.length, 3);
    assert.equal(context.activeNotifications.find(item => item.group === "incoming-attack:7").entry.values.count, "3");
    const timersBeforeDuplicate = timers.length;
    context.renderNotifications([{
        id: 7, key: "hud.attack_incoming", priority: 4, group: "incoming-attack:7",
        values: { count: "3" }, avatars: portraitPair, age_ms: 0
    }]);
    assert.equal(timers.length, timersBeforeDuplicate, "duplicate IDs must not refresh or duplicate cards");
    assert.ok(timersBeforeAttack < timersBeforeDuplicate);

    const burst = Array.from({ length: 100 }, (_, index) => ({
        id: index + 8, key: "hud.resource_request_declined", priority: 1, group: "resource-rejected:" + index,
        values: {}, avatars: [{ kind: "portrait", slug: "boudica" }], age_ms: 0
    }));
    const timersBeforeBurst = timers.length;
    context.renderNotifications(burst);
    assert.equal(context.activeNotifications.length, 3);
    assert.equal(timers.length, timersBeforeBurst, "a low-priority spam burst must not churn timers");

    now += 5000;
    timers.at(-1).callback();
    assert.equal(context.activeNotifications.length, 0);
    assert.ok(cards.every(parts => parts.card.hidden), "expired cards release their visible slots");

    add(108, "hud.attack_incoming", 4, "tribe-attack", {}, [
        { kind: "emblem", symbol: "🐺" }, { kind: "emblem", symbol: "🦅" }
    ]);
    const emblemCard = cards.find(parts => !parts.card.hidden);
    assert.deepEqual(emblemCard.avatars.map(item => item.emblem.textContent), ["🐺", "🦅"]);
    assert.deepEqual(emblemCard.avatars.map(item => item.image.hidden), [true, true]);
    assert.deepEqual(emblemCard.avatars.map(item => item.image.src), ["", ""]);
    assert.match(emblemCard.avatars[1].wrapper.className, /--overlap/);

    add(109, "hud.nuke_struck", 4, "single-avatar", {}, [
        null, { kind: "emblem", symbol: "🏛️" }
    ]);
    const singleAvatarCard = cards.find(parts => parts.copy.textContent === "hud.nuke_struck");
    assert.deepEqual(singleAvatarCard.avatars.map(item => item.wrapper.hidden), [true, false]);
    assert.doesNotMatch(singleAvatarCard.avatars[1].wrapper.className, /--overlap/);
    assert.equal(singleAvatarCard.avatars[0].emblem.textContent, "");

    add(110, "hud.resource_received_gold", 4, "missing-avatar", {}, [
        { kind: "fallback" }, { kind: "future-avatar-kind" }
    ]);
    const fallbackCard = cards.find(parts => parts.copy.textContent === "hud.resource_received_gold");
    assert.deepEqual(fallbackCard.avatars.map(item => item.image.src), [
        "/assets/gameplay/avatars/null.webp", "/assets/gameplay/avatars/null.webp"
    ]);

    now += 5000;
    timers.at(-1).callback();
    assert.equal(context.activeNotifications.length, 0, "the alliance confirmation should be tested without unrelated cards occupying the HUD");
    add(111, "hud.alliance_request_sent", 3, "alliance-request-sent:1:2", { name: "Caesar" }, [
        { kind: "portrait", slug: "boudica" }, { kind: "portrait", slug: "caesar" }
    ]);
    const allianceRequestCard = cards.find(parts => parts.copy.textContent === "You sent an alliance request to Caesar");
    assert.ok(allianceRequestCard, "the outgoing alliance message must include the player's name");
    assert.deepEqual(allianceRequestCard.avatars.map(item => item.image.src), [
        "/assets/gameplay/avatars/boudica.webp", "/assets/gameplay/avatars/caesar.webp"
    ]);

    add(112, "hud.nuke_struck", 4, "nuke:9:7", {}, portraitPair);
    add(113, "hud.attack_incoming", 4, "incoming-attack:7", { count: "2" }, portraitPair);
    assert.equal(cards.filter(parts => !parts.card.hidden).length, 3, "regular desktop keeps its three cards");
    context.window.innerWidth = 390;
    context.window.innerHeight = 844;
    context.renderNotifications([], false);
    assert.equal(context.activeNotifications.length, 1, "portrait keeps one active notification, not a hidden backlog");
    assert.equal(context.activeNotifications[0].entry.id, 113, "rotation keeps the newest highest-priority event");
    assert.equal(cards.filter(parts => !parts.card.hidden).length, 1);
    const recycledCard = cards[0];
    const portraitTimerCount = timers.length;
    add(114, "hud.alliance_request_sent", 3, "alliance:8", { name: "Caesar" });
    assert.equal(context.activeNotifications[0].entry.id, 113, "a routine event cannot erase an urgent attack");
    assert.equal(timers.length, portraitTimerCount, "ignored events cannot extend the alert");
    add(115, "hud.betrayal", 4, "betrayal:8", {}, [null, { kind: "emblem", symbol: "🐺" }]);
    assert.equal(context.activeNotifications[0].entry.id, 115);
    assert.strictEqual(cards.find(parts => !parts.card.hidden), recycledCard, "the next event recycles the same DOM card");
    assert.deepEqual(recycledCard.avatars.map(item => item.wrapper.hidden), [true, false]);
    assert.equal(recycledCard.avatars[1].emblem.textContent, "🐺");

    now += 5000;
    timers.at(-1).callback();
    assert.equal(context.hudRoot.dataset.notificationsVisible, "false");
    assert.ok(cards.every(parts => parts.card.hidden));
    add(116, "hud.resource_received_gold", 3, "resource:received:7", { gold: "100" }, portraitPair, true);
    add(117, "hud.resource_received_troops", 3, "resource:received:7", { troops: "25" }, portraitPair, true);
    assert.equal(context.activeNotifications.length, 1);
    assert.equal(context.activeNotifications[0].entry.key, "hud.resource_received_both");
    assert.deepEqual(JSON.parse(JSON.stringify(context.activeNotifications[0].entry.values)), { gold: "100", troops: "25" });
    assert.equal(context.hudRoot.dataset.notificationsVisible, "true");

    context.window.innerWidth = 1366;
    context.window.innerHeight = 768;
    context.renderNotifications([], false);
    assert.equal(context.activeNotifications.length, 1, "returning to desktop never replays discarded notifications");
    add(118, "hud.nuke_struck", 4, "nuke:9:7", {}, portraitPair);
    add(119, "hud.attack_incoming", 4, "incoming-attack:7", { count: "2" }, portraitPair);
    assert.equal(cards.filter(parts => !parts.card.hidden).length, 3);
    context.window.innerWidth = 844;
    context.window.innerHeight = 390;
    context.hudRoot.dataset.compact = "true";
    context.renderNotifications([], false);
    assert.equal(context.activeNotifications.length, 1, "compact landscape has the same single-card policy");
    assert.equal(context.activeNotifications[0].entry.id, 119);
    add(120, "hud.betrayal", 4, "betrayal:8", {}, portraitPair);
    assert.strictEqual(cards.find(parts => !parts.card.hidden), recycledCard);
    assert.equal(context.activeNotifications[0].entry.id, 120);
    const compactTimerCount = timers.length;
    context.renderNotifications([context.activeNotifications[0].entry]);
    assert.equal(timers.length, compactTimerCount, "a repeated state cannot restart the recycled card");
    now += 5000;
    timers.at(-1).callback();
    assert.ok(cards.every(parts => parts.card.hidden));
});


test("notification cards render typed avatars, recover failed art, and clear reused slots", () => {
    const start = hud.indexOf("    function createNotificationCard(container, contextual) {");
    const end = hud.indexOf("\n    function renderHud(", start);
    assert.ok(start >= 0 && end > start);
    class Element {
        constructor(tagName) {
            this.tagName = tagName;
            this.children = [];
            this.attributes = Object.create(null);
            this.hidden = false;
            this.className = "";
            this.src = "";
        }
        append(...items) { this.children.push(...items); }
        appendChild(item) { this.children.push(item); }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) {
            if (name === "src") return this.src || null;
            return this.attributes[name] || null;
        }
        removeAttribute(name) {
            delete this.attributes[name];
            if (name === "src") this.src = "";
        }
    }
    const container = new Element("container");
    const context = {
        document: { createElement: tagName => new Element(tagName) },
        asset: path => "/assets/" + path,
        hudIcon: name => name,
        SOW_t: key => key
    };
    const renderer = hud.slice(start, end)
        + "\nthis.createNotificationCard = createNotificationCard;"
        + "\nthis.renderNotificationCard = renderNotificationCard;"
        + "\nthis.clearNotificationCard = clearNotificationCard;"
        + "\nthis.renderAvatarIdentity = renderAvatarIdentity;";
    vm.runInNewContext(renderer, context);

    const parts = context.createNotificationCard(container, false);
    context.renderNotificationCard(parts, {
        entry: {
            id: 1, key: "hud.attack_incoming", values: {},
            avatars: [
                { kind: "portrait", slug: "boudica" },
                { kind: "emblem", symbol: "🐺" }
            ]
        },
        priority: 4
    }, false);
    assert.equal(container.children.length, 1);
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/boudica.webp");
    assert.equal(parts.avatars[0].emblem.hidden, true);
    assert.equal(parts.avatars[1].image.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "🐺");
    assert.match(parts.avatars[1].wrapper.className, /--overlap/);

    parts.avatars[0].image.onerror.call(parts.avatars[0].image);
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/null.webp");

    context.renderAvatarIdentity(parts.avatars[0].image, parts.avatars[0].emblem, { kind: "fallback" });
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/null.webp");
    assert.equal(parts.avatars[0].image.hidden, false);

    context.renderNotificationCard(parts, {
        entry: { id: 2, key: "hud.alliance_formed", values: {}, avatars: [{ kind: "emblem", symbol: "🏛️" }] },
        priority: 2
    }, false);
    assert.equal(parts.avatars[0].image.src, "");
    assert.equal(parts.avatars[0].emblem.textContent, "🏛️");
    assert.equal(parts.avatars[1].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "");

    context.renderNotificationCard(parts, {
        entry: { id: 3, key: "hud.nuke_struck", values: {}, avatars: [null, { kind: "emblem", symbol: "🏺" }] },
        priority: 4
    }, false);
    assert.equal(parts.avatars[0].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "🏺");
    assert.doesNotMatch(parts.avatars[1].wrapper.className, /--overlap/);

    context.clearNotificationCard(parts);
    assert.equal(parts.avatars[0].wrapper.hidden, true);
    assert.equal(parts.avatars[1].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "");
});


test("inbox reuses identities and live buttons, resolves focus, and survives locale rebuilds", () => {
    const { doc, Element } = makeHudDocument();
    const makeRefs = () => {
        const refs = {
            inbox: new Element("aside"), inboxRows: new Element("list"), inboxEmpty: new Element("empty"),
            inboxTotal: new Element("total"), inboxClose: new Element("button"), inboxButton: new Element("button"),
            inboxRequests: Object.create(null), inboxRequestKeys: [], inboxRenderKey: ""
        };
        refs.inbox.classList.add("hidden");
        refs.inbox.append(refs.inboxClose, refs.inboxRows, refs.inboxEmpty);
        return refs;
    };
    const calls = [], listeners = {};
    const context = vm.createContext({
        document: doc, hudRefs: makeRefs(), hudRoot: new Element("root"), hudDevLayer: null,
        hudState: { phase: "Playing" }, emoteTextKey: "", hudInitialized: true,
        window: {
            SOW_LOCALE: "en",
            SOW_menu_command: raw => calls.push(JSON.parse(raw)),
            addEventListener: (name, listener) => { listeners[name] = listener; }
        },
        asset: name => "/assets/" + name,
        currencyAsset: name => "/assets/gameplay/currency/" + name + ".webp",
        hudIcon: name => name,
        SOW_t: key => context.window.SOW_LOCALE + ":" + key
    });
    const avatarStart = hud.indexOf("    function bindAvatarFallback(");
    const avatarEnd = hud.indexOf("    function clearNotificationCard(", avatarStart);
    const inboxStart = hud.indexOf("    function createPlayerIdentity(");
    const inboxEnd = hud.indexOf("    function renderNotifications(", inboxStart);
    const clickStart = hud.indexOf("        function handleHudClick(event) {");
    const clickEnd = hud.indexOf('        hudRoot.addEventListener("click", handleHudClick)', clickStart);
    for (const [start, end] of [[avatarStart, avatarEnd], [inboxStart, inboxEnd], [clickStart, clickEnd]]) {
        assert.ok(start >= 0 && end > start);
    }
    vm.runInContext(
        hud.slice(avatarStart, avatarEnd) + hud.slice(inboxStart, inboxEnd)
        + hud.slice(hud.indexOf("    function send("), hud.indexOf("    function updateHudSafeArea("))
        + hud.slice(clickStart, clickEnd), context
    );
    const alliance = { kind: "alliance", requester_id: 2, name: "Caesar", active: true, avatar: { kind: "portrait", slug: "caesar" } };
    const resources = { kind: "resources", requester_id: 3, name: "Trinovantes", active: true, gold: 1234.9, troops: 56.8, avatar: { kind: "emblem", symbol: "🐺" } };
    const campaign = { kind: "resources", requester_id: 4, name: "<img onerror=alert(1)>", active: true, gold: 5, troops: 0, avatar: { kind: "portrait", slug: "boudica" } };
    const eliminated = { kind: "resources", requester_id: 5, name: "Defeated", active: false, gold: 10, troops: 10 };
    let requests = [alliance, resources, campaign, eliminated];
    context.renderInbox(requests);
    const refs = context.hudRefs, cards = refs.inboxRequests;
    assert.equal(refs.inboxRows.children.length, 4);
    assert.equal(cards["alliance:2"].portrait.src, "/assets/gameplay/avatars/caesar.webp");
    assert.equal(cards["resources:3"].emblem.textContent, "🐺");
    assert.equal(cards["resources:3"].portrait.hidden, true);
    assert.equal(cards["resources:3"].amounts.gold.value.textContent, "1,234");
    assert.equal(cards["resources:3"].amounts.troops.value.textContent, "56");
    assert.equal(cards["resources:4"].name.textContent, campaign.name);
    assert.equal(cards["resources:4"].name.children.length, 0, "player names are text, never HTML");
    assert.equal(cards["resources:4"].amounts.troops.element.hidden, true);
    assert.equal(cards["resources:5"].portrait.src, "/assets/gameplay/avatars/null.webp");
    assert.equal(cards["resources:5"].accept.disabled, true);
    assert.equal(cards["resources:5"].reject.disabled, false, "obsolete requests can still be dismissed");
    cards["alliance:2"].portrait.onerror.call(cards["alliance:2"].portrait);
    assert.equal(cards["alliance:2"].portrait.src, "/assets/gameplay/avatars/null.webp");

    const pressed = cards["resources:3"].accept;
    pressed.focus();
    const insertions = refs.inboxRows.insertions;
    context.renderInbox(requests);
    assert.equal(refs.inboxRows.insertions, insertions, "unchanged snapshots never detach a button");
    requests = [alliance, { ...resources, gold: 2000 }, campaign, eliminated];
    context.renderInbox(requests);
    assert.equal(refs.inboxRows.insertions, insertions);
    assert.equal(refs.inboxRequests["resources:3"].accept, pressed);
    assert.equal(doc.activeElement, pressed);
    context.handleHudClick({ target: pressed });
    context.handleHudClick({ target: refs.inboxRequests["alliance:2"].reject });
    assert.deepEqual(calls, [
        { type: "accept_resource_request", target_player_id: 3 },
        { type: "reject_alliance", target_player_id: 2 }
    ]);
    const added = { ...alliance, requester_id: 6, name: "LongPlayerName".repeat(12) };
    context.renderInbox([added, ...requests]);
    assert.equal(refs.inboxRows.insertions, insertions + 1, "a new request preserves the other cards");
    assert.equal(refs.inboxRequests["alliance:6"].name.textContent, added.name, "long names are retained as text");
    assert.equal(refs.inboxRequests["resources:3"].accept, pressed);
    context.renderInbox([added, alliance, campaign, eliminated]);
    assert.equal(doc.activeElement, refs.inboxRequests["resources:4"].accept, "resolved cards move focus to the next request");

    context.window.SOW_LOCALE = "es";
    context.renderInbox([added, alliance, campaign, eliminated]);
    assert.equal(refs.inboxRequests["alliance:2"].accept.textContent, "es:hud.accept");
    assert.equal(refs.inboxRequests["alliance:2"].type.textContent, "es:hud.alliance_request_title");
    context.renderHud = () => { context.hudRefs = makeRefs(); context.renderInbox(requests); };
    const localeStart = hud.indexOf('    window.addEventListener("sow:locale-change", function () {');
    const localeEnd = hud.indexOf('    window.addEventListener("resize", updateHudSafeArea);', localeStart);
    vm.runInContext(hud.slice(localeStart, localeEnd), context);
    listeners["sow:locale-change"]();
    assert.notEqual(context.hudRefs, refs);
    assert.equal(context.hudRefs.inboxRows.children.length, 4, "a rebuilt inbox cannot retain the old DOM render key");
    assert.equal(context.hudRefs.inboxRequests["resources:3"].type.textContent, "es:hud.resource_request_title");
    context.hudRefs.inboxRequests["resources:3"].accept.focus();
    context.renderInbox([]);
    assert.equal(context.hudRefs.inboxRows.hidden, true);
    assert.equal(context.hudRefs.inboxEmpty.hidden, false);
    assert.equal(context.hudRefs.inboxTotal.textContent, "0");
    assert.equal(doc.activeElement, context.hudRefs.inboxClose);

    const visibilityStart = hud.indexOf("        if (hudRefs.inbox) {", hud.indexOf("    function renderHud("));
    const visibilityEnd = hud.indexOf("        if (hudRefs.transfer)", visibilityStart);
    const visibility = new vm.Script(hud.slice(visibilityStart, visibilityEnd));
    context.activeHudPanel = "inbox";
    context.hud = { inbox: requests };
    context.renderHud = () => {
        context.inboxOpen = context.activeHudPanel === "inbox";
        visibility.runInContext(context);
    };
    context.renderHud();
    assert.equal(doc.activeElement, context.hudRefs.inboxClose, "opening the inbox moves focus inside it");
    context.hudRoot.addEventListener = (name, listener) => { listeners[name] = listener; };
    const keyboardStart = hud.indexOf('    if (hudRoot) hudRoot.addEventListener("keydown", function (event) {');
    vm.runInContext(
        hud.slice(hud.indexOf("    function setHudPanel("), hud.indexOf("    function asset("))
        + hud.slice(keyboardStart, localeStart), context
    );
    let prevented = false;
    listeners.keydown({ key: "Escape", preventDefault() { prevented = true; }, stopPropagation() {} });
    assert.equal(prevented, true);
    assert.equal(context.activeHudPanel, null);
    assert.equal(context.hudRefs.inbox.classList.contains("hidden"), true);
    assert.equal(doc.activeElement, context.hudRefs.inboxButton, "Escape returns focus to the toolbar control");
    assert.deepEqual(calls.at(-1), { type: "toggle_inbox" });

    assert.match(hudCss, /\.sow-hud__transfer-actions button:active:not\(:disabled\),\s*\.sow-hud__request-actions button:active:not\(:disabled\)\s*\{[^}]*transform:\s*translateY\(1px\)/);
    assert.match(hudCss, /\.sow-hud__transfer-actions button:disabled,\s*\.sow-hud__request-actions button:disabled\s*\{[^}]*opacity:\s*0\.35/);
    assert.match(hudCss, /\.sow-hud__request-actions button \{[^}]*min-block-size:\s*2\.75rem/);
    assert.match(hudCss, /\.sow-hud__request-name \{[^}]*overflow-wrap:\s*anywhere/);


});


test("map feedback reuses one compact card, clamps to the viewport, and releases message data", () => {
    const start = hud.indexOf("    function createNotificationCard(container, contextual) {");
    const end = hud.indexOf("\n    function renderHud(", start);
    const durationMatch = hud.match(/var MAP_FEEDBACK_DURATION_MS = (\d+);/);
    const fadeOutMatch = hud.match(/var MAP_FEEDBACK_FADE_OUT_MS = (\d+);/);
    assert.ok(durationMatch, "map feedback duration is declared");
    assert.ok(fadeOutMatch, "map feedback fade-out duration is declared");
    const durationMs = Number(durationMatch[1]);
    const fadeOutMs = Number(fadeOutMatch[1]);
    assert.equal(durationMs, 2000, "map feedback stays brief");
    assert.equal(fadeOutMs, 180, "map feedback fades out gently");
    assert.match(hudCss, /\.sow-hud__notification--contextual\s*\{[^}]*opacity: 0;[^}]*transition: opacity 180ms ease, transform 180ms cubic-bezier/);
    assert.match(hudCss, /\.sow-hud__notification--contextual\.sow-hud__map-feedback-card--visible\s*\{[^}]*opacity: 1;/);
    assert.ok(start >= 0 && end > start);
    let showTransitions = 0;
    const classes = new Set();
    const card = {
        hidden: true,
        className: "",
        classList: {
            add(name) {
                classes.add(name);
                if (name === "sow-hud__map-feedback-card--visible") showTransitions += 1;
            },
            remove(name) { classes.delete(name); },
            contains(name) { return classes.has(name); }
        },
        get offsetWidth() { return 220; },
        getBoundingClientRect() { return { width: 220, height: 46 }; }
    };
    const parts = {
        card,
        seal: { innerHTML: "" },
        avatars: Array.from({ length: 2 }, () => ({
            wrapper: { hidden: true, className: "" },
            image: {
                hidden: true,
                src: "",
                getAttribute(name) { return name === "src" ? this.src : null; },
                removeAttribute(name) { if (name === "src") this.src = ""; }
            },
            emblem: { hidden: true, textContent: "" }
        })),
        copy: { textContent: "" },
        renderKey: ""
    };
    const style = {
        left: "",
        top: "",
        removeProperty(name) { this[name] = ""; }
    };
    const container = { hidden: true, style };
    let now = 1000;
    const timers = [];
    const context = {
        hudRefs: { mapFeedback: container },
        mapFeedbackCursor: 0,
        mapFeedbackTimer: null,
        activeMapFeedback: null,
        mapFeedbackCard: parts,
        MAP_FEEDBACK_DURATION_MS: durationMs,
        MAP_FEEDBACK_FADE_OUT_MS: fadeOutMs,
        Date: { now: () => now },
        asset: path => "/assets/" + path,
        hudIcon: name => name,
        SOW_t: key => key,
        window: {
            innerWidth: 375,
            innerHeight: 667,
            clearTimeout(timer) { timer.cleared = true; },
            setTimeout(callback, delay) {
                const timer = { callback, delay, cleared: false };
                timers.push(timer);
                return timer;
            }
        }
    };
    const renderer = hud.slice(start, end) + "\nthis.renderMapFeedback = renderMapFeedback;";
    vm.runInNewContext(renderer, context);

    context.renderMapFeedback({ id: 1, key: "hud.build_no_space", values: {}, x: 0, y: 0, age_ms: 0 });
    assert.equal(container.hidden, false);
    assert.equal(parts.copy.textContent, "hud.build_no_space");
    assert.ok(card.classList.contains("sow-hud__map-feedback-card--visible"), "the card fades in");
    assert.ok(Number.parseFloat(style.left) >= 118, "card stays inside the left edge");
    assert.equal(style.top, "72px", "card clears the HUD top bar");
    const timerCount = timers.length;
    context.renderMapFeedback({ id: 1, key: "hud.build_no_space", values: {}, x: 200, y: 300, age_ms: 0 });
    assert.equal(timers.length, timerCount, "the same event is not rendered twice");

    context.renderMapFeedback({ id: 2, key: "hud.build_no_space", values: {}, x: 200, y: 300, age_ms: 0 });
    assert.equal(timers.length, timerCount + 1, "a repeated click restarts the feedback timer");
    assert.ok(timers.at(-2).cleared, "repeated feedback releases its previous timer");
    assert.equal(style.left, "200px", "repeated feedback follows the latest click");
    assert.equal(style.top, "240px");
    assert.equal(timers.at(-1).delay, 2000);
    assert.equal(showTransitions, 2, "repeated feedback restarts its entrance");

    context.renderMapFeedback({ id: 3, key: "hud.build_land", values: {}, x: 375, y: 650, age_ms: 0 });
    assert.equal(parts.copy.textContent, "hud.build_land");
    assert.ok(timers.at(-2).cleared, "replaced feedback releases its previous timer");
    assert.ok(Number.parseFloat(style.left) <= 257, "card stays inside the right edge");
    now += 2000;
    timers.at(-1).callback();
    assert.equal(container.hidden, false, "the card remains during its fade-out");
    assert.equal(parts.copy.textContent, "hud.build_land");
    assert.equal(card.classList.contains("sow-hud__map-feedback-card--visible"), false);
    assert.equal(timers.at(-1).delay, fadeOutMs);
    now += fadeOutMs;
    timers.at(-1).callback();
    assert.equal(container.hidden, true);
    assert.equal(parts.copy.textContent, "");
    assert.ok(parts.avatars.every(avatar => avatar.wrapper.hidden && avatar.image.src === ""));

    context.renderMapFeedback({ id: 4, key: "hud.build_land", values: {}, x: 375, y: 650, age_ms: 2000 });
    assert.equal(container.hidden, true, "expired feedback is not displayed");
});


test("Campaign Studio avoids rebuilding the route graph on progress-only updates", () => {
    const start = campaignEditor.indexOf("function paintPreview(updateMachine) {");
    const end = campaignEditor.indexOf("function previewLocaleScript", start);
    assert.ok(start >= 0 && end > start);
    const paint = campaignEditor.slice(start, end);
    assert.match(paint, /if \(previousStep !== model\.step\.id\) renderGraph\(\)/);
    assert.equal((paint.match(/renderGraph\(\)/g) || []).length, 1);
    assert.match(campaignEditor, /function markDirty\(\)[\s\S]*?refresh\(true\); syncPreview\(\); renderGraph\(\)/);
});


test("Trinovantes aid guides radial opening, Transfer, and the confirmed delivery separately", async () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    assert.deepEqual(campaign.validate(definition, roster).errors, []);
    const missingTarget = structuredClone(definition);
    delete missingTarget.steps.find(step => step.id === "boudica_transfer_target").marker;
    assert.ok(campaign.validate(missingTarget, roster).errors.some(issue => issue.step === "boudica_transfer_target" && issue.field === "marker"));
    const root = { hidden: true, isConnected: false, contains: () => false };
    const radial = { disabled: false, matches: () => false, getClientRects: () => [{}], contains: target => target === radial };
    const transferButton = {
        disabled: false, matches: () => false, getClientRects: () => [{}], closest: () => null,
        getBoundingClientRect: () => ({ left: 300, top: 400, width: 40, height: 40 }),
        contains: target => target === transferButton
    };
    const hud = {
        tutorial: { active: true, episode_id: "boudica", facts: {}, player: { x: 110, y: 210 } },
        players: [{ id: 1, is_me: true }, { id: 2, campaign_faction_id: "trinovantes" }, { id: 3, campaign_faction_id: "stonea" }],
        settings: {}, map_menu: null, transfer: null
    };
    const listeners = {};
    const document = {
        body: { appendChild(node) { node.isConnected = true; } }, documentElement: { dir: "ltr" },
        createElement: () => root, getElementById: () => null,
        querySelector(selector) {
            if (!hud.map_menu || !hud.map_menu.open || hud.map_menu.view !== "radial") return null;
            if (selector === campaign.UI_TARGETS.map_menu_open) return radial;
            if (selector === campaign.UI_TARGETS.map_transfer) return transferButton;
            return null;
        },
        addEventListener(type, callback) { listeners[type] = callback; }
    };
    let machine, rendered, context;
    const window = {
        addEventListener() {}, SOW_menu_command() {}, SOW_t: key => key, SOW_hasText: () => true,
        SOWCampaign: { ...campaign, create(data) {
            machine = campaign.create(data, "boudica_transfer_target", roster);
            return machine;
        } },
        SOWCampaignView: { mount: () => ({ render(model, nextContext) { rendered = model; context = nextContext; }, destroy() {} }) }
    };
    vm.runInNewContext(tutorial, { window, document, performance: { now: () => 0 }, console,
        fetch(url) { return Promise.resolve({ ok: true, json: () => Promise.resolve(url.endsWith(".triggers.json") ? definition : roster) }); }
    });
    const update = () => window.SOW_tutorial_state_update({ phase: "Playing", hud });
    window.SOW_startCampaignEpisode("boudica");
    await new Promise(setImmediate);
    update();
    await new Promise(setImmediate);
    assert.equal(rendered.step.id, "boudica_transfer_target");
    assert.deepEqual([context.anchor.x, context.anchor.y], [110, 210]);

    hud.map_menu = { open: true, view: "radial", target_player_id: 3 };
    update();
    listeners.click({ type: "click", target: radial });
    listeners.click({ type: "click", target: transferButton });
    assert.equal(machine.view().step.id, "boudica_transfer_target", "another ally or a raw menu click cannot satisfy the target step");
    hud.map_menu = { open: true, view: "building_details", target_player_id: 2 };
    update();
    assert.equal(rendered.step.id, "boudica_transfer_target");
    hud.map_menu = { open: true, view: "radial", target_player_id: 2 };
    update();
    assert.equal(rendered.step.id, "boudica_transfer_choose");
    assert.equal(rendered.paused, true);
    assert.deepEqual([context.anchor.x, context.anchor.y], [320, 420]);
    update();
    listeners.click({ type: "click", target: transferButton });
    assert.equal(machine.view().step.id, "boudica_transfer_choose", "Transfer waits for the confirmed modal destination");

    hud.map_menu = null;
    update();
    assert.equal(rendered.step.id, "boudica_transfer_choose");
    assert.deepEqual([context.anchor.x, context.anchor.y], [110, 210], "closing the radial points back to Trinovantes");
    hud.map_menu = { open: true, view: "radial", target_player_id: 3 };
    update();
    listeners.click({ type: "click", target: transferButton });
    hud.map_menu = null;
    hud.transfer = { target_id: 3 };
    update();
    assert.equal(rendered.step.id, "boudica_transfer_choose", "another ally's transfer modal cannot advance the guide");
    hud.transfer = { target_id: 2 };
    update();
    assert.equal(rendered.step.id, "boudica_transfer_send");
    assert.equal(rendered.step.guide.target, "transfer_send");
    assert.equal(rendered.paused_action, "send_resources_stay_paused");
    hud.tutorial.facts.resource_transfers_by_recipient_faction_id = { stonea: { total: 1 } };
    update();
    assert.equal(rendered.step.id, "boudica_transfer_send");
    hud.tutorial.facts.resource_transfers_by_recipient_faction_id.trinovantes = { total: 1 };
    update();
    assert.equal(rendered.step.id, "boudica_share_sent");

    machine.jump("boudica_transfer_target");
    hud.tutorial.facts = {};
    hud.transfer = null;
    update();
    hud.map_menu = { open: true, view: "radial", target_player_id: 2 };
    update();
    assert.equal(rendered.step.id, "boudica_transfer_choose", "revisiting the guide records a new menu opening");
    hud.map_menu = null;
    hud.transfer = { target_id: 2 };
    update();
    assert.equal(rendered.step.id, "boudica_transfer_send", "revisiting the guide records a new Transfer action");
});


test("Legion IX slider objective waits for the live HUD ratio to reach 100%", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "attack_ratio_test", default_locale: "en", settings: { starting_troops: 0 },
        strings: { en: {} }, speakers: {}, entry: "objective", steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", hint_key: "tutorial.objective_hint", trigger: { type: "ui", action: "attack_ratio", scope: "step", value: 1 }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });

    assert.equal(machine.update({ attack_ratio: 0.5 }, {}).step.id, "objective");
    assert.equal(machine.update({ attack_ratio: 0.99 }, {}).step.id, "objective");
    assert.equal(machine.update({ attack_ratio: 1 }, {}).step.id, "ending");
});


test("gameplay and editor share compact viewport and hand progress rules", () => {
    const view = vm.runInNewContext(campaignView + "\nSOWCampaignView;");
    const root = { dataset: {}, clientWidth: 390, clientHeight: 844 };
    assert.equal(view.isCompactViewport(root), true, "mobile portrait hides the quest panel");
    root.clientWidth = 844; root.clientHeight = 390;
    assert.equal(view.isCompactViewport(root), true, "compact landscape hides the quest panel");
    root.clientWidth = 1366; root.clientHeight = 768;
    assert.equal(view.isCompactViewport(root), false, "normal desktop keeps the quest panel");
    root.clientWidth = 1024; root.clientHeight = 600;
    assert.equal(view.isCompactViewport(root), true, "low-height desktop uses compact gameplay");
    root.clientWidth = 1366; root.clientHeight = 768; root.dataset.compact = "true";
    assert.equal(view.isCompactViewport(root), true, "the shared HUD density flag remains authoritative");

    assert.equal(view.compactGuideMetric(
        { guide: { kind: "world", target: "expand" }, trigger: { type: "territory", value: 250 } },
        { current: 120, target: 250 }
    ).text, "120 / 250");
    assert.equal(view.compactGuideMetric(
        { guide: { kind: "ui", target: "attack_ratio" }, trigger: { type: "ui", action: "attack_ratio", value: 1 } },
        { current: 0.72, target: 1 }
    ).text, "72% / 100%");
    assert.equal(view.compactGuideMetric(
        { guide: { kind: "world", target: "player" }, trigger: { type: "territory", scope: "total", comparison: "lte", value: 5000 } },
        { current: 7200, target: 5000 }
    ), null, "the at-most territory condition is not misrepresented as a progress bar");
    assert.equal(view.compactGuideMetric(
        { trigger: { type: "territory", value: 250 } }, { current: 120, target: 250 }
    ), null, "passive status steps do not gain a fake action counter");
    assert.match(hud, /SOWCampaignView\.isCompactViewport\(hudRoot, width, height\)/);
});


test("short landscape HUD geometry fits the requested phone widths and heights", () => {
    assert.match(hudCss, /@media \(hover: none\) and \(pointer: coarse\) \{[\s\S]*?#sow-hud\[data-compact="true"\]/);
    assert.match(hudCss, /\.sow-hud__dock \{[^}]*width: min\(520px, calc\(100vw - max\(8px, var\(--sow-sal\)\) - max\(8px, var\(--sow-sar\)\) - 204px\)\)/);
    assert.match(hudCss, /\.sow-hud__operations \{[^}]*width: min\(320px, calc\(100vw - max\(8px, var\(--sow-sal\)\) - 280px - max\(8px, var\(--sow-sar\)\)\)/);
    assert.match(hudCss, /\.sow-hud__building-btn \{ flex: 0 0 44px; min-height: 44px; \}/);
    assert.match(hudCss, /\.sow-hud__icon-btn \{ width: 44px; height: 44px/);
    for (const [width, height, safeLeft] of [[568, 320, 44], [667, 375, 0], [844, 390, 0], [932, 430, 0]]) {
        const leftInset = Math.max(8, safeLeft), rightInset = 8;
        const dockWidth = Math.min(520, width - leftInset - rightInset - 204);
        const operationWidth = Math.min(320, width - leftInset - 280 - rightInset);
        const dockLeft = (width - dockWidth) / 2;
        const operationsRight = width - rightInset;
        const operationsHeight = Math.min(height * 0.2, Math.max(0, height - 56 - 84 - 6 - 6));
        assert.ok(dockWidth >= 280, `${width}x${height}: dock has room for the building action strip`);
        assert.ok(operationWidth >= 220, `${width}x${height}: operations keep a usable width`);
        assert.ok(dockLeft >= leftInset && dockLeft + dockWidth <= width - rightInset, `${width}x${height}: dock stays inside safe edges`);
        assert.ok(operationsRight <= width - rightInset && operationsRight - operationWidth >= leftInset, `${width}x${height}: operations stay inside safe edges`);
        assert.ok(operationsHeight > 0 && operationsHeight <= height * 0.2, `${width}x${height}: operations remain scrollable within 20% of the frame`);
    }
    assert.match(hudCss, /@media \(hover: hover\) and \(pointer: fine\) \{[\s\S]*?#sow-hud\[data-compact="true"\]/);
    assert.match(hudCss, /@media \(max-width: 599px\) and \(orientation: portrait\)/);
});


test("transfer acknowledgements release the mobile sidebars without leaving a hidden open panel", () => {
    const transfer = { target_id: 2, target_name: "Trinovantes", suggested_gold: 12, suggested_troops: 34 };
    for (const command of ["close_transfer", "send_resources", "request_resources"]) {
        const { context, calls, update: publish, click } = makePlayerDialogsHarness();
        const panel = context.hudRefs.transfer;
        const update = (value, extra = {}) => publish({ transfer: value, ...extra });
        update(transfer);
        assert.equal(panel.classList.contains("hidden"), false);
        assert.equal(context.hudRoot.dataset.overlayOpen, "true");
        click(command);
        assert.deepEqual(calls, [command === "close_transfer" ? { type: command }
            : { type: command, target_player_id: 2, gold: 12, troops: 34 }]);
        context.renderHud();
        assert.equal(panel.classList.contains("hidden"), false, "the current state keeps a pending transfer visible");
        assert.equal(context.hudRoot.dataset.overlayOpen, "true");
        update(null);
        assert.equal(panel.classList.contains("hidden"), true, command);
        assert.equal(context.hudRoot.dataset.overlayOpen, "false", command + " releases both mobile sidebars");
        update(null, { gold: 100, attack_ratio: 1 });
        context.renderHud();
        assert.equal(context.hudRoot.dataset.overlayOpen, "false", "later tutorial/HUD updates cannot resurrect a closed transfer");
        for (const flag of ["settingsOpen", "mapMenuOpen", "surrenderModalOpen"]) {
            context[flag] = true;
            context.renderHud();
            assert.equal(context.hudRoot.dataset.overlayOpen, "true", flag + " still blocks as intended");
            context[flag] = false;
            context.renderHud();
            assert.equal(context.hudRoot.dataset.overlayOpen, "false");
        }
        click("open_transfer");
        assert.equal(calls.at(-1).type, "open_transfer");
        assert.equal(context.hudRoot.dataset.overlayOpen, "false", "an unconfirmed open cannot hide the rails without a panel");
        update(transfer);
        click("send_resources");
        update(transfer);
        context.renderHud();
        assert.equal(panel.classList.contains("hidden"), false, "a rejected send keeps the authoritative panel open for retry");
        assert.equal(context.hudRefs.transferGold.value, "12");
        assert.equal(context.hudRefs.transferTroops.value, "34");
        update(null);
        assert.equal(context.hudRoot.dataset.overlayOpen, "false", "a state-driven close also restores the rails");
    }
    assert.match(hudCss, /#sow-hud\[data-overlay-open="true"\] \.sow-hud__left-rail,[^}]*\.sow-hud__right-rail,[^}]*display: none;/);
});


test("player dialogs share identities, preserve drafts and focus, and prioritize safe keyboard actions", () => {
    const { context, doc, calls, listeners, update, click, key, canvas } = makePlayerDialogsHarness();
    const transfer = { target_id: 2, target_name: "<img onerror=alert(1)>".repeat(8), target_avatar: { kind: "portrait", slug: "caesar" }, suggested_gold: 12.9, suggested_troops: 34.9 };
    update({ transfer });
    const first = context.hudRefs, gold = first.transferGold, troops = first.transferTroops;
    assert.equal(doc.activeElement, first.transferClose, "opening a transfer does not open the mobile keyboard");
    assert.equal(first.transferIdentity.name.textContent, transfer.target_name);
    assert.equal(first.transferIdentity.name.children.length, 0);
    assert.equal(first.transferIdentity.portrait.src, "/assets/gameplay/avatars/caesar.webp");
    assert.equal(gold.value, "12");
    first.transferIdentity.portrait.onerror.call(first.transferIdentity.portrait);
    assert.equal(first.transferIdentity.portrait.src, "/assets/gameplay/avatars/null.webp");
    gold.value = "1234567";
    troops.value = "7000";
    gold.focus();
    for (let tick = 1; tick <= 5; tick += 1) update({ transfer: { ...transfer, suggested_gold: tick }, tick });
    assert.equal(context.hudRefs.transferGold, gold);
    assert.equal(gold.value, "1234567");
    assert.equal(troops.value, "7000");
    assert.equal(doc.activeElement, gold);
    click("request_resources");
    assert.deepEqual(calls.at(-1), { type: "request_resources", target_player_id: 2, gold: 1234567, troops: 7000 }, "requests are not capped by the sender's balance");
    context.window.SOW_LOCALE = "ar";
    listeners["sow:locale-change"]();
    assert.notEqual(context.hudRefs.transferGold, gold);
    assert.equal(context.hudRefs.transferGold.value, "1234567");
    assert.equal(context.hudRefs.transferTroops.value, "7000");
    assert.equal(doc.activeElement, context.hudRefs.transferGold);
    assert.equal(context.hudRefs.transferIdentity.name.textContent, transfer.target_name);
    context.activeHudPanel = "inbox";
    assert.equal(key("Escape").prevented, true);
    assert.deepEqual(calls.at(-1), { type: "close_transfer" });
    assert.equal(context.activeHudPanel, "inbox", "Escape closes only the foreground transfer");
    assert.equal(context.hudRefs.transfer.classList.contains("hidden"), false, "wait for the authoritative close");
    update({ transfer: null });
    assert.equal(doc.activeElement, canvas);
    update({ transfer });
    assert.equal(context.hudRefs.transferGold.value, "12", "reopening the same target starts a fresh draft");
    update({ transfer: { ...transfer, target_id: 3, target_name: "Trinovantes", target_avatar: { kind: "emblem", symbol: "🐺" }, suggested_gold: 7 } });
    assert.equal(context.hudRefs.transferGold.value, "7");
    assert.equal(context.hudRefs.transferIdentity.emblem.textContent, "🐺");
    assert.equal(context.hudRefs.transferIdentity.portrait.hidden, true);
    update({ transfer: { ...transfer, target_avatar: { kind: "portrait", slug: "boudica" } } });
    assert.equal(context.hudRefs.transferIdentity.portrait.src, "/assets/gameplay/avatars/boudica.webp");
    update({ transfer: null });
    const betrayal = { ally_id: 3, ally_name: "Ally".repeat(30), ally_avatar: { kind: "emblem", symbol: "🐺" } };
    update({ betrayal });
    const refs = context.hudRefs;
    assert.equal(refs.betrayalIdentity.emblem.textContent, "🐺");
    assert.equal(refs.betrayalIdentity.name.textContent, betrayal.ally_name);
    assert.equal(doc.activeElement, refs.betrayalCancel, "the destructive action is never focused by default");
    const commandCount = calls.length;
    context.handleHudClick({ target: refs.betrayal });
    assert.equal(calls.length, commandCount, "backdrop taps do not attack or send anything to the game");
    refs.betrayalConfirm.focus();
    assert.equal(key("Tab").prevented, true);
    assert.equal(doc.activeElement, refs.betrayalClose);
    key("Tab", true);
    assert.equal(doc.activeElement, refs.betrayalConfirm);
    update({ betrayal, tick: 20 });
    assert.equal(doc.activeElement, refs.betrayalConfirm);
    context.window.SOW_LOCALE = "es";
    listeners["sow:locale-change"]();
    assert.equal(doc.activeElement, context.hudRefs.betrayalConfirm);
    assert.equal(key("Escape").prevented, true);
    assert.deepEqual(calls.at(-1), { type: "cancel_betrayal" });
    update({ betrayal: null });
    update({ betrayal });
    const beforeConfirm = calls.length;
    click("confirm_betrayal");
    assert.equal(calls.length, beforeConfirm + 1);
    assert.deepEqual(calls.at(-1), { type: "confirm_betrayal" });
    update({ betrayal: null });
    update({ betrayal: { ally_id: 99, ally_name: "", ally_avatar: { kind: "fallback" } } });
    assert.equal(context.hudRefs.betrayalIdentity.name.textContent, "es:hud.player_name");
    assert.equal(context.hudRefs.betrayalIdentity.portrait.src, "/assets/gameplay/avatars/null.webp");
    assert.match(hud, /hudIcon\("betray", "sow-hud__inline-icon"\)/);
    assert.doesNotMatch(hud, /class="sow-hud__panel sow-hud__transfer/);
    assert.doesNotMatch(hudCss, /sow-hud__modal-card|sow-hud__transfer-target|sow-hud__panel-actions|sow-hud__btn-danger/);
    assert.match(hudCss, /\.sow-hud__dialog-body \{[^}]*min-block-size: 0;[^}]*overflow-y: auto;/);
    assert.match(hudCss, /#sow-hud \.sow-hud__dialog \.sow-hud__transfer-actions button \{[^}]*min-block-size: 2\.75rem/);
    assert.match(hudCss, /\.sow-hud__dialog \.sow-hud__transfer-actions \{[^}]*flex-wrap: wrap/);
    assert.match(hudCss, /\.sow-hud__transfer-actions \.sow-hud__betrayal-confirm \{[^}]*var\(--sow-red\)/);
});


test("shared dialog viewport tracks the available keyboard area without scaling or per-dialog listeners", () => {
    const start = hud.indexOf("    function syncCompactHudBounds() {");
    const end = hud.indexOf("        var density = syncCompactHudDensity();", start);
    assert.ok(start >= 0 && end > start);
    const { doc, Element } = makeHudDocument();
    const root = new Element("root");
    const context = vm.createContext({ hudRoot: root, hudRefs: {}, window: {} });
    vm.runInContext(hud.slice(start, end) + "    }", context);
    for (const [width, height] of [[1366,768], [390,844], [360,640], [844,390], [640,360], [568,320]]) {
        context.window.innerWidth = width;
        context.window.innerHeight = root.clientHeight = height;
        context.window.visualViewport = { height, offsetTop: 0 };
        context.syncCompactHudBounds();
        assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-height"), height + "px");
        assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-bottom"), "0px");
    }
    root.clientHeight = context.window.innerHeight = 844;
    context.window.visualViewport = { height: 360, offsetTop: 80 };
    context.syncCompactHudBounds();
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-height"), "360px");
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-top"), "80px");
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-bottom"), "404px");
    context.window.visualViewport = null;
    context.syncCompactHudBounds();
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-height"), "844px");
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-bottom"), "0px");
    assert.match(hud, /visualViewport\.addEventListener\("resize", onViewportChange/);
    assert.match(hud, /visualViewport\.addEventListener\("scroll", onViewportChange/);
    assert.match(hudCss, /\.sow-hud__dialog \{[^}]*inline-size: min\(28rem/);
    assert.match(hudCss, /data-compact="true"\] \.sow-hud__dialog \{[^}]*inline-size: min\(36rem/);
    assert.match(hudCss, /\.sow-hud__dialog \{[^}]*max-block-size: min\(75dvh/);
    assert.match(hudCss, /\.sow-hud__transfer \{[^}]*bottom: calc\(var\(--sow-hud-dialog-view-bottom/);
});


test("mobile overlays never hide the building dock and story dialogs remain above it", () => {
    const overlayRules = hudCss.match(/#sow-hud\[data-overlay-open="true"\][^{]*\{[^}]*\}/g) || [];
    assert.ok(overlayRules.length >= 2, "portrait and short-landscape overlay rules remain covered");
    assert.ok(overlayRules.every(rule => !rule.includes(".sow-hud__dock")),
        "transient menus and panels cannot hide the persistent building dock");
    assert.match(hud, /hudRefs\.buildingsStrip\.style\.display = !isDeploying && hasBuildingItems \? "flex" : "none"/,
        "the building strip hides when deployment is active or the tutorial locks every building");
    assert.match(hudCss, /\.sow-hud__dock\s*\{[^}]*z-index: 50/s);
    assert.match(storyCss, /\.sow-story\s*\{[^}]*z-index: 210/s,
        "blocking story dialogs can cover the dock");
});


test("tutorial collapses an empty action dock and releases its combat-operation space", () => {
    assert.match(hud, /hasBuildingItems = hudRefs\.buildingButtons\.some\(function \(button\) \{ return !button\.hidden; \}\)/);
    assert.match(hud, /var dockHidden = !\(isDeploying \|\| hasBuildingItems \|\| hasFleetItems\)/);
    assert.match(hud, /if \(hudRefs\.dock && hudRefs\.dock\.hidden !== dockHidden\) \{\s*hudRefs\.dock\.hidden = dockHidden;\s*syncCompactHudBounds\(\);\s*\}/);
    assert.match(hudCss, /\.sow-hud__dock\[hidden\] \{ display: none !important; \}/);
    assert.match(hud, /function syncDefaultDockReserve\(\) \{\s*if \(hudRefs\.dock && hudRefs\.dock\.hidden\) \{\s*hudRoot\.style\.setProperty\("--sow-dock-reserve", "0px"\)/);
    assert.match(hud, /var dockReserve = stackHeight > 0 \? stackHeight \+ Math\.max\(8, safeBottom\) \+ 8 : 0/);
    assert.match(hudCss, /#sow-hud\[data-compact="true"\] \.sow-hud__dock-stack\s*\{[^}]*flex-direction: column-reverse/);
});


test("tutorial locale and UI guides resolve per episode and point to actual HUD controls", () => {
    assert.match(tutorial, /window\.SOW_t\(key\)/);
    assert.match(tutorial, /function menuGuideTranslate\(key\)/);
    assert.match(tutorial, /SOWCampaign\.resolveUiTarget\(guide\.target, document, runtime\.episodeId, guideFacts\)/);
    assert.match(tutorial, /if \(!source \|\| source\.disabled \|\| source\.getClientRects\(\)\.length === 0\) return null/);
    assert.match(tutorial, /tutorial\[guide\.target\]/);
    assert.match(campaignView, /sow-story__gesture/);
    assert.match(campaignView, /sow-story__gesture-label/);
    assert.match(campaignView, /sow-story__gesture-hint/);
    assert.match(campaignView, /sow-story__zoom-fingers/);
    assert.doesNotMatch(campaignView + storyCss, /sow-story__guide-shade|guide-cutout|mask-image:\s*radial-gradient/);
    assert.match(campaignView, /spotlight\.hidden = !guideVisible \|\| !anchor\.width/);
    assert.doesNotMatch(campaignView, /stopGuideShadeInput|guideShade\.addEventListener/);
    assert.match(campaignView, /const modal = \["scene", "choice", "end"\]\.includes\(step\.type\)/);
    assert.match(storyCss, /\.sow-story__gesture-label \{[^}]*text-transform: uppercase/);
    assert.match(storyCss, /data-gesture="zoom_in"/);
    assert.match(storyCss, /data-gesture="zoom_out"/);
    assert.match(storyCss, /\.sow-story__zoom \{[^}]*width: 108px; height: 108px/);
    assert.match(storyCss, /data-gesture="zoom_out"[^\n]*story-left-together/);
    assert.match(storyCss, /data-gesture="zoom_out"[^\n]*story-right-together/);
    assert.match(storyCss, /data-gesture="pan_keys"/);
    assert.match(campaignEditor, /var zoomMode = \$\("#device"\)\.value === "mobile" \? "pinch" : window\.SOWCampaign\.zoomInputMode\(\{ platform: previewPlatform \}\)/);
    assert.match(campaignEditor, /stepTextKey\(model\.step, "_" \+ zoomMode\)[\s\S]*_hint[\s\S]*_label/);
    assert.match(tutorial, /stepTextKey\(step, "_" \+ context\.zoomMode \+ "_hint"\)/);
    assert.match(tutorial, /stepTextKey\(step, "_" \+ context\.zoomMode \+ "_label"\)/);
    assert.match(tutorial, /context\.gestureHint = context\.hintOverride/);
    assert.match(campaignView, /const showGestureHint = !compactStatusVisible && showGestureText && \(gestureType === "hover" \|\| zoomGuide\)[\s\S]*context\.gestureHint \|\| context\.hintOverride/);
    assert.match(campaignEngine, /\["pinch", "trackpad", "wheel"\]\.forEach\(mode => \["_hint", "_label"\]/);
    assert.match(storyCss, /\.sow-story__gesture-hint \{[^}]*text-transform: none/);
    assert.match(campaignEngine, /menu_campaign:/);
    assert.match(campaignEngine, /map_attack:/);
    assert.match(tutorial, /addEventListener\("resize", function \(\) \{\s*runtime\.guidedScrollStepId = null;\s*redrawCampaign\(\);\s*\}/);
    assert.match(tutorial, /var actionEvent = control && control\.matches\("input"\) \? "change" : "click"/);
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


test("building actions choose the selected tile, filter invalid placement, and allow cancel", () => {
    const actionsStart = mapClick.indexOf("pub(crate) fn map_menu_actions");
    const actionsEnd = mapClick.indexOf("pub(crate) fn map_menu_items", actionsStart);
    const actionsBody = mapClick.slice(actionsStart, actionsEnd);
    assert.match(actionsBody, /resolve_building_target\(kind, col, row\)/);
    assert.match(mapClick, /fn resolve_building_target[\s\S]*?validate_structure_site\(/);

    const actionStart = mapClick.indexOf("pub(crate) fn handle_map_menu_action");
    const actionEnd = mapClick.indexOf("pub(crate) fn move_selected_warships", actionStart);
    const actionBody = mapClick.slice(actionStart, actionEnd);
    assert.match(actionBody, /if let Some\(\(col, row\)\) = self\.tile_coords\(tile_idx\) \{\s*self\.select_building_kind\(kind\);\s*self\.move_building_preview\(col, row\);/);
    assert.doesNotMatch(actionBody, /GameplayIntent::BuildStructure/);

    const selectionStart = mapClick.indexOf("pub(crate) fn select_building_kind");
    const selectionEnd = mapClick.indexOf("pub(crate) fn select_warship_build_mode", selectionStart);
    const selectionBody = mapClick.slice(selectionStart, selectionEnd);
    assert.match(selectionBody, /building_placement\s*\.is_some_and\(\|p\| p\.kind == kind\)\s*\{\s*self\.clear_placement\(\)/);
    assert.match(mapClick, /pub\(crate\) fn clear_placement\(&mut self\)\s*\{\s*if self\s*\.ui\s*\.app\s*\.hud_state\s*\.building_placement\s*\.is_some_and\(\|p\| p\.pending\)\s*\{\s*return;\s*\}/);
    assert.match(webMenu, /WebMenuCommand::CancelBuildingMode => self\.clear_placement\(\)/);
    assert.match(hud, /data-command="cancel_building_mode"/);
    assert.match(mapClick, /confirm_building_placement[\s\S]*?GameplayIntent::BuildStructure \{[\s\S]*?target_tile: placement\.tile_idx/);
    assert.match(buildingOverlaySource, /celebration_budget = 24/);
    assert.match(buildingOverlaySource, /!reduced_motion[\s\S]*?celebration_budget >= BUILDING_CELEBRATION_OFFSETS\.len\(\)/);
});


test("building upgrade uses snapshot gold and closes the card after sending", () => {
    assert.match(simUpdateSource, /pub\(crate\) fn current_player_gold\(&self\)[\s\S]*?player\.gold[\s\S]*?unwrap_or\(self\.ui\.app\.hud_state\.gold\)/);
    assert.ok(webMenu.includes("let has_gold = app.current_player_gold() >= cost"));
    assert.match(hud, /renderBuildingCard\(hud\.map_menu, hud\.gold\)/);
    assert.match(hud, /id="sow-hud-building-card-upgrade" data-map-action="upgrade_structure"/);
    assert.doesNotMatch(hud, /remaining_seconds/);
    assert.doesNotMatch(webMenu, /"remaining_seconds"/);
    assert.match(hud, /return !\(mapMenu\.building && mapMenu\.building\.under_construction\)/);
    const actionStart = mapClick.indexOf("pub(crate) fn handle_map_menu_action");
    const actionEnd = mapClick.indexOf("pub(crate) fn move_selected_warships", actionStart);
    const actionBody = mapClick.slice(actionStart, actionEnd);
    assert.match(actionBody, /self\.current_player_gold\(\) < cost/);
    assert.match(actionBody, /let keep_building_card_open = action == MapMenuAction::UpgradeStructure/);
    assert.ok(actionBody.indexOf("self.current_player_gold() < cost") < actionBody.indexOf("GameplayIntent::UpgradeStructure"));
    assert.ok(actionBody.trimEnd().endsWith("self.close_map_context_menu();\n    }"));
    assert.match(mapClick.slice(mapClick.indexOf("fn select_owned_building"), mapClick.indexOf("pub(crate) fn open_map_context_menu")), /building\.under_construction[\s\S]*?close_map_context_menu\(\)[\s\S]*?return true/);
});


test("tutorial HUD evaluates campaign availability without private campaign globals", () => {
    const start = hud.indexOf("    function campaignAllowsBuilding(");
    const end = hud.indexOf("    function setCampaignLock(", start);
    const availability = vm.runInNewContext(hud.slice(start, end) + "\n({ campaignAllowsBuilding, campaignAllowsAction });", {
        window: { SOWCampaign: require(path.join(shell, "sow-campaign.js")) }
    });
    const locked = { buildings: {}, actions: [] };
    const unlocked = { buildings: { City: 1, Port: 1 }, actions: ["warship"] };
    assert.equal(availability.campaignAllowsBuilding(locked, "City", 1), false);
    assert.equal(availability.campaignAllowsBuilding(unlocked, "City", 1), true);
    assert.equal(availability.campaignAllowsBuilding(unlocked, "City", 2), false);
    assert.equal(availability.campaignAllowsBuilding(unlocked, "Unknown", 1), false);
    assert.equal(availability.campaignAllowsAction(unlocked, "warship"), false);
    unlocked.buildings.Port = 2;
    assert.equal(availability.campaignAllowsAction(unlocked, "warship"), true);
    assert.equal(availability.campaignAllowsBuilding(null, "City", 1), true);
    assert.equal(availability.campaignAllowsAction(null, "warship"), true);
});


test("building upgrade sends the selected card identity instead of the radial menu", () => {
    const handler = hud.indexOf("function handleHudClick(event) {");
    const start = hud.indexOf("            var mapButton = event.target.closest(\"[data-map-action]\");", handler);
    const end = hud.indexOf('            var btn = event.target.closest("[data-command]");', start);
    const calls = [];
    const card = { dataset: { session: "12", tileIdx: "240" } };
    const button = {
        dataset: { mapAction: "upgrade_structure" }, disabled: false,
        getAttribute() { return null; },
        closest(selector) { return selector === ".sow-hud__building-card" ? card : null; }
    };
    const click = vm.runInNewContext("(function (event) {" + hud.slice(start, end) + "})", {
        hudRefs: { mapMenu: { dataset: { session: "3", tileIdx: "99" } } },
        send: (command, payload) => calls.push({ command, payload })
    });
    click({ target: { closest: selector => selector === "[data-map-action]" ? button : null }, preventDefault() {}, stopPropagation() {} });
    assert.equal(calls.length, 1);
    assert.equal(calls[0].payload.session, 12);
    assert.equal(calls[0].payload.tile_idx, 240);
    assert.equal(calls[0].payload.action, "upgrade_structure");
});


test("building card anchors to the building and shows compact visual stats", () => {
    const renderStart = hud.indexOf("    function escapeHudText(value) {");
    const renderEnd = hud.indexOf("\n    function updateLeaderboard", renderStart);
    assert.ok(renderStart >= 0 && renderEnd > renderStart);

    const element = () => {
        const classes = new Set();
        return {
            dataset: {},
            style: {},
            innerHTML: "",
            textContent: "",
            disabled: false,
            hidden: false,
            title: "",
            offsetWidth: 180,
            offsetHeight: 120,
            classes,
            attributes: {},
            setAttribute(name, value) { this.attributes[name] = value; },
            classList: {
                toggle(name, force) {
                    if (force) classes.add(name);
                    else classes.delete(name);
                }
            }
        };
    };
    const hudRefs = {
        buildingCard: element(),
        buildingCardIcon: element(),
        buildingCardKind: element(),
        buildingCardLevel: element(),
        buildingCardBenefit: element(),
        buildingCardNext: element(),
        buildingCardGold: element(),
        buildingCardUpgrade: element(),
        buildingCardUpgradeLevel: element()
    };
    const renderBuildingCard = vm.runInNewContext(
        hud.slice(renderStart, renderEnd) + "\nrenderBuildingCard;",
        {
            hudRefs,
            window: { innerWidth: 400, innerHeight: 300 },
            SOW_t: key => key,
            hudIcon: (name) => "<svg data-icon=\"" + name + "\"></svg>",
            buildingIcon: (kind) => "<img data-building=\"" + kind + "\">"
        }
    );
    const menu = (building, view = "building_details") => ({
        open: true, view, session: 4, tile_idx: 12, x: 90, y: 100, building
    });

    renderBuildingCard(menu({
        name: "Bunker",
        kind: "Bunker",
        level: 0,
        under_construction: true,
        can_upgrade: false
    }));
    assert.equal(hudRefs.buildingCard.classes.has("hidden"), true);

    renderBuildingCard(menu({
        name: "Bastion",
        kind: "Bunker",
        level: 2,
        benefit_label: "Raises enemy losses by 10% within range 16.",
        metrics: [
            { icon: "defense", label: "Enemy attack losses", value: 10, prefix: "+", unit: "%" },
            { icon: "range", label: "Defense range", value: 16 }
        ],
        next_level: 3,
        next_benefit_label: "Raises enemy losses by 15% within range 18.",
        next_metrics: [
            { icon: "defense", label: "Enemy attack losses", value: 15, prefix: "+", unit: "%" },
            { icon: "range", label: "Defense range", value: 18 }
        ],
        duration_seconds: 6,
        cost: 150,
        owns: true,
        can_upgrade: true
    }), 120);
    assert.match(hudRefs.buildingCardLevel.innerHTML, /Lv 2/);
    assert.equal(hudRefs.buildingCardBenefit.hidden, true);
    assert.doesNotMatch(hudRefs.buildingCardNext.innerHTML, /Lv 3|15%|18/);
    assert.match(hudRefs.buildingCardNext.innerHTML, /5%/);
    assert.match(hudRefs.buildingCardNext.innerHTML, /2/);
    assert.match(hudRefs.buildingCardNext.innerHTML, /6s/);
    assert.match(hudRefs.buildingCardGold.innerHTML, /120/);
    assert.match(hudRefs.buildingCardGold.innerHTML, /150/);
    assert.equal(hudRefs.buildingCardGold.classes.has("is-short"), true);
    assert.equal(hudRefs.buildingCardUpgrade.attributes["aria-label"], "Upgrade to level 3");
    assert.equal(hudRefs.buildingCardUpgrade.hidden, false);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, true);
    renderBuildingCard(menu({
        kind: "Bunker",
        level: 2,
        next_level: 3,
        metrics: [{ icon: "defense", label: "Enemy attack losses", value: 10, prefix: "+", unit: "%" }],
        next_metrics: [{ icon: "defense", label: "Enemy attack losses", value: 15, prefix: "+", unit: "%" }],
        cost: 150,
        can_upgrade: true
    }), 200);
    assert.equal(hudRefs.buildingCardGold.classes.has("is-short"), false);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, false);
    assert.equal(hudRefs.buildingCard.style.left, "104px");
    assert.equal(hudRefs.buildingCard.style.top, "40px");

    renderBuildingCard(menu({
        kind: "Factory",
        level: 1,
        metrics: [{ icon: "gold", label: "Gold income", value: 10, prefix: "+", unit: "/s" }],
        next_level: 2,
        next_metrics: [{ icon: "gold", label: "Gold income", value: 20, prefix: "+", unit: "/s" }],
        cost: 50,
        can_upgrade: true
    }), 200);
    assert.doesNotMatch(hudRefs.buildingCardNext.innerHTML, /Requires a level 3 City/);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, false);

    renderBuildingCard(menu({
        kind: "Bunker",
        level: 2,
        metrics: [{ icon: "defense", label: "Enemy attack losses", value: 10, prefix: "+", unit: "%" }],
        campaign_upgrade_locked: true,
        can_upgrade: false
    }));
    assert.equal(hudRefs.buildingCardNext.hidden, true);
    assert.equal(hudRefs.buildingCardNext.textContent, "");
    assert.equal(hudRefs.buildingCardUpgrade.hidden, true);

    renderBuildingCard(menu({ kind: "Bunker", level: 2 }, "radial"));
    assert.equal(hudRefs.buildingCard.classes.has("hidden"), true);
    renderBuildingCard(null);
    assert.equal(hudRefs.buildingCard.classes.has("hidden"), true);
});


test("primary click selects owned buildings without changing other map gestures", () => {
    const clickStart = mapClick.indexOf("pub(crate) fn handle_map_click");
    const clickEnd = mapClick.indexOf("pub(crate) fn open_map_context_menu", clickStart);
    const clickBody = mapClick.slice(clickStart, clickEnd);
    assert.ok(clickBody.indexOf("selected_nuke_kind") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("building_placement.is_some()") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("select_warships_at") < clickBody.indexOf("select_owned_building"));
    assert.match(clickBody, /select_owned_building\(x, y\)[\s\S]*?primary_target\(tile_idx/);
    assert.match(mapClick, /building_at_pointer\(/);
    assert.match(mapClick, /target\.owner != target\.my_id/);
    assert.match(buildingOverlaySource, /pub\(crate\) fn building_at_pointer/);
    assert.match(buildingOverlaySource, /cached_buildings\(/);
    assert.match(buildingOverlaySource, /if building\.owner_id != my_id/);
    assert.match(buildingOverlaySource, /building\.kind\.footprint_dimensions\(\)/);
    assert.match(buildingOverlaySource, /pointer_world_x < building\.bx - width as f32 \* 0\.5[\s\S]*?pointer_world_x >= building\.bx \+ width as f32 \* 0\.5/);
    assert.match(buildingOverlaySource, /pointer_world_y < building\.by - height as f32 \* 0\.5[\s\S]*?pointer_world_y >= building\.by \+ height as f32 \* 0\.5/);
    assert.match(buildingOverlaySource, /nearest_building_in_cluster/);
    assert.match(buildingOverlaySource, /building_marker_size\(building, zoom_scaled\)/);
    assert.match(appStateSource, /pub enum MapContextMenuView\s*\{\s*BuildingDetails,\s*Radial/);
    assert.match(mapClick, /MapContextMenuView::BuildingDetails/);
    assert.match(mapClick, /MapContextMenuView::Radial/);
    assert.match(windowInput, /if is_quick_tap\(elapsed_ms, distance_sq\)\s*\{\s*self\.handle_map_click/);
    assert.match(windowInput, /else if right[\s\S]*?self\.open_map_context_menu\(x, y\)/);
    assert.match(windowInput, /pub\(crate\) fn poll_pointer_hold[\s\S]*?self\.open_map_context_menu\(x, y\)/);
    assert.match(webMenu, /"view": match menu\.view[\s\S]*?"building_details"[\s\S]*?"radial"/);
    assert.match(hud, /var showRadial = Boolean\(open && mapMenu\.view === "radial"\)/);
    assert.match(hud, /mapMenu\.view === "building_details" && detail/);
    assert.match(hud, /send\("close_map_context_menu"\)/);
    assert.match(webMenu, /WebMenuCommand::CloseMapContextMenu => self\.close_map_context_menu\(\)/);
    assert.match(mapClick, /pub\(crate\) fn close_map_context_menu\(&mut self\)\s*\{\s*self\.input\.map_context_menu = None;/);
    assert.match(webMenu, /map_menu_view: map_menu\.map\(\|menu\| menu\.view\)/);

    const targetStart = mapClick.indexOf("fn primary_target(&mut self");
    const targetEnd = mapClick.indexOf("fn attack_from_tile", targetStart);
    const targetBody = mapClick.slice(targetStart, targetEnd);
    assert.match(targetBody, /target\.owner == target\.my_id\s*\{\s*return;/);
    assert.match(targetBody, /if target\.is_friendly\(\)\s*\{\s*self\.open_map_context_menu\(x, y\);\s*\} else \{\s*self\.attack_from_tile\(tile_idx\);/);
    assert.doesNotMatch(targetBody, /open_transfer_from_tile/);

    const actionStart = mapClick.indexOf("pub(crate) fn handle_map_menu_action");
    const actionEnd = mapClick.indexOf("fn map_menu_cost", actionStart);
    const actionBody = mapClick.slice(actionStart, actionEnd);
    assert.match(actionBody, /menu\.session != session \|\| menu\.tile_idx != tile_idx/);
    assert.match(actionBody, /!self\.map_menu_actions\(tile_idx\)\.contains\(&action\)/);
    assert.match(actionBody, /MapMenuAction::Transfer => \{\s*self\.open_transfer_from_tile\(tile_idx\);/);
    assert.match(webMenu, /renderer\.owners\.get\(menu\.tile_idx as usize\)\.copied\(\)/);
    assert.match(webMenu, /"target_player_id": target_player_id/);
});


test("WASM right-click opens on pointer press; touch hold stays intact", () => {
    assert.match(windowInput, /let right = matches!\([\s\S]*?MouseButton::Right[\s\S]*?\);/);
    const rightClickStart = windowInput.indexOf("} else if right");
    const rightClickEnd = windowInput.indexOf("\n        }\n    }\n\n    fn handle_pointer_move", rightClickStart);
    assert.notEqual(rightClickStart, -1);
    assert.notEqual(rightClickEnd, -1);
    const rightClickBody = windowInput.slice(rightClickStart, rightClickEnd);
    assert.match(rightClickBody, /self\.ui\.app\.phase == ClientPhase::Playing/);
    assert.match(rightClickBody, /building_placement\.is_some\(\)[\s\S]*selected_nuke_kind\.is_some\(\)[\s\S]*selected_warship_build[\s\S]*self\.clear_placement\(\)/);
    assert.match(rightClickBody, /else if !self\.move_selected_warships\(x, y\) \{\s*self\.open_map_context_menu\(x, y\);/);
    assert.match(rightClickBody, /else \{\s*self\.close_map_context_menu\(\);/);
    assert.match(windowInput, /pub\(crate\) fn poll_pointer_hold[\s\S]*?self\.open_map_context_menu\(x, y\);/);
    assert.match(indexTemplate, /<canvas id="blade" oncontextmenu="return false;"/);
    assert.doesNotMatch(hud, /addEventListener\("contextmenu"/);
    assert.doesNotMatch(webMenu, /OpenMapContextMenu/);
    assert.doesNotMatch(mapClick, /handle_secondary_click/);
});


test("map menu keeps context without inventing unavailable actions", () => {
    const openStart = mapClick.indexOf("pub(crate) fn open_map_context_menu");
    const openEnd = mapClick.indexOf("pub(crate) fn close_map_context_menu", openStart);
    const openBody = mapClick.slice(openStart, openEnd);
    assert.match(openBody, /if self\.map_menu_actions\(tile_idx\)\.is_empty\(\) \{\s*self\.show_map_menu_unavailable\(tile_idx\);\s*\}/);
    assert.match(openBody, /self\.input\.map_context_menu = Some\(MapContextMenu/);

    const renderStart = hud.indexOf("function renderMapMenu(mapMenu)");
    const renderEnd = hud.indexOf("function updateLeaderboard", renderStart);
    const renderBody = hud.slice(renderStart, renderEnd);
    assert.doesNotMatch(renderBody, /if \(!items\.length\)/);
    assert.doesNotMatch(renderBody, /mapDisabledSector|disabledCenter/);
    assert.match(renderBody, /sectors\.forEach/);
});


test("map menu keeps the radial sectors and recovered submenu actions", () => {
    assert.match(hud, /sow-hud__map-radial/);
    assert.match(hud, /function radialSectorGeometry/);
    assert.doesNotMatch(hud, /function mapDisabledSector|var radialCount = 4/);
    assert.match(hud, /if \(emoteItem\) sectors\.push/);
    assert.match(hud, /\["transfer", "fleet", "alliance"\]/);
    assert.match(hud, /index, sectors\.length/);
    assert.match(hud, /dataset\.mapGroup = group/);
    assert.match(hud, /upgrade_structure/);
    assert.match(hud, /build_warship/);
    assert.doesNotMatch(hud, /build_trade_ship/);
    assert.match(hudCss, /\.sow-hud__map-sector--build/);
    assert.match(hudCss, /\.sow-hud__map-sector--nuke/);
    assert.match(hudCss, /isolation: isolate/);
    assert.match(hudCss, /\.sow-hud__map-submenu/);
    assert.match(hudCss, /@keyframes sow-map-menu-in/);
    assert.match(hud, /label\.emoji\s*\?/);
    assert.match(hud, /mapActionContent\(icon, withDetails \? "" : actionName, withDetails \? "" : reason\)/);
    assert.match(hud, /parts\.push\(item\.level/);
    assert.match(hud, /sow-hud__buildings-strip/);
    assert.match(hudCss, /\.sow-hud__buildings-strip/);
    assert.match(hudCss, /\.sow-hud__building-btn/);
    assert.doesNotMatch(hud, /build_structure|cancel_placement/);
    assert.doesNotMatch(hudCss, /sow-hud__bld-btn|sow-hud__cancel-btn/);
    assert.doesNotMatch(hud, /mapClose|data-map-close|close_map_menu|STRATEGIC STRIKE|CONSTRUCT/);
    assert.match(hudCss, /\.sow-hud__map-sector-caption/);
    assert.doesNotMatch(hudCss, /sow-hud__map-close/);
});


test("fleet dock uses building buttons for real actions and an automatic-trade status icon", () => {
    const stripStart = hud.indexOf('<div class="sow-hud__fleet-strip"');
    const stripEnd = hud.indexOf("</div>", stripStart);
    const strip = hud.slice(stripStart, stripEnd);
    assert.match(strip, /<span class="sow-hud__building-btn sow-hud__fleet-status" data-fleet-kind="trade" role="img"/);
    assert.match(strip, /emojiIcon\("⛵", "sow-hud__building-icon"\)/);
    assert.match(strip, /class="sow-hud__building-btn" data-command="select_warship"[\s\S]*?emojiIcon\("🚢", "sow-hud__building-icon"\)/);
    assert.match(strip, /class="sow-hud__building-btn" data-command="select_nuke"[\s\S]*?emojiIcon\("🚀", "sow-hud__building-icon"\)/);
    assert.doesNotMatch(strip, /data-command="[^"]+" data-fleet-kind="trade"|<b(?:\s|>)|<small(?:\s|>)/);
    assert.match(hud, /var tradeStatus = SOW_t\("hud\.map_action_fleet"\) \+ " · " \+ tradeCount \+ "\/" \+ tradeCapacity/);
    assert.match(hud, /hudRefs\.fleetWarship\.disabled = warshipDisabled;[\s\S]*hudRefs\.fleetWarship\.setAttribute\("aria-disabled", String\(warshipDisabled\)\)[\s\S]*hudRefs\.fleetWarship\.setAttribute\("aria-pressed"/);
    assert.match(hud, /hudRefs\.fleetNuke\.disabled = nukeDisabled;[\s\S]*hudRefs\.fleetNuke\.setAttribute\("aria-disabled", String\(nukeDisabled\)\)[\s\S]*hudRefs\.fleetNuke\.setAttribute\("aria-pressed"/);
    assert.match(hud, /fleetTrade\.hidden = hideTrade[\s\S]*fleetWarship\.hidden = hideWarship[\s\S]*fleetNuke\.hidden = hideNuke/);
    assert.match(hud, /fleetStrip\.style\.display = isDeploying \|\| !hasFleetItems \? "none" : "flex"/);
    assert.match(hud, /setCampaignLock\(hudRefs\.fleetWarship, fleetUnlocks === null && warshipDisabled/);
    assert.match(hud, /warshipDisabled = warshipCampaignLocked \|\| !warshipUnlocked \|\| militaryFull \|\| gold < warshipCost/);
    assert.match(hud, /nukeDisabled = nukeCampaignLocked \|\| !nukeUnlocked \|\| !nukeReady/);
    assert.match(hud, /tradeCount = Math\.floor\(Number\(fleetPanel\.trade_ships\)/);
    assert.match(hud, /tradeCapacity = Math\.floor\(Number\(fleetPanel\.trade_capacity\)/);
    assert.doesNotMatch(hudCss, /sow-hud__fleet-btn/);
    assert.match(hudCss, /\.sow-hud__fleet-status \{ cursor: default; \}/);
    assert.match(hud, /else if \(cmd === "select_warship"\) \{\s*send\("select_warship"\)/);
    assert.match(hud, /else if \(cmd === "select_nuke"\) \{\s*send\("select_nuke"\)/);
    assert.match(webMenu, /WebMenuCommand::SelectWarship => \{[\s\S]*?self\.select_warship_build_mode\(\)/);
    assert.match(webMenu, /WebMenuCommand::SelectNuke => \{[\s\S]*?self\.select_nuke_kind/);
    assert.match(mapClick, /self\.build_ship_at\(tile_idx, sow_core::game::UnitType::Warship\)/);
    assert.match(mapClick, /self\.launch_nuke_at\(kind, tile_idx\)/);
    assert.doesNotMatch(mapClick, /BuildTradeShip|build_trade_ship/);
    assert.match(webMenu, /"warship_required_port_level": 1/);
    assert.match(webMenu, /"nuke_required_city_level": 1/);
    assert.match(campaignEngine, /dock_trade_ship: '#sow-hud \[data-fleet-kind="trade"\]'/);
    assert.doesNotMatch(campaignEngine, /map_build_trade_ship/);
    assert.doesNotMatch(campaignEditorHtml, /data-map-action="build_trade_ship"/);
    assert.match(fs.readFileSync(path.join(shell, "../../sow-core/src/building/construction.rs"), "utf8"), /fn passive_trade_ships_spawn_without_manual_purchase_and_choose_another_route/);
});


test("radial actions restore emoji, visible translated labels, and all alliance states", () => {
    const configStart = hud.indexOf("var mapActionLabels =");
    const configEnd = hud.indexOf("var buildMenuActions", configStart);
    const config = vm.runInNewContext(hud.slice(configStart, configEnd) + "({ mapActionLabels, allianceActionLabels });");
    for (const [action, emoji] of Object.entries({
        spawn: "🌱", attack: "⚔️", fleet: "⛵", transfer: "📦", alliance: "🤝", nuke: "🚀"
    })) assert.equal(config.mapActionLabels[action].emoji, emoji);
    for (const [state, key] of Object.entries({
        request: "hud.map_action_request_alliance",
        accept: "hud.map_action_accept_alliance",
        pending: "hud.map_action_alliance_pending",
        active: "hud.map_action_alliance_active",
        renew: "hud.map_action_renew_alliance"
    })) assert.equal(config.allianceActionLabels[state].key, key);
    assert.deepEqual(Object.keys(config.allianceActionLabels).sort(), ["accept", "active", "pending", "renew", "request"]);
    assert.match(webMenu, /map_menu_alliance_state\(menu\.tile_idx\)/);
    assert.match(webMenu, /"alliance_state": alliance_state/);
    assert.doesNotMatch(hud, /has_alliance_request|has_proposed_alliance|alliance_requests/);

    function element() {
        return {
            dataset: {}, children: [], attributes: {}, style: {},
            appendChild(child) { this.children.push(child); return child; },
            setAttribute(name, value) { this.attributes[name] = String(value); },
            getAttribute(name) { return this.attributes[name] ?? null; }
        };
    }
    const buttonSource = hud.slice(hud.indexOf("function mapActionContent("), hud.indexOf("function updateDisabledMapActionReasons", hud.indexOf("function mapActionContent(")));
    const renderButton = vm.runInNewContext(buttonSource + "\nmapActionButton;", {
        document: { createElement: element },
        mapActionLabels: config.mapActionLabels,
        mapActionContent: undefined,
        mapActionReason: item => item.reason_key ? "Unavailable." : "",
        mapLabel: action => action,
        SOW_t: key => "translated:" + key,
        emojiIcon: (emoji, cls) => `<span class="${cls} is-emoji">${emoji}</span>`,
        hudIcon: () => "",
        buildingIcon: () => "",
        hudState: null
    });
    const accepted = renderButton(
        { action: "alliance" }, "sector", false, config.allianceActionLabels.accept
    );
    assert.equal(accepted.disabled, false);
    assert.equal(accepted.getAttribute("aria-label"), "translated:hud.map_action_accept_alliance");
    assert.match(accepted.children[0].children[0].innerHTML, /🤝/);
    assert.equal(accepted.children[0].children[1].textContent, "translated:hud.map_action_accept_alliance");
    const pending = renderButton(
        { action: "alliance" }, "sector", false, config.allianceActionLabels.pending
    );
    assert.equal(pending.disabled, true);
    assert.equal(pending.getAttribute("aria-disabled"), "true");
    assert.equal(pending.children[0].children[1].textContent, "translated:hud.map_action_alliance_pending");
    assert.match(hudCss, /\.sow-hud__map-sector:focus-visible/);
    assert.match(hudCss, /\.sow-hud__map-sector:active/);
});


test("only the confirmed center attack can break an alliance", () => {
    const allianceStart = mapClick.indexOf("    fn alliance_from_tile(");
    const allianceEnd = mapClick.indexOf("    fn map_target(", allianceStart);
    const allianceAction = mapClick.slice(allianceStart, allianceEnd);
    assert.doesNotMatch(allianceAction, /BreakAlliance/);
    assert.match(allianceAction, /target\.alliance_intent\(\)/);
    const allianceIntentStart = mapClick.indexOf("    fn alliance_intent(self)");
    const allianceIntentEnd = mapClick.indexOf("    fn is_friendly(self)", allianceIntentStart);
    const allianceIntent = mapClick.slice(allianceIntentStart, allianceIntentEnd);
    assert.doesNotMatch(allianceIntent, /BreakAlliance/);
    assert.match(allianceIntent, /State::Accept => Some\(GameplayIntent::AcceptAlliance/);
    assert.match(allianceIntent, /State::Request \| State::Renew => Some\(GameplayIntent::ProposeAlliance/);
    assert.match(allianceIntent, /State::Pending \| State::Active => None/);

    const attackStart = mapClick.indexOf("    fn attack_from_tile(");
    const attackEnd = mapClick.indexOf("    pub(crate) fn launch_fleet_from_tile", attackStart);
    assert.match(mapClick.slice(attackStart, attackEnd), /if target\.is_allied[\s\S]*?show_betrayal_warning/);
    const confirmStart = webMenu.indexOf("WebMenuCommand::ConfirmBetrayal =>");
    const cancelStart = webMenu.indexOf("WebMenuCommand::CancelBetrayal =>", confirmStart);
    const confirm = webMenu.slice(confirmStart, cancelStart);
    const cancelEnd = webMenu.indexOf("WebMenuCommand::CancelAttack", cancelStart);
    const cancel = webMenu.slice(cancelStart, cancelEnd);
    assert.match(confirm, /GameplayIntent::BreakAlliance[\s\S]*?self\.send_intent\(intent\)/);
    assert.doesNotMatch(cancel, /send_intent|BreakAlliance/);
});


test("building menus use generated pixel-art sprites with emoji fallback", () => {
    const iconStart = hud.indexOf("var BUILDING_EMOJIS = {");
    const iconEnd = hud.indexOf("function leaderById(id)", iconStart);
    const artManifest = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/gameplay/buildings/atlas.json"), "utf8"));
    const buildingIcon = vm.runInNewContext(hud.slice(iconStart, iconEnd) + "\nbuildingIcon;", {
        window: { SOW_BUILDING_ART: artManifest, SOW_ASSETS_URL: "/assets" },
        asset: (name) => "/assets/" + name,
        escapeHudText: (value) => String(value),
    });
    const expected = { City: "🏕️", Factory: "🛠️", Port: "⚓", Bunker: "👁️" };
    for (const [kind, emoji] of Object.entries(expected)) {
        assert.equal(buildingIcon(kind), '<span class="sow-hud__building-icon is-emoji" aria-hidden="true">' + emoji + "</span>");
        assert.ok(buildingOverlaySource.includes('(BuildingKind::' + kind + ', 1) => "' + emoji + '"'));
    }
    for (const level of [1, 2, 3]) {
        const icon = buildingIcon("Farm", level);
        const sprite = artManifest.sprites.Farm[String(level)];
        assert.match(icon, /class="sow-hud__building-icon is-pixel-art"/);
        assert.match(icon, /assets\/gameplay\/buildings\/atlas\.webp/);
        const x = (sprite.x * 100 / (artManifest.atlas_width - artManifest.cell_size)).toFixed(4);
        const y = (sprite.y * 100 / (artManifest.atlas_height - artManifest.cell_size)).toFixed(4);
        assert.ok(icon.includes("background-position:" + x + "% " + y + "%"));
        assert.equal(sprite.width, 64);
        assert.equal(sprite.height, 64);
        assert.match(sprite.sha256, /^[a-f0-9]{64}$/);
    }
    assert.equal(buildingIcon("Farm", 4), '<span class="sow-hud__building-icon is-emoji" aria-hidden="true">🌱</span>');
    assert.match(buildingOverlaySource, /building_sprite_uv/);
    assert.match(hudCss, /\.sow-hud__building-icon\.is-pixel-art[\s\S]*?image-rendering: pixelated/);
    assert.match(hud, /group: "build", icon: emojiIcon\("🏗️", "sow-hud__map-action-icon"\)/);
    assert.doesNotMatch(hud, /mapDisabledSector/);
    assert.match(hud, /heading\.innerHTML = mapMenuView === "emote" \? hudIcon\("emote"\) : emojiIcon\("🏗️", "sow-hud__map-action-icon"\)/);
    assert.match(hud, /var buildingImg = buildingKind \? buildingIcon\(buildingKind\) : ""/);
    assert.match(hud, /buildingIcon\(detail\.kind, detail\.level\)/);
    assert.match(hudCss, /\.sow-hud__building-icon[\s\S]*?font-size: 30px/);
    assert.match(hudCss, /\.sow-hud__map-action-icon\.is-emoji/);

    const kinds = [...hud.matchAll(/data-command="select_building" data-kind="(City|Factory|Port|Bunker|Farm)"/g)]
        .map((match) => match[1]);
    assert.deepEqual(kinds, ["City", "Factory", "Port", "Bunker", "Farm"]);
    assert.match(hud, /send\("select_building", \{ kind: btn\.dataset\.kind \}\)/);
    assert.match(webMenu, /SelectBuilding\s*\{\s*kind: sow_core::game::BuildingKind/);
    assert.match(webMenu, /WebMenuCommand::SelectBuilding\s*\{\s*kind\s*\}\s*=>\s*\{\s*if !self\.ui\.tutorial_camera_only\s*\{\s*self\.select_building_kind\(kind\)/);
    assert.match(windowInput, /fn rebase_single_touch_drag/);
    assert.match(windowInput, /\} else if is_touch \{[\s\S]*?camera_x \+=/);
    assert.doesNotMatch(windowInput, /self\.input\.dragging && !is_touch/);
    assert.match(mapClick, /MapMenuAction::BuildCity[\s\S]*?self\.select_building_kind\(kind\);\s*self\.move_building_preview\(col, row\)/);
    assert.doesNotMatch(hud, /cancel_placement/);
});


test("mobile hover is Rust-owned and the HUD only presents its state", () => {
    assert.match(hud, /var hov = hud\.hovered/);
    assert.match(hud, /hoverCard\.classList\.remove\("hidden"\)/);
    assert.match(hud, /hoverCard\.classList\.add\("hidden"\)/);
    assert.doesNotMatch(hud, /addEventListener\("(?:touchstart|touchmove|pointermove)"/);
    assert.match(hud, /hudRefs\.slider\.addEventListener\("pointerdown"/);
});


test("army allocation starts centered and only formats send/keep while shown", () => {
    assert.match(hudStateSource, /attack_ratio: 0\.5/);
    assert.match(hud, /type="range" min="5" max="100" value="50"/);
    assert.doesNotMatch(hud, /allocationHideTimer|setTimeout\([^\n]*allocation/);
    assert.match(hud, /detailsVisible = forceDetails \|\|[\s\S]*?if \(!detailsVisible\) return;[\s\S]*?SOW_t\("hud\.attack_allocation"/);
    assert.match(hud, /pointerup[\s\S]*?remove\("is-adjusting"\)/);
    assert.match(hudCss, /writing-mode: vertical-lr;\s*direction: rtl;/);
    assert.match(hudCss, /\.sow-hud__range-vertical \{[^}]*width: 32px;\s*height: 100%/);
    assert.doesNotMatch(hudCss, /\.sow-hud__range-vertical \{ width: (?:58|76|88|92)px/);
    assert.match(hudCss, /range-vertical::-webkit-slider-runnable-track \{ width: 12px/);
    assert.match(hudCss, /range-vertical::-moz-range-track \{ width: 12px/);
    assert.match(hudCss, /range-vertical::-webkit-slider-thumb/);
    assert.match(hudCss, /range-vertical::-moz-range-thumb/);
    const allocationRail = hud.slice(hud.indexOf("'<aside class=\"sow-hud__left-rail\""), hud.indexOf("'<aside class=\"sow-hud__right-rail\""));
    assert.doesNotMatch(allocationRail, /hudIcon\("troops", "sow-hud__action-icon"\)/);
    assert.match(hud, /slider\.style\.setProperty\("--sow-crossed-swords", 'url\("' \+ asset\(HUD_ICONS\.troops\)/);
    assert.match(hudCss, /background: #10181d var\(--sow-crossed-swords\) center/);
});


test("campaign players use the active leader name in both map and leaderboard", () => {
    assert.match(matchStartSource, /name: \{\s*if tutorial \{\s*leader\.name\(\)\.to_string\(\)/);
    const leaderboard = hud.slice(hud.indexOf("function updateLeaderboard"), hud.indexOf("function createNotificationCard"));
    assert.match(leaderboard, /var displayName = player\.name \|\| SOW_t\("hud\.player_name"\)/);
    assert.doesNotMatch(leaderboard, /campaignActive|leaderById/);
});


test("terms dialog smoothly focuses the requesting entity without changing zoom", () => {
    assert.match(actionsSource, /UiAction::CenterCamera[\s\S]*?camera_focus_target = Some\(\(world_cx, world_cy\)\);[\s\S]*?target_zoom = 10\.0/);
    assert.match(webMenu, /WebMenuCommand::FocusWorld \{ x, y \}[\s\S]*?let tutorial_focus = self\.ui\.tutorial_active && self\.net\.is_offline;[\s\S]*?target_zoom = self\.input\.camera_zoom;\s*self\.input\.camera_focus_target = Some\(\(x, y\)\);\s*self\.input\.tutorial_camera_focus = tutorial_focus/);
    assert.match(cameraFrameUiSource, /let \(camera, zoom, target\) = focus_camera_step\([\s\S]*?self\.input\.camera_x = camera\.0;[\s\S]*?self\.input\.camera_zoom = zoom/);
    const focusStepStart = cameraFrameUiSource.indexOf("fn focus_camera_step(");
    const focusStepEnd = cameraFrameUiSource.indexOf("\nimpl SowApp", focusStepStart);
    const focusStep = cameraFrameUiSource.slice(focusStepStart, focusStepEnd);
    assert.match(focusStep, /let current_center = \([\s\S]*?camera\.0\) \/ camera_zoom[\s\S]*?camera\.1\) \/ camera_zoom/);
    assert.match(focusStep, /screen\.0 \* 0\.5 - center\.0 \* zoom[\s\S]*?screen\.1 \* 0\.5 - center\.1 \* zoom/);
    assert.match(webMenu, /WebMenuCommand::SetDialogBorderHighlight \{ player_id \} => \{[\s\S]*?dialog_border_highlight = player_id/);
    assert.match(tutorial, /send\("set_dialog_border_highlight", \{ player_id: highlightId \}\)/);
    assert.match(tutorial, /send\("set_dialog_border_highlight", \{ player_id: null \}\)/);
    assert.match(appStateSource, /pub struct DialogBorderHighlight \{[\s\S]*?pub player_id: u16,[\s\S]*?pub start_time: web_time::Instant,/);
    assert.match(frameSource, /let (?:mut )?dialog_border = match \(&self\.ui\.dialog_border_highlight, snapshot\)/);
    assert.match(mapShader, /fn dialog_border_glow_color\(world_pos: vec2<f32>, age: f32\)/);
    assert.match(mapShader, /dialog_border\.x > 0\.0 && owner_id == u32\(globals\.dialog_border\.x\)/);
});


test("tutorial hand uses the map radial action's exact icon anchor", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(hud, /button\.dataset\.tutorialAnchorX = String\(anchorX\)/);
    assert.match(hud, /button\.dataset\.tutorialAnchorY = String\(anchorY\)/);
    assert.match(tutorial, /SOWCampaign\.resolveUiAnchor\(source, spotlightPanel\)/);
    assert.match(tutorial, /SOWCampaign\.resolveUiAnchor\(destination\)/);
    assert.match(campaignEditor, /SOWCampaign\.resolveUiAnchor\(target, spotlightPanel\)/);
    assert.match(campaignEditor, /SOWCampaign\.resolveUiAnchor\(end\)/);
    assert.match(campaignEditor, /Number\.isFinite\(anchor\.spotlightX\)\) anchor\.spotlightX -= anchorRect\.left/);
    assert.match(campaignEditor, /window\.addEventListener\("resize", function \(\) \{ drawWires\(\); if \(state\.machine\) paintPreview\(false\); \}/);
    const rect = (left, top, width, height) => ({ left, top, width, height });
    const icon = { getClientRects: () => [{}], getBoundingClientRect: () => rect(265, 75, 24, 28) };
    const radial = {
        dataset: { tutorialAnchorX: "75", tutorialAnchorY: "25" },
        getBoundingClientRect: () => rect(100, 50, 200, 100),
        querySelector: (selector) => selector === ".sow-hud__map-action-title" ? icon : null
    };
    assert.deepEqual(campaign.resolveUiAnchor(radial), { x: 250, y: 75, width: 24, height: 28 });
    assert.deepEqual(campaign.resolveUiAnchor({ getBoundingClientRect: () => rect(10, 20, 40, 60) }), { x: 30, y: 50, width: 40, height: 60 });
    const send = { getBoundingClientRect: () => rect(100, 100, 60, 30) };
    const panel = { getBoundingClientRect: () => rect(20, 40, 300, 200) };
    assert.deepEqual(campaign.resolveUiAnchor(send, panel), {
        x: 130, y: 115, width: 60, height: 30,
        spotlightX: 170, spotlightY: 140, spotlightWidth: 300, spotlightHeight: 200
    });
    const range = {
        min: "5", max: "100",
        ownerDocument: { defaultView: { getComputedStyle: () => ({ writingMode: "vertical-lr", direction: "rtl", getPropertyValue: () => "26px" }) } },
        getBoundingClientRect: () => rect(10, 20, 32, 106)
    };
    assert.equal(campaign.resolveUiRangeAnchor(range, 50).x, 26);
    assert.equal(Math.round(campaign.resolveUiRangeAnchor(range, 50).y * 100) / 100, 75.11, "the current range value resolves to its vertical thumb position");
    assert.deepEqual(campaign.resolveUiRangeAnchor(range, 100), { x: 26, y: 33 }, "100% resolves to the top end of the slider");
    const guide25 = campaign.resolveUiRangeGuideAnchor(range, 25, 100);
    const guide50 = campaign.resolveUiRangeGuideAnchor(range, 50, 100);
    assert.notEqual(guide25.y, guide50.y, "the hand follows the moving slider thumb");
    assert.deepEqual([guide25.toX, guide25.toY], [guide50.toX, guide50.toY], "the hand always points toward the 100% endpoint");
    assert.deepEqual(
        [guide25.spotlightX, guide25.spotlightY, guide25.spotlightWidth, guide25.spotlightHeight],
        [guide50.spotlightX, guide50.spotlightY, guide50.spotlightWidth, guide50.spotlightHeight],
        "the frame and dimming geometry stay fixed while the slider moves"
    );
});


test("fixed emotes share catalog definitions and sync translated bubbles once per runtime", () => {
    const core = fs.readFileSync(path.join(shell, "../../sow-core/src/emote.rs"), "utf8");
    const kinds = ["greet", "help", "threat", "celebrate"];
    assert.doesNotMatch(hud, /express_emoji|toggle_emoji|emojiPickerOpen|var EMOJIS/);
    assert.match(webMenu, /SetEmoteTexts/);
    assert.match(core, /enum EmoteKind/);
    const commands = [];
    const context = {
        window: { SOW_LOCALE: "en", SOW_menu_command: function () {} },
        SOW_t: key => "en:" + key,
        send: (type, payload) => { commands.push({ type, payload }); return true; },
        emoteTextKey: "", emoteTextTarget: null, hudSafeArea: [0, 0, 0, 0]
    };
    const start = hud.indexOf("function syncEmoteTexts(");
    const end = hud.indexOf("function setHudPanel(", start);
    const sync = vm.runInNewContext(hud.slice(start, end) + "syncEmoteTexts;", context);
    const state = { emotes: kinds.map(kind => ({ bubble_key: "hud.emote_" + kind + "_message" })) };
    sync(state); sync(state);
    assert.equal(commands.length, 1);
    assert.equal(commands[0].type, "set_emote_texts");
    context.window.SOW_LOCALE = "ar";
    context.SOW_t = key => "ar:" + key;
    sync(state);
    assert.equal(commands.length, 2);
    assert.ok(commands[1].payload.texts.every(text => text.startsWith("ar:")));
    context.window.SOW_menu_command = function () {};
    sync(state);
    assert.equal(commands.length, 3);
    assert.match(hud, /SOW_I18N_READY\.then/);
    assert.match(hud, /if \(!previousHud\) emoteTextKey = ""/);
    assert.match(hud, /event\.key === "Escape"/);
    assert.match(hudCss, /emote-choice[^}]*min-block-size: 44px/);
});



test("radial capabilities put Emote first and omit inapplicable sectors", () => {
    const start = hud.indexOf("var sectors = [];");
    const end = hud.indexOf("menu.appendChild(radial);", start);
    const source = hud.slice(start, end);
    function render(items, buildItems = []) {
        const calls = [];
        vm.runInNewContext(source, {
            items, buildItems, nukeItem: items.find(item => item.action === "nuke"), radial: {},
            hudIcon: name => name, emojiIcon: value => value, SOW_t: key => key,
            mapMenu: { alliance_state: "pending" }, allianceActionLabels: { pending: { disabled: true } },
            mapGroupSector: (radial, items, index, count, group) => calls.push({ group, index, count }),
            mapSector: (radial, item, index, count, kind, presentation) => calls.push({ kind, index, count, disabled: item.disabled, presentation })
        });
        return calls;
    }
    assert.deepEqual(render([{ action: "emote" }]), [{ group: "emote", index: 0, count: 1 }]);
    assert.deepEqual(render([{ action: "emote" }, { action: "build_city" }], [{ action: "build_city" }]), [
        { group: "emote", index: 0, count: 2 }, { group: "build", index: 1, count: 2 }
    ]);
    const enemy = render([{ action: "fleet" }, { action: "alliance", disabled: true }, { action: "nuke" }]);
    assert.deepEqual(enemy.map(item => item.kind), ["fleet", "alliance", "nuke"]);
    assert.equal(enemy[1].disabled, true);
    assert.equal(enemy[1].presentation.disabled, true);
    assert.deepEqual(render([]), []);
});


test("placement controls follow the building, clamp in both orientations and prevent pending duplicates", () => {
    const panel = { hidden: true, offsetWidth: 270, offsetHeight: 128, style: {} };
    const hudRefs = { placement: panel, placementInfo: {}, placementReason: {}, placementConfirm: {}, placementCancel: {} };
    const viewport = { innerWidth: 360, innerHeight: 800 };
    const begin = hud.indexOf("    function renderBuildingPlacement(");
    const end = hud.indexOf("    function positionBuildingCard(", begin);
    const render = vm.runInNewContext(hud.slice(begin, end) + "\nrenderBuildingPlacement;", {
        hudRefs, window: viewport, SOW_t: (key, values) => key + JSON.stringify(values || {})
    });
    const placement = { x: 180, y: 400, half_height: 24, cost: 100, seconds: 5, can_confirm: true, pending: false };
    for (const [width, height] of [[360,800],[800,360],[1600,900]]) {
        viewport.innerWidth = width; viewport.innerHeight = height;
        for (const [x,y] of [[1,1],[width-1,height-1],[width/2,height/2]]) {
            render({ ...placement, x, y });
            assert.equal(panel.hidden, false);
            assert.ok(parseFloat(panel.style.left) >= 8);
            assert.ok(parseFloat(panel.style.top) >= 8);
            assert.ok(parseFloat(panel.style.left) + panel.offsetWidth <= width - 8);
            assert.ok(parseFloat(panel.style.top) + panel.offsetHeight <= height - 8);
        }
    }
    const resizeStart = hud.indexOf("var onViewportChange = function () {");
    const resizeEnd = hud.indexOf("\n            };", resizeStart) + "\n            };".length;
    const resize = vm.runInNewContext(hud.slice(resizeStart, resizeEnd) + "\nonViewportChange;", {
        hudState: { hud: { building_placement: placement } }, renderBuildingPlacement: render,
        syncCompactHudBounds() {}, renderNotifications() {}, scheduleNotificationPlacement() {}
    });
    viewport.innerWidth = 360; viewport.innerHeight = 800; resize();
    const portraitTop = parseFloat(panel.style.top);
    viewport.innerWidth = 800; viewport.innerHeight = 360; resize();
    assert.ok(parseFloat(panel.style.top) < portraitTop);
    assert.ok(parseFloat(panel.style.top) + panel.offsetHeight <= 352);
    render({ ...placement, pending: true, can_confirm: false });
    assert.equal(hudRefs.placementConfirm.disabled, true);
    assert.equal(hudRefs.placementCancel.disabled, true);
    assert.match(hudRefs.placementReason.textContent, /build_pending/);
    render({ ...placement, can_confirm: false, reason: { key: "hud.build_occupied" } });
    assert.equal(hudRefs.placementConfirm.disabled, true);
    assert.equal(hudRefs.placementCancel.disabled, false);
    assert.match(hudRefs.placementReason.textContent, /build_occupied/);
    render(null); assert.equal(panel.hidden, true);
    assert.match(hudCss, /placement-actions button[\s\S]*?min-width: 48px; min-height: 48px/);
    assert.doesNotMatch(windowInput, /begin_hold_build|pump_hold_build|hold_build_active/);
    assert.doesNotMatch(mapShader, /nobuild_slots/);
    assert.match(mapShader, /textureLoad\(occupancy_texture, cell_hex, 0\)/);
});
