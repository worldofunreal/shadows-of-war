import {
  rankEmblem,
  rankLabel as rankLabelText,
  rankName,
  rankOverview,
  rankedPointsBreakdownMarkup,
} from "./main_menu.ranked.ts";
import {
  WOW_PILLAR_ORDER,
  WOW_SUBCATEGORIES,
  enrichAchievementsList,
  getFullAchievementsCatalog,
  matchesAchievementFilters,
  renderAchievementCard,
  renderAchievementChainCard,
  renderHeroMasteryProfileCard,
  resolveProfileAchievements,
  sortAchievementsList,
  summarizeEnrichedHeroAchievements,
  tText,
  wowCategoryTitle,
  wowSubcategoryTitle,
  type EnrichedAchievement,
  type ProfileAchievement,
  type WowCategoryId,
  type WowConcretePillar,
} from "./main_menu.achievements.ts";
import { avatarImageAttribute, setAvatarImage } from "./avatar-presentation.ts";
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
  MenuLeader,
  MenuState,
  RankedLadder,
  RankedPointsBreakdown,
} from "./main_menu.state.ts";

export { getFullAchievementsCatalog, resolveProfileAchievements };
export type { ProfileAchievement };

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

function profileLeaderCard(leader: ProfileLeader) {
  const summary = leader || {};
  const leaderInfo = leaderById(summary.leader);
  const winRate = Math.round((summary.win_rate || 0) * 100);
  const level = 1 + Math.floor((summary.xp || 0) / 100);
  return (
    "<article class='sow-profile__leader'>" +
    "<div class='sow-profile__leader-avatar-frame'>" +
    "<img " +
    avatarImageAttribute(asset("gameplay/avatars/" + leaderArtSlug(leaderInfo) + ".webp")) +
    " alt='' width='52' height='52' loading='lazy'>" +
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
      ? "<div class='sow-profile__match-avatar-frame'><img class='sow-profile__match-avatar' " +
        avatarImageAttribute(asset("gameplay/avatars/" + leaderArtSlug(leader) + ".webp")) +
        " alt='' width='44' height='44' loading='lazy'></div>"
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
      ? "<span class='sow-profile__rank-badge' tabindex='0' data-sow-tooltip='" +
        esc(rankLabelText(rankedRating)) +
        "' data-sow-tooltip-icon='ranked' data-sow-tooltip-tone='gold' data-sow-tooltip-tap='true'>" +
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
    "<img class='sow-profile__avatar' data-profile-avatar " +
    avatarImageAttribute(avatarUrl) +
    " alt='' width='88' height='88'>" +
    "</div>" +
    "<div class='sow-profile__level' tabindex='0' data-sow-tooltip='" +
    esc(SOW_t("profile.level_label") + " " + level) +
    "' data-sow-tooltip-tone='gold' data-sow-tooltip-tap='true'><small>" +
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

function achHeroEntrySlugMap(): Map<string, MenuLeader> {
  const map = new Map<string, MenuLeader>();
  const leaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
  for (const leader of leaders) {
    if (!leader) continue;
    if (leader.slug) map.set(String(leader.slug).toLowerCase(), leader);
    if (leader.id) map.set(String(leader.id).toLowerCase(), leader);
  }
  return map;
}

function normalizeWowCategory(raw: string | undefined): WowCategoryId {
  if (
    raw === "summary" ||
    raw === "general" ||
    raw === "pvp_combat" ||
    raw === "pve_campaign" ||
    raw === "heroes_mastery" ||
    raw === "prestige" ||
    raw === "all"
  ) {
    return raw;
  }
  if (raw === "battles" || raw === "victories") return "general";
  if (raw === "combat") return "pvp_combat";
  if (raw === "campaign") return "pve_campaign";
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

function achPillarCounts(
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

function achSubCounts(
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

function renderAchRailMarkup(
  enriched: EnrichedAchievement[],
  activeCategory: WowCategoryId,
  activeSubcategory: string,
  isOwnProfile: boolean,
  playableOnly: boolean,
): string {
  const railCategories: WowCategoryId[] = [
    "summary",
    ...WOW_PILLAR_ORDER,
    "all",
  ];
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
        "'>" +
        "<button type='button' class='sow-profile__ach-rail-btn" +
        (isActive ? " is-active" : "") +
        "' data-command='profile_achievement_filter' data-achievement-filter='" +
        esc(cat) +
        "'>" +
        "<div class='sow-profile__ach-rail-top'>" +
        "<span>" +
        esc(wowCategoryTitle(cat)) +
        "</span>" +
        "<b>" +
        esc(counts.unlocked + "/" + counts.total) +
        "</b>" +
        "</div>" +
        "<div class='sow-profile__ach-rail-bar'><i style='width:" +
        esc(counts.percent) +
        "%'></i></div>" +
        "</button>" +
        subItems +
        "</div>"
      );
    })
    .join("");
}

function renderAchSubbarMarkup(
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

function renderAchSummaryView(
  enriched: EnrichedAchievement[],
  isOwnProfile: boolean,
): string {
  const pillarCards = WOW_PILLAR_ORDER.map(function (pillar) {
    const stats = achPillarCounts(enriched, pillar, isOwnProfile, false);
    return (
      "<button type='button' class='sow-profile__ach-pillar-card' data-command='profile_achievement_filter' data-achievement-filter='" +
      esc(pillar) +
      "'>" +
      "<div class='sow-profile__ach-pillar-head'>" +
      "<strong>" +
      esc(wowCategoryTitle(pillar)) +
      "</strong>" +
      "<span>" +
      esc(stats.percent) +
      "%</span>" +
      "</div>" +
      "<div class='sow-profile__achievement-track'><i style='width:" +
      esc(stats.percent) +
      "%'></i></div>" +
      "<div class='sow-profile__ach-pillar-foot'>" +
      "<span>" +
      esc(stats.unlocked + " / " + stats.total) +
      "</span>" +
      "<b>" +
      esc(stats.earnedLaurels + " " + tText("profile.laurels", "Laurels")) +
      "</b>" +
      "</div>" +
      "</button>"
    );
  }).join("");

  const almostLocked = enriched
    .filter(function (a) {
      if (a.unlocked) return false;
      if (a.progress > 0) return true;
      if (!isOwnProfile) return false;
      // If brand new player with 0 progress anywhere, suggest quick general/hero starters
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
    .sort(function (a, b) {
      if (b.points !== a.points) return b.points - a.points;
      return b.target - a.target;
    })
    .slice(0, 6);

  return (
    "<section class='sow-profile__ach-summary-section'>" +
    "<h3 class='sow-profile__ach-subhead'>" +
    esc(tText("profile.ach_summary_pillars", "Progress by Pillar")) +
    "</h3>" +
    "<div class='sow-profile__ach-summary-grid'>" +
    pillarCards +
    "</div>" +
    "</section>" +
    "<section class='sow-profile__ach-summary-section'>" +
    "<h3 class='sow-profile__ach-subhead'>" +
    esc(tText("profile.ach_summary_almost", "Close to Unlocking")) +
    "</h3>" +
    (almostLocked.length
      ? "<div class='sow-profile__achievement-grid'>" +
        almostLocked
          .map((a) =>
            renderAchievementCard(a, { tagText: wowCategoryTitle(a.wowCategory) }),
          )
          .join("") +
        "</div>"
      : "<p class='sow-profile__empty'>" +
        esc(
          tText(
            "profile.ach_summary_almost_empty",
            "Play matches or command heroes to start tracking nearby milestones.",
          ),
        ) +
        "</p>") +
    "</section>" +
    "<section class='sow-profile__ach-summary-section'>" +
    "<h3 class='sow-profile__ach-subhead'>" +
    esc(tText("profile.ach_summary_highlights", "Earned Highlights")) +
    "</h3>" +
    (highlights.length
      ? "<div class='sow-profile__achievement-grid'>" +
        highlights
          .map((a) =>
            renderAchievementCard(a, { tagText: wowCategoryTitle(a.wowCategory) }),
          )
          .join("") +
        "</div>"
      : "<p class='sow-profile__empty'>" +
        esc(
          tText(
            "profile.ach_summary_highlights_empty",
            "Complete your first milestone to showcase it here.",
          ),
        ) +
        "</p>") +
    "</section>"
  );
}

let achFlatLimit = 90;

export function resetAchievementFlatLimit() {
  achFlatLimit = 90;
}

export function bumpAchievementFlatLimit() {
  achFlatLimit += 90;
}

function renderAchResultsMarkup(
  enriched: EnrichedAchievement[],
  isOwnProfile: boolean,
): string {
  const activeCategory = normalizeWowCategory(menuUi.profileAchievementFilter);
  const activeSub = menuUi.profileAchievementSubcategory || "all";
  const statusFilter = menuUi.profileAchievementStatus || "all";
  const tierFilter = menuUi.profileAchievementTier || "all";
  const sortBy = menuUi.profileAchievementSort || "smart";
  const playableOnly = Boolean(menuUi.profileAchievementPlayableOnly) && isOwnProfile;
  const viewMode = menuUi.profileAchievementViewMode === "grid" ? "grid" : "chains";
  const searchQuery = (menuUi.profileAchievementSearch || "").trim();

  const hasToolbarFilters =
    Boolean(searchQuery) ||
    statusFilter !== "all" ||
    tierFilter !== "all" ||
    playableOnly;

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
      ? "<div class='sow-profile__ach-search-banner'>" +
        "<span>" +
        esc(
          tText("profile.ach_searching_global", "Searching entire codex ({count} results)", {
            count: scoped.length,
          }),
        ) +
        "</span>" +
        "<button type='button' class='sow-profile__ach-clear-btn' data-command='profile_achievement_clear'>" +
        esc(tText("profile.ach_clear_filters", "Clear filters")) +
        "</button>" +
        "</div>"
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
      "<div class='sow-profile__ach-empty-box'>" +
      "<p class='sow-profile__empty'>" +
      esc(
        tText(
          "profile.ach_empty_filtered",
          "No achievements match your current search or filters.",
        ),
      ) +
      "</p>" +
      "<div class='sow-profile__ach-empty-actions'>" +
      (globalMatches > 0
        ? "<button type='button' class='sow-profile__ach-clear-btn is-primary' data-command='profile_achievement_filter' data-achievement-filter='all'>" +
          esc(wowCategoryTitle("all") + " (" + globalMatches + ")") +
          "</button>"
        : "") +
      (hasToolbarFilters
        ? "<button type='button' class='sow-profile__ach-clear-btn' data-command='profile_achievement_clear'>" +
          esc(tText("profile.ach_clear_filters", "Clear filters")) +
          "</button>"
        : "") +
      "</div>" +
      "</div>"
    );
  }

  // Hero Mastery grouped cards in Chains mode (1 card per hero instead of 15 separate cards)
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
    const leaderLookup = achHeroEntrySlugMap();
    const summaries = Array.from(byHero.entries()).map(function ([slug, list]) {
      const leader = leaderLookup.get(slug) || leaderById(slug);
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
      // "smart": active/played/unlocked heroes first, then playable, then % & alpha
      const aProg = a.unlockedCount > 0 || a.matchesPlayed > 0 ? 1 : 0;
      const bProg = b.unlockedCount > 0 || b.matchesPlayed > 0 ? 1 : 0;
      if (aProg !== bProg) return bProg - aProg;
      if (b.unlockedCount !== a.unlockedCount) return b.unlockedCount - a.unlockedCount;
      if (isOwnProfile && a.playable !== b.playable) return a.playable ? -1 : 1;
      if (b.nextPercent !== a.nextPercent) return b.nextPercent - a.nextPercent;
      return a.name.localeCompare(b.name);
    });

    const visibleHeroes = summaries.slice(0, achFlatLimit);
    const loadMoreHeroes =
      summaries.length > visibleHeroes.length
        ? "<div class='sow-profile__ach-loadmore-wrap'><button type='button' class='sow-profile__load-more' data-command='profile_achievement_show_more'>" +
          esc(tText("profile.view_all", "View all") + " (" + visibleHeroes.length + "/" + summaries.length + ")") +
          "</button></div>"
        : "";
    return (
      "<div class='sow-profile__ach-hero-grid'>" +
      visibleHeroes.map((s) => renderHeroMasteryProfileCard(s, isOwnProfile)).join("") +
      "</div>" +
      loadMoreHeroes
    );
  }

  // Chain / Ladder mode inside categories when status/tier filters aren't narrowing to individual tiers
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
    // Order active/unlocked chains before untouched ones
    chainEntries.sort(function ([kA, aList], [kB, bList]) {
      if (sortBy === "alpha_asc") return kA.localeCompare(kB);
      const aProgress = aList.some((i) => i.unlocked || i.progress > 0) ? 1 : 0;
      const bProgress = bList.some((i) => i.unlocked || i.progress > 0) ? 1 : 0;
      if (aProgress !== bProgress) return bProgress - aProgress;
      return aList.length - bList.length;
    });

    const limitedChains = chainEntries.slice(0, achFlatLimit);
    const cardsHtml =
      limitedChains
        .map(([key, items]) =>
          renderAchievementChainCard(
            key,
            items,
            menuUi.profileAchievementSelectedTiers[key] || null,
          ),
        )
        .join("") +
      sortAchievementsList(singles, sortBy)
        .map((a) => renderAchievementCard(a))
        .join("");

    const loadMoreChains =
      chainEntries.length > limitedChains.length
        ? "<div class='sow-profile__ach-loadmore-wrap'><button type='button' class='sow-profile__load-more' data-command='profile_achievement_show_more'>" +
          esc(tText("profile.view_all", "View all") + " (" + limitedChains.length + "/" + chainEntries.length + ")") +
          "</button></div>"
        : "";
    return (
      "<div class='sow-profile__achievement-grid'>" + cardsHtml + "</div>" + loadMoreChains
    );
  }

  // Flat / Filtered individual achievement cards
  const sorted = sortAchievementsList(scoped, sortBy);
  const visible = sorted.slice(0, achFlatLimit);
  const moreBtn =
    sorted.length > visible.length
      ? "<div class='sow-profile__ach-loadmore-wrap'><button type='button' class='sow-profile__load-more' data-command='profile_achievement_show_more'>" +
        esc(tText("profile.view_all", "View all") + " (" + visible.length + "/" + sorted.length + ")") +
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
  const activeCategory = normalizeWowCategory(menuUi.profileAchievementFilter);
  const activeSub = menuUi.profileAchievementSubcategory || "all";
  const playableOnly = Boolean(menuUi.profileAchievementPlayableOnly) && isOwnProfile;

  const rail = panel.querySelector("[data-ach-rail]");
  if (rail) {
    rail.innerHTML = renderAchRailMarkup(
      enriched,
      activeCategory,
      activeSub,
      isOwnProfile,
      playableOnly,
    );
  }
  const subbar = panel.querySelector("[data-ach-subbar]");
  if (subbar) {
    const html = renderAchSubbarMarkup(
      enriched,
      activeCategory,
      activeSub,
      isOwnProfile,
      playableOnly,
    );
    subbar.innerHTML = html;
    (subbar as HTMLElement).hidden = !html;
  }
  const playableBtn = panel.querySelector<HTMLButtonElement>("[data-command='profile_achievement_toggle_playable']");
  if (playableBtn) {
    playableBtn.classList.toggle("is-active", playableOnly);
    playableBtn.setAttribute("aria-pressed", String(playableOnly));
  }
  const modeBtns = panel.querySelectorAll<HTMLElement>("[data-command='profile_achievement_view_mode']");
  const viewMode = menuUi.profileAchievementViewMode === "grid" ? "grid" : "chains";
  for (let i = 0; i < modeBtns.length; i++) {
    const active = modeBtns[i].dataset.viewMode === viewMode;
    modeBtns[i].classList.toggle("is-active", active);
    modeBtns[i].setAttribute("aria-pressed", String(active));
  }
  const results = panel.querySelector("[data-ach-results]");
  if (results) {
    results.innerHTML = renderAchResultsMarkup(enriched, isOwnProfile);
  }
}

function profileAchievementsMarkup(data: ProfileData, complete: boolean) {
  if (!complete)
    return (
      "<p class='sow-profile__empty'>" +
      esc(profileSnapshotError() || SOW_t("profile.loading_profile")) +
      "</p>"
    );
  const isOwnProfile = Boolean(menuUi.state && data.account_id === menuUi.state.account_id);
  const rawAchievements = resolveProfileAchievements(data);
  const enriched = enrichAchievementsList(rawAchievements, isOwnProfile);
  const unlockedCount = enriched.filter((a) => a.unlocked).length;
  const totalCount = enriched.length;
  const percent = totalCount > 0 ? Math.round((unlockedCount / totalCount) * 100) : 0;
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

  const activeCategory = normalizeWowCategory(menuUi.profileAchievementFilter);
  const activeSub = menuUi.profileAchievementSubcategory || "all";
  const statusFilter = menuUi.profileAchievementStatus || "all";
  const tierFilter = menuUi.profileAchievementTier || "all";
  const sortBy = menuUi.profileAchievementSort || "smart";
  const playableOnly = Boolean(menuUi.profileAchievementPlayableOnly) && isOwnProfile;
  const viewMode = menuUi.profileAchievementViewMode === "grid" ? "grid" : "chains";

  const categoryOptions = (
    ["summary", ...WOW_PILLAR_ORDER, "all"] as WowCategoryId[]
  ).map(function (cat) {
    const c = achPillarCounts(enriched, cat, isOwnProfile, playableOnly);
    return {
      value: cat,
      label: wowCategoryTitle(cat) + " (" + c.unlocked + "/" + c.total + ")",
      shortLabel: wowCategoryTitle(cat),
    };
  });

  const compactCategoryDropdown = SOW_renderDropdown({
    key: "ach-category",
    label: tText("profile.ach_categories_label", "Achievement categories"),
    name: "ach_category",
    value: activeCategory,
    className: "sow-profile__ach-mobile-cat",
    options: categoryOptions,
  });

  const statusDropdown = SOW_renderDropdown({
    key: "ach-status",
    label: tText("profile.ach_filter_status", "Filter by progress"),
    name: "ach_status",
    value: statusFilter,
    className: "sow-profile__ach-dd" + (statusFilter !== "all" ? " is-filtered" : ""),
    options: [
      { value: "all", label: tText("profile.ach_status_all", "All States") },
      { value: "in_progress", label: tText("profile.ach_status_in_progress", "In Progress") },
      { value: "unlocked", label: tText("profile.ach_status_unlocked", "Completed") },
      { value: "locked", label: tText("profile.ach_status_locked", "Not Started") },
    ],
  });

  const tierDropdown = SOW_renderDropdown({
    key: "ach-tier",
    label: tText("profile.ach_filter_tier", "Filter by tier"),
    name: "ach_tier",
    value: tierFilter,
    className: "sow-profile__ach-dd" + (tierFilter !== "all" ? " is-filtered" : ""),
    options: [
      { value: "all", label: tText("profile.ach_tier_all", "All Tiers") },
      { value: "mythic", label: tText("profile.ach_tier_mythic", "Mythic (200+)") },
      { value: "elite", label: tText("profile.ach_tier_elite", "Elite (50–199)") },
      { value: "standard", label: tText("profile.ach_tier_standard", "Standard (<50)") },
    ],
  });

  const sortDropdown = SOW_renderDropdown({
    key: "ach-sort",
    label: tText("profile.ach_sort_label", "Sort achievements"),
    name: "ach_sort",
    value: sortBy,
    className: "sow-profile__ach-dd",
    options: [
      { value: "smart", label: tText("profile.ach_sort_smart", "Smart Order") },
      { value: "closest", label: tText("profile.ach_sort_closest", "Closest (%)") },
      { value: "points_desc", label: tText("profile.ach_sort_points", "Highest Laurels") },
      { value: "alpha_asc", label: tText("profile.ach_sort_alpha", "A-Z") },
    ],
  });

  const subbarHtml = renderAchSubbarMarkup(
    enriched,
    activeCategory,
    activeSub,
    isOwnProfile,
    playableOnly,
  );

  const toolbar =
    "<div class='sow-profile__ach-toolbar' data-ach-toolbar>" +
    "<div class='sow-profile__ach-search-wrap'>" +
    "<span class='sow-profile__ach-search-icon' aria-hidden='true'><svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='11' cy='11' r='8'/><line x1='21' y1='21' x2='16.65' y2='16.65'/></svg></span>" +
    "<input type='search' class='sow-profile__ach-search-input' placeholder='" +
    esc(tText("profile.search_achievements", "Search achievements, heroes, or regions…")) +
    "' value='" +
    esc(menuUi.profileAchievementSearch || "") +
    "' data-role='achievements-search' autocomplete='off' spellcheck='false' />" +
    "</div>" +
    "<div class='sow-profile__ach-controls'>" +
    compactCategoryDropdown +
    statusDropdown +
    tierDropdown +
    sortDropdown +
    (isOwnProfile
      ? "<button type='button' class='sow-heroes__playable-btn sow-profile__ach-playable" +
        (playableOnly ? " is-active" : "") +
        "' data-command='profile_achievement_toggle_playable' aria-pressed='" +
        String(playableOnly) +
        "'>" +
        esc(tText("profile.ach_playable_only", "Owned / Playable")) +
        "</button>"
      : "") +
    "<div class='sow-profile__ach-view-toggle' role='group'>" +
    "<button type='button' class='sow-profile__ach-view-btn" +
    (viewMode === "chains" ? " is-active" : "") +
    "' data-command='profile_achievement_view_mode' data-view-mode='chains' aria-pressed='" +
    String(viewMode === "chains") +
    "'>" +
    esc(tText("profile.ach_view_chains", "Chains")) +
    "</button>" +
    "<button type='button' class='sow-profile__ach-view-btn" +
    (viewMode === "grid" ? " is-active" : "") +
    "' data-command='profile_achievement_view_mode' data-view-mode='grid' aria-pressed='" +
    String(viewMode === "grid") +
    "'>" +
    esc(tText("profile.ach_view_cards", "All Cards")) +
    "</button>" +
    "</div>" +
    "</div>" +
    "</div>";

  return (
    summaryDossier +
    "<div class='sow-profile__ach-layout'>" +
    "<aside class='sow-profile__ach-rail' data-ach-rail aria-label='" +
    esc(tText("profile.ach_categories_label", "Achievement categories")) +
    "'>" +
    renderAchRailMarkup(enriched, activeCategory, activeSub, isOwnProfile, playableOnly) +
    "</aside>" +
    "<div class='sow-profile__ach-main'>" +
    toolbar +
    "<div class='sow-profile__ach-subbar' data-ach-subbar" +
    (subbarHtml ? "" : " hidden") +
    ">" +
    subbarHtml +
    "</div>" +
    "<div class='sow-profile__ach-results' data-ach-results aria-live='polite'>" +
    renderAchResultsMarkup(enriched, isOwnProfile) +
    "</div>" +
    "</div>" +
    "</div>"
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
