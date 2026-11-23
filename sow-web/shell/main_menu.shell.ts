import { skinIconAttributes, skinName, syncMenuSkinTheme } from "./main_menu.skins.ts";
import {
    AudioCue,
    audioEngine,
    consumeScreenSwitchSoundSuppression,
    installSurfaceHoverPressAudio,
    MenuSfx,
    playUiCue,
    syncAudioSettings,
} from "./audio/index.ts";
import { formatCompactNumber } from "./number-format.ts";
import {
    avatarBackgroundCss,
    avatarImageAttribute,
    setAvatarBackground,
    setAvatarImage,
} from "./avatar-presentation.ts";
import { SOWTooltip } from "./sow-tooltip.ts";
import {
    animateRankResult,
    rankEmblem,
    rankName,
    rankProgressMarkup,
    rankTier,
    rankedPointsBreakdownMarkup,
    renderRankedDuel,
} from "./main_menu.ranked.ts";
import menuFlowApi from "./main_menu.flow.ts";
import {
    MAIN_NAV_ITEMS,
    asset,
    avatarImage,
    computeMainNavIndices,
    currencyAsset,
    currentLeaderSelection,
    currentScreen,
    esc,
    findLobby,
    selectedHeroBackdrop,
    leaderById,
    leaderArtSlug,
    leaderDisplayName,
    menuIcon,
    menuUi,
    portalOffersStore,
    profileHeroImage,
    reconcileLeaderSelection,
    renderKey,
    root,
    selfCreds,
    send,
    stageLeaderSelection,
} from "./main_menu.core.ts";
import {
    ensureHeroBiosLoaded,
    refreshHeroesRoster,
    renderHeroInfoModal,
    renderHeroes,
    selectDropdown,
    settleHeroesFeaturedArt,
    setDropdownOpen,
    syncDropdowns,
    updateHeroesPreview,
} from "./main_menu.heroes.ts";
import {
    campaignEpisodeSagaId,
    campaignBackdropArt,
    cloneConfig,
    cycleCreateMap,
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
    selectCreateMap,
    stepCreateSlider,
    syncCreateDraft,
    updateCampaignSelection,
    updateLobbyViews,
    updateQueueDownloadProgress,
    updateQueueRoster,
    updateRankedQueue,
} from "./main_menu.lobbies.ts";
import {
    accelerateChoreographyPlan,
    applyGpuPerfTier,
    flyArcPointsToKeyframes,
    planRewardChoreography,
    sampleFlyArcPoints,
    sowScreenMotion,
    type RewardChoreographyPlan,
    type RewardPlaybackMode,
} from "./main_menu.motion.ts";
import {
    enrichAchievement,
    getAchievementDefinition,
} from "./main_menu.achievements.ts";
import {
    hasHandledMatchGoalToast,
    hasViewedMatchGoals,
    latestMatchGoalSummary,
    markMatchGoalToastHandled,
    selectMatchGoalHighlights,
    type MatchGoalAchievement,
    type MatchGoalHighlight,
    type MatchGoalSummary,
} from "./main_menu.match-goals.ts";
applyGpuPerfTier();
import {
    activeProfileEntry,
    bumpAchievementFlatLimit,
    invalidateProfileCache,
    loadMoreProfileHistory,
    loadProfile,
    loadRankedLeaderboard,
    loadVictoryLeaderboard,
    openProfile,
    popProfileNav,
    profileApi,
    refreshProfileAchievements,
    renderProfile,
    renderProfileDetail,
    resetAchievementFlatLimit,
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
    openOfferPurchase,
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
    surfaceSettingsRenderKey,
    verifyAuthOtp,
} from "./main_menu.surfaces.ts";
import {
    cancelSwapMission,
    claimMission,
    confirmSwapMission,
    isRewardedAdAvailable,
    renderMissionsModal,
    requestSwapMission,
    setMissionClaimHandler,
    missionIconSvg,
    videoPlayIconSvg,
} from "./main_menu.missions.ts";
import type { MenuMission, MenuRewardReceipt, MenuState, RankedResult } from "./main_menu.state.ts";

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
let isDraggingSettingsSlider = false;
let lastSettingsKey = "";
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
let activeRewardRunner: { cancel: () => void } | null = null;
const rewardClassTimers = new WeakMap<HTMLElement, Record<string, number>>();
const rewardClassSeq = new WeakMap<HTMLElement, Record<string, number>>();
let rankedResultDismissedKey: string | null = null;
let rankedStageKey: string | null = null;
let rankedStageCleanup: (() => void) | null = null;
let rewardPresentationReady = false;
let activeMatchGoalToast: HTMLElement | null = null;
let matchGoalToastTimer: number | null = null;
let matchGoalVisibilityHandler: (() => void) | null = null;
const noMatchGoalReceipts: MenuRewardReceipt[] = [];
let matchGoalSummaryCache: {
    accountId: string;
    receipts: MenuRewardReceipt[];
    summary: MatchGoalSummary | null;
} | null = null;
let pendingCreateMode: string | null = null;
const menuFlow = menuFlowApi.create();
window.SOW_menu_flow = menuFlow;
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
    audioContext: AudioContext | null;
    spectrumSources: Array<MediaElementAudioSourceNode | null> | null;
    spectrumGains: Array<GainNode | null> | null;
    spectrumAnalyzers: Array<AnalyserNode | null> | null;
    spectrumData: Array<Uint8Array<ArrayBuffer> | null> | null;
    spectrumPending: boolean;
    spectrumUnavailable: boolean;
    spectrumFrame: number;
    active: number;
    state: MenuState | null;
    activated: boolean;
    completedCycleId: number;
    playBlocked: boolean;
    blockedDeck: number | null;
    pendingVolume: number | null;
    crossfade: MenuMusicCrossfade | null;
    mediaMetadataSet: boolean;
    fetchRequested: boolean;
    frame: number;
}

type RewardStage =
    | { kind: "leader"; amount: 0; slug: string; label: string; selector?: string }
    | { kind: "achievement"; amount: 0; label: string; selector: string }
    | { kind: "level"; amount: number; level: number; selector: string }
    | { kind: "xp" | "crowns" | "laurels" | "gems"; amount: number; selector: string };

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
    audioContext: null,
    spectrumSources: null,
    spectrumGains: null,
    spectrumAnalyzers: null,
    spectrumData: null,
    spectrumPending: false,
    spectrumUnavailable: false,
    spectrumFrame: 0,
    active: 0,
    state: null,
    activated: false,
    completedCycleId: -1,
    playBlocked: false,
    blockedDeck: null,
    pendingVolume: null,
    crossfade: null,
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
        const volume = base * (mixes[index] || 0) * factor;
        const gain = menuMusic.spectrumGains && menuMusic.spectrumGains[index];
        if (gain) gain.gain.value = volume;
        else deck.volume = volume;
    });
}

function menuMusicSpectrumCanvas(): HTMLCanvasElement | null {
    return root.querySelector<HTMLCanvasElement>("[data-menu-music-spectrum]");
}

function menuMusicCancelSpectrumFrame() {
    if (menuMusic.spectrumFrame) window.cancelAnimationFrame(menuMusic.spectrumFrame);
    menuMusic.spectrumFrame = 0;
}

function menuMusicSpectrumIsActive(canvas: HTMLCanvasElement | null) {
    return (
        !!canvas &&
        menuUi.settingsOpen &&
        !root.hidden &&
        !!menuUi.state &&
        menuUi.state.phase === "MainMenu" &&
        !menuMusicLoaderActive(menuUi.state) &&
        document.visibilityState === "visible"
    );
}

function menuMusicSpectrumSameOrigin() {
    if (!menuMusic.decks || !window.location || !window.location.origin) return true;
    try {
        return menuMusic.decks.every(function (deck) {
            return (
                new window.URL(deck.currentSrc || deck.src, document.baseURI).origin ===
                window.location.origin
            );
        });
    } catch {
        return false;
    }
}

function menuMusicConnectSpectrum(context: AudioContext) {
    if (!menuMusic.decks || menuMusic.spectrumGains) return;
    const sources: Array<MediaElementAudioSourceNode | null> = [];
    const gains: Array<GainNode | null> = [];
    const analyzers: Array<AnalyserNode | null> = [];
    const data: Array<Uint8Array<ArrayBuffer> | null> = [];
    menuMusic.decks.forEach(function (deck) {
        let source: MediaElementAudioSourceNode | null = null;
        let gain: GainNode | null = null;
        let analyser: AnalyserNode | null = null;
        try {
            source = context.createMediaElementSource(deck);
            gain = context.createGain();
            gain.gain.value = deck.volume;
            try {
                analyser = context.createAnalyser();
                analyser.fftSize = 512;
                analyser.smoothingTimeConstant = 0.72;
                analyser.minDecibels = -84;
                analyser.maxDecibels = -24;
                source.connect(gain);
                gain.connect(analyser);
                analyser.connect(context.destination);
            } catch {
                try {
                    source.disconnect();
                    gain.disconnect();
                    if (analyser) analyser.disconnect();
                } catch {}
                analyser = null;
                source.connect(gain);
                gain.connect(context.destination);
            }
            deck.volume = 1;
        } catch (error) {
            if (source) {
                try {
                    if (gain) {
                        gain.gain.value = deck.volume;
                        source.disconnect();
                        gain.disconnect();
                        source.connect(gain);
                        gain.connect(context.destination);
                        deck.volume = 1;
                    } else source.connect(context.destination);
                } catch {}
            }
            console.warn("[SOW MUSIC] spectrum analysis unavailable for a music deck:", error);
            analyser = null;
        }
        sources.push(source);
        gains.push(gain);
        analyzers.push(analyser);
        data.push(analyser ? new Uint8Array(analyser.frequencyBinCount) : null);
    });
    menuMusic.spectrumSources = sources;
    menuMusic.spectrumGains = gains;
    menuMusic.spectrumAnalyzers = analyzers;
    menuMusic.spectrumData = data;
}

function menuMusicPrepareSpectrum() {
    if (!menuMusic.decks || menuMusic.spectrumPending || menuMusic.spectrumUnavailable) return;
    if (menuMusic.audioContext) {
        if (menuMusic.audioContext.state === "suspended") {
            menuMusic.spectrumPending = true;
            menuMusic.audioContext
                .resume()
                .then(function () {
                    menuMusic.spectrumPending = false;
                    menuMusicSyncSpectrum();
                })
                .catch(function () {
                    menuMusic.spectrumPending = false;
                });
        }
        return;
    }
    if (!menuMusicSpectrumSameOrigin()) {
        menuMusic.spectrumUnavailable = true;
        const canvas = menuMusicSpectrumCanvas();
        if (canvas) canvas.hidden = true;
        return;
    }
    const context = audioEngine.ensureContext();
    if (!context) {
        menuMusic.spectrumUnavailable = true;
        const canvas = menuMusicSpectrumCanvas();
        if (canvas) canvas.hidden = true;
        return;
    }
    menuMusic.audioContext = context;
    menuMusic.spectrumPending = true;
    context
        .resume()
        .then(function () {
            if (context.state !== "running") throw new Error("Web Audio context did not resume");
            menuMusicConnectSpectrum(context);
            menuMusic.spectrumPending = false;
            if (!menuMusic.spectrumAnalyzers?.some(Boolean)) {
                menuMusic.spectrumUnavailable = true;
                const canvas = menuMusicSpectrumCanvas();
                if (canvas) canvas.hidden = true;
                return;
            }
            menuMusicSyncSpectrum();
        })
        .catch(function () {
            menuMusic.spectrumPending = false;
            menuMusic.audioContext = null;
            menuMusic.spectrumUnavailable = true;
            // Shared AudioContext belongs to AudioEngine — never close it on spectrum failure.
            const canvas = menuMusicSpectrumCanvas();
            if (canvas) canvas.hidden = true;
        });
}

function menuMusicDrawSpectrum(canvas: HTMLCanvasElement) {
    const context = canvas.getContext("2d");
    if (!context) {
        canvas.hidden = true;
        return;
    }
    const width = Math.max(1, canvas.clientWidth || canvas.width || 240);
    const height = Math.max(1, canvas.clientHeight || canvas.height || 20);
    const pixelRatio = Math.max(1, window.devicePixelRatio || 1);
    const pixelWidth = Math.round(width * pixelRatio);
    const pixelHeight = Math.round(height * pixelRatio);
    if (canvas.width !== pixelWidth) canvas.width = pixelWidth;
    if (canvas.height !== pixelHeight) canvas.height = pixelHeight;
    context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
    context.clearRect(0, 0, width, height);
    canvas.hidden = false;

    const analyzers = menuMusic.spectrumAnalyzers || [];
    if (menuMusicMuted() || menuMusicVolume() <= 0) {
        context.fillStyle = "rgb(243 177 43 / 42%)";
        context.fillRect(0, height - 1, width, 1);
        return;
    }
    if (!analyzers.some(Boolean)) {
        canvas.hidden = true;
        return;
    }

    const barCount = 24;
    const levels = new Float32Array(barCount);
    analyzers.forEach(function (analyser, deckIndex) {
        const bins = menuMusic.spectrumData && menuMusic.spectrumData[deckIndex];
        if (!analyser || !bins) return;
        analyser.getByteFrequencyData(bins);
        for (let bar = 0; bar < barCount; bar += 1) {
            const start = Math.floor((bar / barCount) ** 2 * bins.length);
            const end = Math.max(start + 1, Math.floor(((bar + 1) / barCount) ** 2 * bins.length));
            let total = 0;
            for (let bin = start; bin < Math.min(end, bins.length); bin += 1) total += bins[bin];
            levels[bar] = Math.min(255, levels[bar] + total / Math.max(1, end - start));
        }
    });

    const gap = 2;
    const barWidth = Math.max(1, (width - gap * (barCount - 1)) / barCount);
    context.fillStyle = "#f3b12b";
    levels.forEach(function (level, bar) {
        const barHeight = (level / 255) * height;
        if (barHeight > 0.5)
            context.fillRect(bar * (barWidth + gap), height - barHeight, barWidth, barHeight);
    });
}

function menuMusicScheduleSpectrumFrame() {
    if (menuMusic.spectrumFrame) return;
    menuMusic.spectrumFrame = window.requestAnimationFrame(function () {
        menuMusic.spectrumFrame = 0;
        const canvas = menuMusicSpectrumCanvas();
        if (!menuMusicSpectrumIsActive(canvas)) {
            menuMusicCancelSpectrumFrame();
            return;
        }
        if (menuMusic.audioContext?.state !== "running") {
            menuMusicCancelSpectrumFrame();
            return;
        }
        if (reducedRewardMotion()) {
            menuMusicCancelSpectrumFrame();
            if (canvas) menuMusicDrawSpectrum(canvas);
            return;
        }
        if (canvas) menuMusicDrawSpectrum(canvas);
        menuMusicScheduleSpectrumFrame();
    });
}

function menuMusicSyncSpectrum() {
    const canvas = menuMusicSpectrumCanvas();
    if (!menuMusicSpectrumIsActive(canvas)) {
        menuMusicCancelSpectrumFrame();
        return;
    }
    if (menuMusicMuted() || menuMusicVolume() <= 0) {
        if (canvas) menuMusicDrawSpectrum(canvas);
        menuMusicCancelSpectrumFrame();
        return;
    }
    menuMusicPrepareSpectrum();
    if (menuMusic.spectrumUnavailable || !menuMusic.spectrumAnalyzers?.some(Boolean)) {
        if (!menuMusic.spectrumPending && canvas) canvas.hidden = true;
        menuMusicCancelSpectrumFrame();
        return;
    }
    if (canvas) menuMusicDrawSpectrum(canvas);
    if (reducedRewardMotion() || menuMusic.audioContext?.state !== "running") {
        menuMusicCancelSpectrumFrame();
        return;
    }
    menuMusicScheduleSpectrumFrame();
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

// Music is optional during the initial loader; start its request when that
// loader closes so it cannot compete with the tutorial's map and scripts.
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
document.addEventListener("click", menuMusicUnlockFromClick, true);

export function xpForLevel(level: number): number {
    if (level <= 1) return 100;
    return Math.round((100 + 11 * Math.pow(level - 1, 1.05)) / 5) * 5;
}

export function totalXpForLevel(level: number): number {
    let total = 0;
    for (let l = 1; l < level; l++) total += xpForLevel(l);
    return total;
}

export function levelFromXp(xp: number): number {
    let level = 1;
    let remaining = Math.max(0, Math.floor(xp));
    while (true) {
        const req = xpForLevel(level);
        if (remaining < req) return level;
        remaining -= req;
        level += 1;
    }
}

// Rewards hold the displayed level until its own presentation finishes. Keep a full
// bar for that level instead of wrapping the new XP underneath the previous level.
function headerXp(xp: number, level: number) {
    const safeLevel = Math.max(1, level);
    const required = xpForLevel(safeLevel);
    const current = Math.min(required, Math.max(0, Math.floor(xp) - totalXpForLevel(safeLevel)));
    const pct = required > 0 ? Math.min(100, Math.max(0, (current / required) * 100)) : 0;
    return { current, required, pct };
}

// Nameplate (avatar, editable name, level and XP bar) on the start side; balances and settings on the end side.
function renderTopbar() {
    const name = menuUi.state!.player_name || SOW_t("menu.anonymous");
    const xp = Math.max(0, Number(menuUi.state!.xp) || 0);
    const level = Math.max(1, Number(menuUi.state!.level) || levelFromXp(xp));
    const levelXp = headerXp(xp, level);
    const shop = portalOffersStore();
    const balances = (["crowns", "gems"] as const)
        .map(function (kind) {
            const amount = Math.max(0, Math.round(Number(menuUi.state![kind]) || 0));
            const label = SOW_t(kind === "crowns" ? "store.crowns" : "store.gems") + ": " + amount;
            const tag = shop ? "button" : "span";
            return (
                "<" +
                tag +
                " class='sow-menu__currency sow-menu__" +
                kind +
                (shop ? " sow-control-crest' type='button' data-command='main_nav' data-nav-screen='store'" : "'") +
                " data-sow-tooltip='" +
                esc(String(amount)) +
                "' data-sow-tooltip-title='" +
                esc(SOW_t(kind === "crowns" ? "store.crowns" : "store.gems")) +
                "' data-sow-tooltip-tone='" +
                (kind === "crowns" ? "gold" : "gem") +
                "' aria-label='" +
                esc(label) +
                "'><img class='sow-menu__currency-icon' src='" +
                esc(currencyAsset(kind === "crowns" ? "crown" : "gem")) +
                "' alt='' aria-hidden='true'><strong data-progression-" +
                kind +
                "-value>" +
                esc(formatCompactNumber(amount)) +
                "</strong></" +
                tag +
                ">"
            );
        })
        .join("");
    return (
        "<header class='sow-menu__topbar' data-progression>" +
        "<div class='sow-menu__nameplate'>" +
        "<span class='sow-menu__portrait'><button class='sow-menu__avatar sow-control-crest' type='button' data-command='open_leader_picker' aria-label='" +
        esc(SOW_t("menu.select_leader")) +
        "' style=\"" +
        avatarBackgroundCss(avatarImage()) +
        "\"></button>" +
        "<button class='sow-menu__level sow-control-crest' type='button' data-command='open_profile' aria-label='" +
        esc(SOW_t("profile.level", { level })) +
        "'><strong data-progression-level-value>" +
        esc(level) +
        "</strong></button></span>" +
        "<label class='sow-menu__name'><input data-role='display-name' name='display_name' value=\"" +
        esc(name) +
        "\" maxlength='16' aria-label='" +
        esc(SOW_t("menu.display_name")) +
        "'></label>" +
        "<button class='sow-menu__progress sow-control-crest' type='button' data-command='open_profile' aria-label='" +
        esc(SOW_t("menu.open_profile")) +
        "' aria-describedby='sow-menu-account-xp'><span class='sow-menu__xp-track' role='progressbar' aria-label='" +
        esc(SOW_t("menu.xp")) +
        "' aria-valuemin='0' aria-valuemax='" +
        levelXp.required +
        "' aria-valuenow='" +
        levelXp.current +
        "'><i data-progression-xp-fill style='--sow-xp-ratio:" +
        (levelXp.pct / 100).toFixed(4) +
        "' data-xp-empty='" +
        (levelXp.pct <= 0 ? "true" : "false") +
        "'></i><span class='sow-menu__xp-value' id='sow-menu-account-xp'><b data-progression-xp-value>" +
        esc(levelXp.current + "/" + levelXp.required) +
        "</b><small>" +
        esc(SOW_t("menu.xp")) +
        "</small></span></span></button></div>" +
        "<div class='sow-menu__top-actions'>" +
        balances +
        "<button class='sow-menu__topbar-settings sow-control-crest' type='button' data-command='toggle_settings' aria-label='" +
        esc(SOW_t("menu.settings")) +
        "'>" +
        menuIcon("options") +
        "</button></div></header>"
    );
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
        "<button class='sow-menu__primary sow-menu__system-primary' type='button' data-command='system_notice_refresh'" +
        ">" +
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
        const state = menuUi.state;
        if (state?.update_available && state.phase !== "MainMenu") return true;
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
    if (!deleteControl && !error && !privacyLink) return "";
    return (
        "<div class='sow-menu__settings-privacy-row" +
        (menuUi.deleteArmed ? " is-armed" : "") +
        "'>" +
        privacyLink +
        deleteControl +
        error +
        "</div>"
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
        "<div class='sow-menu__shell'>" +
        renderTopbar() +
        "<div class='sow-menu__screen-stage' data-screen-stage>" +
        renderScreenPanel(screen) +
        "</div>" +
        "<div class='sow-menu__nav-dock'>" +
        renderMainNav(screenNav(screen)) +
        renderMenuAttribution() +
        "</div>" +
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

function settingsRenderKey(): string {
    return surfaceSettingsRenderKey(deleteBusy, deleteError);
}

function syncOverlay(key: string, markup: string) {
    const selector = "[data-menu-overlay='" + key + "']";
    const current = root.querySelector(selector);
    if (!markup) {
        if (current) {
            current.remove();
            if (key === "settings") {
                lastSettingsKey = "";
            }
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
        if (key === "settings") {
            lastSettingsKey = settingsRenderKey();
        }
        if (key === "system-notice") {
            systemNoticeReturnFocus =
                document.activeElement instanceof HTMLElement ? document.activeElement : null;
            root.appendChild(next);
            const primary = next.querySelector<HTMLButtonElement>("button");
            if (primary) primary.focus({ preventScroll: true });
        } else {
            root.appendChild(next);
        }
        return;
    }
    if (key === "settings") {
        const newKey = settingsRenderKey();
        if (newKey === lastSettingsKey) {
            return;
        }
        lastSettingsKey = newKey;
        const currentModal = current.querySelector(".sow-menu__settings-modal");
        const nextModal = next.querySelector(".sow-menu__settings-modal");
        if (currentModal && nextModal) {
            const currentCanvas = currentModal.querySelector<HTMLCanvasElement>("[data-menu-music-spectrum]");
            const nextCanvas = nextModal.querySelector<HTMLCanvasElement>("[data-menu-music-spectrum]");
            if (currentCanvas && nextCanvas) {
                nextCanvas.replaceWith(currentCanvas);
            }
            if (isDraggingSettingsSlider) {
                for (const setting of ["music_volume", "sfx_volume"]) {
                    const currentSlider = currentModal.querySelector<HTMLInputElement>(
                        "input[data-setting='" + setting + "']",
                    );
                    const nextSlider = nextModal.querySelector<HTMLInputElement>(
                        "input[data-setting='" + setting + "']",
                    );
                    if (currentSlider && nextSlider) {
                        nextSlider.replaceWith(currentSlider);
                    }
                }
            }
            const activeEl = document.activeElement;
            const activeCommand =
                activeEl instanceof HTMLElement && currentModal.contains(activeEl) ? activeEl.dataset.command : null;
            const activeSetting =
                activeEl instanceof HTMLElement && currentModal.contains(activeEl) ? activeEl.dataset.setting : null;
            currentModal.replaceWith(nextModal);
            if (activeCommand) {
                const restore = nextModal.querySelector<HTMLElement>("[data-command='" + activeCommand + "']");
                if (restore) restore.focus({ preventScroll: true });
            } else if (activeSetting) {
                const restore = nextModal.querySelector<HTMLElement>("[data-setting='" + activeSetting + "']");
                if (restore) restore.focus({ preventScroll: true });
            }
        } else {
            current.innerHTML = next.innerHTML;
        }
        return;
    }
    if (current.outerHTML !== next.outerHTML) {
        current.replaceWith(next);
        if (key === "system-notice") {
            const primary = next.querySelector<HTMLButtonElement>("button");
            if (primary) primary.focus({ preventScroll: true });
        }
    }
}

function updateTopbar(): void {
    const topbar = root.querySelector(".sow-menu__topbar");
    if (!topbar || !menuUi.state) return;
    const nameInput = topbar.querySelector<HTMLInputElement>("[data-role='display-name']");
    const name = menuUi.state.player_name || SOW_t("menu.anonymous");
    if (nameInput && document.activeElement !== nameInput) nameInput.value = name;
    const avatar = topbar.querySelector<HTMLElement>(".sow-menu__avatar");
    const avatarUrl = avatarImage();
    if (avatar && avatar.dataset.avatarUrl !== avatarUrl) {
        setAvatarBackground(avatar, avatarUrl);
        avatar.dataset.avatarUrl = avatarUrl;
    }
}

function updateHeroBackdrop(screen: string): void {
    let desktop: string;
    let mobile: string;
    if (screen === "profile") {
        desktop = mobile = profileHeroImage(menuUi.profileData && menuUi.profileData.preferred_leader);
    } else if (screen === "campaign") {
        const { band, column } = campaignBackdropArt();
        desktop = column;
        mobile = band;
    } else {
        ({ desktop, mobile } = selectedHeroBackdrop());
    }
    for (const [property, url] of [["--sow-hero-desktop", desktop], ["--sow-hero-mobile", mobile]]) {
        const value = 'url("' + url + '")';
        if (root.style.getPropertyValue(property) !== value) root.style.setProperty(property, value);
    }
}

function updateFrameChrome(screen: string, screenChanged = false) {
    const backdrop = root.querySelector<HTMLElement>("[data-menu-backdrop]");
    const backdropClass = screenBackdropClass(screen);
    const reducedMotion = Boolean(menuUi.state && menuUi.state.settings && menuUi.state.settings.reduced_motion);
    if (backdrop) {
        if (backdrop.className !== backdropClass) backdrop.className = backdropClass;
        if (screenChanged && !reducedMotion) {
            backdrop.classList.remove("sow-backdrop--shifting");
            void backdrop.offsetWidth;
            backdrop.classList.add("sow-backdrop--shifting");
        }
    }
    const activeNav = screenNav(screen);
    const navBar = root.querySelector<HTMLElement>(".sow-menu__main-nav");
    const navButtons = Array.from(
        root.querySelectorAll<HTMLElement>(".sow-menu__main-nav-item[data-nav-screen]"),
    );
    if (navBar && navButtons.length) {
        const renderedItems = navButtons
            .map(function (btn) {
                return MAIN_NAV_ITEMS.find(function (candidate) {
                    return candidate[0] === btn.dataset.navScreen;
                });
            })
            .filter(function (entry): entry is string[] {
                return Array.isArray(entry);
            });
        const indices = computeMainNavIndices(activeNav, renderedItems.length ? renderedItems : MAIN_NAV_ITEMS);
        navBar.style.setProperty("--sow-nav-count", String(indices.count));
        navBar.style.setProperty("--sow-nav-row-index", String(indices.rowIndex));
        navBar.style.setProperty("--sow-nav-col-index", String(indices.colIndex));
    }
    // Only the nav bar mirrors the screen; header balances and store links also carry data-nav-screen.
    navButtons.forEach(function (item) {
        const wasActive = item.classList.contains("is-active");
        const active = item.dataset.navScreen === activeNav;
        const navItem = MAIN_NAV_ITEMS.find(function (candidate) {
            return candidate[0] === item.dataset.navScreen;
        });
        const labelText = navItem ? SOW_t(navItem[2]) : "";
        const labelNode = item.querySelector<HTMLElement>("small");
        if (labelNode && labelText) labelNode.textContent = labelText;
        if (labelText) item.setAttribute("aria-label", labelText);
        item.classList.toggle("is-active", active);
        if (!wasActive && active && !reducedMotion) {
            item.classList.remove("is-just-activated");
            void item.offsetWidth;
            item.classList.add("is-just-activated");
        } else if (!active) {
            item.classList.remove("is-just-activated");
        }
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
    if (!stage) return;
    if (screenChanged) {
        const next = panelFromMarkup(renderScreenPanel(screen));
        if (!next) return;
        const settings = menuUi.state.settings;
        sowScreenMotion.show(stage, next, !!settings.reduced_motion);
        return;
    }
    const current = activeScreenPanel();
    if (screen === "heroes" && current && current.dataset.screenPanel === "heroes") {
        updateHeroesPreview();
        return;
    }
    const next = panelFromMarkup(renderScreenPanel(screen));
    if (!next) return;
    if (current) current.replaceWith(next);
    else stage.appendChild(next);
    next.classList.add("is-active");
}

function syncBladeCanvasVisibility() {
    if (typeof document === "undefined") return;
    const blade = document.getElementById("blade");
    if (!blade) return;
    const menuCoversViewport = !root.hidden && Boolean(menuUi.state && menuUi.state.phase === "MainMenu");
    const desired = menuCoversViewport ? "hidden" : "";
    if (blade.style.visibility !== desired) {
        blade.style.visibility = desired;
    }
}

export function render(precomputedKey?: string) {
    if (!menuUi.state) return;
    if (activeMenuPointerId !== null) {
        renderPending = true;
        return;
    }
    renderPending = false;
    const screen = currentScreen();
    if (screen !== "heroes") menuUi.skinPickerOpen = false;
    if (screen !== "heroes" && screen !== "store") {
        menuUi.heroInfoOpen = false;
        menuUi.heroInfoLeaderId = null;
    }
    const prevScreen = menuUi.previousScreen;
    const screenChanged = prevScreen !== screen;
    const sameScreen = !screenChanged;
    if (screenChanged) {
        menuUi.dropdownOpenKey = null;
        if (prevScreen !== null && menuUi.state.phase === "MainMenu") {
            if (!consumeScreenSwitchSoundSuppression()) {
                const goingHome = screen === "home";
                playUiCue(AudioCue.UiTabSwitch, goingHome ? 2 : 1);
            }
        } else {
            consumeScreenSwitchSoundSuppression();
        }
    }
    if (screen === "create" && menuUi.previousScreen !== "create") {
        menuUi.createDraft = cloneConfig();
        if (pendingCreateMode) {
            menuUi.createDraft.game_mode = pendingCreateMode;
            pendingCreateMode = null;
        }
        if (menuUi.createDraft.game_mode !== "Coop") {
            menuUi.createDraft.nation_count = 0;
        } else if (!menuUi.createDraft.nation_count) {
            menuUi.createDraft.nation_count = Math.min(Number(menuUi.state?.bot_limits?.nations) || 129, 8);
        }
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
    updateHeroBackdrop(screen);
    syncMenuSkinTheme(menuUi.state);
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
        updateFrameChrome(screen, screenChanged);
        syncScreenPanel(screen, screenChanged);
    }
    if (screen === "heroes") settleHeroesFeaturedArt();
    syncOverlay("password", renderPasswordModal());
    syncOverlay("settings", menuUi.settingsOpen ? renderSettings() : "");
    syncOverlay("auth", menuUi.authModalOpen ? renderAuthModal() : "");
    syncOverlay("profile-detail", menuUi.profileOpen ? renderProfileDetail() : "");
    syncOverlay("skin-picker", typeof renderSkinPickerModal === "function" ? renderSkinPickerModal() : "");
    syncOverlay("hero-info", typeof renderHeroInfoModal === "function" ? renderHeroInfoModal() : "");
    syncOverlay("purchase", typeof renderPurchaseModal === "function" ? renderPurchaseModal() : "");
    syncOverlay("missions", menuUi.missionsModalOpen ? renderMissionsModal() : "");
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
    menuUi.lastRenderKey = precomputedKey !== undefined ? precomputedKey : renderKey();
    syncBladeCanvasVisibility();
    updateLobbyViews();
}

export function progressionFromState(): ProgressionValues {
    const xp = Math.max(0, Number(menuUi.state && menuUi.state.xp) || 0);
    return {
        xp: xp,
        level: Math.max(1, Number(menuUi.state && menuUi.state.level) || levelFromXp(xp)),
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
        // Store purchases record a presentation-only receipt for the
        // achievements they unlocked; balances were already applied.
        const purchaseApplied =
            receipt &&
            typeof receipt.id === "string" &&
            receipt.id.indexOf("purchase:") === 0 &&
            receipt.match_id === receipt.id;
        return (
            receipt &&
            receipt.id &&
            receipt.status === "applied" &&
            !receipt.presented_at &&
            (tutorialApplied ||
                campaignApplied ||
                purchaseApplied ||
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
        level: Math.max(1, Number(values.level) || levelFromXp(Number(values.xp) || 0)),
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
    const gemsValue = progression.querySelector<HTMLElement>("[data-progression-gems-value]");
    if (levelValue) {
        levelValue.textContent = String(displayedProgression.level);
        levelValue.parentElement?.setAttribute(
            "aria-label",
            SOW_t("profile.level", { level: displayedProgression.level }),
        );
    }
    const levelXp = headerXp(displayedProgression.xp, displayedProgression.level);
    if (xpValue) xpValue.textContent = levelXp.current + "/" + levelXp.required;
    if (xpFill) {
        const ratio = Math.max(0, Math.min(1, levelXp.pct / 100));
        xpFill.style.setProperty("--sow-xp-ratio", ratio.toFixed(4));
        xpFill.dataset.xpEmpty = ratio <= 0.0001 ? "true" : "false";
        xpFill.parentElement?.setAttribute("aria-valuemax", String(levelXp.required));
        xpFill.parentElement?.setAttribute("aria-valuenow", String(levelXp.current));
    }
    for (const [kind, counter] of [
        ["crowns", crownsValue],
        ["gems", gemsValue],
    ] as const) {
        if (!counter) continue;
        const amount = Math.round(displayedProgression[kind]);
        counter.textContent = formatCompactNumber(amount);
        const cell = counter.parentElement;
        const label = SOW_t(kind === "crowns" ? "store.crowns" : "store.gems") + ": " + amount;
        if (cell) {
            SOWTooltip.set(cell, {
                title: SOW_t(kind === "crowns" ? "store.crowns" : "store.gems"),
                text: String(amount),
                tone: kind === "crowns" ? "gold" : "gem",
            });
            cell.setAttribute("aria-label", label);
        }
    }
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
                sum.gems += Math.max(0, Number(receipt.gems) || 0);
                return sum;
            },
            { xp: 0, crowns: 0, laurels: 0, gems: 0 },
        );
        canonical.xp = Math.max(0, canonical.xp - totals.xp);
        canonical.crowns = Math.max(0, canonical.crowns - totals.crowns);
        canonical.laurels = Math.max(0, canonical.laurels - totals.laurels);
        canonical.gems = Math.max(0, canonical.gems - totals.gems);
        canonical.level = levelFromXp(canonical.xp);
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
    let lastLevel = from.level;
    // A filling bar drops its width easing and shows a moving shine with a lit leading edge.
    const track = from.xp !== target.xp ? root.querySelector<HTMLElement>(".sow-menu__xp-track") : null;
    track?.classList.add("is-filling");
    return new Promise<boolean>(function (done) {
        function resolve(shown: boolean) {
            track?.classList.remove("is-filling");
            done(shown);
        }
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
            const currentLevel = levelOverride == null ? levelFromXp(currentXp) : levelOverride;
            writeProgression({
                xp: currentXp,
                level: currentLevel,
                crowns: from.crowns + (target.crowns - from.crowns) * eased,
                laurels: from.laurels + (target.laurels - from.laurels) * eased,
                gems: from.gems + (target.gems - from.gems) * eased,
            });
            if (levelOverride == null && currentLevel > lastLevel) {
                lastLevel = currentLevel;
                pulseRewardCounter("[data-progression-level-value]", true);
                if (activeRewardStage) playRewardImpact("[data-progression-level-value]", "level", 1, currentLevel);
            }
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
    rankedStageCleanup?.();
    rankedStageCleanup = null;
    rankedStageKey = null;
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
    if (activeRewardRunner) {
        const runner = activeRewardRunner;
        activeRewardRunner = null;
        runner.cancel();
    }
    rewardAnimationToken += 1;
    rewardAnimationRunning = false;
    progressionAnimationTarget = null;
    removeRewardStage();
}

interface CreateRewardStageOptions {
    useWaapiReveal?: boolean;
}

function createRewardStage(
    stages: RewardStage[],
    skip: () => void,
    options?: CreateRewardStageOptions,
): RewardItem[] {
    const useWaapiReveal = options?.useWaapiReveal === true && typeof HTMLElement !== "undefined";
    const layer = document.createElement("section");
    layer.className = "sow-menu__reward-stage";
    layer.setAttribute("role", "dialog");
    layer.setAttribute("aria-modal", "true");
    layer.setAttribute("aria-label", SOW_t("endgame.match_result"));
    const cards = document.createElement("div");
    cards.className = "sow-menu__reward-cards" + (stages.length >= 4 ? " is-dense" : "");
    cards.dataset.rewardCount = String(stages.length);
    const items = stages.map(function (stage: RewardStage, index: number): RewardItem {
        const card = document.createElement("div");
        card.className = "sow-menu__reward-card sow-menu__reward-card--" + stage.kind;
        if (useWaapiReveal) {
            card.dataset.waapiReveal = "true";
        } else {
            card.style.animationDelay = 40 + index * 48 + "ms";
        }
        card.setAttribute("role", "group");
        const levelLabel = stage.kind === "level" ? SOW_t("profile.level", { level: stage.level }) : null;
        card.setAttribute(
            "aria-label",
            stage.kind === "leader" || stage.kind === "achievement"
                ? stage.label
                : levelLabel ||
                      stage.amount +
                          " " +
                          SOW_t(
                              stage.kind === "xp"
                                  ? "menu.xp"
                                  : stage.kind === "crowns"
                                    ? "store.crowns"
                                    : stage.kind === "gems"
                                      ? "store.gems"
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
                setAvatarImage(image, avatarImage(stage.slug));
                image.alt = "";
            } else {
                image.src = currencyAsset(
                    stage.kind === "crowns" ? "crown" : stage.kind === "gems" ? "gem" : "laurel",
                );
            }
            emblem = image;
        }
        emblem.className = "sow-menu__reward-emblem";
        medallion.appendChild(emblem);
        const amount = document.createElement("strong");
        amount.className = "sow-menu__reward-amount";
        if (stage.kind !== "leader") amount.setAttribute("aria-hidden", "true");
        amount.textContent = levelLabel || (stage.kind === "leader" || stage.kind === "achievement" ? stage.label : "+0");
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
            return;
        }
        if ((event.key === " " || event.key === "Enter") && event.target !== close) {
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

export interface FlyRewardCardOptions {
    duration?: number;
    playbackRate?: number;
    fromRect?: DOMRect | null;
    toRect?: DOMRect | null;
    onCreateAnimation?: (animation: Animation) => void;
}

export function flyRewardCard(
    item: RewardFlyItem,
    token: number,
    options?: FlyRewardCardOptions,
): Promise<boolean> {
    const opts = options || {};
    let targetRect: DOMRect | null = opts.toRect ?? null;
    if (targetRect === null && opts.toRect === undefined) {
        const target = root.querySelector<HTMLElement>(item.stage.selector);
        if (target) targetRect = target.getBoundingClientRect();
    }
    if (typeof item.card.animate !== "function") {
        item.card.style.visibility = "hidden";
        return Promise.resolve(token === rewardAnimationToken);
    }
    if (!targetRect) {
        if (opts.toRect === undefined) {
            item.card.style.visibility = "hidden";
            return Promise.resolve(token === rewardAnimationToken);
        }
        item.card.classList.add("is-animating");
        return new Promise<boolean>(function (resolve) {
            const motion = item.card.animate(
                [
                    { opacity: 1, transform: "translate3d(0,0,0) scale(1)" },
                    { opacity: 0, transform: "translate3d(0,-12px,0) scale(0.9)" },
                ],
                {
                    duration: Math.min(240, opts.duration ?? 220),
                    easing: "ease-in",
                    fill: "forwards",
                },
            );
            if (opts.playbackRate && opts.playbackRate !== 1) motion.playbackRate = opts.playbackRate;
            opts.onCreateAnimation?.(motion);
            motion.onfinish = function () {
                item.card.classList.remove("is-animating");
                item.card.style.visibility = "hidden";
                resolve(token === rewardAnimationToken);
            };
            motion.oncancel = function () {
                item.card.classList.remove("is-animating");
                resolve(false);
            };
        });
    }
    const from = opts.fromRect ?? item.card.getBoundingClientRect();
    const dx = targetRect.left + targetRect.width / 2 - from.left - from.width / 2;
    const dy = targetRect.top + targetRect.height / 2 - from.top - from.height / 2;
    const path = flyArcPointsToKeyframes(sampleFlyArcPoints(dx, dy));
    item.card.classList.add("is-animating");
    return new Promise<boolean>(function (resolve) {
        const motion = item.card.animate(path, {
            duration: opts.duration ?? 380,
            easing: "linear",
            fill: "forwards",
        });
        if (opts.playbackRate && opts.playbackRate !== 1) {
            motion.playbackRate = opts.playbackRate;
        }
        opts.onCreateAnimation?.(motion);
        motion.onfinish = function () {
            item.card.classList.remove("is-animating");
            item.card.style.visibility = "hidden";
            resolve(token === rewardAnimationToken);
        };
        motion.oncancel = function () {
            item.card.classList.remove("is-animating");
            resolve(false);
        };
    });
}

function restartRewardClass(element: HTMLElement, className: string, duration: number) {
    let timers = rewardClassTimers.get(element);
    if (!timers) {
        timers = Object.create(null) as Record<string, number>;
        rewardClassTimers.set(element, timers);
    }
    let seqMap = rewardClassSeq.get(element);
    if (!seqMap) {
        seqMap = Object.create(null) as Record<string, number>;
        rewardClassSeq.set(element, seqMap);
    }
    if (timers[className] != null) {
        window.clearTimeout(timers[className]);
        delete timers[className];
    }
    const nextSeq = (seqMap[className] || 0) + 1;
    seqMap[className] = nextSeq;
    const scheduleClear = function () {
        timers![className] = window.setTimeout(function () {
            if (seqMap![className] !== nextSeq) return;
            delete timers![className];
            element.classList.remove(className);
        }, duration);
    };
    if (!element.classList.contains(className)) {
        element.classList.add(className);
        scheduleClear();
        return;
    }
    element.classList.remove(className);
    if (typeof window.requestAnimationFrame === "function") {
        window.requestAnimationFrame(function () {
            if (seqMap![className] !== nextSeq || !element.isConnected) return;
            element.classList.add(className);
            scheduleClear();
        });
    } else {
        element.classList.add(className);
        scheduleClear();
    }
}

export function pulseRewardCounter(selector: string, levelUp: boolean) {
    if (reducedRewardMotion()) return;
    const counter = root.querySelector<HTMLElement>(selector);
    if (!counter) return;
    if (selector === ".sow-menu__portrait") {
        restartRewardClass(counter, "is-reward-hit", 600);
        return;
    }
    if (selector === "[data-nav-screen='heroes']") {
        restartRewardClass(counter, "is-reward-hit", 560);
        return;
    }
    restartRewardClass(counter, levelUp ? "is-reward-level-up" : "is-reward-value-hit", levelUp ? 780 : 540);
    // The plate around the counter answers too: its currency pill or XP track, or the whole nameplate on a level-up.
    const plate = counter.closest<HTMLElement>(
        levelUp ? ".sow-menu__nameplate" : ".sow-menu__currency, .sow-menu__xp-track",
    );
    if (plate) restartRewardClass(plate, levelUp ? "is-level-up" : "is-reward-hit", levelUp ? 780 : 540);
}

export interface PlayRewardImpactOptions {
    layerRect?: DOMRect | null;
    toRect?: DOMRect | null;
    playbackRate?: number;
    onMount?: (impactEl: HTMLElement, durationMs: number) => void;
}

// Arrival light is drawn on the reward layer, above its curtain, centered on the header destination.
export function playRewardImpact(
    selector: string,
    kind: RewardStage["kind"],
    power: number,
    level?: number,
    options?: PlayRewardImpactOptions,
) {
    const layer = activeRewardStage;
    if (!layer || reducedRewardMotion()) return;
    const opts = options || {};
    let to: DOMRect | null = opts.toRect ?? null;
    if (to === null && opts.toRect === undefined) {
        const target = root.querySelector<HTMLElement>(selector);
        if (target) to = target.getBoundingClientRect();
    }
    if (!to) return;
    const box = opts.layerRect ?? layer.getBoundingClientRect();
    const impact = document.createElement("span");
    impact.className = "sow-menu__reward-impact sow-menu__reward-impact--" + kind;
    impact.setAttribute("aria-hidden", "true");
    impact.style.left = to.left + to.width / 2 - box.left + "px";
    impact.style.top = to.top + to.height / 2 - box.top + "px";
    impact.style.setProperty("--fx-power", String(power));
    const levelUp = kind === "level";
    const parts = ["flash", "ring"].concat(levelUp ? ["ring", "rays"] : []);
    parts.forEach(function (part) {
        const layerPart = document.createElement("i");
        layerPart.className = "sow-menu__reward-impact-" + part;
        impact.appendChild(layerPart);
    });
    const sparks = levelUp ? 12 : 7;
    for (let i = 0; i < sparks; i++) {
        const spark = document.createElement("i");
        spark.className = "sow-menu__reward-impact-spark";
        spark.style.setProperty("--a", (360 / sparks) * i + Math.random() * 18 + "deg");
        spark.style.setProperty(
            "--d",
            String(Math.round((levelUp ? 34 : 20) + Math.random() * (levelUp ? 26 : 14))),
        );
        impact.appendChild(spark);
    }
    if (levelUp && level) {
        const label = document.createElement("b");
        label.className = "sow-menu__reward-impact-label";
        label.textContent = SOW_t("profile.level", { level });
        impact.appendChild(label);
    }
    layer.appendChild(impact);
    const lifetimeMs = levelUp ? 1300 : 800;
    if (opts.playbackRate && opts.playbackRate !== 1 && typeof impact.getAnimations === "function") {
        impact.getAnimations({ subtree: true }).forEach(function (anim) {
            try {
                anim.playbackRate = opts.playbackRate!;
            } catch {}
        });
    }
    if (opts.onMount) {
        opts.onMount(impact, lifetimeMs);
        return;
    }
    window.setTimeout(function () {
        impact.remove();
    }, lifetimeMs);
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

const REWARD_REVEAL_KEYFRAMES: Keyframe[] = [
    { offset: 0, opacity: 0, transform: "translate3d(0, 24px, 0) scale(0.86)" },
    { offset: 0.58, opacity: 1, transform: "translate3d(0, -4px, 0) scale(1.03)" },
    { offset: 1, opacity: 1, transform: "translate3d(0, 0, 0) scale(1)" },
];

interface NumericChannel {
    from: number;
    to: number;
    startMs: number;
    durMs: number;
    done: boolean;
}

interface ChoreographedStageOptions {
    token: number;
    stages: RewardStage[];
    startValues: ProgressionValues;
    targetValues: ProgressionValues;
    stageClassName?: string;
    impactPower?: number;
    hasBanner?: boolean;
    hasAudio?: boolean;
    beforeStart?: (stageEl: HTMLElement) => void;
    onComplete: (shown: boolean) => void;
}

function formatRewardCardText(stage: RewardStage, progress = 1): string {
    if (stage.kind === "level") return SOW_t("profile.level", { level: stage.level });
    if (stage.kind === "leader" || stage.kind === "achievement") return stage.label;
    const eased = progress >= 1 ? 1 : 1 - Math.pow(1 - Math.max(0, progress), 3);
    return "+" + Math.round(stage.amount * eased);
}

function runChoreographedStage(config: ChoreographedStageOptions) {
    const { token, stages, startValues, targetValues } = config;
    if (!stages.length) {
        config.onComplete(true);
        return;
    }

    const levelStages = stages.filter(function (s): s is Extract<RewardStage, { kind: "level" }> {
        return s.kind === "level";
    });
    const startReq = xpForLevel(startValues.level);
    const firstLevelCap = levelStages.length
        ? Math.min(targetValues.xp, totalXpForLevel(levelStages[0].level))
        : targetValues.xp;
    const xpFillPct =
        startReq > 0 ? Math.min(100, Math.max(0, ((firstLevelCap - startValues.xp) / startReq) * 100)) : 0;

    let plan: RewardChoreographyPlan = planRewardChoreography(
        stages.map(function (stage) {
            return { kind: stage.kind, amount: stage.amount };
        }),
        {
            xpFillPct,
            levelsGained: levelStages.length,
            hasBanner: Boolean(config.hasBanner),
        },
    );

    const trackedAnims = new Set<Animation>();
    const impactTimers = new Set<number>();
    const impactEls = new Set<HTMLElement>();
    const revealAnims: Array<Animation | null> = new Array(stages.length).fill(null);

    let mode: RewardPlaybackMode = "normal";
    let speed = 1;
    let exitSpeed = 1;
    let accelStartedRealMs = 0;
    let virtualTime = 0;
    let lastRealMs = performance.now();
    let frameId = 0;
    let stopped = false;
    let exiting = false;
    let exitElapsedMs = 0;
    let exitAnim: Animation | null = null;
    let geometryCaptured = false;
    let stageRect: DOMRect | null = null;
    let fromRects: Array<DOMRect | null> = [];
    const toRects = new Map<string, DOMRect | null>();

    const launched = new Array<boolean>(stages.length).fill(false);
    const landed = new Array<boolean>(stages.length).fill(false);
    const amountSettled = new Array<boolean>(stages.length).fill(false);

    const displayed: ProgressionValues = Object.assign({}, startValues);
    const xpTrack = root.querySelector<HTMLElement>(".sow-menu__xp-track");
    let crownsChan: NumericChannel | null = null;
    let gemsChan: NumericChannel | null = null;
    let xpChan: (NumericChannel & { targetLevel: number }) | null = null;

    function clearTrackedImpacts() {
        impactTimers.forEach(function (timerId) {
            window.clearTimeout(timerId);
        });
        impactTimers.clear();
        impactEls.forEach(function (el) {
            el.remove();
        });
        impactEls.clear();
    }

    function cancelTrackedAnims() {
        trackedAnims.forEach(function (anim) {
            try {
                anim.cancel();
            } catch {}
        });
        trackedAnims.clear();
        if (exitAnim) {
            try {
                exitAnim.cancel();
            } catch {}
            exitAnim = null;
        }
    }

    function registerStageCssAnims(rate: number) {
        if (activeRewardStage && typeof activeRewardStage.getAnimations === "function") {
            activeRewardStage.getAnimations({ subtree: true }).forEach(function (anim) {
                if (anim === exitAnim) return;
                trackedAnims.add(anim);
                if (rate !== 1 && anim.playState !== "finished" && anim.playState !== "idle") {
                    try {
                        anim.playbackRate = rate;
                    } catch {}
                }
            });
        }
        const progression = root.querySelector<HTMLElement>("[data-progression]");
        if (progression && typeof progression.getAnimations === "function") {
            progression.getAnimations({ subtree: true }).forEach(function (anim) {
                if (rate !== 1 && anim.playState !== "finished" && anim.playState !== "idle") {
                    try {
                        anim.playbackRate = rate;
                    } catch {}
                }
            });
        }
    }

    function syncPlaybackRates(rate: number) {
        trackedAnims.forEach(function (anim) {
            if (anim === exitAnim) return;
            if (anim.playState === "finished" || anim.playState === "idle") return;
            try {
                anim.playbackRate = rate;
            } catch {}
        });
        registerStageCssAnims(rate);
        if (exitAnim && exiting && exitAnim.playState !== "finished") {
            try {
                exitAnim.playbackRate = exitSpeed;
            } catch {}
        }
    }

    function finishRevealAnims() {
        revealAnims.forEach(function (anim, index) {
            if (anim) {
                trackedAnims.delete(anim);
                try {
                    anim.finish();
                    anim.cancel();
                } catch {}
                revealAnims[index] = null;
            }
            items[index]?.card.classList.remove("is-animating");
        });
    }

    function captureBatchGeometry() {
        if (geometryCaptured) return;
        geometryCaptured = true;
        finishRevealAnims();
        stageRect = activeRewardStage ? activeRewardStage.getBoundingClientRect() : null;
        fromRects = items.map(function (item) {
            return item.card.getBoundingClientRect();
        });
        const selectors = new Set<string>();
        stages.forEach(function (stage) {
            if (stage.selector) selectors.add(stage.selector);
        });
        selectors.forEach(function (selector) {
            const target = root.querySelector<HTMLElement>(selector);
            toRects.set(selector, target ? target.getBoundingClientRect() : null);
        });
    }

    function snapAllRevealTexts() {
        items.forEach(function (item, idx) {
            if (amountSettled[idx]) return;
            const text = formatRewardCardText(item.stage, 1);
            if (item.amount.textContent !== text) item.amount.textContent = text;
            amountSettled[idx] = true;
        });
    }

    function rebaseChannel(chan: NumericChannel | null, value: number, vNow: number, capMs: number) {
        if (!chan || chan.done) return;
        const remaining = Math.max(1, chan.startMs + chan.durMs - vNow);
        chan.from = value;
        chan.startMs = vNow;
        chan.durMs = Math.min(remaining, capMs);
    }

    function rebaseActiveChannels(vNow: number) {
        rebaseChannel(crownsChan, displayed.crowns, vNow, 180);
        rebaseChannel(gemsChan, displayed.gems, vNow, 180);
        const xpCap = levelStages.some(function (_s, idx) {
            const stageIdx = stages.indexOf(levelStages[idx]);
            return stageIdx >= 0 && landed[stageIdx];
        })
            ? 180
            : 280;
        rebaseChannel(xpChan, displayed.xp, vNow, xpCap);
    }

    function cleanupRunner() {
        stopped = true;
        if (frameId) {
            cancelAnimationFrame(frameId);
            frameId = 0;
        }
        xpTrack?.classList.remove("is-filling");
        clearTrackedImpacts();
        cancelTrackedAnims();
        if (activeRewardRunner === runnerHandle) activeRewardRunner = null;
    }

    function finishImmediate() {
        if (stopped) return;
        cleanupRunner();
        writeProgression(targetValues);
        config.onComplete(true);
    }

    function abort() {
        if (stopped) return;
        cleanupRunner();
        config.onComplete(false);
    }

    function onSkip() {
        if (stopped || token !== rewardAnimationToken) return;
        const nowReal = performance.now();
        if (mode === "normal") {
            mode = "accelerating";
            accelStartedRealMs = nowReal;
            activeRewardStage?.classList.add("is-accelerating");
            snapAllRevealTexts();
            if (!geometryCaptured) captureBatchGeometry();
            const accel = accelerateChoreographyPlan(plan, virtualTime);
            plan = accel.plan;
            speed = accel.speed;
            exitSpeed = accel.exitSpeed;
            rebaseActiveChannels(virtualTime);
            syncPlaybackRates(speed);
            return;
        }
        if (mode === "accelerating" && nowReal - accelStartedRealMs >= 60) {
            mode = "immediate";
            finishImmediate();
        }
    }

    const runnerHandle = {
        cancel: function () {
            if (stopped) return;
            cleanupRunner();
        },
    };
    activeRewardRunner = runnerHandle;

    const items = createRewardStage(stages, onSkip, {
        useWaapiReveal: typeof HTMLElement !== "undefined" && typeof HTMLElement.prototype.animate === "function",
    });

    if (activeRewardStage && config.stageClassName) {
        activeRewardStage.classList.add(config.stageClassName);
    }
    if (activeRewardStage && config.beforeStart) {
        config.beforeStart(activeRewardStage);
    }
    if (config.hasAudio) {
        playCelebrationSound("fanfare");
    }

    items.forEach(function (item, idx) {
        const timing = plan.stages[idx];
        if (!timing) return;
        if (typeof item.card.animate !== "function") {
            item.card.style.animationDelay = timing.revealDelayMs + "ms";
            item.card.style.animationDuration = timing.revealDurationMs + "ms";
            return;
        }
        item.card.classList.add("is-animating");
        const anim = item.card.animate(REWARD_REVEAL_KEYFRAMES, {
            duration: timing.revealDurationMs,
            delay: timing.revealDelayMs,
            easing: "cubic-bezier(0.22, 1, 0.36, 1)",
            fill: "both",
        });
        trackedAnims.add(anim);
        revealAnims[idx] = anim;
    });
    registerStageCssAnims(1);

    function triggerArrival(index: number, stage: RewardStage, timing: (typeof plan.stages)[number], vNow: number) {
        if (config.hasAudio) playCelebrationSound("impact");
        const selector = stage.selector || "";
        const power =
            stage.kind === "level"
                ? 1 + 0.25 * Math.max(0, levelStages.indexOf(stage))
                : config.impactPower ?? 1;

        if (selector) {
            playRewardImpact(
                selector,
                stage.kind,
                power,
                stage.kind === "level" ? stage.level : undefined,
                {
                    layerRect: stageRect,
                    toRect: toRects.get(selector) ?? null,
                    playbackRate: mode === "accelerating" ? speed : 1,
                    onMount: function (impactEl, lifetimeMs) {
                        impactEls.add(impactEl);
                        const scaledMs =
                            mode === "accelerating"
                                ? Math.max(120, Math.round(lifetimeMs / Math.min(speed, 4)))
                                : lifetimeMs;
                        const timerId = window.setTimeout(function () {
                            impactTimers.delete(timerId);
                            impactEls.delete(impactEl);
                            impactEl.remove();
                        }, scaledMs);
                        impactTimers.add(timerId);
                    },
                },
            );
            pulseRewardCounter(selector, stage.kind === "level");
        }

        if (stage.kind === "laurels") {
            displayed.laurels = targetValues.laurels;
            return;
        }
        if (stage.kind === "leader") {
            return;
        }
        if (stage.kind === "crowns") {
            crownsChan = {
                from: displayed.crowns,
                to: targetValues.crowns,
                startMs: vNow,
                durMs: Math.max(1, timing.applyDurationMs),
                done: false,
            };
            return;
        }
        if (stage.kind === "gems") {
            gemsChan = {
                from: displayed.gems,
                to: targetValues.gems,
                startMs: vNow,
                durMs: Math.max(1, timing.applyDurationMs),
                done: false,
            };
            return;
        }
        if (stage.kind === "xp") {
            xpTrack?.classList.add("is-filling");
            xpChan = {
                from: displayed.xp,
                to: firstLevelCap,
                startMs: vNow,
                durMs: Math.max(1, timing.applyDurationMs),
                targetLevel: levelStages.length ? startValues.level : targetValues.level,
                done: false,
            };
            return;
        }
        if (stage.kind === "level") {
            const levelPos = levelStages.indexOf(stage);
            const isFinal = levelPos === levelStages.length - 1;
            const nextCap =
                isFinal || levelPos < 0
                    ? targetValues.xp
                    : Math.min(targetValues.xp, totalXpForLevel(levelStages[levelPos + 1].level));
            const fromXp = Math.max(displayed.xp, totalXpForLevel(stage.level));
            displayed.level = stage.level;
            displayed.xp = fromXp;
            xpTrack?.classList.add("is-filling");
            xpChan = {
                from: fromXp,
                to: nextCap,
                startMs: vNow,
                durMs: Math.max(1, timing.applyDurationMs),
                targetLevel: stage.level,
                done: false,
            };
        }
    }

    function stepFrame(now: number) {
        if (stopped) return;
        if (token !== rewardAnimationToken || root.hidden || document.visibilityState !== "visible") {
            abort();
            return;
        }

        const dt = Math.min(120, Math.max(0, now - lastRealMs));
        lastRealMs = now;

        if (!exiting) {
            virtualTime += dt * speed;
            const vNow = virtualTime;

            // 1. Reveal card amount roll-ups
            for (let i = 0; i < plan.stages.length; i++) {
                if (amountSettled[i]) continue;
                const s = plan.stages[i];
                const item = items[i];
                if (item.stage.kind === "level" || item.stage.kind === "leader") {
                    amountSettled[i] = true;
                    continue;
                }
                if (vNow >= s.countStartMs) {
                    const p = s.countDurationMs > 0 ? Math.min(1, (vNow - s.countStartMs) / s.countDurationMs) : 1;
                    const text = formatRewardCardText(item.stage, p);
                    if (item.amount.textContent !== text) item.amount.textContent = text;
                    if (p >= 1) amountSettled[i] = true;
                }
            }

            // 2. Capture all layout rects in one batch right as launch phase opens
            if (!geometryCaptured && vNow >= plan.launchPhaseStartMs) {
                captureBatchGeometry();
            }

            // 3. Launch cards at scheduled stagger timestamps
            for (let i = 0; i < plan.stages.length; i++) {
                if (launched[i]) continue;
                const s = plan.stages[i];
                if (vNow < s.launchAtMs) continue;
                launched[i] = true;
                if (!geometryCaptured) captureBatchGeometry();
                const item = items[i];
                if (!amountSettled[i]) {
                    const finalText = formatRewardCardText(item.stage, 1);
                    if (item.amount.textContent !== finalText) item.amount.textContent = finalText;
                    amountSettled[i] = true;
                }
                const rAnim = revealAnims[i];
                if (rAnim) {
                    trackedAnims.delete(rAnim);
                    try {
                        rAnim.finish();
                        rAnim.cancel();
                    } catch {}
                    revealAnims[i] = null;
                }
                if (config.hasAudio) playCelebrationSound("fly");
                const selector = item.stage.selector || "";
                void flyRewardCard(
                    { card: item.card, stage: { selector } },
                    token,
                    {
                        duration: s.flyDurationMs,
                        playbackRate: mode === "accelerating" ? speed : 1,
                        fromRect: fromRects[i] ?? null,
                        toRect: selector ? (toRects.get(selector) ?? null) : null,
                        onCreateAnimation: function (anim) {
                            trackedAnims.add(anim);
                        },
                    },
                );
            }

            // 4. Trigger impacts & start value interpolation channels upon card arrival
            let progressionDirty = false;
            for (let i = 0; i < plan.stages.length; i++) {
                if (!launched[i] || landed[i]) continue;
                const s = plan.stages[i];
                if (vNow < s.landAtMs) continue;
                landed[i] = true;
                items[i].card.style.visibility = "hidden";
                triggerArrival(i, items[i].stage, s, vNow);
                progressionDirty = true;
            }

            // 5. Advance active progression channels concurrently (merged cleanly into 1 DOM write)
            for (const [key, chan] of [
                ["crowns", crownsChan],
                ["gems", gemsChan],
            ] as const) {
                if (!chan || chan.done) continue;
                const p = Math.min(1, Math.max(0, (vNow - chan.startMs) / chan.durMs));
                const eased = 1 - Math.pow(1 - p, 3);
                displayed[key] = chan.from + (chan.to - chan.from) * eased;
                if (p >= 1) {
                    displayed[key] = chan.to;
                    chan.done = true;
                }
                progressionDirty = true;
            }
            if (xpChan && !xpChan.done) {
                const p = Math.min(1, Math.max(0, (vNow - xpChan.startMs) / xpChan.durMs));
                const eased = 1 - Math.pow(1 - p, 3);
                displayed.xp = xpChan.from + (xpChan.to - xpChan.from) * eased;
                displayed.level = xpChan.targetLevel;
                if (p >= 1) {
                    displayed.xp = xpChan.to;
                    displayed.level = xpChan.targetLevel;
                    xpChan.done = true;
                    const moreXpOrLevelPending = plan.stages.some(function (s, idx) {
                        return !landed[idx] && (s.kind === "xp" || s.kind === "level");
                    });
                    if (!moreXpOrLevelPending) {
                        xpTrack?.classList.remove("is-filling");
                    }
                }
                progressionDirty = true;
            }

            if (progressionDirty) {
                writeProgression(displayed);
            }

            // 6. Begin exit fade once every reward and level pulse has settled
            if (vNow >= plan.exitAtMs) {
                exiting = true;
                exitElapsedMs = 0;
                xpTrack?.classList.remove("is-filling");
                writeProgression(targetValues);
                if (activeRewardStage) {
                    if (typeof activeRewardStage.animate === "function") {
                        activeRewardStage.dataset.waapiExit = "true";
                        exitAnim = activeRewardStage.animate([{ opacity: 1 }, { opacity: 0 }], {
                            duration: plan.exitDurationMs,
                            easing: "ease-in",
                            fill: "forwards",
                        });
                        if (mode === "accelerating") {
                            try {
                                exitAnim.playbackRate = exitSpeed;
                            } catch {}
                        }
                    } else {
                        activeRewardStage.classList.add("is-exiting");
                    }
                }
            }
        } else {
            const currentExitRate = mode === "accelerating" ? exitSpeed : 1;
            exitElapsedMs += dt * currentExitRate;
            if (exitElapsedMs >= plan.exitDurationMs) {
                finishImmediate();
                return;
            }
        }

        frameId = requestAnimationFrame(stepFrame);
    }

    frameId = requestAnimationFrame(stepFrame);
}

function offerUnlockedSkin(receipts: MenuRewardReceipt[], done: () => void): boolean {
    const skin = menuUi.state?.store?.skins?.find((item) =>
        item.owned && receipts.some((receipt) => receipt.unlocked_skin === item.id),
    );
    if (!skin || !menuUi.state || menuUi.state.selected_skin === skin.id) return false;

    const layer = document.createElement("div");
    const skinId = skin.id;
    const label = skinName(skin);
    layer.dataset.skinReward = skinId;
    layer.className = "sow-menu__overlay sow-menu__reward-double";
    layer.innerHTML =
        "<section class='sow-menu__modal' role='dialog' aria-modal='true' aria-label='" + esc(label) +
        "'><h2>" + esc(label) + "</h2><div class='sow-store__skin-art'><img " +
        skinIconAttributes(skin.id, asset(skin.asset_path)) + " alt='' width='96' height='96'></div>" +
        "<div class='sow-menu__reward-double-actions'>" +
        "<button class='sow-menu__secondary' type='button' data-skin-continue>" +
        esc(SOW_t("endgame.reward_continue")) +
        "</button><button class='sow-menu__secondary' type='button' data-skin-equip>" +
        esc(SOW_t("store.equip")) + "</button></div></section>";
    layer._shell = root.querySelector<HTMLElement>(".sow-menu__shell");
    layer._shellWasInert = Boolean(layer._shell?.inert);
    layer._restoreFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (layer._shell) layer._shell.inert = true;
    root.appendChild(layer);
    activeRewardStage = layer;

    function close(equip: boolean) {
        if (equip) send("equip_skin", { skin_id: skinId });
        done();
    }
    layer.querySelector("[data-skin-continue]")?.addEventListener("click", () => close(false));
    const equip = layer.querySelector<HTMLButtonElement>("[data-skin-equip]");
    equip?.addEventListener("click", () => close(true));
    layer.addEventListener("keydown", (event) => {
        if (event.key === "Escape") {
            event.stopPropagation();
            close(false);
        }
    });
    equip?.focus({ preventScroll: true });
    return true;
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
            sum.gems += Math.max(0, Number(receipt.gems) || 0);
            return sum;
        },
        { xp: 0, crowns: 0, laurels: 0, gems: 0 },
    );
    const unlockedLeaderIds = Array.from(
        new Set(
            receipts.reduce<string[]>(function (acc, receipt) {
                return acc.concat(Array.isArray(receipt.unlocked_leaders) ? receipt.unlocked_leaders : []);
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
            selector: "[data-nav-screen='heroes']",
        };
    });
    const unlockedAchievementIds = Array.from(
        new Set(
            receipts.reduce<string[]>(function (acc, receipt) {
                const matchReceipt =
                    (receipt.id === receipt.match_id ||
                        receipt.id === "verified_bonus:" + receipt.match_id) &&
                    (receipt.verification_status === "verified" ||
                        receipt.verification_status === "rejected");
                if (matchReceipt) return acc;
                return acc.concat(
                    Array.isArray(receipt.unlocked_achievements) ? receipt.unlocked_achievements : [],
                );
            }, []),
        ),
    ).filter(function (id: string) {
        return /^[a-z][a-z0-9_]{0,63}$/.test(id);
    });
    const achievementStages: RewardStage[] = unlockedAchievementIds.map(function (id: string): RewardStage {
        const definition = getAchievementDefinition(id);
        return {
            kind: "achievement",
            amount: 0,
            label: (definition && definition.title) || id.replace(/_/g, " "),
            selector: ".sow-menu__portrait",
        };
    });
    const canonical = progressionFromState();
    const startValues = Object.assign({}, displayedProgression || canonical);
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
        for (let level = startValues.level + 1; level <= canonical.level; level++) gainedLevels.add(level);
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
        { kind: "gems" as const, amount: totals.gems, selector: "[data-progression-gems-value]" },
        // Laurels live in the profile, so their card lands on the player's portrait.
        { kind: "laurels" as const, amount: totals.laurels, selector: ".sow-menu__portrait" },
        ...leaderStages,
        ...levelStages,
        ...achievementStages,
    ].filter(function (stage) {
        return stage.kind === "leader" || stage.kind === "achievement" || stage.amount > 0;
    });

    function finish(shown: boolean, skinPresented = false) {
        if (token !== rewardAnimationToken || !menuUi.state || menuUi.state.account_id !== accountId) return;
        writeProgression(progressionFromState());
        if (shown && !skinPresented) {
            removeRewardStage();
            if (offerUnlockedSkin(receipts, () => finish(true, true))) return;
        }
        rewardAnimationToken += 1;
        rewardAnimationRunning = false;
        removeRewardStage();
        if (!shown) {
            if (document.visibilityState === "visible") maybePresentRewards();
            return;
        }
        ids.forEach(function (id) {
            rewardAnimationShown.add(id);
        });
        acknowledgeShownReceipts(accountId);
        if (!offerRewardDouble(receipts, accountId)) maybePresentRewards();
    }

    if (reducedRewardMotion()) {
        if (!leaderStages.length && !achievementStages.length) {
            finish(true);
            return;
        }
        const staticItems = createRewardStage(
            stages,
            function () {
                finish(true);
            },
            { useWaapiReveal: false },
        );
        staticItems.forEach(function (item: RewardItem) {
            item.amount.textContent = formatRewardCardText(item.stage, 1);
        });
        return;
    }

    runChoreographedStage({
        token,
        stages,
        startValues,
        targetValues: canonical,
        onComplete: finish,
    });
}

function rankedResultKey(state: MenuState) {
    return [state.account_id || "", state.menu_return && state.menu_return.match_id].join(":");
}

function rankedResultKicker(result: RankedResult) {
    if (result.tier_before !== result.tier_after) {
        return result.score_after > result.score_before
            ? SOW_t("lobbies.rank_promoted")
            : SOW_t("ranked.rank_changed");
    }
    return SOW_t("lobbies.ranked_1v1");
}

function presentRankedResult(result: RankedResult) {
    const layer = document.createElement("section");
    layer.className = "sow-menu__reward-stage sow-menu__ranked-result-stage";
    layer.setAttribute("role", "dialog");
    layer.setAttribute("aria-modal", "true");
    layer.setAttribute("aria-labelledby", "sow-ranked-result-title");
    const card = document.createElement("div");
    const changed =
        result.tier_before !== result.tier_after || result.division_before !== result.division_after;
    const league = result.tier_before !== result.tier_after;
    const promoted = changed && result.score_after > result.score_before;
    const delta = result.score_after - result.score_before;
    const direction = delta > 0 ? "up" : delta < 0 ? "down" : "flat";
    const kicker = rankedResultKicker(result);
    card.className =
        "sow-menu__ranked-result-card" +
        (promoted && league ? " is-league-up" : "") +
        (changed && !promoted ? " is-rank-down" : "");
    card.dataset.tier = rankTier(result.tier_after || "Nomad");
    card.innerHTML =
        "<header class='sow-ranked__result-header'><span class='sow-ranked__eyebrow'>" +
        esc(kicker) +
        "</span></header><div class='sow-ranked__result-pedestal'><div class='sow-ranked__result-seal'>" +
        "<div class='sow-ranked__old-emblem'>" +
        rankEmblem(result.tier_before || "Nomad", result.division_before) +
        "</div><div data-rank-current-emblem>" +
        rankEmblem(result.tier_after || "Nomad", result.division_after) +
        "</div></div>" +
        "<div class='sow-ranked__rank-ribbon'><h2 id='sow-ranked-result-title' class='sow-menu__ranked-result-rank' data-rank-name>" +
        esc(rankName(result.tier_after || "Nomad", result.division_after)) +
        "</h2></div>" +
        "</div>" +
        "<div class='sow-ranked__score-vault'><div class='sow-ranked__score-main'><strong class='sow-ranked__points' data-rank-score>" +
        esc(SOW_t("lobbies.rank_score", { score: result.score_after })) +
        "</strong><span class='sow-menu__ranked-result-change' data-direction='" +
        direction +
        "'>" +
        esc((delta > 0 ? "+" : "") + delta) +
        "</span></div>" +
        (result.points_breakdown ? rankedPointsBreakdownMarkup(result.points_breakdown) : "") +
        "</div>" +
        rankProgressMarkup(result.score_after, result.ladder || menuUi.rankedLadder) +
        "<div class='sow-ranked__actions'><button type='button' class='sow-menu__primary' data-ranked-again>" +
        esc(SOW_t("ranked.play_again")) +
        "</button><button type='button' class='sow-menu__secondary' data-ranked-continue>" +
        esc(SOW_t("lobbies.rank_continue")) +
        "</button></div>";
    const again = card.querySelector<HTMLButtonElement>("[data-ranked-again]")!;
    const close = card.querySelector<HTMLButtonElement>("[data-ranked-continue]")!;
    let dismissed = false;
    let motion: ReturnType<typeof animateRankResult> | null = null;
    const dismiss = function (event: Event, play = false) {
        event.preventDefault();
        event.stopPropagation();
        if (dismissed || !menuUi.state) return;
        dismissed = true;
        again.disabled = close.disabled = true;
        motion?.finish();
        rankedResultDismissedKey = rankedResultKey(menuUi.state);
        send("dismiss_ranked_result");
        removeRewardStage();
        if (play) {
            menuUi.profileOpen = menuUi.storeOpen = menuUi.heroesOpen = menuUi.campaignOpen = false;
            send("quick_match");
        }
    };
    again.addEventListener("click", (event) => dismiss(event, true));
    close.addEventListener("click", (event) => dismiss(event));
    layer.addEventListener("click", (event) => {
        if (event.target instanceof Element && event.target.closest("button")) return;
        motion?.accelerate();
    });
    layer.addEventListener("keydown", (event) => {
        if (event.key === "Escape") dismiss(event);
        if (event.key === "Tab") {
            event.preventDefault();
            (document.activeElement === again ? close : again).focus({ preventScroll: true });
        }
    });
    layer.appendChild(card);
    layer._shell = root.querySelector<HTMLElement>(".sow-menu__shell");
    layer._shellWasInert = Boolean(layer._shell && layer._shell.inert);
    layer._restoreFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    root.appendChild(layer);
    activeRewardStage = layer;
    rankedStageKey = menuUi.state ? rankedResultKey(menuUi.state) : null;
    if (layer._shell && "inert" in layer._shell) layer._shell.inert = true;
    motion = animateRankResult(card, result, reducedRewardMotion);
    rankedStageCleanup = motion.cancel;
    again.focus({ preventScroll: true });
}

const VERIFIED_BONUS_PREFIX = "verified_bonus:";
const rewardDoubleOffered = new Set<string>();

// One rewarded-video offer per verified public match: it pays the match's
// crowns and gems again. Continue and the video button share size and
// placement (Poki and CrazyGames rules); portals without rewarded video,
// such as Jest, report it unavailable and never show the offer.
function offerRewardDouble(receipts: MenuRewardReceipt[], accountId: string | null): boolean {
    if (!accountId || !isRewardedAdAvailable()) return false;
    const bonus = receipts
        .filter(function (receipt) {
            return typeof receipt.id === "string" && receipt.id.indexOf(VERIFIED_BONUS_PREFIX) === 0;
        })
        .pop();
    if (!bonus || rewardDoubleOffered.has(bonus.match_id)) return false;
    const all = (menuUi.state && menuUi.state.reward_receipts) || [];
    const already = all.some(function (receipt) {
        return receipt.id === "ad_double:" + bonus.match_id;
    });
    const participation = all.find(function (receipt) {
        return receipt.id === bonus.match_id;
    });
    const crowns = (Number(bonus.crowns) || 0) + (Number(participation && participation.crowns) || 0);
    const gems = (Number(bonus.gems) || 0) + (Number(participation && participation.gems) || 0);
    if (already || crowns + gems <= 0) return false;
    rewardDoubleOffered.add(bonus.match_id);
    const layer = document.createElement("div");
    layer.className = "sow-menu__overlay sow-menu__reward-double";
    const amounts =
        "<span class='sow-menu__reward-double-amounts'>" +
        (crowns > 0
            ? "<span><img src='" + esc(currencyAsset("crown")) + "' alt=''>+" + crowns + "</span>"
            : "") +
        (gems > 0 ? "<span><img src='" + esc(currencyAsset("gem")) + "' alt=''>+" + gems + "</span>" : "") +
        "</span>";
    layer.innerHTML =
        "<section class='sow-menu__modal' role='dialog' aria-modal='true' aria-label='" +
        esc(SOW_t("endgame.reward_double_title")) +
        "'><h2>" +
        esc(SOW_t("endgame.reward_double_title")) +
        "</h2><p>" +
        esc(SOW_t("endgame.reward_double_body")) +
        "</p>" +
        amounts +
        "<div class='sow-menu__reward-double-actions'>" +
        "<button class='sow-menu__secondary' type='button' data-reward-double='continue'>" +
        esc(SOW_t("endgame.reward_continue")) +
        "</button><button class='sow-menu__secondary' type='button' data-reward-double='watch'>" +
        "<svg class='sow-menu__reward-double-video' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linejoin='round' aria-hidden='true'><rect x='2' y='5' width='14' height='14' rx='2'/><path d='m16 10 6-4v12l-6-4z'/></svg>" +
        esc(SOW_t("endgame.reward_double_watch")) +
        "</button></div></section>";
    const close = function () {
        layer.remove();
        maybePresentRewards();
    };
    layer.addEventListener("click", function (event) {
        const choice =
            event.target instanceof Element
                ? event.target.closest<HTMLElement>("[data-reward-double]")
                : null;
        if (!choice) return;
        event.preventDefault();
        event.stopPropagation();
        if (choice.dataset.rewardDouble !== "watch") {
            close();
            return;
        }
        layer.querySelectorAll("button").forEach(function (button) {
            button.disabled = true;
        });
        window.SOW_portalRewardedBreak!().then(function (watched) {
            if (watched && menuUi.state && menuUi.state.account_id === accountId)
                send("double_match_reward", { account_id: accountId, match_id: bonus.match_id });
            close();
        });
    });
    root.appendChild(layer);
    layer.querySelector<HTMLElement>("[data-reward-double='continue']")?.focus({ preventScroll: true });
    return true;
}

function currentMatchGoalSummary(): MatchGoalSummary | null {
    const accountId = menuUi.state?.account_id || "";
    if (!accountId) return null;
    const receipts = menuUi.state?.reward_receipts || noMatchGoalReceipts;
    if (
        matchGoalSummaryCache &&
        matchGoalSummaryCache.accountId === accountId &&
        matchGoalSummaryCache.receipts === receipts
    )
        return matchGoalSummaryCache.summary;
    const summary = latestMatchGoalSummary(receipts);
    matchGoalSummaryCache = { accountId, receipts, summary };
    return summary;
}

function matchGoalAchievementCatalog(summary: MatchGoalSummary): Map<string, MatchGoalAchievement> {
    const ids = new Set(summary.unlockedIds);
    summary.advances.forEach((advance) => ids.add(advance.achievement_id));
    const catalog = new Map<string, MatchGoalAchievement>();
    for (const id of ids) {
        const definition = getAchievementDefinition(id);
        if (!definition) continue;
        const enriched = enrichAchievement(definition, { isOwnProfile: true });
        catalog.set(id, {
            id,
            title: enriched.localizedTitle,
            points: definition.points,
            target: definition.target,
            chainId: enriched.chainKey,
        });
    }
    return catalog;
}

function matchGoalChainLabel(chainId: string): string {
    if (chainId === "series:battles") return SOW_t("profile.ach_sub_battles");
    if (chainId === "series:victories") return SOW_t("profile.ach_sub_victories");
    if (chainId === "series:kills" || chainId === "series:assists")
        return SOW_t("profile.ach_sub_slaying");
    if (chainId === "series:players") return SOW_t("profile.ach_sub_commanders_defeated");
    if (chainId === "series:empires" || chainId === "series:tribes")
        return SOW_t("profile.ach_sub_conquest");
    const [kind, slug] = chainId.split(":");
    if (kind === "hero_mastery" || kind === "hero_wins") {
        const leader = leaderById(slug);
        const name = leader ? leaderDisplayName(leader) : slug.replace(/_/g, " ");
        const category = SOW_t(
            kind === "hero_mastery" ? "profile.ach_sub_mastery" : "profile.ach_sub_hero_wins",
        );
        return name + " · " + category;
    }
    return SOW_t("profile.achievements");
}

function dismissMatchGoalToast(): void {
    if (matchGoalToastTimer !== null) window.clearTimeout(matchGoalToastTimer);
    matchGoalToastTimer = null;
    if (matchGoalVisibilityHandler) {
        document.removeEventListener("visibilitychange", matchGoalVisibilityHandler);
        matchGoalVisibilityHandler = null;
    }
    if (activeMatchGoalToast) {
        activeMatchGoalToast.remove();
        activeMatchGoalToast = null;
    }
}

function goalToastRow(highlight: MatchGoalHighlight): string {
    const { advance, achievement } = highlight;
    const target = advance ? advance.target : highlight.target;
    const before = advance ? advance.before : highlight.progressBefore;
    const after = advance ? advance.after : highlight.progressAfter;
    const previous = advance ? advance.previous : highlight.previousTarget;
    const amount = Math.max(0, after - before);
    const span = Math.max(1, target - previous);
    const beforePct = Math.max(0, Math.min(100, ((before - previous) / span) * 100));
    const afterPct = Math.max(beforePct, Math.min(100, ((after - previous) / span) * 100));
    const count =
        highlight.kind === "unlock"
            ? "<span class='sow-match-goals-toast__unlocked'>" +
              esc(SOW_t("profile.match_goals_unlocked")) +
              "</span>"
            : "<b class='sow-match-goals-toast__count'>" +
              esc(formatCompactNumber(Math.min(after, target))) +
              "/" +
              esc(formatCompactNumber(target)) +
              " <i>+" +
              esc(formatCompactNumber(amount)) +
              "</i></b>";
    const heroSlug = /^hero_(?:mastery|wins):([a-z0-9_]+)$/.exec(highlight.chainId)?.[1];
    const hero = heroSlug ? leaderById(heroSlug) : null;
    const heroArt =
        hero && heroSlug && hero.id.replace(/[^a-z0-9]/gi, "").toLowerCase() === heroSlug.replace(/[^a-z0-9]/gi, "").toLowerCase()
            ? leaderArtSlug(hero)
            : "";
    const icon = /^[a-z0-9_]+$/.test(heroArt)
        ? "<span class='sow-match-goals-toast__icon'><img " +
          avatarImageAttribute(asset("gameplay/avatars/" + heroArt + ".webp")) +
          " alt='' aria-hidden='true'></span>"
        : "<span class='sow-match-goals-toast__icon sow-match-goals-toast__icon--generic' aria-hidden='true'>✦</span>";
    return (
        "<li><div class='sow-match-goals-toast__row'>" +
        icon +
        "<div><small>" +
        esc(matchGoalChainLabel(highlight.chainId)) +
        "</small><strong>" +
        esc(achievement.title) +
        "</strong></div>" +
        count +
        "</div><div class='sow-match-goals-toast__meter'><i style='width:" +
        beforePct.toFixed(2) +
        "%'></i><b style='inset-inline-start:" +
        beforePct.toFixed(2) +
        "%;width:" +
        Math.max(0, afterPct - beforePct).toFixed(2) +
        "%'></b></div></li>"
    );
}

function toastOverlapsControl(toast: HTMLElement): boolean {
    const rect = toast.getBoundingClientRect();
    const controls = root.querySelectorAll<HTMLElement>(
        "button, a[href], input, select, textarea, [role='button']",
    );
    for (const control of controls) {
        if (toast.contains(control) || !control.getClientRects().length) continue;
        const style = window.getComputedStyle(control);
        if (style.visibility === "hidden" || style.display === "none") continue;
        const other = control.getBoundingClientRect();
        if (
            Math.min(rect.right, other.right) - Math.max(rect.left, other.left) > 8 &&
            Math.min(rect.bottom, other.bottom) - Math.max(rect.top, other.top) > 8
        )
            return true;
    }
    return false;
}

function placeMatchGoalToast(toast: HTMLElement, compactLandscape: boolean): boolean {
    const dock = root.querySelector<HTMLElement>(".sow-menu__nav-dock");
    const dockRect = dock?.getBoundingClientRect();
    const dockAbove = dockRect && dockRect.height > 0 && dockRect.width > window.innerWidth * 0.65;
    const bottom = dockAbove ? Math.max(12, window.innerHeight - dockRect.top + 10) : 18;
    const header = root.querySelector<HTMLElement>(".sow-menu__topbar");
    const top = (header?.getBoundingClientRect().bottom || 0) + 12;
    const placements = dockAbove
        ? [
              { left: 14, right: "auto", top: "auto", bottom },
              { left: "auto", right: 14, top: "auto", bottom },
          ]
        : [
              { left: 14, right: "auto", top: "auto", bottom },
              { left: "auto", right: 14, top: "auto", bottom },
              { left: 14, right: "auto", top, bottom: "auto" },
              { left: "auto", right: 14, top, bottom: "auto" },
          ];
    for (const placement of placements) {
        toast.style.left = typeof placement.left === "number" ? placement.left + "px" : placement.left;
        toast.style.right = typeof placement.right === "number" ? placement.right + "px" : placement.right;
        toast.style.top = typeof placement.top === "number" ? placement.top + "px" : placement.top;
        toast.style.bottom = typeof placement.bottom === "number" ? placement.bottom + "px" : placement.bottom;
        if (!toastOverlapsControl(toast)) return true;
    }
    return !compactLandscape && !toastOverlapsControl(toast);
}

function showMatchGoalToast(
    accountId: string,
    summary: MatchGoalSummary,
    highlights: MatchGoalHighlight[],
): void {
    dismissMatchGoalToast();
    const compactLandscape =
        window.matchMedia("(orientation: landscape) and (max-height: 680px)").matches;
    const toast = document.createElement("section");
    toast.className = "sow-match-goals-toast";
    toast.dataset.accountId = accountId;
    toast.dataset.matchId = summary.matchId;
    toast.dataset.compactLandscape = String(compactLandscape);
    toast.setAttribute("role", "status");
    toast.setAttribute("aria-live", "polite");
    toast.setAttribute("aria-atomic", "true");
    toast.innerHTML =
        "<header><strong>" +
        esc(SOW_t("profile.match_goals_toast_title")) +
        "</strong><button type='button' data-match-goals-dismiss aria-label='" +
        esc(SOW_t("profile.match_goals_dismiss")) +
        "'>×</button></header><ul>" +
        highlights.map(goalToastRow).join("") +
        "</ul><footer><button type='button' data-match-goals-review>" +
        esc(
            SOW_t("profile.match_goals_view", {
                count: Math.max(summary.advances.length, summary.unlockedIds.length),
            }),
        ) +
        "</button></footer><div class='sow-match-goals-toast__timer' aria-hidden='true'><i></i></div>";
    toast.classList.add("is-paused");
    root.appendChild(toast);
    if (!placeMatchGoalToast(toast, compactLandscape)) {
        toast.remove();
        return;
    }
    activeMatchGoalToast = toast;

    let remaining = 9_000;
    let startedAt = 0;
    let timer: number | null = null;
    const timerBar = toast.querySelector<HTMLElement>(".sow-match-goals-toast__timer i");
    if (timerBar) timerBar.style.animationDuration = remaining + "ms";
    const pause = function () {
        if (timer === null) return;
        window.clearTimeout(timer);
        timer = null;
        matchGoalToastTimer = null;
        remaining = Math.max(0, remaining - (performance.now() - startedAt));
        toast.classList.add("is-paused");
    };
    const resume = function () {
        if (
            timer !== null ||
            toast.matches(":hover") ||
            toast.contains(document.activeElement) ||
            document.visibilityState !== "visible"
        )
            return;
        toast.classList.remove("is-paused");
        startedAt = performance.now();
        timer = window.setTimeout(() => dismissMatchGoalToast(), remaining);
        matchGoalToastTimer = timer;
    };
    matchGoalVisibilityHandler = function () {
        if (document.visibilityState === "visible") resume();
        else pause();
    };
    toast.addEventListener("pointerenter", pause);
    toast.addEventListener("pointerleave", resume);
    toast.addEventListener("focusin", pause);
    toast.addEventListener("focusout", function (event) {
        if (!(event.relatedTarget instanceof Node) || !toast.contains(event.relatedTarget)) resume();
    });
    toast.addEventListener("keydown", function (event) {
        if (event.key === "Escape") {
            event.preventDefault();
            dismissMatchGoalToast();
        }
    });
    document.addEventListener("visibilitychange", matchGoalVisibilityHandler);
    toast.querySelector<HTMLButtonElement>("[data-match-goals-dismiss]")?.addEventListener("click", () => {
        dismissMatchGoalToast();
    });
    toast.querySelector<HTMLButtonElement>("[data-match-goals-review]")?.addEventListener("click", () => {
        dismissMatchGoalToast();
        openProfile(null);
        menuUi.profileTab = "achievements";
        render();
        root.querySelector<HTMLElement>("#sow-profile-last-match-goals")?.focus({ preventScroll: true });
    });
    resume();
}

function maybePresentMatchGoalToast(): void {
    const accountId = menuUi.state?.account_id;
    const summary = currentMatchGoalSummary();
    if (!accountId || !summary || hasHandledMatchGoalToast(accountId, summary.matchId)) return;
    if (hasViewedMatchGoals(accountId, summary.matchId)) {
        markMatchGoalToastHandled(accountId, summary.matchId);
        return;
    }
    if (!menuFlow.canGuide()) return;

    markMatchGoalToastHandled(accountId, summary.matchId);
    const guideVisible = Boolean(root.querySelector(".sow-menu-whisper:not([hidden])"));
    const directDestination = Boolean(
        menuUi.state?.menu_return?.requested_screen &&
            menuUi.state.menu_return.requested_screen !== "home",
    );
    if (
        root.hidden ||
        document.visibilityState !== "visible" ||
        currentScreen() !== "home" ||
        guideVisible ||
        directDestination ||
        activeRewardStage ||
        rewardAnimationRunning ||
        menuUi.profileOpen ||
        menuUi.storeOpen ||
        menuUi.heroesOpen ||
        menuUi.campaignOpen ||
        menuUi.settingsOpen ||
        menuUi.authModalOpen ||
        menuUi.missionsModalOpen ||
        menuUi.purchaseModal
    )
        return;
    const highlights = selectMatchGoalHighlights(summary, matchGoalAchievementCatalog(summary));
    if (highlights.length) showMatchGoalToast(accountId, summary, highlights);
}

function syncMatchGoalProfileBadge(): void {
    const accountId = menuUi.state?.account_id;
    const summary = currentMatchGoalSummary();
    const unseen = Boolean(
        accountId &&
            summary &&
            (summary.advances.length || summary.unlockedIds.length) &&
            !hasViewedMatchGoals(accountId, summary.matchId),
    );
    const button = root.querySelector<HTMLElement>(
        ".sow-menu__main-nav-item[data-nav-screen='profile']",
    );
    if (!button) return;
    button.classList.toggle("has-unseen-match-goals", unseen);
    let marker = button.querySelector<HTMLElement>(".sow-menu__nav-marker");
    if (!marker) {
        marker = document.createElement("span");
        marker.className = "sow-menu__nav-marker";
        marker.setAttribute("aria-hidden", "true");
        button.appendChild(marker);
    }
    marker.hidden = !unseen;
    const label = SOW_t("menu.profile");
    button.setAttribute(
        "aria-label",
        unseen ? label + ", " + SOW_t("profile.match_goals_new") : label,
    );
}

function maybePresentRewards() {
    if (!menuUi.state) return;
    if (
        rankedStageKey &&
        (menuUi.state.phase !== "MainMenu" || rankedStageKey !== rankedResultKey(menuUi.state))
    )
        removeRewardStage();
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
    const guide = window.SOW_menu_guide;
    if (guide) guide.update(menuUi.state, menuGuideFacts());
    syncMatchGoalProfileBadge();
    if (
        activeMatchGoalToast &&
        (activeMatchGoalToast.dataset.accountId !== menuUi.state.account_id ||
            root.hidden ||
            currentScreen() !== "home" ||
            menuUi.profileOpen ||
            menuUi.storeOpen ||
            menuUi.settingsOpen ||
            menuUi.authModalOpen ||
            menuUi.missionsModalOpen ||
            Boolean(root.querySelector(".sow-menu-whisper:not([hidden])")))
    )
        dismissMatchGoalToast();
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
    maybePresentMatchGoalToast();
}

function menuGuideFacts() {
    const profileStats = menuUi.state?.profile_stats;
    return {
        screen: currentScreen(),
        missionsOpen: Boolean(menuUi.missionsModalOpen),
        settingsOpen: Boolean(menuUi.settingsOpen),
        heroInfoOpen: Boolean(menuUi.heroInfoOpen),
        authModalOpen: Boolean(menuUi.authModalOpen),
        skinPickerOpen: Boolean(menuUi.skinPickerOpen),
        level: Number(menuUi.state?.level || 1),
        matchesPlayed: Number(profileStats?.matches_played || 0),
        rankedMatches: Number(profileStats?.ranked_matches || 0),
    };
}

function updateDynamic() {
    if (!menuUi.state) return;
    root.dataset.reducedMotion = String(reducedRewardMotion());
    menuMusicSyncSpectrum();
    if (root.hidden) return;
    if (menuUi.storeOpen && typeof loadStoreCatalog === "function") loadStoreCatalog();
    ensureProgressionDisplay();
    if (
        reducedRewardMotion() &&
        (progressionAnimationTarget || (rewardAnimationRunning && !activeRewardStage?.dataset.skinReward))
    ) {
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
    const sfxInput = root.querySelector<HTMLInputElement>("[data-setting='sfx_volume']");
    const freeZoomInput = root.querySelector<HTMLInputElement>("[data-setting='free_zoom_out']");
    const fpsPingInput = root.querySelector<HTMLInputElement>("[data-setting='show_fps_ping']");
    const devToolsInput = root.querySelector<HTMLInputElement>("[data-setting='show_dev_tools']");
    const menuTipsInput = root.querySelector<HTMLInputElement>("[data-setting='menu_tips']");
    for (const [sliderInput, rawVal, badgeKey] of [
        [musicInput, settings.music_volume, "music_vol"],
        [sfxInput, settings.sfx_volume, "sfx_vol"],
    ] as const) {
        const interacting =
            isDraggingSettingsSlider ||
            (sliderInput &&
                (document.activeElement === sliderInput ||
                    sliderInput.matches(":active") ||
                    activeMenuPointerId !== null));
        if (sliderInput && !interacting) {
            sliderInput.value = String(rawVal == null ? 0.8 : rawVal);
        }
        const badge = root.querySelector<HTMLElement>("[data-val-for='" + badgeKey + "']");
        if (badge && sliderInput) {
            const pct = Math.round(Number(sliderInput.value) * 100);
            badge.textContent = pct + "%";
            sliderInput.style.setProperty("--sow-slider-fill", pct + "%");
            sliderInput.style.setProperty("--sow-music-fill", pct + "%");
        }
    }
    if (fpsPingInput && document.activeElement !== fpsPingInput)
        fpsPingInput.checked = settings.show_fps_ping === true;
    if (devToolsInput && document.activeElement !== devToolsInput)
        devToolsInput.checked = Boolean(settings.show_dev_tools);
    if (freeZoomInput && document.activeElement !== freeZoomInput)
        freeZoomInput.checked = Boolean(settings.free_zoom_out);
    if (menuTipsInput && document.activeElement !== menuTipsInput)
        menuTipsInput.checked = !menuUi.state.menu_tips_disabled;
    const timer = panel.querySelector<HTMLElement>("[data-live-countdown]");
    const lobby = joinedLobby();
    if (timer && lobby)
        timer.textContent = menuUi.state.match_preparing
            ? SOW_t("menu.loading")
            : lobby.is_counting_down
              ? SOW_t("lobbies.starting_in", { seconds: Math.ceil(lobby.timer_secs) })
              : SOW_t("lobbies.waiting_for_players");
    updateQueueRoster(lobby);
    updateQueueDownloadProgress();
    updateRankedQueue();
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

document.addEventListener(
    "error",
    (event) => {
        const image = event.target;
        if (!(image instanceof HTMLImageElement)) return;
        const emblem = image.closest(".sow-ranked__emblem");
        if (!emblem) return;
        image.hidden = true;
        emblem.classList.add("is-missing");
    },
    true,
);

let leaderDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let pendingDebouncedSelection: { preferRandom: boolean; leaderId: string | null } | null = null;

export function flushLeaderPersist() {
    if (leaderDebounceTimer) {
        clearTimeout(leaderDebounceTimer);
        leaderDebounceTimer = null;
    }
    if (pendingDebouncedSelection) {
        const selection = pendingDebouncedSelection;
        pendingDebouncedSelection = null;
        if (selection.preferRandom) {
            send("set_random_leader");
        } else if (selection.leaderId) {
            send("set_leader", { leader_id: selection.leaderId });
        }
    }
}

export function scheduleLeaderPersist(preferRandom: boolean, leaderId: string | null) {
    if (leaderDebounceTimer) {
        clearTimeout(leaderDebounceTimer);
        leaderDebounceTimer = null;
    }
    pendingDebouncedSelection = { preferRandom, leaderId };
    leaderDebounceTimer = setTimeout(function () {
        flushLeaderPersist();
    }, 1000);
}

window.addEventListener("pagehide", function () {
    flushLeaderPersist();
});

root.addEventListener("error", function (event) {
    const image = event.target;
    if (!(image instanceof HTMLImageElement)) return;
    const fallback = image.dataset.mapPreviewFallback;
    if (!fallback) return;
    delete image.dataset.mapPreviewFallback;
    image.src = fallback;
}, true);

installSurfaceHoverPressAudio(root);

root.addEventListener("click", function (event) {
    if (menuUi.state && menuUi.state.phase === "MainMenu") syncMenuMusic(menuUi.state, true);
    const clickedElement = event.target instanceof Element ? event.target : null;
    if (!clickedElement) return;
    const purchaseOverlay = clickedElement.closest("[data-menu-overlay='purchase']");
    if (purchaseOverlay && clickedElement === purchaseOverlay) {
        if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
        playUiCue(AudioCue.UiModalClose);
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
        playUiCue(AudioCue.UiModalClose);
        menuUi.skinPickerOpen = false;
        render();
        return;
    }
    const heroInfoOverlay = clickedElement.closest("[data-menu-overlay='hero-info']");
    if (heroInfoOverlay && clickedElement === heroInfoOverlay) {
        playUiCue(AudioCue.UiModalClose);
        menuUi.heroInfoOpen = false;
        menuUi.heroInfoLeaderId = null;
        render();
        return;
    }
    const settingsOverlay = clickedElement.closest("[data-menu-overlay='settings']");
    if (settingsOverlay && clickedElement === settingsOverlay) {
        playUiCue(AudioCue.UiModalClose);
        menuUi.settingsOpen = false;
        menuUi.signOutConfirmOpen = false;
        menuUi.dropdownOpenKey = null;
        render();
        return;
    }
    const missionsOverlay = clickedElement.closest("[data-menu-overlay='missions']");
    if (missionsOverlay && clickedElement === missionsOverlay) {
        playUiCue(AudioCue.UiModalClose);
        menuUi.missionsModalOpen = false;
        render();
        return;
    }
    const targetElement = clickedElement.closest("[data-command]");
    if (!(targetElement instanceof HTMLElement) || !root.contains(targetElement)) return;
    const target = targetElement;
    const command = target.dataset.command;
    if (!command) return;
    if (handleSystemNoticeCommand(command)) {
        MenuSfx.modalClose();
        return;
    }
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
        MenuSfx.battleSubmenu();
        menuUi.profileOpen = false;
        menuUi.profileAccountId = null;
        menuUi.profileMatchDetail = null;
        menuUi.heroesOpen = false;
        menuUi.storeOpen = false;
        menuUi.campaignOpen = true;
        if (target.dataset.sagaId) {
            menuUi.selectedCampaignSaga = target.dataset.sagaId;
        }
        if (target.dataset.episodeId) {
            menuUi.selectedCampaignEpisode = target.dataset.episodeId;
        }
        render();
        updateHeroBackdrop("campaign");
        return;
    }
    if (command === "claim_campaign_hero") {
        const leaderId = target.dataset.leaderId;
        if (leaderId) {
            MenuSfx.purchaseConfirm();
            send("claim_campaign_hero", { leader_id: leaderId });
        }
        return;
    }
    if (command === "close_campaign") {
        MenuSfx.back();
        menuUi.campaignOpen = false;
        render();
        updateHeroBackdrop(currentScreen());
        return;
    }
    if (command === "select_campaign_episode") {
        const episodeId = target.dataset.episodeId;
        if (episodeId) {
            MenuSfx.heroBrowse("available", target);
            menuUi.selectedCampaignEpisode = episodeId;
            if (!updateCampaignSelection(episodeId)) render();
            updateHeroBackdrop("campaign");
        }
        return;
    }
    if (command === "select_campaign_saga") {
        const sagaId = target.dataset.sagaId || "all";
        if (menuUi.selectedCampaignSaga !== sagaId) {
            MenuSfx.infoTab(target);
            menuUi.selectedCampaignSaga = sagaId;
            const campaign = menuUi.state && menuUi.state.campaign;
            const episodes = campaign && Array.isArray(campaign.episodes) ? campaign.episodes : [];
            if (sagaId !== "all") {
                const sagaEpisodes = episodes.filter((ep) => campaignEpisodeSagaId(ep.id) === sagaId);
                if (
                    sagaEpisodes.length &&
                    !sagaEpisodes.some((ep) => ep.id === menuUi.selectedCampaignEpisode)
                ) {
                    const firstAvailable = sagaEpisodes.find((ep) => ep.unlocked && !ep.completed);
                    menuUi.selectedCampaignEpisode = firstAvailable ? firstAvailable.id : sagaEpisodes[0].id;
                }
            }
            render();
            updateHeroBackdrop("campaign");
        }
        return;
    }
    if (command === "start_campaign_episode") {
        const episodeId = target.dataset.episodeId;
        if (!episodeId) return;
        MenuSfx.battleLaunch();
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
        if (navScreen !== "heroes") {
            flushLeaderPersist();
        }
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
            menuUi.heroesRarityFilter = "all";
            if (menuUi.heroesStatusFilter === "playable") menuUi.heroesStatusFilter = "all";
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
    if (command === "retry_ranked_board") {
        MenuSfx.click();
        loadRankedLeaderboard(true);
        return;
    }
    if (command === "quick_match") {
        MenuSfx.battleLaunch();
        flushLeaderPersist();
        target.setAttribute("disabled", "");
        menuUi.profileOpen = menuUi.storeOpen = menuUi.heroesOpen = menuUi.campaignOpen = false;
        send("quick_match");
        return;
    }
    if (command === "open_profile") {
        if (menuUi.profileOpen) MenuSfx.click();
        menuUi.storeOpen = false;
        menuUi.heroesOpen = false;
        menuUi.campaignOpen = false;
        openProfile(target.dataset.accountId || null);
        return;
    }
    if (command === "close_profile") {
        MenuSfx.back();
        if (popProfileNav()) return;
        menuUi.profileOpen = false;
        menuUi.storeOpen = false;
        menuUi.heroesOpen = false;
        menuUi.profileAccountId = null;
        menuUi.profileData = null;
        menuUi.profileMatchDetail = null;
        menuUi.profileDetailLoading = false;
        menuUi.profileDetailRequestKey = null;
        menuUi.profileNavStack = [];
        render();
        return;
    }
    if (command === "back_to_my_profile") {
        MenuSfx.back();
        menuUi.profileNavStack = [];
        openProfile(menuUi.profileOwnId || (menuUi.state && menuUi.state.account_id) || null);
        return;
    }
    if (command === "profile_tab") {
        const nextTab = target.dataset.profileTab || "overview";
        if (menuUi.profileTab !== nextTab) MenuSfx.infoTab(target);
        menuUi.profileTab = nextTab;
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
    if (command === "profile_achievement_filter") {
        MenuSfx.filterChip(target);
        const nextCat = target.dataset.achievementFilter || "summary";
        menuUi.profileAchievementFilter = nextCat;
        menuUi.profileAchievementSubcategory = "all";
        resetAchievementFlatLimit();
        const catInput = root.querySelector<HTMLInputElement>(
            "[data-control-dropdown][data-dropdown-key='ach-category'] [data-dropdown-input]",
        );
        if (catInput) {
            catInput.value = nextCat;
            syncDropdowns();
        }
        refreshProfileAchievements();
        return;
    }
    if (command === "profile_achievement_subcategory") {
        MenuSfx.filterChip(target);
        menuUi.profileAchievementSubcategory = target.dataset.achievementSubcategory || "all";
        resetAchievementFlatLimit();
        refreshProfileAchievements();
        return;
    }
    if (command === "profile_achievement_toggle_playable") {
        menuUi.profileAchievementPlayableOnly = !menuUi.profileAchievementPlayableOnly;
        MenuSfx.toggleSwitch(menuUi.profileAchievementPlayableOnly);
        resetAchievementFlatLimit();
        refreshProfileAchievements();
        return;
    }
    if (command === "profile_achievement_view_mode") {
        MenuSfx.filterChip(target);
        menuUi.profileAchievementViewMode = target.dataset.viewMode === "grid" ? "grid" : "chains";
        resetAchievementFlatLimit();
        refreshProfileAchievements();
        return;
    }
    if (command === "profile_achievement_show_more") {
        MenuSfx.click();
        bumpAchievementFlatLimit();
        refreshProfileAchievements();
        return;
    }
    if (command === "profile_achievement_clear") {
        MenuSfx.back();
        menuUi.profileAchievementSearch = "";
        menuUi.profileAchievementStatus = "all";
        menuUi.profileAchievementTier = "all";
        menuUi.profileAchievementPlayableOnly = false;
        resetAchievementFlatLimit();
        render();
        return;
    }
    if (command === "select_chain_tier") {
        const chainKey = target.dataset.chainKey;
        const tierId = target.dataset.tierId;
        if (!chainKey || !tierId) return;
        MenuSfx.filterChip(target);
        menuUi.profileAchievementSelectedTiers[chainKey] = tierId;
        refreshProfileAchievements();
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
        MenuSfx.click();
        loadMoreProfileHistory();
        return;
    }
    if (command === "load_victory_more") {
        MenuSfx.click();
        loadVictoryLeaderboard(true);
        return;
    }
    if (command === "retry_profile") {
        MenuSfx.click();
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
        MenuSfx.modalOpen();
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
        MenuSfx.modalClose();
        menuUi.profileMatchDetail = null;
        menuUi.profileDetailError = "";
        menuUi.profileDetailId = null;
        menuUi.profileDetailLoading = false;
        menuUi.profileDetailRequestKey = null;
        render();
        return;
    }
    if (command === "open_report") {
        MenuSfx.modalOpen();
        menuUi.reportOpen = true;
        menuUi.reportTarget = target.dataset.accountId || menuUi.profileAccountId;
        menuUi.reportSent = false;
        render();
        return;
    }
    if (command === "cancel_report") {
        MenuSfx.modalClose();
        menuUi.reportOpen = false;
        menuUi.reportTarget = null;
        render();
        return;
    }
    if (command === "request_delete") {
        if (!menuUi.settingsOpen || !selfCreds() || deleteBusy) return;
        MenuSfx.modalOpen();
        menuUi.deleteArmed = true;
        deleteError = "";
        render();
        return;
    }
    if (command === "cancel_delete") {
        MenuSfx.modalClose();
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
        const opening = menuUi.dropdownOpenKey !== dropdownKey;
        MenuSfx.filterChip(target);
        setDropdownOpen(dropdownKey, opening, target);
        return;
    }
    if (command === "select_dropdown") {
        const dropdownKey = target.dataset.dropdownKey;
        const optionValue = target.dataset.dropdownOptionValue;
        if (!dropdownKey || optionValue == null) return;
        MenuSfx.filterChip(target);
        selectDropdown(dropdownKey, optionValue);
        return;
    }
    if (command === "open_hero_info") {
        MenuSfx.modalOpen();
        const requestedId = target.dataset.leaderId || null;
        const rawTab = target.dataset.heroInfoTab;
        const requestedTab =
            rawTab === "biography" || rawTab === "achievements" || rawTab === "overview"
                ? rawTab
                : "overview";
        const prevLeaderId = menuUi.heroInfoLeaderId;
        if (requestedId) {
            menuUi.heroInfoLeaderId = requestedId;
        } else {
            const sel = currentLeaderSelection();
            menuUi.heroInfoLeaderId = menuUi.tempSelectedLeader || (sel && sel.selected_leader) || null;
        }
        if (menuUi.heroInfoLeaderId !== prevLeaderId) {
            menuUi.heroInfoAchCategory = "all";
            menuUi.heroInfoAchStatus = "all";
        }
        menuUi.heroInfoTab = requestedTab;
        menuUi.heroInfoOpen = true;
        void ensureHeroBiosLoaded();
        render();
        return;
    }
    if (command === "switch_hero_info_tab") {
        const rawTab = target.dataset.heroInfoTab;
        const tab =
            rawTab === "biography" || rawTab === "achievements" || rawTab === "overview"
                ? rawTab
                : "overview";
        if (menuUi.heroInfoTab !== tab) {
            const tabRatio = tab === "overview" ? 0 : tab === "biography" ? 0.5 : 1;
            MenuSfx.infoTab(tabRatio);
        }
        menuUi.heroInfoTab = tab;
        if (tab === "biography") void ensureHeroBiosLoaded();
        render();
        return;
    }
    if (command === "hero_info_ach_category") {
        MenuSfx.filterChip(target);
        const cat = target.dataset.heroAchCategory;
        menuUi.heroInfoAchCategory =
            cat === "recruit" || cat === "mastery" || cat === "victories" ? cat : "all";
        render();
        return;
    }
    if (command === "hero_info_ach_status") {
        MenuSfx.filterChip(target);
        const status = target.dataset.heroAchStatus;
        menuUi.heroInfoAchStatus =
            status === "in_progress" || status === "unlocked" || status === "locked" ? status : "all";
        render();
        return;
    }
    if (command === "hero_info_ach_sort") {
        MenuSfx.filterChip(target);
        const sort = target.dataset.heroAchSort;
        menuUi.heroInfoAchSort =
            sort === "closest" || sort === "points" ? sort : "tier";
        render();
        return;
    }
    if (command === "close_hero_info") {
        MenuSfx.modalClose();
        menuUi.heroInfoOpen = false;
        menuUi.heroInfoLeaderId = null;
        render();
        return;
    }
    if (command === "preview_leader") {
        const leaderId = target.dataset.leaderId || null;
        const leader = leaderById(leaderId);
        const alreadyActive = !menuUi.tempSelectedRandom && menuUi.tempSelectedLeader === leaderId;
        if (alreadyActive) {
            MenuSfx.filterChip(target);
        } else {
            MenuSfx.heroBrowse(leader && leader.available === false ? "locked" : "available", target);
        }
        const persistSelection = !!leaderId && leader.available !== false;
        if (persistSelection) stageLeaderSelection(false, leaderId);
        menuUi.tempSelectedLeader = leaderId;
        menuUi.tempSelectedRandom = false;
        if (!updateHeroesPreview()) render();
        updateTopbar();
        updateHeroBackdrop(currentScreen());
        if (persistSelection) scheduleLeaderPersist(false, leaderId);
        maybePresentRewards();
        return;
    }
    if (command === "preview_random_leader") {
        MenuSfx.heroBrowse("random", target);
        stageLeaderSelection(true, null);
        menuUi.tempSelectedLeader = null;
        menuUi.tempSelectedRandom = true;
        if (!updateHeroesPreview()) render();
        updateTopbar();
        updateHeroBackdrop(currentScreen());
        scheduleLeaderPersist(true, null);
        return;
    }
    if (command === "confirm_random_leader") {
        MenuSfx.heroEquip();
        flushLeaderPersist();
        menuUi.heroesOpen = false;
        menuUi.heroesSearchQuery = "";
        menuUi.heroesRegionFilter = "all";
        menuUi.tempSelectedLeader = null;
        menuUi.tempSelectedRandom = false;
        render();
        return;
    }
    if (command === "confirm_leader") {
        MenuSfx.heroEquip();
        flushLeaderPersist();
        menuUi.heroesOpen = false;
        menuUi.heroesSearchQuery = "";
        menuUi.heroesRegionFilter = "all";
        menuUi.tempSelectedLeader = null;
        menuUi.tempSelectedRandom = false;
        render();
        return;
    }
    // Portal builds (Poki, Jest) have no store module; their hero cards
    // unlock with earned currency after a second confirming tap.
    if (command === "portal_unlock_leader") {
        const leaderId = target.dataset.leaderId;
        const currency = target.dataset.currency === "gems" ? "gems" : "crowns";
        if (!leaderId || (menuUi.state && menuUi.state.store_busy)) return;
        const key = leaderId + ":" + currency;
        if (menuUi.portalUnlockConfirm !== key) {
            MenuSfx.purchaseOpen();
            menuUi.portalUnlockConfirm = key;
        } else {
            MenuSfx.purchaseConfirm();
            menuUi.portalUnlockConfirm = null;
            menuUi.tempSelectedLeader = leaderId;
            menuUi.tempSelectedRandom = false;
            send("unlock_leader", { leader_id: leaderId, currency: currency });
        }
        render();
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
        if (purchaseSkinId) openSkinPurchase(purchaseSkinId, target.dataset.currency || "gems");
        return;
    }
    if (command === "open_offer_purchase") {
        const purchaseOfferId = target.dataset.offerId;
        if (purchaseOfferId) openOfferPurchase(purchaseOfferId, target.dataset.currency);
        return;
    }
    if (command === "confirm_purchase") {
        if (!menuUi.purchaseModal || menuUi.purchaseModal.submitted) return;
        MenuSfx.purchaseConfirm();
        menuUi.purchaseModal.submitted = true;
        if (menuUi.purchaseModal.type === "leader") {
            if (!(menuUi.state && menuUi.state.store_busy)) {
                if (menuUi.purchaseModal.leaderId) {
                    menuUi.tempSelectedLeader = menuUi.purchaseModal.leaderId;
                    menuUi.tempSelectedRandom = false;
                }
                send("unlock_leader", {
                    leader_id: menuUi.purchaseModal.leaderId,
                    currency: menuUi.purchaseModal.currency,
                    offer_id: menuUi.purchaseModal.offerId || null,
                });
            }
        } else if (menuUi.purchaseModal.type === "skin") {
            if (!(menuUi.state && menuUi.state.store_busy))
                send("unlock_skin", {
                    skin_id: menuUi.purchaseModal.skinId,
                    currency: menuUi.purchaseModal.currency || "gems",
                    offer_id: menuUi.purchaseModal.offerId || null,
                });
        } else if (menuUi.purchaseModal.type === "store_bundle") {
            if (!(menuUi.state && menuUi.state.store_busy))
                send("unlock_bundle", {
                    bundle_id: menuUi.purchaseModal.bundleOfferId,
                    currency: menuUi.purchaseModal.currency || "crowns",
                });
        }
        render();
        return;
    }
    if (command === "cancel_purchase") {
        if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
        if (menuUi.purchaseModal && menuUi.purchaseModal.phase === "success") MenuSfx.confirm();
        else MenuSfx.modalClose();
        if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
        else {
            menuUi.purchaseModal = null;
            menuUi.purchaseIntent = null;
        }
        render();
        return;
    }
    if (command === "unlock_skin") {
        const skinId = target.dataset.skinId;
        if (skinId && !menuUi.state?.store_busy) {
            MenuSfx.purchaseConfirm();
            send("unlock_skin", { skin_id: skinId });
        }
        return;
    }
    /* POKI_SHARED_STORE_UNLOCK_END */
    if (command === "open_skin_picker") {
        MenuSfx.modalOpen();
        menuUi.skinPickerOpen = true;
        loadStoreCatalog();
        render();
        return;
    }
    if (command === "close_skin_picker") {
        MenuSfx.modalClose();
        menuUi.skinPickerOpen = false;
        render();
        return;
    }
    if (command === "equip_skin" || command === "unequip_skin") {
        const skinId = command === "unequip_skin" ? "" : target.dataset.skinId;
        if (skinId !== undefined && !menuUi.state?.store_busy) {
            if (command === "unequip_skin") playUiCue(AudioCue.UiClick, 2);
            else playUiCue(AudioCue.UiHeroEquip);
            send("equip_skin", { skin_id: skinId });
        }
        return;
    }
    if (command === "set_session_mode") {
        MenuSfx.infoTab(target);
        menuUi.createOffline = target.dataset.mode === "offline";
        render();
        return;
    }
    if (command === "set_create_mode") {
        const mode = target.dataset.mode;
        if (!mode) return;
        MenuSfx.filterChip(target);
        const prevMode = menuUi.createDraft?.game_mode || "FFA";
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.game_mode = mode;
        if (mode !== "Coop") {
            menuUi.createDraft.nation_count = 0;
        } else if (prevMode !== "Coop" && !menuUi.createDraft.nation_count) {
            menuUi.createDraft.nation_count = Math.min(Number(menuUi.state?.bot_limits?.nations) || 129, 8);
        }
        render();
        return;
    }
    if (command === "set_create_diff") {
        const difficulty = target.dataset.diff;
        if (!difficulty) return;
        MenuSfx.filterChip(target);
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.bot_difficulty = difficulty;
        render();
        return;
    }
    if (command === "set_create_spawn") {
        MenuSfx.filterChip(target);
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.random_spawn = target.dataset.spawn === "true";
        render();
        return;
    }
    if (command === "set_create_private") {
        MenuSfx.filterChip(target);
        menuUi.createPrivate = target.dataset.private === "true";
        render();
        return;
    }
    if (command === "randomize_seed") {
        MenuSfx.tactileStep();
        if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
        menuUi.createDraft.seed = Math.floor(Math.random() * 9999) + 1;
        render();
        return;
    }
    if (command === "cycle_create_map") {
        MenuSfx.tactileStep();
        cycleCreateMap(Number(target.dataset.dir) || 1);
        render();
        return;
    }
    if (command === "select_create_map") {
        const mapKey = target.dataset.mapKey;
        if (mapKey) {
            MenuSfx.tactileStep();
            selectCreateMap(mapKey);
            render();
        }
        return;
    }
    if (command === "step_create_slider") {
        const field = target.dataset.field;
        const step = Number(target.dataset.step) || 0;
        if (field && step) {
            playUiCue(AudioCue.UiSliderTick, 0, step > 0 ? 0.68 : 0.34);
            stepCreateSlider(field, step);
            render();
        }
        return;
    }
    if (command === "open_missions") {
        playUiCue(AudioCue.UiModalOpen);
        menuUi.missionsModalOpen = true;
        render();
        return;
    }
    if (command === "close_missions") {
        playUiCue(AudioCue.UiModalClose);
        menuUi.missionsModalOpen = false;
        render();
        return;
    }
    if (command === "switch_missions_tab") {
        const tab = target.dataset.missionsTab;
        if (tab === "onboarding" || tab === "daily" || tab === "weekly") {
            if (menuUi.activeMissionsTab !== tab) MenuSfx.infoTab(target);
            menuUi.activeMissionsTab = tab;
            render();
        }
        return;
    }
    if (command === "claim_mission") {
        const missionId = target.dataset.missionId;
        if (missionId) {
            MenuSfx.purchaseConfirm();
            claimMission(missionId, false);
        }
        return;
    }
    if (command === "claim_mission_double") {
        const missionId = target.dataset.missionId;
        if (missionId) {
            MenuSfx.purchaseConfirm();
            claimMission(missionId, true);
        }
        return;
    }
    if (command === "request_swap_mission") {
        const missionId = target.dataset.missionId;
        if (missionId) {
            MenuSfx.filterChip(target);
            requestSwapMission(missionId, target.dataset.ad === "true");
        }
        return;
    }
    if (command === "confirm_swap_mission") {
        const missionId = target.dataset.missionId;
        if (missionId) {
            MenuSfx.tactileStep();
            confirmSwapMission(missionId, target.dataset.ad === "true");
        }
        return;
    }
    if (command === "cancel_swap_mission") {
        MenuSfx.back();
        cancelSwapMission();
        return;
    }
    if (command === "mission_action") {
        MenuSfx.battleSubmenu();
        const missionId = target.dataset.missionId || "";
        const actionTarget = target.dataset.actionTarget || "battle";
        const HERO_REGION_TARGETS: Record<string, string> = {
            heroes_oceania: "oceania",
            heroes_europe: "europe",
            heroes_asia: "asia",
            heroes_americas: "americas",
            heroes_africa: "africa",
        };
        const HERO_RARITY_TARGETS: Record<string, string> = {
            heroes_common: "common",
            heroes_rare: "rare",
            heroes_epic: "epic",
            heroes_legendary: "legendary",
        };
        const CREATE_MODE_TARGETS: Record<string, string> = {
            teams: "Teams",
            coop: "Coop",
            pve: "Coop",
            hvn: "Coop",
            hnv: "Coop",
            ffa: "FFA",
        };
        const closeMissionPanels = function () {
            menuUi.missionsModalOpen = false;
            menuUi.profileOpen = false;
            menuUi.storeOpen = false;
            menuUi.heroesOpen = false;
            menuUi.campaignOpen = false;
            menuUi.settingsOpen = false;
        };
        SOWTooltip.hide(true);

        if (
            missionId === "onboarding_1_tutorial" ||
            actionTarget === "campaign_tactics" ||
            actionTarget === "campaign_advanced_tactics"
        ) {
            closeMissionPanels();
            menuUi.profileAccountId = null;
            menuUi.profileMatchDetail = null;
            menuUi.selectedCampaignSaga =
                actionTarget === "campaign_advanced_tactics" ? "advanced_tactics" : "tactics";
            menuUi.selectedCampaignEpisode = "";
            menuUi.campaignOpen = true;
            render();
            updateHeroBackdrop("campaign");
            return;
        }
        if (actionTarget === "ranked") {
            closeMissionPanels();
            flushLeaderPersist();
            send("quick_match");
            render();
            return;
        }
        if (actionTarget in CREATE_MODE_TARGETS) {
            const createMode = CREATE_MODE_TARGETS[actionTarget];
            const openModeLobby = (menuUi.state?.lobbies || []).find(function (lobby) {
                return (
                    lobby.game_mode === createMode &&
                    !lobby.is_private &&
                    !lobby.has_password &&
                    lobby.kind === "Matchmaking" &&
                    (Number(lobby.num_players) || 0) < (Number(lobby.max_players) || 0)
                );
            }) || (menuUi.state?.lobbies || []).find(function (lobby) {
                return (
                    lobby.game_mode === createMode &&
                    lobby.kind === "Custom" &&
                    !lobby.is_private &&
                    !lobby.has_password &&
                    (Number(lobby.num_players) || 0) < (Number(lobby.max_players) || 0)
                );
            });
            closeMissionPanels();
            flushLeaderPersist();
            if (openModeLobby) {
                send("join_lobby", { lobby_id: openModeLobby.id });
                render();
            } else if (currentScreen() === "create") {
                const prevMode = menuUi.createDraft?.game_mode || "FFA";
                menuUi.createDraft = menuUi.createDraft || cloneConfig();
                menuUi.createDraft.game_mode = createMode;
                if (createMode !== "Coop") {
                    menuUi.createDraft.nation_count = 0;
                } else if (prevMode !== "Coop" && !menuUi.createDraft.nation_count) {
                    menuUi.createDraft.nation_count = Math.min(Number(menuUi.state?.bot_limits?.nations) || 129, 8);
                }
                render();
            } else {
                pendingCreateMode = createMode;
                send("open_create", {});
                render();
            }
            return;
        }
        if (
            actionTarget in HERO_REGION_TARGETS ||
            actionTarget in HERO_RARITY_TARGETS ||
            actionTarget === "heroes_free_rotation" ||
            actionTarget === "heroes_all"
        ) {
            closeMissionPanels();
            menuUi.profileAccountId = null;
            menuUi.profileMatchDetail = null;
            menuUi.heroesOpen = true;
            menuUi.heroesSearchQuery = "";
            menuUi.heroesRegionFilter = HERO_REGION_TARGETS[actionTarget] || "all";
            menuUi.heroesRarityFilter = HERO_RARITY_TARGETS[actionTarget] || "all";
            if (actionTarget === "heroes_free_rotation") {
                menuUi.heroesStatusFilter = "playable";
            } else if (menuUi.heroesStatusFilter === "playable") {
                menuUi.heroesStatusFilter = "all";
            }
            menuUi.dropdownOpenKey = null;
            refreshHeroesRoster(true);
            render();
            updateHeroBackdrop("heroes");
            return;
        }
        closeMissionPanels();
        const scr = currentScreen();
        if (scr === "browser" || scr === "create") {
            send("close_overlay", {});
        }
        flushLeaderPersist();
        send("join_matchmaking", {});
        render();
        return;
    }
    if (command === "toggle_settings") {
        menuUi.settingsOpen = !menuUi.settingsOpen;
        if (menuUi.settingsOpen) MenuSfx.modalOpen();
        else MenuSfx.modalClose();
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
        MenuSfx.modalClose();
        menuUi.authModalOpen = false;
        render();
        return;
    }
    if (command === "auth_change_email") {
        MenuSfx.back();
        menuUi.authOtpSent = false;
        menuUi.authCode = "";
        menuUi.authError = "";
        menuUi.authNotice = "";
        render();
        return;
    }
    if (command === "auth_resend") {
        MenuSfx.click();
        requestAuthOtp();
        return;
    }
    if (command === "identity_provider") {
        MenuSfx.confirm();
        if (typeof window.SOW_startIdentityOAuth === "function") window.SOW_startIdentityOAuth(target.dataset.provider);
        return;
    }
    if (command === "sign_in") {
        MenuSfx.modalOpen();
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
        MenuSfx.modalOpen();
        menuUi.signOutConfirmOpen = true;
        render();
        return;
    }
    if (command === "cancel_sign_out") {
        MenuSfx.modalClose();
        menuUi.signOutConfirmOpen = false;
        render();
        return;
    }
    if (command === "confirm_sign_out") {
        MenuSfx.back();
        menuUi.signOutConfirmOpen = false;
        if (isAndroidTwa()) send("sign_out");
        else if (typeof window.SOW_signOutIdentity === "function") window.SOW_signOutIdentity();
        return;
    }
    /* POKI_SHARED_AUTH_ACTIONS_END */
    if (command === "close_password") {
        MenuSfx.modalClose();
        menuUi.passwordLobbyId = null;
        menuUi.passwordDraft = "";
        render();
        return;
    }
    if (command === "copy_lobby_code") {
        MenuSfx.tactileStep();
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
            MenuSfx.modalOpen();
            menuUi.passwordLobbyId = selectedLobby.id;
            menuUi.passwordDraft = "";
            render();
        } else {
            MenuSfx.battleLaunch();
            send("join_lobby", { lobby_id: Number(target.dataset.lobbyId) });
        }
        return;
    }
    if (command === "set_leader") {
        if (send("set_leader", { leader_id: target.dataset.leaderId })) {
            MenuSfx.heroEquip();
            menuUi.heroesOpen = false;
        }
        return;
    }
    const payload: { lobby_id?: number; target_player_id?: number } = {};
    if (target.dataset.lobbyId) payload.lobby_id = Number(target.dataset.lobbyId);
    if (target.dataset.playerId) payload.target_player_id = Number(target.dataset.playerId);
    if (command === "open_browser" || command === "open_create") {
        MenuSfx.battleSubmenu();
    } else if (
        command === "quick_match" ||
        command === "join_matchmaking" ||
        command === "start_private" ||
        command === "create_game"
    ) {
        MenuSfx.battleLaunch();
    } else if (
        command.startsWith("close_") ||
        command === "leave_lobby" ||
        command === "cancel_sign_out"
    ) {
        MenuSfx.back();
    } else {
        MenuSfx.click();
    }
    send(command, payload);
});

function playCelebrationSound(kind: "fanfare" | "fly" | "impact") {
    if (kind === "fanfare") {
        playUiCue(AudioCue.UiCelebrationFanfare);
    } else if (kind === "fly") {
        playUiCue(AudioCue.UiCelebrationFly);
    } else if (kind === "impact") {
        playUiCue(AudioCue.UiCelebrationImpact);
    }
}

export interface MissionCelebrationOptions {
    missionId: string;
    title: string;
    description?: string;
    category: string;
    icon: string;
    crowns: number;
    gems: number;
    doubledWithAd: boolean;
}

function attachMissionBanner(stageEl: HTMLElement, options: MissionCelebrationOptions) {
    const cards = stageEl.querySelector(".sow-menu__reward-cards");
    const banner = document.createElement("div");
    banner.className = "sow-menu__mission-banner";
    const categoryLabel =
        options.category === "onboarding"
            ? SOW_t("missions.tab_onboarding")
            : options.category === "weekly"
              ? SOW_t("missions.tab_weekly")
              : SOW_t("missions.tab_daily");
    banner.innerHTML =
        "<div class='sow-menu__mission-banner-aura' aria-hidden='true'></div>" +
        "<div class='sow-menu__mission-shockwave' aria-hidden='true'></div>" +
        "<div class='sow-menu__mission-badge'>" +
        "<div class='sow-menu__mission-badge-rays' aria-hidden='true'></div>" +
        "<span class='sow-menu__mission-icon' aria-hidden='true'>" +
        missionIconSvg(options.icon) +
        "</span>" +
        "</div>" +
        "<div class='sow-menu__mission-kicker-wrap'>" +
        "<span class='sow-menu__mission-kicker-wing' aria-hidden='true'></span>" +
        "<div class='sow-menu__mission-kicker'>" +
        esc(categoryLabel) +
        " · " +
        esc(SOW_t("missions.mission_completed") || "Misión Cumplida") +
        "</div>" +
        "<span class='sow-menu__mission-kicker-wing' aria-hidden='true'></span>" +
        "</div>" +
        "<div class='sow-menu__mission-title-box'>" +
        "<h2 class='sow-menu__mission-title'>" +
        esc(options.title) +
        "</h2>" +
        "<div class='sow-menu__mission-title-sheen' aria-hidden='true'></div>" +
        "</div>" +
        (options.description
            ? "<p class='sow-menu__mission-desc'>" + esc(options.description) + "</p>"
            : "") +
        (options.doubledWithAd
            ? "<div class='sow-menu__mission-doubled-badge'>" +
              videoPlayIconSvg() +
              " <span>" +
              esc(SOW_t("missions.claim_doubled")) +
              "</span></div>"
            : "");
    if (cards) {
        stageEl.insertBefore(banner, cards);
    } else {
        stageEl.appendChild(banner);
    }
}

export function runMissionRewardCelebration(options: MissionCelebrationOptions) {
    if (rewardAnimationRunning) return;
    rewardAnimationRunning = true;
    const token = ++rewardAnimationToken;

    if (menuUi.state) {
        menuUi.state.crowns = (menuUi.state.crowns || 0) + options.crowns;
        menuUi.state.gems = (menuUi.state.gems || 0) + options.gems;
    }
    render();

    const stages: RewardStage[] = [];
    if (options.crowns > 0) {
        stages.push({
            kind: "crowns",
            amount: options.crowns,
            selector: "[data-progression-crowns-value]",
        });
    }
    if (options.gems > 0) {
        stages.push({
            kind: "gems",
            amount: options.gems,
            selector: "[data-progression-gems-value]",
        });
    }

    const canonical = progressionFromState();
    const baseline = Object.assign({}, canonical, {
        crowns: Math.max(0, canonical.crowns - options.crowns),
        gems: Math.max(0, canonical.gems - options.gems),
    });
    const startingProg = displayedProgression || baseline;
    writeProgression(startingProg);

    function finish(_shown: boolean) {
        if (token !== rewardAnimationToken) return;
        rewardAnimationToken += 1;
        rewardAnimationRunning = false;
        removeRewardStage();
        writeProgression(progressionFromState());
        render();
    }

    if (reducedRewardMotion()) {
        if (!stages.length) {
            finish(true);
            return;
        }
        const items = createRewardStage(
            stages,
            function () {
                finish(true);
            },
            { useWaapiReveal: false },
        );
        if (activeRewardStage) {
            activeRewardStage.classList.add("sow-menu__reward-stage--mission");
            attachMissionBanner(activeRewardStage, options);
        }
        playCelebrationSound("fanfare");
        items.forEach(function (item) {
            item.amount.textContent = formatRewardCardText(item.stage, 1);
        });
        return;
    }

    runChoreographedStage({
        token,
        stages,
        startValues: startingProg,
        targetValues: canonical,
        stageClassName: "sow-menu__reward-stage--mission",
        impactPower: 1.2,
        hasBanner: true,
        hasAudio: true,
        beforeStart: function (stageEl) {
            attachMissionBanner(stageEl, options);
        },
        onComplete: finish,
    });
}

setMissionClaimHandler(function (mission: MenuMission, doubled: boolean) {
    const multiplier = doubled ? 2 : 1;
    runMissionRewardCelebration({
        missionId: mission.id,
        title: SOW_t(mission.title_key),
        description: SOW_t(mission.description_key),
        category: mission.category,
        icon: mission.icon,
        crowns: mission.crowns_reward * multiplier,
        gems: mission.gems_reward * multiplier,
        doubledWithAd: doubled,
    });
});

document.addEventListener("pointerdown", function (event) {
    const target = event.target instanceof Element ? event.target : null;
    if (
        target instanceof HTMLInputElement &&
        target.type === "range" &&
        (target.dataset.setting === "music_volume" || target.dataset.setting === "sfx_volume")
    ) {
        isDraggingSettingsSlider = true;
    }
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
    isDraggingSettingsSlider = false;
    renderFlushTimer = setTimeout(function () {
        renderFlushTimer = null;
        activeMenuPointerId = null;
        if (renderPending) render();
    }, 0);
}

document.addEventListener("pointerup", function (event) {
    isDraggingSettingsSlider = false;
    finishMenuPointer(event);
});
document.addEventListener("pointercancel", function (event) {
    isDraggingSettingsSlider = false;
    finishMenuPointer(event);
});

/* POKI_SHARED_AUTH_EVENTS_BEGIN */
window.addEventListener("identity:auth-state-change", function () {
    if (!menuUi.state || root.hidden) return;
    const auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() || {} : {};
    const reauthenticationRequired = window.SOW_IDENTITY_SESSION_STATUS === "reauthentication_required";
    if (auth.linked && !reauthenticationRequired && menuUi.authModalOpen) {
        menuUi.authModalOpen = false;
        resetAuthFlow();
    }
    if (!root.querySelector("[data-screen-stage]")) {
        render();
        return;
    }
    updateTopbar();
    updateFrameChrome(currentScreen());
    updateDynamic();
    if (menuUi.settingsOpen) syncOverlay("settings", renderSettings());
    if (menuUi.authModalOpen) syncOverlay("auth", renderAuthModal());
    syncProfileDom();
});

window.addEventListener("identity:auth-error", function (event) {
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
        MenuSfx.modalClose();
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
        MenuSfx.modalClose();
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
        MenuSfx.modalClose();
        menuUi.skinPickerOpen = false;
        menuUi.heroInfoOpen = false;
        menuUi.heroInfoLeaderId = null;
        render();
        return;
    }
    if (event.key === "Escape" && menuUi.profileMatchDetail) {
        MenuSfx.modalClose();
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
        MenuSfx.battleLaunch();
        const code = (form.elements.namedItem("code") as HTMLInputElement).value.trim();
        send("join_code", { code: code });
    }
    if (form.dataset.form === "password") {
        event.preventDefault();
        menuUi.passwordDraft = (form.elements.namedItem("password") as HTMLInputElement).value;
        if (menuUi.passwordLobbyId != null && menuUi.passwordDraft) {
            MenuSfx.battleLaunch();
            send("join_with_password", { lobby_id: menuUi.passwordLobbyId, password: menuUi.passwordDraft });
        }
    }
    if (form.dataset.form === "create") {
        event.preventDefault();
        MenuSfx.battleLaunch();
        syncCreateDraft(form);
        const config = menuUi.createDraft || cloneConfig();
        if (config.game_mode !== "Coop") {
            config.nation_count = 0;
        }
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
    if (input.dataset.role === "achievements-search") {
        menuUi.profileAchievementSearch = input.value;
        resetAchievementFlatLimit();
        refreshProfileAchievements();
    }
    const createForm = input.closest("form[data-form='create']");
    if (createForm instanceof HTMLFormElement) {
        syncCreateDraft(createForm);
        const valBadge = createForm.querySelector<HTMLElement>("[data-val-for='" + input.name + "']");
        if (valBadge) valBadge.textContent = input.value;
        if (input instanceof HTMLInputElement && input.type === "range") {
            const min = Number(input.min) || 0;
            const max = Number(input.max) || 100;
            const cur = Number(input.value) || 0;
            const pct =
                max > min ? Math.max(0, Math.min(100, Math.round(((cur - min) / (max - min)) * 100))) : 0;
            input.style.setProperty("--sow-slider-fill", pct + "%");
            const bucket = Math.round(pct / 5);
            if (input.dataset.lastTickBucket !== String(bucket)) {
                input.dataset.lastTickBucket = String(bucket);
                playUiCue(AudioCue.UiSliderTick, 0, pct / 100);
            }
        }
        if (menuUi.createDraft) {
            const totalBadge = createForm.querySelector<HTMLElement>("[data-create-total-forces] b");
            if (totalBadge) {
                const mp = Number(menuUi.createDraft.max_players) || 8;
                const bc = menuUi.createDraft.bot_count != null ? Number(menuUi.createDraft.bot_count) : 128;
                const nc =
                    menuUi.createDraft.game_mode === "Coop"
                        ? menuUi.createDraft.nation_count != null
                            ? Number(menuUi.createDraft.nation_count)
                            : 0
                        : 0;
                totalBadge.textContent = String(mp + bc + nc);
            }
        }
    }
    if (input.dataset.setting === "music_volume" || input.dataset.setting === "sfx_volume") {
        const setting = input.dataset.setting;
        const val = Number(input.value);
        const pct = Math.round(val * 100);
        input.style.setProperty("--sow-slider-fill", pct + "%");
        input.style.setProperty("--sow-music-fill", pct + "%");
        const valBadge = root.querySelector<HTMLElement>(
            "[data-val-for='" + (setting === "sfx_volume" ? "sfx_vol" : "music_vol") + "']",
        );
        if (valBadge) valBadge.textContent = pct + "%";
        if (menuUi.state) {
            if (!menuUi.state.settings) menuUi.state.settings = {} as any;
            menuUi.state.settings[setting] = val;
        }
        if (setting === "music_volume") {
            syncAudioSettings({ music_volume: val });
            if (menuUi.state && menuUi.state.phase === "MainMenu") syncMenuMusic(menuUi.state, true, val);
            menuMusicSyncSpectrum();
        } else if (setting === "sfx_volume") {
            syncAudioSettings({ sfx_volume: val });
            const bucket = Math.round(pct / 5);
            if (input.dataset.lastTickBucket !== String(bucket)) {
                input.dataset.lastTickBucket = String(bucket);
                playUiCue(AudioCue.UiSliderTick, 0, val);
            }
        }
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
    if (
        input.name === "ach_category" ||
        input.name === "ach_status" ||
        input.name === "ach_tier" ||
        input.name === "ach_sort"
    ) {
        resetAchievementFlatLimit();
        refreshProfileAchievements();
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
    if (input.dataset.setting === "music_volume" || input.dataset.setting === "sfx_volume") {
        const setting = input.dataset.setting;
        isDraggingSettingsSlider = false;
        const val = Number(input.value);
        if (menuUi.state) {
            if (!menuUi.state.settings) menuUi.state.settings = {} as any;
            menuUi.state.settings[setting] = val;
        }
        send(setting === "sfx_volume" ? "set_sfx_volume" : "set_music_volume", { value: val });
    }
    if (input instanceof HTMLInputElement && input.type === "checkbox" && input.dataset.setting) {
        MenuSfx.toggleSwitch(input.checked);
    }
    if (input instanceof HTMLInputElement && input.dataset.setting === "free_zoom_out")
        send("set_free_zoom_out", { value: input.checked });
    if (input instanceof HTMLInputElement && input.dataset.setting === "show_match_clock")
        send("set_show_match_clock", { value: input.checked });
    if (input instanceof HTMLInputElement && input.dataset.setting === "show_fps_ping")
        send("set_show_fps_ping", { value: input.checked });
    if (input instanceof HTMLInputElement && input.dataset.setting === "show_dev_tools")
        send("set_show_dev_tools", { value: input.checked });
    if (input instanceof HTMLInputElement && input.dataset.setting === "menu_tips") {
        const disabled = !input.checked;
        if (menuUi.state) menuUi.state.menu_tips_disabled = disabled;
        send("save_menu_guide_progress", {
            guide_id: "settings",
            completed_steps: [],
            current_step: null,
            status: "active",
            menu_tips_disabled: disabled,
        });
        if (!disabled && window.SOW_menu_guide && menuUi.state) {
            window.SOW_menu_guide.update(menuUi.state, menuGuideFacts());
        }
    }
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
    if (typeof window.SOW_syncWebLoader === "function")
        window.SOW_syncWebLoader(
            nextState.loader_job === "EnterGame" && nextState.ranked_matchup
                ? Object.assign({}, nextState, {
                      ranked_matchup_html: renderRankedDuel(nextState.ranked_matchup, true),
                  })
                : nextState,
        );
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
    if (document.visibilityState !== "visible") {
        menuMusicCancelSpectrumFrame();
        flushLeaderPersist();
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
    menuMusicSyncSpectrum();
    if (typeof resumeExternalPurchaseDelivery === "function") resumeExternalPurchaseDelivery(true);
    if (!rewardPresentationReady) scheduleRewardPresentation(0);
    else maybePresentRewards();
});

function handleMenuStateUpdate(raw: string, parsed: MenuState) {
    if (typeof raw !== "string" || raw === menuUi.lastRaw) {
        updateDynamic();
        return;
    }
    const hadRankedMatchup = Boolean(menuUi.state?.ranked_matchup);
    menuUi.lastRaw = raw;
    menuUi.state = parsed;
    if (menuUi.state?.settings) {
        syncAudioSettings(menuUi.state.settings);
    }
    if (!hadRankedMatchup && menuUi.state?.ranked_matchup) {
        playUiCue(AudioCue.UiMatchFound);
    }
    if (
        !Number.isSafeInteger(menuUi.state!.loader_cycle_id) ||
        menuUi.state!.loader_cycle_id < latestLoaderCycleId
    )
        return;
    latestLoaderCycleId = menuUi.state!.loader_cycle_id;
    menuUi.state = normalizeCompletedExitState(menuUi.state);
    reconcileLeaderSelection(menuUi.state);
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
    if (menuUi.state!.phase === "MainMenu" && menuUi.state!.profile_ready) loadStoreCatalog();
    if (menuUi.state!.waiting && menuUi.passwordLobbyId != null) {
        menuUi.passwordLobbyId = null;
        menuUi.passwordDraft = "";
    }
    const key = renderKey();
    if (key !== menuUi.lastRenderKey) {
        render(key);
    } else {
        syncMenuSkinTheme(menuUi.state);
        root.hidden = menuUi.state!.phase !== "MainMenu";
        syncBladeCanvasVisibility();
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
