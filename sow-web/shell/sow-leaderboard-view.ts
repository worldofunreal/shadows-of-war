import { avatarImageAttribute } from "./avatar-presentation.ts";
import { syncDropdownScope } from "./sow-dropdown.ts";
import {
    assetUrl,
    esc,
    getLeaderCatalog,
    isValidAccountId,
    leaderArtSlug,
    leaderAvatarUrl,
    leaderById,
    leaderDisplayName,
    profileApiUrl,
    publicProfileUrl,
    tSafe,
} from "./sow-profile-helpers.ts";
import { RANKED_LEAGUES, rankEmblem, rankName, rankTier } from "./sow-ranked-presentation.ts";

export type LeaderboardBoard = "Ranked1v1" | "RankedFFA" | "victories";

export interface LeaderboardRankedEntry {
    rank: number;
    account_id: string;
    handle: string;
    is_bot?: boolean;
    queue?: string;
    mode?: string;
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
    updated_at?: number;
}

export interface LeaderboardVictoryEntry {
    rank: number;
    account_id: string;
    handle: string;
    level: number;
    matches_played: number;
    wins: number;
    preferred_leader?: string | null;
    win_rate?: number;
}

export type LeaderboardEntry = LeaderboardRankedEntry | LeaderboardVictoryEntry;

export interface LeaderboardApiResponse {
    season_id?: number;
    metric?: string;
    items: LeaderboardEntry[];
    next_cursor: number | null;
    total_count?: number;
    tier_counts?: Record<string, number>;
    updated_at?: number;
}

export interface LeaderboardFilters {
    board: LeaderboardBoard;
    tier: string;
    leader: string;
    q: string;
    minGames: number;
    sort: string;
}

export interface LeaderboardControllerOptions {
    mode: "site" | "game";
    initialBoard?: LeaderboardBoard;
    syncUrl?: boolean;
    selfAccountId?: string | null;
    compactHeader?: boolean;
    pageSize?: number;
    onSelectPlayer?: (accountId: string) => void;
    onSfx?: (kind: "click" | "filter" | "tab") => void;
    fetchImpl?: typeof fetch;
}

export interface LeaderboardController {
    getFilters(): LeaderboardFilters;
    setFilters(partial: Partial<LeaderboardFilters>): void;
    refresh(): void;
    rerender(): void;
    destroy(): void;
}

interface DropdownOption {
    value: string;
    label: string;
    shortLabel?: string;
}

function renderDropdown(config: {
    key: string;
    label: string;
    name: string;
    value: string;
    className?: string;
    options: DropdownOption[];
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
        "'>" +
        "<button class='sow-control-dropdown__trigger' type='button' data-command='toggle_dropdown' data-dropdown-key='" +
        esc(config.key) +
        "' data-role='dropdown-trigger' aria-label='" +
        esc(config.label) +
        "' aria-haspopup='listbox' aria-expanded='false' aria-controls='sow-dropdown-" +
        esc(config.key) +
        "'><span class='sow-control-dropdown__trigger-content'><span data-dropdown-label>" +
        esc(displayLabel) +
        "</span></span><span class='sow-control-dropdown__chevron' aria-hidden='true'>⌄</span></button>" +
        "<div class='sow-control-dropdown__menu' id='sow-dropdown-" +
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

function defaultSortForBoard(board: LeaderboardBoard): string {
    return board === "victories" ? "wins" : "score";
}

function normalizeBoard(raw: string | null | undefined, fallback: LeaderboardBoard): LeaderboardBoard {
    if (raw === "Ranked1v1" || raw === "RankedFFA" || raw === "victories") return raw;
    if (raw === "1v1") return "Ranked1v1";
    if (raw === "ffa") return "RankedFFA";
    return fallback;
}

function isRankedEntry(entry: LeaderboardEntry, board: LeaderboardBoard): entry is LeaderboardRankedEntry {
    return board !== "victories" && "score" in entry;
}

function playerWinRatePct(wins: number, games: number, rawRate?: number): number {
    if (typeof rawRate === "number" && Number.isFinite(rawRate) && rawRate >= 0) {
        return Math.round((rawRate > 1 ? rawRate : rawRate * 100));
    }
    return games > 0 ? Math.round((wins / games) * 100) : 0;
}

function buildQueryParams(filters: LeaderboardFilters, cursor: number, limit: number): URLSearchParams {
    const params = new URLSearchParams();
    if (filters.board === "victories") {
        params.set("kind", "victories");
    } else {
        params.set("queue", filters.board);
        params.set("mode", "FFA");
        if (filters.tier && filters.tier !== "all") {
            params.set("tier", filters.tier);
        }
    }
    if (filters.leader && filters.leader !== "all") {
        params.set("leader", filters.leader);
    }
    if (filters.q.trim()) {
        params.set("q", filters.q.trim().slice(0, 48));
    }
    if (filters.minGames > 0) {
        params.set("min_games", String(filters.minGames));
    }
    const defSort = defaultSortForBoard(filters.board);
    if (filters.sort && filters.sort !== defSort) {
        params.set("sort", filters.sort);
    }
    if (cursor > 0) {
        params.set("cursor", String(cursor));
    }
    params.set("limit", String(limit));
    return params;
}

export function buildLeaderboardRequestUrl(filters: LeaderboardFilters, cursor: number, limit: number): string {
    const params = buildQueryParams(filters, cursor, limit);
    return profileApiUrl("/seasons/current/leaderboard?" + params.toString());
}

/** Creates the shared AAA filterable leaderboard controller for both public site and in-game Profile -> Leaderboards. */
export function createLeaderboardController(
    container: HTMLElement,
    options: LeaderboardControllerOptions,
): LeaderboardController {
    const pageSize = Math.max(10, Math.min(100, options.pageSize || 50));
    const fetchFn: typeof fetch | null =
        options.fetchImpl || (typeof fetch === "function" ? fetch.bind(globalThis) : null);

    let filters: LeaderboardFilters = {
        board: options.initialBoard || "Ranked1v1",
        tier: "all",
        leader: "all",
        q: "",
        minGames: 0,
        sort: defaultSortForBoard(options.initialBoard || "Ranked1v1"),
    };

    if (options.syncUrl && typeof window !== "undefined" && window.location) {
        try {
            const urlParams = new URLSearchParams(window.location.search);
            const urlBoard = normalizeBoard(
                urlParams.get("board") || urlParams.get("queue"),
                filters.board,
            );
            filters.board = urlBoard;
            const urlTier = urlParams.get("tier") || "all";
            filters.tier = urlBoard === "victories" ? "all" : urlTier.toLowerCase();
            filters.leader = urlParams.get("leader") || "all";
            filters.q = (urlParams.get("q") || "").slice(0, 48);
            const mg = Number(urlParams.get("min_games") || "0");
            filters.minGames = [0, 5, 10, 25].includes(mg) ? mg : 0;
            const urlSort = urlParams.get("sort");
            const validSorts =
                urlBoard === "victories"
                    ? ["wins", "win_rate", "matches", "level"]
                    : ["score", "peak", "wins", "win_rate", "games"];
            filters.sort = urlSort && validSorts.includes(urlSort) ? urlSort : defaultSortForBoard(urlBoard);
        } catch {}
    }

    let items: LeaderboardEntry[] = [];
    let tierCounts: Record<string, number> = {};
    let totalCount = 0;
    let nextCursor: number | null = null;
    let loading = true;
    let loadingMore = false;
    let error = "";
    let activeAbort: AbortController | null = null;
    let searchTimer: ReturnType<typeof setTimeout> | null = null;
    let requestSerial = 0;
    let openDropdownKey: string | null = null;
    let destroyed = false;

    function syncUrlState(): void {
        if (!options.syncUrl || typeof window === "undefined" || !window.history || !window.location) {
            return;
        }
        try {
            const url = new URL(window.location.href);
            url.search = "";
            if (filters.board !== "Ranked1v1") url.searchParams.set("board", filters.board);
            if (filters.board !== "victories" && filters.tier !== "all") url.searchParams.set("tier", filters.tier);
            if (filters.leader !== "all") url.searchParams.set("leader", filters.leader);
            if (filters.q.trim()) url.searchParams.set("q", filters.q.trim());
            if (filters.minGames > 0) url.searchParams.set("min_games", String(filters.minGames));
            if (filters.sort !== defaultSortForBoard(filters.board)) url.searchParams.set("sort", filters.sort);
            window.history.replaceState(null, "", url.pathname + url.search + url.hash);
        } catch {}
    }

    function hasActiveFilters(): boolean {
        return Boolean(
            (filters.board !== "victories" && filters.tier !== "all") ||
                filters.leader !== "all" ||
                filters.q.trim() ||
                filters.minGames > 0 ||
                filters.sort !== defaultSortForBoard(filters.board),
        );
    }

    function renderPlayerItemWrap(
        className: string,
        accountId: string,
        extraAttrs: string,
        innerHtml: string,
    ): string {
        if (options.mode === "site") {
            const href = isValidAccountId(accountId) ? publicProfileUrl(accountId) : "#";
            return (
                "<a class='" +
                className +
                "' href='" +
                esc(href) +
                "' data-account-id='" +
                esc(accountId) +
                "' " +
                extraAttrs +
                ">" +
                innerHtml +
                "</a>"
            );
        }
        return (
            "<button type='button' class='" +
            className +
            "' data-command='open_profile' data-account-id='" +
            esc(accountId) +
            "' " +
            extraAttrs +
            ">" +
            innerHtml +
            "</button>"
        );
    }

    function renderPodium(topThree: LeaderboardEntry[]): string {
        if (topThree.length < 3 || hasActiveFilters()) return "";
        return (
            "<div class='sow-lb__podium' aria-label='" +
            esc(tSafe("site.top_champions", "Top Champions")) +
            "'>" +
            topThree
                .map((entry, idx) => {
                    const place = idx + 1;
                    const tier = isRankedEntry(entry, filters.board) ? rankTier(entry.tier) : "deity";
                    const badge = isRankedEntry(entry, filters.board)
                        ? rankEmblem(entry.tier, entry.division)
                        : "<img class='sow-lb__avatar-fallback' " +
                          avatarImageAttribute(leaderAvatarUrl(entry.preferred_leader || "Caesar")) +
                          " alt='' width='52' height='52' loading='lazy'>";
                    const avatar = entry.preferred_leader
                        ? "<img class='sow-lb__podium-avatar' " +
                          avatarImageAttribute(leaderAvatarUrl(entry.preferred_leader)) +
                          " alt='' width='54' height='54' loading='lazy'>"
                        : "";
                    const games = isRankedEntry(entry, filters.board)
                        ? entry.games_played
                        : entry.matches_played;
                    const wins = entry.wins || 0;
                    const wr = playerWinRatePct(wins, games, entry.win_rate);
                    const scoreLine = isRankedEntry(entry, filters.board)
                        ? tSafe("lobbies.rank_score", "{score} pts", { score: entry.score })
                        : wins + " " + tSafe("profile.wins", "Wins");
                    const subLine = isRankedEntry(entry, filters.board)
                        ? rankName(entry.tier, entry.division)
                        : tSafe("profile.level", "Level {level}", { level: entry.level || 1 });
                    const leaderName = entry.preferred_leader
                        ? " · " + leaderDisplayName(leaderById(entry.preferred_leader))
                        : "";
                    const selfClass =
                        options.selfAccountId && entry.account_id === options.selfAccountId
                            ? " is-self"
                            : "";
                    return renderPlayerItemWrap(
                        "sow-lb__podium-card sow-lb__podium-card--" + place + selfClass,
                        entry.account_id,
                        "data-tier='" + esc(tier) + "'",
                        "<span class='sow-lb__podium-rank'>#" +
                            place +
                            "</span><div class='sow-lb__podium-visual'>" +
                            avatar +
                            badge +
                            "</div><h3 class='sow-lb__podium-name'>" +
                            esc(entry.handle || entry.account_id) +
                            "</h3><span class='sow-lb__podium-sub'>" +
                            esc(subLine + leaderName) +
                            "</span><strong class='sow-lb__podium-score'>" +
                            esc(scoreLine) +
                            "</strong><div class='sow-lb__podium-stats'><span>" +
                            esc(wins + "W / " + games + "G") +
                            "</span><span>" +
                            esc(wr + "% WR") +
                            "</span></div>",
                    );
                })
                .join("") +
            "</div>"
        );
    }

    function renderRow(entry: LeaderboardEntry): string {
        const ranked = isRankedEntry(entry, filters.board);
        const tierAttr = ranked ? "data-tier='" + esc(rankTier(entry.tier)) + "'" : "";
        const selfClass =
            options.selfAccountId && entry.account_id === options.selfAccountId ? " is-self" : "";
        const games = ranked ? entry.games_played : entry.matches_played;
        const wins = entry.wins || 0;
        const wr = playerWinRatePct(wins, games, entry.win_rate);
        const leaderInfo = entry.preferred_leader ? leaderById(entry.preferred_leader) : null;
        const leaderLabel = leaderInfo ? leaderDisplayName(leaderInfo) : "";
        const playerAvatar = entry.preferred_leader
            ? "<img class='sow-lb__player-avatar' " +
              avatarImageAttribute(leaderAvatarUrl(entry.preferred_leader)) +
              " alt='' width='34' height='34' loading='lazy'>"
            : "";
        const badgeOrAvatar = ranked
            ? rankEmblem(entry.tier, entry.division)
            : "<img class='sow-lb__avatar-fallback' " +
              avatarImageAttribute(
                  leaderAvatarUrl(entry.preferred_leader || "Caesar"),
              ) +
              " alt='' width='40' height='40' loading='lazy'>";
        const levelTag =
            typeof entry.level === "number" && entry.level > 0
                ? "<span class='sow-lb__level-tag'>Lv " + esc(entry.level) + "</span>"
                : "";
        const streakTag =
            ranked && typeof entry.win_streak === "number" && entry.win_streak >= 2
                ? "<span class='sow-lb__streak-tag'>" +
                  esc(entry.win_streak + "W") +
                  "</span>"
                : "";
        const primaryMetric = ranked
            ? tSafe("lobbies.rank_score", "{score} pts", { score: entry.score })
            : wins + " " + tSafe("profile.wins", "Wins");
        const secondaryMetric = ranked
            ? wins + "W · " + games + "G · " + wr + "%" +
              (entry.peak_score ? " · " + tSafe("profile.peak_score", "Peak") + " " + entry.peak_score : "")
            : games + " " + tSafe("profile.matches", "Matches") + " · " + wr + "%";
        const subtitleParts = [
            ranked
                ? rankName(entry.tier, entry.division)
                : tSafe("profile.level", "Level {level}", { level: entry.level || 1 }),
            leaderLabel,
        ].filter(Boolean);

        return renderPlayerItemWrap(
            "sow-lb__row" + selfClass,
            entry.account_id,
            tierAttr,
            "<b class='sow-lb__rank'>#" +
                esc(entry.rank) +
                "</b>" +
                badgeOrAvatar +
                "<div class='sow-lb__player'>" +
                playerAvatar +
                "<div class='sow-lb__player-copy'><div class='sow-lb__player-title'><strong>" +
                esc(entry.handle || entry.account_id) +
                "</strong>" +
                levelTag +
                streakTag +
                "</div><span class='sow-lb__player-sub'>" +
                esc(subtitleParts.join(" · ")) +
                "</span></div></div><div class='sow-lb__metrics'><strong>" +
                esc(primaryMetric) +
                "</strong><small>" +
                esc(secondaryMetric) +
                "</small></div>",
        );
    }

    function render(): void {
        if (destroyed) return;
        const catalog = getLeaderCatalog();
        const leaderOptions: DropdownOption[] = [
            { value: "all", label: tSafe("site.filter_all_leaders", "All Commanders") },
            ...catalog.map((leader) => ({
                value: leader.id,
                label: leaderDisplayName(leader) || leader.id,
            })),
        ];
        const minGamesOptions: DropdownOption[] = [
            { value: "0", label: tSafe("site.filter_min_games_any", "Any Matches") },
            { value: "5", label: tSafe("site.filter_min_games_n", "5+ Matches", { count: 5 }) },
            { value: "10", label: tSafe("site.filter_min_games_n", "10+ Matches", { count: 10 }) },
            { value: "25", label: tSafe("site.filter_min_games_n", "25+ Matches", { count: 25 }) },
        ];
        const sortOptions: DropdownOption[] =
            filters.board === "victories"
                ? [
                      { value: "wins", label: tSafe("site.sort_wins", "Most Wins") },
                      { value: "win_rate", label: tSafe("site.sort_win_rate", "Highest Win Rate") },
                      { value: "matches", label: tSafe("site.sort_games", "Most Matches") },
                      { value: "level", label: tSafe("profile.level_label", "Level") },
                  ]
                : [
                      { value: "score", label: tSafe("site.sort_score", "Rating Points") },
                      { value: "peak", label: tSafe("site.sort_peak", "Peak Score") },
                      { value: "wins", label: tSafe("site.sort_wins", "Most Wins") },
                      { value: "win_rate", label: tSafe("site.sort_win_rate", "Highest Win Rate") },
                      { value: "games", label: tSafe("site.sort_games", "Most Matches") },
                  ];

        const tierRailHtml =
            filters.board !== "victories"
                ? "<div class='sow-lb__tiers' role='group' aria-label='" +
                  esc(tSafe("site.filter_tier", "League Tier")) +
                  "'><button type='button' class='sow-lb__tier-chip" +
                  (filters.tier === "all" ? " is-active" : "") +
                  "' data-lb-tier='all' aria-pressed='" +
                  (filters.tier === "all" ? "true" : "false") +
                  "'>" +
                  esc(tSafe("site.filter_all_tiers", "All Leagues")) +
                  "</button>" +
                  RANKED_LEAGUES.map((league) => {
                      const active = filters.tier === league.key;
                      const count =
                          typeof tierCounts[league.key] === "number" ? tierCounts[league.key] : 0;
                      return (
                          "<button type='button' class='sow-lb__tier-chip" +
                          (active ? " is-active" : "") +
                          "' data-lb-tier='" +
                          esc(league.key) +
                          "' data-tier='" +
                          esc(league.key) +
                          "' aria-pressed='" +
                          (active ? "true" : "false") +
                          "'>" +
                          rankEmblem(league.tier) +
                          "<span>" +
                          esc(rankName(league.tier)) +
                          "</span><b>" +
                          esc(count) +
                          "</b></button>"
                      );
                  }).join("") +
                  "</div>"
                : "";

        const showPodium = !loading && !error && !hasActiveFilters() && items.length >= 3;
        const podiumHtml = showPodium ? renderPodium(items.slice(0, 3)) : "";
        const listItems = showPodium ? items.slice(3) : items;

        let bodyHtml = "";
        if (loading) {
            bodyHtml =
                "<div class='sow-skeleton-list' aria-busy='true' aria-label='" +
                esc(tSafe("profile.leaderboard_loading", "Loading leaderboard...")) +
                "'>" +
                Array.from({ length: 6 }, () => "<div class='sow-skeleton'></div>").join("") +
                "</div>";
        } else if (error) {
            bodyHtml =
                "<div class='sow-profile__ach-empty-box'><p class='sow-profile__empty'>" +
                esc(error) +
                "</p><button type='button' class='sow-profile__load-more' data-lb-retry>" +
                esc(tSafe("profile.try_again", "Try again")) +
                "</button></div>";
        } else if (!items.length) {
            bodyHtml =
                "<div class='sow-profile__ach-empty-box'><p class='sow-profile__empty'>" +
                esc(
                    filters.board === "victories"
                        ? tSafe("profile.no_victory_leaders", "No players match the current filters.")
                        : tSafe("profile.no_ranked_leaders", "No ranked contenders match the current filters."),
                ) +
                "</p>" +
                (hasActiveFilters()
                    ? "<button type='button' class='sow-lb__reset-btn' data-lb-reset>" +
                      esc(tSafe("site.reset_filters", "Reset filters")) +
                      "</button>"
                    : "") +
                "</div>";
        } else {
            bodyHtml =
                podiumHtml +
                (listItems.length
                    ? "<div class='sow-lb__list'>" + listItems.map(renderRow).join("") + "</div>"
                    : "");
        }

        const loadMoreHtml =
            !loading && !error && nextCursor !== null
                ? "<div><button type='button' class='sow-profile__load-more' data-lb-more" +
                  (loadingMore ? " disabled" : "") +
                  ">" +
                  esc(
                      loadingMore
                          ? tSafe("profile.loading_more", "Loading more...")
                          : tSafe("site.load_more", "Load More"),
                  ) +
                  "</button></div>"
                : "";

        container.innerHTML =
            "<div class='sow-lb'>" +
            "<div class='sow-lb__board-tabs' role='tablist' aria-label='" +
            esc(tSafe("profile.leaderboards", "Leaderboards")) +
            "'>" +
            "<button type='button' role='tab' class='sow-lb__board-tab" +
            (filters.board === "Ranked1v1" ? " is-active" : "") +
            "' data-lb-board='Ranked1v1' aria-pressed='" +
            (filters.board === "Ranked1v1" ? "true" : "false") +
            "'>" +
            esc(tSafe("lobbies.ranked_1v1", "Ranked 1v1")) +
            "</button>" +
            "<button type='button' role='tab' class='sow-lb__board-tab" +
            (filters.board === "RankedFFA" ? " is-active" : "") +
            "' data-lb-board='RankedFFA' aria-pressed='" +
            (filters.board === "RankedFFA" ? "true" : "false") +
            "'>" +
            esc(tSafe("lobbies.ranked_ffa", "Ranked FFA")) +
            "</button>" +
            "<button type='button' role='tab' class='sow-lb__board-tab" +
            (filters.board === "victories" ? " is-active" : "") +
            "' data-lb-board='victories' aria-pressed='" +
            (filters.board === "victories" ? "true" : "false") +
            "'>" +
            esc(tSafe("profile.verified_wins", "Verified Wins")) +
            "</button>" +
            "</div>" +
            "<div class='sow-lb__toolbar'>" +
            "<div class='sow-lb__search-wrap'>" +
            "<span class='sow-lb__search-icon' aria-hidden='true'><svg width='15' height='15' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2'><circle cx='11' cy='11' r='8'/><line x1='21' y1='21' x2='16.65' y2='16.65'/></svg></span>" +
            "<input type='search' class='sow-lb__search-input' data-lb-search placeholder='" +
            esc(tSafe("site.search_players_placeholder", "Search player handle...")) +
            "' value='" +
            esc(filters.q) +
            "' maxlength='48' autocomplete='off' spellcheck='false' />" +
            (filters.q
                ? "<button type='button' class='sow-lb__search-clear' data-lb-search-clear aria-label='" +
                  esc(tSafe("site.reset_filters", "Clear")) +
                  "'>×</button>"
                : "") +
            "</div>" +
            "<div class='sow-lb__filters'>" +
            renderDropdown({
                key: "lb-leader",
                label: tSafe("site.filter_leader", "Commander"),
                name: "leader",
                value: filters.leader,
                className: "sow-lb__dd" + (filters.leader !== "all" ? " is-filtered" : ""),
                options: leaderOptions,
            }) +
            renderDropdown({
                key: "lb-min-games",
                label: tSafe("site.filter_min_games", "Min Matches"),
                name: "min_games",
                value: String(filters.minGames),
                className: "sow-lb__dd" + (filters.minGames > 0 ? " is-filtered" : ""),
                options: minGamesOptions,
            }) +
            renderDropdown({
                key: "lb-sort",
                label: tSafe("site.filter_sort", "Sort"),
                name: "sort",
                value: filters.sort,
                className: "sow-lb__dd",
                options: sortOptions,
            }) +
            "</div>" +
            "</div>" +
            tierRailHtml +
            "<div class='sow-lb__meta-bar'><span>" +
            esc(
                tSafe("site.showing_players_count", "Showing {shown} of {total} players", {
                    shown: items.length,
                    total: totalCount || items.length,
                }),
            ) +
            "</span>" +
            (hasActiveFilters()
                ? "<button type='button' class='sow-lb__reset-btn' data-lb-reset>" +
                  esc(tSafe("site.reset_filters", "Reset filters")) +
                  "</button>"
                : "") +
            "</div>" +
            bodyHtml +
            loadMoreHtml +
            "</div>";

        if (options.mode === "site") {
            syncDropdownScope(container, openDropdownKey);
        }
    }

    async function fetchPage(append: boolean): Promise<void> {
        if (!fetchFn || destroyed) {
            loading = false;
            render();
            return;
        }
        if (append && (loadingMore || nextCursor === null)) return;
        const cursor = append && nextCursor !== null ? nextCursor : 0;
        if (activeAbort) activeAbort.abort();
        activeAbort = typeof AbortController === "function" ? new AbortController() : null;
        const serial = ++requestSerial;
        if (append) loadingMore = true;
        else {
            loading = true;
            error = "";
        }
        render();
        try {
            const response = await fetchFn(buildLeaderboardRequestUrl(filters, cursor, pageSize), {
                headers: { Accept: "application/json" },
                signal: activeAbort?.signal,
            });
            if (!response.ok) throw new Error("leaderboard request failed: " + response.status);
            const data = (await response.json()) as LeaderboardApiResponse;
            if (serial !== requestSerial || destroyed) return;
            const batch = Array.isArray(data.items) ? data.items : [];
            items = append ? items.concat(batch) : batch;
            nextCursor = data.next_cursor == null ? null : Number(data.next_cursor);
            totalCount = typeof data.total_count === "number" ? data.total_count : items.length;
            if (data.tier_counts && typeof data.tier_counts === "object") {
                tierCounts = data.tier_counts;
            }
            error = "";
        } catch (err) {
            if ((err as { name?: string })?.name === "AbortError" || serial !== requestSerial || destroyed) {
                return;
            }
            error = tSafe("profile.leaderboard_unavailable", "Leaderboard unavailable right now.");
        } finally {
            if (serial === requestSerial && !destroyed) {
                loading = false;
                loadingMore = false;
                render();
            }
        }
    }

    function applyFilterChange(): void {
        openDropdownKey = null;
        syncUrlState();
        fetchPage(false);
    }

    function handleClick(event: MouseEvent): void {
        const raw = event.target;
        const el = raw instanceof Element ? raw : null;
        if (!el) return;

        if (options.mode === "site") {
            const toggleBtn = el.closest<HTMLElement>("[data-command='toggle_dropdown']");
            if (toggleBtn) {
                const key = toggleBtn.dataset.dropdownKey || "";
                openDropdownKey = openDropdownKey === key ? null : key;
                syncDropdownScope(container, openDropdownKey);
                return;
            }
            const optBtn = el.closest<HTMLElement>("[data-command='select_dropdown']");
            if (optBtn) {
                const key = optBtn.dataset.dropdownKey || "";
                const val = optBtn.dataset.dropdownOptionValue || "";
                openDropdownKey = null;
                const dd = container.querySelector<HTMLElement>("[data-dropdown-key='" + key + "']");
                const input = dd?.querySelector<HTMLInputElement>("[data-dropdown-input]");
                if (input) {
                    input.value = val;
                    input.dispatchEvent(new Event("change", { bubbles: true }));
                }
                return;
            }
        }

        const boardBtn = el.closest<HTMLElement>("[data-lb-board]");
        if (boardBtn) {
            const nextBoard = normalizeBoard(boardBtn.dataset.lbBoard, filters.board);
            if (nextBoard !== filters.board) {
                options.onSfx?.("tab");
                filters.board = nextBoard;
                filters.tier = "all";
                filters.sort = defaultSortForBoard(nextBoard);
                applyFilterChange();
            }
            return;
        }

        const tierBtn = el.closest<HTMLElement>("[data-lb-tier]");
        if (tierBtn) {
            const nextTier = tierBtn.dataset.lbTier || "all";
            options.onSfx?.("filter");
            filters.tier = filters.tier === nextTier && nextTier !== "all" ? "all" : nextTier;
            applyFilterChange();
            return;
        }

        if (el.closest("[data-lb-search-clear]")) {
            options.onSfx?.("filter");
            filters.q = "";
            applyFilterChange();
            return;
        }

        if (el.closest("[data-lb-reset]")) {
            options.onSfx?.("filter");
            filters.tier = "all";
            filters.leader = "all";
            filters.q = "";
            filters.minGames = 0;
            filters.sort = defaultSortForBoard(filters.board);
            applyFilterChange();
            return;
        }

        if (el.closest("[data-lb-more]")) {
            options.onSfx?.("click");
            fetchPage(true);
            return;
        }

        if (el.closest("[data-lb-retry]")) {
            options.onSfx?.("click");
            fetchPage(false);
            return;
        }

        if (options.mode === "site" && options.onSelectPlayer) {
            const playerLink = el.closest<HTMLElement>("[data-account-id]");
            const accountId = playerLink?.dataset.accountId;
            if (
                playerLink &&
                accountId &&
                isValidAccountId(accountId) &&
                !event.metaKey &&
                !event.ctrlKey &&
                !event.shiftKey &&
                !event.altKey &&
                event.button === 0
            ) {
                event.preventDefault();
                options.onSelectPlayer(accountId);
            }
        }
    }

    function handleInput(event: Event): void {
        const input = event.target;
        if (!(input instanceof HTMLInputElement) || !input.matches("[data-lb-search]")) return;
        filters.q = input.value.slice(0, 48);
        if (searchTimer) clearTimeout(searchTimer);
        searchTimer = setTimeout(() => {
            syncUrlState();
            fetchPage(false);
        }, 250);
    }

    function handleChange(event: Event): void {
        const input = event.target;
        if (!(input instanceof HTMLInputElement) || !input.matches("[data-dropdown-input]")) return;
        const dd = input.closest<HTMLElement>("[data-dropdown-key]");
        const key = dd?.dataset.dropdownKey;
        if (key === "lb-leader") {
            filters.leader = input.value || "all";
            applyFilterChange();
        } else if (key === "lb-min-games") {
            filters.minGames = Number(input.value) || 0;
            applyFilterChange();
        } else if (key === "lb-sort") {
            filters.sort = input.value || defaultSortForBoard(filters.board);
            applyFilterChange();
        }
    }

    function handleDocPointerDown(event: PointerEvent): void {
        if (!openDropdownKey) return;
        const target = event.target;
        if (target instanceof Node && container.contains(target)) return;
        openDropdownKey = null;
        syncDropdownScope(container, null);
    }

    function handleDocKeydown(event: KeyboardEvent): void {
        if (event.key === "Escape" && openDropdownKey) {
            openDropdownKey = null;
            syncDropdownScope(container, null);
        }
    }

    container.addEventListener("click", handleClick);
    container.addEventListener("input", handleInput);
    container.addEventListener("change", handleChange);
    if (options.mode === "site" && typeof document !== "undefined") {
        document.addEventListener("pointerdown", handleDocPointerDown);
        document.addEventListener("keydown", handleDocKeydown);
    }

    fetchPage(false);

    return {
        getFilters() {
            return { ...filters };
        },
        setFilters(partial) {
            Object.assign(filters, partial);
            if (partial.board && !partial.sort) {
                filters.sort = defaultSortForBoard(filters.board);
            }
            applyFilterChange();
        },
        refresh() {
            fetchPage(false);
        },
        rerender() {
            render();
        },
        destroy() {
            destroyed = true;
            if (searchTimer) clearTimeout(searchTimer);
            if (activeAbort) activeAbort.abort();
            container.removeEventListener("click", handleClick);
            container.removeEventListener("input", handleInput);
            container.removeEventListener("change", handleChange);
            if (options.mode === "site" && typeof document !== "undefined") {
                document.removeEventListener("pointerdown", handleDocPointerDown);
                document.removeEventListener("keydown", handleDocKeydown);
            }
        },
    };
}
