// Minimal Poki runtime bridge. This file is copied to the Poki artifact as
// sdk/store_portals.js; it intentionally contains no account, store, or
// CrazyGames integration.
(function () {
  "use strict";

  var sdk = null;
  var loadingFinished = false;
  var gameplayReady = false;
  var gameplayActive = false;
  var firstInputSeen = false;
  var resumePending = false;
  var inputInstalled = false;
  var progressKey = "sow_player_progress";

  function pokiSdk() {
    return window.PokiSDK || null;
  }

  function isPoki() {
    return window.SOW_PORTAL === "poki";
  }

  function safeLocalGet(key) {
    try { return window.localStorage.getItem(key); } catch (e) { return null; }
  }

  function safeLocalSet(key, value) {
    try { window.localStorage.setItem(key, value); } catch (e) {}
  }

  function portalAdPause() {
    window.SOW_adPlaying = true;
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.dataset.sowWasPaused = el.paused ? "1" : "0";
      el.pause();
      el.muted = true;
    });
  }

  function portalAdResume() {
    window.SOW_adPlaying = false;
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = false;
      if (el.dataset.sowWasPaused !== "1") el.play().catch(function () {});
      delete el.dataset.sowWasPaused;
    });
  }

  function startGameplayAfterAd() {
    if (!gameplayReady || gameplayActive || !sdk || typeof sdk.gameplayStart !== "function") return;
    gameplayActive = true;
    sdk.gameplayStart();
  }

  function requestCommercialBreakThenStart() {
    if (!sdk || typeof sdk.commercialBreak !== "function") {
      startGameplayAfterAd();
      return;
    }
    portalAdPause();
    try {
      Promise.resolve(sdk.commercialBreak()).then(function () {
        portalAdResume();
        startGameplayAfterAd();
      }).catch(function (error) {
        console.warn("Poki commercialBreak failed:", error);
        portalAdResume();
        startGameplayAfterAd();
      });
    } catch (error) {
      console.warn("Poki commercialBreak failed:", error);
      portalAdResume();
      startGameplayAfterAd();
    }
  }

  function onFirstInput() {
    if (!gameplayReady || firstInputSeen || window.SOW_adPlaying) return;
    firstInputSeen = true;
    if (resumePending) {
      resumePending = false;
      requestCommercialBreakThenStart();
    } else {
      startGameplayAfterAd();
    }
  }

  function blockGameInputDuringAd(event) {
    if (!window.SOW_adPlaying) return;
    var target = event.target;
    var gameSurface = target && target.closest && target.closest("canvas, #sow-menu");
    if (!gameSurface) return;
    event.preventDefault();
    event.stopImmediatePropagation();
  }

  function installFirstInputListener() {
    if (inputInstalled) return;
    inputInstalled = true;
    ["pointerdown", "touchstart", "keydown"].forEach(function (type) {
      document.addEventListener(type, onFirstInput, { capture: true, passive: true });
      document.addEventListener(type, blockGameInputDuringAd, { capture: true, passive: false });
    });
  }

  function refreshPortalFlags() {
    window.SOW_RUNTIME = { portal_embed: true, site_embed: false, crazygames: false, poki: true };
    window.SOW_isSiteEmbed = false;
    window.SOW_isPortalEmbed = function () { return true; };
    window.SOW_isOnCrazyGames = function () { return false; };
    window.SOW_isOnPoki = function () { return true; };
    window.SOW_DISABLE_CHAT = true;
    window.SOW_PORTAL_LOCALE = navigator.language || "en";
    window.SOW_PORTAL_PROGRESS_JSON = safeLocalGet(progressKey);
  }

  function measure(category, what, action) {
    if (!sdk || typeof sdk.measure !== "function") return;
    if (arguments.length === 1) {
      var parts = String(category || "").split("|");
      category = parts[0] || "gameplay";
      what = parts[1] || "event";
      action = parts[2] || "interact";
    }
    try { sdk.measure(String(category), String(what), String(action)); } catch (e) {}
  }

  window.SOW_REFRESH_POKI_FLAGS = refreshPortalFlags;
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
  window.SOW_getWouSession = function () { return { token: "", accountId: "", user: null }; };
  window.SOW_ensureWouAnonymousSession = function () {
    return Promise.resolve({ token: "", accountId: "", user: null });
  };
  window.SOW_isSowProductionHost = function () { return false; };
  window.SOW_getAuthState = function () {
    return { platform: "poki", provider: null, authenticated: false, pending: false };
  };
  window.SOW_isBlockedId = function () { return false; };
  window.SOW_portalShowAuthPrompt = function () {};
  window.SOW_portalSignOut = function () {};
  window.SOW_startWouOAuth = function () { return false; };
  window.SOW_signOutWou = function () { return false; };
  window.SOW_portalClearAuthChanged = function () { window.SOW_AUTH_CHANGED = false; };
  window.SOW_AUTH_CHANGED = false;

  window.SOW_portalMuteGameAudio = portalAdPause;
  window.SOW_portalUnmuteGameAudio = portalAdResume;
  window.SOW_portalAdPause = portalAdPause;
  window.SOW_portalAdResume = portalAdResume;
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

  window.SOW_pokiMeasure = measure;
  window.SOW_pokiOpenExternalLink = function (url) {
    if (sdk && typeof sdk.openExternalLink === "function") sdk.openExternalLink(String(url));
  };

  window.SOW_initPortalSdk = async function () {
    refreshPortalFlags();
    sdk = pokiSdk();
    installFirstInputListener();
    if (!sdk || typeof sdk.init !== "function") {
      console.warn("Poki SDK unavailable; continuing anonymously");
      return;
    }
    try {
      await sdk.init();
      console.info("Poki SDK initialized");
    } catch (error) {
      console.warn("Poki SDK init failed; continuing anonymously:", error);
    }
  };

  window.SOW_portalGameLoadingFinished = function () {
    if (loadingFinished) return;
    loadingFinished = true;
    if (sdk && typeof sdk.gameLoadingFinished === "function") {
      try { sdk.gameLoadingFinished(); } catch (e) {}
    }
  };

  window.SOW_portalGameplayStart = function () {
    gameplayReady = true;
    installFirstInputListener();
    measure("gameplay", "session", "ready");
  };

  window.SOW_portalGameplayStop = function () {
    if (!gameplayReady && !gameplayActive) return;
    gameplayReady = false;
    firstInputSeen = false;
    resumePending = gameplayActive;
    if (gameplayActive && sdk && typeof sdk.gameplayStop === "function") {
      try { sdk.gameplayStop(); } catch (e) {}
    }
    gameplayActive = false;
  };

  window.SOW_portalLoadStart = function () {};
  window.SOW_portalLoadStop = window.SOW_portalGameLoadingFinished;

  refreshPortalFlags();
})();
