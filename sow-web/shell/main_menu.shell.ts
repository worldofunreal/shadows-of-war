import menuFlowApi from "./main_menu.flow.ts";
import {
    MAIN_NAV_ITEMS,
    avatarImage,
    currencyAsset,
    currentLeaderSelection,
    currentScreen,
    esc,
    findLobby,
    heroImage,
    leaderById,
    leaderDisplayName,
    menuIcon,
    menuUi,
    portalOffersSignIn,
    profileHeroImage,
    reconcileLeaderSelection,
    renderKey,
    root,
    selfCreds,
    send,
    stageLeaderSelection,
} from "./main_menu.core.ts";
import {
    refreshHeroesRoster,
    renderHeroInfoModal,
    renderHeroes,
    selectDropdown,
    setDropdownOpen,
    syncDropdowns,
    syncHeroesRotationCountdown,
    updateHeroesPreview,
} from "./main_menu.heroes.ts";
import {
    cloneConfig,
    joinedLobby,
    lobbyLabel,
    lobbyPlayerCount,
    lobbyStatusText,
    renderBrowser,
    renderCampaign,
    renderCreate,
    renderHome,
    renderPasswordModal,
    renderQueue,
    syncCreateDraft,
    updateLobbyViews,
    updateQueueRoster,
} from "./main_menu.lobbies.ts";
import { sowScreenMotion } from "./main_menu.motion.ts";
import {
    activeProfileEntry,
    invalidateProfileCache,
    loadMoreProfileHistory,
    loadProfile,
    loadRankedLeaderboard,
    loadVictoryLeaderboard,
    openProfile,
    profileApi,
    renderProfile,
    renderProfileDetail,
    syncProfileDom,
    syncProfilePreload,
} from "./main_menu.profile.ts";
import {
    abandonExternalPurchaseAttempt,
    beginStorePurchase,
    beginStoreRestore,
    canDismissPurchaseModal,
    completeExternalPurchase,
    currentExternalPurchaseAttempt,
    dismissPurchaseModal,
    isAndroidTwa,
    loadStoreCatalog,
    openLeaderPurchase,
    openSkinPurchase,
    renderAuthModal,
    renderMainNav,
    renderMenuAttribution,
    renderPurchaseModal,
    renderSettings,
    renderSkinPickerModal,
    renderStore,
    requestAuthOtp,
    resetAuthFlow,
    resolvePurchaseModal,
    resumeExternalPurchaseDelivery,
    verifyAuthOtp,
} from "./main_menu.surfaces.ts";
import type { MenuRewardReceipt, MenuState, RankedResult } from "./main_menu.state.ts";

// Shared menu chrome, overlays, lifecycle, events, and state bridge.

// ── Conduct & privacy (Terms/Privacy enforcement in-client) ──
export const REPORT_REASONS = [
    ["cheating", "profile.cheating"],
    ["harassment", "profile.harassment"],
    ["hate_speech", "profile.hate_speech"],
    ["threats", "profile.threats"],
    ["spam", "profile.spam"],
    ["inappropriate_name", "profile.inappropriate_name"],
    ["exploiting", "profile.exploiting"],
    ["other", "profile.other_explain"],
];

export let reportBusy = false;
let latestLoaderCycleId = -1;
let completedLoaderCycleId = -1;

let deleteBusy = false;
let deleteError = "";
let activeMenuPointerId: number | null = null;
let renderPending = false;
let renderFlushTimer: number | null = null;
let disconnectNoticeTimer: number | null = null;
let disconnectNoticeDismissed = false;
let disconnectNoticeVisible = false;
let systemNoticeReturnFocus: HTMLElement | null = null;
let rewardAnimationTimer: number | null = null;
let rewardAckRetryTimer: number | null = null;
let rewardAnimationRunning = false;
let rewardAnimationAccount: string | null = null;
const rewardAnimationShown = new Set<string>();
let rewardAckTimes: Record<string, number> = Object.create(null);
export let rewardAnimationToken = 0;
let activeRewardStage: HTMLElement | null = null;
let rankedResultDismissedKey: string | null = null;
let rewardPresentationReady = false;
const menuFlow = menuFlowApi.create();
window.SOW_menu_flow = menuFlow;
let lastMenuFlowPhase: string | null = null;
interface ProgressionValues {
    xp: number;
    level: number;
    crowns: number;
    laurels: number;
    gems: number;
}

let displayedProgression: ProgressionValues | null = null;
let progressionAnimationFrame = 0;
let progressionAnimationTarget: ProgressionValues | null = null;

interface MenuMusicCrossfade {
    outgoing: number;
    incoming: number;
    progress: number;
    startedAt: number | null;
}

interface MenuMusicState {
    decks: HTMLAudioElement[] | null;
    active: number;
    state: MenuState | null;
    activated: boolean;
    completedCycleId: number;
    playBlocked: boolean;
    blockedDeck: number | null;
    pendingVolume: number | null;
    crossfade: MenuMusicCrossfade | null;
    cinematicActive: boolean;
    mediaMetadataSet: boolean;
    fetchRequested: boolean;
    frame: number;
}

type RewardStage =
    | { kind: "leader"; amount: 0; slug: string; label: string }
    | { kind: "level"; amount: number; level: number; selector: string }
    | { kind: "xp" | "crowns" | "laurels"; amount: number; selector: string };

interface RewardItem {
    stage: RewardStage;
    card: HTMLElement;
    amount: HTMLElement;
}

interface RewardFlyItem {
    stage: { selector: string };
    card: HTMLElement;
}

const menuMusic: MenuMusicState = {
    decks: null,
    active: 0,
    state: null,
    activated: false,
    completedCycleId: -1,
    playBlocked: false,
    blockedDeck: null,
    pendingVolume: null,
    crossfade: null,
    cinematicActive: false,
    mediaMetadataSet: false,
    fetchRequested: false,
    frame: 0,
};

function menuMusicVolume() {
    const settings = menuMusic.state && menuMusic.state.settings;
    const raw =
        menuMusic.pendingVolume == null
            ? !settings || settings.music_volume == null
                ? 0.8
                : settings.music_volume
            : menuMusic.pendingVolume;
    const value = Number(raw);
    return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0.8;
}

function menuMusicMuted() {
    return Boolean(menuMusic.state && menuMusic.state.settings && menuMusic.state.settings.mute_all);
}

function menuMusicAssetUrl() {
    const base = String(window.SOW_ASSETS_URL || "").replace(/\/$/, "");
    if (!base) {
        console.error("[SOW MUSIC] SOW_ASSETS_URL is missing; theme cannot load.");
        return null;
    }
    const url = base + "/gameplay/music/shadows-of-war.webm";
    const versions = window.SOW_ASSET_TS || {};
    let version = String(versions["/assets/gameplay/music/shadows-of-war.webm"] || window.SOW_BUILD_TS || "");
    if (version === "__BUILD_TS__") version = "";
    return version ? url + "?v=" + encodeURIComponent(version) : url;
}

function menuMusicSetVolumes(mixes: number[], fade?: number) {
    if (!menuMusic.decks) return;
    const base = menuMusicMuted() ? 0 : menuMusicVolume();
    const factor = fade == null ? 1 : fade;
    menuMusic.decks.forEach(function (deck, index) {
        deck.volume = base * (mixes[index] || 0) * factor;
    });
}

function menuMusicCancelFrame() {
    if (menuMusic.frame) window.cancelAnimationFrame(menuMusic.frame);
    menuMusic.frame = 0;
}

function menuMusicLoaderActive(state: MenuState | null): boolean {
    return (
        !!state &&
        (state.loader_job === "Boot" ||
            state.loader_job === "EnterGame" ||
            state.loader_job === "ExitGame") &&
        (!Number.isSafeInteger(state.loader_cycle_id) || state.loader_cycle_id > menuMusic.completedCycleId)
    );
}

function menuMusicPausePlayback() {
    menuMusicCancelFrame();
    menuMusic.crossfade = null;
    if (!menuMusic.decks) return;
    menuMusic.decks.forEach(function (deck) {
        deck.pause();
    });
    menuMusicSetVolumes(menuMusicMixes());
}

function menuMusicReset() {
    menuMusicPausePlayback();
    menuMusic.active = 0;
    if (menuMusic.decks) {
        menuMusic.decks.forEach(function (deck) {
            try {
                deck.currentTime = 0;
            } catch {}
        });
        menuMusicSetVolumes(menuMusicMixes());
    }
}

function menuMusicFinishLoaderCycle(cycleId: number) {
    const currentState = menuMusic.state;
    if (
        !Number.isSafeInteger(cycleId) ||
        cycleId <= menuMusic.completedCycleId ||
        !currentState ||
        currentState.loader_cycle_id !== cycleId ||
        (currentState.loader_job !== "Boot" &&
            currentState.loader_job !== "EnterGame" &&
            currentState.loader_job !== "ExitGame")
    )
        return false;
    menuMusic.completedCycleId = cycleId;
    menuMusicReset();
    syncMenuMusic(currentState, false);
    return true;
}

function menuMusicResetOnMenuReturn(previousState: MenuState | null, nextState: MenuState) {
    if (!previousState || nextState.phase !== "MainMenu") return false;
    const returnedFromGame = previousState.phase !== "MainMenu";
    const leftLobby = previousState.phase === "MainMenu" && previousState.waiting && !nextState.waiting;
    return (
        (returnedFromGame || leftLobby) &&
        nextState.loader_job !== "EnterGame" &&
        nextState.loader_job !== "ExitGame"
    );
}

function menuMusicMixes() {
    if (!menuMusic.crossfade) return menuMusic.active === 0 ? [1, 0] : [0, 1];
    const progress = menuMusic.crossfade.progress;
    const mixes = [0, 0];
    mixes[menuMusic.crossfade.outgoing] = 1 - progress;
    mixes[menuMusic.crossfade.incoming] = progress;
    return mixes;
}

function menuMusicFinishCrossfade() {
    if (!menuMusic.crossfade) return;
    menuMusicCancelFrame();
    const outgoing = menuMusic.crossfade.outgoing;
    menuMusic.active = menuMusic.crossfade.incoming;
    menuMusic.crossfade = null;
    menuMusic.decks![outgoing].pause();
    try {
        menuMusic.decks![outgoing].currentTime = 0;
    } catch {}
    menuMusicSetVolumes(menuMusicMixes());
}

function menuMusicScheduleCrossfade() {
    if (!menuMusic.crossfade || menuMusic.frame) return;
    menuMusic.frame = window.requestAnimationFrame(function (timestamp) {
        menuMusic.frame = 0;
        if (!menuMusic.crossfade) return;
        if (menuMusic.crossfade.startedAt == null) menuMusic.crossfade.startedAt = timestamp;
        menuMusic.crossfade.progress = Math.min(
            1,
            Math.max(0, (timestamp - menuMusic.crossfade.startedAt) / 3000),
        );
        menuMusicSetVolumes(menuMusicMixes());
        if (menuMusic.crossfade.progress >= 1) menuMusicFinishCrossfade();
        else menuMusicScheduleCrossfade();
    });
}

function menuMusicPlaybackFailed(error: unknown, index: number) {
    const errorName = error && typeof error === "object" && "name" in error ? error.name : undefined;
    if (errorName === "NotAllowedError") {
        menuMusic.playBlocked = true;
        menuMusic.blockedDeck = index;
        return true;
    }
    console.error("[SOW MUSIC] Opus playback failed:", error);
    return false;
}

function menuMusicSetMediaMetadata() {
    const mediaSession = window.navigator && window.navigator.mediaSession;
    if (menuMusic.mediaMetadataSet || !mediaSession || typeof window.MediaMetadata !== "function") return;
    const artworkMeta = document.querySelector<HTMLMetaElement>('meta[name="sow-media-artwork"]');
    if (!artworkMeta || !artworkMeta.content) return;
    try {
        mediaSession.metadata = new window.MediaMetadata({
            title: "Shadows of War",
            artwork: [
                {
                    src: new window.URL(artworkMeta.content, document.baseURI).href,
                    sizes: "192x192",
                    type: "image/png",
                },
            ],
        });
        menuMusic.mediaMetadataSet = true;
    } catch (error) {
        console.warn("[SOW MUSIC] could not set player metadata:", error);
    }
}

function menuMusicPlayDeck(index: number, userGesture: boolean) {
    const deck = menuMusic.decks && menuMusic.decks[index];
    if (
        !deck ||
        !menuMusic.activated ||
        menuMusicLoaderActive(menuMusic.state) ||
        menuMusicMuted() ||
        menuMusicVolume() <= 0
    )
        return false;
    if (!menuMusic.fetchRequested) menuMusicBeginLoading();
    if (userGesture) {
        menuMusic.playBlocked = false;
        menuMusic.blockedDeck = null;
    }
    if (menuMusic.playBlocked && !userGesture) return false;
    if (!deck.paused) return true;
    try {
        const result = deck.play();
        const started = function () {
            menuMusic.playBlocked = false;
            menuMusic.blockedDeck = null;
            menuMusicSetMediaMetadata();
            if (menuMusic.crossfade && menuMusic.crossfade.incoming === index) menuMusicScheduleCrossfade();
        };
        if (result && typeof result.then === "function") {
            result.then(started).catch(function (error) {
                const blocked = menuMusicPlaybackFailed(error, index);
                if (!blocked && menuMusic.crossfade && menuMusic.crossfade.incoming === index) {
                    menuMusic.crossfade = null;
                    menuMusicSetVolumes(menuMusicMixes());
                }
            });
        } else {
            started();
        }
        return true;
    } catch (error) {
        const blocked = menuMusicPlaybackFailed(error, index);
        if (!blocked && menuMusic.crossfade && menuMusic.crossfade.incoming === index) {
            menuMusic.crossfade = null;
            menuMusicSetVolumes(menuMusicMixes());
        }
        return false;
    }
}

function menuMusicBeginCrossfade() {
    if (!menuMusic.decks || menuMusic.crossfade || menuMusicLoaderActive(menuMusic.state)) return;
    const incoming = 1 - menuMusic.active;
    try {
        menuMusic.decks[incoming].currentTime = 0;
    } catch {}
    menuMusic.crossfade = {
        outgoing: menuMusic.active,
        incoming: incoming,
        progress: 0,
        startedAt: null,
    };
    menuMusicSetVolumes(menuMusicMixes());
    if (!menuMusicPlayDeck(incoming, false)) menuMusic.crossfade = null;
}

function menuMusicCreatePlayer() {
    try {
        const url = menuMusicAssetUrl();
        if (!url) return false;
        menuMusic.decks = [new window.Audio(), new window.Audio()];
        menuMusic.decks!.forEach(function (deck, index) {
            deck.preload = "none";
            deck.loop = false;
            deck.volume = 0;
            deck.addEventListener("error", function () {
                console.error("[SOW MUSIC] Opus theme failed to load:", deck.error || "media error");
            });
            deck.addEventListener("timeupdate", function () {
                if (
                    index !== menuMusic.active ||
                    menuMusic.crossfade ||
                    menuMusicLoaderActive(menuMusic.state)
                )
                    return;
                if (
                    Number.isFinite(deck.duration) &&
                    deck.duration > 3.1 &&
                    deck.currentTime >= deck.duration - 3
                ) {
                    menuMusicBeginCrossfade();
                }
            });
            deck.addEventListener("ended", function () {
                if (index !== menuMusic.active || menuMusicLoaderActive(menuMusic.state)) return;
                if (menuMusic.crossfade && menuMusic.crossfade.outgoing === index) {
                    menuMusicFinishCrossfade();
                    return;
                }
                console.warn("[SOW MUSIC] loop crossfade missed the track end; restarting at the opening.");
                menuMusic.active = 1 - index;
                try {
                    menuMusic.decks![menuMusic.active].currentTime = 0;
                } catch {}
                menuMusicSetVolumes(menuMusicMixes());
                menuMusicPlayDeck(menuMusic.active, false);
            });
            deck.src = url;
        });
        return true;
    } catch (error) {
        console.error("[SOW MUSIC] could not create the WebView audio player:", error);
        return false;
    }
}

function syncMenuMusic(nextState: MenuState | null, userGesture: boolean, volumeOverride?: number) {
    if (!nextState) return;
    const previousState = menuMusic.state;
    menuMusic.state = nextState;
    const reportedVolume = nextState.settings && Number(nextState.settings.music_volume);
    if (volumeOverride != null && Number.isFinite(Number(volumeOverride))) {
        menuMusic.pendingVolume = Math.max(0, Math.min(1, Number(volumeOverride)));
    } else if (
        menuMusic.pendingVolume != null &&
        Number.isFinite(reportedVolume) &&
        Math.abs(reportedVolume - menuMusic.pendingVolume) < 0.001
    ) {
        menuMusic.pendingVolume = null;
    }
    if (userGesture) {
        menuMusic.activated = true;
        menuMusic.playBlocked = false;
    }
    if (menuMusicLoaderActive(nextState)) {
        menuMusicPausePlayback();
        return;
    }
    if (menuMusic.cinematicActive) {
        menuMusicPausePlayback();
        return;
    }
    if (menuMusicResetOnMenuReturn(previousState, nextState)) menuMusicReset();
    if (menuMusicMuted() || menuMusicVolume() <= 0) {
        menuMusic.playBlocked = false;
        menuMusic.blockedDeck = null;
        menuMusicSetVolumes(menuMusicMixes());
        return;
    }
    if (!menuMusic.activated) return;
    if (!menuMusic.decks && !menuMusicCreatePlayer()) return;
    menuMusicSetVolumes(menuMusicMixes());
    const nextDeck = menuMusic.crossfade
        ? menuMusic.crossfade.incoming
        : menuMusic.blockedDeck == null
          ? menuMusic.active
          : menuMusic.blockedDeck;
    menuMusicPlayDeck(nextDeck, Boolean(userGesture));
}

function menuMusicSetCinematicActive(active: boolean) {
    active = Boolean(active);
    if (menuMusic.cinematicActive === active) return;
    menuMusic.cinematicActive = active;
    if (active) menuMusicPausePlayback();
    else if (menuMusic.state) syncMenuMusic(menuMusic.state, false);
}

window.SOW_menu_music_cinematic = menuMusicSetCinematicActive;

// Music bytes wait for their own window: the boot template fires
// sow:wasm-bytes-ready once the engine download completes, so the theme
// buffers against CPU/identity work instead of the WASM wire.
function menuMusicBeginLoading() {
    if (menuMusic.fetchRequested || !menuMusic.decks) return;
    if (menuMusicMuted() || menuMusicVolume() <= 0) return;
    menuMusic.fetchRequested = true;
    try {
        menuMusic.decks[menuMusic.active].load();
    } catch {}
}

function menuMusicStartAtLoaderEnd() {
    menuMusicBeginLoading();
    menuMusic.activated = true;
    if (menuMusic.state && Number.isSafeInteger(menuMusic.state.loader_cycle_id)) {
        menuMusic.completedCycleId = Math.max(menuMusic.completedCycleId, menuMusic.state.loader_cycle_id);
    }
    if (menuMusic.state) syncMenuMusic(menuMusic.state, false);
}

function menuMusicUnlockFromClick(event: MouseEvent) {
    if (
        !event ||
        !event.isTrusted ||
        !(event.target instanceof Node) ||
        typeof root.contains !== "function" ||
        root.contains(event.target) ||
        !menuMusic.activated ||
        !menuMusic.state ||
        menuMusicLoaderActive(menuMusic.state) ||
        menuMusic.cinematicActive ||
        menuMusicMuted() ||
        menuMusicVolume() <= 0
    )
        return;
    const deckIndex = menuMusic.crossfade
        ? menuMusic.crossfade.incoming
        : menuMusic.blockedDeck == null
          ? menuMusic.active
          : menuMusic.blockedDeck;
    const deck = menuMusic.decks && menuMusic.decks[deckIndex];
    if (!menuMusic.playBlocked && (!deck || !deck.paused)) return;
    syncMenuMusic(menuMusic.state, true);
}

menuMusicCreatePlayer();
window.addEventListener("sow:loader-ready", menuMusicStartAtLoaderEnd, { once: true });
window.addEventListener("sow:wasm-bytes-ready", menuMusicBeginLoading, { once: true });
document.addEventListener("click", menuMusicUnlockFromClick, true);

function renderTopbar() {
    const name = menuUi.state!.player_name || SOW_t("menu.anonymous");
    const auth: SowAuthState = { linked: false, pending: false };
    /* POKI_SHARED_AUTH_LOOKUP_BEGIN */
    if (typeof window.SOW_getAuthState === "function") Object.assign(auth, window.SOW_getAuthState());
    /* POKI_SHARED_AUTH_LOOKUP_END */
    const accountXp = Math.max(0, Number(menuUi.state!.xp) || 0);
    const crowns = menuUi.state!.crowns || 0;
    const laurels = menuUi.state!.laurels || 0;
    const gems = menuUi.state!.gems || 0;
    const showSignIn = portalOffersSignIn() && !(auth.linked || auth.pending);
    const signInMarkup = { value: "" };
    /* POKI_SHARED_SIGNIN_MARKUP_BEGIN */
    signInMarkup.value =
        "<button class='sow-menu__signin' type='button' data-command='sign_in'>" +
        esc(SOW_t("menu.sign_in")) +
        "</button>";
    /* POKI_SHARED_SIGNIN_MARKUP_END */
    return (
        "" +
        "<header class='sow-menu__topbar'>" +
        "<div class='sow-menu__identity'>" +
        "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
        "aria-label='" +
        esc(SOW_t("menu.select_leader")) +
        "' style=\"background-image:url('" +
        esc(avatarImage()) +
        "')\"></button>" +
        "<div class='sow-menu__profile'>" +
        "<input data-role='display-name' name='display_name' value=\"" +
        esc(name) +
        "\" maxlength='16' aria-label='" +
        esc(SOW_t("menu.display_name")) +
        "'>" +
        "</div>" +
        "</div>" +
        "<div class='sow-menu__top-actions'>" +
        "<div class='sow-menu__progress' data-progression data-command='open_profile' role='button' tabindex='0' title='" +
        esc(SOW_t("menu.open_profile")) +
        "' aria-label='" +
        esc(SOW_t("menu.open_profile")) +
        "'>" +
        "<span class='sow-menu__progress-cell sow-menu__level'><small>" +
        esc(SOW_t("menu.level_short")) +
        "</small><strong data-progression-level-value>" +
        esc(menuUi.state!.level) +
        "</strong></span>" +
        "<span class='sow-menu__progress-cell sow-menu__xp'><span class='sow-menu__xp-value' data-progression-xp-value>" +
        esc(Math.floor(accountXp)) +
        " " +
        esc(SOW_t("menu.xp")) +
        "</span><span class='sow-menu__xp-track' aria-hidden='true'><i data-progression-xp-fill style='width:" +
        (accountXp % 100) +
        "%'></i></span></span>" +
        "<span class='sow-menu__progress-cell sow-menu__crowns'><img class='sow-menu__currency-icon' src='" +
        esc(currencyAsset("crown")) +
        "' alt='' aria-hidden='true'><strong data-progression-crowns-value>" +
        esc(crowns) +
        "</strong></span>" +
        "<span class='sow-menu__progress-cell sow-menu__laurels' aria-label='" +
        esc(SOW_t("profile.laurels")) +
        "'><img class='sow-menu__currency-icon' src='" +
        esc(currencyAsset("laurel")) +
        "' alt='' aria-hidden='true'><strong data-progression-laurels-value>" +
        esc(laurels) +
        "</strong></span>" +
        "<span class='sow-menu__progress-cell sow-menu__gems'><img class='sow-menu__currency-icon' src='" +
        esc(currencyAsset("gem")) +
        "' alt='' aria-hidden='true'><strong data-progression-gems-value>" +
        esc(gems) +
        "</strong></span>" +
        "</div>" +
        (showSignIn
            ? signInMarkup.value
            : !portalOffersSignIn()
              ? "<span class='sow-menu__account-label'>" + esc(SOW_t("menu.anonymous")) + "</span>"
              : "") +
        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" +
        esc(SOW_t("menu.settings")) +
        "'>" +
        menuIcon("settings") +
        "</button>" +
        "</div>" +
        "</header>"
    );
}

export function localeOptions() {
    const codes =
        typeof window.SOW_getSupportedLocales === "function" ? window.SOW_getSupportedLocales() : [];
    return codes.map(function (code) {
        return {
            value: code,
            label:
                typeof window.SOW_getLocaleLabel === "function"
                    ? window.SOW_getLocaleLabel(code)
                    : SOW_t("menu.language_" + String(code).replace(/-/g, "_")),
        };
    });
}

function syncDisconnectNotice(nextState: MenuState) {
    if (nextState.connected) {
        if (disconnectNoticeTimer !== null) window.clearTimeout(disconnectNoticeTimer);
        disconnectNoticeTimer = null;
        disconnectNoticeDismissed = false;
        disconnectNoticeVisible = false;
        return;
    }
    if (nextState.phase !== "MainMenu") {
        if (disconnectNoticeTimer !== null) window.clearTimeout(disconnectNoticeTimer);
        disconnectNoticeTimer = null;
        disconnectNoticeVisible = false;
        return;
    }
    if (disconnectNoticeDismissed || disconnectNoticeVisible || disconnectNoticeTimer !== null) return;
    disconnectNoticeTimer = window.setTimeout(function () {
        disconnectNoticeTimer = null;
        if (
            !menuUi.state ||
            menuUi.state.phase !== "MainMenu" ||
            menuUi.state.connected ||
            disconnectNoticeDismissed
        )
            return;
        disconnectNoticeVisible = true;
        render();
    }, 30_000);
}

function renderSystemNotice() {
    if (!menuUi.state || menuUi.state.phase !== "MainMenu") return "";
    const updateAvailable = menuUi.state.update_available === true;
    const disconnected = !updateAvailable && disconnectNoticeVisible && !menuUi.state.connected;
    if (!updateAvailable && !disconnected) return "";
    const title = updateAvailable ? SOW_t("menu.version_update_title") : SOW_t("menu.connection_lost");
    const description = updateAvailable
        ? SOW_t("menu.version_update_body")
        : SOW_t("menu.connection_lost_body");
    const primaryLabel = updateAvailable ? SOW_t("menu.update_now") : SOW_t("menu.retry_connection");
    const offlineAction = disconnected
        ? "<button class='sow-menu__secondary sow-menu__system-secondary' type='button' data-command='system_notice_continue_offline'>" +
          esc(SOW_t("menu.continue_offline")) +
          "</button>"
        : "";
    return (
        "<div class='sow-menu__overlay sow-menu__system-overlay' data-menu-overlay='system-notice'>" +
        "<section class='sow-menu__modal sow-menu__system-modal' role='alertdialog' aria-modal='true' aria-labelledby='sow-system-notice-title' aria-describedby='sow-system-notice-copy' tabindex='-1'>" +
        "<h2 id='sow-system-notice-title'>" +
        esc(title) +
        "</h2>" +
        "<p id='sow-system-notice-copy'>" +
        esc(description) +
        "</p>" +
        "<div class='sow-menu__system-actions'>" +
        "<button class='sow-menu__primary sow-menu__system-primary' type='button' data-command='system_notice_refresh'>" +
        esc(primaryLabel) +
        "</button>" +
        offlineAction +
        "</div>" +
        "</section>" +
        "</div>"
    );
}

function dismissDisconnectNotice() {
    disconnectNoticeDismissed = true;
    disconnectNoticeVisible = false;
    render();
}

function handleSystemNoticeCommand(command: string | undefined) {
    if (command === "system_notice_refresh") {
        window.location.reload();
        return true;
    }
    if (command === "system_notice_continue_offline") {
        dismissDisconnectNotice();
        return true;
    }
    return false;
}

export function renderSettingsPrivacySection(privacyLinkMarkup: string) {
    let deleteControl = "";
    if (selfCreds()) {
        deleteControl = deleteBusy
            ? "<p class='sow-menu__settings-delete-status' role='status'>" +
              esc(SOW_t("profile.deleting_account")) +
              "</p>"
            : menuUi.deleteArmed
              ? "<section class='sow-menu__settings-confirm' role='alertdialog' aria-label='" +
                esc(SOW_t("profile.delete_account")) +
                "'><p>" +
                esc(SOW_t("profile.permanent_delete")) +
                "</p><div class='sow-menu__settings-confirm-actions'><button class='sow-menu__secondary' type='button' data-command='cancel_delete'>" +
                esc(SOW_t("lobbies.cancel")) +
                "</button><button class='sow-menu__danger' type='button' data-command='confirm_delete'>" +
                esc(SOW_t("profile.delete_confirm")) +
                "</button></div></section>"
              : "<button class='sow-menu__danger sow-menu__settings-delete' type='button' data-command='request_delete'>" +
                esc(SOW_t("profile.delete_account")) +
                "</button>";
    }
    const error = deleteError
        ? "<p class='sow-menu__settings-delete-error' role='alert'>" + esc(deleteError) + "</p>"
        : "";
    const privacyLink = privacyLinkMarkup
        ? "<nav class='sow-menu__settings-links sow-menu__settings-privacy-links' aria-label='" +
          esc(SOW_t("menu.privacy")) +
          "'>" +
          privacyLinkMarkup +
          "</nav>"
        : "";
    return (
        "<section class='sow-menu__settings-section sow-menu__settings-privacy'><h3>" +
        esc(SOW_t("menu.privacy")) +
        "</h3>" +
        privacyLink +
        deleteControl +
        error +
        "</section>"
    );
}

function renderScreenPanel(screen: string) {
    if (screen === "home") return renderHome();
    if (screen === "campaign") return renderCampaign();
    if (screen === "browser") return renderBrowser();
    if (screen === "create") return renderCreate();
    if (screen === "queue") return renderQueue();
    if (screen === "profile") return renderProfile();
    if (screen === "heroes") return renderHeroes();
    if (screen === "store") return renderStore();
    return "";
}

function screenNav(screen: string) {
    return screen === "store"
        ? "store"
        : screen === "heroes"
          ? "heroes"
          : screen === "profile"
            ? "profile"
            : "battle";
}

function screenShellClass(screen: string) {
    return (
        "sow-menu__shell" +
        (screen === "store"
            ? " sow-menu__store"
            : screen === "heroes"
              ? " sow-menu__heroes"
              : screen === "profile"
                ? " sow-profile"
                : "")
    );
}

function screenBackdropClass(screen: string) {
    return (
        "sow-menu__backdrop" +
        (screen === "store" ? " sow-store__backdrop" : screen === "heroes" ? " sow-heroes__backdrop" : "")
    );
}

function renderFrame(screen: string) {
    return (
        "<div class='" +
        screenBackdropClass(screen) +
        "' data-menu-backdrop></div>" +
        "<div class='" +
        screenShellClass(screen) +
        "'>" +
        renderTopbar() +
        "<div class='sow-menu__screen-stage' data-screen-stage>" +
        renderScreenPanel(screen) +
        "</div>" +
        renderMainNav(screenNav(screen)) +
        renderMenuAttribution() +
        "</div>" +
        renderPasswordModal() +
        renderProfileDetail()
    );
}

function panelFromMarkup(markup: string): HTMLElement | null {
    const template = document.createElement("template");
    template.innerHTML = String(markup || "").trim();
    const panel = template.content.firstElementChild;
    return panel instanceof HTMLElement ? panel : null;
}

function syncOverlay(key: string, markup: string) {
    const selector = "[data-menu-overlay='" + key + "']";
    const current = root.querySelector(selector);
    if (!markup) {
        if (current) {
            current.remove();
            if (key === "system-notice") {
                const restoreFocus = systemNoticeReturnFocus;
                systemNoticeReturnFocus = null;
                if (restoreFocus && restoreFocus.isConnected && !root.hidden)
                    restoreFocus.focus({ preventScroll: true });
            }
        }
        return;
    }
    const next = panelFromMarkup(markup);
    if (!next) return;
    if (!current) {
        if (key === "system-notice") {
            systemNoticeReturnFocus =
                document.activeElement instanceof HTMLElement ? document.activeElement : null;
            root.appendChild(next);
            const primary = next.querySelector<HTMLButtonElement>("button");
            if (primary) primary.focus({ preventScroll: true });
        } else {
            root.appendChild(next);
        }
    } else if (current.outerHTML !== next.outerHTML) {
        current.replaceWith(next);
        if (key === "system-notice") {
            const primary = next.querySelector<HTMLButtonElement>("button");
            if (primary) primary.focus({ preventScroll: true });
        }
    }
}

function updateTopbar() {
    const topbar = root.querySelector(".sow-menu__topbar");
    if (!topbar || !menuUi.state) return;
    const nameInput = topbar.querySelector<HTMLInputElement>("[data-role='display-name']");
    const name = menuUi.state.player_name || SOW_t("menu.anonymous");
    if (nameInput && document.activeElement !== nameInput) nameInput.value = name;
    const avatar = topbar.querySelector<HTMLElement>(".sow-menu__avatar");
    const avatarUrl = avatarImage();
    if (avatar && avatar.dataset.avatarUrl !== avatarUrl) {
        avatar.style.backgroundImage = "url(" + JSON.stringify(avatarUrl) + ")";
        avatar.dataset.avatarUrl = avatarUrl;
    }
    const auth: SowAuthState = { linked: false, pending: false };
    /* POKI_SHARED_AUTH_LOOKUP_BEGIN */
    if (typeof window.SOW_getAuthState === "function") Object.assign(auth, window.SOW_getAuthState());
    /* POKI_SHARED_AUTH_LOOKUP_END */
    const showSignIn = portalOffersSignIn() && !(auth.linked || auth.pending);
    const actions = topbar.querySelector(".sow-menu__top-actions");
    const settingsButton = actions && actions.querySelector("[data-command='toggle_settings']");
    let signIn = actions && actions.querySelector<HTMLButtonElement>(".sow-menu__signin");
    if (showSignIn) {
        if (!signIn && actions) {
            signIn = document.createElement("button");
            signIn.className = "sow-menu__signin";
            signIn.type = "button";
            signIn.dataset.command = "sign_in";
            actions.insertBefore(signIn, settingsButton);
        }
        if (signIn) signIn.textContent = SOW_t("menu.sign_in");
    } else if (signIn) {
        signIn.remove();
    }
}

function updateFrameChrome(screen: string) {
    const backdrop = root.querySelector<HTMLElement>("[data-menu-backdrop]");
    const shell = root.querySelector<HTMLElement>(".sow-menu__shell");
    const backdropClass = screenBackdropClass(screen);
    const shellClass = screenShellClass(screen);
    if (backdrop && backdrop.className !== backdropClass) backdrop.className = backdropClass;
    if (shell && shell.className !== shellClass) shell.className = shellClass;
    root.querySelectorAll<HTMLElement>("[data-nav-screen]").forEach(function (item) {
        const active = item.dataset.navScreen === screenNav(screen);
        const navItem = MAIN_NAV_ITEMS.find(function (candidate) {
            return candidate[0] === item.dataset.navScreen;
        });
        const labelText = navItem ? SOW_t(navItem[2]) : "";
        const labelNode = item.querySelector<HTMLElement>("small");
        if (labelNode && labelText) labelNode.textContent = labelText;
        if (labelText) item.setAttribute("aria-label", labelText);
        item.classList.toggle("is-active", active);
        if (active) item.setAttribute("aria-current", "page");
        else item.removeAttribute("aria-current");
    });
    updateTopbar();
}

export function activeScreenPanel() {
    const stage = root.querySelector<HTMLElement>("[data-screen-stage]");
    const panel =
        stage &&
        (stage.querySelector<HTMLElement>("[data-screen-panel].is-active") || stage.lastElementChild);
    return panel instanceof HTMLElement ? panel : null;
}

function syncScreenPanel(screen: string, screenChanged: boolean) {
    if (!menuUi.state) return;
    const stage = root.querySelector<HTMLElement>("[data-screen-stage]");
    const next = panelFromMarkup(renderScreenPanel(screen));
    if (!stage || !next) return;
    if (screenChanged) {
        const settings = menuUi.state.settings;
        sowScreenMotion.show(stage, next, !!settings.reduced_motion);
        return;
    }
    const current = activeScreenPanel();
    if (current) current.replaceWith(next);
    else stage.appendChild(next);
    next.classList.add("is-active");
}

export function render() {
    if (!menuUi.state) return;
    if (activeMenuPointerId !== null) {
        renderPending = true;
        return;
    }
    renderPending = false;
    const screen = currentScreen();
    if (screen !== "heroes") menuUi.skinPickerOpen = false;
    if (screen !== "heroes") menuUi.heroInfoOpen = false;
    const screenChanged = menuUi.previousScreen !== screen;
    const sameScreen = !screenChanged;
    if (screenChanged) menuUi.dropdownOpenKey = null;
    if (screen === "create" && menuUi.previousScreen !== "create") {
        menuUi.createDraft = cloneConfig();
        menuUi.createOffline = !!menuUi.state.custom_game_is_sp;
        menuUi.createPrivate = !!menuUi.state.custom_game_is_private;
        menuUi.createPassword = "";
    }
    if (screen !== "create") {
        menuUi.createDraft = null;
        menuUi.createOffline = false;
        menuUi.createPrivate = false;
        menuUi.createPassword = "";
    }
    menuUi.previousScreen = screen;
    const nextHero =
        screen === "profile"
            ? profileHeroImage(menuUi.profileData && menuUi.profileData.preferred_leader)
            : heroImage();
    if (root.dataset.hero !== nextHero) {
        root.style.setProperty("--sow-hero", 'url("' + nextHero + '")');
        root.dataset.hero = nextHero;
    }
    root.dataset.screen = screen;
    root.dataset.ready = typeof window.SOW_menu_command === "function" ? "true" : "false";
    root.hidden = menuUi.state.phase !== "MainMenu";

    const activeElement = document.activeElement;
    const activeControl =
        activeElement instanceof HTMLInputElement ||
        activeElement instanceof HTMLTextAreaElement ||
        activeElement instanceof HTMLSelectElement
            ? activeElement
            : null;
    const isTyping = activeControl !== null && root.contains(activeControl);
    const activeRole = isTyping ? activeControl.dataset.role : null;
    const activeName = isTyping ? activeControl.name : null;
    const activeVal = isTyping ? activeControl.value : null;
    const isTextControl =
        activeControl instanceof HTMLInputElement || activeControl instanceof HTMLTextAreaElement;
    const selStart = isTyping && isTextControl ? activeControl.selectionStart : null;
    const selEnd = isTyping && isTextControl ? activeControl.selectionEnd : null;
    const currentPanel = activeScreenPanel();
    let scrollTop = null;
    if (sameScreen) {
        const currentScrollOwner =
            screen === "create"
                ? currentPanel && currentPanel.querySelector<HTMLElement>(".sow-create__scroll")
                : currentPanel;
        if (currentScrollOwner) scrollTop = currentScrollOwner.scrollTop;
    }

    if (!root.querySelector("[data-screen-stage]")) {
        root.innerHTML = renderFrame(screen);
        const initialPanel = activeScreenPanel();
        if (initialPanel) initialPanel.classList.add("is-active");
    } else {
        updateFrameChrome(screen);
        syncScreenPanel(screen, screenChanged);
    }
    syncOverlay("password", renderPasswordModal());
    syncOverlay("settings", menuUi.settingsOpen ? renderSettings() : "");
    syncOverlay("auth", menuUi.authModalOpen ? renderAuthModal() : "");
    syncOverlay("profile-detail", menuUi.profileOpen ? renderProfileDetail() : "");
    syncOverlay("skin-picker", typeof renderSkinPickerModal === "function" ? renderSkinPickerModal() : "");
    syncOverlay("hero-info", typeof renderHeroInfoModal === "function" ? renderHeroInfoModal() : "");
    syncOverlay("purchase", typeof renderPurchaseModal === "function" ? renderPurchaseModal() : "");
    syncOverlay("system-notice", renderSystemNotice());

    if (sameScreen && isTyping) {
        let restored: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement | null = null;
        if (activeRole) {
            restored = root.querySelector<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>(
                "[data-role='" + activeRole + "']",
            );
        } else if (activeName) {
            restored = root.querySelector<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>(
                "[name='" + activeName + "']",
            );
        }
        if (restored) {
            if (activeVal != null) restored.value = activeVal;
            try {
                restored.focus();
                if (
                    (restored instanceof HTMLInputElement || restored instanceof HTMLTextAreaElement) &&
                    selStart != null &&
                    selEnd != null
                ) {
                    restored.setSelectionRange(selStart, selEnd);
                }
            } catch {}
        }
    }
    updateDynamic();
    if (screen === "store") loadStoreCatalog();
    if (screen === "store" && screenChanged && typeof window.SOW_trackExperienceEvent === "function") {
        window.SOW_trackExperienceEvent({ name: "store_open" });
    }
    if (sameScreen && scrollTop !== null) {
        const nextScrollOwner =
            screen === "create"
                ? activeScreenPanel()?.querySelector<HTMLElement>(".sow-create__scroll") || null
                : activeScreenPanel();
        if (nextScrollOwner) nextScrollOwner.scrollTop = scrollTop;
    }
    syncDropdowns();
    menuUi.lastRenderKey = renderKey();
    updateLobbyViews();
}

export function progressionFromState(): ProgressionValues {
    const xp = Math.max(0, Number(menuUi.state && menuUi.state.xp) || 0);
    return {
        xp: xp,
        level: Math.max(1, Number(menuUi.state && menuUi.state.level) || Math.floor(xp / 100) + 1),
        crowns: Math.max(0, Number(menuUi.state && menuUi.state.crowns) || 0),
        laurels: Math.max(0, Number(menuUi.state && menuUi.state.laurels) || 0),
        gems: Math.max(0, Number(menuUi.state && menuUi.state.gems) || 0),
    };
}

function pendingRewardReceipts(): MenuRewardReceipt[] {
    return ((menuUi.state && menuUi.state.reward_receipts) || []).filter(function (receipt) {
        const tutorialApplied =
            receipt &&
            receipt.id === "tutorial" &&
            receipt.match_id === "tutorial" &&
            receipt.status === "applied";
        const campaignApplied =
            receipt &&
            typeof receipt.id === "string" &&
            /^campaign:[a-z][a-z0-9_]{0,63}$/.test(receipt.id) &&
            receipt.match_id === receipt.id;
        return (
            receipt &&
            receipt.id &&
            receipt.status === "applied" &&
            !receipt.presented_at &&
            (tutorialApplied ||
                campaignApplied ||
                receipt.verification_status === "verified" ||
                receipt.verification_status === "rejected")
        );
    });
}

export function reducedRewardMotion() {
    return (
        !!(menuUi.state && menuUi.state.settings && menuUi.state.settings.reduced_motion) ||
        !!(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches)
    );
}

export function writeProgression(values: ProgressionValues) {
    if (!values) return;
    displayedProgression = {
        xp: Math.max(0, Number(values.xp) || 0),
        level: Math.max(1, Number(values.level) || Math.floor((Number(values.xp) || 0) / 100) + 1),
        crowns: Math.max(0, Number(values.crowns) || 0),
        laurels: Math.max(0, Number(values.laurels) || 0),
        gems: Math.max(0, Number(values.gems) || 0),
    };
    const progression = root.querySelector<HTMLElement>("[data-progression]");
    if (!progression) return;
    const levelValue = progression.querySelector<HTMLElement>("[data-progression-level-value]");
    const xpValue = progression.querySelector<HTMLElement>("[data-progression-xp-value]");
    const xpFill = progression.querySelector<HTMLElement>("[data-progression-xp-fill]");
    const crownsValue = progression.querySelector<HTMLElement>("[data-progression-crowns-value]");
    const laurelsValue = progression.querySelector<HTMLElement>("[data-progression-laurels-value]");
    const gemsValue = progression.querySelector<HTMLElement>("[data-progression-gems-value]");
    if (levelValue) levelValue.textContent = String(displayedProgression.level);
    if (xpValue) xpValue.textContent = Math.floor(displayedProgression.xp) + " " + SOW_t("menu.xp");
    if (xpFill) xpFill.style.width = (displayedProgression.xp % 100) + "%";
    if (crownsValue) crownsValue.textContent = String(Math.round(displayedProgression.crowns));
    if (laurelsValue) laurelsValue.textContent = String(Math.round(displayedProgression.laurels));
    if (gemsValue) gemsValue.textContent = String(Math.round(displayedProgression.gems));
}

function ensureProgressionDisplay() {
    if (displayedProgression) return;
    const canonical = progressionFromState();
    const pending = pendingRewardReceipts().filter(function (receipt) {
        return !rewardAnimationShown.has(receipt.id);
    });
    if (pending.length) {
        const totals = pending.reduce(
            function (sum, receipt) {
                sum.xp += Math.max(0, Number(receipt.xp) || 0);
                sum.crowns += Math.max(0, Number(receipt.crowns) || 0);
                sum.laurels += Math.max(0, Number(receipt.laurels) || 0);
                return sum;
            },
            { xp: 0, crowns: 0, laurels: 0 },
        );
        canonical.xp = Math.max(0, canonical.xp - totals.xp);
        canonical.crowns = Math.max(0, canonical.crowns - totals.crowns);
        canonical.laurels = Math.max(0, canonical.laurels - totals.laurels);
        canonical.level = Math.floor(canonical.xp / 100) + 1;
    }
    writeProgression(canonical);
}

export function animateProgressionTo(
    target: ProgressionValues,
    duration: number,
    token: number,
    levelOverride?: number | null,
): Promise<boolean> {
    const from = Object.assign({}, displayedProgression || progressionFromState());
    if (reducedRewardMotion() || duration <= 0) {
        writeProgression(target);
        if (token === rewardAnimationToken) progressionAnimationTarget = null;
        return Promise.resolve(true);
    }
    progressionAnimationTarget = Object.assign({}, target);
    const start = performance.now();
    return new Promise<boolean>(function (resolve) {
        let frameId = 0;
        function frame(now: number) {
            if (token !== rewardAnimationToken) {
                if (progressionAnimationFrame === frameId) progressionAnimationFrame = 0;
                resolve(false);
                return;
            }
            if (root.hidden || document.visibilityState !== "visible") {
                progressionAnimationFrame = 0;
                progressionAnimationTarget = null;
                writeProgression(progressionFromState());
                resolve(false);
                return;
            }
            const t = Math.min(1, (now - start) / duration);
            const eased = 1 - Math.pow(1 - t, 3);
            const currentXp = from.xp + (target.xp - from.xp) * eased;
            writeProgression({
                xp: currentXp,
                level: levelOverride == null ? Math.floor(currentXp / 100) + 1 : levelOverride,
                crowns: from.crowns + (target.crowns - from.crowns) * eased,
                laurels: from.laurels + (target.laurels - from.laurels) * eased,
                gems: from.gems + (target.gems - from.gems) * eased,
            });
            if (t < 1) {
                frameId = requestAnimationFrame(frame);
                progressionAnimationFrame = frameId;
            } else {
                if (progressionAnimationFrame === frameId) progressionAnimationFrame = 0;
                writeProgression(target);
                progressionAnimationTarget = null;
                if (!activeRewardStage && duration >= 900) {
                    if (from.xp !== target.xp) pulseRewardCounter("[data-progression-xp-value]", false);
                    if (from.crowns !== target.crowns)
                        pulseRewardCounter("[data-progression-crowns-value]", false);
                    if (from.laurels !== target.laurels)
                        pulseRewardCounter("[data-progression-laurels-value]", false);
                    if (from.gems !== target.gems) pulseRewardCounter("[data-progression-gems-value]", false);
                    if (target.level > from.level) pulseRewardCounter("[data-progression-level-value]", true);
                }
                resolve(true);
            }
        }
        frameId = requestAnimationFrame(frame);
        progressionAnimationFrame = frameId;
    });
}

function removeRewardStage() {
    const layer = activeRewardStage;
    activeRewardStage = null;
    if (!layer) return;
    if (typeof layer.getAnimations === "function")
        layer.getAnimations({ subtree: true }).forEach(function (motion) {
            motion.cancel();
        });
    layer.remove();
    if (layer._shell && "inert" in layer._shell) layer._shell.inert = Boolean(layer._shellWasInert);
    if (layer._restoreFocus && layer._restoreFocus.isConnected && !root.hidden)
        layer._restoreFocus.focus({ preventScroll: true });
}

function cancelRewardAnimation() {
    rewardAnimationToken += 1;
    rewardAnimationRunning = false;
    progressionAnimationTarget = null;
    removeRewardStage();
}

function createRewardStage(stages: RewardStage[], skip: () => void): RewardItem[] {
    const layer = document.createElement("section");
    layer.className = "sow-menu__reward-stage";
    layer.setAttribute("role", "dialog");
    layer.setAttribute("aria-modal", "true");
    layer.setAttribute("aria-label", SOW_t("endgame.match_result"));
    const cards = document.createElement("div");
    cards.className = "sow-menu__reward-cards";
    const items = stages.map(function (stage: RewardStage, index: number): RewardItem {
        const card = document.createElement("div");
        card.className = "sow-menu__reward-card sow-menu__reward-card--" + stage.kind;
        card.style.animationDelay = 120 + index * 140 + "ms";
        card.setAttribute("role", "group");
        const levelLabel = stage.kind === "level" ? SOW_t("profile.level", { level: stage.level }) : null;
        card.setAttribute(
            "aria-label",
            stage.kind === "leader"
                ? stage.label
                : levelLabel ||
                      stage.amount +
                          " " +
                          SOW_t(
                              stage.kind === "xp"
                                  ? "menu.xp"
                                  : stage.kind === "crowns"
                                    ? "store.crowns"
                                    : "store.laurels",
                          ),
        );
        const medallion = document.createElement("span");
        medallion.className = "sow-menu__reward-medallion";
        medallion.setAttribute("aria-hidden", "true");
        let emblem: HTMLElement;
        if (stage.kind === "xp" || stage.kind === "level") {
            const label = document.createElement("span");
            label.textContent = stage.kind === "xp" ? "XP" : "↑";
            emblem = label;
        } else {
            const image = document.createElement("img");
            if (stage.kind === "leader") {
                image.src = avatarImage(stage.slug);
                image.alt = "";
            } else {
                image.src = currencyAsset(stage.kind === "crowns" ? "crown" : "laurel");
            }
            emblem = image;
        }
        emblem.className = "sow-menu__reward-emblem";
        medallion.appendChild(emblem);
        const amount = document.createElement("strong");
        amount.className = "sow-menu__reward-amount";
        if (stage.kind !== "leader") amount.setAttribute("aria-hidden", "true");
        amount.textContent = levelLabel || (stage.kind === "leader" ? stage.label : "+0");
        card.appendChild(medallion);
        card.appendChild(amount);
        cards.appendChild(card);
        return { stage: stage, card: card, amount: amount };
    });
    const close = document.createElement("button");
    close.className = "sow-menu__reward-skip sow-menu__icon-button";
    close.type = "button";
    close.textContent = "×";
    close.setAttribute("aria-label", SOW_t("menu.close"));
    layer.addEventListener("click", function (event) {
        event.preventDefault();
        event.stopPropagation();
        skip();
    });
    layer.addEventListener("keydown", function (event) {
        if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            skip();
        }
    });
    layer.appendChild(cards);
    layer.appendChild(close);
    layer._shell = root.querySelector<HTMLElement>(".sow-menu__shell");
    layer._shellWasInert = Boolean(layer._shell && layer._shell.inert);
    layer._restoreFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    root.appendChild(layer);
    activeRewardStage = layer;
    if (layer._shell && "inert" in layer._shell) layer._shell.inert = true;
    close.focus({ preventScroll: true });
    return items;
}

function revealRewardAmounts(items: RewardItem[], token: number): Promise<boolean> {
    const start = performance.now();
    return new Promise<boolean>(function (resolve) {
        function frame(now: number) {
            if (token !== rewardAnimationToken || root.hidden || document.visibilityState !== "visible") {
                resolve(false);
                return;
            }
            const elapsed = now - start;
            items.forEach(function (item: RewardItem, index: number) {
                const t = Math.max(0, Math.min(1, (elapsed - 300 - index * 140) / 650));
                const value =
                    item.stage.kind === "level"
                        ? SOW_t("profile.level", { level: item.stage.level })
                        : item.stage.kind === "leader"
                          ? item.stage.label
                          : "+" + Math.round(item.stage.amount * (1 - Math.pow(1 - t, 3)));
                if (item.amount.textContent !== value) item.amount.textContent = value;
            });
            if (elapsed < 1450) requestAnimationFrame(frame);
            else resolve(true);
        }
        requestAnimationFrame(frame);
    });
}

export function flyRewardCard(item: RewardFlyItem, token: number): Promise<boolean> {
    const target = root.querySelector<HTMLElement>(item.stage.selector);
    if (!target || typeof item.card.animate !== "function") {
        item.card.style.visibility = "hidden";
        return Promise.resolve(true);
    }
    const from = item.card.getBoundingClientRect();
    const to = target.getBoundingClientRect();
    const dx = to.left + to.width / 2 - from.left - from.width / 2;
    const dy = to.top + to.height / 2 - from.top - from.height / 2;
    const bend = Math.min(160, Math.max(70, Math.hypot(dx, dy) * 0.2));
    const controlX = dx / 2 + (dx < 0 ? -bend * 0.6 : bend * 0.6);
    const controlY = dy / 2 - bend;
    const path: Keyframe[] = [];
    for (let i = 0; i <= 12; i++) {
        const t = i / 12;
        const inverse = 1 - t;
        const x = 2 * inverse * t * controlX + t * t * dx;
        const y = 2 * inverse * t * controlY + t * t * dy;
        path.push({
            offset: t,
            transform:
                "translate3d(" +
                x +
                "px," +
                y +
                "px,0) scale(" +
                (1 - 0.82 * t) +
                ") rotate(" +
                Math.sin(Math.PI * t) * 8 +
                "deg)",
            opacity: 1 - 0.4 * Math.max(0, (t - 0.8) / 0.2),
        });
    }
    return new Promise<boolean>(function (resolve) {
        const motion = item.card.animate(path, {
            duration: 520,
            easing: "cubic-bezier(.45,0,.55,1)",
            fill: "forwards",
        });
        motion.onfinish = function () {
            item.card.style.visibility = "hidden";
            resolve(token === rewardAnimationToken);
        };
        motion.oncancel = function () {
            resolve(false);
        };
    });
}

export function pulseRewardCounter(selector: string, levelUp: boolean) {
    if (reducedRewardMotion()) return;
    const counter = root.querySelector<HTMLElement>(selector);
    if (!counter) return;
    const className = levelUp ? "is-reward-level-up" : "is-reward-value-hit";
    counter.classList.remove(className);
    void counter.offsetWidth;
    counter.classList.add(className);
    window.setTimeout(
        function () {
            counter.classList.remove(className);
        },
        levelUp ? 900 : 640,
    );
}

function acknowledgeShownReceipts(accountId: string | null) {
    if (!menuUi.state || !accountId || menuUi.state.account_id !== accountId) return;
    const now = Date.now();
    const ids = pendingRewardReceipts()
        .filter(function (receipt) {
            return rewardAnimationShown.has(receipt.id) && now - (rewardAckTimes[receipt.id] || 0) >= 30000;
        })
        .map(function (receipt) {
            return receipt.id;
        });
    if (ids.length) {
        ids.forEach(function (id) {
            rewardAckTimes[id] = now;
        });
        send("acknowledge_reward_receipts", { account_id: accountId, receipt_ids: ids });
        if (rewardAckRetryTimer === null) {
            rewardAckRetryTimer = window.setTimeout(function () {
                rewardAckRetryTimer = null;
                if (
                    document.visibilityState === "visible" &&
                    menuUi.state &&
                    menuUi.state.phase === "MainMenu"
                )
                    acknowledgeShownReceipts(accountId);
            }, 30000);
        }
    }
}

function runRewardPresentation(receipts: MenuRewardReceipt[]) {
    if (rewardAnimationRunning) return;
    rewardAnimationRunning = true;
    const token = ++rewardAnimationToken;
    const accountId = menuUi.state && menuUi.state.account_id;
    const ids = receipts.map(function (receipt) {
        return receipt.id;
    });
    const totals = receipts.reduce(
        function (sum, receipt) {
            sum.xp += Math.max(0, Number(receipt.xp) || 0);
            sum.crowns += Math.max(0, Number(receipt.crowns) || 0);
            sum.laurels += Math.max(0, Number(receipt.laurels) || 0);
            return sum;
        },
        { xp: 0, crowns: 0, laurels: 0 },
    );
    const unlockedLeaderIds = Array.from(
        new Set(
            receipts.reduce<string[]>(function (ids, receipt) {
                return ids.concat(Array.isArray(receipt.unlocked_leaders) ? receipt.unlocked_leaders : []);
            }, []),
        ),
    ).filter(function (id: string) {
        return /^[a-z][a-z0-9_]{0,63}$/.test(id);
    });
    const leaderStages: RewardStage[] = unlockedLeaderIds.map(function (slug: string): RewardStage {
        const leader = leaderById(slug);
        const name = leader ? leaderDisplayName(leader) : slug.replace(/_/g, " ");
        return {
            kind: "leader",
            amount: 0,
            slug: slug,
            label: SOW_t("heroes.leader_unlocked", { name: name }),
        };
    });
    const canonical = progressionFromState();
    const target = canonical;
    const startLevel = (displayedProgression || canonical).level;
    const gainedLevels = new Set<number>();
    receipts.forEach(function (receipt) {
        if (Number.isSafeInteger(receipt.level_before) && Number.isSafeInteger(receipt.level_after)) {
            for (let level = Number(receipt.level_before) + 1; level <= Number(receipt.level_after); level++)
                gainedLevels.add(level);
        }
    });
    if (
        !receipts.every(function (receipt) {
            return Number.isSafeInteger(receipt.level_before);
        })
    ) {
        for (let level = startLevel + 1; level <= target.level; level++) gainedLevels.add(level);
    }
    const levelStages: RewardStage[] = Array.from(gainedLevels)
        .sort(function (a, b) {
            return a - b;
        })
        .map(function (level): RewardStage {
            return {
                kind: "level",
                amount: level,
                level: level,
                selector: "[data-progression-level-value]",
            };
        });
    const stages: RewardStage[] = [
        { kind: "xp" as const, amount: totals.xp, selector: "[data-progression-xp-value]" },
        { kind: "crowns" as const, amount: totals.crowns, selector: "[data-progression-crowns-value]" },
        { kind: "laurels" as const, amount: totals.laurels, selector: "[data-progression-laurels-value]" },
        ...leaderStages,
        ...levelStages,
    ].filter(function (stage) {
        return stage.kind === "leader" || stage.amount > 0;
    });

    function finish(shown: boolean) {
        if (token !== rewardAnimationToken || !menuUi.state || menuUi.state.account_id !== accountId) return;
        rewardAnimationToken += 1;
        rewardAnimationRunning = false;
        removeRewardStage();
        if (!shown) {
            writeProgression(progressionFromState());
            if (document.visibilityState === "visible") maybePresentRewards();
            return;
        }
        writeProgression(progressionFromState());
        ids.forEach(function (id) {
            rewardAnimationShown.add(id);
        });
        console.info("[menu-flow] rewards and levels presented", ids, "cycle", menuUi.state.loader_cycle_id);
        acknowledgeShownReceipts(accountId);
        maybePresentRewards();
    }

    function complete(shown: boolean) {
        if (!shown || !activeRewardStage) {
            finish(shown);
            return;
        }
        activeRewardStage.classList.add("is-exiting");
        window.setTimeout(function () {
            finish(true);
        }, 240);
    }

    function next(index: number) {
        if (token !== rewardAnimationToken || root.hidden || document.visibilityState !== "visible") {
            finish(false);
            return;
        }
        if (index >= stages.length) {
            const latest = progressionFromState();
            const current = displayedProgression || progressionFromState();
            const changed =
                latest.xp !== current.xp ||
                latest.level !== current.level ||
                latest.crowns !== current.crowns ||
                latest.laurels !== current.laurels ||
                latest.gems !== current.gems;
            if (changed) animateProgressionTo(latest, 900, token).then(complete);
            else complete(true);
            return;
        }
        const item = items[index];
        const stage = item.stage;
        if (stage.kind === "leader") {
            window.setTimeout(function () {
                next(index + 1);
            }, 900);
            return;
        }
        const nextValues = Object.assign({}, displayedProgression || progressionFromState());
        const oldLevel = nextValues.level;
        if (stage.kind === "level") {
            nextValues.level = stage.level;
        } else {
            nextValues[stage.kind] = target[stage.kind];
            nextValues.level = levelStages.length ? oldLevel : Math.floor(nextValues.xp / 100) + 1;
        }
        flyRewardCard({ card: item.card, stage: stage }, token).then(function (arrived) {
            if (!arrived) {
                finish(false);
                return;
            }
            if (stage.kind === "level") {
                writeProgression(nextValues);
                pulseRewardCounter(stage.selector, true);
                window.setTimeout(function () {
                    next(index + 1);
                }, 250);
                return;
            }
            const heldLevel = levelStages.length ? oldLevel : null;
            animateProgressionTo(nextValues, 380, token, heldLevel).then(function (shown) {
                if (!shown) {
                    finish(false);
                    return;
                }
                pulseRewardCounter(stage.selector, false);
                next(index + 1);
            });
        });
    }
    if (reducedRewardMotion()) {
        if (!leaderStages.length) {
            finish(true);
            return;
        }
        const staticItems = createRewardStage(stages, function () {
            finish(true);
        });
        staticItems.forEach(function (item: RewardItem) {
            item.amount.textContent =
                item.stage.kind === "leader"
                    ? item.stage.label
                    : item.stage.kind === "level"
                      ? SOW_t("profile.level", { level: item.stage.level })
                      : "+" + item.stage.amount;
        });
        return;
    }
    if (!stages.length) {
        finish(true);
        return;
    }
    const items = createRewardStage(stages, function () {
        finish(true);
    });
    revealRewardAmounts(items, token).then(function (shown) {
        if (shown) next(0);
        else finish(false);
    });
}

function rankedResultKey(state: MenuState) {
    return [state.account_id || "", state.menu_return && state.menu_return.match_id].join(":");
}

function presentRankedResult(result: RankedResult) {
    const layer = document.createElement("section");
    layer.className = "sow-menu__reward-stage sow-menu__ranked-result-stage";
    layer.setAttribute("role", "dialog");
    layer.setAttribute("aria-modal", "true");
    layer.setAttribute("aria-label", SOW_t("lobbies.ranked_result_title"));
    const card = document.createElement("div");
    card.className = "sow-menu__ranked-result-card";
    const before = result.tier_before + (result.division_before ? " " + result.division_before : "");
    const after = result.tier_after + (result.division_after ? " " + result.division_after : "");
    const moved = before !== after;
    const movement = moved
        ? result.score_after > result.score_before
            ? SOW_t("lobbies.rank_promoted")
            : SOW_t("lobbies.rank_demoted")
        : SOW_t("lobbies.rank_updated");
    const emblem = document.createElement("span");
    emblem.className = "sow-menu__ranked-result-emblem";
    emblem.setAttribute("aria-hidden", "true");
    emblem.textContent = result.tier_after.slice(0, 1);
    const kicker = document.createElement("span");
    kicker.className = "sow-menu__ranked-result-kicker";
    kicker.textContent = SOW_t("lobbies.ranked_1v1");
    const title = document.createElement("h2");
    title.textContent = movement;
    const rank = document.createElement("strong");
    rank.className = "sow-menu__ranked-result-rank";
    rank.textContent = after;
    const score = document.createElement("span");
    score.className = "sow-menu__ranked-result-score";
    score.textContent = SOW_t("lobbies.rank_score", { score: result.score_after });
    const change = document.createElement("p");
    change.className = "sow-menu__ranked-result-change";
    change.textContent = (result.rating_delta > 0 ? "+" : "") + result.rating_delta + " MMR";
    const previous = document.createElement("p");
    previous.className = "sow-menu__ranked-result-previous";
    previous.textContent = SOW_t("lobbies.rank_previous", { score: result.score_before });
    const close = document.createElement("button");
    close.type = "button";
    close.className = "sow-menu__primary sow-menu__ranked-result-continue";
    close.textContent = SOW_t("lobbies.rank_continue");
    const dismiss = function (event: Event) {
        event.preventDefault();
        event.stopPropagation();
        if (!menuUi.state) return;
        rankedResultDismissedKey = rankedResultKey(menuUi.state);
        send("dismiss_ranked_result");
        removeRewardStage();
    };
    close.addEventListener("click", dismiss);
    layer.addEventListener("keydown", function (event) {
        if (event.key === "Escape") dismiss(event);
    });
    card.append(emblem, kicker, title, rank, score, change, previous, close);
    layer.appendChild(card);
    layer._shell = root.querySelector<HTMLElement>(".sow-menu__shell");
    layer._shellWasInert = Boolean(layer._shell && layer._shell.inert);
    layer._restoreFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    root.appendChild(layer);
    activeRewardStage = layer;
    if (layer._shell && "inert" in layer._shell) layer._shell.inert = true;
    close.focus({ preventScroll: true });
}

function maybePresentRewards() {
    if (!menuUi.state) return;
    const pending = pendingRewardReceipts().filter(function (receipt) {
        return !rewardAnimationShown.has(receipt.id);
    });
    menuFlow.update(menuUi.state, {
        visible: !root.hidden && document.visibilityState === "visible",
        ready: rewardPresentationReady,
        running: rewardAnimationRunning,
        receipts: pending,
        screen: currentScreen(),
    });
    if (lastMenuFlowPhase !== menuFlow.phase()) {
        lastMenuFlowPhase = menuFlow.phase();
        console.info(
            "[menu-flow]",
            lastMenuFlowPhase,
            "cycle",
            menuUi.state.loader_cycle_id,
            "match",
            menuUi.state.menu_return && menuUi.state.menu_return.match_id,
        );
    }
    const guide = window.SOW_menu_guide;
    if (guide) guide.update(menuUi.state, menuGuideFacts());
    if (menuFlow.canPresent() && !rewardAnimationRunning && pending.length) {
        ensureProgressionDisplay();
        runRewardPresentation(pending);
        return;
    }
    const rankedResult = menuUi.state.menu_return && menuUi.state.menu_return.ranked_result;
    const rankedKey = rankedResultKey(menuUi.state);
    if (
        menuFlow.canPresentRanked() &&
        rankedResult &&
        !activeRewardStage &&
        rankedResultDismissedKey !== rankedKey
    ) {
        presentRankedResult(rankedResult);
        return;
    }
    if (menuUi.state.phase === "MainMenu") acknowledgeShownReceipts(menuUi.state.account_id);
    const destination = menuFlow.takeDestination();
    if (destination === "store" && window.SOW_PORTAL !== "poki" && window.SOW_PORTAL !== "jest") {
        menuUi.storeOpen = true;
        render();
        return;
    }
}

function menuGuideFacts() {
    return { screen: currentScreen() };
}

function updateDynamic() {
    syncHeroesRotationCountdown();
    if (!menuUi.state || root.hidden) return;
    if (menuUi.storeOpen && typeof loadStoreCatalog === "function") loadStoreCatalog();
    ensureProgressionDisplay();
    if (reducedRewardMotion() && (rewardAnimationRunning || progressionAnimationTarget)) {
        cancelRewardAnimation();
        writeProgression(progressionFromState());
    }
    const pendingRewards = pendingRewardReceipts().filter(function (receipt) {
        return !rewardAnimationShown.has(receipt.id);
    });
    const holdBundleGems =
        menuUi.purchaseModal && menuUi.purchaseModal.type === "bundle" && !menuUi.purchaseModal.gemPresented;
    if (!rewardAnimationRunning && !pendingRewards.length) {
        const canonical = progressionFromState();
        const currentProgression = displayedProgression || progressionFromState();
        const changed =
            canonical.xp !== currentProgression.xp ||
            canonical.level !== currentProgression.level ||
            canonical.crowns !== currentProgression.crowns ||
            canonical.laurels !== currentProgression.laurels ||
            canonical.gems !== currentProgression.gems;
        const sameTarget =
            progressionAnimationTarget &&
            canonical.xp === progressionAnimationTarget.xp &&
            canonical.level === progressionAnimationTarget.level &&
            canonical.crowns === progressionAnimationTarget.crowns &&
            canonical.laurels === progressionAnimationTarget.laurels &&
            canonical.gems === progressionAnimationTarget.gems;
        if (changed && !sameTarget && !holdBundleGems) {
            if (reducedRewardMotion() || root.hidden || document.visibilityState !== "visible")
                writeProgression(canonical);
            else animateProgressionTo(canonical, 900, ++rewardAnimationToken);
        }
    }
    const panel = activeScreenPanel() || root;
    const settings = menuUi.state.settings;
    const musicInput = root.querySelector<HTMLInputElement>("[data-setting='music_volume']");
    const motionInput = root.querySelector<HTMLInputElement | HTMLSelectElement>(
        "[data-setting='reduced_motion']",
    );
    const freeZoomInput = root.querySelector<HTMLInputElement>("[data-setting='free_zoom_out']");
    const fpsPingInput = root.querySelector<HTMLInputElement>("[data-setting='show_fps_ping']");
    const devToolsInput = root.querySelector<HTMLInputElement>("[data-setting='show_dev_tools']");
    if (musicInput && document.activeElement !== musicInput)
        musicInput.value = String(settings.music_volume == null ? 0.8 : settings.music_volume);
    if (motionInput && document.activeElement !== motionInput)
        motionInput.value = settings.reduced_motion ? "reduced" : "full";
    if (fpsPingInput && document.activeElement !== fpsPingInput)
        fpsPingInput.checked = settings.show_fps_ping === true;
    if (devToolsInput && document.activeElement !== devToolsInput)
        devToolsInput.checked = Boolean(settings.show_dev_tools);
    if (freeZoomInput && document.activeElement !== freeZoomInput)
        freeZoomInput.checked = Boolean(settings.free_zoom_out);
    const musicValBadge = root.querySelector<HTMLElement>("[data-val-for='music_vol']");
    if (musicValBadge && musicInput)
        musicValBadge.textContent = Math.round(Number(musicInput.value) * 100) + "%";
    const timer = panel.querySelector<HTMLElement>("[data-live-countdown]");
    const lobby = joinedLobby();
    if (timer && lobby)
        timer.textContent = menuUi.state.match_preparing
            ? SOW_t("menu.loading")
            : lobby.is_counting_down
              ? SOW_t("lobbies.starting_in", { seconds: Math.ceil(lobby.timer_secs) })
              : SOW_t("lobbies.waiting_for_players");
    updateQueueRoster(lobby);
    const cardStatuses = panel.querySelectorAll<HTMLElement>("[data-lobby-status-for]");
    for (let i = 0; i < cardStatuses.length; i++) {
        const status = cardStatuses[i];
        const cardLobby = findLobby(Number(status.dataset.lobbyStatusFor));
        status.textContent = cardLobby ? lobbyStatusText(cardLobby) : "";
        const card = status.closest("[data-lobby-card]");
        const count = card && card.querySelector<HTMLElement>("[data-lobby-count]");
        if (count) count.textContent = cardLobby ? lobbyPlayerCount(cardLobby) : "";
        if (cardLobby && card) card.setAttribute("aria-label", lobbyLabel(cardLobby));
    }
    maybePresentRewards();
}

root.addEventListener("click", function (event) {
    if (menuUi.state && menuUi.state.phase === "MainMenu") syncMenuMusic(menuUi.state, true);
    const clickedElement = event.target instanceof Element ? event.target : null;
    if (!clickedElement) return;
    const purchaseOverlay = clickedElement.closest("[data-menu-overlay='purchase']");
    if (purchaseOverlay && clickedElement === purchaseOverlay) {
        if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
        if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
        else {
            menuUi.purchaseModal = null;
            menuUi.purchaseIntent = null;
        }
        render();
        return;
    }
    const skinPickerOverlay = clickedElement.closest("[data-menu-overlay='skin-picker']");
    if (skinPickerOverlay && clickedElement === skinPickerOverlay) {
        menuUi.skinPickerOpen = false;
        render();
        return;
    }
    const heroInfoOverlay = clickedElement.closest("[data-menu-overlay='hero-info']");
    if (heroInfoOverlay && clickedElement === heroInfoOverlay) {
        menuUi.heroInfoOpen = false;
        render();
        return;
    }
    const settingsOverlay = clickedElement.closest("[data-menu-overlay='settings']");
    if (settingsOverlay && clickedElement === settingsOverlay) {
        menuUi.settingsOpen = false;
        menuUi.signOutConfirmOpen = false;
        menuUi.dropdownOpenKey = null;
        render();
        return;
    }
    const targetElement = clickedElement.closest("[data-command]");
    if (!(targetElement instanceof HTMLElement) || !root.contains(targetElement)) return;
    const target = targetElement;
    const command = target.dataset.command;
    if (!command) return;
    if (handleSystemNoticeCommand(command)) return;
    const experienceEvent = (
        {
            open_campaign: "menu_campaign_open",
            open_browser: "menu_lobby_browser_open",
            open_create: "menu_custom_create_open",
            confirm_leader: "menu_leader_confirm",
        } as Record<string, string>
    )[command];
    if (experienceEvent && typeof window.SOW_trackExperienceEvent === "function") {
        window.SOW_trackExperienceEvent({ name: experienceEvent });
    }
    if (command === "open_campaign") {
        menuUi.profileOpen = false;
        menuUi.profileAccountId = null;
        menuUi.profileMatchDetail = null;
        menuUi.heroesOpen = false;
        menuUi.storeOpen = false;
        menuUi.campaignOpen = true;
        render();
        return;
    }
    if (command === "close_campaign") {
        menuUi.campaignOpen = false;
        render();
        return;
    }
    if (command === "watch_campaign_cinematic") {
        const cinematicEpisodeId = target.dataset.episodeId;
        if (cinematicEpisodeId && typeof window.SOW_watchCampaignCinematic === "function") {
            window.SOW_watchCampaignCinematic(cinematicEpisodeId);
        }
        return;
    }
    if (command === "start_campaign_episode") {
        const episodeId = target.dataset.episodeId;
        if (!episodeId) return;
        menuUi.campaignOpen = false;
        if (typeof window.SOW_startCampaignEpisode === "function") {
            window.SOW_startCampaignEpisode(episodeId);
        }
        render();
        return;
    }
    /* POKI_SHARED_STORE_ACTIONS_BEGIN */
    if (command === "buy_product") {
        beginStorePurchase(target.dataset.productId);
        return;
    }
    if (command === "restore_purchases") {
        beginStoreRestore();
        return;
    }
    /* POKI_SHARED_STORE_ACTIONS_END */
    if (command === "main_nav") {
        const navScreen = target.dataset.navScreen || "battle";
        menuUi.storeCheckoutRequestId = null;
        menuUi.storeCheckoutBusy = false;
        menuUi.purchaseModal = null;
        menuUi.purchaseIntent = null;
        menuUi.settingsOpen = false;
        menuUi.signOutConfirmOpen = false;
        menuUi.campaignOpen = false;
        menuUi.passwordLobbyId = null;
        menuUi.passwordDraft = "";
        menuUi.tempSelectedLeader = null;
        menuUi.tempSelectedRandom = false;
        if (navScreen === "heroes") {
            menuUi.profileOpen = false;
            menuUi.profileAccountId = null;
            menuUi.profileMatchDetail = null;
            menuUi.profileDetailLoading = false;
            menuUi.profileDetailRequestKey = null;
            menuUi.storeOpen = false;
            menuUi.heroesOpen = true;
            menuUi.heroesSearchQuery = "";
            menuUi.heroesRegionFilter = "all";
            menuUi.dropdownOpenKey = null;
            const navLeaderSelection = currentLeaderSelection();
            menuUi.tempSelectedLeader = navLeaderSelection.selected_leader;
            menuUi.tempSelectedRandom = navLeaderSelection.prefer_random_leader;
            render();
            return;
        }
        if (navScreen === "profile") {
            menuUi.storeOpen = false;
            if (
                !menuUi.profileOpen ||
                menuUi.profileAccountId !== (menuUi.state && menuUi.state.account_id)
            ) {
                openProfile(null);
            } else {
                render();
            }
            return;
        }
        if (navScreen === "store") {
            menuUi.profileOpen = false;
            menuUi.heroesOpen = false;
            menuUi.storeOpen = true;
            render();
            return;
        }
        menuUi.profileOpen = false;
        menuUi.profileAccountId = null;
        menuUi.profileMatchDetail = null;
        menuUi.profileDetailLoading = false;
        menuUi.profileDetailRequestKey = null;
        menuUi.heroesOpen = false;
        menuUi.storeOpen = false;
        if (menuUi.state!.show_browser || menuUi.state!.show_create) {
            send("close_overlay");
        } else {
            render();
        }
        return;
    }
    if (command === "open_profile") {
        menuUi.storeOpen = false;
        menuUi.heroesOpen = false;
        menuUi.campaignOpen = false;
        openProfile(target.dataset.accountId || null);
        return;
    }
    if (command === "close_profile") {
        menuUi.profileOpen = false;
        menuUi.storeOpen = false;
        menuUi.heroesOpen = false;
        menuUi.profileAccountId = null;
        menuUi.profileData = null;
        menuUi.profileMatchDetail = null;
        menuUi.profileDetailLoading = false;
        menuUi.profileDetailRequestKey = null;
        render();
        return;
    }
    if (command === "profile_tab") {
        menuUi.profileTab = target.dataset.profileTab || "overview";
        render();
        if (menuUi.profileTab === "history") {
            const historyEntry = activeProfileEntry();
            if (
                historyEntry &&
                (historyEntry.historyHasMore ||
                    historyEntry.historyError ||
                    !historyEntry.historyLoaded ||
                    historyEntry.stale)
            )
                loadMoreProfileHistory();
        } else if (menuUi.profileTab === "victories") {
            loadRankedLeaderboard();
            loadVictoryLeaderboard();
        }
        return;
    }
    if (command === "open_play_games") {
        if (typeof window.SOW_openAndroidPlayGames === "function") {
            window.SOW_openAndroidPlayGames(target.dataset.playGamesSection || "");
        }
        return;
    }
    if (command === "open_android_oss_licenses") {
        event.preventDefault();
        if (typeof window.SOW_openAndroidOssLicenses === "function") {
            window.SOW_openAndroidOssLicenses();
        }
        return;
    }
    if (command === "open_android_external_link") {
        event.preventDefault();
        const url = target.dataset.url || "";
        const opened =
            typeof window.SOW_openAndroidExternalLink === "function" &&
            window.SOW_openAndroidExternalLink(url);
        if (!opened && url) window.open(url, "_blank", "noopener,noreferrer");
        return;
    }
    if (command === "load_profile_more") {
        loadMoreProfileHistory();
        return;
    }
    if (command === "load_victory_more") {
        loadVictoryLeaderboard(true);
        return;
    }
    if (command === "retry_profile") {
        invalidateProfileCache(menuUi.profileAccountId);
        menuUi.profileSnapshotLoading = false;
        menuUi.profileHistoryLoading = false;
        loadProfile(menuUi.profileAccountId);
        syncProfileDom();
        return;
    }
    if (command === "open_match") {
        const matchId = target.dataset.matchId;
        const detailEntry = activeProfileEntry();
        if (!matchId || !detailEntry || menuUi.profileDetailLoading) return;
        if (detailEntry.details[matchId]) {
            menuUi.profileDetailId = matchId;
            menuUi.profileDetailError = "";
            menuUi.profileDetailRequestKey = null;
            menuUi.profileMatchDetail = detailEntry.details[matchId];
            render();
            return;
        }
        const detailProfileId = menuUi.profileAccountId;
        const detailRequestKey = detailProfileId + ":" + matchId;
        if (detailEntry.detailRequests[matchId]) {
            menuUi.profileDetailId = matchId;
            menuUi.profileDetailError = "";
            menuUi.profileDetailLoading = true;
            menuUi.profileDetailRequestKey = detailRequestKey;
            return;
        }
        menuUi.profileDetailId = matchId;
        menuUi.profileDetailError = "";
        menuUi.profileMatchDetail = null;
        menuUi.profileDetailLoading = true;
        menuUi.profileDetailRequestKey = detailRequestKey;
        detailEntry.detailRequests[matchId] = fetch(profileApi("/matches/" + encodeURIComponent(matchId)), {
            headers: { Accept: "application/json" },
        })
            .then(function (response) {
                if (!response.ok) throw new Error("match detail failed");
                return response.json();
            })
            .then(function (detail) {
                detailEntry.details[matchId] = detail;
                if (menuUi.profileOpen && menuUi.profileAccountId === detailProfileId) {
                    menuUi.profileDetailError = "";
                    menuUi.profileMatchDetail = detail;
                }
            })
            .catch(function () {
                if (menuUi.profileOpen && menuUi.profileAccountId === detailProfileId)
                    menuUi.profileDetailError = SOW_t("profile.match_details_unavailable");
            })
            .finally(function () {
                delete detailEntry.detailRequests[matchId];
                if (menuUi.profileDetailRequestKey === detailRequestKey) {
                    menuUi.profileDetailLoading = false;
                    menuUi.profileDetailRequestKey = null;
                }
                if (
                    menuUi.profileOpen &&
                    menuUi.profileAccountId === detailProfileId &&
                    menuUi.profileDetailId === matchId
                )
                    render();
            });
        return;
    }
    if (command === "close_match") {
        menuUi.profileMatchDetail = null;
        menuUi.profileDetailError = "";
        menuUi.profileDetailId = null;
        menuUi.profileDetailLoading = false;
        menuUi.profileDetailRequestKey = null;
        render();
        return;
    }
    if (command === "open_report") {
        menuUi.reportOpen = true;
        menuUi.reportTarget = target.dataset.accountId || menuUi.profileAccountId;
        menuUi.reportSent = false;
        render();
        return;
    }
    if (command === "cancel_report") {
        menuUi.reportOpen = false;
        menuUi.reportTarget = null;
        render();
        return;
    }
    if (command === "request_delete") {
        if (!menuUi.settingsOpen || !selfCreds() || deleteBusy) return;
        menuUi.deleteArmed = true;
        deleteError = "";
        render();
        return;
    }
    if (command === "cancel_delete") {
        menuUi.deleteArmed = false;
        deleteError = "";
        render();
        return;
    }
    if (command === "confirm_delete") {
        const delCreds = selfCreds();
        if (!menuUi.settingsOpen || !menuUi.deleteArmed || !delCreds || deleteBusy) {
            menuUi.deleteArmed = false;
            render();
            return;
        }
        deleteBusy = true;
        deleteError = "";
        render();
        fetch(profileApi("/profile/anonymous/delete"), {
            method: "POST",
            headers: { "Content-Type": "application/json", Accept: "application/json" },
            body: JSON.stringify({ account_id: delCreds.account_id, auth_secret: delCreds.auth_secret }),
        })
            .then(function (response) {
                return response
                    .json()
                    .catch(function () {
                        return {};
                    })
                    .then(function (data) {
                        if (!response.ok || data.deleted !== true) throw new Error("delete failed");
                    });
            })
            .then(function () {
                try {
                    [
                        "sow_account_id",
                        "sow_account_secret",
                        "sow_player_progress",
                        "poki_ignore_sow_pending_display_name",
                    ].forEach(function (k) {
                        window.localStorage.removeItem(k);
                    });
                } catch {}
                window.location.reload();
            })
            .catch(function () {
                deleteBusy = false;
                menuUi.deleteArmed = false;
                deleteError = SOW_t("menu.account_deletion_unavailable");
                render();
            });
        return;
    }
    if (command === "open_leader_picker") {
        menuUi.profileOpen = false;
        menuUi.profileAccountId = null;
        menuUi.profileMatchDetail = null;
        menuUi.storeOpen = false;
        menuUi.campaignOpen = false;
        menuUi.heroesOpen = true;
        menuUi.heroesSearchQuery = "";
        menuUi.heroesRegionFilter = "all";
        menuUi.dropdownOpenKey = null;
        const pickerLeaderSelection = currentLeaderSelection();
        menuUi.tempSelectedLeader = pickerLeaderSelection.selected_leader;
        menuUi.tempSelectedRandom = pickerLeaderSelection.prefer_random_leader;
        menuUi.settingsOpen = false;
        menuUi.signOutConfirmOpen = false;
        render();
        return;
    }
    if (command === "toggle_dropdown") {
        const dropdownKey = target.dataset.dropdownKey;
        if (!dropdownKey) return;
        setDropdownOpen(dropdownKey, menuUi.dropdownOpenKey !== dropdownKey, target);
        return;
    }
    if (command === "select_dropdown") {
        const dropdownKey = target.dataset.dropdownKey;
        const optionValue = target.dataset.dropdownOptionValue;
        if (!dropdownKey || optionValue == null) return;
        selectDropdown(dropdownKey, optionValue);
        return;
    }
    if (command === "open_hero_info") {
        menuUi.heroInfoOpen = true;
        render();
        return;
    }
    if (command === "close_hero_info") {
        menuUi.heroInfoOpen = false;
        render();
        return;
    }
    if (command === "preview_leader") {
        menuUi.tempSelectedLeader = target.dataset.leaderId || null;
        menuUi.tempSelectedRandom = false;
        if (!updateHeroesPreview()) render();
        maybePresentRewards();
        return;
    }
    if (command === "preview_random_leader") {
        menuUi.tempSelectedLeader = null;
        menuUi.tempSelectedRandom = true;
        if (!updateHeroesPreview()) render();
        return;
    }
    if (command === "confirm_random_leader") {
        stageLeaderSelection(true, null);
        send("set_random_leader");
        menuUi.heroesOpen = false;
        menuUi.heroesSearchQuery = "";
        menuUi.heroesRegionFilter = "all";
        menuUi.tempSelectedLeader = null;
        menuUi.tempSelectedRandom = false;
        render();
        return;
    }
    if (command === "confirm_leader") {
        const leaderId = target.dataset.leaderId || menuUi.tempSelectedLeader;
        const leader = leaderById(leaderId);
        if (leaderId && leader.available !== false) {
            stageLeaderSelection(false, leaderId);
            send("set_leader", { leader_id: leaderId });
            menuUi.heroesOpen = false;
            menuUi.heroesSearchQuery = "";
            menuUi.heroesRegionFilter = "all";
            menuUi.tempSelectedLeader = null;
            menuUi.tempSelectedRandom = false;
            render();
        }
        return;
    }
    /* POKI_SHARED_STORE_UNLOCK_BEGIN */
    if (command === "unlock_leader") {
        const unlockLeaderId = target.dataset.leaderId;
        const unlockCurrency = target.dataset.currency || "crowns";
        if (unlockLeaderId) openLeaderPurchase(unlockLeaderId, unlockCurrency);
        return;
    }
    if (command === "open_skin_purchase") {
        const purchaseSkinId = target.dataset.skinId;
        if (purchaseSkinId) openSkinPurchase(purchaseSkinId);
        return;
    }
    if (command === "open_skin_picker") {
        menuUi.skinPickerOpen = true;
        loadStoreCatalog();
        render();
        return;
    }
    if (command === "close_skin_picker") {
        menuUi.skinPickerOpen = false;
        render();
        return;
    }
    if (command === "confirm_purchase") {
        if (!menuUi.purchaseModal || menuUi.purchaseModal.submitted) return;
        menuUi.purchaseModal.submitted = true;
        if (menuUi.purchaseModal.type === "leader") {
            if (!(menuUi.state && menuUi.state.store_busy))
                send("unlock_leader", {
                    leader_id: menuUi.purchaseModal.leaderId,
                    currency: menuUi.purchaseModal.currency,
                });
        } else if (menuUi.purchaseModal.type === "skin") {
            if (!(menuUi.state && menuUi.state.store_busy))
                send("unlock_skin", { skin_id: menuUi.purchaseModal.skinId });
        }
        render();
        return;
    }
    if (command === "cancel_purchase") {
        if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
        if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
        else {
            menuUi.purchaseModal = null;
            menuUi.purchaseIntent = null;
        }
        render();
        return;
    }
    if (command === "unlock_skin" || command === "equip_skin") {
        const skinId = target.dataset.skinId;
        if (skinId && !(menuUi.state && menuUi.state.store_busy)) {
            send(command, { skin_id: skinId });
        }
        return;
    }
    /* POKI_SHARED_STORE_UNLOCK_END */
    if (command === "set_session_mode") {
        menuUi.createOffline = target.dataset.mode === "offline";
        render();
        return;
    }
    if (command === "set_create_mode") {
        const mode = target.dataset.mode;
        if (!mode) return;
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.game_mode = mode;
        render();
        return;
    }
    if (command === "set_create_diff") {
        const difficulty = target.dataset.diff;
        if (!difficulty) return;
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.bot_difficulty = difficulty;
        render();
        return;
    }
    if (command === "set_create_spawn") {
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.random_spawn = target.dataset.spawn === "true";
        render();
        return;
    }
    if (command === "set_create_private") {
        menuUi.createPrivate = target.dataset.private === "true";
        render();
        return;
    }
    if (command === "randomize_seed") {
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.seed = Math.floor(Math.random() * 9999) + 1;
        render();
        return;
    }
    if (command === "toggle_settings") {
        menuUi.settingsOpen = !menuUi.settingsOpen;
        if (!menuUi.settingsOpen) {
            menuUi.signOutConfirmOpen = false;
            menuUi.deleteArmed = false;
            deleteError = "";
        }
        menuUi.authModalOpen = false;
        render();
        return;
    }
    /* POKI_SHARED_AUTH_ACTIONS_BEGIN */
    if (command === "close_auth") {
        menuUi.authModalOpen = false;
        render();
        return;
    }
    if (command === "auth_change_email") {
        menuUi.authOtpSent = false;
        menuUi.authCode = "";
        menuUi.authError = "";
        menuUi.authNotice = "";
        render();
        return;
    }
    if (command === "auth_resend") {
        requestAuthOtp();
        return;
    }
    if (command === "wou_provider") {
        if (typeof window.SOW_startWouOAuth === "function") window.SOW_startWouOAuth(target.dataset.provider);
        return;
    }
    if (command === "sign_in") {
        if (isAndroidTwa()) {
            send(command);
        } else {
            menuUi.settingsOpen = false;
            menuUi.signOutConfirmOpen = false;
            resetAuthFlow();
            menuUi.authModalOpen = true;
            render();
        }
        return;
    }
    if (command === "sign_out") {
        menuUi.signOutConfirmOpen = true;
        render();
        return;
    }
    if (command === "cancel_sign_out") {
        menuUi.signOutConfirmOpen = false;
        render();
        return;
    }
    if (command === "confirm_sign_out") {
        menuUi.signOutConfirmOpen = false;
        if (isAndroidTwa()) send("sign_out");
        else if (typeof window.SOW_signOutWou === "function") window.SOW_signOutWou();
        return;
    }
    /* POKI_SHARED_AUTH_ACTIONS_END */
    if (command === "close_password") {
        menuUi.passwordLobbyId = null;
        menuUi.passwordDraft = "";
        render();
        return;
    }
    if (command === "copy_lobby_code") {
        const lobbyCode = String(target.dataset.lobbyId || "");
        if (navigator.clipboard && lobbyCode) {
            navigator.clipboard
                .writeText(lobbyCode)
                .then(function () {
                    target.textContent = SOW_t("menu.copied");
                })
                .catch(function () {
                    target.textContent = SOW_t("menu.copy_failed");
                });
        } else if (lobbyCode) {
            target.textContent = SOW_t("menu.copy_unavailable");
        }
        return;
    }
    if (command === "ban_player" && !window.confirm(SOW_t("auth.ban_player_confirm"))) {
        return;
    }
    if (command === "join_lobby") {
        const selectedLobby = findLobby(Number(target.dataset.lobbyId));
        if (selectedLobby && selectedLobby.has_password) {
            menuUi.passwordLobbyId = selectedLobby.id;
            menuUi.passwordDraft = "";
            render();
        } else {
            send("join_lobby", { lobby_id: Number(target.dataset.lobbyId) });
        }
        return;
    }
    if (command === "set_leader") {
        if (send("set_leader", { leader_id: target.dataset.leaderId })) {
            menuUi.heroesOpen = false;
        }
        return;
    }
    const payload: { lobby_id?: number; target_player_id?: number } = {};
    if (target.dataset.lobbyId) payload.lobby_id = Number(target.dataset.lobbyId);
    if (target.dataset.playerId) payload.target_player_id = Number(target.dataset.playerId);
    send(command, payload);
});

document.addEventListener("pointerdown", function (event) {
    const target = event.target instanceof Element ? event.target : null;
    const interactive =
        target?.closest("[data-command], button, input, textarea, a, [role='button']") || null;
    if (interactive && root.contains(interactive)) activeMenuPointerId = event.pointerId;
    const dropdownElement = target && target.closest("[data-control-dropdown]");
    const dropdown = dropdownElement instanceof HTMLElement ? dropdownElement : null;
    if (menuUi.dropdownOpenKey && (!dropdown || dropdown.dataset.dropdownKey !== menuUi.dropdownOpenKey)) {
        setDropdownOpen(menuUi.dropdownOpenKey, false);
    }
});

function finishMenuPointer(event: PointerEvent) {
    if (activeMenuPointerId !== event.pointerId || renderFlushTimer !== null) return;
    renderFlushTimer = setTimeout(function () {
        renderFlushTimer = null;
        activeMenuPointerId = null;
        if (renderPending) render();
    }, 0);
}

document.addEventListener("pointerup", finishMenuPointer);
document.addEventListener("pointercancel", finishMenuPointer);

/* POKI_SHARED_AUTH_EVENTS_BEGIN */
window.addEventListener("wou:auth-state-change", function () {
    if (!menuUi.state || root.hidden) return;
    const auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() || {} : {};
    if (auth.linked && menuUi.authModalOpen) {
        menuUi.authModalOpen = false;
        resetAuthFlow();
    }
    if (!root.querySelector("[data-screen-stage]")) {
        render();
        return;
    }
    updateFrameChrome(currentScreen());
    updateDynamic();
    if (menuUi.settingsOpen) syncOverlay("settings", renderSettings());
    if (menuUi.authModalOpen) syncOverlay("auth", renderAuthModal());
    syncProfileDom();
});

window.addEventListener("wou:auth-error", function (event) {
    menuUi.authBusy = false;
    menuUi.authModalOpen = true;
    const detail = event.detail as { message?: unknown } | null;
    menuUi.authError =
        detail && typeof detail.message === "string" ? detail.message : SOW_t("auth.sign_in_unavailable");
    render();
});
/* POKI_SHARED_AUTH_EVENTS_END */

/* POKI_SHARED_STORE_EVENTS_BEGIN */
window.addEventListener("sow:android-purchase-bridge-ready", function () {
    if (menuUi.state && !root.hidden) render();
});

window.addEventListener("sow:android-purchase-result", function (event) {
    const result = event.detail as { request_id?: string; status?: string; product_id?: string };
    if (
        result.request_id &&
        menuUi.storeCheckoutRequestId &&
        result.request_id !== menuUi.storeCheckoutRequestId
    )
        return;
    menuUi.storeCheckoutBusy = false;
    if (!menuUi.state) return;
    menuUi.storeCheckoutRequestId = null;
    if (result.status === "success") {
        menuUi.state.error = null;
        const attempt =
            typeof currentExternalPurchaseAttempt === "function" ? currentExternalPurchaseAttempt() : null;
        if (attempt && (!result.product_id || attempt.product_id === result.product_id)) {
            completeExternalPurchase(attempt.product_id);
        } else {
            menuUi.purchaseIntent = null;
            send("refresh_profile");
        }
    } else if (result.status === "restored") {
        menuUi.purchaseIntent = null;
        send("refresh_profile");
    } else if (result.status === "cancelled") {
        if (typeof abandonExternalPurchaseAttempt === "function")
            abandonExternalPurchaseAttempt(result.product_id);
        if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
        else {
            menuUi.purchaseModal = null;
            menuUi.purchaseIntent = null;
        }
        menuUi.state.error = SOW_t("menu.purchase_cancelled");
    } else {
        if (typeof abandonExternalPurchaseAttempt === "function")
            abandonExternalPurchaseAttempt(result.product_id);
        if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
        else {
            menuUi.purchaseModal = null;
            menuUi.purchaseIntent = null;
        }
        menuUi.state.error = SOW_t("menu.purchase_failed");
    }
    render();
});
/* POKI_SHARED_STORE_EVENTS_END */

root.addEventListener("keydown", function (event) {
    const keyboardTarget = event.target instanceof Element ? event.target : null;
    if (!keyboardTarget) return;
    const profileTabElement = keyboardTarget.closest("[role='tab'][data-command='profile_tab']");
    const profileTab = profileTabElement instanceof HTMLElement ? profileTabElement : null;
    if (profileTab && ["ArrowLeft", "ArrowRight", "Home", "End"].indexOf(event.key) >= 0) {
        const profileTabs = Array.from(
            root.querySelectorAll<HTMLElement>("[role='tab'][data-command='profile_tab']"),
        );
        const profileTabIndex = profileTabs.indexOf(profileTab);
        if (profileTabs.length && profileTabIndex >= 0) {
            const nextProfileTabIndex =
                event.key === "Home"
                    ? 0
                    : event.key === "End"
                      ? profileTabs.length - 1
                      : (profileTabIndex + (event.key === "ArrowRight" ? 1 : -1) + profileTabs.length) %
                        profileTabs.length;
            event.preventDefault();
            profileTabs[nextProfileTabIndex].click();
            const selectedProfileTab = root.querySelector<HTMLElement>("[role='tab'][aria-selected='true']");
            if (selectedProfileTab) selectedProfileTab.focus();
            return;
        }
    }
    const dropdownTriggerElement = keyboardTarget.closest("[data-role='dropdown-trigger']");
    const dropdownTrigger = dropdownTriggerElement instanceof HTMLElement ? dropdownTriggerElement : null;
    const dropdownOptionElement = keyboardTarget.closest(
        "[data-role='dropdown-option'], [data-command='select_dropdown']",
    );
    const dropdownOption = dropdownOptionElement instanceof HTMLElement ? dropdownOptionElement : null;
    if (dropdownTrigger) {
        if (
            event.key === "ArrowDown" ||
            event.key === "ArrowUp" ||
            event.key === "Enter" ||
            event.key === " "
        ) {
            event.preventDefault();
            const triggerKey = dropdownTrigger.dataset.dropdownKey;
            if (!triggerKey) return;
            setDropdownOpen(triggerKey, true);
            const triggerOptions = dropdownTrigger.parentElement!.querySelectorAll<HTMLElement>(
                "[data-command='select_dropdown']",
            );
            if (triggerOptions.length)
                triggerOptions[event.key === "ArrowUp" ? triggerOptions.length - 1 : 0].focus();
            return;
        }
    }
    if (dropdownOption) {
        const dropdown = dropdownOption.closest("[data-control-dropdown]");
        const options = dropdown
            ? Array.from(dropdown.querySelectorAll<HTMLElement>("[data-command='select_dropdown']"))
            : [];
        const optionIndex = options.indexOf(dropdownOption);
        let nextIndex = optionIndex;
        if (event.key === "ArrowDown") nextIndex = Math.min(options.length - 1, optionIndex + 1);
        if (event.key === "ArrowUp") nextIndex = Math.max(0, optionIndex - 1);
        if (event.key === "Home") nextIndex = 0;
        if (event.key === "End") nextIndex = options.length - 1;
        if (nextIndex !== optionIndex) {
            event.preventDefault();
            options[nextIndex].focus();
            return;
        }
        if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            dropdownOption.click();
            return;
        }
    }
    if (event.key === "Escape" && menuUi.dropdownOpenKey) {
        event.preventDefault();
        const openKey = menuUi.dropdownOpenKey;
        setDropdownOpen(
            openKey,
            false,
            root.querySelector<HTMLElement>(
                "[data-control-dropdown][data-dropdown-key='" + openKey + "'] [data-role='dropdown-trigger']",
            ),
        );
        return;
    }
    if (event.key === "Escape" && menuUi.settingsOpen) {
        event.preventDefault();
        if (menuUi.signOutConfirmOpen) {
            menuUi.signOutConfirmOpen = false;
            render();
            return;
        }
        menuUi.settingsOpen = false;
        menuUi.signOutConfirmOpen = false;
        menuUi.deleteArmed = false;
        deleteError = "";
        menuUi.dropdownOpenKey = null;
        render();
        return;
    }
    if (event.key === "Escape" && menuUi.purchaseModal) {
        event.preventDefault();
        if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
        if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
        else {
            menuUi.purchaseModal = null;
            menuUi.purchaseIntent = null;
        }
        render();
        return;
    }
    if (event.key === "Escape" && (menuUi.skinPickerOpen || menuUi.heroInfoOpen)) {
        event.preventDefault();
        menuUi.skinPickerOpen = false;
        menuUi.heroInfoOpen = false;
        render();
        return;
    }
    if (event.key === "Escape" && menuUi.profileMatchDetail) {
        menuUi.profileMatchDetail = null;
        render();
        return;
    }
    if (event.key === "Enter") {
        const input = event.target;
        if (input instanceof HTMLInputElement && input.dataset.role === "display-name") {
            event.preventDefault();
            input.blur();
            return;
        }
    }
    if (event.key !== "Enter" && event.key !== " ") return;
    const targetElement = keyboardTarget.closest("[data-command='join_lobby']");
    if (!(targetElement instanceof HTMLElement) || !root.contains(targetElement)) return;
    event.preventDefault();
    targetElement.click();
});

root.addEventListener("submit", function (event) {
    const form = event.target as HTMLFormElement;
    /* POKI_SHARED_AUTH_SUBMIT_BEGIN */
    if (form.dataset.authForm === "request") {
        event.preventDefault();
        requestAuthOtp();
        return;
    }
    if (form.dataset.authForm === "verify") {
        event.preventDefault();
        verifyAuthOtp();
        return;
    }
    /* POKI_SHARED_AUTH_SUBMIT_END */
    if (form.dataset.form === "report") {
        event.preventDefault();
        if (reportBusy) return;
        const repCreds = selfCreds();
        const reasonField = form.elements.namedItem("reason") as
            RadioNodeList | HTMLInputElement | HTMLSelectElement | null;
        const detailsField = form.elements.namedItem("details") as
            HTMLInputElement | HTMLTextAreaElement | null;
        const reason = reasonField ? reasonField.value : "";
        const details = detailsField ? detailsField.value.trim() : "";
        const reportedAccountId = menuUi.reportTarget;
        if (!repCreds || !reportedAccountId) {
            menuUi.reportOpen = false;
            render();
            return;
        }
        if (reason === "other" && !details) {
            render();
            return;
        }
        reportBusy = true;
        render();
        fetch(profileApi("/profile/anonymous/report"), {
            method: "POST",
            headers: { "Content-Type": "application/json", Accept: "application/json" },
            body: JSON.stringify({
                account_id: repCreds.account_id,
                auth_secret: repCreds.auth_secret,
                reported_account_id: reportedAccountId,
                reason: reason,
                details: details || null,
            }),
        })
            .then(function (response) {
                if (!response.ok) throw new Error("report failed");
                return response.json();
            })
            .then(function () {
                menuUi.reportSent = true;
                menuUi.reportOpen = false;
                try {
                    const list = window.SOW_BLOCKED_IDS || [];
                    if (reportedAccountId && list.indexOf(reportedAccountId) === -1)
                        list.push(reportedAccountId);
                    window.SOW_BLOCKED_IDS = list;
                } catch {}
            })
            .catch(function () {})
            .finally(function () {
                reportBusy = false;
                render();
            });
        return;
    }
    if (form.dataset.form === "join") {
        event.preventDefault();
        const code = (form.elements.namedItem("code") as HTMLInputElement).value.trim();
        send("join_code", { code: code });
    }
    if (form.dataset.form === "password") {
        event.preventDefault();
        menuUi.passwordDraft = (form.elements.namedItem("password") as HTMLInputElement).value;
        if (menuUi.passwordLobbyId != null && menuUi.passwordDraft) {
            send("join_with_password", { lobby_id: menuUi.passwordLobbyId, password: menuUi.passwordDraft });
        }
    }
    if (form.dataset.form === "create") {
        event.preventDefault();
        syncCreateDraft(form);
        const config = menuUi.createDraft || cloneConfig();
        if (menuUi.createOffline) {
            send("start_single_player", { config: config });
        } else {
            send("create_game", {
                config: config,
                is_private: menuUi.createPrivate,
                password: menuUi.createPassword || null,
            });
        }
    }
});

root.addEventListener("input", function (event) {
    const input = event.target;
    if (
        !(input instanceof HTMLInputElement) &&
        !(input instanceof HTMLTextAreaElement) &&
        !(input instanceof HTMLSelectElement)
    )
        return;
    if (input.dataset.authField === "email") menuUi.authEmail = input.value;
    if (input.dataset.authField === "code") menuUi.authCode = input.value.replace(/\D/g, "").slice(0, 6);
    if (input.dataset.role === "browser-search") {
        menuUi.browserSearchQuery = input.value;
        const publicPanel = root.querySelector(".sow-menu__public");
        if (publicPanel) {
            updateLobbyViews();
        }
    }
    if (input.dataset.role === "heroes-search") {
        menuUi.heroesSearchQuery = input.value;
        refreshHeroesRoster(true);
    }
    const createForm = input.closest("form[data-form='create']");
    if (createForm instanceof HTMLFormElement) {
        syncCreateDraft(createForm);
        const valBadge = createForm.querySelector<HTMLElement>("[data-val-for='" + input.name + "']");
        if (valBadge) valBadge.textContent = input.value;
    }
    if (input.dataset.setting === "music_volume") {
        const musicValBadge = root.querySelector<HTMLElement>("[data-val-for='music_vol']");
        if (musicValBadge) musicValBadge.textContent = Math.round(Number(input.value) * 100) + "%";
        if (menuUi.state && menuUi.state.phase === "MainMenu")
            syncMenuMusic(menuUi.state, true, Number(input.value));
    }
    if (input.name === "password" && input.closest("form[data-form='password']")) {
        menuUi.passwordDraft = input.value;
    }
});

root.addEventListener("change", function (event) {
    const input = event.target;
    if (!(input instanceof HTMLInputElement) && !(input instanceof HTMLSelectElement)) return;
    if (input.dataset.role === "display-name") {
        const name = input.value.trim();
        if (name && name !== menuUi.state!.player_name) send("save_display_name", { name: name });
        return;
    }
    const createForm = input.closest("form[data-form='create']");
    if (createForm instanceof HTMLFormElement) {
        syncCreateDraft(createForm);
        render();
        return;
    }
    if (input.dataset.setting === "locale") {
        if (typeof window.SOW_setLocale === "function") window.SOW_setLocale(input.value);
        return;
    }
    if (!input.dataset.setting) return;
    if (input.dataset.setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
    if (input.dataset.setting === "reduced_motion")
        send("set_reduced_motion", { value: input.value === "reduced" });
    if (input instanceof HTMLInputElement && input.dataset.setting === "free_zoom_out")
        send("set_free_zoom_out", { value: input.checked });
    if (input instanceof HTMLInputElement && input.dataset.setting === "show_fps_ping")
        send("set_show_fps_ping", { value: input.checked });
    if (input instanceof HTMLInputElement && input.dataset.setting === "show_dev_tools")
        send("set_show_dev_tools", { value: input.checked });
});

function normalizeCompletedExitState(nextState: MenuState): MenuState {
    if (
        nextState.phase === "MainMenu" &&
        nextState.loader_job === "ExitGame" &&
        Number.isSafeInteger(nextState.loader_cycle_id) &&
        nextState.loader_cycle_id <= completedLoaderCycleId
    ) {
        return Object.assign({}, nextState, { loader_job: "Boot", loader_leader: null });
    }
    return nextState;
}

function syncWebLoaderForState(nextState: MenuState) {
    if (typeof window.SOW_syncWebLoader === "function") window.SOW_syncWebLoader(nextState);
}

function scheduleRewardPresentation(delay: number) {
    if (
        !menuUi.state ||
        menuUi.state.phase !== "MainMenu" ||
        menuUi.pendingExitScreenIntro ||
        rewardPresentationReady ||
        rewardAnimationTimer !== null
    )
        return;
    rewardAnimationTimer = window.setTimeout(function () {
        rewardAnimationTimer = null;
        if (!menuUi.state || menuUi.state.phase !== "MainMenu") return;
        rewardPresentationReady = true;
        maybePresentRewards();
    }, delay);
}

window.addEventListener("sow:loader-cycle-ready", function (event) {
    const detail = event && (event.detail as { cycle_id?: number } | null);
    const cycleId = detail && detail.cycle_id;
    if (
        typeof cycleId !== "number" ||
        !Number.isSafeInteger(cycleId) ||
        cycleId < latestLoaderCycleId ||
        cycleId < completedLoaderCycleId
    )
        return;
    completedLoaderCycleId = cycleId;
    menuMusicFinishLoaderCycle(cycleId);
    if (
        !menuUi.state ||
        menuUi.state.phase !== "MainMenu" ||
        menuUi.state.loader_job !== "ExitGame" ||
        menuUi.state.loader_cycle_id !== cycleId
    )
        return;

    const replayExitIntro = menuUi.pendingExitScreenIntro;
    menuUi.state = normalizeCompletedExitState(menuUi.state);
    reconcileLeaderSelection(menuUi.state);
    syncMenuMusic(menuUi.state, false);
    if (replayExitIntro) {
        menuUi.pendingExitScreenIntro = false;
        menuUi.previousScreen = null;
    }
    render();
    if (replayExitIntro) scheduleRewardPresentation(250);
});

document.addEventListener("visibilitychange", function () {
    syncHeroesRotationCountdown();
    if (document.visibilityState !== "visible") {
        if (window.SOW_menu_guide) window.SOW_menu_guide.pause();
        if (menuUi.state && (rewardAnimationRunning || progressionAnimationTarget)) {
            cancelRewardAnimation();
            writeProgression(progressionFromState());
            const unpresentedReceipt = pendingRewardReceipts().some(function (receipt) {
                return !rewardAnimationShown.has(receipt.id);
            });
            if (unpresentedReceipt) displayedProgression = null;
        }
        return;
    }
    if (!menuUi.state || menuUi.state.phase !== "MainMenu") return;
    if (typeof resumeExternalPurchaseDelivery === "function") resumeExternalPurchaseDelivery(true);
    if (!rewardPresentationReady) scheduleRewardPresentation(0);
    else maybePresentRewards();
});

function handleMenuStateUpdate(raw: string, parsed: MenuState) {
    if (typeof raw !== "string" || raw === menuUi.lastRaw) {
        updateDynamic();
        return;
    }
    menuUi.lastRaw = raw;
    menuUi.state = parsed;
    if (
        !Number.isSafeInteger(menuUi.state!.loader_cycle_id) ||
        menuUi.state!.loader_cycle_id < latestLoaderCycleId
    )
        return;
    latestLoaderCycleId = menuUi.state!.loader_cycle_id;
    menuUi.state = normalizeCompletedExitState(menuUi.state);
    syncDisconnectNotice(menuUi.state);
    syncMenuMusic(menuUi.state, false);
    const accountId = menuUi.state!.account_id || "";
    if (rewardAnimationAccount !== accountId) {
        rewardAnimationAccount = accountId;
        rewardAnimationShown.clear();
        rewardAckTimes = Object.create(null);
        if (rewardAckRetryTimer !== null) window.clearTimeout(rewardAckRetryTimer);
        rewardAckRetryTimer = null;
        displayedProgression = null;
        rewardPresentationReady = false;
        cancelRewardAnimation();
        if (rewardAnimationTimer !== null) window.clearTimeout(rewardAnimationTimer);
        rewardAnimationTimer = null;
    }
    if (typeof resumeExternalPurchaseDelivery === "function") resumeExternalPurchaseDelivery(false);
    if (typeof resolvePurchaseModal === "function") resolvePurchaseModal();
    const returnedFromGame =
        menuUi.lastMenuPhase !== null &&
        menuUi.lastMenuPhase !== "MainMenu" &&
        menuUi.state!.phase === "MainMenu" &&
        menuUi.state!.loader_job === "ExitGame";
    menuUi.lastMenuPhase = menuUi.state!.phase;
    if (menuUi.state!.phase !== "MainMenu") {
        rewardPresentationReady = false;
        if (rewardAnimationRunning) {
            cancelRewardAnimation();
            writeProgression(progressionFromState());
        }
        if (rewardAnimationTimer !== null) window.clearTimeout(rewardAnimationTimer);
        rewardAnimationTimer = null;
        maybePresentRewards();
    }
    if (returnedFromGame) {
        menuUi.profileOpen = false;
        menuUi.heroesOpen = false;
        menuUi.campaignOpen = false;
        menuUi.storeOpen = false;
        menuUi.pendingExitScreenIntro = true;
    }
    if (typeof window.SOW_tutorial_menu_state_update === "function") {
        window.SOW_tutorial_menu_state_update(menuUi.state);
    }
    syncProfilePreload(menuUi.state!);
    if (menuUi.state!.waiting && menuUi.passwordLobbyId != null) {
        menuUi.passwordLobbyId = null;
        menuUi.passwordDraft = "";
    }
    const key = renderKey();
    if (key !== menuUi.lastRenderKey) {
        render();
    } else {
        root.hidden = menuUi.state!.phase !== "MainMenu";
        updateLobbyViews();
        updateDynamic();
    }
    syncWebLoaderForState(menuUi.state);
    if (menuUi.state!.phase === "MainMenu") {
        scheduleRewardPresentation(250);
        maybePresentRewards();
    }
}

window.addEventListener("sow:locale-change", function () {
    if (menuUi.state && !root.hidden) render();
});

window.SOW_menu_state_update = handleMenuStateUpdate;
root.hidden = true;
