// Minimal Jest runtime bridge. This file is copied to the Jest artifact as
// sdk/store_portals.js; it intentionally contains no account, store,
// CrazyGames, or Poki integration. Jest has no ads: this bridge never
// pauses for commercial breaks and never calls gameplayStart/Stop style ad
// hooks.
(function () {
    "use strict";

    let sdkInitPromise = null;
    let loadingFinished = false;
    let sdkReady = false;
    let entrySource = null;
    let firstMilestonePending = false;
    let firstMilestoneMarked = false;
    const progressKey = "sow_player_progress";
    const ENTRY_SOURCE_TYPES = [
        "explore",
        "home",
        "profile",
        "chat",
        "notification",
        "jest_ad",
        "external_ad",
        "referral",
        "direct",
        "unknown",
    ];
    const SOURCE_ATTRIBUTED_EVENTS = new Set([
        "boot_start",
        "tutorial_start",
        "tutorial_step",
        "matchmaking_joined",
        "matchmaking_cancelled",
        "lobby_joined",
        "lobby_join_failed",
        "match_loading_start",
        "match_exit",
        "campaign_episode_complete",
        "match_started_client",
        "match_ended_client",
    ]);
    const FIRST_MILESTONE_EVENTS = new Set([
        "tutorial_start",
        "campaign_episode_complete",
        "match_started_client",
        "match_ended_client",
    ]);

    // D1-D7 re-engagement sequence (launch checklist: at least one per day
    // once the user is registered). Stable identifiers make re-scheduling
    // idempotent: scheduling the same identifier replaces the old one.
    const REENGAGE = [
        {
            id: "sow_reengage_d0",
            days: 0,
            body: "Battle stations: your next strategy match is ready when you are.",
            cta: "Play now",
        },
        {
            id: "sow_reengage_d1",
            days: 1,
            body: "Choose another commander and start a fresh battle.",
            cta: "Play now",
        },
        {
            id: "sow_reengage_d2",
            days: 2,
            body: "New strategy battles are ready whenever you are.",
            cta: "Play now",
        },
        {
            id: "sow_reengage_d3",
            days: 3,
            body: "Claim territory in your next Shadows of War match.",
            cta: "Play now",
        },
        {
            id: "sow_reengage_d4",
            days: 4,
            body: "Try a different commander in your next strategy battle.",
            cta: "Play now",
        },
        {
            id: "sow_reengage_d5",
            days: 5,
            body: "Can you outmaneuver your rivals in the next battle?",
            cta: "Play now",
        },
        {
            id: "sow_reengage_d6",
            days: 6,
            body: "Return to the map and fight for more territory.",
            cta: "Play now",
        },
        {
            id: "sow_reengage_d7",
            days: 7,
            body: "Ready for another battle? Your commanders are waiting.",
            cta: "Play now",
        },
    ];

    function jestSdk() {
        return window.JestSDK || null;
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
            return true;
        } catch {}
        return false;
    }

    function safeLocalRemove(key) {
        try {
            window.localStorage.removeItem(key);
        } catch {}
    }

    function progressKeyFor(playerId) {
        return progressKey + ":jest:" + encodeURIComponent(playerId);
    }

    function loadPlayerProgress() {
        const player = currentPlayer();
        if (!player || typeof player.playerId !== "string" || !player.playerId) {
            window.SOW_PORTAL_PROGRESS_JSON = null;
            return;
        }
        const key = progressKeyFor(player.playerId);
        let progress = safeLocalGet(key);
        if (!progress) {
            const legacyProgress = safeLocalGet(progressKey);
            if (legacyProgress && safeLocalSet(key, legacyProgress)) {
                safeLocalRemove(progressKey);
                progress = legacyProgress;
            }
        }
        window.SOW_PORTAL_PROGRESS_JSON = progress;
    }

    function currentPlayer() {
        const sdk = jestSdk();
        if (!sdk || typeof sdk.getPlayer !== "function") return null;
        try {
            return sdk.getPlayer() || null;
        } catch {
            return null;
        }
    }

    function scheduleReengagement() {
        const sdk = jestSdk();
        if (!sdk || !sdk.notifications || typeof sdk.notifications.scheduleNotification !== "function")
            return;
        const player = currentPlayer();
        // The platform discards guest notifications; only registered users qualify.
        if (!player || !player.registered) return;
        REENGAGE.forEach(function (item) {
            try {
                sdk.notifications.scheduleNotification({
                    body: item.body,
                    ctaText: item.cta,
                    identifier: item.id,
                    priority: "medium",
                    scheduledInDays: item.days,
                });
            } catch {}
        });
    }

    function refreshEntrySource() {
        const sdk = jestSdk();
        if (!sdk || typeof sdk.getEntrySource !== "function") return;
        try {
            const source = sdk.getEntrySource();
            entrySource = source && ENTRY_SOURCE_TYPES.includes(source.type) ? source.type : "unknown";
        } catch {
            entrySource = "unknown";
        }
    }

    function markFirstMilestone() {
        firstMilestonePending = true;
        if (firstMilestoneMarked || !sdkReady) return;
        const sdk = jestSdk();
        if (!sdk || typeof sdk.markFirstMilestone !== "function") return;
        try {
            sdk.markFirstMilestone();
            firstMilestoneMarked = true;
            firstMilestonePending = false;
        } catch (error) {
            console.warn("Jest first milestone could not be marked:", error);
        }
    }

    function refreshPortalFlags() {
        window.SOW_RUNTIME = {
            portal_embed: true,
            site_embed: false,
            crazygames: false,
            poki: false,
            jest: true,
        };
        window.SOW_isSiteEmbed = false;
        window.SOW_isPortalEmbed = function () {
            return true;
        };
        window.SOW_isOnCrazyGames = function () {
            return false;
        };
        window.SOW_isOnPoki = function () {
            return false;
        };
        window.SOW_isOnJest = function () {
            return true;
        };
        window.SOW_DISABLE_CHAT = true;
        window.SOW_PORTAL_PROGRESS_JSON = null;
        syncPlatformIdentity();
    }

    function syncPlatformIdentity() {
        const player = currentPlayer();
        if (player && player.playerId) {
            window.SOW_PLATFORM_IDENTITY = {
                provider: "jest",
                displayName: player.username || "Player",
                externalId: player.playerId,
                registered: player.registered === true,
                avatarUrl: player.avatarUrl || null,
                token: null,
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
    window.SOW_AUTH_CHANGED = false;
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
    // No ads on Jest: pause/resume exist only so shared callers stay safe.
    window.SOW_portalAdPause = function () {};
    window.SOW_portalAdResume = function () {};
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
        const player = currentPlayer();
        if (player && typeof player.playerId === "string" && player.playerId) {
            safeLocalSet(progressKeyFor(player.playerId), window.SOW_PORTAL_PROGRESS_JSON);
        }
    };
    window.SOW_portalClearAuthChanged = function () {
        window.SOW_AUTH_CHANGED = false;
    };

    // Registration entry point for the menu (settings save-progress button).
    // Uses the platform default popup; resolves immediately when registered.
    window.SOW_jestLogin = function () {
        const sdk = jestSdk();
        if (!sdk || typeof sdk.login !== "function") return Promise.resolve();
        const player = currentPlayer();
        if (!sdkReady || !player || player.registered !== false) return Promise.resolve();
        const done = function () {
            const player = currentPlayer();
            syncPlatformIdentity();
            if (!player || !player.registered) return;
            window.SOW_AUTH_CHANGED = true;
            try {
                if (typeof window.dispatchEvent === "function" && typeof window.Event === "function") {
                    window.dispatchEvent(new window.Event("sow:platform-identity-changed"));
                }
            } catch {}
            scheduleReengagement();
        };
        try {
            return Promise.resolve(sdk.login({ entryPayload: { source: "settings_save_progress" } })).then(
                done,
                done,
            );
        } catch {
            return Promise.resolve();
        }
    };

    window.SOW_initPortalSdk = async function () {
        if (sdkInitPromise) return sdkInitPromise;
        refreshPortalFlags();
        const sdk = jestSdk();
        if (!sdk || typeof sdk.init !== "function") {
            console.warn("Jest SDK unavailable; continuing anonymously");
            sdkInitPromise = Promise.resolve();
            return sdkInitPromise;
        }
        sdkInitPromise = Promise.resolve()
            .then(function () {
                return sdk.init();
            })
            .then(function () {
                sdkReady = true;
                if (sdk.lifecycle) {
                    try {
                        if (typeof sdk.lifecycle.onHide === "function") sdk.lifecycle.onHide(muteGameAudio);
                        if (typeof sdk.lifecycle.onShow === "function") sdk.lifecycle.onShow(unmuteGameAudio);
                    } catch {}
                }
                syncPlatformIdentity();
                loadPlayerProgress();
                refreshEntrySource();
                scheduleReengagement();
                if (firstMilestonePending) markFirstMilestone();
            })
            .catch(function (error) {
                console.warn("Jest SDK init failed; continuing anonymously:", error);
            });
        return sdkInitPromise;
    };

    window.SOW_portalGameLoadingFinished = function () {
        if (loadingFinished) return;
        loadingFinished = true;
        const sdk = jestSdk();
        if (sdk && typeof sdk.markGameLoaded === "function") {
            try {
                sdk.markGameLoaded();
            } catch {}
        }
    };

    window.SOW_portalCaptureExperienceEvent = function (event) {
        const sdk = jestSdk();
        if (!sdk || typeof sdk.captureEvent !== "function" || !event || typeof event.name !== "string")
            return;
        const props = Object.assign({}, event.props || {});
        const completedTutorialStep =
            event.name === "tutorial_step" && event.props && event.props.action === "complete";
        if ((SOURCE_ATTRIBUTED_EVENTS.has(event.name) || completedTutorialStep) && entrySource) {
            props.entry_source = entrySource;
        }
        if (completedTutorialStep) {
            markFirstMilestone();
        } else if (FIRST_MILESTONE_EVENTS.has(event.name)) {
            markFirstMilestone();
        }
        try {
            sdk.captureEvent(event.name, Object.keys(props).length ? props : undefined);
        } catch (error) {
            console.warn("Jest analytics event could not be captured:", error);
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
