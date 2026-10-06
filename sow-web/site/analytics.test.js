const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { webcrypto } = require("node:crypto");

const source = fs.readFileSync(path.join(__dirname, "analytics.js"), "utf8");

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
    setAttribute() {},
    remove() { this.removed = true; },
    querySelector() { return null; },
    click() { for (const handler of listeners.get("click") || []) handler({ preventDefault() {} }); }
  };
}

function start() {
  const local = storage();
  const session = storage();
  const fetchCalls = [];
  const domListeners = new Map();
  const windowListeners = new Map();
  const head = element("head");
  const body = element("body");
  const document = {
    readyState: "loading",
    referrer: "",
    documentElement: { lang: "es", dataset: {} },
    head,
    body,
    createElement: element,
    getElementById(id) { return head.children.find((node) => node.id === id) || null; },
    addEventListener(name, handler) { domListeners.set(name, handler); },
    querySelector() { return null; }
  };
  const window = {
    location: { hostname: "shadowsofwar.io", pathname: "/", search: "" },
    addEventListener(name, handler) { windowListeners.set(name, handler); },
    setInterval() {},
    SOW_t(key) {
      return ({
        "site.analytics_title": "Analítica opcional",
        "site.analytics_accept": "Permitir analítica opcional",
        "site.analytics_reject": "Rechazar analítica opcional"
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
      return Promise.resolve({ ok: true });
    }
  };

  vm.runInNewContext(source, context, { filename: "analytics.js" });
  domListeners.get("DOMContentLoaded")();
  const card = body.children.find((node) => node.className.includes("sow-analytics-consent"));
  assert.ok(card, "consent choice is presented");
  return { card, head, local, session, fetchCalls };
}

test("consent panel loads and rejecting sends nothing or creates an analytics session", () => {
  const runtime = start();
  assert.ok(runtime.head.children.some((node) => node.id === "sow-analytics-consent-style"));
  assert.equal(runtime.card.children[0].textContent, "Analítica opcional");
  const [accept, reject] = runtime.card.children[3].children;
  assert.notEqual(accept.textContent, reject.textContent);
  assert.deepEqual(runtime.fetchCalls, []);

  reject.click();
  assert.equal(JSON.parse(runtime.local.getItem("sow_analytics_consent_v2")).choice, "rejected");
  assert.equal(runtime.session.values.size, 0);
  assert.deepEqual(runtime.fetchCalls, []);
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
