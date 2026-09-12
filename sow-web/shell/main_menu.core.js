(function () {
    "use strict";

    var root = document.getElementById("sow-menu");
    if (!root) return;

    var state = null;
    var lastRaw = "";
    var lastRenderKey = "";
    var previousScreen = null;
    var tempSelectedLeader = null;
    var browserSearchQuery = "";
    var heroesSearchQuery = "";
    var heroesRegionFilter = "all";
    var dropdownOpenKey = null;
    var authModalOpen = false;
    var settingsOpen = false;
    var profileOpen = false;
    var profilePublicId = null;
    var profileTab = "overview";
    var profileData = null;
    var profileHistory = [];
    var profileRatings = null;
    var profileMatchDetail = null;
    var profileSearchResults = [];
    var profileCache = Object.create(null);
    var profileSnapshotLoading = false;
    var profileHistoryLoading = false;
    var profileRatingsLoading = false;
    var profileDetailLoading = false;
    var profileDetailError = "";
    var profileDetailId = null;
    var profileDetailRequestKey = null;
    var profileOwnId = null;
    var profileLastPhase = null;
    var storeOpen = false;
    var heroesOpen = false;
    var createDraft = null;
    var createOffline = false;
    var createPrivate = false;
    var createPassword = "";
    var passwordLobbyId = null;
    var passwordDraft = "";
    var storeCheckoutProduct = null;
    var storeCheckoutRequestId = null;
    var storeCheckoutBusy = false;
    var storeCheckoutInstance = null;
    var stripePromise = null;
    var storeCatalogLoading = false;
    var storeCatalogLoaded = false;
    var pendingCommands = [];

    var LEADER_REGIONS = {
        caesar: "Europe",
        cleopatra: "Africa",
        ragnar: "Europe",
        sun_tzu: "Asia",
        alexander: "Europe",
        genghis_khan: "Asia",
        richard_the_lionheart: "Europe",
        vercingetorix: "Europe",
        boudica: "Europe",
        lady_six_sky: "Americas",
        leonidas: "Europe",
        napoleon: "Europe"
    };

    function esc(value) {
        return String(value == null ? "" : value)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    function asset(path) {
        var base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        return base + "/" + path.split("/").map(encodeURIComponent).join("/");
    }

    function currencyAsset(kind) {
        return asset("gameplay/currency/" + kind + ".webp");
    }

    function leaderById(id) {
        var leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        var found = leaders.find(function (leader) { return leader.id === id; });
        if (found) return found;
        return leaders[0] || {
            id: "Caesar", name: "Caesar", civilization: "Roman Empire", perk: "Imperium: +15% Territory Expansion Speed", slug: "caesar"
        };
    }

    function mapInfo(key) {
        var catalog = state && Array.isArray(state.map_catalog) ? state.map_catalog : [];
        var found = catalog.find(function (m) { return m.key === key; });
        if (found) return found;
        return { key: key || "world", display_name: (key || "WORLD MAP").toUpperCase(), width: 2048, height: 1024 };
    }

    function findLobby(id) {
        return (state && state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    function currentScreen() {
        if (!state) return "boot";
        if (profileOpen) return "profile";
        if (heroesOpen) return "heroes";
        if (storeOpen) return "store";
        if (state.waiting) return "queue";
        if (state.show_create) return "create";
        if (state.show_browser) return "browser";
        return "home";
    }

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
        if (!state || state.public_profile_id !== id) return null;
        var stats = state.profile_stats || {};
        var matches = profileStat(stats.matches_played);
        var wins = profileStat(stats.wins);
        return {
            public_id: id,
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
        if (!summary || !summary.public_id) return null;
        return {
            public_id: summary.public_id,
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
        return profileCacheEntry(profilePublicId);
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
        if (profilePublicId === id) activateProfileEntry(id);
    }

    function loadProfile(id) {
        var entry = profileCacheEntry(id);
        if (!entry) return Promise.resolve(null);
        if (entry.complete && !entry.stale) {
            if (profilePublicId === id) activateProfileEntry(id);
            return Promise.resolve(entry.data);
        }
        if (entry.request) {
            if (profilePublicId === id) profileSnapshotLoading = true;
            return entry.request;
        }
        if (profilePublicId === id) {
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
            if (profileOpen && profilePublicId === id) {
                profileSnapshotLoading = false;
                activateProfileEntry(id);
                syncProfileDom();
            }
        });
        return entry.request;
    }

    function loadMoreProfileHistory() {
        var id = profilePublicId;
        var entry = activeProfileEntry();
        if (!id || !entry || profileHistoryLoading || entry.historyRequest) return;
        if (!entry.complete || entry.stale) {
            profileHistoryLoading = true;
            loadProfile(id).then(function (data) {
                if (profileOpen && profilePublicId === id) {
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
            if (profilePublicId === id) activateProfileEntry(id);
        }).catch(function () {
            entry.historyError = "Match history unavailable.";
        }).finally(function () {
            entry.historyRequest = null;
            if (profileOpen && profilePublicId === id) {
                profileHistoryLoading = false;
                syncProfileDom();
            }
        });
    }

    function loadProfileRatings() {
        var id = profilePublicId;
        var entry = activeProfileEntry();
        if (!id || !entry || profileRatingsLoading || entry.ratingsRequest || entry.ratings !== null) return;
        if (!entry.complete || entry.stale) {
            profileRatingsLoading = true;
            loadProfile(id).then(function (data) {
                if (profileOpen && profilePublicId === id) {
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
            if (profilePublicId === id) activateProfileEntry(id);
        }).catch(function () {
            entry.ratingsError = "Ranked records unavailable.";
        }).finally(function () {
            entry.ratingsRequest = null;
            if (profileOpen && profilePublicId === id) {
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
        var nextId = nextState && nextState.public_profile_id ? nextState.public_profile_id : null;
        var previousId = profileOwnId;
        var returnedToMenu = profileLastPhase && profileLastPhase !== "MainMenu" && nextState.phase === "MainMenu";
        if (previousId && previousId !== nextId && profileOpen && profilePublicId === previousId) {
            profileOpen = false;
            profilePublicId = null;
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
            if (profilePublicId === nextId) activateProfileEntry(nextId);
        }
        if (previousId !== nextId || returnedToMenu || (entry && !entry.complete && !entry.request && !entry.error)) {
            loadProfile(nextId);
        }
    }

    function openProfile(id) {
        var targetId = id || (state && state.public_profile_id);
        if (!targetId) return;
        reportOpen = false;
        reportTarget = null;
        reportSent = false;
        deleteArmed = false;
        try {
            if (typeof window.SOW_isBlockedId === "function" && window.SOW_isBlockedId(targetId)) {
                profileOpen = true;
                profilePublicId = targetId;
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
        profilePublicId = targetId;
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

    function send(type, extra) {
        var command = Object.assign({ type: type }, extra || {});
        var serialized = JSON.stringify(command);
        if (typeof window.SOW_menu_command !== "function") {
            pendingCommands.push(serialized);
            return true;
        }
        window.SOW_menu_command(serialized);
        return true;
    }

    window.SOW_flush_menu_commands = function () {
        if (typeof window.SOW_menu_command !== "function") return;
        while (pendingCommands.length) window.SOW_menu_command(pendingCommands.shift());
    };

    function heroImage() {
        var leader = leaderById(state && state.selected_leader);
        return asset("shell/leaders/" + leader.slug + "_desktop.webp");
    }

    function avatarImage() {
        var leader = leaderById(state && state.selected_leader);
        return asset("gameplay/avatars/" + leader.slug + ".webp");
    }

    function lobbyThumb(lobby) {
        var base = String(window.SOW_MAPS_URL || "/maps").replace(/\/$/, "");
        var bust = String(window.SOW_MAPS_CACHE_BUST || window.SOW_BUILD_TS || "");
        var mapKey = String((lobby && lobby.map_name) || "world").trim().toLowerCase();
        return base + "/" + encodeURIComponent(mapKey) + "/thumbnail.webp" +
            (bust ? "?v=" + encodeURIComponent(bust) : "");
    }

    function renderKey() {
        return JSON.stringify({
            phase: state.phase,
            waiting: state.waiting,
            browser: state.show_browser,
            create: state.show_create,
            name: state.player_name,
            locked: state.name_locked,
            leader: state.selected_leader,
            gems: state.gems,
            crowns: state.crowns == null ? state.laurels : state.crowns,
            selected_skin: state.selected_skin,
            skins: (state.store && state.store.skins || []).map(function (skin) {
                return [skin.id, skin.owned, skin.cost_gems];
            }),
            store: state.store && {
                gems: state.store.gems,
                crowns: state.store.crowns == null ? state.store.laurels : state.store.crowns,
                leaders: (state.store.leaders || []).map(function (leader) {
                return [leader.id, leader.owned, leader.free_rotation, leader.cost_crowns, leader.cost_laurels, leader.cost_gems];
                }),
                skins: (state.store.skins || []).map(function (skin) {
                    return [skin.id, skin.owned, skin.cost_gems];
                }),
                gem_bundles: (state.store.gem_bundles || []).map(function (bundle) {
                    return [bundle.id, bundle.product_id, bundle.gems, bundle.asset_path];
                }),
                direct_leaders: (state.store.leaders || []).map(function (leader) {
                    return [leader.id, leader.direct_product_id, leader.direct_price_label];
                }),
                direct_skins: (state.store.skins || []).map(function (skin) {
                    return [skin.id, skin.direct_product_id, skin.direct_price_label];
                })
            },
            public_profile_id: state.public_profile_id,
            joined: state.joined_lobby_id,
            pending: state.pending_lobby_id,
            host: state.is_lobby_host,
            my_player_id: state.my_player_id,
            private_game: state.custom_game_is_private,
            single_player: state.custom_game_is_sp,
            downloading: state.is_downloading_map,
            download_name: state.downloading_map_name,
            download_progress: state.map_download_progress,
            error: state.error,
            notice: state.notice,
            maps: (state.map_catalog || []).map(function (map) {
                return [map.key, map.display_name, map.width, map.height];
            })
        });
    }

    var displayNameDraft = null;
