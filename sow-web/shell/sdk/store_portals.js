/**
 * CrazyGames / Poki portal hooks. No-op when SDKs are absent (self-hosted).
 */
(function () {
  function crazyGameApi() {
    return window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.game;
  }

  function isLocalDevHost() {
    var h = window.location.hostname || "";
    return h === "localhost" || h === "127.0.0.1" || h === "[::1]";
  }

  // CrazyGames sitelock: crazygames.* TLDs and subdomains (see docs.crazygames.com/resources/sitelock).
  function isValidCrazyGamesDomain(hostname) {
    hostname = hostname || "";
    if (/^dev-crazygames\.com$/i.test(hostname)) {
      return true;
    }
    if (/\.game-files\.crazygames\.com$/i.test(hostname)) {
      return true;
    }
    var parts = hostname.split(".");
    var idx = parts.indexOf("crazygames");
    return idx !== -1 && idx >= parts.length - 3;
  }

  function isCrazyGamesHost() {
    if (isValidCrazyGamesDomain(window.location.hostname)) {
      return true;
    }
    var ref = document.referrer || "";
    return /crazygames/i.test(ref);
  }

  function enforceCrazyGamesSitelock() {
    if (window.SOW_PORTAL !== "crazygames") {
      return;
    }
    if (isValidCrazyGamesDomain(window.location.hostname) || isLocalDevHost()) {
      return;
    }
    document.body.innerHTML =
      '<div style="display:flex;align-items:center;justify-content:center;height:100vh;margin:0;font-family:sans-serif;background:#101018;color:#fff;text-align:center;padding:24px;">Available only on CrazyGames</div>';
    throw new Error("CrazyGames sitelock: unauthorized host");
  }

  function isSiteEmbed() {
    return window.SOW_PORTAL === "site";
  }

  function isPortalEmbed() {
    return (
      window.SOW_PORTAL === "crazygames" ||
      window.SOW_PORTAL === "poki" ||
      window.SOW_PORTAL === "site" ||
      isOnCrazyGames() ||
      isOnPoki()
    );
  }

  function isOnCrazyGames() {
    if (isSiteEmbed()) {
      return false;
    }
    if (window.SOW_PORTAL === "crazygames") {
      return true;
    }
    if (isCrazyGamesHost()) {
      return true;
    }
    try {
      if (window.self !== window.top) {
        return (window.top.location.hostname || "").includes("crazygames");
      }
      return false;
    } catch (e) {
      if (document.referrer && document.referrer.includes("crazygames")) {
        return true;
      }
      return window.self !== window.top;
    }
  }

  function crazyGamesSdkReady() {
    var sdk = window.CrazyGames && window.CrazyGames.SDK;
    if (!sdk) {
      return false;
    }
    var env = sdk.environment;
    return env === "local" || env === "crazygames";
  }

  function isOnPoki() {
    if (isSiteEmbed()) {
      return false;
    }
    if (window.SOW_PORTAL === "poki") {
      return true;
    }
    var h = window.location.hostname || "";
    if (/poki\.com$/i.test(h) || /poki-gdn\.com$/i.test(h) || /poki\.io$/i.test(h)) {
      return true;
    }
    var ref = document.referrer || "";
    return /poki\.com/i.test(ref) || /poki-gdn\.com/i.test(ref);
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
      if (el.dataset.sowWasPaused !== "1") {
        el.play().catch(function () {});
      }
      delete el.dataset.sowWasPaused;
    });
  }

  function parseInviteLobbyId(inviteParams) {
    if (!inviteParams || typeof inviteParams !== "object") {
      return null;
    }
    var raw = inviteParams.lobbyId;
    if (raw === undefined || raw === null || raw === "") {
      return null;
    }
    var id = parseInt(String(raw), 10);
    return isNaN(id) ? null : id;
  }

  function queueInviteLobbyId(lobbyId) {
    if (lobbyId !== null && lobbyId !== undefined) {
      try {
        var existing = window.SOW_PENDING_INVITE_LOBBY_ID;
        if (existing !== null && existing !== undefined && Number(existing) === Number(lobbyId)) {
          console.warn('store_portals: skipping duplicate pending invite', lobbyId);
          return;
        }
      } catch (e) {
        console.warn('store_portals: error reading existing pending invite', e);
      }
      console.log('store_portals: queueing pending invite', lobbyId);
      window.SOW_PENDING_INVITE_LOBBY_ID = lobbyId;
    }
  }

  function installJoinRoomListener() {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (!game || !game.addJoinRoomListener) {
      return;
    }
    game.addJoinRoomListener(function (inviteParams) {
      var lobbyId = parseInviteLobbyId(inviteParams);
      if (lobbyId !== null) {
        console.log("CrazyGames join room listener: lobby", lobbyId);
        queueInviteLobbyId(lobbyId);
      }
    });
  }

  function triggerHappytime() {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (game && game.happytime) {
      try {
        game.happytime();
      } catch (e) {
        console.warn("CrazyGames happytime trigger failed:", e);
      }
    }
  }

  function installInviteLinkHappytime() {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (!game || !game.inviteLink) {
      return;
    }
    var origInviteLink = game.inviteLink.bind(game);
    game.inviteLink = function (params) {
      var link = origInviteLink(params);
      triggerHappytime();
      return link;
    };
  }

  function applyGameSettings(settings) {
    if (!settings) {
      return;
    }
    window.SOW_PORTAL_MUTE_AUDIO = !!settings.muteAudio;
    if (settings.muteAudio) {
      window.SOW_portalMuteGameAudio();
    }
    window.SOW_DISABLE_CHAT = !!settings.disableChat;
  }

  var SOW_PROGRESS_KEY = "sow_player_progress";

  window.SOW_PLATFORM_IDENTITY = null;
  window.SOW_PENDING_INVITE_LOBBY_ID = null;
  window.SOW_HOST_PRIVATE_PENDING = false;
  window.SOW_PORTAL_MUTE_AUDIO = false;
  window.SOW_DISABLE_CHAT = false;
  window.SOW_PORTAL_LOCALE = null;

  function isAndroidTwa() {
    var referrer = String(document.referrer || "");
    if (/^android-app:\/\/com\.shadowsofwar(?:\/|$)/i.test(referrer)) {
      return true;
    }
    try {
      return new URLSearchParams(window.location.search).get("sow_platform") === "android" &&
        /Android/i.test(navigator.userAgent || "");
    } catch (e) {
      return false;
    }
  }

  window.SOW_isAndroidTwa = isAndroidTwa;

  var ANDROID_MODE_KEY = "sow_playgames_mode";
  var ANDROID_ANONYMOUS_MODE = "anonymous";
  var ANDROID_PENDING_KEY = "sow_playgames_pending";
  var androidPurchasePort = null;
  var androidPurchaseProducts = null;
  var androidAuthAttempt = 0;
  var androidLoaderReady = false;
  var androidSilentAuthTimer = 0;

  function androidBridgeMessage(message) {
    if (!message) return;
    if (message.type === "playgames_auth_result") {
      if (message.status === "unavailable" || message.status === "error") {
        var pending = androidPending();
        if (pending && pending.indexOf("auto:") === 0) {
          continueAnonymouslyAfterSilentAuth();
        }
      }
      window.dispatchEvent(new CustomEvent("sow:android-playgames-auth-result", { detail: message }));
      return;
    }
    if (message.type === "purchase_result") {
      window.dispatchEvent(new CustomEvent("sow:android-purchase-result", { detail: message }));
    }
  }

  function continueAnonymouslyAfterSilentAuth() {
    var pending = androidPending();
    if (!pending || pending.indexOf("auto:") !== 0) return;
    if (androidSilentAuthTimer) {
      clearTimeout(androidSilentAuthTimer);
      androidSilentAuthTimer = 0;
    }
    androidAuthAttempt += 1;
    androidPending(null);
    androidStorage(ANDROID_ANONYMOUS_MODE);
    try { sessionStorage.removeItem("sow_playgames_identity"); } catch (e) {}
    var params = new URLSearchParams(window.location.search);
    clearAndroidAuthParam(params, "sow_playgames_rendezvous");
    window.SOW_PLATFORM_IDENTITY = null;
    emitAuthStateChange();
    console.info("Play Games silent authentication unavailable; continuing anonymously");
  }

  function postAndroidBridgeMessage(message) {
    if (!androidPurchasePort) return false;
    try {
      androidPurchasePort.postMessage(JSON.stringify(message));
      return true;
    } catch (e) {
      return false;
    }
  }

  function requestAndroidSilentAuth() {
    var pending = androidPending();
    if (!androidLoaderReady || !pending || pending.indexOf("auto:") !== 0) return false;
    var rendezvousId = pending.slice("auto:".length);
    if (!postAndroidBridgeMessage({
      type: "playgames_silent_auth",
      rendezvous_id: rendezvousId,
    })) {
      if (!androidSilentAuthTimer) {
        androidSilentAuthTimer = setTimeout(function () {
          androidSilentAuthTimer = 0;
          continueAnonymouslyAfterSilentAuth();
        }, 5000);
      }
      return false;
    }
    if (androidSilentAuthTimer) {
      clearTimeout(androidSilentAuthTimer);
      androidSilentAuthTimer = 0;
    }
    resumeAndroidAuth();
    return true;
  }

  window.addEventListener("message", function (event) {
    if (!isAndroidTwa() || event.origin !== "https://shadowsofwar.io") return;
    var ready;
    try {
      ready = JSON.parse(String(event.data || ""));
      if (ready.type !== "sow_bridge_ready") return;
    } catch (e) {
      return;
    }
    var port = event.ports && event.ports[0];
    if (!port) return;
    androidPurchasePort = port;
    androidPurchaseProducts = Array.isArray(ready.products) ? ready.products : null;
    window.dispatchEvent(new CustomEvent("sow:android-purchase-bridge-ready"));
    requestAndroidSilentAuth();
    port.onmessage = function (messageEvent) {
      try {
        androidBridgeMessage(JSON.parse(String(messageEvent.data || "")));
      } catch (e) {}
    };
    if (typeof port.start === "function") port.start();
    port.postMessage(JSON.stringify({ type: "sow_bridge_ack" }));
  });

  window.SOW_isAndroidPurchaseBridgeReady = function () {
    return isAndroidTwa() && !!androidPurchasePort;
  };

  window.SOW_androidPurchaseSupports = function (productId) {
    return window.SOW_isAndroidPurchaseBridgeReady() &&
      Array.isArray(androidPurchaseProducts) && androidPurchaseProducts.indexOf(productId) !== -1;
  };

  window.SOW_requestAndroidPurchase = function (productId, appUserId) {
    if (!window.SOW_isAndroidPurchaseBridgeReady() || !productId || !appUserId ||
        (androidPurchaseProducts && androidPurchaseProducts.indexOf(productId) === -1)) {
      return null;
    }
    var requestId;
    try {
      requestId = window.crypto && typeof window.crypto.randomUUID === "function"
        ? window.crypto.randomUUID()
        : "purchase-" + Date.now() + "-" + Math.random().toString(36).slice(2);
    } catch (e) {
      requestId = "purchase-" + Date.now();
    }
    if (!postAndroidBridgeMessage({
      type: "purchase",
      request_id: requestId,
      product_id: productId,
      app_user_id: appUserId,
    })) return null;
    return requestId;
  };

  window.SOW_requestAndroidRestore = function (appUserId) {
    if (!window.SOW_isAndroidPurchaseBridgeReady() || !appUserId) return null;
    var requestId;
    try {
      requestId = window.crypto && typeof window.crypto.randomUUID === "function"
        ? window.crypto.randomUUID()
        : "restore-" + Date.now() + "-" + Math.random().toString(36).slice(2);
    } catch (e) {
      requestId = "restore-" + Date.now();
    }
    if (!postAndroidBridgeMessage({
      type: "restore",
      request_id: requestId,
      app_user_id: appUserId,
    })) return null;
    return requestId;
  };

  function androidStorage(mode) {
    try {
      if (!arguments.length) return window.localStorage.getItem(ANDROID_MODE_KEY);
      if (mode) window.localStorage.setItem(ANDROID_MODE_KEY, mode);
      else window.localStorage.removeItem(ANDROID_MODE_KEY);
    } catch (e) {}
    return null;
  }

  function androidPending(value) {
    try {
      if (!arguments.length) return sessionStorage.getItem(ANDROID_PENDING_KEY);
      if (value) sessionStorage.setItem(ANDROID_PENDING_KEY, value);
      else sessionStorage.removeItem(ANDROID_PENDING_KEY);
    } catch (e) {}
    return null;
  }

  window.SOW_openAndroidPlayGames = function (section) {
    if (!isAndroidTwa()) {
      return false;
    }
    var allowed = { achievements: true, leaderboards: true };
    if (!allowed[section]) {
      return false;
    }
    window.location.href = "sow://playgames/" + section;
    return true;
  };

  function androidIdentityFromResponse(serverIdentity) {
    if (!serverIdentity || serverIdentity.provider !== "playgames" ||
        !serverIdentity.external_id || !serverIdentity.token) {
      throw new Error("Play Games identity response is invalid");
    }
    return {
      provider: "playgames",
      externalId: serverIdentity.external_id,
      displayName: serverIdentity.display_name || "Player",
      avatarUrl: serverIdentity.avatar_url || null,
      nameLocked: serverIdentity.name_locked === true,
      token: serverIdentity.token,
    };
  }

  window.SOW_isAndroidPlayGamesAuthenticated = function () {
    var identity = window.SOW_PLATFORM_IDENTITY;
    return isAndroidTwa() && identity && identity.provider === "playgames" &&
      !!identity.externalId && !!identity.token;
  };

  function hasWouSession() {
    try {
      return !!window.localStorage.getItem("wou_session_token") && !!window.localStorage.getItem("wou_user_data");
    } catch (e) {
      return false;
    }
  }

  window.SOW_getAuthState = function () {
    var identity = window.SOW_PLATFORM_IDENTITY;
    if (isAndroidTwa()) {
      return {
        platform: "twa",
        provider: identity && identity.provider === "playgames" ? "playgames" : null,
        authenticated: !!(identity && identity.provider === "playgames" && identity.externalId && identity.token),
        pending: !!androidPending(),
      };
    }
    return {
      platform: "web",
      provider: identity && identity.provider ? identity.provider : (hasWouSession() ? "wou" : null),
      authenticated: !!(identity && identity.externalId && identity.token) || hasWouSession(),
      pending: false,
    };
  };

  function clearAndroidAuthParam(params, key) {
    params.delete(key);
    var cleanUrl = window.location.pathname +
      (params.toString() ? "?" + params.toString() : "") + window.location.hash;
    window.history.replaceState({}, document.title, cleanUrl);
  }

  async function pollAndroidPlayGames(base, rendezvousId, timeoutMs) {
    var deadline = Date.now() + (Number(timeoutMs) || 5000);
    while (Date.now() < deadline) {
      try {
        var response = await fetch(
          base + "/auth/playgames/poll?rendezvous_id=" + encodeURIComponent(rendezvousId),
          { headers: { "Accept": "application/json" } }
        );
        if (response.status === 204) {
          await new Promise(function (resolve) { setTimeout(resolve, Math.min(100, Math.max(1, deadline - Date.now()))); });
          continue;
        }
        if (response.status !== 200) {
          return null;
        }
        return androidIdentityFromResponse(await response.json());
      } catch (error) {
        return null;
      }
    }
    return null;
  }

  window.SOW_prepareAndroidAuthState = function () {
    if (!isAndroidTwa()) {
      return;
    }

    var params = new URLSearchParams(window.location.search);
    var rendezvousId = params.get("sow_playgames_rendezvous");
    var anonymousMode = params.get("sow_playgames_mode") === ANDROID_ANONYMOUS_MODE ||
      androidStorage() === ANDROID_ANONYMOUS_MODE;
    var saved = null;
    try {
      saved = sessionStorage.getItem("sow_playgames_identity");
    } catch (e) {}

    if (anonymousMode) {
      try { sessionStorage.removeItem("sow_playgames_identity"); } catch (e) {}
      window.SOW_PLATFORM_IDENTITY = null;
      clearAndroidAuthParam(params, "sow_playgames_mode");
      console.info("Play Games anonymous mode enabled");
      return;
    }

    if (saved) {
      try {
        var cachedIdentity = JSON.parse(saved);
        if (cachedIdentity && cachedIdentity.provider === "playgames" &&
            cachedIdentity.externalId && cachedIdentity.token) {
          window.SOW_PLATFORM_IDENTITY = cachedIdentity;
          return;
        }
      } catch (e) {}
    }

    // The native side waits for the loader-ready bridge message before it
    // checks Play Games. Keep the rendezvous until that handoff completes.
    if (rendezvousId) {
      window.SOW_PLATFORM_IDENTITY = null;
      return;
    }

    window.SOW_PLATFORM_IDENTITY = null;
    console.info("Play Games unavailable; continuing with anonymous identity");
  };

  var androidAuthResumeBusy = false;
  async function resumeAndroidAuth() {
    if (!isAndroidTwa() || androidAuthResumeBusy) return;
    var pending = androidPending();
    if (!pending) return;
    var attempt = ++androidAuthAttempt;
    androidAuthResumeBusy = true;
    try {
      if (pending.indexOf("signin:") === 0 || pending.indexOf("auto:") === 0) {
        var rendezvousId = pending.slice(pending.indexOf(":") + 1);
        var base = String(window.SOW_DATABASE_URL || "/api").replace(/\/$/, "");
        var identity = await pollAndroidPlayGames(base, rendezvousId, pending.indexOf("auto:") === 0 ? 5000 : 30000);
        if (attempt !== androidAuthAttempt) return;
        androidPending(null);
        if (pending.indexOf("auto:") === 0) {
          var params = new URLSearchParams(window.location.search);
          clearAndroidAuthParam(params, "sow_playgames_rendezvous");
        }
        if (identity) {
          window.SOW_PLATFORM_IDENTITY = identity;
          try { sessionStorage.setItem("sow_playgames_identity", JSON.stringify(identity)); } catch (e) {}
          androidStorage("");
          emitAuthStateChange();
          console.info(pending.indexOf("auto:") === 0
            ? "Play Games silent authentication complete"
            : "Play Games interactive authentication complete");
        } else {
          if (pending.indexOf("auto:") === 0) androidStorage(ANDROID_ANONYMOUS_MODE);
          emitAuthStateChange();
          console.info(pending.indexOf("auto:") === 0
            ? "Play Games silent authentication unavailable; continuing anonymously"
            : "Play Games sign-in cancelled or unavailable; continuing anonymously");
        }
      } else if (pending === "signout") {
        androidPending(null);
        window.location.reload();
      }
    } finally {
      androidAuthResumeBusy = false;
    }
  }

  window.SOW_signInAndroidPlayGames = function () {
    if (!isAndroidTwa() || window.SOW_isAndroidPlayGamesAuthenticated()) return false;
    var rendezvousId;
    try {
      rendezvousId = crypto.randomUUID().replace(/-/g, "");
    } catch (e) {
      rendezvousId = String(Date.now()) + String(Math.random()).slice(2);
    }
    androidStorage("");
    androidPending("signin:" + rendezvousId);
    window.location.href = "sow://playgames/signin?rendezvous_id=" + encodeURIComponent(rendezvousId);
    return true;
  };

  window.SOW_startAndroidPlayGamesAutoAuth = function () {
    if (!isAndroidTwa() || androidStorage() === ANDROID_ANONYMOUS_MODE) return false;
    androidLoaderReady = true;
    var rendezvousId = new URLSearchParams(window.location.search).get("sow_playgames_rendezvous");
    if (!rendezvousId) return false;
    var pending = androidPending();
    if (pending) {
      if (pending.indexOf("auto:") === 0 && pending.slice("auto:".length) === rendezvousId) {
        requestAndroidSilentAuth();
        return true;
      }
      if (pending.indexOf("auto:") === 0) androidPending(null);
      else return false;
    }
    androidPending("auto:" + rendezvousId);
    requestAndroidSilentAuth();
    return true;
  };

  window.SOW_signOutAndroidPlayGames = function () {
    if (!isAndroidTwa()) return false;
    androidStorage(ANDROID_ANONYMOUS_MODE);
    try { sessionStorage.removeItem("sow_playgames_identity"); } catch (e) {}
    window.SOW_PLATFORM_IDENTITY = null;
    emitAuthStateChange();
    androidPending("signout");
    window.location.href = "sow://playgames/signout";
    return true;
  };

  document.addEventListener("visibilitychange", function () {
    if (document.visibilityState === "visible") resumeAndroidAuth();
  });
  window.addEventListener("pageshow", resumeAndroidAuth);

  function refreshPortalFlags() {
    var portal = isPortalEmbed();
    var site = isSiteEmbed();
    var crazy = isOnCrazyGames();
    var poki = isOnPoki();
    window.SOW_RUNTIME = {
      portal_embed: portal,
      site_embed: site,
      crazygames: crazy,
      poki: poki,
    };
    window.SOW_isSiteEmbed = site;
    window.SOW_isPortalEmbed = portal;
    window.SOW_isOnCrazyGames = crazy;
    window.SOW_isOnPoki = poki;
    if (portal && "serviceWorker" in navigator) {
      navigator.serviceWorker.getRegistrations().then(function (regs) {
        regs.forEach(function (r) {
          r.unregister();
        });
      });
    }
  }
  window.SOW_refreshPortalFlags = refreshPortalFlags;
  refreshPortalFlags();
  window.SOW_PORTAL_PROGRESS_JSON = null;
  window.SOW_portalAdPause = portalAdPause;
  window.SOW_portalAdResume = portalAdResume;

  window.SOW_portalMuteGameAudio = function () {
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = true;
    });
  };

  window.SOW_portalUnmuteGameAudio = function () {
    if (window.SOW_PORTAL_MUTE_AUDIO || window.SOW_adPlaying) {
      return;
    }
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = false;
    });
  };

  window.SOW_portalUpdateRoom = function (jsonStr) {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (!game || !game.updateRoom) {
      return;
    }
    try {
      var payload = JSON.parse(jsonStr);
      game.updateRoom(payload);
      // Link invites are CrazyGames' own UI: keep their invite button in sync with
      // room joinability. In-game we only surface the room code.
      if (payload.isJoinable && game.showInviteButton && payload.inviteParams) {
        game.showInviteButton(payload.inviteParams);
      } else if (!payload.isJoinable && game.hideInviteButton) {
        game.hideInviteButton();
      }
    } catch (e) {
      console.warn("SOW_portalUpdateRoom parse failed:", e);
    }
  };

  window.SOW_portalLeftRoom = function () {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (game && game.leftRoom) {
      game.leftRoom();
    }
    if (game && game.hideInviteButton) {
      game.hideInviteButton();
    }
  };

  window.SOW_portalClearHostPrivatePending = function () {
    console.log('store_portals: clearing host private pending');
    window.SOW_HOST_PRIVATE_PENDING = false;
  };

  window.SOW_portalClearPendingInvite = function () {
    console.log('store_portals: clearing pending invite');
    window.SOW_PENDING_INVITE_LOBBY_ID = null;
  };

  function loadPortalProgressFromSdk() {
    window.SOW_PORTAL_PROGRESS_JSON = null;
    if (!isOnCrazyGames() || !crazyGamesSdkReady()) {
      return;
    }
    var data = window.CrazyGames.SDK.data;
    if (!data || !data.getItem) {
      return;
    }
    try {
      var raw = data.getItem(SOW_PROGRESS_KEY);
      if (raw) {
        window.SOW_PORTAL_PROGRESS_JSON = raw;
      }
    } catch (e) {
      console.warn("CrazyGames data.getItem failed:", e);
    }
  }

  window.SOW_portalSaveProgress = function (jsonStr) {
    if (!isOnCrazyGames() || !crazyGamesSdkReady()) {
      return;
    }
    var data = window.CrazyGames.SDK.data;
    if (!data || !data.setItem) {
      return;
    }
    try {
      data.setItem(SOW_PROGRESS_KEY, jsonStr);
    } catch (e) {
      console.warn("SOW_portalSaveProgress failed:", e);
    }
  };

  window.SOW_AUTH_CHANGED = false;
  function emitAuthStateChange() {
    window.SOW_AUTH_CHANGED = true;
    try {
      window.dispatchEvent(new CustomEvent("wou:auth-state-change", {
        detail: typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() : null,
      }));
    } catch (e) {}
  }

  window.SOW_portalClearAuthChanged = function () {
    window.SOW_AUTH_CHANGED = false;
  };

  async function crazyGamesIdentityFromUser(user) {
    if (!user || !user.username) {
      return null;
    }
    var token = null;
    try {
      if (window.CrazyGames.SDK.user.getUserToken) {
        token = await window.CrazyGames.SDK.user.getUserToken();
      }
    } catch (e) {
      console.warn("CrazyGames getUserToken failed:", e);
    }
    return {
      provider: "crazygames",
      displayName: user.username,
      externalId: user.userId || user.id || null,
      avatarUrl: user.profilePictureUrl || null,
      nameLocked: true,
      token: token,
    };
  }

  window.SOW_startWouOAuth = function (provider) {
    var allowed = { google: true, discord: true, twitter: true, meta: true };
    provider = allowed[provider] ? provider : "google";
    try {
      var returnTo = window.location.href.split("#")[0];
      var stateObj = { returnTo: returnTo, accountId: "", provider: provider };
      var statePayload = "";
      try {
        statePayload = btoa(unescape(encodeURIComponent(JSON.stringify(stateObj))))
          .replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
      } catch (e) {
        statePayload = encodeURIComponent(JSON.stringify(stateObj));
      }
      try { sessionStorage.setItem("wou_oauth_provider", provider); } catch (e) {}
      var targetUrl = "https://id.worldofunreal.com/api/v1/auth/oauth/login/" +
        encodeURIComponent(provider) +
        "?redirect_uri=" + encodeURIComponent("https://worldofunreal.com/auth/callback") +
        "&state=" + encodeURIComponent(statePayload);
      window.location.href = targetUrl;
      return true;
    } catch (e) {
      console.warn("WOU login redirect failed:", e);
      return false;
    }
  };

  window.SOW_portalShowAuthPrompt = async function () {
    if (isAndroidTwa()) {
      window.SOW_signInAndroidPlayGames();
      return;
    }
    if (crazyGamesSdkReady() && window.CrazyGames.SDK.user && window.CrazyGames.SDK.user.showAuthPrompt) {
      try {
        var user = await window.CrazyGames.SDK.user.showAuthPrompt();
        var identity = await crazyGamesIdentityFromUser(user);
        if (identity) {
          window.SOW_PLATFORM_IDENTITY = identity;
          emitAuthStateChange();
          console.log("Auth prompt successful login:", user.username);
        }
      } catch (e) {
        console.warn("Auth prompt failed or cancelled:", e);
      }
      return;
    }
    window.SOW_startWouOAuth(window.SOW_WOU_PROVIDER || "google");
  };

  window.SOW_signOutWou = function () {
    if (isAndroidTwa()) return window.SOW_signOutAndroidPlayGames();
    try {
      window.localStorage.removeItem("wou_session_token");
      window.localStorage.removeItem("wou_user_data");
    } catch (e) {}
    if (window.SOW_PLATFORM_IDENTITY && window.SOW_PLATFORM_IDENTITY.provider === "wou") {
      window.SOW_PLATFORM_IDENTITY = null;
    }
    emitAuthStateChange();
    window.location.reload();
    return true;
  };

  window.SOW_portalSignOut = function () {
    if (isAndroidTwa()) {
      window.SOW_signOutAndroidPlayGames();
    } else {
      window.SOW_signOutWou();
    }
  };

  // Capture a returning WOU SSO login (?session_token&account=) into the
  // same localStorage keys the Rust client reads. Runs at shell boot.
  function sowCaptureWouReturn() {
    try {
      var params = new URLSearchParams(window.location.search);
      var token = params.get("session_token");
      var acc = params.get("account");
      if (!token || !acc) return;
      var account = JSON.parse(decodeURIComponent(acc));
      window.localStorage.setItem("wou_session_token", token);
      window.localStorage.setItem("wou_user_data", JSON.stringify(account));
      emitAuthStateChange();
      params.delete("session_token");
      params.delete("account");
      var clean = window.location.pathname + (params.toString() ? "?" + params.toString() : "") + window.location.hash;
      window.history.replaceState({}, document.title, clean);
      console.log("WOU login captured for:", account.display_name || account.id);
    } catch (e) {
      console.warn("WOU login capture failed:", e);
    }
  }

  // Block list cache for conduct enforcement (profiles/search filtering).
  // Loaded at boot when an anonymous identity exists; refreshed on report.
  window.SOW_BLOCKED_IDS = [];
  function sowLoadBlocks() {
    try {
      var accountId = window.localStorage.getItem("sow_account_id");
      var secret = window.localStorage.getItem("sow_account_secret");
      if (!accountId || !secret) return;
      var base = String(window.SOW_DATABASE_URL || "/api").replace(/\/$/, "");
      fetch(base + "/profile/anonymous/blocks", {
        method: "POST",
        headers: { "Content-Type": "application/json", "Accept": "application/json" },
        body: JSON.stringify({ account_id: accountId, auth_secret: secret })
      }).then(function (res) {
        if (!res.ok) throw new Error("blocks unavailable");
        return res.json();
      }).then(function (data) {
        window.SOW_BLOCKED_IDS = Array.isArray(data.blocked_ids) ? data.blocked_ids : [];
      }).catch(function (e) {
        console.warn("Block list load failed:", e);
      });
    } catch (e) {
      console.warn("Block list load failed:", e);
    }
  }

  window.SOW_isBlockedId = function (id) {
    if (!id) return false;
    return (window.SOW_BLOCKED_IDS || []).indexOf(String(id)) !== -1;
  };

  try {
    sowCaptureWouReturn();
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", sowLoadBlocks);
    } else {
      sowLoadBlocks();
    }
  } catch (e) {}

  window.SOW_LINK_PROMPT_RESPONSE = null;
  window.SOW_portalShowAccountLinkPrompt = async function () {
    if (!crazyGamesSdkReady() || !window.CrazyGames.SDK.user || !window.CrazyGames.SDK.user.showAccountLinkPrompt) {
      return;
    }
    try {
      var res = await window.CrazyGames.SDK.user.showAccountLinkPrompt();
      window.SOW_LINK_PROMPT_RESPONSE = res.response; // "yes" or "no"
    } catch (e) {
      console.warn("Account link prompt failed:", e);
      window.SOW_LINK_PROMPT_RESPONSE = "error";
    }
  };

  window.SOW_portalClearAccountLinkPrompt = function () {
    window.SOW_LINK_PROMPT_RESPONSE = null;
  };

  window.SOW_portalConsumeBootIntent = function () {
    window.SOW_PENDING_INVITE_LOBBY_ID = null;
    window.SOW_HOST_PRIVATE_PENDING = false;
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (!game) {
      return;
    }
    var coldInvite = parseInviteLobbyId(game.inviteParams);
    if (coldInvite !== null) {
      console.log("CrazyGames cold-start invite lobby:", coldInvite);
      queueInviteLobbyId(coldInvite);
      return;
    }
    if (game.isInstantMultiplayer) {
      console.log("CrazyGames instant multiplayer: host private lobby");
      window.SOW_HOST_PRIVATE_PENDING = true;
    }
  };

  window.SOW_initPortalSdk = async function () {
    if (!isOnCrazyGames()) {
      return;
    }
    if (!(window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.init)) {
      console.warn("CrazyGames SDK script not loaded");
      return;
    }
    try {
      await window.CrazyGames.SDK.init();
      refreshPortalFlags();
      var env = window.CrazyGames.SDK.environment;
      console.log("CrazyGames SDK init OK (env=" + env + ")");

      if (crazyGamesSdkReady() && window.CrazyGames.SDK.user) {
        try {
          var sysInfo = window.CrazyGames.SDK.user.systemInfo;
          if (sysInfo && sysInfo.locale) {
            window.SOW_PORTAL_LOCALE = sysInfo.locale;
            console.log("CrazyGames system locale detected: " + sysInfo.locale);
          }
        } catch (e) {
          console.warn("CrazyGames systemInfo reading failed:", e);
        }
        try {
          if (window.CrazyGames.SDK.user.isUserAccountAvailable) {
            var user = await window.CrazyGames.SDK.user.getUser();
            var identity = await crazyGamesIdentityFromUser(user);
            if (identity) {
              window.SOW_PLATFORM_IDENTITY = identity;
              emitAuthStateChange();
              console.log("CrazyGames user:", user.username);
            }
          }
          if (window.CrazyGames.SDK.user.addAuthListener) {
            window.CrazyGames.SDK.user.addAuthListener(async function (u) {
              var identity = await crazyGamesIdentityFromUser(u);
              if (identity) {
                window.SOW_PLATFORM_IDENTITY = identity;
                emitAuthStateChange();
                console.log("CrazyGames user changed via AuthListener:", u.username);
              }
            });
          }
        } catch (e) {
          console.warn("CrazyGames getUser failed:", e);
        }
      }

      if (crazyGamesSdkReady() && window.CrazyGames.SDK.ad && window.CrazyGames.SDK.ad.hasAdblock) {
        try {
          window.SOW_hasAdblock = await window.CrazyGames.SDK.ad.hasAdblock();
          console.log("CrazyGames adblock check:", window.SOW_hasAdblock);
        } catch (e) {
          console.warn("CrazyGames hasAdblock failed:", e);
        }
      }

      if (crazyGamesSdkReady()) {
        var game = crazyGameApi();
        if (game && game.settings) {
          applyGameSettings(game.settings);
        }
        if (game && game.addSettingsChangeListener) {
          game.addSettingsChangeListener(applyGameSettings);
        }
        installJoinRoomListener();
        installInviteLinkHappytime();
        window.SOW_portalConsumeBootIntent();
        loadPortalProgressFromSdk();
      }
    } catch (e) {
      console.warn("CrazyGames SDK init failed:", e);
    }
  };

  window.SOW_portalGameplayStart = function (isRetry) {
    console.log("SOW gameplayStart called" + (isRetry ? " (retry)" : ""));
    if (isSiteEmbed() || window.SOW_adPlaying) {
      return;
    }
    // Off-portal (site or standalone embed): no CrazyGames/Poki machinery.
    if (!isOnCrazyGames() && !isOnPoki() && typeof PokiSDK === "undefined") {
      return;
    }
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStart) {
      PokiSDK.gameplayStart();
    }
    if (!crazyGamesSdkReady()) {
      if (!isRetry) {
        console.log("CrazyGames SDK not ready for gameplayStart yet, scheduling retry...");
        requestAnimationFrame(function () {
          window.SOW_portalGameplayStart(true);
        });
      } else {
        console.warn("CrazyGames SDK still not ready on gameplayStart retry.");
      }
      return;
    }
    const game = crazyGameApi();
    if (game) {
      if (game.gameplayStart) {
        console.log("Calling CrazyGames gameplayStart");
        game.gameplayStart();
      } else if (game.play) {
        console.log("Calling CrazyGames play");
        game.play();
      }
    }
  };

  function portalAdsEnabled() {
    return window.SOW_ENABLE_PORTAL_ADS === true;
  }

  function requestCrazyGamesMidgameAd() {
    if (!portalAdsEnabled() || !crazyGamesSdkReady() || window.SOW_adPlaying) {
      return;
    }
    var ad = window.CrazyGames.SDK.ad;
    if (!ad || !ad.requestAd) {
      return;
    }
    var callbacks = {
      adStarted: function () {
        portalAdPause();
      },
      adFinished: function () {
        portalAdResume();
      },
      adError: function (error) {
        console.warn("CrazyGames midgame ad error:", error);
        portalAdResume();
      },
    };
    try {
      ad.requestAd("midgame", callbacks);
    } catch (e) {
      console.warn("CrazyGames requestAd failed:", e);
    }
  }

  window.SOW_portalGameplayStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStop) {
      PokiSDK.gameplayStop();
    }
    if (crazyGamesSdkReady()) {
      const game = crazyGameApi();
      if (game) {
        if (game.gameplayStop) {
          game.gameplayStop();
        } else if (game.pause) {
          game.pause();
        }
      }
      requestCrazyGamesMidgameAd();
    }
  };

  window.SOW_portalLoadStart = function () {
    if (isSiteEmbed()) {
      return;
    }
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStart) {
      PokiSDK.loadStart();
    }
    if (!crazyGamesSdkReady()) {
      return;
    }
    const game = crazyGameApi();
    if (!game) {
      return;
    }
    if (game.loadingStart) {
      game.loadingStart();
    } else if (game.sdkGameLoadingStart) {
      game.sdkGameLoadingStart();
    }
  };

  window.SOW_portalLoadStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStop) {
      PokiSDK.loadStop();
    }
    if (!crazyGamesSdkReady()) {
      return;
    }
    const game = crazyGameApi();
    if (!game) {
      return;
    }
    if (game.loadingStop) {
      game.loadingStop();
    } else if (game.sdkGameLoadingStop) {
      game.sdkGameLoadingStop();
    }
  };

  window.SOW_portalHappytime = function () {
    triggerHappytime();
  };

  document.addEventListener(
    "keydown",
    function (e) {
      if (isSiteEmbed() || !document.fullscreenElement) {
        return;
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
      }
    },
    true
  );

  // CrazyGames common fixes: block page scroll and browser context menu outside the canvas.
  window.addEventListener(
    "wheel",
    function (e) {
      // The menu owns its internal scrolling (Store, Profile, Browser, etc.).
      // Only suppress wheel scrolling while the fullscreen game surface is active.
      if (e.target && e.target.closest && e.target.closest("#sow-menu")) {
        return;
      }
      e.preventDefault();
    },
    { passive: false }
  );

  document.addEventListener("contextmenu", function (e) {
    if (e.target && e.target.id === "blade") {
      return;
    }
    e.preventDefault();
  });

  document.addEventListener(
    "keydown",
    function (e) {
      if (e.key !== "ArrowUp" && e.key !== "ArrowDown" && e.key !== " ") {
        return;
      }
      var tag = e.target && e.target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || (e.target && e.target.isContentEditable)) {
        return;
      }
      e.preventDefault();
    },
    true
  );

  enforceCrazyGamesSitelock();
})();
