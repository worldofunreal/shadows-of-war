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


test("match rewards present in the main menu, while endgame keeps only match stats", () => {
    assert.match(hud, /id="sow-hud-endgame-modal"/);
    assert.match(hud, /endgame\.classList\.toggle\("hidden", !isOver\)/);
    assert.match(webMenu, /endgame_active,[\s\S]*endgame_winner:[\s\S]*endgame_team:/);
    assert.match(hud, /id="sow-hud-surrender-kda"/);
    assert.doesNotMatch(hud, /sow-hud-endgame-(xp|leader-xp|crowns|laurels|rewards)/);
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
    assert.doesNotMatch(webMenu, /hud\.rewards/);
    assert.match(coreSource, /receipt\.leader_xp/);
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


test("participation receipt stays synced until match verification resolves", () => {
    assert.match(dataDbSource, /pub async fn mark_match_reward_verification_status/);
    assert.match(dataDbSource, /receipt\.verification_status = Some\(ReplayVerificationStatus::Verified\)/);
    const rejectStart = dataMainSource.indexOf("async fn reject_match_verification");
    const rejectEnd = dataMainSource.indexOf("async fn handle_bot_pool_seed", rejectStart);
    assert.ok(rejectStart >= 0 && rejectEnd > rejectStart);
    assert.match(dataMainSource.slice(rejectStart, rejectEnd), /mark_match_reward_verification_status[\s\S]*ReplayVerificationStatus::Rejected[\s\S]*finish_match_verification/);
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


test("return observation cannot issue a second exit command", () => {
    const dataExit = fs.readFileSync(path.join(shell, "../../sow-data/src/match_exit.rs"), "utf8");
    const observe = dataExit.slice(dataExit.indexOf("async fn handle_result"), dataExit.indexOf("pub(super) async fn worker"));
    assert.match(observe, /account_with_events\(&account_id, &\[&request\.match_id\]\)[\s\S]*terminal\(account\.profile\.reward_receipts\.get\(&request\.match_id\)\)/);
    assert.doesNotMatch(observe, /queue_match_work|dispatch_recovery_exit|internal\/match\/exit|MatchWork::/);
    assert.doesNotMatch(dataMainSource, /\/profile\/match-exit|match_exit::handle_exit/);
    assert.doesNotMatch(serverSource, /\/internal\/match\/exit|handle_match_exit|verify_match_participant_replay_handler/);
    assert.doesNotMatch(relaySource, /\/internal\/match\/exit|RelayEvent::ExitRequested/);
    const observeClient = accountSource.slice(accountSource.indexOf("fn fetch_match_result"), accountSource.indexOf("pub(crate) fn observe_relay_exit_recording"));
    assert.match(observeClient, /profile\/match-result/);
    assert.doesNotMatch(observeClient, /profile\/match-exit/);
    assert.match(accountSource, /context\.exit_recorded \|\| context\.voluntary_exit\.unwrap_or\(true\)/);
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
    assert.doesNotMatch(relaySource, /active_empty_secs/);
});

test("missing maps stay retryable and turn batches wait for server acceptance", () => {
    assert.match(serverSource, /MissingMap\(String\),/);
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
    assert.match(relayProgress, /async fn deliver_until_accepted/);
    assert.match(relayProgress, /server turn delivery pending/);
    assert.match(relayProgress, /response\.status\(\)\.is_client_error\(\)/);
    assert.doesNotMatch(relaySource, /ReplayJournal|recover_detached_exit|post_participant_settle/);
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


test("lobby cards stay accurate and portal invites target the selected lobby", () => {
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

    assert.match(portalsSource, /function parseInviteLobbyId\(inviteParams\)[\s\S]*var raw = inviteParams\.lobbyId/);
    assert.match(portalsSource, /game\.addJoinRoomListener\(function \(inviteParams\)[\s\S]*queueInviteLobbyId\(lobbyId\)/);
    assert.match(netUpdateSource, /pending_join_lobby_id[\s\S]*make_join_message\(Some\(id\), false, None, None\)/);
    assert.match(sessionSource, /target_lobby_id,[\s\S]*ClientMessage::JoinWithAuth/);
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
