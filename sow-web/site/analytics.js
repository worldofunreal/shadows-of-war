(() => {
  "use strict";

  const POLICY = "2026-10-06";
  const CONSENT_KEY = "sow_analytics_consent_v2";
  const SESSION_KEY = "sow_analytics_session_v2";
  const QUEUE_KEY = "sow_analytics_queue_v2";
  const MAX_QUEUE = 200;
  const MAX_BATCH = 100;
  const KNOWN_EVENTS = new Set([
    "landing_visit", "shell_loaded", "play_now_click", "boot_start", "boot_route_decision", "load_stage",
    "menu_quick_match", "menu_join_attempt", "menu_password_join_attempt", "menu_code_join_attempt",
    "menu_custom_create", "menu_single_player_start", "menu_campaign_start", "menu_campaign_open",
    "menu_lobby_browser_open", "menu_custom_create_open", "menu_leader_confirm", "matchmaking_joined",
    "lobby_joined", "lobby_join_failed", "match_exit", "match_loading_start", "match_started_client", "match_ended_client",
    "tutorial_start", "tutorial_step",
    "tutorial_objective_complete", "tutorial_dialog_choice", "tutorial_exit_early", "campaign_episode_complete"
  ]);
  const isGameShell = Boolean(document.getElementById("blade"));
  const isAndroidTwa = () => {
    if (typeof window.SOW_isAndroidTwa === "function" && window.SOW_isAndroidTwa()) return true;
    const query = new URLSearchParams(window.location.search);
    return document.referrer.startsWith("android-app://com.shadowsofwar")
      || (query.get("sow_platform") === "android" && /Android/i.test(navigator.userAgent));
  };
  const canSend = () => ["shadowsofwar.io", "www.shadowsofwar.io"].includes(window.location.hostname)
    && !isAndroidTwa() && !["poki", "crazygames", "jest"].includes(window.SOW_PORTAL);
  if (isAndroidTwa()) {
    window.SOW_analyticsTrack = () => {};
    window.SOW_analyticsManage = () => {};
    return;
  }

  let consent = null;
  let sessionId = null;
  let queue = [];
  let panel = null;
  let busy = false;
  let controller = null;
  let flushTimer = null;
  let landingRecorded = false;

  function text(key, fallback) {
    if (typeof window.SOW_t !== "function") return fallback;
    const value = window.SOW_t(key);
    return value && value !== `[${key}]` ? value : fallback;
  }

  function uuid() {
    if (crypto && typeof crypto.randomUUID === "function") return crypto.randomUUID();
    if (!crypto || typeof crypto.getRandomValues !== "function") return null;
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, value => value.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }

  function readChoice() {
    try {
      const saved = JSON.parse(localStorage.getItem(CONSENT_KEY) || "null");
      return saved && saved.policy === POLICY && ["accepted", "rejected"].includes(saved.choice)
        ? saved.choice : null;
    } catch (_) { return null; }
  }

  function startFlushTimer() {
    if (flushTimer === null) flushTimer = window.setInterval(() => { void flush(); }, 10_000);
  }

  function stopFlushTimer() {
    if (flushTimer === null) return;
    window.clearInterval(flushTimer);
    flushTimer = null;
  }

  function saveChoice(choice) {
    consent = choice;
    try { localStorage.setItem(CONSENT_KEY, JSON.stringify({ choice, policy: POLICY })); } catch (_) {}
    if (choice === "accepted") {
      if (canSend()) {
        ensureSession();
        restoreQueue();
        startFlushTimer();
      } else {
        queue = [];
      }
      if (isLandingPage()) recordLanding();
      flush();
    } else {
      stopFlushTimer();
      sessionId = null;
      queue = [];
      if (controller) controller.abort();
      try {
        sessionStorage.removeItem(SESSION_KEY);
        sessionStorage.removeItem(QUEUE_KEY);
      } catch (_) {}
    }
    closePanel();
  }

  function isLandingPage() {
    return window.location.pathname === "/" || window.location.pathname === "/index.html";
  }

  function ensureSession() {
    if (sessionId) return sessionId;
    try { sessionId = sessionStorage.getItem(SESSION_KEY); } catch (_) {}
    if (!sessionId) {
      sessionId = uuid();
      if (!sessionId) return null;
      try { sessionStorage.setItem(SESSION_KEY, sessionId); } catch (_) {}
    }
    return sessionId;
  }

  function restoreQueue() {
    try {
      const saved = JSON.parse(sessionStorage.getItem(QUEUE_KEY) || "[]");
      queue = Array.isArray(saved) ? saved.filter(event => event && typeof event.event_id === "string").slice(-MAX_QUEUE) : [];
    } catch (_) { queue = []; }
  }

  function persistQueue() {
    try { sessionStorage.setItem(QUEUE_KEY, JSON.stringify(queue)); } catch (_) {}
  }

  function validProps(name, props) {
    if (props == null) return null;
    if (name === "load_stage" && ["relay_connect_start", "relay_connect_complete", "engine_init_complete", "gpu_upload_complete", "snapshot_available", "ready_sent"].includes(props.stage)) {
      return { stage: props.stage };
    }
    if (name === "boot_route_decision" && ["menu", "intro"].includes(props.route)) return { route: props.route };
    if (name === "tutorial_step") {
      if (Object.keys(props).length === 1 && Number.isInteger(props.step_index) && props.step_index >= 0 && props.step_index < 512) {
        return { step_index: props.step_index };
      }
      if (Object.keys(props).length === 4
        && typeof props.episode_id === "string" && /^[a-z][a-z0-9_]{0,63}$/.test(props.episode_id)
        && typeof props.step_id === "string" && /^[a-z][a-z0-9_-]{0,95}$/.test(props.step_id)
        && Number.isInteger(props.step_index) && props.step_index >= 0 && props.step_index < 512
        && ["start", "complete"].includes(props.action)) {
        return {
          episode_id: props.episode_id,
          step_id: props.step_id,
          step_index: props.step_index,
          action: props.action
        };
      }
    }
    if (name === "tutorial_exit_early" && Object.keys(props).length === 4
      && typeof props.episode_id === "string" && /^[a-z][a-z0-9_]{0,63}$/.test(props.episode_id)
      && typeof props.step_id === "string" && /^[a-z][a-z0-9_-]{0,95}$/.test(props.step_id)
      && Number.isInteger(props.step_index) && props.step_index >= 0 && props.step_index < 512
      && props.action === "fail") {
      return {
        episode_id: props.episode_id,
        step_id: props.step_id,
        step_index: props.step_index,
        action: props.action
      };
    }
    if (["tutorial_start", "tutorial_exit_early", "campaign_episode_complete"].includes(name)
      && Object.keys(props).length === 1 && typeof props.episode_id === "string"
      && /^[a-z][a-z0-9_]{0,63}$/.test(props.episode_id)) {
      return { episode_id: props.episode_id };
    }
    return null;
  }

  function accountId() {
    try {
      const value = localStorage.getItem("sow_account_id") || "";
      return /^[a-f0-9]{32}$/i.test(value) ? value : null;
    } catch (_) { return null; }
  }

  function deviceClass() {
    const width = Math.max(document.documentElement.clientWidth || 0, window.innerWidth || 0);
    return width < 680 ? "mobile" : width < 1100 ? "tablet" : "desktop";
  }

  function track(name, props) {
    if (consent !== "accepted" || !canSend() || !KNOWN_EVENTS.has(name)) return;
    const id = ensureSession();
    const eventId = uuid();
    if (!id || !eventId) return;
    const event = {
      v: 2,
      event_id: eventId,
      consented: true,
      consent_version: POLICY,
      name,
      ts_ms: Date.now(),
      session_id: id,
      portal: "site",
      platform: "web",
      device_class: deviceClass(),
      build: String(window.SOW_BUILD_VERSION || "web").slice(0, 32),
      locale: String(document.documentElement.lang || navigator.language || "en").slice(0, 32)
    };
    const currentAccount = accountId();
    if (currentAccount) event.account_id = currentAccount;
    const safeProps = validProps(name, props);
    if (safeProps) event.props = safeProps;
    queue.push(event);
    if (queue.length > MAX_QUEUE) queue.splice(0, queue.length - MAX_QUEUE);
    persistQueue();
    if (queue.length >= 20) flush();
  }

  async function flush(keepalive = false) {
    if (busy || !queue.length || consent !== "accepted" || !canSend()) return;
    const batch = queue.slice(0, MAX_BATCH);
    const sentIds = new Set(batch.map(event => event.event_id));
    busy = true;
    controller = new AbortController();
    try {
      const response = await fetch("/api/event", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ events: batch }),
        credentials: "omit",
        cache: "no-store",
        keepalive,
        signal: controller.signal
      });
      if (!response.ok) return;
      queue = queue.filter(event => !sentIds.has(event.event_id));
      persistQueue();
    } catch (_) {
      // Keep the same IDs so a later retry cannot double-count the batch.
    } finally {
      busy = false;
      controller = null;
    }
  }

  function closePanel() {
    if (panel) panel.remove();
    panel = null;
  }

  function openSettings() {
    if (!panel) buildPanel();
    panel.hidden = false;
  }

  function buildPanel() {
    const card = document.createElement("section");
    card.className = `sow-analytics-consent${isGameShell ? " sow-analytics-consent--game" : ""}`;
    card.setAttribute("aria-labelledby", "sow-analytics-title");
    card.setAttribute("aria-describedby", "sow-analytics-copy");
    const title = document.createElement("h2");
    title.id = "sow-analytics-title";
    title.className = "sow-analytics-consent__title";
    const body = document.createElement("p");
    body.id = "sow-analytics-copy";
    body.className = "sow-analytics-consent__copy";
    const policyLink = document.createElement("a");
    policyLink.className = "sow-analytics-consent__policy";
    policyLink.href = "/cookies/";
    const actions = document.createElement("div");
    actions.className = "sow-analytics-consent__actions";
    const accept = document.createElement("button");
    const reject = document.createElement("button");
    accept.type = reject.type = "button";
    accept.className = "sow-analytics-consent__button sow-analytics-consent__button--accept";
    reject.className = "sow-analytics-consent__button sow-analytics-consent__button--reject";
    accept.addEventListener("click", () => saveChoice("accepted"));
    reject.addEventListener("click", () => saveChoice("rejected"));
    card.append(title, body, policyLink, actions);
    actions.append(accept, reject);
    document.body.append(card);
    panel = card;
    card._refreshCopy = () => {
      title.textContent = text("site.analytics_title", "Optional analytics");
      body.textContent = text("site.analytics_body", "We use site and game activity to improve Shadows of War. It stays off unless you allow it.");
      policyLink.textContent = text("site.analytics_policy", "Cookie details");
      accept.textContent = text("site.analytics_accept", "Allow");
      reject.textContent = text("site.analytics_reject", "Reject");
    };
    card._refreshCopy();
    card.hidden = true;
  }

  function recordLanding() {
    if (landingRecorded) return;
    landingRecorded = true;
    track("landing_visit");
  }

  function addManageControl() {
    const footer = document.querySelector(".footer-links");
    if (!footer) return;
    let button = footer.querySelector("[data-sow-cookie-settings]");
    if (!button) {
      button = document.createElement("button");
      button.type = "button";
      button.dataset.sowCookieSettings = "";
      button.className = "sow-cookie-settings-link";
      button.addEventListener("click", openSettings);
      footer.append(button);
    }
    button.textContent = text("site.analytics_manage", "Cookie settings");
  }

  window.SOW_analyticsTrack = track;
  window.SOW_analyticsManage = openSettings;
  document.addEventListener("click", event => {
    const target = event.target && event.target.closest ? event.target.closest("[data-sow-cookie-settings]") : null;
    if (target) { event.preventDefault(); openSettings(); }
    const link = event.target && event.target.closest ? event.target.closest("a[href]") : null;
    if (link && link.pathname === "/play/" && consent === "accepted") track("play_now_click");
  });
  window.addEventListener("pagehide", () => { void flush(true); });
  window.addEventListener("sow:locale-change", () => {
    if (panel && panel._refreshCopy) panel._refreshCopy();
    addManageControl();
  });

  consent = readChoice();
  if (consent === "accepted") {
    if (canSend()) {
      ensureSession();
      restoreQueue();
    }
    if (isLandingPage()) recordLanding();
    startFlushTimer();
  } else if (consent === "rejected") {
    try { sessionStorage.removeItem(SESSION_KEY); sessionStorage.removeItem(QUEUE_KEY); } catch (_) {}
  }

  const start = () => {
    addManageControl();
    if (consent === null) {
      if (!isGameShell) openSettings();
      return;
    }
    if (queue.length) void flush();
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start, { once: true });
  else start();
})();
