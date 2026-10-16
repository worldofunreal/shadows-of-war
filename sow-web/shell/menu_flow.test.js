const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = __dirname;
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.js"), "utf8");
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const menuGuideDefinition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/guides/menu.triggers.json"), "utf8"));
const menuFlowApi = require(path.join(shell, "main_menu.flow.js"));
const campaignRuntime = require(path.join(shell, "sow-campaign.js"));
const storeSource = fs.readFileSync(path.join(shell, "main_menu.store.js"), "utf8");
const heroesSource = fs.readFileSync(path.join(shell, "main_menu.heroes.js"), "utf8");
const profileCss = fs.readFileSync(path.join(shell, "main_menu.profile.css"), "utf8");
const profileSource = fs.readFileSync(path.join(shell, "main_menu.profile.js"), "utf8");
const nativeProfileSource = [
    "../../sow-client/src/app/account.rs",
    "../../sow-client/src/asset.rs",
    "../../sow-client/src/lib.rs",
    "../../sow-client/src/player_progress.rs",
    "../../sow-client/src/render/interact/actions.rs",
    "../../sow-client/src/ui/main_menu/mod.rs",
    "../../sow-client/src/ui.rs"
].map((file) => fs.readFileSync(path.join(shell, file), "utf8")).join("\n");
const loaderSource = fs.readFileSync(path.join(shell, "loader.js"), "utf8");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.js"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const siteDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/main.rs"), "utf8");
const prodDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/prod.rs"), "utf8");
const siteChrome = fs.readFileSync(path.join(shell, "../../sow-web/site/site-chrome.js"), "utf8");
const siteDropdown = fs.readFileSync(path.join(shell, "sow-dropdown.js"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const matchStartSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/match_start.rs"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");
const sessionSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/session.rs"), "utf8");
const netMessagesSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/update/messages.rs"), "utf8");
const actionsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/interact/actions.rs"), "utf8");
const netUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/update/mod.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const appStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/state.rs"), "utf8");
const progressSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/progress.rs"), "utf8");
const dataDbSource = fs.readFileSync(path.join(shell, "../../sow-data/src/db.rs"), "utf8");
const dataMainSource = fs.readFileSync(path.join(shell, "../../sow-data/src/main.rs"), "utf8");
const accountSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/account.rs"), "utf8");
const bootstrapSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/bootstrap.rs"), "utf8");
const identitySource = fs.readFileSync(path.join(shell, "../../sow-client/src/anonymous_identity.rs"), "utf8");
const assetSource = fs.readFileSync(path.join(shell, "../../sow-client/src/asset.rs"), "utf8");
const relaySource = fs.readFileSync(path.join(shell, "../../sow-relay/src/main.rs"), "utf8");
const serverSource = fs.readFileSync(path.join(shell, "../../sow-server/src/main.rs"), "utf8");
const menuCssFiles = [
    "main_menu.base.css",
    "main_menu.layout.css",
    "main_menu.lobbies.css",
    "main_menu.create.css",
    "main_menu.queue.css",
    "main_menu.settings.css",
    "main_menu.auth.css",
    "main_menu.campaign.css"
];
const menuCss = Object.fromEntries(menuCssFiles.map((file) => [file, fs.readFileSync(path.join(shell, file), "utf8")]));
function renderSettingsBody(source, endMarker) {
    const start = source.indexOf("function renderSettings()");
    const end = source.indexOf(endMarker, start);
    assert.notEqual(start, -1);
    assert.notEqual(end, -1);
    return source.slice(start, end);
}
function makeMenuGuideHarness(storage = new Map()) {
    const calls = [], renderedSteps = [], saveTimers = [];
    let clockNow = Date.now();
    let destroyedViews = 0;
    let chooseMenuGuideStep = null;
    class FakeElement {
        constructor() {
            this.hidden = false;
            this.isConnected = false;
            this.dataset = {};
            this.style = {};
            const classes = new Set();
            this.classList = {
                add: (...names) => names.forEach(name => classes.add(name)),
                remove: (...names) => names.forEach(name => classes.delete(name)),
                contains: name => classes.has(name),
                toggle: (name, force) => force ? classes.add(name) : classes.delete(name)
            };
        }
        appendChild(child) { child.isConnected = true; return child; }
        contains() { return false; }
        querySelector() { return null; }
        querySelectorAll() { return []; }
        addEventListener() {}
        removeEventListener() {}
    }
    const body = new FakeElement();
    const document = {
        body,
        documentElement: { lang: "en", dir: "ltr", dataset: {} },
        visibilityState: "visible",
        getElementById: () => null,
        createElement: () => new FakeElement(),
        querySelector: () => null,
        querySelectorAll: () => [],
        addEventListener() {}
    };
    const window = {
        SOW_ASSETS_URL: "/assets",
        SOWCampaign: campaignRuntime,
        SOWCampaignView: {
            mount(_root, options) {
                chooseMenuGuideStep = options && options.onChoice;
                return {
                    render(model) { if (model && model.step) renderedSteps.push(model.step.id); },
                    destroy() { destroyedViews++; chooseMenuGuideStep = null; }
                };
            }
        },
        SOW_menu_flow: menuFlowApi.create(),
        localStorage: {
            getItem: key => storage.has(key) ? storage.get(key) : null,
            setItem: (key, value) => storage.set(key, String(value)),
            removeItem: key => storage.delete(key)
        },
        SOW_menu_command: payload => calls.push(JSON.parse(payload)),
        setTimeout: callback => { saveTimers.push(callback); return saveTimers.length; }, clearTimeout() {},
        addEventListener() {}
    };
    body.appendChild = child => { child.isConnected = true; return child; };
    const context = {
        document,
        window,
        Date: class extends Date { static now() { return clockNow; } },
        fetch: async () => ({ ok: true, json: async () => menuGuideDefinition }),
        console: { error: console.error }
    };
    vm.runInNewContext(tutorial, context, { filename: "main_menu.tutorial.js" });
    return {
        flushSaveRetries() { clockNow += 2500; saveTimers.splice(0).forEach(callback => callback()); },
        calls, renderedSteps, storage, flow: window.SOW_menu_flow,
        guide: window.SOW_menu_guide,
        choose: choice => { if (chooseMenuGuideStep) chooseMenuGuideStep(choice); },
        window, document,
        get destroyedViews() { return destroyedViews; }
    };
}
function makeMenuShellRewardHarness(guideHarness, initialState, initialScreen = "home") {
    const progressionStart = shellSource.indexOf("    function progressionFromState()");
    const progressionEnd = shellSource.indexOf("    function animateProgressionTo(", progressionStart);
    const presentationStart = shellSource.indexOf("    function runRewardPresentation(receipts)");
    const presentationEnd = shellSource.indexOf("    function maybePresentRewards()", presentationStart);
    const flowEnd = shellSource.indexOf("    function updateDynamic()", presentationEnd);
    assert.ok(progressionStart >= 0 && progressionEnd > progressionStart
        && presentationStart >= 0 && presentationEnd > presentationStart && flowEnd > presentationEnd,
    "the shell reward presenter and guide handoff are present");
    const rewardStageSets = [], rewardStageRuns = [], rewardPulses = [], acknowledgements = [], logs = [];
    let holdReveal = false, releaseReveal = null;
    const context = {
        window: Object.assign(guideHarness.window, { SOW_PORTAL: "site" }),
        document: { visibilityState: "visible" },
        Object, Number, Array, Math,
        state: initialState,
        rewardStageSets, rewardStageRuns, rewardPulses, acknowledgements, logs,
        screen: initialScreen,
        menuFlow: guideHarness.flow,
        rewardPresentationReady: true,
        lastMenuFlowPhase: null,
        holdPresentation() { holdReveal = true; },
        releasePresentation() { holdReveal = false; if (releaseReveal) releaseReveal(true); releaseReveal = null; },
        rewardAnimationRunning: false,
        rewardAnimationToken: 0,
        rewardAnimationShown: new Set(),
        displayedProgression: null,
        canonicalProgression: null,
        activeRewardStage: null,
        root: { hidden: false, querySelector: () => null },
        menuOutcomeCounts: Object.create(null),
        menuOutcomeReady: Object.create(null),
        profileAccountId: null,
        profileTab: "overview",
        profileHistory: [],
        purchaseModal: null,
        purchasedLeaderId: null,
        tempSelectedLeader: null,
        storeCatalogLoaded: false,
        storeCatalogLoading: false,
        storeOpen: false,
        profileEntry: null,
        profileDataIsComplete: false,
        currentScreen: () => context.screen,
        activeProfileEntry: () => context.profileEntry,
        profileDataComplete: () => context.profileDataIsComplete,
        SOW_t: key => key,
        createRewardStage(stages) {
            rewardStageSets.push(stages.map(stage => ({ kind: stage.kind, amount: stage.amount, level: stage.level })));
            const layer = { classList: { add() {} } };
            context.activeRewardStage = layer;
            return stages.map(stage => ({ stage, card: { style: {} }, amount: { textContent: "" } }));
        },
        revealRewardAmounts(items) {
            if (!holdReveal) return Promise.resolve(true);
            return new Promise(resolve => { releaseReveal = resolve; });
        },
        flyRewardCard(item) {
            rewardStageRuns.push({ kind: item.stage.kind, level: item.stage.level });
            return Promise.resolve(true);
        },
        pulseRewardCounter(selector, levelUp) {
            rewardPulses.push({ selector, levelUp, level: context.displayedProgression && context.displayedProgression.level });
        },
        removeRewardStage() { context.activeRewardStage = null; },
        acknowledgeShownReceipts(accountId) { acknowledgements.push(accountId); },
        console: { info(...args) { logs.push(args); }, error() {} },
        Date,
        Promise,
        Set,
        render: () => { context.screen = context.storeOpen ? "store" : context.screen; }
    };
    const scheduleSave = context.window.setTimeout;
    context.window.setTimeout = (callback, delay) => { if (Number(delay) >= 2000) return scheduleSave(callback); callback(); return 1; };
    context.window.clearTimeout = () => {};
    context.window.matchMedia = () => ({ matches: false });
    context.window.SOW_menu_guide = guideHarness.guide;
    context.window.SOW_PORTAL = "site";
    context.console = { info(...args) { logs.push(args); }, error() {} };
    const rewardSource = shellSource.slice(progressionStart, progressionEnd)
        + shellSource.slice(presentationStart, presentationEnd)
        + shellSource.slice(presentationEnd, flowEnd);
    const animationShim = "function animateProgressionTo(target, duration, token, levelOverride) { var values = Object.assign({}, target); if (levelOverride != null) values.level = levelOverride; writeProgression(values); return Promise.resolve(true); }\n";
    vm.runInNewContext(rewardSource + animationShim +
        "this.menuRewardApi = { maybePresentRewards, menuGuideFacts, setState(value) { state = value; }, setOutcome(name, value) { menuOutcomeCounts[name] = value; }, setPurchasePrompt(id, phase) { purchaseModal = id ? { type: 'leader', leaderId: id, phase: phase || 'confirm' } : null; }, setPurchasedLeader(id) { purchasedLeaderId = id; }, setPreviewLeader(id) { tempSelectedLeader = id; }, setRunning(value) { rewardAnimationRunning = value; }, setScreen(value) { screen = value; storeOpen = false; }, getScreen() { return screen; }, setProfile(value) { profileAccountId = value.accountId; profileTab = value.tab || 'overview'; profileEntry = value.entry || null; profileHistory = value.history || []; profileDataIsComplete = value.complete === true; }, setStoreReady(loaded, loading) { storeCatalogLoaded = loaded; storeCatalogLoading = loading; }, holdPresentation, releasePresentation, get phase() { return menuFlow.phase(); }, get shownReceipts() { return Array.from(rewardAnimationShown); }, get pendingReceipts() { return pendingRewardReceipts().map(function (receipt) { return receipt.id; }); }, get progression() { return displayedProgression; }, get facts() { return menuGuideFacts(); }, rewardStageSets, rewardStageRuns, rewardPulses, acknowledgements, logs };",
    context);
    return context.menuRewardApi;
}
function confirmedMenuReturn(overrides = {}) {
    const receipt = { id: "tutorial", match_id: "tutorial", status: "applied", level_before: 1, level_after: 2 };
    const state = {
        phase: "MainMenu",
        account_id: "account-a",
        loader_cycle_id: "cycle-1",
        profile_ready: true,
        menu_return: {
            account_id: "account-a", cycle_id: "cycle-1", source: "tutorial", match_id: "tutorial",
            receipt_ids: ["tutorial"], status: "confirmed", requested_screen: null
        },
        reward_receipts: [receipt],
        menu_guides: { welcome: { completed_steps: [], status: "active", selected_leader: null } },
        selected_leader: "caesar",
        leaders: [{ id: "caesar", available: true }, { id: "boudica", available: true }],
        menu_guide_save_revision: 0,
        menu_guide_save_pending: false
    };
    return { state: Object.assign(state, overrides), receipt };
}

test("live multiplayer return uses the confirmed client state", { skip: !process.env.SOW_LIVE_RETURN_STATE_FILE }, async () => {
    const state = JSON.parse(fs.readFileSync(process.env.SOW_LIVE_RETURN_STATE_FILE, "utf8"));
    assert.equal(state.menu_return.source, "multiplayer");
    assert.equal(state.menu_return.status, "confirmed");
    assert.equal(state.menu_return.account_id, state.account_id);
    const matchId = state.menu_return.match_id;
    assert.ok(state.reward_receipts.some(receipt => receipt.id === matchId && receipt.verification_status === "verified"));
    const guide = makeMenuGuideHarness();
    const presenter = makeMenuShellRewardHarness(guide, state);
    presenter.holdPresentation();
    presenter.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(presenter.phase, "rewards");
    assert.deepEqual(guide.renderedSteps, [], "real confirmed rewards must finish before the guide");
    presenter.releasePresentation();
    for (let step = 0; step < 100 && presenter.phase === "rewards"; step++) {
        await new Promise(resolve => setImmediate(resolve));
    }
    assert.ok(presenter.rewardStageRuns.some(stage => stage.kind === "xp"));
    assert.ok(presenter.shownReceipts.includes(matchId));
    assert.equal(presenter.phase, "guide");
    const count = presenter.rewardStageSets.length;
    presenter.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(presenter.rewardStageSets.length, count, "confirmed receipts are presented once");
    const fixture = JSON.parse(fs.readFileSync(process.env.SOW_LIVE_MATCH_EXIT_FILE, "utf8"));
    const ack = await fetch(fixture.database_base + "/profile/reward-receipts/ack", {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ account_id: fixture.account.account_id, auth_secret: fixture.account.auth_secret,
            receipt_ids: presenter.shownReceipts })
    });
    assert.equal(ack.status, 200);
    const profile = await ack.json();
    assert.equal(profile.profile.xp, state.xp);
});


test("mobile menu renders its language selector after locale bootstrap", async () => {
    const controls = Array.from({ length: 2 }, () => ({ className: "", innerHTML: "", querySelector: () => null }));
    const menu = { classList: { toggle() {} }, setAttribute() {}, querySelectorAll: () => [] };
    const menuToggle = { setAttribute() {}, addEventListener() {} };
    const document = {
        documentElement: {},
        querySelectorAll: selector => selector === "[data-locale-select]" ? controls : [],
        querySelector: selector => selector === "#mobile-menu" ? menu : selector === "[data-menu-toggle]" ? menuToggle : null,
        addEventListener() {}
    };
    const window = {
        SOW_I18N_READY: Promise.resolve(),
        SOW_t: key => key,
        SOW_getLocale: () => "en",
        SOW_getLocaleTag: locale => locale,
        SOW_getSupportedLocales: () => ["en", "es"],
        SOW_getLocaleLabel: locale => locale,
        addEventListener() {}
    };
    const context = { document, window };
    vm.runInNewContext(siteDropdown, context);
    vm.runInNewContext(siteChrome, context);
    await window.SOW_I18N_READY;
    await Promise.resolve();
    assert.match(controls[1].innerHTML, /sow-control-dropdown__trigger/);
});


test("menu CSS keeps shared layout separate from live screen domains", () => {
    for (const file of menuCssFiles) assert.ok(menuCss[file].length > 0, file);
    assert.match(menuCss["main_menu.base.css"], /#sow-menu/);
    assert.match(menuCss["main_menu.layout.css"], /sow-menu__main-nav/);
    assert.match(menuCss["main_menu.lobbies.css"], /sow-menu__lobbies/);
    assert.match(menuCss["main_menu.create.css"], /sow-create__/);
    assert.match(menuCss["main_menu.queue.css"], /sow-menu__queue-summary-card/);
    assert.match(menuCss["main_menu.settings.css"], /sow-menu__settings-modal/);
    assert.match(menuCss["main_menu.auth.css"], /sow-auth__/);
    assert.match(menuCss["main_menu.campaign.css"], /sow-campaign__/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /sow-(create|auth|campaign)__/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /\.sow-menu__queue(?![-\w])/);
    assert.doesNotMatch(menuCss["main_menu.layout.css"], /\.sow-menu__queue(?![-\w])/);
    assert.match(lobbiesSource, /modeClass = "sow-menu__mode-chip--"/);
    assert.match(shellSource, /sow-auth__provider-icon--/);
});


test("match rewards present in the main menu, while endgame keeps only match stats", () => {
    assert.match(hud, /id="sow-hud-endgame-modal"/);
    assert.match(hud, /endgame\.classList\.toggle\("hidden", !isOver\)/);
    assert.match(webMenu, /endgame_active,[\s\S]*endgame_winner:[\s\S]*endgame_team:/);
    assert.match(hud, /id="sow-hud-surrender-kda"/);
    assert.doesNotMatch(hud, /sow-hud-endgame-(xp|leader-xp|crowns|laurels|rewards)/);
    assert.doesNotMatch(sessionSource, /submit_online_stats_on_exit/);
    assert.doesNotMatch(progressSource, /submit_online_stats|SubmitMatchReport|SubmitStatsWithLeader/);
    assert.doesNotMatch(progressSource, /capture_online_reward_preview|preview_participation_laurels/);
    assert.doesNotMatch(progressSource, /capture_tutorial_reward_preview|store_exit_reward_preview/);
    assert.match(sessionSource, /tutorial_reward_id[\s\S]*begin_reward_profile_sync\(Some\(receipt_id\), !was_offline\)/);
    assert.doesNotMatch(shellSource, /priorTarget|preview\["base_"/);
    assert.match(sessionSource, /reward_match_id\.map\(\|id\| id\.to_string\(\)\)[\s\S]*begin_reward_profile_sync\(Some\(receipt_id\), !was_offline\)/);
    assert.doesNotMatch(progressSource, /reward_cache/);
    assert.match(dataDbSource, /pub async fn record_match_participation/);
    assert.match(dataDbSource, /reward_receipts\.contains_key/);
    assert.doesNotMatch(shellSource, /data-reward-summary|reward-toast/);
    assert.match(shellSource, /sow:loader-cycle-ready/);
    assert.match(shellSource, /runRewardPresentation/);
    assert.match(shellSource, /createRewardStage\(stages/);
    assert.match(shellSource, /for \(var level = startLevel \+ 1; level <= target\.level; level\+\+\)/);
    assert.match(shellSource, /kind: "level",[\s\S]*selector: "\[data-progression-level-value\]"/);
    assert.match(shellSource, /SOW_t\("profile\.level", \{ level: stage\.level \}\)/);
    assert.match(shellSource, /var heldLevel = levelStages\.length \? oldLevel : null;[\s\S]*animateProgressionTo\(nextValues, 380, token, heldLevel\)/);
    assert.match(shellSource, /layer\.addEventListener\("click", function \(event\) \{\s*event\.preventDefault\(\);\s*event\.stopPropagation\(\);\s*skip\(\);\s*\}\);/);
    assert.match(shellSource, /createRewardStage\(stages, function \(\) \{ finish\(true\); \}\)/);
    assert.match(shellSource, /revealRewardAmounts\(items, token\)/);
    assert.match(shellSource, /flyRewardCard\(item, token\)\.then[\s\S]*animateProgressionTo\(nextValues, 380, token(?:, heldLevel)?\)/);
    assert.match(shellSource, /2 \* inverse \* t \* controlX \+ t \* t \* dx/);
    assert.match(shellSource, /gems: from\.gems \+ \(target\.gems - from\.gems\) \* eased/);
    assert.doesNotMatch(shellSource, /writeProgression\(Object\.assign\(\{\}, displayedProgression, \{ gems: Number\(state\.gems\)/);
    assert.match(shellSource, /function pulseRewardCounter\(selector, levelUp\)/);
    assert.match(menuCss["main_menu.base.css"], /\.sow-menu__reward-stage/);
    assert.match(menuCss["main_menu.base.css"], /sow-reward-card-reveal/);
    assert.match(menuCss["main_menu.base.css"], /sow-menu__reward-card--level/);
    assert.match(menuCss["main_menu.base.css"], /sow-reward-value-impact/);
    assert.match(shellSource, /acknowledge_reward_receipts/);
    assert.match(shellSource, /send\("acknowledge_reward_receipts", \{ account_id: accountId, receipt_ids: ids \}\)/);
    assert.match(shellSource, /receipt\.id === "tutorial"[\s\S]*receipt\.status === "applied"/);
    assert.doesNotMatch(shellSource, /exit_reward_preview|acknowledge_reward_presentation/);
    assert.match(webMenu, /AcknowledgeRewardReceipts \{[\s\S]*account_id,[\s\S]*receipt_ids,[\s\S]*progress_account_id\.as_deref\(\) == Some\(account_id\.as_str\(\)\)/);
    assert.doesNotMatch(webMenu, /AcknowledgeRewardPresentation|exit_reward_preview/);
    assert.match(shellSource, /if \(!shown\) \{\s*writeProgression\(progressionFromState\(\)\);\s*if \(document\.visibilityState === "visible"\) maybePresentRewards\(\);/);
    assert.doesNotMatch(relaySource, /record_client_stats|record_match_report|SubmitStatsWithLeader|SubmitMatchReport/);
    assert.match(serverSource, /SubmitStatsWithLeader \{ \.\. \}[\s\S]*SubmitMatchReport \{ \.\. \} => \{\}/);
    assert.doesNotMatch(webMenu, /hud\.rewards/);
    assert.match(coreSource, /receipt\.leader_xp/);
    assert.match(coreSource, /return leaderArtUrl\(leader\.slug\);/);
    // The heroes featured art mapping is guarded by its own test:
    // "heroes featured art follows the panel shape, not the device name".
});


test("menu return waits for its exact server receipt and finished presentation", () => {
    const flow = menuFlowApi.create();
    const { state, receipt } = confirmedMenuReturn();
    const view = { visible: true, ready: true, running: false, receipts: [], screen: "home" };
    const attempt = candidate => flow.update(candidate, view);

    assert.equal(attempt({ ...state, menu_return: { ...state.menu_return, status: "waiting" } }), "waiting");
    assert.equal(attempt({ ...state, menu_return: { ...state.menu_return, status: "error" } }), "waiting");
    assert.equal(attempt({ ...state, menu_return: { ...state.menu_return, account_id: "account-b" } }), "waiting");
    assert.equal(attempt({ ...state, menu_return: { ...state.menu_return, cycle_id: "cycle-old" } }), "waiting");
    assert.equal(attempt({ ...state, menu_return: { ...state.menu_return, receipt_ids: [] } }), "waiting");
    assert.equal(attempt({ ...state, reward_receipts: [] }), "waiting", "a confirmed response without its receipt stays blocked");
    assert.equal(attempt({ ...state, reward_receipts: [{ ...receipt, match_id: "other-match" }] }), "waiting");
    assert.equal(attempt({ ...state, reward_receipts: [{ ...receipt, status: "pending" }] }), "waiting");

    assert.equal(flow.update(state, { ...view, ready: false }), "waiting");
    assert.equal(flow.update(state, { ...view, visible: false }), "waiting");
    assert.equal(flow.update({ ...state, phase: "ExitLoader" }, view), "waiting");
    assert.equal(flow.update(state, { ...view, receipts: [receipt] }), "rewards", "late receipt starts presentation");
    assert.equal(flow.canPresent(), true);
    assert.equal(flow.canGuide(), false);
    assert.equal(flow.update(state, { ...view, running: true }), "rewards", "guide stays held until animation completion");
    assert.equal(flow.update(state, view), "guide");
    assert.equal(flow.canGuide(), true);

    const noReward = { ...state, menu_return: { ...state.menu_return, status: "not_applicable", receipt_ids: [] }, reward_receipts: [] };
    assert.equal(flow.update(noReward, view), "guide", "only an explicit no-reward result releases the gate");
    assert.equal(flow.update({ ...state, menu_return: undefined, profile_ready: false }, view), "waiting");
    assert.equal(flow.update({ ...state, menu_return: undefined, profile_ready: true }, view), "guide");

    const storeFlow = menuFlowApi.create();
    const storeReturn = {
        ...state,
        menu_return: { ...state.menu_return, requested_screen: "store" }
    };
    assert.equal(storeFlow.update(storeReturn, { ...view, receipts: [receipt] }), "rewards");
    assert.equal(storeFlow.update(storeReturn, view), "destination");
    assert.equal(storeFlow.takeDestination(), "store");
    assert.equal(storeFlow.takeDestination(), null, "a destination is delivered once");
    assert.equal(storeFlow.update(storeReturn, { ...view, screen: "store" }), "destination");
    assert.equal(storeFlow.canGuide(), false, "a selected screen holds the guide until home is ready");
    assert.equal(storeFlow.update(storeReturn, view), "guide");

    const nextReturn = {
        ...state,
        loader_cycle_id: "cycle-2",
        menu_return: { ...state.menu_return, cycle_id: "cycle-2", source: "multiplayer", match_id: "match-2", receipt_ids: ["match-2"], requested_screen: null },
        reward_receipts: [{ id: "match-2", match_id: "match-2", status: "applied", level_before: 2, level_after: 4 }]
    };
    assert.equal(storeFlow.update(nextReturn, { ...view, receipts: nextReturn.reward_receipts }), "rewards", "a later match gets a new return cycle");
    assert.equal(storeFlow.update(nextReturn, view), "guide");
});


test("the shell presents only settled receipts and accepts a final rejected verification", () => {
    const { state } = confirmedMenuReturn();
    state.menu_return = undefined;
    state.reward_receipts = [
        { id: "tutorial", match_id: "tutorial", status: "applied" },
        { id: "verified", match_id: "match-v", status: "applied", verification_status: "verified" },
        { id: "rejected", match_id: "match-r", status: "applied", verification_status: "rejected" },
        { id: "pending-verified", match_id: "match-pv", status: "pending", verification_status: "verified" },
        { id: "pending-rejected", match_id: "match-pr", status: "pending", verification_status: "rejected" },
        { id: "presented", match_id: "match-old", status: "presented", verification_status: "verified" }
    ];
    const shell = makeMenuShellRewardHarness(makeMenuGuideHarness(), state);
    assert.deepEqual(Array.from(shell.pendingReceipts), ["tutorial", "verified", "rejected"]);
});


test("menu return shows every server receipt level before releasing its guide", async () => {
    const harness = makeMenuGuideHarness();
    const { state: baseState, receipt: baseReceipt } = confirmedMenuReturn();
    const receipt = { ...baseReceipt, xp: 350, crowns: 25, laurels: 3, level_after: 4 };
    const state = { ...baseState, xp: 350, level: 4, crowns: 25, laurels: 3, reward_receipts: [receipt] };
    const shell = makeMenuShellRewardHarness(harness, { ...state, menu_return: { ...state.menu_return, status: "waiting" } });
    shell.holdPresentation();
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(harness.renderedSteps, [], "waiting for the server cannot release the hand guide");
    assert.deepEqual(shell.rewardStageSets, [], "the shell does not animate an unconfirmed local guess");

    shell.setState(state);
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(shell.phase, "rewards");
    assert.equal(shell.rewardStageSets.length, 1, "the real shell presenter created reward cards");
    assert.deepEqual(Array.from(shell.rewardStageSets[0], stage => [stage.kind, stage.level]), [
        ["xp", undefined], ["crowns", undefined], ["laurels", undefined], ["level", 2], ["level", 3], ["level", 4]
    ]);
    assert.deepEqual(harness.renderedSteps, [], "the guide stays paused while reward cards are on screen");
    assert.deepEqual(shell.rewardStageRuns, [], "the first card waits for its reveal animation");

    shell.releasePresentation();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(shell.rewardStageRuns.map(stage => [stage.kind, stage.level]), [
        ["xp", undefined], ["crowns", undefined], ["laurels", undefined], ["level", 2], ["level", 3], ["level", 4]
    ]);
    assert.deepEqual(shell.rewardPulses.map(pulse => pulse.level), [1, 1, 1, 2, 3, 4],
        "the level counter changes after currency cards, once for each earned level");
    assert.deepEqual(Array.from(shell.shownReceipts), ["tutorial"]);
    assert.equal(shell.phase, "guide", "only the completed reward presentation releases the guide");
    assert.equal(harness.renderedSteps.at(-1), "welcome_name");

    shell.setState({ ...state, name_save_pending: true, name_save_revision: 0 });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "welcome_name", "a rejected or pending name save does not advance");

    shell.setState({ ...state, name_save_pending: false, name_save_revision: 1 });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "welcome_heroes", "the server-confirmed revision advances the guide");
    assert.ok(harness.calls.some(call => call.type === "save_menu_guide_progress" && call.completed_steps.includes("welcome_name")));

    harness.guide.pause();
    const rendersBeforeResume = harness.renderedSteps.length;
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "welcome_heroes", "pausing and resuming preserves the current step");
    assert.ok(harness.renderedSteps.length > rendersBeforeResume);

    const reloaded = makeMenuGuideHarness(harness.storage);
    const bootState = {
        ...state, menu_return: undefined, profile_ready: true,
        reward_receipts: [{ ...receipt, status: "presented", presented_at: 1 }]
    };
    const reloadedShell = makeMenuShellRewardHarness(reloaded, bootState);
    reloadedShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(reloaded.renderedSteps.at(-1), "welcome_heroes", "reload resumes the saved server-confirmed step");

    const otherAccount = makeMenuGuideHarness(harness.storage);
    const otherState = { ...bootState, account_id: "account-b", reward_receipts: [] };
    const otherAccountShell = makeMenuShellRewardHarness(otherAccount, otherState);
    otherAccountShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(otherAccount.renderedSteps.at(-1), "welcome_name", "one account cannot inherit another account's saved step");
});


test("welcome teaches a different available hero before confirming it", async () => {
    const { state } = confirmedMenuReturn();
    state.menu_return = undefined; state.reward_receipts = [];
    state.menu_guides.welcome = { status: "active", completed_steps: ["welcome_name"], current_step: "welcome_heroes" };
    const harness = makeMenuGuideHarness(), shell = makeMenuShellRewardHarness(harness, state);
    shell.maybePresentRewards(); await new Promise(resolve => setImmediate(resolve));
    shell.setScreen("heroes"); shell.maybePresentRewards(); await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "welcome_owned_selection");
    const nodes = ["caesar", "boudica", "locked"].map(id => ({ dataset: { leaderId: id }, getClientRects: () => [1] }));
    assert.equal(campaignRuntime.resolveUiTarget("menu_alternate_leader_card", { querySelectorAll: () => nodes }, null,
        { available_leader_ids: ["caesar", "boudica"], owned_leader_ids: ["caesar"], selected_leader: "caesar" }), nodes[1]);
    shell.setPreviewLeader("boudica"); shell.setOutcome("leader_preview_selected", 1);
    shell.maybePresentRewards(); await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "welcome_owned_confirm");
    shell.setState({ ...state, selected_leader: "boudica", leader_selection_revision: 1 });
    shell.setScreen("home"); shell.maybePresentRewards(); await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "welcome_reopen_heroes");
});


test("a saved menu step recovers navigation and the exact selected hero", () => {
    const equip = menuGuideDefinition.steps.find(step => step.id === "welcome_equip");
    const facts = { screen: "home", menu_guide_selected_leader: "boudica", preview_leader_id: "caesar" };
    assert.equal(campaignRuntime.menuGuideControl(equip, facts).target, "menu_heroes");
    assert.equal(campaignRuntime.menuGuideControl(equip, { ...facts, screen: "heroes" }).target, "menu_selected_leader_card");
    assert.equal(campaignRuntime.menuGuideControl(equip, { ...facts, screen: "heroes", preview_leader_id: "boudica" }).target, "menu_leader_confirm");
    const history = menuGuideDefinition.steps.find(step => step.id === "first_history");
    assert.equal(campaignRuntime.menuGuideControl(history, facts).target, "menu_profile");
});


test("welcome purchase waits for the exact server delivery and equipped leader", async () => {
    async function atPurchaseConfirm() {
        const { state } = confirmedMenuReturn();
        state.menu_return = undefined;
        state.reward_receipts = [];
        state.store = { leaders: [{ id: "cleopatra", owned: false, cost_gems: 10 }] };
        state.gems = 20;
        state.menu_guides = {
            welcome: {
                completed_steps: ["welcome_name", "welcome_heroes", "welcome_owned_selection", "welcome_owned_confirm", "welcome_reopen_heroes",
                    "welcome_offer_selection", "welcome_purchase_choice", "welcome_offer"],
                status: "active", selected_leader: "cleopatra"
            }
        };
        const harness = makeMenuGuideHarness();
        const shell = makeMenuShellRewardHarness(harness, state, "heroes");
        shell.setPurchasePrompt("cleopatra");
        shell.setOutcome("leader_purchase_prompt_opened", 1);
        shell.maybePresentRewards();
        await new Promise(resolve => setImmediate(resolve));
        assert.equal(harness.renderedSteps.at(-1), "welcome_purchase_confirm");
        return { state, harness, shell };
    }

    const failed = await atPurchaseConfirm();
    failed.shell.setPurchasePrompt(null);
    failed.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(failed.harness.renderedSteps.at(-1), "welcome_purchase_confirm",
        "a dismissed prompt or failed request without a server revision leaves the step incomplete");

    const cancelled = await atPurchaseConfirm();
    cancelled.shell.setPurchasePrompt(null);
    cancelled.shell.setOutcome("leader_purchase_cancelled", 1);
    cancelled.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(cancelled.harness.renderedSteps.at(-1), "welcome_cancel_home",
        "an explicit cancellation follows the no-purchase route");

    const mismatch = await atPurchaseConfirm();
    mismatch.shell.setPurchasePrompt("dido", "success");
    mismatch.shell.setPurchasedLeader("dido");
    mismatch.shell.setState({ ...mismatch.state, purchase_revision: 1 });
    mismatch.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(mismatch.harness.renderedSteps.at(-1), "welcome_purchase_close",
        "a successful purchase waits for the existing success modal to close");
    mismatch.shell.setOutcome("leader_purchase_closed", 1);
    mismatch.shell.setPurchasePrompt(null);
    mismatch.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(mismatch.harness.renderedSteps.at(-1), "welcome_cancel_home",
        "closing a different purchased hero follows the no-purchase route");

    const spending = await atPurchaseConfirm();
    spending.shell.setPurchasePrompt("cleopatra", "spending");
    spending.shell.setPurchasedLeader("cleopatra");
    spending.shell.setState({ ...spending.state, purchase_revision: 1 });
    spending.shell.maybePresentRewards(); await new Promise(resolve => setImmediate(resolve));
    assert.equal(spending.harness.renderedSteps.at(-1), "welcome_purchase_confirm", "the hand hides while the existing purchase animation runs");
    assert.ok(spending.harness.destroyedViews > 0);
    spending.shell.setPurchasePrompt("cleopatra", "success");
    spending.shell.maybePresentRewards(); await new Promise(resolve => setImmediate(resolve));
    assert.equal(spending.harness.renderedSteps.at(-1), "welcome_purchase_close");

    const success = await atPurchaseConfirm();
    success.shell.setPurchasePrompt("cleopatra", "success");
    success.shell.setPurchasedLeader("cleopatra");
    success.shell.setState({ ...success.state, purchase_revision: 1 });
    success.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(success.harness.renderedSteps.at(-1), "welcome_purchase_close");

    success.shell.setOutcome("leader_purchase_closed", 1);
    success.shell.setPurchasePrompt(null);
    success.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(success.harness.renderedSteps.at(-1), "welcome_equip",
        "the exact purchased offer reaches equip only after the success modal closes");

    success.shell.setState({
        ...success.state, purchase_revision: 1, leader_selection_revision: 1, selected_leader: "caesar",
        store: { leaders: [{ id: "cleopatra", owned: true, cost_gems: 10 }] }
    });
    success.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(success.harness.renderedSteps.at(-1), "welcome_equip",
        "confirming a different owned leader cannot satisfy the purchased leader step");

    success.shell.setState({
        ...success.state, purchase_revision: 1, leader_selection_revision: 2, selected_leader: "cleopatra",
        store: { leaders: [{ id: "cleopatra", owned: true, cost_gems: 10 }] }
    });
    success.shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(success.harness.renderedSteps.at(-1), "welcome_home",
        "the guide advances only after the purchased leader is equipped");

    const recoveredState = {
        ...success.state, selected_leader: "cleopatra", purchase_revision: 1,
        store: { leaders: [{ id: "cleopatra", owned: true, cost_gems: 10 }] },
        menu_guides: {
            welcome: {
                completed_steps: ["welcome_name", "welcome_heroes", "welcome_owned_selection", "welcome_owned_confirm", "welcome_reopen_heroes",
                    "welcome_offer_selection", "welcome_purchase_choice", "welcome_offer", "welcome_purchase_confirm"],
                status: "active", selected_leader: "cleopatra"
            }
        }
    };
    const recoveredGuide = makeMenuGuideHarness(new Map());
    const recoveredShell = makeMenuShellRewardHarness(recoveredGuide, recoveredState, "heroes");
    recoveredShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(recoveredGuide.renderedSteps.at(-1), "welcome_equip",
        "an exact owned offer resumes at equip without waiting for a closed success modal");
});


test("welcome guide routes unavailable offers and an explicit skip through the shared machine", async () => {
    const { state: noOfferState } = confirmedMenuReturn();
    noOfferState.menu_return = undefined;
    noOfferState.reward_receipts = [];
    noOfferState.store = { leaders: [] };
    noOfferState.gems = 0;
    noOfferState.menu_guides = {
        welcome: {
            completed_steps: ["welcome_name", "welcome_heroes", "welcome_owned_selection", "welcome_owned_confirm"],
            status: "active", selected_leader: null
        }
    };
    const noOfferGuide = makeMenuGuideHarness();
    const noOfferShell = makeMenuShellRewardHarness(noOfferGuide, noOfferState);
    noOfferShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(noOfferGuide.renderedSteps.at(-1), "welcome_reopen_heroes");
    noOfferShell.setScreen("heroes");
    noOfferShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(noOfferGuide.renderedSteps.at(-1), "welcome_no_offer_home", "no affordable offer takes the explicit no-offer route");

    const { state: skipState } = confirmedMenuReturn();
    skipState.menu_return = undefined;
    skipState.reward_receipts = [];
    skipState.store = { leaders: [{ id: "cleopatra", owned: false, cost_gems: 10 }] };
    skipState.gems = 20;
    skipState.menu_guides = {
        welcome: {
            completed_steps: ["welcome_name", "welcome_heroes", "welcome_owned_selection", "welcome_owned_confirm", "welcome_reopen_heroes"],
            status: "active", selected_leader: "cleopatra"
        }
    };
    const skipGuide = makeMenuGuideHarness();
    const skipShell = makeMenuShellRewardHarness(skipGuide, skipState, "heroes");
    skipShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(skipGuide.renderedSteps.at(-1), "welcome_offer_selection");
    skipShell.setPreviewLeader("cleopatra");
    skipShell.setOutcome("leader_offer_selected", 1);
    skipShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(skipGuide.renderedSteps.at(-1), "welcome_purchase_choice");
    assert.ok(skipGuide.calls.some(call => call.type === "save_menu_guide_progress" && call.selected_leader === "cleopatra"),
        "the exact affordable offer is persisted before the optional choice");
    skipGuide.choose("skip");
    assert.equal(skipGuide.renderedSteps.at(-1), "welcome_skip_home", "the player's skip choice uses its own guided route");
    const reloaded = makeMenuGuideHarness(skipGuide.storage);
    makeMenuShellRewardHarness(reloaded, skipState, "home").maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(reloaded.renderedSteps.at(-1), "welcome_quick_match", "reload preserves the skip branch and its pending navigation");
});


test("menu shell honors the requested store return before releasing the first-match guide", async () => {
    const { state, receipt } = confirmedMenuReturn();
    state.menu_return = {
        ...state.menu_return, source: "multiplayer", match_id: "match-1", receipt_ids: ["match-1"], requested_screen: "store"
    };
    state.reward_receipts = [{ ...receipt, id: "match-1", match_id: "match-1", verification_status: "verified" }];
    state.menu_guides = { first_multiplayer: { completed_steps: [], status: "active", selected_leader: null, source_match_id: "match-1" } };
    const guideHarness = makeMenuGuideHarness();
    const shell = makeMenuShellRewardHarness(guideHarness, state);
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(shell.getScreen(), "store", "the chosen destination opens after rewards");
    assert.deepEqual(Array.from(shell.shownReceipts), ["match-1"]);
    assert.deepEqual(guideHarness.renderedSteps, [], "the guide remains paused in the requested screen");

    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(guideHarness.renderedSteps, [], "being in the store does not release the guide");
    shell.setScreen("home");
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(guideHarness.renderedSteps.at(-1), "first_profile", "the guide starts only after the player returns home");
});


test("participation receipt stays synced until replay verification resolves", () => {
    assert.match(dataDbSource, /pub async fn mark_match_reward_verification_status/);
    assert.match(dataDbSource, /receipt\.verification_status = Some\(ReplayVerificationStatus::Verified\)/);
    assert.match(dataMainSource, /async fn reject_queued_replay/);
    assert.match(dataMainSource, /else if let Err\(error\) = reject_queued_replay\(&state, &payload\.match_id\)\.await/);
    assert.match(accountSource, /receipt\.verification_status\s*!=\s*Some\(sow_data::profile::ReplayVerificationStatus::Pending\)/);
    assert.match(accountSource, /receipt\.verification_status\s*==\s*Some\(sow_data::profile::ReplayVerificationStatus::Pending\)/);
    assert.match(coreSource, /receipt\.verification_status/);
    assert.match(accountSource, /fn apply_menu_return_profile[\s\S]*context\.account_id[\s\S]*context\.match_id[\s\S]*RewardSettlementStatus::Applied[\s\S]*RewardSettlementStatus::Presented[\s\S]*ReplayVerificationStatus::Verified[\s\S]*ReplayVerificationStatus::Rejected[\s\S]*context\.status = "confirmed"/);
    assert.match(accountSource, /tutorial_exit_waits_for_its_applied_server_receipt/);
    assert.match(accountSource, /EXIT_REWARD_PROFILE_RETRY_INTERVAL: Duration = Duration::from_secs\(1\)/);
    assert.match(accountSource, /waiting_in_exit_loader[\s\S]*SplashJob::ExitGame/);
    assert.match(accountSource, /self\.pending_reward_receipt_ids\.is_empty\(\) && !self\.waiting_menu_return\(\)/);
    assert.match(assetSource, /!self\.pending_reward_receipt_ids\.is_empty\(\)[\s\S]*self\.waiting_menu_return\(\)/);
    const engineLoaderSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/engine.rs"), "utf8");
    assert.match(engineLoaderSource, /reward_receipt_ready = !self\.waiting_menu_return\(\)[\s\S]*&& reward_receipt_ready/);
    assert.match(shellSource, /receipt\.verification_status === "verified" \|\| receipt\.verification_status === "rejected"/);
    assert.match(shellSource, /pendingRewardReceipts\(\)\.filter\(function \(receipt\) \{ return !rewardAnimationShown\.has\(receipt\.id\); \}\)/);
    assert.match(shellSource, /ids\.forEach\(function \(id\) \{ rewardAnimationShown\.add\(id\); \}\)/);
});


test("return observation and recovery have separate owners", () => {
    const dataExit = fs.readFileSync(path.join(shell, "../../sow-data/src/match_exit.rs"), "utf8");
    const observe = dataExit.slice(dataExit.indexOf("async fn handle_result"), dataExit.indexOf("async fn handle_exit"));
    assert.ok(observe.includes("committed_terminal"));
    assert.doesNotMatch(observe, /queue_match_work|dispatch_recovery_exit|internal\/match\/exit/);
    assert.match(accountSource, /profile\/match-result/);
    assert.match(accountSource, /voluntary_exit\.unwrap_or\(true\)[\s\S]*!context\.exit_recorded/);
    assert.doesNotMatch(accountSource, /ExitRewardWait/);
    assert.doesNotMatch(shellSource, /waitForExitMenuArt|exitMenuAssets/);
    const loaderSync = shellSource.slice(shellSource.indexOf("    function syncWebLoaderForState("), shellSource.indexOf("    function scheduleRewardPresentation("));
    const ready = { phase: "MainMenu", loader_job: "ExitGame", loader_cycle_id: 4, loader_done: true };
    let delivered = null;
    vm.runInNewContext(loaderSync + "; syncWebLoaderForState(ready);", {
        ready, window: { SOW_syncWebLoader: value => { delivered = value; } }
    });
    assert.equal(delivered, ready, "menu images cannot hold a completed return loader");
    assert.doesNotMatch(netUpdateSource, /relay_retry_count|relay_connect_start|15-second total relay timeout/);
    assert.doesNotMatch(relaySource, /RelayEvent::RematchRequest|generated_rematch_id|active_empty_secs/);
});

test("missing replay maps warn and unacknowledged results remain recoverable", () => {
    assert.match(serverSource, /MissingMap\(String\),/);
    assert.match(serverSource, /ReplayVerificationError::MissingMap\(format!\("map read failed/);
    assert.match(serverSource, /replay map unavailable, will retry after a later release/);
    assert.match(serverSource, /"error": "replay map unavailable"/);
    assert.doesNotMatch(relaySource, /permanently rejected by database.*dropping spool entry/);
    assert.match(relaySource, /remains unacknowledged/);
    assert.match(relaySource, /database did not acknowledge.*retaining spool/);
});


test("pending reward lookups survive reload without crossing accounts", () => {
    assert.match(identitySource, /PENDING_REWARD_RECEIPTS_STORAGE_PREFIX/);
    assert.match(identitySource, /load_pending_reward_receipt_ids\(account_id: &str\)/);
    assert.match(accountSource, /track_reward_receipt_sync[\s\S]*save_pending_reward_receipt_ids/);
    assert.match(accountSource, /load_pending_reward_receipt_ids\(&account_id\)/);
    assert.match(accountSource, /save_pending_reward_receipt_ids\([\s\S]*&account_id,[\s\S]*&self\.pending_reward_receipt_ids/);
    assert.match(bootstrapSource, /stored_account_id[\s\S]*load_pending_reward_receipt_ids/);
});


test("queued identity refresh is applied before an older profile response", () => {
    const profileStart = assetSource.indexOf("DbEvent::ProfileLoaded {");
    const profileEnd = assetSource.indexOf("DbEvent::DisplayNameSaved", profileStart);
    assert.ok(profileStart >= 0 && profileEnd > profileStart);
    const profileEvent = assetSource.slice(profileStart, profileEnd);
    assert.match(profileEvent, /profile_refresh_pending[\s\S]*fetch_cloud_progress\(\);[\s\S]*continue;/);
    assert.ok(profileEvent.indexOf("continue;") < profileEvent.indexOf("profile_last_applied_request = request_id"));

    const failedStart = assetSource.indexOf("DbEvent::LoadFailed { request_id, status }");
    const failedEnd = assetSource.indexOf("DbEvent::TutorialCompletionFailed", failedStart);
    assert.ok(failedStart >= 0 && failedEnd > failedStart);
    assert.match(assetSource.slice(failedStart, failedEnd), /profile_refresh_pending[\s\S]*fetch_cloud_progress\(\);[\s\S]*continue;/);
});


test("lobby cards keep live counts and one DOM card per lobby id", () => {
    assert.match(lobbiesSource, /function lobbyPlayerCount\(lobby\)/);
    assert.match(lobbiesSource, /function lobbyStatusText\(lobby\)/);
    assert.match(lobbiesSource, /data-lobby-status-for/);
    assert.match(lobbiesSource, /data-lobby-count/);
    assert.match(lobbiesSource, /var seen = Object\.create\(null\)/);
    assert.match(lobbiesSource, /card\.remove\(\)/);
    assert.doesNotMatch(lobbiesSource, /sow-menu__lobby-join/);
    assert.doesNotMatch(shellSource, /cardTimers|data-timer-for|lobbyTimerText/);
    assert.match(lobbiesSource, /data-command='join_lobby'/);
    assert.match(lobbiesSource, /menu\.join/);
    assert.match(menuCss["main_menu.lobbies.css"], /\.sow-menu__lobby-count/);
    assert.doesNotMatch(menuCss["main_menu.lobbies.css"], /\.sow-menu__lobby-join/);
});


test("lobby exit keeps the live connection and match exit owns teardown", () => {
    assert.match(sessionSource, /fn leave_lobby_to_main_menu\(&mut self\)/);
    assert.match(sessionSource, /fn finish_lobby_exit_to_main_menu\(&mut self\)/);
    const lobbyActionStart = actionsSource.indexOf("UiAction::LeaveLobby =>");
    const matchActionStart = actionsSource.indexOf("UiAction::ReturnToMenu =>");
    assert.ok(lobbyActionStart >= 0 && matchActionStart > lobbyActionStart);
    const lobbyAction = actionsSource.slice(lobbyActionStart, matchActionStart);
    assert.match(lobbyAction, /leave_lobby_to_main_menu\(\)/);
    assert.doesNotMatch(lobbyAction, /begin_exit_to_main_menu|client = None|reconnect_now/);
    const matchAction = actionsSource.slice(matchActionStart);
    assert.match(matchAction, /begin_exit_to_main_menu\(\);/);
    assert.doesNotMatch(matchAction.slice(0, matchAction.indexOf("UiAction::SetAttackRatio")), /send_leave_message/);
    assert.match(sessionSource, /self\.send_leave_message\(\);[\s\S]*self\.net\.client\.take\(\)/);
    assert.match(webMenu, /WebMenuCommand::ReturnToMenu[\s\S]*UiAction::ReturnToMenu/);
    assert.doesNotMatch(hud, /send\("leave_lobby"\)/);
    assert.match(hud, /send\("return_to_menu"\)/);
    assert.doesNotMatch(sessionSource, /reconnect_now/);
    assert.doesNotMatch(netUpdateSource, /reconnect_now/);
});


test("game exit resets Battle and replays the screen entrance after the loader", () => {
    assert.match(coreSource, /var lastMenuPhase = null/);
    assert.match(coreSource, /var pendingExitScreenIntro = false/);
    const resetStart = shellSource.indexOf("if (returnedFromGame) {");
    const resetEnd = shellSource.indexOf("if (typeof window.SOW_tutorial_menu_state_update", resetStart);
    assert.ok(resetStart >= 0 && resetEnd > resetStart);
    const resetBlock = shellSource.slice(resetStart, resetEnd);
    for (const flag of ["profileOpen", "heroesOpen", "campaignOpen", "storeOpen"]) {
        assert.match(resetBlock, new RegExp(flag + " = false"));
    }
    assert.match(resetBlock, /pendingExitScreenIntro = true/);
    assert.match(shellSource, /sow:loader-cycle-ready/);
    assert.match(shellSource, /previousScreen = null;[\s\S]*render\(\);/);
    assert.match(loaderSource, /sow:loader-cycle-ready/);
    assert.match(loaderSource, /if \(!loaderReadyDispatched\)/);
    assert.doesNotMatch(shellSource, /SOW_open_store_after_match/);
    assert.match(menuCss["main_menu.layout.css"], /sow-screen-panel-enter 240ms/);
});


test("shell media URLs bust by content hash when the build ships one", () => {
    const assetUrlStart = loaderSource.indexOf("    function assetUrl(");
    const assetUrlEnd = loaderSource.indexOf("    function leaderArtUrl(", assetUrlStart);
    assert.ok(assetUrlStart >= 0 && assetUrlEnd > assetUrlStart);
    const context = { window: {} };
    vm.runInNewContext(
        loaderSource.slice(assetUrlStart, assetUrlEnd) + "; this.assetUrl = assetUrl;",
        context
    );
    context.window.SOW_BUILD_TS = "build-42";
    assert.equal(
        context.assetUrl("/assets/shell/leaders/boudica_desktop.webp"),
        "/assets/shell/leaders/boudica_desktop.webp?v=build-42",
        "without a content entry the build bust still applies"
    );
    context.window.SOW_ASSET_TS = { "/assets/shell/leaders/other_desktop.webp": "cafe1234" };
    assert.equal(
        context.assetUrl("/assets/shell/leaders/boudica_desktop.webp"),
        "/assets/shell/leaders/boudica_desktop.webp?v=build-42",
        "an unlisted path keeps the build bust"
    );
    context.window.SOW_ASSET_TS = { "/assets/shell/leaders/boudica_desktop.webp": "cafe1234" };
    assert.equal(
        context.assetUrl("/assets/shell/leaders/boudica_desktop.webp"),
        "/assets/shell/leaders/boudica_desktop.webp?v=cafe1234",
        "a content hash wins over the build stamp so unrelated deploys keep the cache"
    );
});


test("first-run tutorial keeps one boot loader and splash art until Rust signals completion", async () => {
    assert.match(loaderSource, /window\.SOW_leaderArtUrl = leaderArtUrl/);
    assert.match(loaderSource, /window\.SOW_prepareLeaderArt = prepareLeaderArt/);
    assert.match(loaderSource, /if \(loaderArtKey === 'boot' && state && state\.loader_job === 'EnterGame'\) \{[\s\S]*?prepareLeaderArt\(slug, 'low'\);[\s\S]*?activeMatchArt = preparedLeaderArt;/,
        "tutorial art is prepared behind the original splash and pinned to the match");
    const attachExitArt = loaderSource.indexOf("const pendingArt = syncLoaderArt(state);");
    const closeAfterArt = loaderSource.indexOf("if (state.loader_done === true) {\n            if (pendingArt) return;\n            finish(cycleId);", attachExitArt);
    assert.ok(attachExitArt >= 0 && closeAfterArt > attachExitArt,
        "exit art is attached before the loader begins closing");
    assert.match(loaderSource, /function waitForExitLeaderArt\(prepared, slug, cycleId\)[\s\S]*?syncLoaderArt\(current\)[\s\S]*?finish\(cycleId\);/,
        "a pending exit reuses the same image and closes only its own loader cycle");
    assert.doesNotMatch(loaderSource, /prepared\.variant !== leaderArtVariant\(\)/,
        "orientation changes do not discard the art used by this match");
    assert.match(coreSource, /return window\.SOW_leaderArtUrl\(slug, variant\)/);
    assert.match(heroesSource, /prepareLeaderArt\(activeLeader\)/);
    assert.doesNotMatch(shellSource, /state\.phase === "Playing" && state\.loader_leader[\s\S]*prepareLeaderArt\(leaderById\(state\.loader_leader\)\)/,
        "finishing EnterGame does not trigger a redundant image preparation");
    assert.match(shellSource, /function normalizeCompletedExitState\([\s\S]*?loader_cycle_id <= completedLoaderCycleId[\s\S]*?loader_job: "Boot"/,
        "a completed exit is normalized before later menu snapshots reach the loader");
    assert.match(shellSource, /sow:loader-cycle-ready", function \(event\)[\s\S]*?event\.detail\.cycle_id[\s\S]*?normalizeCompletedExitState\(state\)/,
        "menu completion is tied to the exact loader cycle");
    for (const source of [coreSource, heroesSource, storeSource, profileSource]) {
        assert.doesNotMatch(source, /asset\(["']shell\/leaders\//, "leader art must use the same versioned URL as the loader");
    }
    const introStart = matchStartSource.indexOf('log::info!("Portal boot: new player -> JavaScript campaign bootstrap")');
    const introEnd = matchStartSource.indexOf("crate::store_portals::gameplay_stop();", introStart);
    assert.ok(introStart >= 0 && introEnd > introStart);
    const bootIntro = matchStartSource.slice(introStart, introEnd);
    assert.match(bootIntro, /let campaign = crate::campaign::CampaignId::Boudica/);
    assert.match(bootIntro, /boot_campaign_pending = Some\(campaign\.episode_id\(\)\.to_string\(\)\)/);
    assert.match(bootIntro, /begin_enter_game_loader\(campaign\.advisor\(\)\)/);
    assert.doesNotMatch(bootIntro, /hide_web_loader|splash_state\.done = true|phase = ClientPhase::MainMenu/);
    assert.match(webMenu, /"phase": "Playing",\s*"account_id": app\.progress_account_id,\s*"loader_cycle_id": app\.ui\.app\.splash_state\.cycle_id,\s*"loader_job": splash_job_name\(&app\.ui\.app\.splash_state\.job\),\s*"loader_leader": app\.ui\.app\.splash_state\.loader_leader\.map\(leader_id\),\s*"loader_progress": app\.ui\.app\.splash_state\.progress\.clamp\(0\.0, 1\.0\),\s*"loader_done": app\.ui\.app\.splash_state\.done/);
    assert.match(webMenu, /"loader_cycle_id": app\.ui\.app\.splash_state\.cycle_id/);
    assert.match(loaderSource, /if \(loaderArtKey === key \|\| loaderArtKey === key \+ ':failed'\) return null;/);
    assert.match(loaderSource, /if \(!prepared\.ready\) \{[\s\S]*?return state\.loader_job === 'ExitGame' \? prepared : null;\s*\}\s*const image = prepared\.image;/,
        "a completed existing preload can recover from an earlier pending state");
    assert.doesNotMatch(loaderSource, /state\.phase !== 'MainMenu' && state\.phase !== 'Playing'/);
    assert.doesNotMatch(loaderSource, /window\.hideWebLoader/);
    assert.doesNotMatch(appStateSource, /web_loader_hidden/);
    assert.match(tutorial, /state\.boot_campaign && runtime\.bootEpisode !== state\.boot_campaign\) startEpisode\(state\.boot_campaign, true\)/);
    const tutorialStateUpdate = shellSource.indexOf("window.SOW_tutorial_menu_state_update(state)");
    const loaderStateSync = shellSource.indexOf("syncWebLoaderForState(state)", tutorialStateUpdate);
    assert.ok(tutorialStateUpdate >= 0 && loaderStateSync > tutorialStateUpdate,
        "the same menu-state update starts the tutorial request before syncing the existing loader");
    const campaignCommandStart = webMenu.lastIndexOf("WebMenuCommand::StartCampaignEpisode {");
    const campaignCommandEnd = webMenu.indexOf("WebMenuCommand::CompleteCampaignEpisode {", campaignCommandStart);
    assert.ok(campaignCommandStart >= 0 && campaignCommandEnd > campaignCommandStart);
    assert.match(webMenu.slice(campaignCommandStart, campaignCommandEnd),
        /start_campaign_episode_from_web\(campaign, roster, match_config\)/);
    assert.match(matchStartSource, /self\.start_offline_match\([\s\S]*?\},\s*true,\s*\);/);
    const campaignStartStart = matchStartSource.indexOf("pub(crate) fn start_campaign_episode_from_web");
    const offlineStartStart = matchStartSource.indexOf("pub(crate) fn start_offline_match", campaignStartStart);
    const campaignStart = matchStartSource.slice(campaignStartStart, offlineStartStart);
    const offlineStart = matchStartSource.slice(offlineStartStart);
    assert.doesNotMatch(campaignStart, /set_selected_leader\(/,
        "starting a campaign does not replace the account's menu leader");
    assert.doesNotMatch(offlineStart, /set_selected_leader\(/,
        "starting a tutorial match keeps its leader separate from menu selection");
    assert.match(offlineStart, /color: leader\.filler_rgb\(\)/);
    assert.match(offlineStart, /civilization: leader\.civilization\(\),\s*leader,/,
        "the tutorial's actual game player still uses its campaign leader");
    const serverStartStart = netMessagesSource.indexOf("ServerMessage::Start(start_msg) =>");
    const serverStartEnd = netMessagesSource.indexOf("self.tasks.engine_init_queued_msg = Some(*start_msg);", serverStartStart);
    assert.ok(serverStartStart >= 0 && serverStartEnd > serverStartStart);
    const serverStart = netMessagesSource.slice(serverStartStart, serverStartEnd + "self.tasks.engine_init_queued_msg = Some(*start_msg);".length);
    assert.match(serverStart, /let loader_leader = start_msg[\s\S]*?\.map\(\|player\| player\.leader\)/,
        "the server-assigned leader is used for the match loader");
    assert.match(serverStart, /self\.tasks\.engine_init_queued_msg = Some\(\*start_msg\);/,
        "the match starts from the server-assigned player data");
    assert.doesNotMatch(serverStart, /sync_selected_leader_from_match\(|preferred_leader\s*=/,
        "starting a match cannot overwrite the account's saved leader choice");
    const exitStart = sessionSource.indexOf("pub(crate) fn begin_exit_to_main_menu");
    const exitEnd = sessionSource.indexOf("/// Enter the EnterGame splash", exitStart);
    assert.match(sessionSource.slice(exitStart, exitEnd),
        /self\.ui\.app\.splash_state\.loader_leader = exit_leader;/,
        "leaving a tutorial keeps its campaign leader for the exit splash");
    assert.doesNotMatch(sessionSource.slice(exitStart, exitEnd), /sync_selected_leader_from_match\(/,
        "leaving a tutorial does not replace the account's menu selection");
    assert.doesNotMatch(nativeProfileSource, /fn sync_selected_leader_from_match\(/,
        "a match no longer owns the account's selected hero");
    const preferencesStart = accountSource.indexOf("pub(crate) fn apply_progress_preferences(");
    const preferencesEnd = accountSource.indexOf("pub(crate) fn apply_cloud_profile(", preferencesStart);
    assert.ok(preferencesStart >= 0 && preferencesEnd > preferencesStart);
    const preferences = accountSource.slice(preferencesStart, preferencesEnd);
    assert.match(preferences, /has_explicit_leader_selection\(\) \{\s*return;/,
        "a profile refresh cannot overwrite an explicit hero choice");
    assert.match(preferences, /if let Some\(leader\) = self\.progress\.preferred_leader[\s\S]*?set_selected_leader\(leader, false\)/,
        "the saved profile, not the tutorial hero, restores the menu selection");
    const applyCloudProfileStart = accountSource.indexOf("pub(crate) fn apply_cloud_profile(");
    const applyCloudProfileEnd = accountSource.indexOf("\n    pub(crate) fn ", applyCloudProfileStart + 1);
    assert.ok(applyCloudProfileStart >= 0);
    const applyCloudProfile = accountSource.slice(applyCloudProfileStart, applyCloudProfileEnd);
    assert.match(applyCloudProfile,
        /cloud_preferred_leader\.unwrap_or_else\([\s\S]*?assigned_leader_for_account\([\s\S]*?current_rotation_period\(\)/,
        "a missing cloud preference resolves to the account's free-rotation leader, not Boudica");
    const tutorialCompleteStart = dataDbSource.indexOf("pub async fn complete_tutorial(");
    const tutorialCompleteEnd = dataDbSource.indexOf("/// Register expected players", tutorialCompleteStart);
    const tutorialComplete = dataDbSource.slice(tutorialCompleteStart, tutorialCompleteEnd);
    assert.match(tutorialComplete,
        /assigned_leader_for_account\(\s*account_id,\s*crate::commerce::current_rotation_period\(\)\s*,?\s*\)/,
        "an account missing its preference receives its normal free-rotation assignment");
    assert.doesNotMatch(tutorialComplete, /Leader::Boudica/);
    const engineLoaderSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/engine.rs"), "utf8");
    assert.match(engineLoaderSource,
        /if step == 3 && self\.sim\.current_snapshot\.is_some\(\) \{[\s\S]*?if ready_to_release \{[\s\S]*?self\.ui\.app\.splash_state\.done = true;/,
        "Rust reports done only after the initialized game snapshot is ready");
    const enterLoaderStart = sessionSource.indexOf("pub(crate) fn begin_enter_game_loader");
    const enterLoaderEnd = sessionSource.indexOf("/// Whether the map/mover GPU path", enterLoaderStart);
    assert.ok(enterLoaderStart >= 0 && enterLoaderEnd > enterLoaderStart);
    const enterLoader = sessionSource.slice(enterLoaderStart, enterLoaderEnd);
    assert.match(enterLoader, /job == SplashJob::EnterGame[\s\S]*loader_leader == Some\(leader\)[\s\S]*!self\.ui\.app\.splash_state\.done[\s\S]*return;/);
    assert.match(sessionSource.slice(enterLoaderStart, enterLoaderEnd), /SplashJob::Boot[\s\S]*?&& !self\.ui\.app\.splash_state\.done/);

    class Element {
        constructor(tagName) {
            this.tagName = tagName;
            this.children = [];
            this.style = {};
            this.dataset = {};
            this.attributes = {};
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
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) {
            if (Object.prototype.hasOwnProperty.call(this.attributes, name)) return this.attributes[name];
            return this[name] == null ? null : String(this[name]);
        }
        removeAttribute(name) { delete this.attributes[name]; }
        get firstChild() { return this.children[0] || null; }
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
    for (const id of ["loader-bar-wrap", "loader-bar-fill", "loader-bar-full", "loader-bar-empty", "loader-text"]) {
        const child = new Element("div");
        child.id = id;
        root.appendChild(child);
    }
    const find = (node, predicate) => predicate(node) ? node : node.children.map(child => find(child, predicate)).find(Boolean) || null;
    const findPicture = () => find(root, node => node.className === "splash-picture");
    const document = {
        readyState: "loading",
        body: new Element("body"),
        createElement: tagName => new Element(tagName),
        getElementById: id => find(root, node => node.id === id),
        querySelector: selector => selector === "#web-loader .splash-picture" ? findPicture() : null,
        addEventListener() {}
    };
    const timers = new Map();
    const events = [];
    const completedLoaderCycles = [];
    const leaderPreloads = [];
    class PreloadImage {
        constructor() { this.parentNode = null; this.children = []; }
        set src(value) { this._src = value; leaderPreloads.push(this); }
        get src() { return this._src; }
        decode() { return Promise.resolve(); }
        remove() {
            if (!this.parentNode) return;
            const siblings = this.parentNode.children;
            siblings.splice(siblings.indexOf(this), 1);
            this.parentNode = null;
        }
    }
    let nextTimer = 0;
    const window = {
        SOW_BOOT_UI_BASE: "/assets/shell/loader",
        SOW_BUILD_TS: "regression",
        SOW_PORTAL: "jest",
        innerWidth: 1024,
        innerHeight: 768,
        matchMedia: () => ({ matches: window.innerHeight >= window.innerWidth }),
        location: { href: "https://game.test/", origin: "https://game.test" },
        addEventListener() {},
        dispatchEvent(event) {
            events.push(event.type);
            if (event.type === "sow:loader-cycle-ready") completedLoaderCycles.push(event.detail.cycle_id);
        }
    };
    const context = {
        window, document, URL, Image: PreloadImage,
        Event: class { constructor(type) { this.type = type; } },
        CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
        performance: { now: () => 0 },
        requestAnimationFrame: () => 1,
        cancelAnimationFrame() {},
        setTimeout(callback) { const id = ++nextTimer; timers.set(id, callback); return id; },
        clearTimeout(id) { timers.delete(id); }
    };
    vm.runInNewContext(loaderSource, context);
    window.SOW_initWebLoader();
    assert.equal(findPicture(), initialPicture, "initialization preserves the HTML splash picture");
    assert.equal(findPicture().children[1], initialImage, "initialization preserves the embedded splash image node");
    assert.equal(initialImage.src, embeddedSplash, "initialization does not replace the embedded splash with a network URL");
    assert.equal(leaderPreloads.length, 0, "initialization creates no duplicate hero-image request");

    const leaders = [
        { id: "Boudica", slug: "boudica" },
        { id: "Caesar", slug: "caesar" },
        { id: "Ragnar", slug: "ragnar" }
    ];
    let cycleId = -1;
    let previousJob = null;
    let previousDone = false;
    const state = (job, leader, progress = 0.4, phase = "Splash", done = false) => {
        if (job !== previousJob || (previousDone && !done)) cycleId += 1;
        previousJob = job;
        previousDone = done;
        return {
            phase, loader_cycle_id: cycleId, loader_job: job, loader_leader: leader,
            loader_progress: progress, loader_done: done, leaders
        };
    };
    window.SOW_syncWebLoader(state("Boot", null, 0.2, "Splash"));
    const bootPicture = findPicture();
    const bootImage = bootPicture.children[1];
    assert.equal(bootPicture, initialPicture, "Boot updates keep the original splash picture");
    assert.equal(bootImage, initialImage, "Boot updates keep the original splash image");
    const loaderBar = document.getElementById("loader-bar-fill");

    const campaign = require(path.join(shell, "sow-campaign.js"));
    const pendingTutorialFetches = [];
    const tutorialCommands = [];
    window.SOW_t = key => key;
    window.SOW_hasText = key => Boolean(window.SOW_t(key));
    window.SOW_menu_command = message => tutorialCommands.push(JSON.parse(message));
    window.SOWCampaign = campaign;
    window.SOWCampaignView = { mount: () => ({ render() {}, destroy() {} }) };
    vm.runInNewContext(tutorial, {
        window, document, performance: { now: () => 0 }, console,
        fetch(url) {
            return new Promise(resolve => pendingTutorialFetches.push({ url, resolve }));
        }
    });
    const bootCampaignState = {
        ...state("EnterGame", "Boudica", 0.95, "Splash"),
        boot_campaign: "boudica"
    };
    window.SOW_tutorial_menu_state_update(bootCampaignState);
    assert.equal(pendingTutorialFetches.length, 2, "the tutorial's paired files are genuinely pending");
    assert.deepEqual(
        pendingTutorialFetches.map(request => request.url).sort(),
        [
            "/assets/campaign/boudica.json?v=regression",
            "/assets/campaign/boudica.triggers.json?v=regression"
        ].sort(),
        "tutorial JSON carries the build stamp so deploys always refresh it"
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
    assert.equal(findPicture(), bootPicture, "the same splash remains while tutorial and game initialization are pending");
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar, "the progress bar node is never recreated");
    assert.equal(root.style.visibility, "visible");
    assert.equal(loaderBar.style.width, "97.0%", "progress advances monotonically through tutorial startup");
    assert.equal(events.length, 0, "phase changes cannot close the loader before Rust reports done");

    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    for (const request of pendingTutorialFetches) {
        request.resolve({
            ok: true,
            json: () => Promise.resolve(request.url.split("?")[0].endsWith(".triggers.json") ? definition : roster)
        });
    }
    await new Promise(setImmediate);
    assert.equal(tutorialCommands.length, 1, "the resolved tutorial data starts the campaign through the existing command");
    assert.equal(tutorialCommands[0].type, "start_campaign_episode");
    assert.equal(tutorialCommands[0].episode_id, "boudica");
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.98, "Playing"));
    assert.equal(findPicture(), bootPicture, "the splash remains through game initialization after tutorial download");
    assert.equal(root.style.visibility, "visible");
    assert.equal(events.length, 0);

    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    assert.equal(findPicture(), null, "the art is released after the tutorial finishes loading");
    assert.ok(events.includes("sow:loader-ready"));
    assert.equal(window.SOW_leaderArtUrl("boudica", "desktop"), "/assets/shell/leaders/boudica_desktop.webp?v=regression");
    assert.equal(window.SOW_leaderArtUrl("boudica", "mobile"), "/assets/shell/leaders/boudica_mobile.webp?v=regression");
    assert.equal(window.SOW_leaderArtUrl("boudica"), "/assets/shell/leaders/boudica_desktop.webp?v=regression",
        "the menu backdrop resolver defaults to the landscape variant on a landscape screen");
    assert.equal(window.SOW_prepareLeaderArt("boudica"), boudicaReady,
        "requesting the same prepared URL reuses its existing promise");
    assert.equal(leaderPreloads.length, 1);

    window.innerWidth = 390;
    window.innerHeight = 844;
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 0.6));
    assert.equal(findPicture(), bootPicture, "the original tutorial splash remains while its one Boudica preload is pending");
    assert.equal(findPicture().children[1], bootImage);
    assert.equal(leaderPreloads.length, 1, "showing the loader starts no additional hero-image request");
    const tutorialExitDone = state("ExitGame", "Boudica", 1, "MainMenu", true);
    window.SOW_syncWebLoader(tutorialExitDone);
    assert.equal(root.style.visibility, "visible", "loader_done cannot hide the bar before the prepared image is attached");
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar);
    assert.equal(events.filter(event => event === "sow:loader-cycle-ready").length, 1);
    boudicaPreload.onload();
    assert.equal(await boudicaReady, true);
    await new Promise(setImmediate);
    assert.equal(findPicture().children[0], boudicaPreload,
        "the same Boudica image that began loading behind Boot is attached on tutorial exit");
    const exitBoudicaPicture = findPicture();
    const exitBoudicaImage = exitBoudicaPicture.children[0];
    assert.equal(exitBoudicaImage, boudicaPreload);
    assert.match(boudicaPreload.src, /leaders\/boudica_desktop\.webp/,
        "exit keeps the exact prepared variant after orientation changes");
    assert.equal(leaderPreloads.length, 1, "the full tutorial cycle creates one Boudica image");
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar, "tutorial exit keeps the one original progress bar");
    [...timers.values()][0]();
    timers.clear();
    assert.ok(events.includes("sow:loader-cycle-ready"));
    assert.equal(completedLoaderCycles.filter(id => id === tutorialExitDone.loader_cycle_id).length, 1);
    window.SOW_syncWebLoader({ ...tutorialExitDone, loader_progress: 1 });
    window.SOW_syncWebLoader({ ...tutorialExitDone, loader_progress: 1 });
    assert.equal(root.style.visibility, "hidden", "repeated completed MainMenu snapshots cannot reopen the old ExitGame cycle");
    assert.equal(root.style.pointerEvents, "none", "the completed loader cannot block menu or lobby input");
    assert.equal(findPicture(), null, "the old Boudica image stays detached after the cycle is consumed");
    assert.equal(completedLoaderCycles.filter(id => id === tutorialExitDone.loader_cycle_id).length, 1,
        "one cycle dispatches one completion event");
    window.innerWidth = 1024;
    window.innerHeight = 768;
    const reusedBoudicaReady = window.SOW_prepareLeaderArt("boudica");
    assert.equal(reusedBoudicaReady, boudicaReady, "the decoded image remains prepared after the loader cycle");
    assert.equal(leaderPreloads.length, 1, "the finished cycle does not trigger another Boudica image");
    assert.equal(await reusedBoudicaReady, true);

    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.45));
    const normalEntryPicture = findPicture();
    assert.equal(normalEntryPicture.children[0], boudicaPreload,
        "a normal game entry displays the already-decoded selected hero");
    assert.equal(leaderPreloads.length, 1);
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    window.innerWidth = 390;
    window.innerHeight = 844;
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 0.45));
    const normalExitPicture = findPicture();
    assert.equal(normalExitPicture.children[0], boudicaPreload,
        "the same decoded image is reused for game exit");
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 1, "MainMenu", true));
    assert.equal(findPicture().children[0], boudicaPreload,
        "the final done update adopts the exit image before closing");
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
    assert.equal(findPicture(), bootPicture, "failed preload keeps the original splash instead of leaving the loader blank");
    assert.equal(root.style.visibility, "visible");
    assert.equal(loaderBar.style.width, "35.0%");
    assert.equal(leaderPreloads.length, requestCountBeforeUnreadyLoader);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();

    window.innerWidth = 390;
    window.innerHeight = 844;
    assert.equal(window.SOW_leaderArtUrl("caesar"), "/assets/shell/leaders/caesar_mobile.webp?v=regression",
        "the menu backdrop resolver defaults to the portrait variant on a portrait screen");
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
    assert.equal(findPicture(), caesarPicture, "repeated progress updates do not replace the active hero picture");
    assert.equal(findPicture().children[0], caesarImage, "repeated progress updates preserve the decoded image node");
    assert.equal(leaderPreloads.length, 3, "repeated progress updates create no new Image");
    assert.equal(loaderBar.style.width, "65.0%");
    exitBoudicaImage.onerror();
    assert.equal(findPicture(), caesarPicture, "a delayed old-image error cannot touch the current hero");

    caesarImage.onerror();
    assert.equal(findPicture(), bootPicture, "a failed current image falls back to the original splash");
    assert.equal(root.style.visibility, "visible", "art failure does not close the loader");
    assert.equal(document.getElementById("loader-bar-fill").style.width, "65.0%");

    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    assert.equal(events.filter(event => event === "sow:loader-ready").length, 1);
    assert.equal(events.filter(event => event === "sow:loader-cycle-ready").length, 6,
        "each completed loader cycle emits one ready event");

    window.innerWidth = 1024;
    window.innerHeight = 768;
    const nextCaesarReady = window.SOW_prepareLeaderArt("caesar");
    const nextCaesarPreload = leaderPreloads.at(-1);
    nextCaesarPreload.onload();
    assert.equal(await nextCaesarReady, true);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.45));
    const nextCaesarPicture = findPicture();
    assert.equal(nextCaesarPicture.children[0], nextCaesarPreload);
    assert.match(nextCaesarPreload.src, /leaders\/caesar_desktop\.webp\?v=regression/,
        "the loader follows the same landscape resolver after orientation changes");
    caesarImage.onerror();
    assert.equal(findPicture(), nextCaesarPicture, "an old cycle cannot invalidate the same hero in a new cycle");
    nextCaesarPicture.children[0].onerror();
    assert.equal(findPicture(), bootPicture, "a failed current image falls back to the original splash");
    assert.equal(root.style.visibility, "visible", "art failure does not close the loader");
    assert.equal(document.getElementById("loader-bar-fill").style.width, "45.0%");

    const pendingExitReady = window.SOW_prepareLeaderArt("ragnar");
    const pendingExitImage = leaderPreloads.at(-1);
    window.SOW_syncWebLoader(state("ExitGame", "Ragnar", 0.45));
    assert.equal(findPicture(), bootPicture, "a pending exit keeps the original splash until that same request is ready");
    const imageCountBeforePendingExitReady = leaderPreloads.length;
    pendingExitImage.onload();
    assert.equal(await pendingExitReady, true);
    window.SOW_syncWebLoader(state("ExitGame", "Ragnar", 1, "MainMenu", true));
    assert.equal(findPicture().children[0], pendingExitImage,
        "a completed pending preload recovers from unready and is attached before finish");
    assert.equal(leaderPreloads.length, imageCountBeforePendingExitReady,
        "the final exit sync reuses the in-flight image");
    [...timers.values()][0]();
    timers.clear();
    assert.equal(findPicture(), null);
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar,
        "entry and exit preserve the same single progress bar");
    assert.equal(events.filter(event => event === "sow:loader-ready").length, 1);
    assert.equal(events.filter(event => event === "sow:loader-cycle-ready").length, 7);
});


test("profile presents verified history, career commanders, localized achievements, and a paged global board", () => {
    const matchRowStart = profileSource.indexOf("function profileMatchRow");
    const matchRowEnd = profileSource.indexOf("function profileHeaderMarkup", matchRowStart);
    assert.ok(matchRowStart >= 0 && matchRowEnd > matchRowStart);
    const matchRow = profileSource.slice(matchRowStart, matchRowEnd);
    assert.match(matchRow, /match\.verified[\s\S]*profile\.match_pending[\s\S]*profile\.match_unverified/);
    assert.match(matchRow, /var verified = !!match\.verified;/);
    assert.match(matchRow, /var result = verified\s+\? \(match\.won \? SOW_t\("profile\.win"\) : SOW_t\("profile\.loss"\)\)/);
    assert.match(profileSource, /profile\.achievement_category_/);
    assert.match(profileSource, /profile\.achievement_" \+ achievement\.id/);
    assert.match(profileSource, /\(data\.leaders \|\| \[\]\)\.map\(profileLeaderCard\)/);
    assert.doesNotMatch(profileSource, /achievement\.id === "first_command"|achievement\.id === "first_victory"/);
    assert.match(profileSource, /leaderboard\?kind=victories&cursor=/);
    assert.match(profileSource, /profileVictoryLeaderboardCursor = data\.next_cursor/);
    assert.match(profileSource, /data-command='load_victory_more'/);
    assert.match(profileSource, /profile\.preferred_commander/);
    assert.match(profileSource, /sow-profile__dossier/);
    assert.match(profileSource, /sow-profile__tools/);
    assert.match(profileSource, /aria-controls='sow-profile-panel-active'/);
    assert.match(profileSource, /id='sow-profile-panel-active'[\s\S]*role='tabpanel'[\s\S]*aria-labelledby='sow-profile-tab-/);
    assert.match(shellSource, /\["ArrowLeft", "ArrowRight", "Home", "End"\][\s\S]*profileTabs\[nextProfileTabIndex\]\.click\(\)[\s\S]*selectedProfileTab\.focus\(\)/);
    assert.match(profileCss, /--sow-profile-gold-line/);
    assert.match(profileCss, /grid-template-columns: minmax\(0, 1fr\) minmax\(228px, 272px\)/);
    assert.match(profileCss, /@media \(max-width: 900px\)[\s\S]*sow-profile__page[\s\S]*grid-template-columns: minmax\(0, 1fr\)/);
    assert.match(profileCss, /sow-profile__tools \.sow-profile__section[\s\S]*background: var\(--sow-profile-panel\)/);
    assert.match(profileCss, /@media \(max-width: 700px\), \(orientation: portrait\)[\s\S]*sow-profile__stats[\s\S]*repeat\(2, minmax\(0, 1fr\)\)/);
    assert.match(profileCss, /:focus-visible\s*\{\s*outline: 2px solid var\(--sow-cyan\)/);
    assert.match(profileCss, /sow-profile__victory-row > b:first-child[\s\S]*color: var\(--sow-cyan\)/);
    assert.match(profileCss, /\.sow-profile__heading[\s\S]*var\(--sow-hero\)/);
    assert.match(profileCss, /\.sow-profile__match-kda\s*\{\s*grid-column: 3;\s*grid-row: 2;/);
    assert.doesNotMatch(profileCss, /\.sow-profile__match-kda\s*\{\s*display:\s*none/);
    assert.match(shellSource, /heroImage\(profileData && profileData\.preferred_leader\)/);
    assert.doesNotMatch(profileSource.slice(profileSource.indexOf('profileTab === "overview"'), profileSource.indexOf('profileTab === "leaders"')), /profileLeaderCard|profileAchievementsMarkup/);
    assert.doesNotMatch(profileSource, /loadProfileRatings|profileTab === "ranked"|profileRecentLeadersPanel/);
    assert.doesNotMatch(profileCss, /sow-profile__favorites|sow-profile__rating/);
    assert.doesNotMatch(nativeProfileSource, /LoadProfileRatings|ProfileRatingsLoaded|ratings_loaded|ProfileTab::Ranked|ranked records/i);
});


test("match history row shows the leader and verified KDA while provisional exits never become losses", () => {
    const rowStart = profileSource.indexOf("function profileMatchRow");
    const rowEnd = profileSource.indexOf("function profileHeaderMarkup", rowStart);
    const context = {
        SOW_t: (key, values) => key === "profile.kda_value"
            ? `${values.kills}/${values.deaths}/${values.assists}`
            : key,
        asset: (path) => `/assets/${path}`,
        esc: (value) => String(value),
        formatMapName: () => "Britain",
        leaderById: (id) => ({ slug: id.toLowerCase() }),
        leaderDisplayName: () => "Boudica",
        profileMatchDate: () => "Today",
        profileMatchDuration: () => "12m 4s"
    };
    vm.runInNewContext(`${profileSource.slice(rowStart, rowEnd)}; this.profileMatchRow = profileMatchRow;`, context);

    const verified = context.profileMatchRow({
        match_id: "verified-match", verified: true, won: false,
        leader: "Boudica", kills: 4, deaths: 2, assists: 1
    });
    assert.match(verified, /Boudica/);
    assert.match(verified, /4\/2\/1/);
    assert.match(verified, /profile\.loss/);

    const provisional = context.profileMatchRow({
        match_id: "provisional-match", verified: false, provisional: true,
        leader: "Boudica", kills: 0, deaths: 0, assists: 0
    });
    assert.match(provisional, /profile\.match_pending/);
    assert.doesNotMatch(provisional, /profile\.loss/);
});


test("settings panel keeps only useful controls and real account state", () => {
    const settings = [
        renderSettingsBody(shellSource, "function authApi"),
        renderSettingsBody(pokiSource, "function pokiLicenseUrl")
    ];
    for (const body of settings) {
        assert.match(body, /menu\.settings/);
        assert.match(body, /menu\.settings_preferences/);
        assert.match(body, /menu\.game_links/);
        assert.match(body, /sow-menu__settings-section/);
        assert.match(body, /data-command='toggle_settings'/);
        assert.match(body, /menu\.music_volume/);
        assert.match(body, /menu\.motion_animation/);
        assert.match(body, /menu\.language/);
        assert.match(body, /account-value/);
        assert.match(body, /settings-account/);
        assert.doesNotMatch(body, /system_configuration|menu\.done|master_audio|settings-mute|account-row|WOU-ID|\+1/);
    }
    assert.match(shellSource, /SOW_getWouSession/);
    assert.match(shellSource, /data-command='confirm_sign_out'/);
    assert.match(shellSource, /data-command='cancel_sign_out'/);
    assert.match(shellSource, /data-menu-overlay='settings'/);
    assert.match(shellSource, /event\.target === settingsOverlay/);
    assert.match(shellSource, /event\.key === "Escape" && settingsOpen/);
    assert.doesNotMatch(lobbiesSource, /lobbies\.connecting_server/);
});


test("Opus theme preserves its crossfade and resumes from ordinary trusted clicks", async () => {
    const start = shellSource.indexOf("    var menuMusic = {");
    const end = shellSource.indexOf("    function renderTopbar()", start);
    assert.ok(start >= 0 && end > start, "shared shell has no menu music lifecycle");

    function createHarness(assetsUrl = "https://cdn.test/assets/") {
        const audios = [];
        const mediaMetadata = [];
        const frames = new Map();
        const logs = [];
        const windowListeners = new Map();
        const documentListeners = new Map();
        const body = { children: [], appendChild(element) { this.children.push(element); element.parentNode = this; return element; } };
        let frameId = 0;
        class FakeAudio {
            constructor() {
                this.listeners = {};
                this.paused = true;
                this.currentTime = 0;
                this.duration = 240;
                this.volume = 1;
                this.preload = "auto";
                this.loop = false;
                this.playCalls = 0;
                this.pauseCalls = 0;
                this.loadCalls = 0;
                this.nextPlayError = null;
                this.error = null;
                audios.push(this);
            }
            addEventListener(type, callback) { (this.listeners[type] ||= []).push(callback); }
            dispatch(type) { (this.listeners[type] || []).forEach(callback => callback()); }
            play() {
                this.playCalls += 1;
                if (this.nextPlayError) {
                    const error = this.nextPlayError;
                    this.nextPlayError = null;
                    return Promise.reject(error);
                }
                this.paused = false;
                return Promise.resolve();
            }
            pause() { this.pauseCalls += 1; this.paused = true; }
            load() { this.loadCalls += 1; }
            set src(value) { this._src = value; }
            get src() { return this._src; }
        }
        class FakeMediaMetadata {
            constructor(value) { Object.assign(this, value); mediaMetadata.push(this); }
        }
        const mediaSession = { metadata: null };
        const window = {
            Audio: FakeAudio,
            MediaMetadata: FakeMediaMetadata,
            URL,
            navigator: { mediaSession },
            SOW_ASSETS_URL: assetsUrl,
            SOW_BUILD_TS: "build-42",
            SOW_t: key => key,
            addEventListener(type, callback, options) {
                (windowListeners.get(type) || windowListeners.set(type, []).get(type)).push({ callback, once: Boolean(options && options.once) });
            },
            dispatchEvent(event) {
                const listeners = windowListeners.get(event.type) || [];
                listeners.slice().forEach(listener => {
                    listener.callback(event);
                    if (listener.once) listeners.splice(listeners.indexOf(listener), 1);
                });
            },
            requestAnimationFrame(callback) { const id = ++frameId; frames.set(id, callback); return id; },
            cancelAnimationFrame(id) { frames.delete(id); }
        };
        const root = { contains(target) { return Boolean(target && target.insideMenu); } };
        const document = {
            body,
            baseURI: "https://cdn.test/play/",
            querySelector(selector) {
                return selector === 'meta[name="sow-media-artwork"]'
                    ? { content: "../icon-192.png" }
                    : null;
            },
            addEventListener(type, callback, capture) {
                (documentListeners.get(type) || documentListeners.set(type, []).get(type)).push({ callback, capture: Boolean(capture) });
            },
            dispatchClick(target, isTrusted = true) {
                const event = { target, isTrusted, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
                (documentListeners.get("click") || []).filter(listener => listener.capture)
                    .forEach(listener => listener.callback(event));
                return event;
            }
        };
        const context = {
            window,
            document,
            root,
            console: {
                error: (...args) => logs.push(["error", ...args]),
                warn: (...args) => logs.push(["warn", ...args])
            }
        };
        vm.runInNewContext(shellSource.slice(start, end) +
            "this.musicApi = { syncMenuMusic, menuMusic, menuMusicFinishLoaderCycle, menuMusicSetCinematicActive };", context);
        return {
            api: context.musicApi,
            audios,
            logs,
            mediaMetadata,
            mediaSession,
            window,
            document,
            body,
            frame(timestamp) {
                const next = frames.entries().next().value;
                assert.ok(next, "expected a scheduled music fade frame");
                frames.delete(next[0]);
                next[1](timestamp);
            }
        };
    }
    const flushPlay = async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); };
    const state = (phase, volume = 0.8, muted = false, loader_job = "Boot", loader_cycle_id = 0, waiting = false) => ({
        phase, loader_job, loader_cycle_id, waiting, settings: { music_volume: volume, mute_all: muted }
    });

    const h = createHarness();
    h.api.syncMenuMusic(state("Playing", 0.8, false, "EnterGame", 1), false);
    assert.equal(h.audios.length, 2, "both routes prepare the existing loop player during the loader");
    const [first, second] = h.audios;
    assert.equal(first.src, "https://cdn.test/assets/gameplay/music/shadows-of-war.webm?v=build-42");
    assert.equal(second.src, first.src, "both decks use Opus");
    assert.equal(first.preload, "none", "music never competes with the WASM download");
    assert.equal(second.preload, "none", "the loop's second deck does not preload a duplicate copy");
    assert.equal(first.loadCalls, 0, "no theme bytes are requested during the engine download");
    assert.equal(second.loadCalls, 0);
    h.window.dispatchEvent({ type: "sow:wasm-bytes-ready" });
    assert.equal(first.loadCalls, 1, "the theme starts fetching once the engine bytes are down");
    assert.equal(second.loadCalls, 0, "only the active deck buffers early");
    assert.equal(first.playCalls, 0, "music waits until the initial loader finishes");
    h.api.menuMusicSetCinematicActive(true);
    h.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(first.paused, true, "an opening tutorial cinematic keeps music silent after the loader");
    assert.equal(first.playCalls, 0, "the theme does not start underneath the cinematic");
    h.api.menuMusicSetCinematicActive(false);
    await flushPlay();
    assert.equal(first.paused, false, "the theme starts when the tutorial cinematic ends");
    assert.equal(first.playCalls, 1);
    assert.equal(first.volume, 0.8);
    assert.equal(h.mediaMetadata.length, 1, "the existing player publishes metadata when playback starts");
    assert.equal(h.mediaMetadata[0].title, "Shadows of War");
    assert.deepEqual(Array.from(h.mediaMetadata[0].artwork, art => ({
        src: art.src,
        sizes: art.sizes,
        type: art.type
    })), [{ src: "https://cdn.test/icon-192.png", sizes: "192x192", type: "image/png" }]);
    assert.equal(h.mediaSession.metadata, h.mediaMetadata[0]);

    first.currentTime = 55;
    h.api.menuMusicSetCinematicActive(true);
    assert.equal(first.paused, true, "a later cinematic pauses the current theme");
    h.api.menuMusicSetCinematicActive(false);
    await flushPlay();
    assert.equal(first.currentTime, 55, "the theme resumes where it paused after a later cinematic");
    h.api.syncMenuMusic(state("Splash", 0.8, false, "Boot", 1), false);
    h.api.syncMenuMusic(state("Playing", 0.8, false, "Boot", 1), false);
    await flushPlay();
    assert.equal(first.currentTime, 55, "menu, tutorial, and gameplay state changes keep the current position");
    assert.equal(first.paused, false, "playable phases keep the theme running");

    first.currentTime = 237.01;
    first.dispatch("timeupdate");
    await flushPlay();
    assert.ok(h.api.menuMusic.crossfade, "the existing loop crossfade starts near the track end");
    h.api.syncMenuMusic(state("Playing", 0.8, false, "EnterGame", 2), false);
    assert.equal(h.api.menuMusic.crossfade, null, "entering a loader cancels an in-progress loop crossfade");
    assert.equal(first.paused, true, "the active deck pauses as the game loader begins");
    assert.equal(second.paused, true, "the incoming crossfade deck also pauses");
    first.dispatch("timeupdate");
    assert.equal(h.api.menuMusic.crossfade, null, "late media events cannot restart the loop during a loader");
    assert.equal(h.api.menuMusicFinishLoaderCycle(2), true);
    await flushPlay();
    assert.equal(first.currentTime, 0, "a new game starts the theme from its opening");
    assert.equal(second.currentTime, 0, "both decks reset at a game boundary");
    assert.equal(first.paused, false, "the theme resumes after the game loader closes");
    assert.equal(second.paused, true, "only one deck resumes after the loader");

    h.api.syncMenuMusic(state("MainMenu", 0.8, false, "ExitGame", 3), false);
    assert.equal(first.paused, true, "the return-to-menu loader is silent");
    assert.equal(h.api.menuMusicFinishLoaderCycle(3), true);
    await flushPlay();
    assert.equal(first.currentTime, 0, "returning to the menu restarts the theme");
    assert.equal(first.paused, false);

    h.api.syncMenuMusic(state("MainMenu", 0.8, false, "Boot", 3, true), false);
    first.currentTime = 37;
    h.api.syncMenuMusic(state("MainMenu", 0.8, false, "Boot", 3, false), false);
    await flushPlay();
    assert.equal(first.currentTime, 0, "leaving a lobby without a loader restarts the theme");
    assert.equal(first.paused, false);

    first.currentTime = 23;
    assert.equal(h.api.menuMusicFinishLoaderCycle(2), false, "an old loader event cannot reset the current track");
    assert.equal(h.api.menuMusicFinishLoaderCycle(3), false, "a duplicate completion cannot reset the current track");
    assert.equal(first.currentTime, 23);

    const returning = createHarness();
    returning.api.syncMenuMusic(state("MainMenu"), false);
    assert.equal(returning.api.menuMusic.decks[0].playCalls, 0, "Boot stays silent while its loader is visible");
    returning.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(returning.api.menuMusic.decks[0].paused, false, "a returning account starts music on the menu route");

    h.api.syncMenuMusic(state("MainMenu", 0.3), true, 0.3);
    assert.equal(first.volume, 0.3, "the slider changes music gain");
    h.api.syncMenuMusic(state("MainMenu", 0.3, true), false);
    assert.equal(first.volume, 0, "global mute silences music");
    h.api.syncMenuMusic(state("MainMenu", 0.3, false), false);
    assert.equal(first.volume, 0.3, "unmute restores the music slider level");

    first.duration = 240;
    first.currentTime = 237.01;
    first.dispatch("timeupdate");
    await flushPlay();
    h.frame(2000);
    h.frame(3500);
    assert.ok(Math.abs(first.volume - 0.15) < 0.001 && Math.abs(second.volume - 0.15) < 0.001,
        "the loop crossfade reaches an even mix halfway through three seconds");
    h.frame(5000);
    assert.equal(h.api.menuMusic.active, 1);
    assert.equal(first.paused, true);
    assert.equal(first.currentTime, 0);

    const blocked = createHarness();
    blocked.api.syncMenuMusic(state("MainMenu"), false);
    blocked.audios[0].nextPlayError = Object.assign(new Error("gesture required"), { name: "NotAllowedError" });
    blocked.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(blocked.api.menuMusic.playBlocked, true, "the browser rejection is remembered for the next user gesture");
    assert.equal(blocked.api.menuMusic.unlockButton, undefined, "blocked autoplay creates no recovery button");
    assert.equal(blocked.body.children.length, 0, "music recovery adds no floating UI");
    blocked.document.dispatchClick({ insideMenu: false }, false);
    await flushPlay();
    assert.equal(blocked.api.menuMusic.decks[0].playCalls, 1, "an untrusted event cannot unlock playback");
    const musicClick = blocked.document.dispatchClick({ insideMenu: false });
    await flushPlay();
    assert.equal(blocked.api.menuMusic.decks[0].paused, false, "an ordinary trusted click starts the theme");
    assert.equal(musicClick.defaultPrevented, false, "music recovery does not consume the clicked action");

    const resumed = createHarness();
    resumed.api.syncMenuMusic(state("Playing"), false);
    resumed.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    const resumedDeck = resumed.api.menuMusic.decks[0];
    const callsBeforeSleep = resumedDeck.playCalls;
    resumedDeck.pause();
    const wakeClick = resumed.document.dispatchClick({ insideMenu: false });
    await flushPlay();
    assert.equal(resumedDeck.playCalls, callsBeforeSleep + 1, "a normal game click resumes music paused by the platform");
    assert.equal(resumedDeck.paused, false);
    assert.equal(wakeClick.defaultPrevented, false, "audio recovery leaves the game click intact");

    const unsupported = createHarness();
    unsupported.api.syncMenuMusic(state("MainMenu"), true);
    await flushPlay();
    const failedOpusDeck = unsupported.api.menuMusic.decks[0];
    failedOpusDeck.dispatch("error");
    assert.equal(failedOpusDeck.src, "https://cdn.test/assets/gameplay/music/shadows-of-war.webm?v=build-42",
        "an unsupported Opus file gets no alternate format");
    assert.ok(unsupported.logs.some(entry => entry[0] === "error"), "Opus failure is visible in the console");
    assert.doesNotMatch(shellSource, /shadows-of-war\.mp3|menuMusicSwitchToMp3|canPlayType/,
        "the player has no codec fallback");

    const withoutMediaSession = createHarness();
    delete withoutMediaSession.window.navigator;
    withoutMediaSession.api.syncMenuMusic(state("MainMenu"), true);
    withoutMediaSession.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(withoutMediaSession.api.menuMusic.decks[0].paused, false,
        "missing platform metadata support does not block music playback");

    const native = createHarness("../assets");
    native.api.syncMenuMusic(state("MainMenu"), false);
    assert.equal(native.api.menuMusic.decks[0].src, "../assets/gameplay/music/shadows-of-war.webm?v=build-42",
        "Tauri resolves the same music through its local SOW_ASSETS_URL");
    assert.match(shellSource, /addEventListener\("sow:loader-ready", menuMusicStartAtLoaderEnd, \{ once: true \}\)/);
    assert.match(shellSource, /window\.addEventListener\("sow:loader-cycle-ready", function \(event\)[\s\S]*?menuMusicFinishLoaderCycle\(cycleId\)/,
        "the existing loader completion event releases the paused theme once per cycle");
    assert.doesNotMatch(shellSource, /menuMusicPauseForGame|menuMusic\.inMenu|menuMusic\.fade/,
        "music is not tied to a menu-only playback mode");
    assert.doesNotMatch(shellSource, /menuMusicShowUnlockPrompt|data-sow-music-unlock|sow-menu__music-unlock/);
    assert.doesNotMatch(storyCss, /sow-menu__music-unlock/);
});


test("music volume no longer controls SFX and music assets reach every shared-shell package", () => {
    const musicCommand = webMenu.match(/WebMenuCommand::SetMusicVolume \{ value \} => \{[\s\S]*?\n                \}/);
    assert.ok(musicCommand);
    assert.match(musicCommand[0], /settings_state\.music_volume = volume/);
    assert.doesNotMatch(musicCommand[0], /set_master_volume/);
    assert.match(webMenu, /WebMenuCommand::SetMute \{ value \} => \{[\s\S]*set_master_volume\(if value \{ 0\.0 \} else \{ 0\.8 \}\)/);
    assert.match(shellSource, /syncMenuMusic\(state, false\)/);
    assert.match(shellSource, /syncMenuMusic\(state, true\)/);
    assert.match(siteDistSource, /copy_dir\(&src\.join\("gameplay\/music"\), &dst\.join\("gameplay\/music"\)\)/);
    assert.match(siteDistSource, /assets_gameplay\.join\("music"\)/);
    assert.equal((siteDistSource.match(/"assets\/gameplay\/music\/shadows-of-war\.webm"/g) || []).length, 4,
        "web, Poki, Jest, and native layout checks require the theme assets");
    assert.match(siteDistSource, /paths\.assets_gameplay\.join\("music"\)/,
        "native package fingerprint and copy include music assets");
    assert.doesNotMatch(siteDistSource, /shadows-of-war\.mp3/,
        "no package requires an MP3 duplicate");
    assert.match(prodDistSource, /&paths\.assets_gameplay,/,
        "the web cache fingerprint includes gameplay music");
});


test("hero purchase stays server-backed, direct, and visually honest", () => {
    assert.match(storeSource, /var leaderActions = !offer \|\| offer\.owned \|\| offer\.permanently_free \? ""/);
    assert.doesNotMatch(storeSource, /offer\.free_rotation \|\| offer\.available/);
    assert.match(storeSource, /data-command='unlock_leader'/);
    assert.match(storeSource, /data-command='open_skin_purchase'/);
    assert.match(storeSource, /sow-store__currency-amount--insufficient/);
    assert.match(storeSource, /command = confirm \? "open_product_purchase" : "buy_product"/);
    assert.match(storeSource, /skin\.direct_product_id, SOW_t\("store\.buy"\), true/);
    assert.match(shellSource, /data-menu-overlay='purchase'/);
    assert.match(shellSource, /openLeaderPurchase/);
    assert.match(shellSource, /send\("unlock_leader", \{ leader_id: purchaseModal\.leaderId, currency: purchaseModal\.currency \}\)/);
    assert.match(shellSource, /send\("unlock_skin", \{ skin_id: purchaseModal\.skinId \}\)/);
    assert.match(storeSource, /purchaseModal\.phase = "success"/);
    assert.match(storeSource, /purchaseItemOwned/);
    assert.match(storeSource, /reducedRewardMotion\(\) \? " is-reduced-motion"/);
    assert.match(storeSource, /sow-purchase-modal__visual/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-modal__layout/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /sow-purchase-art-reveal/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-impact-ring/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-modal-particle/);
    const leaderStart = coreSource.indexOf("function leaderById(id)");
    const leaderEnd = coreSource.indexOf("function leaderPerk", leaderStart);
    const context = { state: { leaders: [{ id: "SunTzu", slug: "sun_tzu" }] } };
    vm.runInNewContext(coreSource.slice(leaderStart, leaderEnd) + "this.leaderById = leaderById;", context);
    assert.equal(context.leaderById("sun_tzu").slug, "sun_tzu");
});


test("hero selection opens the shared catalog for equipped and purchasable skins", () => {
    const rowStart = storeSource.indexOf("function renderLeaderPurchase");
    const rowEnd = storeSource.indexOf("function renderSkinPickerModal", rowStart);
    assert.ok(rowStart >= 0 && rowEnd > rowStart);
    assert.match(storeSource.slice(rowStart, rowEnd), /data-command='open_skin_picker'/);
    assert.doesNotMatch(storeSource.slice(rowStart, rowEnd), /if \(!offer \|\| offer\.owned\) return/);
    assert.match(storeSource, /function renderSkinPickerModal\(\)[\s\S]*skins\.map\(renderStoreSkinPromo\)/);
    assert.match(storeSource, /data-menu-overlay='skin-picker'/);
    assert.match(storeSource, /data-command='equip_skin'/);
    assert.match(storeSource, /data-command='open_skin_purchase'/);
    assert.match(shellSource, /command === "open_skin_picker"[\s\S]*loadStoreCatalog\(\)/);
    assert.match(shellSource, /command === "close_skin_picker"/);
    assert.match(shellSource, /syncOverlay\("skin-picker"[\s\S]*syncOverlay\("purchase"/);
    assert.match(profileCss, /\.sow-menu__modal\.sow-skin-picker/);
});


test("leader display names use the catalog translation keys for every leader", () => {
    const start = coreSource.indexOf("function leaderTranslationSlug(leader)");
    const end = coreSource.indexOf("function mapInfo(key)", start);
    assert.ok(start >= 0 && end > start);
    const requestedKeys = [];
    const context = {
        SOW_t(key) {
            requestedKeys.push(key);
            return "translated:" + key;
        }
    };
    vm.runInNewContext(coreSource.slice(start, end) + "this.leaderDisplayName = leaderDisplayName; this.leaderHistoricalName = leaderHistoricalName;", context);

    const slugs = [
        "caesar", "cleopatra", "ragnar", "sun_tzu", "alexander", "genghis_khan",
        "richard_the_lionheart", "vercingetorix", "boudica", "lady_six_sky", "leonidas", "napoleon"
    ];
    const expectedSlugs = [
        "caesar", "cleopatra", "ragnar", "suntzu", "alexander", "genghiskhan",
        "richard", "vercingetorix", "boudica", "ladysixsky", "leonidas", "napoleon"
    ];
    for (let i = 0; i < slugs.length; i++) {
        const expectedKey = "heroes.leader_" + expectedSlugs[i] + "_name";
        assert.equal(context.leaderDisplayName({ slug: slugs[i], name: "Fallback" }), "translated:" + expectedKey);
        assert.equal(requestedKeys[i], expectedKey);
    }
    assert.equal(
        context.leaderHistoricalName({ slug: "richard_the_lionheart", name: "Richard the Lionheart" }),
        "translated:heroes.leader_richard_historical"
    );
});


test("direct purchases wait for delivery before celebrating", () => {
    assert.match(storeSource, /EXTERNAL_PURCHASE_POLL_MS = 3000/);
    assert.match(storeSource, /EXTERNAL_PURCHASE_WAIT_MS = 120000/);
    assert.match(storeSource, /sow_pending_store_purchase_v1/);
    assert.match(storeSource, /profileApi\("\/store\/purchases"\)/);
    assert.match(storeSource, /record\.status === "granted"/);
    assert.match(storeSource, /onComplete: function \(\) \{\s*completeExternalPurchase\(productId\);/);
    assert.match(storeSource, /purchaseModal\.type === "product"[\s\S]*attempt\.delivery_id[\s\S]*profileHasExternalPurchase/);
    assert.match(storeSource, /purchaseModal\.type === "bundle"[\s\S]*beginBundlePurchasePresentation/);
    assert.match(storeSource, /purchaseModal\.phase === "spending" \|\| purchaseModal\.phase === "success"/);
    assert.match(shellSource, /resumeExternalPurchaseDelivery\(true\)/);
    assert.doesNotMatch(shellSource, /URLSearchParams\(window\.location\.search\)\.get\("purchase"\)/);
});


test("weekly free rotation is a splash-only status", () => {
    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function heroesRoster", cardStart);
    assert.ok(cardStart >= 0 && cardEnd > cardStart);
    assert.doesNotMatch(heroesSource.slice(cardStart, cardEnd), /data-hero-status|weekly_free_rotation/);
    const predicateStart = heroesSource.indexOf("function isWeeklyFreeRotation");
    const predicateEnd = heroesSource.indexOf("function renderHeroesCard", predicateStart);
    assert.ok(predicateStart >= 0 && predicateEnd > predicateStart);
    const context = {};
    vm.runInNewContext(heroesSource.slice(predicateStart, predicateEnd) + "this.isWeeklyFreeRotation = isWeeklyFreeRotation;", context);
    assert.equal(context.isWeeklyFreeRotation({ available: false, free_rotation: true, owned: false }), false);
    assert.equal(context.isWeeklyFreeRotation({ available: true, free_rotation: true, owned: false }), true);
    assert.equal((heroesSource.match(/isWeeklyFreeRotation\(activeLeader\)/g) || []).length, 2);
    assert.match(profileCss, /\.sow-heroes__rotation/);
});


test("random leader is hidden from welcome purchase choices", () => {
    const start = heroesSource.indexOf("function randomLeaderName");
    const end = heroesSource.indexOf("function heroesRoster", start);
    assert.ok(start >= 0 && end > start);
    const context = {
        state: { menu_guides: { welcome: { status: "active", current_step: "welcome_offer_selection" } } },
        heroesSearchQuery: "",
        heroesRegionFilter: "all",
        SOW_t: key => key,
        asset: path => "/assets/" + path,
        esc: value => String(value)
    };
    vm.runInNewContext(heroesSource.slice(start, end) + "this.renderRandomLeaderCard = renderRandomLeaderCard;", context);
    assert.equal(context.renderRandomLeaderCard(true), "");
    context.state.menu_guides.welcome.current_step = "welcome_owned_selection";
    assert.equal(context.renderRandomLeaderCard(false), "");
    context.state.menu_guides.welcome.current_step = "welcome_complete";
    assert.match(context.renderRandomLeaderCard(false), /data-command='preview_random_leader'/);
});

// GUARD — owner decision. Read this before changing anything in it.
// The heroes featured picture picks its art by the SHAPE OF THE PANEL, not by
// the device or the file name: desktop/landscape = tall column panel ->
// *_mobile.webp (1080x1920); phone/portrait = short wide band ->
// *_desktop.webp (1920x1080). The file names are backwards, so this mapping
// was flipped without authorization three times (80c7043d 2026-09-24,
// b945feb3 2026-09-24, 12486980 2026-10-01) and the last one shipped broken.
// It runs in the ./sow p preflight: a red run here means someone flipped it
// again. Full rationale: heroesFeaturedArt() in main_menu.heroes.js and
// AGENTS.md -> "Heroes featured art (owner decision — do not flip)".

test("heroes featured art follows the panel shape, not the device name", () => {
    const why = "OWNER DECISION: desktop gets *_mobile.webp (tall column panel), " +
        "portrait screens get *_desktop.webp (short band panel). The file names are backwards on purpose. " +
        "Flipping this broke production in 80c7043d, b945feb3 and 12486980. " +
        "Read heroesFeaturedArt() and AGENTS.md before touching it.";
    const helperStart = heroesSource.indexOf("function heroesFeaturedArt");
    const helperEnd = heroesSource.indexOf("function updateHeroesPreview", helperStart);
    assert.ok(helperStart >= 0 && helperEnd > helperStart, "heroesFeaturedArt() must exist as the single owner of the mapping — " + why);
    const helper = heroesSource.slice(helperStart, helperEnd);
    assert.match(helper, /band: leaderArtUrl\(slug, "desktop"\)/, why);
    assert.match(helper, /column: leaderArtUrl\(slug, "mobile"\)/, why);
    assert.match(heroesSource, /OWNER DECISION — DO NOT "FIX" THIS MAPPING/,
        "the mapping must keep its explanation in the source — " + why);
    assert.equal((heroesSource.match(/heroesFeaturedArt\(activeLeader\.slug\)/g) || []).length, 2,
        "renderHeroes() and updateHeroesPreview() must both go through the helper — " + why);
    const previewStart = heroesSource.indexOf("function updateHeroesPreview");
    const previewEnd = heroesSource.indexOf("function renderHeroInfoModal", previewStart);
    const renderStart = heroesSource.indexOf("function renderHeroes()");
    assert.ok(previewStart >= 0 && previewEnd > previewStart && renderStart > previewEnd);
    const previewBody = heroesSource.slice(previewStart, previewEnd);
    const renderBody = heroesSource.slice(renderStart);
    assert.doesNotMatch(previewBody, /leaderArtUrl\(/, "no second inline art mapping in updateHeroesPreview() — " + why);
    assert.doesNotMatch(renderBody, /leaderArtUrl\(/, "no second inline art mapping in renderHeroes() — " + why);
    assert.match(previewBody, /source\.srcset = featuredArt\.band/, why);
    assert.match(previewBody, /image\.src = featuredArt\.column/, why);
    assert.match(renderBody, /source media='\(max-width: 680px\), \(orientation: portrait\)' srcset='" \+ esc\(featuredArt\.band\)/, why);
    assert.match(renderBody, /img src='" \+ esc\(featuredArt\.column\) \+ "' alt='[^']*' width='1080' height='1920'/,
        "the fallback <img> is the tall art, so 1080x1920 must stay — " + why);
    // The <picture> switches on exactly the media query that turns the panel
    // into a band, so art and layout can never disagree again.
    assert.match(profileCss,
        /@media \(max-width: 680px\), \(orientation: portrait\) \{[^@]*?\.sow-heroes__workspace \{\s*grid-template-columns: 1fr;/,
        "picture media query must equal the CSS band-layout query — " + why);
    // The info dialog is one tall card on every device; it is never swapped.
    assert.match(heroesSource, /function renderHeroInfoModal[\s\S]*?leaderArtUrl\(activeLeader\.slug, "mobile"\)/, why);
});


test("heroes preview never resolves the random option through leader i18n keys", () => {
    const previewStart = heroesSource.indexOf("function updateHeroesPreview");
    const previewEnd = heroesSource.indexOf("function renderHeroInfoModal", previewStart);
    assert.ok(previewStart >= 0 && previewEnd > previewStart);
    const previewBody = heroesSource.slice(previewStart, previewEnd);
    assert.match(previewBody, /image\.alt = randomSelected \? randomLeaderName\(\) : leaderDisplayName\(activeLeader\)/,
        "image alt must use randomLeaderName() when random is selected, or it reports heroes.leader_random_name missing");
    assert.match(previewBody, /randomSelected \? "" : leaderHistoricalName\(activeLeader\)/,
        "historical name must not call leaderHistoricalName() for the random option, or it reports heroes.leader_random_historical missing");
    assert.doesNotMatch(previewBody, /historical\.textContent = leaderHistoricalName\(activeLeader\)/,
        "historical text must go through the randomSelected branch");
});


test("random leader description speaks to the player, not about the server", () => {
    for (const locale of ["en", "es", "de", "fr", "it", "pt-br", "ru", "ja", "ko", "zh-cn", "ar", "tr", "vi", "id", "fil"]) {
        const catalog = fs.readFileSync(path.join(shell, `../../sow-i18n/strings/${locale}/web.toml`), "utf8");
        const line = catalog.split("\n").find((entry) => entry.startsWith("random_leader_description"));
        assert.ok(line && line.split("=")[1].trim().length > 2, `heroes.random_leader_description must exist in ${locale}`);
        assert.doesNotMatch(line, /server|servidor|serveur|Servidor|Server|Serveur|сервер|Сервер|servidor|サーバー|서버|服务器|الخادم|Sunucu|Máy chủ|khí/i,
            `heroes.random_leader_description must not talk about the server in ${locale}`);
    }
});


test("header exposes server progress currencies and keeps the real XP remainder", () => {
    assert.match(shellSource, /data-progression-gems-value/);
    assert.match(shellSource, /data-progression-laurels-value/);
    assert.doesNotMatch(pokiSource, /function renderTopbar|data-progression-gems-value/);
    assert.match(shellSource, /accountXp % 100/);
    assert.match(shellSource, /state\.gems/);
    assert.match(webMenu, /"gems": progress\.gems/);
});


test("campaign episode cards use leader portraits and emphasize the next playable chapter", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];
    assert.doesNotMatch(lobbiesSource, /campaign_tagline/);
    assert.match(lobbiesSource, /var mapName = episode\.id === "boudica" \? "eastanglia" : "northamerica"/);
    assert.match(lobbiesSource, /var mapArt = lobbyThumb\(\{ map_name: mapName \}\)/);
    assert.match(lobbiesSource, /asset\("gameplay\/avatars\/" \+ \(episode\.id === "boudica" \? "boudica" : "lady_six_sky"\) \+ "\.webp"\)/);
    assert.match(lobbiesSource, /sow-campaign__map-art[\s\S]*?sow-campaign__leader-art/);
    assert.match(lobbiesSource, /isNext \? " is-next"/);
    assert.match(lobbiesSource, /var watch = campaignCinematicAvailability\[episode\.id\]/);
    assert.match(lobbiesSource, /function discoverCampaignCinematics\(episodes\)[\s\S]*?definition\.entry[\s\S]*?video_src/);
    assert.match(lobbiesSource, /data-command='watch_campaign_cinematic'/);
    assert.match(campaignCss, /\.sow-campaign__episode-art img[\s\S]*?object-fit: cover/);
    assert.match(campaignCss, /\.sow-campaign__map-art[\s\S]*?brightness\(\.58\)/);
    assert.match(campaignCss, /\.sow-campaign__leader-art[\s\S]*?border-radius: 50%/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-next/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-locked \.sow-campaign__episode-art img[\s\S]*?grayscale/);
});


test("campaign card map and leader art match every Rust episode and its JSON assets", () => {
    const start = campaignModule.indexOf("pub fn episode_id(");
    const end = campaignModule.indexOf("pub fn from_episode_id(", start);
    assert.ok(start >= 0 && end > start);
    const episodeIds = [...campaignModule.slice(start, end).matchAll(/=> "([^"]+)"/g)].map(match => match[1]);
    assert.deepEqual(episodeIds, ["boudica", "six_sky_ep1", "six_sky_ep2", "six_sky_ep3"]);
    const expected = { boudica: ["eastanglia", "boudica"], six_sky_ep1: ["northamerica", "lady_six_sky"], six_sky_ep2: ["northamerica", "lady_six_sky"], six_sky_ep3: ["northamerica", "lady_six_sky"] };
    const campaignDir = path.join(shell, "../../assets/campaign");
    const cinematicEpisodes = [];
    for (const episodeId of episodeIds) {
        const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".json"), "utf8"));
        const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".triggers.json"), "utf8"));
        assert.equal(definition.episode_id, episodeId);
        const opening = definition.steps.find(step => step.id === definition.entry);
        if (opening && opening.video_src) {
            cinematicEpisodes.push(episodeId);
            assert.equal(opening.type, "scene", `${episodeId} opening cinematic is a scene`);
            assert.equal(opening.presentation, "cinematic", `${episodeId} opening cinematic uses the shared cinematic presentation`);
            assert.ok(opening.video_src.startsWith(`/assets/campaign/${episodeId}/`), `${episodeId} cinematic stays in its asset folder`);
            const mediaPath = path.join(shell, "../..", opening.video_src.replace(/^\/assets\//, "assets/"));
            assert.ok(fs.existsSync(mediaPath), `${episodeId} video asset exists`);
            const stem = mediaPath.replace(/\.(webm|mp4)$/i, "");
            assert.ok(fs.existsSync(stem + ".mp4") && fs.existsSync(stem + ".webm"), `${episodeId} cinematic ships both WebM and MP4 renditions`);
            assert.ok(fs.existsSync(stem + ".poster.webp"), `${episodeId} cinematic ships a poster`);
            assert.ok(fs.statSync(mediaPath).size <= 6 * 1024 * 1024, `${episodeId} cinematic rendition stays under 6 MB`);
        }
        if (episodeId === "boudica") {
            const ratioOverrides = definition.steps.filter(step => Object.hasOwn(step, "attack_ratio_on_enter"))
                .map(step => ({ id: step.id, ratio: step.attack_ratio_on_enter }));
            assert.deepEqual(ratioOverrides, [{ id: definition.entry, ratio: 0.25 }], "Boudica sets 25% once at entry and never resets the player's allocation");
        }
        assert.equal(roster.map, expected[episodeId][0], episodeId + " card map");
        assert.ok(Object.values(definition.speakers || {}).some(speaker => speaker.avatar === expected[episodeId][1]), episodeId + " card leader portrait");
        for (const faction of roster.factions) {
            assert.match(faction.id, /^[a-z][a-z0-9_]{0,95}$/i, `${episodeId}/${faction.name}: stable ID`);
            assert.ok(Number.isInteger(faction.starting_troops) && faction.starting_troops >= 0, `${episodeId}/${faction.name}: starting troops`);
            assert.ok(["allied", "neutral", "enemy"].includes(faction.relation), `${episodeId}/${faction.name}: relationship`);
            assert.ok(typeof faction.civ === "string" && faction.civ, `${episodeId}/${faction.name}: civilization`);
            assert.ok(typeof faction.leader === "string" && faction.leader, `${episodeId}/${faction.name}: leader`);
            for (const removed of ["role", "betrayal"]) assert.equal(Object.hasOwn(faction, removed), false, `${episodeId}/${faction.name}: no ${removed}`);
            assert.equal(Object.hasOwn(faction, "hostility"), false, `${episodeId}/${faction.name}: no combat override`);
            assert.match(faction.color, /^#[0-9a-f]{6}$/i, `${episodeId}/${faction.name}: map color`);
        }
    }
    assert.deepEqual(cinematicEpisodes, ["boudica"]);
    const watchFlow = tutorial.slice(tutorial.indexOf("function watchCampaignCinematic("), tutorial.indexOf("function showError(", tutorial.indexOf("function watchCampaignCinematic(")));
    assert.match(watchFlow, /SOWCampaign\.create\(data\.definition, data\.definition\.entry, data\.roster\)/);
    assert.doesNotMatch(watchFlow, /send\("start_campaign_episode"/);
});


test("menu guide uses the shared flow engine from its own entry", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "menu_guide_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "game_open", menu_guide: { entry: "menu_open", dismissible: true },
        steps: [
            { id: "game_open", type: "scene", title_key: "tutorial.game", next: "game_end" },
            { id: "game_end", type: "end", title_key: "tutorial.game_end" },
            { id: "menu_open", type: "scene", title_key: "tutorial.menu", next: "menu_campaign" },
            { id: "menu_campaign", type: "guide", title_key: "tutorial.campaign", trigger: { type: "ui", action: "menu_campaign", scope: "step" }, guide: { kind: "ui", target: "menu_campaign", gesture: "tap" }, next: "menu_end" },
            { id: "menu_end", type: "end", title_key: "tutorial.menu_end" }
        ]
    };
    const report = campaign.validate(definition);
    assert.deepEqual(report.errors, []);
    assert.deepEqual(report.warnings, []);
    const machine = campaign.create(definition, definition.menu_guide.entry);
    assert.equal(machine.view().step.id, "menu_open");
    machine.update({}, {}, 0);
    machine.advance(null, "menu_open");
    machine.update({}, { menu_campaign: 1 }, 1);
    machine.update({}, { menu_campaign: 1 }, 801);
    assert.equal(machine.view().step.id, "menu_end");
});


test("first multiplayer guide waits for confirmed outcomes and stays completed", async () => {
    const { state, receipt } = confirmedMenuReturn();
    state.menu_return = {
        ...state.menu_return, source: "multiplayer", match_id: "match-1", receipt_ids: ["match-1"]
    };
    state.reward_receipts = [{ ...receipt, id: "match-1", match_id: "match-1", verification_status: "verified" }];
    state.menu_guides = { first_multiplayer: { completed_steps: [], status: "active", selected_leader: null, source_match_id: "match-1" } };
    const harness = makeMenuGuideHarness();
    const shell = makeMenuShellRewardHarness(harness, state);
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_profile");
    shell.setProfile({ accountId: "account-a", tab: "overview", complete: true });
    shell.setScreen("profile");
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_history", "a ready own profile confirms the first step");

    shell.setProfile({
        accountId: "account-a", tab: "history", complete: true,
        entry: { historyLoaded: true, historyError: false }, history: [{ match_id: "match-1" }]
    });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_store", "the return match must appear in the loaded history");

    shell.setStoreReady(true, false);
    shell.setScreen("store");
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_home", "the store advances only when its catalog is ready");

    shell.setScreen("home");
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_quick_match");
    shell.setState({ ...state, quick_match_revision: 1 });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_quick_match", "the accepted match closes the guide");
    harness.flushSaveRetries();
    assert.ok(harness.calls.some(call => call.type === "save_menu_guide_progress" && call.status === "completed"));

    const completedState = {
        ...state,
        menu_guides: { first_multiplayer: { completed_steps: menuGuideDefinition.menu_guides.first_multiplayer.step_ids, status: "completed", selected_leader: null, source_match_id: "match-1" } }
    };
    const reloaded = makeMenuGuideHarness(harness.storage);
    makeMenuShellRewardHarness(reloaded, completedState).maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(reloaded.renderedSteps, [], "a server-completed first-match guide does not replay");
});


test("first multiplayer guide waits for its exact settled receipt after reload", async () => {
    const { state, receipt } = confirmedMenuReturn();
    state.menu_return = undefined;
    const missingSourceState = {
        ...state,
        menu_guides: { first_multiplayer: { completed_steps: [], status: "active", selected_leader: null } },
        reward_receipts: [{ ...receipt, id: "match-1", match_id: "match-1", verification_status: "verified" }]
    };
    const missingSourceGuide = makeMenuGuideHarness();
    makeMenuShellRewardHarness(missingSourceGuide, missingSourceState).maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(missingSourceGuide.renderedSteps, [], "a missing server source ID never falls back to a client guess");

    state.menu_guides = {
        first_multiplayer: { completed_steps: [], status: "active", selected_leader: null, source_match_id: "match-1" }
    };
    state.reward_receipts = [{ ...receipt, id: "other-match", match_id: "other-match", verification_status: "verified" }];
    const harness = makeMenuGuideHarness();
    const shell = makeMenuShellRewardHarness(harness, state);
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(harness.renderedSteps, [], "a different settled match cannot start the first-match guide on boot");

    shell.setState({
        ...state,
        reward_receipts: [
            { ...receipt, id: "other-match", match_id: "other-match", status: "presented", presented_at: 1, verification_status: "verified" },
            { ...receipt, id: "match-1", match_id: "match-1", status: "pending", verification_status: "pending" }
        ]
    });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(harness.renderedSteps, [], "the exact source match still waits for final server settlement");

    shell.setState({
        ...state,
        reward_receipts: [{ ...receipt, id: "match-1", match_id: "match-1", verification_status: "rejected" }]
    });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_profile", "the exact final applied receipt releases it after presentation");

    shell.setProfile({ accountId: "account-a", tab: "overview", complete: true });
    shell.setScreen("profile");
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_history");
    shell.setProfile({
        accountId: "account-a", tab: "history", complete: true,
        entry: { historyLoaded: true, historyError: false }, history: [{ match_id: "other-match" }]
    });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_history", "another match in history cannot satisfy the stored source ID");
    shell.setProfile({
        accountId: "account-a", tab: "history", complete: true,
        entry: { historyLoaded: true, historyError: false }, history: [{ match_id: "match-1" }]
    });
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.renderedSteps.at(-1), "first_store", "history must contain the exact source match");
});

// Six Sky stays closed until its shared campaign registry publishes the episodes.
