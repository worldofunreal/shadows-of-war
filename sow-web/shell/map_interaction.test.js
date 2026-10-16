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
    assert.match(hud, /var status = presentation && presentation\.status \|\| label\.status;/);
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
    assert.match(strip, /hudIcon\("trade_ship", "sow-hud__building-icon"\)/);
    assert.match(strip, /class="sow-hud__building-btn" data-command="select_warship"[\s\S]*?hudIcon\("warship", "sow-hud__building-icon"\)/);
    assert.match(strip, /class="sow-hud__building-btn" data-command="select_nuke"[\s\S]*?hudIcon\("nuke", "sow-hud__building-icon"\)/);
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


test("radial and HUD icons use one concept-based visual contract", () => {
    const controlsCss = fs.readFileSync(path.join(shell, "sow-controls.css"), "utf8");
    const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
    assert.match(controlsCss, /--sow-accent: #f59e0b;/);
    assert.match(controlsCss, /--sow-text-outline: \.18em var\(--sow-text-ink\);/);
    for (const css of [hudCss, storyCss]) assert.doesNotMatch(css, /233,? ?187,? ?100|#e9bb64/i);
    assert.match(hudCss, /--sow-gold: var\(--sow-accent\);/);
    assert.doesNotMatch(hudCss, /--sow-button-primary-bg:/);
    assert.match(storyCss, /--story-accent: var\(--sow-accent\);/);
    assert.match(storyCss, /\.sow-story__continue \{[^}]*background: var\(--sow-button-primary-bg\)/);
    assert.match(hudCss, /-webkit-text-stroke: var\(--sow-text-outline\)/);
    assert.match(storyCss, /\.sow-story__title,[^{]*\{ -webkit-text-stroke: var\(--sow-text-outline\)/);
    assert.match(hudCss, /\.sow-hud__status-right \{\s*flex-direction: column;\s*align-items: stretch;\s*gap: 6px;/);
    const configStart = hud.indexOf("var mapActionLabels =");
    const configEnd = hud.indexOf("var buildMenuActions", configStart);
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
        statusIcon: (kind, cls) => `<span class="${cls} sow-hud__status-icon" data-status="${kind}"></span>`,
        hudIcon: () => "",
        buildingIcon: () => "",
        hudState: null
    });
    const accepted = renderButton(
        { action: "alliance" }, "sector", false, config.allianceActionLabels.accept
    );
    assert.equal(accepted.disabled, false);
    assert.equal(accepted.getAttribute("aria-label"), "translated:hud.map_action_accept_alliance");
    assert.match(accepted.children[0].children[0].innerHTML, /data-status="allied"/);
    assert.equal(accepted.children[0].children[1].textContent, "translated:hud.map_action_accept_alliance");
    const pending = renderButton(
        { action: "alliance" }, "sector", false, config.allianceActionLabels.pending
    );
    assert.equal(pending.disabled, true);
    assert.equal(pending.getAttribute("aria-disabled"), "true");
    assert.equal(pending.children[0].children[1].textContent, "translated:hud.map_action_alliance_pending");
    assert.match(pending.children[0].children[0].innerHTML, /data-status="request"/);
    assert.match(hudCss, /\.sow-hud__map-sector:focus-visible/);
    assert.match(hudCss, /\.sow-hud__map-sector:active/);

    // Emoji render differently on every device and mix art styles; use hudIcon, statusIcon or buildingIcon.
    assert.doesNotMatch(hud, /\p{Extended_Pictographic}/u);
    assert.doesNotMatch(hud, /emojiIcon|BUILDING_EMOJIS|emoji:/);
    assert.match(hud, /var STATUS_CELLS = \{ request: 0, allied: 1, traitor: 2 \};/);
    assert.match(statusSpriteSource, /Self::Request => 0\.0,\s*Self::Allied => 1\.0,\s*Self::Traitor => 2\.0,/);
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


test("building menus use generated pixel-art sprites with white glyph fallback", () => {
    const iconStart = hud.indexOf("var HUD_ICONS = {");
    const iconEnd = hud.indexOf("function leaderById(id)", iconStart);
    const artManifest = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/gameplay/buildings/atlas.json"), "utf8"));
    const buildingIcon = vm.runInNewContext(hud.slice(iconStart, iconEnd) + "\nbuildingIcon;", {
        window: { SOW_BUILDING_ART: artManifest, SOW_ASSETS_URL: "/assets" },
        asset: (name) => "/assets/" + name,
        escapeHudText: (value) => String(value),
    });
    const glyphs = { City: "city", Factory: "factory", Port: "anchor", Bunker: "defense" };
    const glyphSource = (name) => hud.match(new RegExp("\\n        " + name + ": '([^']+)'"))[1];
    for (const [kind, glyph] of Object.entries(glyphs)) {
        const icon = buildingIcon(kind);
        assert.match(icon, /^<svg class="sow-hud__building-icon"/);
        assert.ok(icon.includes('<g class="sow-hud__glyph-ink">' + glyphSource(glyph) + "</g>"), kind);
        assert.ok(icon.includes('<g class="sow-hud__glyph-face">' + glyphSource(glyph) + "</g>"), kind);
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
    assert.match(buildingIcon("Farm", 4), /^<svg class="sow-hud__building-icon"/);
    assert.match(buildingIcon("Port", 1, "sow-hud__inline-icon"), /^<svg class="sow-hud__inline-icon"/);
    assert.match(buildingOverlaySource, /building_sprite_uv/);
    assert.match(hudCss, /\.sow-hud__building-icon\.is-pixel-art[\s\S]*?image-rendering: pixelated/);
    assert.match(hud, /group: "build", icon: hudIcon\("construction", "sow-hud__map-action-icon"\)/);
    assert.doesNotMatch(hud, /mapDisabledSector/);
    assert.match(hud, /heading\.innerHTML = mapMenuView === "emote" \? hudIcon\("emote"\) : hudIcon\("construction", "sow-hud__map-action-icon"\)/);
    assert.match(hud, /var buildingImg = buildingKind \? buildingIcon\(buildingKind\) : ""/);
    assert.match(hud, /buildingIcon\(detail\.kind, detail\.level\)/);
    assert.match(hudCss, /\.sow-hud__building-icon[\s\S]*?font-size: 30px/);
    assert.doesNotMatch(hudCss, /is-emoji/);

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
            hudIcon: name => name, statusIcon: name => name, SOW_t: key => key,
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
    const panel = { hidden: true, dataset: {}, style: {},
        get offsetWidth() { return Math.min(this.dataset.layout === "docked" ? 344 : 270, parseFloat(this.style.maxWidth) || Infinity); },
        get offsetHeight() { return this.dataset.layout === "docked" ? 112 : 128; }
    };
    const moves = ["up", "right", "down", "left"].map(direction => ({ dataset: { direction }, style: {} }));
    const hudRefs = { placement: panel, placementMoves: moves,
        placementInfo: {}, placementReason: {}, placementConfirm: {}, placementCancel: {} };
    const viewport = { innerWidth: 360, innerHeight: 800 };
    const hudSafeArea = [0, 0, 0, 0];
    const begin = hud.indexOf("    function buildingPlacementLayout(");
    const end = hud.indexOf("    function positionBuildingCard(", begin);
    const render = vm.runInNewContext(hud.slice(begin, end) + "\nrenderBuildingPlacement;", {
        hudRefs, window: viewport, hudSafeArea, SOW_t: (key, values) => key + JSON.stringify(values || {})
    });
    const placement = { x: 180, y: 400, half_height: 24, half_width: 24, cost: 100,
        seconds: 5, can_confirm: true, site_valid: true, affordable: true, pending: false,
        can_nudge: { up: true, right: true, down: true, left: true } };
    for (const [width, height] of [[320,800],[360,800],[800,360],[1600,900]]) {
        viewport.innerWidth = width; viewport.innerHeight = height;
        for (const [x,y] of [[1,1],[width-1,height-1],[width/2,height/2]]) {
            render({ ...placement, x, y });
            assert.equal(panel.hidden, false);
            assert.ok(parseFloat(panel.style.left) >= 8);
            assert.ok(parseFloat(panel.style.top) >= 8);
            assert.ok(parseFloat(panel.style.left) + panel.offsetWidth <= width - 8);
            assert.ok(parseFloat(panel.style.top) + panel.offsetHeight <= height - 8);
            if (panel.dataset.layout === "floating") moves.forEach(button => {
                assert.ok(parseFloat(button.style.left) >= 8 && parseFloat(button.style.left) + 48 <= width - 8);
                assert.ok(parseFloat(button.style.top) >= 8 && parseFloat(button.style.top) + 48 <= height - 8);
            });
        }
    }
    const resizeStart = hud.indexOf("            var onViewportChange = function () {");
    const resizeEnd = hud.indexOf("            window.addEventListener(\"resize\"", resizeStart);
    const resize = vm.runInNewContext(hud.slice(resizeStart, resizeEnd) + "\nonViewportChange;", {
        hudState: { hud: { building_placement: placement } }, renderBuildingPlacement: render,
        syncCompactHudBounds() {}, renderNotifications() {}, scheduleNotificationPlacement() {}
    });
    viewport.innerWidth = 360; viewport.innerHeight = 800; resize();
    assert.equal(panel.dataset.layout, "floating");
    viewport.innerWidth = 800; viewport.innerHeight = 360;
    render({ ...placement, x: 400, y: 180 });
    assert.equal(panel.dataset.layout, "docked", "landscape reuses one compact group");
    const surface = (left, top, width, height) => ({ getBoundingClientRect() { return { left, top, width, height, right: left + width, bottom: top + height }; } });
    hudRefs.topbar = surface(0, 0, 800, 56);
    hudRefs.dock = surface(0, 292, 800, 68);
    hudSafeArea.splice(0, 4, 3, 7, 9, 11);
    render({ ...placement, x: 400, y: 180 });
    assert.ok(parseFloat(panel.style.top) >= 64);
    assert.ok(parseFloat(panel.style.top) + panel.offsetHeight <= 284);
    assert.ok(parseFloat(panel.style.left) + panel.offsetWidth <= 368,
        "compact controls leave the selected foundation visible");
    render({ ...placement, pending: true, can_confirm: false });
    assert.equal(hudRefs.placementConfirm.disabled, true);
    assert.equal(hudRefs.placementCancel.disabled, true);
    assert.ok(moves.every(button => button.disabled));
    assert.match(hudRefs.placementReason.textContent, /build_pending/);
    render({ ...placement, can_confirm: false, reason: { key: "hud.build_occupied" },
        site_valid: false, can_nudge: { ...placement.can_nudge, up: false } });
    assert.equal(panel.dataset.siteValid, "false");
    assert.equal(hudRefs.placementConfirm.disabled, true);
    assert.equal(hudRefs.placementCancel.disabled, false);
    assert.equal(moves[0].disabled, true);
    assert.equal(moves[1].disabled, false, "invalid terrain can still be explored");
    assert.match(hudRefs.placementReason.textContent, /build_occupied/);
    render({ ...placement, affordable: false, can_confirm: false, reason: { key: "hud.need_gold" } });
    assert.equal(panel.dataset.siteValid, "true", "low gold never marks a fitting foundation invalid");
    assert.equal(panel.dataset.affordable, "false");
    assert.equal(hudRefs.placementConfirm.disabled, true);
    const commands = [];
    const state = { hud: { building_placement: placement } };
    const branchStart = hud.indexOf('                var placement = hudState', hud.indexOf('cmd === "nudge_building_placement"'));
    const branchEnd = hud.indexOf('            } else if (cmd === "confirm_building_placement")', branchStart);
    const click = vm.runInNewContext('(function(btn) {' + hud.slice(branchStart, branchEnd) + '})', {
        hudState: state, send: (type, payload) => commands.push({ type, ...payload })
    });
    moves.forEach(button => click(button));
    assert.deepEqual(commands.map(command => command.direction), ["up", "right", "down", "left"]);
    state.hud.building_placement = { ...placement, pending: true };
    click(moves[0]); assert.equal(commands.length, 4, "pending blocks repeated commands");
    const keyStart = hud.indexOf('        if (hudRefs && hudRefs.placement && !hudRefs.placement.hidden');
    const keyEnd = hud.indexOf('        if (hudRefs && hudRefs.betrayal', keyStart);
    panel.contains = () => true;
    const keydown = vm.runInNewContext('(function(event) {' + hud.slice(keyStart, keyEnd) + '})', {
        hudRefs, hudState: state, send: (type, payload) => commands.push({ type, ...payload })
    });
    state.hud.building_placement = placement;
    const key = value => ({ key: value, target: moves[0], preventDefault() {}, stopPropagation() {} });
    keydown(key("ArrowRight")); assert.equal(commands.at(-1).direction, "right");
    const beforeEnter = commands.length;
    keydown(key("Enter")); assert.equal(commands.length, beforeEnter, "Enter leaves focused button activation to the browser");
    keydown(key("Escape")); assert.equal(commands.at(-1).type, "cancel_building_mode");
    render(null); assert.equal(panel.hidden, true);
    assert.match(hudCss, /placement-actions button[\s\S]*?min-width: 48px; min-height: 48px/);
    assert.doesNotMatch(windowInput, /begin_hold_build|pump_hold_build|hold_build_active/);
    assert.doesNotMatch(mapShader, /nobuild_slots/);
    assert.match(mapShader, /textureLoad\(placement_texture, cell_hex, 0\)/);
});
