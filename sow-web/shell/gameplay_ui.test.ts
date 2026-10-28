import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { vm } from "./typescript-test-support.ts";
const shell = __dirname;
import { bundleTs, loadTypeScriptModule } from "./typescript-test-support.ts";
void fs.readFileSync(path.join(shell, "main_menu.tutorial.ts"), "utf8");
const tutorialBundle =
    '(function () { if (typeof window === "undefined") return; const providedView = window.SOWCampaignView;\n' +
    bundleTs(path.join(shell, "sow-campaign-view.ts")) +
    "\nif (providedView) Object.assign(window.SOWCampaignView, providedView); })();\n" +
    bundleTs(path.join(shell, "main_menu.tutorial.ts"));
void fs.readFileSync(path.join(shell, "sow-campaign-view.ts"), "utf8");
void bundleTs(path.join(shell, "sow-campaign-view.ts"));
void fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.ts"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
void fs.readFileSync(path.join(shell, "sow-campaign.ts"), "utf8");
const campaignEditor = fs.readFileSync(
    path.join(shell, "../../sow-tools/editors/campaign-editor/editor.ts"),
    "utf8",
);

test("endgame shows the five match stats with exact combat counts and compact totals on either result", () => {
    const formatterStart = hud.indexOf("    function formatEndgameExact(");
    const formatterEnd = hud.indexOf("\n    function buildingIcon(", formatterStart);
    assert.ok(formatterStart >= 0 && formatterEnd > formatterStart);
    const format = vm.runInNewContext(
        hud.slice(formatterStart, formatterEnd) + "\n({ formatEndgameExact, formatEndgameCompact });",
    );
    assert.equal(format.formatEndgameExact(1234), "1234");
    assert.equal(format.formatEndgameCompact(999), "999");
    assert.equal(format.formatEndgameCompact(1_000), "1K");
    assert.equal(format.formatEndgameCompact(1_250), "1.3K");
    assert.equal(format.formatEndgameCompact(999_950), "1M");
    assert.equal(format.formatEndgameCompact(1_500_000), "1.5M");

    const statsMarkupStart = hud.indexOf("    function formatEndgameExact(");
    const statsMarkupEnd = hud.indexOf("\n    function buildingIcon(", statsMarkupStart);
    assert.ok(statsMarkupStart >= 0 && statsMarkupEnd > statsMarkupStart);
    const helpers = vm.runInNewContext(
        hud.slice(statsMarkupStart, statsMarkupEnd) + "\n({ endgameStatsMarkup, renderEndgameStats });",
        {
            hudIcon: (name) => `<icon>${name}</icon>`,
            statusIcon: (name) => `<status>${name}</status>`,
            SOW_t: (key) => key,
        },
    );
    const cards = helpers.endgameStatsMarkup("endgame");
    const surrenderCards = helpers.endgameStatsMarkup("surrender");
    assert.equal(cards.replace(/sow-hud-endgame-/g, "sow-hud-surrender-"), surrenderCards);
    const accessibleNames = [
        "endgame.stat_kills",
        "endgame.stat_assists",
        "endgame.stat_max_territory",
        "endgame.stat_gold_received",
        "endgame.stat_troops_received",
    ];
    for (const prefix of ["endgame", "surrender"]) {
        const markup = prefix === "endgame" ? cards : surrenderCards;
        for (const id of ["kills", "assists", "max-territory", "gold-received", "troops-received"]) {
            assert.match(markup, new RegExp(`sow-hud-${prefix}-${id}`));
        }
        assert.equal((markup.match(/role="group" aria-label=/g) || []).length, 5);
        for (const name of accessibleNames) assert.ok(markup.includes(`aria-label="${name}"`), name);
        assert.doesNotMatch(markup, /sow-hud__endgame-stat-label/);
        assert.doesNotMatch(markup, /profile\.kda|endgame-kda|\bDeaths\b|\bKDA\b/i);
    }
    for (const icon of ["attack", "allied", "globe", "gold", "troops"]) assert.ok(cards.includes(icon), icon);
    assert.ok(hud.includes('endgameStatsMarkup("surrender")'));
    assert.ok(hud.includes('endgameStatsMarkup("endgame")'));
    assert.doesNotMatch(hud, /sow-hud__exit-content/);
    assert.ok(hud.includes('id="sow-hud-surrender-save-note"'));
    assert.ok(hud.includes("renderEndgameStats(hudRefs!.surrenderStatValues, hud.player_endgame_stats)"));
    assert.ok(hud.includes('send("request_endgame_stats")'));
    assert.ok(hud.includes('send("clear_endgame_stats")'));

    const statValues = Object.fromEntries(
        ["kills", "assists", "maxTerritory", "goldReceived", "troopsReceived"].map((key) => [
            key,
            { textContent: "" },
        ]),
    );
    helpers.renderEndgameStats(statValues, {
        kills: 17,
        assists: 3,
        max_territory: 1_234,
        gold_received: 12_345_678,
        troops_received: 99_999_999,
    });
    assert.deepEqual(
        Object.values(statValues).map((node) => node.textContent),
        ["17", "3", "1.2K", "12.3M", "100M"],
    );

    const start = hud.indexOf("        // Endgame Screen");
    const end = hud.search(/ {8}(?:const|let|var) mapMenuOpen =/);
    assert.ok(start >= 0 && end > start);
    for (const [isWinner, result] of [
        [true, "victory"],
        [false, "defeat"],
    ]) {
        const node = () => ({
            dataset: {},
            textContent: "",
            src: "",
            alt: "",
            classList: {
                toggle(name, enabled) {
                    this[name] = enabled;
                },
            },
        });
        const refs = {
            endgame: node(),
            endgameCard: node(),
            endgameBanner: node(),
            endgamePortrait: node(),
            endgameStatValues: {
                kills: node(),
                assists: node(),
                maxTerritory: node(),
                goldReceived: node(),
                troopsReceived: node(),
            },
            endgameStore: node(),
            endgameObserve: node(),
        };
        const context = vm.createContext({
            hud: {
                match_over: true,
                is_winner: isWinner,
                player_endgame_stats: {
                    kills: 17,
                    assists: 3,
                    max_territory: 1_234,
                    gold_received: 12_345_678,
                    troops_received: 99_999_999,
                },
                player_leader: "test",
                featured_skin: null,
                winner_name: "",
            },
            hudRefs: refs,
            hudState: { selected_leader: "test" },
            window: { SOW_PORTAL: "web" },
            SOW_t: (key) => key,
            formatEndgameExact: format.formatEndgameExact,
            formatEndgameCompact: format.formatEndgameCompact,
            renderEndgameStats: helpers.renderEndgameStats,
            leaderById: () => ({ slug: "test", name: "Test" }),
            asset: (name) => "/assets/" + name,
        });
        vm.runInContext(hud.slice(start, end), context);
        assert.equal(refs.endgameCard.dataset.result, result);
        assert.equal(refs.endgameBanner.textContent, `hud.${result}`);
        assert.deepEqual(
            [
                refs.endgameStatValues.kills.textContent,
                refs.endgameStatValues.assists.textContent,
                refs.endgameStatValues.maxTerritory.textContent,
                refs.endgameStatValues.goldReceived.textContent,
                refs.endgameStatValues.troopsReceived.textContent,
            ],
            ["17", "3", "1.2K", "12.3M", "100M"],
        );
    }
    const statsRules = [...hudCss.matchAll(/\.sow-hud__endgame-stats\s*\{([^}]*)\}/g)];
    assert.equal(statsRules.length, 1, "portrait and horizontal views share one responsive layout");
    assert.match(statsRules[0][1], /grid-template-columns:\s*repeat\(auto-fit,\s*minmax\(72px,\s*1fr\)\)/);
    const minimumStatWidth = Number(statsRules[0][1].match(/minmax\((\d+)px/)[1]);
    assert.deepEqual(
        [242, 297, 458].map((width) => Math.min(5, Math.floor(width / minimumStatWidth))),
        [3, 4, 5],
    );
    const statRule = hudCss.match(/\.sow-hud__endgame-stat\s*\{([^}]*)\}/)[1];
    assert.match(statRule, /display:\s*flex/);
    assert.doesNotMatch(statRule, /border:|background:/);
    assert.match(hudCss, /\.sow-hud__endgame-stat \+ \.sow-hud__endgame-stat::before/);
    assert.match(hudCss, /\.sow-hud__exit-card\s*\{\s*min-height: 0;[^}]*max-height: min\([^}]*60svh\)/);
    assert.doesNotMatch(hudCss, /sow-hud__exit-content|\.sow-hud__exit-card \.sow-hud__endgame-stats/);
    assert.match(hudCss, /\.sow-hud__exit-card \.sow-hud__endgame-actions\s*\{\s*flex: 0 0 auto;/);
});

test("manual exit requests one stats snapshot and clears it when canceled", () => {
    const { context, calls, click } = makePlayerDialogsHarness();
    context.renderHud = () => {};
    click("prompt_surrender");
    assert.deepEqual(calls, [{ type: "request_endgame_stats" }]);
    assert.equal(context.surrenderModalOpen, true);
    click("close_surrender_modal");
    assert.deepEqual(calls, [{ type: "request_endgame_stats" }, { type: "clear_endgame_stats" }]);
    assert.equal(context.surrenderModalOpen, false);
});

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
            const styles = new Map(),
                classes = new Set();
            this.style = {
                getPropertyValue: (key) => styles.get(key) || "",
                setProperty: (key, value) => styles.set(key, value),
                removeProperty: (key) => styles.delete(key),
            };
            this.classList = {
                add: (name) => classes.add(name),
                contains: (name) => classes.has(name),
                toggle(name, on) {
                    if (on) classes.add(name);
                    else classes.delete(name);
                },
            };
        }
        append(...items) {
            items.forEach((item) => this.appendChild(item));
        }
        appendChild(item) {
            return this.insertBefore(item, null);
        }
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
        replaceChildren(...items) {
            this.children.slice().forEach((item) => item.remove());
            this.append(...items);
        }
        contains(item) {
            return item === this || this.children.some((child) => child.contains(item));
        }
        focus() {
            doc.activeElement = this;
        }
        getClientRects() {
            return !this.isConnected ||
                this.hidden ||
                this.classList.contains("hidden") ||
                (this.parentNode && !this.parentNode.getClientRects().length)
                ? []
                : [{}];
        }
        closest(selector) {
            if (selector === "[data-command]")
                return this.dataset.command ? this : this.parentNode?.closest(selector) || null;
            return null;
        }
        querySelectorAll(selector) {
            return this.children
                .flatMap((child) => [child, ...child.querySelectorAll(selector)])
                .filter(
                    (child) =>
                        selector === "button:not(:disabled)" &&
                        child.tagName.toLowerCase() === "button" &&
                        !child.disabled,
                );
        }
        setAttribute(name, value) {
            this.attributes[name] = String(value);
        }
        getAttribute(name) {
            return name === "src" ? this.src || null : this.attributes[name] || null;
        }
        removeAttribute(name) {
            delete this.attributes[name];
            if (name === "src") this.src = "";
        }
    }
    doc.createElement = (tag) => new Element(tag);
    doc.getElementById = (id) => doc.nodes[id] || null;
    return { doc, Element };
}

function makePlayerDialogsHarness() {
    const { doc, Element } = makeHudDocument();
    const calls = [],
        listeners = {};
    const context = vm.createContext({
        Element,
        HTMLElement: Element,
        HTMLButtonElement: Element,
        HTMLImageElement: Element,
        HTMLInputElement: Element,
        Node: Object,
        document: doc,
        hudRoot: new Element("root"),
        hudDevLayer: null,
        leaderboardOpen: false,
        inboxOpen: false,
        settingsOpen: false,
        mapMenuOpen: false,
        isOver: false,
        window: {
            SOW_LOCALE: "en",
            SOW_menu_command: (raw) => calls.push(JSON.parse(raw)),
            addEventListener: (name, listener) => {
                listeners[name] = listener;
            },
        },
        asset: (name) => "/assets/" + name,
        SOW_t: (key) => context.window.SOW_LOCALE + ":" + key,
    });
    const part = (start, end) => {
        const begin = hud.indexOf(start),
            finish = hud.indexOf(end, begin + start.length);
        assert.ok(begin >= 0 && finish > begin, start);
        return hud.slice(begin, finish);
    };
    const stateSource = part("    let hudState:", "    function send(").replace(
        /^(\s*)(?:const|let)(?=\s)/gm,
        "$1var",
    );
    vm.runInContext(
        stateSource +
            part("    function send(", "    function asset(") +
            part("    function bindAvatarFallback(", "    function clearNotificationCard(") +
            part("    function createPlayerIdentity(", "    function createInboxRequest(") +
            part(
                "        function handleHudClick(event",
                '        hudRoot.addEventListener("click", handleHudClick)',
            ) +
            part("    function handleHudStateUpdate(", "    window.SOW_onStateUpdate ="),
        context,
    );
    const canvas = (doc.nodes.blade = new Element("canvas"));
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
            betrayalIdentity: context.createPlayerIdentity(),
        };
        refs.transfer.classList.add("hidden");
        refs.betrayal.classList.add("hidden");
        refs.transferTarget.append(refs.transferIdentity.element);
        refs.betrayalTarget.append(refs.betrayalIdentity.element);
        refs.transfer.append(refs.transferClose, refs.transferTarget, refs.transferGold, refs.transferTroops);
        refs.betrayal.append(
            refs.betrayalClose,
            refs.betrayalTarget,
            refs.betrayalCancel,
            refs.betrayalConfirm,
        );
        context.hudRoot.append(refs.transfer, refs.betrayal);
        return refs;
    };
    context.hudRefs = makeRefs();
    const renderStart = hud.indexOf("    function renderHud(");
    const transferStart = hud.indexOf("if (hudRefs!.transfer)", renderStart);
    const transferEnd = hud.indexOf("renderNotifications(", transferStart);
    const overlayStart = hud.indexOf("hudRoot.dataset.overlayOpen =", transferEnd);
    const overlayEnd = hud.search(/(?:const|let|var)\s+dockHidden/, overlayStart);
    assert.ok(
        transferStart > renderStart &&
            transferEnd > transferStart &&
            overlayStart > transferEnd &&
            overlayEnd > overlayStart,
    );
    const redraw = new vm.Script(hud.slice(transferStart, transferEnd) + hud.slice(overlayStart, overlayEnd));
    context.renderHud = () => {
        if (context.hudInitialized === false) {
            context.hudRefs = makeRefs();
            context.hudInitialized = true;
        }
        context.hud = context.hudState.hud;
        redraw.runInContext(context);
    };
    context.hudInitialized = true;
    context.hudRoot.addEventListener = (name, listener) => {
        listeners[name] = listener;
    };
    const keyboardStart = hud.search(/if\s*\(hudRoot\)\s*hudRoot\.addEventListener\("keydown",/);
    const keyboardEnd = hud.indexOf('window.addEventListener("resize", updateHudSafeArea);', keyboardStart);
    assert.ok(keyboardStart >= 0 && keyboardEnd > keyboardStart);
    vm.runInContext(hud.slice(keyboardStart, keyboardEnd), context);
    return {
        context,
        doc,
        calls,
        listeners,
        canvas,
        update: (hudValue, extra = {}) => {
            const raw = JSON.stringify({
                phase: "Playing",
                hud: { attack_ratio: 0.25, ...hudValue },
                ...extra,
            });
            return context.handleHudStateUpdate(raw, JSON.parse(raw));
        },
        click: (command) => {
            const target = new Element("button");
            target.dataset.command = command;
            target.dataset.playerId = "2";
            context.handleHudClick({ target });
        },
        key: (key, shiftKey = false) => {
            const event = {
                key,
                shiftKey,
                prevented: false,
                preventDefault() {
                    this.prevented = true;
                },
                stopPropagation() {},
            };
            listeners.keydown(event);
            return event;
        },
    };
}

test("inbox reuses identities and live buttons, resolves focus, and survives locale rebuilds", () => {
    const { doc, Element } = makeHudDocument();
    const makeRefs = () => {
        const refs = {
            inbox: new Element("aside"),
            inboxRows: new Element("list"),
            inboxEmpty: new Element("empty"),
            inboxTotal: new Element("total"),
            inboxClose: new Element("button"),
            inboxButton: new Element("button"),
            inboxRequests: Object.create(null),
            inboxRequestKeys: [],
            inboxRenderKey: "",
        };
        refs.inbox.classList.add("hidden");
        refs.inbox.append(refs.inboxClose, refs.inboxRows, refs.inboxEmpty);
        return refs;
    };
    const calls = [],
        listeners = {};
    const context = vm.createContext({
        Element,
        HTMLElement: Element,
        HTMLButtonElement: Element,
        HTMLImageElement: Element,
        HTMLInputElement: Element,
        Node: Object,
        avatarIdentityKeys: new WeakMap(),
        document: doc,
        hudRefs: makeRefs(),
        hudRoot: new Element("root"),
        hudDevLayer: null,
        hudState: { phase: "Playing" },
        emoteTextKey: "",
        hudInitialized: true,
        window: {
            SOW_LOCALE: "en",
            SOW_menu_command: (raw) => calls.push(JSON.parse(raw)),
            addEventListener: (name, listener) => {
                listeners[name] = listener;
            },
        },
        asset: (name) => "/assets/" + name,
        currencyAsset: (name) => "/assets/gameplay/currency/" + name + ".webp",
        hudIcon: (name) => name,
        statusIcon: (name) => name,
        SOW_t: (key) => context.window.SOW_LOCALE + ":" + key,
    });
    const avatarStart = hud.indexOf("    function bindAvatarFallback(");
    const avatarEnd = hud.indexOf("    function clearNotificationCard(", avatarStart);
    const inboxStart = hud.indexOf("    function createPlayerIdentity(");
    const inboxEnd = hud.indexOf("    function renderNotifications(", inboxStart);
    const clickStart = hud.indexOf("        function handleHudClick(event");
    const clickEnd = hud.indexOf('        hudRoot.addEventListener("click", handleHudClick)', clickStart);
    for (const [start, end] of [
        [avatarStart, avatarEnd],
        [inboxStart, inboxEnd],
        [clickStart, clickEnd],
    ]) {
        assert.ok(start >= 0 && end > start);
    }
    vm.runInContext(
        hud.slice(avatarStart, avatarEnd) +
            hud.slice(inboxStart, inboxEnd) +
            hud.slice(hud.indexOf("    function send("), hud.indexOf("    function updateHudSafeArea(")) +
            hud.slice(clickStart, clickEnd),
        context,
    );
    const alliance = {
        kind: "alliance",
        requester_id: 2,
        name: "Caesar",
        active: true,
        avatar: { kind: "portrait", slug: "caesar" },
    };
    const resources = {
        kind: "resources",
        requester_id: 3,
        name: "Trinovantes",
        active: true,
        gold: 1234.9,
        troops: 56.8,
        avatar: { kind: "portrait", slug: "ndebele" },
    };
    const campaign = {
        kind: "resources",
        requester_id: 4,
        name: "<img onerror=alert(1)>",
        active: true,
        gold: 5,
        troops: 0,
        avatar: { kind: "portrait", slug: "boudica" },
    };
    const eliminated = {
        kind: "resources",
        requester_id: 5,
        name: "Defeated",
        active: false,
        gold: 10,
        troops: 10,
    };
    let requests = [alliance, resources, campaign, eliminated];
    context.renderInbox(requests);
    const refs = context.hudRefs,
        cards = refs.inboxRequests;
    assert.equal(refs.inboxRows.children.length, 4);
    assert.equal(cards["alliance:2"].portrait.src, "/assets/gameplay/avatars/caesar.webp");
    assert.equal(cards["resources:3"].portrait.src, "/assets/gameplay/avatars/ndebele.webp");
    assert.equal(cards["resources:3"].emblem.hidden, true);
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
        { type: "reject_alliance", target_player_id: 2 },
    ]);
    const added = { ...alliance, requester_id: 6, name: "LongPlayerName".repeat(12) };
    context.renderInbox([added, ...requests]);
    assert.equal(refs.inboxRows.insertions, insertions + 1, "a new request preserves the other cards");
    assert.equal(
        refs.inboxRequests["alliance:6"].name.textContent,
        added.name,
        "long names are retained as text",
    );
    assert.equal(refs.inboxRequests["resources:3"].accept, pressed);
    context.renderInbox([added, alliance, campaign, eliminated]);
    assert.equal(
        doc.activeElement,
        refs.inboxRequests["resources:4"].accept,
        "resolved cards move focus to the next request",
    );

    context.window.SOW_LOCALE = "es";
    context.renderInbox([added, alliance, campaign, eliminated]);
    assert.equal(refs.inboxRequests["alliance:2"].accept.textContent, "es:hud.accept");
    assert.equal(refs.inboxRequests["alliance:2"].type.textContent, "es:hud.alliance_request_title");
    context.renderHud = () => {
        context.hudRefs = makeRefs();
        context.renderInbox(requests);
    };
    const localeStart = hud.indexOf('    window.addEventListener("sow:locale-change", function () {');
    const localeEnd = hud.indexOf('    window.addEventListener("resize", updateHudSafeArea);', localeStart);
    vm.runInContext(hud.slice(localeStart, localeEnd), context);
    listeners["sow:locale-change"]();
    assert.notEqual(context.hudRefs, refs);
    assert.equal(
        context.hudRefs.inboxRows.children.length,
        4,
        "a rebuilt inbox cannot retain the old DOM render key",
    );
    assert.equal(
        context.hudRefs.inboxRequests["resources:3"].type.textContent,
        "es:hud.resource_request_title",
    );
    context.hudRefs.inboxRequests["resources:3"].accept.focus();
    context.renderInbox([]);
    assert.equal(context.hudRefs.inboxRows.hidden, true);
    assert.equal(context.hudRefs.inboxEmpty.hidden, false);
    assert.equal(context.hudRefs.inboxTotal.textContent, "0");
    assert.equal(doc.activeElement, context.hudRefs.inboxClose);

    const visibilityStart = hud.indexOf(
        "        if (hudRefs!.inbox) {",
        hud.indexOf("    function renderHud("),
    );
    const visibilityEnd = hud.indexOf("        if (hudRefs!.transfer)", visibilityStart);
    const visibility = new vm.Script(hud.slice(visibilityStart, visibilityEnd));
    context.activeHudPanel = "inbox";
    context.hud = { inbox: requests };
    let renderInboxOpen = true;
    context.renderHud = () => {
        context.inboxOpen = renderInboxOpen;
        visibility.runInContext(context);
        renderInboxOpen = false;
    };
    context.renderHud();
    assert.equal(doc.activeElement, context.hudRefs.inboxClose, "opening the inbox moves focus inside it");
    assert.equal(context.hudRefs.inbox.contains(doc.activeElement), true);
    context.hudRoot.addEventListener = (name, listener) => {
        listeners[name] = listener;
    };
    const keyboardStart = hud.search(/if \(hudRoot\)\s*hudRoot\.addEventListener\("keydown",/);
    vm.runInContext(
        hud.slice(hud.indexOf("    function setHudPanel("), hud.indexOf("    function asset(")) +
            hud.slice(keyboardStart, localeStart),
        context,
    );
    let prevented = false;
    listeners.keydown({
        key: "Escape",
        preventDefault() {
            prevented = true;
        },
        stopPropagation() {},
    });
    assert.equal(prevented, true);
    assert.equal(context.activeHudPanel, null);
    assert.equal(context.hudRefs.inbox.classList.contains("hidden"), true);
    assert.equal(
        doc.activeElement,
        context.hudRefs.inboxButton,
        "Escape returns focus to the toolbar control",
    );
    assert.deepEqual(calls.at(-1), { type: "toggle_inbox" });
});

test("map feedback reuses one compact card, clamps to the viewport, and releases message data", () => {
    const start = hud.indexOf("    function createNotificationCard(");
    const end = hud.indexOf("\n    function renderHud(", start);
    const durationMatch = hud.match(/const MAP_FEEDBACK_DURATION_MS = (\d+);/);
    const fadeOutMatch = hud.match(/const MAP_FEEDBACK_FADE_OUT_MS = (\d+);/);
    assert.ok(durationMatch, "map feedback duration is declared");
    assert.ok(fadeOutMatch, "map feedback fade-out duration is declared");
    const durationMs = Number(durationMatch[1]);
    const fadeOutMs = Number(fadeOutMatch[1]);
    assert.equal(durationMs, 2000, "map feedback stays brief");
    assert.equal(fadeOutMs, 180, "map feedback fades out gently");
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
            remove(name) {
                classes.delete(name);
            },
            contains(name) {
                return classes.has(name);
            },
        },
        get offsetWidth() {
            return 220;
        },
        getBoundingClientRect() {
            return { width: 220, height: 46 };
        },
    };
    const parts = {
        card,
        seal: { innerHTML: "" },
        avatars: Array.from({ length: 2 }, () => ({
            wrapper: { hidden: true, className: "" },
            image: {
                hidden: true,
                src: "",
                getAttribute(name) {
                    return name === "src" ? this.src : null;
                },
                removeAttribute(name) {
                    if (name === "src") this.src = "";
                },
            },
            emblem: { hidden: true, textContent: "" },
        })),
        copy: { textContent: "" },
        renderKey: "",
    };
    const style = {
        left: "",
        top: "",
        removeProperty(name) {
            this[name] = "";
        },
    };
    const container = { hidden: true, style };
    let now = 1000;
    const timers = [];
    const context = {
        hudRefs: { mapFeedback: container },
        avatarIdentityKeys: new WeakMap(),
        mapFeedbackCursor: 0,
        mapFeedbackTimer: null,
        activeMapFeedback: null,
        mapFeedbackCard: parts,
        MAP_FEEDBACK_DURATION_MS: durationMs,
        MAP_FEEDBACK_FADE_OUT_MS: fadeOutMs,
        Date: { now: () => now },
        asset: (path) => "/assets/" + path,
        hudIcon: (name) => name,
        statusIcon: (name) => name,
        SOW_t: (key) => key,
        window: {
            innerWidth: 375,
            innerHeight: 667,
            clearTimeout(timer) {
                timer.cleared = true;
            },
            setTimeout(callback, delay) {
                const timer = { callback, delay, cleared: false };
                timers.push(timer);
                return timer;
            },
        },
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
    assert.ok(parts.avatars.every((avatar) => avatar.wrapper.hidden && avatar.image.src === ""));

    context.renderMapFeedback({ id: 4, key: "hud.build_land", values: {}, x: 375, y: 650, age_ms: 2000 });
    assert.equal(container.hidden, true, "expired feedback is not displayed");
});

test("Campaign Studio avoids rebuilding the route graph on progress-only updates", () => {
    const start = campaignEditor.indexOf("function paintPreview(updateMachine?: boolean) {");
    const end = campaignEditor.indexOf("function previewLocaleScript", start);
    assert.ok(start >= 0 && end > start);
    const paint = campaignEditor.slice(start, end);
    assert.match(paint, /if \(previousStep !== model\.step\.id\) renderGraph\(\)/);
    assert.equal((paint.match(/renderGraph\(\)/g) || []).length, 1);
});

test("Trinovantes aid guides radial opening, Transfer, and the confirmed delivery separately", async () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    assert.deepEqual(campaign.validate(definition, roster).errors, []);
    const missingTarget = structuredClone(definition);
    delete missingTarget.steps.find((step) => step.id === "boudica_transfer_target").marker;
    assert.ok(
        campaign
            .validate(missingTarget, roster)
            .errors.some((issue) => issue.step === "boudica_transfer_target" && issue.field === "marker"),
    );
    const root = { dataset: {}, hidden: true, isConnected: false, contains: () => false };
    const radial = {
        dataset: {},
        disabled: false,
        matches: () => false,
        getClientRects: () => [{}],
        contains: (target) => target === radial,
    };
    const transferButton = {
        dataset: {},
        disabled: false,
        matches: () => false,
        getClientRects: () => [{}],
        closest: () => null,
        getBoundingClientRect: () => ({ left: 300, top: 400, width: 40, height: 40 }),
        contains: (target) => target === transferButton,
    };
    const hud = {
        tutorial: { active: true, episode_id: "boudica", facts: {}, player: { x: 110, y: 210 } },
        players: [
            { id: 1, is_me: true },
            { id: 2, campaign_faction_id: "trinovantes" },
            { id: 3, campaign_faction_id: "stonea" },
        ],
        settings: {},
        map_menu: null,
        transfer: null,
    };
    const listeners = {};
    const document = {
        body: {
            appendChild(node) {
                node.isConnected = true;
            },
        },
        documentElement: { dir: "ltr" },
        createElement: () => root,
        getElementById: () => null,
        querySelector(selector) {
            if (!hud.map_menu || !hud.map_menu.open || hud.map_menu.view !== "radial") return null;
            if (selector === campaign.UI_TARGETS.map_menu_open) return radial;
            if (selector === campaign.UI_TARGETS.map_transfer) return transferButton;
            return null;
        },
        addEventListener(type, callback) {
            listeners[type] = callback;
        },
    };
    let machine, rendered, context;
    const window = {
        addEventListener() {},
        SOW_menu_command() {},
        SOW_t: (key) => (key.startsWith("tutorial.") ? "Text for " + key : key),
        SOW_hasText: () => true,
        SOWCampaign: {
            ...campaign,
            create(data) {
                machine = campaign.create(data, "boudica_transfer_target", roster);
                return machine;
            },
        },
        SOWCampaignView: {
            mount: () => ({
                render(model, nextContext) {
                    rendered = model;
                    context = nextContext;
                },
                destroy() {},
            }),
        },
    };
    vm.runInNewContext(tutorialBundle, {
        window,
        document,
        Node: Object,
        performance: { now: () => 0 },
        console,
        fetch(url) {
            return Promise.resolve({
                ok: true,
                json: () => Promise.resolve(url.endsWith(".triggers.json") ? definition : roster),
            });
        },
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
    assert.equal(
        machine.view().step.id,
        "boudica_transfer_target",
        "another ally or a raw menu click cannot satisfy the target step",
    );
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
    assert.equal(
        machine.view().step.id,
        "boudica_transfer_choose",
        "Transfer waits for the confirmed modal destination",
    );

    hud.map_menu = null;
    update();
    assert.equal(rendered.step.id, "boudica_transfer_choose");
    assert.deepEqual(
        [context.anchor.x, context.anchor.y],
        [110, 210],
        "closing the radial points back to Trinovantes",
    );
    hud.map_menu = { open: true, view: "radial", target_player_id: 3 };
    update();
    listeners.click({ type: "click", target: transferButton });
    hud.map_menu = null;
    hud.transfer = { target_id: 3 };
    update();
    assert.equal(
        rendered.step.id,
        "boudica_transfer_choose",
        "another ally's transfer modal cannot advance the guide",
    );
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
    assert.equal(machine.view().step.id, "boudica_share_sent");
    assert.equal(rendered, null, "the scene hides the guide while its emote exchange plays");

    machine.jump("boudica_transfer_target");
    hud.tutorial.facts = {};
    hud.transfer = null;
    update();
    hud.map_menu = { open: true, view: "radial", target_player_id: 2 };
    update();
    assert.equal(
        rendered.step.id,
        "boudica_transfer_choose",
        "revisiting the guide records a new menu opening",
    );
    hud.map_menu = null;
    hud.transfer = { target_id: 2 };
    update();
    assert.equal(
        rendered.step.id,
        "boudica_transfer_send",
        "revisiting the guide records a new Transfer action",
    );
});

test("Legion IX slider objective waits for the live HUD ratio to reach 100%", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const machine = campaign.create({
        version: 2,
        episode_id: "attack_ratio_test",
        default_locale: "en",
        settings: { starting_troops: 0 },
        strings: { en: {} },
        speakers: {},
        entry: "objective",
        steps: [
            {
                id: "objective",
                type: "objective",
                title_key: "tutorial.objective",
                hint_key: "tutorial.objective_hint",
                trigger: { type: "ui", action: "attack_ratio", scope: "step", value: 1 },
                next: "ending",
            },
            { id: "ending", type: "end", title_key: "tutorial.ending" },
        ],
    });

    assert.equal(machine.update({ attack_ratio: 0.5 }, {}).step.id, "objective");
    assert.equal(machine.update({ attack_ratio: 0.99 }, {}).step.id, "objective");
    assert.equal(machine.update({ attack_ratio: 1 }, {}).step.id, "ending");
});

test("gameplay and editor share compact viewport and hand progress rules", () => {
    const view = loadTypeScriptModule(path.join(shell, "sow-campaign-view.ts"));
    const root = { dataset: {}, clientWidth: 390, clientHeight: 844 };
    assert.equal(view.isCompactViewport(root), true, "mobile portrait hides the quest panel");
    root.clientWidth = 844;
    root.clientHeight = 390;
    assert.equal(view.isCompactViewport(root), true, "compact landscape hides the quest panel");
    root.clientWidth = 1366;
    root.clientHeight = 768;
    assert.equal(view.isCompactViewport(root), false, "normal desktop keeps the quest panel");
    root.clientWidth = 1024;
    root.clientHeight = 600;
    assert.equal(view.isCompactViewport(root), true, "low-height desktop uses compact gameplay");
    root.clientWidth = 1366;
    root.clientHeight = 768;
    root.dataset.compact = "true";
    assert.equal(view.isCompactViewport(root), true, "the shared HUD density flag remains authoritative");

    assert.equal(
        view.compactGuideMetric(
            { guide: { kind: "world", target: "expand" }, trigger: { type: "territory", value: 250 } },
            { current: 120, target: 250 },
        ).text,
        "120 / 250",
    );
    assert.equal(
        view.compactGuideMetric(
            {
                guide: { kind: "ui", target: "attack_ratio" },
                trigger: { type: "ui", action: "attack_ratio", value: 1 },
            },
            { current: 0.72, target: 1 },
        ).text,
        "72% / 100%",
    );
    assert.equal(
        view.compactGuideMetric(
            {
                guide: { kind: "world", target: "player" },
                trigger: { type: "territory", scope: "total", comparison: "lte", value: 5000 },
            },
            { current: 7200, target: 5000 },
        ),
        null,
        "the at-most territory condition is not misrepresented as a progress bar",
    );
    assert.equal(
        view.compactGuideMetric(
            { trigger: { type: "territory", value: 250 } },
            { current: 120, target: 250 },
        ),
        null,
        "passive status steps do not gain a fake action counter",
    );
});

test("short landscape HUD geometry fits the requested phone widths and heights", () => {
    for (const [width, height, safeLeft] of [
        [568, 320, 44],
        [667, 375, 0],
        [844, 390, 0],
        [932, 430, 0],
    ]) {
        const leftInset = Math.max(8, safeLeft),
            rightInset = 8;
        const dockWidth = Math.min(520, width - leftInset - rightInset - 204);
        const operationWidth = Math.min(320, width - leftInset - 280 - rightInset);
        const dockLeft = (width - dockWidth) / 2;
        const operationsRight = width - rightInset;
        const operationsHeight = Math.min(height * 0.2, Math.max(0, height - 56 - 84 - 6 - 6));
        assert.ok(dockWidth >= 280, `${width}x${height}: dock has room for the building action strip`);
        assert.ok(operationWidth >= 220, `${width}x${height}: operations keep a usable width`);
        assert.ok(
            dockLeft >= leftInset && dockLeft + dockWidth <= width - rightInset,
            `${width}x${height}: dock stays inside safe edges`,
        );
        assert.ok(
            operationsRight <= width - rightInset && operationsRight - operationWidth >= leftInset,
            `${width}x${height}: operations stay inside safe edges`,
        );
        assert.ok(
            operationsHeight > 0 && operationsHeight <= height * 0.2,
            `${width}x${height}: operations remain scrollable within 20% of the frame`,
        );
    }
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
        assert.deepEqual(calls, [
            command === "close_transfer"
                ? { type: command }
                : { type: command, target_player_id: 2, gold: 12, troops: 34 },
        ]);
        context.renderHud();
        assert.equal(
            panel.classList.contains("hidden"),
            false,
            "the current state keeps a pending transfer visible",
        );
        assert.equal(context.hudRoot.dataset.overlayOpen, "true");
        update(null);
        assert.equal(panel.classList.contains("hidden"), true, command);
        assert.equal(
            context.hudRoot.dataset.overlayOpen,
            "false",
            command + " releases both mobile sidebars",
        );
        update(null, { gold: 100, attack_ratio: 1 });
        context.renderHud();
        assert.equal(
            context.hudRoot.dataset.overlayOpen,
            "false",
            "later tutorial/HUD updates cannot resurrect a closed transfer",
        );
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
        assert.equal(
            context.hudRoot.dataset.overlayOpen,
            "false",
            "an unconfirmed open cannot hide the rails without a panel",
        );
        update(transfer);
        click("send_resources");
        update(transfer);
        context.renderHud();
        assert.equal(
            panel.classList.contains("hidden"),
            false,
            "a rejected send keeps the authoritative panel open for retry",
        );
        assert.equal(context.hudRefs.transferGold.value, "12");
        assert.equal(context.hudRefs.transferTroops.value, "34");
        update(null);
        assert.equal(
            context.hudRoot.dataset.overlayOpen,
            "false",
            "a state-driven close also restores the rails",
        );
    }
});

test("player dialogs share identities, preserve drafts and focus, and prioritize safe keyboard actions", () => {
    const { context, doc, calls, listeners, update, click, key, canvas } = makePlayerDialogsHarness();
    const transfer = {
        target_id: 2,
        target_name: "<img onerror=alert(1)>".repeat(8),
        target_avatar: { kind: "portrait", slug: "caesar" },
        suggested_gold: 12.9,
        suggested_troops: 34.9,
    };
    update({ transfer });
    const first = context.hudRefs,
        gold = first.transferGold,
        troops = first.transferTroops;
    assert.equal(
        doc.activeElement,
        first.transferClose,
        "opening a transfer does not open the mobile keyboard",
    );
    assert.equal(first.transferIdentity.name.textContent, transfer.target_name);
    assert.equal(first.transferIdentity.name.children.length, 0);
    assert.equal(first.transferIdentity.portrait.src, "/assets/gameplay/avatars/caesar.webp");
    assert.equal(gold.value, "12");
    first.transferIdentity.portrait.onerror.call(first.transferIdentity.portrait);
    assert.equal(first.transferIdentity.portrait.src, "/assets/gameplay/avatars/null.webp");
    gold.value = "1234567";
    troops.value = "7000";
    gold.focus();
    for (let tick = 1; tick <= 5; tick += 1)
        update({ transfer: { ...transfer, suggested_gold: tick }, tick });
    assert.equal(context.hudRefs.transferGold, gold);
    assert.equal(gold.value, "1234567");
    assert.equal(troops.value, "7000");
    assert.equal(doc.activeElement, gold);
    click("request_resources");
    assert.deepEqual(
        calls.at(-1),
        { type: "request_resources", target_player_id: 2, gold: 1234567, troops: 7000 },
        "requests are not capped by the sender's balance",
    );
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
    assert.equal(
        context.hudRefs.transfer.classList.contains("hidden"),
        false,
        "wait for the authoritative close",
    );
    update({ transfer: null });
    assert.equal(doc.activeElement, canvas);
    update({ transfer });
    assert.equal(context.hudRefs.transferGold.value, "12", "reopening the same target starts a fresh draft");
    update({
        transfer: {
            ...transfer,
            target_id: 3,
            target_name: "Trinovantes",
            target_avatar: { kind: "portrait", slug: "ndebele" },
            suggested_gold: 7,
        },
    });
    assert.equal(context.hudRefs.transferGold.value, "7");
    assert.equal(context.hudRefs.transferIdentity.portrait.src, "/assets/gameplay/avatars/ndebele.webp");
    assert.equal(context.hudRefs.transferIdentity.emblem.hidden, true);
    update({ transfer: { ...transfer, target_avatar: { kind: "portrait", slug: "boudica" } } });
    assert.equal(context.hudRefs.transferIdentity.portrait.src, "/assets/gameplay/avatars/boudica.webp");
    update({ transfer: null });
    const betrayal = {
        ally_id: 3,
        ally_name: "Ally".repeat(30),
        ally_avatar: { kind: "portrait", slug: "ndebele" },
    };
    update({ betrayal });
    const refs = context.hudRefs;
    assert.equal(refs.betrayalIdentity.portrait.src, "/assets/gameplay/avatars/ndebele.webp");
    assert.equal(refs.betrayalIdentity.name.textContent, betrayal.ally_name);
    assert.equal(
        doc.activeElement,
        refs.betrayalCancel,
        "the destructive action is never focused by default",
    );
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
});

test("shared dialog viewport tracks the available keyboard area without scaling or per-dialog listeners", () => {
    const start = hud.indexOf("    function syncCompactHudBounds(");
    const end = hud.search(/ {8}(?:const|let|var) density = syncCompactHudDensity\(/);
    assert.ok(start >= 0 && end > start);
    const { Element } = makeHudDocument();
    const root = new Element("root");
    const context = vm.createContext({ hudRoot: root, hudRefs: {}, window: {} });
    vm.runInContext(hud.slice(start, end) + "    }", context);
    for (const [width, height] of [
        [1366, 768],
        [390, 844],
        [360, 640],
        [844, 390],
        [640, 360],
        [568, 320],
    ]) {
        context.window.innerWidth = width;
        context.window.innerHeight = root.clientHeight = height;
        context.window.visualViewport = { height, offsetTop: 0 };
        context.syncCompactHudBounds({ width, height });
        assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-height"), height + "px");
        assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-bottom"), "0px");
    }
    root.clientHeight = context.window.innerHeight = 844;
    context.window.visualViewport = { height: 360, offsetTop: 80 };
    context.syncCompactHudBounds({ width: 844, height: 844 });
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-height"), "360px");
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-top"), "80px");
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-bottom"), "404px");
    context.window.visualViewport = null;
    context.syncCompactHudBounds({ width: 844, height: 844 });
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-height"), "844px");
    assert.equal(root.style.getPropertyValue("--sow-hud-dialog-view-bottom"), "0px");
});

test("mobile overlays never hide the building dock and story dialogs remain above it", () => {
    const overlayRules = hudCss.match(/#sow-hud\[data-overlay-open="true"\][^{]*\{[^}]*\}/g) || [];
    assert.ok(overlayRules.length >= 2, "portrait and short-landscape overlay rules remain covered");
    assert.ok(
        overlayRules.every((rule) => !rule.includes(".sow-hud__dock")),
        "transient menus and panels cannot hide the persistent building dock",
    );
});
