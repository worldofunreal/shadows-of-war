// Minimal Poki runtime bridge. This file is copied to the Poki artifact as
// sdk/store_portals.js; it intentionally contains no account, store, or
// CrazyGames integration.
(function () {
    "use strict";

    let sdk = null;
    let sdkInitPromise = null;
    let loadingFinished = false;
    let gameplayReady = false;
    let gameplayActive = false;
    let firstInputSeen = false;
    // A match was played since the last break. Poki's commercialBreak runs
    // before the player's next match starts, never inside a running match:
    // multiplayer keeps going while an ad plays.
    let breakPending = false;
    let breakRunning = false;
    let inputInstalled = false;
    const progressKey = "sow_player_progress";
    let adFocus = null;

    function pokiSdk() {
        return window.PokiSDK || null;
    }

    function safeLocalGet(key) {
        try {
            return window.localStorage.getItem(key);
        } catch {
            return null;
        }
    }

    function safeLocalSet(key, value) {
        try {
            window.localStorage.setItem(key, value);
        } catch {}
    }

    function syncWasmAudio() {
        if (typeof window.SOW_setPortalAudioMuted !== "function") return;
        try {
            window.SOW_setPortalAudioMuted(!!(window.SOW_adPlaying || window.SOW_PORTAL_MUTE_AUDIO));
        } catch {}
    }

    function portalAdPause() {
        window.SOW_adPlaying = true;
        syncWasmAudio();
        adFocus = document.activeElement || null;
        document.querySelectorAll("audio,video").forEach(function (el) {
            el.dataset.sowWasPaused = el.paused ? "1" : "0";
            el.dataset.sowWasMuted = el.muted ? "1" : "0";
            el.pause();
            el.muted = true;
        });
    }

    function portalAdResume() {
        window.SOW_adPlaying = false;
        syncWasmAudio();
        document.querySelectorAll("audio,video").forEach(function (el) {
            el.muted = window.SOW_PORTAL_MUTE_AUDIO || el.dataset.sowWasMuted === "1";
            if (el.dataset.sowWasPaused !== "1") el.play().catch(function () {});
            delete el.dataset.sowWasPaused;
            delete el.dataset.sowWasMuted;
        });
        const focus = adFocus;
        adFocus = null;
        if (
            focus &&
            focus !== document.body &&
            typeof focus.focus === "function" &&
            typeof document.contains === "function" &&
            document.contains(focus)
        ) {
            focus.focus();
        }
    }

    function muteGameAudio() {
        window.SOW_PORTAL_MUTE_AUDIO = true;
        syncWasmAudio();
        document.querySelectorAll("audio,video").forEach(function (el) {
            el.muted = true;
        });
    }

    function unmuteGameAudio() {
        window.SOW_PORTAL_MUTE_AUDIO = false;
        syncWasmAudio();
        if (window.SOW_adPlaying) return;
        document.querySelectorAll("audio,video").forEach(function (el) {
            el.muted = false;
        });
    }

    function startGameplay() {
        if (!gameplayReady || gameplayActive || !sdk || typeof sdk.gameplayStart !== "function") return;
        gameplayActive = true;
        try {
            sdk.gameplayStart();
        } catch {}
    }

    // Runs the pending commercial break (if any) and then the player's match
    // command. Called by the menu for commands that start or join a match.
    function beforeMatch(proceed) {
        if (breakRunning) return;
        if (!breakPending || !sdk || typeof sdk.commercialBreak !== "function") {
            breakPending = false;
            proceed();
            return;
        }
        breakPending = false;
        breakRunning = true;
        portalAdPause();
        const finish = function () {
            breakRunning = false;
            portalAdResume();
            proceed();
        };
        try {
            Promise.resolve(sdk.commercialBreak()).then(finish, function (error) {
                console.warn("Poki commercialBreak failed:", error);
                finish();
            });
        } catch (error) {
            console.warn("Poki commercialBreak failed:", error);
            finish();
        }
    }

    function stopGameplayForInterruption() {
        if (!gameplayActive) return;
        gameplayActive = false;
        if (!sdk || typeof sdk.gameplayStop !== "function") return;
        try {
            sdk.gameplayStop();
        } catch {}
    }

    // Rewarded video. Resolves true only when Poki reports the video played.
    function rewardedBreak() {
        if (breakRunning || window.SOW_adPlaying || !sdk || typeof sdk.rewardedBreak !== "function") {
            return Promise.resolve(false);
        }
        breakRunning = true;
        portalAdPause();
        const finish = function (success) {
            breakRunning = false;
            portalAdResume();
            return success === true;
        };
        try {
            return Promise.resolve(sdk.rewardedBreak()).then(finish, function (error) {
                console.warn("Poki rewardedBreak failed:", error);
                return finish(false);
            });
        } catch (error) {
            console.warn("Poki rewardedBreak failed:", error);
            return Promise.resolve(finish(false));
        }
    }

    function onFirstInput() {
        if (!gameplayReady || firstInputSeen || window.SOW_adPlaying) return;
        firstInputSeen = true;
        startGameplay();
    }

    function blockGameInputDuringAd(event) {
        if (!window.SOW_adPlaying) return;
        const target = event.target;
        const gameSurface = target && target.closest && target.closest("canvas, #sow-menu");
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

    document.addEventListener("visibilitychange", function () {
        if (document.hidden) {
            stopGameplayForInterruption();
        } else if (gameplayReady && firstInputSeen) {
            startGameplay();
        }
    });

    function refreshPortalFlags() {
        window.SOW_RUNTIME = { portal_embed: true, site_embed: false, crazygames: false, poki: true };
        window.SOW_isSiteEmbed = false;
        window.SOW_isPortalEmbed = function () {
            return true;
        };
        window.SOW_isOnCrazyGames = function () {
            return false;
        };
        window.SOW_isOnPoki = function () {
            return true;
        };
        window.SOW_DISABLE_CHAT = true;
        window.SOW_PORTAL_PROGRESS_JSON = safeLocalGet(progressKey);
    }

    function measure(category, what, action) {
        if (window.SOW_adPlaying || breakRunning) return;
        if (!sdk || typeof sdk.measure !== "function") return;
        if (arguments.length === 1) {
            const parts = String(category || "").split("|");
            category = parts[0] || "gameplay";
            what = parts[1] || "event";
            action = parts[2] || "interact";
        }
        try {
            sdk.measure(String(category), String(what), String(action));
        } catch {}
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

    window.SOW_isAndroidTwa = function () {
        return false;
    };

    window.SOW_portalMuteGameAudio = muteGameAudio;
    window.SOW_portalUnmuteGameAudio = unmuteGameAudio;
    window.SOW_portalAdPause = portalAdPause;
    window.SOW_portalAdResume = portalAdResume;
    window.SOW_portalConsumeBootIntent = function () {};
    window.SOW_portalClearPendingInvite = function () {
        window.SOW_PENDING_INVITE_LOBBY_ID = null;
    };
    window.SOW_portalClearHostPrivatePending = function () {
        window.SOW_HOST_PRIVATE_PENDING = false;
    };
    window.SOW_portalUpdateRoom = function () {};
    window.SOW_portalLeftRoom = function () {};
    window.SOW_portalHappytime = function () {};
    window.SOW_portalSaveProgress = function (json) {
        window.SOW_PORTAL_PROGRESS_JSON = String(json || "");
        safeLocalSet(progressKey, window.SOW_PORTAL_PROGRESS_JSON);
    };

    window.SOW_portalBeforeMatch = beforeMatch;
    window.SOW_portalRewardedAvailable = function () {
        return !!(sdk && typeof sdk.rewardedBreak === "function") && !window.SOW_adPlaying && !breakRunning;
    };
    window.SOW_portalRewardedBreak = rewardedBreak;

    window.SOW_pokiMeasure = measure;
    window.SOW_pokiOpenExternalLink = function (url) {
        if (sdk && typeof sdk.openExternalLink === "function") sdk.openExternalLink(String(url));
    };

    window.SOW_initPortalSdk = async function () {
        if (sdkInitPromise) return sdkInitPromise;
        refreshPortalFlags();
        sdk = pokiSdk();
        installFirstInputListener();
        if (!sdk || typeof sdk.init !== "function") {
            console.warn("Poki SDK unavailable; continuing anonymously");
            sdkInitPromise = Promise.resolve();
            return sdkInitPromise;
        }
        sdkInitPromise = Promise.resolve()
            .then(function () {
                return sdk.init();
            })
            .catch(function (error) {
                console.warn("Poki SDK init failed; continuing anonymously:", error);
            });
        return sdkInitPromise;
    };

    window.SOW_portalGameLoadingFinished = function () {
        if (loadingFinished) return;
        loadingFinished = true;
        if (sdk && typeof sdk.gameLoadingFinished === "function") {
            try {
                sdk.gameLoadingFinished();
            } catch {}
        }
    };

    window.SOW_portalGameplayStart = function () {
        if (gameplayReady) return;
        gameplayReady = true;
        installFirstInputListener();
    };

    window.SOW_portalGameplayStop = function () {
        if (!gameplayReady && !gameplayActive) return;
        gameplayReady = false;
        firstInputSeen = false;
        if (gameplayActive) breakPending = true;
        if (gameplayActive && sdk && typeof sdk.gameplayStop === "function") {
            try {
                sdk.gameplayStop();
            } catch {}
        }
        gameplayActive = false;
    };

    window.SOW_portalLoadStart = function () {};
    window.SOW_portalLoadStop = window.SOW_portalGameLoadingFinished;

    refreshPortalFlags();
})();
