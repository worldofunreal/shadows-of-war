const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { webcrypto } = require("node:crypto");

const source = fs.readFileSync(path.join(__dirname, "analytics.js"), "utf8");
const controlsCss = fs.readFileSync(path.join(__dirname, "../shell/sow-controls.css"), "utf8");

function storage() {
  const values = new Map();
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, String(value)),
    removeItem: (key) => values.delete(key)
  };
}

function element(tag) {
  const listeners = new Map();
  return {
    tagName: tag,
    children: [],
    dataset: {},
    style: {},
    append(...items) { this.children.push(...items); },
    addEventListener(name, handler) {
      if (!listeners.has(name)) listeners.set(name, []);
      listeners.get(name).push(handler);
    },
    attributes: {},
    setAttribute(name, value) { this.attributes[name] = value; },
    remove() { this.removed = true; },
    querySelector() { return null; },
    click() { for (const handler of listeners.get("click") || []) handler({ preventDefault() {} }); }
  };
}

function start({ fetchImpl = async () => ({ ok: true }), pathname = "/", gameShell = false, choice = null } = {}) {
  const local = storage();
  const session = storage();
  if (choice) local.setItem("sow_analytics_consent_v2", JSON.stringify({ choice, policy: "2026-10-06" }));
  const fetchCalls = [];
  const domListeners = new Map();
  const windowListeners = new Map();
  let intervalCallback = null;
  const head = element("head");
  const body = element("body");
  const document = {
    readyState: "loading",
    referrer: "",
    documentElement: { lang: "es", dataset: {} },
    head,
    body,
    createElement: element,
    getElementById(id) {
      if (id === "blade" && gameShell) return {};
      return head.children.find((node) => node.id === id) || null;
    },
    addEventListener(name, handler) { domListeners.set(name, handler); },
    querySelector() { return null; }
  };
  const window = {
    location: { hostname: "shadowsofwar.io", pathname, search: "" },
    addEventListener(name, handler) { windowListeners.set(name, handler); },
    setInterval(callback) { intervalCallback = callback; return 1; },
    clearInterval() { intervalCallback = null; },
    SOW_t(key) {
      return ({
        "site.analytics_title": "Analítica opcional",
        "site.analytics_body": "Usamos la actividad del sitio y del juego para mejorar Shadows of War. Solo se activa si la permites.",
        "site.analytics_policy": "Detalles de cookies",
        "site.analytics_accept": "Permitir",
        "site.analytics_reject": "Rechazar"
      })[key] || key;
    }
  };
  const context = {
    window,
    document,
    navigator: { language: "es", userAgent: "test" },
    localStorage: local,
    sessionStorage: session,
    crypto: webcrypto,
    AbortController,
    URLSearchParams,
    fetch(url, options) {
      fetchCalls.push({ url, options });
      return fetchImpl(url, options);
    }
  };

  vm.runInNewContext(source, context, { filename: "analytics.js" });
  domListeners.get("DOMContentLoaded")();
  const card = body.children.find((node) => node.className.includes("sow-analytics-consent"));
  return { card, body, head, local, session, fetchCalls, window, flushAgain: () => intervalCallback() };
}

test("consent panel loads and rejecting sends nothing or creates an analytics session", () => {
  const runtime = start();
  assert.match(controlsCss, /\.sow-analytics-consent \{/);
  assert.equal(runtime.head.children.some((node) => node.id === "sow-analytics-consent-style"), false);
  assert.ok(runtime.card, "website presents the consent choice");
  assert.equal(runtime.card.hidden, false, "the choice stays visible until an explicit answer");
  assert.notEqual(runtime.body.style.overflow, "hidden", "the page remains scrollable behind the banner");
  assert.equal(runtime.card.children[0].textContent, "Analítica opcional");
  assert.equal(runtime.card.children[1].textContent, "Usamos la actividad del sitio y del juego para mejorar Shadows of War. Solo se activa si la permites.");
  assert.equal(runtime.card.children[2].textContent, "Detalles de cookies");
  assert.equal(runtime.card.children[3].children[0].textContent, "Permitir");
  assert.equal(runtime.card.children[3].children[1].textContent, "Rechazar");
  assert.equal(runtime.card.attributes["aria-describedby"], "sow-analytics-copy");
  const [accept, reject] = runtime.card.children[3].children;
  assert.notEqual(accept.textContent, reject.textContent);
  assert.equal(runtime.local.values.size, 0);
  assert.deepEqual(runtime.fetchCalls, []);

  reject.click();
  assert.equal(JSON.parse(runtime.local.getItem("sow_analytics_consent_v2")).choice, "rejected");
  assert.equal(runtime.session.values.size, 0);
  assert.deepEqual(runtime.fetchCalls, []);
});

test("website consent stays in a bottom strip while game settings keep their top panel", () => {
  const siteRule = controlsCss.match(/\.sow-analytics-consent:not\(\.sow-analytics-consent--game\)\s*\{([^}]*)\}/);
  assert.ok(siteRule, "website has a separate consent layout");
  assert.match(siteRule[1], /inset-inline:\s*0;/);
  assert.match(siteRule[1], /bottom:\s*0;/);
  assert.match(siteRule[1], /grid-template-areas:\s*"title actions" "copy actions" "policy actions";/);
  assert.match(controlsCss, /@media \(max-width: 720px\)\s*\{\s*\.sow-analytics-consent:not\(\.sow-analytics-consent--game\)\s*\{[^}]*grid-template-areas:\s*"title" "copy" "policy" "actions";/s);
  const gameRule = controlsCss.match(/\.sow-analytics-consent--game\s*\{([^}]*)\}/);
  assert.ok(gameRule, "game settings keep their separate panel");
  assert.match(gameRule[1], /top:\s*max\(/);
  assert.match(gameRule[1], /bottom:\s*auto;/);
});

test("network retries reuse the same event IDs and clear the queue only after success", async () => {
  let attempts = 0;
  const runtime = start({ fetchImpl: async () => {
    attempts += 1;
    if (attempts === 1) throw new Error("offline");
    return { ok: true };
  } });
  runtime.card.children[3].children[0].click();
  await new Promise((resolve) => setImmediate(resolve));

  const queued = JSON.parse(runtime.session.getItem("sow_analytics_queue_v2"));
  assert.equal(runtime.fetchCalls.length, 1);
  assert.ok(queued.length > 0);

  runtime.flushAgain();
  await new Promise((resolve) => setImmediate(resolve));
  const firstBatch = JSON.parse(runtime.fetchCalls[0].options.body);
  const retryBatch = JSON.parse(runtime.fetchCalls[1].options.body);
  assert.deepEqual(retryBatch.events.map((event) => event.event_id), firstBatch.events.map((event) => event.event_id));
  assert.equal(runtime.session.getItem("sow_analytics_queue_v2"), "[]");
});

test("accepting sends consented events only to the first-party endpoint", () => {
  const runtime = start();
  const [accept] = runtime.card.children[3].children;
  accept.click();

  assert.equal(JSON.parse(runtime.local.getItem("sow_analytics_consent_v2")).choice, "accepted");
  assert.ok(runtime.session.getItem("sow_analytics_session_v2"));
  assert.equal(runtime.fetchCalls.length, 1);
  assert.equal(runtime.fetchCalls[0].url, "/api/event");
  assert.equal(runtime.fetchCalls[0].options.credentials, "omit");
  const batch = JSON.parse(runtime.fetchCalls[0].options.body);
  assert.equal(batch.events[0].consented, true);
  assert.equal(batch.events[0].consent_version, "2026-10-06");
  assert.equal(batch.events[0].portal, "site");
  assert.equal(batch.events[0].platform, "web");
  assert.equal(batch.events[0].device_class, "mobile");
});

test("direct game entry does not prompt or create/send analytics without consent", () => {
  const runtime = start({ pathname: "/play/", gameShell: true });
  assert.equal(runtime.card, undefined);
  runtime.window.SOW_analyticsTrack("shell_loaded");
  assert.equal(runtime.session.values.size, 0);
  assert.deepEqual(runtime.fetchCalls, []);

  runtime.window.SOW_analyticsManage();
  const card = runtime.body.children.find((node) => node.className.includes("sow-analytics-consent"));
  assert.ok(card, "settings can open the notice when the player asks");
  assert.equal(card.hidden, false);
});

test("previously accepted website consent works in game without another prompt", async () => {
  const runtime = start({ pathname: "/play/", gameShell: true, choice: "accepted" });
  assert.equal(runtime.card, undefined);
  runtime.window.SOW_analyticsTrack("shell_loaded");
  assert.ok(runtime.session.getItem("sow_analytics_session_v2"));
  runtime.flushAgain();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(runtime.fetchCalls.length, 1);
});

test("tutorial step analytics keeps only the bounded shared episode and action fields", () => {
  const runtime = start({ pathname: "/play/", gameShell: true, choice: "accepted" });
  runtime.window.SOW_analyticsTrack("tutorial_step", {
    episode_id: "boudica", step_id: "claim-wilderness", step_index: 2, action: "complete"
  });
  const queued = JSON.parse(runtime.session.getItem("sow_analytics_queue_v2"));
  assert.equal(queued.at(-1).name, "tutorial_step");
  assert.deepEqual(queued.at(-1).props, {
    episode_id: "boudica", step_id: "claim-wilderness", step_index: 2, action: "complete"
  });

  runtime.window.SOW_analyticsTrack("tutorial_step", {
    episode_id: "boudica", step_id: "claim-wilderness", step_index: 2, action: "skip", player_name: "ignored"
  });
  const afterUnsafeEvent = JSON.parse(runtime.session.getItem("sow_analytics_queue_v2"));
  assert.equal(afterUnsafeEvent.length, queued.length + 1);
  assert.equal(afterUnsafeEvent.at(-1).props, undefined);
});

test("tutorial exit analytics keeps a bounded failed-step identity and accepts queued legacy events", () => {
  const runtime = start({ pathname: "/play/", gameShell: true, choice: "accepted" });
  runtime.window.SOW_analyticsTrack("tutorial_exit_early", {
    episode_id: "boudica", step_id: "claim-wilderness", step_index: 2, action: "fail"
  });
  let queued = JSON.parse(runtime.session.getItem("sow_analytics_queue_v2"));
  assert.deepEqual(queued.at(-1).props, {
    episode_id: "boudica", step_id: "claim-wilderness", step_index: 2, action: "fail"
  });

  runtime.window.SOW_analyticsTrack("tutorial_exit_early", {
    episode_id: "boudica", step_id: "claim-wilderness", step_index: 2, action: "fail", player_name: "ignored"
  });
  runtime.window.SOW_analyticsTrack("tutorial_exit_early", { episode_id: "boudica" });
  queued = JSON.parse(runtime.session.getItem("sow_analytics_queue_v2"));
  assert.equal(queued.at(-2).props, undefined);
  assert.deepEqual(queued.at(-1).props, { episode_id: "boudica" });
});
