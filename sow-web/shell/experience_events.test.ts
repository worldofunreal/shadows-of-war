import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { vm } from "./typescript-test-support.ts";

const source = fs.readFileSync(path.join(__dirname, "experience_events.ts"), "utf8");
const jestBridgeSource = fs.readFileSync(path.join(__dirname, "sdk/jest_portals.ts"), "utf8");
const pokiBridgeSource = fs.readFileSync(path.join(__dirname, "sdk/poki_portals.ts"), "utf8");
const siteAnalyticsSource = fs.readFileSync(path.join(__dirname, "../site/analytics.ts"), "utf8");
const backendEventsSource = fs.readFileSync(path.join(__dirname, "../../sow-data/src/events.rs"), "utf8");

function eventNames(source, pattern) {
    const body = source.match(pattern)?.[1];
    assert.ok(body, "event allowlist is missing");
    return [...body.matchAll(/"([a-z][a-z0-9_]*)"/g)].map((match) => match[1]).sort();
}

function createRuntime(portal) {
    const calls = { analytics: [], poki: [], jest: [] };
    const window = {
        SOW_PORTAL: portal,
        SOW_analyticsTrack: (...args) => calls.analytics.push(args),
        SOW_pokiMeasure: (...args) => calls.poki.push(args),
        SOW_portalCaptureExperienceEvent: (event) => calls.jest.push(event),
    };
    vm.runInNewContext(source, { window, JSON, Set, Object, Number, String });
    return { window, calls };
}

test("one canonical event reaches only the owned-site analytics adapter", () => {
    const { window, calls } = createRuntime(undefined);
    window.SOW_trackExperienceEvent({
        name: "tutorial_step",
        props: {
            episode_id: "boudica",
            step_id: "claim_wilderness",
            step_index: 2,
            action: "start",
        },
    });
    assert.equal(calls.analytics.length, 1);
    assert.deepEqual(JSON.parse(JSON.stringify(calls.analytics[0])), [
        "tutorial_step",
        {
            episode_id: "boudica",
            step_id: "claim_wilderness",
            step_index: 2,
            action: "start",
        },
    ]);
    assert.equal(calls.poki.length + calls.jest.length, 0);
});

test("shared, first-party, and server allowlists cannot silently drift", () => {
    const shared = eventNames(source, /const EVENTS = new Set\(\[([\s\S]*?)\]\);/);
    const site = eventNames(siteAnalyticsSource, /const KNOWN_EVENTS = new Set\(\[([\s\S]*?)\]\);/);
    const server = eventNames(backendEventsSource, /pub const EVENT_NAMES: &\[&str\] = &\[([\s\S]*?)\];/);
    assert.deepEqual(shared, site);
    assert.deepEqual(shared, server);
});

test("Poki receives step-specific progress events without first-party forwarding", () => {
    const { window, calls } = createRuntime("poki");
    window.SOW_trackExperienceEvent({
        name: "tutorial_step",
        props: {
            episode_id: "boudica",
            step_id: "claim_wilderness",
            step_index: 2,
            action: "complete",
        },
    });
    assert.deepEqual(calls.poki, [["tutorial", "boudica_claim_wilderness", "complete"]]);
    assert.equal(calls.analytics.length, 0);
});

test("an explicit tutorial exit maps one active-step failure to each supported destination", () => {
    for (const portal of [undefined, "poki", "jest"]) {
        const { window, calls } = createRuntime(portal);
        window.SOW_tutorial_exit_context = (episodeId) =>
            episodeId === "boudica" ? { step_id: "claim_wilderness", step_index: 2, action: "fail" } : null;
        window.SOW_trackExperienceEvent({ name: "tutorial_exit_early", props: { episode_id: "boudica" } });

        if (portal === "poki") {
            assert.deepEqual(calls.poki, [["tutorial", "boudica_claim_wilderness", "fail"]]);
        } else if (portal === "jest") {
            assert.deepEqual(JSON.parse(JSON.stringify(calls.jest)), [
                {
                    name: "tutorial_exit_early",
                    props: {
                        episode_id: "boudica",
                        step_id: "claim_wilderness",
                        step_index: 2,
                        action: "fail",
                    },
                },
            ]);
        } else {
            assert.deepEqual(JSON.parse(JSON.stringify(calls.analytics)), [
                [
                    "tutorial_exit_early",
                    {
                        episode_id: "boudica",
                        step_id: "claim_wilderness",
                        step_index: 2,
                        action: "fail",
                    },
                ],
            ]);
        }
        assert.equal(calls.analytics.length + calls.poki.length + calls.jest.length, 1);
    }
});

test("completed tutorials do not become exits and queued episode-only exits remain compatible", () => {
    const completed = createRuntime("poki");
    completed.window.SOW_tutorial_exit_context = () => false;
    completed.window.SOW_trackExperienceEvent({
        name: "tutorial_exit_early",
        props: { episode_id: "boudica" },
    });
    assert.deepEqual(completed.calls.poki, []);

    const legacy = createRuntime("poki");
    legacy.window.SOW_trackExperienceEvent({ name: "tutorial_exit_early", props: { episode_id: "boudica" } });
    assert.deepEqual(legacy.calls.poki, [["tutorial", "boudica", "exit"]]);
});

test("Jest receives the canonical event and bounded properties exactly once", () => {
    const { window, calls } = createRuntime("jest");
    window.SOW_trackExperienceEvent({ name: "campaign_episode_complete", props: { episode_id: "boudica" } });
    assert.deepEqual(JSON.parse(JSON.stringify(calls.jest)), [
        {
            name: "campaign_episode_complete",
            props: { episode_id: "boudica" },
        },
    ]);
    assert.equal(calls.analytics.length + calls.poki.length, 0);
});

test("the shared funnel sequence is sent once to each SDK that supports custom events", () => {
    const sequence = [
        { name: "load_stage", props: { stage: "snapshot_available" } },
        { name: "tutorial_start", props: { episode_id: "boudica" } },
        {
            name: "tutorial_step",
            props: { episode_id: "boudica", step_id: "opening", step_index: 0, action: "start" },
        },
        {
            name: "tutorial_step",
            props: { episode_id: "boudica", step_id: "opening", step_index: 0, action: "complete" },
        },
        { name: "campaign_episode_complete", props: { episode_id: "boudica" } },
        { name: "matchmaking_joined", props: { mode: "ranked_1v1", wait_bucket: "5_15s" } },
        { name: "lobby_joined", props: { mode: "ranked_1v1", wait_bucket: "5_15s" } },
        { name: "match_loading_start" },
        { name: "match_started_client", props: { mode: "ranked_1v1" } },
        {
            name: "match_ended_client",
            props: { mode: "ranked_1v1", outcome: "win", duration_bucket: "5_15m" },
        },
    ];
    for (const portal of [undefined, "poki", "jest"]) {
        const { window, calls } = createRuntime(portal);
        for (const event of sequence) window.SOW_trackExperienceEvent(event);
        const delivered =
            portal === "poki"
                ? calls.poki.length
                : portal === "jest"
                  ? calls.jest.length
                  : calls.analytics.length;
        assert.equal(delivered, sequence.length);
        assert.equal(calls.analytics.length + calls.poki.length + calls.jest.length, sequence.length);
    }
});

test("CrazyGames does not receive unsupported custom events", () => {
    const { window, calls } = createRuntime("crazygames");
    let completionReports = 0;
    window.CrazyGames = {
        SDK: {
            game: {
                reportGameCompletedPercentage: () => {
                    completionReports += 1;
                },
            },
        },
    };
    window.SOW_trackExperienceEvent({
        name: "tutorial_step",
        props: {
            episode_id: "boudica",
            step_id: "claim_wilderness",
            step_index: 2,
            action: "start",
        },
    });
    window.SOW_trackExperienceEvent({ name: "campaign_episode_complete", props: { episode_id: "boudica" } });
    assert.equal(calls.analytics.length + calls.poki.length + calls.jest.length, 0);
    assert.equal(completionReports, 0, "a campaign episode must not masquerade as whole-game completion");
});

test("unknown names are rejected and invalid tutorial steps use only the safe legacy fallback", () => {
    const { window, calls } = createRuntime("poki");
    window.SOW_trackExperienceEvent({ name: "not_registered" });
    window.SOW_trackExperienceEvent({
        name: "tutorial_step",
        props: {
            episode_id: "boudica",
            step_id: "step|other",
            step_index: 2,
            action: "start",
        },
    });
    assert.deepEqual(calls.poki, [["tutorial", "legacy_step_unknown", "start"]]);
    assert.deepEqual(calls.analytics, []);
});

test("loading stages and failure outcomes share stable portal mappings", () => {
    const { window, calls } = createRuntime("poki");
    window.SOW_trackExperienceEvent({ name: "load_stage", props: { stage: "relay_connect_start" } });
    window.SOW_trackExperienceEvent({ name: "load_stage", props: { stage: "relay_connect_complete" } });
    window.SOW_trackExperienceEvent({ name: "load_stage", props: { stage: "engine_init_complete" } });
    window.SOW_trackExperienceEvent({
        name: "lobby_join_failed",
        props: { mode: "other", wait_bucket: "unknown", reason_category: "lobby_unavailable" },
    });
    assert.deepEqual(calls.poki, [
        ["loading", "relay_connect", "start"],
        ["loading", "relay_connect", "complete"],
        ["loading", "engine_init", "complete"],
        ["lobby", "join", "fail"],
    ]);
});

test("match funnel accepts only closed mode, outcome, wait, duration and failure categories", () => {
    const { window, calls } = createRuntime("poki");
    window.SOW_trackExperienceEvent({
        name: "match_ended_client",
        props: { mode: "ranked_1v1", outcome: "win", duration_bucket: "5_15m" },
    });
    window.SOW_trackExperienceEvent({
        name: "lobby_join_failed",
        props: {
            mode: "custom",
            wait_bucket: "15_60s",
            reason_category: "wrong_password",
        },
    });
    window.SOW_trackExperienceEvent({
        name: "match_ended_client",
        props: {
            mode: "ranked_1v1",
            outcome: "win",
            duration_bucket: "5_15m",
            player_id: 42,
        },
    });
    assert.deepEqual(calls.poki, [
        ["match", "round", "win"],
        ["lobby", "join", "fail"],
    ]);
});

test("match events require their bounded dimensions and reject identity fields", () => {
    const { window, calls } = createRuntime("jest");
    window.SOW_trackExperienceEvent({ name: "match_started_client" });
    window.SOW_trackExperienceEvent({
        name: "match_started_client",
        props: { mode: "ranked_1v1", account_id: "0123456789abcdef0123456789abcdef" },
    });
    window.SOW_trackExperienceEvent({
        name: "match_ended_client",
        props: { mode: "ranked_1v1", outcome: "win", duration_bucket: "5_15m", player_id: 44 },
    });
    assert.equal(calls.jest.length, 0);
});

test("Jest SDK adapter captures the same event and safely ignores a missing SDK", () => {
    const captured = [];
    const window = {
        JestSDK: { captureEvent: (name, props) => captured.push([name, props]) },
        localStorage: { getItem: () => null },
    };
    vm.runInNewContext(jestBridgeSource, { window, console });
    window.SOW_portalCaptureExperienceEvent({
        name: "tutorial_step",
        props: {
            episode_id: "boudica",
            step_id: "claim_wilderness",
            step_index: 2,
            action: "start",
        },
    });
    assert.equal(captured.length, 1);
    assert.equal(captured[0][0], "tutorial_step");
    assert.equal(captured[0][1].step_id, "claim_wilderness");

    window.JestSDK = null;
    assert.doesNotThrow(() => window.SOW_portalCaptureExperienceEvent({ name: "shell_loaded" }));
});

test("Jest captures only the official entry-source type and marks one first milestone", async () => {
    const captured = [];
    let milestones = 0;
    const window = {
        JestSDK: {
            init: () => Promise.resolve(),
            captureEvent: (name, props) => captured.push([name, props]),
            getEntrySource: () => ({ type: "jest_ad", campaign: { id: "private", name: "private" } }),
            markFirstMilestone: () => milestones++,
        },
        localStorage: { getItem: () => null, setItem: () => {} },
    };
    vm.runInNewContext(jestBridgeSource, { window, console, Promise, Set, Object });
    await window.SOW_initPortalSdk();
    window.SOW_portalCaptureExperienceEvent({ name: "match_started_client", props: { mode: "ranked_1v1" } });
    window.SOW_portalCaptureExperienceEvent({
        name: "match_ended_client",
        props: { mode: "ranked_1v1", outcome: "win", duration_bucket: "5_15m" },
    });
    window.SOW_portalCaptureExperienceEvent({
        name: "match_ended_client",
        props: { mode: "ranked_1v1", outcome: "loss", duration_bucket: "1_5m" },
    });
    assert.equal(captured.length, 3);
    assert.equal(captured[0][1].entry_source, "jest_ad");
    assert.equal("campaign" in captured[0][1], false);
    assert.equal(milestones, 1);
});

test("Jest guest progress is keyed by stable playerId and legacy progress migrates once", async () => {
    const values = new Map([["sow_player_progress", "legacy-progress"]]);
    const localStorage = {
        getItem: (key) => values.get(key) || null,
        setItem: (key, value) => values.set(key, value),
        removeItem: (key) => values.delete(key),
    };
    const makeWindow = (playerId) => ({
        JestSDK: {
            init: () => Promise.resolve(),
            getPlayer: () => ({ playerId, registered: false }),
        },
        localStorage,
    });
    const initialize = async (playerId) => {
        const window = makeWindow(playerId);
        vm.runInNewContext(jestBridgeSource, { window, console, Promise, Set, Object });
        await window.SOW_initPortalSdk();
        return window;
    };

    const guestA = await initialize("guest/a");
    assert.equal(guestA.SOW_PORTAL_PROGRESS_JSON, "legacy-progress");
    guestA.SOW_portalSaveProgress("guest-a-progress");
    assert.equal(values.get("sow_player_progress:jest:guest%2Fa"), "guest-a-progress");
    assert.equal(values.has("sow_player_progress"), false);

    const guestB = await initialize("guest-b");
    assert.equal(guestB.SOW_PORTAL_PROGRESS_JSON, null);
    guestB.SOW_portalSaveProgress("guest-b-progress");
    assert.equal(values.get("sow_player_progress:jest:guest-b"), "guest-b-progress");
    assert.equal((await initialize("guest/a")).SOW_PORTAL_PROGRESS_JSON, "guest-a-progress");
});

test("Jest never prompts a registered player to log in again", async () => {
    let loginCalls = 0;
    let identityChanges = 0;
    const player = { playerId: "guest", registered: false };
    const window = {
        JestSDK: {
            init: () => Promise.resolve(),
            getPlayer: () => player,
            login: () => {
                loginCalls++;
                player.playerId = "registered-player";
                player.registered = true;
                return Promise.resolve();
            },
        },
        localStorage: { getItem: () => null, setItem: () => {}, removeItem: () => {} },
        Event: function (type) {
            this.type = type;
        },
        dispatchEvent: (event) => {
            if (event.type === "sow:platform-identity-changed") identityChanges++;
        },
    };
    vm.runInNewContext(jestBridgeSource, { window, console, Promise, Set, Object });
    await window.SOW_initPortalSdk();
    await window.SOW_jestLogin();
    await window.SOW_jestLogin();
    assert.equal(loginCalls, 1);
    assert.equal(identityChanges, 1);
    assert.equal(window.SOW_AUTH_CHANGED, true);
    window.SOW_portalClearAuthChanged();
    assert.equal(window.SOW_AUTH_CHANGED, false);
});

test("Jest schedules the recommended same-day and D1-D7 sequence only for registered players", async () => {
    const scheduled = [];
    const window = {
        JestSDK: {
            init: () => Promise.resolve(),
            getPlayer: () => ({ registered: true }),
            notifications: { scheduleNotification: (item) => scheduled.push(item) },
        },
        localStorage: { getItem: () => null, setItem: () => {} },
    };
    vm.runInNewContext(jestBridgeSource, { window, console, Promise, Set, Object });
    await window.SOW_initPortalSdk();
    assert.deepEqual(scheduled.map((item) => item.scheduledInDays), [0, 1, 2, 3, 4, 5, 6, 7]);
    assert.equal(scheduled.every((item) => item.body.length <= 100), true);
    assert.equal(scheduled.every((item) => item.ctaText === "Play now"), true);
    assert.equal(scheduled.some((item) => /daily reward|win rate|tournament/i.test(item.body)), false);
});

test("Poki suppresses measure calls while a commercial break is active", async () => {
    let measures = 0;
    const document = { addEventListener: () => {} };
    const window = {
        PokiSDK: { init: () => Promise.resolve(), measure: () => measures++ },
        localStorage: { getItem: () => null },
    };
    vm.runInNewContext(pokiBridgeSource, { window, document, console, Promise, Set, String });
    await window.SOW_initPortalSdk();
    window.SOW_adPlaying = true;
    window.SOW_pokiMeasure("match", "round", "win");
    assert.equal(measures, 0);
    window.SOW_adPlaying = false;
    window.SOW_pokiMeasure("match", "round", "win");
    assert.equal(measures, 1);
});

test("Poki closes and resumes gameplay events when a playable tab is interrupted", async () => {
    const listeners = new Map();
    const counts = { starts: 0, stops: 0 };
    const document = {
        hidden: false,
        addEventListener: (type, callback) => {
            const callbacks = listeners.get(type) || [];
            callbacks.push(callback);
            listeners.set(type, callbacks);
        },
    };
    const window = {
        PokiSDK: {
            init: () => Promise.resolve(),
            gameplayStart: () => counts.starts++,
            gameplayStop: () => counts.stops++,
        },
        localStorage: { getItem: () => null },
    };
    vm.runInNewContext(pokiBridgeSource, { window, document, console, Promise, Set, String });
    await window.SOW_initPortalSdk();
    window.SOW_portalGameplayStart();
    listeners.get("pointerdown")[0]();
    assert.equal(counts.starts, 1);

    document.hidden = true;
    listeners.get("visibilitychange")[0]();
    assert.equal(counts.stops, 1);

    document.hidden = false;
    listeners.get("visibilitychange")[0]();
    assert.equal(counts.starts, 2);

    window.SOW_portalGameplayStop();
    assert.equal(counts.stops, 2);
});
