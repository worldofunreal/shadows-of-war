// Profile data, rendering, history, rankings, and match details.

    function profileApi(path) {
        var base = String(window.SOW_DATABASE_URL || "/api").replace(/\/$/, "");
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
                ratings: null,
                ratingsError: "",
                ratingsRequest: null,
                details: Object.create(null),
                detailRequests: Object.create(null)
            };
        }
        return profileCache[id];
    }

    function profileStat(value) {
        var number = Number(value);
        return Number.isFinite(number) ? Math.max(0, Math.floor(number)) : 0;
    }

    function profileSeedFromState(id) {
        if (!state || state.account_id !== id) return null;
        var stats = state.profile_stats || {};
        var matches = profileStat(stats.matches_played);
        var wins = profileStat(stats.wins);
        return {
            account_id: id,
            handle: state.player_name || "",
            display_name: state.player_name || "ANONYMOUS",
            level: profileStat(state.level) || 1,
            matches_played: matches,
            wins: wins,
            win_rate: matches ? wins / matches : 0,
            kills: profileStat(stats.kills),
            deaths: profileStat(stats.deaths),
            assists: profileStat(stats.assists),
            players_defeated: profileStat(stats.players_defeated),
            empires_defeated: profileStat(stats.empires_defeated),
            tribes_defeated: profileStat(stats.tribes_defeated),
            preferred_leader: state.selected_leader || null,
            leaders: [],
            recent_matches: []
        };
    }

    function profileSeedFromSummary(summary) {
        if (!summary || !summary.account_id) return null;
        return {
            account_id: summary.account_id,
            handle: summary.handle || "",
            display_name: summary.display_name || "PLAYER",
            level: profileStat(summary.level) || 1,
            matches_played: profileStat(summary.matches_played),
            wins: profileStat(summary.wins),
            win_rate: Number(summary.win_rate) || 0,
            kills: profileStat(summary.kills),
            deaths: profileStat(summary.deaths),
            assists: profileStat(summary.assists),
            players_defeated: 0,
            empires_defeated: 0,
            tribes_defeated: 0,
            preferred_leader: null,
            leaders: [],
            recent_matches: []
        };
    }

    function cacheProfileSeed(id, seed) {
        var entry = profileCacheEntry(id);
        if (!entry || !seed || entry.complete) return entry;
        entry.data = Object.assign({}, entry.data || {}, seed);
        return entry;
    }

    function activateProfileEntry(id) {
        var entry = profileCacheEntry(id);
        if (!entry) return null;
        profileData = entry.data;
        profileHistory = entry.history;
        profileRatings = entry.ratings;
        return entry;
    }

    function activeProfileEntry() {
        return profileCacheEntry(profileAccountId);
    }

    function profileDataComplete() {
        var entry = activeProfileEntry();
        return !!(entry && entry.complete);
    }

    function profileSnapshotError() {
        var entry = activeProfileEntry();
        return entry ? entry.error : "";
    }

    function applyProfileSnapshot(id, data) {
        var entry = profileCacheEntry(id);
        if (!entry) return;
        var recent = Array.isArray(data.recent_matches) ? data.recent_matches.slice() : [];
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
        var entry = profileCacheEntry(id);
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
            entry.error = "Profile unavailable.";
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
        var id = profileAccountId;
        var entry = activeProfileEntry();
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
            var items = Array.isArray(data.items) ? data.items : [];
            entry.history = entry.history.concat(items);
            entry.historyCursor = data.next_cursor == null ? entry.historyCursor + items.length : Number(data.next_cursor);
            entry.historyLoaded = true;
            entry.historyHasMore = data.next_cursor != null && items.length > 0;
            if (profileAccountId === id) activateProfileEntry(id);
        }).catch(function () {
            entry.historyError = "Match history unavailable.";
        }).finally(function () {
            entry.historyRequest = null;
            if (profileOpen && profileAccountId === id) {
                profileHistoryLoading = false;
                syncProfileDom();
            }
        });
    }

    function loadProfileRatings() {
        var id = profileAccountId;
        var entry = activeProfileEntry();
        if (!id || !entry || profileRatingsLoading || entry.ratingsRequest || entry.ratings !== null) return;
        if (!entry.complete || entry.stale) {
            profileRatingsLoading = true;
            loadProfile(id).then(function (data) {
                if (profileOpen && profileAccountId === id) {
                    profileRatingsLoading = false;
                    if (data) loadProfileRatings();
                    else syncProfileDom();
                }
            });
            return;
        }
        profileRatingsLoading = true;
        entry.ratingsError = "";
        entry.ratingsRequest = fetch(profileApi("/profiles/" + encodeURIComponent(id) + "/seasons"), {
            headers: { "Accept": "application/json" }
        }).then(function (response) {
            if (!response.ok) throw new Error("profile ratings failed");
            return response.json();
        }).then(function (data) {
            entry.ratings = Array.isArray(data.items) ? data.items : [];
            entry.ratingsError = "";
            if (profileAccountId === id) activateProfileEntry(id);
        }).catch(function () {
            entry.ratingsError = "Ranked records unavailable.";
        }).finally(function () {
            entry.ratingsRequest = null;
            if (profileOpen && profileAccountId === id) {
                profileRatingsLoading = false;
                syncProfileDom();
            }
        });
    }

    function invalidateProfileCache(id) {
        var entry = profileCacheEntry(id);
        if (!entry) return;
        entry.stale = true;
        entry.error = "";
        entry.historyLoaded = false;
        entry.historyError = "";
        entry.ratings = null;
        entry.ratingsError = "";
    }

    function syncProfilePreload(nextState) {
        var nextId = nextState && nextState.account_id ? nextState.account_id : null;
        var previousId = profileOwnId;
        var returnedToMenu = profileLastPhase && profileLastPhase !== "MainMenu" && nextState.phase === "MainMenu";
        if (previousId && previousId !== nextId && profileOpen && profileAccountId === previousId) {
            profileOpen = false;
            profileAccountId = null;
            profileData = null;
            profileHistory = [];
            profileRatings = null;
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
        var entry = cacheProfileSeed(nextId, profileSeedFromState(nextId));
        if (returnedToMenu) invalidateProfileCache(nextId);
        if (entry && !entry.complete) {
            if (profileAccountId === nextId) activateProfileEntry(nextId);
        }
        if (previousId !== nextId || returnedToMenu || (entry && !entry.complete && !entry.request && !entry.error)) {
            loadProfile(nextId);
        }
    }

    function openProfile(id) {
        var targetId = id || (state && state.account_id);
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
                var blockedEntry = profileCacheEntry(targetId);
                blockedEntry.data = null;
                blockedEntry.complete = false;
                blockedEntry.stale = false;
                blockedEntry.error = "You blocked this player.";
                profileData = null;
                profileHistory = blockedEntry.history = [];
                blockedEntry.historyCursor = 0;
                blockedEntry.historyLoaded = true;
                blockedEntry.historyHasMore = false;
                profileRatings = blockedEntry.ratings = null;
                profileMatchDetail = null;
                profileDetailError = "";
                profileDetailId = null;
                profileSnapshotLoading = false;
                profileHistoryLoading = false;
                profileRatingsLoading = false;
                profileDetailLoading = false;
                profileDetailRequestKey = null;
                profileSearchResults = [];
                render();
                return;
            }
        } catch (e) {}
        profileOpen = true;
        profileAccountId = targetId;
        profileTab = "overview";
        profileSnapshotLoading = false;
        profileHistoryLoading = false;
        profileRatingsLoading = false;
        profileDetailLoading = false;
        profileMatchDetail = null;
        profileDetailError = "";
        profileDetailId = null;
        profileDetailRequestKey = null;
        profileSearchResults = [];
        cacheProfileSeed(targetId, profileSeedFromState(targetId));
        activateProfileEntry(targetId);
        loadProfile(targetId);
        render();
    }

    function isBlockedAccountId(accountId) {
        try {
            return typeof window.SOW_isBlockedId === "function" && window.SOW_isBlockedId(accountId);
        } catch (e) {
            return false;
        }
    }

    function profileLeaderCard(leader) {
        var summary = leader || {};
        var leaderInfo = leaderById(summary.leader);
        return "<article class='sow-profile__leader'>" +
            "<img src='" + esc(asset("gameplay/avatars/" + leaderInfo.slug + ".webp")) + "' alt='' width='48' height='48' loading='lazy'>" +
            "<div><strong>" + esc(summary.leader || leaderInfo.name) + "</strong>" +
            "<span>" + esc(summary.matches_played || 0) + " matches · " + esc(Math.round((summary.win_rate || 0) * 100)) + "% wins</span></div>" +
            "<b>LV " + esc(1 + Math.floor((summary.xp || 0) / 100)) + "</b>" +
            "</article>";
    }

    function profileRecentLeaders(matches) {
        var recent = Array.isArray(matches) ? matches.slice(0, 10) : [];
        var knownLeaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        var counts = {};
        recent.forEach(function (match, index) {
            var leaderId = match && match.leader;
            var leaderInfo = knownLeaders.find(function (leader) { return leader.id === leaderId; });
            if (!leaderInfo) return;
            var entry = counts[leaderInfo.id];
            if (!entry) {
                entry = counts[leaderInfo.id] = { leader: leaderInfo, matches: 0, lastIndex: index };
            }
            entry.matches += 1;
            entry.lastIndex = Math.min(entry.lastIndex, index);
        });
        return Object.keys(counts).map(function (id) { return counts[id]; }).sort(function (left, right) {
            return right.matches - left.matches || left.lastIndex - right.lastIndex;
        }).slice(0, 3);
    }

    function profileRecentLeadersPanel(matches, loading) {
        var recent = Array.isArray(matches) ? matches.slice(0, 10) : [];
        var favorites = profileRecentLeaders(recent);
        var cards = favorites.map(function (entry) {
            var leader = entry.leader;
            var share = recent.length ? Math.round((entry.matches / recent.length) * 100) : 0;
            var unit = entry.matches === 1 ? "game" : "games";
            return "<article class='sow-profile__favorite'><img src='" + esc(asset("gameplay/avatars/" + leader.slug + ".webp")) + "' alt='" + esc(leader.name) + " avatar' width='64' height='64' loading='lazy'><div class='sow-profile__favorite-copy'><strong>" + esc(leader.name) + "</strong><span>" + esc(entry.matches) + " " + unit + " · " + esc(share) + "%</span></div></article>";
        }).join("");
        return "<section class='sow-profile__favorites' data-profile-favorites aria-labelledby='sow-profile-favorites-title'><div class='sow-profile__favorites-head'><h2 id='sow-profile-favorites-title'>Most played leaders</h2><span>Last " + esc(recent.length) + " matches</span></div>" +
            (cards ? "<div class='sow-profile__favorites-track' data-count='" + esc(favorites.length) + "'>" + cards + "</div>" : "<div class='sow-profile__favorites-track' data-count='0'><p class='sow-profile__favorites-empty'>" + (loading ? "Loading leader data…" : "No leader data yet.") + "</p></div>") +
            "</section>";
    }

    function profileMatchRow(match) {
        var result = match.won ? "WIN" : "LOSS";
        var mode = match.mode || "FFA";
        var map = formatMapName(match);
        var kda = (match.kills || 0) + " / " + (match.deaths || 0) + " / " + (match.assists || 0);
        return "<button type='button' class='sow-profile__match' data-command='open_match' data-match-id='" + esc(match.match_id) + "'>" +
            "<span class='sow-profile__match-result " + (match.won ? "is-win" : "is-loss") + "'>" + result + "</span>" +
            "<span class='sow-profile__match-context'><strong>" + esc(mode) + "</strong><small>" + esc(map) + " · " + esc(match.queue || "MATCHMAKING") + "</small></span>" +
            "<span class='sow-profile__match-kda'><strong>" + esc(match.leader || "—") + "</strong><small>" + esc(kda) + " K/D/A</small></span>" +
            "<span class='sow-profile__match-rating'>" + (match.rating_delta == null ? "—" : (match.rating_delta >= 0 ? "+" : "") + esc(match.rating_delta) + " SR") + "</span>" +
            "</button>";
    }

    function profileHeaderMarkup(data, own) {
        var title = own ? "Your profile" : "Player profile";
        var entry = activeProfileEntry();
        var snapshotError = profileSnapshotError();
        var displayName = data && data.display_name ? data.display_name : (profileSnapshotLoading ? "Loading profile…" : "Profile unavailable");
        var handle = data && data.handle ? data.handle : (profileSnapshotLoading ? "Fetching player data…" : "");
        var level = data && data.level != null ? data.level : "—";
        var status = data && snapshotError
            ? "<div class='sow-profile__status' data-profile-status aria-live='polite'>" + esc(snapshotError) + " <button type='button' class='sow-profile__status-action' data-command='retry_profile'>Try again</button></div>"
            : "<div class='sow-profile__status' data-profile-status aria-live='polite' hidden></div>";
        return "<section class='sow-profile__heading' data-profile-heading aria-labelledby='sow-profile-title'><div class='sow-profile__identity-card'><div class='sow-profile__heading-top'><span class='sow-profile__kicker'>" + esc(title) + "</span><button type='button' class='sow-profile__back' data-command='close_profile'>← Back</button></div><div class='sow-profile__identity-main'><div class='sow-profile__identity-copy'><h1 id='sow-profile-title' data-profile-display-name>" + esc(displayName) + "</h1><p class='sow-profile__handle' data-profile-handle>" + esc(handle) + "</p>" + status + "</div><div class='sow-profile__level'><small>LEVEL</small><strong data-profile-level>" + esc(level) + "</strong></div></div></div>" + profileRecentLeadersPanel(profileHistory, !!(entry && !entry.complete && profileSnapshotLoading)) + "</section>";
    }

    function profileContentMarkup(data) {
        var entry = activeProfileEntry();
        var complete = profileDataComplete();
        var content = "";
        if (data && profileTab === "overview") {
            var recent = complete
                ? (profileHistory.slice(0, 10).map(profileMatchRow).join("") || "<p class='sow-profile__empty'>No completed matches.</p>")
                : "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || "Loading profile…") + "</p>";
            var leaders = complete
                ? ((data.leaders || []).slice(0, 4).map(profileLeaderCard).join("") || "<p class='sow-profile__empty'>No leader history.</p>")
                : "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || "Loading profile…") + "</p>";
            content = "<div id='sow-profile-panel-overview' class='sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-overview'><div class='sow-profile__stats'>" +
                "<div><strong>" + esc(data.matches_played) + "</strong><span>Matches</span></div>" +
                "<div><strong>" + esc(data.wins) + "</strong><span>Wins</span></div>" +
                "<div><strong>" + esc(Math.round((data.win_rate || 0) * 100)) + "%</strong><span>Win rate</span></div>" +
                "<div class='sow-profile__stat-kda'><strong><i>" + esc(data.kills) + "</i><i>" + esc(data.deaths) + "</i><i>" + esc(data.assists) + "</i></strong><span>K / D / A</span></div>" +
                "</div><div class='sow-profile__columns'><section class='sow-profile__section'><div class='sow-profile__section-head'><h2>Recent matches</h2><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='history'>View all</button></div>" + recent +
                "</section><section class='sow-profile__section'><div class='sow-profile__section-head'><h2>Leaders</h2><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='leaders'>View all</button></div>" + leaders +
                "</section></div></div>";
        } else if (data && profileTab === "leaders") {
            var leaderList = complete
                ? ((data.leaders || []).map(profileLeaderCard).join("") || "<p class='sow-profile__empty'>No leader history.</p>")
                : "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || "Loading profile…") + "</p>";
            content = "<section id='sow-profile-panel-leaders' class='sow-profile__section sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-leaders'><div class='sow-profile__section-head'><h2>Leader mastery</h2><span class='sow-profile__section-note'>" + esc(complete ? (data.leaders || []).length : "—") + " leaders</span></div><div class='sow-profile__leaders'>" + leaderList + "</div></section>";
        } else if (profileTab === "history") {
            var historyList = !complete
                ? "<p class='sow-profile__empty'>" + esc(profileSnapshotError() || "Loading profile…") + "</p>"
                : (profileHistory.map(profileMatchRow).join("") || "<p class='sow-profile__empty'>No completed matches.</p>");
            var canLoadMore = !!(entry && complete && (entry.historyHasMore || entry.historyError));
            var historyAction = profileAccountId && (canLoadMore || !complete)
                ? "<button type='button' class='sow-profile__load-more' data-command='load_profile_more'" + (profileHistoryLoading ? " disabled" : "") + ">" + (profileHistoryLoading ? "Loading…" : (entry && entry.historyError ? "Try again" : "Load more matches")) + "</button>"
                : "";
            content = "<section id='sow-profile-panel-history' class='sow-profile__section sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-history'><div class='sow-profile__section-head'><h2>Match history</h2><span class='sow-profile__section-note'>" + esc(data ? data.matches_played : 0) + " matches</span></div><div class='sow-profile__history'>" + historyList + "</div>" + historyAction + "</section>";
        } else if (data && profileTab === "ranked") {
            var ratings = profileRatings === null
                ? "<p class='sow-profile__empty'>" + esc(profileRatingsLoading || !complete ? "Loading ranked records…" : (entry && entry.ratingsError ? entry.ratingsError : "No ranked records.")) + "</p>"
                : (profileRatings.map(function (rating) {
                    return "<article class='sow-profile__rating'><div><strong>" + esc(rating.season_name) + "</strong><span>" + esc(rating.queue) + " · " + esc(rating.mode) + "</span></div><b>" + esc(rating.tier) + (rating.division ? " " + esc(rating.division) : "") + "</b><strong>" + esc(rating.score) + " SR</strong><small>" + esc(rating.games_played) + " games · " + esc(rating.wins) + " wins · peak " + esc(rating.peak_score) + "</small></article>";
                }).join("") || "<p class='sow-profile__empty'>No ranked records.</p>");
            content = "<section id='sow-profile-panel-ranked' class='sow-profile__section sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-ranked'><div class='sow-profile__section-head'><h2>Ranked</h2><span class='sow-profile__section-note'>Season records</span></div><div class='sow-profile__ratings'>" + ratings + "</div></section>";
        } else if (!data) {
            content = "<section class='sow-profile__state' aria-live='polite'><strong>" + esc(profileSnapshotLoading ? "Loading profile…" : "Profile unavailable") + "</strong><span>" + esc(profileSnapshotError() || "Try again or return to the menu.") + "</span>" + (profileSnapshotLoading ? "" : "<button type='button' class='sow-profile__load-more' data-command='retry_profile'>Try again</button>") + "</section>";
        }
        return content;
    }

    function renderProfile() {
        var data = profileData;
        var own = state && state.account_id === profileAccountId;
        var header = profileHeaderMarkup(data, own);
        var tabs = ["overview", "leaders", "history", "ranked"].map(function (tab) {
            var active = profileTab === tab;
            return "<button type='button' role='tab' id='sow-profile-tab-" + tab + "' class='sow-profile__tab" + (active ? " is-active" : "") + "' aria-selected='" + active + "' aria-controls='sow-profile-panel-" + tab + "' data-command='profile_tab' data-profile-tab='" + tab + "'>" + tab.charAt(0).toUpperCase() + tab.slice(1) + "</button>";
        }).join("");
        var playGamesActions = "";
        if (typeof window.SOW_isAndroidTwa === "function" && window.SOW_isAndroidTwa()) {
            playGamesActions = "<section class='sow-profile__section' aria-label='Google Play Games'><div class='sow-profile__section-head'><h2>Google Play Games</h2><span class='sow-profile__section-note'>Android</span></div><div class='sow-profile__actions'><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='achievements'>Achievements</button><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='leaderboards'>Leaderboards</button></div></section>";
        }
        var search = "<section class='sow-profile__directory'><div><h2>Find a player</h2><p>Search public player profiles by name or handle.</p></div><form class='sow-profile__search' data-form='profile-search'><label class='sow-profile__sr-only' for='sow-profile-search-input'>Player name or handle</label><input id='sow-profile-search-input' name='q' type='search' autocomplete='off' placeholder='Name or handle…' aria-label='Player name or handle'><button type='submit'>Search</button></form></section>";
        var searchResults = profileSearchResults.length
            ? "<div class='sow-profile__search-results' aria-live='polite'>" + profileSearchResults.map(function (summary) {
                return "<button type='button' class='sow-profile__search-result' data-command='open_public_profile' data-account-id='" + esc(summary.account_id) + "'><strong>" + esc(summary.display_name) + "</strong><span>" + esc(summary.handle) + " · LV " + esc(summary.level) + "</span></button>";
            }).join("") + "</div>"
            : "";
        return "<main class='sow-menu__main sow-profile__main' data-screen-panel='profile'><section class='sow-profile__page'>" + header +
            "<nav class='sow-profile__tabs' role='tablist' aria-label='Profile sections'>" + tabs + "</nav>" + playGamesActions + "<div data-profile-content class='sow-profile__content'>" + profileContentMarkup(data) + "</div>" + renderModeration(own) + search + searchResults +
            "</section></main>";
    }

    function syncProfileDom() {
        if (!profileOpen || currentScreen() !== "profile") return;
        var panel = activeScreenPanel();
        if (!panel) return;
        var data = profileData;
        var displayName = panel.querySelector("[data-profile-display-name]");
        var handle = panel.querySelector("[data-profile-handle]");
        var level = panel.querySelector("[data-profile-level]");
        if (displayName) displayName.textContent = data && data.display_name ? data.display_name : (profileSnapshotLoading ? "Loading profile…" : "Profile unavailable");
        if (handle) handle.textContent = data && data.handle ? data.handle : (profileSnapshotLoading ? "Fetching player data…" : "");
        if (level) level.textContent = data && data.level != null ? data.level : "—";
        var status = panel.querySelector("[data-profile-status]");
        var snapshotError = data && profileSnapshotError();
        if (status) {
            status.hidden = !snapshotError;
            status.textContent = snapshotError ? snapshotError + " " : "";
            if (snapshotError) {
                var retry = document.createElement("button");
                retry.type = "button";
                retry.className = "sow-profile__status-action";
                retry.dataset.command = "retry_profile";
                retry.textContent = "Try again";
                status.appendChild(retry);
            }
        }
        var favorites = panel.querySelector("[data-profile-favorites]");
        if (favorites) favorites.outerHTML = profileRecentLeadersPanel(profileHistory, !!(activeProfileEntry() && !profileDataComplete() && profileSnapshotLoading));
        var content = panel.querySelector("[data-profile-content]");
        if (content) content.innerHTML = profileContentMarkup(data);
    }

    function renderProfileDetail() {
        if (!profileMatchDetail) {
            if (!profileDetailError) return "";
            return "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='Close match details'>×</button><span class='sow-profile__kicker'>Match details</span><h2 id='sow-profile-detail-title'>Unavailable</h2><p class='sow-profile__empty' aria-live='polite'>" + esc(profileDetailError) + "</p><button type='button' class='sow-profile__load-more' data-command='open_match' data-match-id='" + esc(profileDetailId) + "'>Try again</button></section></div>";
        }
        return "<div class='sow-profile__detail-backdrop' data-menu-overlay='profile-detail'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='Close match details'>×</button><span class='sow-profile__kicker'>Match details</span><h2 id='sow-profile-detail-title'>" + esc(profileMatchDetail.mode || "MATCH") + "</h2><p>" + esc(formatMapName(profileMatchDetail)) + " · " + esc(profileMatchDetail.queue || "MATCHMAKING") + "</p><div class='sow-profile__detail-players'>" + (profileMatchDetail.participants || []).map(function (participant) {
            return "<div><strong>" + esc(participant.handle || participant.account_id) + "</strong><span>" + esc(participant.leader || "—") + " · " + esc(participant.kills || 0) + " / " + esc(participant.deaths || 0) + " / " + esc(participant.assists || 0) + "</span><b>" + (participant.won ? "WIN" : "LOSS") + "</b></div>";
        }).join("") + "</div></section></div>";
    }

    // Conduct + privacy controls on profiles. Own profile: self-service
    // erasure (double-confirm, ownership-proofed server-side). Others:
    // report with closed-reason dropdown + free text for Other; filing
    // also blocks the account client-side and notifies moderation.
    function renderModeration(own) {
        if (own) {
            if (!selfCreds()) return "";
            var delAction = deleteBusy
                ? "<p class='sow-profile__empty'>Deleting account…</p>"
                : deleteArmed
                    ? "<p><strong>This is permanent.</strong> Your account, profile, items, and history will be erased. Click again to confirm.</p><button type='button' class='sow-menu__danger' data-command='confirm_delete'>YES, DELETE MY ACCOUNT</button> <button type='button' class='sow-menu__ghost-button' data-command='cancel_delete'>CANCEL</button>"
                    : "<button type='button' class='sow-menu__ghost-button' data-command='request_delete'>Delete my account…</button>";
            return "<section class='sow-profile__section' aria-label='Danger zone'><div class='sow-profile__section-head'><h2>Danger zone</h2></div>" + delAction + "</section>";
        }
        if (!profileAccountId || !selfCreds()) return "";
        if (reportSent) {
            return "<section class='sow-profile__section' aria-label='Report filed'><div class='sow-profile__section-head'><h2>Report filed</h2></div><p class='sow-profile__empty'>Thanks — our moderation team will review this player. They are now blocked for you.</p></section>";
        }
        if (isBlockedAccountId(profileAccountId)) {
            return "<section class='sow-profile__section' aria-label='Blocked'><div class='sow-profile__section-head'><h2>Blocked</h2></div><p class='sow-profile__empty'>You blocked this player when you reported them.</p></section>";
        }
        if (!reportOpen || reportTarget !== profileAccountId) {
            return "<section class='sow-profile__section' aria-label='Conduct'><div class='sow-profile__section-head'><h2>Conduct</h2></div><button type='button' class='sow-menu__ghost-button' data-command='open_report' data-account-id='" + esc(profileAccountId) + "'>Report player…</button></section>";
        }
        return "<section class='sow-profile__section' aria-label='Report player'><div class='sow-profile__section-head'><h2>Report player</h2></div>" +
            "<form data-form='report'>" +
            "<label for='sow-report-reason'>Reason</label>" +
            renderDropdown({ key: "report-reason", name: "reason", value: REPORT_REASONS[0][0], options: REPORT_REASONS.map(function (r) { return { value: r[0], label: r[1] }; }) }) +
            "<label for='sow-report-details'>Details (required for Other)</label>" +
            "<textarea id='sow-report-details' name='details' rows='3' maxlength='500' placeholder='What happened?'></textarea>" +
            "<p class='sow-profile__empty'>Filing also blocks this player for you and notifies our moderation team. False reports violate the Terms.</p>" +
            "<button type='submit'" + (reportBusy ? " disabled" : "") + ">" + (reportBusy ? "Sending…" : "Send report & block") + "</button> " +
            "<button type='button' class='sow-menu__ghost-button' data-command='cancel_report'>Cancel</button>" +
            "</form></section>";
    }
