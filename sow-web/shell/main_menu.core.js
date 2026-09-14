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
    var authEmail = "";
    var authCode = "";
    var authOtpSent = false;
    var authBusy = false;
    var authError = "";
    var authNotice = "";
    var settingsOpen = false;
    var profileOpen = false;
    var profileAccountId = null;
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
    var campaignOpen = false;
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
    /* POKI_STRIPE_STATE_BEGIN */
    var stripePromise = null;
    /* POKI_STRIPE_STATE_END */
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

    var MAP_DISPLAY_NAMES = {
        africa: "Africa",
        asia: "Asia",
        bajacalifornia: "Baja California",
        eastanglia: "East Anglia",
        eastasia: "East Asia",
        europe: "Europe",
        indiansubcontinent: "Indian Subcontinent",
        mena: "MENA",
        middleeast: "Middle East",
        northamerica: "North America",
        oceania: "Oceania",
        pangaea: "Pangaea",
        southamerica: "South America",
        southeastasia: "Southeast Asia",
        world: "World"
    };

    function formatMapName(value) {
        var key = "";
        var displayName = "";
        if (value && typeof value === "object") {
            key = value.map_name || value.key || "";
            displayName = value.display_name || "";
        } else {
            key = value || "";
        }
        key = String(key || "world").trim().toLowerCase();
        if (MAP_DISPLAY_NAMES[key]) return MAP_DISPLAY_NAMES[key];

        var info = mapInfo(key);
        var label = String(displayName || info.display_name || "").trim();
        if (label && label.toLowerCase() !== key) return label;
        return key
            .replace(/[-_]+/g, " ")
            .replace(/\b[a-z]/g, function (letter) { return letter.toUpperCase(); }) || "World";
    }

    function findLobby(id) {
        return (state && state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    function currentScreen() {
        if (!state) return "boot";
        if (profileOpen) return "profile";
        if (heroesOpen) return "heroes";
        if (storeOpen) return "store";
        if (campaignOpen) return "campaign";
        if (state.waiting) return "queue";
        if (state.show_create) return "create";
        if (state.show_browser) return "browser";
        return "home";
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
            campaign: state.campaign,
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
            account_id: state.account_id,
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
