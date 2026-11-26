import { MenuSfx } from "./audio/index.ts";
import { avatarImageAttribute, setAvatarImage } from "./avatar-presentation.ts";
import {
    WOW_PILLAR_ORDER,
    WOW_SUBCATEGORIES,
    enrichAchievement,
    enrichAchievementsList,
    getAchievementDefinition,
    getFullAchievementsCatalog,
    resolveProfileAchievements,
    setAchievementRuntimeProvider,
    wowCategoryTitle,
    type EnrichedAchievement,
    type ProfileAchievement,
    type WowCategoryId,
} from "./main_menu.achievements.ts";
import {
    asset,
    avatarImage,
    currencyAsset,
    currentScreen,
    esc,
    leaderArtSlug,
    leaderById,
    leaderDisplayName,
    menuUi,
    profileCache,
    profileHeroImage,
    root,
    selfCreds,
} from "./main_menu.core.ts";
import {
    latestMatchGoalSummary,
    markMatchGoalsViewed,
    matchGoalChainMeta,
} from "./main_menu.match-goals.ts";
import {
    rankEmblem,
    rankLabel as rankLabelText,
    rankOverview,
} from "./main_menu.ranked.ts";
import {
    REPORT_REASONS,
    activeScreenPanel,
    render,
    reportBusy,
} from "./main_menu.shell.ts";
import type { MenuState, RankedLadder } from "./main_menu.state.ts";
import {
    createLeaderboardController,
    type LeaderboardController,
} from "./sow-leaderboard-view.ts";
import { bindPointerSpotlight } from "./sow-profile-helpers.ts";
import {
    achPillarCounts,
    defaultAchievementCodexState,
    filterMatchesByQueue,
    normalizeWowCategory,
    profileStat,
    renderAchRailMarkup,
    renderAchResultsMarkup,
    renderAchSubbarMarkup,
    renderHistoryQueueFilterPills,
    renderProfileAchievementsCodex,
    renderProfileHeader,
    renderProfileLeaderCard,
    renderProfileMatchDetailModal,
    renderProfileMatchRow,
    renderProfileStatsRibbon,
    type AchievementCodexViewState,
    type ProfileData,
    type ProfileMatch,
    type ProfileMatchDetail,
    type ProfileRating,
} from "./sow-profile-view.ts";

export { getFullAchievementsCatalog, resolveProfileAchievements };
export type {
    ProfileAchievement,
    ProfileData,
    ProfileMatch,
    ProfileMatchDetail,
    ProfileRating,
};

// Inject live game state into the shared Achievements engine without coupling public pages to menuUi.
setAchievementRuntimeProvider(function () {
    return menuUi.state;
});

export interface ProfileCacheEntry {
    data: ProfileData | null;
    complete: boolean;
    stale: boolean;
    error: string;
    request: Promise<ProfileData | null> | null;
    history: ProfileMatch[];
    historyCursor: number;
    historyLoaded: boolean;
    historyHasMore: boolean;
    historyError: string;
    historyRequest: Promise<void> | null;
    details: Record<string, ProfileMatchDetail>;
    detailRequests: Record<string, Promise<void> | undefined>;
}

interface ProfilePage<Item> {
    items: Item[];
    next_cursor: number | null;
}

type RankedQueue = "Ranked1v1" | "RankedFFA";

let gameLbController: LeaderboardController | null = null;
let gameLbHost: HTMLElement | null = null;
let achFlatLimit = 90;

export function destroyGameLeaderboard(): void {
    if (gameLbController) {
        gameLbController.destroy();
        gameLbController = null;
    }
    gameLbHost = null;
}

export function mountGameProfileInteractive(panel?: HTMLElement | null): void {
    const scope = panel || activeScreenPanel();
    if (!scope || !menuUi.profileOpen || currentScreen() !== "profile") {
        destroyGameLeaderboard();
        return;
    }
    bindPointerSpotlight(scope.querySelector<HTMLElement>("[data-ranked-overview]"));
    if (menuUi.profileTab === "victories") {
        const mount = scope.querySelector<HTMLElement>("[data-game-lb-mount]");
        if (mount && mount !== gameLbHost) {
            destroyGameLeaderboard();
            gameLbController = createLeaderboardController(mount, {
                mode: "game",
                initialBoard: menuUi.profileRankedQueue,
                syncUrl: false,
                selfAccountId: menuUi.state?.account_id || null,
                onSfx(kind) {
                    if (kind === "tab") MenuSfx.infoTab(mount);
                    else if (kind === "filter") MenuSfx.filterChip(mount);
                    else MenuSfx.click();
                },
            });
            gameLbHost = mount;
        }
    } else {
        destroyGameLeaderboard();
    }
}

export function profileApi(path: string) {
    const base = String(window.SOW_DATABASE_URL || "/api").replace(/\/$/, "");
    return base + path;
}

function profileCacheEntry(id: string | null): ProfileCacheEntry | null {
    if (!id) return null;
    if (!profileCache[id]) {
        profileCache[id] = {
            data: null,
            complete: false,
            stale: false,
            error: "",
            request: null,
            history: [],
            historyCursor: 0,
            historyLoaded: false,
            historyHasMore: false,
            historyError: "",
            historyRequest: null,
            details: Object.create(null),
            detailRequests: Object.create(null),
        };
    }
    return profileCache[id];
}

function profileSeedFromState(id: string): ProfileData | null {
    if (!menuUi.state || menuUi.state.account_id !== id) return null;
    const stats = menuUi.state.profile_stats || {};
    const matches = profileStat(stats.matches_played);
    const wins = profileStat(stats.wins);
    return {
        account_id: id,
        handle: menuUi.state.player_name || "",
        display_name: menuUi.state.player_name || SOW_t("menu.anonymous"),
        level: profileStat(menuUi.state.level) || 1,
        matches_played: matches,
        wins: wins,
        win_rate: matches ? wins / matches : 0,
        kills: profileStat(stats.kills),
        deaths: profileStat(stats.deaths),
        assists: profileStat(stats.assists),
        laurels: profileStat(menuUi.state.laurels),
        players_defeated: profileStat(stats.players_defeated),
        empires_defeated: profileStat(stats.empires_defeated),
        tribes_defeated: profileStat(stats.tribes_defeated),
        preferred_leader: menuUi.state.selected_leader || null,
        leaders: [],
        recent_matches: [],
        ratings: [],
    };
}

function cacheProfileSeed(id: string, seed: ProfileData | null) {
    const entry = profileCacheEntry(id);
    if (!entry || !seed || entry.complete) return entry;
    entry.data = Object.assign({}, entry.data || {}, seed);
    return entry;
}

function activateProfileEntry(id: string) {
    const entry = profileCacheEntry(id);
    if (!entry) return null;
    menuUi.profileData = entry.data;
    menuUi.profileHistory = entry.history;
    return entry;
}

export function activeProfileEntry() {
    return profileCacheEntry(menuUi.profileAccountId);
}

function profileDataComplete() {
    const entry = activeProfileEntry();
    return !!(entry && entry.complete);
}

function profileSnapshotError() {
    const entry = activeProfileEntry();
    return entry ? entry.error : "";
}

function applyProfileSnapshot(id: string, data: ProfileData) {
    const entry = profileCacheEntry(id);
    if (!entry) return;
    const recent = Array.isArray(data.recent_matches)
        ? data.recent_matches.slice()
        : [];
    entry.data = data;
    entry.complete = true;
    entry.stale = false;
    entry.error = "";
    entry.history = recent;
    entry.historyCursor = recent.length;
    entry.historyLoaded = true;
    entry.historyHasMore = profileStat(data.matches_played) > recent.length;
    entry.historyError = "";
    if (menuUi.profileOwnId === id) {
        menuUi.rankedLadder = data.ranked_ladder || null;
        menuUi.rankedRating =
            (data.ratings || []).find(function (rating) {
                return rating.queue === "Ranked1v1" && rating.mode === "FFA";
            }) || null;
        menuUi.rankedFfaRating =
            (data.ratings || []).find(function (rating) {
                return rating.queue === "RankedFFA" && rating.mode === "FFA";
            }) || null;
        const rankLabel = root.querySelector<HTMLElement>("[data-ranked-current]");
        if (rankLabel) {
            const rating = menuUi.rankedRating;
            rankLabel.textContent = rankLabelText(rating);
            const emblem = root.querySelector<HTMLElement>("[data-ranked-emblem]");
            if (emblem)
                emblem.innerHTML = rankEmblem(
                    rating?.tier || "Nomad",
                    rating?.division || null,
                );
        }
        const ffaLabel = root.querySelector<HTMLElement>("[data-ranked-ffa-current]");
        if (ffaLabel) ffaLabel.textContent = rankLabelText(menuUi.rankedFfaRating);
        const ffaEmblem = root.querySelector<HTMLElement>("[data-ranked-ffa-emblem]");
        if (ffaEmblem) {
            const ffaRating = menuUi.rankedFfaRating;
            ffaEmblem.innerHTML = rankEmblem(
                ffaRating?.tier || "Nomad",
                ffaRating?.division || null,
            );
        }
    }
    if (menuUi.profileAccountId === id) {
        const r1 = (data.ratings || []).find((r) => r.queue === "Ranked1v1");
        const rFfa = (data.ratings || []).find((r) => r.queue === "RankedFFA");
        if ((!r1 || r1.games_played === 0) && rFfa && rFfa.games_played > 0) {
            menuUi.profileOverviewQueue = "RankedFFA";
        }
        activateProfileEntry(id);
    }
}

export function loadProfile(id: string | null): Promise<ProfileData | null> {
    const entry = profileCacheEntry(id);
    if (!id || !entry) return Promise.resolve(null);
    if (entry.complete && !entry.stale) {
        if (menuUi.profileAccountId === id) activateProfileEntry(id);
        return Promise.resolve(entry.data);
    }
    if (entry.request) {
        if (menuUi.profileAccountId === id) menuUi.profileSnapshotLoading = true;
        return entry.request;
    }
    if (menuUi.profileAccountId === id) {
        menuUi.profileSnapshotLoading = true;
    }
    entry.request = fetch(profileApi("/profiles/" + encodeURIComponent(id)), {
        headers: { Accept: "application/json" },
    })
        .then(function (response) {
            if (!response.ok) throw new Error("profile request failed");
            return response.json();
        })
        .then(function (data: ProfileData | null) {
            if (!data || typeof data !== "object")
                throw new Error("invalid profile response");
            applyProfileSnapshot(id, data);
            return data;
        })
        .catch(function () {
            entry.error = SOW_t("profile.profile_unavailable");
            return null;
        })
        .finally(function () {
            entry.request = null;
            if (menuUi.profileOpen && menuUi.profileAccountId === id) {
                menuUi.profileSnapshotLoading = false;
                activateProfileEntry(id);
                syncProfileDom();
            }
        });
    return entry.request;
}

export function loadMoreProfileHistory() {
    const id = menuUi.profileAccountId;
    const entry = activeProfileEntry();
    if (!id || !entry || menuUi.profileHistoryLoading || entry.historyRequest)
        return;
    if (!entry.complete || entry.stale) {
        menuUi.profileHistoryLoading = true;
        loadProfile(id).then(function (data) {
            if (menuUi.profileOpen && menuUi.profileAccountId === id) {
                menuUi.profileHistoryLoading = false;
                if (data) loadMoreProfileHistory();
                else syncProfileDom();
            }
        });
        return;
    }
    if (!entry.historyHasMore) return;
    menuUi.profileHistoryLoading = true;
    entry.historyError = "";
    entry.historyRequest = fetch(
        profileApi(
            "/profiles/" +
                encodeURIComponent(id) +
                "/matches?cursor=" +
                entry.historyCursor +
                "&limit=20",
        ),
        {
            headers: { Accept: "application/json" },
        },
    )
        .then(function (response) {
            if (!response.ok) throw new Error("history request failed");
            return response.json();
        })
        .then(function (data: ProfilePage<ProfileMatch>) {
            const items = Array.isArray(data.items) ? data.items : [];
            entry.history = entry.history.concat(items);
            entry.historyCursor =
                data.next_cursor == null
                    ? entry.historyCursor + items.length
                    : Number(data.next_cursor);
            entry.historyLoaded = true;
            entry.historyHasMore = data.next_cursor != null && items.length > 0;
            if (menuUi.profileAccountId === id) activateProfileEntry(id);
        })
        .catch(function () {
            entry.historyError = SOW_t("profile.match_history_unavailable");
        })
        .finally(function () {
            entry.historyRequest = null;
            if (menuUi.profileOpen && menuUi.profileAccountId === id) {
                menuUi.profileHistoryLoading = false;
                syncProfileDom();
            }
        });
}

export function loadVictoryLeaderboard(_append?: boolean) {
    if (gameLbController) {
        gameLbController.setFilters({ board: "victories" });
    }
}

export function loadRankedLeaderboard(
    retry = false,
    queue: RankedQueue = menuUi.profileRankedQueue,
) {
    menuUi.profileRankedQueue = queue;
    if (gameLbController) {
        if (retry) gameLbController.refresh();
        else if (gameLbController.getFilters().board !== queue) {
            gameLbController.setFilters({ board: queue });
        }
    }
}

export function invalidateProfileCache(id: string | null) {
    const entry = profileCacheEntry(id);
    if (!entry) return;
    entry.stale = true;
    entry.error = "";
    entry.historyLoaded = false;
    entry.historyError = "";
}

export function syncProfilePreload(nextState: MenuState) {
    const nextId =
        nextState && nextState.account_id ? nextState.account_id : null;
    const previousId = menuUi.profileOwnId;
    const returnedToMenu =
        menuUi.profileLastPhase &&
        menuUi.profileLastPhase !== "MainMenu" &&
        nextState.phase === "MainMenu";
    if (
        previousId &&
        previousId !== nextId &&
        menuUi.profileOpen &&
        menuUi.profileAccountId === previousId
    ) {
        menuUi.profileOpen = false;
        menuUi.profileAccountId = null;
        menuUi.profileData = null;
        menuUi.profileHistory = [];
        menuUi.profileMatchDetail = null;
        menuUi.profileDetailError = "";
        menuUi.profileDetailId = null;
        menuUi.profileDetailLoading = false;
        menuUi.profileDetailRequestKey = null;
        destroyGameLeaderboard();
    }
    if (previousId && previousId !== nextId) invalidateProfileCache(previousId);
    menuUi.profileOwnId = nextId;
    if (previousId !== nextId) {
        menuUi.rankedRating = null;
        menuUi.rankedFfaRating = null;
        menuUi.rankedLadder = null;
    }
    menuUi.profileLastPhase = nextState.phase;
    if (!nextId) return;
    const entry = cacheProfileSeed(nextId, profileSeedFromState(nextId));
    if (returnedToMenu) {
        invalidateProfileCache(nextId);
        if (gameLbController) gameLbController.refresh();
    }
    if (entry && !entry.complete) {
        if (menuUi.profileAccountId === nextId) activateProfileEntry(nextId);
    }
    if (
        previousId !== nextId ||
        returnedToMenu ||
        (entry && !entry.complete && !entry.request && !entry.error)
    ) {
        loadProfile(nextId);
    }
}

export function openProfile(id: string | null, isNavigation = false) {
    const targetId = id || (menuUi.state && menuUi.state.account_id);
    if (!targetId) return;
    if (
        menuUi.profileOpen &&
        menuUi.profileAccountId &&
        menuUi.profileAccountId !== targetId &&
        !isNavigation
    ) {
        if (!menuUi.profileNavStack) menuUi.profileNavStack = [];
        menuUi.profileNavStack.push({
            accountId: menuUi.profileAccountId,
            tab: menuUi.profileTab || "overview",
        });
    }
    if (!id && !isNavigation) {
        menuUi.profileNavStack = [];
    }
    menuUi.reportOpen = false;
    menuUi.reportTarget = null;
    menuUi.reportSent = false;
    menuUi.deleteArmed = false;
    menuUi.profileOverviewQueue = "Ranked1v1";
    menuUi.profileHistoryQueueFilter = "all";
    menuUi.profileShareCopied = false;
    destroyGameLeaderboard();
    try {
        if (
            typeof window.SOW_isBlockedId === "function" &&
            window.SOW_isBlockedId(targetId)
        ) {
            menuUi.profileOpen = true;
            menuUi.profileAccountId = targetId;
            menuUi.profileTab = "overview";
            const blockedEntry = profileCacheEntry(targetId)!;
            blockedEntry.data = null;
            blockedEntry.complete = false;
            blockedEntry.stale = false;
            blockedEntry.error = SOW_t("profile.blocked_player");
            menuUi.profileData = null;
            menuUi.profileHistory = blockedEntry.history = [];
            blockedEntry.historyCursor = 0;
            blockedEntry.historyLoaded = true;
            blockedEntry.historyHasMore = false;
            menuUi.profileMatchDetail = null;
            menuUi.profileDetailError = "";
            menuUi.profileDetailId = null;
            menuUi.profileSnapshotLoading = false;
            menuUi.profileHistoryLoading = false;
            menuUi.profileDetailLoading = false;
            menuUi.profileDetailRequestKey = null;
            render();
            return;
        }
    } catch {}
    menuUi.profileOpen = true;
    menuUi.profileAccountId = targetId;
    menuUi.profileTab = "overview";
    if (!menuUi.state || menuUi.state.account_id !== targetId) {
        menuUi.profileAchievementPlayableOnly = false;
    }
    menuUi.profileSnapshotLoading = false;
    menuUi.profileHistoryLoading = false;
    menuUi.profileDetailLoading = false;
    menuUi.profileMatchDetail = null;
    menuUi.profileDetailError = "";
    menuUi.profileDetailId = null;
    menuUi.profileDetailRequestKey = null;
    if (menuUi.state && menuUi.state.account_id === targetId)
        invalidateProfileCache(targetId);
    cacheProfileSeed(targetId, profileSeedFromState(targetId));
    activateProfileEntry(targetId);
    loadProfile(targetId);
    render();
}

export function popProfileNav(): boolean {
    if (menuUi.profileNavStack && menuUi.profileNavStack.length > 0) {
        const previous = menuUi.profileNavStack.pop()!;
        openProfile(previous.accountId, true);
        menuUi.profileTab = previous.tab || "overview";
        return true;
    }
    const ownId =
        menuUi.profileOwnId || (menuUi.state && menuUi.state.account_id);
    if (menuUi.profileAccountId && ownId && menuUi.profileAccountId !== ownId) {
        openProfile(ownId, true);
        return true;
    }
    return false;
}

function isBlockedAccountId(accountId: string) {
    try {
        return (
            typeof window.SOW_isBlockedId === "function" &&
            window.SOW_isBlockedId(accountId)
        );
    } catch {
        return false;
    }
}

export function resetAchievementFlatLimit() {
    achFlatLimit = 90;
}

export function bumpAchievementFlatLimit() {
    achFlatLimit += 90;
}

function currentAchViewState(): AchievementCodexViewState {
    return {
        category: normalizeWowCategory(menuUi.profileAchievementFilter),
        subcategory: menuUi.profileAchievementSubcategory || "all",
        status: menuUi.profileAchievementStatus || "all",
        tier: menuUi.profileAchievementTier || "all",
        sort: menuUi.profileAchievementSort || "smart",
        viewMode: menuUi.profileAchievementViewMode === "grid" ? "grid" : "chains",
        search: menuUi.profileAchievementSearch || "",
        playableOnly: Boolean(menuUi.profileAchievementPlayableOnly),
        flatLimit: achFlatLimit,
        selectedTiers: menuUi.profileAchievementSelectedTiers || Object.create(null),
    };
}

export function refreshProfileAchievements() {
    if (!menuUi.profileOpen || currentScreen() !== "profile") return;
    if (menuUi.profileTab !== "achievements") return;
    const data = menuUi.profileData;
    if (!data || !profileDataComplete()) return;
    const panel = activeScreenPanel();
    if (!panel) return;

    const isOwnProfile = Boolean(menuUi.state && data.account_id === menuUi.state.account_id);
    const raw = resolveProfileAchievements(data);
    const enriched = enrichAchievementsList(raw, isOwnProfile);
    const state = currentAchViewState();
    const playableOnly = state.playableOnly && isOwnProfile;

    const rail = panel.querySelector("[data-ach-rail]");
    if (rail) {
        rail.innerHTML = renderAchRailMarkup(
            enriched,
            state.category,
            state.subcategory,
            isOwnProfile,
            playableOnly,
        );
    }
    const subbar = panel.querySelector("[data-ach-subbar]");
    if (subbar) {
        const html = renderAchSubbarMarkup(
            enriched,
            state.category,
            state.subcategory,
            isOwnProfile,
            playableOnly,
        );
        subbar.innerHTML = html;
        (subbar as HTMLElement).hidden = !html;
    }
    const playableBtn = panel.querySelector<HTMLButtonElement>(
        "[data-command='profile_achievement_toggle_playable']",
    );
    if (playableBtn) {
        playableBtn.classList.toggle("is-active", playableOnly);
        playableBtn.setAttribute("aria-pressed", String(playableOnly));
    }
    const modeBtns = panel.querySelectorAll<HTMLElement>("[data-command='profile_achievement_view_mode']");
    for (let i = 0; i < modeBtns.length; i++) {
        const active = modeBtns[i].dataset.viewMode === state.viewMode;
        modeBtns[i].classList.toggle("is-active", active);
        modeBtns[i].setAttribute("aria-pressed", String(active));
    }
    const results = panel.querySelector("[data-ach-results]");
    if (results) {
        results.innerHTML = renderAchResultsMarkup(
            enriched,
            isOwnProfile,
            state,
            menuUi.state?.leaders,
        );
    }
}

function matchGoalChainLabel(chainId: string): string {
    const meta = matchGoalChainMeta(chainId);
    if (meta.heroSlug) {
        const leader = leaderById(meta.heroSlug);
        const heroName = leader ? leaderDisplayName(leader) : meta.heroSlug.replace(/_/g, " ");
        return heroName + " · " + SOW_t(meta.labelKey);
    }
    return SOW_t(meta.labelKey);
}

function latestMatchGoalsMarkup(
    summary: ReturnType<typeof latestMatchGoalSummary>,
    achievements: Map<string, EnrichedAchievement>,
) {
    if (!summary || (!summary.advances.length && !summary.unlockedIds.length)) return "";
    const progressRows = summary.advances
        .map(function (advance) {
            const achievement = achievements.get(advance.achievement_id);
            const title = achievement?.localizedTitle || advance.achievement_id.replace(/_/g, " ");
            const chainLabel = matchGoalChainLabel(advance.chain_id);
            const span = Math.max(1, advance.target - advance.previous);
            const beforePct = Math.max(
                0,
                Math.min(100, ((advance.before - advance.previous) / span) * 100),
            );
            const afterPct = Math.max(
                beforePct,
                Math.min(100, ((advance.after - advance.previous) / span) * 100),
            );
            const visibleProgress = Math.min(advance.after, advance.target);
            const amount = advance.after - advance.before;
            return (
                "<li class='sow-profile__match-goal-row'><div class='sow-profile__match-goal-copy'><div><strong>" +
                esc(title) +
                "</strong><small>" +
                esc(chainLabel) +
                "</small></div><b>" +
                esc(profileStat(visibleProgress)) +
                "/" +
                esc(profileStat(advance.target)) +
                " <span>+" +
                esc(profileStat(amount)) +
                "</span></b></div><div class='sow-profile__match-goal-meter' role='progressbar' aria-label='" +
                esc(title) +
                "' aria-valuemin='" +
                esc(advance.previous) +
                "' aria-valuemax='" +
                esc(advance.target) +
                "' aria-valuenow='" +
                esc(visibleProgress) +
                "'><i style='width:" +
                beforePct.toFixed(2) +
                "%'></i><b style='inset-inline-start:" +
                beforePct.toFixed(2) +
                "%;width:" +
                Math.max(0, afterPct - beforePct).toFixed(2) +
                "%'></b></div></li>"
            );
        })
        .join("");
    const unlockRows = summary.unlockedIds
        .map(function (id) {
            const achievement = achievements.get(id);
            if (!achievement) return "";
            return (
                "<li><img class='sow-profile__match-goal-laurel' src='" +
                esc(currencyAsset("laurel")) +
                "' alt='' aria-hidden='true' decoding='async'><strong>" +
                esc(achievement.localizedTitle) +
                "</strong><small>+" +
                esc(profileStat(achievement.points)) +
                " " +
                esc(SOW_t("profile.laurels")) +
                "</small></li>"
            );
        })
        .join("");
    const unlocks = unlockRows
        ? "<div class='sow-profile__match-goal-unlocks'><h4>" +
          esc(SOW_t("profile.match_goals_unlocked")) +
          "</h4><ul>" +
          unlockRows +
          "</ul></div>"
        : "";
    return (
        "<section class='sow-profile__match-goals' aria-labelledby='sow-profile-last-match-goals'>" +
        "<div class='sow-profile__match-goals-head'><h3 id='sow-profile-last-match-goals' tabindex='-1'>" +
        esc(SOW_t("profile.match_goals_last_match")) +
        "</h3><span>" +
        esc(SOW_t("profile.match_goals_saved")) +
        "</span></div>" +
        (progressRows
            ? "<ul class='sow-profile__match-goal-list'>" + progressRows + "</ul>"
            : "<p class='sow-profile__match-goals-empty'>" +
              esc(SOW_t("profile.match_goals_unlocked_only")) +
              "</p>") +
        unlocks +
        "</section>"
    );
}

function profileAchievementsMarkup(data: ProfileData, complete: boolean) {
    const isOwnProfile = Boolean(menuUi.state && data.account_id === menuUi.state.account_id);
    const matchSummary = isOwnProfile
        ? latestMatchGoalSummary(menuUi.state?.reward_receipts || [])
        : null;
    if (
        matchSummary &&
        (matchSummary.advances.length || matchSummary.unlockedIds.length) &&
        menuUi.state?.account_id
    ) {
        markMatchGoalsViewed(menuUi.state.account_id, matchSummary.matchId);
    }
    const matchAchievementIds = new Set([
        ...(matchSummary?.unlockedIds || []),
        ...(matchSummary?.advances.map((advance) => advance.achievement_id) || []),
    ]);
    const matchAchievements = new Map<string, EnrichedAchievement>();
    for (const id of matchAchievementIds) {
        const profileAchievement = getAchievementDefinition(id);
        if (profileAchievement) {
            matchAchievements.set(
                id,
                enrichAchievement(profileAchievement, { isOwnProfile: true }),
            );
        }
    }
    const matchGoals = latestMatchGoalsMarkup(matchSummary, matchAchievements);
    if (!complete) {
        return (
            matchGoals +
            "<p class='sow-profile__empty'>" +
            esc(profileSnapshotError() || SOW_t("profile.loading_profile")) +
            "</p>"
        );
    }
    const codex = renderProfileAchievementsCodex({
        data,
        isOwnProfile,
        state: currentAchViewState(),
        catalog: menuUi.state?.leaders,
    });
    return matchGoals + codex.markup;
}

function profileContentMarkup(data: ProfileData | null) {
    const entry = activeProfileEntry();
    const complete = profileDataComplete();
    let content = "";
    if (data && menuUi.profileTab === "overview") {
        const ranked = (data.ratings || []).find(function (rating) {
            return rating.queue === "Ranked1v1" && rating.mode === "FFA";
        });
        const rankedFfa = (data.ratings || []).find(function (rating) {
            return rating.queue === "RankedFFA" && rating.mode === "FFA";
        });
        const rankedPanel = complete
            ? rankOverview(
                  ranked || null,
                  rankedFfa || null,
                  data.ranked_ladder,
                  data.account_id === menuUi.state?.account_id,
                  menuUi.profileOverviewQueue,
              )
            : "";
        const recent = complete
            ? menuUi.profileHistory.slice(0, 10).map(renderProfileMatchRow).join("") ||
              "<p class='sow-profile__empty'>" +
                  esc(SOW_t("profile.no_completed_matches")) +
                  "</p>"
            : "<p class='sow-profile__empty'>" +
              esc(profileSnapshotError() || SOW_t("profile.loading_profile")) +
              "</p>";
        const topLeaders =
            complete && data.leaders && data.leaders.length
                ? "<section class='sow-profile__section sow-profile__section--widget'><div class='sow-profile__section-head'><h3>" +
                  esc(SOW_t("profile.most_played_leaders")) +
                  "</h3><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='leaders'>" +
                  esc(SOW_t("profile.view_all")) +
                  "</button></div><div class='sow-profile__top-leaders'>" +
                  data.leaders.slice(0, 3).map(renderProfileLeaderCard).join("") +
                  "</div></section>"
                : "";
        content =
            "<div class='sow-profile__overview'>" +
            "<div class='sow-profile__overview-feed'>" +
            "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.recent_matches")) +
            "</h2><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='history'>" +
            esc(SOW_t("profile.view_all")) +
            "</button></div>" +
            recent +
            "</section></div>" +
            (rankedPanel || topLeaders
                ? "<aside class='sow-profile__overview-sidebar'>" +
                  rankedPanel +
                  topLeaders +
                  "</aside>"
                : "") +
            "</div>";
    } else if (data && menuUi.profileTab === "leaders") {
        const leaderList = complete
            ? (data.leaders || []).map(renderProfileLeaderCard).join("") ||
              "<p class='sow-profile__empty'>" +
                  esc(SOW_t("profile.no_leader_history")) +
                  "</p>"
            : "<p class='sow-profile__empty'>" +
              esc(profileSnapshotError() || SOW_t("profile.loading_profile")) +
              "</p>";
        content =
            "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.tab_commanders")) +
            "</h2><span class='sow-profile__section-note'>" +
            esc(
                SOW_t("profile.leader_count", {
                    count: complete ? (data.leaders || []).length : "—",
                }),
            ) +
            "</span></div><div class='sow-profile__leaders'>" +
            leaderList +
            "</div></section>";
    } else if (menuUi.profileTab === "history") {
        const filteredHistory = filterMatchesByQueue(
            menuUi.profileHistory,
            menuUi.profileHistoryQueueFilter || "all",
        );
        const historyList = !complete
            ? "<p class='sow-profile__empty'>" +
              esc(profileSnapshotError() || SOW_t("profile.loading_profile")) +
              "</p>"
            : filteredHistory.map(renderProfileMatchRow).join("") ||
              "<p class='sow-profile__empty'>" +
                  esc(SOW_t("profile.no_completed_matches")) +
                  "</p>";
        const canLoadMore = !!(
            entry &&
            complete &&
            (entry.historyHasMore || entry.historyError)
        );
        const historyAction =
            menuUi.profileAccountId && (canLoadMore || !complete)
                ? "<button type='button' class='sow-profile__load-more' data-command='load_profile_more'" +
                  (menuUi.profileHistoryLoading ? " disabled" : "") +
                  ">" +
                  (menuUi.profileHistoryLoading
                      ? esc(SOW_t("profile.loading_more"))
                      : entry && entry.historyError
                        ? esc(SOW_t("profile.try_again"))
                        : esc(SOW_t("profile.load_more_matches"))) +
                  "</button>"
                : "";
        content =
            "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.match_history")) +
            "</h2><span class='sow-profile__section-note'>" +
            esc(
                SOW_t("profile.match_count", { count: data ? data.matches_played : 0 }),
            ) +
            "</span></div>" +
            renderHistoryQueueFilterPills(menuUi.profileHistoryQueueFilter || "all") +
            "<div class='sow-profile__history'>" +
            historyList +
            "</div>" +
            historyAction +
            "</section>";
    } else if (data && menuUi.profileTab === "achievements") {
        content =
            "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.achievements")) +
            "</h2><span class='sow-profile__section-note'>" +
            esc(data.laurels || 0) +
            " " +
            esc(SOW_t("profile.laurels")) +
            "</span></div>" +
            profileAchievementsMarkup(data, complete) +
            "</section>";
    } else if (menuUi.profileTab === "victories") {
        content =
            "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.leaderboards")) +
            "</h2></div><div class='sow-profile__lb-host' data-game-lb-mount></div></section>";
    } else if (!data) {
        content =
            "<section class='sow-profile__state' aria-live='polite'><strong>" +
            esc(
                menuUi.profileSnapshotLoading
                    ? SOW_t("profile.loading_profile")
                    : SOW_t("profile.profile_unavailable"),
            ) +
            "</strong><span>" +
            esc(profileSnapshotError() || SOW_t("profile.return_to_menu")) +
            "</span>" +
            (menuUi.profileSnapshotLoading
                ? ""
                : "<button type='button' class='sow-profile__load-more' data-command='retry_profile'>" +
                  esc(SOW_t("profile.try_again")) +
                  "</button>") +
            "</section>";
    }
    return (
        "<div id='sow-profile-panel-active' class='sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-" +
        menuUi.profileTab +
        "'>" +
        content +
        "</div>"
    );
}

function buildHeaderActions(own: boolean): string {
    const hasNavBack =
        !own || (menuUi.profileNavStack && menuUi.profileNavStack.length > 0);
    const backLabel = hasNavBack
        ? "← " + SOW_t("lobbies.back")
        : SOW_t("lobbies.back");
    const myProfileButton = !own
        ? "<button type='button' class='sow-profile__my-profile-btn' data-command='back_to_my_profile'>" +
          esc(SOW_t("profile.your_profile")) +
          "</button>"
        : "";
    return (
        myProfileButton +
        "<button type='button' class='sow-profile__back' data-command='close_profile'>" +
        esc(backLabel) +
        "</button>"
    );
}

export function renderProfile() {
    const data = menuUi.profileData;
    const own = Boolean(menuUi.state && menuUi.state.account_id === menuUi.profileAccountId);
    const header = renderProfileHeader({
        data,
        own,
        loading: menuUi.profileSnapshotLoading,
        snapshotError: profileSnapshotError(),
        shareCopied: menuUi.profileShareCopied,
        showShare: !window.SOW_PORTAL,
        extraTopActions: buildHeaderActions(own),
        avatarFallbackUrl: avatarImage(),
    });
    const tabs = ["overview", "leaders", "history", "achievements", "victories"]
        .map(function (tab) {
            const active = menuUi.profileTab === tab;
            const label =
                tab === "victories"
                    ? SOW_t("profile.leaderboards")
                    : SOW_t("profile.tab_" + (tab === "leaders" ? "commanders" : tab));
            return (
                "<button type='button' role='tab' id='sow-profile-tab-" +
                tab +
                "' class='sow-profile__tab" +
                (active ? " is-active" : "") +
                "' aria-selected='" +
                active +
                "' aria-controls='sow-profile-panel-active' data-command='profile_tab' data-profile-tab='" +
                tab +
                "'>" +
                esc(label) +
                "</button>"
            );
        })
        .join("");
    let playGamesActions = "";
    if (
        typeof window.SOW_isAndroidTwa === "function" &&
        window.SOW_isAndroidTwa()
    ) {
        playGamesActions =
            "<section class='sow-profile__section' aria-label='" +
            esc(SOW_t("profile.google_play_games")) +
            "'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.google_play_games")) +
            "</h2><span class='sow-profile__section-note'>" +
            esc(SOW_t("profile.android")) +
            "</span></div><div class='sow-profile__actions'><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='achievements'>" +
            esc(SOW_t("profile.achievements")) +
            "</button><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='leaderboards'>" +
            esc(SOW_t("profile.leaderboards")) +
            "</button></div></section>";
    }
    const profileActions = playGamesActions + renderModeration(own);
    return (
        "<main class='sow-menu__main sow-profile__main sow-profile-host' data-screen-panel='profile'><section class='sow-profile__page'><div class='sow-profile__dossier'>" +
        header +
        "<nav class='sow-profile__tabs' role='tablist' aria-label='" +
        esc(SOW_t("profile.profile_sections")) +
        "'>" +
        tabs +
        "</nav><div data-profile-content class='sow-profile__content'>" +
        profileContentMarkup(data) +
        "</div></div>" +
        (profileActions
            ? "<aside class='sow-profile__tools'>" + profileActions + "</aside>"
            : "") +
        "</section></main>"
    );
}

export function syncProfileDom() {
    if (!menuUi.profileOpen || currentScreen() !== "profile") return;
    const panel = activeScreenPanel();
    if (!panel) return;
    const data = menuUi.profileData;
    const displayName = panel.querySelector("[data-profile-display-name]");
    const handle = panel.querySelector("[data-profile-handle]");
    const level = panel.querySelector("[data-profile-level]");
    if (displayName)
        displayName.textContent =
            data && data.display_name
                ? data.display_name
                : menuUi.profileSnapshotLoading
                  ? SOW_t("profile.loading_profile")
                  : SOW_t("profile.profile_unavailable");
    if (handle)
        handle.textContent =
            data && data.handle
                ? "@" + data.handle.replace(/^@/, "")
                : menuUi.profileSnapshotLoading
                  ? SOW_t("profile.fetching_player_data")
                  : "";
    if (level)
        level.textContent = data && data.level != null ? String(data.level) : "—";
    const avatar = panel.querySelector<HTMLImageElement>("[data-profile-avatar]");
    if (avatar) {
        const preferredLeader =
            data && data.preferred_leader ? leaderById(data.preferred_leader) : null;
        const avatarUrl = preferredLeader
            ? asset("gameplay/avatars/" + leaderArtSlug(preferredLeader) + ".webp")
            : avatarImage();
        if (avatar.src !== avatarUrl) setAvatarImage(avatar, avatarUrl);
    }
    const hero = profileHeroImage(data && data.preferred_leader);
    if (root && root.dataset.hero !== hero) {
        root.style.setProperty("--sow-hero", 'url("' + hero + '")');
        root.dataset.hero = hero;
    }
    const cover = panel.querySelector<HTMLElement>("[data-profile-cover]");
    if (cover) {
        cover.style.setProperty("--sow-hero", 'url("' + hero + '")');
    }
    const statsContainer = panel.querySelector("[data-profile-stats]");
    if (statsContainer && data) {
        statsContainer.outerHTML = renderProfileStatsRibbon(data);
    }
    const shareWrap = panel.querySelector<HTMLElement>(".sow-profile__share-wrap");
    if (shareWrap) {
        const existingToast = shareWrap.querySelector(".sow-profile__toast");
        if (menuUi.profileShareCopied && !existingToast) {
            const toast = document.createElement("span");
            toast.className = "sow-profile__toast";
            toast.setAttribute("role", "status");
            toast.textContent = SOW_t("site.profile_link_copied");
            shareWrap.appendChild(toast);
        } else if (!menuUi.profileShareCopied && existingToast) {
            existingToast.remove();
        }
    }
    const status = panel.querySelector<HTMLElement>("[data-profile-status]");
    const snapshotError = data && profileSnapshotError();
    if (status) {
        status.hidden = !snapshotError;
        status.textContent = snapshotError ? snapshotError + " " : "";
        if (snapshotError) {
            const retry = document.createElement("button");
            retry.type = "button";
            retry.className = "sow-profile__status-action";
            retry.dataset.command = "retry_profile";
            retry.textContent = SOW_t("profile.try_again");
            status.appendChild(retry);
        }
    }
    const content = panel.querySelector("[data-profile-content]");
    const hasLiveLb = Boolean(
        menuUi.profileTab === "victories" &&
            gameLbController &&
            gameLbHost &&
            panel.contains(gameLbHost),
    );
    if (content && !hasLiveLb) {
        content.innerHTML = profileContentMarkup(data);
    }
    mountGameProfileInteractive(panel);
}

export function renderProfileDetail() {
    return renderProfileMatchDetailModal({
        detail: menuUi.profileMatchDetail,
        detailId: menuUi.profileDetailId,
        detailError: menuUi.profileDetailError,
        mode: "game",
    });
}

// Other-player profiles can be reported here; account deletion lives in Settings.
function renderModeration(own: boolean | null) {
    if (own || !menuUi.profileAccountId || !selfCreds()) return "";
    if (menuUi.reportSent) {
        return (
            "<section class='sow-profile__section' aria-label='" +
            esc(SOW_t("profile.report_filed")) +
            "'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.report_filed")) +
            "</h2></div><p class='sow-profile__empty'>" +
            esc(SOW_t("profile.thanks_report")) +
            "</p></section>"
        );
    }
    if (isBlockedAccountId(menuUi.profileAccountId)) {
        return (
            "<section class='sow-profile__section' aria-label='" +
            esc(SOW_t("profile.blocked")) +
            "'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.blocked")) +
            "</h2></div><p class='sow-profile__empty'>" +
            esc(SOW_t("profile.blocked_player")) +
            "</p></section>"
        );
    }
    if (!menuUi.reportOpen || menuUi.reportTarget !== menuUi.profileAccountId) {
        return (
            "<section class='sow-profile__section' aria-label='" +
            esc(SOW_t("profile.conduct")) +
            "'><div class='sow-profile__section-head'><h2>" +
            esc(SOW_t("profile.conduct")) +
            "</h2></div><button type='button' class='sow-menu__ghost-button' data-command='open_report' data-account-id='" +
            esc(menuUi.profileAccountId) +
            "'>" +
            esc(SOW_t("profile.report_player")) +
            "</button></section>"
        );
    }
    return (
        "<section class='sow-profile__section' aria-label='" +
        esc(SOW_t("profile.report_player")) +
        "'><div class='sow-profile__section-head'><h2>" +
        esc(SOW_t("profile.report_player")) +
        "</h2></div>" +
        "<form data-form='report'>" +
        "<label for='sow-report-reason'>" +
        esc(SOW_t("profile.reason")) +
        "</label>" +
        SOW_renderDropdown({
            key: "report-reason",
            name: "reason",
            value: REPORT_REASONS[0][0],
            options: REPORT_REASONS.map(function (r) {
                return { value: r[0], label: SOW_t(r[1]) };
            }),
        }) +
        "<label for='sow-report-details'>" +
        esc(SOW_t("profile.report_details")) +
        "</label>" +
        "<textarea id='sow-report-details' name='details' rows='3' maxlength='500' placeholder='" +
        esc(SOW_t("profile.what_happened")) +
        "'></textarea>" +
        "<p class='sow-profile__empty'>" +
        esc(SOW_t("profile.report_notice")) +
        "</p>" +
        "<button type='submit'" +
        (reportBusy ? " disabled" : "") +
        ">" +
        esc(
            reportBusy
                ? SOW_t("profile.sending_report")
                : SOW_t("profile.send_report_block"),
        ) +
        "</button> " +
        "<button type='button' class='sow-menu__ghost-button' data-command='cancel_report'>" +
        esc(SOW_t("profile.cancel_report")) +
        "</button>" +
        "</form></section>"
    );
}
