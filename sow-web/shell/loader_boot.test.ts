import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { vm } from "./typescript-test-support.ts";
const shell = __dirname;
import { bundleTs, loadTypeScriptModule } from "./typescript-test-support.ts";
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.ts"), "utf8");
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.ts"), "utf8");
const heroesSource = fs.readFileSync(path.join(shell, "main_menu.heroes.ts"), "utf8");
const loaderSource = fs.readFileSync(path.join(shell, "loader.ts"), "utf8");
const loaderTemplate = fs.readFileSync(path.join(shell, "index.html.template"), "utf8");
const tutorialBundle =
    '(function () { if (typeof window === "undefined") return; const providedView = window.SOWCampaignView;\n' +
    bundleTs(path.join(shell, "sow-campaign-view.ts")) +
    "\nif (providedView) Object.assign(window.SOWCampaignView, providedView); })();\n" +
    bundleTs(path.join(shell, "main_menu.tutorial.ts"));
const sessionSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/session.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const dataDbSource = fs.readFileSync(path.join(shell, "../../sow-data/src/db.rs"), "utf8");
const accountSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/account.rs"), "utf8");
const storeSource = fs.readFileSync(path.join(shell, "main_menu.store.ts"), "utf8");
const profileSource = fs.readFileSync(path.join(shell, "main_menu.profile.ts"), "utf8");
void [
    "../../sow-client/src/app/account.rs",
    "../../sow-client/src/asset.rs",
    "../../sow-client/src/lib.rs",
    "../../sow-client/src/player_progress.rs",
    "../../sow-client/src/render/interact/actions.rs",
    "../../sow-client/src/ui/main_menu/mod.rs",
    "../../sow-client/src/ui.rs",
]
    .map((file) => fs.readFileSync(path.join(shell, file), "utf8"))
    .join("\n");
const matchStartSource = fs.readFileSync(
    path.join(shell, "../../sow-client/src/loader/match_start.rs"),
    "utf8",
);
const engineLoaderSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/engine.rs"), "utf8");
const simUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/update.rs"), "utf8");
const netMessagesSource = fs.readFileSync(
    path.join(shell, "../../sow-client/src/net/update/messages.rs"),
    "utf8",
);

test("first-run tutorial keeps one boot loader and splash art until Rust signals completion", async () => {
    for (const id of ["loader-footer", "loader-ability", "loader-bar-wrap", "loader-bar-fill"]) {
        assert.ok(loaderTemplate.includes(`id="${id}"`), `production loader template must contain ${id}`);
    }
    assert.doesNotMatch(
        loaderTemplate,
        /id="loader-hero-name"|id="loader-hero-civilization"|id="loader-ability-icon"|id="loader-ability-text"/,
        "the loader must render the shared hero briefing instead of maintaining a second copy",
    );
    assert.match(
        loaderTemplate,
        /backdrop-filter:\s*blur\(/,
        "loader details need a dark blurred surface over splash art",
    );
    assert.match(
        loaderTemplate,
        /@media \(max-width: 599px\) and \(orientation: portrait\)[\s\S]*?sow-hero-briefing__icon/,
        "the loader briefing has a vertical-screen size",
    );
    assert.match(
        loaderTemplate,
        /@media \(orientation: landscape\) and \(max-height: 560px\) and \(min-width: 600px\)[\s\S]*?sow-hero-briefing__icon/,
        "the loader briefing has a compact-landscape size",
    );
    assert.equal(
        (webMenu.match(/"loader_info_leader":/g) || []).length,
        2,
        "both menu and in-game loader updates must keep the actual leader identity",
    );
    assert.equal(
        (webMenu.match(/"loader_show_hero_briefing":/g) || []).length,
        2,
        "both loader updates must carry the explicit normal-match presentation signal",
    );
    assert.match(
        matchStartSource,
        /show_hero_briefing = !tutorial/,
        "normal offline matches show details while tutorial starts keep them hidden",
    );
    assert.match(
        netMessagesSource,
        /show_hero_briefing = !start_msg\.config\.tutorial/,
        "network matches use the server's tutorial flag for loader details",
    );
    assert.match(
        loaderTemplate,
        /id="loader-map-error"[\s\S]*?id="loader-map-error-text"[\s\S]*?id="loader-map-retry"/,
        "the match loader includes a retry action for a failed map download",
    );
    assert.match(
        loaderSource,
        /SOW_menu_command\(JSON\.stringify\(\{\s*type: ["']retry_map_download["']\s*\}\)\)/,
        "the loader retry button uses the existing Rust menu command bridge",
    );
    assert.match(
        engineLoaderSource,
        /self\.sim\.turn_queue\.is_empty\(\)/,
        "the loader stays visible until queued relay turns are applied",
    );
    assert.match(
        simUpdateSource,
        /if self\.sim\.engine\.is_some\(\) \{[\s\S]*?while let Some\(turn\) = self\.sim\.turn_queue\.pop_front\(\)/,
        "relay turns remain queued until the simulation engine exists, then drain in order",
    );
    const attachExitArt = loaderSource.indexOf("const pendingArt = syncLoaderArt(state);");
    const closeAfterArt = loaderSource.indexOf(
        "if (state.loader_done === true) {\n            if (pendingArt) return;\n            finish(cycleId);",
        attachExitArt,
    );
    assert.ok(
        attachExitArt >= 0 && closeAfterArt > attachExitArt,
        "exit art is attached before the loader begins closing",
    );
    for (const source of [coreSource, heroesSource, storeSource, profileSource]) {
        assert.doesNotMatch(
            source,
            /asset\(["']shell\/leaders\//,
            "leader art must use the same versioned URL as the loader",
        );
    }
    const introStart = matchStartSource.indexOf(
        'log::info!("Portal boot: new player -> JavaScript campaign bootstrap")',
    );
    const introEnd = matchStartSource.indexOf("crate::store_portals::gameplay_stop();", introStart);
    assert.ok(introStart >= 0 && introEnd > introStart);
    const bootIntro = matchStartSource.slice(introStart, introEnd);
    assert.match(bootIntro, /let campaign = crate::campaign::CampaignId::Boudica/);
    assert.match(bootIntro, /boot_campaign_pending = Some\(campaign\.episode_id\(\)\.to_string\(\)\)/);
    assert.match(bootIntro, /begin_enter_game_loader\(campaign\.advisor\(\)\)/);
    assert.doesNotMatch(bootIntro, /hide_web_loader|splash_state\.done = true|phase = ClientPhase::MainMenu/);
    const tutorialStateUpdate = shellSource.indexOf("window.SOW_tutorial_menu_state_update(menuUi.state)");
    const loaderStateSync = shellSource.indexOf("syncWebLoaderForState(menuUi.state)", tutorialStateUpdate);
    assert.ok(
        tutorialStateUpdate >= 0 && loaderStateSync > tutorialStateUpdate,
        "the same menu-state update starts the tutorial request before syncing the existing loader",
    );
    const campaignCommandStart = webMenu.lastIndexOf("WebMenuCommand::StartCampaignEpisode {");
    const campaignCommandEnd = webMenu.indexOf(
        "WebMenuCommand::CompleteCampaignEpisode {",
        campaignCommandStart,
    );
    assert.ok(campaignCommandStart >= 0 && campaignCommandEnd > campaignCommandStart);
    const campaignStartStart = matchStartSource.indexOf("pub(crate) fn start_campaign_episode_from_web");
    const offlineStartStart = matchStartSource.indexOf(
        "pub(crate) fn start_offline_match",
        campaignStartStart,
    );
    const campaignStart = matchStartSource.slice(campaignStartStart, offlineStartStart);
    const offlineStart = matchStartSource.slice(offlineStartStart);
    assert.doesNotMatch(
        campaignStart,
        /set_selected_leader\(/,
        "starting a campaign does not replace the account's menu leader",
    );
    assert.doesNotMatch(
        offlineStart,
        /set_selected_leader\(/,
        "starting a tutorial match keeps its leader separate from menu selection",
    );
    assert.match(offlineStart, /color: leader\.filler_rgb\(\)/);
    assert.match(
        offlineStart,
        /spawn_y: 0,\s*leader,/,
        "the tutorial's actual game player still uses its campaign leader",
    );
    const serverStartStart = netMessagesSource.indexOf("ServerMessage::Start(start_msg) =>");
    const serverStartEnd = netMessagesSource.indexOf(
        "self.tasks.engine_init_queued_msg = Some(*start_msg);",
        serverStartStart,
    );
    assert.ok(serverStartStart >= 0 && serverStartEnd > serverStartStart);
    const serverStart = netMessagesSource.slice(
        serverStartStart,
        serverStartEnd + "self.tasks.engine_init_queued_msg = Some(*start_msg);".length,
    );
    assert.match(
        serverStart,
        /let loader_leader = start_msg[\s\S]*?\.map\(\|player\| player\.leader\)/,
        "the server-assigned leader is used for the match loader",
    );
    assert.match(
        serverStart,
        /self\.tasks\.engine_init_queued_msg = Some\(\*start_msg\);/,
        "the match starts from the server-assigned player data",
    );
    assert.doesNotMatch(
        serverStart,
        /sync_selected_leader_from_match\(|preferred_leader\s*=/,
        "starting a match cannot overwrite the account's saved leader choice",
    );
    const exitStart = sessionSource.indexOf("pub(crate) fn begin_exit_to_main_menu");
    const preferencesStart = accountSource.indexOf("pub(crate) fn apply_progress_preferences(");
    const preferencesEnd = accountSource.indexOf("pub(crate) fn apply_cloud_profile(", preferencesStart);
    assert.ok(preferencesStart >= 0 && preferencesEnd > preferencesStart);
    const preferences = accountSource.slice(preferencesStart, preferencesEnd);
    assert.match(
        preferences,
        /has_explicit_leader_selection\(\) \{\s*return;/,
        "a profile refresh cannot overwrite an explicit hero choice",
    );
    assert.match(
        preferences,
        /if let Some\(leader\) = self\.progress\.preferred_leader[\s\S]*?set_selected_leader\(leader, false\)/,
        "the saved profile, not the tutorial hero, restores the menu selection",
    );
    const applyCloudProfileStart = accountSource.indexOf("pub(crate) fn apply_cloud_profile(");
    const applyCloudProfileEnd = accountSource.indexOf("\n    pub(crate) fn ", applyCloudProfileStart + 1);
    assert.ok(applyCloudProfileStart >= 0);
    const applyCloudProfile = accountSource.slice(applyCloudProfileStart, applyCloudProfileEnd);
    assert.match(
        applyCloudProfile,
        /cloud_preferred_leader\.unwrap_or_else\([\s\S]*?assigned_leader_for_account\([\s\S]*?current_rotation_period\(\)/,
        "a missing cloud preference resolves to the account's free-rotation leader, not Boudica",
    );
    const tutorialCompleteStart = dataDbSource.indexOf("pub async fn complete_tutorial(");
    const tutorialCompleteEnd = dataDbSource.indexOf("/// Register expected players", tutorialCompleteStart);
    const tutorialComplete = dataDbSource.slice(tutorialCompleteStart, tutorialCompleteEnd);
    assert.match(
        tutorialComplete,
        /assigned_leader_for_account\(\s*account_id,\s*crate::commerce::current_rotation_period\(\)\s*,?\s*\)/,
        "an account missing its preference receives its normal free-rotation assignment",
    );
    assert.doesNotMatch(tutorialComplete, /Leader::Boudica/);
    const enterLoaderStart = sessionSource.indexOf("pub(crate) fn begin_enter_game_loader");
    const enterLoaderEnd = sessionSource.indexOf("/// Whether the map/mover GPU path", enterLoaderStart);
    assert.ok(enterLoaderStart >= 0 && enterLoaderEnd > enterLoaderStart);
    const enterLoader = sessionSource.slice(enterLoaderStart, enterLoaderEnd);
    assert.match(
        enterLoader,
        /job == SplashJob::EnterGame[\s\S]*loader_leader == Some\(leader\)[\s\S]*!self\.ui\.app\.splash_state\.done[\s\S]*return;/,
    );

    class Element {
        constructor(tagName) {
            this.tagName = tagName;
            this.children = [];
            this.style = {};
            this.dataset = {};
            this.attributes = {};
            this.listeners = {};
            this.parentNode = null;
        }
        appendChild(child) {
            if (child.parentNode) child.remove();
            this.children.push(child);
            child.parentNode = this;
            return child;
        }
        insertBefore(child, before) {
            if (child.parentNode) child.remove();
            const index = this.children.indexOf(before);
            this.children.splice(index < 0 ? this.children.length : index, 0, child);
            child.parentNode = this;
            return child;
        }
        replaceChild(next, previous) {
            const index = this.children.indexOf(previous);
            if (index < 0) throw new Error("old loader picture is not attached");
            this.children[index] = next;
            next.parentNode = this;
            previous.parentNode = null;
            return previous;
        }
        remove() {
            if (!this.parentNode) return;
            const siblings = this.parentNode.children;
            siblings.splice(siblings.indexOf(this), 1);
            this.parentNode = null;
        }
        setAttribute(name, value) {
            this.attributes[name] = String(value);
        }
        addEventListener(type, callback) {
            this.listeners[type] = callback;
        }
        click() {
            this.listeners.click?.();
        }
        getAttribute(name) {
            if (Object.prototype.hasOwnProperty.call(this.attributes, name)) return this.attributes[name];
            return this[name] == null ? null : String(this[name]);
        }
        removeAttribute(name) {
            delete this.attributes[name];
        }
        get firstChild() {
            return this.children[0] || null;
        }
        get classList() {
            const names = () => new Set(String(this.className || "").split(/\s+/).filter(Boolean));
            return {
                add: value => { const set = names(); set.add(value); this.className = [...set].join(" "); },
                remove: value => { const set = names(); set.delete(value); this.className = [...set].join(" "); },
                toggle: (value, enabled) => { const set = names(); if (enabled) set.add(value); else set.delete(value); this.className = [...set].join(" "); },
            };
        }
        querySelector(selector) {
            const matches = child => selector.startsWith(".") && String(child.className || "").split(/\s+/).includes(selector.slice(1));
            for (const child of this.children) {
                if (matches(child)) return child;
                const nested = child.querySelector?.(selector);
                if (nested) return nested;
            }
            return null;
        }
    }

    const root = new Element("div");
    root.id = "web-loader";
    const initialPicture = new Element("picture");
    initialPicture.className = "splash-picture";
    const initialSource = new Element("source");
    initialSource.id = "splash-mobile";
    const initialImage = new Element("img");
    initialImage.id = "splash-bg";
    const embeddedSplash = "data:image/webp;base64,embedded-splash";
    initialImage.setAttribute("src", embeddedSplash);
    initialImage.src = embeddedSplash;
    initialPicture.appendChild(initialSource);
    initialPicture.appendChild(initialImage);
    root.appendChild(initialPicture);
    for (const id of [
        "loader-footer",
        "loader-ability",
        "loader-bar-wrap",
        "loader-bar-fill",
        "loader-bar-full",
        "loader-bar-empty",
        "loader-text",
    ]) {
        const child = new Element("div");
        child.id = id;
        child.hidden = id === "loader-ability";
        root.appendChild(child);
    }
    const mapError = new Element("section");
    mapError.id = "loader-map-error";
    mapError.hidden = true;
    root.appendChild(mapError);
    const mapErrorText = new Element("span");
    mapErrorText.id = "loader-map-error-text";
    root.appendChild(mapErrorText);
    const retryButton = new Element("button");
    retryButton.id = "loader-map-retry";
    root.appendChild(retryButton);
    const find = (node, predicate) =>
        predicate(node) ? node : node.children.map((child) => find(child, predicate)).find(Boolean) || null;
    const findPicture = () => find(root, (node) => node.className === "splash-picture");
    const document = {
        readyState: "loading",
        body: new Element("body"),
        createElement: (tagName) => new Element(tagName),
        getElementById: (id) => find(root, (node) => node.id === id),
        querySelector: (selector) => (selector === "#web-loader .splash-picture" ? findPicture() : null),
        addEventListener() {},
    };
    const timers = new Map();
    const events = [];
    const completedLoaderCycles = [];
    const leaderPreloads = [];
    const menuCommands = [];
    class PreloadImage {
        constructor() {
            this.parentNode = null;
            this.children = [];
        }
        set src(value) {
            this._src = value;
            leaderPreloads.push(this);
        }
        get src() {
            return this._src;
        }
        decode() {
            return Promise.resolve();
        }
        remove() {
            if (!this.parentNode) return;
            const siblings = this.parentNode.children;
            siblings.splice(siblings.indexOf(this), 1);
            this.parentNode = null;
        }
    }
    let nextTimer = 0;
    const briefingCalls = [];
    const window = {
        SOW_BOOT_UI_BASE: "/assets/shell/loader",
        SOW_BUILD_TS: "regression",
        SOW_PORTAL: "jest",
        SOW_renderHeroBriefing(leader, surface) {
            briefingCalls.push([leader, surface]);
            return `<article class="sow-hero-briefing sow-hero-briefing--${surface}">${leader} briefing</article>`;
        },
        innerWidth: 1024,
        innerHeight: 768,
        matchMedia: () => ({ matches: window.innerHeight >= window.innerWidth }),
        location: { href: "https://game.test/", origin: "https://game.test" },
        addEventListener() {},
        dispatchEvent(event) {
            events.push(event.type);
            if (event.type === "sow:loader-cycle-ready") completedLoaderCycles.push(event.detail.cycle_id);
        },
    };
    window.SOW_t = (key) => key;
    window.SOW_menu_command = (message) => menuCommands.push(JSON.parse(message));
    const context = {
        window,
        document,
        URL,
        Image: PreloadImage,
        Event: class {
            constructor(type) {
                this.type = type;
            }
        },
        CustomEvent: class {
            constructor(type, options) {
                this.type = type;
                this.detail = options.detail;
            }
        },
        performance: { now: () => 0 },
        requestAnimationFrame: () => 1,
        cancelAnimationFrame() {},
        setTimeout(callback) {
            const id = ++nextTimer;
            timers.set(id, callback);
            return id;
        },
        clearTimeout(id) {
            timers.delete(id);
        },
    };
    vm.runInNewContext(loaderSource, context);
    window.SOW_initWebLoader();
    assert.equal(findPicture(), initialPicture, "initialization preserves the HTML splash picture");
    assert.equal(
        findPicture().children[1],
        initialImage,
        "initialization preserves the embedded splash image node",
    );
    assert.equal(
        initialImage.src,
        embeddedSplash,
        "initialization does not replace the embedded splash with a network URL",
    );
    assert.equal(leaderPreloads.length, 0, "initialization creates no duplicate hero-image request");

    const leaders = [
        { id: "Boudica", slug: "boudica" },
        { id: "Caesar", slug: "caesar" },
        { id: "Ragnar", slug: "ragnar" },
    ];
    let cycleId = -1;
    let previousJob = null;
    let previousDone = false;
    const state = (job, leader, progress = 0.4, phase = "Splash", done = false, showHeroBriefing = false) => {
        if (job !== previousJob || (previousDone && !done)) cycleId += 1;
        previousJob = job;
        previousDone = done;
        return {
            phase,
            loader_cycle_id: cycleId,
            loader_job: job,
            loader_leader: leader,
            loader_info_leader: leader && leader !== "random" ? leader.toLowerCase() : "caesar",
            loader_show_hero_briefing: showHeroBriefing,
            loader_progress: progress,
            loader_done: done,
            ...(phase === "Playing" ? {} : { leaders }),
        };
    };
    window.SOW_syncWebLoader(state("Boot", null, 0.2, "Splash"));
    window.SOW_syncWebLoader({ ...state("Boot", null, 0.2, "Splash"), map_download_error: true });
    assert.equal(mapError.hidden, false, "a failed map download remains visible in the active loader");
    assert.equal(mapErrorText.textContent, "lobbies.map_download_failed");
    assert.equal(retryButton.textContent, "lobbies.map_download_retry");
    retryButton.click();
    assert.deepEqual(
        menuCommands,
        [{ type: "retry_map_download" }],
        "retry keeps the player in the same match loader and invokes the existing command",
    );
    window.SOW_syncWebLoader({ ...state("Boot", null, 0.2, "Splash"), map_download_error: false });
    assert.equal(mapError.hidden, true, "a retry clears the visible download error");
    const bootPicture = findPicture();
    const bootImage = bootPicture.children[1];
    assert.equal(bootPicture, initialPicture, "Boot updates keep the original splash picture");
    assert.equal(bootImage, initialImage, "Boot updates keep the original splash image");
    const loaderBar = document.getElementById("loader-bar-fill");

    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const pendingTutorialFetches = [];
    const tutorialCommands = [];
    window.SOW_t = (key) => (key.startsWith("tutorial.") ? "Text for " + key : key);
    window.SOW_hasText = (key) => Boolean(window.SOW_t(key));
    window.SOW_menu_command = (message) => tutorialCommands.push(JSON.parse(message));
    window.SOWCampaign = campaign;
    window.SOWCampaignView = { mount: () => ({ render() {}, destroy() {} }) };
    vm.runInNewContext(tutorialBundle, {
        window,
        document,
        performance: { now: () => 0 },
        console,
        fetch(url) {
            return new Promise((resolve) => pendingTutorialFetches.push({ url, resolve }));
        },
    });
    const bootCampaignState = {
        ...state("EnterGame", "Boudica", 0.95, "Splash"),
        boot_campaign: "boudica",
        prefer_random_leader: true,
    };
    window.SOW_tutorial_menu_state_update(bootCampaignState);
    assert.equal(pendingTutorialFetches.length, 2, "the tutorial's paired files are genuinely pending");
    assert.deepEqual(
        pendingTutorialFetches.map((request) => request.url).sort(),
        [
            "/assets/campaign/boudica.json?v=regression",
            "/assets/campaign/boudica.triggers.json?v=regression",
        ].sort(),
        "tutorial JSON carries the build stamp so deploys always refresh it",
    );
    window.SOW_syncWebLoader(bootCampaignState);
    assert.equal(findPicture(), bootPicture, "Boot -> EnterGame keeps the original picture node");
    assert.equal(findPicture().children[1], bootImage, "the tutorial does not replace the startup image");
    assert.equal(bootImage.src, embeddedSplash, "Boot states do not reload the embedded splash image");
    assert.equal(leaderPreloads.length, 1, "Boudica is preloaded during the tutorial's existing loader");
    const boudicaReady = window.SOW_prepareLeaderArt("boudica");
    const boudicaPreload = leaderPreloads[0];
    assert.equal(boudicaPreload.src, window.SOW_leaderArtUrl("boudica", "desktop"));
    assert.equal(leaderPreloads.length, 1, "the tutorial loader owns one Boudica image request");
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.96, "Splash"));
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.97, "Playing"));
    assert.equal(
        findPicture(),
        bootPicture,
        "the same splash remains while tutorial and game initialization are pending",
    );
    assert.equal(
        document.getElementById("loader-bar-fill"),
        loaderBar,
        "the progress bar node is never recreated",
    );
    assert.equal(root.style.visibility, "visible");
    assert.equal(loaderBar.style.width, "97.0%", "progress advances monotonically through tutorial startup");
    assert.equal(
        document.getElementById("loader-ability").hidden,
        true,
        "the tutorial keeps its custom loader free of the hero briefing",
    );
    assert.equal(briefingCalls.length, 0, "the tutorial never asks the shared component to render");
    assert.equal(events.length, 0, "phase changes cannot close the loader before Rust reports done");

    const roster = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"),
    );
    const definition = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"),
    );
    for (const request of pendingTutorialFetches) {
        request.resolve({
            ok: true,
            json: () =>
                Promise.resolve(request.url.split("?")[0].endsWith(".triggers.json") ? definition : roster),
        });
    }
    await new Promise(setImmediate);
    assert.equal(
        tutorialCommands.length,
        1,
        "the resolved tutorial data starts the campaign through the existing command",
    );
    assert.equal(tutorialCommands[0].type, "start_campaign_episode");
    assert.equal(tutorialCommands[0].episode_id, "boudica");
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.98, "Playing"));
    assert.equal(
        findPicture(),
        bootPicture,
        "the splash remains through game initialization after tutorial download",
    );
    assert.equal(root.style.visibility, "visible");
    assert.equal(events.length, 0);

    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    assert.equal(findPicture(), null, "the art is released after the tutorial finishes loading");
    assert.ok(events.includes("sow:loader-ready"));
    assert.equal(
        window.SOW_leaderArtUrl("boudica", "desktop"),
        "/assets/shell/leaders/boudica_desktop.webp?v=regression",
    );
    assert.equal(
        window.SOW_leaderArtUrl("boudica", "mobile"),
        "/assets/shell/leaders/boudica_mobile.webp?v=regression",
    );
    assert.equal(
        window.SOW_leaderArtUrl("boudica"),
        "/assets/shell/leaders/boudica_desktop.webp?v=regression",
        "the menu backdrop resolver defaults to the landscape variant on a landscape screen",
    );
    assert.equal(
        window.SOW_prepareLeaderArt("boudica"),
        boudicaReady,
        "requesting the same prepared URL reuses its existing promise",
    );
    assert.equal(leaderPreloads.length, 1);

    window.innerWidth = 390;
    window.innerHeight = 844;
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 0.6));
    assert.equal(
        findPicture(),
        null,
        "the completed startup splash cannot return while tutorial art is pending",
    );
    assert.equal(bootPicture.parentNode, null, "the startup picture stays detached after its only use");
    assert.equal(leaderPreloads.length, 1, "showing the loader starts no additional hero-image request");
    const tutorialExitDone = {
        ...state("ExitGame", "Boudica", 1, "MainMenu", true),
        prefer_random_leader: true,
    };
    window.SOW_syncWebLoader(tutorialExitDone);
    assert.equal(
        root.style.visibility,
        "visible",
        "loader_done cannot hide the bar before the prepared image is attached",
    );
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar);
    assert.equal(events.filter((event) => event === "sow:loader-cycle-ready").length, 1);
    boudicaPreload.onload();
    assert.equal(await boudicaReady, true);
    await new Promise(setImmediate);
    assert.equal(
        findPicture().children[0],
        boudicaPreload,
        "the same Boudica image that began loading behind Boot is attached on tutorial exit",
    );
    const exitBoudicaPicture = findPicture();
    const exitBoudicaImage = exitBoudicaPicture.children[0];
    assert.equal(exitBoudicaImage, boudicaPreload);
    assert.match(
        boudicaPreload.src,
        /leaders\/boudica_desktop\.webp/,
        "exit keeps the exact prepared variant after orientation changes",
    );
    assert.equal(leaderPreloads.length, 1, "the full tutorial cycle creates one Boudica image");
    assert.equal(
        document.getElementById("loader-bar-fill"),
        loaderBar,
        "tutorial exit keeps the one original progress bar",
    );
    [...timers.values()][0]();
    timers.clear();
    assert.ok(events.includes("sow:loader-cycle-ready"));
    assert.equal(completedLoaderCycles.filter((id) => id === tutorialExitDone.loader_cycle_id).length, 1);
    window.SOW_syncWebLoader({ ...tutorialExitDone, loader_progress: 1 });
    window.SOW_syncWebLoader({ ...tutorialExitDone, loader_progress: 1 });
    assert.equal(
        root.style.visibility,
        "hidden",
        "repeated completed MainMenu snapshots cannot reopen the old ExitGame cycle",
    );
    assert.equal(root.style.pointerEvents, "none", "the completed loader cannot block menu or lobby input");
    assert.equal(findPicture(), null, "the old Boudica image stays detached after the cycle is consumed");
    assert.equal(
        completedLoaderCycles.filter((id) => id === tutorialExitDone.loader_cycle_id).length,
        1,
        "one cycle dispatches one completion event",
    );
    window.innerWidth = 1024;
    window.innerHeight = 768;
    const reusedBoudicaReady = window.SOW_prepareLeaderArt("boudica");
    assert.equal(
        reusedBoudicaReady,
        boudicaReady,
        "the decoded image remains prepared after the loader cycle",
    );
    assert.equal(leaderPreloads.length, 1, "the finished cycle does not trigger another Boudica image");
    assert.equal(await reusedBoudicaReady, true);

    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.45, "Splash", false, true));
    const normalEntryPicture = findPicture();
    assert.equal(
        document.getElementById("loader-ability").hidden,
        false,
        "normal matches show the shared hero briefing",
    );
    assert.match(document.getElementById("loader-ability").innerHTML, /sow-hero-briefing--loader/);
    assert.deepEqual(briefingCalls.at(-1), ["boudica", "loader"]);
    assert.equal(
        normalEntryPicture.children[0],
        boudicaPreload,
        "a normal game entry displays the already-decoded selected hero",
    );
    assert.equal(leaderPreloads.length, 1);
    const normalEntryDone = state("EnterGame", "Boudica", 1, "Playing", true, true);
    assert.equal(
        normalEntryDone.leaders,
        undefined,
        "the real Playing update does not resend the hero catalog",
    );
    window.SOW_syncWebLoader(normalEntryDone);
    assert.equal(
        findPicture(),
        normalEntryPicture,
        "completion preserves the visible hero until the close timer runs",
    );
    assert.equal(findPicture().children[0], boudicaPreload);
    assert.equal(bootPicture.parentNode, null);
    assert.equal(leaderPreloads.length, 1, "completion does not start another image request");
    [...timers.values()][0]();
    timers.clear();
    window.innerWidth = 390;
    window.innerHeight = 844;
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 0.45));
    const normalExitPicture = findPicture();
    assert.equal(
        normalExitPicture.children[0],
        boudicaPreload,
        "the same decoded image is reused for game exit",
    );
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 1, "MainMenu", true));
    assert.equal(
        findPicture().children[0],
        boudicaPreload,
        "the final done update adopts the exit image before closing",
    );
    assert.equal(leaderPreloads.length, 1, "entry and exit create no second Boudica image");
    [...timers.values()][0]();
    timers.clear();
    window.innerWidth = 1024;
    window.innerHeight = 768;

    const failedReady = window.SOW_prepareLeaderArt("caesar");
    const failedPreload = leaderPreloads[leaderPreloads.length - 1];
    failedPreload.onerror();
    assert.equal(await failedReady, false);
    const requestCountBeforeUnreadyLoader = leaderPreloads.length;
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.35));
    assert.equal(
        findPicture(),
        null,
        "failed preload leaves the existing dark background, never the startup splash",
    );
    assert.equal(root.style.visibility, "visible");
    assert.equal(loaderBar.style.width, "35.0%");
    assert.equal(leaderPreloads.length, requestCountBeforeUnreadyLoader);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();

    window.innerWidth = 390;
    window.innerHeight = 844;
    assert.equal(
        window.SOW_leaderArtUrl("caesar"),
        "/assets/shell/leaders/caesar_mobile.webp?v=regression",
        "the menu backdrop resolver defaults to the portrait variant on a portrait screen",
    );
    const caesarReady = window.SOW_prepareLeaderArt("caesar");
    const caesarPreload = leaderPreloads.at(-1);
    assert.match(caesarPreload.src, /leaders\/caesar_mobile\.webp\?v=regression/);
    caesarPreload.onload();
    assert.equal(await caesarReady, true);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.45));
    const caesarPicture = findPicture();
    const caesarImage = caesarPicture.children[0];
    assert.notEqual(caesarPicture, exitBoudicaPicture, "a separate game load gets fresh art");
    assert.equal(exitBoudicaPicture.parentNode, null, "finished-cycle art is removed");
    assert.equal(caesarImage, caesarPreload, "the selected hero's decoded art is adopted by the loader");
    assert.match(caesarImage.src, /leaders\/caesar_mobile\.webp/);
    assert.equal(leaderPreloads.length, 3, "the loader adds no request after the selection preload");
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.55));
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.65));
    assert.equal(
        findPicture(),
        caesarPicture,
        "repeated progress updates do not replace the active hero picture",
    );
    assert.equal(
        findPicture().children[0],
        caesarImage,
        "repeated progress updates preserve the decoded image node",
    );
    assert.equal(leaderPreloads.length, 3, "repeated progress updates create no new Image");
    assert.equal(loaderBar.style.width, "65.0%");
    exitBoudicaImage.onerror();
    assert.equal(findPicture(), caesarPicture, "a delayed old-image error cannot touch the current hero");

    caesarImage.onerror();
    assert.equal(findPicture(), null, "a failed current image cannot restore the released startup splash");
    assert.equal(root.style.visibility, "visible", "art failure does not close the loader");
    assert.equal(document.getElementById("loader-bar-fill").style.width, "65.0%");

    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    assert.equal(events.filter((event) => event === "sow:loader-ready").length, 1);
    assert.equal(
        events.filter((event) => event === "sow:loader-cycle-ready").length,
        6,
        "each completed loader cycle emits one ready event",
    );

    window.innerWidth = 1024;
    window.innerHeight = 768;
    const nextCaesarReady = window.SOW_prepareLeaderArt("caesar");
    const nextCaesarPreload = leaderPreloads.at(-1);
    nextCaesarPreload.onload();
    assert.equal(await nextCaesarReady, true);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.45));
    const nextCaesarPicture = findPicture();
    assert.equal(nextCaesarPicture.children[0], nextCaesarPreload);
    assert.match(
        nextCaesarPreload.src,
        /leaders\/caesar_desktop\.webp\?v=regression/,
        "the loader follows the same landscape resolver after orientation changes",
    );
    caesarImage.onerror();
    assert.equal(
        findPicture(),
        nextCaesarPicture,
        "an old cycle cannot invalidate the same hero in a new cycle",
    );
    nextCaesarPicture.children[0].onerror();
    assert.equal(findPicture(), null, "a failed current image cannot restore the released startup splash");
    assert.equal(root.style.visibility, "visible", "art failure does not close the loader");
    assert.equal(document.getElementById("loader-bar-fill").style.width, "45.0%");

    const pendingExitReady = window.SOW_prepareLeaderArt("ragnar");
    const pendingExitImage = leaderPreloads.at(-1);
    window.SOW_syncWebLoader(state("ExitGame", "Ragnar", 0.45));
    assert.equal(
        findPicture(),
        null,
        "a pending exit keeps the dark background until that same request is ready",
    );
    const imageCountBeforePendingExitReady = leaderPreloads.length;
    pendingExitImage.onload();
    assert.equal(await pendingExitReady, true);
    window.SOW_syncWebLoader(state("ExitGame", "Ragnar", 1, "MainMenu", true));
    assert.equal(
        findPicture().children[0],
        pendingExitImage,
        "a completed pending preload recovers from unready and is attached before finish",
    );
    assert.equal(
        leaderPreloads.length,
        imageCountBeforePendingExitReady,
        "the final exit sync reuses the in-flight image",
    );
    [...timers.values()][0]();
    timers.clear();
    assert.equal(findPicture(), null);
    assert.equal(
        document.getElementById("loader-bar-fill"),
        loaderBar,
        "entry and exit preserve the same single progress bar",
    );
    assert.equal(events.filter((event) => event === "sow:loader-ready").length, 1);
    assert.equal(events.filter((event) => event === "sow:loader-cycle-ready").length, 7);

    let previousRandomImage = null;
    for (const [variant, width, height] of [
        ["desktop", 1024, 768],
        ["mobile", 390, 844],
    ]) {
        window.innerWidth = width;
        window.innerHeight = height;
        const randomReady = window.SOW_prepareLeaderArt("random");
        const randomImage = leaderPreloads.at(-1);
        assert.equal(randomImage.src, "/assets/shell/leaders/random_" + variant + ".webp?v=regression");
        randomImage.onload();
        assert.equal(await randomReady, true);
        assert.equal(
            window.SOW_prepareLeaderArt("random"),
            randomReady,
            "Random reuses the menu's prepared URL",
        );
        const requestCount = leaderPreloads.length;

        window.SOW_syncWebLoader(state("EnterGame", "random", 0.45, "Splash", false, true));
        const randomPicture = findPicture();
        assert.equal(
            randomPicture.children[0],
            randomImage,
            "Random enters through the same prepared-image path as a hero",
        );
        assert.equal(document.getElementById("loader-ability").hidden, false);
        assert.match(
            document.getElementById("loader-ability").innerHTML,
            /random briefing/,
            "random art displays the random option instead of the resolved leader's ability",
        );
        assert.deepEqual(briefingCalls.at(-1), ["random", "loader"]);
        if (previousRandomImage) {
            previousRandomImage.onerror();
            assert.equal(
                findPicture(),
                randomPicture,
                "an old Random variant cannot touch the current cycle",
            );
        }
        window.SOW_syncWebLoader(state("EnterGame", "random", 0.9, "Splash", false, true));
        assert.equal(findPicture(), randomPicture);
        const randomEntryDone = state("EnterGame", "random", 1, "Playing", true, true);
        assert.equal(randomEntryDone.leaders, undefined);
        window.SOW_syncWebLoader(randomEntryDone);
        assert.equal(findPicture(), randomPicture, "Random stays visible throughout the final close window");
        assert.equal(bootPicture.parentNode, null);
        [...timers.values()][0]();
        timers.clear();

        window.innerWidth = height;
        window.innerHeight = width;
        window.SOW_syncWebLoader(state("ExitGame", "random", 0.45));
        assert.equal(
            findPicture().children[0],
            randomImage,
            "exit retains the entry image even after an orientation change",
        );
        const randomExitDone = state("ExitGame", "random", 1, "MainMenu", true);
        window.SOW_syncWebLoader(randomExitDone);
        assert.equal(findPicture().children[0], randomImage);
        assert.equal(
            leaderPreloads.length,
            requestCount,
            "Random entry and exit create no second image request",
        );
        [...timers.values()][0]();
        timers.clear();
        window.SOW_syncWebLoader(randomExitDone);
        assert.equal(root.style.visibility, "hidden");
        assert.equal(findPicture(), null);
        assert.equal(document.getElementById("loader-bar-fill"), loaderBar);
        previousRandomImage = randomImage;
    }

    const requestCountAfterMatches = leaderPreloads.length;
    window.SOW_syncWebLoader(state("Boot", null, 0.2));
    assert.equal(findPicture(), null, "later Boot states cannot recreate the startup splash");
    assert.equal(bootPicture.parentNode, null);
    assert.equal(leaderPreloads.length, requestCountAfterMatches);
    window.SOW_syncWebLoader(state("Boot", null, 1, "MainMenu", true));
    [...timers.values()][0]();
    timers.clear();
    assert.equal(events.filter((event) => event === "sow:loader-ready").length, 1);
    const rankedLoad = { ...state("EnterGame", "Boudica", 0.3), ranked_matchup_html: "<div>Me VS Rival</div>" };
    window.SOW_syncWebLoader(rankedLoad);
    const matchup = root.querySelector(".sow-ranked__loader");
    assert.ok(matchup, "the existing loader carries the confirmed encounter");
    assert.equal(matchup.innerHTML, rankedLoad.ranked_matchup_html);
    window.SOW_syncWebLoader({ ...rankedLoad, loader_progress: 0.8 });
    assert.equal(root.querySelector(".sow-ranked__loader"), matchup, "progress updates keep the same matchup node");
    window.SOW_syncWebLoader({ ...rankedLoad, loader_done: true, loader_progress: 1 });
    assert.equal(root.querySelector(".sow-ranked__loader"), null, "a ready game never waits for the matchup animation");
    [...timers.values()][0]();
    timers.clear();
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 0.4));
    assert.equal(root.querySelector(".sow-ranked__loader"), null, "the encounter does not leak into the return loader");
});

test("game exit resets Battle and replays the screen entrance after the loader", () => {
    const resetStart = shellSource.indexOf("if (returnedFromGame) {");
    const resetEnd = shellSource.indexOf("if (typeof window.SOW_tutorial_menu_state_update", resetStart);
    assert.ok(resetStart >= 0 && resetEnd > resetStart);
    const resetBlock = shellSource.slice(resetStart, resetEnd);
    for (const flag of ["profileOpen", "heroesOpen", "campaignOpen", "storeOpen"]) {
        assert.match(resetBlock, new RegExp(flag + " = false"));
    }
    assert.match(resetBlock, /pendingExitScreenIntro = true/);
});

test("shell media URLs bust by content hash when the build ships one", () => {
    const assetUrlStart = loaderSource.indexOf("    function assetUrl(");
    const assetUrlEnd = loaderSource.indexOf("    function leaderArtUrl(", assetUrlStart);
    assert.ok(assetUrlStart >= 0 && assetUrlEnd > assetUrlStart);
    const context = { window: {} };
    vm.runInNewContext(
        loaderSource.slice(assetUrlStart, assetUrlEnd) + "; this.assetUrl = assetUrl;",
        context,
    );
    context.window.SOW_BUILD_TS = "build-42";
    assert.equal(
        context.assetUrl("/assets/shell/leaders/boudica_desktop.webp"),
        "/assets/shell/leaders/boudica_desktop.webp?v=build-42",
        "without a content entry the build bust still applies",
    );
    context.window.SOW_ASSET_TS = { "/assets/shell/leaders/other_desktop.webp": "cafe1234" };
    assert.equal(
        context.assetUrl("/assets/shell/leaders/boudica_desktop.webp"),
        "/assets/shell/leaders/boudica_desktop.webp?v=build-42",
        "an unlisted path keeps the build bust",
    );
    context.window.SOW_ASSET_TS = { "/assets/shell/leaders/boudica_desktop.webp": "cafe1234" };
    assert.equal(
        context.assetUrl("/assets/shell/leaders/boudica_desktop.webp"),
        "/assets/shell/leaders/boudica_desktop.webp?v=cafe1234",
        "a content hash wins over the build stamp so unrelated deploys keep the cache",
    );
});
