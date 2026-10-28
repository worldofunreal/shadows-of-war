import { rankEmblem, rankLabel } from "./main_menu.ranked.ts";
import type { LocalizedText, MenuLeader, MenuLobby, MenuMap, MenuState, RankedLadder } from "./main_menu.state.ts";
import type {
    ProfileCacheEntry,
    ProfileData,
    ProfileMatch,
    ProfileMatchDetail,
    ProfileRating,
} from "./main_menu.profile.ts";

export interface PurchaseIntent {
    type: "bundle" | "leader" | "skin";
    productId?: string;
    leaderId?: string;
    skinId?: string;
}

export interface PurchaseModal {
    type: "bundle" | "product" | "leader" | "skin";
    phase: "confirm" | "processing" | "spending" | "success";
    productId?: string;
    bundleId?: string;
    gems?: number;
    leaderId?: string;
    skinId?: string;
    currency?: string;
    submitted: boolean;
    timedOut?: boolean;
    gemBaseline?: number;
    gemPresented?: boolean;
    gemFlightStarted?: boolean;
}

export const root = document.getElementById("sow-menu") as HTMLElement;

interface LeaderSelection {
    account_id: string;
    selected_leader: string | null;
    prefer_random_leader: boolean;
}

interface MenuUi {
    state: MenuState | null;
    lastRaw: string;
    lastRenderKey: string;
    previousScreen: string | null;
    lastMenuPhase: string | null;
    pendingExitScreenIntro: boolean;
    tempSelectedLeader: string | null;
    tempSelectedRandom: boolean;
    browserSearchQuery: string;
    heroesSearchQuery: string;
    heroesRegionFilter: string;
    dropdownOpenKey: string | null;
    authModalOpen: boolean;
    authEmail: string;
    authCode: string;
    authOtpSent: boolean;
    authBusy: boolean;
    authError: string;
    authNotice: string;
    settingsOpen: boolean;
    signOutConfirmOpen: boolean;
    profileOpen: boolean;
    profileAccountId: string | null;
    profileTab: string;
    profileData: ProfileData | null;
    rankedRating: ProfileRating | null;
    rankedLadder: RankedLadder | null;
    profileHistory: ProfileMatch[];
    profileMatchDetail: ProfileMatchDetail | null;
    profileSnapshotLoading: boolean;
    profileHistoryLoading: boolean;
    profileDetailLoading: boolean;
    profileDetailError: string;
    profileDetailId: string | null;
    profileDetailRequestKey: string | null;
    profileOwnId: string | null;
    profileLastPhase: string | null;
    storeOpen: boolean;
    skinPickerOpen: boolean;
    heroInfoOpen: boolean;
    heroesOpen: boolean;
    campaignOpen: boolean;
    createDraft: MenuState["custom_game_config"] | null;
    createOffline: boolean;
    createPrivate: boolean;
    createPassword: string;
    passwordLobbyId: number | null;
    passwordDraft: string;
    storeCheckoutRequestId: string | null;
    storeCheckoutBusy: boolean;
    purchaseModal: PurchaseModal | null;
    purchaseIntent: PurchaseIntent | null;
    storeCatalogLoading: boolean;
    storeCatalogLoaded: boolean;
    storeCatalogRetryAt: number;
    reportOpen: boolean;
    reportTarget: string | null;
    reportSent: boolean;
    deleteArmed: boolean;
}

export const menuUi: MenuUi = {
    state: null,
    lastRaw: "",
    lastRenderKey: "",
    previousScreen: null,
    lastMenuPhase: null,
    pendingExitScreenIntro: false,
    tempSelectedLeader: null,
    tempSelectedRandom: false,
    browserSearchQuery: "",
    heroesSearchQuery: "",
    heroesRegionFilter: "all",
    dropdownOpenKey: null,
    authModalOpen: false,
    authEmail: "",
    authCode: "",
    authOtpSent: false,
    authBusy: false,
    authError: "",
    authNotice: "",
    settingsOpen: false,
    signOutConfirmOpen: false,
    profileOpen: false,
    profileAccountId: null,
    profileTab: "overview",
    profileData: null,
    rankedRating: null,
    rankedLadder: null,
    profileHistory: [],
    profileMatchDetail: null,
    profileSnapshotLoading: false,
    profileHistoryLoading: false,
    profileDetailLoading: false,
    profileDetailError: "",
    profileDetailId: null,
    profileDetailRequestKey: null,
    profileOwnId: null,
    profileLastPhase: null,
    storeOpen: false,
    skinPickerOpen: false,
    heroInfoOpen: false,
    heroesOpen: false,
    campaignOpen: false,
    createDraft: null,
    createOffline: false,
    createPrivate: false,
    createPassword: "",
    passwordLobbyId: null,
    passwordDraft: "",
    storeCheckoutRequestId: null,
    storeCheckoutBusy: false,
    purchaseModal: null,
    purchaseIntent: null,
    storeCatalogLoading: false,
    storeCatalogLoaded: false,
    storeCatalogRetryAt: 0,
    reportOpen: false,
    reportTarget: null,
    reportSent: false,
    deleteArmed: false,
};

export const profileCache: Record<string, ProfileCacheEntry> = Object.create(null);

const pendingCommands: string[] = [];
let pendingLeaderSelection: LeaderSelection | null = null;

// Line icons live in main_menu.hud.ts (loaded after the menu, used at render time).
export function menuIcon(name: string): string {
    return window.SOW_hudIcon ? window.SOW_hudIcon(name) : "";
}

export function esc(value: unknown): string {
    return String(value == null ? "" : value)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

export function renderQuickMatchButton(): string {
    const ranked = menuUi.rankedRating;
    const rankedLabel = rankLabel(ranked);
    return (
        "<button class='sow-menu__primary sow-menu__quick-match' type='button' data-command='quick_match'><div><strong>" +
        "<img class='sow-ranked__mode-icon' src='" + esc(asset("shell/ranked/ranked.svg")) + "' alt=''>" + esc(SOW_t("menu.quick_match")) +
        "</strong><small>" +
        esc(SOW_t("lobbies.ranked_1v1")) +
        " · <b data-ranked-current>" +
        esc(rankedLabel) +
        "</b></small></div><span data-ranked-emblem>" + rankEmblem(ranked?.placements_complete ? ranked.tier : "unranked", ranked?.placements_complete ? ranked.division : null) + "</span></button>" +
        "<button class='sow-menu__ghost-button sow-ranked__my-rank' type='button' data-command='open_ranked_profile'>" + esc(SOW_t("ranked.my_rank")) + "</button>"
    );
}

export function localizedText(value: LocalizedText | null | undefined): string {
    if (value && typeof value === "object" && value.key) {
        return SOW_t(value.key, value.values || {});
    }
    return String(value == null ? "" : value);
}

export function selfCreds() {
    let id: string | null = null;
    let secret: string | null = null;
    try {
        id = window.localStorage.getItem("sow_account_id");
        secret = window.localStorage.getItem("sow_account_secret");
    } catch {}
    return id && secret ? { account_id: id, auth_secret: secret } : null;
}

function normalizeLeaderSelectionId(value: unknown): string {
    return String(value || "")
        .replace(/[^a-z0-9]/gi, "")
        .toLowerCase();
}

function leaderSelectionMatches(
    selection: Pick<LeaderSelection, "prefer_random_leader" | "selected_leader"> | null,
    preferRandom: boolean | undefined,
    leaderId: string | null | undefined,
): boolean {
    if (!selection || !!selection.prefer_random_leader !== !!preferRandom) return false;
    return (
        !!preferRandom ||
        normalizeLeaderSelectionId(selection.selected_leader) === normalizeLeaderSelectionId(leaderId)
    );
}

export function currentLeaderSelection(): LeaderSelection {
    const accountId = String((menuUi.state && menuUi.state.account_id) || "");
    if (pendingLeaderSelection && pendingLeaderSelection.account_id === accountId) {
        return pendingLeaderSelection;
    }
    const preferRandom = !!(menuUi.state && menuUi.state.prefer_random_leader);
    return {
        account_id: accountId,
        selected_leader: preferRandom ? null : (menuUi.state && menuUi.state.selected_leader) || "Caesar",
        prefer_random_leader: preferRandom,
    };
}

export function stageLeaderSelection(preferRandom: boolean, leaderId: string | null): void {
    const accountId = String((menuUi.state && menuUi.state.account_id) || "");
    if (pendingLeaderSelection && pendingLeaderSelection.account_id !== accountId) {
        pendingLeaderSelection = null;
    }
    if (leaderSelectionMatches(currentLeaderSelection(), preferRandom, leaderId)) return;
    pendingLeaderSelection = {
        account_id: accountId,
        selected_leader: preferRandom ? null : leaderId,
        prefer_random_leader: !!preferRandom,
    };
}

export function reconcileLeaderSelection(nextState: MenuState | null): void {
    if (!pendingLeaderSelection) return;
    const accountId = String((nextState && nextState.account_id) || "");
    if (
        pendingLeaderSelection.account_id !== accountId ||
        leaderSelectionMatches(
            nextState,
            pendingLeaderSelection.prefer_random_leader,
            pendingLeaderSelection.selected_leader,
        )
    ) {
        pendingLeaderSelection = null;
    }
}

export function asset(path: string): string {
    const base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
    const url = base + "/" + path.split("/").map(encodeURIComponent).join("/");
    // One versioning owner: loader.js maps heavy art to content hashes and
    // stamps everything else with the build, exactly like ?v= on the HTML.
    return window.SOW_assetUrl ? window.SOW_assetUrl(url) : url;
}

export function leaderArtUrl(slug: string, variant?: string): string {
    return window.SOW_leaderArtUrl(slug, variant);
}

export function prepareLeaderArt(
    slugOrLeader: string | MenuLeader,
    fetchPriority?: string,
): Promise<boolean> {
    const leader = typeof slugOrLeader === "string" ? leaderById(slugOrLeader) : slugOrLeader;
    const slug =
        typeof slugOrLeader === "string" && slugOrLeader.toLowerCase() === "random"
            ? "random"
            : leaderArtSlug(leader);
    return slug ? window.SOW_prepareLeaderArt(slug, fetchPriority) : Promise.resolve(false);
}

export const MAIN_NAV_ITEMS = [
    ["store", "shell/mobile-nav/store.webp", "menu.shop"],
    ["heroes", "shell/mobile-nav/heroes.webp", "menu.heroes"],
    ["battle", "shell/mobile-nav/battle.webp", "menu.battle"],
    ["profile", "shell/mobile-nav/profile.webp", "menu.profile"],
];

export function renderMainNavMarkup(active: string | null | undefined, items: string[][]): string {
    return (
        "<nav class='sow-menu__main-nav' aria-label='" +
        esc(SOW_t("menu.main_menu_navigation")) +
        "'>" +
        items
            .map(function (item) {
                const selected = active === item[0];
                const label = SOW_t(item[2]);
                return (
                    "<button type='button' class='sow-menu__main-nav-item" +
                    (selected ? " is-active" : "") +
                    "' data-command='main_nav' data-nav-screen='" +
                    item[0] +
                    "'" +
                    (selected ? " aria-current='page'" : "") +
                    " aria-label='" +
                    esc(label) +
                    "'><span aria-hidden='true'><img src='" +
                    esc(asset(item[1])) +
                    "' alt='' width='128' height='128' decoding='async' draggable='false'></span><small>" +
                    esc(label) +
                    "</small></button>"
                );
            })
            .join("") +
        "</nav>"
    );
}

export function currencyAsset(kind: string): string {
    return asset("gameplay/currency/" + kind + ".webp");
}

export function leaderById(id: unknown): MenuLeader {
    const leaders: MenuLeader[] =
        menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    const normalize = function (value: unknown) {
        return String(value || "")
            .replace(/[^a-z0-9]/gi, "")
            .toLowerCase();
    };
    const wanted = normalize(id);
    const found = leaders.find(function (leader) {
        return normalize(leader.id) === wanted;
    });
    if (found) return found;
    return (
        leaders[0] || {
            id: "Caesar",
            name: "Caesar",
            civilization_key: "heroes.leader_caesar_civilization",
            perk_key: "site.leader_caesar_description",
            slug: "caesar",
            region: "europe",
        }
    );
}

export function leaderArtSlug(leader: MenuLeader | null | undefined): string {
    return String((leader && (leader.art_slug || leader.slug)) || "");
}

export function leaderPerk(leader: MenuLeader | null | undefined): string {
    const key = leader && leader.perk_key;
    if (key) {
        const value = SOW_t(key);
        if (value && value !== "[" + key + "]") return value;
    }
    return SOW_t("heroes.enhanced_bonuses");
}

export function leaderCivilization(leader: MenuLeader | null | undefined): string {
    const key = leader && leader.civilization_key;
    if (key) {
        const value = SOW_t(key);
        if (value && value !== "[" + key + "]") return value;
    }
    return leader && leader.civilization ? leader.civilization : "";
}

function leaderTranslationSlug(leader: MenuLeader | null | undefined): string {
    return leader && leader.slug ? String(leader.slug) : "";
}

export function leaderDisplayName(leader: MenuLeader | null | undefined): string {
    const slug = leaderTranslationSlug(leader);
    const key = slug ? "heroes.leader_" + slug + "_name" : "";
    if (key) {
        const value = SOW_t(key);
        if (value && value !== "[" + key + "]") return value;
    }
    return leader && leader.name ? leader.name : "";
}

export function leaderHistoricalName(leader: MenuLeader | null | undefined): string {
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

function heroBriefingLeader(leaderOrSlug: unknown): MenuLeader | null {
    if (leaderOrSlug && typeof leaderOrSlug === "object") return leaderOrSlug as MenuLeader;
    const slug = String(leaderOrSlug || "")
        .trim()
        .toLowerCase();
    if (slug === "random") return { id: "random", slug: "random" };
    if (!/^[a-z0-9_]+$/.test(slug)) return null;
    const leaders: MenuLeader[] =
        menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    return (
        leaders.find(function (leader) {
            return leader && leader.slug === slug;
        }) || {
            id: slug,
            slug: slug,
            name: slug
                .split("_")
                .map(function (part) {
                    return part.charAt(0).toUpperCase() + part.slice(1);
                })
                .join(" "),
            civilization_key: "heroes.leader_" + slug + "_civilization",
            perk_key: "site.leader_" + slug + "_description",
        }
    );
}

function heroBriefingText(key: string | null | undefined): string {
    if (!key || typeof SOW_t !== "function") return "";
    const value = String(SOW_t(key) || "").trim();
    return value && value !== key && value !== "[" + key + "]" ? value : "";
}

export function renderHeroBriefing(leaderOrSlug: unknown, surface?: string, headingId?: string): string {
    const leader = heroBriefingLeader(leaderOrSlug);
    if (!leader) return "";
    const slug = leader.slug || "";
    const random = slug === "random" || String(leader.id || "").toLowerCase() === "random";
    const name = random
        ? heroBriefingText("heroes.random_leader")
        : heroBriefingText("heroes.leader_" + slug + "_name") || leader.name || "";
    const civilization = random
        ? ""
        : heroBriefingText(leader.civilization_key || "heroes.leader_" + slug + "_civilization") ||
          leader.civilization ||
          "";
    const abilityName = random ? "" : heroBriefingText("site.leader_" + slug + "_ability");
    const description = random
        ? heroBriefingText("heroes.random_leader_description")
        : heroBriefingText(leader.perk_key || "site.leader_" + slug + "_description") ||
          heroBriefingText("site.leader_" + slug + "_description") ||
          heroBriefingText("heroes.enhanced_bonuses");
    const surfaceName =
        ["home", "selection", "modal", "loader"].indexOf(surface || "") !== -1 ? surface : "default";
    const id = /^[a-z][a-z0-9_-]*$/.test(String(headingId || "")) ? " id='" + headingId + "'" : "";
    const identity =
        (civilization
            ? "<small class='sow-heroes__civilization sow-hero-briefing__civilization'>" +
              esc(civilization) +
              "</small>"
            : "") + (name ? "<h2" + id + " class='sow-hero-briefing__name'>" + esc(name) + "</h2>" : "");
    let ability = "";
    if (random) {
        if (description)
            ability =
                "<div class='sow-hero-briefing__random-copy'><p class='sow-hero-briefing__description'>" +
                esc(description) +
                "</p></div>";
    } else if (abilityName || description) {
        const artSlug = leaderArtSlug(leader) || slug;
        const icon =
            artSlug && /^[a-z0-9_]+$/.test(artSlug)
                ? "<span class='sow-hero-briefing__icon-slot'><img class='sow-heroes__ability-icon sow-hero-briefing__icon' src='" +
                  esc(asset("gameplay/icons/abilities/" + artSlug + ".webp")) +
                  "' alt='' decoding='async'></span>"
                : "";
        ability =
            "<div class='sow-heroes__perk-row sow-hero-briefing__ability'>" +
            icon +
            "<div class='sow-hero-briefing__ability-copy'>" +
            (abilityName ? "<h3 class='sow-hero-briefing__ability-name'>" + esc(abilityName) + "</h3>" : "") +
            (description
                ? "<p class='sow-heroes__perk sow-hero-briefing__description'>" + esc(description) + "</p>"
                : "") +
            "</div></div>";
    }
    return (
        "<section class='sow-hero-briefing sow-hero-briefing--" +
        surfaceName +
        (random ? " sow-hero-briefing--random" : "") +
        "' data-hero-briefing-surface='" +
        surfaceName +
        "' aria-label='" +
        esc(name) +
        "'>" +
        (identity ? "<div class='sow-hero-briefing__identity'>" + identity + "</div>" : "") +
        ability +
        "</section>"
    );
}

window.SOW_renderHeroBriefing = renderHeroBriefing;

function hideBrokenHeroBriefingIcon(event: Event): void {
    const image = event && (event.target as Element | null);
    const slot = image && (image.parentNode as Element | null);
    if (
        image &&
        image.classList &&
        image.classList.contains("sow-hero-briefing__icon") &&
        slot &&
        slot.classList &&
        slot.classList.contains("sow-hero-briefing__icon-slot")
    ) {
        slot.remove();
    }
}
document.addEventListener("error", hideBrokenHeroBriefingIcon, true);

export function mapInfo(key: string | null | undefined): MenuMap {
    const catalog: MenuMap[] =
        menuUi.state && Array.isArray(menuUi.state.map_catalog) ? menuUi.state.map_catalog : [];
    const found = catalog.find(function (m) {
        return m.key === key;
    });
    if (found) return found;
    return {
        key: key || "world",
        display_name: (key || "WORLD MAP").toUpperCase(),
        width: 2048,
        height: 1024,
    };
}

const MAP_DISPLAY_NAMES: Record<string, string> = {
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
    world: "World",
};

export function formatMapName(
    value: string | { map_name?: string; key?: string; display_name?: string } | null | undefined,
): string {
    let key: string;
    let displayName = "";
    if (value && typeof value === "object") {
        key = value.map_name || value.key || "";
        displayName = value.display_name || "";
    } else {
        key = value || "";
    }
    key = String(key || "world")
        .trim()
        .toLowerCase();
    if (MAP_DISPLAY_NAMES[key]) return MAP_DISPLAY_NAMES[key];

    const info = mapInfo(key);
    const label = String(displayName || info.display_name || "").trim();
    if (label && label.toLowerCase() !== key) return label;
    return (
        key.replace(/[-_]+/g, " ").replace(/\b[a-z]/g, function (letter) {
            return letter.toUpperCase();
        }) || "World"
    );
}

export function findLobby(id: unknown): MenuLobby | null {
    const lobbies: MenuLobby[] = (menuUi.state && menuUi.state.lobbies) || [];
    return (
        lobbies.find(function (lobby) {
            return lobby.id === id;
        }) || null
    );
}

// Email and social login are offered only on the owned site and Android;
// Poki, Jest and CrazyGames supply their own account system.
export function portalOffersSignIn() {
    return ["poki", "jest", "crazygames"].indexOf(window.SOW_PORTAL) === -1;
}

export function currentScreen() {
    if (!menuUi.state) return "boot";
    if (menuUi.profileOpen) return "profile";
    if (menuUi.heroesOpen) return "heroes";
    if (menuUi.storeOpen) return "store";
    if (menuUi.campaignOpen) return "campaign";
    if (menuUi.state.waiting) return "queue";
    if (menuUi.state.show_create) return "create";
    if (menuUi.state.show_browser) return "browser";
    return "home";
}

export function send(type: string, extra?: Record<string, unknown>): boolean {
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
    while (pendingCommands.length) window.SOW_menu_command(pendingCommands.shift()!);
};

export function heroImage(leaderId?: unknown, variant?: string): string {
    const selected = arguments.length === 0;
    const selection = selected ? currentLeaderSelection() : null;
    if (
        selected &&
        selection &&
        selection.prefer_random_leader &&
        menuUi.state &&
        menuUi.state.phase === "MainMenu" &&
        menuUi.state.loader_job !== "EnterGame" &&
        menuUi.state.loader_job !== "ExitGame"
    ) {
        window.SOW_prepareLeaderArt("random", menuUi.state.loader_job === "Boot" ? "low" : "high");
        return leaderArtUrl("random");
    }
    const leader = arguments.length
        ? leaderById(leaderId || "Caesar")
        : leaderById(selection && selection.selected_leader);
    if (
        selected &&
        menuUi.state &&
        menuUi.state.phase === "MainMenu" &&
        menuUi.state.loader_job !== "EnterGame" &&
        menuUi.state.loader_job !== "ExitGame"
    ) {
        prepareLeaderArt(leader, menuUi.state.loader_job === "Boot" ? "low" : "high");
    }
    const artSlug = leaderArtSlug(leader);
    if (variant) return leaderArtUrl(artSlug, variant);
    return leaderArtUrl(artSlug);
}

// OWNER DECISION: the profile identity banner always uses landscape art,
// even on portrait screens. It is a wide banner; never use the viewport
// default here or the tall image will be cropped into the profile header.
export function profileHeroImage(leaderId: unknown): string {
    return heroImage(leaderId, "desktop");
}

export function avatarImage(leaderId?: unknown): string {
    const selected = arguments.length === 0;
    const selection = selected ? currentLeaderSelection() : null;
    if (arguments.length && !leaderId) return asset("gameplay/avatars/random.webp");
    if (selection && selection.prefer_random_leader) return asset("gameplay/avatars/random.webp");
    if (arguments.length) {
        const explicitLeader = leaderById(leaderId);
        return asset("gameplay/avatars/" + leaderArtSlug(explicitLeader) + ".webp");
    }
    const leader = leaderById(selection && selection.selected_leader);
    return asset("gameplay/avatars/" + leaderArtSlug(leader) + ".webp");
}

export function lobbyThumb(lobby: Pick<MenuLobby, "map_name"> | null | undefined): string {
    const base = String(window.SOW_MAPS_URL || "/maps").replace(/\/$/, "");
    const bust = String(window.SOW_MAPS_CACHE_BUST || window.SOW_BUILD_TS || "");
    const mapKey = String((lobby && lobby.map_name) || "world")
        .trim()
        .toLowerCase();
    return (
        base +
        "/" +
        encodeURIComponent(mapKey) +
        "/thumbnail.webp" +
        (bust ? "?v=" + encodeURIComponent(bust) : "")
    );
}

export function renderKey(): string {
    const state = menuUi.state!;
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
            return [
                receipt.id,
                receipt.xp,
                receipt.leader_xp,
                receipt.crowns,
                receipt.laurels,
                receipt.unlocked_leaders,
                receipt.status,
                receipt.presented_at,
                receipt.verification_status,
            ];
        }),
        selected_skin: state.selected_skin,
        store_busy: state.store_busy,
        skins: ((state.store && state.store.skins) || []).map(function (skin) {
            return [skin.id, skin.owned, skin.cost_gems];
        }),
        store: state.store && {
            gems: state.store.gems,
            crowns: state.store.crowns,
            rotation_ends_at: state.store.rotation_ends_at,
            leaders: (state.store.leaders || []).map(function (leader) {
                return [
                    leader.id,
                    leader.owned,
                    leader.free_rotation,
                    leader.permanently_free,
                    leader.available,
                    leader.cost_crowns,
                    leader.cost_gems,
                ];
            }),
            skins: (state.store.skins || []).map(function (skin) {
                return [skin.id, skin.owned, skin.cost_gems];
            }),
            gem_bundles: (state.store.gem_bundles || []).map(function (bundle) {
                return [bundle.id, bundle.product_id, bundle.gems, bundle.asset_path, bundle.bonus_percent];
            }),
        },
        account_id: state.account_id,
        connected: state.connected,
        update_available: state.update_available,
        joined: state.joined_lobby_id,
        pending: state.pending_lobby_id,
        host: state.is_lobby_host,
        my_player_id: state.my_player_id,
        private_game: state.custom_game_is_private,
        single_player: state.custom_game_is_sp,
        downloading: state.ranked_matchup ? null : state.downloading_map,
        download_progress: state.ranked_matchup ? null : state.map_download_progress,
        download_error: state.map_download_error,
        match_preparing: state.match_preparing,
        error: state.error,
        notice: state.notice,
        maps: (state.map_catalog || []).map(function (map) {
            return [
                map.key,
                map.display_name,
                map.width,
                map.height,
                map.default_roster,
                (map.roster_presets || []).map(function (preset) {
                    return [preset.id, preset.name];
                }),
            ];
        }),
    });
}
