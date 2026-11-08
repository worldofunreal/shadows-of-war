import {
  rankEmblem,
  rankLabel as rankLabelText,
  rankName,
  rankOverview,
  rankedPointsBreakdownMarkup,
} from "./main_menu.ranked.ts";
import {
  asset,
  avatarImage,
  currencyAsset,
  currentScreen,
  esc,
  formatMapName,
  leaderById,
  leaderArtSlug,
  leaderDisplayName,
  menuUi,
  profileCache,
  profileHeroImage,
  root,
  selfCreds,
} from "./main_menu.core.ts";
import {
  REPORT_REASONS,
  activeScreenPanel,
  render,
  reportBusy,
} from "./main_menu.shell.ts";
import type {
  MenuState,
  RankedLadder,
  RankedPointsBreakdown,
} from "./main_menu.state.ts";

// Profile data, rendering, history, and match details.

// Shapes served by sow-data (sow-data/src/profile.rs is the source of truth); only the
// fields the profile screen reads or seeds are described.
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
  // The seed built from menu state has no achievements until the server snapshot arrives.
  achievements?: ProfileAchievement[];
  leaders: ProfileLeader[];
  recent_matches: ProfileMatch[];
  ratings?: ProfileRating[];
  ranked_ladder?: RankedLadder | null;
}

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

interface ProfileLeader {
  leader: string;
  matches_played: number;
  wins: number;
  win_rate: number;
  xp: number;
}

interface ProfileAchievement {
  id: string;
  category: string;
  title?: string;
  description?: string;
  points: number;
  progress: number;
  target: number;
  unlocked: boolean;
}

export interface ProfileMatch {
  match_id: string;
  completed_at: number;
  queue: string;
  mode: string;
  map_name: string;
  duration_seconds: number;
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

interface ProfileMatchParticipant {
  account_id: string;
  handle: string;
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

interface ProfileVictoryLeader {
  rank: number;
  account_id: string;
  handle: string;
  level: number;
  matches_played: number;
  wins: number;
}

interface ProfileRankedLeader {
  rank: number;
  account_id: string;
  handle: string;
  is_bot: boolean;
  score: number;
  tier: string;
  division: string | null;
  games_played: number;
  wins: number;
}

interface ProfilePage<Item> {
  items: Item[];
  next_cursor: number | null;
}

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

let profileVictoryLeaderboard: ProfileVictoryLeader[] | null = null;
let profileVictoryLeaderboardLoading = false;
let profileVictoryLeaderboardError = "";
let profileVictoryLeaderboardRequest: Promise<void> | null = null;
let profileVictoryLeaderboardCursor = 0;
let profileVictoryLeaderboardHasMore = false;
let profileRankedLeaderboard: ProfileRankedLeader[] | null = null;
let profileRankedLeaderboardLoading = false;
let profileRankedLeaderboardError = "";
let profileRankedLeaderboardRequest: Promise<void> | null = null;
let profileRankedLeaderboardRevision = 0;

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

function profileStat(value: number) {
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, Math.floor(number)) : 0;
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
    const rankLabel = root.querySelector<HTMLElement>("[data-ranked-current]");
    if (rankLabel) {
      const rating = menuUi.rankedRating;
      rankLabel.textContent = rankLabelText(rating);
      const emblem = root.querySelector<HTMLElement>("[data-ranked-emblem]");
      if (emblem)
        emblem.innerHTML = rankEmblem(
          rating?.placements_complete ? rating.tier : "unranked",
          rating?.placements_complete ? rating.division : null,
        );
    }
  }
  if (menuUi.profileAccountId === id) activateProfileEntry(id);
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

export function loadVictoryLeaderboard(append?: boolean) {
  if (
    profileVictoryLeaderboardRequest ||
    (profileVictoryLeaderboard !== null && !append)
  )
    return;
  const appendItems = !!append && profileVictoryLeaderboard !== null;
  if (appendItems && !profileVictoryLeaderboardHasMore) return;
  const cursor = appendItems ? profileVictoryLeaderboardCursor : 0;
  profileVictoryLeaderboardLoading = true;
  profileVictoryLeaderboardError = "";
  profileVictoryLeaderboardRequest = fetch(
    profileApi(
      "/seasons/1/leaderboard?kind=victories&cursor=" + cursor + "&limit=25",
    ),
    {
      headers: { Accept: "application/json" },
    },
  )
    .then(function (response) {
      if (!response.ok) throw new Error("victories leaderboard failed");
      return response.json();
    })
    .then(function (data: ProfilePage<ProfileVictoryLeader>) {
      const items = Array.isArray(data.items) ? data.items : [];
      profileVictoryLeaderboard = appendItems
        ? profileVictoryLeaderboard!.concat(items)
        : items;
      profileVictoryLeaderboardCursor =
        data.next_cursor == null ? cursor : Number(data.next_cursor);
      profileVictoryLeaderboardHasMore =
        data.next_cursor != null && items.length > 0;
    })
    .catch(function () {
      profileVictoryLeaderboardError = SOW_t("profile.leaderboard_unavailable");
    })
    .finally(function () {
      profileVictoryLeaderboardRequest = null;
      profileVictoryLeaderboardLoading = false;
      if (menuUi.profileOpen) syncProfileDom();
    });
}

export function loadRankedLeaderboard(retry = false) {
  if (
    profileRankedLeaderboardRequest ||
    (!retry && profileRankedLeaderboard !== null)
  )
    return;
  const revision = profileRankedLeaderboardRevision;
  profileRankedLeaderboardLoading = true;
  profileRankedLeaderboardError = "";
  profileRankedLeaderboardRequest = fetch(
    profileApi("/seasons/1/leaderboard?queue=Ranked1v1&mode=FFA&limit=100"),
    { headers: { Accept: "application/json" } },
  )
    .then(function (response) {
      if (!response.ok) throw new Error("ranked leaderboard failed");
      return response.json();
    })
    .then(function (data: ProfilePage<ProfileRankedLeader>) {
      if (revision === profileRankedLeaderboardRevision)
        profileRankedLeaderboard = Array.isArray(data.items) ? data.items : [];
    })
    .catch(function () {
      if (revision === profileRankedLeaderboardRevision)
        profileRankedLeaderboardError = SOW_t(
          "profile.leaderboard_unavailable",
        );
    })
    .finally(function () {
      profileRankedLeaderboardRequest = null;
      profileRankedLeaderboardLoading = false;
      if (
        revision !== profileRankedLeaderboardRevision &&
        menuUi.profileOpen &&
        menuUi.profileTab === "victories"
      )
        loadRankedLeaderboard();
      if (menuUi.profileOpen) syncProfileDom();
    });
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
  }
  if (previousId && previousId !== nextId) invalidateProfileCache(previousId);
  menuUi.profileOwnId = nextId;
  if (previousId !== nextId) {
    menuUi.rankedRating = null;
    menuUi.rankedLadder = null;
  }
  menuUi.profileLastPhase = nextState.phase;
  if (!nextId) return;
  const entry = cacheProfileSeed(nextId, profileSeedFromState(nextId));
  if (returnedToMenu) {
    invalidateProfileCache(nextId);
    profileRankedLeaderboardRevision++;
    profileRankedLeaderboard = null;
    profileRankedLeaderboardError = "";
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

function profileLeaderCard(leader: ProfileLeader) {
  const summary = leader || {};
  const leaderInfo = leaderById(summary.leader);
  const winRate = Math.round((summary.win_rate || 0) * 100);
  const level = 1 + Math.floor((summary.xp || 0) / 100);
  return (
    "<article class='sow-profile__leader'>" +
    "<div class='sow-profile__leader-avatar-frame'>" +
    "<img src='" +
    esc(asset("gameplay/avatars/" + leaderArtSlug(leaderInfo) + ".webp")) +
    "' alt='' width='52' height='52' loading='lazy'>" +
    "<span class='sow-profile__leader-badge'>" +
    esc(level) +
    "</span>" +
    "</div>" +
    "<div class='sow-profile__leader-info'>" +
    "<div class='sow-profile__leader-header'>" +
    "<strong>" +
    esc(leaderDisplayName(leaderInfo)) +
    "</strong>" +
    "<b>" +
    esc(SOW_t("profile.level", { level: level })) +
    "</b>" +
    "</div>" +
    "<div class='sow-profile__leader-track' aria-hidden='true'><i style='width:" +
    winRate +
    "%'></i></div>" +
    "<span>" +
    esc(
      SOW_t("profile.commander_stats", {
        matches: summary.matches_played || 0,
        wins: summary.wins || 0,
        rate: winRate,
      }),
    ) +
    "</span></div>" +
    "</article>"
  );
}

function profileMatchDate(timestamp: number) {
  const seconds = Number(timestamp);
  if (!Number.isFinite(seconds) || seconds <= 0) return "—";
  try {
    return new Intl.DateTimeFormat(document.documentElement.lang || undefined, {
      dateStyle: "medium",
    }).format(new Date(seconds * 1000));
  } catch {
    return new Date(seconds * 1000).toLocaleDateString();
  }
}

function profileMatchDuration(seconds: number) {
  const duration = Math.max(0, Math.floor(Number(seconds) || 0));
  if (!duration) return "—";
  return Math.floor(duration / 60) + "m " + (duration % 60) + "s";
}

function profileMatchRow(match: ProfileMatch) {
  const verified = !!match.verified;
  const result = verified
    ? match.won
      ? SOW_t("profile.win")
      : SOW_t("profile.loss")
    : SOW_t(
        match.provisional
          ? "profile.match_pending"
          : "profile.match_unverified",
      );
  const mode = match.mode || SOW_t("profile.ffa");
  const map = formatMapName(match);
  const leader = match.leader ? leaderById(match.leader) : null;
  const context =
    profileMatchDate(match.completed_at) +
    " · " +
    profileMatchDuration(match.duration_seconds);
  const statusModifier = verified
    ? match.won
      ? "is-win"
      : "is-loss"
    : "is-pending";
  const deltaMarkup =
    match.rating_delta == null
      ? ""
      : "<span class='sow-profile__match-delta " +
        (match.rating_delta > 0
          ? "is-positive"
          : match.rating_delta < 0
            ? "is-negative"
            : "") +
        "'>" +
        (match.ranked_points_breakdown
          ? esc(
              SOW_t("lobbies.rank_score", {
                score: (match.rating_delta > 0 ? "+" : "") + match.rating_delta,
              }),
            )
          : (match.rating_delta > 0 ? "+" : "") +
            esc(match.rating_delta) +
            " MMR") +
        "</span>";
  return (
    "<button type='button' class='sow-profile__match " +
    statusModifier +
    "' data-command='open_match' data-match-id='" +
    esc(match.match_id) +
    "'>" +
    "<span class='sow-profile__match-blade' aria-hidden='true'></span>" +
    "<span class='sow-profile__match-result " +
    statusModifier +
    "'>" +
    esc(result) +
    "</span>" +
    (leader
      ? "<div class='sow-profile__match-avatar-frame'><img class='sow-profile__match-avatar' src='" +
        esc(asset("gameplay/avatars/" + leaderArtSlug(leader) + ".webp")) +
        "' alt='' width='44' height='44' loading='lazy'></div>"
      : "<span class='sow-profile__match-avatar' aria-hidden='true'></span>") +
    "<span class='sow-profile__match-context'><strong>" +
    esc(mode) +
    "</strong><small>" +
    esc(map) +
    " · " +
    esc(match.queue || SOW_t("profile.matchmaking_queue")) +
    (match.rating_delta == null
      ? ""
      : " · " +
        (match.ranked_points_breakdown
          ? esc(
              SOW_t("lobbies.rank_score", {
                score: (match.rating_delta > 0 ? "+" : "") + match.rating_delta,
              }),
            )
          : (match.rating_delta > 0 ? "+" : "") +
            esc(match.rating_delta) +
            " MMR")) +
    "</small></span>" +
    "<span class='sow-profile__match-kda'><strong>" +
    esc(leader ? leaderDisplayName(leader) : "—") +
    "</strong><small>" +
    (verified
      ? esc(
          SOW_t("profile.kda_value", {
            kills: match.kills || 0,
            deaths: match.deaths || 0,
            assists: match.assists || 0,
          }),
        ) +
        " " +
        esc(SOW_t("profile.kda"))
      : "") +
    "</small></span>" +
    deltaMarkup +
    "<span class='sow-profile__match-time'>" +
    esc(context) +
    "</span>" +
    "<span class='sow-profile__match-chevron' aria-hidden='true'>›</span>" +
    "</button>"
  );
}

function profileHeaderMarkup(data: ProfileData | null, own: boolean | null) {
  const title = own
    ? SOW_t("profile.your_profile")
    : SOW_t("profile.player_profile");
  const snapshotError = profileSnapshotError();
  const displayName =
    data && data.display_name
      ? data.display_name
      : menuUi.profileSnapshotLoading
        ? SOW_t("profile.loading_profile")
        : SOW_t("profile.profile_unavailable");
  const handle =
    data && data.handle
      ? data.handle
      : menuUi.profileSnapshotLoading
        ? SOW_t("profile.fetching_player_data")
        : "";
  const level = data && data.level != null ? data.level : "—";
  const status =
    data && snapshotError
      ? "<div class='sow-profile__status' data-profile-status aria-live='polite'>" +
        esc(snapshotError) +
        " <button type='button' class='sow-profile__status-action' data-command='retry_profile'>" +
        esc(SOW_t("profile.try_again")) +
        "</button></div>"
      : "<div class='sow-profile__status' data-profile-status aria-live='polite' hidden></div>";
  const preferred =
    data && data.preferred_leader ? leaderById(data.preferred_leader) : null;
  const avatarUrl = preferred
    ? asset("gameplay/avatars/" + leaderArtSlug(preferred) + ".webp")
    : avatarImage();
  const rankedRating = (data?.ratings || []).find(function (r) {
    return r.queue === "Ranked1v1" && r.mode === "FFA";
  });
  const rankBadge =
    rankedRating && rankedRating.placements_complete
      ? "<span class='sow-profile__rank-badge' title='" +
        esc(rankLabelText(rankedRating)) +
        "'>" +
        rankEmblem(rankedRating.tier, rankedRating.division) +
        "<b>" +
        esc(rankName(rankedRating.tier, rankedRating.division)) +
        "</b>" +
        "</span>"
      : "";
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
  const heroArt = profileHeroImage(data && data.preferred_leader);
  return (
    "<section class='sow-profile__heading' data-profile-heading data-profile-cover aria-labelledby='sow-profile-title' style=\"--sow-hero:url('" +
    esc(heroArt) +
    "')\">" +
    "<div class='sow-profile__cover-overlay'></div>" +
    "<div class='sow-profile__heading-top'>" +
    "<div class='sow-profile__breadcrumbs'>" +
    "<span class='sow-profile__kicker'>" +
    esc(title) +
    "</span>" +
    (!own
      ? "<span class='sow-profile__viewing-tag'>· " +
        esc(displayName) +
        "</span>"
      : "") +
    "</div>" +
    "<div class='sow-profile__heading-actions'>" +
    myProfileButton +
    "<button type='button' class='sow-profile__back' data-command='close_profile'>" +
    esc(backLabel) +
    "</button>" +
    "</div>" +
    "</div>" +
    "<div class='sow-profile__identity-card'>" +
    "<div class='sow-profile__avatar-container'>" +
    "<div class='sow-profile__avatar-medallion'>" +
    "<img class='sow-profile__avatar' data-profile-avatar src='" +
    esc(avatarUrl) +
    "' alt='' width='88' height='88'>" +
    "</div>" +
    "<div class='sow-profile__level' title='" +
    esc(SOW_t("profile.level_label")) +
    "'><small>" +
    esc(SOW_t("profile.level_label")) +
    "</small><strong data-profile-level>" +
    esc(level) +
    "</strong></div>" +
    "</div>" +
    "<div class='sow-profile__identity-main'>" +
    "<div class='sow-profile__identity-copy'>" +
    "<div class='sow-profile__identity-title-row'>" +
    "<h1 id='sow-profile-title' data-profile-display-name>" +
    esc(displayName) +
    "</h1>" +
    rankBadge +
    "</div>" +
    "<p class='sow-profile__handle' data-profile-handle>" +
    (handle ? "@" + esc(handle.replace(/^@/, "")) : "") +
    "</p>" +
    (data && data.is_bot
      ? "<p class='sow-profile__bot-meta'><span>" +
        esc(SOW_t("profile.bot")) +
        "</span> · " +
        esc(SOW_t("profile.ranked_bot_iq", { iq: data.ranked_bot_iq || 0 })) +
        "</p>"
      : "") +
    status +
    "</div>" +
    "</div>" +
    "</div>" +
    profileStatsRibbonMarkup(data) +
    "</section>"
  );
}

function profileStatsRibbonMarkup(data: ProfileData | null) {
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
    esc(SOW_t("profile.matches")) +
    "</span></div>" +
    "</div>" +
    "<div class='sow-profile__stat-cell sow-profile__stat-wins'>" +
    "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M6 9H4.5a2.5 2.5 0 0 1 0-5H6'/><path d='M18 9h1.5a2.5 2.5 0 0 0 0-5H18'/><path d='M4 22h16'/><path d='M10 14.66V17c0 .55-.45 1-1 1H8c-.55 0-1 .45-1 1v1h10v-1c0-.55-.45-1-1-1h-1c-.55 0-1-.45-1-1v-2.34'/><path d='M18 4H6v7a6 6 0 0 0 12 0V4Z'/></svg></span>" +
    "<div class='sow-profile__stat-body'><strong>" +
    esc(wins) +
    "</strong><span>" +
    esc(SOW_t("profile.wins")) +
    "</span></div>" +
    "</div>" +
    "<div class='sow-profile__stat-cell sow-profile__stat-winrate'>" +
    "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><path d='m4.93 4.93 4.24 4.24'/><path d='m14.83 9.17 4.24-4.24'/><path d='m14.83 14.83 4.24 4.24'/><path d='m9.17 14.83-4.24 4.24'/><circle cx='12' cy='12' r='4'/></svg></span>" +
    "<div class='sow-profile__stat-body'><strong>" +
    esc(winRate) +
    "%</strong><span>" +
    esc(SOW_t("profile.win_rate")) +
    "</span>" +
    "<div class='sow-profile__stat-gauge' aria-hidden='true'><i style='width:" +
    winRate +
    "%'></i></div></div>" +
    "</div>" +
    "<div class='sow-profile__stat-cell sow-profile__stat-laurels'>" +
    "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M8 3a9 9 0 0 0-5 8c0 4.97 3.58 9 8 9M16 3a9 9 0 0 1 5 8c0 4.97-3.58 9-8 9M9 7l3 3M15 7l-3 3M8 12l4 2M16 12l-4 2'/></svg></span>" +
    "<div class='sow-profile__stat-body'><strong>" +
    esc(laurels) +
    "</strong><span>" +
    esc(SOW_t("profile.laurels")) +
    "</span></div>" +
    "</div>" +
    "<div class='sow-profile__stat-cell sow-profile__stat-kda'>" +
    "<span class='sow-profile__stat-icon' aria-hidden='true'><svg width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='22' y1='12' x2='18' y2='12'/><line x1='6' y1='12' x2='2' y2='12'/><line x1='12' y1='6' x2='12' y2='2'/><line x1='12' y1='22' x2='12' y2='18'/></svg></span>" +
    "<div class='sow-profile__stat-body'><strong><i>" +
    esc(data.kills || 0) +
    "</i><i>" +
    esc(data.deaths || 0) +
    "</i><i>" +
    esc(data.assists || 0) +
    "</i></strong><span>" +
    esc(SOW_t("profile.kda")) +
    "</span></div>" +
    "</div>" +
    "</div>"
  );
}

function achievementCategoryTitle(category: string): string {
  if (category === "battles")
    return SOW_t("profile.achievement_category_battles");
  if (category === "victories")
    return SOW_t("profile.achievement_category_victories");
  if (category === "commanders")
    return SOW_t("profile.achievement_category_commanders");
  if (category === "laurels")
    return SOW_t("profile.achievement_category_laurels");
  if (category === "combat") {
    const t = SOW_t("profile.achievement_category_combat");
    return t !== "profile.achievement_category_combat" ? t : "Combate";
  }
  if (category === "campaign") {
    const t = SOW_t("profile.achievement_category_campaign");
    return t !== "profile.achievement_category_campaign" ? t : "Campaña";
  }
  return category;
}

const ALL_HEROES: { slug: string; name: string }[] = [
  { slug: "caesar", name: "Caesar" },
  { slug: "cleopatra", name: "Cleopatra" },
  { slug: "ragnar", name: "Ragnar" },
  { slug: "sun_tzu", name: "Sun Tzu" },
  { slug: "alexander", name: "Alexander" },
  { slug: "genghis_khan", name: "Genghis Khan" },
  { slug: "richard_the_lionheart", name: "Richard the Lionheart" },
  { slug: "vercingetorix", name: "Vercingetorix" },
  { slug: "boudica", name: "Boudica" },
  { slug: "lady_six_sky", name: "Lady Six Sky" },
  { slug: "leonidas", name: "Leonidas" },
  { slug: "napoleon", name: "Napoleon" },
  { slug: "amina_de_zazzau", name: "Amina of Zazzau" },
  { slug: "yaa_asantewaa", name: "Yaa Asantewaa" },
  { slug: "anacaona", name: "Anacaona" },
  { slug: "lozen", name: "Lozen" },
  { slug: "tipu_sultan", name: "Tipu Sultan" },
  { slug: "william_the_conqueror", name: "William the Conqueror" },
  { slug: "hongi_hika", name: "Hongi Hika" },
  { slug: "salote_tupou_iii", name: "Sālote Tupou III" },
  { slug: "makeda", name: "Makeda" },
  { slug: "rani_durgavati", name: "Rani Durgavati" },
  { slug: "empress_gi", name: "Empress Gi" },
  { slug: "ashoka", name: "Ashoka" },
  { slug: "hannibal", name: "Hannibal" },
  { slug: "bolivar", name: "Simón Bolívar" },
  { slug: "joan_of_arc", name: "Joan of Arc" },
  { slug: "catherine_the_great", name: "Catherine the Great" },
  { slug: "isabella_of_castile", name: "Isabella I of Castile" },
  { slug: "cyrus_the_great", name: "Cyrus the Great" },
  { slug: "saladin", name: "Saladin" },
  { slug: "suleiman", name: "Suleiman the Magnificent" },
  { slug: "tomoe_gozen", name: "Tomoe Gozen" },
  { slug: "trieu_thi_trinh", name: "Lady Triệu" },
  { slug: "musa_i", name: "Mansa Musa" },
  { slug: "shaka", name: "Shaka" },
  { slug: "nzinga", name: "Nzinga" },
  { slug: "kamehameha", name: "Kamehameha I" },
  { slug: "gajah_mada", name: "Gajah Mada" },
  { slug: "solomon", name: "Solomon" },
  { slug: "sayyid_said", name: "Sayyid Said" },
  { slug: "lapu_lapu", name: "Lapu-Lapu" },
  { slug: "constantine", name: "Constantine" },
  { slug: "vlad_iii", name: "Vlad III" },
  { slug: "mary_of_burgundy", name: "Mary of Burgundy" },
  { slug: "semiramis", name: "Semiramis" },
  { slug: "shima_kalingga", name: "Shima" },
  { slug: "tomyris", name: "Tomyris" },
  { slug: "gorgo", name: "Gorgo" },
  { slug: "artigas", name: "Artigas" },
  { slug: "bernardo_ohiggins", name: "Bernardo O'Higgins" },
  { slug: "nezahualcoyotl", name: "Nezahualcóyotl" },
  { slug: "tlacaelel", name: "Tlacaelel" },
  { slug: "tenamaxtli", name: "Tenamaxtli" },
  { slug: "cosijoeza", name: "Cosijoeza" },
  { slug: "mangas_coloradas", name: "Mangas Coloradas" },
  { slug: "tomochichi", name: "Tomochichi" },
  { slug: "eight_deer", name: "Eight Deer" },
  { slug: "nemequene", name: "Nemequene" },
  { slug: "thanadelthur", name: "Thanadelthur" },
  { slug: "artemisia_i", name: "Artemisia I" },
  { slug: "shajar_al_durr", name: "Shajar al-Durr" },
  { slug: "ramesses_ii", name: "Ramesses II" },
  { slug: "qin_liangyu", name: "Qin Liangyu" },
  { slug: "lobengula", name: "Lobengula" },
  { slug: "shah_jahan", name: "Shah Jahan" },
  { slug: "xerxes_i", name: "Xerxes" },
  { slug: "marcus_aurelius", name: "Marcus Aurelius" },
  { slug: "justinian_ii_hero", name: "Justinian II" },
  { slug: "totila", name: "Totila" },
  { slug: "chilperic_i", name: "Chilperic I" },
  { slug: "lothair_i", name: "Lothair I" },
  { slug: "frederick_barbarossa", name: "Barbarossa" },
  { slug: "charles_v", name: "Charles V" },
  { slug: "wanli", name: "Wanli" },
  { slug: "jayaatu_khan", name: "Jayaatu Khan" },
  { slug: "sitting_bull", name: "Sitting Bull" },
  { slug: "matoaka", name: "Matoaka" },
  { slug: "burebista", name: "Burebista" },
  { slug: "boleslaw_ii", name: "Bolesław II" },
  { slug: "abd_al_rahman_al_ghafiqi", name: "Abd al-Rahman al-Ghafiqi" },
  { slug: "tribe_wiradjuri", name: "Windradyne" },
  { slug: "nation_yagan", name: "Yagan" },
  { slug: "peter_the_great", name: "Peter the Great" },
  { slug: "empress_suiko", name: "Empress Suiko" },
  { slug: "nebuchadnezzar_ii", name: "Nebuchadnezzar II" },
  { slug: "king_david", name: "King David" },
  { slug: "sarraounia_mangou", name: "Sarraounia Mangou" },
  { slug: "muhammad_of_ghor", name: "Muhammad of Ghor" },
  { slug: "puyi", name: "Puyi" },
  { slug: "pertinax", name: "Pertinax" },
  { slug: "ewuare", name: "Ewuare the Great" },
  { slug: "veleda", name: "Veleda" },
];

const MASTERY_TIERS: [number, number, string][] = [
  [3, 15, "Vanguard"],
  [10, 30, "Centurion"],
  [25, 50, "Veteran"],
  [50, 75, "Champion"],
  [100, 100, "Warlord"],
  [250, 200, "Warmaster"],
  [500, 350, "Juggernaut"],
  [1000, 1000, "Ascendant"],
];

const WIN_TIERS: [number, number, string][] = [
  [1, 25, "Victor"],
  [5, 50, "Triumph"],
  [25, 100, "Conqueror"],
  [100, 200, "Overlord"],
  [500, 500, "Imperator"],
  [1000, 1000, "Sovereign"],
];

function getSpecialHeroMasteryTitle(
  slug: string,
  target: number,
): string | null {
  const specials: Record<string, Record<number, string>> = {
    boudica: {
      3: "Wildfire",
      10: "Fury",
      25: "Iceni",
      50: "Uprising",
      100: "Vengeance",
      250: "War Queen",
      500: "Blood Mother",
      1000: "Nemesis",
    },
    caesar: {
      3: "Tribune",
      10: "Centurion",
      25: "Legatus",
      50: "Rubicon",
      100: "Imperium",
      250: "Dictator",
      500: "Colossus",
      1000: "Veni Vidi Vici",
    },
    lady_six_sky: {
      3: "Eclipse",
      10: "Sun Priest",
      25: "Starlight",
      50: "Wikal",
      100: "Cosmos",
      250: "Serpent",
      500: "Sky Empress",
      1000: "Ascension",
    },
    alexander: {
      3: "Phaistos",
      10: "Phalanx",
      25: "Gordian",
      50: "Issus",
      100: "Alexandria",
      250: "Hegemon",
      500: "Amun",
      1000: "World's End",
    },
    genghis_khan: {
      3: "Rider",
      10: "Steppe Wolf",
      25: "Karakorum",
      50: "Golden Horde",
      100: "Tengri",
      250: "Great Khan",
      500: "Earth Shaker",
      1000: "Scourge of God",
    },
    leonidas: {
      3: "Spartiate",
      10: "Shield Wall",
      25: "Molon Labe",
      50: "Hot Gates",
      100: "Thermopylae",
      250: "Lion of Sparta",
      500: "Immortal Bane",
      1000: "Last Stand",
    },
    sun_tzu: {
      3: "Strategist",
      10: "The Art of War",
      25: "Subtle Dragon",
      50: "Silent Wind",
      100: "Formless",
      250: "Grand Tactician",
      500: "Supreme Art",
      1000: "Master of War",
    },
    cleopatra: {
      3: "Nile Lotus",
      10: "Pharaoh's Grace",
      25: "Alexandrine",
      50: "Siren of Kings",
      100: "Ptolemaic Throne",
      250: "Queen of Kings",
      500: "Isis Reborn",
      1000: "Eternal Nile",
    },
    hannibal: {
      3: "Barca",
      10: "Alpine March",
      25: "Cannae",
      50: "Double Envelopment",
      100: "Rome's Terror",
      250: "Carthaginian Lion",
      500: "Ad Portas",
      1000: "Thunderbolt",
    },
    napoleon: {
      3: "Corsican",
      10: "Artillery Officer",
      25: "Austerlitz",
      50: "Sun of Austerlitz",
      100: "Grande Armée",
      250: "First Consul",
      500: "Emperor of France",
      1000: "Eagle of Destiny",
    },
  };
  return (specials[slug] && specials[slug][target]) || null;
}

let catalogCache: ProfileAchievement[] | null = null;

export function getFullAchievementsCatalog(): ProfileAchievement[] {
  if (catalogCache) return catalogCache;
  const list: ProfileAchievement[] = [
    // 9 Canonical Google Play / Core achievements
    {
      id: "first_command",
      category: "battles",
      title: "First Command",
      description: "Complete your first match.",
      points: 10,
      target: 1,
      unlocked: false,
      progress: 0,
    },
    {
      id: "first_victory",
      category: "victories",
      title: "First Victory",
      description: "Win a verified match.",
      points: 25,
      target: 1,
      unlocked: false,
      progress: 0,
    },
    {
      id: "battle_hardened",
      category: "battles",
      title: "Battle Hardened",
      description: "Complete 10 matches.",
      points: 25,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "victory_march",
      category: "victories",
      title: "Victory March",
      description: "Win 10 verified matches.",
      points: 50,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "laurel_hoard",
      category: "laurels",
      title: "Laurel Hoard",
      description: "Earn 100 achievement points.",
      points: 50,
      target: 100,
      unlocked: false,
      progress: 0,
    },
    {
      id: "commander_victorious",
      category: "victories",
      title: "Commander Victorious",
      description: "Win with a commander.",
      points: 25,
      target: 1,
      unlocked: false,
      progress: 0,
    },
    {
      id: "veteran_commander",
      category: "commanders",
      title: "Veteran Commander",
      description: "Complete 10 matches with commanders.",
      points: 50,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "banner_collector",
      category: "commanders",
      title: "Banner Collector",
      description: "Command three different leaders.",
      points: 50,
      target: 3,
      unlocked: false,
      progress: 0,
    },
    {
      id: "leader_path",
      category: "commanders",
      title: "Leader Path",
      description: "Earn 1,000 leader experience.",
      points: 100,
      target: 1000,
      unlocked: false,
      progress: 0,
    },

    // Tiered Battles Matrix
    {
      id: "battles_3",
      category: "battles",
      title: "Vanguard",
      description: "Complete 3 matches.",
      points: 15,
      target: 3,
      unlocked: false,
      progress: 0,
    },
    {
      id: "battles_25",
      category: "battles",
      title: "Ironclad",
      description: "Complete 25 matches.",
      points: 50,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "battles_50",
      category: "battles",
      title: "Centurion",
      description: "Complete 50 matches.",
      points: 75,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "battles_100",
      category: "battles",
      title: "Warlord",
      description: "Complete 100 matches.",
      points: 100,
      target: 100,
      unlocked: false,
      progress: 0,
    },
    {
      id: "battles_250",
      category: "battles",
      title: "Warmaster",
      description: "Complete 250 matches.",
      points: 200,
      target: 250,
      unlocked: false,
      progress: 0,
    },
    {
      id: "battles_500",
      category: "battles",
      title: "Juggernaut",
      description: "Complete 500 matches.",
      points: 350,
      target: 500,
      unlocked: false,
      progress: 0,
    },
    {
      id: "battles_1000",
      category: "battles",
      title: "Oblivion",
      description: "Complete 1,000 matches.",
      points: 1000,
      target: 1000,
      unlocked: false,
      progress: 0,
    },

    // Tiered Victories Matrix
    {
      id: "victories_3",
      category: "victories",
      title: "Blood Oath",
      description: "Win 3 verified matches.",
      points: 30,
      target: 3,
      unlocked: false,
      progress: 0,
    },
    {
      id: "victories_25",
      category: "victories",
      title: "Triumph",
      description: "Win 25 verified matches.",
      points: 75,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "victories_50",
      category: "victories",
      title: "Conqueror",
      description: "Win 50 verified matches.",
      points: 100,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "victories_100",
      category: "victories",
      title: "Overlord",
      description: "Win 100 verified matches.",
      points: 150,
      target: 100,
      unlocked: false,
      progress: 0,
    },
    {
      id: "victories_250",
      category: "victories",
      title: "Dominion",
      description: "Win 250 verified matches.",
      points: 250,
      target: 250,
      unlocked: false,
      progress: 0,
    },
    {
      id: "victories_500",
      category: "victories",
      title: "Imperator",
      description: "Win 500 verified matches.",
      points: 500,
      target: 500,
      unlocked: false,
      progress: 0,
    },
    {
      id: "victories_1000",
      category: "victories",
      title: "Sovereign",
      description: "Win 1,000 verified matches.",
      points: 1000,
      target: 1000,
      unlocked: false,
      progress: 0,
    },

    // Combat Supremacy
    {
      id: "kills_10",
      category: "combat",
      title: "First Blood",
      description: "Defeat 10 enemy units.",
      points: 20,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_25",
      category: "combat",
      title: "Skirmisher",
      description: "Defeat 25 enemy units.",
      points: 35,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_50",
      category: "combat",
      title: "Reaper",
      description: "Defeat 50 enemy units.",
      points: 50,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_100",
      category: "combat",
      title: "Slayer",
      description: "Defeat 100 enemy units.",
      points: 75,
      target: 100,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_250",
      category: "combat",
      title: "Executioner",
      description: "Defeat 250 enemy units.",
      points: 150,
      target: 250,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_500",
      category: "combat",
      title: "Warbringer",
      description: "Defeat 500 enemy units.",
      points: 250,
      target: 500,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_1000",
      category: "combat",
      title: "Death Incarnate",
      description: "Defeat 1,000 enemy units.",
      points: 500,
      target: 1000,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_2500",
      category: "combat",
      title: "Doom of Empires",
      description: "Defeat 2,500 enemy units.",
      points: 750,
      target: 2500,
      unlocked: false,
      progress: 0,
    },
    {
      id: "kills_5000",
      category: "combat",
      title: "Lord of the Grave",
      description: "Defeat 5,000 enemy units.",
      points: 1000,
      target: 5000,
      unlocked: false,
      progress: 0,
    },
    {
      id: "assists_10",
      category: "combat",
      title: "Wingman",
      description: "Record 10 combat assists.",
      points: 15,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "assists_50",
      category: "combat",
      title: "Vanguard Shield",
      description: "Record 50 combat assists.",
      points: 50,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "assists_250",
      category: "combat",
      title: "Tactical Hand",
      description: "Record 250 combat assists.",
      points: 150,
      target: 250,
      unlocked: false,
      progress: 0,
    },
    {
      id: "assists_1000",
      category: "combat",
      title: "Master of Cohorts",
      description: "Record 1,000 combat assists.",
      points: 500,
      target: 1000,
      unlocked: false,
      progress: 0,
    },
    {
      id: "players_5",
      category: "combat",
      title: "Duelist Initiate",
      description: "Eliminate 5 rival commanders.",
      points: 15,
      target: 5,
      unlocked: false,
      progress: 0,
    },
    {
      id: "players_10",
      category: "combat",
      title: "Duelist",
      description: "Eliminate 10 rival commanders.",
      points: 30,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "players_25",
      category: "combat",
      title: "Champion Slayer",
      description: "Eliminate 25 rival commanders.",
      points: 50,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "players_50",
      category: "combat",
      title: "Crown Breaker",
      description: "Eliminate 50 rival commanders.",
      points: 100,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "players_100",
      category: "combat",
      title: "Sovereign Hunter",
      description: "Eliminate 100 rival commanders.",
      points: 200,
      target: 100,
      unlocked: false,
      progress: 0,
    },
    {
      id: "players_250",
      category: "combat",
      title: "Kingmaker",
      description: "Eliminate 250 rival commanders.",
      points: 500,
      target: 250,
      unlocked: false,
      progress: 0,
    },
    {
      id: "empires_5",
      category: "combat",
      title: "Border Raider",
      description: "Overthrow 5 rival empires.",
      points: 25,
      target: 5,
      unlocked: false,
      progress: 0,
    },
    {
      id: "empires_10",
      category: "combat",
      title: "Empire Slayer",
      description: "Overthrow 10 rival empires.",
      points: 50,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "empires_25",
      category: "combat",
      title: "Dynasty Fall",
      description: "Overthrow 25 rival empires.",
      points: 100,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "empires_50",
      category: "combat",
      title: "World Breaker",
      description: "Overthrow 50 rival empires.",
      points: 250,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "empires_100",
      category: "combat",
      title: "Scourge of Realms",
      description: "Overthrow 100 rival empires.",
      points: 500,
      target: 100,
      unlocked: false,
      progress: 0,
    },
    {
      id: "tribes_10",
      category: "combat",
      title: "Chieftain Bane",
      description: "Subjugate 10 native tribes.",
      points: 25,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "tribes_25",
      category: "combat",
      title: "Subjugator",
      description: "Subjugate 25 native tribes.",
      points: 50,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "tribes_50",
      category: "combat",
      title: "Steppe Dominator",
      description: "Subjugate 50 native tribes.",
      points: 100,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "tribes_100",
      category: "combat",
      title: "Overlord of the Wild",
      description: "Subjugate 100 native tribes.",
      points: 250,
      target: 100,
      unlocked: false,
      progress: 0,
    },

    // Campaign Chronicles
    {
      id: "campaign_1",
      category: "campaign",
      title: "Pathfinder",
      description: "Complete 1 campaign episode.",
      points: 25,
      target: 1,
      unlocked: false,
      progress: 0,
    },
    {
      id: "campaign_3",
      category: "campaign",
      title: "Trailblazer",
      description: "Complete 3 campaign episodes.",
      points: 35,
      target: 3,
      unlocked: false,
      progress: 0,
    },
    {
      id: "campaign_5",
      category: "campaign",
      title: "Saga Victor",
      description: "Complete 5 campaign episodes.",
      points: 50,
      target: 5,
      unlocked: false,
      progress: 0,
    },
    {
      id: "campaign_10",
      category: "campaign",
      title: "Epic Historian",
      description: "Complete 10 campaign episodes.",
      points: 75,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "campaign_15",
      category: "campaign",
      title: "Chronicles of War",
      description: "Complete 15 campaign episodes.",
      points: 100,
      target: 15,
      unlocked: false,
      progress: 0,
    },
    {
      id: "campaign_25",
      category: "campaign",
      title: "Grand Campaigner",
      description: "Complete 25 campaign episodes.",
      points: 150,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "campaign_40",
      category: "campaign",
      title: "Master of History",
      description: "Complete 40 campaign episodes.",
      points: 250,
      target: 40,
      unlocked: false,
      progress: 0,
    },

    // Laurels & Prestige
    {
      id: "laurels_250",
      category: "laurels",
      title: "Gilded Crown",
      description: "Earn 250 achievement points.",
      points: 50,
      target: 250,
      unlocked: false,
      progress: 0,
    },
    {
      id: "laurels_500",
      category: "laurels",
      title: "Imperial Grace",
      description: "Earn 500 achievement points.",
      points: 100,
      target: 500,
      unlocked: false,
      progress: 0,
    },
    {
      id: "laurels_1000",
      category: "laurels",
      title: "Vault of Glory",
      description: "Earn 1,000 achievement points.",
      points: 200,
      target: 1000,
      unlocked: false,
      progress: 0,
    },
    {
      id: "laurels_2500",
      category: "laurels",
      title: "Golden Emperor",
      description: "Earn 2,500 achievement points.",
      points: 500,
      target: 2500,
      unlocked: false,
      progress: 0,
    },
    {
      id: "laurels_5000",
      category: "laurels",
      title: "Immortal Renown",
      description: "Earn 5,000 achievement points.",
      points: 1000,
      target: 5000,
      unlocked: false,
      progress: 0,
    },
    {
      id: "laurels_10000",
      category: "laurels",
      title: "Demigod of War",
      description: "Earn 10,000 achievement points.",
      points: 2500,
      target: 10000,
      unlocked: false,
      progress: 0,
    },

    // Hero Roster Milestones
    {
      id: "roster_1",
      category: "commanders",
      title: "First Recruit",
      description: "Command or own 1 hero.",
      points: 25,
      target: 1,
      unlocked: false,
      progress: 0,
    },
    {
      id: "roster_3",
      category: "commanders",
      title: "Cohort",
      description: "Command or own 3 heroes.",
      points: 50,
      target: 3,
      unlocked: false,
      progress: 0,
    },
    {
      id: "roster_5",
      category: "commanders",
      title: "Council of War",
      description: "Command or own 5 heroes.",
      points: 75,
      target: 5,
      unlocked: false,
      progress: 0,
    },
    {
      id: "roster_10",
      category: "commanders",
      title: "Grand Legion",
      description: "Command or own 10 heroes.",
      points: 100,
      target: 10,
      unlocked: false,
      progress: 0,
    },
    {
      id: "roster_25",
      category: "commanders",
      title: "High Command",
      description: "Command or own 25 heroes.",
      points: 200,
      target: 25,
      unlocked: false,
      progress: 0,
    },
    {
      id: "roster_50",
      category: "commanders",
      title: "Army of Legends",
      description: "Command or own 50 heroes.",
      points: 350,
      target: 50,
      unlocked: false,
      progress: 0,
    },
    {
      id: "roster_80",
      category: "commanders",
      title: "Pantheon of War",
      description: "Command or own 80 heroes.",
      points: 1000,
      target: 80,
      unlocked: false,
      progress: 0,
    },
  ];

  for (const hero of ALL_HEROES) {
    const slug = hero.slug;
    const name = hero.name;

    // 1. Recruit
    list.push({
      id: "recruit_" + slug,
      category: "commanders",
      title: "Recruit " + name,
      description: "Unlock or recruit " + name + " into your vanguard.",
      points: 25,
      target: 1,
      unlocked: false,
      progress: 0,
    });

    // 2. Mastery (3 to 1000)
    for (const [target, points, tierTitle] of MASTERY_TIERS) {
      const special = getSpecialHeroMasteryTitle(slug, target);
      const title = special || name + " " + tierTitle;
      list.push({
        id: slug + "_mastery_" + target,
        category: "commanders",
        title: title,
        description: "Command " + name + " in " + target + " battles.",
        points: points,
        target: target,
        unlocked: false,
        progress: 0,
      });
    }

    // 3. Wins (1 to 1000)
    for (const [target, points, tierTitle] of WIN_TIERS) {
      list.push({
        id: slug + "_wins_" + target,
        category: "victories",
        title: name + " " + tierTitle,
        description: "Win " + target + " battles commanding " + name + ".",
        points: points,
        target: target,
        unlocked: false,
        progress: 0,
      });
    }
  }

  catalogCache = list;
  return list;
}

export function resolveProfileAchievements(
  data: ProfileData,
): ProfileAchievement[] {
  if (data.achievements && data.achievements.length >= 1000) {
    return data.achievements;
  }
  const catalog = getFullAchievementsCatalog();
  const serverMap = new Map<string, ProfileAchievement>();
  if (Array.isArray(data.achievements)) {
    for (const ach of data.achievements) {
      if (ach && ach.id) {
        serverMap.set(ach.id, ach);
      }
    }
  }

  const matches = Math.max(0, Number(data.matches_played) || 0);
  const wins = Math.max(0, Number(data.wins) || 0);
  const kills = Math.max(0, Number(data.kills) || 0);
  const assists = Math.max(0, Number(data.assists) || 0);
  const playersDefeated = Math.max(0, Number(data.players_defeated) || 0);
  const empiresDefeated = Math.max(0, Number(data.empires_defeated) || 0);
  const tribesDefeated = Math.max(0, Number(data.tribes_defeated) || 0);
  const laurels = Math.max(0, Number(data.laurels) || 0);

  const leadersList = Array.isArray(data.leaders) ? data.leaders : [];
  const leaderMatchesMap = new Map<string, number>();
  const leaderWinsMap = new Map<string, number>();
  const ownedSet = new Set<string>();

  for (const l of leadersList) {
    if (l && l.leader) {
      const slug = String(l.leader).toLowerCase().trim();
      const m = Math.max(0, Number(l.matches_played) || 0);
      const w = Math.max(0, Number(l.wins) || 0);
      leaderMatchesMap.set(slug, m);
      leaderWinsMap.set(slug, w);
      if (m > 0 || w > 0 || (Number(l.xp) || 0) > 0) {
        ownedSet.add(slug);
      }
    }
  }

  if (
    menuUi.state &&
    menuUi.state.account_id === data.account_id &&
    Array.isArray(menuUi.state.leaders)
  ) {
    for (const l of menuUi.state.leaders) {
      if (l && l.slug && (l.owned || l.permanently_free)) {
        ownedSet.add(l.slug.toLowerCase().trim());
      }
    }
  }

  const ownedCount = Math.max(ownedSet.size, leadersList.length);
  const totalLeaderXp = leadersList.reduce(
    (acc, l) => acc + (Number(l.xp) || 0),
    0,
  );

  return catalog.map(function (def) {
    const srv = serverMap.get(def.id);
    let progress = 0;
    const target = def.target;

    if (def.id === "first_command") {
      progress = matches >= 1 ? 1 : 0;
    } else if (def.id === "first_victory") {
      progress = wins >= 1 ? 1 : 0;
    } else if (def.id === "battle_hardened") {
      progress = Math.min(10, matches);
    } else if (def.id === "victory_march") {
      progress = Math.min(10, wins);
    } else if (def.id === "commander_victorious") {
      progress = wins >= 1 ? 1 : 0;
    } else if (def.id === "veteran_commander") {
      progress = Math.min(10, matches);
    } else if (def.id === "banner_collector") {
      progress = Math.min(3, ownedCount);
    } else if (def.id === "leader_path") {
      progress = Math.min(1000, totalLeaderXp);
    } else if (def.id === "laurel_hoard") {
      progress = Math.min(100, laurels);
    } else if (def.id.startsWith("battles_")) {
      progress = Math.min(target, matches);
    } else if (def.id.startsWith("victories_")) {
      progress = Math.min(target, wins);
    } else if (def.id.startsWith("kills_")) {
      progress = Math.min(target, kills);
    } else if (def.id.startsWith("assists_")) {
      progress = Math.min(target, assists);
    } else if (def.id.startsWith("players_")) {
      progress = Math.min(target, playersDefeated);
    } else if (def.id.startsWith("empires_")) {
      progress = Math.min(target, empiresDefeated);
    } else if (def.id.startsWith("tribes_")) {
      progress = Math.min(target, tribesDefeated);
    } else if (def.id.startsWith("campaign_")) {
      progress = srv ? Number(srv.progress) || 0 : 0;
    } else if (def.id.startsWith("laurels_")) {
      progress = Math.min(target, laurels);
    } else if (def.id.startsWith("roster_")) {
      progress = Math.min(target, ownedCount);
    } else if (def.id.startsWith("recruit_")) {
      const heroSlug = def.id.slice(8);
      progress = ownedSet.has(heroSlug) ? 1 : 0;
    } else if (def.id.includes("_mastery_")) {
      const parts = def.id.split("_mastery_");
      const heroSlug = parts[0];
      progress = Math.min(target, leaderMatchesMap.get(heroSlug) || 0);
    } else if (def.id.includes("_wins_")) {
      const parts = def.id.split("_wins_");
      const heroSlug = parts[0];
      progress = Math.min(target, leaderWinsMap.get(heroSlug) || 0);
    } else {
      progress = srv ? Number(srv.progress) || 0 : 0;
    }

    if (srv && Number(srv.progress) > progress) {
      progress = Number(srv.progress);
    }

    const unlocked = !!(srv && srv.unlocked) || progress >= target;

    return {
      id: def.id,
      category: def.category,
      title: def.title,
      description: def.description,
      points: def.points,
      target: def.target,
      progress: Math.min(target, progress),
      unlocked: unlocked,
    };
  });
}

function profileAchievementCard(achievement: ProfileAchievement) {
  const category = achievement.category || "commanders";
  const prefix = "profile.achievement_" + achievement.id;
  const progress = Math.max(0, Number(achievement.progress) || 0);
  const target = Math.max(1, Number(achievement.target) || 1);
  const percent = Math.min(100, Math.round((progress / target) * 100));
  const unlocked = !!achievement.unlocked;
  const isMythic = target >= 1000;

  const rawTitle = SOW_t(prefix + "_title");
  const title =
    rawTitle !== prefix + "_title"
      ? rawTitle
      : achievement.title || achievement.id;
  const rawDesc = SOW_t(prefix + "_description");
  const desc =
    rawDesc !== prefix + "_description"
      ? rawDesc
      : achievement.description || "";

  let badgeIcon = "";
  let heroSlug: string | null = null;
  if (achievement.id.startsWith("recruit_")) {
    heroSlug = achievement.id.slice(8);
  } else if (achievement.id.includes("_mastery_")) {
    heroSlug = achievement.id.split("_mastery_")[0];
  } else if (achievement.id.includes("_wins_")) {
    heroSlug = achievement.id.split("_wins_")[0];
  }

  if (heroSlug) {
    badgeIcon =
      "<img src='" +
      esc(asset("gameplay/avatars/" + heroSlug + ".webp")) +
      "' alt='' width='36' height='36' class='sow-profile__ach-hero-avatar' loading='lazy' />";
  } else if (category === "battles") {
    badgeIcon =
      "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
      "<path d='M6.92 5L5 6.92l4.95 4.95-1.41 1.41L5 9.75 3.59 11.16l4.24 4.24 1.41-1.41-3.53-3.54 1.41-1.41 4.95 4.95L13.99 12 6.92 5zm11.75 1.58L17.25 5 12.3 9.95l1.41 1.41 3.54-3.53 1.41 1.41-4.95 4.95 1.92 1.92 7.07-7.07-4.23-4.24-1.41 1.41 3.53 3.53-1.41 1.41zM3 19.5L4.5 21l3.5-3.5-1.5-1.5L3 19.5zm16.5 0l-3.5-3.5-1.5 1.5 3.5 3.5 1.5-1.5z'/>" +
      "</svg>";
  } else if (category === "victories") {
    badgeIcon =
      "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
      "<path d='M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94A5.01 5.01 0 0011 15.9V19H7v2h10v-2h-4v-3.1a5.01 5.01 0 003.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z'/>" +
      "</svg>";
  } else if (category === "combat") {
    badgeIcon =
      "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
      "<path d='M12 2C6.49 2 2 6.49 2 12s4.49 10 10 10 10-4.49 10-10S17.51 2 12 2zm1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z'/>" +
      "</svg>";
  } else if (category === "campaign") {
    badgeIcon =
      "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
      "<path d='M20.5 3l-.16.03L15 5.1 9 3 3.36 4.9c-.21.07-.36.25-.36.48V20.5c0 .28.22.5.5.5l.16-.03L9 18.9l6 2.1 5.64-1.9c.21-.07.36-.25.36-.48V3.5c0-.28-.22-.5-.5-.5zM15 19l-6-2.11V5l6 2.11V19z'/>" +
      "</svg>";
  } else if (category === "laurels") {
    badgeIcon =
      "<img src='" +
      esc(currencyAsset("laurel")) +
      "' alt='' width='28' height='28' class='sow-profile__ach-laurel-badge' />";
  } else {
    badgeIcon =
      "<svg class='sow-profile__ach-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'>" +
      "<path d='M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 3c1.66 0 3 1.34 3 3s-1.34 3-3 3-3-1.34-3-3 1.34-3 3-3zm0 14.2c-2.5 0-4.71-1.28-6-3.22.03-1.99 4-3.08 6-3.08 1.99 0 5.97 1.09 6 3.08-1.29 1.94-3.5 3.22-6 3.22z'/>" +
      "</svg>";
  }

  return {
    category: category,
    markup:
      "<article class='sow-profile__achievement" +
      (unlocked ? " is-unlocked" : " is-locked") +
      (isMythic ? " is-mythic" : "") +
      "'>" +
      "<div class='sow-profile__achievement-header'>" +
      "<div class='sow-profile__achievement-badge-frame" +
      (unlocked ? " is-unlocked" : "") +
      (isMythic ? " is-mythic" : "") +
      "'>" +
      badgeIcon +
      (unlocked
        ? "<span class='sow-profile__achievement-check' aria-hidden='true'>✓</span>"
        : "") +
      "</div>" +
      "<div class='sow-profile__achievement-info'>" +
      "<div class='sow-profile__achievement-top-row'>" +
      "<div class='sow-profile__achievement-title-wrap'>" +
      "<strong>" +
      esc(title) +
      "</strong>" +
      (isMythic
        ? "<span class='sow-profile__achievement-mythic-pill'>MYTHIC</span>"
        : "") +
      "</div>" +
      "<span class='sow-profile__achievement-reward-pill" +
      (isMythic ? " is-mythic" : "") +
      "'>" +
      "<img src='" +
      esc(currencyAsset("laurel")) +
      "' alt='' width='14' height='14' />" +
      "<span>+" +
      esc(achievement.points) +
      "</span>" +
      "</span>" +
      "</div>" +
      "<span>" +
      esc(desc) +
      "</span>" +
      "</div>" +
      "</div>" +
      "<div class='sow-profile__achievement-progress-wrap'>" +
      "<div class='sow-profile__achievement-track" +
      (isMythic ? " is-mythic" : "") +
      "' role='progressbar' aria-valuenow='" +
      esc(progress) +
      "' aria-valuemax='" +
      esc(target) +
      "'>" +
      "<i style='width: " +
      esc(percent) +
      "%'></i>" +
      "</div>" +
      "<div class='sow-profile__achievement-meta'>" +
      "<span class='sow-profile__achievement-status" +
      (unlocked ? " is-done" : "") +
      "'>" +
      (unlocked ? "COMPLETADO" : percent + "%") +
      "</span>" +
      "<b>" +
      esc(progress) +
      "/" +
      esc(target) +
      " · " +
      esc(achievement.points) +
      " " +
      esc(SOW_t("profile.laurels")) +
      "</b>" +
      "</div>" +
      "</div>" +
      "</article>",
  };
}

function profileAchievementsMarkup(data: ProfileData, complete: boolean) {
  if (!complete)
    return (
      "<p class='sow-profile__empty'>" +
      esc(profileSnapshotError() || SOW_t("profile.loading_profile")) +
      "</p>"
    );
  const allAchievements = resolveProfileAchievements(data);
  const unlockedCount = allAchievements.filter(function (a) {
    return !!a.unlocked;
  }).length;
  const totalCount = allAchievements.length;
  const percent =
    totalCount > 0 ? Math.round((unlockedCount / totalCount) * 100) : 0;
  const laurels = profileStat(data.laurels);

  const summaryDossier =
    "<div class='sow-profile__achievements-dossier'>" +
    "<div class='sow-profile__achievements-laurel-card'>" +
    "<div class='sow-profile__achievements-laurel-icon'>" +
    "<img src='" +
    esc(currencyAsset("laurel")) +
    "' alt='' width='40' height='40' />" +
    "</div>" +
    "<div class='sow-profile__achievements-laurel-info'>" +
    "<span class='sow-profile__achievements-laurel-title'>" +
    esc(SOW_t("profile.laurels")) +
    "</span>" +
    "<strong class='sow-profile__achievements-laurel-val'>" +
    esc(laurels) +
    "</strong>" +
    "</div>" +
    "</div>" +
    "<div class='sow-profile__achievements-progress-card'>" +
    "<div class='sow-profile__achievements-progress-header'>" +
    "<span>" +
    esc(SOW_t("profile.achievements")) +
    "</span>" +
    "<strong>" +
    esc(unlockedCount) +
    " / " +
    esc(totalCount) +
    " (" +
    esc(percent) +
    "%)</strong>" +
    "</div>" +
    "<div class='sow-profile__achievements-progress-track' role='progressbar' aria-valuenow='" +
    esc(percent) +
    "' aria-valuemin='0' aria-valuemax='100'>" +
    "<i style='width: " +
    esc(percent) +
    "%'></i>" +
    "</div>" +
    "</div>" +
    "</div>";

  const allCategories = [
    "battles",
    "victories",
    "commanders",
    "combat",
    "campaign",
    "laurels",
  ];
  const grouped: Record<string, string[]> = {};
  const categoryTotals: Record<string, number> = {};
  allCategories.forEach(function (cat) {
    grouped[cat] = [];
    categoryTotals[cat] = 0;
  });

  allAchievements.forEach(function (achievement) {
    const cat = grouped[achievement.category]
      ? achievement.category
      : "commanders";
    categoryTotals[cat] = (categoryTotals[cat] || 0) + 1;
  });

  const searchQuery = (menuUi.profileAchievementSearch || "")
    .trim()
    .toLowerCase();
  const filteredAchievements = searchQuery
    ? allAchievements.filter(function (a) {
        return (
          (a.title || "").toLowerCase().includes(searchQuery) ||
          (a.description || "").toLowerCase().includes(searchQuery) ||
          a.id.toLowerCase().includes(searchQuery)
        );
      })
    : allAchievements;

  filteredAchievements.forEach(function (achievement) {
    const card = profileAchievementCard(achievement);
    const cat = grouped[card.category] ? card.category : "commanders";
    grouped[cat].push(card.markup);
  });

  const activeFilter = menuUi.profileAchievementFilter || "all";
  const filterTabs = [
    { id: "all", label: "Todos", count: totalCount },
    {
      id: "battles",
      label: achievementCategoryTitle("battles"),
      count: categoryTotals.battles || 0,
    },
    {
      id: "victories",
      label: achievementCategoryTitle("victories"),
      count: categoryTotals.victories || 0,
    },
    {
      id: "commanders",
      label: achievementCategoryTitle("commanders"),
      count: categoryTotals.commanders || 0,
    },
    {
      id: "combat",
      label: achievementCategoryTitle("combat"),
      count: categoryTotals.combat || 0,
    },
    {
      id: "campaign",
      label: achievementCategoryTitle("campaign"),
      count: categoryTotals.campaign || 0,
    },
    {
      id: "laurels",
      label: achievementCategoryTitle("laurels"),
      count: categoryTotals.laurels || 0,
    },
  ];

  const searchBox =
    "<div class='sow-profile__ach-search-wrap'>" +
    "<input type='search' class='sow-profile__ach-search-input' placeholder='" +
    esc(SOW_t("heroes.search_leaders")) +
    "...' value='" +
    esc(menuUi.profileAchievementSearch || "") +
    "' data-role='achievements-search' />" +
    "</div>";

  const filterChips =
    "<nav class='sow-profile__ach-filter-bar' aria-label='Categorías de Logros'>" +
    filterTabs
      .filter(function (tab) {
        return tab.id === "all" || tab.count > 0;
      })
      .map(function (tab) {
        const isActive = activeFilter === tab.id;
        return (
          "<button type='button' class='sow-profile__ach-filter-chip" +
          (isActive ? " is-active" : "") +
          "' data-command='profile_achievement_filter' data-achievement-filter='" +
          esc(tab.id) +
          "'>" +
          "<span>" +
          esc(tab.label) +
          "</span>" +
          "<b class='sow-profile__ach-filter-count'>" +
          esc(tab.count) +
          "</b>" +
          "</button>"
        );
      })
      .join("") +
    "</nav>";

  const categoriesToRender =
    activeFilter === "all" ? allCategories : [activeFilter];

  const groups = categoriesToRender
    .map(function (category) {
      const cards = grouped[category] || [];
      if (!cards.length) return "";
      return (
        "<section class='sow-profile__achievement-group'><h3>" +
        esc(achievementCategoryTitle(category)) +
        "</h3><div class='sow-profile__achievement-grid'>" +
        cards.join("") +
        "</div></section>"
      );
    })
    .join("");

  return (
    summaryDossier +
    searchBox +
    filterChips +
    (groups ||
      "<p class='sow-profile__empty'>" +
        esc(SOW_t("profile.achievements")) +
        "</p>")
  );
}

function profileContentMarkup(data: ProfileData | null) {
  const entry = activeProfileEntry();
  const complete = profileDataComplete();
  let content = "";
  if (data && menuUi.profileTab === "overview") {
    const ranked = (data.ratings || []).find(function (rating) {
      return rating.queue === "Ranked1v1" && rating.mode === "FFA";
    });
    const rankedPanel = complete
      ? rankOverview(
          ranked || null,
          data.ranked_ladder,
          data.account_id === menuUi.state?.account_id,
        )
      : "";
    const recent = complete
      ? menuUi.profileHistory.slice(0, 10).map(profileMatchRow).join("") ||
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
          data.leaders.slice(0, 3).map(profileLeaderCard).join("") +
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
      ? (data.leaders || []).map(profileLeaderCard).join("") ||
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
    const historyList = !complete
      ? "<p class='sow-profile__empty'>" +
        esc(profileSnapshotError() || SOW_t("profile.loading_profile")) +
        "</p>"
      : menuUi.profileHistory.map(profileMatchRow).join("") ||
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
      "</span></div><div class='sow-profile__history'>" +
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
    const rankedRows =
      profileRankedLeaderboard === null
        ? "<p class='sow-profile__empty'>" +
          esc(
            profileRankedLeaderboardError ||
              SOW_t("profile.leaderboard_loading"),
          ) +
          "</p>"
        : profileRankedLeaderboard
            .map(function (entry) {
              return (
                "<button type='button' class='sow-profile__victory-row sow-profile__ranked-row" +
                (entry.account_id === menuUi.state?.account_id
                  ? " is-self"
                  : "") +
                "' data-command='open_profile' data-account-id='" +
                esc(entry.account_id) +
                "'><b>#" +
                esc(entry.rank) +
                "</b>" +
                rankEmblem(entry.tier, entry.division) +
                "<div><strong>" +
                esc(entry.handle || entry.account_id) +
                "</strong><span>" +
                (entry.is_bot
                  ? "<i>" + esc(SOW_t("profile.bot")) + "</i> · "
                  : "") +
                esc(rankName(entry.tier, entry.division)) +
                " · " +
                esc(entry.games_played) +
                " " +
                esc(SOW_t("profile.matches")) +
                "</span></div><strong>" +
                esc(SOW_t("lobbies.rank_score", { score: entry.score })) +
                "</strong></button>"
              );
            })
            .join("") ||
          "<p class='sow-profile__empty'>" +
            esc(SOW_t("profile.no_ranked_leaders")) +
            "</p>";
    const victoryRows =
      profileVictoryLeaderboard === null
        ? "<p class='sow-profile__empty'>" +
          esc(
            profileVictoryLeaderboardError ||
              SOW_t("profile.leaderboard_loading"),
          ) +
          "</p>"
        : profileVictoryLeaderboard
            .map(function (entry) {
              return (
                "<article class='sow-profile__victory-row'><b>#" +
                esc(entry.rank) +
                "</b><div><strong>" +
                esc(entry.handle || entry.account_id) +
                "</strong><span>" +
                esc(SOW_t("profile.level", { level: entry.level || 1 })) +
                "</span></div><strong>" +
                esc(entry.wins || 0) +
                " " +
                esc(SOW_t("profile.wins")) +
                "</strong><small>" +
                esc(entry.matches_played || 0) +
                " " +
                esc(SOW_t("profile.matches")) +
                "</small></article>"
              );
            })
            .join("") ||
          "<p class='sow-profile__empty'>" +
            esc(SOW_t("profile.no_victory_leaders")) +
            "</p>";
    const victoryAction =
      profileVictoryLeaderboardHasMore || profileVictoryLeaderboardError
        ? "<button type='button' class='sow-profile__load-more' data-command='load_victory_more'" +
          (profileVictoryLeaderboardLoading ? " disabled" : "") +
          ">" +
          esc(
            profileVictoryLeaderboardLoading
              ? SOW_t("profile.loading_more")
              : profileVictoryLeaderboardError
                ? SOW_t("profile.try_again")
                : SOW_t("profile.load_more_matches"),
          ) +
          "</button>"
        : "";
    content =
      "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
      esc(SOW_t("profile.ranked_leaderboard")) +
      "</h2><span class='sow-profile__section-note'>Ranked 1v1</span></div><div class='sow-profile__victory-list'>" +
      rankedRows +
      "</div>" +
      (profileRankedLeaderboardError
        ? "<button type='button' class='sow-menu__secondary' data-command='retry_ranked_board'>" +
          esc(SOW_t("profile.try_again")) +
          "</button>"
        : "") +
      "</section>" +
      "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" +
      esc(SOW_t("profile.leaderboards")) +
      "</h2><span class='sow-profile__section-note'>" +
      esc(SOW_t("profile.verified_wins")) +
      "</span></div><div class='sow-profile__victory-list'>" +
      victoryRows +
      "</div>" +
      victoryAction +
      "</section>";
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

export function renderProfile() {
  const data = menuUi.profileData;
  const own =
    menuUi.state && menuUi.state.account_id === menuUi.profileAccountId;
  const header = profileHeaderMarkup(data, own);
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
    "<main class='sow-menu__main sow-profile__main' data-screen-panel='profile'><section class='sow-profile__page'><div class='sow-profile__dossier'>" +
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
        ? data.handle
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
    if (avatar.src !== avatarUrl) avatar.src = avatarUrl;
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
    statsContainer.outerHTML = profileStatsRibbonMarkup(data);
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
  if (content) content.innerHTML = profileContentMarkup(data);
}

export function renderProfileDetail() {
  const detail = menuUi.profileMatchDetail;
  if (!detail) {
    if (!menuUi.profileDetailError) return "";
    return (
      "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='" +
      esc(SOW_t("profile.close_match_details")) +
      "'>×</button><span class='sow-profile__kicker'>" +
      esc(SOW_t("profile.match_details")) +
      "</span><h2 id='sow-profile-detail-title'>" +
      esc(SOW_t("profile.unavailable")) +
      "</h2><p class='sow-profile__empty' aria-live='polite'>" +
      esc(menuUi.profileDetailError) +
      "</p><button type='button' class='sow-profile__load-more' data-command='open_match' data-match-id='" +
      esc(menuUi.profileDetailId) +
      "'>" +
      esc(SOW_t("profile.try_again")) +
      "</button></section></div>"
    );
  }
  return (
    "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='" +
    esc(SOW_t("profile.close_match_details")) +
    "'>×</button><span class='sow-profile__kicker'>" +
    esc(SOW_t("profile.match_details")) +
    "</span><h2 id='sow-profile-detail-title'>" +
    esc(detail.mode || SOW_t("profile.match")) +
    "</h2><p>" +
    esc(formatMapName(detail)) +
    " · " +
    esc(detail.queue || SOW_t("profile.matchmaking_queue")) +
    (detail.ranked_bot_iq == null
      ? ""
      : " · " +
        esc(SOW_t("profile.ranked_bot_iq", { iq: detail.ranked_bot_iq }))) +
    " · " +
    esc(profileMatchDate(detail.completed_at)) +
    " · " +
    esc(profileMatchDuration(detail.duration_seconds)) +
    "</p><div class='sow-profile__detail-players'>" +
    (detail.participants || [])
      .map(function (participant) {
        const status = detail.verified
          ? participant.won
            ? SOW_t("profile.win")
            : SOW_t("profile.loss")
          : SOW_t(
              detail.provisional
                ? "profile.match_pending"
                : "profile.match_unverified",
            );
        const stats = detail.verified
          ? SOW_t("profile.kda_value", {
              kills: participant.kills || 0,
              deaths: participant.deaths || 0,
              assists: participant.assists || 0,
            })
          : "";
        const name =
          detail.queue === "Ranked1v1"
            ? "<button type='button' class='sow-profile__player-profile' data-command='open_profile' data-account-id='" +
              esc(participant.account_id) +
              "'>" +
              esc(participant.handle || participant.account_id) +
              "</button>"
            : "<strong>" +
              esc(participant.handle || participant.account_id) +
              "</strong>";
        return (
          "<div>" +
          name +
          "<span>" +
          esc(
            participant.leader
              ? leaderDisplayName(leaderById(participant.leader))
              : "—",
          ) +
          (stats ? " · " + esc(stats) : "") +
          (participant.rating_delta == null
            ? ""
            : " · " +
              (participant.ranked_points_breakdown
                ? esc(
                    SOW_t("lobbies.rank_score", {
                      score:
                        (participant.rating_delta > 0 ? "+" : "") +
                        participant.rating_delta,
                    }),
                  )
                : (participant.rating_delta > 0 ? "+" : "") +
                  esc(participant.rating_delta) +
                  " MMR")) +
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

// Other-player profiles can be reported here; account deletion lives in Settings.
function renderModeration(own: boolean | null) {
  if (
    own ||
    menuUi.profileData?.is_bot ||
    !menuUi.profileAccountId ||
    !selfCreds()
  )
    return "";
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
