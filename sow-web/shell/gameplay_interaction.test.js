const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = __dirname;
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const campaignView = fs.readFileSync(path.join(shell, "sow-campaign-view.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const siteDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/main.rs"), "utf8");
const prodDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/prod.rs"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const intentInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/intents.rs"), "utf8");
const hudStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/hud/state.rs"), "utf8");
const simUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/update.rs"), "utf8");
const simEventsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/events/mod.rs"), "utf8");
const eliminationSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/events/elimination.rs"), "utf8");
const snapshotFxSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/snapshot_fx.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const settingsStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/settings/mod.rs"), "utf8");
const uiAppSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/app.rs"), "utf8");
const intentApplySource = fs.readFileSync(path.join(shell, "../../sow-core/src/intent/apply.rs"), "utf8");
const appStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/state.rs"), "utf8");
const mapClick = fs.readFileSync(path.join(shell, "../../sow-client/src/input/map_click.rs"), "utf8");
const frameSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/mod.rs"), "utf8");
const mapShader = fs.readFileSync(path.join(shell, "../../sow-render/src/shaders/map.wgsl"), "utf8");


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
    const desktopRules = [...hudCss.matchAll(/@media \(min-width: 721px\) \{([^}]*)\}/g)];
    assert.ok(desktopRules.some(([, rules]) => rules.includes("sow-hud__notifications")));
    // Opening any panel closes Dev Tools, so only one sheet is ever open.
    // One shared sheet layout: bottom half on phones, side panel on landscape phones and desktop, never density-scaled.
});






test("dev tools values show at most two decimals", () => {
    const start = hud.indexOf("    function devDecimals(");
    const end = hud.indexOf("\n    function devIsDirty", start);
    assert.ok(start >= 0 && end > start);
    const { devDecimals, devFormat } = vm.runInNewContext(hud.slice(start, end) + "\n({ devDecimals, devFormat });");
    assert.equal(devDecimals(0.001), 2);
    assert.equal(devDecimals(0.05), 2);
    assert.equal(devDecimals(0.5), 1);
    assert.equal(devDecimals(1), 0);
    assert.equal(devFormat(0.30000000000000004, 2), "0.30");
    assert.equal(devFormat(0.123456, 2), "0.12");
    assert.equal(devFormat(0.001, 2), "0.001");
    assert.equal(devFormat(0, 2), "0.00");
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

    const placementStart = hud.indexOf("    function syncNotificationPlacement(");
    const placementEnd = hud.indexOf("\n    function ensureHudDom", placementStart);
    assert.ok(placementStart >= 0 && placementEnd > placementStart);
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
        statusIcon: name => name,
        renderBuildingPlacement() {},
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
        + hud.slice(hud.indexOf("    function buildingPlacementLayout("), hud.indexOf("    function positionBuildingCard("))
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
        { kind: "attack", id: 8, direction: "outgoing", player_id: 0, neutral: true, troops: 200, avatar: { kind: "wilderness" } },
        { kind: "fleet", id: 9, direction: "outgoing", player_id: 3, name: "Ally", troops: 50, avatar: { kind: "portrait", slug: "queruscos" } }
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
    assert.equal(firstRows[1].children[0].children[1].innerHTML, "wilderness", "unclaimed land uses assets/art/icons/wilderness");
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
    // The gold reward is the coin sprite plus the amount; assists add their own badge.

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
        statusIcon: name => name,
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
        { kind: "portrait", slug: "ndebele" }, { kind: "portrait", slug: "queruscos" }
    ]);
    const botCard = cards.find(parts => !parts.card.hidden);
    assert.deepEqual(botCard.avatars.map(item => item.emblem.hidden), [true, true]);
    assert.deepEqual(botCard.avatars.map(item => item.image.hidden), [false, false]);
    assert.deepEqual(botCard.avatars.map(item => item.image.src), [
        "/assets/gameplay/avatars/ndebele.webp",
        "/assets/gameplay/avatars/queruscos.webp"
    ]);
    assert.match(botCard.avatars[1].wrapper.className, /--overlap/);

    add(109, "hud.nuke_struck", 4, "single-avatar", {}, [
        null, { kind: "portrait", slug: "nationbot_hatshepsut" }
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
    add(115, "hud.betrayal", 4, "betrayal:8", {}, [null, { kind: "portrait", slug: "ndebele" }]);
    assert.equal(context.activeNotifications[0].entry.id, 115);
    assert.strictEqual(cards.find(parts => !parts.card.hidden), recycledCard, "the next event recycles the same DOM card");
    assert.deepEqual(recycledCard.avatars.map(item => item.wrapper.hidden), [true, false]);
    assert.equal(recycledCard.avatars[1].image.src, "/assets/gameplay/avatars/ndebele.webp");

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
        statusIcon: name => name,
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
                { kind: "portrait", slug: "ndebele" }
            ]
        },
        priority: 4
    }, false);
    assert.equal(container.children.length, 1);
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/boudica.webp");
    assert.equal(parts.avatars[0].emblem.hidden, true);
    assert.equal(parts.avatars[1].emblem.hidden, true);
    assert.equal(parts.avatars[1].image.src, "/assets/gameplay/avatars/ndebele.webp");
    assert.match(parts.avatars[1].wrapper.className, /--overlap/);

    parts.avatars[0].image.onerror.call(parts.avatars[0].image);
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/null.webp");

    context.renderAvatarIdentity(parts.avatars[0].image, parts.avatars[0].emblem, { kind: "fallback" });
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/null.webp");
    assert.equal(parts.avatars[0].image.hidden, false);

    context.renderNotificationCard(parts, {
        entry: { id: 2, key: "hud.alliance_formed", values: {}, avatars: [{ kind: "portrait", slug: "nationbot_hatshepsut" }] },
        priority: 2
    }, false);
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/nationbot_hatshepsut.webp");
    assert.equal(parts.avatars[0].emblem.hidden, true);
    assert.equal(parts.avatars[1].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "");

    context.renderNotificationCard(parts, {
        entry: { id: 3, key: "hud.nuke_struck", values: {}, avatars: [null, { kind: "wilderness" }] },
        priority: 4
    }, false);
    assert.equal(parts.avatars[0].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.hidden, false, "unclaimed land shows its icon");
    assert.doesNotMatch(parts.avatars[1].wrapper.className, /--overlap/);

    context.clearNotificationCard(parts);
    assert.equal(parts.avatars[0].wrapper.hidden, true);
    assert.equal(parts.avatars[1].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "");
});
