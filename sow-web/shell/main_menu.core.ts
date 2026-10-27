(function () {
    "use strict";

    const root = document.getElementById("sow-menu");
    if (!root) return;

    let state = null;
    let lastRaw = "";
    let lastRenderKey = "";
    let previousScreen = null;
    let lastMenuPhase = null;
    let pendingExitScreenIntro = false;
    let tempSelectedLeader = null;
    let tempSelectedRandom = false;
    let browserSearchQuery = "";
    let heroesSearchQuery = "";
    let heroesRegionFilter = "all";
    let dropdownOpenKey = null;
    let authModalOpen = false;
    let authEmail = "";
    let authCode = "";
    let authOtpSent = false;
    let authBusy = false;
    let authError = "";
    let authNotice = "";
    let settingsOpen = false;
    let signOutConfirmOpen = false;
    let profileOpen = false;
    let profileAccountId = null;
    let profileTab = "overview";
    let profileData = null;
    let profileHistory = [];
    let profileMatchDetail = null;
    const profileCache = Object.create(null);
    let profileSnapshotLoading = false;
    let profileHistoryLoading = false;
    let profileDetailLoading = false;
    let profileDetailError = "";
    let profileDetailId = null;
    let profileDetailRequestKey = null;
    let profileOwnId = null;
    let profileLastPhase = null;
    let storeOpen = false;
    let skinPickerOpen = false;
    let heroInfoOpen = false;
    let heroesOpen = false;
    let campaignOpen = false;
    let createDraft = null;
    let createOffline = false;
    let createPrivate = false;
    let createPassword = "";
    let passwordLobbyId = null;
    let passwordDraft = "";
    let storeCheckoutRequestId = null;
    let storeCheckoutBusy = false;
    let purchaseModal = null;
    let purchaseIntent = null;
    let storeCatalogLoading = false;
    let storeCatalogLoaded = false;
    let storeCatalogRetryAt = 0;
    const pendingCommands = [];
    let pendingLeaderSelection = null;

    // Line icons live in main_menu.hud.ts (loaded after the menu, used at render time).
    function menuIcon(name) {
        return window.SOW_hudIcon ? window.SOW_hudIcon(name) : "";
    }

    function esc(value) {
        return String(value == null ? "" : value)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    function localizedText(value) {
        if (value && typeof value === "object" && value.key) {
            return SOW_t(value.key, value.values || {});
        }
        return String(value == null ? "" : value);
    }

    function selfCreds() {
        let id = null;
        let secret = null;
        try {
            id = window.localStorage.getItem("sow_account_id");
            secret = window.localStorage.getItem("sow_account_secret");
        } catch {}
        return (id && secret) ? { account_id: id, auth_secret: secret } : null;
    }

    function normalizeLeaderSelectionId(value) {
        return String(value || "").replace(/[^a-z0-9]/gi, "").toLowerCase();
    }

    function leaderSelectionMatches(selection, preferRandom, leaderId) {
        if (!selection || !!selection.prefer_random_leader !== !!preferRandom) return false;
        return !!preferRandom || normalizeLeaderSelectionId(selection.selected_leader) ===
            normalizeLeaderSelectionId(leaderId);
    }

    function currentLeaderSelection() {
        const accountId = String(state && state.account_id || "");
        if (pendingLeaderSelection && pendingLeaderSelection.account_id === accountId) {
            return pendingLeaderSelection;
        }
        const preferRandom = !!(state && state.prefer_random_leader);
        return {
            account_id: accountId,
            selected_leader: preferRandom ? null : (state && state.selected_leader || "Caesar"),
            prefer_random_leader: preferRandom
        };
    }

    function stageLeaderSelection(preferRandom, leaderId) {
        const accountId = String(state && state.account_id || "");
        if (pendingLeaderSelection && pendingLeaderSelection.account_id !== accountId) {
            pendingLeaderSelection = null;
        }
        if (leaderSelectionMatches(currentLeaderSelection(), preferRandom, leaderId)) return;
        pendingLeaderSelection = {
            account_id: accountId,
            selected_leader: preferRandom ? null : leaderId,
            prefer_random_leader: !!preferRandom
        };
    }

    function reconcileLeaderSelection(nextState) {
        if (!pendingLeaderSelection) return;
        const accountId = String(nextState && nextState.account_id || "");
        if (pendingLeaderSelection.account_id !== accountId ||
            leaderSelectionMatches(nextState, pendingLeaderSelection.prefer_random_leader,
                pendingLeaderSelection.selected_leader)) {
            pendingLeaderSelection = null;
        }
    }

    function asset(path) {
        const base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        const url = base + "/" + path.split("/").map(encodeURIComponent).join("/");
        // One versioning owner: loader.js maps heavy art to content hashes and
        // stamps everything else with the build, exactly like ?v= on the HTML.
        return window.SOW_assetUrl ? window.SOW_assetUrl(url) : url;
    }

    function leaderArtUrl(slug, variant?) {
        return window.SOW_leaderArtUrl(slug, variant);
    }

    function prepareLeaderArt(slugOrLeader, fetchPriority?) {
        const leader = typeof slugOrLeader === "string" ? leaderById(slugOrLeader) : slugOrLeader;
        return leader ? window.SOW_prepareLeaderArt(leader.slug, fetchPriority) : Promise.resolve(false);
    }

    const MAIN_NAV_ITEMS = [
        ["store", "shell/mobile-nav/store.webp", "menu.shop"],
        ["heroes", "shell/mobile-nav/heroes.webp", "menu.heroes"],
        ["battle", "shell/mobile-nav/battle.webp", "menu.battle"],
        ["profile", "shell/mobile-nav/profile.webp", "menu.profile"]
    ];

    function renderMainNavMarkup(active, items) {
        return "<nav class='sow-menu__main-nav' aria-label='" + esc(SOW_t("menu.main_menu_navigation")) + "'>" + items.map(function (item) {
            const selected = active === item[0];
            const label = SOW_t(item[2]);
            return "<button type='button' class='sow-menu__main-nav-item" + (selected ? " is-active" : "") + "' data-command='main_nav' data-nav-screen='" + item[0] + "'" +
                (selected ? " aria-current='page'" : "") + " aria-label='" + esc(label) + "'><span aria-hidden='true'><img src='" + esc(asset(item[1])) + "' alt='' width='128' height='128' decoding='async' draggable='false'></span><small>" + esc(label) + "</small></button>";
        }).join("") + "</nav>";
    }

    function currencyAsset(kind) {
        return asset("gameplay/currency/" + kind + ".webp");
    }

    function leaderById(id) {
        const leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        const normalize = function (value) { return String(value || "").replace(/[^a-z0-9]/gi, "").toLowerCase(); };
        const wanted = normalize(id);
        const found = leaders.find(function (leader) { return normalize(leader.id) === wanted; });
        if (found) return found;
        return leaders[0] || {
            id: "Caesar", name: "Caesar", civilization_key: "heroes.leader_caesar_civilization", perk_key: "site.leader_caesar_description", slug: "caesar", region: "europe"
        };
    }

    function leaderPerk(leader) {
        const key = leader && leader.perk_key;
        if (key) {
            const value = SOW_t(key);
            if (value && value !== "[" + key + "]") return value;
        }
        return SOW_t("heroes.enhanced_bonuses");
    }

    function leaderCivilization(leader) {
        const key = leader && leader.civilization_key;
        if (key) {
            const value = SOW_t(key);
            if (value && value !== "[" + key + "]") return value;
        }
        return leader && leader.civilization ? leader.civilization : "";
    }

    function leaderTranslationSlug(leader) {
        return leader && leader.slug ? String(leader.slug) : "";
    }

    function leaderDisplayName(leader) {
        const slug = leaderTranslationSlug(leader);
        const key = slug ? "heroes.leader_" + slug + "_name" : "";
        if (key) {
            const value = SOW_t(key);
            if (value && value !== "[" + key + "]") return value;
        }
        return leader && leader.name ? leader.name : "";
    }

    function leaderHistoricalName(leader) {
        const slug = leaderTranslationSlug(leader);
        const key = slug ? "heroes.leader_" + slug + "_historical" : "";
        if (key) {
            const value = SOW_t(key);
            if (value && value !== "[" + key + "]") {
                const displayName = leaderDisplayName(leader);
                return value === displayName ? "" : value;
            }
        }
        return "";
    }

    function heroBriefingLeader(leaderOrSlug) {
        if (leaderOrSlug && typeof leaderOrSlug === "object") return leaderOrSlug;
        const slug = String(leaderOrSlug || "").trim().toLowerCase();
        if (slug === "random") return { id: "random", slug: "random" };
        if (!/^[a-z0-9_]+$/.test(slug)) return null;
        const leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        return leaders.find(function (leader) { return leader && leader.slug === slug; }) || {
            id: slug,
            slug: slug,
            name: slug.split("_").map(function (part) { return part.charAt(0).toUpperCase() + part.slice(1); }).join(" "),
            civilization_key: "heroes.leader_" + slug + "_civilization",
            perk_key: "site.leader_" + slug + "_description"
        };
    }

    function heroBriefingText(key) {
        if (!key || typeof SOW_t !== "function") return "";
        const value = String(SOW_t(key) || "").trim();
        return value && value !== key && value !== "[" + key + "]" ? value : "";
    }

    function renderHeroBriefing(leaderOrSlug, surface, headingId) {
        const leader = heroBriefingLeader(leaderOrSlug);
        if (!leader) return "";
        const slug = leader.slug || "";
        const random = slug === "random" || String(leader.id || "").toLowerCase() === "random";
        const name = random
            ? heroBriefingText("heroes.random_leader")
            : heroBriefingText("heroes.leader_" + slug + "_name") || leader.name || "";
        const civilization = random ? "" :
            heroBriefingText(leader.civilization_key || "heroes.leader_" + slug + "_civilization") || leader.civilization || "";
        const abilityName = random ? "" : heroBriefingText("site.leader_" + slug + "_ability");
        const description = random
            ? heroBriefingText("heroes.random_leader_description")
            : heroBriefingText(leader.perk_key || "site.leader_" + slug + "_description") ||
                heroBriefingText("site.leader_" + slug + "_description") || heroBriefingText("heroes.enhanced_bonuses");
        const surfaceName = ["home", "selection", "modal", "loader"].indexOf(surface) !== -1 ? surface : "default";
        const id = /^[a-z][a-z0-9_-]*$/.test(String(headingId || "")) ? " id='" + headingId + "'" : "";
        const identity = (civilization ? "<small class='sow-heroes__civilization sow-hero-briefing__civilization'>" + esc(civilization) + "</small>" : "") +
            (name ? "<h2" + id + " class='sow-hero-briefing__name'>" + esc(name) + "</h2>" : "");
        let ability = "";
        if (random) {
            if (description) ability = "<div class='sow-hero-briefing__random-copy'><p class='sow-hero-briefing__description'>" + esc(description) + "</p></div>";
        } else if (abilityName || description) {
            const icon = slug && /^[a-z0-9_]+$/.test(slug)
                ? "<span class='sow-hero-briefing__icon-slot'><img class='sow-heroes__ability-icon sow-hero-briefing__icon' src='" + esc(asset("gameplay/icons/abilities/" + slug + ".webp")) + "' alt='' decoding='async'></span>"
                : "";
            ability = "<div class='sow-heroes__perk-row sow-hero-briefing__ability'>" + icon +
                "<div class='sow-hero-briefing__ability-copy'>" +
                (abilityName ? "<h3 class='sow-hero-briefing__ability-name'>" + esc(abilityName) + "</h3>" : "") +
                (description ? "<p class='sow-heroes__perk sow-hero-briefing__description'>" + esc(description) + "</p>" : "") +
                "</div></div>";
        }
        return "<section class='sow-hero-briefing sow-hero-briefing--" + surfaceName + (random ? " sow-hero-briefing--random" : "") + "' data-hero-briefing-surface='" + surfaceName + "' aria-label='" + esc(name) + "'>" +
            (identity ? "<div class='sow-hero-briefing__identity'>" + identity + "</div>" : "") + ability +
            "</section>";
    }

    window.SOW_renderHeroBriefing = renderHeroBriefing;

    function hideBrokenHeroBriefingIcon(event) {
        const image = event && event.target;
        const slot = image && image.parentNode;
        if (image && image.classList && image.classList.contains("sow-hero-briefing__icon") &&
            slot && slot.classList && slot.classList.contains("sow-hero-briefing__icon-slot")) {
            slot.remove();
        }
    }
    document.addEventListener("error", hideBrokenHeroBriefingIcon, true);

    function mapInfo(key) {
        const catalog = state && Array.isArray(state.map_catalog) ? state.map_catalog : [];
        const found = catalog.find(function (m) { return m.key === key; });
        if (found) return found;
        return { key: key || "world", display_name: (key || "WORLD MAP").toUpperCase(), width: 2048, height: 1024 };
    }

    const MAP_DISPLAY_NAMES = {
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
        let key: string;
        let displayName = "";
        if (value && typeof value === "object") {
            key = value.map_name || value.key || "";
            displayName = value.display_name || "";
        } else {
            key = value || "";
        }
        key = String(key || "world").trim().toLowerCase();
        if (MAP_DISPLAY_NAMES[key]) return MAP_DISPLAY_NAMES[key];

        const info = mapInfo(key);
        const label = String(displayName || info.display_name || "").trim();
        if (label && label.toLowerCase() !== key) return label;
        return key
            .replace(/[-_]+/g, " ")
            .replace(/\b[a-z]/g, function (letter) { return letter.toUpperCase(); }) || "World";
    }

    function findLobby(id) {
        return (state && state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    // Email and social login are offered only on the owned site and Android;
    // Poki, Jest and CrazyGames supply their own account system.
    function portalOffersSignIn() {
        return ["poki", "jest", "crazygames"].indexOf(window.SOW_PORTAL) === -1;
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

    function send(type, extra?) {
        const command = Object.assign({ type: type }, extra || {});
        const serialized = JSON.stringify(command);
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

    function heroImage(leaderId?, variant?) {
        const selected = arguments.length === 0;
        const selection = selected ? currentLeaderSelection() : null;
        if (selected && selection.prefer_random_leader && state && state.phase === "MainMenu" &&
            state.loader_job !== "EnterGame" && state.loader_job !== "ExitGame") {
            window.SOW_prepareLeaderArt("random", state.loader_job === "Boot" ? "low" : "high");
            return leaderArtUrl("random");
        }
        const leader = arguments.length
            ? leaderById(leaderId || "Caesar")
            : leaderById(selection.selected_leader);
        if (selected && state && state.phase === "MainMenu" &&
            state.loader_job !== "EnterGame" && state.loader_job !== "ExitGame") {
            prepareLeaderArt(leader.slug, state.loader_job === "Boot" ? "low" : "high");
        }
        if (variant) return leaderArtUrl(leader.slug, variant);
        return leaderArtUrl(leader.slug);
    }

    // OWNER DECISION: the profile identity banner always uses landscape art,
    // even on portrait screens. It is a wide banner; never use the viewport
    // default here or the tall image will be cropped into the profile header.
    function profileHeroImage(leaderId) {
        return heroImage(leaderId, "desktop");
    }

    function avatarImage(leaderId?) {
        const selected = arguments.length === 0;
        const selection = selected ? currentLeaderSelection() : null;
        if (arguments.length && !leaderId) return asset("gameplay/avatars/random.webp");
        if (selected && selection.prefer_random_leader) return asset("gameplay/avatars/random.webp");
        if (arguments.length) {
            const explicitLeader = leaderById(leaderId);
            return asset("gameplay/avatars/" + explicitLeader.slug + ".webp");
        }
        const leader = leaderById(selection.selected_leader);
        return asset("gameplay/avatars/" + leader.slug + ".webp");
    }

    function lobbyThumb(lobby) {
        const base = String(window.SOW_MAPS_URL || "/maps").replace(/\/$/, "");
        const bust = String(window.SOW_MAPS_CACHE_BUST || window.SOW_BUILD_TS || "");
        const mapKey = String((lobby && lobby.map_name) || "world").trim().toLowerCase();
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
            leader: state.selected_leader,
            random_leader: state.prefer_random_leader,
            gems: state.gems,
            crowns: state.crowns,
            laurels: state.laurels,
            reward_receipts: (state.reward_receipts || []).map(function (receipt) {
                return [receipt.id, receipt.xp, receipt.leader_xp, receipt.crowns, receipt.laurels,
                    receipt.unlocked_leaders, receipt.status, receipt.presented_at, receipt.verification_status];
            }),
            selected_skin: state.selected_skin,
            store_busy: state.store_busy,
            skins: (state.store && state.store.skins || []).map(function (skin) {
                return [skin.id, skin.owned, skin.cost_gems];
            }),
            store: state.store && {
                gems: state.store.gems,
                crowns: state.store.crowns,
                rotation_ends_at: state.store.rotation_ends_at,
                leaders: (state.store.leaders || []).map(function (leader) {
                    return [leader.id, leader.owned, leader.free_rotation, leader.permanently_free,
                        leader.available, leader.cost_crowns, leader.cost_gems];
                }),
                skins: (state.store.skins || []).map(function (skin) {
                    return [skin.id, skin.owned, skin.cost_gems];
                }),
                gem_bundles: (state.store.gem_bundles || []).map(function (bundle) {
                    return [bundle.id, bundle.product_id, bundle.gems, bundle.asset_path,
                        bundle.bonus_percent];
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
            download_error: state.map_download_error,
            match_preparing: state.match_preparing,
            error: state.error,
            notice: state.notice,
            maps: (state.map_catalog || []).map(function (map) {
                return [map.key, map.display_name, map.width, map.height, map.default_roster,
                    (map.roster_presets || []).map(function (preset) { return [preset.id, preset.name]; })];
            })
        });
    }
