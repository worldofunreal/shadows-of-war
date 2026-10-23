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
const heroesSource = fs.readFileSync(path.join(shell, "main_menu.heroes.js"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.js"), "utf8");
const portalsSource = fs.readFileSync(path.join(shell, "sdk/store_portals.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const sessionSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/session.rs"), "utf8");
const actionsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/interact/actions.rs"), "utf8");
const netUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/update/mod.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const progressSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/progress.rs"), "utf8");
const dataDbSource = fs.readFileSync(path.join(shell, "../../sow-data/src/db.rs"), "utf8");
const dataMainSource = fs.readFileSync(path.join(shell, "../../sow-data/src/main.rs"), "utf8");
const accountSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/account.rs"), "utf8");
const bootstrapSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/bootstrap.rs"), "utf8");
const identitySource = fs.readFileSync(path.join(shell, "../../sow-client/src/anonymous_identity.rs"), "utf8");
const assetSource = fs.readFileSync(path.join(shell, "../../sow-client/src/asset.rs"), "utf8");
const relaySource = fs.readFileSync(path.join(shell, "../../sow-relay/src/main.rs"), "utf8");
const serverSource = fs.readFileSync(path.join(shell, "../../sow-server/src/main.rs"), "utf8");
const verificationSource = fs.readFileSync(path.join(shell, "../../sow-server/src/verification.rs"), "utf8");
const menuCssFiles = [
    "main_menu.base.css",
    "main_menu.lobbies.css"
];
const menuCss = Object.fromEntries(menuCssFiles.map((file) => [file, fs.readFileSync(path.join(shell, file), "utf8")]));

function makeMenuGuideHarness(storage = new Map()) {
    const calls = [], renderedSteps = [], saveTimers = new Map();
    let timerId = 0;
    let clockNow = Date.now();
    let destroyedViews = 0;
    let dismissMenuGuide = null;
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
                dismissMenuGuide = options && options.onDismiss;
                return {
                    render(model) { if (model && model.step) renderedSteps.push(model.step.id); },
                    destroy() { destroyedViews++; dismissMenuGuide = null; }
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
        setTimeout: callback => { saveTimers.set(++timerId, callback); return timerId; },
        clearTimeout: id => saveTimers.delete(id),
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
        flushSaveRetries() { clockNow += 2500; const callbacks = [...saveTimers.values()]; saveTimers.clear(); callbacks.forEach(callback => callback()); },
        calls, renderedSteps, storage, flow: window.SOW_menu_flow,
        guide: window.SOW_menu_guide,
        dismiss: () => { if (dismissMenuGuide) dismissMenuGuide(); },
        get pendingTimers() { return saveTimers.size; },
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
    context.window.matchMedia = () => ({ matches: false });
    context.window.SOW_menu_guide = guideHarness.guide;
    context.window.SOW_PORTAL = "site";
    context.console = { info(...args) { logs.push(args); }, error() {} };
    const rewardSource = shellSource.slice(progressionStart, progressionEnd)
        + shellSource.slice(presentationStart, presentationEnd)
        + shellSource.slice(presentationEnd, flowEnd);
    const animationShim = "function animateProgressionTo(target, duration, token, levelOverride) { var values = Object.assign({}, target); if (levelOverride != null) values.level = levelOverride; writeProgression(values); return Promise.resolve(true); }\n";
    vm.runInNewContext(rewardSource + animationShim +
        "this.menuRewardApi = { maybePresentRewards, menuGuideFacts, setState(value) { state = value; }, setRunning(value) { rewardAnimationRunning = value; }, setScreen(value) { screen = value; storeOpen = false; }, getScreen() { return screen; }, holdPresentation, releasePresentation, get phase() { return menuFlow.phase(); }, get shownReceipts() { return Array.from(rewardAnimationShown); }, get pendingReceipts() { return pendingRewardReceipts().map(function (receipt) { return receipt.id; }); }, get progression() { return displayedProgression; }, get facts() { return menuGuideFacts(); }, rewardStageSets, rewardStageRuns, rewardPulses, acknowledgements, logs };",
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
        menu_guide_save_pending: false
    };
    return { state: Object.assign(state, overrides), receipt };
}

const profileCss = fs.readFileSync(path.join(shell, "main_menu.profile.css"), "utf8");


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
    assert.deepEqual(guide.renderedSteps, [], "the real multiplayer return cannot revive an old guide");
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
    assert.equal(harness.renderedSteps.at(-1), "welcome_heroes");
    assert.equal(harness.calls.some(call => call.type === "save_menu_guide_progress"), false,
        "the one-step guide does not save intermediate progress or require a name change");
    shell.setScreen("heroes");
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(harness.destroyedViews, 1);
    assert.deepEqual(harness.calls.filter(call => call.type === "save_menu_guide_progress").map(call => call.status), ["completed"]);
    assert.equal(harness.renderedSteps.includes("welcome_complete"), false, "completion has no extra visible step");
});


test("requested store return finishes rewards without reviving a retired guide", async () => {
    const { state, receipt } = confirmedMenuReturn();
    state.menu_return.requested_screen = "store";
    state.menu_guides = { first_multiplayer: { status: "active", completed_steps: [] } };
    const guide = makeMenuGuideHarness();
    const shell = makeMenuShellRewardHarness(guide, state);
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.ok(shell.shownReceipts.includes(receipt.id));
    assert.equal(shell.getScreen(), "store");
    shell.setScreen("home");
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(guide.renderedSteps, []);
    assert.equal(guide.calls.some(call => call.type === "save_menu_guide_progress"), false);
});


test("participation receipt stays synced until match verification resolves", () => {
    const rejectStart = dataMainSource.indexOf("async fn reject_match_verification");
    const rejectEnd = dataMainSource.indexOf("async fn handle_bot_pool_seed", rejectStart);
    assert.ok(rejectStart >= 0 && rejectEnd > rejectStart);
    const engineLoaderSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/engine.rs"), "utf8");
});


test("return observation cannot issue a second exit command", () => {
    const dataExit = fs.readFileSync(path.join(shell, "../../sow-data/src/match_exit.rs"), "utf8");
    const observe = dataExit.slice(dataExit.indexOf("async fn handle_result"), dataExit.indexOf("pub(super) async fn worker"));
    assert.match(observe, /account_with_events\(&account_id, &\[&request\.match_id\]\)[\s\S]*terminal\(account\.profile\.reward_receipts\.get\(&request\.match_id\)\)/);
    assert.doesNotMatch(observe, /queue_match_work|dispatch_recovery_exit|internal\/match\/exit|MatchWork::/);
    const observeClient = accountSource.slice(accountSource.indexOf("fn fetch_match_result"), accountSource.indexOf("pub(crate) fn observe_relay_exit_recording"));
    assert.match(observeClient, /profile\/match-result/);
    assert.doesNotMatch(observeClient, /profile\/match-exit/);
    const loaderSync = shellSource.slice(shellSource.indexOf("    function syncWebLoaderForState("), shellSource.indexOf("    function scheduleRewardPresentation("));
    const ready = { phase: "MainMenu", loader_job: "ExitGame", loader_cycle_id: 4, loader_done: true };
    let delivered = null;
    vm.runInNewContext(loaderSync + "; syncWebLoaderForState(ready);", {
        ready, window: { SOW_syncWebLoader: value => { delivered = value; } }
    });
    assert.equal(delivered, ready, "menu images cannot hold a completed return loader");
});

test("missing maps stay retryable and turn batches wait for server acceptance", () => {
    const mapLoader = verificationSource.slice(verificationSource.indexOf("pub(super) fn match_map_bytes"), verificationSource.indexOf("pub(super) async fn register_map"));
    assert.match(mapLoader, /std::fs::metadata[\s\S]*?map_err[\s\S]*?ReplayVerificationError::MissingMap/);
    assert.match(mapLoader, /std::fs::read\(path\)[\s\S]*?map_err[\s\S]*?ReplayVerificationError::MissingMap/);
    const handleProgress = verificationSource.slice(verificationSource.indexOf("pub(super) async fn handle_progress"), verificationSource.indexOf("pub(super) async fn handle_result"));
    assert.match(handleProgress, /let invalid = matches!\(&error, ReplayVerificationError::Invalid\(_\)\)/);
    assert.match(handleProgress, /if invalid\s*\{\s*StatusCode::UNPROCESSABLE_ENTITY\s*\} else \{\s*StatusCode::SERVICE_UNAVAILABLE/);
    const dataExit = fs.readFileSync(path.join(shell, "../../sow-data/src/match_exit.rs"), "utf8");
    const permanentRejection = dataExit.slice(dataExit.indexOf("fn permanent_progress_rejection"), dataExit.indexOf("fn invalid_verified_result"));
    assert.match(permanentRejection, /reqwest::StatusCode::BAD_REQUEST \| reqwest::StatusCode::UNPROCESSABLE_ENTITY/);
    assert.doesNotMatch(permanentRejection, /SERVICE_UNAVAILABLE/);
    const relayProgress = fs.readFileSync(path.join(shell, "../../sow-relay/src/progress.rs"), "utf8");
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
});



test("lobby exit keeps the live connection and match exit owns teardown", () => {
    const lobbyActionStart = actionsSource.indexOf("UiAction::LeaveLobby =>");
    const matchActionStart = actionsSource.indexOf("UiAction::ReturnToMenu =>");
    assert.ok(lobbyActionStart >= 0 && matchActionStart > lobbyActionStart);
    const lobbyAction = actionsSource.slice(lobbyActionStart, matchActionStart);
    assert.match(lobbyAction, /leave_lobby_to_main_menu\(\)/);
    assert.doesNotMatch(lobbyAction, /begin_exit_to_main_menu|client = None|reconnect_now/);
    const matchAction = actionsSource.slice(matchActionStart);
    assert.match(matchAction, /begin_exit_to_main_menu\(\);/);
    assert.doesNotMatch(matchAction.slice(0, matchAction.indexOf("UiAction::SetAttackRatio")), /send_leave_message/);
});


test("random leader remains available regardless of legacy guide progress", () => {
    const start = heroesSource.indexOf("function randomLeaderName");
    const end = heroesSource.indexOf("function heroesRoster", start);
    const context = {
        state: { menu_guides: { welcome: { status: "active", current_step: "welcome_owned_selection" } } },
        heroesSearchQuery: "", heroesRegionFilter: "all", SOW_t: key => key,
        asset: path => "/assets/" + path, esc: value => String(value)
    };
    vm.runInNewContext(heroesSource.slice(start, end) + "this.renderRandomLeaderCard = renderRandomLeaderCard;", context);
    assert.match(context.renderRandomLeaderCard(true), /data-command='preview_random_leader'/);
    context.state.menu_guides.welcome.current_step = "welcome_offer_selection";
    assert.match(context.renderRandomLeaderCard(false), /data-command='preview_random_leader'/);
});

test("hero roster keeps available heroes first and preserves order after filtering", () => {
    const start = heroesSource.indexOf("function heroesRoster");
    const end = heroesSource.indexOf("function refreshHeroesRoster", start);
    assert.ok(start >= 0 && end > start);
    const context = {
        state: { leaders: [
            { id: "locked-first", name: "Locked first", available: false, region: "europe" },
            { id: "owned", name: "Owned", available: true, region: "europe" },
            { id: "locked-other-region", name: "Locked other", available: false, region: "americas" },
            { id: "weekly", name: "Weekly", available: true, region: "europe" },
            { id: "permanent", name: "Permanent", available: true, region: "europe" },
            { id: "locked-last", name: "Locked last", available: false, region: "europe" }
        ] },
        heroesSearchQuery: "",
        heroesRegionFilter: "all",
        leaderDisplayName: leader => leader.name,
        leaderHistoricalName: () => "",
        leaderCivilization: () => "",
        renderRandomLeaderCard: () => "Random|",
        renderHeroesCard: leader => leader.id + "|",
        esc: String,
        SOW_t: key => key
    };
    vm.runInNewContext(heroesSource.slice(start, end) + "this.renderHeroesRoster = renderHeroesRoster;", context);

    assert.equal(context.renderHeroesRoster("", false), "Random|owned|weekly|permanent|locked-first|locked-other-region|locked-last|");
    context.heroesRegionFilter = "europe";
    assert.equal(context.renderHeroesRoster("", false), "Random|owned|weekly|permanent|locked-first|locked-last|");
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
    // The info dialog is one tall card on every device; it is never swapped.
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








test("the menu catalog contains one visible Heroes step", () => {
    assert.deepEqual(campaignRuntime.validateMenuGuides(menuGuideDefinition).errors, []);
    assert.deepEqual(Object.keys(menuGuideDefinition.menu_guides), ["welcome"]);
    assert.deepEqual(menuGuideDefinition.menu_guides.welcome.step_ids, ["welcome_heroes", "welcome_complete"]);
    assert.equal(menuGuideDefinition.steps.filter(step => step.type !== "end").length, 1);
    assert.equal(menuGuideDefinition.strings.en["menu.guide.welcome_heroes.hint"],
        "Open Heroes to choose a commander or unlock one with gems.");
});

test("Skip stays pending through failure and reload until the server confirms it", async () => {
    const { state } = confirmedMenuReturn({ menu_return: undefined, reward_receipts: [] });
    const guide = makeMenuGuideHarness();
    const shell = makeMenuShellRewardHarness(guide, state);
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    guide.dismiss();
    assert.equal(guide.destroyedViews, 1, "Skip removes the guide immediately");
    assert.equal(guide.calls.at(-1).status, "skipped");
    assert.equal(guide.storage.size, 1);
    shell.setState({ ...state, menu_guide_save_pending: true });
    shell.maybePresentRewards();
    guide.flushSaveRetries();
    assert.equal(guide.calls.filter(call => call.type === "save_menu_guide_progress").length, 1);
    shell.setState({ ...state, phase: "Playing", menu_guide_save_pending: false });
    shell.maybePresentRewards();
    assert.equal(guide.calls.filter(call => call.type === "save_menu_guide_progress").length, 2,
        "a state update retries the pending save even after the view is gone and the player is in a match");
    guide.flushSaveRetries();
    assert.equal(guide.calls.filter(call => call.type === "save_menu_guide_progress").length, 3,
        "scheduled retries continue until the server confirms the save");

    const reloaded = makeMenuGuideHarness(guide.storage);
    const reloadedShell = makeMenuShellRewardHarness(reloaded, state);
    reloadedShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(reloaded.renderedSteps, []);
    assert.equal(reloaded.calls.at(-1).status, "skipped", "reload resends the pending server write");
    reloadedShell.setState({ ...state, menu_guides: { welcome: { status: "skipped", completed_steps: [] } } });
    reloadedShell.maybePresentRewards();
    assert.equal(reloaded.storage.size, 0, "only the server confirmation clears the pending local copy");
    assert.equal(reloaded.pendingTimers, 0);
    reloaded.flushSaveRetries();
    assert.equal(reloaded.calls.length, 1);

    reloadedShell.setState({ ...state, account_id: "account-b" });
    reloadedShell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(reloaded.renderedSteps.at(-1), "welcome_heroes", "Skip belongs to its account");
});

test("legacy progress resolves without replaying removed steps", async () => {
    const cases = [
        [{ status: "active", current_step: "welcome_name", completed_steps: [] }, 0, "welcome_heroes", null],
        [{ status: "active", current_step: "welcome_purchase_confirm", completed_steps: ["welcome_name", "welcome_heroes"] }, 0, null, "completed"],
        [{ status: "completed", completed_steps: ["welcome_complete"] }, 0, null, null],
        [{ status: "skipped", completed_steps: [] }, 0, null, null],
        [{ status: "active", current_step: "welcome_name", completed_steps: [] }, 1, null, "skipped"]
    ];
    for (const [record, matches, expectedStep, expectedSave] of cases) {
        const { state } = confirmedMenuReturn({ menu_return: undefined, reward_receipts: [],
            menu_guides: { welcome: record, first_multiplayer: { status: "active", completed_steps: [] } },
            profile_stats: { matches_played: matches } });
        const guide = makeMenuGuideHarness();
        makeMenuShellRewardHarness(guide, state).maybePresentRewards();
        await new Promise(resolve => setImmediate(resolve));
        assert.deepEqual(guide.renderedSteps, expectedStep ? [expectedStep] : []);
        const saves = guide.calls.filter(call => call.type === "save_menu_guide_progress");
        assert.deepEqual(saves.map(call => call.status), expectedSave ? [expectedSave] : []);
        assert.ok(saves.every(call => call.guide_id === "welcome" &&
            call.completed_steps.every(id => menuGuideDefinition.menu_guides.welcome.step_ids.includes(id))));
    }
});

test("starting a match directly closes the welcome without another visible step", async () => {
    const { state } = confirmedMenuReturn({ menu_return: undefined, reward_receipts: [], quick_match_revision: 0 });
    const guide = makeMenuGuideHarness();
    const shell = makeMenuShellRewardHarness(guide, state);
    shell.maybePresentRewards();
    await new Promise(resolve => setImmediate(resolve));
    shell.setState({ ...state, quick_match_revision: 1 });
    shell.maybePresentRewards();
    assert.equal(guide.destroyedViews, 1);
    assert.equal(guide.calls.at(-1).status, "skipped");
    assert.deepEqual(guide.renderedSteps, ["welcome_heroes"]);
});
