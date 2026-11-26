import { avatarImageAttribute, setAvatarImage } from "./avatar-presentation.ts";
import { syncDropdownScope } from "./sow-dropdown.ts";
import {
    WOW_PILLAR_ORDER,
    WOW_SUBCATEGORIES,
    enrichAchievement,
    enrichAchievementsList,
    matchesAchievementFilters,
    renderAchievementCard,
    renderAchievementChainCard,
    renderHeroMasteryProfileCard,
    resolveProfileAchievements,
    sortAchievementsList,
    summarizeEnrichedHeroAchievements,
    wowCategoryTitle,
    wowSubcategoryTitle,
    type EnrichedAchievement,
    type ProfileAchievement,
    type WowCategoryId,
    type WowConcretePillar,
} from "./main_menu.achievements.ts";
import type { MenuLeader, RankedLadder, RankedPointsBreakdown } from "./main_menu.state.ts";
import {
    assetUrl,
    bindPointerSpotlight,
    copyTextToClipboard,
    currencyAssetUrl,
    esc,
    formatMapName,
    getLeaderCatalog,
    isValidAccountId,
    leaderArtSlug,
    leaderAvatarUrl,
    leaderById,
    leaderDisplayName,
    levelFromXp,
    profileApiUrl,
    profileHeroBackdropUrl,
    publicLeaderboardsUrl,
    publicProfileUrl,
    tSafe,
} from "./sow-profile-helpers.ts";
import {
    rankEmblem,
    rankLabel,
    rankName,
    rankTier,
    rankedPointsBreakdownMarkup,
    renderRankedOverview,
} from "./sow-ranked-presentation.ts";
import {
    buildLeaderboardRequestUrl,
    type LeaderboardRankedEntry,
} from "./sow-leaderboard-view.ts";

export interface ProfileRating {
    season_id: number;
    season_name: string;
    queue: string;
    mode: string;
    games_played: number;
    wins: number;
    placements_complete: boolean;
    score: number;
    tier: string;
    division: string | null;
    peak_score: number;
}

export interface ProfileLeader {
    leader: string;
    matches_played: number;
    wins: number;
    win_rate: number;
    xp: number;
}

export interface ProfileMatch {
    match_id: string;
    completed_at: number;
    queue: string;
    mode: string;
    map_name: string;
    duration_seconds: number;
    placement?: number;
    won: boolean;
    leader: string | null;
    kills: number;
    deaths: number;
    assists: number;
    verified: boolean;
    provisional: boolean;
    rating_delta?: number | null;
    ranked_bot_iq?: number | null;
    ranked_points_breakdown?: RankedPointsBreakdown | null;
}

export interface ProfileMatchParticipant {
    account_id: string;
    handle: string;
    placement?: number;
    leader: string | null;
    won: boolean;
    kills: number;
    deaths: number;
    assists: number;
    is_bot: boolean;
    rating_delta?: number | null;
    rating_before?: number | null;
    rating_after?: number | null;
    ranked_points_breakdown?: RankedPointsBreakdown | null;
}

export interface ProfileMatchDetail {
    completed_at: number;
    queue: string;
    mode: string;
    map_name: string;
    duration_seconds: number;
    verified: boolean;
    provisional: boolean;
    ranked_bot_iq?: number | null;
    participants: ProfileMatchParticipant[];
}

export interface ProfileData {
    account_id: string;
    handle: string;
    display_name: string;
    is_bot?: boolean;
    ranked_bot_iq?: number | null;
    level: number;
    matches_played: number;
    wins: number;
    win_rate: number;
    kills: number;
    deaths: number;
    assists: number;
    laurels: number;
    players_defeated: number;
    empires_defeated: number;
    tribes_defeated: number;
    preferred_leader: string | null;
    achievements?: ProfileAchievement[];
    leaders: ProfileLeader[];
    recent_matches: ProfileMatch[];
    ratings?: ProfileRating[];
    ranked_ladder?: RankedLadder | null;
}

export interface ProfileVictoryLeader {
    rank: number;
    account_id: string;
    handle: string;
    level: number;
    matches_played: number;
    wins: number;
    preferred_leader?: string | null;
    win_rate?: number;
}

export interface ProfileRankedLeader {
    rank: number;
    account_id: string;
    handle: string;
    is_bot: boolean;
    score: number;
    tier: string;
    division: string | null;
    games_played: number;
    wins: number;
    peak_score?: number;
    win_rate?: number;
    win_streak?: number;
    level?: number;
    preferred_leader?: string | null;
}

export interface ProfileSearchItem {
    account_id: string;
    handle: string;
    display_name: string;
    kind: string;
    verified_wins: number;
    level?: number;
    matches_played?: number;
    preferred_leader?: string | null;
    ranked_1v1_score?: number;
    ranked_ffa_score?: number;
}

export interface ProfilePage<Item> {
    items: Item[];
    next_cursor: number | null;
}

export interface AchievementCodexViewState {
    category: WowCategoryId;
    subcategory: string;
    status: string;
    tier: string;
    sort: string;
    viewMode: "chains" | "grid";
    search: string;
    playableOnly: boolean;
    flatLimit: number;
    selectedTiers: Record<string, string>;
}

export function defaultAchievementCodexState(): AchievementCodexViewState {
    return {
        category: "summary",
        subcategory: "all",
        status: "all",
        tier: "all",
        sort: "smart",
        viewMode: "chains",
        search: "",
        playableOnly: false,
        flatLimit: 90,
        selectedTiers: Object.create(null),
    };
}

export function profileStat(value: unknown): number {
    const number = Number(value);
    return Number.isFinite(number) ? Math.max(0, Math.floor(number)) : 0;
}

export function profileMatchDate(timestamp: number): string {
    const seconds = Number(timestamp);
    if (!Number.isFinite(seconds) || seconds <= 0) return "—";
    try {
        return new Intl.DateTimeFormat(
            (typeof document !== "undefined" && document.documentElement?.lang) || undefined,
            { dateStyle: "medium" },
        ).format(new Date(seconds * 1000));
    } catch {
        return new Date(seconds * 1000).toLocaleDateString();
    }
}

export function profileMatchDuration(seconds: number): string {
    const duration = Math.max(0, Math.floor(Number(seconds) || 0));
    if (!duration) return "—";
    return Math.floor(duration / 60) + "m " + (duration % 60) + "s";
}

export function normalizeWowCategory(raw: string | undefined): WowCategoryId {
    if (
        raw === "summary" ||
        raw === "general" ||
        raw === "pvp_combat" ||
        raw === "coop_campaign" ||
        raw === "heroes_mastery" ||
        raw === "prestige" ||
        raw === "all"
    ) {
        return raw;
    }
    if (raw === "pve_campaign" || raw === "campaign") return "coop_campaign";
    if (raw === "battles" || raw === "victories") return "general";
    if (raw === "combat") return "pvp_combat";
    if (raw === "commanders") return "heroes_mastery";
    if (raw === "laurels") return "prestige";
    return "summary";
}

function matchesWowScope(
    ach: EnrichedAchievement,
    category: WowCategoryId,
    subcategory: string,
): boolean {
    if (category === "all" || category === "summary") return true;
    if (ach.wowCategory !== category) return false;
    if (!subcategory || subcategory === "all") return true;
    if (subcategory.startsWith("region:")) {
        return ach.heroRegion === subcategory.slice(7);
    }
    if (subcategory.startsWith("kind:")) {
        return ach.heroKind === subcategory.slice(5);
    }
    return ach.wowSubcategory === subcategory;
}

export function achPillarCounts(
    enriched: EnrichedAchievement[],
    pillar: WowCategoryId,
    isOwnProfile: boolean,
    playableOnly: boolean,
) {
    let total = 0;
    let unlocked = 0;
    let earnedLaurels = 0;
    for (const a of enriched) {
        if (pillar !== "all" && pillar !== "summary" && a.wowCategory !== pillar) continue;
        if (playableOnly && isOwnProfile && a.heroSlug && !a.heroPlayable) continue;
        total += 1;
        if (a.unlocked) {
            unlocked += 1;
            earnedLaurels += a.points;
        }
    }
    const percent = total > 0 ? Math.round((unlocked / total) * 100) : 0;
    return { total, unlocked, percent, earnedLaurels };
}

export function achSubCounts(
    enriched: EnrichedAchievement[],
    pillar: WowConcretePillar,
    subcategory: string,
    isOwnProfile: boolean,
    playableOnly: boolean,
) {
    let total = 0;
    let unlocked = 0;
    for (const a of enriched) {
        if (!matchesWowScope(a, pillar, subcategory)) continue;
        if (playableOnly && isOwnProfile && a.heroSlug && !a.heroPlayable) continue;
        total += 1;
        if (a.unlocked) unlocked += 1;
    }
    return { total, unlocked };
}

export function renderAchRailMarkup(
    enriched: EnrichedAchievement[],
    activeCategory: WowCategoryId,
    activeSubcategory: string,
    isOwnProfile: boolean,
    playableOnly: boolean,
): string {
    const railCategories: WowCategoryId[] = ["summary", ...WOW_PILLAR_ORDER, "all"];
    return railCategories
        .map(function (cat) {
            const counts = achPillarCounts(enriched, cat, isOwnProfile, playableOnly);
            const isActive = activeCategory === cat;
            const isConcrete = cat !== "summary" && cat !== "all";
            const subItems =
                isActive && isConcrete
                    ? "<div class='sow-profile__ach-rail-subs'>" +
                      WOW_SUBCATEGORIES[cat]
                          .map(function (sub) {
                              const subC = achSubCounts(enriched, cat, sub, isOwnProfile, playableOnly);
                              if (sub !== "all" && subC.total === 0) return "";
                              const subActive = activeSubcategory === sub;
                              return (
                                  "<button type='button' class='sow-profile__ach-rail-sub" +
                                  (subActive ? " is-active" : "") +
                                  "' data-command='profile_achievement_subcategory' data-achievement-subcategory='" +
                                  esc(sub) +
                                  "'><span>" +
                                  esc(wowSubcategoryTitle(sub)) +
                                  "</span><b>" +
                                  esc(subC.unlocked + "/" + subC.total) +
                                  "</b></button>"
                              );
                          })
                          .join("") +
                      "</div>"
                    : "";
            return (
                "<div class='sow-profile__ach-rail-group" +
                (isActive ? " is-active" : "") +
                "'><button type='button' class='sow-profile__ach-rail-btn" +
                (isActive ? " is-active" : "") +
                "' data-command='profile_achievement_filter' data-achievement-filter='" +
                esc(cat) +
                "'><div class='sow-profile__ach-rail-top'><span>" +
                esc(wowCategoryTitle(cat)) +
                "</span><b>" +
                esc(counts.unlocked + "/" + counts.total) +
                "</b></div><div class='sow-profile__ach-rail-bar'><i style='width:" +
                esc(counts.percent) +
                "%'></i></div></button>" +
                subItems +
                "</div>"
            );
        })
        .join("");
}

export function renderAchSubbarMarkup(
    enriched: EnrichedAchievement[],
    activeCategory: WowCategoryId,
    activeSubcategory: string,
    isOwnProfile: boolean,
    playableOnly: boolean,
): string {
    if (activeCategory === "summary" || activeCategory === "all") return "";
    const subs = WOW_SUBCATEGORIES[activeCategory] || [];
    if (subs.length <= 2) return "";
    return subs
        .map(function (sub) {
            const subC = achSubCounts(enriched, activeCategory, sub, isOwnProfile, playableOnly);
            if (sub !== "all" && subC.total === 0) return "";
            const isActive = activeSubcategory === sub;
            return (
                "<button type='button' class='sow-profile__ach-subchip" +
                (isActive ? " is-active" : "") +
                "' data-command='profile_achievement_subcategory' data-achievement-subcategory='" +
                esc(sub) +
                "'><span>" +
                esc(wowSubcategoryTitle(sub)) +
                "</span><b>" +
                esc(subC.unlocked + "/" + subC.total) +
                "</b></button>"
            );
        })
        .join("");
}

function renderAchSummaryView(enriched: EnrichedAchievement[], isOwnProfile: boolean): string {
    const pillarCards = WOW_PILLAR_ORDER.map(function (pillar) {
        const stats = achPillarCounts(enriched, pillar, isOwnProfile, false);
        return (
            "<button type='button' class='sow-profile__ach-pillar-card' data-command='profile_achievement_filter' data-achievement-filter='" +
            esc(pillar) +
            "'><div class='sow-profile__ach-pillar-head'><strong>" +
            esc(wowCategoryTitle(pillar)) +
            "</strong><span>" +
            esc(stats.percent) +
            "%</span></div><div class='sow-profile__achievement-track'><i style='width:" +
            esc(stats.percent) +
            "%'></i></div><div class='sow-profile__ach-pillar-foot'><span>" +
            esc(stats.unlocked + " / " + stats.total) +
            "</span><b>" +
            esc(stats.earnedLaurels + " " + tSafe("profile.laurels", "Laurels")) +
            "</b></div></button>"
        );
    }).join("");

    const almostLocked = enriched
        .filter(function (a) {
            if (a.unlocked) return false;
            if (a.progress > 0) return true;
            if (!isOwnProfile) return false;
            return !a.heroSlug || a.heroPlayable;
        })
        .sort(function (a, b) {
            const aHas = a.progress > 0 ? 1 : 0;
            const bHas = b.progress > 0 ? 1 : 0;
            if (aHas !== bHas) return bHas - aHas;
            if (b.percent !== a.percent) return b.percent - a.percent;
            const aGlobal = a.heroSlug ? 1 : 0;
            const bGlobal = b.heroSlug ? 1 : 0;
            if (aGlobal !== bGlobal) return aGlobal - bGlobal;
            return a.target - b.target;
        })
        .slice(0, 6);

    const highlights = enriched
        .filter((a) => a.unlocked)
        .sort((a, b) => (b.points !== a.points ? b.points - a.points : b.target - a.target))
        .slice(0, 6);

    return (
        "<section class='sow-profile__ach-summary-section'><h3 class='sow-profile__ach-subhead'>" +
        esc(tSafe("profile.ach_summary_pillars", "Progress by Pillar")) +
        "</h3><div class='sow-profile__ach-summary-grid'>" +
        pillarCards +
        "</div></section><section class='sow-profile__ach-summary-section'><h3 class='sow-profile__ach-subhead'>" +
        esc(tSafe("profile.ach_summary_almost", "Close to Unlocking")) +
        "</h3>" +
        (almostLocked.length
            ? "<div class='sow-profile__achievement-grid'>" +
              almostLocked
                  .map((a) => renderAchievementCard(a, { tagText: wowCategoryTitle(a.wowCategory) }))
                  .join("") +
              "</div>"
            : "<p class='sow-profile__empty'>" +
              esc(
                  tSafe(
                      "profile.ach_summary_almost_empty",
                      "Play matches or command heroes to start tracking nearby milestones.",
                  ),
              ) +
              "</p>") +
        "</section><section class='sow-profile__ach-summary-section'><h3 class='sow-profile__ach-subhead'>" +
        esc(tSafe("profile.ach_summary_highlights", "Earned Highlights")) +
        "</h3>" +
        (highlights.length
            ? "<div class='sow-profile__achievement-grid'>" +
              highlights
                  .map((a) => renderAchievementCard(a, { tagText: wowCategoryTitle(a.wowCategory) }))
                  .join("") +
              "</div>"
            : "<p class='sow-profile__empty'>" +
              esc(
                  tSafe(
                      "profile.ach_summary_highlights_empty",
                      "Complete your first milestone to showcase it here.",
                  ),
              ) +
              "</p>") +
        "</section>"
    );
}

export function renderAchResultsMarkup(
    enriched: EnrichedAchievement[],
    isOwnProfile: boolean,
    state: AchievementCodexViewState,
    catalog?: MenuLeader[],
): string {
    const activeCategory = normalizeWowCategory(state.category);
    const activeSub = state.subcategory || "all";
    const statusFilter = state.status || "all";
    const tierFilter = state.tier || "all";
    const sortBy = state.sort || "smart";
    const playableOnly = Boolean(state.playableOnly) && isOwnProfile;
    const viewMode = state.viewMode === "grid" ? "grid" : "chains";
    const searchQuery = (state.search || "").trim();

    const hasToolbarFilters =
        Boolean(searchQuery) || statusFilter !== "all" || tierFilter !== "all" || playableOnly;

    if (activeCategory === "summary" && !hasToolbarFilters) {
        return renderAchSummaryView(enriched, isOwnProfile);
    }

    const scopeCategory: WowCategoryId =
        activeCategory === "summary" && hasToolbarFilters ? "all" : activeCategory;

    const scoped = enriched.filter(function (ach) {
        if (!matchesWowScope(ach, scopeCategory, activeSub)) return false;
        return matchesAchievementFilters(ach, {
            search: searchQuery,
            status: statusFilter,
            tier: tierFilter,
            playableOnly,
            isOwnProfile,
        });
    });

    const searchBanner =
        searchQuery && scopeCategory === "all"
            ? "<div class='sow-profile__ach-search-banner'><span>" +
              esc(
                  tSafe("profile.ach_searching_global", "Searching entire codex ({count} results)", {
                      count: scoped.length,
                  }),
              ) +
              "</span><button type='button' class='sow-profile__ach-clear-btn' data-command='profile_achievement_clear'>" +
              esc(tSafe("profile.ach_clear_filters", "Clear filters")) +
              "</button></div>"
            : "";

    if (!scoped.length) {
        const globalMatches =
            scopeCategory !== "all" && hasToolbarFilters
                ? enriched.filter((a) =>
                      matchesAchievementFilters(a, {
                          search: searchQuery,
                          status: statusFilter,
                          tier: tierFilter,
                          playableOnly,
                          isOwnProfile,
                      }),
                  ).length
                : 0;
        return (
            searchBanner +
            "<div class='sow-profile__ach-empty-box'><p class='sow-profile__empty'>" +
            esc(
                tSafe(
                    "profile.ach_empty_filtered",
                    "No achievements match your current search or filters.",
                ),
            ) +
            "</p><div class='sow-profile__ach-empty-actions'>" +
            (globalMatches > 0
                ? "<button type='button' class='sow-profile__ach-clear-btn is-primary' data-command='profile_achievement_filter' data-achievement-filter='all'>" +
                  esc(wowCategoryTitle("all") + " (" + globalMatches + ")") +
                  "</button>"
                : "") +
            (hasToolbarFilters
                ? "<button type='button' class='sow-profile__ach-clear-btn' data-command='profile_achievement_clear'>" +
                  esc(tSafe("profile.ach_clear_filters", "Clear filters")) +
                  "</button>"
                : "") +
            "</div></div>"
        );
    }

    if (
        scopeCategory === "heroes_mastery" &&
        viewMode === "chains" &&
        statusFilter === "all" &&
        tierFilter === "all" &&
        !searchQuery &&
        (activeSub === "all" || activeSub.startsWith("region:"))
    ) {
        const byHero = new Map<string, EnrichedAchievement[]>();
        for (const a of scoped) {
            if (!a.heroSlug) continue;
            let list = byHero.get(a.heroSlug);
            if (!list) {
                list = [];
                byHero.set(a.heroSlug, list);
            }
            list.push(a);
        }
        const leaderMap = new Map<string, MenuLeader>();
        for (const leader of catalog || getLeaderCatalog()) {
            if (leader.slug) leaderMap.set(String(leader.slug).toLowerCase(), leader);
            if (leader.id) leaderMap.set(String(leader.id).toLowerCase(), leader);
        }
        const summaries = Array.from(byHero.entries()).map(function ([slug, list]) {
            const leader = leaderMap.get(slug) || leaderById(slug);
            return summarizeEnrichedHeroAchievements(
                slug,
                leader && leader.id !== "random" ? leader : null,
                slug,
                list,
            );
        });
        summaries.sort(function (a, b) {
            if (sortBy === "alpha_asc") return a.name.localeCompare(b.name);
            if (sortBy === "points_desc") {
                if (b.earnedLaurels !== a.earnedLaurels) return b.earnedLaurels - a.earnedLaurels;
                return a.name.localeCompare(b.name);
            }
            if (sortBy === "closest") {
                if (b.nextPercent !== a.nextPercent) return b.nextPercent - a.nextPercent;
                return b.unlockedCount - a.unlockedCount;
            }
            const aProg = a.unlockedCount > 0 || a.matchesPlayed > 0 ? 1 : 0;
            const bProg = b.unlockedCount > 0 || b.matchesPlayed > 0 ? 1 : 0;
            if (aProg !== bProg) return bProg - aProg;
            if (b.unlockedCount !== a.unlockedCount) return b.unlockedCount - a.unlockedCount;
            if (isOwnProfile && a.playable !== b.playable) return a.playable ? -1 : 1;
            if (b.nextPercent !== a.nextPercent) return b.nextPercent - a.nextPercent;
            return a.name.localeCompare(b.name);
        });

        const visibleHeroes = summaries.slice(0, state.flatLimit);
        const loadMoreHeroes =
            summaries.length > visibleHeroes.length
                ? "<div class='sow-profile__ach-loadmore-wrap'><button type='button' class='sow-profile__load-more' data-command='profile_achievement_show_more'>" +
                  esc(
                      tSafe("profile.view_all", "View all") +
                          " (" +
                          visibleHeroes.length +
                          "/" +
                          summaries.length +
                          ")",
                  ) +
                  "</button></div>"
                : "";
        return (
            "<div class='sow-profile__ach-hero-grid'>" +
            visibleHeroes.map((s) => renderHeroMasteryProfileCard(s, isOwnProfile)).join("") +
            "</div>" +
            loadMoreHeroes
        );
    }

    if (
        viewMode === "chains" &&
        statusFilter === "all" &&
        tierFilter === "all" &&
        !searchQuery &&
        activeSub !== "kind:recruit" &&
        scopeCategory !== "all"
    ) {
        const singles: EnrichedAchievement[] = [];
        const chainMap = new Map<string, EnrichedAchievement[]>();
        for (const a of scoped) {
            if (a.chainKey) {
                let arr = chainMap.get(a.chainKey);
                if (!arr) {
                    arr = [];
                    chainMap.set(a.chainKey, arr);
                }
                arr.push(a);
            } else {
                singles.push(a);
            }
        }
        const chainEntries = Array.from(chainMap.entries());
        chainEntries.sort(function ([kA, aList], [kB, bList]) {
            if (sortBy === "alpha_asc") return kA.localeCompare(kB);
            const aProgress = aList.some((i) => i.unlocked || i.progress > 0) ? 1 : 0;
            const bProgress = bList.some((i) => i.unlocked || i.progress > 0) ? 1 : 0;
            if (aProgress !== bProgress) return bProgress - aProgress;
            return aList.length - bList.length;
        });

        const limitedChains = chainEntries.slice(0, state.flatLimit);
        const cardsHtml =
            limitedChains
                .map(([key, list]) =>
                    renderAchievementChainCard(key, list, state.selectedTiers[key] || null),
                )
                .join("") +
            sortAchievementsList(singles, sortBy)
                .map((a) => renderAchievementCard(a))
                .join("");

        const loadMoreChains =
            chainEntries.length > limitedChains.length
                ? "<div class='sow-profile__ach-loadmore-wrap'><button type='button' class='sow-profile__load-more' data-command='profile_achievement_show_more'>" +
                  esc(
                      tSafe("profile.view_all", "View all") +
                          " (" +
                          limitedChains.length +
                          "/" +
                          chainEntries.length +
                          ")",
                  ) +
                  "</button></div>"
                : "";
        return "<div class='sow-profile__achievement-grid'>" + cardsHtml + "</div>" + loadMoreChains;
    }

    const sorted = sortAchievementsList(scoped, sortBy);
    const visible = sorted.slice(0, state.flatLimit);
    const moreBtn =
        sorted.length > visible.length
            ? "<div class='sow-profile__ach-loadmore-wrap'><button type='button' class='sow-profile__load-more' data-command='profile_achievement_show_more'>" +
              esc(
                  tSafe("profile.view_all", "View all") +
                      " (" +
                      visible.length +
                      "/" +
                      sorted.length +
                      ")",
              ) +
              "</button></div>"
            : "";

    return (
        searchBanner +
        "<div class='sow-profile__achievement-grid'>" +
        visible
            .map((a) =>
                renderAchievementCard(a, {
                    tagText: scopeCategory === "all" ? wowCategoryTitle(a.wowCategory) : undefined,
                }),
            )
            .join("") +
        "</div>" +
        moreBtn
    );
}

function renderCodexDropdown(config: {
    key: string;
    label: string;
    name: string;
    value: string;
    className?: string;
    options: { value: string; label: string; shortLabel?: string }[];
}): string {
    const win = (typeof window !== "undefined" ? window : {}) as {
        SOW_renderDropdown?: (cfg: typeof config) => string;
    };
    if (typeof win.SOW_renderDropdown === "function") {
        return win.SOW_renderDropdown(config);
    }
    const selected = config.options.find((opt) => String(opt.value) === String(config.value));
    const displayLabel = selected ? selected.shortLabel || selected.label : config.value;
    return (
        "<div class='sow-control-dropdown" +
        (config.className ? " " + esc(config.className) : "") +
        "' data-control-dropdown data-dropdown-key='" +
        esc(config.key) +
        "'><button class='sow-control-dropdown__trigger' type='button' data-command='toggle_dropdown' data-dropdown-key='" +
        esc(config.key) +
        "' data-role='dropdown-trigger' aria-label='" +
        esc(config.label) +
        "' aria-haspopup='listbox' aria-expanded='false' aria-controls='sow-dropdown-" +
        esc(config.key) +
        "'><span class='sow-control-dropdown__trigger-content'><span data-dropdown-label>" +
        esc(displayLabel) +
        "</span></span><span class='sow-control-dropdown__chevron' aria-hidden='true'>⌄</span></button><div class='sow-control-dropdown__menu' id='sow-dropdown-" +
        esc(config.key) +
        "' role='listbox' aria-label='" +
        esc(config.label) +
        "' hidden>" +
        config.options
            .map((opt) => {
                const isSelected = String(opt.value) === String(config.value);
                return (
                    "<button class='sow-control-dropdown__option' type='button' role='option' data-command='select_dropdown' data-dropdown-key='" +
                    esc(config.key) +
                    "' data-dropdown-option-value='" +
                    esc(opt.value) +
                    "' aria-selected='" +
                    (isSelected ? "true" : "false") +
                    "'>" +
                    esc(opt.label) +
                    "</button>"
                );
            })
            .join("") +
        "</div><input type='hidden' name='" +
        esc(config.name) +
        "' value='" +
        esc(config.value) +
        "' data-dropdown-input data-dropdown-value></div>"
    );
}

/** Renders the complete Achievements Codex (Lorel/Progress cards + Rail + Toolbar + Results). */
export function renderProfileAchievementsCodex(opts: {
    data: ProfileData;
    isOwnProfile: boolean;
    state: AchievementCodexViewState;
    catalog?: MenuLeader[];
}): { markup: string; enriched: EnrichedAchievement[] } {
    const rawAchievements = resolveProfileAchievements(opts.data);
    const enriched = enrichAchievementsList(rawAchievements, opts.isOwnProfile);
    const unlockedCount = enriched.filter((a) => a.unlocked).length;
    const totalCount = enriched.length;
    const percent = totalCount > 0 ? Math.round((unlockedCount / totalCount) * 100) : 0;
    const laurels = profileStat(opts.data.laurels);

    const activeCategory = normalizeWowCategory(opts.state.category);
    const activeSub = opts.state.subcategory || "all";
    const statusFilter = opts.state.status || "all";
    const tierFilter = opts.state.tier || "all";
    const sortBy = opts.state.sort || "smart";
    const playableOnly = Boolean(opts.state.playableOnly) && opts.isOwnProfile;
    const viewMode = opts.state.viewMode === "grid" ? "grid" : "chains";

    const summaryDossier =
        "<div class='sow-profile__achievements-dossier'>" +
        "<div class='sow-profile__achievements-laurel-card'>" +
        "<div class='sow-profile__achievements-laurel-icon'><img src='" +
        esc(currencyAssetUrl("laurel")) +
        "' alt='' width='40' height='40' /></div>" +
        "<div class='sow-profile__achievements-laurel-info'>" +
        "<span class='sow-profile__achievements-laurel-title'>" +
        esc(tSafe("profile.laurels", "Laurels")) +
        "</span><strong class='sow-profile__achievements-laurel-val'>" +
        esc(laurels) +
        "</strong></div></div>" +
        "<div class='sow-profile__achievements-progress-card'>" +
        "<div class='sow-profile__achievements-progress-header'><span>" +
        esc(tSafe("profile.achievements", "Achievements")) +
        "</span><strong>" +
        esc(unlockedCount + " / " + totalCount + " (" + percent + "%)") +
        "</strong></div><div class='sow-profile__achievements-progress-track' role='progressbar' aria-valuenow='" +
        esc(percent) +
        "' aria-valuemin='0' aria-valuemax='100'><i style='width:" +
        esc(percent) +
        "%'></i></div></div></div>";

    const categoryOptions = (["summary", ...WOW_PILLAR_ORDER, "all"] as WowCategoryId[]).map(
        function (cat) {
            const c = achPillarCounts(enriched, cat, opts.isOwnProfile, playableOnly);
            return {
                value: cat,
                label: wowCategoryTitle(cat) + " (" + c.unlocked + "/" + c.total + ")",
                shortLabel: wowCategoryTitle(cat),
            };
        },
    );

    const toolbar =
        "<div class='sow-profile__ach-toolbar' data-ach-toolbar>" +
        "<div class='sow-profile__ach-search-wrap sow-control-search'>" +
        "<span class='sow-profile__ach-search-icon sow-control-search__icon' aria-hidden='true'><svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='11' cy='11' r='8'/><line x1='21' y1='21' x2='16.65' y2='16.65'/></svg></span>" +
        "<input type='search' class='sow-profile__ach-search-input sow-control-search__input' placeholder='" +
        esc(tSafe("profile.search_achievements", "Search achievements, heroes, or regions…")) +
        "' value='" +
        esc(opts.state.search || "") +
        "' data-role='achievements-search' autocomplete='off' spellcheck='false' /></div>" +
        "<div class='sow-profile__ach-controls'>" +
        renderCodexDropdown({
            key: "ach-category",
            label: tSafe("profile.ach_categories_label", "Achievement categories"),
            name: "ach_category",
            value: activeCategory,
            className: "sow-profile__ach-mobile-cat",
            options: categoryOptions,
        }) +
        renderCodexDropdown({
            key: "ach-status",
            label: tSafe("profile.ach_filter_status", "Filter by progress"),
            name: "ach_status",
            value: statusFilter,
            className: "sow-profile__ach-dd" + (statusFilter !== "all" ? " is-filtered" : ""),
            options: [
                { value: "all", label: tSafe("profile.ach_status_all", "All States") },
                { value: "in_progress", label: tSafe("profile.ach_status_in_progress", "In Progress") },
                { value: "unlocked", label: tSafe("profile.ach_status_unlocked", "Completed") },
                { value: "locked", label: tSafe("profile.ach_status_locked", "Not Started") },
            ],
        }) +
        renderCodexDropdown({
            key: "ach-tier",
            label: tSafe("profile.ach_filter_tier", "Filter by tier"),
            name: "ach_tier",
            value: tierFilter,
            className: "sow-profile__ach-dd" + (tierFilter !== "all" ? " is-filtered" : ""),
            options: [
                { value: "all", label: tSafe("profile.ach_tier_all", "All Tiers") },
                { value: "mythic", label: tSafe("profile.ach_tier_mythic", "Mythic (200+)") },
                { value: "elite", label: tSafe("profile.ach_tier_elite", "Elite (50–199)") },
                { value: "standard", label: tSafe("profile.ach_tier_standard", "Standard (<50)") },
            ],
        }) +
        renderCodexDropdown({
            key: "ach-sort",
            label: tSafe("profile.ach_sort_label", "Sort achievements"),
            name: "ach_sort",
            value: sortBy,
            className: "sow-profile__ach-dd",
            options: [
                { value: "smart", label: tSafe("profile.ach_sort_smart", "Smart Order") },
                { value: "closest", label: tSafe("profile.ach_sort_closest", "Closest (%)") },
                { value: "points_desc", label: tSafe("profile.ach_sort_points", "Highest Laurels") },
                { value: "alpha_asc", label: tSafe("profile.ach_sort_alpha", "A-Z") },
            ],
        }) +
        (opts.isOwnProfile
            ? "<button type='button' class='sow-heroes__playable-btn sow-profile__ach-playable" +
              (playableOnly ? " is-active" : "") +
              "' data-command='profile_achievement_toggle_playable' aria-pressed='" +
              String(playableOnly) +
              "'>" +
              esc(tSafe("profile.ach_playable_only", "Owned / Playable")) +
              "</button>"
            : "") +
        "<div class='sow-profile__ach-view-toggle' role='group'>" +
        "<button type='button' class='sow-profile__ach-view-btn" +
        (viewMode === "chains" ? " is-active" : "") +
        "' data-command='profile_achievement_view_mode' data-view-mode='chains' aria-pressed='" +
        String(viewMode === "chains") +
        "'>" +
        esc(tSafe("profile.ach_view_chains", "Chains")) +
        "</button>" +
        "<button type='button' class='sow-profile__ach-view-btn" +
        (viewMode === "grid" ? " is-active" : "") +
        "' data-command='profile_achievement_view_mode' data-view-mode='grid' aria-pressed='" +
        String(viewMode === "grid") +
        "'>" +
        esc(tSafe("profile.ach_view_cards", "All Cards")) +
        "</button>" +
        "</div></div></div>";

    const subbarHtml = renderAchSubbarMarkup(
        enriched,
        activeCategory,
        activeSub,
        opts.isOwnProfile,
        playableOnly,
    );

    const markup =
        summaryDossier +
        "<div class='sow-profile__ach-layout'><aside class='sow-profile__ach-rail' data-ach-rail aria-label='" +
        esc(tSafe("profile.ach_categories_label", "Achievement categories")) +
        "'>" +
        renderAchRailMarkup(enriched, activeCategory, activeSub, opts.isOwnProfile, playableOnly) +
        "</aside><div class='sow-profile__ach-main'>" +
        toolbar +
        "<div class='sow-profile__ach-subbar' data-ach-subbar" +
        (subbarHtml ? "" : " hidden") +
        ">" +
        subbarHtml +
        "</div><div class='sow-profile__ach-results' data-ach-results aria-live='polite'>" +
        renderAchResultsMarkup(enriched, opts.isOwnProfile, opts.state, opts.catalog) +
        "</div></div></div>";

    return { markup, enriched };
}

/** Renders the 5-card Stats Ribbon (Matches, Wins, Win Rate, Laurels, K/D/A). */
export function renderProfileStatsRibbon(data: ProfileData | null): string {
    if (!data) return "";
    const matches = profileStat(data.matches_played);
    const wins = profileStat(data.wins);
    const winRate = matches ? Math.round((wins / matches) * 100) : 0;
    const laurels = profileStat(data.laurels);
    return (
        "<div class='sow-profile__stats' data-profile-stats>" +
        "<div class='sow-profile__stat-cell sow-profile__stat-matches'>" +
        "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M14.5 17.5L3 6V3h3l11.5 11.5'/><path d='m13 19 6-6'/><path d='M16 16l4 4'/><path d='M19 21l2-2'/></svg></span>" +
        "<div class='sow-profile__stat-body'><strong>" +
        esc(matches) +
        "</strong><span>" +
        esc(tSafe("profile.matches", "Matches")) +
        "</span></div></div>" +
        "<div class='sow-profile__stat-cell sow-profile__stat-wins'>" +
        "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M6 9H4.5a2.5 2.5 0 0 1 0-5H6'/><path d='M18 9h1.5a2.5 2.5 0 0 0 0-5H18'/><path d='M4 22h16'/><path d='M10 14.66V17c0 .55-.45 1-1 1H8c-.55 0-1 .45-1 1v1h10v-1c0-.55-.45-1-1-1h-1c-.55 0-1-.45-1-1v-2.34'/><path d='M18 4H6v7a6 6 0 0 0 12 0V4Z'/></svg></span>" +
        "<div class='sow-profile__stat-body'><strong>" +
        esc(wins) +
        "</strong><span>" +
        esc(tSafe("profile.wins", "Wins")) +
        "</span></div></div>" +
        "<div class='sow-profile__stat-cell sow-profile__stat-winrate'>" +
        "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><path d='m4.93 4.93 4.24 4.24'/><path d='m14.83 9.17 4.24-4.24'/><path d='m14.83 14.83 4.24 4.24'/><path d='m9.17 14.83-4.24 4.24'/><circle cx='12' cy='12' r='4'/></svg></span>" +
        "<div class='sow-profile__stat-body'><strong>" +
        esc(winRate) +
        "%</strong><span>" +
        esc(tSafe("profile.win_rate", "Win Rate")) +
        "</span><div class='sow-profile__stat-gauge' aria-hidden='true'><i style='width:" +
        winRate +
        "%'></i></div></div></div>" +
        "<div class='sow-profile__stat-cell sow-profile__stat-laurels'>" +
        "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M8 3a9 9 0 0 0-5 8c0 4.97 3.58 9 8 9M16 3a9 9 0 0 1 5 8c0 4.97-3.58 9-8 9M9 7l3 3M15 7l-3 3M8 12l4 2M16 12l-4 2'/></svg></span>" +
        "<div class='sow-profile__stat-body'><strong>" +
        esc(laurels) +
        "</strong><span>" +
        esc(tSafe("profile.laurels", "Laurels")) +
        "</span></div></div>" +
        "<div class='sow-profile__stat-cell sow-profile__stat-kda'>" +
        "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='22' y1='12' x2='18' y2='12'/><line x1='6' y1='12' x2='2' y2='12'/><line x1='12' y1='6' x2='12' y2='2'/><line x1='12' y1='22' x2='12' y2='18'/></svg></span>" +
        "<div class='sow-profile__stat-body'><strong><i>" +
        esc(data.kills || 0) +
        "</i><i>" +
        esc(data.deaths || 0) +
        "</i><i>" +
        esc(data.assists || 0) +
        "</i></strong><span>" +
        esc(tSafe("profile.kda", "K/D/A")) +
        "</span></div></div></div>"
    );
}

/** Renders the profile identity banner with dual 1v1 & FFA rank badges, Share Profile button, and stats ribbon. */
export function renderProfileHeader(opts: {
    data: ProfileData | null;
    own: boolean;
    loading: boolean;
    snapshotError: string;
    shareCopied: boolean;
    showShare?: boolean;
    extraTopActions?: string;
    avatarFallbackUrl?: string;
}): string {
    const data = opts.data;
    const title = opts.own
        ? tSafe("profile.your_profile", "Your Profile")
        : tSafe("profile.player_profile", "Player Profile");
    const displayName =
        data && data.display_name
            ? data.display_name
            : opts.loading
              ? tSafe("profile.loading_profile", "Loading profile...")
              : tSafe("profile.profile_unavailable", "Profile unavailable");
    const handle =
        data && data.handle
            ? data.handle
            : opts.loading
              ? tSafe("profile.fetching_player_data", "Fetching player dossier...")
              : "";
    const level = data && data.level != null ? data.level : "—";
    const status =
        data && opts.snapshotError
            ? "<div class='sow-profile__status' data-profile-status aria-live='polite'>" +
              esc(opts.snapshotError) +
              " <button type='button' class='sow-profile__status-action' data-command='retry_profile'>" +
              esc(tSafe("profile.try_again", "Try again")) +
              "</button></div>"
            : "<div class='sow-profile__status' data-profile-status aria-live='polite' hidden></div>";

    const preferred = data && data.preferred_leader ? leaderById(data.preferred_leader) : null;
    const avatarUrl = preferred
        ? assetUrl("gameplay/avatars/" + leaderArtSlug(preferred) + ".webp")
        : opts.avatarFallbackUrl || leaderAvatarUrl();

    const r1v1 = (data?.ratings || []).find((r) => r.queue === "Ranked1v1" && r.mode === "FFA");
    const rFfa = (data?.ratings || []).find((r) => r.queue === "RankedFFA" && r.mode === "FFA");
    const rankPills = [
        r1v1 ? { rating: r1v1, label: tSafe("lobbies.ranked_1v1", "Ranked 1v1") } : null,
        rFfa ? { rating: rFfa, label: tSafe("lobbies.ranked_ffa", "Ranked FFA") } : null,
    ]
        .filter((item): item is { rating: ProfileRating; label: string } => item !== null)
        .map(({ rating, label }) => {
            const tooltip = label + " · " + rankLabel(rating);
            return (
                "<span class='sow-profile__rank-badge' data-tier='" +
                rankTier(rating.tier) +
                "' tabindex='0' data-sow-tooltip='" +
                esc(tooltip) +
                "' data-sow-tooltip-icon='ranked' data-sow-tooltip-tone='gold' data-sow-tooltip-tap='true'>" +
                rankEmblem(rating.tier, rating.division) +
                "<small>" +
                esc(label) +
                "</small><b>" +
                esc(rankLabel(rating)) +
                "</b></span>"
            );
        })
        .join("");

    const shareButton =
        opts.showShare !== false && data && isValidAccountId(data.account_id)
            ? "<div class='sow-profile__share-wrap'>" +
              "<button type='button' class='sow-profile__share-btn' data-command='share_profile' data-account-id='" +
              esc(data.account_id) +
              "'><svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71'/><path d='M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71'/></svg><span>" +
              esc(tSafe("site.share_profile", "Share Profile")) +
              "</span></button>" +
              (opts.shareCopied
                  ? "<span class='sow-profile__toast' role='status'>" +
                    esc(tSafe("site.profile_link_copied", "Link copied")) +
                    "</span>"
                  : "") +
              "</div>"
            : "";

    const heroArt = profileHeroBackdropUrl(data && data.preferred_leader);
    return (
        "<section class='sow-profile__heading' data-profile-heading data-profile-cover aria-labelledby='sow-profile-title' style=\"--sow-hero:url('" +
        esc(heroArt) +
        "')\">" +
        "<div class='sow-profile__cover-overlay'></div>" +
        "<div class='sow-profile__heading-top'>" +
        "<div class='sow-profile__breadcrumbs'><span class='sow-profile__kicker'>" +
        esc(title) +
        "</span>" +
        (!opts.own
            ? "<span class='sow-profile__viewing-tag'>· " + esc(displayName) + "</span>"
            : "") +
        "</div><div class='sow-profile__heading-actions'>" +
        shareButton +
        (opts.extraTopActions || "") +
        "</div></div>" +
        "<div class='sow-profile__identity-card'>" +
        "<div class='sow-profile__avatar-container'><div class='sow-profile__avatar-medallion'>" +
        "<img class='sow-profile__avatar' data-profile-avatar " +
        avatarImageAttribute(avatarUrl) +
        " alt='' width='88' height='88'></div>" +
        "<div class='sow-profile__level' tabindex='0' data-sow-tooltip='" +
        esc(tSafe("profile.level_label", "Level") + " " + level) +
        "' data-sow-tooltip-tone='gold' data-sow-tooltip-tap='true'><small>" +
        esc(tSafe("profile.level_label", "Level")) +
        "</small><strong data-profile-level>" +
        esc(level) +
        "</strong></div></div>" +
        "<div class='sow-profile__identity-main'><div class='sow-profile__identity-copy'>" +
        "<div class='sow-profile__identity-title-row'><h1 id='sow-profile-title' data-profile-display-name>" +
        esc(displayName) +
        "</h1>" +
        (rankPills ? "<div class='sow-profile__rank-badges'>" + rankPills + "</div>" : "") +
        "</div><p class='sow-profile__handle' data-profile-handle>" +
        (handle ? "@" + esc(handle.replace(/^@/, "")) : "") +
        "</p>" +
        status +
        "</div></div></div>" +
        renderProfileStatsRibbon(data) +
        "</section>"
    );
}

/** Renders one Commander Mastery Card. */
export function renderProfileLeaderCard(leader: ProfileLeader): string {
    const summary = leader || ({} as ProfileLeader);
    const leaderInfo = leaderById(summary.leader);
    const winRate = Math.round((summary.win_rate || 0) * 100);
    const level = levelFromXp(summary.xp || 0);
    return (
        "<article class='sow-profile__leader'>" +
        "<div class='sow-profile__leader-avatar-frame'><img " +
        avatarImageAttribute(assetUrl("gameplay/avatars/" + leaderArtSlug(leaderInfo) + ".webp")) +
        " alt='' width='52' height='52' loading='lazy'><span class='sow-profile__leader-badge'>" +
        esc(level) +
        "</span></div>" +
        "<div class='sow-profile__leader-info'><div class='sow-profile__leader-header'><strong>" +
        esc(leaderDisplayName(leaderInfo)) +
        "</strong><b>" +
        esc(tSafe("profile.level", "Level {level}", { level })) +
        "</b></div><div class='sow-profile__leader-track' aria-hidden='true'><i style='width:" +
        winRate +
        "%'></i></div><span>" +
        esc(
            tSafe("profile.commander_stats", "{matches} matches · {wins} wins ({rate}%)", {
                matches: summary.matches_played || 0,
                wins: summary.wins || 0,
                rate: winRate,
            }),
        ) +
        "</span></div></article>"
    );
}

/** Renders one clickable Match History row. */
export function renderProfileMatchRow(match: ProfileMatch): string {
    const verified = !!match.verified;
    const result = verified
        ? match.queue === "RankedFFA" && typeof match.placement === "number" && match.placement > 0
            ? "#" + match.placement + " · " + tSafe(match.won ? "profile.win" : "profile.loss", match.won ? "Victory" : "Defeat")
            : match.won
              ? tSafe("profile.win", "Victory")
              : tSafe("profile.loss", "Defeat")
        : tSafe(match.provisional ? "profile.match_pending" : "profile.match_unverified", match.provisional ? "Pending" : "Unverified");
    const mode = match.mode || tSafe("profile.ffa", "FFA");
    const map = formatMapName(match);
    const leader = match.leader ? leaderById(match.leader) : null;
    const context = profileMatchDate(match.completed_at) + " · " + profileMatchDuration(match.duration_seconds);
    const statusModifier = verified ? (match.won ? "is-win" : "is-loss") : "is-pending";
    const deltaText =
        match.rating_delta == null
            ? ""
            : match.ranked_points_breakdown
              ? tSafe("lobbies.rank_score", "{score} pts", {
                    score: (match.rating_delta > 0 ? "+" : "") + match.rating_delta,
                })
              : (match.rating_delta > 0 ? "+" : "") + String(match.rating_delta) + " MMR";
    const deltaMarkup =
        match.rating_delta == null
            ? ""
            : "<span class='sow-profile__match-delta " +
              (match.rating_delta > 0 ? "is-positive" : match.rating_delta < 0 ? "is-negative" : "") +
              "'>" +
              esc(deltaText) +
              "</span>";

    return (
        "<button type='button' class='sow-profile__match " +
        statusModifier +
        "' data-command='open_match' data-match-id='" +
        esc(match.match_id) +
        "'><span class='sow-profile__match-blade' aria-hidden='true'></span>" +
        "<span class='sow-profile__match-result " +
        statusModifier +
        "'>" +
        esc(result) +
        "</span>" +
        (leader
            ? "<div class='sow-profile__match-avatar-frame'><img class='sow-profile__match-avatar' " +
              avatarImageAttribute(assetUrl("gameplay/avatars/" + leaderArtSlug(leader) + ".webp")) +
              " alt='' width='44' height='44' loading='lazy'></div>"
            : "<span class='sow-profile__match-avatar' aria-hidden='true'></span>") +
        "<span class='sow-profile__match-context'><strong>" +
        esc(mode) +
        "</strong><small>" +
        esc(map) +
        " · " +
        esc(match.queue || tSafe("profile.matchmaking_queue", "Matchmaking")) +
        (deltaText ? " · " + esc(deltaText) : "") +
        "</small></span><span class='sow-profile__match-kda'><strong>" +
        esc(leader ? leaderDisplayName(leader) : "—") +
        "</strong><small>" +
        (verified
            ? esc(
                  tSafe("profile.kda_value", "{kills}/{deaths}/{assists}", {
                      kills: match.kills || 0,
                      deaths: match.deaths || 0,
                      assists: match.assists || 0,
                  }) +
                      " " +
                      tSafe("profile.kda", "K/D/A"),
              )
            : "") +
        "</small></span>" +
        deltaMarkup +
        "<span class='sow-profile__match-time'>" +
        esc(context) +
        "</span><span class='sow-profile__match-chevron' aria-hidden='true'>›</span></button>"
    );
}

/** Filters match history list by queue chip. */
export function filterMatchesByQueue(matches: ProfileMatch[], queueFilter: string): ProfileMatch[] {
    if (!queueFilter || queueFilter === "all") return matches;
    if (queueFilter === "Custom") {
        return matches.filter((m) => m.queue !== "Ranked1v1" && m.queue !== "RankedFFA" && m.queue !== "Matchmaking");
    }
    return matches.filter((m) => m.queue === queueFilter);
}

/** Renders the queue filter pills inside Match History. */
export function renderHistoryQueueFilterPills(activeQueue: string): string {
    const filters: [string, string][] = [
        ["all", tSafe("site.filter_all_queues", "All Queues")],
        ["Ranked1v1", tSafe("lobbies.ranked_1v1", "Ranked 1v1")],
        ["RankedFFA", tSafe("lobbies.ranked_ffa", "Ranked FFA")],
        ["Matchmaking", tSafe("profile.matchmaking_queue", "Matchmaking")],
        ["Custom", tSafe("site.queue_custom", "Custom")],
    ];
    return (
        "<div class='sow-profile__history-filters' role='group' aria-label='" +
        esc(tSafe("site.filter_queue", "Filter by Queue")) +
        "'>" +
        filters
            .map(([key, label]) => {
                const active = activeQueue === key;
                return (
                    "<button type='button' class='sow-profile__filter-pill" +
                    (active ? " is-active" : "") +
                    "' data-command='profile_history_queue' data-history-queue='" +
                    esc(key) +
                    "' aria-pressed='" +
                    (active ? "true" : "false") +
                    "'>" +
                    esc(label) +
                    "</button>"
                );
            })
            .join("") +
        "</div>"
    );
}

/** Renders the Match Detail modal dialog shared between in-game overlay and public site. */
export function renderProfileMatchDetailModal(opts: {
    detail: ProfileMatchDetail | null;
    detailId: string | null;
    detailError: string;
    mode: "site" | "game";
}): string {
    const detail = opts.detail;
    if (!detail) {
        if (!opts.detailError) return "";
        return (
            "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='" +
            esc(tSafe("profile.close_match_details", "Close match details")) +
            "'>×</button><span class='sow-profile__kicker'>" +
            esc(tSafe("profile.match_details", "Match Details")) +
            "</span><h2 id='sow-profile-detail-title'>" +
            esc(tSafe("profile.unavailable", "Unavailable")) +
            "</h2><p class='sow-profile__empty' aria-live='polite'>" +
            esc(opts.detailError) +
            "</p><button type='button' class='sow-profile__load-more' data-command='open_match' data-match-id='" +
            esc(opts.detailId) +
            "'>" +
            esc(tSafe("profile.try_again", "Try again")) +
            "</button></section></div>"
        );
    }

    return (
        "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='" +
        esc(tSafe("profile.close_match_details", "Close match details")) +
        "'>×</button><span class='sow-profile__kicker'>" +
        esc(tSafe("profile.match_details", "Match Details")) +
        "</span><h2 id='sow-profile-detail-title'>" +
        esc(detail.mode || tSafe("profile.match", "Match")) +
        "</h2><p>" +
        esc(formatMapName(detail)) +
        " · " +
        esc(detail.queue || tSafe("profile.matchmaking_queue", "Matchmaking")) +
        " · " +
        esc(profileMatchDate(detail.completed_at)) +
        " · " +
        esc(profileMatchDuration(detail.duration_seconds)) +
        "</p><div class='sow-profile__detail-players'>" +
        (detail.participants || [])
            .map(function (participant) {
                const status = detail.verified
                    ? participant.won
                        ? tSafe("profile.win", "Victory")
                        : tSafe("profile.loss", "Defeat")
                    : tSafe(
                          detail.provisional ? "profile.match_pending" : "profile.match_unverified",
                          detail.provisional ? "Pending" : "Unverified",
                      );
                const stats = detail.verified
                    ? tSafe("profile.kda_value", "{kills}/{deaths}/{assists}", {
                          kills: participant.kills || 0,
                          deaths: participant.deaths || 0,
                          assists: participant.assists || 0,
                      })
                    : "";
                const clickable = isValidAccountId(participant.account_id);
                const name = clickable
                    ? opts.mode === "site"
                        ? "<a class='sow-profile__player-profile' href='" +
                          esc(publicProfileUrl(participant.account_id)) +
                          "' data-account-id='" +
                          esc(participant.account_id) +
                          "'>" +
                          esc(participant.handle || participant.account_id) +
                          "</a>"
                        : "<button type='button' class='sow-profile__player-profile' data-command='open_profile' data-account-id='" +
                          esc(participant.account_id) +
                          "'>" +
                          esc(participant.handle || participant.account_id) +
                          "</button>"
                    : "<strong>" + esc(participant.handle || participant.account_id) + "</strong>";

                const deltaPart =
                    participant.rating_delta == null
                        ? ""
                        : " · " +
                          (participant.ranked_points_breakdown
                              ? esc(
                                    tSafe("lobbies.rank_score", "{score} pts", {
                                        score:
                                            (participant.rating_delta > 0 ? "+" : "") +
                                            participant.rating_delta,
                                    }),
                                )
                              : (participant.rating_delta > 0 ? "+" : "") +
                                esc(participant.rating_delta) +
                                " MMR");

                return (
                    "<div>" +
                    name +
                    "<span>" +
                    (detail.queue === "RankedFFA" &&
                    typeof participant.placement === "number" &&
                    participant.placement > 0
                        ? "#" + esc(participant.placement) + " · "
                        : "") +
                    esc(
                        participant.leader
                            ? leaderDisplayName(leaderById(participant.leader))
                            : "—",
                    ) +
                    (stats ? " · " + esc(stats) : "") +
                    deltaPart +
                    "</span>" +
                    (detail.queue === "Ranked1v1" && participant.ranked_points_breakdown
                        ? rankedPointsBreakdownMarkup(participant.ranked_points_breakdown)
                        : "") +
                    "<b>" +
                    esc(status) +
                    "</b></div>"
                );
            })
            .join("") +
        "</div></section></div>"
    );
}

export async function fetchPublicProfile(
    accountId: string,
    fetchImpl?: typeof fetch,
): Promise<ProfileData> {
    const fn = fetchImpl || fetch;
    const response = await fn(profileApiUrl("/profiles/" + encodeURIComponent(accountId)), {
        headers: { Accept: "application/json" },
    });
    if (!response.ok) throw new Error("profile request failed: " + response.status);
    return (await response.json()) as ProfileData;
}

export async function fetchPublicHistory(
    accountId: string,
    opts: { cursor?: number; limit?: number; queue?: string },
    fetchImpl?: typeof fetch,
): Promise<ProfilePage<ProfileMatch>> {
    const fn = fetchImpl || fetch;
    const params = new URLSearchParams();
    params.set("cursor", String(opts.cursor || 0));
    params.set("limit", String(opts.limit || 20));
    if (opts.queue && opts.queue !== "all" && opts.queue !== "Custom") {
        params.set("queue", opts.queue);
    }
    const response = await fn(
        profileApiUrl("/profiles/" + encodeURIComponent(accountId) + "/matches?" + params.toString()),
        { headers: { Accept: "application/json" } },
    );
    if (!response.ok) throw new Error("history request failed: " + response.status);
    return (await response.json()) as ProfilePage<ProfileMatch>;
}

export async function fetchPublicMatchDetail(
    matchId: string,
    fetchImpl?: typeof fetch,
): Promise<ProfileMatchDetail> {
    const fn = fetchImpl || fetch;
    const response = await fn(profileApiUrl("/matches/" + encodeURIComponent(matchId)), {
        headers: { Accept: "application/json" },
    });
    if (!response.ok) throw new Error("match detail failed: " + response.status);
    return (await response.json()) as ProfileMatchDetail;
}

export async function searchPublicProfiles(
    query: string,
    limit = 12,
    fetchImpl?: typeof fetch,
    signal?: AbortSignal,
): Promise<ProfileSearchItem[]> {
    const fn = fetchImpl || fetch;
    const params = new URLSearchParams({
        q: query.trim().slice(0, 48),
        limit: String(limit),
    });
    const response = await fn(profileApiUrl("/profiles/search?" + params.toString()), {
        headers: { Accept: "application/json" },
        signal,
    });
    if (!response.ok) throw new Error("profile search failed: " + response.status);
    const payload = (await response.json()) as ProfilePage<ProfileSearchItem>;
    return Array.isArray(payload.items) ? payload.items : [];
}

export interface PublicProfileControllerOptions {
    accountId: string;
    initialTab?: "overview" | "leaders" | "history" | "achievements";
    onNavigateProfile?: (accountId: string) => void;
    onProfileLoaded?: (profile: ProfileData) => void;
    fetchImpl?: typeof fetch;
}

export interface PublicProfileController {
    setAccountId(accountId: string): void;
    rerender(): void;
    destroy(): void;
}

/** Mounts a full standalone guest profile dossier on /players/<account_id>. */
export function createProfileController(
    container: HTMLElement,
    options: PublicProfileControllerOptions,
): PublicProfileController {
    const fetchFn: typeof fetch | null =
        options.fetchImpl || (typeof fetch === "function" ? fetch.bind(globalThis) : null);

    let accountId = options.accountId;
    let profile: ProfileData | null = null;
    let loading = true;
    let error = "";
    let activeTab: "overview" | "leaders" | "history" | "achievements" = options.initialTab || "overview";
    let activeRankedQueue: "Ranked1v1" | "RankedFFA" = "Ranked1v1";
    let history: ProfileMatch[] = [];
    let historyCursor = 0;
    let historyHasMore = false;
    let historyLoading = false;
    let historyError = "";
    let historyQueueFilter = "all";
    let detail: ProfileMatchDetail | null = null;
    let detailId: string | null = null;
    let detailError = "";
    let achState = defaultAchievementCodexState();
    let achOpenDropdownKey: string | null = null;
    let shareCopied = false;
    let shareTimer: ReturnType<typeof setTimeout> | null = null;
    let loadSerial = 0;
    let destroyed = false;

    function hydrateDynamicAvatars(): void {
        const avatar = container.querySelector<HTMLImageElement>("[data-profile-avatar]");
        if (avatar && profile?.preferred_leader) {
            setAvatarImage(avatar, leaderAvatarUrl(profile.preferred_leader));
        }
        bindPointerSpotlight(container.querySelector<HTMLElement>("[data-ranked-overview]"));
    }

    function render(): void {
        if (destroyed) return;
        const backToBoards =
            "<a class='sow-profile__back' href='" +
            esc(publicLeaderboardsUrl()) +
            "'>" +
            esc(tSafe("profile.leaderboards", "Leaderboards")) +
            "</a>";

        const header = renderProfileHeader({
            data: profile,
            own: false,
            loading,
            snapshotError: error,
            shareCopied,
            showShare: true,
            extraTopActions: backToBoards,
        });

        const tabs = (["overview", "leaders", "history", "achievements"] as const)
            .map((tab) => {
                const active = activeTab === tab;
                const label = tSafe(
                    "profile.tab_" + (tab === "leaders" ? "commanders" : tab),
                    tab,
                );
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

        let content = "";
        if (!profile) {
            content = loading
                ? "<div class='sow-skeleton-list' style='margin-top:16px'>" +
                  Array.from({ length: 4 }, () => "<div class='sow-skeleton'></div>").join("") +
                  "</div>"
                : "<section class='sow-profile__state' aria-live='polite'><strong>" +
                  esc(tSafe("profile.profile_unavailable", "Profile unavailable")) +
                  "</strong><span>" +
                  esc(error || tSafe("site.player_not_found", "Could not load this player profile.")) +
                  "</span><div><button type='button' class='sow-profile__load-more' data-command='retry_profile'>" +
                  esc(tSafe("profile.try_again", "Try again")) +
                  "</button></div></section>";
        } else if (activeTab === "overview") {
            const r1v1 = (profile.ratings || []).find((r) => r.queue === "Ranked1v1" && r.mode === "FFA") || null;
            const rFfa = (profile.ratings || []).find((r) => r.queue === "RankedFFA" && r.mode === "FFA") || null;
            const rankedPanel = renderRankedOverview({
                rating1v1: r1v1,
                ratingFfa: rFfa,
                ladder: profile.ranked_ladder,
                own: false,
                mode: "site",
                activeQueue: activeRankedQueue,
            });
            const recentHtml =
                history.slice(0, 10).map(renderProfileMatchRow).join("") ||
                "<p class='sow-profile__empty'>" +
                    esc(tSafe("profile.no_completed_matches", "No completed matches recorded yet.")) +
                    "</p>";
            const topLeaders =
                profile.leaders && profile.leaders.length
                    ? "<section class='sow-profile__section sow-profile__section--widget'><div class='sow-profile__section-head'><h3>" +
                      esc(tSafe("profile.most_played_leaders", "Most Played Commanders")) +
                      "</h3><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='leaders'>" +
                      esc(tSafe("profile.view_all", "View all")) +
                      "</button></div><div class='sow-profile__top-leaders'>" +
                      profile.leaders.slice(0, 3).map(renderProfileLeaderCard).join("") +
                      "</div></section>"
                    : "";
            content =
                "<div class='sow-profile__overview'><div class='sow-profile__overview-feed'><section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
                esc(tSafe("profile.recent_matches", "Recent Matches")) +
                "</h2><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='history'>" +
                esc(tSafe("profile.view_all", "View all")) +
                "</button></div>" +
                recentHtml +
                "</section></div><aside class='sow-profile__overview-sidebar'>" +
                rankedPanel +
                topLeaders +
                "</aside></div>";
        } else if (activeTab === "leaders") {
            const leaderList =
                (profile.leaders || []).map(renderProfileLeaderCard).join("") ||
                "<p class='sow-profile__empty'>" +
                    esc(tSafe("profile.no_leader_history", "No commander history recorded yet.")) +
                    "</p>";
            content =
                "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
                esc(tSafe("profile.tab_commanders", "Commanders")) +
                "</h2><span class='sow-profile__section-note'>" +
                esc(tSafe("profile.leader_count", "{count} commanders", { count: (profile.leaders || []).length })) +
                "</span></div><div class='sow-profile__leaders'>" +
                leaderList +
                "</div></section>";
        } else if (activeTab === "history") {
            const filtered = filterMatchesByQueue(history, historyQueueFilter);
            const rows =
                filtered.map(renderProfileMatchRow).join("") ||
                "<p class='sow-profile__empty'>" +
                    esc(tSafe("profile.no_completed_matches", "No completed matches found.")) +
                    "</p>";
            const moreBtn =
                historyHasMore || historyError
                    ? "<button type='button' class='sow-profile__load-more' data-command='load_profile_more'" +
                      (historyLoading ? " disabled" : "") +
                      ">" +
                      esc(
                          historyLoading
                              ? tSafe("profile.loading_more", "Loading more...")
                              : historyError
                                ? tSafe("profile.try_again", "Try again")
                                : tSafe("profile.load_more_matches", "Load more matches"),
                      ) +
                      "</button>"
                    : "";
            content =
                "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
                esc(tSafe("profile.match_history", "Match History")) +
                "</h2><span class='sow-profile__section-note'>" +
                esc(tSafe("profile.match_count", "{count} matches", { count: profile.matches_played || 0 })) +
                "</span></div>" +
                renderHistoryQueueFilterPills(historyQueueFilter) +
                "<div class='sow-profile__history'>" +
                rows +
                "</div>" +
                moreBtn +
                "</section>";
        } else if (activeTab === "achievements") {
            const codex = renderProfileAchievementsCodex({
                data: profile,
                isOwnProfile: false,
                state: achState,
            });
            content =
                "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
                esc(tSafe("profile.achievements", "Achievements")) +
                "</h2><span class='sow-profile__section-note'>" +
                esc((profile.laurels || 0) + " " + tSafe("profile.laurels", "Laurels")) +
                "</span></div>" +
                codex.markup +
                "</section>";
        }

        const modalHtml = renderProfileMatchDetailModal({
            detail,
            detailId,
            detailError,
            mode: "site",
        });

        container.innerHTML =
            "<div class='sow-profile__page'><div class='sow-profile__dossier'>" +
            header +
            "<nav class='sow-profile__tabs' role='tablist' aria-label='" +
            esc(tSafe("profile.profile_sections", "Profile sections")) +
            "'>" +
            tabs +
            "</nav><div data-profile-content class='sow-profile__content'><div id='sow-profile-panel-active' class='sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-" +
            activeTab +
            "'>" +
            content +
            "</div></div></div></div>" +
            modalHtml;

        if (activeTab === "achievements") {
            syncDropdownScope(container, achOpenDropdownKey);
        }
        hydrateDynamicAvatars();
    }

    async function loadCurrentProfile(): Promise<void> {
        if (!fetchFn || destroyed) return;
        const serial = ++loadSerial;
        loading = true;
        error = "";
        profile = null;
        history = [];
        detail = null;
        detailError = "";
        detailId = null;
        render();
        try {
            const data = await fetchPublicProfile(accountId, fetchFn);
            if (serial !== loadSerial || destroyed) return;
            profile = data;
            history = Array.isArray(data.recent_matches) ? data.recent_matches.slice() : [];
            historyCursor = history.length;
            historyHasMore = profileStat(data.matches_played) > history.length;
            const r1v1 = (data.ratings || []).find((r) => r.queue === "Ranked1v1");
            const rFfa = (data.ratings || []).find((r) => r.queue === "RankedFFA");
            if ((!r1v1 || r1v1.games_played === 0) && rFfa && rFfa.games_played > 0) {
                activeRankedQueue = "RankedFFA";
            } else {
                activeRankedQueue = "Ranked1v1";
            }
            options.onProfileLoaded?.(data);
        } catch {
            if (serial !== loadSerial || destroyed) return;
            error = tSafe("site.player_not_found", "Could not load this player profile.");
        } finally {
            if (serial === loadSerial && !destroyed) {
                loading = false;
                render();
            }
        }
    }

    async function loadMoreHistory(): Promise<void> {
        if (!fetchFn || !profile || historyLoading) return;
        historyLoading = true;
        historyError = "";
        render();
        try {
            const page = await fetchPublicHistory(
                profile.account_id,
                { cursor: historyCursor, limit: 20 },
                fetchFn,
            );
            if (destroyed) return;
            const batch = Array.isArray(page.items) ? page.items : [];
            history = history.concat(batch);
            historyCursor =
                page.next_cursor == null ? historyCursor + batch.length : Number(page.next_cursor);
            historyHasMore = page.next_cursor != null && batch.length > 0;
        } catch {
            historyError = tSafe("profile.match_history_unavailable", "Match history unavailable.");
        } finally {
            historyLoading = false;
            render();
        }
    }

    async function openMatchDetail(matchId: string): Promise<void> {
        if (!fetchFn || !matchId) return;
        detailId = matchId;
        detail = null;
        detailError = "";
        render();
        try {
            const loaded = await fetchPublicMatchDetail(matchId, fetchFn);
            if (destroyed || detailId !== matchId) return;
            detail = loaded;
        } catch {
            if (destroyed || detailId !== matchId) return;
            detailError = tSafe("profile.match_details_unavailable", "Match details unavailable.");
        } finally {
            render();
        }
    }

    function handleClick(event: MouseEvent): void {
        const raw = event.target;
        const el = raw instanceof Element ? raw : null;
        if (!el) return;

        if (el.matches("[data-menu-overlay='profile-detail']")) {
            detail = null;
            detailId = null;
            detailError = "";
            render();
            return;
        }

        if (activeTab === "achievements") {
            const toggleBtn = el.closest<HTMLElement>("[data-command='toggle_dropdown']");
            if (toggleBtn) {
                const key = toggleBtn.dataset.dropdownKey || "";
                achOpenDropdownKey = achOpenDropdownKey === key ? null : key;
                syncDropdownScope(container, achOpenDropdownKey);
                return;
            }
            const optBtn = el.closest<HTMLElement>("[data-command='select_dropdown']");
            if (optBtn) {
                const key = optBtn.dataset.dropdownKey || "";
                const val = optBtn.dataset.dropdownOptionValue || "";
                achOpenDropdownKey = null;
                if (key === "ach-category") {
                    achState.category = normalizeWowCategory(val);
                    achState.subcategory = "all";
                } else if (key === "ach-status") {
                    achState.status = val || "all";
                } else if (key === "ach-tier") {
                    achState.tier = val || "all";
                } else if (key === "ach-sort") {
                    achState.sort = val || "smart";
                }
                achState.flatLimit = 90;
                render();
                return;
            }
        }

        const cmdBtn = el.closest<HTMLElement>("[data-command]");
        if (cmdBtn) {
            const command = cmdBtn.dataset.command;
            if (command === "profile_tab") {
                const next = cmdBtn.dataset.profileTab as typeof activeTab;
                if (next && next !== activeTab) {
                    activeTab = next;
                    achOpenDropdownKey = null;
                    render();
                }
                return;
            }
            if (command === "ranked_overview_mode") {
                const queue = cmdBtn.dataset.rankedOverviewMode;
                if (queue === "Ranked1v1" || queue === "RankedFFA") {
                    activeRankedQueue = queue;
                    render();
                }
                return;
            }
            if (command === "profile_history_queue") {
                historyQueueFilter = cmdBtn.dataset.historyQueue || "all";
                render();
                return;
            }
            if (command === "load_profile_more") {
                loadMoreHistory();
                return;
            }
            if (command === "open_match") {
                const mid = cmdBtn.dataset.matchId;
                if (mid) openMatchDetail(mid);
                return;
            }
            if (command === "close_match") {
                detail = null;
                detailId = null;
                detailError = "";
                render();
                return;
            }
            if (command === "retry_profile") {
                loadCurrentProfile();
                return;
            }
            if (command === "share_profile") {
                const id = cmdBtn.dataset.accountId || accountId;
                const url =
                    typeof window !== "undefined" && window.location
                        ? new URL(publicProfileUrl(id), window.location.origin).toString()
                        : publicProfileUrl(id);
                copyTextToClipboard(url).then((ok) => {
                    if (!ok || destroyed) return;
                    shareCopied = true;
                    if (shareTimer) clearTimeout(shareTimer);
                    shareTimer = setTimeout(() => {
                        shareCopied = false;
                        render();
                    }, 2200);
                    render();
                });
                return;
            }
            if (command === "profile_achievement_filter") {
                achState.category = normalizeWowCategory(cmdBtn.dataset.achievementFilter);
                achState.subcategory = "all";
                achState.flatLimit = 90;
                render();
                return;
            }
            if (command === "profile_achievement_subcategory") {
                achState.subcategory = cmdBtn.dataset.achievementSubcategory || "all";
                achState.flatLimit = 90;
                render();
                return;
            }
            if (command === "profile_achievement_view_mode") {
                achState.viewMode = cmdBtn.dataset.viewMode === "grid" ? "grid" : "chains";
                achState.flatLimit = 90;
                render();
                return;
            }
            if (command === "profile_achievement_show_more") {
                achState.flatLimit += 90;
                render();
                return;
            }
            if (command === "profile_achievement_clear") {
                achState.search = "";
                achState.status = "all";
                achState.tier = "all";
                achState.flatLimit = 90;
                render();
                return;
            }
            if (command === "select_chain_tier") {
                const ck = cmdBtn.dataset.chainKey;
                const tid = cmdBtn.dataset.tierId;
                if (ck && tid) {
                    achState.selectedTiers[ck] = tid;
                    render();
                }
                return;
            }
        }

        if (options.onNavigateProfile) {
            const playerLink = el.closest<HTMLElement>("[data-account-id]");
            const nextId = playerLink?.dataset.accountId;
            if (
                playerLink &&
                playerLink.matches("a.sow-profile__player-profile") &&
                nextId &&
                isValidAccountId(nextId) &&
                !event.metaKey &&
                !event.ctrlKey &&
                !event.shiftKey &&
                !event.altKey &&
                event.button === 0
            ) {
                event.preventDefault();
                options.onNavigateProfile(nextId);
            }
        }
    }

    function handleInput(event: Event): void {
        const input = event.target;
        if (!(input instanceof HTMLInputElement)) return;
        if (input.dataset.role === "achievements-search") {
            achState.search = input.value;
            achState.flatLimit = 90;
            const resultsEl = container.querySelector<HTMLElement>("[data-ach-results]");
            if (resultsEl && profile) {
                const enriched = enrichAchievementsList(resolveProfileAchievements(profile), false);
                resultsEl.innerHTML = renderAchResultsMarkup(enriched, false, achState);
            }
        }
    }

    function handleKeydown(event: KeyboardEvent): void {
        if (event.key === "Escape") {
            if (detail || detailError) {
                detail = null;
                detailId = null;
                detailError = "";
                render();
            } else if (achOpenDropdownKey) {
                achOpenDropdownKey = null;
                syncDropdownScope(container, null);
            }
        }
    }

    container.addEventListener("click", handleClick);
    container.addEventListener("input", handleInput);
    if (typeof document !== "undefined") {
        document.addEventListener("keydown", handleKeydown);
    }

    loadCurrentProfile();

    return {
        setAccountId(nextId: string) {
            if (nextId === accountId && profile) return;
            accountId = nextId;
            activeTab = "overview";
            loadCurrentProfile();
        },
        rerender() {
            render();
        },
        destroy() {
            destroyed = true;
            if (shareTimer) clearTimeout(shareTimer);
            container.removeEventListener("click", handleClick);
            container.removeEventListener("input", handleInput);
            if (typeof document !== "undefined") {
                document.removeEventListener("keydown", handleKeydown);
            }
        },
    };
}

/** Mounts a live player search directory + Top-3 1v1 & FFA champions showcase when /players/ has no ID. */
export function renderPlayerLookup(
    container: HTMLElement,
    options: {
        errorMessage?: string;
        onSelectPlayer?: (accountId: string) => void;
        fetchImpl?: typeof fetch;
    },
): { destroy(): void; rerender(): void } {
    const fetchFn: typeof fetch | null =
        options.fetchImpl || (typeof fetch === "function" ? fetch.bind(globalThis) : null);

    let query = "";
    let searching = false;
    let results: ProfileSearchItem[] = [];
    let champs1v1: LeaderboardRankedEntry[] = [];
    let champsFfa: LeaderboardRankedEntry[] = [];
    let searchAbort: AbortController | null = null;
    let searchTimer: ReturnType<typeof setTimeout> | null = null;
    let destroyed = false;

    function renderChampList(items: LeaderboardRankedEntry[]): string {
        if (!items.length) {
            return "<p class='sow-profile__empty'>" + esc(tSafe("profile.no_ranked_leaders", "No contenders ranked yet.")) + "</p>";
        }
        return (
            "<div class='sow-lb__list'>" +
            items
                .map((entry) => {
                    return (
                        "<a class='sow-lb__row' href='" +
                        esc(publicProfileUrl(entry.account_id)) +
                        "' data-account-id='" +
                        esc(entry.account_id) +
                        "' data-tier='" +
                        esc(rankTier(entry.tier)) +
                        "'><b class='sow-lb__rank'>#" +
                        esc(entry.rank) +
                        "</b>" +
                        rankEmblem(entry.tier, entry.division) +
                        "<div class='sow-lb__player'><div class='sow-lb__player-copy'><div class='sow-lb__player-title'><strong>" +
                        esc(entry.handle || entry.account_id) +
                        "</strong></div><span class='sow-lb__player-sub'>" +
                        esc(rankName(entry.tier, entry.division)) +
                        "</span></div></div><div class='sow-lb__metrics'><strong>" +
                        esc(tSafe("lobbies.rank_score", "{score} pts", { score: entry.score })) +
                        "</strong></div></a>"
                    );
                })
                .join("") +
            "</div>"
        );
    }

    function render(): void {
        if (destroyed) return;
        const resultsHtml =
            query.trim().length >= 2
                ? searching
                    ? "<div class='sow-skeleton-list'><div class='sow-skeleton'></div><div class='sow-skeleton'></div></div>"
                    : results.length
                      ? results
                            .map((item) => {
                                const avatar = leaderAvatarUrl(item.preferred_leader || "Caesar");
                                return (
                                    "<a class='sow-lb__row' href='" +
                                    esc(publicProfileUrl(item.account_id)) +
                                    "' data-account-id='" +
                                    esc(item.account_id) +
                                    "'><b class='sow-lb__rank'>Lv " +
                                    esc(item.level || 1) +
                                    "</b><img class='sow-lb__avatar-fallback' " +
                                    avatarImageAttribute(avatar) +
                                    " alt='' width='40' height='40' loading='lazy'><div class='sow-lb__player'><div class='sow-lb__player-copy'><div class='sow-lb__player-title'><strong>" +
                                    esc(item.handle || item.display_name || item.account_id) +
                                    "</strong></div><span class='sow-lb__player-sub'>" +
                                    esc((item.verified_wins || 0) + " " + tSafe("profile.wins", "Wins")) +
                                    "</span></div></div><div class='sow-lb__metrics'><strong>" +
                                    esc(
                                        item.ranked_1v1_score
                                            ? tSafe("lobbies.rank_score", "{score} pts", { score: item.ranked_1v1_score })
                                            : "—",
                                    ) +
                                    "</strong></div></a>"
                                );
                            })
                            .join("")
                      : "<p class='sow-profile__empty'>" +
                        esc(tSafe("site.search_players_empty", "No matching players found.")) +
                        "</p>"
                : "";

        container.innerHTML =
            "<div class='sow-player-lookup'>" +
            (options.errorMessage
                ? "<div class='sow-profile__status'>" + esc(options.errorMessage) + "</div>"
                : "") +
            "<section class='sow-player-lookup__card'>" +
            "<h1>" +
            esc(tSafe("site.players_heading", "Player Lookup")) +
            "</h1><p class='sow-profile__empty' style='margin:0'>" +
            esc(
                tSafe(
                    "site.players_lookup_hint",
                    "Search by handle or paste a player account ID to inspect ratings, match history, and achievements.",
                ),
            ) +
            "</p><div class='sow-player-lookup__search-row'>" +
            "<div class='sow-lb__search-wrap' style='flex:1'><span class='sow-lb__search-icon' aria-hidden='true'><svg width='15' height='15' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2'><circle cx='11' cy='11' r='8'/><line x1='21' y1='21' x2='16.65' y2='16.65'/></svg></span>" +
            "<input type='search' class='sow-lb__search-input' data-lookup-input placeholder='" +
            esc(tSafe("site.search_players_placeholder", "Search player handle...")) +
            "' value='" +
            esc(query) +
            "' maxlength='48' autocomplete='off' spellcheck='false' /></div>" +
            "<a class='sow-menu__secondary' href='" +
            esc(publicLeaderboardsUrl()) +
            "'>" +
            esc(tSafe("profile.leaderboards", "Leaderboards")) +
            "</a></div>" +
            (resultsHtml ? "<div class='sow-player-lookup__results'>" + resultsHtml + "</div>" : "") +
            "</section>" +
            "<div class='sow-player-lookup__champions'>" +
            "<section class='sow-player-lookup__champ-box'><h2>" +
            esc(tSafe("lobbies.ranked_1v1", "Ranked 1v1") + " · Top 3") +
            "</h2>" +
            renderChampList(champs1v1) +
            "</section>" +
            "<section class='sow-player-lookup__champ-box'><h2>" +
            esc(tSafe("lobbies.ranked_ffa", "Ranked FFA") + " · Top 3") +
            "</h2>" +
            renderChampList(champsFfa) +
            "</section></div></div>";
    }

    async function runSearch(): Promise<void> {
        if (!fetchFn || destroyed) return;
        const trimmed = query.trim();
        if (trimmed.length < 2) {
            searching = false;
            results = [];
            render();
            return;
        }
        if (isValidAccountId(trimmed) && options.onSelectPlayer) {
            options.onSelectPlayer(trimmed);
            return;
        }
        if (searchAbort) searchAbort.abort();
        searchAbort = typeof AbortController === "function" ? new AbortController() : null;
        searching = true;
        render();
        try {
            const list = await searchPublicProfiles(trimmed, 12, fetchFn, searchAbort?.signal);
            if (destroyed) return;
            results = list;
        } catch (err) {
            if ((err as { name?: string })?.name === "AbortError") return;
            results = [];
        } finally {
            if (!destroyed) {
                searching = false;
                render();
            }
        }
    }

    async function loadTopChampions(): Promise<void> {
        if (!fetchFn) return;
        const loadQueue = async (board: "Ranked1v1" | "RankedFFA") => {
            try {
                const url = buildLeaderboardRequestUrl(
                    { board, tier: "all", leader: "all", q: "", minGames: 0, sort: "score" },
                    0,
                    3,
                );
                const resp = await fetchFn(url, { headers: { Accept: "application/json" } });
                if (!resp.ok) return [];
                const json = (await resp.json()) as { items?: LeaderboardRankedEntry[] };
                return Array.isArray(json.items) ? json.items.slice(0, 3) : [];
            } catch {
                return [];
            }
        };
        const [a, b] = await Promise.all([loadQueue("Ranked1v1"), loadQueue("RankedFFA")]);
        if (destroyed) return;
        champs1v1 = a;
        champsFfa = b;
        render();
    }

    function handleInput(event: Event): void {
        const input = event.target;
        if (!(input instanceof HTMLInputElement) || !input.matches("[data-lookup-input]")) return;
        query = input.value;
        if (searchTimer) clearTimeout(searchTimer);
        searchTimer = setTimeout(runSearch, 250);
    }

    function handleClick(event: MouseEvent): void {
        if (!options.onSelectPlayer) return;
        const raw = event.target;
        const link = raw instanceof Element ? raw.closest<HTMLElement>("[data-account-id]") : null;
        const id = link?.dataset.accountId;
        if (
            link &&
            id &&
            isValidAccountId(id) &&
            !event.metaKey &&
            !event.ctrlKey &&
            !event.shiftKey &&
            !event.altKey &&
            event.button === 0
        ) {
            event.preventDefault();
            options.onSelectPlayer(id);
        }
    }

    container.addEventListener("input", handleInput);
    container.addEventListener("click", handleClick);
    render();
    loadTopChampions();

    return {
        rerender: render,
        destroy() {
            destroyed = true;
            if (searchTimer) clearTimeout(searchTimer);
            if (searchAbort) searchAbort.abort();
            container.removeEventListener("input", handleInput);
            container.removeEventListener("click", handleClick);
        },
    };
}
