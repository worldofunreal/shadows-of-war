// Minimal Jest runtime bridge. This file is copied to the Jest artifact as
// sdk/store_portals.js; it intentionally contains no account, store,
// CrazyGames, or Poki integration. Jest has no ads: this bridge never
// pauses for commercial breaks and never calls gameplayStart/Stop style ad
// hooks.
(function () {
  "use strict";

  var sdkInitPromise = null;
  var loadingFinished = false;
  var progressKey = "sow_player_progress";

  // D1-D7 re-engagement sequence (launch checklist: at least one per day
  // once the user is registered). Stable identifiers make re-scheduling
  // idempotent: scheduling the same identifier replaces the old one.
  var REENGAGE = [
    { id: "sow_reengage_d0", days: 0, body: "Battle stations: your first match is ready when you are.", cta: "Play now" },
    { id: "sow_reengage_d1", days: 1, body: "Your army held the line. Fresh battles are waiting.", cta: "Play now" },
    { id: "sow_reengage_d2", days: 2, body: "Daily reward ready: extra troops for your next match.", cta: "Claim" },
    { id: "sow_reengage_d3", days: 3, body: "Your rivals expanded overnight. Take the territory back.", cta: "Attack" },
    { id: "sow_reengage_d4", days: 4, body: "New commanders joined the war. Try a different leader.", cta: "Play now" },
    { id: "sow_reengage_d5", days: 5, body: "Your win rate slipped. Warm up with a quick match.", cta: "Quick match" },
    { id: "sow_reengage_d6", days: 6, body: "A tournament lobby with your name on it just opened.", cta: "Join" },
    { id: "sow_reengage_d7", days: 7, body: "One week of war. Your empire misses its commander.", cta: "Return" }
  ];

  function jestSdk() {
    return window.JestSDK || null;
  }

  function safeLocalGet(key) {
    try { return window.localStorage.getItem(key); } catch (e) { return null; }
  }

  function safeLocalSet(key, value) {
    try { window.localStorage.setItem(key, value); } catch (e) {}
  }

  function currentPlayer() {
    var sdk = jestSdk();
    if (!sdk || typeof sdk.getPlayer !== "function") return null;
    try { return sdk.getPlayer() || null; } catch (e) { return null; }
  }

  function scheduleReengagement() {
    var sdk = jestSdk();
    if (!sdk || !sdk.notifications || typeof sdk.notifications.scheduleNotification !== "function") return;
    var player = currentPlayer();
    // The platform discards guest notifications; only registered users qualify.
    if (!player || !player.registered) return;
    REENGAGE.forEach(function (item) {
      try {
        sdk.notifications.scheduleNotification({
          body: item.body,
          ctaText: item.cta,
          identifier: item.id,
          priority: "medium",
          scheduledInDays: item.days
        });
      } catch (e) {}
    });
  }

  function refreshPortalFlags() {
    window.SOW_RUNTIME = { portal_embed: true, site_embed: false, crazygames: false, poki: false, jest: true };
    window.SOW_isSiteEmbed = false;
    window.SOW_isPortalEmbed = function () { return true; };
    window.SOW_isOnCrazyGames = function () { return false; };
    window.SOW_isOnPoki = function () { return false; };
    window.SOW_isOnJest = function () { return true; };
    window.SOW_DISABLE_CHAT = true;
    window.SOW_PORTAL_PROGRESS_JSON = safeLocalGet(progressKey);
    syncPlatformIdentity();
  }

  function syncPlatformIdentity() {
    var player = currentPlayer();
    if (player && player.playerId) {
      window.SOW_PLATFORM_IDENTITY = {
        provider: "jest",
        displayName: player.username || "Player",
        externalId: player.playerId,
        avatarUrl: player.avatarUrl || null,
        token: null
      };
    } else {
      window.SOW_PLATFORM_IDENTITY = null;
    }
  }

  function muteGameAudio() {
    window.SOW_PORTAL_MUTE_AUDIO = true;
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = true;
    });
  }

  function unmuteGameAudio() {
    window.SOW_PORTAL_MUTE_AUDIO = false;
    if (window.SOW_adPlaying) return;
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = false;
    });
  }

  window.SOW_REFRESH_JEST_FLAGS = refreshPortalFlags;
  window.SOW_refreshPortalFlags = refreshPortalFlags;
  window.SOW_PORTAL_PROGRESS_JSON = null;
  window.SOW_PLATFORM_IDENTITY = null;
  window.SOW_PENDING_INVITE_LOBBY_ID = null;
  window.SOW_HOST_PRIVATE_PENDING = false;
  window.SOW_PORTAL_MUTE_AUDIO = false;
  window.SOW_adPlaying = false;
  window.SOW_DISABLE_CHAT = true;
  window.SOW_BLOCKED_IDS = [];

  window.SOW_isAndroidTwa = function () { return false; };

  window.SOW_portalMuteGameAudio = muteGameAudio;
  window.SOW_portalUnmuteGameAudio = unmuteGameAudio;
  // No ads on Jest: pause/resume exist only so shared callers stay safe.
  window.SOW_portalAdPause = function () {};
  window.SOW_portalAdResume = function () {};
  window.SOW_portalConsumeBootIntent = function () {};
  window.SOW_portalClearPendingInvite = function () { window.SOW_PENDING_INVITE_LOBBY_ID = null; };
  window.SOW_portalClearHostPrivatePending = function () { window.SOW_HOST_PRIVATE_PENDING = false; };
  window.SOW_portalUpdateRoom = function () {};
  window.SOW_portalLeftRoom = function () {};
  window.SOW_portalHappytime = function () {};
  window.SOW_portalSaveProgress = function (json) {
    window.SOW_PORTAL_PROGRESS_JSON = String(json || "");
    safeLocalSet(progressKey, window.SOW_PORTAL_PROGRESS_JSON);
  };

  // Registration entry point for the menu (settings save-progress button).
  // Uses the platform default popup; resolves immediately when registered.
  window.SOW_jestLogin = function () {
    var sdk = jestSdk();
    if (!sdk || typeof sdk.login !== "function") return Promise.resolve();
    var player = currentPlayer();
    var done = function () {
      syncPlatformIdentity();
      scheduleReengagement();
    };
    try {
      return Promise.resolve(sdk.login({ entryPayload: { source: "settings_save_progress" } })).then(done, done);
    } catch (e) {
      return Promise.resolve();
    }
  };

  window.SOW_initPortalSdk = async function () {
    if (sdkInitPromise) return sdkInitPromise;
    refreshPortalFlags();
    var sdk = jestSdk();
    if (!sdk || typeof sdk.init !== "function") {
      console.warn("Jest SDK unavailable; continuing anonymously");
      sdkInitPromise = Promise.resolve();
      return sdkInitPromise;
    }
    sdkInitPromise = Promise.resolve().then(function () {
      return sdk.init();
    }).then(function () {
      console.info("Jest SDK initialized");
      if (sdk.lifecycle) {
        try {
          if (typeof sdk.lifecycle.onHide === "function") sdk.lifecycle.onHide(muteGameAudio);
          if (typeof sdk.lifecycle.onShow === "function") sdk.lifecycle.onShow(unmuteGameAudio);
        } catch (e) {}
      }
      syncPlatformIdentity();
      scheduleReengagement();
    }).catch(function (error) {
      console.warn("Jest SDK init failed; continuing anonymously:", error);
    });
    return sdkInitPromise;
  };

  window.SOW_portalGameLoadingFinished = function () {
    if (loadingFinished) return;
    loadingFinished = true;
    var sdk = jestSdk();
    if (sdk && typeof sdk.markGameLoaded === "function") {
      try { sdk.markGameLoaded(); } catch (e) {}
    }
  };

  // The Rust client calls these on match boundaries; Jest has no ad-gated
  // gameplay hooks, so they stay no-ops (progress persists separately).
  window.SOW_portalGameplayStart = function () {};
  window.SOW_portalGameplayStop = function () {};

  window.SOW_portalLoadStart = function () {};
  window.SOW_portalLoadStop = window.SOW_portalGameLoadingFinished;

  refreshPortalFlags();
})();
