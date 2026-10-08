const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const source = fs.readFileSync(path.join(__dirname, "experience_events.js"), "utf8");
const jestBridgeSource = fs.readFileSync(path.join(__dirname, "sdk/jest_portals.js"), "utf8");
const siteAnalyticsSource = fs.readFileSync(path.join(__dirname, "../site/analytics.js"), "utf8");
const backendEventsSource = fs.readFileSync(path.join(__dirname, "../../sow-data/src/events.rs"), "utf8");

function eventNames(source, pattern) {
  const body = source.match(pattern)?.[1];
  assert.ok(body, "event allowlist is missing");
  return [...body.matchAll(/"([a-z][a-z0-9_]*)"/g)].map(match => match[1]).sort();
}

function createRuntime(portal) {
  const calls = { analytics: [], poki: [], jest: [] };
  const window = {
    SOW_PORTAL: portal,
    SOW_analyticsTrack: (...args) => calls.analytics.push(args),
    SOW_pokiMeasure: (...args) => calls.poki.push(args),
    SOW_portalCaptureExperienceEvent: event => calls.jest.push(event)
  };
  vm.runInNewContext(source, { window, JSON, Set, Object, Number, String });
  return { window, calls };
}

test("one canonical event reaches only the owned-site analytics adapter", () => {
  const { window, calls } = createRuntime(undefined);
  window.SOW_trackExperienceEvent({ name: "tutorial_step", props: {
    episode_id: "boudica", step_id: "claim_wilderness", step_index: 2, action: "start"
  } });
  assert.equal(calls.analytics.length, 1);
  assert.deepEqual(JSON.parse(JSON.stringify(calls.analytics[0])), ["tutorial_step", {
    episode_id: "boudica", step_id: "claim_wilderness", step_index: 2, action: "start"
  }]);
  assert.equal(calls.poki.length + calls.jest.length, 0);
});

test("shared, first-party, and server allowlists cannot silently drift", () => {
  const shared = eventNames(source, /var EVENTS = new Set\(\[([\s\S]*?)\]\);/);
  const site = eventNames(siteAnalyticsSource, /const KNOWN_EVENTS = new Set\(\[([\s\S]*?)\]\);/);
  const server = eventNames(backendEventsSource, /pub const EVENT_NAMES: &\[&str\] = &\[([\s\S]*?)\];/);
  assert.deepEqual(shared, site);
  assert.deepEqual(shared, server);
});

test("Poki receives step-specific progress events without first-party forwarding", () => {
  const { window, calls } = createRuntime("poki");
  window.SOW_trackExperienceEvent({ name: "tutorial_step", props: {
    episode_id: "boudica", step_id: "claim_wilderness", step_index: 2, action: "complete"
  } });
  assert.deepEqual(calls.poki, [["tutorial", "boudica_claim_wilderness", "complete"]]);
  assert.equal(calls.analytics.length, 0);
});

test("an explicit tutorial exit maps one active-step failure to each supported destination", () => {
  for (const portal of [undefined, "poki", "jest"]) {
    const { window, calls } = createRuntime(portal);
    window.SOW_tutorial_exit_context = episodeId => episodeId === "boudica"
      ? { step_id: "claim_wilderness", step_index: 2, action: "fail" }
      : null;
    window.SOW_trackExperienceEvent({ name: "tutorial_exit_early", props: { episode_id: "boudica" } });

    if (portal === "poki") {
      assert.deepEqual(calls.poki, [["tutorial", "boudica_claim_wilderness", "fail"]]);
    } else if (portal === "jest") {
      assert.deepEqual(JSON.parse(JSON.stringify(calls.jest)), [{
        name: "tutorial_exit_early",
        props: { episode_id: "boudica", step_id: "claim_wilderness", step_index: 2, action: "fail" }
      }]);
    } else {
      assert.deepEqual(JSON.parse(JSON.stringify(calls.analytics)), [["tutorial_exit_early", {
        episode_id: "boudica", step_id: "claim_wilderness", step_index: 2, action: "fail"
      }]]);
    }
    assert.equal(calls.analytics.length + calls.poki.length + calls.jest.length, 1);
  }
});

test("completed tutorials do not become exits and queued episode-only exits remain compatible", () => {
  const completed = createRuntime("poki");
  completed.window.SOW_tutorial_exit_context = () => false;
  completed.window.SOW_trackExperienceEvent({ name: "tutorial_exit_early", props: { episode_id: "boudica" } });
  assert.deepEqual(completed.calls.poki, []);

  const legacy = createRuntime("poki");
  legacy.window.SOW_trackExperienceEvent({ name: "tutorial_exit_early", props: { episode_id: "boudica" } });
  assert.deepEqual(legacy.calls.poki, [["tutorial", "boudica", "exit"]]);
});

test("Jest receives the canonical event and bounded properties exactly once", () => {
  const { window, calls } = createRuntime("jest");
  window.SOW_trackExperienceEvent({ name: "campaign_episode_complete", props: { episode_id: "boudica" } });
  assert.deepEqual(JSON.parse(JSON.stringify(calls.jest)), [{
    name: "campaign_episode_complete", props: { episode_id: "boudica" }
  }]);
  assert.equal(calls.analytics.length + calls.poki.length, 0);
});

test("the shared funnel sequence is sent once to each SDK that supports custom events", () => {
  const sequence = [
    { name: "load_stage", props: { stage: "snapshot_available" } },
    { name: "tutorial_start", props: { episode_id: "boudica" } },
    { name: "tutorial_step", props: { episode_id: "boudica", step_id: "opening", step_index: 0, action: "start" } },
    { name: "tutorial_step", props: { episode_id: "boudica", step_id: "opening", step_index: 0, action: "complete" } },
    { name: "campaign_episode_complete", props: { episode_id: "boudica" } },
    { name: "matchmaking_joined" },
    { name: "lobby_joined" },
    { name: "match_loading_start" },
    { name: "match_started_client" },
    { name: "match_ended_client" }
  ];
  for (const portal of [undefined, "poki", "jest"]) {
    const { window, calls } = createRuntime(portal);
    for (const event of sequence) window.SOW_trackExperienceEvent(event);
    const delivered = portal === "poki" ? calls.poki.length
      : portal === "jest" ? calls.jest.length : calls.analytics.length;
    assert.equal(delivered, sequence.length);
    assert.equal(calls.analytics.length + calls.poki.length + calls.jest.length, sequence.length);
  }
});

test("CrazyGames does not receive unsupported custom events", () => {
  const { window, calls } = createRuntime("crazygames");
  let completionReports = 0;
  window.CrazyGames = { SDK: { game: {
    reportGameCompletedPercentage: () => { completionReports += 1; }
  } } };
  window.SOW_trackExperienceEvent({ name: "tutorial_step", props: {
    episode_id: "boudica", step_id: "claim_wilderness", step_index: 2, action: "start"
  } });
  window.SOW_trackExperienceEvent({ name: "campaign_episode_complete", props: { episode_id: "boudica" } });
  assert.equal(calls.analytics.length + calls.poki.length + calls.jest.length, 0);
  assert.equal(completionReports, 0, "a campaign episode must not masquerade as whole-game completion");
});

test("unknown names are rejected and invalid tutorial steps use only the safe legacy fallback", () => {
  const { window, calls } = createRuntime("poki");
  window.SOW_trackExperienceEvent({ name: "not_registered" });
  window.SOW_trackExperienceEvent({ name: "tutorial_step", props: {
    episode_id: "boudica", step_id: "step|other", step_index: 2, action: "start"
  } });
  assert.deepEqual(calls.poki, [["tutorial", "legacy_step_unknown", "start"]]);
  assert.deepEqual(calls.analytics, []);
});

test("loading stages and failure outcomes share stable portal mappings", () => {
  const { window, calls } = createRuntime("poki");
  window.SOW_trackExperienceEvent({ name: "load_stage", props: { stage: "relay_connect_start" } });
  window.SOW_trackExperienceEvent({ name: "load_stage", props: { stage: "relay_connect_complete" } });
  window.SOW_trackExperienceEvent({ name: "load_stage", props: { stage: "engine_init_complete" } });
  window.SOW_trackExperienceEvent({ name: "lobby_join_failed" });
  assert.deepEqual(calls.poki, [
    ["loading", "relay_connect", "start"],
    ["loading", "relay_connect", "complete"],
    ["loading", "engine_init", "complete"],
    ["lobby", "join", "fail"]
  ]);
});

test("Jest SDK adapter captures the same event and safely ignores a missing SDK", () => {
  const captured = [];
  const window = {
    JestSDK: { captureEvent: (name, props) => captured.push([name, props]) },
    localStorage: { getItem: () => null }
  };
  vm.runInNewContext(jestBridgeSource, { window, console });
  window.SOW_portalCaptureExperienceEvent({ name: "tutorial_step", props: {
    episode_id: "boudica", step_id: "claim_wilderness", step_index: 2, action: "start"
  } });
  assert.equal(captured.length, 1);
  assert.equal(captured[0][0], "tutorial_step");
  assert.equal(captured[0][1].step_id, "claim_wilderness");

  window.JestSDK = null;
  assert.doesNotThrow(() => window.SOW_portalCaptureExperienceEvent({ name: "shell_loaded" }));
});
