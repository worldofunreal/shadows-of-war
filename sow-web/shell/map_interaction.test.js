const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = __dirname;
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const hudStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/hud/state.rs"), "utf8");
const simUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/update.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const appStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/state.rs"), "utf8");
const mapClick = fs.readFileSync(path.join(shell, "../../sow-client/src/input/map_click.rs"), "utf8");
const frameSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/mod.rs"), "utf8");
const mapShader = fs.readFileSync(path.join(shell, "../../sow-render/src/shaders/map.wgsl"), "utf8");
const campaignEngine = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
const campaignEditor = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.js"), "utf8");
const campaignEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"), "utf8");
const statusSpriteSource = fs.readFileSync(path.join(shell, "../../sow-render/src/text/types.rs"), "utf8");
const indexTemplate = fs.readFileSync(path.join(shell, "index.html.template"), "utf8");
const windowInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/window.rs"), "utf8");
const matchStartSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/match_start.rs"), "utf8");
const actionsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/interact/actions.rs"), "utf8");
const cameraFrameUiSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/ui.rs"), "utf8");
const buildingOverlaySource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/world/overlays.rs"), "utf8");


// Loads the command-ring helpers (ringSlots … ringActivate) with stubbed HUD dependencies.
function loadRing(overrides = {}) {
    const start = hud.indexOf("    var RING_SLOTS = 6;");
    const end = hud.indexOf("    function renderMapMenu(mapMenu) {", start);
    const labelsStart = hud.indexOf("var mapActionLabels =");
    const labelsEnd = hud.indexOf("function mapLabel(action)", labelsStart);
    const context = Object.assign({
        mapMenuView: "root",
        ring: { hot: -1, slots: [], press: null, hold: null, holdConfirmed: false },
        hudState: { hud: {} },
        SOW_t: key => key,
        mapLabel: action => "label:" + action,
        mapActionReason: item => item.reason_key ? "reason:" + item.reason_key : "",
        hudIcon: name => "<icon:" + name + ">",
        statusIcon: name => "<status:" + name + ">",
        buildingIcon: kind => "<building:" + kind + ">",
        escapeHudText: value => String(value),
        leaderById: () => ({ slug: "caesar" }),
        asset: value => value,
        performance: { now: () => context.now || 0 },
        Math, Number, String, Array, Boolean
    }, overrides);
    vm.runInNewContext(hud.slice(labelsStart, labelsEnd) + hud.slice(start, end), context);
    return context;
}



test("building actions choose the selected tile, filter invalid placement, and allow cancel", () => {
    const actionsStart = mapClick.indexOf("pub(crate) fn map_menu_actions");
    const actionsEnd = mapClick.indexOf("pub(crate) fn map_menu_items", actionsStart);
    const actionsBody = mapClick.slice(actionsStart, actionsEnd);
    assert.match(actionsBody, /resolve_building_target\(kind, col, row\)/);

    const actionStart = mapClick.indexOf("pub(crate) fn handle_map_menu_action");
    const actionEnd = mapClick.indexOf("pub(crate) fn move_selected_warships", actionStart);
    const actionBody = mapClick.slice(actionStart, actionEnd);
    assert.match(actionBody, /BuildingPlacement \{\s*kind,\s*tile_idx,\s*pending: false,[\s\S]*?self\.confirm_building_placement\(\)/);
    assert.doesNotMatch(actionBody, /select_building_kind|move_building_preview/);
    assert.doesNotMatch(actionBody, /GameplayIntent::BuildStructure/);

    const selectionStart = mapClick.indexOf("pub(crate) fn select_building_kind");
    const selectionEnd = mapClick.indexOf("pub(crate) fn select_warship_build_mode", selectionStart);
    const selectionBody = mapClick.slice(selectionStart, selectionEnd);
    assert.match(selectionBody, /building_placement\s*\.is_some_and\(\|p\| p\.kind == kind\)\s*\{\s*self\.clear_placement\(\)/);
});


test("building panel is drawn by the game: one-tap upgrade, radial opens it, seconds while building", () => {
    const actionStart = mapClick.indexOf("pub(crate) fn handle_map_menu_action");
    const actionEnd = mapClick.indexOf("pub(crate) fn move_selected_warships", actionStart);
    const actionBody = mapClick.slice(actionStart, actionEnd);
    assert.ok(actionBody.indexOf("self.current_player_gold() < cost") < actionBody.indexOf("GameplayIntent::UpgradeStructure"));
    assert.ok(actionBody.indexOf("MapContextMenuView::Radial") < actionBody.indexOf("GameplayIntent::UpgradeStructure"), "the radial opens the panel");
    assert.ok(actionBody.trimEnd().endsWith("if !keep_building_panel_open {\n            self.close_map_context_menu();\n        }\n    }"));
    const selectBody = mapClick.slice(mapClick.indexOf("fn select_owned_building"), mapClick.indexOf("pub(crate) fn open_map_context_menu"));
    assert.doesNotMatch(selectBody, /under_construction/, "buildings under construction open the panel too");
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


test("primary click selects owned buildings without changing other map gestures", () => {
    const clickStart = mapClick.indexOf("pub(crate) fn handle_map_click");
    const clickEnd = mapClick.indexOf("pub(crate) fn open_map_context_menu", clickStart);
    const clickBody = mapClick.slice(clickStart, clickEnd);
    assert.ok(clickBody.indexOf("selected_nuke_kind") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("building_placement.is_some()") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("select_warships_at") < clickBody.indexOf("select_owned_building"));
    assert.match(clickBody, /select_owned_building\(x, y\)[\s\S]*?primary_target\(tile_idx/);

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
});


test("WASM right-click opens on pointer press; touch hold stays intact", () => {
    const rightClickStart = windowInput.indexOf("} else if right");
    const rightClickEnd = windowInput.indexOf("\n        }\n    }\n\n    fn handle_pointer_move", rightClickStart);
    assert.notEqual(rightClickStart, -1);
    assert.notEqual(rightClickEnd, -1);
    const rightClickBody = windowInput.slice(rightClickStart, rightClickEnd);
    assert.match(rightClickBody, /self\.ui\.app\.phase == ClientPhase::Playing/);
    assert.match(rightClickBody, /building_placement\.is_some\(\)[\s\S]*selected_nuke_kind\.is_some\(\)[\s\S]*selected_warship_build[\s\S]*self\.clear_placement\(\)/);
    assert.match(rightClickBody, /else if !self\.move_selected_warships\(x, y\) \{\s*self\.open_map_context_menu\(x, y\);/);
    assert.match(rightClickBody, /else \{\s*self\.close_map_context_menu\(\);/);
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
    assert.match(renderBody, /ringSlots\(mapMenu, items, hud\)/);
});





test("fleet dock uses building buttons for real actions and an automatic-trade status icon", () => {
    const stripStart = hud.indexOf('<div class="sow-hud__fleet-strip"');
    const stripEnd = hud.indexOf("</div>", stripStart);
    const strip = hud.slice(stripStart, stripEnd);
    assert.match(strip, /<span class="sow-hud__building-btn sow-hud__fleet-status" data-fleet-kind="trade" role="img"/);
    assert.match(strip, /hudIcon\("trade_ship", "sow-hud__building-icon"\)/);
    assert.match(strip, /class="sow-hud__building-btn" data-command="select_warship"[\s\S]*?hudIcon\("warship", "sow-hud__building-icon"\)/);
    assert.match(strip, /class="sow-hud__building-btn" data-command="select_nuke"[\s\S]*?hudIcon\("nuke", "sow-hud__building-icon"\)/);
    assert.doesNotMatch(strip, /data-command="[^"]+" data-fleet-kind="trade"|<b(?:\s|>)|<small(?:\s|>)/);
});


test("radial and HUD icons use one concept-based visual contract", () => {
    const configStart = hud.indexOf("var mapActionLabels =");
    const configEnd = hud.indexOf("function mapLabel(action)", configStart);
    const config = vm.runInNewContext(hud.slice(configStart, configEnd) + "({ mapActionLabels, allianceActionLabels });");
    for (const [action, icon] of Object.entries({
        spawn: "spawn", attack: "attack", fleet: "fleet", transfer: "transfer", nuke: "nuke"
    })) assert.equal(config.mapActionLabels[action].icon, icon);
    assert.equal(config.mapActionLabels.alliance.status, "allied");
    for (const [state, status] of Object.entries({
        request: "allied", accept: "allied", pending: "request", active: "allied", renew: "allied"
    })) assert.equal(config.allianceActionLabels[state].status, status);
    for (const [state, key] of Object.entries({
        request: "hud.map_action_request_alliance",
        accept: "hud.map_action_accept_alliance",
        pending: "hud.map_action_alliance_pending",
        active: "hud.map_action_alliance_active",
        renew: "hud.map_action_renew_alliance"
    })) assert.equal(config.allianceActionLabels[state].key, key);
    assert.deepEqual(Object.keys(config.allianceActionLabels).sort(), ["accept", "active", "pending", "renew", "request"]);

    const ringContext = loadRing({ SOW_t: key => "translated:" + key });
    const accepted = ringContext.ringSlots({ alliance_state: "accept" }, [{ action: "alliance" }], {});
    const pending = ringContext.ringSlots({ alliance_state: "pending" }, [{ action: "alliance" }], {});
    assert.equal(ringContext.ringSlotDisabled(accepted[3]), false);
    assert.equal(ringContext.ringSlotDisabled(pending[3]), true);
    assert.equal(pending[3].presentation.status, "request");
    const readout = { firstChild: { textContent: "" }, lastChild: { innerHTML: "", hidden: false }, dataset: {} };
    const button = {
        disabled: false, dataset: {}, lastChild: { innerHTML: "" }, attributes: {},
        setAttribute(name, value) { this.attributes[name] = String(value); },
        getAttribute(name) { return this.attributes[name] ?? null; }
    };
    const menu = { querySelector: selector => selector === ".sow-hud__ring-readout" ? readout : selector.startsWith(".sow-hud__ring-slot") ? button : null };
    ringContext.ring.hot = 3;
    ringContext.patchRing(menu, accepted, 0);
    assert.equal(button.getAttribute("aria-label"), "translated:hud.map_action_accept_alliance");
    assert.equal(readout.firstChild.textContent, "translated:hud.map_action_accept_alliance");
    ringContext.patchRing(menu, pending, 0);
    assert.equal(button.disabled, true);
    assert.equal(button.getAttribute("aria-disabled"), "true");
    assert.equal(button.getAttribute("aria-label"), "translated:hud.map_action_alliance_pending");
    assert.equal(readout.firstChild.textContent, "translated:hud.map_action_alliance_pending");

    // Emoji render differently on every device and mix art styles; use hudIcon, statusIcon or buildingIcon.
    const badgeAtlas = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/generated/badges-atlas.json"), "utf8"));
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
    const confirmStart = webMenu.indexOf("WebMenuCommand::ConfirmBetrayal =>");
    const cancelStart = webMenu.indexOf("WebMenuCommand::CancelBetrayal =>", confirmStart);
    const confirm = webMenu.slice(confirmStart, cancelStart);
    const cancelEnd = webMenu.indexOf("WebMenuCommand::CancelAttack", cancelStart);
    const cancel = webMenu.slice(cancelStart, cancelEnd);
    assert.match(confirm, /GameplayIntent::BreakAlliance[\s\S]*?self\.send_intent\(intent\)/);
    assert.doesNotMatch(cancel, /send_intent|BreakAlliance/);
});


test("building menus use generated pixel-art sprites with white glyph fallback", () => {
    const iconStart = hud.indexOf("var HUD_ICONS = {");
    const iconEnd = hud.indexOf("function leaderById(id)", iconStart);
    const artManifest = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/generated/buildings-atlas.json"), "utf8"));
    const load = (art) => vm.runInNewContext(hud.slice(iconStart, iconEnd) + "\nbuildingIcon;", {
        window: { SOW_BUILDING_ART: art, SOW_ASSETS_URL: "/assets" },
        asset: (name) => "/assets/" + name,
        escapeHudText: (value) => String(value),
    });
    const buildingIcon = load(artManifest);
    for (const kind of ["City", "Factory", "Port", "Bunker", "Farm"]) {
        for (const level of [1, 2, 3, 4]) {
            const icon = buildingIcon(kind, level);
            const sprite = artManifest.sprites[kind][String(level)];
            assert.match(icon, /class="sow-hud__building-icon is-pixel-art"/, kind + level);
            assert.match(icon, /assets\/gameplay\/buildings\/atlas\.webp/);
            const x = (sprite.x * 100 / (artManifest.atlas_width - artManifest.cell_size)).toFixed(4);
            const y = (sprite.y * 100 / (artManifest.atlas_height - artManifest.cell_size)).toFixed(4);
            assert.ok(icon.includes("background-position:" + x + "% " + y + "%"));
            assert.equal(sprite.width, 64);
            assert.match(sprite.sha256, /^[a-f0-9]{64}$/);
        }
    }
    assert.match(buildingIcon("Port", 1, "sow-hud__inline-icon"), /^<span class="sow-hud__inline-icon is-pixel-art"/);
    // Before the manifest arrives the HUD shows the white glyph, never an emoji.
    const glyphSource = (name) => hud.match(new RegExp("\\n        " + name + ": '([^']+)'"))[1];
    const withoutArt = load(undefined);
    for (const [kind, glyph] of Object.entries({ City: "city", Factory: "factory", Port: "anchor", Bunker: "defense", Farm: "farm" })) {
        assert.ok(withoutArt(kind).includes('<g class="sow-hud__glyph-face">' + glyphSource(glyph) + "</g>"), kind);
    }

    const kinds = [...hud.matchAll(/data-command="select_building" data-kind="(City|Factory|Port|Bunker|Farm)"/g)]
        .map((match) => match[1]);
    assert.deepEqual(kinds, ["City", "Factory", "Port", "Bunker", "Farm"]);
});


test("mobile hover is Rust-owned and the HUD only presents its state", () => {
    // The only pointer tracking in the HUD is the command ring's own slot selection.
    const pointerMoves = [...hud.matchAll(/addEventListener\("pointermove"/g)].map(match => match.index);
    assert.equal(pointerMoves.length, 1);
    const ringInput = hud.indexOf("function bindRingInput()");
});


test("ring center closes at the root and goes back inside submenus", () => {
    assert.match(hud, /if \(back\) back\.click\(\);\s*else send\("close_map_context_menu"\);/);
});


test("army allocation starts centered and only formats send/keep while shown", () => {
    const allocationRail = hud.slice(hud.indexOf("'<aside class=\"sow-hud__left-rail\""), hud.indexOf("'<aside class=\"sow-hud__right-rail\""));
    assert.doesNotMatch(allocationRail, /hudIcon\("troops", "sow-hud__action-icon"\)/);
});


test("campaign players use the active leader name in both map and leaderboard", () => {
    const leaderboard = hud.slice(hud.indexOf("function updateLeaderboard"), hud.indexOf("function createNotificationCard"));
    assert.match(leaderboard, /var displayName = player\.name \|\| SOW_t\("hud\.player_name"\)/);
    assert.doesNotMatch(leaderboard, /campaignActive|leaderById/);
});


test("terms dialog smoothly focuses the requesting entity without changing zoom", () => {
    const focusStepStart = cameraFrameUiSource.indexOf("fn focus_camera_step(");
    const focusStepEnd = cameraFrameUiSource.indexOf("\nimpl SowApp", focusStepStart);
    const focusStep = cameraFrameUiSource.slice(focusStepStart, focusStepEnd);
    assert.match(focusStep, /let current_center = \([\s\S]*?camera\.0\) \/ camera_zoom[\s\S]*?camera\.1\) \/ camera_zoom/);
    assert.match(focusStep, /screen\.0 \* 0\.5 - center\.0 \* zoom[\s\S]*?screen\.1 \* 0\.5 - center\.1 \* zoom/);
});


test("tutorial hand uses the map radial action's exact icon anchor", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    // Ring slots sit on the icon radius, so the hand points at the slot button's own center.
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
});



test("ring slots stay fixed per action and leave notches for inapplicable ones", () => {
    const context = loadRing();
    const ids = (items, hudState = {}) => context.ringSlots({ alliance_state: "request" }, items, hudState).map(context.ringSlotIdentity);
    const act = (...actions) => actions.map(action => ({ action }));
    // Enemy: offense on the right, diplomacy at the bottom; transfer and emote leave notches.
    assert.deepEqual(ids(act("attack", "fleet", "nuke", "alliance")), ["attack", "fleet", "nuke", "alliance", "-", "-"]);
    // Ally: same positions for shared actions.
    assert.deepEqual(ids(act("transfer", "attack", "fleet", "alliance")), ["attack", "fleet", "-", "alliance", "transfer", "-"]);
    // Teammate: a single action never becomes a 360° slice.
    assert.deepEqual(ids(act("transfer")), ["-", "-", "-", "-", "transfer", "-"]);
    // Own empty tile: Build is the primary slot, Emote keeps 300°.
    assert.deepEqual(ids(act("emote", "build_city", "build_port")), ["group:build", "-", "-", "-", "-", "group:emote"]);
    // Own building: Upgrade is direct, not a one-item submenu.
    assert.deepEqual(ids(act("emote", "upgrade_structure")), ["upgrade_structure", "-", "-", "-", "-", "group:emote"]);
    assert.deepEqual(ids(act("spawn")), ["spawn", "-", "-", "-", "-", "-"]);
    assert.deepEqual(ids([]), ["-", "-", "-", "-", "-", "-"]);

    context.mapMenuView = "build";
    assert.deepEqual(ids(act("emote", "build_city", "build_bunker", "build_farm")), ["build_city", "-", "-", "build_bunker", "build_farm", "-"]);
    context.mapMenuView = "emote";
    const emotes = [{ kind: "greet" }, { kind: "help" }, { kind: "threat" }, { kind: "celebrate" }];
    assert.deepEqual(ids([], { emotes }), ["emote:greet", "emote:celebrate", "emote:threat", "emote:help", "-", "pin"]);
    assert.deepEqual(ids([], { emotes, emote_pinned: true }), ["emote:greet", "emote:celebrate", "emote:threat", "emote:help", "clear", "pin"]);
});


test("ring resolves pointer direction, keeps live values in place, and gates the nuke behind a hold", () => {
    const radial = { getBoundingClientRect: () => ({ left: 0, top: 0, width: 200, height: 200 }) };
    const clicks = [];
    const buttons = {};
    const menu = {
        querySelector: selector => {
            if (selector === ".sow-hud__map-radial") return radial;
            const slot = /data-slot="(\d)"/.exec(selector);
            return slot && selector.startsWith(".sow-hud__ring-slot") ? buttons[slot[1]] || null : null;
        },
        querySelectorAll: () => []
    };
    const context = loadRing({ renderRingReadout: () => {} });
    const at = (x, y, unbounded) => context.ringSlotAt(menu, x, y, unbounded);
    assert.equal(at(100, 100), "hub");
    assert.equal(at(100, 10), 0);
    assert.equal(at(180, 60), 1);
    assert.equal(at(180, 140), 2);
    assert.equal(at(100, 190), 3);
    assert.equal(at(20, 140), 4);
    assert.equal(at(20, 60), 5);
    assert.equal(at(100, -50), null, "hover past the rim selects nothing");
    assert.equal(at(100, -50, true), 0, "a drag that opened the ring selects by direction");

    for (const index of ["0", "2"]) buttons[index] = { disabled: false, dataset: { slot: index }, click: () => clicks.push(index) };
    buttons["2"].dataset.ringHold = "true";
    context.ringActivate(menu, 0);
    assert.deepEqual(clicks, ["0"]);
    context.ringActivate(menu, 2);
    assert.deepEqual(clicks, ["0"], "a tap never launches a hold action");
    context.now = 1000;
    context.ringHoldStart(menu, 2);
    context.now = 1000 + 599;
    context.ringActivate(menu, 2);
    assert.deepEqual(clicks, ["0"], "releasing early cancels");
    context.ringHoldStart(menu, 2);
    context.now += 600;
    context.ringActivate(menu, 2);
    assert.deepEqual(clicks, ["0", "2"]);
    assert.equal(context.ring.holdConfirmed, false);

    // Countdown values patch the ring; only the structure key rebuilds it.
    const renderStart = hud.indexOf("function renderMapMenu(mapMenu)");
    const renderBody = hud.slice(renderStart, hud.indexOf("function openRingMenu()", renderStart));
    const keyLine = renderBody.slice(renderBody.indexOf("var renderKey"), renderBody.indexOf("if (menu.dataset.renderKey !== renderKey)"));
    assert.doesNotMatch(keyLine, /JSON\.stringify|reason_seconds|items/);
    assert.match(renderBody, /patchRing\(menu, slots, hud\.gold\)/);
});



test("build mode controls live in the world renderer, not the HTML HUD", () => {
    const clickStart = mapClick.indexOf("pub(crate) fn handle_map_click");
    const clickBody = mapClick.slice(clickStart, mapClick.indexOf("fn try_tutorial_campaign_nameplate_attack", clickStart));
    assert.ok(clickBody.indexOf("button_at(") < clickBody.indexOf("self.mouse_to_tile(x, y)"), "buttons resolve before the tile");
    // Poka-yoke: every move lands on a legal foundation; no gold or no room never enters build mode.
    const selectStart = mapClick.indexOf("pub(crate) fn select_building_kind");
    const selectBody = mapClick.slice(selectStart, mapClick.indexOf("pub(crate) fn select_warship_build_mode", selectStart));
    assert.ok(selectBody.indexOf("building_gold_shortfall(kind)") < selectBody.indexOf("nearest_structure_site("));
    assert.match(selectBody, /hud\.build_no_space/);
    const placementStart = mapShader.indexOf("if (globals.placement_scale > 0.0) {");
    const placementShader = mapShader.slice(placementStart, mapShader.indexOf("return base_color", placementStart));
    assert.match(placementShader, /let own_land = placement_own\(cell_hex\);[\s\S]*?let border_distance =[\s\S]*?let boundary = 1\.0 - smoothstep_safe/);
    assert.doesNotMatch(placementShader, /spotlight|length\(/);
});
