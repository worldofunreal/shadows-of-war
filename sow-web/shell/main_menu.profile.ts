// Profile data, rendering, history, and match details.

    let profileVictoryLeaderboard = null;
    let profileVictoryLeaderboardLoading = false;
    let profileVictoryLeaderboardError = "";
    let profileVictoryLeaderboardRequest = null;
    let profileVictoryLeaderboardCursor = 0;
    let profileVictoryLeaderboardHasMore = false;

    function profileApi(path) {
        const base = String(window.SOW_DATABASE_URL || "/api").replace(/\/$/, "");
        return base + path;
    }

    function profileCacheEntry(id) {
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
                detailRequests: Object.create(null)
            };
        }
        return profileCache[id];
    }

    function profileStat(value) {
        const number = Number(value);
        return Number.isFinite(number) ? Math.max(0, Math.floor(number)) : 0;
    }

    function profileSeedFromState(id) {
        if (!state || state.account_id !== id) return null;
        const stats = state.profile_stats || {};
        const matches = profileStat(stats.matches_played);
        const wins = profileStat(stats.wins);
        return {
            account_id: id,
            handle: state.player_name || "",
            display_name: state.player_name || SOW_t("menu.anonymous"),
            level: profileStat(state.level) || 1,
            matches_played: matches,
            wins: wins,
            win_rate: matches ? wins / matches : 0,
            kills: profileStat(stats.kills),
            deaths: profileStat(stats.deaths),
            assists: profileStat(stats.assists),
            laurels: profileStat(state.laurels),
            players_defeated: profileStat(stats.players_defeated),
            empires_defeated: profileStat(stats.empires_defeated),
            tribes_defeated: profileStat(stats.tribes_defeated),
            preferred_leader: state.selected_leader || null,
            leaders: [],
            recent_matches: []
        };
    }

    function cacheProfileSeed(id, seed) {
        const entry = profileCacheEntry(id);
        if (!entry || !seed || entry.complete) return entry;
        entry.data = Object.assign({}, entry.data || {}, seed);
        return entry;
    }

    function activateProfileEntry(id) {
        const entry = profileCacheEntry(id);
        if (!entry) return null;
        profileData = entry.data;
        profileHistory = entry.history;
        return entry;
    }

    function activeProfileEntry() {
        return profileCacheEntry(profileAccountId);
    }

    function profileDataComplete() {
        const entry = activeProfileEntry();
        return !!(entry && entry.complete);
    }

    function profileSnapshotError() {
        const entry = activeProfileEntry();
        return entry ? entry.error : "";
    }

    function applyProfileSnapshot(id, data) {
        const entry = profileCacheEntry(id);
        if (!entry) return;
        const recent = Array.isArray(data.recent_matches) ? data.recent_matches.slice() : [];
        entry.data = data;
        entry.complete = true;
        entry.stale = false;
        entry.error = "";
        entry.history = recent;
        entry.historyCursor = recent.length;
        entry.historyLoaded = true;
        entry.historyHasMore = profileStat(data.matches_played) > recent.length;
        entry.historyError = "";
        if (profileAccountId === id) activateProfileEntry(id);
    }

    function loadProfile(id) {
        const entry = profileCacheEntry(id);
        if (!entry) return Promise.resolve(null);
        if (entry.complete && !entry.stale) {
            if (profileAccountId === id) activateProfileEntry(id);
            return Promise.resolve(entry.data);
        }
        if (entry.request) {
            if (profileAccountId === id) profileSnapshotLoading = true;
            return entry.request;
        }
        if (profileAccountId === id) {
            profileSnapshotLoading = true;
        }
        entry.request = fetch(profileApi("/profiles/" + encodeURIComponent(id)), {
            headers: { "Accept": "application/json" }
        }).then(function (response) {
            if (!response.ok) throw new Error("profile request failed");
            return response.json();
        }).then(function (data) {
            if (!data || typeof data !== "object") throw new Error("invalid profile response");
            applyProfileSnapshot(id, data);
            return data;
        }).catch(function () {
            entry.error = SOW_t("profile.profile_unavailable");
            return null;
        }).finally(function () {
            entry.request = null;
            if (profileOpen && profileAccountId === id) {
                profileSnapshotLoading = false;
                activateProfileEntry(id);
                syncProfileDom();
            }
        });
        return entry.request;
    }

    function loadMoreProfileHistory() {
        const id = profileAccountId;
        const entry = activeProfileEntry();
        if (!id || !entry || profileHistoryLoading || entry.historyRequest) return;
        if (!entry.complete || entry.stale) {
            profileHistoryLoading = true;
            loadProfile(id).then(function (data) {
                if (profileOpen && profileAccountId === id) {
                    profileHistoryLoading = false;
                    if (data) loadMoreProfileHistory();
                    else syncProfileDom();
                }
            });
            return;
        }
        if (!entry.historyHasMore) return;
        profileHistoryLoading = true;
        entry.historyError = "";
        entry.historyRequest = fetch(profileApi("/profiles/" + encodeURIComponent(id) + "/matches?cursor=" + entry.historyCursor + "&limit=20"), {
            headers: { "Accept": "application/json" }
        }).then(function (response) {
            if (!response.ok) throw new Error("history request failed");
            return response.json();
        }).then(function (data) {
            const items = Array.isArray(data.items) ? data.items : [];
            entry.history = entry.history.concat(items);
            entry.historyCursor = data.next_cursor == null ? entry.historyCursor + items.length : Number(data.next_cursor);
            entry.historyLoaded = true;
            entry.historyHasMore = data.next_cursor != null && items.length > 0;
            if (profileAccountId === id) activateProfileEntry(id);
        }).catch(function () {
            entry.historyError = SOW_t("profile.match_history_unavailable");
        }).finally(function () {
            entry.historyRequest = null;
            if (profileOpen && profileAccountId === id) {
                profileHistoryLoading = false;
                syncProfileDom();
            }
        });
    }

    function loadVictoryLeaderboard(append?) {
        if (profileVictoryLeaderboardRequest || (profileVictoryLeaderboard !== null && !append)) return;
        const appendItems = !!append && profileVictoryLeaderboard !== null;
        if (appendItems && !profileVictoryLeaderboardHasMore) return;
        const cursor = appendItems ? profileVictoryLeaderboardCursor : 0;
        profileVictoryLeaderboardLoading = true;
        profileVictoryLeaderboardError = "";
        profileVictoryLeaderboardRequest = fetch(profileApi("/seasons/1/leaderboard?kind=victories&cursor=" + cursor + "&limit=25"), {
            headers: { "Accept": "application/json" }
        }).then(function (response) {
            if (!response.ok) throw new Error("victories leaderboard failed");
            return response.json();
        }).then(function (data) {
            const items = Array.isArray(data.items) ? data.items : [];
            profileVictoryLeaderboard = appendItems ? profileVictoryLeaderboard.concat(items) : items;
            profileVictoryLeaderboardCursor = data.next_cursor == null ? cursor : Number(data.next_cursor);
            profileVictoryLeaderboardHasMore = data.next_cursor != null && items.length > 0;
        }).catch(function () {
            profileVictoryLeaderboardError = SOW_t("profile.leaderboard_unavailable");
        }).finally(function () {
            profileVictoryLeaderboardRequest = null;
            profileVictoryLeaderboardLoading = false;
            if (profileOpen) syncProfileDom();
        });
    }

    function invalidateProfileCache(id) {
        const entry = profileCacheEntry(id);
        if (!entry) return;
        entry.stale = true;
        entry.error = "";
        entry.historyLoaded = false;
        entry.historyError = "";
    }

    function syncProfilePreload(nextState) {
        const nextId = nextState && nextState.account_id ? nextState.account_id : null;
        const previousId = profileOwnId;
        const returnedToMenu = profileLastPhase && profileLastPhase !== "MainMenu" && nextState.phase === "MainMenu";
        if (previousId && previousId !== nextId && profileOpen && profileAccountId === previousId) {
            profileOpen = false;
            profileAccountId = null;
            profileData = null;
            profileHistory = [];
            profileMatchDetail = null;
            profileDetailError = "";
            profileDetailId = null;
            profileDetailLoading = false;
            profileDetailRequestKey = null;
        }
        if (previousId && previousId !== nextId) invalidateProfileCache(previousId);
        profileOwnId = nextId;
        profileLastPhase = nextState.phase;
        if (!nextId) return;
        const entry = cacheProfileSeed(nextId, profileSeedFromState(nextId));
        if (returnedToMenu) invalidateProfileCache(nextId);
        if (entry && !entry.complete) {
            if (profileAccountId === nextId) activateProfileEntry(nextId);
        }
        if (previousId !== nextId || returnedToMenu || (entry && !entry.complete && !entry.request && !entry.error)) {
            loadProfile(nextId);
        }
    }

    function openProfile(id) {
        const targetId = id || (state && state.account_id);
        if (!targetId) return;
        reportOpen = false;
        reportTarget = null;
        reportSent = false;
        deleteArmed = false;
        try {
            if (typeof window.SOW_isBlockedId === "function" && window.SOW_isBlockedId(targetId)) {
                profileOpen = true;
                profileAccountId = targetId;
                profileTab = "overview";
                const blockedEntry = profileCacheEntry(targetId);
                blockedEntry.data = null;
                blockedEntry.complete = false;
                blockedEntry.stale = false;
                blockedEntry.error = SOW_t("profile.blocked_player");
                profileData = null;
                profileHistory = blockedEntry.history = [];
                blockedEntry.historyCursor = 0;
                blockedEntry.historyLoaded = true;
                blockedEntry.historyHasMore = false;
                profileMatchDetail = null;
                profileDetailError = "";
                profileDetailId = null;
                profileSnapshotLoading = false;
                profileHistoryLoading = false;
                profileDetailLoading = false;
                profileDetailRequestKey = null;
                render();
                return;
            }
        } catch {}
        profileOpen = true;
        profileAccountId = targetId;
        profileTab = "overview";
        profileSnapshotLoading = false;
        profileHistoryLoading = false;
        profileDetailLoading = false;
        profileMatchDetail = null;
        profileDetailError = "";
        profileDetailId = null;
        profileDetailRequestKey = null;
        if (state && state.account_id === targetId) invalidateProfileCache(targetId);
        cacheProfileSeed(targetId, profileSeedFromState(targetId));
        activateProfileEntry(targetId);
        loadProfile(targetId);
        render();
    }

    function isBlockedAccountId(accountId) {
        try {
            return typeof window.SOW_isBlockedId === "function" && window.SOW_isBlockedId(accountId);
        } catch {
            return false;
        }
    }

    function profileLeaderCard(leader) {
        const summary = leader || {};
        const leaderInfo = leaderById(summary.leader);
        return "<article class='sow-profile__leader'>" +
            "<img src='" + esc(asset("gameplay/avatars/" + leaderInfo.slug + ".webp")) + "' alt='' width='48' height='48' loading='lazy'>" +
            "<div><strong>" + esc(leaderDisplayName(leaderInfo)) + "</strong>" +
            "<span>" + esc(SOW_t("profile.commander_stats", { matches: summary.matches_played || 0, wins: summary.wins || 0, rate: Math.round((summary.win_rate || 0) * 100) })) + "</span></div>" +
            "<b>" + esc(SOW_t("profile.level", { level: 1 + Math.floor((summary.xp || 0) / 100) })) + "</b>" +
            "</article>";
    }

    function profileMatchDate(timestamp) {
        const seconds = Number(timestamp);
        if (!Number.isFinite(seconds) || seconds <= 0) return "—";
        try {
            return new Intl.DateTimeFormat(document.documentElement.lang || undefined, { dateStyle: "medium" }).format(new Date(seconds * 1000));
        } catch {
            return new Date(seconds * 1000).toLocaleDateString();
        }
    }

    function profileMatchDuration(seconds) {
        const duration = Math.max(0, Math.floor(Number(seconds) || 0));
        if (!duration) return "—";
        return Math.floor(duration / 60) + "m " + (duration % 60) + "s";
    }

    function profileMatchRow(match) {
        const verified = !!match.verified;
        const result = verified
            ? (match.won ? SOW_t("profile.win") : SOW_t("profile.loss"))
            : SOW_t(match.provisional ? "profile.match_pending" : "profile.match_unverified");
        const mode = match.mode || SOW_t("profile.ffa");
        const map = formatMapName(match);
        const leader = match.leader ? leaderById(match.leader) : null;
        const context = profileMatchDate(match.completed_at) + " · " + profileMatchDuration(match.duration_seconds);
        return "<button type='button' class='sow-profile__match' data-command='open_match' data-match-id='" + esc(match.match_id) + "'>" +
            "<span class='sow-profile__match-result " + (verified ? (match.won ? "is-win" : "is-loss") : "is-pending") + "'>" + esc(result) + "</span>" +
            (leader ? "<img class='sow-profile__match-avatar' src='" + esc(asset("gameplay/avatars/" + leader.slug + ".webp")) + "' alt='' width='40' height='40' loading='lazy'>" : "<span class='sow-profile__match-avatar' aria-hidden='true'></span>") +
            "<span class='sow-profile__match-context'><strong>" + esc(mode) + "</strong><small>" + esc(map) + " · " + esc(match.queue || SOW_t("profile.matchmaking_queue")) + "</small></span>" +
            "<span class='sow-profile__match-kda'><strong>" + esc(leader ? leaderDisplayName(leader) : "—") + "</strong><small>" + (verified ? esc(SOW_t("profile.kda_value", { kills: match.kills || 0, deaths: match.deaths || 0, assists: match.assists || 0 })) + " " + esc(SOW_t("profile.kda")) : "") + "</small></span>" +
            "<span class='sow-profile__match-time'>" + esc(context) + "</span>" +
            "</button>";
    }

    function profileHeaderMarkup(data, own) {
        const title = own ? SOW_t("profile.your_profile") : SOW_t("profile.player_profile");
        const snapshotError = profileSnapshotError();
        const displayName = data && data.display_name ? data.display_name : (profileSnapshotLoading ? SOW_t("profile.loading_profile") : SOW_t("profile.profile_unavailable"));
        const handle = data && data.handle ? data.handle : (profileSnapshotLoading ? SOW_t("profile.fetching_player_data") : "");
        const level = data && data.level != null ? data.level : "—";
        const status = data && snapshotError
            ? "<div class='sow-profile__status' data-profile-status aria-live='polite'>" + esc(snapshotError) + " <button type='button' class='sow-profile__status-action' data-command='retry_profile'>" + esc(SOW_t("profile.try_again")) + "</button></div>"
            : "<div class='sow-profile__status' data-profile-status aria-live='polite' hidden></div>";
        return "<section class='sow-profile__heading' data-profile-heading aria-labelledby='sow-profile-title'><div class='sow-profile__identity-card'><div class='sow-profile__heading-top'><span class='sow-profile__kicker'>" + esc(title) + "</span><button type='button' class='sow-profile__back' data-command='close_profile'>" + esc(SOW_t("lobbies.back")) + "</button></div><div class='sow-profile__identity-main'><div class='sow-profile__identity-copy'><h1 id='sow-profile-title' data-profile-display-name>" + esc(displayName) + "</h1><p class='sow-profile__handle' data-profile-handle>" + esc(handle) + "</p><div data-profile-preferred>" + profilePreferredCommanderMarkup(data) + "</div>" + status + "</div><div class='sow-profile__level'><small>" + esc(SOW_t("profile.level_label")) + "</small><strong data-profile-level>" + esc(level) + "</strong></div></div></div></section>";
    }

    function profilePreferredCommanderMarkup(data) {
        const preferred = data && data.preferred_leader ? leaderById(data.preferred_leader) : null;
        return preferred ? "<p class='sow-profile__preferred'><img src='" + esc(asset("gameplay/avatars/" + preferred.slug + ".webp")) + "' alt='' width='40' height='40' loading='lazy'><span>" + esc(SOW_t("profile.preferred_commander")) + " · " + esc(leaderDisplayName(preferred)) + "</span></p>" : "";
    }

    function profileAchievementCard(achievement) {
        const category = achievement.category || "commanders";
        const prefix = "profile.achievement_" + achievement.id;
        const progress = Math.max(0, Number(achievement.progress) || 0);
        const target = Math.max(1, Number(achievement.target) || 1);
        return { category: category, markup: "<article class='sow-profile__achievement" + (achievement.unlocked ? " is-unlocked" : "") + "'><strong>" + esc(SOW_t(prefix + "_title")) + "</strong><span>" + esc(SOW_t(prefix + "_description")) + "</span><b>" + esc(progress) + "/" + esc(target) + " · " + esc(achievement.points) + " " + esc(SOW_t("profile.laurels")) + "</b></article>" };
    }

    function profileAchievementsMarkup(data, complete) {
        if (!complete) return "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || SOW_t("profile.loading_profile")) + "</p>";
        const grouped = { battles: [], victories: [], commanders: [], laurels: [] };
        (data.achievements || []).forEach(function (achievement) {
            const card = profileAchievementCard(achievement);
            (grouped[card.category] || grouped.commanders).push(card.markup);
        });
        const groups = ["battles", "victories", "commanders", "laurels"].map(function (category) {
            const cards = grouped[category];
            if (!cards.length) return "";
            return "<section class='sow-profile__achievement-group'><h3>" + esc(SOW_t("profile.achievement_category_" + category)) + "</h3><div class='sow-profile__achievement-grid'>" + cards.join("") + "</div></section>";
        }).join("");
        return groups || "<p class='sow-profile__empty'>" + esc(SOW_t("profile.achievements")) + "</p>";
    }

    function profileContentMarkup(data) {
        const entry = activeProfileEntry();
        const complete = profileDataComplete();
        let content = "";
        if (data && profileTab === "overview") {
            const recent = complete
                ? (profileHistory.slice(0, 10).map(profileMatchRow).join("") || "<p class='sow-profile__empty'>" + esc(SOW_t("profile.no_completed_matches")) + "</p>")
                : "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || SOW_t("profile.loading_profile")) + "</p>";
            content = "<div class='sow-profile__overview'><div class='sow-profile__stats'>" +
                "<div><strong>" + esc(data.matches_played) + "</strong><span>" + esc(SOW_t("profile.matches")) + "</span></div>" +
                "<div><strong>" + esc(data.wins) + "</strong><span>" + esc(SOW_t("profile.wins")) + "</span></div>" +
                "<div><strong>" + esc(Math.round((data.win_rate || 0) * 100)) + "%</strong><span>" + esc(SOW_t("profile.win_rate")) + "</span></div>" +
                "<div class='sow-profile__stat-laurels'><strong>" + esc(data.laurels == null ? 0 : data.laurels) + "</strong><span>" + esc(SOW_t("profile.laurels")) + "</span></div>" +
                "<div class='sow-profile__stat-kda'><strong><i>" + esc(data.kills) + "</i><i>" + esc(data.deaths) + "</i><i>" + esc(data.assists) + "</i></strong><span>" + esc(SOW_t("profile.kda")) + "</span></div>" +
                "</div><section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.recent_matches")) + "</h2><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='history'>" + esc(SOW_t("profile.view_all")) + "</button></div>" + recent + "</section></div>";
        } else if (data && profileTab === "leaders") {
            const leaderList = complete
                ? ((data.leaders || []).map(profileLeaderCard).join("") || "<p class='sow-profile__empty'>" + esc(SOW_t("profile.no_leader_history")) + "</p>")
                : "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || SOW_t("profile.loading_profile")) + "</p>";
            content = "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.tab_commanders")) + "</h2><span class='sow-profile__section-note'>" + esc(SOW_t("profile.leader_count", { count: complete ? (data.leaders || []).length : "—" })) + "</span></div><div class='sow-profile__leaders'>" + leaderList + "</div></section>";
        } else if (profileTab === "history") {
            const historyList = !complete
                ? "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || SOW_t("profile.loading_profile")) + "</p>"
                : (profileHistory.map(profileMatchRow).join("") || "<p class='sow-profile__empty'>" + esc(SOW_t("profile.no_completed_matches")) + "</p>");
            const canLoadMore = !!(entry && complete && (entry.historyHasMore || entry.historyError));
            const historyAction = profileAccountId && (canLoadMore || !complete)
                ? "<button type='button' class='sow-profile__load-more' data-command='load_profile_more'" + (profileHistoryLoading ? " disabled" : "") + ">" + (profileHistoryLoading ? esc(SOW_t("profile.loading_more")) : (entry && entry.historyError ? esc(SOW_t("profile.try_again")) : esc(SOW_t("profile.load_more_matches")))) + "</button>"
                : "";
            content = "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.match_history")) + "</h2><span class='sow-profile__section-note'>" + esc(SOW_t("profile.match_count", { count: data ? data.matches_played : 0 })) + "</span></div><div class='sow-profile__history'>" + historyList + "</div>" + historyAction + "</section>";
        } else if (data && profileTab === "achievements") {
            content = "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.achievements")) + "</h2><span class='sow-profile__section-note'>" + esc(data.laurels || 0) + " " + esc(SOW_t("profile.laurels")) + "</span></div>" + profileAchievementsMarkup(data, complete) + "</section>";
        } else if (profileTab === "victories") {
            const victoryRows = profileVictoryLeaderboard === null
                ? "<p class='sow-profile__empty'>" + esc(profileVictoryLeaderboardError || SOW_t("profile.leaderboard_loading")) + "</p>"
                : (profileVictoryLeaderboard.map(function (entry) {
                    return "<article class='sow-profile__victory-row'><b>#" + esc(entry.rank) + "</b><div><strong>" + esc(entry.handle || entry.account_id) + "</strong><span>" + esc(SOW_t("profile.level", { level: entry.level || 1 })) + "</span></div><strong>" + esc(entry.wins || 0) + " " + esc(SOW_t("profile.wins")) + "</strong><small>" + esc(entry.matches_played || 0) + " " + esc(SOW_t("profile.matches")) + "</small></article>";
                }).join("") || "<p class='sow-profile__empty'>" + esc(SOW_t("profile.no_victory_leaders")) + "</p>");
            const victoryAction = profileVictoryLeaderboardHasMore || profileVictoryLeaderboardError
                ? "<button type='button' class='sow-profile__load-more' data-command='load_victory_more'" + (profileVictoryLeaderboardLoading ? " disabled" : "") + ">" + esc(profileVictoryLeaderboardLoading ? SOW_t("profile.loading_more") : (profileVictoryLeaderboardError ? SOW_t("profile.try_again") : SOW_t("profile.load_more_matches"))) + "</button>"
                : "";
            content = "<section class='sow-profile__section'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.leaderboards")) + "</h2><span class='sow-profile__section-note'>" + esc(SOW_t("profile.verified_wins")) + "</span></div><div class='sow-profile__victory-list'>" + victoryRows + "</div>" + victoryAction + "</section>";
        } else if (!data) {
            content = "<section class='sow-profile__state' aria-live='polite'><strong>" + esc(profileSnapshotLoading ? SOW_t("profile.loading_profile") : SOW_t("profile.profile_unavailable")) + "</strong><span>" + esc(profileSnapshotError() || SOW_t("profile.return_to_menu")) + "</span>" + (profileSnapshotLoading ? "" : "<button type='button' class='sow-profile__load-more' data-command='retry_profile'>" + esc(SOW_t("profile.try_again")) + "</button>") + "</section>";
        }
        return "<div id='sow-profile-panel-active' class='sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-" + profileTab + "'>" + content + "</div>";
    }

    function renderProfile() {
        const data = profileData;
        const own = state && state.account_id === profileAccountId;
        const header = profileHeaderMarkup(data, own);
        const tabs = ["overview", "leaders", "history", "achievements", "victories"].map(function (tab) {
            const active = profileTab === tab;
            const label = tab === "victories" ? SOW_t("profile.leaderboards") : SOW_t("profile.tab_" + (tab === "leaders" ? "commanders" : tab));
            return "<button type='button' role='tab' id='sow-profile-tab-" + tab + "' class='sow-profile__tab" + (active ? " is-active" : "") + "' aria-selected='" + active + "' aria-controls='sow-profile-panel-active' data-command='profile_tab' data-profile-tab='" + tab + "'>" + esc(label) + "</button>";
        }).join("");
        let playGamesActions = "";
        if (typeof window.SOW_isAndroidTwa === "function" && window.SOW_isAndroidTwa()) {
            playGamesActions = "<section class='sow-profile__section' aria-label='" + esc(SOW_t("profile.google_play_games")) + "'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.google_play_games")) + "</h2><span class='sow-profile__section-note'>" + esc(SOW_t("profile.android")) + "</span></div><div class='sow-profile__actions'><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='achievements'>" + esc(SOW_t("profile.achievements")) + "</button><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='leaderboards'>" + esc(SOW_t("profile.leaderboards")) + "</button></div></section>";
        }
        const profileActions = playGamesActions + renderModeration(own);
        return "<main class='sow-menu__main sow-profile__main' data-screen-panel='profile'><section class='sow-profile__page'><div class='sow-profile__dossier'>" + header +
            "<nav class='sow-profile__tabs' role='tablist' aria-label='" + esc(SOW_t("profile.profile_sections")) + "'>" + tabs + "</nav><div data-profile-content class='sow-profile__content'>" + profileContentMarkup(data) + "</div></div>" +
            (profileActions ? "<aside class='sow-profile__tools'>" + profileActions + "</aside>" : "") + "</section></main>";
    }

    function syncProfileDom() {
        if (!profileOpen || currentScreen() !== "profile") return;
        const panel = activeScreenPanel();
        if (!panel) return;
        const data = profileData;
        const displayName = panel.querySelector("[data-profile-display-name]");
        const handle = panel.querySelector("[data-profile-handle]");
        const level = panel.querySelector("[data-profile-level]");
        if (displayName) displayName.textContent = data && data.display_name ? data.display_name : (profileSnapshotLoading ? SOW_t("profile.loading_profile") : SOW_t("profile.profile_unavailable"));
        if (handle) handle.textContent = data && data.handle ? data.handle : (profileSnapshotLoading ? SOW_t("profile.fetching_player_data") : "");
        if (level) level.textContent = data && data.level != null ? data.level : "—";
        const preferred = panel.querySelector("[data-profile-preferred]");
        if (preferred) preferred.innerHTML = profilePreferredCommanderMarkup(data);
        const hero = profileHeroImage(data && data.preferred_leader);
        if (root && root.dataset.hero !== hero) {
            root.style.setProperty("--sow-hero", "url(\"" + hero + "\")");
            root.dataset.hero = hero;
        }
        const status = panel.querySelector("[data-profile-status]");
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

    function renderProfileDetail() {
        if (!profileMatchDetail) {
            if (!profileDetailError) return "";
            return "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='" + esc(SOW_t("profile.close_match_details")) + "'>×</button><span class='sow-profile__kicker'>" + esc(SOW_t("profile.match_details")) + "</span><h2 id='sow-profile-detail-title'>" + esc(SOW_t("profile.unavailable")) + "</h2><p class='sow-profile__empty' aria-live='polite'>" + esc(profileDetailError) + "</p><button type='button' class='sow-profile__load-more' data-command='open_match' data-match-id='" + esc(profileDetailId) + "'>" + esc(SOW_t("profile.try_again")) + "</button></section></div>";
        }
        return "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='" + esc(SOW_t("profile.close_match_details")) + "'>×</button><span class='sow-profile__kicker'>" + esc(SOW_t("profile.match_details")) + "</span><h2 id='sow-profile-detail-title'>" + esc(profileMatchDetail.mode || SOW_t("profile.match")) + "</h2><p>" + esc(formatMapName(profileMatchDetail)) + " · " + esc(profileMatchDetail.queue || SOW_t("profile.matchmaking_queue")) + " · " + esc(profileMatchDate(profileMatchDetail.completed_at)) + " · " + esc(profileMatchDuration(profileMatchDetail.duration_seconds)) + "</p><div class='sow-profile__detail-players'>" + (profileMatchDetail.participants || []).map(function (participant) {
            const status = profileMatchDetail.verified
                ? (participant.won ? SOW_t("profile.win") : SOW_t("profile.loss"))
                : SOW_t(profileMatchDetail.provisional ? "profile.match_pending" : "profile.match_unverified");
            const stats = profileMatchDetail.verified
                ? SOW_t("profile.kda_value", { kills: participant.kills || 0, deaths: participant.deaths || 0, assists: participant.assists || 0 })
                : "";
            return "<div><strong>" + esc(participant.handle || participant.account_id) + "</strong><span>" + esc(participant.leader ? leaderDisplayName(leaderById(participant.leader)) : "—") + (stats ? " · " + esc(stats) : "") + "</span><b>" + esc(status) + "</b></div>";
        }).join("") + "</div></section></div>";
    }

    // Other-player profiles can be reported here; account deletion lives in Settings.
    function renderModeration(own) {
        if (own || !profileAccountId || !selfCreds()) return "";
        if (reportSent) {
            return "<section class='sow-profile__section' aria-label='" + esc(SOW_t("profile.report_filed")) + "'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.report_filed")) + "</h2></div><p class='sow-profile__empty'>" + esc(SOW_t("profile.thanks_report")) + "</p></section>";
        }
        if (isBlockedAccountId(profileAccountId)) {
            return "<section class='sow-profile__section' aria-label='" + esc(SOW_t("profile.blocked")) + "'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.blocked")) + "</h2></div><p class='sow-profile__empty'>" + esc(SOW_t("profile.blocked_player")) + "</p></section>";
        }
        if (!reportOpen || reportTarget !== profileAccountId) {
            return "<section class='sow-profile__section' aria-label='" + esc(SOW_t("profile.conduct")) + "'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.conduct")) + "</h2></div><button type='button' class='sow-menu__ghost-button' data-command='open_report' data-account-id='" + esc(profileAccountId) + "'>" + esc(SOW_t("profile.report_player")) + "</button></section>";
        }
        return "<section class='sow-profile__section' aria-label='" + esc(SOW_t("profile.report_player")) + "'><div class='sow-profile__section-head'><h2>" + esc(SOW_t("profile.report_player")) + "</h2></div>" +
            "<form data-form='report'>" +
            "<label for='sow-report-reason'>" + esc(SOW_t("profile.reason")) + "</label>" +
            SOW_renderDropdown({ key: "report-reason", name: "reason", value: REPORT_REASONS[0][0], options: REPORT_REASONS.map(function (r) { return { value: r[0], label: SOW_t(r[1]) }; }) }) +
            "<label for='sow-report-details'>" + esc(SOW_t("profile.report_details")) + "</label>" +
            "<textarea id='sow-report-details' name='details' rows='3' maxlength='500' placeholder='" + esc(SOW_t("profile.what_happened")) + "'></textarea>" +
            "<p class='sow-profile__empty'>" + esc(SOW_t("profile.report_notice")) + "</p>" +
            "<button type='submit'" + (reportBusy ? " disabled" : "") + ">" + esc(reportBusy ? SOW_t("profile.sending_report") : SOW_t("profile.send_report_block")) + "</button> " +
            "<button type='button' class='sow-menu__ghost-button' data-command='cancel_report'>" + esc(SOW_t("profile.cancel_report")) + "</button>" +
            "</form></section>";
    }
